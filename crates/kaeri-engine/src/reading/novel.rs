//! Novels in the platform web view: the open book served through the platform's scheme, its
//! documents cleaned, and the paginator's settings.
//!
//! The web view asks for `kaeri-book://<session>/<path>` (Apple, WebView2, WebKitGTK) or
//! `https://appassets.androidplatform.net/book/<session>/<path>` (Android). The scheme handler
//! calls [`NovelBooks::resource`] on whatever thread it runs on: it reads the book directly,
//! never through the event loop, so a page never waits behind a command. Every answer carries
//! the content security policy from `kaeri_books::novel::csp`.
//!
//! What is served:
//! - XHTML: cleaned (`kaeri_books::novel::document`), soft hyphens on Windows and Linux, the
//!   reader's settings in a `<meta name="kaeri-setup">`; kept in memory while the book is open,
//!   and never cached by the web view, as the settings in it change.
//! - CSS and SVG: cleaned once and kept in memory while the book is open.
//! - Images: written into the cache and answered as files; on e-ink processed for the panel
//!   first (grey levels, tone, colour), as manga pages are.
//! - Fonts: written into the cache once and answered as files; audio and video as files.
//!   Scripts and anything else: 403.
//!
//! Everything but a document may be cached by the web view: a URL names one open book's file,
//! which never changes while the book is open, so the book's fonts and stylesheets are read
//! once for all of its documents.

use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, RwLock},
};

use kaeri_books::{
    Book, Epub, Limits,
    novel::{self, Clean, Document},
};
use kaeri_model::{
    eink::NovelSupport,
    playback::HttpHeader,
    reader::{
        BookBody, BookResource, NovelAssets, ReadDirection, ReaderSettingKey as Key,
        ReaderSurface, WritingMode,
    },
    reading::ReadingFailure,
};
use kaeri_pages::{Look, Panel, Renderer, Target, resize::Fit};

use super::settings::Layers;

/// A stylesheet or picture kept for an open book: its type and its bytes.
type Kept = (&'static str, Arc<[u8]>);

/// The path a source's text chapter is served at.
pub const TEXT_CHAPTER: &str = "chapter.xhtml";
/// The page's foot on a desktop: room for the reader's floating bottom group (about 90 px
/// with 20 under it on the Mac, 80 and 20 on Linux, 80 and 16 on Windows) and a gap, so the
/// text stops above the group whether it shows or not, as the reader board's desktop frames
/// have it.
const DESKTOP_FOOT: f64 = 124.0;
/// Cleaned documents kept per book; a chapter read through holds a few.
const KEPT_DOCUMENTS: usize = 24;
/// Cleaned stylesheets and pictures kept per book. A book links a handful; this only bounds a
/// hostile one.
const KEPT_FILES: usize = 256;
/// How long a web view may keep a book's file other than a document: a day, more than any
/// reading session, and the URL names the open book, so nothing stale is ever shown.
const KEEP_FOR: &str = "private, max-age=86400, immutable";

/// What every host injects: the paginator, its stylesheet and the policy.
pub fn assets() -> NovelAssets {
    NovelAssets {
        script: novel::script(),
        stylesheet: novel::STYLESHEET.to_owned(),
        csp: novel::csp(),
    }
}

/// Where the platform's web view loads books from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheme {
    /// `kaeri-book://<session>/<path>`.
    Kaeri,
    /// `https://appassets.androidplatform.net/book/<session>/<path>` (WebViewAssetLoader).
    AndroidAssets,
}

const KAERI_PREFIX: &str = "kaeri-book://";
const ANDROID_PREFIX: &str = "https://appassets.androidplatform.net/book/";

impl Scheme {
    /// This build's.
    pub const fn native() -> Self {
        if cfg!(target_os = "android") { Self::AndroidAssets } else { Self::Kaeri }
    }
}

/// The books open in a web view, by session. Shared between the engine's actor, which opens
/// and closes them, and the scheme handler's threads.
#[derive(Debug)]
pub struct NovelBooks {
    books: RwLock<HashMap<u64, Arc<ServedBook>>>,
    scheme: Scheme,
}

impl Default for NovelBooks {
    fn default() -> Self {
        Self { books: RwLock::default(), scheme: Scheme::native() }
    }
}

impl NovelBooks {
    #[cfg(test)]
    pub fn with_scheme(scheme: Scheme) -> Self {
        Self { books: RwLock::default(), scheme }
    }

    /// The URL of a file of the session's book.
    pub fn url(&self, session: u64, path: &str) -> String {
        let encoded: Vec<String> = path.split('/').map(encode_segment).collect();
        let prefix = match self.scheme {
            Scheme::Kaeri => KAERI_PREFIX,
            Scheme::AndroidAssets => ANDROID_PREFIX,
        };
        format!("{prefix}{session}/{}", encoded.join("/"))
    }

