//! Marking skip times in the player: the marking bar's steps, saving a mark, clearing one,
//! reporting AniSkip's times, asking "Was that the opening?" after a skip by hand, and asking
//! about the edges of a skip found from the audio.
//!
//! The steps and checks are `kaeri_playback::marking`'s; this holds them for the file
//! playing, keeps what is saved in the store, and says each change to the player as an
//! [`Overlay`].

use kaeri_model::{
    command::PlayerCommand,
    event::Event,
    playback::{
        FOUND_PROMPT_DELAY_SECS, SkipEdge, SkipMarkCheck, SkipMarkNote, SkipMarkStep,
        SkipMarkView, SkipNotice, SkipPrompt,
    },
};
use kaeri_playback::{
    AniSkipTime, MarkedSkip, PlaybackTracker, SkipInterval, SkipKind, SkipOrigin,
    aniskip::{self, Request, Vote},
    marking::{self, Check, Edge, JumpWatch, Marking, Step},
};
use kaeri_store::{SkipMark, Store, skip_found::state};

use super::{
    Run,
    skips::{offered, taken},
};
use crate::playback::link::{Overlay, PlayerLink};

/// The length an episode is taken to be when the file has not said its own yet, in seconds: a
/// broadcast episode's.
const USUAL_LENGTH: f64 = 1440.0;

/// What marking holds for the playback: the bar while it is open, the prompt while it is up,
/// and the viewer's seeks, for "Was that the opening?".
#[derive(Debug)]
pub(super) struct SkipMarking {
    bar: Option<Marking>,
    prompt: Option<SkipPrompt>,
    jumps: JumpWatch,
    /// Whether "Was that the opening?" is asked, as Settings had it, until Don't ask again.
    ask: bool,
    /// Whether marks and votes go to AniSkip, as Settings had it, until the viewer answers
    /// "Share your skip times with AniSkip?". `None` until asked.
    share: Option<bool>,
    /// What the save that asked would send, held until the answer.
    unshared: Vec<Request>,
    clock: std::time::Instant,
    /// The bar reviews the found stretch of this kind, opened by Yes or by Adjust.
    found_review: Option<(SkipKind, FoundFrom)>,
    /// A found stretch of this kind was skipped, landing here: its card is due once playback has
    /// gone on past it.
    found_landed: Option<(SkipKind, f64)>,
}

/// How a found stretch came to be confirmed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FoundFrom {
    /// Yes on the card: saved at once, and the card says so.
    Yes,
    /// Adjust: the review opens, and its Save confirms.
    Adjust,
}

impl SkipMarking {
    pub(super) fn new(ask: bool, share: Option<bool>) -> Self {
        Self {
            bar: None,
            prompt: None,
            jumps: JumpWatch::default(),
            ask,
            share,
            unshared: Vec::new(),
            clock: std::time::Instant::now(),
            found_review: None,
            found_landed: None,
        }
    }

    fn sharing(&self) -> bool {
        self.share == Some(true)
    }

    /// A new file opens: the bar and the prompt were about the last one.
    pub(super) fn file_opened(&mut self) {
        self.bar = None;
        self.prompt = None;
        self.found_review = None;
        self.found_landed = None;
    }

    fn now(&self) -> f64 {
        self.clock.elapsed().as_secs_f64()
    }
}

/// Whether `command` is one of marking's, which [`Run::mark_command`] answers.
pub(super) const fn is_marking(command: &PlayerCommand) -> bool {
    matches!(
        command,
        PlayerCommand::BeginSkipMark { .. }
            | PlayerCommand::SetSkipMarkKind { .. }
            | PlayerCommand::MarkSkipEdge
            | PlayerCommand::NudgeSkipEdge { .. }
            | PlayerCommand::PreviewSkipEdges
            | PlayerCommand::SaveSkipMark
            | PlayerCommand::CancelSkipMark
            | PlayerCommand::ClearMySkip { .. }
            | PlayerCommand::ReportSkip { .. }
            | PlayerCommand::DismissSkipPrompt
            | PlayerCommand::NeverAskWasOpening
            | PlayerCommand::AnswerShareSkipTimes { .. }
            | PlayerCommand::ConfirmFoundSkip { .. }
            | PlayerCommand::AdjustFoundSkip { .. }
    )
}

