//! The calendar: one timeline of what recently aired and what is coming, the pages further back
//! that a scroll toward its top asks for, and the pages further ahead that a step forward asks
//! for.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{Arc, Mutex},
};

use kaeri_core::ids::AnilistId;
use kaeri_meta::{
    Meta,
    anilist::{AiringEntry, AiringPage},
};
use kaeri_model::{
    catalog::{AiringStatus, ContentFor, ContentList, Entry},
    event::Event,
};
use kaeri_providers::ProviderRegistry;
use kaeri_store::Store;

use crate::{
    clock::Clock,
    data,
    dispatch::Lent,
    now_epoch,
    titles::dub::{Dubs, route},
};

/// One page of broadcasts as AniList answered it.
#[derive(Debug, Clone)]
struct Block {
    /// In the order they were fetched: newest first for the past, oldest first for the future.
    items: Vec<AiringEntry>,
    has_next: bool,
}

/// The pages the calendar has loaded, numbered against one anchor.
#[derive(Debug, Clone, Default)]
pub struct Window {
    anchor: i64,
    past: BTreeMap<u32, Block>,
    ahead: BTreeMap<u32, Block>,
}

impl Window {
    /// How many pages from the first are held without a gap, and whether AniList has more
    /// behind them. With none held there is more to find.
    fn run(pages: &BTreeMap<u32, Block>) -> (u32, bool) {
        let mut count = 0;
        let mut more = true;
        for (page, block) in pages {
            if *page != count + 1 {
                break;
            }
            count = *page;
            more = block.has_next;
        }
        (count, more)
    }

    /// The window the front end is told of.
    fn event(&self) -> Event {
        let (pages, more_past) = Self::run(&self.past);
        let (ahead_pages, more_ahead) = Self::run(&self.ahead);
        Event::CalendarWindow {
            anchor: self.anchor,
            pages,
            more_past,
            floor: self.anchor - CALENDAR_MAX_PAST,
            ceiling: self.anchor + CALENDAR_CEILING,
            ahead_pages,
            more_ahead,
        }
    }

    /// Every broadcast held, in time order: the past pages with the oldest first, then the
    /// future.
    fn broadcasts(&self) -> Vec<&AiringEntry> {
        let (pages, _) = Self::run(&self.past);
        let (ahead_pages, _) = Self::run(&self.ahead);
        let past = self.past.range(..=pages).rev().flat_map(|(_, b)| b.items.iter().rev());
        let ahead = self.ahead.range(..=ahead_pages).flat_map(|(_, b)| b.items.iter());
        past.chain(ahead).collect()
    }
}

/// The one schedule of broadcasts the Calendar, the widgets, the notifications' week and the
/// airing digest read: the last window, kept for [`REMEMBERED_FOR`], so a return to the calendar
/// asks AniList nothing and the widgets ask only for the pages the calendar has not. Cheap to
/// clone; every clone is the same memory.
///
/// A page is fetched once however many ask for it at the same time, and a page that arrived is
/// kept when the pages after it fail, so a week cut short still has what it had.
#[derive(Clone, Default)]
pub struct CalendarMemory {
    inner: Arc<Mutex<Option<Remembered>>>,
    /// Held while a page of the past, and of the days ahead, is asked for, so two readers
    /// wanting the same page ask once. One for each side, so the two go out side by side.
    gates: Arc<[tokio::sync::Mutex<()>; 2]>,
}

struct Remembered {
    /// The engine's clock when the window was fetched.
    at: i64,
    window: Window,
}

/// What a reader of the schedule needs held, from the anchor outward.
#[derive(Debug, Clone, Copy, Default)]
pub struct Need {
    /// Pages of the past held without a gap, at least.
    pub past_pages: u32,
    /// And the past held back to this instant, when the schedule reaches it.
    pub past_to: Option<i64>,
    /// Pages of the days ahead held without a gap, at least.
    pub ahead_pages: u32,
    /// And the days ahead held on to this instant, when the schedule reaches it.
    pub ahead_to: Option<i64>,
}

/// The schedule as read: the window, and why it falls short of what was needed, if it does.
#[derive(Debug, Clone)]
pub struct Covered {
    pub window: Window,
    pub failure: Option<String>,
}

