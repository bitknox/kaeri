//! A reading series' page in the terminal: its title, facts and synopsis, where it is read from
//! and which list keeps it, how far it is read, and its chapters newest first, one line each.
//!
//! The page is asked for with `LoadSeries`, which the engine answers with the header at once and
//! the chapters once a source does. The chapter under the cursor is what Enter reads, `u` marks
//! read or unread, `d` downloads; `a` picks the list the series is on.

use kaeri_core::books::{ChapterId, WorkId};
use kaeri_i18n::t;
use kaeri_model::{
    Msg, ReadingMsg,
    command::Command,
    reading::{
        BookEditionRow, ChapterDownloadState, ChapterReadState, ChapterRow, DownloadPick,
        LibrarySegmentRead, ReadingCommand, ReadingFailure, SeriesAction, SeriesView,
        TrackerMapping, TrackerReadRow, VolumeGroup, count_words, read_filters,
    },
};

use super::{App, alert, info, reading::Wait, say};
use crate::{
    keymap::Action,
    nav::{Overlay, StageView},
};

/// The lists `a` offers on the series page, in the order the apps list them.
pub const READ_LISTS: [LibrarySegmentRead; 6] = [
    LibrarySegmentRead::Reading,
    LibrarySegmentRead::Planning,
    LibrarySegmentRead::Completed,
    LibrarySegmentRead::Paused,
    LibrarySegmentRead::Dropped,
    LibrarySegmentRead::Rereading,
];

/// A reading list's name: "Reading", "Planning".
pub fn read_list_name(list: LibrarySegmentRead) -> String {
    match list {
        LibrarySegmentRead::Reading => t::read_list_reading(),
        LibrarySegmentRead::Planning => t::read_list_planning(),
        LibrarySegmentRead::Completed => t::read_list_completed(),
        LibrarySegmentRead::Paused => t::read_list_paused(),
        LibrarySegmentRead::Dropped => t::read_list_dropped(),
        LibrarySegmentRead::Rereading => t::read_list_rereading(),
        LibrarySegmentRead::YourFiles => t::read_list_your_files(),
    }
}

/// The series page as the engine has sent it so far.
#[derive(Debug, Clone)]
pub struct SeriesPage {
    pub work: WorkId,
    /// The header, once the engine sends it.
    pub view: Option<SeriesView>,
    /// The page asked for, so its loading line waits before it shows and then holds.
    pub wait: Wait,
    /// Why the page could not be had, while there is no header to show.
    pub failure: Option<ReadingFailure>,
    /// The chapters, newest first, once a source answers.
    pub chapters: Option<Vec<ChapterRow>>,
    /// The source the chapters are read from, as the chapters came.
    pub edition: Option<BookEditionRow>,
    /// Why the chapters could not be had, or not all of them: offline, the kept list with it.
    pub chapters_failure: Option<ReadingFailure>,
    /// The chapters asked for, so their placeholder and their line wait before they show.
    pub chapters_wait: Wait,
    /// Which trackers keep the series, for the list it is on.
    pub tracking: Vec<TrackerReadRow>,
    /// The cursor has been put on the chapter the page reads next.
    placed: bool,
}

impl SeriesPage {
    fn asked(work: WorkId, at: std::time::Instant) -> Self {
        Self {
            work,
            view: None,
            wait: Wait::asked(at),
            failure: None,
            chapters: None,
            edition: None,
            chapters_failure: None,
            chapters_wait: Wait::asked(at),
            tracking: Vec::new(),
            placed: false,
        }
    }

    /// The chapters as listed, newest first; none until they come.
    pub fn rows(&self) -> &[ChapterRow] {
        self.chapters.as_deref().unwrap_or_default()
    }

    /// Whether the computer is offline: the chapters came from what was kept, and only the
    /// downloaded ones open.
    pub fn offline(&self) -> bool {
        matches!(self.chapters_failure, Some(ReadingFailure::Offline))
            || matches!(self.failure, Some(ReadingFailure::Offline))
    }

