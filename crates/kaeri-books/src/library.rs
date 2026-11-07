//! Reading a folder of books into series.
//!
//! The scan only reads. It opens each book to learn its page count and its own metadata
//! (ComicInfo for image books, the package metadata for EPUB), groups files into series by
//! folder, and works out each file's volume and chapter from that metadata first and its name
//! second. Nothing is written into the folder, and a file the scan met before, with the same size
//! and modification time, is not opened again.
//!
//! A series is the folder directly under the one added. A book directly in it is a series of its
//! own, named after the file. A folder that holds images and no books is itself a book.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{
    Book, ComicInfo, Format, Limits, naming,
    worker::{self, Budget},
};

/// The deepest folder the scan descends into.
const MAX_DEPTH: usize = 12;

/// How long opening one PDF may take before the scan gives up on it. The PDF reader has no way
/// to be cancelled, so a file that takes longer is left running on its own thread and set aside.
pub const PDF_BUDGET: Duration = Duration::from_secs(8);

/// One book found in a folder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScannedFile {
    /// The path as the scan met it, `/`-separated.
    pub path: String,
    pub format: String,
    pub size: u64,
    /// Modification time, seconds since the epoch.
    pub mtime: i64,
    /// Pages of an image book or PDF; `None` for a novel or a file that would not open.
    pub pages: Option<u32>,
    /// The volume as written without leading zeros.
    pub volume: Option<String>,
    /// The chapter in thousandths, when the file is one.
    pub number_milli: Option<i64>,
    /// The chapter as written: `10.5`.
    pub number: Option<String>,
    /// What to call the file when it has no number: its tidied name, or the book's own title.
    pub label: String,
    /// ComicInfo, when the book carries it.
    pub info: Option<ComicInfo>,
    /// The series the book names itself: ComicInfo's, or the EPUB's.
    pub own_series: Option<String>,
    /// The language the book names.
    pub language: Option<String>,
    /// A novel: an EPUB with reflowable text.
    pub novel: bool,
    /// Why the file would not open, if it did not. It is kept so the series can say so.
    pub broken: Option<String>,
    /// It would not open because it is locked: DRM, or a password.
    #[serde(default)]
    pub locked: bool,
}

/// The files of one series.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScannedSeries {
    /// The folder, or the file's name for a book directly in the root. Stable between scans.
    pub key: String,
    /// What the series is called: ComicInfo's or the EPUB's when the files agree, else the folder.
    pub name: String,
    pub files: Vec<ScannedFile>,
    /// Chapter numbers start again in each volume.
    pub restarts_per_volume: bool,
    /// The books say they read right to left.
    pub right_to_left: Option<bool>,
    pub novel: bool,
}

/// What a scan found.
#[derive(Debug, Default)]
pub struct Scan {
    pub series: Vec<ScannedSeries>,
    /// Files that were not books, or were empty.
    pub ignored: u32,
    /// Books that would not open. They are still in their series, marked `broken`.
    pub broken: u32,
    /// Every file the scan met, books or not, hidden ones aside.
    pub seen: u32,
    /// Books Kaeri can't open, by format (`MOBI`, `AZW3`), most first.
    pub unsupported: Vec<(String, u32)>,
    /// Books that would not open because they are locked: DRM, or a password. They are also
    /// counted in `broken`.
    pub locked: u32,
    /// The files behind `unsupported` and `locked`, for a screen that shows them; at most
    /// [`MAX_LISTED`].
    pub unsupported_files: Vec<String>,
}

/// The most unsupported files a scan lists by path.
pub const MAX_LISTED: usize = 200;

/// Book formats Kaeri can't open. A file with one of these is counted as a book that came in
/// but can't be read, not as a stray file.
pub const UNSUPPORTED_BOOKS: &[&str] = &[
    "mobi", "azw", "azw3", "azw4", "kfx", "prc", "pdb", "lit", "lrf", "fb2", "djvu", "djv",
    "cbt", "cba", "xps", "oxps", "ibooks", "chm", "kepub",
];

impl Scan {
    fn count_unsupported(&mut self, format: &str, path: &Path) {
        let format = format.to_ascii_uppercase();
        match self.unsupported.iter_mut().find(|(f, _)| *f == format) {
            Some((_, n)) => *n += 1,
            None => self.unsupported.push((format, 1)),
        }
        if self.unsupported_files.len() < MAX_LISTED {
            self.unsupported_files.push(slash(path));
        }
    }
}