impl Window {
    /// The next page of the past this window still needs for `need`, if any.
    fn next_past(&self, need: &Need) -> Option<u32> {
        let (count, more) = Self::run(&self.past);
        let reached = |t: i64| {
            self.past.get(&count).and_then(|b| b.items.last()).is_some_and(|a| a.airing_at <= t)
        };
        let wanted =
            count < need.past_pages || need.past_to.is_some_and(|t| count == 0 || !reached(t));
        (wanted && more && count < CALENDAR_JOB_PAGES * 2).then_some(count + 1)
    }

    /// The next page of the days ahead this window still needs for `need`, if any.
    fn next_ahead(&self, need: &Need) -> Option<u32> {
        let (count, more) = Self::run(&self.ahead);
        let reached = |t: i64| {
            self.ahead
                .get(&count)
                .and_then(|b| b.items.last())
                .is_some_and(|a| a.airing_at >= t)
        };
        let wanted = count < need.ahead_pages
            || need.ahead_to.is_some_and(|t| count == 0 || !reached(t));
        (wanted && more && count < CALENDAR_JOB_PAGES * 2).then_some(count + 1)
    }

    /// The pages of one side this window still needs for `need`, in order: the next one, and
    /// those after it that are sure to be wanted too. A count of pages says how many; an instant
    /// to reach is reckoned from how far the pages held went each, so a page that turns out not
    /// to be needed is left unread. Empty when nothing more is needed.
    fn wanted(&self, need: &Need, ahead: bool) -> Vec<u32> {
        let next = if ahead { self.next_ahead(need) } else { self.next_past(need) };
        let Some(next) = next else { return Vec::new() };
        let pages = if ahead { &self.ahead } else { &self.past };
        let (count, _) = Self::run(pages);
        let counted = if ahead { need.ahead_pages } else { need.past_pages };
        let mut more = counted.saturating_sub(count).max(1);
        let to = if ahead { need.ahead_to } else { need.past_to };
        let reached = pages.get(&count).and_then(|b| b.items.last()).map(|a| a.airing_at);
        if let (Some(to), Some(reached)) = (to, reached)
            && count > 0
        {
            // How far a page went, on average, and how many such pages the rest would take.
            let (went, left) = if ahead {
                (reached - self.anchor, to - reached)
            } else {
                (self.anchor - reached, reached - to)
            };
            let each = went / i64::from(count);
            if each > 0 && left > 0 {
                let pages = u32::try_from((left + each - 1) / each).unwrap_or(u32::MAX);
                more = more.max(pages);
            }
        }
        let last = (CALENDAR_JOB_PAGES * 2).min(next.saturating_add(more - 1));
        (next..=last).collect()
    }

    /// The broadcasts held, in time order, strictly between `from` and `to`.
    pub fn between(&self, from: i64, to: i64) -> Vec<AiringEntry> {
        self.broadcasts()
            .into_iter()
            .filter(|a| a.airing_at > from && a.airing_at < to)
            .cloned()
            .collect()
    }
}

impl CalendarMemory {
    fn lock(&self) -> std::sync::MutexGuard<'_, Option<Remembered>> {
        self.inner.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The window held, however old: what stands in while the schedule is asked for again.
    pub fn held(&self) -> Option<Window> {
        self.lock().as_ref().map(|k| k.window.clone())
    }

    /// Add pages a job fetched to the window they belong to, if it is still the one kept.
    fn add(&self, anchor: i64, ahead: bool, pages: &[(u32, Block)]) {
        let mut kept = self.lock();
        if let Some(kept) = kept.as_mut().filter(|k| k.window.anchor == anchor) {
            let into = if ahead { &mut kept.window.ahead } else { &mut kept.window.past };
            into.extend(pages.iter().cloned());
        }
    }

    /// The window to read from: the kept one while it is fresh, and otherwise a new one at
    /// `anchor`, which replaces the kept one at its first page. `renew` asks for a new one
    /// whatever the kept one's age.
    fn window_for(&self, at: i64, anchor: i64, renew: bool) -> (i64, Option<Window>) {
        let mut kept = self.lock();
        let fresh = kept
            .as_ref()
            .filter(|k| !renew && at - k.at < REMEMBERED_FOR && at >= k.at)
            .map(|k| k.window.anchor);
        match fresh {
            Some(anchor) => (anchor, None),
            None => (anchor, kept.take().map(|k| k.window)),
        }
    }