    /// How many of the chapters listed are read, and how many there are.
    pub fn read_count(&self) -> (usize, usize) {
        let rows = self.rows();
        let read = rows.iter().filter(|row| row.state == ChapterReadState::Read).count();
        (read, rows.len())
    }

    /// The source the chapters are read from: as the chapters came, else as the header says.
    pub fn edition(&self) -> Option<&BookEditionRow> {
        self.edition.as_ref().or(self.view.as_ref().and_then(|view| view.edition.as_ref()))
    }

    /// The trackers that keep the series' list, by name, joined.
    pub fn trackers(&self) -> String {
        self.tracking
            .iter()
            .filter(|row| row.mapping == TrackerMapping::Mapped)
            .map(|row| tracker_name(&row.tracker))
            .collect::<Vec<_>>()
            .join(", ") // i18n-skip
    }

    /// Where a chapter row is in the list.
    pub(crate) fn position(&self, chapter: &ChapterId) -> Option<usize> {
        self.rows().iter().position(|row| row.id == *chapter)
    }
}

/// A tracker by the name the apps give it.
fn tracker_name(id: &str) -> String {
    match id {
        "mangaupdates" => "MangaUpdates".into(), // i18n-skip: a product name
        other => kaeri_model::present::tracker_name(other),
    }
}

/// The chapter the page's main action names, which the cursor starts on.
fn action_chapter(action: &SeriesAction) -> Option<&ChapterId> {
    match action {
        SeriesAction::Read { chapter, .. }
        | SeriesAction::Continue { chapter, .. }
        | SeriesAction::ContinuePart { chapter, .. }
        | SeriesAction::ReadPart { chapter, .. } => Some(chapter),
        SeriesAction::ReadAgain | SeriesAction::NoSource => None,
    }
}

/// The label is a chapter's number, "12", "10.5" or "12-13", which the words put "chapter" or
/// "part" before; anything else is the chapter's own title.
fn numbered(label: &str) -> bool {
    label.starts_with(|c: char| c.is_ascii_digit())
        && label.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-')
}

/// The page's main action in words, as the apps' button says it: "Continue chapter 14 · page 8",
/// "Read chapter 1", "Read again". None when no source has the series.
pub fn action_words(action: &SeriesAction) -> Option<String> {
    Some(match action {
        SeriesAction::Read { label, .. } if !numbered(label) => t::action_read_named(label),
        SeriesAction::Read { label, .. } => t::action_read_chapter(label),
        SeriesAction::Continue { label, page, .. } if !numbered(label) => {
            t::action_continue_named_page(label, i64::from(*page))
        }
        SeriesAction::Continue { label, page, .. } => {
            t::action_continue_chapter(label, i64::from(*page))
        }
        SeriesAction::ContinuePart { named: Some(named), percent: Some(percent), .. } => {
            t::action_continue_named(named, i64::from(*percent))
        }
        SeriesAction::ContinuePart { named: Some(named), percent: None, .. } => {
            t::action_continue_named_plain(named)
        }
        SeriesAction::ContinuePart { label, named: None, percent: Some(percent), .. } => {
            t::action_continue_part(label, i64::from(*percent))
        }
        SeriesAction::ContinuePart { label, named: None, percent: None, .. } => {
            t::action_continue_part_plain(label)
        }
        SeriesAction::ReadPart { named: Some(named), .. } => t::action_read_named(named),
        SeriesAction::ReadPart { label, named: None, .. } if !numbered(label) => {
            t::action_read_named(label)
        }
        SeriesAction::ReadPart { label, named: None, .. } => t::action_read_part(label),
        SeriesAction::ReadAgain => t::action_read_again(),
        SeriesAction::NoSource => return None,
    })
}

