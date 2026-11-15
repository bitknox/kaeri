//! Private watching and reading.
//!
//! Private mode lasts until it is turned off or the app quits, on this device only: it is never
//! written to the config, the store or a sync payload, and a new launch starts with it off. While
//! it is on, and for a title kept always private on this device, what Kaeri records by itself is
//! held: the history and read history, tracker progress, track memory, skip-time votes, recent
//! searches, Handoff, presence, downloads ahead, and the widget and Watch Next feeds. Edits made
//! by hand go through. Where an episode or chapter was left is kept in memory instead, read by
//! the title and series pages and by the next play as if it were stored, until the app quits.
//! Places kept in memory are never written, even after the mode is turned off.
//!
//! Switching applies at once, both ways: every gate reads [`Privacy`] when it would write, never
//! a copy taken when a playback or a book opened.
//!
//! Logs leave out titles, addresses and search words while private and for always-private titles:
//! see [`redact`].

use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicBool, Ordering},
    },
};

use kaeri_core::{books::WorkId, ids::AnilistId};
use kaeri_model::{
    event::Event,
    private::{PrivateMode, PrivateTitle},
};
use kaeri_store::{Store, WatchEvent, reading::ReadEvent};

/// The engine's private state: the mode, the titles kept always private, and the places held in
/// memory. Shared by every task through [`crate::services::Services`].
#[derive(Default)]
pub struct Privacy {
    on: AtomicBool,
    always: RwLock<HashSet<PrivateTitle>>,
    places: Mutex<Places>,
    /// Episode downloads asked for while private, by row: their notices name no title.
    downloads: Mutex<HashSet<i64>>,
}

impl std::fmt::Debug for Privacy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // What is kept private is not for a log either.
        f.debug_struct("Privacy").field("on", &self.is_on()).finish_non_exhaustive()
    }
}

/// Where private plays and reads were left, until the app quits.
#[derive(Default)]
struct Places {
    shows: HashMap<AnilistId, WatchEvent>,
    works: HashMap<String, ReadEvent>,
}

impl Privacy {
    /// Whether private mode is on.
    pub fn is_on(&self) -> bool {
        self.on.load(Ordering::SeqCst)
    }

    /// Turn private mode on or off. Returns whether that changed it.
    pub fn set(&self, on: bool) -> bool {
        let was = self.on.swap(on, Ordering::SeqCst);
        redact::session(on);
        was != on
    }

    /// Read the titles kept always private from `store`: at launch, and when another profile
    /// takes over.
    pub fn load(&self, store: &Store) {
        let keys = store.private_titles().unwrap_or_else(|error| {
            tracing::warn!(%error, "could not read the titles kept private");
            Vec::new()
        });
        let titles: HashSet<_> =
            keys.iter().filter_map(|k| PrivateTitle::from_key(k)).collect();
        for title in &titles {
            redact::title_id(title);
        }
        *self.always.write().unwrap_or_else(|e| e.into_inner()) = titles;
    }

    /// Keep `title` always private, or not, in `store` and here.
    pub fn set_always(
        &self,
        store: &Store,
        title: &PrivateTitle,
        on: bool,
    ) -> kaeri_store::Result<()> {
        store.set_private_title(&title.key(), on, kaeri_store::now())?;
        let mut always = self.always.write().unwrap_or_else(|e| e.into_inner());
        if on {
            redact::title_id(title);
            always.insert(title.clone());
        } else {
            always.remove(title);
        }
        Ok(())
    }

    /// Whether `title` is kept always private.
    pub fn is_always(&self, title: &PrivateTitle) -> bool {
        self.always.read().unwrap_or_else(|e| e.into_inner()).contains(title)
    }

    pub fn show_always(&self, id: AnilistId) -> bool {
        self.is_always(&PrivateTitle::Show { id })
    }

    pub fn series_always(&self, work: &str) -> bool {
        self.is_always(&PrivateTitle::Series { work: WorkId::new(work) })
    }

    /// Whether what is watched of show `id` is held now.
    pub fn holds_show(&self, id: AnilistId) -> bool {
        self.is_on() || self.show_always(id)
    }

    /// Whether what is read of `work` is held now.
    pub fn holds_series(&self, work: &str) -> bool {
        self.is_on() || self.series_always(work)
    }