/// What the scan asks of the world outside it.
pub trait Known {
    /// The file as an earlier scan saw it.
    fn file(&self, path: &str) -> Option<&ScannedFile>;
}

impl Known for HashMap<String, ScannedFile> {
    fn file(&self, path: &str) -> Option<&ScannedFile> {
        self.get(path)
    }
}

/// Nothing is known yet.
pub struct Nothing;

impl Known for Nothing {
    fn file(&self, _: &str) -> Option<&ScannedFile> {
        None
    }
}

fn mtime_of(meta: &fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

fn slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn extension(path: &Path) -> Option<String> {
    path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase)
}

fn hidden(name: &str) -> bool {
    name.starts_with('.') || name == "__MACOSX"
}

/// Scan `root`. `found` is called with the number of books seen so far, so a screen can say
/// "Scanning · 12 found".
pub fn scan(
    root: &Path,
    known: &dyn Known,
    limits: &Limits,
    found: &mut dyn FnMut(u32),
) -> std::io::Result<Scan> {
    let meta = fs::metadata(root)?;
    if !meta.is_dir() {
        return Err(std::io::Error::new(std::io::ErrorKind::NotADirectory, "not a folder"));
    }
    let mut out = Scan::default();
    let mut groups: Vec<(String, Vec<ScannedFile>)> = Vec::new();
    let mut count = 0u32;
    walk(
        root,
        root,
        0,
        known,
        limits,
        &mut |group, file| {
            count += 1;
            found(count);
            match groups.iter_mut().find(|(key, _)| *key == group) {
                Some((_, files)) => files.push(file),
                None => groups.push((group, vec![file])),
            }
        },
        &mut out,
    )?;
    for (key, files) in groups {
        out.series.push(series_of(key, files));
    }
    out.series.sort_by(|a, b| crate::natural::compare(&a.name, &b.name));
    out.broken =
        out.series.iter().flat_map(|s| &s.files).filter(|f| f.broken.is_some()).count() as u32;
    out.locked = out.series.iter().flat_map(|s| &s.files).filter(|f| f.locked).count() as u32;
    for file in out.series.iter().flat_map(|s| &s.files).filter(|f| f.locked) {
        if out.unsupported_files.len() < MAX_LISTED {
            out.unsupported_files.push(file.path.clone());
        }
    }
    out.unsupported.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    Ok(out)
}

#[allow(clippy::too_many_arguments)]
fn walk(
    root: &Path,
    dir: &Path,
    depth: usize,
    known: &dyn Known,
    limits: &Limits,
    emit: &mut dyn FnMut(String, ScannedFile),
    out: &mut Scan,
) -> std::io::Result<()> {
    let mut entries: Vec<_> = fs::read_dir(dir)?.filter_map(Result::ok).collect();
    entries.sort_by_key(fs::DirEntry::file_name);
    let images = entries
        .iter()
        .filter(|e| {
            e.path().is_file()
                && extension(&e.path())
                    .is_some_and(|x| naming::IMAGE_EXTENSIONS.contains(&x.as_str()))
        })
        .count();
    let books = entries
        .iter()
        .filter(|e| {
            e.path().is_file()
                && extension(&e.path())
                    .is_some_and(|x| naming::BOOK_EXTENSIONS.contains(&x.as_str()))
        })
        .count();
    // A folder of images and no books is a book: a chapter of its parent's series.
    if dir != root && images > 0 && books == 0 {
        let group = group_of(root, dir, true);
        if let Some(file) = scan_file(dir, root, known, limits, true) {
            emit(group, file);
        }
        return Ok(());
    }
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        if hidden(&name) {
            continue;
        }
        let path = entry.path();
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_symlink() && path.is_dir() {
            // A link to a folder could lead back into this one.
            continue;
        }
        if path.is_dir() {
            if depth < MAX_DEPTH {
                // An unreadable subfolder is skipped, not fatal to the rest.
                let _ = walk(root, &path, depth + 1, known, limits, emit, out);
            }
        } else if extension(&path)
            .is_some_and(|x| naming::BOOK_EXTENSIONS.contains(&x.as_str()))
        {
            out.seen += 1;
            match scan_file(&path, root, known, limits, false) {
                Some(file) => {
                    let group = if file.novel {
                        // A novel stands alone, or joins the series its own metadata names;
                        // the folder it sits in says nothing about which novels belong together.
                        match &file.own_series {
                            Some(series) => format!("novel:{series}"),
                            None => format!("novel:{}", naming::tidy(naming::stem(&name))),
                        }
                    } else {
                        group_of(root, &path, false)
                    };
                    emit(group, file);
                }
                None => out.ignored += 1,
            }
        } else if let Some(format) =
            extension(&path).filter(|x| UNSUPPORTED_BOOKS.contains(&x.as_str()))
        {
            out.seen += 1;
            out.count_unsupported(&format, &path);
        } else {
            out.seen += 1;
            out.ignored += 1;
        }
    }
    Ok(())
}

