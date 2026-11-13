//! Writing the files of one send: pages processed for the target and set into a fixed-layout
//! EPUB, KEPUB or CBZ, or the text of chapters set into a plain EPUB or KEPUB.
//!
//! Pure of the engine: chapter files in, files out, with progress and a stop flag. The
//! processed pages of a manga are written to a scratch folder first, so a long series is not
//! held in memory and the split into parts knows the sizes it is splitting.

use std::{
    fs::{self, File},
    io::BufWriter,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

use kaeri_books::{
    Book, BookError, ComicInfo, Limits,
    comicinfo::MangaFlag,
    epub::Direction as BookDirection,
    write::{
        BookMeta, CbzWriter, EpubFlavor, EpubWriter, FixedLayout, KINDLE_PART_BYTES,
        ProblemKind, check_epub, split_parts,
    },
};
use kaeri_model::reading::{ExportFile, ExportFormat, ReadingFailure, SendProblem};
use kaeri_pages::{
    export::{ExportPage, prepare as prepare_page},
    layout::Direction,
    render::{Look, Renderer, Target},
};

use super::profiles::Profile;

/// One chapter to send, and the file it is downloaded as.
#[derive(Debug, Clone)]
pub struct Chapter {
    pub label: String,
    pub path: PathBuf,
}

/// What one send is asked to write.
#[derive(Debug, Clone)]
pub struct Request {
    pub title: String,
    pub chapters: Vec<Chapter>,
    pub profile: Profile,
    pub format: ExportFormat,
    /// Where the files go.
    pub out: PathBuf,
    /// A part is kept under this many bytes; `None` takes the Kindle limit for a profile that
    /// splits and no limit otherwise. Set by tests.
    pub limit: Option<u64>,
    /// Which way a manga's pages turn, as the reader turns them.
    pub direction: PageDirection,
    /// Identifies the book to the e-reader, so a second send of the same chapters keeps its
    /// place there.
    pub identifier: String,
}

/// Which way a manga's pages turn: what the series says, else what its book says, else the
/// medium's default, in the order the reader takes them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PageDirection {
    /// Right to left or not, when the series says: the person's choice for it, its source's
    /// word, or the reader's kept guess. A series read as a strip turns its pages forward.
    pub said: Option<bool>,
    /// Right to left or not when neither the series nor its book says: the medium's default.
    pub otherwise: bool,
}

/// Why a send stopped.
#[derive(Debug)]
pub enum Stop {
    Cancelled,
    Failed(ReadingFailure),
}

impl From<ReadingFailure> for Stop {
    fn from(f: ReadingFailure) -> Self {
        Stop::Failed(f)
    }
}

fn unreadable(path: &Path, format: &str) -> Stop {
    Stop::Failed(ReadingFailure::Unreadable {
        path: path.display().to_string(),
        format: format.into(),
    })
}

/// A file read failed: it is not there, or no longer whole.
fn io(e: &std::io::Error, path: &Path) -> Stop {
    if e.kind() == std::io::ErrorKind::StorageFull {
        return Stop::Failed(ReadingFailure::DiskFull);
    }
    Stop::Failed(ReadingFailure::FileMissing { path: path.display().to_string() })
}

/// A file or folder could not be written.
fn write_failed(e: &std::io::Error, path: &Path) -> Stop {
    Stop::Failed(crate::downloads::books::io_failure(e, path))
}

fn book(e: &BookError, path: &Path) -> Stop {
    tracing::debug!(error = %e, "a chapter was not prepared");
    unreadable(path, "book")
}

/// Whether the chapters are text (EPUB) or pages.
fn is_text(chapters: &[Chapter]) -> bool {
    chapters
        .first()
        .and_then(|c| c.path.extension())
        .is_some_and(|e| e.eq_ignore_ascii_case("epub"))
}

/// Writes the files. `progress` is told `(done, total)` as work finishes.
pub fn run(
    req: &Request,
    progress: &mut dyn FnMut(u32, u32),
    cancel: &AtomicBool,
) -> Result<Vec<ExportFile>, Stop> {
    if req.chapters.is_empty() {
        return Err(ReadingFailure::NotFound.into());
    }
    let text = is_text(&req.chapters);
    if !req.profile.accepts(text).contains(&req.format) {
        return Err(unsupported(req.format));
    }
    fs::create_dir_all(&req.out).map_err(|e| write_failed(&e, &req.out))?;
    let result = if text { novel(req, progress, cancel) } else { manga(req, progress, cancel) };
    let result = result.and_then(|files| preflight(req, &files).map(|()| files));
    if result.is_err() {
        // Nothing half written is left to be shared.
        let _ = fs::remove_dir_all(&req.out);
    }
    result
}

