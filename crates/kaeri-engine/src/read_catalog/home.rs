//! Reading's Home: the spotlight, Continue, This week, Up next, Releasing now, Because you
//! finished and the genre shelf.
//!
//! Home shows at once from what was kept. Then one AniList request asks for everything the
//! screen lacks: the books the person reads that nothing is kept of, and the discovery shelves
//! when they are stale. After it the launch's background work may begin, and Because you
//! finished and Up next's suggestions are asked for in the paced queue.

use std::{
    collections::{HashMap, HashSet},
    sync::atomic::Ordering,
};

use kaeri_core::{books::ChapterId, ids::AnilistId};
use kaeri_meta::{
    BrowseFilter, KeptBook, Meta,
    anilist::books::{Book, BookAsk},
};
use kaeri_model::{
    Msg, ReadingMsg,
    reading::{
        ReadHomeEnd, ReadHomePart, ReadSpotlight, ReadSpotlightCase, ReadWeekDay,
        ReadWeekEntry, ReadingEvent, ReadingHomeView, SeriesAction, WorkTile,
    },
};
use kaeri_store::Store;
use serde::{Deserialize, Serialize};

use super::{Ask, Kit, chapters, send, tiles};
use crate::{outbox::Outbox, read_sources::BookSources};

/// How long a discovery shelf is shown again before AniList is asked again.
const SHELF_KEPT_FOR: i64 = 30 * 60;
const RELEASING: &str = "read:home:releasing";
const GENRE: &str = "read:home:genre";
const BECAUSE: &str = "read:home:because";
const SHELF: u32 = 20;
const CONTINUE: usize = 12;
/// The genre shelf's genre when nothing read says one.
const FALLBACK_GENRE: &str = "Fantasy";
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
}

/// A discovery shelf as kept: the books on it, when it was asked, and what it is about.
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

