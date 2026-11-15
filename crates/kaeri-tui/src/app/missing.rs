//! A title page whose next episode has aired and that no source lists yet.
//!
//! The page says so under the progress meter, `r` (Refresh) checks the sources again and Space
//! plays the episode before it. The terminal has no fades: a check shows nothing for
//! [`CHECKING_AFTER`], then the status line says it is checking, and the answer takes its place
//! once Checking has been up for [`CHECKING_HELD`].

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use kaeri_core::ids::AnilistId;
use kaeri_model::{
    catalog::Entry,
    command::Command,
    missing::{CheckOutcome, EpisodeCheck, MissingCommand, MissingEpisode, missing_in},
};

use super::App;
use crate::nav::StageView;

/// How long a check runs before the page says it is checking. Most sources answer sooner, and
/// an answer inside this shows at once.
pub const CHECKING_AFTER: Duration = Duration::from_millis(600);

/// How long Checking stays once it is up, so it never flickers past.
pub const CHECKING_HELD: Duration = Duration::from_millis(300);

/// What the status line under the meter says.
#[derive(Debug, Clone, PartialEq)]
pub enum MissingStatus {
    /// A check is running and has taken longer than [`CHECKING_AFTER`].
    Checking,
    /// The episode, and what the last check the person asked for came to, if one did.
    Says { missing: MissingEpisode, answer: Option<CheckOutcome> },
}

/// The checks of missing episodes the page has asked for, and what they came to.
#[derive(Debug, Default)]
pub(crate) struct Checks {
    /// The check under way, until its answer is on screen.
    asking: Option<Asking>,
    /// The last answer to a check, by title and episode.
    said: HashMap<(AnilistId, u32), CheckOutcome>,
    /// The title whose episode table the page asked for to find out, so a record sent again
    /// before the table answers does not ask twice.
    listing: Option<AnilistId>,
}

/// A check asked for with `r`.
#[derive(Debug, Clone)]
struct Asking {
    id: AnilistId,
    /// The episode as the page had it when asked. A find relists the title ahead of its answer,
    /// and while Checking is held the page goes on saying this rather than jumping early.
    missing: MissingEpisode,
    /// The answer before this one, said until Checking comes up.
    before: Option<CheckOutcome>,
    pressed: Instant,
    /// The answer, and when it came.
    answer: Option<(CheckOutcome, Instant)>,
}

impl Asking {
    /// When the answer takes the place of what was there: at once inside the wait, else once
    /// Checking has been up for its while.
    fn shows_at(&self) -> Option<Instant> {
        let (_, at) = self.answer?;
        let checking = self.pressed + CHECKING_AFTER;
        Some(if at < checking { at } else { at.max(checking + CHECKING_HELD) })
    }

    /// Whether the answer is on screen at `now`.
    fn shown(&self, now: Instant) -> bool {
        self.shows_at().is_some_and(|at| now >= at)
    }

    fn is(&self, id: AnilistId, episode: u32) -> bool {
        self.id == id && self.missing.episode == episode
    }
}

/// Whether `entry`'s record says the episode its page would play next has aired: the one case
/// the page needs the episode table to know whether a source lists it.
fn next_has_aired(entry: &Entry) -> bool {
    if super::title::watched_all(entry) {
        return false;
    }
    let next = entry.progress.map_or(1, |p| p.next);
    entry.latest_aired().is_some_and(|(latest, _)| latest >= next)
}

impl App {
    /// The missing episode of the title page on screen, when it has one: its next episode
    /// aired and no source lists it. `None` anywhere else, and until the page's episode table
    /// is in.
    pub fn missing_here(&self) -> Option<MissingEpisode> {
        let StageView::Title(id) = self.nav.current() else { return None };
        let entry = self.target()?;
        let next = entry.progress.map_or(1, |p| p.next);
        let rows = self.episode_rows_for(*id);
        let found = missing_in(rows, next, entry.resume.is_some(), self.clock.now_epoch());
        // Held while a check runs and until its answer shows, so a find that relists the title
        // ahead of its answer does not clear the line before Checking has had its while.
        let held = self
            .checks
            .asking
            .as_ref()
            .filter(|asking| asking.id == *id && !asking.shown(self.clock.now_instant()));
        found.or_else(|| held.map(|asking| asking.missing.clone()))
    }

    /// What the status line under the meter says, when the page has a missing episode.
    pub fn missing_status(&self) -> Option<MissingStatus> {
        let missing = self.missing_here()?;
        let StageView::Title(id) = self.nav.current() else { return None };
        let now = self.clock.now_instant();
        let asking = self.checks.asking.as_ref().filter(|a| a.is(*id, missing.episode));
        let answer = match asking {
            Some(asking) if asking.shown(now) => asking.answer.map(|(outcome, _)| outcome),
            Some(asking) if now >= asking.pressed + CHECKING_AFTER => {
                return Some(MissingStatus::Checking);
            }
            Some(asking) => asking.before,
            None => self.checks.said.get(&(*id, missing.episode)).copied(),
        };
        Some(MissingStatus::Says { missing, answer })
    }