/// The facts under the title: the kind, where it is from, its status and count as the engine
/// words them, then when chapters come out. "Manga · Japan · Releasing · 17 chapters · Usually
/// Fridays". A series still releasing has no count from the engine; the chapters listed give it.
pub fn series_facts(view: &SeriesView, listed: usize) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut facts = view.facts.iter();
    parts.extend(facts.next().map(say));
    let origin = view.origin.as_deref().and_then(|code| {
        read_filters().origins.into_iter().find(|choice| choice.key == code).map(|c| c.label)
    });
    parts.extend(origin.as_ref().map(say));
    parts.extend(facts.map(say));
    let counted = view.facts.iter().any(|fact| {
        matches!(
            fact,
            Msg::Reading(
                ReadingMsg::ReadingChapters { .. } | ReadingMsg::ReadingVolumes { .. }
            )
        )
    });
    if !counted
        && listed > 0
        && let Some(count) = count_words(view.medium, Some(listed as u32), None)
    {
        parts.push(say(&count));
    }
    parts.extend(view.rhythm.as_ref().map(say));
    parts.join(" · ") // i18n-skip
}

/// A chapter's line: the engine's words, joined, or its state when the engine sent none.
pub fn chapter_line(row: &ChapterRow) -> String {
    if !row.line.is_empty() {
        return row.line.iter().map(say).collect::<Vec<_>>().join(" · "); // i18n-skip
    }
    let mut parts: Vec<String> = Vec::new();
    match row.state {
        ChapterReadState::Unread => {}
        ChapterReadState::InProgress { page, of: Some(of) } => {
            parts.push(t::chapter_state_page_of(i64::from(page), i64::from(of)));
        }
        ChapterReadState::InProgress { page, of: None } => {
            parts.push(t::chapter_state_page(i64::from(page)));
        }
        ChapterReadState::Read => parts.push(t::chapter_state_read()),
    }
    if downloaded(row) {
        parts.push(t::chapter_state_downloaded());
    }
    parts.join(" · ") // i18n-skip
}

/// A chapter's label with its title: "Chapter 14 · Tide Tables".
pub fn chapter_name(row: &ChapterRow) -> String {
    match row.title.as_deref().filter(|title| !title.is_empty()) {
        Some(title) => format!("{} · {title}", row.label), // i18n-skip
        None => row.label.clone(),
    }
}

/// Whether a chapter opens from this computer: downloaded, or a file in the person's folders.
pub fn downloaded(row: &ChapterRow) -> bool {
    row.on_device || matches!(row.download, Some(ChapterDownloadState::Done))
}

/// The newest first: the engine lists oldest first, volume by volume.
fn newest_first(volumes: Vec<VolumeGroup>) -> Vec<ChapterRow> {
    let mut rows: Vec<ChapterRow> =
        volumes.into_iter().flat_map(|volume| volume.chapters).collect();
    rows.reverse();
    rows
}

impl App {
    /// The series page on top, when it is.
    pub fn series_here(&self) -> Option<&SeriesPage> {
        match self.nav.current() {
            StageView::Series(work) => self.reading.series.as_ref().filter(|p| p.work == *work),
            _ => None,
        }
    }

    /// The series page on top is offline.
    pub fn series_offline(&self) -> bool {
        self.series_here().is_some_and(SeriesPage::offline)
    }

    /// The chapter under the cursor on the series page.
    pub fn series_selected(&self) -> Option<&ChapterRow> {
        self.series_here()?.rows().get(self.cursors.chapters.selected)
    }

    /// Open a series' page over the screen it was opened from, which keeps its cursor for Back.
    pub(super) fn open_series(&mut self, work: WorkId) -> Option<Command> {
        let now = self.clock.now_instant();
        self.reading.series = Some(SeriesPage::asked(work.clone(), now));
        self.cursors.chapters = Default::default();
        self.synopsis_expanded = false;
        self.nav.push(StageView::Series(work.clone()));
        self.commands
            .push(Command::Reading(ReadingCommand::LoadTracking { work: work.clone() }));
        Some(Command::Reading(ReadingCommand::LoadSeries { work }))
    }