    /// The answer to one request from a web view. `None` for a URL that is not a book's at all,
    /// which the host lets fail; a book's file that is missing or refused is a 404 or 403.
    pub fn resource(&self, url: &str) -> Option<BookResource> {
        let (session, path) = parse_url(url)?;
        let book = self.books.read().ok()?.get(&session).cloned();
        Some(match (book, path) {
            (Some(book), Some(path)) => book.resource(&path),
            _ => respond(404, "text/plain", BookBody::Bytes { data: Vec::new() }),
        })
    }

    pub(crate) fn put(&self, book: Arc<ServedBook>) {
        if let Ok(mut books) = self.books.write() {
            // One reader at a time: an older session's book goes.
            for (_, old) in books.drain() {
                old.forget();
            }
            books.insert(book.session, book);
        }
    }

    pub(crate) fn remove(&self, session: u64) {
        if let Some(book) = self.books.write().ok().and_then(|mut b| b.remove(&session)) {
            book.forget();
        }
    }
}

fn encode_segment(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~!$&'()*+,;=:@".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The session and the normalised path in the book: `None` for a URL off both origins,
/// `Some((session, None))` for one whose path leaves the book.
fn parse_url(url: &str) -> Option<(u64, Option<String>)> {
    let rest = url.strip_prefix(KAERI_PREFIX).or_else(|| url.strip_prefix(ANDROID_PREFIX))?;
    let rest = rest.split(['?', '#']).next().unwrap_or_default();
    let (session, path) = rest.split_once('/')?;
    let session = session.parse().ok()?;
    let path = kaeri_books::path::percent_decode(path).and_then(|p| {
        let limits = Limits::default();
        kaeri_books::path::entry_name(&p, limits.max_name_bytes, limits.max_path_depth)
    });
    Some((session, path))
}

fn respond(status: u16, mime: &str, body: BookBody) -> BookResource {
    answer(status, mime, body, "no-store")
}

/// A book's file that does not change while the book is open: the web view may keep it.
fn kept(mime: &str, body: BookBody) -> BookResource {
    answer(200, mime, body, KEEP_FOR)
}

fn answer(status: u16, mime: &str, body: BookBody, cache: &str) -> BookResource {
    let content_type = if mime.starts_with("text/") || mime.ends_with("+xml") {
        format!("{mime}; charset=utf-8")
    } else {
        mime.to_owned()
    };
    let header = |name: &str, value: String| HttpHeader { name: name.to_owned(), value };
    BookResource {
        status,
        mime: mime.to_owned(),
        headers: vec![
            header("Content-Type", content_type),
            header("Content-Security-Policy", novel::csp()),
            header("X-Content-Type-Options", "nosniff".into()),
            header("Referrer-Policy", "no-referrer".into()),
            header("Cache-Control", cache.into()),
            header("Cross-Origin-Resource-Policy", "same-origin".into()),
        ],
        body,
    }
}

fn refused() -> BookResource {
    respond(403, "text/plain", BookBody::Bytes { data: Vec::new() })
}

fn missing() -> BookResource {
    respond(404, "text/plain", BookBody::Bytes { data: Vec::new() })
}

/// How a book's pictures are prepared on e-ink.
#[derive(Debug, Clone, Copy)]
pub(crate) struct EinkImages {
    pub target: Target,
    pub look: Look,
}

/// What opening a book needs from the session.
#[derive(Debug, Clone)]
pub(crate) struct Opening {
    pub session: u64,
    pub soft_hyphens: bool,
    /// The cache folder for this session's files.
    pub dir: PathBuf,
    pub eink: Option<EinkImages>,
    pub limits: Limits,
}

enum Content {
    Epub(Mutex<Box<Epub>>),
    Text(Arc<Document>),
}

/// One open book, as the web view sees it.
pub(crate) struct ServedBook {
    pub session: u64,
    content: Content,
    lang: Option<String>,
    soft_hyphens: bool,
    limits: Limits,
    /// The paginator's settings, as JSON, without the per-document part.
    style: RwLock<String>,
    /// Each spine document's normalised path.
    spine: Vec<String>,
    /// The last document in the reading flow.
    last_linear: usize,
    docs: Mutex<HashMap<String, Arc<Document>>>,
    /// Cleaned stylesheets and SVG pictures, by path: (MIME type, bytes).
    files: Mutex<HashMap<String, Kept>>,
    dir: PathBuf,
    eink: Option<EinkImages>,
}

impl std::fmt::Debug for ServedBook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServedBook")
            .field("session", &self.session)
            .field("spine", &self.spine.len())
            .finish_non_exhaustive()
    }
}

impl ServedBook {
    /// Chinese, Japanese or Korean: the serif setting keeps the book's own font.
    pub fn cjk(&self) -> bool {
        let primary = self.lang.as_deref().and_then(|l| l.split(['-', '_']).next());
        primary.is_some_and(|p| ["ja", "zh", "ko"].iter().any(|c| p.eq_ignore_ascii_case(c)))
    }