/// The series a file belongs to: the first folder under the root, or the file's own tidied name
/// when it sits directly in the root.
fn group_of(root: &Path, path: &Path, is_dir: bool) -> String {
    let rel = path.strip_prefix(root).unwrap_or(path);
    let parts: Vec<_> =
        rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    match parts.len() {
        0 => String::new(),
        1 => {
            let name = &parts[0];
            // A book directly in the root is a series of its own, minus its volume and chapter.
            let parsed = naming::parse(naming::stem(name));
            let _ = is_dir;
            parsed.series.unwrap_or_else(|| naming::tidy(naming::stem(name)))
        }
        _ => parts[0].clone(),
    }
}

fn scan_file(
    path: &Path,
    root: &Path,
    known: &dyn Known,
    limits: &Limits,
    folder: bool,
) -> Option<ScannedFile> {
    let meta = fs::metadata(path).ok()?;
    let text = slash(path);
    if !folder && meta.len() == 0 {
        // An empty file is a download that never finished, not a book.
        return None;
    }
    let (size, mtime) = if folder { folder_stamp(path) } else { (meta.len(), mtime_of(&meta)) };
    if let Some(old) = known.file(&text)
        && old.size == size
        && old.mtime == mtime
    {
        return Some(old.clone());
    }
    let file_name = path.file_name()?.to_string_lossy().into_owned();
    let stem = if folder { file_name.clone() } else { naming::stem(&file_name).to_owned() };
    let mut named = naming::parse(&stem);
    // A chapter inside a volume's folder: `Series/Vol. 2/ch003.cbz`.
    if named.volume.is_none()
        && let Some(parent) = path.parent().filter(|p| *p != root)
        && let Some(volume) =
            parent.file_name().and_then(|n| n.to_str()).and_then(naming::volume_folder)
    {
        named.volume = Some(volume);
    }
    let mut file = ScannedFile {
        path: text,
        format: if folder { "folder".into() } else { extension(path).unwrap_or_default() },
        size,
        mtime,
        pages: None,
        volume: named.volume,
        number_milli: named.chapter_milli,
        number: named.chapter,
        label: naming::tidy(&stem),
        info: None,
        own_series: None,
        language: None,
        novel: false,
        broken: None,
        locked: false,
    };
    open_into(&mut file, path, limits);
    Some(file)
}

/// The summed size and newest modification time of a folder's images.
fn folder_stamp(dir: &Path) -> (u64, i64) {
    let mut size = 0;
    let mut newest = 0;
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.filter_map(Result::ok) {
            if let Ok(meta) = entry.metadata() {
                size += meta.len();
                newest = newest.max(mtime_of(&meta));
            }
        }
    }
    (size, newest)
}

