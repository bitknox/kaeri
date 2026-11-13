//! Pictures in a source's text chapter. A plugin keeps an illustration as
//! `<img src="https://…">` on one of the hosts it declares; the engine fetches it the way it
//! fetches page images and keeps it inside the book, so the reader, a download and what is made
//! from a download show it with no network. A host the plugin did not declare is never asked,
//! and a server or folder never has its pictures fetched.
//!
//! The reader waits for a chapter's pictures together for [`OPENING_WAIT`] at most, so one slow
//! host never holds the text back. What has not come by then is fetched on, and the page shows a
//! box of a fixed size in its place that the picture fills when it comes; one that fails shows
//! the failed state, and a tap fetches it again. A download waits for every picture.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
    time::Duration,
};

use kaeri_core::books::{BookSourceKind, PageRef};
use kaeri_providers::BookRegistry;
use tokio::{runtime::Handle, sync::Notify, task::AbortHandle};

use super::BookSources;

/// How long opening a text chapter waits for its pictures, all of them together.
pub const OPENING_WAIT: Duration = Duration::from_secs(3);
/// How long one picture may take, retries included, before it counts as failed.
const PICTURE_WAIT: Duration = Duration::from_secs(30);

/// One picture kept for a downloaded text chapter, at its path in the book. Paths sit beside
/// the chapter (`images/1.jpg`), so the chapter finds them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextPicture {
    pub path: String,
    pub media_type: &'static str,
    pub bytes: Vec<u8>,
}

/// A picture that came: its type and its bytes.
#[derive(Debug, PartialEq, Eq)]
pub struct Fetched {
    pub media_type: &'static str,
    pub bytes: Vec<u8>,
}

/// Where one picture of an open chapter stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shown {
    /// Still being fetched.
    Waiting,
    Ready(Arc<Fetched>),
    /// It did not come, or came as something that is no picture. A retry fetches it again.
    Failed,
}

struct Slot {
    url: String,
    shown: Shown,
}

/// The pictures of one text chapter, fetched together and on after the chapter opens. The
/// reader's picture `n` (from 1) is served at `images/<n>`.
pub struct TextPictures {
    source: String,
    registry: BookRegistry,
    runtime: Handle,
    slots: Mutex<Vec<Slot>>,
    changed: Notify,
    tasks: Mutex<Vec<AbortHandle>>,
}

impl std::fmt::Debug for TextPictures {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextPictures").field("source", &self.source).finish_non_exhaustive()
    }
}

impl Drop for TextPictures {
    /// A closed chapter stops fetching.
    fn drop(&mut self) {
        for task in self.tasks.get_mut().unwrap_or_else(|e| e.into_inner()).drain(..) {
            task.abort();
        }
    }
}

/// The path picture `index` (from 0) is served at in the reader.
fn reader_path(index: usize) -> String {
    format!("images/{}", index + 1)
}

impl TextPictures {
    /// Starts fetching every one of `urls` at once; the registry keeps each host to its pace.
    fn start(registry: BookRegistry, source: &str, urls: Vec<String>) -> Arc<Self> {
        let count = urls.len();
        let pictures = Arc::new(Self {
            source: source.to_owned(),
            registry,
            runtime: Handle::current(),
            slots: Mutex::new(
                urls.into_iter().map(|url| Slot { url, shown: Shown::Waiting }).collect(),
            ),
            changed: Notify::new(),
            tasks: Mutex::default(),
        });
        for index in 0..count {
            pictures.fetch(index);
        }
        pictures
    }

    /// Pictures that stand as given, fetched from nowhere: for the served book's tests.
    #[cfg(test)]
    pub(crate) fn standing(shown: Vec<Shown>) -> Arc<Self> {
        let slots = shown.into_iter().enumerate();
        Arc::new(Self {
            source: "nowhere".into(),
            registry: BookRegistry::new(),
            runtime: Handle::current(),
            slots: Mutex::new(
                slots
                    .map(|(i, shown)| Slot { url: format!("https://p.example/{i}"), shown })
                    .collect(),
            ),
            changed: Notify::new(),
            tasks: Mutex::default(),
        })
    }

