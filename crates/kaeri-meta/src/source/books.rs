//! Books through the weather: AniList's answers for manga and novels, each kept in `book_work`
//! as it lands, so a screen opens on what was kept and a launch asks for no more than its first
//! Home.

use kaeri_core::ids::AnilistId;
use kaeri_store::book_catalog::BookWork;
use serde::{Deserialize, Serialize};

use super::{Meta, Result};
use crate::anilist::{
    Page,
    books::{Book, BookAsk},
};

/// How long a book's facts are used again before AniList is asked for them.
pub const BOOK_KEPT_FOR: i64 = 24 * 3_600;

/// What `book_work.meta` holds: the book as AniList answered, and when its page was last asked
/// (its makers, relations and recommendations ride only that).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Kept {
    book: Book,
    #[serde(default)]
    detail_at: Option<i64>,
}

/// A book as this device kept it.
#[derive(Debug, Clone, PartialEq)]
pub struct KeptBook {
    pub book: Book,
    pub fetched_at: i64,
    /// When its page was last asked, if ever.
    pub detail_at: Option<i64>,
}

impl KeptBook {
    pub fn fresh(&self, now: i64) -> bool {
        now - self.fetched_at < BOOK_KEPT_FOR
    }

    pub fn detail_fresh(&self, now: i64) -> bool {
        self.detail_at.is_some_and(|at| now - at < BOOK_KEPT_FOR)
    }
}

fn work_of(id: AnilistId) -> String {
    format!("a:{}", id.get())
}

impl Meta {
    /// Several pages and id lists of books in one request, each kept as it lands.
    pub async fn books(&self, asks: &[BookAsk]) -> Result<Vec<Page<Book>>> {
        let pages = self.weathered(self.anilist.books(asks)).await?;
        let now = kaeri_store::now();
        for page in &pages {
            for book in &page.items {
                self.keep_book(book, None, now);
            }
        }
        Ok(pages)
    }

    /// Books by id, in the order asked, each kept.
    pub async fn books_many(&self, ids: &[AnilistId]) -> Result<Vec<Book>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let books = self.weathered(self.anilist.books_many(ids)).await?;
        let now = kaeri_store::now();
        for book in &books {
            self.keep_book(book, None, now);
        }
        Ok(books)
    }

    /// One book's page: its makers, relations and recommendations, kept.
    pub async fn book(&self, id: AnilistId) -> Result<Book> {
        let book = self.weathered(self.anilist.book(id)).await?;
        let now = kaeri_store::now();
        self.keep_book(&book, Some(now), now);
        Ok(book)
    }

    /// A book as kept, however old.
    pub fn kept_book(&self, id: AnilistId) -> Option<KeptBook> {
        let row = self.store.book_work(&work_of(id)).ok()??;
        kept_of(&row)
    }

    /// The books kept of `ids`, in the order asked.
    pub fn kept_books(&self, ids: &[AnilistId]) -> Vec<KeptBook> {
        let works: Vec<String> = ids.iter().map(|id| work_of(*id)).collect();
        let rows = self.store.book_works(&works).unwrap_or_default();
        let mut found: Vec<KeptBook> = rows.iter().filter_map(kept_of).collect();
        found.sort_by_key(|k| ids.iter().position(|id| *id == k.book.id));
        found
    }

    /// Keep a book. A list's answer has no makers, relations or recommendations, so the ones a
    /// page brought stay until the page is asked again.
    fn keep_book(&self, book: &Book, detail_at: Option<i64>, now: i64) {
        let work = work_of(book.id);
        let before = self.store.book_work(&work).ok().flatten().as_ref().and_then(kept_of);
        let mut book = book.clone();
        let mut detail_at = detail_at;
        if detail_at.is_none()
            && let Some(before) = before
        {
            book.staff = before.book.staff;
            book.relations = before.book.relations;
            book.recommendations = before.book.recommendations;
            detail_at = before.detail_at;
        }
        let titles = serde_json::json!({
            "romaji": book.title.romaji,
            "english": book.title.english,
            "native": book.title.native,
            "synonyms": book.synonyms,
        });
        let status = book
            .status
            .and_then(|s| serde_json::to_value(s).ok())
            .and_then(|v| v.as_str().map(str::to_owned));
        let row = BookWork {
            work,
            anilist_id: Some(book.id.get()),
            medium: book.medium().as_str().to_owned(),
            format: Some(book.reading_format().as_str().to_owned()),
            origin: book.origin().map(|o| o.code().to_owned()),
            titles: titles.to_string(),
            cover: book.cover_image.best().map(str::to_owned),
            status,
            chapters_total: book.chapters,
            volumes_total: book.volumes,
            meta: serde_json::to_string(&Kept { book, detail_at }).ok(),
            fetched_at: now,
        };
        if let Err(e) = self.store.put_book_work(&row) {
            tracing::warn!(error = %e, "could not keep a book");
        }
    }
}

fn kept_of(row: &BookWork) -> Option<KeptBook> {
    let kept: Kept = serde_json::from_str(row.meta.as_deref()?).ok()?;
    Some(KeptBook { book: kept.book, fetched_at: row.fetched_at, detail_at: kept.detail_at })
}