    /// Read the schedule with `need` held: the pages it lacks asked for one at a time, each
    /// once however many readers want it, and what is held handed back with the reason it
    /// falls short, if it does. Asked of AniList as background work when `low`: the widgets,
    /// the notices and the digest wait behind what is on screen.
    ///
    /// The walk goes on if the caller stops waiting, so a reader that gave up on a slow answer
    /// finds the pages held when it comes back.
    pub async fn cover(
        &self,
        meta: &Meta,
        at: i64,
        anchor: i64,
        need: Need,
        renew: bool,
        low: bool,
    ) -> Covered {
        let (anchor, stale) = self.window_for(at, anchor, renew);
        let me = self.clone();
        let meta = meta.clone();
        let walk = async move {
            // A window just begun is kept empty at once, for the other readers to join.
            {
                let mut kept = me.lock();
                if kept.as_ref().is_none_or(|k| k.window.anchor != anchor) {
                    *kept =
                        Some(Remembered { at, window: Window { anchor, ..Window::default() } });
                }
            }
            // The pages each side lacks go out together, two to a request: the first page of
            // the past and of the days ahead in one, and the pages a side is sure to need after
            // it in the next. A side whose gate another reader holds is not waited for while the
            // other has pages to ask.
            let mut errors: [Option<String>; 2] = [None, None];
            loop {
                let wanted = |ahead: bool| {
                    let kept = me.lock();
                    let window = kept.as_ref().filter(|k| k.window.anchor == anchor);
                    window.map(|k| k.window.wanted(&need, ahead)).unwrap_or_default()
                };
                let sides: Vec<bool> = [false, true]
                    .into_iter()
                    .filter(|ahead| errors[usize::from(*ahead)].is_none())
                    .filter(|ahead| !wanted(*ahead).is_empty())
                    .collect();
                let Some(first) = sides.first().copied() else { break };
                let mut gates = Vec::new();
                for ahead in &sides {
                    if let Ok(gate) = me.gates[usize::from(*ahead)].try_lock() {
                        gates.push((*ahead, gate));
                    }
                }
                if gates.is_empty() {
                    gates.push((first, me.gates[usize::from(first)].lock().await));
                }
                // Looked at again behind the gates: a page may have arrived.
                let mut asked: Vec<(bool, u32)> = Vec::new();
                let lists: Vec<(bool, Vec<u32>)> =
                    gates.iter().map(|(ahead, _)| (*ahead, wanted(*ahead))).collect();
                for n in 0..CALENDAR_PAGES_TOGETHER {
                    for (ahead, pages) in &lists {
                        if let Some(page) = pages.get(n) {
                            asked.push((*ahead, *page));
                        }
                    }
                }
                asked.truncate(CALENDAR_PAGES_TOGETHER);
                if asked.is_empty() {
                    continue;
                }
                let found = fetch_pages(&meta, anchor, &asked).await;
                for ahead in [false, true] {
                    // In page order, and only as far as the need reaches, as asked one by one.
                    for ((side, page), block) in asked.iter().zip(&found) {
                        if *side != ahead {
                            continue;
                        }
                        let next = {
                            let kept = me.lock();
                            let window = kept.as_ref().filter(|k| k.window.anchor == anchor);
                            window.and_then(|k| {
                                if ahead {
                                    k.window.next_ahead(&need)
                                } else {
                                    k.window.next_past(&need)
                                }
                            })
                        };
                        if next != Some(*page) {
                            break;
                        }
                        match block {
                            Ok(block) => me.add(anchor, ahead, &[(*page, block.clone())]),
                            Err(e) => {
                                errors[usize::from(ahead)] = Some(e.clone());
                                break;
                            }
                        }
                    }
                }
                drop(gates);
            }
            let [past, ahead] = errors;
            (past, ahead)
        };
        let walk =
            async move { if low { kaeri_meta::in_background(walk).await } else { walk.await } };
        let (past_error, ahead_error) = match tokio::spawn(walk).await {
            Ok(errors) => errors,
            Err(e) => (Some(e.to_string()), None),
        };
        let mut failure = past_error.or(ahead_error);
        let now_kept = self.held();
        let held = now_kept.as_ref().filter(|w| w.anchor == anchor).cloned();
        let window = match (held, stale) {
            // Nothing arrived of a new window: the last good one stands in for it.
            (Some(w), Some(stale))
                if failure.is_some()
                    && Window::run(&w.past).0 + Window::run(&w.ahead).0 == 0 =>
            {
                *self.lock() = Some(Remembered { at, window: stale.clone() });
                stale
            }
            (Some(w), _) => w,
            // Another reader began a newer window while this one was read (a Calendar
            // refresh): that one stands in, and the reader is told it may not be whole, so it
            // asks again rather than keeping an empty week as the answer.
            (None, stale) => {
                failure
                    .get_or_insert_with(|| "the schedule was renewed while it was read".into());
                now_kept.or(stale).unwrap_or_else(|| Window { anchor, ..Window::default() })
            }
        };
        Covered { window, failure }
    }
}