    fn slots(&self) -> std::sync::MutexGuard<'_, Vec<Slot>> {
        self.slots.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn fetch(self: &Arc<Self>, index: usize) {
        let Some(url) = self.slots().get(index).map(|s| s.url.clone()) else { return };
        let me: Weak<Self> = Arc::downgrade(self);
        let registry = self.registry.clone();
        let source = self.source.clone();
        let task = self.runtime.spawn(async move {
            let picture = PageRef::new(&url);
            let asked = registry.fetch_picture(&source, &picture);
            let shown = match tokio::time::timeout(PICTURE_WAIT, asked).await {
                Ok(Ok(bytes)) => checked(&source, bytes).map_or(Shown::Failed, Shown::Ready),
                Ok(Err(error)) => {
                    tracing::info!(source, %error, "a chapter's picture did not come");
                    Shown::Failed
                }
                Err(_) => {
                    tracing::info!(source, "a chapter's picture took too long");
                    Shown::Failed
                }
            };
            if let Some(pictures) = me.upgrade() {
                pictures.set(index, shown);
            }
        });
        let mut tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        tasks.retain(|t| !t.is_finished());
        tasks.push(task.abort_handle());
    }

    fn set(&self, index: usize, shown: Shown) {
        if let Some(slot) = self.slots().get_mut(index) {
            slot.shown = shown;
        }
        self.changed.notify_waiters();
    }

    /// Returns once no picture is still being fetched.
    pub async fn settled(&self) {
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if !self.slots().iter().any(|s| s.shown == Shown::Waiting) {
                return;
            }
            changed.await;
        }
    }

    /// Each picture's address and its path in the reader.
    fn reader_paths(&self) -> HashMap<String, String> {
        self.slots().iter().enumerate().map(|(i, s)| (s.url.clone(), reader_path(i))).collect()
    }

    fn index(&self, path: &str) -> Option<usize> {
        let n: usize = path.strip_prefix("images/")?.parse().ok()?;
        (1..=self.slots().len()).contains(&n).then(|| n - 1)
    }

    /// Where the picture the reader serves at `path` stands; `None` for a path that is none.
    pub fn shown(&self, path: &str) -> Option<Shown> {
        let index = self.index(path)?;
        self.slots().get(index).map(|s| s.shown.clone())
    }

    /// The reader's paths of the pictures not in yet, waiting or failed.
    pub fn not_in(&self) -> Vec<String> {
        let slots = self.slots();
        let out = slots.iter().enumerate().filter(|(_, s)| !matches!(s.shown, Shown::Ready(_)));
        out.map(|(i, _)| reader_path(i)).collect()
    }

    /// Fetches the failed picture at `path` again. False when it had not failed.
    pub fn retry(self: &Arc<Self>, path: &str) -> bool {
        let Some(index) = self.index(path) else { return false };
        {
            let mut slots = self.slots();
            match slots.get_mut(index) {
                Some(slot) if slot.shown == Shown::Failed => slot.shown = Shown::Waiting,
                _ => return false,
            }
        }
        self.fetch(index);
        true
    }

    /// The pictures that came, each with its address, at paths named for their type.
    fn kept(&self) -> Vec<(String, TextPicture)> {
        let slots = self.slots();
        let ready = slots.iter().enumerate().filter_map(|(i, s)| match &s.shown {
            Shown::Ready(fetched) => Some((i, &s.url, fetched)),
            _ => None,
        });
        ready
            .map(|(i, url, fetched)| {
                let path = format!("images/{}.{}", i + 1, extension(fetched.media_type));
                let picture = TextPicture {
                    path,
                    media_type: fetched.media_type,
                    bytes: fetched.bytes.clone(),
                };
                (url.clone(), picture)
            })
            .collect()
    }
}

