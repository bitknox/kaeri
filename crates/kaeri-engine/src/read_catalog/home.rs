//! Reading's Home: the spotlight, Continue, This week, Up next, then the discovery rows:
//! Trending, Because you finished, New, More of a genre and Finished, per format.
//!
//! Home shows at once from what was kept. Then one AniList request asks for everything the
//! screen lacks: the books the person reads that nothing is kept of, and the Trending rows when
//! they are stale. After it the launch's background work may begin, and the paced queue asks for
//! the other rows in one request, the pages of the series Because you finished and Up next come
//! from in another, and the books those suggest in a third. Rows are kept 30 minutes; a Home
//! whose rows are all fresh asks for nothing. Discovery tiles are catalogue entries: no source is
//! asked about them until a series opens.

use std::{
    collections::{HashMap, HashSet},
    sync::atomic::Ordering,
};

use kaeri_core::{books::ChapterId, ids::AnilistId, media::Medium};
use kaeri_meta::{
    KeptBook, Meta,
    anilist::books::{Book, BookAsk},
};
use kaeri_model::{
    Msg, ReadingMsg,
    reading::{
        self as words, READ_ROW_TITLES, ReadHomeEnd, ReadHomePart, ReadHomeRow, ReadRowFormat,
        ReadSpotlight, ReadSpotlightCase, ReadWeekDay, ReadWeekEntry, ReadingEvent,
        ReadingHomeView, SeriesAction, WorkTile,
    },
};
use kaeri_store::Store;
use serde::{Deserialize, Serialize};

use super::{
    Ask, Kit, chapters,
    discovery::{self, MIX_DAYS, Planned},
    search, send, tiles,
};
use crate::{outbox::Outbox, read_sources::BookSources};

/// How long a discovery row is shown again before AniList is asked again.
const SHELF_KEPT_FOR: i64 = 30 * 60;
/// Where each row's titles are kept, after its id.
const ROW_KEY: &str = "read:home:row:";
const BECAUSE: &str = "read:home:because";
const SHELF: u32 = READ_ROW_TITLES;
const CONTINUE: usize = 12;
const DAY: i64 = 86_400;

/// The parts of Home on their way, and how the last load went.
#[derive(Debug, Default)]
pub struct Loading {
    pub parts: HashSet<ReadHomePart>,
    /// A load is asking AniList now.
    pub running: bool,
    pub failed: bool,
    /// AniList was not reached: what is shown was kept then.
    pub offline_since: Option<i64>,
    /// Which load the parts are of. A refresh starts a load while one runs, and the older one
    /// ending leaves the newer one's parts alone.
    pub generation: u64,
}

/// A discovery row as kept: the books on it, when it was asked, and what it is about.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct Shelf {
    pub at: i64,
    pub ids: Vec<u32>,
    #[serde(default)]
    pub name: Option<String>,
}

impl Shelf {
    fn of(store: &Store, key: &str) -> Option<Self> {
        let text = store.get_meta_text(key).ok()??;
        serde_json::from_str(&text).ok()
    }

    fn keep(&self, store: &Store, key: &str) {
        let text = serde_json::to_string(self).unwrap_or_default();
        if let Err(e) = store.set_meta_text(key, &text) {
            tracing::warn!(error = %e, "a reading shelf was not kept");
        }
    }

    fn fresh(&self, now: i64) -> bool {
        (now - self.at).abs() < SHELF_KEPT_FOR
    }

    fn ids(&self) -> Vec<AnilistId> {
        self.ids.iter().map(|id| AnilistId::new(*id)).collect()
    }
}

fn key_of(row: Planned) -> String {
    match row {
        Planned::Because => BECAUSE.to_owned(),
        other => format!("{ROW_KEY}{}", other.id()),
    }
}

/// Load Home: at once from what was kept, then again as each part lands.
pub fn load(kit: &Kit, ask: Ask, refresh: bool, tx: &Outbox) {
    super::changed::home(kit);
    let generation = begin(&kit.sources.catalog.home, refresh);
    let home = Home { generation: generation.unwrap_or(0), ..Home::of(kit, ask) };
    home.send(tx);
    if generation.is_none() {
        return;
    }
    let tx = tx.clone();
    tokio::spawn(async move {
        home.run(refresh, &tx).await;
    });
}

/// Start a load, unless one runs and this is no refresh: the new load's generation.
fn begin(memory: &std::sync::Mutex<Loading>, refresh: bool) -> Option<u64> {
    let mut loading = memory.lock().unwrap_or_else(|e| e.into_inner());
    if loading.running && !refresh {
        return None;
    }
    loading.running = true;
    loading.failed = false;
    loading.parts = [ReadHomePart::Yours, ReadHomePart::Discovery, ReadHomePart::Because]
        .into_iter()
        .collect();
    loading.generation += 1;
    Some(loading.generation)
}

/// The network path came or went: an opened Home says so at once, and coming back loads it
/// again, asking only for what is stale, so it stops saying offline without the app asking.
pub fn path_moved(kit: &Kit, ask: Ask, tx: &Outbox) {
    let opened =
        kit.sources.catalog.home.lock().unwrap_or_else(|e| e.into_inner()).generation > 0;
    if !opened {
        return;
    }
    if kit.offline.load(Ordering::Relaxed) {
        resend(kit, ask, tx);
    } else {
        load(kit, ask, false, tx);
    }
}

/// Home as it stands, asking for nothing: for a change that shows on it, such as a series found
/// in no source.
pub fn resend(kit: &Kit, ask: Ask, tx: &Outbox) {
    Home::of(kit, ask).send(tx);
}

/// What a Home load works with.
#[derive(Clone)]
struct Home {
    meta: Meta,
    store: Store,
    sources: BookSources,
    ask: Ask,
    zone: jiff::tz::TimeZone,
    offline: std::sync::Arc<std::sync::atomic::AtomicBool>,
    no_sources: bool,
    /// The load this is, as [`begin`] gave it; 0 for a view alone.
    generation: u64,
}

impl Home {
    fn of(kit: &Kit, ask: Ask) -> Self {
        Self {
            meta: kit.meta.clone(),
            store: kit.store.clone(),
            sources: kit.sources.clone(),
            ask,
            zone: kit.clock.zone(),
            offline: kit.offline.clone(),
            no_sources: kit.no_sources(),
            generation: 0,
        }
    }