    /// The header the engine sent for the page on screen.
    pub(super) fn take_series(&mut self, view: SeriesView) {
        let now = self.clock.now_instant();
        let Some(page) = self.reading.series.as_mut().filter(|p| p.work == view.work) else {
            return;
        };
        page.wait.land(now);
        page.failure = None;
        page.view = Some(view);
        self.place_chapter_cursor();
    }

    /// The chapters the engine sent for the page on screen, with why some or all could not be
    /// had. A list that comes again keeps the cursor on its chapter.
    pub(super) fn take_chapters(
        &mut self,
        work: &WorkId,
        edition: Option<BookEditionRow>,
        volumes: Vec<VolumeGroup>,
        failure: Option<ReadingFailure>,
    ) {
        let now = self.clock.now_instant();
        // The page's own cursor, read under the reader too when it is open over the page.
        let selected = (self.reading.series.as_ref())
            .filter(|p| p.work == *work)
            .and_then(|p| p.rows().get(self.cursors.chapters.selected))
            .map(|row| row.id.clone());
        let Some(page) = self.reading.series.as_mut().filter(|p| p.work == *work) else {
            return;
        };
        page.chapters_wait.land(now);
        page.chapters = Some(newest_first(volumes));
        if edition.is_some() {
            page.edition = edition;
        }
        page.chapters_failure = failure;
        let len = page.rows().len();
        let cursor = &mut self.cursors.chapters;
        match selected.and_then(|id| page.position(&id)) {
            Some(at) => cursor.selected = at,
            None => cursor.selected = cursor.selected.min(len.saturating_sub(1)),
        }
        self.place_chapter_cursor();
    }

    /// The trackers' rows for the page on screen.
    pub(super) fn take_tracking(&mut self, work: &WorkId, rows: Vec<TrackerReadRow>) {
        if let Some(page) = self.reading.series.as_mut().filter(|p| p.work == *work) {
            page.tracking = rows;
        }
    }

    /// The page, or its chapters, could not be had. While the page has no header it says why,
    /// and its chapters' failure shows in their place. A page that could not be had otherwise is
    /// a toast; chapters asked for off the page say nothing, as before.
    pub(super) fn series_failed(&mut self, failure: &ReadingFailure, chapters: bool) {
        if !self.series_took_failure(failure, chapters) && !chapters {
            self.push_toast(alert(say(&failure.msg())));
        }
    }

    /// `false` when no page on screen is waiting on it.
    fn series_took_failure(&mut self, failure: &ReadingFailure, chapters: bool) -> bool {
        let now = self.clock.now_instant();
        let Some(page) = self.reading.series.as_mut() else { return false };
        if !matches!(self.nav.current(), StageView::Series(work) if *work == page.work) {
            return false;
        }
        if chapters {
            page.chapters_wait.land(now);
            page.chapters_failure = Some(failure.clone());
            page.chapters.get_or_insert_with(Vec::new);
            return true;
        }
        if page.view.is_none() {
            page.wait.land(now);
            page.failure = Some(failure.clone());
            return true;
        }
        false
    }

    /// Once both the header and the chapters are in, the cursor goes to the chapter the page
    /// reads next, once: the one in progress, or the next unread.
    fn place_chapter_cursor(&mut self) {
        let Some(page) = self.reading.series.as_mut() else { return };
        if page.placed || page.chapters.is_none() {
            return;
        }
        let Some(view) = &page.view else { return };
        page.placed = true;
        if let Some(at) =
            action_chapter(&view.action).and_then(|chapter| page.position(chapter))
        {
            self.cursors.chapters.selected = at;
        }
    }