    /// What a view of show `id` says of it now.
    pub fn show_mode(&self, id: AnilistId) -> Option<PrivateMode> {
        mode(self.show_always(id), self.is_on())
    }

    /// What a view of `work` says of it now.
    pub fn series_mode(&self, work: &str) -> Option<PrivateMode> {
        mode(self.series_always(work), self.is_on())
    }

    /// Hold where a private play left an episode, in place of the history row it would write.
    pub fn keep_watch(&self, watch: WatchEvent) {
        self.places().shows.insert(watch.anilist_id, watch);
    }

    /// Where a private play last left show `id`, when one did since launch.
    pub fn watch_kept(&self, id: AnilistId) -> Option<WatchEvent> {
        self.places().shows.get(&id).cloned()
    }

    /// Where `episode` of `id` resumes from memory: the place a private play left it, when it
    /// was the last episode one played and it was not finished.
    pub fn resume_kept(&self, id: AnilistId, episode: &str) -> Option<f64> {
        let places = self.places();
        let kept = places.shows.get(&id)?;
        (kept.episode.trim() == episode.trim() && !kept.completed).then_some(kept.position_secs)
    }

    /// Hold where a private read left a chapter, in place of the read row it would write.
    pub fn keep_read(&self, read: ReadEvent) {
        self.places().works.insert(read.work.clone(), read);
    }

    /// Where a private read last left `work`, when one did since launch.
    pub fn read_kept(&self, work: &str) -> Option<ReadEvent> {
        self.places().works.get(work).cloned()
    }

    /// An episode download asked for while private: its notices name no title while it runs.
    pub fn note_download(&self, row: i64) {
        self.downloads.lock().unwrap_or_else(|e| e.into_inner()).insert(row);
    }

    /// Whether download `row` of show `id` is private: asked for while private, or of an
    /// always-private show.
    pub fn download_private(&self, row: i64, id: AnilistId) -> bool {
        self.show_always(id)
            || self.downloads.lock().unwrap_or_else(|e| e.into_inner()).contains(&row)
    }

    /// A gate for one playback of show `id`.
    pub fn gate(self: &Arc<Self>, id: AnilistId) -> Gate {
        Gate(Some((Arc::clone(self), id)))
    }

    /// Put what is held in memory, and the private state, in an event on its way to the front
    /// end: a title's standing, a page, a download row, a staged play.
    pub fn overlay(&self, event: &mut Event) {
        match event {
            Event::Progress { id, progress, resume, private, kept, .. } => {
                *private = self.show_mode(*id);
                if let Some(watch) = self.watch_kept(*id) {
                    *kept = overlay_standing(&watch, progress, resume);
                }
                if private.is_some() {
                    redact::show_id(*id);
                }
            }
            Event::Detail(entry) => {
                if let Some(watch) = self.watch_kept(entry.id) {
                    overlay_standing(&watch, &mut entry.progress, &mut entry.resume);
                }
                if self.holds_show(entry.id) {
                    redact::show_id(entry.id);
                    redact::word(&entry.title);
                    if let Some(secondary) = &entry.secondary {
                        redact::word(secondary);
                    }
                }
            }
            Event::PlaybackStaged { id, title, always_private, .. } => {
                *always_private = self.show_always(*id);
                if self.holds_show(*id) {
                    redact::show_id(*id);
                    redact::word(title);
                }
            }
            Event::ShowSettings(view) => view.always_private = self.show_always(view.id),
            Event::Downloads(rows) => {
                for row in rows {
                    row.private = self.download_private(row.id, row.anilist_id);
                }
            }
            // No notice ever names an always-private title: not on the lock screen, not ahead.
            Event::NewEpisodes(episodes) => episodes.retain(|e| !self.show_always(e.id)),
            Event::UpcomingNotices(notices) => {
                notices.airings.retain(|a| !self.show_always(a.id));
            }
            Event::Reading(reading) => match reading {
                kaeri_model::reading::ReadingEvent::NewChapters(chapters) => {
                    chapters.retain(|c| !self.series_always(c.work.as_str()));
                }
                kaeri_model::reading::ReadingEvent::Series(view) => {
                    view.private = self.series_mode(view.work.as_str());
                    if view.private.is_some() {
                        redact::work_id(view.work.as_str());
                        let titles = &view.titles;
                        for title in [&titles.romaji, &titles.english, &titles.native] {
                            title.iter().for_each(|t| redact::word(t));
                        }
                        redact::word(&titles.display);
                    }
                }
                _ => {}
            },
            // The widgets show no always-private title: they sit on the home screen.
            Event::WidgetSnapshot(data) => {
                data.continue_watching.retain(|c| !self.show_always(c.id));
                data.airing_today.retain(|a| !self.show_always(a.id));
                data.next_days.retain(|a| !self.show_always(a.id));
                if data.next_airing.as_ref().is_some_and(|n| self.show_always(n.id)) {
                    data.next_airing = None;
                }
            }
            _ => {}
        }
    }