/// The check before a send: every file within what the target takes, and every EPUB or KEPUB
/// laid out as the format requires, declaring UTF-8, and opening again. A file that fails names
/// the exact reason, and nothing is sent.
fn preflight(req: &Request, files: &[ExportFile]) -> Result<(), Stop> {
    for file in files {
        if let Some(limit) = req.profile.max_file_bytes
            && file.bytes > limit
        {
            return Err(ReadingFailure::SendTooLarge {
                file: file.name.clone(),
                target: req.profile.name.clone(),
                bytes: file.bytes,
                limit,
            }
            .into());
        }
        if req.format == ExportFormat::Cbz {
            continue;
        }
        check_epub(Path::new(&file.path), &Limits::default()).map_err(|problem| {
            tracing::warn!(file = %file.name, part = %problem.part, kind = ?problem.kind, "a prepared file failed its check");
            ReadingFailure::SendCheck {
                file: file.name.clone(),
                part: problem.part,
                problem: match problem.kind {
                    ProblemKind::Missing => SendProblem::Missing,
                    ProblemKind::NotUtf8 => SendProblem::NotUtf8,
                    ProblemKind::Malformed => SendProblem::Malformed,
                },
            }
        })?;
    }
    Ok(())
}

fn unsupported(format: ExportFormat) -> Stop {
    let extension = match format {
        ExportFormat::FixedEpub | ExportFormat::Epub => ".epub",
        ExportFormat::Kepub => ".kepub.epub",
        ExportFormat::Cbz => ".cbz",
    };
    Stop::Failed(ReadingFailure::Unsupported { extension: extension.into() })
}

fn limit_of(req: &Request) -> u64 {
    req.limit.unwrap_or(if req.profile.split { KINDLE_PART_BYTES } else { u64::MAX })
}

fn stopped(cancel: &AtomicBool) -> Result<(), Stop> {
    if cancel.load(Ordering::Relaxed) { Err(Stop::Cancelled) } else { Ok(()) }
}

/// The most bytes a file's name takes from its title. With the part and the extension the name
/// stays far under the 255 bytes file systems allow, in any script.
const TITLE_BYTES: usize = 160;

/// A file name from a title: letters, digits, spaces and a few marks, at most 80 characters
/// and [`TITLE_BYTES`] bytes, and never a name Windows reserves.
fn clean(title: &str) -> String {
    let kept: String = title
        .chars()
        .map(|c| if c.is_alphanumeric() || " -_.,'()".contains(c) { c } else { ' ' })
        .collect();
    let kept = kept.split_whitespace().collect::<Vec<_>>().join(" ");
    let kept: String = kept.trim_matches(|c| c == '.' || c == ' ').chars().take(80).collect();
    let kept = kaeri_core::file_name::readable(&kept, TITLE_BYTES);
    if kept.is_empty() { "Book".into() } else { kept }
}

/// `Title 40-45`, `Title 7`; the part, when there are several, as ` (2 of 3)`.
fn file_name(req: &Request, part: Option<(usize, usize)>) -> String {
    let mut name = clean(&book_title(req, None));
    if let Some((n, of)) = part.filter(|(_, of)| *of > 1) {
        name.push_str(&format!(" ({n} of {of})"));
    }
    name.push_str(match req.format {
        ExportFormat::Kepub => ".kepub.epub",
        ExportFormat::Cbz => ".cbz",
        ExportFormat::FixedEpub | ExportFormat::Epub => ".epub",
    });
    name
}

/// The book's title as the e-reader's library lists it: `Title 40-45`, `Title 7`, so each send
/// is told apart as its file name tells it; the part, when there are several, as ` (2 of 3)`.
fn book_title(req: &Request, part: Option<(usize, usize)>) -> String {
    let first = req.chapters.first().map(|c| c.label.as_str()).unwrap_or("");
    let last = req.chapters.last().map(|c| c.label.as_str()).unwrap_or("");
    let range = if req.chapters.len() <= 1 || first == last {
        first.to_owned()
    } else {
        format!("{first}-{last}")
    };
    let mut title = format!("{} {range}", req.title).trim().to_owned();
    if let Some((n, of)) = part.filter(|(_, of)| *of > 1) {
        title.push_str(&format!(" ({n} of {of})"));
    }
    title
}

fn done_file(path: PathBuf, name: String, part: Option<u32>) -> Result<ExportFile, Stop> {
    let bytes = fs::metadata(&path).map_err(|e| io(&e, &path))?.len();
    Ok(ExportFile { path: path.display().to_string(), name, bytes, part })
}

/// A language tag a book gives, `None` when it gives none or says it is undetermined.
fn said_language(tag: Option<&str>) -> Option<String> {
    tag.map(str::trim)
        .filter(|t| !t.is_empty() && !t.eq_ignore_ascii_case("und"))
        .map(str::to_owned)
}

fn chapter_title(label: &str) -> String {
    kaeri_i18n::t::export_chapter(label)
}

// ---------------------------------------------------------------------------------------------
// Pages

struct Prepared {
    chapter: usize,
    file: PathBuf,
    bytes: u64,
}