    fn memory(&self) -> std::sync::MutexGuard<'_, Loading> {
        self.sources.catalog.home.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn landed(&self, part: ReadHomePart) {
        let mut memory = self.memory();
        if memory.generation == self.generation {
            memory.parts.remove(&part);
        }
    }

    fn send(&self, tx: &Outbox) {
        send(tx, ReadingEvent::Home(Box::new(self.view())));
    }

    async fn run(&self, refresh: bool, tx: &Outbox) {
        if self.offline.load(Ordering::Relaxed) {
            self.went_offline();
            self.meta.anilist().launched();
            self.finish(tx);
            return;
        }
        self.screen(refresh, tx).await;
        // The screen's part is in: the launch's background work may begin.
        self.meta.anilist().launched();
        kaeri_meta::in_background(self.behind(refresh, tx)).await;
        self.finish(tx);
    }

    fn finish(&self, tx: &Outbox) {
        {
            let mut memory = self.memory();
            if memory.generation == self.generation {
                memory.parts.clear();
                memory.running = false;
            }
        }
        self.send(tx);
    }

    /// Every row's key, for when they were kept.
    fn keys() -> Vec<String> {
        let mut keys: Vec<String> = ReadRowFormat::ALL
            .into_iter()
            .flat_map(|f| [Planned::Trending(f), Planned::New(f), Planned::Finished(f)])
            .chain([Planned::Genre, Planned::Because])
            .map(key_of)
            .collect();
        keys.dedup();
        keys
    }

    fn went_offline(&self) {
        let since = Self::keys()
            .iter()
            .filter_map(|key| Shelf::of(&self.store, key))
            .map(|s| s.at)
            .max();
        let mut memory = self.memory();
        memory.offline_since = since;
        memory.failed = since.is_none() && self.yours().is_empty();
    }

    /// A row's kept titles, when they are fresh and about what the row is about now.
    fn kept(&self, row: Planned, name: Option<&str>) -> Option<Shelf> {
        Shelf::of(&self.store, &key_of(row))
            .filter(|s| s.fresh(self.ask.now) && s.name.as_deref() == name)
    }

    /// The AniList ask that fills a row: exactly its View all's search, first page.
    fn row_ask(&self, row: Planned, genre: Option<&str>) -> Option<BookAsk> {
        let query = row.query(genre)?;
        let filter = search::filter_of(&query, self.ask.adult, self.ask.now);
        Some(BookAsk::browse("", &filter, 1, SHELF))
    }

    /// Ask for `rows` in one request with `before` in front, and keep each. False when AniList
    /// did not answer.
    async fn fill(&self, before: Vec<BookAsk>, rows: &[(Planned, Option<String>)]) -> bool {
        let mut asks = before;
        let first = asks.len();
        for (row, name) in rows {
            asks.extend(self.row_ask(*row, name.as_deref()));
        }
        if asks.is_empty() {
            return true;
        }
        match self.meta.books(&asks).await {
            Ok(pages) => {
                for (i, (row, name)) in rows.iter().enumerate() {
                    let ids = pages
                        .get(first + i)
                        .map(|p| p.items.iter().map(|b| b.id.get()).collect())
                        .unwrap_or_default();
                    Shelf { at: self.ask.now, ids, name: name.clone() }
                        .keep(&self.store, &key_of(*row));
                }
                true
            }
            Err(e) => {
                tracing::info!(error = %e, "reading's Home rows were not asked for");
                false
            }
        }
    }

    /// Whether any of `rows` has nothing kept to show: a failed ask then says so.
    fn any_empty(&self, rows: &[(Planned, Option<String>)]) -> bool {
        rows.iter().any(|(row, _)| Shelf::of(&self.store, &key_of(*row)).is_none())
    }

    /// The one request the screen waits for: the person's books nothing is kept of, and the
    /// Trending rows that are stale.
    async fn screen(&self, refresh: bool, tx: &Outbox) {
        let now = self.ask.now;
        let yours = self.yours();
        let missing = if refresh {
            yours.iter().filter_map(|w| tiles::anilist(w)).collect()
        } else {
            tiles::missing(&self.meta, &yours, now)
        };
        let stale: Vec<(Planned, Option<String>)> = ReadRowFormat::ALL
            .into_iter()
            .map(Planned::Trending)
            .filter(|row| refresh || self.kept(*row, None).is_none())
            .map(|row| (row, None))
            .collect();
        let asks: Vec<BookAsk> = missing.chunks(50).map(|c| BookAsk::Ids(c.to_vec())).collect();
        if asks.is_empty() && stale.is_empty() {
            // Online with all of it kept: an earlier offline pass no longer stands.
            let mut memory = self.memory();
            memory.offline_since = None;
            memory.failed = false;
            drop(memory);
            self.landed(ReadHomePart::Yours);
            self.send(tx);
            return;
        }
        match self.fill(asks, &stale).await {
            true => self.memory().offline_since = None,
            false => {
                self.went_offline();
                if self.any_empty(&stale) {
                    self.memory().failed = true;
                }
            }
        }
        self.landed(ReadHomePart::Yours);
        self.send(tx);
    }