    /// Whether a check is running or its answer waits on Checking's hold: the screen changes
    /// with no event to say so, so the loop keeps painting.
    pub fn checking_missing(&self) -> bool {
        let now = self.clock.now_instant();
        self.checks.asking.as_ref().is_some_and(|asking| !asking.shown(now))
    }

    /// `r` on a page whose episode is missing: the sources asked again for it. A check already
    /// running is not asked twice.
    pub(super) fn check_again(&mut self) -> Option<Command> {
        let missing = self.missing_here()?;
        let id = self.title_on_screen()?;
        if !missing.checkable() || self.checking_missing() {
            return None;
        }
        self.images.requests().retry_failed();
        let episode = missing.episode;
        let before = self.checks.said.get(&(id, episode)).copied();
        let pressed = self.clock.now_instant();
        self.checks.asking = Some(Asking { id, missing, before, pressed, answer: None });
        Some(Command::Missing(MissingCommand::CheckAgain { id, episode }))
    }

    /// Space on a page whose episode is missing: the episode before it, when a source has one.
    /// With none, nothing: the missing one would only fail to start.
    pub(super) fn play_before_missing(&mut self) -> Option<Command> {
        let id = self.title_on_screen()?;
        let previous = self.missing_here()?.previous?;
        Some(self.begin_playback(id, previous.to_string()))
    }

    /// What a check came to. One the person asked for is kept to show once Checking has had
    /// its while; a quiet recheck that found the episode has already relisted the title, so the
    /// page has the episode by now.
    pub(super) fn apply_check(&mut self, check: EpisodeCheck) {
        let EpisodeCheck { id, episode, outcome, asked } = check;
        self.checks.said.insert((id, episode), outcome);
        if !asked {
            return;
        }
        let now = self.clock.now_instant();
        if let Some(asking) = self
            .checks
            .asking
            .as_mut()
            .filter(|asking| asking.is(id, episode) && asking.answer.is_none())
        {
            asking.answer = Some((outcome, now));
        }
    }