/// The calendar's first load: what recently aired and what is coming, with the window a
/// backfill or a step forward continues from. Answered from memory when the last load is
/// fresh.
pub async fn load_calendar(
    lent: Lent,
    dubs: Dubs,
    memory: CalendarMemory,
    clock: Clock,
    remembered: bool,
) {
    let Lent { meta, store, registry, tx, .. } = lent;
    let started = clock.now();
    let need = Need {
        past_pages: CALENDAR_INITIAL_PAST_PAGES,
        ahead_pages: CALENDAR_FUTURE_PAGES,
        ..Need::default()
    };
    // Shared with the widgets, the notices and the digest: only the pages they have not read.
    let Covered { window, failure } =
        memory.cover(&meta, started, now_epoch(), need, !remembered, false).await;
    let update = match timeline(&window, &store) {
        Some(mut entries) => {
            mark_dubbed(&mut entries, &dubs, &store, &registry);
            // Sent before the content, so the reducer knows how to continue the
            // list at the moment the list exists — a backfill triggered by the
            // very first keypress has everything it needs.
            let _ = tx.send(window.event());
            Event::Content {
                view: ContentFor::Calendar,
                content: ContentList::Entries(entries),
            }
        }
        None => {
            let error = failure.unwrap_or_else(|| "nothing airing in this window".into());
            Event::Content { view: ContentFor::Calendar, content: ContentList::Failed(error) }
        }
    };
    let _ = tx.send(update);
}

/// Pages further back in the calendar, for a scroll toward its top: `page`, and with `through`
/// the pages after it until a broadcast at or before that instant is in.
pub async fn load_calendar_past(
    lent: Lent,
    dubs: Dubs,
    memory: CalendarMemory,
    anchor: i64,
    page: u32,
    through: Option<i64>,
) {
    let Lent { meta, store, registry, tx, .. } = lent;
    let page = page.max(1);
    let last = if through.is_some() { page + CALENDAR_JOB_PAGES - 1 } else { page };
    let (blocks, failure) = walk(
        page,
        last,
        |page| fetch_past(&meta, anchor, page),
        // Newest first, so the last is the oldest.
        |b| through.is_some_and(|t| b.items.last().is_some_and(|a| a.airing_at <= t)),
    )
    .await;
    memory.add(anchor, false, &blocks);
    let update = match blocks.last() {
        Some((last, block)) => {
            let mut airings: Vec<&AiringEntry> = Vec::new();
            for (_, block) in blocks.iter().rev() {
                airings.extend(block.items.iter().rev());
            }
            let mut entries = data::entries_from_airings(airings, now_epoch(), &store);
            mark_dubbed(&mut entries, &dubs, &store, &registry);
            Event::CalendarBackfill {
                page: Some(*last),
                entries,
                more_past: failure.is_some() || block.has_next,
            }
        }
        // Not a toast. The user never asked for this page by name — they scrolled —
        // and the calendar in front of them is unchanged and still correct. The
        // reducer clears its in-flight flag so the next scroll tries again.
        None => {
            tracing::warn!(error = ?failure, page, "calendar backfill failed");
            Event::CalendarBackfill { page: None, entries: Vec::new(), more_past: true }
        }
    };
    let _ = tx.send(update);
}