    /// The paced part: the other rows in one request, then the pages Because you finished and
    /// Up next's suggestions come from in another, then the books they suggest in a third.
    async fn behind(&self, refresh: bool, tx: &Outbox) {
        if self.memory().offline_since.is_some() || self.memory().failed {
            return;
        }
        let now = self.ask.now;
        let mut asked = false;
        // New, Finished and More of a genre, as the mix plans them now that the person's books
        // are kept.
        let genre = self.genre();
        let stale: Vec<(Planned, Option<String>)> = self
            .plan()
            .into_iter()
            .filter(|row| !row.on_screen() && *row != Planned::Because)
            .map(|row| (row, (row == Planned::Genre).then(|| genre.clone()).flatten()))
            .filter(|(row, name)| refresh || self.kept(*row, name.as_deref()).is_none())
            .collect();
        if !stale.is_empty() {
            asked = true;
            if !self.fill(Vec::new(), &stale).await && self.any_empty(&stale) {
                self.memory().failed = true;
            }
        }
        self.landed(ReadHomePart::Discovery);
        if asked {
            self.send(tx);
        }
        // The series to say "Because you finished" of, and the one Up next's suggestions come
        // from: their pages in one request, when what is kept of them is old.
        let finished = self.finished();
        let because_stale = refresh
            || finished.is_some() && self.kept(Planned::Because, finished.as_deref()).is_none();
        let because_id = finished.as_deref().filter(|_| because_stale).and_then(tiles::anilist);
        let up_next_id =
            self.continuing().first().and_then(|w| tiles::anilist(w)).filter(|id| {
                refresh || self.meta.kept_book(*id).is_none_or(|k| !k.detail_fresh(now))
            });
        let wanted: Vec<AnilistId> = because_id.into_iter().chain(up_next_id).collect();
        if !wanted.is_empty() {
            asked = true;
            let details = self.details(&wanted, refresh).await;
            let suggested = |id: Option<AnilistId>| -> Vec<AnilistId> {
                id.and_then(|id| details.iter().find(|b| b.id == id))
                    .map(Book::recommended)
                    .unwrap_or_default()
            };
            let because: Vec<AnilistId> = suggested(because_id)
                .into_iter()
                .filter(|r| !self.on_a_list(*r))
                .take(SHELF as usize)
                .collect();
            let mut all = because.clone();
            all.extend(suggested(up_next_id).into_iter().filter(|id| !because.contains(id)));
            self.fetch_missing(&all).await;
            if let (Some(work), Some(id)) = (finished.as_deref(), because_id)
                && details.iter().any(|b| b.id == id)
            {
                let ids = because.iter().map(|id| id.get()).collect();
                Shelf { at: now, ids, name: Some(work.to_owned()) }.keep(&self.store, BECAUSE);
            }
        }
        self.landed(ReadHomePart::Because);
        if asked {
            self.send(tx);
        }
    }

    /// Books' pages, the kept ones when fresh, the rest asked in one request. A book AniList
    /// did not give is its kept page, however old.
    async fn details(&self, ids: &[AnilistId], refresh: bool) -> Vec<Book> {
        let now = self.ask.now;
        let mut out = Vec::new();
        let mut ask = Vec::new();
        for id in ids {
            match self.meta.kept_book(*id) {
                Some(kept) if !refresh && kept.detail_fresh(now) => out.push(kept.book),
                _ => ask.push(*id),
            }
        }
        match self.meta.book_details(&ask).await {
            Ok(books) => out.extend(books),
            Err(e) => {
                tracing::info!(error = %e, "books' pages were not asked for");
                out.extend(self.meta.kept_books(&ask).into_iter().map(|k| k.book));
            }
        }
        out
    }

    async fn fetch_missing(&self, ids: &[AnilistId]) {
        let works: Vec<String> = ids.iter().map(|id| tiles::work_of(*id).0).collect();
        let missing = tiles::missing(&self.meta, &works, self.ask.now);
        if let Err(e) = self.meta.books_many(&missing).await {
            tracing::info!(error = %e, "suggested books were not asked for");
        }
    }

    // -----------------------------------------------------------------------------------------
    // What the person reads

    fn statuses(&self) -> HashMap<String, String> {
        self.store.read_statuses().unwrap_or_default().into_iter().collect()
    }

    /// Continue's works, newest first, without the finished and the dropped.
    fn continuing(&self) -> Vec<String> {
        let statuses = self.statuses();
        self.store
            .continue_reading(CONTINUE * 2)
            .unwrap_or_default()
            .into_iter()
            .map(|w| w.work)
            .filter(|w| {
                !matches!(statuses.get(w).map(String::as_str), Some("completed" | "dropped"))
            })
            .take(CONTINUE)
            .collect()
    }

    /// The series followed: Continue's, and those on Reading or Rereading.
    fn following(&self) -> Vec<String> {
        let mut out = self.continuing();
        for (work, status) in self.store.read_statuses().unwrap_or_default() {
            if matches!(status.as_str(), "reading" | "rereading") && !out.contains(&work) {
                out.push(work);
            }
        }
        out
    }

    fn planning(&self) -> Vec<String> {
        self.store.read_status_works("planning").unwrap_or_default()
    }

    fn finished(&self) -> Option<String> {
        self.store
            .read_status_works("completed")
            .unwrap_or_default()
            .into_iter()
            .find(|w| tiles::anilist(w).is_some())
    }

    /// Every work Home shows of the person's own.
    fn yours(&self) -> Vec<String> {
        let mut out = self.following();
        for work in self.planning().into_iter().chain(self.finished()) {
            if !out.contains(&work) {
                out.push(work);
            }
        }
        out
    }

    /// Every series that is the person's: on any list, or read here.
    fn own(&self) -> HashSet<String> {
        let mut out: HashSet<String> = self.yours().into_iter().collect();
        out.extend(self.store.read_statuses().unwrap_or_default().into_iter().map(|(w, _)| w));
        out.extend(
            self.store
                .continue_reading(CONTINUE * 4)
                .unwrap_or_default()
                .into_iter()
                .map(|w| w.work),
        );
        out
    }

    fn on_a_list(&self, id: AnilistId) -> bool {
        let work = tiles::work_of(id);
        self.store.read_status(work.as_str()).ok().and_then(|(s, _)| s).is_some()
            || self.store.read_work(work.as_str()).ok().flatten().is_some()
    }

    /// The reading mix: series on lists plus chapters read in the last 90 days, counted by
    /// format, from what is kept of AniList's books. A Reading that a read put there counts by
    /// its chapters alone, as it did before it had a status.
    fn mix(&self) -> HashMap<ReadRowFormat, u32> {
        let mut weight: HashMap<String, u32> = HashMap::new();
        for (work, _) in self.store.read_statuses().unwrap_or_default() {
            if self.store.read_status_is_auto(&work).unwrap_or(false) {
                continue;
            }
            *weight.entry(work).or_default() += 1;
        }
        let since = self.ask.now - MIX_DAYS * DAY;
        for (work, read) in self.store.chapters_read_since(since).unwrap_or_default() {
            *weight.entry(work).or_default() += read;
        }
        let works: Vec<String> = weight.keys().cloned().collect();
        let mut counts = HashMap::new();
        for kept in tiles::kept_of(&self.meta, &works) {
            let work = tiles::work_of(kept.book.id);
            let format = discovery::format_of(kept.book.reading_format());
            *counts.entry(format).or_default() +=
                weight.get(work.as_str()).copied().unwrap_or(0);
        }
        counts
    }

    /// The rows the mix plans now.
    fn plan(&self) -> Vec<Planned> {
        let ranked = discovery::ranked(&self.mix());
        discovery::plan(&ranked, self.finished().is_some(), self.genre().is_some())
    }