    pub fn set_style(&self, style: String) {
        if let Ok(mut s) = self.style.write() {
            *s = style;
        }
    }

    /// Drops the files written for this session.
    fn forget(&self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }

    /// The settings for the document at spine index `spine`, as the paginator reads them.
    fn setup(&self, spine: Option<usize>) -> String {
        let style = self.style.read().map(|s| s.clone()).unwrap_or_default();
        let index = spine.unwrap_or(0);
        let last = spine.is_some_and(|i| i >= self.last_linear);
        let head = format!("{{\"spine\":{index},\"last\":{last}");
        match style.trim().strip_prefix('{') {
            Some(rest) if rest.trim_start().starts_with('}') => format!("{head}}}"),
            Some(rest) => format!("{head},{rest}"),
            None => format!("{head}}}"),
        }
    }

    fn clean<'a>(&'a self, path: &'a str) -> Clean<'a> {
        Clean {
            path,
            lang: self.lang.as_deref(),
            soft_hyphens: self.soft_hyphens,
            limits: &self.limits,
        }
    }

    fn served_document(&self, path: &str, doc: &Document) -> BookResource {
        let spine = self.spine.iter().position(|p| p == path);
        let text = doc.served(&self.setup(spine));
        respond(200, "application/xhtml+xml", BookBody::Bytes { data: text.into_bytes() })
    }

    /// The cleaned document at `path`, from memory or cleaned now.
    fn document(&self, path: &str, bytes: &[u8]) -> Arc<Document> {
        if let Some(doc) = self.docs.lock().ok().and_then(|d| d.get(path).cloned()) {
            return doc;
        }
        let doc = Arc::new(novel::document(bytes, &self.clean(path)));
        if let Ok(mut docs) = self.docs.lock() {
            if docs.len() >= KEPT_DOCUMENTS {
                docs.clear();
            }
            docs.insert(path.to_owned(), doc.clone());
        }
        doc
    }

    pub fn resource(&self, path: &str) -> BookResource {
        let epub = match &self.content {
            Content::Text(doc) if path == TEXT_CHAPTER => {
                return self.served_document(path, doc);
            }
            Content::Text(_) => return missing(),
            Content::Epub(epub) => epub,
        };
        let cached = self.docs.lock().ok().and_then(|d| d.get(path).cloned());
        if let Some(doc) = cached {
            return self.served_document(path, &doc);
        }
        if let Some((mime, bytes)) = self.files.lock().ok().and_then(|f| f.get(path).cloned()) {
            return kept(mime, BookBody::Bytes { data: bytes.to_vec() });
        }
        let resource = {
            let Ok(mut epub) = epub.lock() else { return missing() };
            if !epub.contains(path) {
                return missing();
            }
            match epub.resource(path) {
                Ok(r) => r,
                Err(e) => {
                    tracing::debug!(error = %e, path, "a book file did not read");
                    return missing();
                }
            }
        };
        let mime = resource.media_type.to_ascii_lowercase();
        let mime = mime.split(';').next().unwrap_or_default().trim().to_owned();
        match mime.as_str() {
            "application/xhtml+xml" | "text/html" | "application/xml" | "text/xml" => {
                let doc = self.document(path, &resource.data);
                self.served_document(path, &doc)
            }
            "image/svg+xml" => match novel::svg(&resource.data, &self.clean(path)) {
                Some(svg) => self.keep(path, "image/svg+xml", svg.into_bytes()),
                None => refused(),
            },
            "text/css" => {
                let text = String::from_utf8_lossy(&resource.data);
                let css = novel::stylesheet(&text, path, &self.limits);
                self.keep(path, "text/css", css.into_bytes())
            }
            m if kaeri_books::mime::is_image(m) => self.image(path, m, &resource.data),
            // Written out once, however many documents ask: a font can be megabytes, and a
            // file is answered without reading it into memory again.
            m if is_font(m, path) => match self.write_file(path, &resource.data, None) {
                Some(file) => kept(font_mime(m, path), BookBody::File { path: file }),
                None => kept(font_mime(m, path), BookBody::Bytes { data: resource.data }),
            },
            m if m.starts_with("audio/") || m.starts_with("video/") => {
                match self.write_file(path, &resource.data, None) {
                    Some(file) => respond(200, m, BookBody::File { path: file }),
                    None => missing(),
                }
            }
            _ => refused(),
        }
    }

    /// A cleaned stylesheet or SVG picture, kept for the next document that asks.
    fn keep(&self, path: &str, mime: &'static str, bytes: Vec<u8>) -> BookResource {
        if let Ok(mut files) = self.files.lock() {
            if files.len() >= KEPT_FILES {
                files.clear();
            }
            files.insert(path.to_owned(), (mime, Arc::from(bytes.as_slice())));
        }
        kept(mime, BookBody::Bytes { data: bytes })
    }