/// Pages further ahead in the calendar: `page`, and with `through` the pages after it until a
/// broadcast at or after that instant is in.
pub async fn load_calendar_ahead(
    lent: Lent,
    dubs: Dubs,
    memory: CalendarMemory,
    anchor: i64,
    page: u32,
    through: Option<i64>,
) {
    let Lent { meta, store, registry, tx, .. } = lent;
    let page = page.max(1);
    let last = if through.is_some() { page + CALENDAR_JOB_PAGES - 1 } else { page };
    let (blocks, failure) = walk(
        page,
        last,
        |page| fetch_ahead(&meta, anchor, page),
        |b| through.is_some_and(|t| b.items.last().is_some_and(|a| a.airing_at >= t)),
    )
    .await;
    memory.add(anchor, true, &blocks);
    let update = match blocks.last() {
        Some((last, block)) => {
            let airings = blocks.iter().flat_map(|(_, b)| b.items.iter());
            let mut entries = data::entries_from_airings(airings, now_epoch(), &store);
            mark_dubbed(&mut entries, &dubs, &store, &registry);
            Event::CalendarAhead {
                page: Some(*last),
                entries,
                more_ahead: failure.is_some() || block.has_next,
            }
        }
        None => {
            tracing::warn!(error = ?failure, page, "calendar ahead failed");
            Event::CalendarAhead { page: None, entries: Vec::new(), more_ahead: true }
        }
    };
    let _ = tx.send(update);
}

/// Pages of the past (`false`) and the days ahead (`true`), each as [`fetch_past`] and
/// [`fetch_ahead`] ask for one, in one request: see [`Meta::airing_pages`].
async fn fetch_pages(
    meta: &Meta,
    anchor: i64,
    pages: &[(bool, u32)],
) -> Vec<Result<Block, String>> {
    let asked: Vec<AiringPage> = pages
        .iter()
        .map(|(ahead, page)| match ahead {
            true => AiringPage {
                from: anchor,
                to: anchor + CALENDAR_CEILING,
                page: *page,
                per_page: CALENDAR_PER_PAGE,
                newest_first: false,
            },
            false => AiringPage {
                from: anchor - CALENDAR_MAX_PAST,
                to: anchor,
                page: *page,
                per_page: CALENDAR_PER_PAGE,
                newest_first: true,
            },
        })
        .collect();
    meta.airing_pages(&asked)
        .await
        .into_iter()
        .map(|found| {
            let found = found.map_err(|e| e.to_string())?;
            Ok(Block { items: found.items, has_next: found.has_next })
        })
        .collect()
}