    /// The genre the person reads most, by what is kept of their books; between two as common,
    /// the one of the series read most recently. `None` with nothing read.
    fn genre(&self) -> Option<String> {
        let mut works = self.following();
        works.extend(self.store.read_status_works("completed").unwrap_or_default());
        // Each genre's count, and the first series in that order to have it.
        let mut counts: HashMap<String, (usize, usize)> = HashMap::new();
        for (at, kept) in tiles::kept_of(&self.meta, &works).into_iter().enumerate() {
            for genre in kept.book.genres {
                if kaeri_model::reading::READ_GENRES.contains(&genre.as_str()) {
                    counts.entry(genre).or_insert((0, at)).0 += 1;
                }
            }
        }
        counts
            .into_iter()
            .max_by(|a, b| a.1.0.cmp(&b.1.0).then_with(|| b.1.1.cmp(&a.1.1)))
            .map(|(genre, _)| genre)
    }

    // -----------------------------------------------------------------------------------------
    // The view

    fn tile(&self, kept: &KeptBook) -> WorkTile {
        tiles::reading(&self.meta, &self.store, &self.sources, &kept.book)
    }

    /// Continue's covers, in its order: AniList's book where one is kept, else what the store
    /// holds of the work, so Continue stays while AniList can't be reached.
    fn continue_tiles(&self, continuing: &[String]) -> Vec<WorkTile> {
        let kept = tiles::kept_of(&self.meta, continuing);
        continuing
            .iter()
            .filter_map(|work| {
                match kept.iter().find(|k| tiles::work_of(k.book.id).as_str() == work) {
                    Some(kept) => Some(self.tile(kept)),
                    None => self.known(work),
                }
            })
            .collect()
    }

    /// A work's cover from what the store holds of it, without AniList's: its title, medium
    /// and kind, with where the person is in it.
    fn known(&self, work: &str) -> Option<WorkTile> {
        let kind = self.store.book_work_kind(work).ok().flatten()?;
        let title = kind.title_in(crate::data::title_form(self.meta.titles()))?;
        let medium = Medium::parse(&kind.medium).unwrap_or(Medium::Manga);
        let format = kind.format.as_deref().map(str::to_ascii_lowercase);
        let origin = kind.origin.as_deref().and_then(kaeri_core::media::Origin::parse);
        let mut tile = WorkTile {
            work: kaeri_core::books::WorkId::new(work.to_owned()),
            title,
            medium,
            cover: None,
            format_label: None,
            progress: None,
            next: None,
            new_chapter: false,
            adult: false,
            facts: vec![words::kind_words(format.as_deref(), medium, kind.origin.as_deref())],
            demo: false,
            not_in_sources: false,
            origin: tiles::origin(origin),
            amount: None,
            started: None,
        };
        tiles::mark(&mut tile, &self.store, &self.sources);
        Some(tile)
    }

    fn shown(&self, book: &Book) -> bool {
        self.ask.adult || !book.adult()
    }

    /// A row's titles as kept, however old, with when they were asked; adult ones left out
    /// unless they are on. Each says so when it was opened and no source had it since: asked of
    /// the sources as they stand, and with no series of a source that is on matched to it since,
    /// as a folder's scan matches one.
    fn shelf(&self, row: Planned, name: Option<&str>) -> Option<(Vec<WorkTile>, i64)> {
        let shelf =
            Shelf::of(&self.store, &key_of(row)).filter(|s| s.name.as_deref() == name)?;
        let not_found =
            self.sources.catalog.not_found.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let mark = (!not_found.is_empty()).then(|| self.sources.sources_mark());
        let tiles = self
            .meta
            .kept_books(&shelf.ids())
            .iter()
            .filter(|k| self.shown(&k.book))
            .map(|k| {
                let mut tile = tiles::plain(&self.meta, &k.book);
                tile.not_in_sources = not_found
                    .get(tile.work.as_str())
                    .is_some_and(|(at, asked)| *at >= shelf.at && Some(*asked) == mark)
                    && self.sources.edition_of(&tile.work, None).is_none();
                // A New row's caption says when each series started: "Japan · since March".
                if matches!(row, Planned::New(_)) {
                    tile.started = tiles::started(&k.book);
                }
                tile
            })
            .collect();
        Some((tiles, shelf.at))
    }

    /// The discovery rows, in the mix's order, each title once and none of the person's own.
    fn rows(&self, pending: &[ReadHomePart], seen: &mut HashSet<String>) -> Vec<ReadHomeRow> {
        let genre = self.genre();
        let finished = self.finished();
        let because_title = finished
            .as_deref()
            .and_then(tiles::anilist)
            .and_then(|id| self.meta.kept_book(id))
            .map(|k| self.meta.titles().pick(&k.book.title).to_owned());
        let mut planned = Vec::new();
        let mut tiles = Vec::new();
        for row in self.plan() {
            let name = match row {
                Planned::Genre => genre.as_deref(),
                Planned::Because => finished.as_deref(),
                _ => None,
            };
            let Some(title) = row.title(because_title.as_deref(), genre.as_deref()) else {
                continue;
            };
            let part = match row {
                Planned::Trending(_) => ReadHomePart::Yours,
                Planned::Because => ReadHomePart::Because,
                _ => ReadHomePart::Discovery,
            };
            let kept = self.shelf(row, name).map(|(t, _)| t);
            let on_its_way = kept.is_none() && pending.contains(&part);
            planned.push((row, title, on_its_way));
            tiles.push(kept.unwrap_or_default());
        }
        discovery::once_each(&mut tiles, seen);
        planned
            .into_iter()
            .zip(tiles)
            // A row with nothing, and nothing on its way, leaves with its heading.
            .filter(|((_, _, pending), tiles)| *pending || !tiles.is_empty())
            .map(|((row, title, pending), tiles)| ReadHomeRow {
                id: row.id(),
                kind: row.kind(),
                format: row.format(),
                title,
                side: row.side(),
                tiles,
                view_all: row.query(genre.as_deref()),
                view_all_title: row
                    .query(genre.as_deref())
                    .and_then(|_| row.view_all_title(genre.as_deref())),
                pending,
            })
            .collect()
    }