    /// A picture as a file; on e-ink prepared for the panel.
    fn image(&self, path: &str, mime: &str, bytes: &[u8]) -> BookResource {
        if let Some(eink) = self.eink
            && let Some(png) = eink_image(bytes, eink)
        {
            return match self.write_file(path, &png, Some("png")) {
                Some(file) => kept("image/png", BookBody::File { path: file }),
                None => missing(),
            };
        }
        let mime = kaeri_books::mime::sniff_image(bytes).unwrap_or(mime);
        match self.write_file(path, bytes, None) {
            Some(file) => kept(mime, BookBody::File { path: file }),
            None => missing(),
        }
    }

    /// Writes a book file into the session's folder once, under a name of its own, whole.
    fn write_file(&self, path: &str, bytes: &[u8], ext: Option<&str>) -> Option<String> {
        let mut h = DefaultHasher::new();
        path.hash(&mut h);
        let ext = ext.map(str::to_owned).or_else(|| kaeri_books::path::extension(path));
        let name = match ext {
            Some(e) if e.chars().all(|c| c.is_ascii_alphanumeric()) && e.len() <= 5 => {
                format!("{:016x}.{e}", h.finish())
            }
            _ => format!("{:016x}", h.finish()),
        };
        let file = self.dir.join(name);
        let whole = std::fs::metadata(&file).is_ok_and(|m| m.len() == bytes.len() as u64);
        if !whole {
            std::fs::create_dir_all(&self.dir).ok()?;
            let temp = self.dir.join(format!(".{}.tmp", h.finish()));
            std::fs::write(&temp, bytes).ok()?;
            std::fs::rename(&temp, &file).ok()?;
        }
        Some(file.display().to_string())
    }
}

fn is_font(mime: &str, path: &str) -> bool {
    mime.starts_with("font/")
        || mime.starts_with("application/font")
        || mime.starts_with("application/x-font")
        || mime == "application/vnd.ms-opentype"
        || mime == "application/vnd.ms-fontobject"
        || (mime == "application/octet-stream"
            && matches!(
                kaeri_books::path::extension(path).as_deref(),
                Some("otf" | "ttf" | "woff" | "woff2")
            ))
}

fn font_mime<'a>(mime: &'a str, path: &str) -> &'a str {
    match kaeri_books::path::extension(path).as_deref() {
        Some("otf") => "font/otf",
        Some("ttf") => "font/ttf",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        _ => mime,
    }
}

/// A picture for an e-ink panel: grey (or the colour panel's colours), toned, never larger
/// than the panel or the picture, as a PNG. The web view scales it, so it is not dithered.
fn eink_image(bytes: &[u8], eink: EinkImages) -> Option<Vec<u8>> {
    let mut renderer = Renderer::new(eink.target, eink.look);
    let page = renderer.open(bytes).ok()?;
    let width = eink.target.width.min(page.bitmap.width()).max(1);
    let height = eink.target.height.min(page.bitmap.height()).max(1);
    let scaled = renderer.scale(&page, None, Fit::Within { width, height }, false).ok()?;
    let bitmap = if eink.target.panel == Panel::Grey || !page.colour {
        scaled.into_gray()
    } else {
        renderer.finish(scaled, page.colour).ok()?.bitmap.into_rgba()
    };
    let (w, h) = (bitmap.width(), bitmap.height());
    let grey = bitmap.format() == kaeri_pages::PixelFormat::Gray8;
    let row = w as usize * if grey { 1 } else { 4 };
    let mut packed = Vec::with_capacity(row * h as usize);
    for line in bitmap.data().chunks(bitmap.stride()).take(h as usize) {
        packed.extend_from_slice(line.get(..row)?);
    }
    let mut out = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut out);
    if grey {
        image::GrayImage::from_raw(w, h, packed)?
            .write_to(&mut cursor, image::ImageFormat::Png)
            .ok()?;
    } else {
        image::RgbaImage::from_raw(w, h, packed)?
            .write_to(&mut cursor, image::ImageFormat::Png)
            .ok()?;
    }
    Some(out)
}

/// A book ready for the web view.
#[derive(Debug)]
pub(crate) struct NovelBook {
    pub served: Arc<ServedBook>,
    /// Each spine document's path and whether it is in the reading flow.
    pub spine: Vec<(String, bool)>,
    /// Contents lines: label, `path#fragment`, spine index, depth.
    pub toc: Vec<(String, String, u32, u32)>,
    pub writing_mode: WritingMode,
    pub progression: ReadDirection,
    pub fixed: bool,
    /// What counts are kept under, before the settings and the screen.
    pub identity: String,
    /// Each spine document's words: see [`words`].
    pub words: Vec<u32>,
}