/// Open the book and fill in what it says of itself.
fn open_into(file: &mut ScannedFile, path: &Path, limits: &Limits) {
    let is_pdf = file.format == "pdf";
    let opened = if is_pdf {
        // The PDF reader cannot be cancelled: open it on a worker that can be left behind.
        let (path, limits) = (path.to_owned(), limits.clone());
        match worker::run(Budget::of(PDF_BUDGET), move || {
            crate::open(&path, &limits).map(|book| facts(&book))
        }) {
            Ok(result) => result,
            Err(_) => {
                file.broken = Some("opening it took too long".into());
                return;
            }
        }
    } else {
        crate::open(path, limits).map(|book| facts(&book))
    };
    match opened {
        Ok(facts) => {
            file.pages = facts.pages;
            file.format = facts.format.as_str().to_owned();
            file.novel = facts.novel;
            file.language = facts.language;
            file.own_series = facts.series;
            if let Some(info) = facts.info {
                // A name that says "volume 3" and no chapter names a whole-volume book, and
                // ComicInfo's Number then only counts the volumes.
                let volume_book = file.volume.is_some() && file.number.is_none();
                // The book's own numbers come before its name's.
                if let Some(volume) = info.volume.as_deref().and_then(clean_number) {
                    file.volume = Some(volume);
                }
                let number = info.number.as_deref().and_then(|n| naming::milli(n.trim()));
                let counts_volumes = volume_book
                    || number.as_ref().zip(file.volume.as_deref()).is_some_and(
                        |((_, shown), volume)| file.number.is_none() && shown == volume,
                    );
                if let Some((m, shown)) = number
                    && !counts_volumes
                {
                    file.number_milli = Some(m);
                    file.number = Some(shown);
                }
                file.info = Some(info);
            }
            if let Some(title) = facts.title
                && file.number.is_none()
            {
                file.label = title;
            }
        }
        Err(error) => {
            file.locked = matches!(error, crate::BookError::Protected);
            file.broken = Some(error.to_string());
        }
    }
}

fn clean_number(text: &str) -> Option<String> {
    naming::milli(text.trim().trim_start_matches('0').trim_start_matches(' '))
        .or_else(|| naming::milli(text.trim()))
        .map(|(_, shown)| shown)
}

struct Facts {
    format: Format,
    pages: Option<u32>,
    novel: bool,
    language: Option<String>,
    series: Option<String>,
    title: Option<String>,
    info: Option<ComicInfo>,
}

fn facts(book: &Book) -> Facts {
    match book {
        Book::Comic(comic) => {
            let info = comic.info().cloned();
            Facts {
                format: comic.format(),
                pages: u32::try_from(comic.pages().len()).ok(),
                novel: false,
                language: info.as_ref().and_then(|i| i.language.clone()),
                series: info.as_ref().and_then(|i| i.series.clone()),
                title: info.as_ref().and_then(|i| i.title.clone()),
                info,
            }
        }
        Book::Epub(epub) => {
            let meta = epub.metadata();
            let fixed = epub.is_fixed_layout();
            Facts {
                format: Format::Epub,
                pages: None,
                novel: !fixed,
                language: meta.languages.first().cloned(),
                series: meta.series.as_ref().map(|s| s.name.clone()),
                title: meta.titles.first().cloned(),
                info: None,
            }
        }
        #[cfg(feature = "pdf")]
        Book::Pdf(pdf) => Facts {
            format: Format::Pdf,
            pages: u32::try_from(pdf.page_count()).ok(),
            novel: false,
            language: None,
            series: None,
            title: None,
            info: None,
        },
    }
}

/// Group a series' files: the name, the order, whether numbering restarts per volume.
fn series_of(key: String, mut files: Vec<ScannedFile>) -> ScannedSeries {
    let name = display_name(&key, &files);
    // Numbers restart per volume when one chapter number turns up in two different volumes.
    let mut seen: HashMap<i64, Option<&str>> = HashMap::new();
    let mut restarts = false;
    for file in &files {
        if let Some(n) = file.number_milli {
            match seen.get(&n) {
                Some(volume) if *volume != file.volume.as_deref() => restarts = true,
                _ => {
                    seen.insert(n, file.volume.as_deref());
                }
            }
        }
    }
    files.sort_by(|a, b| {
        let volume = |f: &ScannedFile| f.volume.as_deref().and_then(naming::milli).map(|v| v.0);
        // Volume first when numbering restarts, else the chapter number alone, so a missing
        // volume tag cannot scatter a series. Files with no number follow, by name.
        let key = |f: &ScannedFile| {
            (
                if restarts { volume(f) } else { None },
                f.number_milli.is_none(),
                if restarts || f.number_milli.is_some() { f.number_milli } else { volume(f) },
            )
        };
        key(a).cmp(&key(b)).then_with(|| crate::natural::compare(&a.path, &b.path))
    });
    let right_to_left = files
        .iter()
        .filter_map(|f| f.info.as_ref())
        .find_map(|i| i.manga.map(|_| i.right_to_left()));
    let novel =
        files.iter().any(|f| f.novel) && files.iter().all(|f| f.novel || f.broken.is_some());
    ScannedSeries { key, name, files, restarts_per_volume: restarts, right_to_left, novel }
}