    fn view(&self) -> ReadingHomeView {
        let (pending, failed, offline_since) = {
            let memory = self.memory();
            let mut pending: Vec<ReadHomePart> = memory.parts.iter().copied().collect();
            pending.sort_by_key(|p| *p as u8);
            (pending, memory.failed, memory.offline_since)
        };
        let continuing = self.continuing();
        let continue_tiles = self.continue_tiles(&continuing);
        let following = self.following();
        let followed = tiles::kept_of(&self.meta, &following);
        let week = self.week(&followed);
        let mut seen = self.own();
        let up_next = self.up_next(&continuing, &seen);
        seen.extend(up_next.iter().map(|t| t.work.0.clone()));
        let rows = self.rows(&pending, &mut seen);
        let row_of = |kind| rows.iter().find(|r| r.kind == kind);
        let trending: Vec<WorkTile> = rows
            .iter()
            .filter(|r| r.kind == kaeri_model::reading::ReadRowKind::Trending)
            .flat_map(|r| r.tiles.iter().cloned())
            .collect();
        // The rows' first fields, for front ends not built against rows yet: the first
        // Trending row in place of Releasing now.
        let releasing = row_of(kaeri_model::reading::ReadRowKind::Trending)
            .map(|r| r.tiles.clone())
            .unwrap_or_default();
        let because_row =
            row_of(kaeri_model::reading::ReadRowKind::Because).filter(|r| !r.tiles.is_empty());
        let because_title = because_row.and_then(|_| {
            self.finished()
                .as_deref()
                .and_then(tiles::anilist)
                .and_then(|id| self.meta.kept_book(id))
                .map(|k| self.meta.titles().pick(&k.book.title).to_owned())
        });
        let because = because_row.map(|r| r.tiles.clone()).unwrap_or_default();
        let genre_row = row_of(kaeri_model::reading::ReadRowKind::MoreGenre)
            .filter(|r| !r.tiles.is_empty());
        let genre = genre_row.and_then(|_| self.genre());
        let genre_shelf = genre_row.map(|r| r.tiles.clone()).unwrap_or_default();
        let spot = self.spotlight(&followed, &continue_tiles, &up_next, &trending);
        let end = pending.is_empty().then_some(if self.no_sources {
            ReadHomeEnd::NoSources
        } else {
            ReadHomeEnd::Everything
        });
        let offline = offline_since.is_some() || self.offline.load(Ordering::Relaxed);
        ReadingHomeView {
            spotlight: spot.as_ref().map(|(tile, _)| tile.clone()),
            continue_reading: continue_tiles,
            this_week: week
                .iter()
                .flat_map(|d| d.entries.iter().map(|e| e.tile.clone()))
                .collect(),
            up_next,
            releasing,
            because_title,
            because,
            genre,
            genre_shelf,
            no_sources: self.no_sources,
            spot: spot.map(|(_, spot)| spot),
            week,
            pending,
            offline,
            offline_since: offline.then_some(offline_since).flatten(),
            failed,
            end,
            rows,
        }
    }

    /// The Planning list, then what is like the series read most recently.
    fn up_next(&self, continuing: &[String], seen: &HashSet<String>) -> Vec<WorkTile> {
        let mut out: Vec<WorkTile> = tiles::kept_of(&self.meta, &self.planning())
            .iter()
            .map(|k| {
                let mut tile = tiles::plain(&self.meta, &k.book);
                tile.facts.push(tiles::on_planning());
                tile
            })
            .collect();
        let liked: Vec<AnilistId> = continuing
            .iter()
            .filter_map(|w| tiles::anilist(w))
            .filter_map(|id| self.meta.kept_book(id))
            .flat_map(|k| k.book.recommended())
            .collect();
        for kept in self.meta.kept_books(&liked) {
            let work = tiles::work_of(kept.book.id);
            if out.len() >= SHELF as usize {
                break;
            }
            if seen.contains(work.as_str())
                || out.iter().any(|t| t.work == work)
                || !self.shown(&kept.book)
            {
                continue;
            }
            out.push(tiles::plain(&self.meta, &kept.book));
        }
        out
    }

    /// This week, today first: each followed series on the day its chapters usually come
    /// out, or on today when one came out today.
    fn week(&self, followed: &[KeptBook]) -> Vec<ReadWeekDay> {
        let now = self.ask.now;
        let Some(today) = chapters::day_start(now, &self.zone) else { return Vec::new() };
        let mut days: Vec<ReadWeekDay> = (0..7)
            .map(|n| ReadWeekDay {
                day: chapters::day_start(today + n * DAY + DAY / 2, &self.zone)
                    .unwrap_or(today + n * DAY),
                today: n == 0,
                entries: Vec::new(),
            })
            .collect();
        for kept in followed {
            let work = tiles::work_of(kept.book.id);
            let rhythm = self.store.book_rhythm(work.as_str()).ok().flatten();
            let list = chapters::of_work(&self.sources, &self.store, &work).map(|(_, _, l)| l);
            let newest_chapter =
                list.as_deref().and_then(|l| l.iter().max_by_key(|c| c.out_at()));
            let newest = newest_chapter
                .and_then(chapters::Kept::out_at)
                .or(rhythm.and_then(|r| r.last_published));
            let mut tile = self.tile(kept);
            let out_today = newest.filter(|at| *at >= today && *at <= now);
            let due = match (out_today, rhythm) {
                (Some(at), _) => Some((0usize, Some(at), false)),
                (None, Some(r)) => self.due_day(&r, &days).map(|i| (i, None, true)),
                (None, None) => None,
            };
            let Some((index, out_at, usually)) = due else { continue };
            if usually && let Some(words) = rhythm.as_ref().and_then(chapters::rhythm_words) {
                tile.facts.insert(0, words);
            }
            let novel = kept.book.medium() == Medium::Novel;
            let chapter = match out_at {
                Some(_) => newest_chapter.map(|c| {
                    let number = c.number.clone().unwrap_or_else(|| c.label.clone());
                    let named = words::part_named(&c.label, c.number.as_deref());
                    Msg::Reading(match (c.own_title(), novel, named) {
                        (Some(title), false, _) => ReadingMsg::ReadingChapterTitled {
                            chapter: number,
                            title: title.to_owned(),
                        },
                        (Some(title), true, Some(label)) => {
                            ReadingMsg::ReadingLabelTitled { label, title: title.to_owned() }
                        }
                        (Some(title), true, None) => ReadingMsg::ReadingPartTitled {
                            part: number,
                            title: title.to_owned(),
                        },
                        (None, _, named) => {
                            return words::next_unit(
                                kept.book.medium(),
                                &number,
                                named.as_deref(),
                            );
                        }
                    })
                }),
                // Only expected: the chapter after the newest, when that is a confident guess,
                // named the way the source names the ones before it.
                None => rhythm.as_ref().zip(list.as_deref()).and_then(|(r, l)| {
                    let n = expected_next(r, l, days[index].day)?;
                    let named = named_like(l, &n);
                    Some(words::next_unit(kept.book.medium(), &n, named.as_deref()))
                }),
            };
            days[index].entries.push(ReadWeekEntry { tile, out_at, usually, chapter });
        }
        days.retain(|d| d.today || !d.entries.is_empty());
        if days.iter().all(|d| d.entries.is_empty()) {
            return Vec::new();
        }
        days
    }