impl NovelBook {
    pub fn first_linear(&self) -> u32 {
        self.spine.iter().position(|(_, linear)| *linear).unwrap_or(0) as u32
    }

    pub fn last_linear(&self) -> u32 {
        self.spine.iter().rposition(|(_, linear)| *linear).unwrap_or(0) as u32
    }
}

/// What an EPUB turned out to be.
#[derive(Debug)]
pub(crate) enum Opened {
    /// Pictures only: read like manga, through the page worker.
    Pages,
    Novel(NovelBook),
}

/// Opens an EPUB for the web view. Blocking: run off the actor.
pub(crate) fn open_epub(path: &Path, opening: Opening) -> Result<Opened, ReadingFailure> {
    let shown = path.display().to_string();
    let format = "EPUB".to_owned();
    let book = kaeri_books::open(path, &opening.limits)
        .map_err(|e| super::pages::failure_of(&e, shown.clone(), format.clone()))?;
    let Book::Epub(mut epub) = book else {
        return Err(ReadingFailure::Unsupported {
            extension: path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase(),
        });
    };
    if epub.image_pages().is_some() {
        return Ok(Opened::Pages);
    }
    let lang = epub.metadata().languages.first().cloned();
    let spine: Vec<(String, bool)> =
        epub.spine().map(|(s, item)| (item.path.clone(), s.linear)).collect();
    if spine.is_empty() {
        return Err(ReadingFailure::Unreadable { path: shown, format });
    }
    let mut toc = Vec::new();
    flatten_toc(epub.toc(), &spine, 0, &mut toc);
    let fixed = epub.is_fixed_layout();
    let rtl = epub.package().direction == Some(kaeri_books::epub::Direction::Rtl);
    let identity = {
        let mut h = DefaultHasher::new();
        shown.hash(&mut h);
        std::fs::metadata(path).map(|m| m.len()).unwrap_or(0).hash(&mut h);
        epub.metadata().identifier.hash(&mut h);
        format!("{:016x}", h.finish())
    };
    let last_linear = spine.iter().rposition(|(_, l)| *l).unwrap_or(0);
    let words = spine
        .iter()
        .map(|(path, _)| {
            epub.resource(path).map_or(0, |r| words(&String::from_utf8_lossy(&r.data)))
        })
        .collect();
    let served = Arc::new(ServedBook {
        session: opening.session,
        content: Content::Epub(Mutex::new(epub)),
        lang,
        soft_hyphens: opening.soft_hyphens,
        limits: opening.limits.clone(),
        style: RwLock::default(),
        spine: spine.iter().map(|(p, _)| p.clone()).collect(),
        last_linear,
        docs: Mutex::default(),
        files: Mutex::default(),
        dir: opening.dir,
        eink: opening.eink,
    });
    let first = spine.iter().position(|(_, l)| *l).unwrap_or(0);
    let flow = flow_of(&served, &spine[first].0);
    let writing_mode = match flow {
        novel::Flow::VerticalRl => WritingMode::VerticalRightToLeft,
        novel::Flow::VerticalLr => WritingMode::VerticalLeftToRight,
        novel::Flow::Horizontal => WritingMode::Horizontal,
    };
    let progression = if rtl || flow == novel::Flow::VerticalRl {
        ReadDirection::RightToLeft
    } else {
        ReadDirection::LeftToRight
    };
    Ok(Opened::Novel(NovelBook {
        served,
        spine,
        toc,
        writing_mode,
        progression,
        fixed,
        identity,
        words,
    }))
}

/// The writing mode of the first document, from its own styles and its stylesheets.
fn flow_of(book: &ServedBook, path: &str) -> novel::Flow {
    let Content::Epub(epub) = &book.content else { return novel::Flow::Horizontal };
    let bytes = match epub.lock().ok().and_then(|mut e| e.resource(path).ok()) {
        Some(r) => r.data,
        None => return novel::Flow::Horizontal,
    };
    let doc = book.document(path, &bytes);
    if let Some(flow) = doc.flow {
        return flow;
    }
    for sheet in &doc.stylesheets {
        let css = epub.lock().ok().and_then(|mut e| e.resource(sheet).ok());
        if let Some(flow) =
            css.and_then(|c| novel::vertical_flow(&String::from_utf8_lossy(&c.data)))
        {
            return flow;
        }
    }
    novel::Flow::Horizontal
}

fn flatten_toc(
    entries: &[kaeri_books::epub::TocEntry],
    spine: &[(String, bool)],
    depth: u32,
    out: &mut Vec<(String, String, u32, u32)>,
) {
    for e in entries {
        if let Some(href) = &e.href {
            let path = href.split('#').next().unwrap_or_default();
            if let Some(index) = spine.iter().position(|(p, _)| p == path) {
                out.push((e.label.clone(), href.clone(), index as u32, depth));
            }
        }
        if out.len() < 5000 {
            flatten_toc(&e.children, spine, depth + 1, out);
        }
    }
}

