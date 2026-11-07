//! Image books: CBZ, CBR, CB7 and folders of images. Pages are the image entries in natural name
//! order, with ComicInfo.xml read when there is one.

use std::fs;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

use crate::archive::{Archive, read_capped};
use crate::comicinfo::{self, ComicInfo, ComicPageType};
use crate::{BookError, Format, Limits, Result, mime, natural, path};

/// One page of an image book.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    /// The entry's normalised name, or the file's name in a folder.
    pub name: String,
    /// The type the name says. The bytes may disagree; [`mime::sniff_image`] on what
    /// [`Comic::read_page`] returns says what they are.
    pub mime: &'static str,
    /// The declared size in bytes.
    pub size: u64,
    /// ComicInfo's type for the page, when it gives one.
    pub kind: Option<ComicPageType>,
    /// ComicInfo marks the page as a two-page spread.
    pub double_page: bool,
    entry: usize,
}

enum Store {
    Archive(Archive),
    Folder(Folder),
}

/// An open image book.
pub struct Comic {
    format: Format,
    store: Store,
    pages: Vec<Page>,
    info: Option<ComicInfo>,
    info_error: Option<String>,
}

impl std::fmt::Debug for Comic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Comic")
            .field("format", &self.format)
            .field("pages", &self.pages.len())
            .field("info", &self.info.is_some())
            .finish_non_exhaustive()
    }
}

/// Whether a name is an image page and not system clutter: `__MACOSX`, dot files, thumbnails.
fn is_page(name: &str) -> Option<&'static str> {
    let clutter = name.split('/').any(|part| {
        part.starts_with('.')
            || part.eq_ignore_ascii_case("__MACOSX")
            || part.eq_ignore_ascii_case("Thumbs.db")
    });
    if clutter { None } else { mime::page_image(name) }
}

fn is_comicinfo(name: &str) -> bool {
    path::file_name(name).eq_ignore_ascii_case("ComicInfo.xml")
}