    /// A key on the series page, ahead of the shows' meaning of it. `None` lets it through.
    pub(super) fn series_key(&mut self, action: Action) -> Option<Option<Command>> {
        let page = self.series_here()?;
        let work = page.work.clone();
        Some(match action {
            Action::Open => {
                let chapter = self.series_selected().map(|row| row.id.clone())?;
                self.open_reader(work, Some(chapter))
            }
            Action::ToggleWatched => self.mark_selected(work),
            Action::Download => self.download_selected(work),
            Action::SetListStatus => {
                self.pick_read_status();
                None
            }
            Action::Refresh => self.reload_series(),
            Action::ToggleSynopsis => return None,
            _ if action.acts_on_a_title() => {
                self.works_elsewhere(action);
                None
            }
            _ => return None,
        })
    }

    /// `u`: the chapter under the cursor read, or unread when it is read.
    fn mark_selected(&mut self, work: WorkId) -> Option<Command> {
        let row = self.series_selected()?;
        let read = row.state != ChapterReadState::Read;
        Some(Command::Reading(ReadingCommand::MarkRead {
            work,
            chapters: vec![row.id.clone()],
            read,
        }))
    }

    /// `d`: the chapter under the cursor downloaded. One already on this computer says so.
    fn download_selected(&mut self, work: WorkId) -> Option<Command> {
        let row = self.series_selected()?;
        if downloaded(row) {
            self.push_toast(info(t::chapter_state_downloaded()));
            return None;
        }
        let pick = DownloadPick::Chapters { ids: vec![row.id.clone()] };
        Some(Command::Reading(ReadingCommand::Download { work, pick }))
    }

    /// `r`: the page asked for again, header and chapters.
    fn reload_series(&mut self) -> Option<Command> {
        let page = self.reading.series.as_mut()?;
        let now = self.clock.now_instant();
        let work = page.work.clone();
        if page.view.is_none() {
            page.wait = Wait::asked(now);
            page.failure = None;
        }
        if page.chapters.is_none() || page.chapters_failure.is_some() {
            page.chapters_wait = Wait::asked(now);
        }
        self.commands
            .push(Command::Reading(ReadingCommand::LoadTracking { work: work.clone() }));
        Some(Command::Reading(ReadingCommand::LoadSeries { work }))
    }

    /// `a`: the list picker, on the list the series is on.
    fn pick_read_status(&mut self) {
        let current = self.series_here().and_then(|page| page.view.as_ref()?.list);
        self.cursors.overlay.selected =
            current.and_then(|list| READ_LISTS.iter().position(|l| *l == list)).unwrap_or(0);
        self.nav.open_overlay(Overlay::ListStatus);
    }

    /// The list picked for the series on the page.
    pub(super) fn confirm_read_status(&mut self) -> Option<Command> {
        let work = self.series_here()?.work.clone();
        let status = *READ_LISTS.get(self.cursors.overlay.selected)?;
        self.nav.close_overlay();
        Some(Command::Reading(ReadingCommand::SetStatus { work, status }))
    }
}

#[cfg(test)]
mod tests {
    use kaeri_core::{
        books::{ChapterId, WorkId},
        media::Medium,
    };
    use kaeri_model::{
        event::Event,
        medium::{AppEdition, KaeriIsFor, MediumView},
        reading::{
            ChapterReadState, ChapterRow, ReadingCommand, ReadingEvent, ReadingFailure,
            ReadingHomeView, SeriesAction, SeriesView, VolumeGroup, WorkTile, WorkTitles,
        },
    };

    use super::*;
    use crate::{app::fixtures::*, nav::Section};

    fn tile(id: &str, title: &str) -> WorkTile {
        WorkTile {
            work: WorkId::new(id),
            title: title.into(),
            medium: Medium::Manga,
            cover: None,
            format_label: None,
            progress: None,
            next: None,
            new_chapter: false,
            adult: false,
            facts: Vec::new(),
            demo: false,
            not_in_sources: false,
            origin: None,
            amount: None,
            started: None,
        }
    }