/// A source's text chapter, ready for the web view.
pub(crate) fn open_text(
    xhtml: &str,
    title: &str,
    lang: Option<String>,
    identity: String,
    opening: Opening,
) -> NovelBook {
    let clean = Clean {
        path: TEXT_CHAPTER,
        lang: lang.as_deref(),
        soft_hyphens: opening.soft_hyphens,
        limits: &opening.limits,
    };
    let doc = novel::text_chapter(xhtml, title, &clean);
    let flow = doc.flow.unwrap_or(novel::Flow::Horizontal);
    let served = Arc::new(ServedBook {
        session: opening.session,
        content: Content::Text(Arc::new(doc)),
        lang,
        soft_hyphens: opening.soft_hyphens,
        limits: opening.limits.clone(),
        style: RwLock::default(),
        spine: vec![TEXT_CHAPTER.to_owned()],
        last_linear: 0,
        docs: Mutex::default(),
        files: Mutex::default(),
        dir: opening.dir,
        eink: opening.eink,
    });
    let vertical = flow != novel::Flow::Horizontal;
    NovelBook {
        served,
        spine: vec![(TEXT_CHAPTER.to_owned(), true)],
        toc: Vec::new(),
        writing_mode: match flow {
            novel::Flow::VerticalRl => WritingMode::VerticalRightToLeft,
            novel::Flow::VerticalLr => WritingMode::VerticalLeftToRight,
            novel::Flow::Horizontal => WritingMode::Horizontal,
        },
        progression: if vertical && flow == novel::Flow::VerticalRl {
            ReadDirection::RightToLeft
        } else {
            ReadDirection::LeftToRight
        },
        fixed: false,
        identity,
        words: vec![words(xhtml)],
    }
}

/// A document's words, for the time left in it: the words of its body, two Chinese, Japanese
/// or Korean characters counting as one (they read about twice as many characters a minute as
/// other scripts read words).
pub(crate) fn words(markup: &str) -> u32 {
    let body = markup
        .find("<body")
        .and_then(|at| markup[at..].find('>').map(|end| at + end + 1))
        .map_or(markup, |start| &markup[start..]);
    let (mut words, mut cjk, mut in_tag, mut in_word) = (0u32, 0u32, false, false);
    let mut entity = false;
    for c in body.chars() {
        match c {
            '<' => {
                in_tag = true;
                in_word = false;
            }
            '>' if in_tag => in_tag = false,
            _ if in_tag => {}
            // "&amp;" and "&#8217;" are one character, not a word.
            '&' => entity = true,
            ';' if entity => entity = false,
            c if entity && c.is_alphanumeric() || c == '#' && entity => {}
            c if is_cjk(c) => {
                cjk = cjk.saturating_add(1);
                in_word = false;
            }
            c if c.is_alphanumeric() || (in_word && matches!(c, '\'' | '\u{2019}' | '-')) => {
                if !in_word {
                    words = words.saturating_add(1);
                    in_word = true;
                }
            }
            _ => {
                entity = false;
                in_word = false;
            }
        }
    }
    words.saturating_add(cjk / 2)
}

fn is_cjk(c: char) -> bool {
    matches!(u32::from(c),
        0x3040..=0x30FF   // kana
        | 0x3400..=0x4DBF // CJK extension A
        | 0x4E00..=0x9FFF // CJK unified ideographs
        | 0xAC00..=0xD7AF // Hangul syllables
        | 0xF900..=0xFAFF // compatibility ideographs
        | 0x20000..=0x2FA1F)
}

/// The looks' colours (the reader board). `ink` turns the book's own colours off.
fn look(
    word: &str,
    surface: &ReaderSurface,
) -> (&'static str, &'static str, &'static str, bool) {
    if surface.eink {
        // E-ink: paper and ink, or the dark look inverted.
        return if word == "dark" {
            ("#000000", "#FFFFFF", "#FFFFFF", true)
        } else {
            ("#FFFFFF", "#000000", "#000000", true)
        };
    }
    let word =
        if word == "system" { if surface.dark { "dark" } else { "light" } } else { word };
    match word {
        "paper" => ("#EFE9DC", "#2A2620", "#2A2620", false),
        "sepia" => ("#E9DCC4", "#3B2F22", "#3B2F22", false),
        "dark" => ("#161A2E", "#D9D5CB", "#D9D5CB", true),
        _ => ("#F7F5F0", "#1E2033", "#1E2033", false),
    }
}

/// The characters a line of a novel holds on a wide screen, at least and at most: the
/// paginator's columns keep to it.
const MEASURE: [u32; 2] = [65, 75];