    /// The day of this week a series' rhythm says its next chapter comes.
    fn due_day(
        &self,
        rhythm: &kaeri_store::book_catalog::BookRhythm,
        days: &[ReadWeekDay],
    ) -> Option<usize> {
        if let Some(weekday) = rhythm.weekday {
            return days
                .iter()
                .position(|d| chapters::weekday(d.day + DAY / 2, &self.zone) == Some(weekday));
        }
        let every = i64::from(rhythm.every_days?).max(1);
        let mut next = rhythm.last_published? + every * DAY;
        while next < days.first()?.day {
            next += every * DAY;
        }
        let start = chapters::day_start(next, &self.zone)?;
        days.iter().position(|d| d.day == start)
    }

    /// The series at the top: a new chapter in one followed, the one in progress, the first of
    /// Up next, or the most popular still coming out.
    fn spotlight(
        &self,
        followed: &[KeptBook],
        continuing: &[WorkTile],
        up_next: &[WorkTile],
        trending: &[WorkTile],
    ) -> Option<(WorkTile, ReadSpotlight)> {
        let today = chapters::day_start(self.ask.now, &self.zone).unwrap_or(self.ask.now);
        for kept in followed {
            let work = tiles::work_of(kept.book.id);
            let read = chapters::Read::of(&self.store, work.as_str());
            let Some((_, _, list)) = chapters::of_work(&self.sources, &self.store, &work)
            else {
                continue;
            };
            // Only for someone caught up to it: someone still behind is Continuing. The line
            // names the chapter the button opens.
            let Some(next) = read.new_for_you(&list) else { continue };
            let number = next.number.clone().unwrap_or_else(|| next.label.clone());
            let medium = kept.book.medium();
            let today_out = next.out_at().is_some_and(|at| at >= today);
            // A light novel's part goes by the source's own name for it where it has one, as
            // the button below and its row do.
            let named = words::part_named(&next.label, next.number.as_deref())
                .filter(|_| medium == Medium::Novel);
            let line =
                match (next.own_title(), today_out, medium == Medium::Novel, named.clone()) {
                    (Some(title), true, false, _) => ReadingMsg::ReadingOutTodayTitled {
                        chapter: number,
                        title: title.to_owned(),
                    },
                    (Some(title), true, true, Some(label)) => {
                        ReadingMsg::ReadingLabelOutTodayTitled {
                            label,
                            title: title.to_owned(),
                        }
                    }
                    (Some(title), true, true, None) => ReadingMsg::ReadingPartOutTodayTitled {
                        part: number,
                        title: title.to_owned(),
                    },
                    (None, true, false, _) => ReadingMsg::ReadingOutToday { chapter: number },
                    (None, true, true, Some(label)) => {
                        ReadingMsg::ReadingLabelOutToday { label }
                    }
                    (None, true, true, None) => {
                        ReadingMsg::ReadingPartOutToday { part: number }
                    }
                    (_, false, false, _) => ReadingMsg::ReadingChapterNew { chapter: number },
                    (_, false, true, Some(label)) => ReadingMsg::ReadingLabelNew { label },
                    (_, false, true, None) => ReadingMsg::ReadingPartNew { part: number },
                };
            let action = tiles::read_action(
                medium,
                ChapterId::new(next.id.clone()),
                next.number.clone().unwrap_or_else(|| next.label.clone()),
                named,
            );
            return Some(self.spot(
                kept,
                ReadSpotlightCase::NewChapter,
                ReadingMsg::ReadingSpotlightNewChapter,
                Some(Msg::Reading(line)),
                Some(action),
            ));
        }
        if let Some(tile) = continuing.first()
            && let Some(kept) =
                tiles::anilist(tile.work.as_str()).and_then(|id| self.meta.kept_book(id))
        {
            let action = self.continue_action(&kept.book);
            return Some(self.spot(
                &kept,
                ReadSpotlightCase::Continuing,
                ReadingMsg::ReadingSpotlightContinue,
                tile.next.clone(),
                action,
            ));
        }
        if let Some(tile) = up_next.first()
            && let Some(kept) =
                tiles::anilist(tile.work.as_str()).and_then(|id| self.meta.kept_book(id))
        {
            let mut found = self.spot(
                &kept,
                ReadSpotlightCase::UpNext,
                ReadingMsg::ReadingSpotlightUpNext,
                None,
                None,
            );
            found.1.planned = self.planning().contains(&tile.work.0);
            return Some(found);
        }
        // The popular pick: the Trending rows' most popular series still coming out, or with
        // none coming out, their most popular.
        let popular =
            trending.iter().filter_map(|t| tiles::anilist(t.work.as_str())).collect::<Vec<_>>();
        let kept = self.meta.kept_books(&popular);
        let releasing =
            |k: &&KeptBook| k.book.status == Some(kaeri_core::media::MediaStatus::Releasing);
        let best = kept
            .iter()
            .filter(releasing)
            .max_by_key(|k| k.book.popularity.unwrap_or_default())
            .or_else(|| kept.iter().max_by_key(|k| k.book.popularity.unwrap_or_default()))?
            .clone();
        Some(self.spot(
            &best,
            ReadSpotlightCase::Popular,
            ReadingMsg::ReadingSpotlightPopular,
            None,
            None,
        ))
    }

    fn spot(
        &self,
        kept: &KeptBook,
        case: ReadSpotlightCase,
        label: ReadingMsg,
        line: Option<Msg>,
        action: Option<SeriesAction>,
    ) -> (WorkTile, ReadSpotlight) {
        let tile = self.tile(kept);
        // The same facts as the series' page, with how often chapters come out after them.
        let mut facts = tiles::facts(&kept.book);
        let work = tiles::work_of(kept.book.id);
        if let Some(rhythm) = self
            .store
            .book_rhythm(work.as_str())
            .ok()
            .flatten()
            .as_ref()
            .and_then(chapters::rhythm_words)
        {
            facts.push(rhythm);
        }
        let spot = ReadSpotlight {
            case,
            label: Msg::Reading(label),
            facts,
            synopsis: kept.book.synopsis(),
            line,
            action,
            planned: false,
            banner: kept.book.banner_image.clone(),
        };
        (tile, spot)
    }