impl Comic {
    /// Reads the page list and ComicInfo from an open archive. A damaged ComicInfo.xml does not
    /// stop the book from opening; [`Comic::info_error`] says what was wrong with it.
    pub fn from_archive(mut archive: Archive, limits: &Limits) -> Result<Self> {
        let format = archive.kind().format();
        let mut pages: Vec<Page> = archive
            .entries()
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                is_page(&e.name).map(|mime| Page {
                    name: e.name.clone(),
                    mime,
                    size: e.size,
                    kind: None,
                    double_page: false,
                    entry: i,
                })
            })
            .collect();
        pages.sort_by(|a, b| natural::compare(&a.name, &b.name));
        // The root ComicInfo.xml wins over one in a subfolder.
        let info_entry = {
            let entries = archive.entries();
            entries
                .iter()
                .position(|e| e.name.eq_ignore_ascii_case("ComicInfo.xml"))
                .or_else(|| entries.iter().position(|e| is_comicinfo(&e.name)))
        };
        let (info, info_error) = match info_entry {
            Some(i) if archive.entries()[i].size <= limits.max_xml_bytes => {
                match archive.read(i).and_then(|b| comicinfo::parse(&b, format, limits)) {
                    Ok(info) => (Some(info), None),
                    Err(BookError::Protected) => return Err(BookError::Protected),
                    Err(e) => (None, Some(e.to_string())),
                }
            }
            Some(_) => (None, Some("ComicInfo.xml is over the size limit".to_owned())),
            None => (None, None),
        };
        Self::finish(format, Store::Archive(archive), pages, info, info_error)
    }

    /// Opens a folder of images. Only the folder's own files count: subfolders, links and
    /// anything that is not a regular file are passed over.
    pub fn open_folder(dir: &Path, limits: &Limits) -> Result<Self> {
        let folder = Folder::scan(dir, limits)?;
        let mut pages: Vec<Page> = folder
            .files
            .iter()
            .enumerate()
            .filter_map(|(i, (name, size))| {
                is_page(name).map(|mime| Page {
                    name: name.clone(),
                    mime,
                    size: *size,
                    kind: None,
                    double_page: false,
                    entry: i,
                })
            })
            .collect();
        pages.sort_by(|a, b| natural::compare(&a.name, &b.name));
        let info_at = folder.files.iter().position(|(n, _)| is_comicinfo(n));
        let (info, info_error) = match info_at {
            Some(i) => match folder
                .read(i, limits.max_xml_bytes)
                .and_then(|b| comicinfo::parse(&b, Format::Folder, limits))
            {
                Ok(info) => (Some(info), None),
                Err(e) => (None, Some(e.to_string())),
            },
            None => (None, None),
        };
        Self::finish(Format::Folder, Store::Folder(folder), pages, info, info_error)
    }

    fn finish(
        format: Format,
        store: Store,
        mut pages: Vec<Page>,
        info: Option<ComicInfo>,
        info_error: Option<String>,
    ) -> Result<Self> {
        if pages.is_empty() {
            return Err(BookError::Empty);
        }
        if let Some(info) = &info {
            for (i, page) in pages.iter_mut().enumerate() {
                if let Some(p) = info.page(i) {
                    page.kind = p.kind;
                    page.double_page = p.double_page;
                }
            }
        }
        Ok(Self { format, store, pages, info, info_error })
    }

    pub fn format(&self) -> Format {
        self.format
    }

    /// The pages in reading order.
    pub fn pages(&self) -> &[Page] {
        &self.pages
    }

    pub fn info(&self) -> Option<&ComicInfo> {
        self.info.as_ref()
    }

    /// Why ComicInfo.xml was passed over, when the book has one that could not be read.
    pub fn info_error(&self) -> Option<&str> {
        self.info_error.as_deref()
    }

    /// Whether pages are best read once in order with [`Comic::extract_pages`]: a solid RAR or
    /// 7z decodes every earlier page to reach a later one.
    pub fn is_solid(&self) -> bool {
        match &self.store {
            Store::Archive(a) => a.is_solid(),
            Store::Folder(_) => false,
        }
    }

    /// Reads one page's bytes, as stored: the platform or the image pipeline decodes them. In a
    /// solid archive this decodes every earlier entry first; see [`Comic::is_solid`].
    pub fn read_page(&mut self, index: usize) -> Result<Vec<u8>> {
        let page = self
            .pages
            .get(index)
            .ok_or_else(|| BookError::missing(format!("page {}", index + 1)))?;
        let entry = page.entry;
        match &mut self.store {
            Store::Archive(a) => a.read(entry),
            Store::Folder(f) => f.read(entry, u64::MAX),
        }
    }

    /// Hands every page from `from` on to `each` in reading order, until `each` breaks. A solid
    /// archive is decoded once from the start; pages earlier than `from` are decoded and dropped.
    /// Pages arrive in archive order when it differs from reading order; the index passed is the
    /// page's place in [`Comic::pages`].
    pub fn extract_pages(
        &mut self,
        from: usize,
        each: &mut dyn FnMut(usize, Vec<u8>) -> ControlFlow<()>,
    ) -> Result<()> {
        let page_of_entry: std::collections::HashMap<usize, usize> =
            self.pages.iter().enumerate().skip(from).map(|(i, p)| (p.entry, i)).collect();
        match &mut self.store {
            Store::Archive(a) => a
                .extract(&|entry| page_of_entry.contains_key(&entry), &mut |entry, data| {
                    each(page_of_entry[&entry], data)
                }),
            Store::Folder(f) => {
                for i in from..self.pages.len() {
                    let data = f.read(self.pages[i].entry, u64::MAX)?;
                    if each(i, data).is_break() {
                        break;
                    }
                }
                Ok(())
            }
        }
    }
}

/// A folder's regular files, listed once.
struct Folder {
    dir: PathBuf,
    files: Vec<(String, u64)>,
    max_entry_bytes: u64,
}

impl Folder {
    fn scan(dir: &Path, limits: &Limits) -> Result<Self> {
        let mut files = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if files.len() >= limits.max_entries {
                return Err(BookError::TooLarge {
                    what: "entry count",
                    limit: limits.max_entries as u64,
                });
            }
            // symlink_metadata: a link is passed over, never followed out of the folder.
            let meta = entry.path().symlink_metadata()?;
            if !meta.file_type().is_file() {
                continue;
            }
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if path::entry_name(&name, limits.max_name_bytes, 1).as_deref()
                != Some(name.as_str())
            {
                continue;
            }
            files.push((name, meta.len()));
        }
        Ok(Self { dir: dir.to_owned(), files, max_entry_bytes: limits.max_entry_bytes })
    }

    fn read(&self, index: usize, cap: u64) -> Result<Vec<u8>> {
        let (name, size) = &self.files[index];
        let cap = cap.min(self.max_entry_bytes);
        if *size > cap {
            return Err(BookError::TooLarge { what: "entry size", limit: cap });
        }
        let path = self.dir.join(name);
        // Checked again at read time: the file could have been swapped for a link since the scan.
        if !path.symlink_metadata()?.file_type().is_file() {
            return Err(BookError::missing(name.clone()));
        }
        read_capped(fs::File::open(path)?, *size, cap)
    }
}