/// A series' name for people: the one name the books give themselves, else the folder's.
fn display_name(key: &str, files: &[ScannedFile]) -> String {
    let mut own: Vec<&str> = files.iter().filter_map(|f| f.own_series.as_deref()).collect();
    own.sort_unstable();
    own.dedup();
    let tidy = naming::tidy(key.strip_prefix("novel:").unwrap_or(key));
    let plain = |text: &str| -> String {
        text.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
    };
    // The books' own name wins when every book gives it, or when it is the folder's name in
    // better spelling. A folder with one odd book in it keeps the folder's name.
    if own.len() == 1
        && (files.iter().all(|f| f.own_series.is_some() || f.broken.is_some())
            || plain(own[0]) == plain(&tidy))
    {
        return own[0].trim().to_owned();
    }
    // A lone EPUB is its own series: the book's title says it better than a file name.
    if files.len() == 1 && files[0].novel && !files[0].label.is_empty() {
        return files[0].label.clone();
    }
    if tidy.is_empty() { key.to_owned() } else { tidy }
}

/// The path of a scanned file as a `PathBuf`.
pub fn path_of(file: &ScannedFile) -> PathBuf {
    PathBuf::from(&file.path)
}

/// A time in the past, for tests and callers that need "long ago".
pub fn epoch() -> SystemTime {
    UNIX_EPOCH
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn cbz(path: &Path, pages: usize, info: Option<&str>) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let file = fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default();
        for i in 1..=pages {
            zip.start_file(format!("{i:03}.png"), opts).unwrap();
            zip.write_all(&tiny_png()).unwrap();
        }
        if let Some(info) = info {
            zip.start_file("ComicInfo.xml", opts).unwrap();
            zip.write_all(info.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }

    fn tiny_png() -> Vec<u8> {
        let mut out = Vec::new();
        image::DynamicImage::new_rgb8(2, 3)
            .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
        out
    }

    fn scan_dir(root: &Path) -> Scan {
        scan(root, &Nothing, &Limits::default(), &mut |_| {}).unwrap()
    }

    fn series<'a>(scan: &'a Scan, name: &str) -> &'a ScannedSeries {
        scan.series.iter().find(|s| s.name == name).unwrap_or_else(|| panic!("no {name}"))
    }

    fn numbers(series: &ScannedSeries) -> Vec<String> {
        series
            .files
            .iter()
            .map(|f| match (&f.volume, &f.number) {
                (Some(v), Some(n)) => format!("v{v}c{n}"),
                (None, Some(n)) => format!("c{n}"),
                (Some(v), None) => format!("v{v}"),
                (None, None) => f.label.clone(),
            })
            .collect()
    }

    #[test]
    fn comicinfo_comes_before_the_file_name() {
        let dir = tempfile::tempdir().unwrap();
        cbz(
            &dir.path().join("Moon Harbour/scan one.cbz"),
            3,
            Some(
                "<ComicInfo><Series>Moon Harbour</Series><Number>4</Number><Volume>2</Volume><Manga>YesAndRightToLeft</Manga></ComicInfo>",
            ),
        );
        let scan = scan_dir(dir.path());
        let s = series(&scan, "Moon Harbour");
        assert_eq!(numbers(s), ["v2c4"]);
        assert_eq!(s.files[0].pages, Some(3));
        assert_eq!(s.right_to_left, Some(true));
    }

    #[test]
    fn decimal_chapters_sort_between_their_neighbours() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["ch011", "ch010.5", "ch010", "ch009", "ch002"] {
            cbz(&dir.path().join(format!("Tin Club/Tin Club {name}.cbz")), 1, None);
        }
        let scan = scan_dir(dir.path());
        assert_eq!(numbers(series(&scan, "Tin Club")), ["c2", "c9", "c10", "c10.5", "c11"]);
        assert!(!series(&scan, "Tin Club").restarts_per_volume);
    }

    #[test]
    fn numbering_that_restarts_per_volume_is_found_and_sorted_by_volume() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["v02 c001", "v01 c002", "v01 c001", "v02 c002"] {
            cbz(&dir.path().join(format!("Kites/Kites {name}.cbz")), 1, None);
        }
        let scan = scan_dir(dir.path());
        let s = series(&scan, "Kites");
        assert!(s.restarts_per_volume);
        assert_eq!(numbers(s), ["v1c1", "v1c2", "v2c1", "v2c2"]);
    }

    #[test]
    fn a_book_in_the_root_is_a_series_of_its_own_and_volumes_share_one() {
        let dir = tempfile::tempdir().unwrap();
        cbz(&dir.path().join("Loose Single Volume.cbz"), 2, None);
        cbz(&dir.path().join("Harbor v01.cbz"), 2, None);
        cbz(&dir.path().join("Harbor v02.cbz"), 2, None);
        let scan = scan_dir(dir.path());
        assert_eq!(scan.series.len(), 2);
        assert_eq!(series(&scan, "Loose Single Volume").files.len(), 1);
        assert_eq!(numbers(series(&scan, "Harbor")), ["v1", "v2"]);
    }

    #[test]
    fn a_chapter_inside_a_volume_folder_takes_the_volume() {
        let dir = tempfile::tempdir().unwrap();
        cbz(&dir.path().join("Series/Vol. 2/ch003.cbz"), 1, None);
        cbz(&dir.path().join("Series/Vol. 1/ch003.cbz"), 1, None);
        let scan = scan_dir(dir.path());
        let s = series(&scan, "Series");
        assert!(s.restarts_per_volume);
        assert_eq!(numbers(s), ["v1c3", "v2c3"]);
    }

    #[test]
    fn a_folder_of_images_is_a_chapter() {
        let dir = tempfile::tempdir().unwrap();
        for ch in ["ch001", "ch002"] {
            let chapter = dir.path().join(format!("Folded/{ch}"));
            fs::create_dir_all(&chapter).unwrap();
            for i in 1..=3 {
                fs::write(chapter.join(format!("{i}.png")), tiny_png()).unwrap();
            }
        }
        let scan = scan_dir(dir.path());
        let s = series(&scan, "Folded");
        assert_eq!(numbers(s), ["c1", "c2"]);
        assert_eq!(s.files[0].format, "folder");
        assert_eq!(s.files[0].pages, Some(3));
    }

    #[test]
    fn broken_and_foreign_files_are_set_aside_without_stopping_the_scan() {
        let dir = tempfile::tempdir().unwrap();
        cbz(&dir.path().join("Mixed/Mixed ch001.cbz"), 1, None);
        fs::write(dir.path().join("Mixed/Mixed ch002.cbz"), b"this is not a zip").unwrap();
        fs::write(dir.path().join("Mixed/empty.cbz"), b"").unwrap();
        fs::write(dir.path().join("Mixed/notes.txt"), b"hello").unwrap();
        fs::write(dir.path().join("Mixed/.hidden.cbz"), b"hello").unwrap();
        let scan = scan_dir(dir.path());
        assert_eq!(scan.series.len(), 1);
        let s = &scan.series[0];
        assert_eq!(s.files.len(), 2);
        assert_eq!(scan.broken, 1);
        assert!(s.files.iter().any(|f| f.broken.is_some()));
        assert_eq!(scan.ignored, 2, "an empty file and a note; hidden files are not counted");
    }

    #[test]
    fn a_file_the_scan_knows_is_not_opened_again() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Known/Known ch001.cbz");
        cbz(&path, 2, None);
        let first = scan_dir(dir.path());
        let mut known: HashMap<String, ScannedFile> = HashMap::new();
        for file in &first.series[0].files {
            let mut kept = file.clone();
            kept.label = "kept from before".into();
            known.insert(file.path.clone(), kept);
        }
        let again = scan(dir.path(), &known, &Limits::default(), &mut |_| {}).unwrap();
        assert_eq!(again.series[0].files[0].label, "kept from before");
        // A changed file is read again.
        cbz(&path, 5, None);
        let mut changed = known.clone();
        changed.values_mut().for_each(|f| f.size += 1);
        let again = scan(dir.path(), &changed, &Limits::default(), &mut |_| {}).unwrap();
        assert_eq!(again.series[0].files[0].pages, Some(5));
    }

    #[test]
    fn the_found_count_climbs_and_a_missing_folder_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        cbz(&dir.path().join("A/A ch001.cbz"), 1, None);
        cbz(&dir.path().join("A/A ch002.cbz"), 1, None);
        let mut counts = Vec::new();
        scan(dir.path(), &Nothing, &Limits::default(), &mut |n| counts.push(n)).unwrap();
        assert_eq!(counts, [1, 2]);
        assert!(
            scan(&dir.path().join("gone"), &Nothing, &Limits::default(), &mut |_| {}).is_err()
        );
    }
}