    /// Reading alone, on Home: Continue with one title, Up next with two.
    fn home() -> App {
        let mut a = app();
        a.apply(Event::Medium(MediumView {
            answer: KaeriIsFor::Read,
            edition: AppEdition::Full,
            fixed: false,
            reading_available: true,
            ask: None,
            account: None,
        }));
        a.go_to_section(Section::Home);
        a.apply(Event::Reading(ReadingEvent::Home(Box::new(ReadingHomeView {
            continue_reading: vec![tile("a:1", "Harbor Light Recipes")],
            up_next: vec![tile("a:2", "Orchard of Small Hours"), tile("a:3", "Tide Tables")],
            ..Default::default()
        }))));
        a.take_commands();
        a.nav.focus_stage();
        a
    }

    fn chapter(n: u32, state: ChapterReadState) -> ChapterRow {
        ChapterRow {
            id: ChapterId(format!("c{n}")),
            label: format!("Chapter {n}"),
            state,
            number: Some(n.to_string()),
            title: None,
            volume: None,
            published: None,
            pages: Some(20),
            new_chapter: false,
            download: None,
            versions: 1,
            bookmarked: false,
            version_label: None,
            read_at: None,
            line: Vec::new(),
            gone_from_source: false,
            parts: 0,
            missing_parts: Vec::new(),
            part_groups: Vec::new(),
            other_group: false,
            group_language: None,
            other_source: None,
            on_device: false,
        }
    }

    fn view(work: &str) -> SeriesView {
        SeriesView {
            work: WorkId::new(work),
            medium: Medium::Manga,
            titles: WorkTitles {
                display: "Tide Tables".into(),
                romaji: None,
                english: None,
                native: None,
                synonyms: Vec::new(),
            },
            action: SeriesAction::Continue {
                chapter: ChapterId("c2".into()),
                label: "2".into(),
                page: 5,
            },
            cover: None,
            banner: None,
            format: None,
            origin: None,
            status: None,
            synopsis: None,
            genres: Vec::new(),
            authors: Vec::new(),
            chapters_total: None,
            volumes_total: None,
            edition: None,
            list: None,
            score: None,
            tracking: Vec::new(),
            other_medium: None,
            ask_where_stopped: None,
            rhythm: None,
            facts: Vec::new(),
            year: None,
            adult: false,
            recommended: Vec::new(),
            partial: false,
            demo: None,
            demo_other: None,
        }
    }

    /// The page's header and its three chapters, the second in progress.
    fn answer(a: &mut App, work: &str) {
        a.apply(Event::Reading(ReadingEvent::Series(Box::new(view(work)))));
        a.apply(Event::Reading(ReadingEvent::Chapters {
            work: WorkId::new(work),
            edition: None,
            volumes: vec![VolumeGroup {
                volume: None,
                title: None,
                chapters: vec![
                    chapter(1, ChapterReadState::Read),
                    chapter(2, ChapterReadState::InProgress { page: 5, of: Some(20) }),
                    chapter(3, ChapterReadState::Unread),
                ],
                all_read: false,
                flat: true,
            }],
            failure: None,
        }));
    }

    #[test]
    fn enter_on_a_home_line_opens_the_series_page() {
        let mut a = home();
        a.handle(Action::Down, 10);
        let task = a.handle(Action::Open, 10);
        assert_eq!(
            task,
            Some(Command::Reading(ReadingCommand::LoadSeries { work: WorkId::new("a:2") }))
        );
        assert_eq!(a.nav.current(), &StageView::Series(WorkId::new("a:2")));
        assert_eq!(
            a.take_commands(),
            vec![Command::Reading(ReadingCommand::LoadTracking { work: WorkId::new("a:2") })]
        );
    }