#[expect(clippy::cast_precision_loss, reason = "milliseconds of a nudge")]
fn ms_to_secs(ms: i64) -> f64 {
    ms as f64 / 1000.0
}

#[expect(clippy::cast_possible_truncation, reason = "seconds within an episode")]
fn secs_to_ms(secs: f64) -> i64 {
    (secs * 1000.0).round() as i64
}

/// Run `f` on the store off the async runtime. `None` when it failed, which is logged: marks
/// are an extra, and playback goes on without them.
async fn on_store<T: Send + 'static>(
    store: &Store,
    what: &'static str,
    f: impl FnOnce(&Store) -> kaeri_store::Result<T> + Send + 'static,
) -> Option<T> {
    let store = store.clone();
    match tokio::task::spawn_blocking(move || f(&store)).await {
        Ok(Ok(value)) => Some(value),
        Ok(Err(error)) => {
            tracing::warn!(%error, what, "skip marks");
            None
        }
        Err(_) => None,
    }
}

impl Run<'_> {
    /// Aniskip's times as the tracker uses them: the ones not reported.
    pub(super) fn given(&self) -> Vec<SkipInterval> {
        self.aniskip.iter().map(|time| time.interval).collect()
    }

    /// Aniskip's time of `kind` in use, if any.
    fn aniskip_time(&self, kind: SkipKind, tracker: &PlaybackTracker) -> Option<AniSkipTime> {
        let in_use =
            tracker.skips().iter().any(|s| s.kind == kind && s.origin == SkipOrigin::AniSkip);
        in_use.then(|| self.aniskip.iter().find(|t| t.interval.kind == kind).cloned()).flatten()
    }

    fn episode_length(&self, tracker: &PlaybackTracker) -> f64 {
        tracker
            .duration()
            .or_else(|| self.context.runtime_secs.map(f64::from))
            .unwrap_or(USUAL_LENGTH)
    }

    /// The bar as the player draws it.
    fn mark_view(&self, bar: &Marking, tracker: &PlaybackTracker) -> SkipMarkView {
        let check = bar.check(self.episode_length(tracker));
        let mut notes = Vec::new();
        let found = self.marking.found_review.is_some_and(|(kind, _)| kind == bar.kind);
        if found && bar.step == Step::Review {
            notes.push(SkipMarkNote::Found);
        }
        if bar.step == Step::Review {
            if self.context.mal_id.is_none() {
                notes.push(SkipMarkNote::NoEntry);
            } else if self.marking.sharing() && check == Check::Fine {
                notes.push(SkipMarkNote::Shares);
            }
            if let (Some(stretch), Some(time)) =
                (bar.stretch(), self.aniskip_time(bar.kind, tracker))
            {
                let theirs = (time.interval.start, time.interval.end);
                if !marking::same(stretch, theirs) {
                    notes.push(SkipMarkNote::Replaces { start: theirs.0, end: theirs.1 });
                }
            }
        }
        SkipMarkView {
            kind: offered(bar.kind),
            step: match bar.step {
                Step::Start => SkipMarkStep::Start,
                Step::End => SkipMarkStep::End,
                Step::Review => SkipMarkStep::Review,
            },
            start: bar.start,
            end: bar.end,
            length: bar.length(),
            check: match check {
                Check::Fine => SkipMarkCheck::Fine,
                Check::TooShort => SkipMarkCheck::TooShort,
                Check::TooLong { most } => SkipMarkCheck::TooLong { most },
            },
            notes,
            can_save: bar.step == Step::Review && check != Check::TooShort,
            editing: bar.editing,
            previewing: bar.previewing(),
            found,
        }
    }

    async fn say_bar(&self, link: &dyn PlayerLink, tracker: &PlaybackTracker) {
        let view = self.marking.bar.as_ref().map(|bar| self.mark_view(bar, tracker));
        let _ = link.overlay(Overlay::SkipMark(view)).await;
    }

    async fn say_prompt(&mut self, link: &dyn PlayerLink, prompt: Option<SkipPrompt>) {
        if self.marking.prompt == prompt {
            return;
        }
        // Said in the log, so a card that never showed can be told from one never asked.
        match (self.marking.prompt, prompt) {
            (_, Some(card)) => tracing::info!(?card, "skip card shown"),
            (Some(card), None) => tracing::info!(?card, "skip card gone"),
            (None, None) => {}
        }
        self.marking.prompt = prompt;
        let _ = link.overlay(Overlay::SkipPrompt(prompt)).await;
    }

    /// Answer one of marking's commands: see [`is_marking`].
    pub(super) async fn mark_command(
        &mut self,
        command: PlayerCommand,
        link: &dyn PlayerLink,
        tracker: &mut PlaybackTracker,
    ) {
        if let Some(card) = self.marking.prompt {
            let answer = match command {
                PlayerCommand::ConfirmFoundSkip { .. } => Some("yes"),
                PlayerCommand::AdjustFoundSkip { .. } => Some("adjust"),
                PlayerCommand::NeverAskWasOpening => Some("don't ask again"),
                PlayerCommand::BeginSkipMark { .. } => Some("mark"),
                PlayerCommand::ReportSkip { .. } => Some("report"),
                PlayerCommand::AnswerShareSkipTimes { share: true } => Some("share"),
                PlayerCommand::AnswerShareSkipTimes { share: false } => Some("don't share"),
                // The app's own: the card's time ran out, or the viewer closed it.
                PlayerCommand::DismissSkipPrompt => Some("dismissed"),
                _ => None,
            };
            if let Some(answer) = answer {
                tracing::info!(?card, answer, "skip card answered");
            }
        }
        match command {
            PlayerCommand::BeginSkipMark { kind } => self.begin_mark(kind, link, tracker).await,
            PlayerCommand::SetSkipMarkKind { kind } => {
                if let Some(bar) = &mut self.marking.bar {
                    bar.set_kind(taken(kind));
                }
                self.say_bar(link, tracker).await;
            }
            PlayerCommand::MarkSkipEdge => {
                if let Some(bar) = &mut self.marking.bar {
                    bar.mark(tracker.position());
                }
                self.say_bar(link, tracker).await;
            }
            PlayerCommand::NudgeSkipEdge { edge, by_ms } => {
                let by = ms_to_secs(by_ms);
                let duration = tracker.duration();
                match &mut self.marking.bar {
                    Some(bar) if bar.step == Step::Review => {
                        let edge = match edge {
                            SkipEdge::Start => Edge::Start,
                            SkipEdge::End => Edge::End,
                        };
                        bar.nudge(edge, by, duration);
                        // The frame the edge is now on, to judge it by.
                        let at = match edge {
                            Edge::Start => bar.start,
                            Edge::End => bar.end,
                        };
                        if let Some(at) = at {
                            let _ = link.seek(at, true).await;
                        }
                    }
                    // Nothing marked to move: the arrows move the playhead.
                    _ => {
                        let _ = link.seek(by, false).await;
                    }
                }
                self.say_bar(link, tracker).await;
            }
            PlayerCommand::PreviewSkipEdges => {
                if let Some(to) = self.marking.bar.as_mut().and_then(Marking::preview) {
                    let _ = link.seek(to, true).await;
                }
                self.say_bar(link, tracker).await;
            }
            PlayerCommand::SaveSkipMark => self.save_mark(link, tracker).await,
            PlayerCommand::CancelSkipMark => {
                // A found review has no steps before it: Cancel leaves the found times as they
                // were.
                if self.marking.found_review.take().is_some() {
                    self.marking.bar = None;
                } else if let Some(bar) = &mut self.marking.bar
                    && !bar.back()
                {
                    self.marking.bar = None;
                }
                self.say_bar(link, tracker).await;
            }
            PlayerCommand::ConfirmFoundSkip { kind } => {
                self.confirm_found(taken(kind), FoundFrom::Yes, link, tracker).await;
            }
            PlayerCommand::AdjustFoundSkip { kind } => {
                self.confirm_found(taken(kind), FoundFrom::Adjust, link, tracker).await;
            }
            PlayerCommand::ClearMySkip { kind } => {
                self.clear_mark(taken(kind), link, tracker).await
            }
            PlayerCommand::ReportSkip { kind } => self.report(taken(kind), link, tracker).await,
            PlayerCommand::DismissSkipPrompt => self.say_prompt(link, None).await,
            PlayerCommand::NeverAskWasOpening => {
                self.marking.ask = false;
                // The found times' card says so in place; "Was that the opening?" just goes.
                let said = matches!(self.marking.prompt, Some(SkipPrompt::FoundEdges { .. }))
                    .then_some(SkipPrompt::FoundNever);
                self.say_prompt(link, said).await;
                let _ = self.tx.send(Event::PlayerAskWasOpening(false));
            }
            PlayerCommand::AnswerShareSkipTimes { share } => {
                self.marking.share = Some(share);
                let held = std::mem::take(&mut self.marking.unshared);
                if share {
                    self.send(&held);
                }
                if self.marking.prompt == Some(SkipPrompt::Share) {
                    self.say_prompt(link, None).await;
                }
                let _ = self.tx.send(Event::PlayerShareSkipTimes(share));
            }
            _ => {}
        }
    }

    async fn begin_mark(
        &mut self,
        kind: Option<kaeri_model::playback::SkipKind>,
        link: &dyn PlayerLink,
        tracker: &PlaybackTracker,
    ) {
        if self.in_party {
            tracing::debug!("no marking in a watch party");
            return;
        }
        let kind = kind
            .map(taken)
            .unwrap_or_else(|| marking::guess_kind(tracker.position(), tracker.duration()));
        let skipped = match self.marking.prompt {
            Some(SkipPrompt::WasOpening { start, end }) if kind == SkipKind::Opening => {
                Some(SkipInterval { kind, start, end, origin: SkipOrigin::Yours })
            }
            _ => None,
        };
        let mine = self.marks.iter().find(|m| m.interval.kind == kind).map(|m| m.interval);
        self.marking.bar = Some(match (skipped, mine) {
            (Some(skipped), _) => Marking::review(&skipped, false),
            (None, Some(mine)) => Marking::review(&mine, true),
            (None, None) => Marking::begin(kind),
        });
        self.say_prompt(link, None).await;
        self.say_bar(link, tracker).await;
    }

    async fn save_mark(&mut self, link: &dyn PlayerLink, tracker: &mut PlaybackTracker) {
        let Some(bar) = self.marking.bar.clone() else { return };
        let length = self.episode_length(tracker);
        let check = bar.check(length);
        let (Some((start, end)), true) = (bar.stretch(), check != Check::TooShort) else {
            return;
        };
        let kind = bar.kind;
        let replaced = self.aniskip_time(kind, tracker);
        let row = SkipMark {
            anilist_id: self.context.anilist_id,
            episode: self.context.episode.trim().to_owned(),
            kind: kind.api_type().to_owned(),
            start_ms: secs_to_ms(start),
            end_ms: secs_to_ms(end),
            length_ms: secs_to_ms(length),
            marked_at: kaeri_store::now(),
            skip_id: None,
        };
        if on_store(self.store, "save", move |store| store.put_skip_mark(&row)).await.is_none()
        {
            return;
        }
        self.marks.retain(|m| m.interval.kind != kind);
        self.marks.push(MarkedSkip::new(kind, start, end, length));

        // A mark the same as aniskip's confirms it; one that differs replaces it, which is a
        // report of aniskip's.
        let mut confirmed = false;
        // Whether this install had voted on aniskip's time before: AniSkip counts one vote.
        let mut voted = false;
        if let Some(time) = replaced.as_ref().filter(|t| !t.skip_id.is_empty()) {
            confirmed = marking::same((start, end), (time.interval.start, time.interval.end));
            let vote = if confirmed { 1 } else { -1 };
            let id = time.skip_id.clone();
            let at = kaeri_store::now();
            voted = !on_store(self.store, "vote", move |store| {
                store.record_skip_vote(&id, vote, at)
            })
            .await
            .unwrap_or(true);
            if !confirmed {
                self.aniskip.retain(|t| t.skip_id != time.skip_id);
                tracker.set_given(self.given());
            }
        }
        tracker.set_marks(self.marks.clone());

        // What AniSkip is sent of it: once the viewer is asked, on this first Save.
        let theirs =
            replaced.as_ref().map(|t| (t.skip_id.as_str(), (t.interval.start, t.interval.end)));
        let mut requests = self.for_aniskip(kind, (start, end), length, theirs);
        if voted {
            requests.retain(|request| !matches!(request, Request::Vote(_)));
        }
        let requests_empty = requests.is_empty();
        let ask = match self.marking.share {
            Some(true) => {
                self.send(&requests);
                false
            }
            Some(false) => false,
            None => {
                self.marking.unshared = requests;
                self.context.mal_id.is_some()
            }
        };

        // A found stretch saved is confirmed: its kind is skipped by itself for this show from
        // now on, and the card that asked says it was saved.
        let found = self.marking.found_review.take().filter(|(found, _)| *found == kind);
        if found.is_some() {
            let shared = self.marking.sharing() && !requests_empty;
            let (id, episode, api) = (
                self.context.anilist_id,
                self.context.episode.trim().to_owned(),
                kind.api_type(),
            );
            on_store(self.store, "confirm found", move |store| {
                store.confirm_found_skips(id, api)?;
                let now = if shared { state::SHARED } else { state::CONFIRMED };
                store.set_found_skip_state(id, &episode, api, now)
            })
            .await;
            tracker.trust_found(kind, true);
        }

        let notice = match check {
            Check::TooLong { most } => SkipNotice::SavedHere { kind: offered(kind), most },
            _ if confirmed => SkipNotice::Confirmed,
            _ => SkipNotice::Saved { kind: offered(kind) },
        };
        self.marking.bar = None;
        self.say_bar(link, tracker).await;
        match (found, self.episode_number()) {
            (Some((_, FoundFrom::Yes)), Some(episode)) => {
                let saved = SkipPrompt::FoundSaved { kind: offered(kind), episode };
                self.say_prompt(link, Some(saved)).await;
            }
            _ => {
                let _ = link.overlay(Overlay::SkipNotice(notice)).await;
            }
        }
        if ask {
            self.say_prompt(link, Some(SkipPrompt::Share)).await;
        }
    }

    /// The episode's number, for AniSkip and for the found times' card.
    fn episode_number(&self) -> Option<u32> {
        self.context.episode.trim().parse::<u32>().ok()
    }

    /// Yes or Adjust on the found stretch of `kind`: saved as the person's own at once, or the
    /// review opened on it, playing its edges once.
    async fn confirm_found(
        &mut self,
        kind: SkipKind,
        from: FoundFrom,
        link: &dyn PlayerLink,
        tracker: &mut PlaybackTracker,
    ) {
        if self.in_party {
            return;
        }
        let Some(found) = self.found.iter().find(|s| s.kind == kind).copied() else { return };
        let yours = SkipInterval { origin: SkipOrigin::Yours, ..found };
        let mut bar = Marking::review(&yours, false);
        self.marking.found_review = Some((kind, from));
        match from {
            FoundFrom::Yes => {
                self.marking.bar = Some(bar);
                self.save_mark(link, tracker).await;
            }
            FoundFrom::Adjust => {
                if let Some(to) = bar.preview() {
                    let _ = link.seek(to, true).await;
                }
                self.marking.bar = Some(bar);
                self.say_prompt(link, None).await;
                self.say_bar(link, tracker).await;
            }
        }
    }

    /// A new file: the found times kept for this episode, to use once its length is known, and
    /// whether this show's found times are skipped by themselves. `None` where none are used.
    pub(super) async fn found_opened(
        &mut self,
        tracker: &mut PlaybackTracker,
    ) -> Option<Vec<(SkipInterval, f64)>> {
        self.found.clear();
        if !self.settings.find_skip_times || !self.settings.uses_skip_times() {
            return None;
        }
        let (id, episode) = (self.context.anilist_id, self.context.episode.trim().to_owned());
        let (rows, opening, ending) = on_store(self.store, "found", move |store| {
            Ok((
                store.found_skips(id, &episode)?,
                store.found_skip_show(id, SkipKind::Opening.api_type())?,
                store.found_skip_show(id, SkipKind::Ending.api_type())?,
            ))
        })
        .await?;
        tracker.trust_found(SkipKind::Opening, opening.trusted());
        tracker.trust_found(SkipKind::Ending, ending.trusted());
        let kept = rows
            .into_iter()
            .filter(|row| row.state == state::FOUND || row.state == state::SKIPPED)
            .filter_map(|row| {
                let kind = SkipKind::from_api_type(&row.kind)?;
                let interval = SkipInterval {
                    kind,
                    start: ms_to_secs(row.start_ms),
                    end: ms_to_secs(row.end_ms),
                    origin: SkipOrigin::Found,
                };
                Some((interval, ms_to_secs(row.length_ms)))
            })
            .collect();
        Some(kept)
    }

    /// The found times kept for this episode that fit a file `length` seconds long, where this
    /// file has not found its own.
    pub(super) fn use_kept_found(
        &mut self,
        kept: Vec<(SkipInterval, f64)>,
        length: f64,
        tracker: &mut PlaybackTracker,
    ) {
        let mut added = false;
        for (interval, found_on) in kept {
            let fits = (found_on - length).abs() <= kaeri_playback::skip::MARK_LENGTH_TOLERANCE;
            if fits && !self.found.iter().any(|s| s.kind == interval.kind) {
                self.found.push(interval);
                added = true;
            }
        }
        if added {
            tracker.set_found(self.found.clone());
        }
    }

    /// Whether the skip of `kind` in use is a found one.
    fn uses_found(kind: SkipKind, tracker: &PlaybackTracker) -> bool {
        tracker.skips().iter().any(|s| s.kind == kind && s.origin == SkipOrigin::Found)
    }

    /// The playhead was taken past the skip of `kind`, to `to`, by the button or by itself:
    /// where it was a found one, counted, and its card due a little after.
    pub(super) async fn skipped(&mut self, kind: SkipKind, to: f64, tracker: &PlaybackTracker) {
        if !Self::uses_found(kind, tracker) {
            return;
        }
        let (id, episode, api) =
            (self.context.anilist_id, self.context.episode.trim().to_owned(), kind.api_type());
        on_store(self.store, "found skipped", move |store| {
            store.note_found_skip(id, api)?;
            store.set_found_skip_state(id, &episode, api, state::SKIPPED)
        })
        .await;
        self.marking.found_landed = Some((kind, to));
    }

    /// Undo after a skip of `kind`: where it was a found one, the person wanted to watch it.
    /// Its count starts again, and it is offered, not skipped, until trusted again.
    pub(super) async fn found_undone(&mut self, kind: SkipKind, tracker: &mut PlaybackTracker) {
        if !Self::uses_found(kind, tracker) {
            return;
        }
        self.marking.found_landed = None;
        let (id, episode, api) =
            (self.context.anilist_id, self.context.episode.trim().to_owned(), kind.api_type());
        let confirmed = on_store(self.store, "found undone", move |store| {
            store.undo_found_skip(id, api)?;
            store.set_found_skip_state(id, &episode, api, state::REJECTED)?;
            Ok(store.found_skip_show(id, api)?.confirmed)
        })
        .await
        .unwrap_or(false);
        tracker.trust_found(kind, confirmed);
    }

    /// The found times' card for `kind`, if it is to be asked: sharing on, AniSkip with nothing
    /// for this episode, a show it can be shared for, not in a party, nothing else up, and not
    /// asked for this show and kind before. Counted as asked as it shows.
    async fn ask_found(&mut self, kind: SkipKind, link: &dyn PlayerLink) {
        let Some(episode) = self.episode_number() else { return };
        if !self.marking.ask
            || !self.marking.sharing()
            || !self.aniskip.is_empty()
            || self.context.mal_id.is_none()
            || self.in_party
            || self.marking.bar.is_some()
            || self.marking.prompt.is_some()
        {
            tracing::debug!(?kind, "found times' card not asked here");
            return;
        }
        let (id, api) = (self.context.anilist_id, kind.api_type());
        let first =
            on_store(self.store, "ask found", move |store| store.ask_found_skip(id, api))
                .await
                .unwrap_or(false);
        if first {
            let card = SkipPrompt::FoundEdges { kind: offered(kind), episode };
            self.say_prompt(link, Some(card)).await;
        } else {
            tracing::debug!(?kind, "found times' card asked for this show before");
        }
    }

    /// What a save of `kind` from `start` to `end` sends to AniSkip: see
    /// [`aniskip::for_save`]. Nothing for a show with no MyAnimeList entry or an episode with
    /// no number.
    fn for_aniskip(
        &self,
        kind: SkipKind,
        stretch: (f64, f64),
        length: f64,
        theirs: Option<(&str, (f64, f64))>,
    ) -> Vec<Request> {
        let (Some(mal_id), Ok(episode)) =
            (self.context.mal_id, self.context.episode.trim().parse::<u32>())
        else {
            return Vec::new();
        };
        aniskip::for_save(mal_id, episode, kind, stretch, length, theirs)
    }

    /// Queue `requests` for AniSkip and ask for them to be sent.
    fn send(&self, requests: &[Request]) {
        let episode = self.context.episode.trim();
        crate::aniskip::queue(
            &self.http,
            self.store,
            requests,
            Some((self.context.anilist_id, episode)),
        );
    }

    async fn clear_mark(
        &mut self,
        kind: SkipKind,
        link: &dyn PlayerLink,
        tracker: &mut PlaybackTracker,
    ) {
        let (id, episode) = (self.context.anilist_id, self.context.episode.trim().to_owned());
        let api = kind.api_type();
        on_store(self.store, "clear", move |store| store.clear_skip_mark(id, &episode, api))
            .await;
        self.marks.retain(|m| m.interval.kind != kind);
        tracker.set_marks(self.marks.clone());
        if self.marking.bar.as_ref().is_some_and(|bar| bar.editing && bar.kind == kind) {
            self.marking.bar = None;
            self.say_bar(link, tracker).await;
        }
    }

    /// Report aniskip's time of `kind` as wrong: it is not used here again.
    async fn report(
        &mut self,
        kind: SkipKind,
        link: &dyn PlayerLink,
        tracker: &mut PlaybackTracker,
    ) {
        let Some(time) = self.aniskip.iter().find(|t| t.interval.kind == kind).cloned() else {
            return;
        };
        // Sent only with sharing on, and only the first time: AniSkip counts one vote.
        let mut reported = false;
        if !time.skip_id.is_empty() {
            let (id, at) = (time.skip_id.clone(), kaeri_store::now());
            let new = on_store(self.store, "report", move |store| {
                store.record_skip_vote(&id, -1, at)
            })
            .await
            .unwrap_or(false);
            if self.marking.sharing() {
                if new {
                    self.send(&[Request::Vote(Vote {
                        skip_id: time.skip_id.clone(),
                        up: false,
                    })]);
                }
                reported = true;
            }
        }
        self.aniskip.retain(|t| t.interval != time.interval);
        tracker.set_given(self.given());
        if matches!(self.marking.prompt, Some(SkipPrompt::WrongTimes { .. })) {
            self.say_prompt(link, None).await;
        }
        let notice = if reported { SkipNotice::Reported } else { SkipNotice::HiddenHere };
        let _ = link.overlay(Overlay::SkipNotice(notice)).await;
    }

    /// After Undo on a skip of `kind`: where it was aniskip's, ask whether its times are wrong.
    pub(super) async fn undone(
        &mut self,
        kind: SkipKind,
        link: &dyn PlayerLink,
        tracker: &PlaybackTracker,
    ) {
        if self.aniskip_time(kind, tracker).is_some() {
            self.say_prompt(link, Some(SkipPrompt::WrongTimes { kind: offered(kind) })).await;
        }
    }

    /// The viewer sought from `from` to `to`, with the player's own controls.
    pub(super) fn sought(&mut self, from: f64, to: f64) {
        let now = self.marking.now();
        self.marking.jumps.sought(from, to, now);
    }

    /// The playhead moved: Watch the edges goes on, and a skip by hand that has settled may
    /// be asked about.
    pub(super) async fn mark_progress(
        &mut self,
        link: &dyn PlayerLink,
        tracker: &PlaybackTracker,
    ) {
        let position = tracker.position();
        if let Some((kind, landed)) = self.marking.found_landed {
            if position < landed - 1.0 {
                // Gone back: the skip was not kept.
                self.marking.found_landed = None;
            } else if position >= landed + FOUND_PROMPT_DELAY_SECS && !tracker.is_paused() {
                self.marking.found_landed = None;
                self.ask_found(kind, link).await;
            }
        }
        if let Some(bar) = self.marking.bar.as_mut().filter(|bar| bar.previewing()) {
            if let Some(to) = bar.previewed(position) {
                let _ = link.seek(to, true).await;
            }
            if !bar.previewing() {
                self.say_bar(link, tracker).await;
            }
        }
        if !self.marking.ask || self.in_party || self.marking.bar.is_some() {
            return;
        }
        let known = tracker.skips().iter().any(|s| s.kind == SkipKind::Opening);
        let now = self.marking.now();
        if let Some((start, end)) = self.marking.jumps.settled(now, known) {
            self.say_prompt(link, Some(SkipPrompt::WasOpening { start, end })).await;
        }
    }
}