/// The bytes as a picture, or `None` when they break the page limits or are no image.
fn checked(source: &str, bytes: Vec<u8>) -> Option<Arc<Fetched>> {
    if let Err(why) = kaeri_plugin::books::check_page(&bytes) {
        tracing::info!(source, %why, "a chapter's picture was refused");
        return None;
    }
    let media_type = kaeri_books::mime::sniff_image(&bytes)?;
    Some(Arc::new(Fetched { media_type, bytes }))
}

impl BookSources {
    /// Starts fetching the pictures text chapter `xhtml` of `source` links on the source's own
    /// hosts. `None` when there are none, or the source is a server or a folder.
    fn start_pictures(&self, source: &str, xhtml: &str) -> Option<Arc<TextPictures>> {
        let manifest = self.registry.manifest(source)?;
        // A server or a folder hands over its own books; only a source that declares hosts
        // (a plugin, or the in-process fixtures) has pictures fetched for it.
        if !matches!(manifest.kind, BookSourceKind::Plugin | BookSourceKind::Native) {
            return None;
        }
        let wanted: Vec<String> = kaeri_books::novel::web_images(xhtml)
            .into_iter()
            .filter(|url| kaeri_plugin::sandbox::is_allowed(url, &manifest.allowed_hosts))
            .collect();
        if wanted.is_empty() {
            return None;
        }
        Some(TextPictures::start(self.registry.clone(), source, wanted))
    }

    /// For the reader: points every picture text chapter `xhtml` of `source` fetches at its
    /// path in the book and waits [`OPENING_WAIT`] at most for them, then hands over the
    /// pictures as they stand, still fetching what has not come.
    pub async fn reader_pictures(
        &self,
        source: &str,
        xhtml: &mut String,
    ) -> Option<Arc<TextPictures>> {
        let pictures = self.start_pictures(source, xhtml)?;
        *xhtml = kaeri_books::novel::with_local_images(xhtml, &pictures.reader_paths());
        if tokio::time::timeout(OPENING_WAIT, pictures.settled()).await.is_err() {
            tracing::info!(source, "a chapter opens before all its pictures came");
        }
        Some(pictures)
    }

    /// For a download: fetches the pictures text chapter `xhtml` of `source` links on the
    /// source's own hosts, waits for all of them, and points the chapter at the ones that came.
    /// A picture that does not come, or is no image, keeps its address, which the sanitiser
    /// drops.
    pub async fn pictures_in(&self, source: &str, xhtml: &mut String) -> Vec<TextPicture> {
        let Some(pictures) = self.start_pictures(source, xhtml) else { return Vec::new() };
        pictures.settled().await;
        let kept = pictures.kept();
        let local: HashMap<String, String> =
            kept.iter().map(|(url, p)| (url.clone(), p.path.clone())).collect();
        if !local.is_empty() {
            *xhtml = kaeri_books::novel::with_local_images(xhtml, &local);
        }
        kept.into_iter().map(|(_, picture)| picture).collect()
    }
}

fn extension(media_type: &str) -> &'static str {
    match media_type {
        "image/png" => "png",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "image/avif" => "avif",
        "image/bmp" => "bmp",
        "image/jxl" => "jxl",
        _ => "jpg",
    }
}