    #[test]
    fn enter_on_continue_reads_and_e_opens_the_series_page() {
        use kaeri_model::reader::{OpenAt, ReaderCommand};
        let mut a = home();
        let task = a.handle(Action::Open, 10);
        assert!(
            matches!(&task, Some(Command::Reader(ReaderCommand::Open { work, chapter: None, at: OpenAt::Resume, .. })) if work.0 == "a:1"),
            "{task:?}"
        );
        assert_eq!(a.nav.current(), &StageView::Reader(WorkId::new("a:1")));
        // q stops the open and goes back to Home.
        let task = a.handle(Action::ReaderLeave, 10);
        assert_eq!(task, Some(Command::Reader(ReaderCommand::Close { session: 0 })));
        assert!(matches!(a.nav.current(), StageView::Section(Section::Home)));
        let task = a.handle(Action::ShowEpisodes, 10);
        assert_eq!(
            task,
            Some(Command::Reading(ReadingCommand::LoadSeries { work: WorkId::new("a:1") }))
        );
        assert_eq!(a.nav.current(), &StageView::Series(WorkId::new("a:1")));
    }

    #[test]
    fn the_cursor_starts_on_the_chapter_the_page_reads_next() {
        let mut a = home();
        a.handle(Action::Down, 10);
        a.handle(Action::Open, 10);
        answer(&mut a, "a:2");
        // Newest first: chapter 3, then 2, the one in progress.
        assert_eq!(a.series_selected().map(|r| r.id.0.as_str()), Some("c2"));
        a.handle(Action::Up, 10);
        assert_eq!(a.series_selected().map(|r| r.id.0.as_str()), Some("c3"));
    }

    #[test]
    fn u_marks_the_chapter_read_and_unread() {
        let mut a = home();
        a.handle(Action::Down, 10);
        a.handle(Action::Open, 10);
        answer(&mut a, "a:2");
        assert_eq!(
            a.handle(Action::ToggleWatched, 10),
            Some(Command::Reading(ReadingCommand::MarkRead {
                work: WorkId::new("a:2"),
                chapters: vec![ChapterId("c2".into())],
                read: true,
            }))
        );
        a.handle(Action::Bottom, 10);
        assert_eq!(
            a.handle(Action::ToggleWatched, 10),
            Some(Command::Reading(ReadingCommand::MarkRead {
                work: WorkId::new("a:2"),
                chapters: vec![ChapterId("c1".into())],
                read: false,
            }))
        );
    }

    #[test]
    fn d_downloads_the_chapter_and_a_picks_the_list() {
        let mut a = home();
        a.handle(Action::Down, 10);
        a.handle(Action::Open, 10);
        answer(&mut a, "a:2");
        assert_eq!(
            a.handle(Action::Download, 10),
            Some(Command::Reading(ReadingCommand::Download {
                work: WorkId::new("a:2"),
                pick: DownloadPick::Chapters { ids: vec![ChapterId("c2".into())] },
            }))
        );
        a.handle(Action::SetListStatus, 10);
        assert_eq!(a.nav.overlay(), Some(&Overlay::ListStatus));
        a.handle(Action::Down, 10);
        assert_eq!(
            a.handle(Action::Open, 10),
            Some(Command::Reading(ReadingCommand::SetStatus {
                work: WorkId::new("a:2"),
                status: LibrarySegmentRead::Planning,
            }))
        );
        assert!(!a.nav.has_overlay());
    }

    #[test]
    fn esc_goes_back_with_the_cursor_where_it_was() {
        let mut a = home();
        a.handle(Action::Down, 10);
        a.handle(Action::Down, 10);
        a.handle(Action::Open, 10);
        answer(&mut a, "a:3");
        a.handle(Action::Down, 10);
        a.handle(Action::Back, 10);
        assert!(matches!(a.nav.current(), StageView::Section(Section::Home)));
        assert_eq!(a.cursor(), 2, "on Tide Tables, as it was");
        assert!(
            matches!(a.read_selected(), Some(crate::app::ReadLine::Work { title, .. }) if title == "Tide Tables")
        );
    }

    #[test]
    fn enter_on_a_chapter_opens_it_in_the_reader() {
        use kaeri_model::reader::{OpenAt, ReaderCommand};
        let mut a = home();
        a.handle(Action::Down, 10);
        a.handle(Action::Open, 10);
        answer(&mut a, "a:2");
        a.toasts.clear();
        let task = a.handle(Action::Open, 10);
        assert!(
            matches!(&task, Some(Command::Reader(ReaderCommand::Open { work, chapter: Some(c), at: OpenAt::Resume, .. })) if work.0 == "a:2" && c.0 == "c2"),
            "{task:?}"
        );
        assert_eq!(a.nav.current(), &StageView::Reader(WorkId::new("a:2")));
        assert!(a.toasts.is_empty(), "nothing says it opens elsewhere");
    }