    fn places(&self) -> std::sync::MutexGuard<'_, Places> {
        self.places.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn mode(always: bool, on: bool) -> Option<PrivateMode> {
    if always {
        Some(PrivateMode::Always)
    } else if on {
        Some(PrivateMode::Session)
    } else {
        None
    }
}

/// Move a title's standing to the place a private play left it: the episode it was in and where,
/// or the one after a finished one. Returns whether it moved. The episodes' own watched state
/// stays as it was saved.
fn overlay_standing(
    watch: &WatchEvent,
    progress: &mut Option<kaeri_model::progress::TitleProgress>,
    resume: &mut Option<kaeri_model::catalog::ResumePoint>,
) -> bool {
    let Ok(number) = watch.episode.trim().parse::<u32>() else { return false };
    let done = progress.map_or(0, |p| p.done);
    if watch.completed {
        *progress = Some(kaeri_model::progress::TitleProgress { done, next: number + 1 });
        *resume = None;
    } else {
        *progress = Some(kaeri_model::progress::TitleProgress { done, next: number });
        *resume = Some(kaeri_model::catalog::ResumePoint {
            position: watch.position_secs,
            fraction: watch
                .duration_secs
                .filter(|d| *d > 0.0)
                .map(|d| (watch.position_secs / d).clamp(0.0, 1.0)),
        });
    }
    true
}

/// Before a command runs while private mode is on: the words it searches for are left out of the
/// log from now on, the job that names them included.
pub fn note_command(privacy: &Privacy, command: &kaeri_model::command::Command) {
    use kaeri_model::{command::Command, reading::ReadingCommand};
    if !privacy.is_on() {
        return;
    }
    match command {
        Command::Search { query }
        | Command::SearchLive { query }
        | Command::SearchMore { query, .. } => {
            redact::word(query.trimmed_text());
        }
        Command::ManualSearch { query, .. } => redact::word(query),
        Command::Reading(
            ReadingCommand::Search { query }
            | ReadingCommand::SearchMore { query, .. }
            | ReadingCommand::RetrySourceSearch { query, .. },
        ) => redact::word(query.text.trim()),
        _ => {}
    }
}

/// [`Command::SetPrivate`](kaeri_model::command::Command::SetPrivate): the mode on or off, at
/// once, said back with [`Event::Private`]. Turned on, Handoff stops offering what plays and the
/// widgets are no longer fed; turned off, what plays from then on is recorded again.
pub fn set(
    on: bool,
    engine: &mut crate::state::EngineState,
    services: &crate::services::Services,
    tx: &crate::outbox::Outbox,
) {
    if services.privacy.set(on) {
        tracing::info!(on, "private mode changed");
    }
    let _ = tx.send(Event::Private(on));
    // The open title's line and main button, as they stand now.
    if let Some(progress) = engine.cache.table().and_then(|(id, _)| engine.cache.progress(id)) {
        let _ = tx.send(progress);
    }
}

/// [`Command::SetAlwaysPrivate`](kaeri_model::command::Command::SetAlwaysPrivate): a title kept
/// always private on this device, or not, said back with [`Event::AlwaysPrivate`] and, for a
/// show, its standing again.
pub fn set_always(
    title: PrivateTitle,
    on: bool,
    engine: &mut crate::state::EngineState,
    services: &crate::services::Services,
    tx: &crate::outbox::Outbox,
) {
    if let Err(error) = services.privacy.set_always(&services.store, &title, on) {
        tx.not_kept("always private", &error);
        return;
    }
    if let PrivateTitle::Show { id } = &title
        && let Some(progress) = engine.cache.progress(*id)
    {
        let _ = tx.send(progress);
    }
    let _ = tx.send(Event::AlwaysPrivate { title, on });
}

/// One playback's view of [`Privacy`]: whether what it would write of its show is held now.
/// The default holds nothing, for a play with no engine behind it.
#[derive(Clone, Default)]
pub struct Gate(Option<(Arc<Privacy>, AnilistId)>);

impl Gate {
    /// Whether the playback's writes are held now.
    pub fn holds(&self) -> bool {
        self.0.as_ref().is_some_and(|(privacy, id)| privacy.holds_show(*id))
    }