/// A PNG of one pixel, whole enough for the page checks.
#[cfg(test)]
pub(crate) const ONE_PIXEL_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
    0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90,
    0x77, 0x53, 0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x30,
    0x48, 0x98, 0x00, 0x00, 0x01, 0xe4, 0x01, 0x21, 0xca, 0x6c, 0xd2, 0x32, 0x00, 0x00, 0x00,
    0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use kaeri_core::{
        ProviderError,
        books::{
            BookHints, BookSourceManifest, ChapterContent, SeriesDetail, SeriesHit,
            SourceChapter,
        },
        config::Paths,
        media::Medium,
        traits::{BookSource, PageError},
    };
    use kaeri_store::Store;

    use super::*;

    /// A source that answers every picture fetch from its address: `/ok/` a picture, `/gone/`
    /// a 404, `/never/` never, `/later/` a picture once `later` has a permit, anything else
    /// bytes that are no image. It notes every address it is asked for.
    struct Easel {
        manifest: BookSourceManifest,
        asked: Mutex<Vec<String>>,
        later: tokio::sync::Semaphore,
    }

    #[async_trait]
    impl BookSource for Easel {
        fn manifest(&self) -> &BookSourceManifest {
            &self.manifest
        }

        async fn search(
            &self,
            _: &str,
            _: &BookHints,
        ) -> Result<Vec<SeriesHit>, ProviderError> {
            Ok(Vec::new())
        }

        async fn series(&self, id: &str, _: &BookHints) -> Result<SeriesDetail, ProviderError> {
            Ok(SeriesDetail::new(id, "Saltglass Letters"))
        }

        async fn chapters(
            &self,
            _: &str,
            _: &BookHints,
        ) -> Result<Vec<SourceChapter>, ProviderError> {
            Ok(Vec::new())
        }

        async fn content(&self, _: &str, _: &str) -> Result<ChapterContent, ProviderError> {
            Err(ProviderError::NotFound)
        }

        async fn fetch_page(&self, page: &PageRef) -> Result<Vec<u8>, PageError> {
            self.asked.lock().unwrap().push(page.url.clone());
            if page.url.contains("/never/") {
                return std::future::pending().await;
            }
            if page.url.contains("/later/") {
                self.later.acquire().await.unwrap().forget();
            }
            if page.url.contains("/ok/") || page.url.contains("/later/") {
                Ok(ONE_PIXEL_PNG.to_vec())
            } else if page.url.contains("/gone/") {
                Err(PageError::Status(404))
            } else {
                Ok(b"<html>not a picture</html>".to_vec())
            }
        }
    }

    fn sources(kind: BookSourceKind) -> (BookSources, Arc<Easel>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths {
            config_file: dir.path().join("config.toml"),
            data_dir: dir.path().join("data"),
            cache_dir: dir.path().join("cache"),
        };
        let tokens = kaeri_track::TokenStore::new(dir.path().join("tokens"));
        let store = Store::open_in_memory().unwrap();
        let sources = BookSources::new(
            store,
            tokens,
            &kaeri_net::HttpClient::new(&Default::default()).unwrap(),
            &paths,
        );
        let easel = Arc::new(Easel {
            manifest: BookSourceManifest {
                id: "easel".into(),
                name: "Easel".into(),
                version: "1".into(),
                kind,
                media: vec![Medium::Novel],
                languages: vec!["en".into()],
                allowed_hosts: vec!["img.books.example".into()],
                capabilities: Vec::new(),
            },
            asked: Mutex::default(),
            later: tokio::sync::Semaphore::new(0),
        });
        sources.registry.put(easel.clone());
        (sources, easel, dir)
    }

    const CHAPTER: &str = "<p>The bottle was cold.</p>\
        <img src=\"https://img.books.example/ok/bottle.png\" alt=\"A bottle\"/>\
        <img src=\"https://elsewhere.example/ok/gull.png\"/>\
        <img src=\"https://img.books.example/gone/tide.png\"/>\
        <img src=\"https://img.books.example/odd/letter.png\"/>\
        <img src=\"http://img.books.example/ok/plain.png\"/>";

    #[tokio::test]
    async fn pictures_on_the_plugins_hosts_are_kept_and_no_other_host_is_asked() {
        let (sources, easel, _dir) = sources(BookSourceKind::Plugin);
        let mut xhtml = CHAPTER.to_owned();
        let pictures = sources.pictures_in("easel", &mut xhtml).await;

        assert_eq!(
            pictures,
            [TextPicture {
                path: "images/1.png".into(),
                media_type: "image/png",
                bytes: ONE_PIXEL_PNG.to_vec(),
            }]
        );
        assert!(xhtml.contains("<img src=\"images/1.png\" alt=\"A bottle\"/>"), "{xhtml}");
        // What did not come keeps its address, which the sanitiser drops.
        assert!(xhtml.contains("https://img.books.example/gone/tide.png"), "{xhtml}");
        assert!(xhtml.contains("https://img.books.example/odd/letter.png"), "{xhtml}");

        let mut asked = easel.asked.lock().unwrap().clone();
        asked.sort();
        assert_eq!(
            asked,
            [
                "https://img.books.example/gone/tide.png",
                "https://img.books.example/odd/letter.png",
                "https://img.books.example/ok/bottle.png",
            ],
            "a host the plugin did not declare, and plain http, are never asked"
        );
    }

    #[tokio::test]
    async fn a_server_has_no_pictures_fetched() {
        let (sources, easel, _dir) = sources(BookSourceKind::Server);
        let mut xhtml = CHAPTER.to_owned();
        assert!(sources.pictures_in("easel", &mut xhtml).await.is_empty());
        assert_eq!(xhtml, CHAPTER);
        assert!(easel.asked.lock().unwrap().is_empty());
        assert!(sources.pictures_in("nobody", &mut xhtml).await.is_empty());
    }

    const WAITING: &str = "<p>The tide came in.</p>\
        <img src=\"https://img.books.example/ok/shell.png\"/>\
        <img src=\"https://img.books.example/never/kite.png\"/>\
        <img src=\"https://img.books.example/later/boat.png\"/>";

    #[tokio::test(start_paused = true)]
    async fn a_host_that_never_answers_holds_the_chapter_back_three_seconds_at_most() {
        let (sources, easel, _dir) = sources(BookSourceKind::Plugin);
        let mut xhtml = WAITING.to_owned();
        let began = tokio::time::Instant::now();
        let pictures = sources.reader_pictures("easel", &mut xhtml).await.unwrap();

        assert_eq!(began.elapsed(), OPENING_WAIT, "the text waits for no picture past the cap");
        for path in ["images/1", "images/2", "images/3"] {
            assert!(xhtml.contains(&format!("src=\"{path}\"")), "{xhtml}");
        }
        assert!(matches!(pictures.shown("images/1"), Some(Shown::Ready(_))));
        assert_eq!(pictures.shown("images/2"), Some(Shown::Waiting));
        assert_eq!(pictures.not_in(), ["images/2", "images/3"]);
        assert_eq!(pictures.shown("images/4"), None);
        assert!(!pictures.retry("images/2"), "a picture still coming is not asked again");

        // The late one fills its place once it comes; the one that never answers fails.
        easel.later.add_permits(1);
        pictures.settled().await;
        let Some(Shown::Ready(boat)) = pictures.shown("images/3") else {
            panic!("the late one")
        };
        assert_eq!(boat.bytes, ONE_PIXEL_PNG);
        assert_eq!(pictures.shown("images/2"), Some(Shown::Failed));
        assert_eq!(pictures.not_in(), ["images/2"]);

        // A tap on the failed one fetches it again.
        assert!(pictures.retry("images/2"));
        assert_eq!(pictures.shown("images/2"), Some(Shown::Waiting));
        let kites =
            |e: &Easel| e.asked.lock().unwrap().iter().filter(|u| u.contains("kite")).count();
        for _ in 0..4 {
            tokio::task::yield_now().await;
        }
        assert_eq!(kites(&easel), 2);
    }

    #[tokio::test]
    async fn a_chapter_whose_pictures_come_in_time_waits_for_them_all() {
        let (sources, _easel, _dir) = sources(BookSourceKind::Plugin);
        let mut xhtml = CHAPTER.to_owned();
        let pictures = sources.reader_pictures("easel", &mut xhtml).await.unwrap();
        assert!(matches!(pictures.shown("images/1"), Some(Shown::Ready(_))));
        // Gone, and no image: both failed, and served as failed, not waited on.
        assert_eq!(pictures.not_in(), ["images/2", "images/3"]);
        assert_eq!(pictures.shown("images/2"), Some(Shown::Failed));
        assert!(xhtml.contains("https://elsewhere.example/ok/gull.png"), "{xhtml}");
    }
}