    /// Where Continue opens a series: the chapter in progress, or the next one.
    fn continue_action(&self, book: &Book) -> Option<SeriesAction> {
        let work = tiles::work_of(book.id);
        Some(tiles::action(&self.store, &self.sources, &work, book.medium()))
            .filter(|a| !matches!(a, SeriesAction::NoSource | SeriesAction::ReadAgain))
    }
}

/// The source's name for a part not listed yet, numbered `number`, after the way it names the
/// newest it lists: "Chapter 13" after "Chapter 12". None when it names them by number alone.
fn named_like(list: &[chapters::Kept], number: &str) -> Option<String> {
    let newest = list
        .iter()
        .filter_map(|c| Some((c.number.as_deref()?.parse::<f64>().ok()?, c)))
        .max_by(|a, b| a.0.total_cmp(&b.0))?
        .1;
    let at = newest.number.as_deref()?;
    let named = words::part_named(&newest.label, Some(at))?;
    let (before, after) = named.rsplit_once(at)?;
    Some(format!("{before}{number}{after}"))
}

/// The chapter a series' rhythm says comes out on `due`: the one after its newest, when that
/// is a confident guess. That needs whole numbers that go up with the dates, and a newest
/// chapter recent enough that no release was missed between it and `due`.
fn expected_next(
    rhythm: &kaeri_store::book_catalog::BookRhythm,
    list: &[chapters::Kept],
    due: i64,
) -> Option<String> {
    let every = match (rhythm.every_days, rhythm.weekday) {
        (Some(days), _) => i64::from(days.max(1)),
        (None, Some(_)) => 7,
        (None, None) => return None,
    };
    let numbered: Vec<(u64, i64)> = list
        .iter()
        .filter_map(|c| Some((c.number.as_deref()?.parse::<u64>().ok()?, c.out_at()?)))
        .collect();
    let (newest, at) = numbered.iter().max_by_key(|(n, _)| *n).copied()?;
    // The highest number is also the newest out: a list that runs the other way is no guide.
    if numbered.iter().any(|(n, out)| *out > at && *n < newest) {
        return None;
    }
    // One step of the rhythm, with a day to spare: more would mean a chapter in between.
    let gap = due - at;
    if gap <= 0 || gap > (every + 1) * DAY {
        return None;
    }
    Some((newest + 1).to_string())
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, atomic::AtomicBool};

    use kaeri_core::{books::SourceChapter, config::Paths};
    use kaeri_model::{medium::KaeriIsFor, reading::ReadSpotlightCase};
    use kaeri_store::{
        book_catalog::BookWork,
        reading::{EditionPref, ReadEvent},
    };

    use super::*;

    #[test]
    fn an_expected_chapter_is_named_only_when_the_guess_is_sure() {
        const DUE: i64 = 1_700_000_000;
        let chapter = |n: &str, at: i64| chapters::Kept {
            id: n.into(),
            source_ref: n.into(),
            label: n.into(),
            number: Some(n.into()),
            volume: None,
            title: None,
            version_label: None,
            language: None,
            published: Some(at),
            pages: None,
            seen_at: None,
            part: None,
        };
        let weekly = kaeri_store::book_catalog::BookRhythm {
            weekday: Some(4),
            every_days: Some(7),
            last_published: Some(DUE - 7 * DAY),
            checked_at: DUE,
        };
        let list = [chapter("42", DUE - 14 * DAY), chapter("43", DUE - 7 * DAY)];
        assert_eq!(expected_next(&weekly, &list, DUE).as_deref(), Some("44"));
        // A release missed in between, or a half chapter last: no guess.
        let behind = [chapter("42", DUE - 21 * DAY), chapter("43", DUE - 14 * DAY)];
        assert_eq!(expected_next(&weekly, &behind, DUE), None);
        let half = [chapter("43", DUE - 14 * DAY), chapter("43.5", DUE - 7 * DAY)];
        assert_eq!(expected_next(&weekly, &half, DUE), None);
        // Numbers that go down with the dates are no guide.
        let odd = [chapter("50", DUE - 14 * DAY), chapter("43", DUE - 7 * DAY)];
        assert_eq!(expected_next(&weekly, &odd, DUE), None);
        // No rhythm, no guess.
        let none =
            kaeri_store::book_catalog::BookRhythm { weekday: None, every_days: None, ..weekly };
        assert_eq!(expected_next(&none, &list, DUE), None);
    }

    const WORK: &str = "a:900301";
    // Noon UTC on a fixed day, read in UTC, so nothing depends on the machine's clock or zone.
    const NOW: i64 = 1_700_000_000 - 1_700_000_000 % DAY + 12 * 3_600;

    fn kit(dir: &std::path::Path) -> Kit {
        let store = Store::open_in_memory().unwrap();
        let paths = Paths {
            config_file: dir.join("config.toml"),
            data_dir: dir.join("data"),
            cache_dir: dir.join("cache"),
        };
        let tokens = kaeri_track::TokenStore::new(dir.join("tokens"));
        let sources = BookSources::new(store.clone(), tokens, reqwest::Client::new(), &paths);
        let http = kaeri_net::HttpClient::offline(&Default::default()).unwrap();
        let meta =
            kaeri_meta::Meta::new(kaeri_meta::AniList::new(http, 30), None, store.clone());
        Kit {
            meta,
            store,
            sources,
            plugins: crate::book_plugins::BookPlugins::default(),
            clock: crate::clock::Clock::in_zone(jiff::tz::TimeZone::UTC),
            offline: Arc::new(AtomicBool::new(false)),
        }
    }

    /// A series on Reading whose source listed chapters 1 to 12 a week ago, and chapter 13,
    /// "The Far Shore", an hour ago.
    fn followed(kit: &Kit) {
        let store = &kit.store;
        let meta = r#"{"book":{"id":900301,"title":{"romaji":"The Lantern Ferry"}}}"#;
        let work = BookWork {
            work: WORK.into(),
            anilist_id: Some(900_301),
            medium: "manga".into(),
            titles: "{}".into(),
            status: Some("RELEASING".into()),
            meta: Some(meta.into()),
            fetched_at: NOW,
            ..BookWork::default()
        };
        store.put_book_work(&work).unwrap();
        store.set_book_match("shelf", "s-1", WORK, 100, true, false, 1).unwrap();
        let pref =
            EditionPref { source_id: "shelf".into(), version_label: None, language: None };
        store.set_edition_pref(WORK, Some(&pref)).unwrap();
        store.set_read_status(WORK, Some("reading"), 2).unwrap();
        let list = |n: u32| -> Vec<SourceChapter> {
            (1..=n).map(|n| SourceChapter::new(format!("c-{n}"), n.to_string())).collect()
        };
        chapters::keep(store, "shelf", "s-1", &list(12), NOW - 7 * DAY);
        let mut later = list(13);
        later[12].title = Some("The Far Shore".into());
        later[12].published = Some(NOW - 3_600);
        chapters::keep(store, "shelf", "s-1", &later, NOW - 3_600);
    }

    fn home(kit: &Kit) -> ReadingHomeView {
        let ask =
            Ask { answer: KaeriIsFor::Both, adult: false, chapter_notices: false, now: NOW };
        Home::of(kit, ask).view()
    }

    fn reading(msg: ReadingMsg) -> Option<Msg> {
        Some(Msg::Reading(msg))
    }

    /// Home loaded offline says so; loaded again once the connection is back, with every row
    /// it shows first still fresh, it no longer does.
    #[tokio::test]
    async fn the_offline_line_goes_when_a_load_runs_online() {
        let dir = tempfile::tempdir().unwrap();
        let kit = kit(dir.path());
        for format in ReadRowFormat::ALL {
            Shelf { at: NOW, ids: Vec::new(), name: None }
                .keep(&kit.store, &key_of(Planned::Trending(format)));
        }
        let ask =
            Ask { answer: KaeriIsFor::Both, adult: false, chapter_notices: false, now: NOW };
        let (tx, _rx) = Outbox::inbox();
        kit.offline.store(true, Ordering::Relaxed);
        Home::of(&kit, ask).run(false, &tx).await;
        let view = home(&kit);
        assert!(view.offline && view.offline_since == Some(NOW));

        kit.offline.store(false, Ordering::Relaxed);
        Home::of(&kit, ask).run(false, &tx).await;
        let view = home(&kit);
        assert!(!view.offline, "back online");
        assert_eq!(view.offline_since, None);
    }

    /// A refresh sent while the first load runs starts its own; the first one ending leaves
    /// the refresh's parts on their way and the load running, so Home does not say it is done
    /// before the refresh's rows land.
    #[test]
    fn a_refresh_during_a_load_keeps_its_parts_when_the_first_load_ends() {
        let dir = tempfile::tempdir().unwrap();
        let kit = kit(dir.path());
        let ask =
            Ask { answer: KaeriIsFor::Both, adult: false, chapter_notices: false, now: NOW };
        let memory = &kit.sources.catalog.home;
        let first = begin(memory, false).unwrap();
        assert_eq!(begin(memory, false), None, "a second load joins the one running");
        let refresh = begin(memory, true).unwrap();
        let (tx, _rx) = Outbox::inbox();
        let older = Home { generation: first, ..Home::of(&kit, ask) };
        older.landed(ReadHomePart::Yours);
        older.finish(&tx);
        {
            let loading = memory.lock().unwrap();
            assert!(loading.running, "the refresh still runs");
            assert_eq!(loading.parts.len(), 3, "the refresh's parts are still on their way");
        }
        let newer = Home { generation: refresh, ..Home::of(&kit, ask) };
        newer.landed(ReadHomePart::Yours);
        newer.finish(&tx);
        let loading = memory.lock().unwrap();
        assert!(!loading.running && loading.parts.is_empty());
    }

    #[test]
    fn someone_caught_up_is_offered_the_new_chapter_with_its_words() {
        let dir = tempfile::tempdir().unwrap();
        let kit = kit(dir.path());
        followed(&kit);
        kit.store.adopt_read(WORK, "12", None, NOW - 2 * DAY).unwrap();

        let view = home(&kit);
        let spot = view.spot.unwrap();
        assert_eq!(spot.case, ReadSpotlightCase::NewChapter);
        assert_eq!(
            spot.line,
            reading(ReadingMsg::ReadingOutTodayTitled {
                chapter: "13".into(),
                title: "The Far Shore".into()
            })
        );
        assert_eq!(
            spot.action,
            Some(SeriesAction::Read { chapter: ChapterId::new("13"), label: "13".into() }),
            "the button opens the chapter the line names"
        );
        let tile = view.spotlight.unwrap();
        assert!(tile.new_chapter, "the dot");
        assert_eq!(
            tile.next,
            reading(ReadingMsg::ReadingChapterNew { chapter: "13".into() }),
            "the dot always has its words"
        );

        // This week: the chapter out today, in words, beside the person's place.
        let today = &view.week[0];
        assert!(today.today);
        assert_eq!(
            today.entries[0].chapter,
            reading(ReadingMsg::ReadingChapterTitled {
                chapter: "13".into(),
                title: "The Far Shore".into()
            })
        );
        assert_eq!(today.entries[0].out_at, Some(NOW - 3_600));
    }

    #[test]
    fn someone_behind_continues_where_they_stopped_with_the_track_and_no_dot() {
        let dir = tempfile::tempdir().unwrap();
        let kit = kit(dir.path());
        followed(&kit);
        kit.store.adopt_read(WORK, "10", None, NOW - 3 * DAY).unwrap();
        let mut place = ReadEvent::new(WORK, "11", NOW - 2 * DAY);
        place.page = Some(8);
        place.of_pages = Some(20);
        kit.store.record_read(&place, Vec::new()).unwrap();

        let view = home(&kit);
        let spot = view.spot.unwrap();
        assert_eq!(spot.case, ReadSpotlightCase::Continuing, "chapter 13 waits behind 11");
        assert_eq!(
            spot.line,
            reading(ReadingMsg::ReadingChapterPage { chapter: "11".into(), page: 9 }),
            "page 8 kept from 0 is page 9"
        );
        let tile = &view.continue_reading[0];
        assert!(!tile.new_chapter, "no dot on a place");
        assert_eq!(tile.progress, Some(0.4));
        assert_eq!(tile.next, spot.line);
    }
}