    /// Hold `watch` in memory, when the playback is private now. Returns whether it was held.
    pub fn keep(&self, watch: &WatchEvent) -> bool {
        match &self.0 {
            Some((privacy, id)) if privacy.holds_show(*id) => {
                privacy.keep_watch(watch.clone());
                true
            }
            _ => false,
        }
    }
}

impl std::fmt::Debug for Gate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Gate").field(&self.holds()).finish()
    }
}

/// Logs leave out titles, addresses and search words while private and for always-private titles.
///
/// The log is the process's, so this is too: every engine in it adds the words it holds back.
/// [`redact::Redacting`] wraps the log's writer and puts each line through [`line`]: the words learnt here
/// are taken out at any time, and while the mode is on, or a private play or read is open, every
/// address too.
pub mod redact {
    use std::{
        io::Write,
        sync::{
            RwLock,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
    };

    use kaeri_core::ids::AnilistId;
    use kaeri_model::private::PrivateTitle;

    /// What a word or an address is replaced by.
    pub const HIDDEN: &str = "[private]";

    /// The log's own [`Redactor`]: the process has one log, so every engine in it adds here.
    static LOG: Redactor = Redactor::new();

    /// Private mode turned on or off.
    pub fn session(on: bool) {
        LOG.session(on);
    }

    /// Whether addresses are left out of the log now.
    pub fn active() -> bool {
        LOG.active()
    }

    /// A private play or read is open: addresses are left out until the guard goes.
    pub fn open() -> Open<'static> {
        LOG.open()
    }

    /// Leave `word` out of the log from now on: a title, or a search.
    pub fn word(word: &str) {
        LOG.word(word);
    }

    /// Leave a private show's id out of the log.
    pub fn show_id(id: AnilistId) {
        LOG.show_id(id);
    }

    /// Leave an always-private title's id out of the log.
    pub fn title_id(title: &PrivateTitle) {
        LOG.title_id(title);
    }

    /// Leave a private series' key out of the log.
    pub fn work_id(work: &str) {
        LOG.work_id(work);
    }

    /// `text` as the log writes it. See [`Redactor::line`].
    pub fn line(text: &str) -> String {
        LOG.line(text)
    }

    /// The words and ids a log leaves out, and whether it leaves out addresses now.
    #[derive(Debug)]
    pub struct Redactor {
        words: RwLock<Vec<String>>,
        ids: RwLock<Vec<String>>,
        session: AtomicBool,
        open: AtomicUsize,
    }

    impl Default for Redactor {
        fn default() -> Self {
            Self::new()
        }
    }