/// Kaeri Serif CJK is the system's own Mincho face (reader.css), named before the generic serif,
/// which falls back to a sans face for kana and kanji on Android.
const SERIF: &str =
    "\"Literata\", \"Iowan Old Style\", Charter, Georgia, \"Kaeri Serif CJK\", serif";
const SANS: &str = "system-ui, -apple-system, \"Segoe UI\", Roboto, \"Noto Sans\", Helvetica, Arial, sans-serif";

/// The paginator's settings as JSON, from the reader's settings and the screen.
pub(crate) fn style(
    layers: &Layers,
    surface: &ReaderSurface,
    handheld: bool,
    support: NovelSupport,
    fixed: bool,
    cjk: bool,
) -> String {
    let eink = surface.eink;
    let (bg, fg, link, ink) = look(&layers.word(Key::Look), surface);
    // CJK books keep their own (or the system's) font for serif: Literata has no kana.
    let font = match layers.word(Key::Font).as_str() {
        "sans" => serde_json::Value::from(SANS),
        _ if cjk => serde_json::Value::Null,
        _ => serde_json::Value::from(SERIF),
    };
    let base = if eink {
        20.0
    } else if handheld {
        17.0
    } else {
        19.0
    };
    let size = (base * layers.amount(Key::TextSize).clamp(0.5, 3.0) * 2.0).round() / 2.0;
    let margin_base = if eink {
        26.0
    } else if handheld {
        24.0
    } else {
        40.0
    };
    let margin = (margin_base * layers.amount(Key::Margins).clamp(0.0, 4.0)).round();
    let top = if handheld { 64.0 } else { 48.0 };
    let scrolled = !eink && layers.word(Key::Mode) == "scrolled";
    let value = serde_json::json!({
        "look": { "name": layers.word(Key::Look), "bg": bg, "fg": fg, "link": link, "ink": ink },
        "font": font,
        "size": size,
        "line": (layers.amount(Key::LineSpacing).clamp(1.0, 3.0) * 100.0).round() / 100.0,
        "margin": margin,
        "top": top,
        "bottom": if handheld { top + 8.0 } else { DESKTOP_FOOT },
        "justify": layers.on(Key::Justify),
        "hyphens": true,
        "mode": if scrolled { "scrolled" } else { "paged" },
        "columns": if eink { serde_json::Value::from(1) } else { serde_json::Value::from("auto") },
        "column": 720,
        // A line holds between these many characters: two columns only where each holds the
        // first, and no column wider than the second, so a wide screen grows its margins.
        "measure": MEASURE,
        "eink": eink,
        "legacy": support == NovelSupport::KnownFlaws,
        "fixed": fixed,
    });
    value.to_string()
}