fn manga(
    req: &Request,
    progress: &mut dyn FnMut(u32, u32),
    cancel: &AtomicBool,
) -> Result<Vec<ExportFile>, Stop> {
    if req.format == ExportFormat::Epub {
        return Err(unsupported(req.format));
    }
    let limits = Limits::default();
    let mut comics = Vec::new();
    for chapter in &req.chapters {
        match kaeri_books::open(&chapter.path, &limits) {
            Ok(Book::Comic(c)) => comics.push(c),
            Ok(_) => return Err(unreadable(&chapter.path, "CBZ")),
            Err(e) => return Err(book(&e, &chapter.path)),
        }
    }
    let total: u32 = comics.iter().map(|c| c.pages().len() as u32).sum();
    let info = comics.first().and_then(|c| c.info());
    // A book's ComicInfo marks manga as the reader reads it: any manga right to left.
    let book_says = info.and_then(|i| match i.manga {
        Some(MangaFlag::YesAndRightToLeft | MangaFlag::Yes) => Some(true),
        Some(MangaFlag::No) => Some(false),
        _ => None,
    });
    let right_to_left = req.direction.said.or(book_says).unwrap_or(req.direction.otherwise);
    let language = info.and_then(|i| said_language(i.language.as_deref()));
    let direction = if right_to_left { Direction::RightToLeft } else { Direction::LeftToRight };
    let target = Target {
        panel: req.profile.panel,
        width: req.profile.width,
        height: req.profile.height,
    };
    let mut renderer = Renderer::new(target, Look::default());
    let scratch = req.out.join(".pages");
    fs::create_dir_all(&scratch).map_err(|e| write_failed(&e, &scratch))?;
    let mut prepared: Vec<Prepared> = Vec::new();
    let mut done = 0u32;
    progress(0, total);
    for (chapter, comic) in comics.iter_mut().enumerate() {
        for index in 0..comic.pages().len() {
            stopped(cancel)?;
            let path = &req.chapters[chapter].path;
            let source = comic.read_page(index).map_err(|e| book(&e, path))?;
            let pages: Vec<ExportPage> = prepare_page(&mut renderer, &source, direction)
                .map_err(|e| {
                    tracing::debug!(error = %e, "a page was not prepared");
                    unreadable(path, "CBZ")
                })?;
            for page in pages {
                let file = scratch.join(format!("{:06}", prepared.len()));
                fs::write(&file, &page.bytes).map_err(|e| write_failed(&e, &file))?;
                prepared.push(Prepared { chapter, file, bytes: page.bytes.len() as u64 });
            }
            done += 1;
            progress(done, total);
        }
    }
    stopped(cancel)?;
    let sizes: Vec<u64> = prepared.iter().map(|p| p.bytes).collect();
    let parts = split_parts(&sizes, limit_of(req));
    let count = parts.len();
    let mut files = Vec::with_capacity(count);
    for (n, range) in parts.into_iter().enumerate() {
        stopped(cancel)?;
        let name = file_name(req, Some((n + 1, count)));
        let path = req.out.join(&name);
        let out = BufWriter::new(File::create(&path).map_err(|e| write_failed(&e, &path))?);
        let items = &prepared[range];
        match req.format {
            ExportFormat::Cbz => {
                write_cbz(req, items, out, &path, right_to_left, language.as_deref())?
            }
            _ => write_fixed(
                req,
                items,
                out,
                &path,
                (right_to_left, language.as_deref()),
                n + 1,
                count,
            )?,
        }
        files.push(done_file(path, name, (count > 1).then_some(n as u32 + 1))?);
    }
    let _ = fs::remove_dir_all(&scratch);
    Ok(files)
}

fn write_cbz(
    req: &Request,
    items: &[Prepared],
    out: BufWriter<File>,
    path: &Path,
    right_to_left: bool,
    language: Option<&str>,
) -> Result<(), Stop> {
    let info = ComicInfo {
        title: Some(book_title(req, None)),
        series: Some(req.title.clone()),
        manga: Some(if right_to_left { MangaFlag::YesAndRightToLeft } else { MangaFlag::Yes }),
        page_count: Some(items.len() as u32),
        language: language.map(str::to_owned),
        ..ComicInfo::default()
    };
    let mut cbz = CbzWriter::new(out, Some(&info)).map_err(|e| book(&e, path))?;
    for item in items {
        let bytes = fs::read(&item.file).map_err(|e| io(&e, &item.file))?;
        cbz.add_page(&bytes).map_err(|e| book(&e, path))?;
    }
    cbz.finish().map_err(|e| book(&e, path))?;
    Ok(())
}