    /// Held while a private play or read is open. See [`open`].
    #[derive(Debug)]
    pub struct Open<'a>(&'a Redactor);

    impl Drop for Open<'_> {
        fn drop(&mut self) {
            self.0.open.fetch_sub(1, Ordering::SeqCst);
        }
    }

    impl Redactor {
        pub const fn new() -> Self {
            Self {
                words: RwLock::new(Vec::new()),
                ids: RwLock::new(Vec::new()),
                session: AtomicBool::new(false),
                open: AtomicUsize::new(0),
            }
        }

        pub fn session(&self, on: bool) {
            self.session.store(on, Ordering::SeqCst);
        }

        pub fn active(&self) -> bool {
            self.session.load(Ordering::SeqCst) || self.open.load(Ordering::SeqCst) > 0
        }

        pub fn open(&self) -> Open<'_> {
            self.open.fetch_add(1, Ordering::SeqCst);
            Open(self)
        }

        /// Words shorter than three characters say nothing on their own and are kept.
        pub fn word(&self, word: &str) {
            let word = word.trim();
            if word.chars().count() < 3 {
                return;
            }
            let lower = word.to_lowercase();
            let mut words = self.words.write().unwrap_or_else(|e| e.into_inner());
            if !words.contains(&lower) {
                words.push(lower);
                // Longest first, so a title is taken out whole before a word inside it.
                words.sort_by_key(|w| std::cmp::Reverse(w.len()));
            }
        }

        pub fn show_id(&self, id: AnilistId) {
            self.number(&id.get().to_string());
        }

        pub fn title_id(&self, title: &PrivateTitle) {
            match title {
                PrivateTitle::Show { id } => self.show_id(*id),
                PrivateTitle::Series { work } => self.work_id(work.as_str()),
            }
        }

        /// `a:30001` and its number, or `l:<hex>` and its hex.
        pub fn work_id(&self, work: &str) {
            if let Some((_, id)) = work.split_once(':') {
                self.number(id);
            }
            self.number(work);
        }

        fn number(&self, id: &str) {
            if id.is_empty() {
                return;
            }
            let mut ids = self.ids.write().unwrap_or_else(|e| e.into_inner());
            if !ids.iter().any(|i| i == id) {
                ids.push(id.to_owned());
                ids.sort_by_key(|i| std::cmp::Reverse(i.len()));
            }
        }

        /// `text` with the words and ids learnt taken out, and while [`Self::active`], every
        /// address.
        pub fn line(&self, text: &str) -> String {
            let mut out = text.to_owned();
            if self.active() {
                out = addresses(&out);
            }
            let words = self.words.read().unwrap_or_else(|e| e.into_inner());
            for word in words.iter() {
                out = replace_ignoring_case(&out, word);
            }
            drop(words);
            let ids = self.ids.read().unwrap_or_else(|e| e.into_inner());
            for id in ids.iter() {
                out = replace_token(&out, id);
            }
            out
        }
    }

    /// Every `scheme://…` in `text`, up to the next space or quote, as [`HIDDEN`].
    fn addresses(text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(at) = rest.find("://") {
            // The scheme: the letters before `://`.
            let head = &rest[..at];
            let start = head
                .char_indices()
                .rev()
                .take_while(|(_, c)| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))
                .last()
                .map_or(at, |(i, _)| i);
            out.push_str(&rest[..start]);
            let tail = &rest[at + 3..];
            let end = tail
                .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>'))
                .unwrap_or(tail.len());
            out.push_str(HIDDEN);
            rest = &tail[end..];
        }
        out.push_str(rest);
        // A magnet names the release.
        if out.contains("magnet:?") {
            out = out
                .split(' ')
                .map(|w| if w.contains("magnet:?") { HIDDEN } else { w })
                .collect::<Vec<_>>()
                .join(" ");
        }
        out
    }

    fn replace_ignoring_case(text: &str, lower_word: &str) -> String {
        let lower = text.to_lowercase();
        // Lowercasing can change byte lengths outside ASCII; then only exact matches are taken.
        if lower.len() != text.len() {
            return text.replace(lower_word, HIDDEN);
        }
        let mut out = String::with_capacity(text.len());
        let mut from = 0;
        while let Some(at) = lower[from..].find(lower_word) {
            let at = from + at;
            out.push_str(&text[from..at]);
            out.push_str(HIDDEN);
            from = at + lower_word.len();
        }
        out.push_str(&text[from..]);
        out
    }

    /// `id` taken out wherever it stands alone, not inside a longer number or word.
    fn replace_token(text: &str, id: &str) -> String {
        let bytes = text.as_bytes();
        let mut out = String::with_capacity(text.len());
        let mut from = 0;
        let mut search = 0;
        while let Some(at) = text[search..].find(id) {
            let at = search + at;
            let end = at + id.len();
            let before = at.checked_sub(1).map(|i| bytes[i]);
            let after = bytes.get(end).copied();
            let alone = |b: Option<u8>| b.is_none_or(|b| !b.is_ascii_alphanumeric());
            if alone(before) && alone(after) {
                out.push_str(&text[from..at]);
                out.push_str(HIDDEN);
                from = end;
            }
            search = end;
        }
        out.push_str(&text[from..]);
        out
    }

    /// A log writer that puts each line through [`line`] on its way.
    pub struct Redacting<W>(pub W);

    impl<W: Write> Write for Redacting<W> {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let text = String::from_utf8_lossy(buf);
            self.0.write_all(line(&text).as_bytes())?;
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            self.0.flush()
        }
    }
}

#[cfg(test)]
mod tests;