/// One page of past broadcasts, newest first.
///
/// The window's *upper* bound is the anchor and the sort is descending, so page numbering means
/// something stable for as long as the anchor holds: page three is the same fifty broadcasts
/// whether it is fetched with the first load or ten minutes later. That is what lets the backfill
/// ask for "the next page" without re-fetching what is on screen, and without a broadcast slipping
/// across a moving boundary and appearing twice. Re-reading the clock per page would break both.
async fn fetch_past(meta: &Meta, anchor: i64, page: u32) -> Result<Block, String> {
    let fetched = meta
        .airing_between_sorted(
            anchor - CALENDAR_MAX_PAST,
            anchor,
            page,
            CALENDAR_PER_PAGE,
            true,
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok(Block { items: fetched.items, has_next: fetched.has_next })
}

/// One page of future broadcasts, oldest first, from the anchor to the ceiling. Numbered
/// against a fixed upper bound for the same reason the past is.
async fn fetch_ahead(meta: &Meta, anchor: i64, page: u32) -> Result<Block, String> {
    let fetched = meta
        .airing_between_sorted(
            anchor,
            anchor + CALENDAR_CEILING,
            page,
            CALENDAR_PER_PAGE,
            false,
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok(Block { items: fetched.items, has_next: fetched.has_next })
}

/// Fetch pages `first` to `last`, stopping early where AniList has no more or `covered` says
/// enough has come. Returns the pages that arrived, in order, and the error that stopped the
/// walk, if one did. A page failing part-way still leaves a usable calendar: what arrived is
/// kept, and the front end asks for the failed page again.
async fn walk<F, Fut>(
    first: u32,
    last: u32,
    mut fetch: F,
    covered: impl Fn(&Block) -> bool,
) -> (Vec<(u32, Block)>, Option<String>)
where
    F: FnMut(u32) -> Fut,
    Fut: Future<Output = Result<Block, String>>,
{
    let mut blocks = Vec::new();
    for page in first..=last {
        match fetch(page).await {
            Ok(block) => {
                let done = !block.has_next || covered(&block);
                blocks.push((page, block));
                if done {
                    break;
                }
            }
            Err(e) => return (blocks, Some(e)),
        }
    }
    (blocks, None)
}

/// The window as one timeline of rows: what recently aired, then what is coming, with the
/// rows the calendar estimates among the week ahead. `None` when it holds no broadcast.
fn timeline(window: &Window, store: &Store) -> Option<Vec<Entry>> {
    let now = now_epoch();
    let mut entries = data::entries_from_airings(window.broadcasts(), now, store);
    if entries.is_empty() {
        return None;
    }
    let boundary = entries.iter().filter(|e| e.airing_in.is_some_and(|t| t <= 0)).count();
    let guessed = estimated(&entries, now, window.anchor + CALENDAR_FUTURE, |id, episode| {
        store.airing_time(id, episode).ok().flatten()
    });
    if !guessed.is_empty() {
        tracing::info!(count = guessed.len(), "calendar rows estimated");
        entries.extend(guessed);
        // Back into time order: the estimates belong among the week ahead.
        entries.sort_by_key(|entry| entry.airing_in.unwrap_or(i64::MAX));
    }
    tracing::info!(recent = boundary, upcoming = entries.len() - boundary, "calendar timeline");
    Some(entries)
}

/// Mark each row of `entries` whose episode is out in the dub, as far as `dubs` already knows,
/// asking nothing: see [`Entry::airing_dubbed`]. Only rows that have aired, of titles whose dub
/// some page found, and only as far as the sources a dubbed play would use list it now.
fn mark_dubbed(entries: &mut [Entry], dubs: &Dubs, store: &Store, registry: &ProviderRegistry) {
    let mut through: HashMap<AnilistId, Option<u32>> = HashMap::new();
    dubbed_rows(entries, |id| {
        *through.entry(id).or_insert_with(|| {
            dubs.knows(id)
                .then(|| dubs.aired_through(id, &route(store, registry, id)))
                .flatten()
        })
    });
}

/// Mark the aired rows of `entries` whose episode `through` says the title's dub has reached.
fn dubbed_rows(entries: &mut [Entry], mut through: impl FnMut(AnilistId) -> Option<u32>) {
    for entry in entries {
        let (Some(airing_in), Some(episode)) = (entry.airing_in, entry.airing_episode) else {
            continue;
        };
        if airing_in > 0 || entry.airing_approximate {
            continue;
        }
        entry.airing_dubbed = through(entry.id).is_some_and(|through| episode <= through);
    }
}

/// The rows the calendar estimates, each with "~" for its time: for a show still airing whose
/// schedule AniList has stopped giving, its next episode, a week on from the last.
///
/// AniList dates every broadcast it lists, and a calendar of only its rows loses a show the
/// moment its schedule lapses, which is the week its viewers look for it. So a show is given
/// one estimated row when all of these hold, from what the calendar already loaded and what
/// this device saw of the schedule before (`aired_at`), asking nothing more:
///
/// - AniList says it is releasing, and has no next broadcast for it.
/// - Its latest broadcast here has aired, and no row of it is still to come.
/// - That broadcast came a week after the one before it, within an hour: a weekly show, since
///   a sequel's first episodes can air half an hour apart and the next eight days later.
/// - The count AniList gives, if any, has episodes left.
/// - A week on from the last broadcast is still within the calendar's week ahead.
fn estimated(
    entries: &[Entry],
    now: i64,
    until: i64,
    aired_at: impl Fn(AnilistId, u32) -> Option<i64>,
) -> Vec<Entry> {
    // Each show's latest aired row, and the shows with a row still to come.
    let mut latest: HashMap<AnilistId, &Entry> = HashMap::new();
    let mut coming: HashSet<AnilistId> = HashSet::new();
    for entry in entries {
        let (Some(airing_in), Some(episode)) = (entry.airing_in, entry.airing_episode) else {
            continue;
        };
        if airing_in > 0 {
            coming.insert(entry.id);
        } else if latest.get(&entry.id).is_none_or(|e| e.airing_episode < Some(episode)) {
            latest.insert(entry.id, entry);
        }
    }
    let at = |entry: &Entry| entry.airing_in.map(|airing_in| now + airing_in);
    let mut guessed: Vec<Entry> = latest
        .into_values()
        .filter(|last| !coming.contains(&last.id))
        .filter(|last| {
            last.status == Some(AiringStatus::Releasing) && last.next_episode.is_none()
        })
        .filter_map(|last| {
            let episode = last.airing_episode?;
            if last.episodes.is_some_and(|total| episode >= total) || episode < 2 {
                return None;
            }
            let aired = at(last)?;
            let before = entries
                .iter()
                .find(|e| e.id == last.id && e.airing_episode == Some(episode - 1))
                .and_then(at)
                .or_else(|| aired_at(last.id, episode - 1))?;
            if (aired - before - WEEK).abs() > WEEKLY_WITHIN {
                return None;
            }
            let next = aired + WEEK;
            (next > now && next <= until).then(|| Entry {
                airing_episode: Some(episode + 1),
                airing_in: Some(next - now),
                delayed_from: None,
                airing_approximate: true,
                ..last.clone()
            })
        })
        .collect();
    guessed.sort_by_key(|entry| (entry.airing_in, entry.id.get()));
    guessed
}

/// A week, which an estimated row is a week on from its show's last broadcast.
const WEEK: i64 = 7 * 86_400;
/// How far from a week apart a show's last two broadcasts may be and still count as weekly:
/// AniList nudges a slot by minutes as a broadcaster pins it down.
const WEEKLY_WITHIN: i64 = 3_600;

/// The furthest back the calendar will ever reach, and so the floor of every past query.
///
/// It bounds the *backfill*, not the first load: because the sort is descending from the anchor,
/// page one is the fifty most recent broadcasts whatever this is set to. All it decides is when
/// scrolling up finally runs out. Three months is far enough back that reaching the end is a
/// journey rather than an accident.
const CALENDAR_MAX_PAST: i64 = 90 * 86_400;
/// How far ahead the calendar will ever reach, and so the ceiling of every future query: four
/// weeks from the anchor, fixed so that page numbers stay stable.
const CALENDAR_CEILING: i64 = 28 * 86_400;
/// How far forward the first load promises. A week covers a full broadcast cycle, so every
/// airing show you follow has exactly one upcoming episode in view. Estimated rows stay within
/// it.
const CALENDAR_FUTURE: i64 = 7 * 86_400;
/// Rows per request. AniList's ceiling: ask for 75 or 100 and the response says `perPage: 50`
/// and returns fifty. Depth is bought in pages, never here.
const CALENDAR_PER_PAGE: u32 = 50;
/// Pages of history fetched before the screen is first drawn. At roughly 127 broadcasts a week
/// three pages covers a full week and change, which is what the seven-day window always claimed
/// to offer. Scrolling past them backfills further without being asked.
const CALENDAR_INITIAL_PAST_PAGES: u32 = 3;
/// And forward, covering the week ahead by the same measure.
const CALENDAR_FUTURE_PAGES: u32 = 3;
/// The most pages one job walks to reach its `through`. About a week and a half of broadcasts
/// is three pages, so this is several weeks; a job that stops short says there is more.
const CALENDAR_JOB_PAGES: u32 = 12;
/// The most pages of the schedule asked in one request: a hundred broadcasts, each with its
/// title, which is what Search's start asks in its five rails of twenty, measured to answer.
const CALENDAR_PAGES_TOGETHER: usize = 2;
/// How long the last window is kept (seconds), so a return to the calendar asks AniList nothing.
const REMEMBERED_FOR: i64 = 600;

#[cfg(test)]
mod tests {
    use super::*;

    const SHOW: AnilistId = AnilistId::new(900_101);
    const NOW: i64 = 1_800_000_000;
    const HOUR: i64 = 3_600;
    const DAY: i64 = 86_400;
    const UNTIL: i64 = NOW + 7 * DAY;

    /// A row of `SHOW` about `episode`, airing `at` seconds from now, AniList calling the show
    /// releasing with no broadcast to come.
    fn row(episode: u32, at: i64) -> Entry {
        let mut entry = Entry::new(SHOW, "Invented Show");
        entry.airing_episode = Some(episode);
        entry.airing_in = Some(at);
        entry.status = Some(AiringStatus::Releasing);
        entry.episodes = Some(12);
        entry
    }

    fn never(_: AnilistId, _: u32) -> Option<i64> {
        None
    }

    #[test]
    fn a_weekly_show_with_no_schedule_left_gets_its_next_episode_a_week_on() {
        let rows = [row(4, -8 * DAY), row(5, -DAY)];
        let guessed = estimated(&rows, NOW, UNTIL, never);
        assert_eq!(guessed.len(), 1);
        let next = &guessed[0];
        assert_eq!(next.airing_episode, Some(6));
        assert_eq!(next.airing_in, Some(6 * DAY));
        assert!(next.airing_approximate);
        assert!(!rows[1].airing_approximate, "AniList's own rows are not estimates");
    }

    #[test]
    fn the_week_before_can_come_from_what_this_device_saw() {
        let rows = [row(5, -DAY)];
        let seen = |id: AnilistId, episode: u32| {
            (id == SHOW && episode == 4).then_some(NOW - 8 * DAY + 20 * 60)
        };
        assert_eq!(estimated(&rows, NOW, UNTIL, seen)[0].airing_episode, Some(6));
        assert!(estimated(&rows, NOW, UNTIL, never).is_empty(), "no cadence, no guess");
    }

    #[test]
    fn nothing_is_guessed_where_anilist_still_has_a_say() {
        // A row still to come.
        assert!(
            estimated(&[row(4, -8 * DAY), row(5, -DAY), row(6, 6 * DAY)], NOW, UNTIL, never)
                .is_empty()
        );
        // A next broadcast AniList has, past the week: a break.
        let mut on_break = row(5, -DAY);
        on_break.next_episode = Some(6);
        assert!(estimated(&[row(4, -8 * DAY), on_break], NOW, UNTIL, never).is_empty());
        // Finished, or at its last episode.
        let mut done = row(5, -DAY);
        done.status = Some(AiringStatus::Finished);
        assert!(estimated(&[row(4, -8 * DAY), done], NOW, UNTIL, never).is_empty());
        let mut last = row(12, -DAY);
        last.episodes = Some(12);
        assert!(estimated(&[row(11, -8 * DAY), last], NOW, UNTIL, never).is_empty());
    }

    #[test]
    fn broadcasts_not_a_week_apart_are_no_cadence() {
        // Half an hour apart, as a sequel's first two can be.
        assert!(
            estimated(&[row(4, -DAY - HOUR / 2), row(5, -DAY)], NOW, UNTIL, never).is_empty()
        );
        // Eight days apart.
        assert!(estimated(&[row(4, -9 * DAY), row(5, -DAY)], NOW, UNTIL, never).is_empty());
        // A nudge of minutes is still weekly.
        assert_eq!(
            estimated(&[row(4, -8 * DAY - 10 * 60), row(5, -DAY)], NOW, UNTIL, never).len(),
            1
        );
    }

    #[test]
    fn an_aired_row_is_marked_dubbed_as_far_as_the_dub_is_known_to_go() {
        let mut rows = [row(4, -8 * DAY), row(5, -DAY), row(6, 6 * DAY)];
        dubbed_rows(&mut rows, |id| (id == SHOW).then_some(4));
        let marked: Vec<bool> = rows.iter().map(|row| row.airing_dubbed).collect();
        assert_eq!(marked, [true, false, false], "to episode 4, and nothing yet to air");

        let mut rows = [row(4, -8 * DAY)];
        dubbed_rows(&mut rows, |_| None);
        assert!(!rows[0].airing_dubbed, "nothing known, nothing marked");
    }

    #[test]
    fn a_guess_past_the_week_ahead_or_already_due_is_left_out() {
        // Aired ten days ago: a week on is already past.
        assert!(
            estimated(&[row(4, -17 * DAY), row(5, -10 * DAY)], NOW, UNTIL, never).is_empty()
        );
        // The calendar's week ends sooner than a week on.
        assert!(
            estimated(&[row(4, -8 * DAY), row(5, -DAY)], NOW, NOW + 5 * DAY, never).is_empty()
        );
    }
}