/// What whole-volume counts are kept under: the book, the settings and the viewport.
pub(crate) fn count_key(identity: &str, style: &str, surface: &ReaderSurface) -> String {
    let mut h = DefaultHasher::new();
    identity.hash(&mut h);
    style.hash(&mut h);
    (surface.width_px, surface.height_px, surface.scale.to_bits(), surface.two_pages)
        .hash(&mut h);
    format!("{:016x}", h.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_text_stops_above_the_bottom_group() {
        let store = kaeri_store::Store::open_in_memory().unwrap();
        let config = kaeri_core::config::ReadingConfig::default();
        let surface = ReaderSurface {
            width_px: 1280,
            height_px: 800,
            scale: 1.0,
            eink: false,
            two_pages: false,
            dark: true,
        };
        let foot = |handheld: bool| {
            let screen = super::super::settings::Screen { handheld, ..Default::default() };
            let layers =
                Layers::read(&store, &config, kaeri_core::media::Medium::Novel, None, screen);
            let style = style(&layers, &surface, handheld, NovelSupport::Full, false, false);
            serde_json::from_str::<serde_json::Value>(&style).unwrap()["bottom"]
                .as_f64()
                .unwrap()
        };
        // The tallest group, the Mac's, is 110 px with the space under it.
        assert!(foot(false) >= 120.0, "{}", foot(false));
        assert!(foot(true) < 100.0, "phones keep their own foot");
    }

    #[test]
    fn a_line_holds_65_to_75_characters_on_every_screen_but_e_ink() {
        let store = kaeri_store::Store::open_in_memory().unwrap();
        let config = kaeri_core::config::ReadingConfig::default();
        let style_on = |eink: bool| {
            let surface = ReaderSurface {
                width_px: 2388,
                height_px: 1668,
                scale: 2.0,
                eink,
                two_pages: true,
                dark: false,
            };
            let screen = super::super::settings::Screen::default();
            let layers =
                Layers::read(&store, &config, kaeri_core::media::Medium::Novel, None, screen);
            let style = style(&layers, &surface, false, NovelSupport::Full, false, false);
            serde_json::from_str::<serde_json::Value>(&style).unwrap()
        };
        let wide = style_on(false);
        assert_eq!(wide["measure"], serde_json::json!([65, 75]));
        assert_eq!(wide["columns"], "auto", "the paginator fits columns to the measure");
        assert_eq!(style_on(true)["columns"], 1, "e-ink keeps one column");
    }

    #[test]
    fn words_are_the_body_s_with_two_cjk_characters_as_one() {
        let page = "<html><head><title>Not counted here</title></head>\
            <body><h1>Chapter 4</h1><p>The ferryman&#8217;s lamp &amp; the keeper&apos;s \
            well-worn map.</p></body></html>";
        // Chapter, 4, The, ferryman's, lamp, the, keeper's, well-worn, map: an entity is a
        // character, never a word.
        assert_eq!(words(page), 9);
        assert_eq!(
            words("<body><p>\u{6E21}\u{3057}\u{5B88}\u{306E}\u{706F}\u{308A}</p></body>"),
            3
        );
        assert_eq!(words("<p>No body, so all of it.</p>"), 6);
        assert_eq!(words(""), 0);
    }

    fn text_book(scheme: Scheme, dir: &Path) -> (NovelBooks, Arc<ServedBook>) {
        let books = NovelBooks::with_scheme(scheme);
        let opening = Opening {
            session: 7,
            soft_hyphens: false,
            dir: dir.join("novel-7"),
            eink: None,
            limits: Limits::default(),
        };
        let book = open_text(
            "<p>The lantern keeper climbed the stair.</p><script>x()</script>",
            "Chapter 1",
            Some("en".into()),
            "w:1".into(),
            opening,
        );
        book.served.set_style("{\"size\":18}".into());
        books.put(book.served.clone());
        (books, book.served)
    }

    fn header<'a>(r: &'a BookResource, name: &str) -> Option<&'a str> {
        r.headers.iter().find(|h| h.name == name).map(|h| h.value.as_str())
    }

    #[test]
    fn a_text_chapter_is_served_cleaned_with_its_setup_and_the_policy() {
        let dir = tempfile::tempdir().unwrap();
        let (books, _) = text_book(Scheme::Kaeri, dir.path());
        let url = books.url(7, TEXT_CHAPTER);
        assert_eq!(url, "kaeri-book://7/chapter.xhtml");
        let r = books.resource(&format!("{url}#kaeri:pos=0/0/3")).unwrap();
        assert_eq!((r.status, r.mime.as_str()), (200, "application/xhtml+xml"));
        assert!(header(&r, "Content-Security-Policy").unwrap().contains("script-src 'none'"));
        let BookBody::Bytes { data } = &r.body else { panic!("bytes") };
        let text = String::from_utf8(data.clone()).unwrap();
        assert!(text.contains("The lantern keeper"), "{text}");
        assert!(!text.contains("script>"), "{text}");
        assert!(
            text.contains(
                "content=\"{&quot;spine&quot;:0,&quot;last&quot;:true,&quot;size&quot;:18}\""
            ),
            "{text}"
        );
        // Off the book, another session, a climb out: refused without reading anything.
        assert_eq!(books.resource("kaeri-book://7/other.xhtml").unwrap().status, 404);
        assert_eq!(books.resource("kaeri-book://8/chapter.xhtml").unwrap().status, 404);
        assert_eq!(books.resource("kaeri-book://7/../../etc/passwd").unwrap().status, 404);
        assert!(books.resource("https://x.example/a").is_none());
        // Kaeri's stylesheet comes with the paginator: the chapter links none, and none is served.
        assert!(!text.contains("<link"), "{text}");
        assert_eq!(books.resource("kaeri-book://7/_kaeri/reader.css").unwrap().status, 404);
        books.remove(7);
        assert_eq!(books.resource(&url).unwrap().status, 404);
    }

    #[test]
    fn android_addresses_are_read_too() {
        let dir = tempfile::tempdir().unwrap();
        let (books, _) = text_book(Scheme::AndroidAssets, dir.path());
        let url = books.url(7, TEXT_CHAPTER);
        assert_eq!(url, "https://appassets.androidplatform.net/book/7/chapter.xhtml");
        assert_eq!(books.resource(&url).unwrap().status, 200);
        assert_eq!(
            NovelBooks::default().url(1, "OEBPS/Text/Chapter One.xhtml"),
            format!(
                "{}1/OEBPS/Text/Chapter%20One.xhtml",
                if cfg!(target_os = "android") { ANDROID_PREFIX } else { KAERI_PREFIX }
            )
        );
    }

    #[test]
    fn the_assets_carry_the_script_and_the_policy() {
        let a = assets();
        assert!(a.script.contains("window.kaeriReader") && !a.script.contains("__KAERI_CSS__"));
        assert!(a.stylesheet.contains("html.kaeri"));
        assert!(a.csp.starts_with("default-src 'none'"));
    }
}