    #[test]
    fn q_in_the_reader_returns_to_the_series_page_with_the_cursor_where_it_was() {
        use kaeri_model::reader::ReaderCommand;
        let mut a = home();
        a.handle(Action::Down, 10);
        a.handle(Action::Open, 10);
        answer(&mut a, "a:2");
        a.handle(Action::Down, 10);
        assert_eq!(a.series_selected().map(|r| r.id.0.as_str()), Some("c1"));
        a.handle(Action::Open, 10);
        assert_eq!(a.nav.current(), &StageView::Reader(WorkId::new("a:2")));
        // The reader asks for the chapter list too; it must not move the page's cursor.
        answer(&mut a, "a:2");
        let task = a.handle(Action::ReaderLeave, 10);
        assert_eq!(task, Some(Command::Reader(ReaderCommand::Close { session: 0 })));
        assert_eq!(a.nav.current(), &StageView::Series(WorkId::new("a:2")));
        assert_eq!(a.series_selected().map(|r| r.id.0.as_str()), Some("c1"));
    }

    #[test]
    fn offline_a_chapter_that_isnt_downloaded_says_so_and_stays() {
        let mut a = home();
        a.handle(Action::Down, 10);
        a.handle(Action::Open, 10);
        answer(&mut a, "a:2");
        a.apply(Event::Reading(ReadingEvent::Failed {
            about: kaeri_model::reading::ReadingAbout::Chapters,
            failure: ReadingFailure::Offline,
        }));
        a.toasts.clear();
        assert_eq!(a.handle(Action::Open, 10), None);
        assert_eq!(a.nav.current(), &StageView::Series(WorkId::new("a:2")));
        let said: Vec<&str> = a.toasts.iter().map(|t| t.notice.text.as_str()).collect();
        assert_eq!(said, vec!["Chapter 2 isn't downloaded"]);
    }

    #[test]
    fn a_failure_before_the_header_is_the_pages_and_r_asks_again() {
        let mut a = home();
        a.handle(Action::Down, 10);
        a.handle(Action::Open, 10);
        a.apply(Event::Reading(ReadingEvent::Failed {
            about: kaeri_model::reading::ReadingAbout::Series,
            failure: ReadingFailure::Offline,
        }));
        assert!(a.toasts.is_empty(), "said on the page, not as a toast");
        assert_eq!(
            a.series_here().and_then(|p| p.failure.clone()),
            Some(ReadingFailure::Offline)
        );
        assert_eq!(
            a.handle(Action::Refresh, 10),
            Some(Command::Reading(ReadingCommand::LoadSeries { work: WorkId::new("a:2") }))
        );
        assert!(a.series_here().is_some_and(|p| p.failure.is_none()));
    }

    #[test]
    fn a_list_sent_again_keeps_the_cursor_on_its_chapter() {
        let mut a = home();
        a.handle(Action::Down, 10);
        a.handle(Action::Open, 10);
        answer(&mut a, "a:2");
        a.apply(Event::Reading(ReadingEvent::Chapters {
            work: WorkId::new("a:2"),
            edition: None,
            volumes: vec![VolumeGroup {
                volume: None,
                title: None,
                chapters: vec![
                    chapter(1, ChapterReadState::Read),
                    chapter(2, ChapterReadState::Read),
                    chapter(3, ChapterReadState::Unread),
                    chapter(4, ChapterReadState::Unread),
                ],
                all_read: false,
                flat: true,
            }],
            failure: None,
        }));
        assert_eq!(a.series_selected().map(|r| r.id.0.as_str()), Some("c2"));
    }
}
