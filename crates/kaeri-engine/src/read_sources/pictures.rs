//! Pictures in a source's text chapter. A plugin keeps an illustration as
//! `<img src="https://…">` on one of the hosts it declares; the engine fetches it the way it
//! fetches page images and keeps it inside the book, so the reader, a download and what is made
//! from a download show it with no network. A host the plugin did not declare is never asked,
//! and a server or folder never has its pictures fetched.

use std::collections::HashMap;

use futures::{StreamExt, stream};
use kaeri_core::books::{BookSourceKind, PageRef};

use super::BookSources;

/// One picture kept for a text chapter, at its path in the book. Paths sit beside the chapter
/// (`images/1.jpg`), so the reader's chapter and a download's chapter both find them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextPicture {
    pub path: String,
    pub media_type: &'static str,
    pub bytes: Vec<u8>,
}

/// Pictures fetched at once, as a page strip's are.
const AT_ONCE: usize = 3;

impl BookSources {
    /// Fetch the pictures text chapter `xhtml` of `source` links on the plugin's own hosts, and
    /// point the chapter at the kept copies. A picture that does not come, or is no image,
    /// keeps its address, which the sanitiser drops.
    pub async fn pictures_in(&self, source: &str, xhtml: &mut String) -> Vec<TextPicture> {
        let Some(manifest) = self.registry.manifest(source) else { return Vec::new() };
        // A server or a folder hands over its own books; only a source that declares hosts
        // (a plugin, or the in-process fixtures) has pictures fetched for it.
        if !matches!(manifest.kind, BookSourceKind::Plugin | BookSourceKind::Native) {
            return Vec::new();
        }
        let wanted: Vec<String> = kaeri_books::novel::web_images(xhtml)
            .into_iter()
            .filter(|url| kaeri_plugin::sandbox::is_allowed(url, &manifest.allowed_hosts))
            .collect();
        if wanted.is_empty() {
            return Vec::new();
        }
        let fetched: Vec<(String, Option<Vec<u8>>)> = stream::iter(wanted)
            .map(|url| async move {
                match self.registry.fetch_picture(source, &PageRef::new(&url)).await {
                    Ok(bytes) => (url, Some(bytes)),
                    Err(error) => {
                        tracing::info!(source, %error, "a chapter's picture did not come");
                        (url, None)
                    }
                }
            })
            .buffered(AT_ONCE)
            .collect()
            .await;

        let mut local = HashMap::new();
        let mut kept = Vec::new();
        for (url, bytes) in fetched {
            let Some(bytes) = bytes else { continue };
            if let Err(why) = kaeri_plugin::books::check_page(&bytes) {
                tracing::info!(source, %why, "a chapter's picture was refused");
                continue;
            }
            let Some(media_type) = kaeri_books::mime::sniff_image(&bytes) else { continue };
            let path = format!("images/{}.{}", kept.len() + 1, extension(media_type));
            local.insert(url, path.clone());
            kept.push(TextPicture { path, media_type, bytes });
        }
        if !local.is_empty() {
            *xhtml = kaeri_books::novel::with_local_images(xhtml, &local);
        }
        kept
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
    /// a 404, anything else bytes that are no image. It notes every address it is asked for.
    struct Easel {
        manifest: BookSourceManifest,
        asked: Mutex<Vec<String>>,
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
            if page.url.contains("/ok/") {
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
        let sources = BookSources::new(store, tokens, reqwest::Client::new(), &paths);
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
}