fn write_fixed(
    req: &Request,
    items: &[Prepared],
    out: BufWriter<File>,
    path: &Path,
    (right_to_left, language): (bool, Option<&str>),
    part: usize,
    parts: usize,
) -> Result<(), Stop> {
    let flavor =
        if req.format == ExportFormat::Kepub { EpubFlavor::Kepub } else { EpubFlavor::Epub };
    let title = book_title(req, Some((part, parts)));
    let meta = BookMeta {
        title,
        language: language.unwrap_or_default().to_owned(),
        identifier: Some(format!("{}:{part}", req.identifier)),
        direction: Some(if right_to_left { BookDirection::Rtl } else { BookDirection::Ltr }),
        ..BookMeta::default()
    };
    let fixed =
        FixedLayout { width: req.profile.width, height: req.profile.height, spreads: false };
    let mut epub =
        EpubWriter::new(out, meta, flavor, Some(fixed)).map_err(|e| book(&e, path))?;
    let mut last_chapter = None;
    for item in items {
        let bytes = fs::read(&item.file).map_err(|e| io(&e, &item.file))?;
        let title = (last_chapter != Some(item.chapter))
            .then(|| chapter_title(&req.chapters[item.chapter].label));
        last_chapter = Some(item.chapter);
        epub.add_image_page(&bytes, None, title.as_deref()).map_err(|e| book(&e, path))?;
    }
    epub.finish().map_err(|e| book(&e, path))?;
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Text

/// The inside of a document's `<body>`.
fn body_inner(xhtml: &str) -> Option<&str> {
    let lower = xhtml.to_ascii_lowercase();
    let open = lower.find("<body")?;
    let start = open + xhtml[open..].find('>')? + 1;
    let end = lower.rfind("</body>")?;
    (end >= start).then(|| &xhtml[start..end])
}

/// One chapter's text with its images carried over into `writer`: each image the text names is
/// added under a name of its own and the text points at that.
fn chapter_body(
    epub: &mut kaeri_books::Epub,
    chapter: usize,
    writer: &mut EpubWriter<BufWriter<File>>,
    path: &Path,
) -> Result<String, Stop> {
    let spine: Vec<(String, bool)> = epub
        .spine()
        .map(|(_, item)| (item.path.clone(), item.media_type == "application/xhtml+xml"))
        .collect();
    let mut body = String::new();
    let mut images = 0;
    for (item, xhtml) in spine {
        if !xhtml {
            continue;
        }
        let bytes = epub.resource(&item).map_err(|e| book(&e, path))?.data;
        let text = String::from_utf8_lossy(&bytes).into_owned();
        let Some(inner) = body_inner(&text) else { continue };
        let mut inner = inner.to_owned();
        // Each `src="…"` that names a file in the book becomes a file of the new one.
        let mut from = 0;
        while let Some(at) = inner[from..].find("src=\"") {
            let start = from + at + 5;
            let Some(len) = inner[start..].find('"') else { break };
            let href = inner[start..start + len].to_owned();
            from = start + len;
            let Some(resolved) = epub.resolve(&item, &href) else { continue };
            if !epub.contains(&resolved.path) {
                continue;
            }
            let Ok(res) = epub.resource(&resolved.path) else { continue };
            if !res.media_type.starts_with("image/") {
                continue;
            }
            images += 1;
            let ext = res.media_type.trim_start_matches("image/").replace("jpeg", "jpg");
            let ext = ext.split(['+', ';']).next().unwrap_or("img").to_owned();
            let name = format!("img/c{chapter}-{images}.{ext}");
            if writer.add_resource(&name, &res.media_type, &res.data).is_err() {
                continue;
            }
            inner.replace_range(start..start + len, &name);
            from = start + name.len();
        }
        body.push_str(&inner);
    }
    Ok(body)
}

fn novel(
    req: &Request,
    progress: &mut dyn FnMut(u32, u32),
    cancel: &AtomicBool,
) -> Result<Vec<ExportFile>, Stop> {
    let flavor = match req.format {
        ExportFormat::Epub => EpubFlavor::Epub,
        ExportFormat::Kepub => EpubFlavor::Kepub,
        _ => return Err(unsupported(req.format)),
    };
    let limits = Limits::default();
    let sizes: Vec<u64> = req
        .chapters
        .iter()
        .map(|c| kaeri_books::tree::stat(&c.path).map(|m| m.size).map_err(|e| io(&e, &c.path)))
        .collect::<Result<_, _>>()?;
    let parts = split_parts(&sizes, limit_of(req));
    let count = parts.len();
    let total = req.chapters.len() as u32;
    // The first chapter's language is the book's, so the e-reader sets it as that language.
    let language =
        req.chapters.first().and_then(|c| match kaeri_books::open(&c.path, &limits) {
            Ok(Book::Epub(epub)) => {
                said_language(epub.metadata().languages.first().map(String::as_str))
            }
            _ => None,
        });
    let mut done = 0;
    progress(0, total);
    let mut files = Vec::with_capacity(count);
    for (n, range) in parts.into_iter().enumerate() {
        let name = file_name(req, Some((n + 1, count)));
        let path = req.out.join(&name);
        let out = BufWriter::new(File::create(&path).map_err(|e| write_failed(&e, &path))?);
        let title = book_title(req, Some((n + 1, count)));
        let meta = BookMeta {
            title,
            language: language.clone().unwrap_or_default(),
            identifier: Some(format!("{}:{}", req.identifier, n + 1)),
            ..BookMeta::default()
        };
        let mut writer =
            EpubWriter::new(out, meta, flavor, None).map_err(|e| book(&e, &path))?;
        let mut covered = false;
        for index in range {
            stopped(cancel)?;
            let chapter = &req.chapters[index];
            let mut epub = match kaeri_books::open(&chapter.path, &limits) {
                Ok(Book::Epub(e)) => e,
                Ok(_) => return Err(unreadable(&chapter.path, "EPUB")),
                Err(e) => return Err(book(&e, &chapter.path)),
            };
            if !covered
                && let Some(cover) = epub.cover().map(|c| c.path.clone())
                && let Ok(res) = epub.resource(&cover)
            {
                covered = writer.set_cover(&res.data).is_ok();
            }
            let body = chapter_body(&mut epub, index, &mut writer, &chapter.path)?;
            writer
                .add_chapter(&chapter_title(&chapter.label), &body)
                .map_err(|e| book(&e, &chapter.path))?;
            done += 1;
            progress(done, total);
        }
        let bytes = writer.finish().map_err(|e| book(&e, &path))?;
        drop(bytes);
        files.push(done_file(path, name, (count > 1).then_some(n as u32 + 1))?);
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::export::profiles::{builtin_profiles, find};

    fn noise(width: u32, height: u32, seed: u32) -> Vec<u8> {
        let mut state = seed.wrapping_mul(2_654_435_761).wrapping_add(1);
        let image = image::RgbImage::from_fn(width, height, |_, _| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let v = (state >> 24) as u8;
            image::Rgb([v, v.wrapping_add(40), v.wrapping_add(90)])
        });
        let mut out = Cursor::new(Vec::new());
        image.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    fn cbz(path: &Path, pages: u32, wide: bool) {
        let mut w = CbzWriter::new(BufWriter::new(File::create(path).unwrap()), None).unwrap();
        for n in 0..pages {
            let (width, height) = if wide && n == 1 { (400, 300) } else { (200, 300) };
            w.add_page(&noise(width, height, n + 1)).unwrap();
        }
        w.finish().unwrap();
    }

    fn text_epub(path: &Path, title: &str, paragraphs: usize) {
        let meta = BookMeta { title: title.into(), ..BookMeta::default() };
        let out = BufWriter::new(File::create(path).unwrap());
        let mut w = EpubWriter::new(out, meta, EpubFlavor::Epub, None).unwrap();
        let body: String =
            (0..paragraphs).map(|n| format!("<p>Lantern light, line {n}.</p>")).collect();
        w.add_chapter("One", &body).unwrap();
        w.finish().unwrap();
    }

    fn profile(id: &str) -> Profile {
        builtin_profiles().into_iter().find(|p| p.id == id).unwrap()
    }

    fn request(
        dir: &Path,
        chapters: Vec<Chapter>,
        profile: &str,
        format: ExportFormat,
    ) -> Request {
        Request {
            title: "The Lantern Ferry".into(),
            chapters,
            profile: profile_by(profile),
            format,
            out: dir.join("out"),
            limit: None,
            direction: PageDirection::default(),
            identifier: "made-up:1".into(),
        }
    }

    fn profile_by(id: &str) -> Profile {
        let mut p = profile(id);
        // A small screen keeps the generated pages quick to process.
        p.width = 120;
        p.height = 160;
        p
    }

    fn chapters(dir: &Path, count: usize, pages: u32) -> Vec<Chapter> {
        (0..count)
            .map(|n| {
                let path = dir.join(format!("c{n}.cbz"));
                cbz(&path, pages, false);
                Chapter { label: format!("Chapter {}", n + 1), path }
            })
            .collect()
    }

    fn go(req: &Request) -> Result<Vec<ExportFile>, Stop> {
        run(req, &mut |_, _| {}, &AtomicBool::new(false))
    }

    #[test]
    fn a_long_cjk_title_makes_a_name_any_file_system_takes() {
        let dir = tempfile::tempdir().unwrap();
        let mut req = request(dir.path(), Vec::new(), "kobo", ExportFormat::Kepub);
        let label: String = std::iter::repeat_n('話', 60).collect();
        req.title = std::iter::repeat_n('灯', 90).collect();
        req.chapters = vec![
            Chapter { label: label.clone(), path: PathBuf::new() },
            Chapter { label: format!("{label}二"), path: PathBuf::new() },
        ];
        let name = file_name(&req, Some((12, 12)));
        assert!(name.len() <= 200, "{} bytes", name.len());
        assert!(name.ends_with(" (12 of 12).kepub.epub"), "{name}");
        let written = dir.path().join(&name);
        File::create(&written).unwrap();
        assert!(written.exists());
    }

    #[test]
    fn a_title_windows_reserves_is_not_a_file_name() {
        let dir = tempfile::tempdir().unwrap();
        let mut req = request(dir.path(), Vec::new(), "kobo", ExportFormat::Epub);
        req.title = "Con".into();
        assert_eq!(file_name(&req, None), "Con_.epub");
        req.title = "Lantern...".into();
        assert_eq!(file_name(&req, None), "Lantern.epub");
    }

    #[test]
    fn a_kepub_of_pages_reads_back_as_fixed_layout() {
        let dir = tempfile::tempdir().unwrap();
        let req = request(dir.path(), chapters(dir.path(), 2, 2), "kobo", ExportFormat::Kepub);
        let files = go(&req).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].name.ends_with(".kepub.epub"), "{}", files[0].name);
        let Ok(Book::Epub(epub)) =
            kaeri_books::open(Path::new(&files[0].path), &Limits::default())
        else {
            panic!("not an EPUB");
        };
        assert!(epub.is_fixed_layout());
        assert_eq!(epub.spine().count(), 4);
        assert!(!req.out.join(".pages").exists());
    }

    #[test]
    fn a_fixed_epub_for_the_kindle_class_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        let req =
            request(dir.path(), chapters(dir.path(), 1, 3), "kindle", ExportFormat::FixedEpub);
        let files = go(&req).unwrap();
        assert!(files[0].name.ends_with(".epub"));
        let Ok(Book::Epub(epub)) =
            kaeri_books::open(Path::new(&files[0].path), &Limits::default())
        else {
            panic!("not an EPUB");
        };
        assert!(epub.is_fixed_layout());
        assert_eq!(epub.spine().count(), 3);
    }

    #[test]
    fn a_cbz_keeps_every_page_and_splits_a_spread() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wide.cbz");
        cbz(&path, 3, true);
        let chapters = vec![Chapter { label: "Chapter 1".into(), path }];
        let req = request(dir.path(), chapters, "kobo", ExportFormat::Cbz);
        let files = go(&req).unwrap();
        assert!(files[0].name.ends_with(".cbz"));
        let Ok(Book::Comic(comic)) =
            kaeri_books::open(Path::new(&files[0].path), &Limits::default())
        else {
            panic!("not a comic");
        };
        // Three pages, the middle one a spread of two.
        assert_eq!(comic.pages().len(), 4);
    }

    #[test]
    fn a_novel_goes_out_as_epub_and_kepub() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("n.epub");
        text_epub(&path, "Ferry Notes", 20);
        for (format, ext) in
            [(ExportFormat::Epub, ".epub"), (ExportFormat::Kepub, ".kepub.epub")]
        {
            let mut req = request(
                dir.path(),
                vec![Chapter { label: "Chapter 1".into(), path: path.clone() }],
                "kobo",
                format,
            );
            req.out = dir.path().join(format!("out{ext}"));
            let files = go(&req).unwrap();
            assert!(files[0].name.ends_with(ext), "{}", files[0].name);
            let Ok(Book::Epub(mut epub)) =
                kaeri_books::open(Path::new(&files[0].path), &Limits::default())
            else {
                panic!("not an EPUB");
            };
            assert!(!epub.is_fixed_layout());
            let item = epub.spine().next().map(|(_, i)| i.path.clone()).unwrap();
            let text = String::from_utf8(epub.resource(&item).unwrap().data).unwrap();
            assert!(text.contains("Lantern light, line 19."));
        }
    }

    /// Which way an EPUB's pages turn, and the languages it says.
    fn read_back(file: &ExportFile) -> (Option<BookDirection>, Vec<String>) {
        let Ok(Book::Epub(epub)) = kaeri_books::open(Path::new(&file.path), &Limits::default())
        else {
            panic!("not an EPUB");
        };
        (epub.package().direction, epub.metadata().languages.clone())
    }

    #[test]
    fn each_sent_book_is_titled_with_its_chapters_and_part() {
        let dir = tempfile::tempdir().unwrap();
        let title = |req: &Request| {
            let file = &go(req).unwrap()[0];
            let Ok(Book::Epub(epub)) =
                kaeri_books::open(Path::new(&file.path), &Limits::default())
            else {
                panic!("not an EPUB");
            };
            epub.metadata().titles.clone()
        };
        let mut req =
            request(dir.path(), chapters(dir.path(), 2, 1), "kobo", ExportFormat::Kepub);
        assert_eq!(title(&req), ["The Lantern Ferry Chapter 1-Chapter 2"]);
        req.chapters.truncate(1);
        req.out = dir.path().join("one");
        assert_eq!(title(&req), ["The Lantern Ferry Chapter 1"]);
        assert_eq!(book_title(&req, Some((2, 3))), "The Lantern Ferry Chapter 1 (2 of 3)");
    }

    #[test]
    fn a_send_turns_its_pages_and_says_its_language_as_the_reader_does() {
        let dir = tempfile::tempdir().unwrap();
        // A Japanese chapter that is marked manga and nothing more.
        let path = dir.path().join("ja.cbz");
        let info = ComicInfo {
            manga: Some(MangaFlag::Yes),
            language: Some("ja".into()),
            ..ComicInfo::default()
        };
        let mut w =
            CbzWriter::new(BufWriter::new(File::create(&path).unwrap()), Some(&info)).unwrap();
        w.add_page(&noise(200, 300, 1)).unwrap();
        w.finish().unwrap();
        let marked = vec![Chapter { label: "Chapter 1".into(), path }];
        let mut req = request(dir.path(), marked, "kindle", ExportFormat::FixedEpub);
        let files = go(&req).unwrap();
        assert_eq!(read_back(&files[0]), (Some(BookDirection::Rtl), vec!["ja".to_owned()]));

        // What the series says comes before its book.
        req.direction = PageDirection { said: Some(false), otherwise: true };
        req.out = dir.path().join("said");
        assert_eq!(read_back(&go(&req).unwrap()[0]).0, Some(BookDirection::Ltr));

        // A book that says nothing turns as the medium does.
        let plain = Request {
            direction: PageDirection { said: None, otherwise: true },
            out: dir.path().join("plain"),
            ..request(dir.path(), chapters(dir.path(), 1, 2), "kindle", ExportFormat::FixedEpub)
        };
        assert_eq!(
            read_back(&go(&plain).unwrap()[0]),
            (Some(BookDirection::Rtl), vec!["und".to_owned()])
        );

        // A novel keeps its chapters' language.
        let path = dir.path().join("ja.epub");
        let meta = BookMeta {
            title: "Ferry Notes".into(),
            language: "ja".into(),
            ..BookMeta::default()
        };
        let out = BufWriter::new(File::create(&path).unwrap());
        let mut w = EpubWriter::new(out, meta, EpubFlavor::Epub, None).unwrap();
        w.add_chapter("One", "<p>Lantern light.</p>").unwrap();
        w.finish().unwrap();
        let mut req = request(
            dir.path(),
            vec![Chapter { label: "Chapter 1".into(), path }],
            "kobo",
            ExportFormat::Kepub,
        );
        req.out = dir.path().join("novel");
        assert_eq!(read_back(&go(&req).unwrap()[0]).1, ["ja"]);
    }

    #[test]
    fn a_small_limit_splits_the_files_into_parts() {
        let dir = tempfile::tempdir().unwrap();
        let mut req =
            request(dir.path(), chapters(dir.path(), 3, 2), "kobo", ExportFormat::Cbz);
        // Each part carries a fixed overhead, so the limit sits just over it and holds about
        // one chapter's pages.
        req.limit = Some(512 * 1024 + 1);
        let files = go(&req).unwrap();
        assert!(files.len() > 1, "{files:?}");
        let mut pages = 0;
        for (n, file) in files.iter().enumerate() {
            assert_eq!(file.part, Some(n as u32 + 1));
            assert!(file.name.contains(&format!("({} of {})", n + 1, files.len())));
            let Ok(Book::Comic(comic)) =
                kaeri_books::open(Path::new(&file.path), &Limits::default())
            else {
                panic!("not a comic");
            };
            pages += comic.pages().len();
        }
        assert_eq!(pages, 6);
    }

    #[test]
    fn the_kindle_class_splits_and_the_others_do_not() {
        assert!(find(&Default::default(), "kindle").unwrap().split);
        assert!(!find(&Default::default(), "kobo").unwrap().split);
    }

    #[test]
    fn a_cancelled_send_leaves_nothing_behind() {
        let dir = tempfile::tempdir().unwrap();
        let req = request(dir.path(), chapters(dir.path(), 1, 2), "kobo", ExportFormat::Cbz);
        let cancel = AtomicBool::new(true);
        let result = run(&req, &mut |_, _| {}, &cancel);
        assert!(matches!(result, Err(Stop::Cancelled)));
        assert!(!req.out.exists());
    }

    #[test]
    fn progress_counts_pages_up_to_the_total() {
        let dir = tempfile::tempdir().unwrap();
        let req = request(dir.path(), chapters(dir.path(), 2, 2), "kobo", ExportFormat::Cbz);
        let mut seen = Vec::new();
        run(&req, &mut |d, t| seen.push((d, t)), &AtomicBool::new(false)).unwrap();
        assert_eq!(seen.first(), Some(&(0, 4)));
        assert_eq!(seen.last(), Some(&(4, 4)));
    }

    /// The XML documents in an EPUB, by path.
    fn documents(path: &Path) -> Vec<(String, String)> {
        let mut zip = zip::ZipArchive::new(File::open(path).unwrap()).unwrap();
        let names: Vec<String> = zip.file_names().map(|n| n.unwrap().into_owned()).collect();
        let mut out = Vec::new();
        for name in names {
            if [".xhtml", ".opf", ".xml", ".ncx"].iter().any(|e| name.ends_with(e)) {
                let mut text = String::new();
                std::io::Read::read_to_string(&mut zip.by_name(&name).unwrap(), &mut text)
                    .unwrap();
                out.push((name, text));
            }
        }
        out
    }

    #[test]
    fn a_title_beyond_ascii_keeps_its_name_and_every_document_says_utf8() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("n.epub");
        text_epub(&path, "Ferry Notes", 5);
        let title = "Ränder des Flusses: 渡し舟の灯り, Été";
        for (format, profile) in [
            (ExportFormat::Epub, "kindle"),
            (ExportFormat::Kepub, "kobo"),
            (ExportFormat::FixedEpub, "kindle"),
        ] {
            let chapters = if format == ExportFormat::FixedEpub {
                chapters(dir.path(), 1, 1)
            } else {
                vec![Chapter { label: "第一章 Départ".into(), path: path.clone() }]
            };
            let mut req = request(dir.path(), chapters, profile, format);
            req.title = title.into();
            req.out = dir.path().join(format!("out-{format:?}"));
            let files = go(&req).unwrap();
            let file = &files[0];
            assert!(file.name.contains("渡し舟の灯り"), "{}", file.name);
            assert!(file.name.contains("Ränder") && file.name.contains("Été"), "{}", file.name);
            let at = Path::new(&file.path);
            assert!(at.exists());
            check_epub(at, &Limits::default()).unwrap();
            let Ok(Book::Epub(epub)) = kaeri_books::open(at, &Limits::default()) else {
                panic!("not an EPUB");
            };
            // The title is the series' with the chapter, as the file name has it.
            let label = req.chapters[0].label.clone();
            assert_eq!(epub.metadata().titles, [format!("{title} {label}")]);
            for (name, text) in documents(at) {
                let head: String = text.chars().take(60).collect::<String>().to_lowercase();
                assert!(head.contains("encoding=\"utf-8\""), "{format:?} {name}: {head}");
                if name.ends_with(".xhtml") {
                    assert!(text.contains("<meta charset=\"utf-8\"/>"), "{format:?} {name}");
                }
            }
        }
    }

    #[test]
    fn a_file_over_the_targets_limit_is_not_sent_and_says_why() {
        let dir = tempfile::tempdir().unwrap();
        // The Kindle class splits into parts under its limit, so a set too big for one file
        // still goes out.
        let mut req =
            request(dir.path(), chapters(dir.path(), 3, 2), "kindle", ExportFormat::FixedEpub);
        req.limit = Some(512 * 1024 + 1);
        let files = go(&req).unwrap();
        assert!(files.len() > 1, "{files:?}");
        // A target that takes files up to a size, given a chapter set over it in one file.
        let mut req =
            request(dir.path(), chapters(dir.path(), 3, 2), "kobo", ExportFormat::Kepub);
        req.profile.max_file_bytes = Some(50_000);
        let Err(Stop::Failed(failure)) = go(&req) else { panic!("an oversize file was sent") };
        let ReadingFailure::SendTooLarge { file, target, bytes, limit } = &failure else {
            panic!("{failure:?}");
        };
        assert!(file.ends_with(".kepub.epub"), "{file}");
        assert_eq!(target, "Kobo");
        assert!(*bytes > 50_000);
        assert_eq!(*limit, 50_000);
        assert!(!req.out.exists(), "nothing is left to share");
        let words = failure.msg().english();
        assert!(words.contains("Kobo takes files up to 50 kB"), "{words}");
        assert!(words.ends_with("try again."), "{words}");
        // The built-in targets know their limits.
        assert_eq!(profile("kindle").max_file_bytes, Some(200_000_000));
        assert_eq!(profile("kobo").max_file_bytes, Some(4_294_967_295));
        assert_eq!(profile("tablet").max_file_bytes, None);
    }

    #[test]
    fn a_damaged_epub_fails_the_check_and_names_the_part() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("n.epub");
        text_epub(&path, "Ferry Notes", 5);
        let req = request(
            dir.path(),
            vec![Chapter { label: "Chapter 1".into(), path }],
            "kobo",
            ExportFormat::Epub,
        );
        let files = go(&req).unwrap();
        let good = Path::new(&files[0].path);
        check_epub(good, &Limits::default()).unwrap();
        let docs = documents(good);
        let opf = docs.iter().find(|(n, _)| n.ends_with(".opf")).unwrap().0.clone();
        let chapter = docs
            .iter()
            .find(|(n, t)| n.ends_with(".xhtml") && !t.contains("epub:type=\"toc\""))
            .unwrap()
            .0
            .clone();

        // Each damage, made by copying the book with one part changed, or left out for `None`.
        type Change<'a> = &'a dyn Fn(&str, Vec<u8>) -> Option<Vec<u8>>;
        let copy = |name: &str, change: Change<'_>| {
            let to = dir.path().join(name);
            let mut from = zip::ZipArchive::new(File::open(good).unwrap()).unwrap();
            let mut w = zip::ZipWriter::new(File::create(&to).unwrap());
            for i in 0..from.len() {
                let mut entry = from.by_index(i).unwrap();
                let entry_name = entry.name().unwrap().into_owned();
                let options = zip::write::SimpleFileOptions::default()
                    .compression_method(entry.compression());
                let mut data = Vec::new();
                std::io::Read::read_to_end(&mut entry, &mut data).unwrap();
                if let Some(data) = change(&entry_name, data) {
                    w.start_file(entry_name, options).unwrap();
                    std::io::Write::write_all(&mut w, &data).unwrap();
                }
            }
            w.finish().unwrap();
            to
        };
        let latin1 = |text: &str| -> Vec<u8> {
            text.replace("utf-8", "iso-8859-1").replace("UTF-8", "iso-8859-1").into_bytes()
        };
        let cases: Vec<(PathBuf, &str, ProblemKind)> = vec![
            (
                copy("no-chapter.epub", &|n, d| (n != chapter).then_some(d)),
                &chapter,
                ProblemKind::Missing,
            ),
            (
                copy("latin1.epub", &|n, d| {
                    Some(if n == opf { latin1(std::str::from_utf8(&d).unwrap()) } else { d })
                }),
                &opf,
                ProblemKind::NotUtf8,
            ),
            (
                copy("broken.epub", &|n, d| {
                    Some(if n == chapter {
                        b"<?xml version=\"1.0\" encoding=\"utf-8\"?><html><head><meta charset=\"utf-8\"/></head><body>".to_vec()
                    } else {
                        d
                    })
                }),
                &chapter,
                ProblemKind::Malformed,
            ),
            (
                copy("no-mimetype.epub", &|n, d| (n != "mimetype").then_some(d)),
                "mimetype",
                ProblemKind::Missing,
            ),
        ];
        for (path, part, kind) in cases {
            let problem = check_epub(&path, &Limits::default()).unwrap_err();
            assert_eq!((problem.part.as_str(), problem.kind), (part, kind), "{path:?}");
        }
        let failure = ReadingFailure::SendCheck {
            file: "Ferry Notes.epub".into(),
            part: opf.clone(),
            problem: SendProblem::NotUtf8,
        };
        let words = failure.msg().english();
        assert!(words.contains(&opf) && words.contains("text encoding"), "{words}");
    }

    #[test]
    fn a_format_the_device_does_not_open_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let req = request(dir.path(), chapters(dir.path(), 1, 1), "kindle", ExportFormat::Cbz);
        assert!(matches!(go(&req), Err(Stop::Failed(ReadingFailure::Unsupported { .. }))));
    }
}