    /// A title page's full record is in: when it says the next episode has aired, the page asks
    /// for the episode table to find out whether a source lists it.
    pub(super) fn list_if_missing(&mut self) {
        let Some(entry) = self.detail.as_ref() else { return };
        let id = entry.id;
        if self.title_on_screen() != Some(id)
            || !self.episode_rows_for(id).is_empty()
            || self.checks.listing == Some(id)
            || !next_has_aired(entry)
        {
            return;
        }
        self.checks.listing = Some(id);
        self.commands.push(Command::LoadEpisodes(id));
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use kaeri_model::{
        catalog::Aired,
        missing::{CheckOutcome, EpisodeCheck, MissingCommand, MissingPhase},
        progress::TitleProgress,
    };

    use super::{CHECKING_AFTER, CHECKING_HELD, MissingStatus};
    use crate::{
        app::fixtures::*,
        clock::{Clock, Zone},
    };

    const NOW: i64 = 1_800_000_000;
    const MINUTE: i64 = 60;

    /// Episodes 1 to 5 listed, 6 aired 25 minutes ago and listed by none.
    fn table(six_listed: bool) -> Vec<EpisodeRow> {
        (1..=6)
            .map(|n| EpisodeRow {
                number: n.to_string(),
                title: None,
                thumbnail: None,
                description: None,
                duration_secs: Some(1_435),
                watched: if n < 6 { 1.0 } else { 0.0 },
                completed: n < 6,
                kind: None,
                skippable: false,
                airs_at: Some(if n == 6 {
                    NOW - 25 * MINUTE
                } else {
                    NOW - (6 - n) * 7 * 86_400
                }),
                delayed_from: None,
                metadata_only: false,
                source: None,
                unsourced: n == 6 && !six_listed,
                past_progress: false,
                spoiler_ahead: false,
                synopsis_pending: false,
            })
            .collect()
    }

    /// Kumo no Ue no Toshokan's page, five watched, with the record in and the table asked for.
    fn on_the_page() -> App {
        let mut a = app();
        a.clock = Clock::fixed(NOW, Zone::utc());
        a.handle(Action::JumpSection(1), 20);
        let row = Entry::new(AnilistId::new(900_002), "Kumo no Ue no Toshokan");
        a.apply(a.content_here(ContentList::Entries(vec![row])));
        a.handle(Action::Open, 20);
        let mut entry = Entry::new(AnilistId::new(900_002), "Kumo no Ue no Toshokan");
        entry.episodes = Some(12);
        entry.progress = Some(TitleProgress { done: 5, next: 6 });
        entry.last_aired = Some(Aired { episode: 6, ago: 25 * MINUTE });
        a.apply(Event::Detail(Box::new(entry)));
        assert_eq!(
            a.take_commands(),
            [Command::LoadEpisodes(AnilistId::new(900_002))],
            "the page asks for the table"
        );
        a.apply(a.episodes_here(table(false)));
        a
    }

    fn checked(a: &mut App, outcome: CheckOutcome) {
        let id = AnilistId::new(900_002);
        a.apply(Event::EpisodeChecked(EpisodeCheck { id, episode: 6, outcome, asked: true }));
    }

    fn answer(a: &App) -> Option<CheckOutcome> {
        match a.missing_status() {
            Some(MissingStatus::Says { answer, .. }) => answer,
            other => panic!("not the status: {other:?}"),
        }
    }

    #[test]
    fn the_page_says_the_episode_is_missing_and_space_plays_the_one_before() {
        let mut a = on_the_page();
        let missing = a.missing_here().expect("episode 6 is missing");
        assert_eq!(
            (missing.episode, missing.phase, missing.previous),
            (6, MissingPhase::Quiet, Some(5))
        );
        assert_eq!(answer(&a), None);
        assert_eq!(
            a.handle(Action::PlayNext, 20),
            Some(Command::play(AnilistId::new(900_002), "5".to_string()))
        );
    }

    #[test]
    fn a_page_whose_next_episode_has_not_aired_asks_for_no_table() {
        let mut a = home_list(3);
        a.handle(Action::Open, 20);
        let mut entry = Entry::new(AnilistId::new(1), "Title 0");
        entry.progress = Some(TitleProgress { done: 6, next: 7 });
        entry.last_aired = Some(Aired { episode: 6, ago: 3 * 86_400 });
        a.apply(Event::Detail(Box::new(entry.clone())));
        assert!(a.take_commands().is_empty());
        assert_eq!(a.missing_status(), None);
    }

    #[test]
    fn r_checks_again_and_an_answer_inside_the_wait_shows_at_once() {
        let mut a = on_the_page();
        let task = a.handle(Action::Refresh, 20);
        assert_eq!(
            task,
            Some(Command::Missing(MissingCommand::CheckAgain {
                id: AnilistId::new(900_002),
                episode: 6
            }))
        );
        // A second press while it runs asks nothing more.
        assert_eq!(a.handle(Action::Refresh, 20), None);
        a.clock.advance(Duration::from_millis(200));
        assert!(matches!(a.missing_status(), Some(MissingStatus::Says { answer: None, .. })));
        checked(&mut a, CheckOutcome::Missing);
        assert_eq!(answer(&a), Some(CheckOutcome::Missing), "no Checking was ever shown");
        assert!(!a.checking_missing());
    }

    #[test]
    fn checking_shows_after_the_wait_and_holds_before_the_answer_takes_its_place() {
        let mut a = on_the_page();
        a.handle(Action::Refresh, 20);
        a.clock.advance(CHECKING_AFTER - Duration::from_millis(1));
        assert!(matches!(a.missing_status(), Some(MissingStatus::Says { .. })));
        a.clock.advance(Duration::from_millis(1));
        assert_eq!(a.missing_status(), Some(MissingStatus::Checking));
        assert!(a.checking_missing(), "the loop keeps painting");

        // The source finds it a tenth of a second later: the table comes first, then the
        // answer, and Checking stays its while all the same.
        a.clock.advance(Duration::from_millis(100));
        a.apply(a.episodes_here(table(true)));
        checked(&mut a, CheckOutcome::Found);
        assert_eq!(a.missing_status(), Some(MissingStatus::Checking));
        a.clock.advance(CHECKING_HELD - Duration::from_millis(100));
        assert_eq!(a.missing_status(), None, "found: the status goes and Play is next");
        assert!(!a.checking_missing());
        assert_eq!(
            a.handle(Action::PlayNext, 20),
            Some(Command::play(AnilistId::new(900_002), "6".to_string()))
        );
    }

    #[test]
    fn no_answer_and_offline_are_said_and_r_asks_again_after_an_answer() {
        let mut a = on_the_page();
        a.handle(Action::Refresh, 20);
        checked(&mut a, CheckOutcome::NoAnswer);
        assert_eq!(answer(&a), Some(CheckOutcome::NoAnswer));
        assert!(a.handle(Action::Refresh, 20).is_some(), "asked again after the answer");
        // Until Checking comes up the last answer stays.
        assert_eq!(answer(&a), Some(CheckOutcome::NoAnswer));
        checked(&mut a, CheckOutcome::Offline);
        assert_eq!(answer(&a), Some(CheckOutcome::Offline));
    }

    #[test]
    fn a_quiet_recheck_that_finds_it_needs_no_check_on_screen() {
        let mut a = on_the_page();
        a.apply(a.episodes_here(table(true)));
        let id = AnilistId::new(900_002);
        let found = CheckOutcome::Found;
        a.apply(Event::EpisodeChecked(EpisodeCheck {
            id,
            episode: 6,
            outcome: found,
            asked: false,
        }));
        assert_eq!(a.missing_status(), None);
        assert!(!a.checking_missing());
    }

    #[test]
    fn a_week_on_r_is_refresh_again() {
        let mut a = on_the_page();
        a.clock.advance(Duration::from_secs(8 * 86_400));
        let missing = a.missing_here().expect("still missing");
        assert_eq!(missing.phase, MissingPhase::LongAgo);
        assert_eq!(
            a.handle(Action::Refresh, 20),
            Some(Command::LoadDetail(AnilistId::new(900_002)))
        );
    }
}