/// Load Home: at once from what was kept, then again as each part lands.
pub fn load(kit: &Kit, ask: Ask, refresh: bool, tx: &Outbox) {
    let memory = kit.sources.catalog.home.clone();
    let already = {
        let mut loading = memory.lock().unwrap_or_else(|e| e.into_inner());
        let already = loading.running && !refresh;
        if !already {
            loading.running = true;
            loading.failed = false;
            loading.parts =
                [ReadHomePart::Yours, ReadHomePart::Discovery, ReadHomePart::Because]
                    .into_iter()
                    .collect();
        }
        already
    };
    let home = Home::of(kit, ask);
    home.send(tx);
    if already {
        return;
    }
    let tx = tx.clone();
    tokio::spawn(async move {
        home.run(refresh, &tx).await;
    });
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
        }
    }

    fn memory(&self) -> std::sync::MutexGuard<'_, Loading> {
        self.sources.catalog.home.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn landed(&self, part: ReadHomePart) {
        self.memory().parts.remove(&part);
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
            memory.parts.clear();
            memory.running = false;
        }
        self.send(tx);
    }

    fn went_offline(&self) {
        let since = [RELEASING, GENRE, BECAUSE]
            .iter()
            .filter_map(|key| Shelf::of(&self.store, key))
            .map(|s| s.at)
            .max();
        let mut memory = self.memory();
        memory.offline_since = since;
        memory.failed = since.is_none() && self.yours().is_empty();
    }

    /// The one request the screen waits for: the person's books nothing is kept of, and the
    /// discovery shelves that are stale.
    async fn screen(&self, refresh: bool, tx: &Outbox) {
        let now = self.ask.now;
        let yours = self.yours();
        let missing = if refresh {
            yours.iter().filter_map(|w| tiles::anilist(w)).collect()
        } else {
            tiles::missing(&self.meta, &yours, now)
        };
        let releasing = Shelf::of(&self.store, RELEASING);
        let genre_name = self.genre();
        let genre = Shelf::of(&self.store, GENRE)
            .filter(|s| s.name.as_deref() == Some(genre_name.as_str()));
        let stale = |s: &Option<Shelf>| refresh || !s.as_ref().is_some_and(|s| s.fresh(now));
        let mut asks = Vec::new();
        let mut at = (None, None, None);
        for chunk in missing.chunks(50) {
            at.0.get_or_insert(asks.len());
            asks.push(BookAsk::Ids(chunk.to_vec()));
        }
        let adult = (!self.ask.adult).then_some(false);
        if stale(&releasing) {
            at.1 = Some(asks.len());
            let filter = BrowseFilter {
                sort: vec!["TRENDING_DESC".into()],
                statuses: vec!["RELEASING".into()],
                adult,
                ..BrowseFilter::default()
            };
            asks.push(BookAsk::browse("", &filter, 1, SHELF));
        }
        // The genre is the one the person reads most, which needs their books: with some not
        // kept yet, the shelf is asked for behind the screen's request, once they are.
        let genre_later = stale(&genre) && !missing.is_empty();
        if stale(&genre) && !genre_later {
            at.2 = Some(asks.len());
            let filter = BrowseFilter {
                sort: vec!["POPULARITY_DESC".into()],
                genres: vec![genre_name.clone()],
                adult,
                ..BrowseFilter::default()
            };
            asks.push(BookAsk::browse("", &filter, 1, SHELF));
        }
        let discovery_done = |home: &Self| {
            if !genre_later {
                home.landed(ReadHomePart::Discovery);
            }
        };
        if asks.is_empty() {
            self.landed(ReadHomePart::Yours);
            discovery_done(self);
            self.send(tx);
            return;
        }
        match self.meta.books(&asks).await {
            Ok(pages) => {
                let ids_of = |i: usize| -> Vec<u32> {
                    pages
                        .get(i)
                        .map(|p| p.items.iter().map(|b| b.id.get()).collect())
                        .unwrap_or_default()
                };
                if let Some(i) = at.1 {
                    Shelf { at: now, ids: ids_of(i), name: None }.keep(&self.store, RELEASING);
                }
                if let Some(i) = at.2 {
                    Shelf { at: now, ids: ids_of(i), name: Some(genre_name) }
                        .keep(&self.store, GENRE);
                }
                self.memory().offline_since = None;
            }
            Err(e) => {
                tracing::info!(error = %e, "reading's Home was not asked for");
                self.went_offline();
            }
        }
        self.landed(ReadHomePart::Yours);
        discovery_done(self);
        self.send(tx);
    }

    /// The paced part: Because you finished, and Up next's suggestions from what is read.
    async fn behind(&self, refresh: bool, tx: &Outbox) {
        if self.memory().offline_since.is_some() || self.memory().failed {
            return;
        }
        let now = self.ask.now;
        let mut asked = false;
        // The genre shelf, when the screen's request had to find the person's books first.
        let genre_name = self.genre();
        let genre = Shelf::of(&self.store, GENRE);
        let genre_stale = refresh
            || genre.is_none_or(|s| {
                !s.fresh(now) || s.name.as_deref() != Some(genre_name.as_str())
            });
        if self.memory().parts.contains(&ReadHomePart::Discovery) && genre_stale {
            asked = true;
            let filter = BrowseFilter {
                sort: vec!["POPULARITY_DESC".into()],
                genres: vec![genre_name.clone()],
                adult: (!self.ask.adult).then_some(false),
                ..BrowseFilter::default()
            };
            match self.meta.books(&[BookAsk::browse("", &filter, 1, SHELF)]).await {
                Ok(pages) => {
                    let ids = pages
                        .first()
                        .map(|p| p.items.iter().map(|b| b.id.get()).collect())
                        .unwrap_or_default();
                    Shelf { at: now, ids, name: Some(genre_name) }.keep(&self.store, GENRE);
                }
                Err(e) => tracing::info!(error = %e, "the genre shelf was not asked for"),
            }
        }
        self.landed(ReadHomePart::Discovery);
        // The series to say "Because you finished" of: the one finished most recently.
        let finished = self.finished();
        let kept = Shelf::of(&self.store, BECAUSE);
        let because_stale = refresh
            || kept
                .as_ref()
                .is_none_or(|s| !s.fresh(now) || s.name.as_deref() != finished.as_deref());
        if let Some(work) = finished.as_deref()
            && because_stale
            && let Some(id) = tiles::anilist(work)
        {
            asked = true;
            if let Some(book) = self.detail(id, refresh).await {
                let ids: Vec<AnilistId> = book
                    .recommended()
                    .into_iter()
                    .filter(|r| !self.on_a_list(*r))
                    .take(SHELF as usize)
                    .collect();
                self.fetch_missing(&ids).await;
                let ids = ids.iter().map(|id| id.get()).collect();
                Shelf { at: now, ids, name: Some(work.to_owned()) }.keep(&self.store, BECAUSE);
            }
        }
        self.landed(ReadHomePart::Because);
        // Up next's suggestions: what is like the series read most recently.
        if let Some(work) = self.continuing().first()
            && let Some(id) = tiles::anilist(work)
            && self.meta.kept_book(id).is_none_or(|k| !k.detail_fresh(now))
        {
            asked = true;
            if let Some(book) = self.detail(id, refresh).await {
                let ids: Vec<AnilistId> = book.recommended();
                self.fetch_missing(&ids).await;
            }
        }
        if asked {
            self.send(tx);
        }
    }

    /// A book's page, asked again only when what was kept of it is old.
    async fn detail(&self, id: AnilistId, refresh: bool) -> Option<Book> {
        let kept = self.meta.kept_book(id);
        if !refresh && let Some(kept) = kept.as_ref().filter(|k| k.detail_fresh(self.ask.now)) {
            return Some(kept.book.clone());
        }
        match self.meta.book(id).await {
            Ok(book) => Some(book),
            Err(e) => {
                tracing::info!(error = %e, "a book's page was not asked for");
                kept.map(|k| k.book)
            }
        }
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

    fn on_a_list(&self, id: AnilistId) -> bool {
        let work = tiles::work_of(id);
        self.store.read_status(work.as_str()).ok().and_then(|(s, _)| s).is_some()
            || self.store.read_work(work.as_str()).ok().flatten().is_some()
    }

    /// The genre the person reads most, by what is kept of their books; between two as common,
    /// the one of the series read most recently.
    fn genre(&self) -> String {
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
            .map_or_else(|| FALLBACK_GENRE.to_owned(), |(genre, _)| genre)
    }

    // -----------------------------------------------------------------------------------------
    // The view

    fn tile(&self, kept: &KeptBook) -> WorkTile {
        tiles::reading(&self.meta, &self.store, &self.sources, &kept.book)
    }

    fn shown(&self, book: &Book) -> bool {
        self.ask.adult || !book.adult()
    }

    fn shelf(&self, key: &str, skip: &HashSet<String>) -> (Vec<WorkTile>, Option<String>) {
        let Some(shelf) = Shelf::of(&self.store, key) else { return (Vec::new(), None) };
        let tiles = self
            .meta
            .kept_books(&shelf.ids())
            .iter()
            .filter(|k| self.shown(&k.book))
            .map(|k| tiles::plain(&self.meta, &k.book))
            .filter(|t| !skip.contains(t.work.as_str()))
            .collect();
        (tiles, shelf.name)
    }

    fn view(&self) -> ReadingHomeView {
        let (pending, failed, offline_since) = {
            let memory = self.memory();
            let mut pending: Vec<ReadHomePart> = memory.parts.iter().copied().collect();
            pending.sort_by_key(|p| *p as u8);
            (pending, memory.failed, memory.offline_since)
        };
        let continuing = self.continuing();
        let continue_tiles: Vec<WorkTile> =
            tiles::kept_of(&self.meta, &continuing).iter().map(|k| self.tile(k)).collect();
        let following = self.following();
        let followed = tiles::kept_of(&self.meta, &following);
        let week = self.week(&followed);
        let mut seen: HashSet<String> = HashSet::new();
        seen.extend(self.yours());
        let up_next = self.up_next(&continuing, &seen);
        // The shelves leave out what is the person's own; one may repeat another's.
        let (releasing, _) = self.shelf(RELEASING, &seen);
        let (because, because_work) = self.shelf(BECAUSE, &seen);
        let because_title = because_work
            .as_deref()
            .filter(|_| !because.is_empty())
            .and_then(tiles::anilist)
            .and_then(|id| self.meta.kept_book(id))
            .map(|k| self.meta.titles().pick(&k.book.title).to_owned());
        let (genre_shelf, genre) = self.shelf(GENRE, &seen);
        let spot = self.spotlight(&followed, &continue_tiles, &up_next, &releasing);
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
            genre: genre.filter(|_| !genre_shelf.is_empty()),
            genre_shelf,
            no_sources: self.no_sources,
            spot: spot.map(|(_, spot)| spot),
            week,
            pending,
            offline,
            offline_since: offline.then_some(offline_since).flatten(),
            failed,
            end,
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
            let chapter = out_at.and(newest_chapter).map(|c| {
                let number = c.number.clone().unwrap_or_else(|| c.label.clone());
                Msg::Reading(match c.own_title() {
                    Some(title) => ReadingMsg::ReadingChapterTitled {
                        chapter: number,
                        title: title.to_owned(),
                    },
                    None => ReadingMsg::ReadingNextChapter { chapter: number },
                })
            });
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
        releasing: &[WorkTile],
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
            let line = match (next.own_title(), next.out_at().is_some_and(|at| at >= today)) {
                (Some(title), true) => ReadingMsg::ReadingOutTodayTitled {
                    chapter: number,
                    title: title.to_owned(),
                },
                (None, true) => ReadingMsg::ReadingOutToday { chapter: number },
                (_, false) => ReadingMsg::ReadingChapterNew { chapter: number },
            };
            let action = SeriesAction::Read {
                chapter: ChapterId::new(next.id.clone()),
                label: next.number.clone().unwrap_or_else(|| next.label.clone()),
            };
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
        let popular = releasing
            .iter()
            .filter_map(|t| tiles::anilist(t.work.as_str()))
            .collect::<Vec<_>>();
        let best = self
            .meta
            .kept_books(&popular)
            .into_iter()
            .max_by_key(|k| k.book.popularity.unwrap_or_default())?;
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
        let spot = ReadSpotlight {
            case,
            label: Msg::Reading(label),
            facts: tiles::facts(&kept.book),
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
