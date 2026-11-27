//! Casting to a TV with Chromecast built in, as the picker and the card read it. The engine casts
//! (`CastSession`): this holds the cast running, what it last said, the line that counts while
//! it starts, and what the card offers when it ends badly. The card is the remote's face, drawn
//! from [`CastView`] in place of the link's card.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use gtk::glib;
use kaeri_core::ids::AnilistId;
use kaeri_engine::{
    cast::{CastHub, SessionState, SessionView, protocol::VolumeControl},
    engine::RemoteSender,
    remote::{
        protocol::command::RemoteCommand,
        sender::{
            CastCard, CastRouteHint, CastRow, CastSession, CastUndo, CastUpNext, LinkPhase,
            NowCard,
        },
    },
};
use kaeri_i18n::t;
use kaeri_model::Msg;

use super::{Request, Sheet, Tv};

/// What the sender casts through: the engine's hub, and where its commands go.
pub struct Casting {
    pub hub: Arc<CastHub>,
    pub engine: RemoteSender,
    /// What sync hears of this computer's remote key, so it can offer the Remote switch.
    pub this_device: kaeri_engine::syncing::ThisDevice,
}

/// A seconds counter joins the starting line after this long, as under that it is only noise.
pub const COUNT_AFTER: Duration = Duration::from_secs(4);

/// The status line's mark while the app waits on the TV: a small spinner in the dot's place.
pub const SPIN: &str = "spin";

/// Play here carries the TV's place from this far in, as the engine's sender does: nearer the
/// start the episode plays from where history has it.
const CARRY_FROM: f64 = 5.0;

/// Where a cast is: starting, playing, or ended in a way the card keeps and explains.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Starting,
    Playing,
    /// It never started, or the TV stopped it on a stream: the card says why.
    Failed,
    /// It played, then ended from afar: the TV stopped answering, or was stopped on the TV.
    Lost,
}

impl Stage {
    pub fn ended(self) -> bool {
        matches!(self, Self::Failed | Self::Lost)
    }
}

/// What a card that has ended offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Offer {
    ChooseSource,
    PlayHere,
    TryAgain,
    ChooseAnother,
}

/// The cast as the card draws it.
#[derive(Debug, Clone, PartialEq)]
pub struct CastView {
    pub name: String,
    pub stage: Stage,
    /// The line while it starts, with its seconds once it takes a while.
    pub starting: String,
    pub now: Option<NowCard>,
    /// The engine's words for why it ended.
    pub notice: Option<Msg>,
    pub offers: Vec<Offer>,
    /// Whether the TV takes a level: not where its own volume is in charge.
    pub volume: VolumeControl,
    /// Whether Mute is offered: not where the TV's own volume is in charge, or it refused one.
    pub can_mute: bool,
    pub muted: bool,
    /// Up next, while the engine counts down to it.
    pub up_next: Option<CastUpNext>,
    /// An automatic skip, while it can be undone.
    pub undo: Option<CastUndo>,
    /// Whole seconds the TV has waited, while it plays: for the stream, or for word of why the
    /// receiver left.
    pub waiting: Option<u32>,
    /// The connection to the TV closed and is being made again: every control but Stop casting
    /// waits for it.
    pub reconnecting: bool,
}

/// The cast running now, and what it last said.
pub struct CastRun {
    session: Arc<CastSession>,
    /// The picker row it was started from, for Try again.
    key: String,
    request: Request,
    card: CastCard,
    asked: Instant,
    /// When the card's playhead was last said, for the seek line.
    card_at: Instant,
    /// It has played once: the player that handed it over has closed.
    played: bool,
    /// The card was opened again after it ended, so closing it now lets it go.
    reopened: bool,
}

/// What a cast this computer joined plays, by its card: none for another app's cast or an older
/// Kaeri's, which don't say, and none once it ended.
fn joined_now(card: &CastCard) -> Option<&NowCard> {
    card.card.now.as_ref().filter(|_| !card.card.over && card.notice.is_none())
}

/// Whether the TV shows a picture: the engine has said what plays, and the TV has loaded it.
/// `loading` is a start or a reload only; the length is not asked for, since a TV's status
/// after a pause, play or seek can leave it out, and the card must not flip back to starting.
fn playing(card: &CastCard) -> bool {
    card.card.phase == LinkPhase::Connected
        && card.card.now.as_ref().is_some_and(|n| !n.loading)
}

pub fn stage_of(card: &CastCard) -> Stage {
    match &card.notice {
        Some(Msg::NoticeCastLost { .. } | Msg::NoticeCastStoppedElsewhere { .. }) => {
            Stage::Lost
        }
        Some(_) => Stage::Failed,
        None if playing(card) => Stage::Playing,
        None => Stage::Starting,
    }
}

/// Keep in `next`, a card that ended from afar, where the TV got to by `was` read `since`
/// seconds ago, and the episode's length, when it is still the same episode.
fn hold_playhead(was: &CastCard, since: f64, next: &mut CastCard) {
    let Some(before) = was.card.now.as_ref() else { return };
    // The engine's card can drop the episode itself once the TV is gone.
    let now = next.card.now.get_or_insert_with(|| before.clone());
    if before.id != now.id || before.episode != now.episode {
        return;
    }
    let moving = stage_of(was) == Stage::Playing && was.waiting.is_none() && !was.reconnecting;
    now.position = if moving { before.position_after(since) } else { before.position };
    now.duration = now.duration.or(before.duration);
}

/// The line and mark of a cast that plays, as it stands: reconnecting, waiting on the TV,
/// paused, or playing, which reads as `playing` says where the line shows. The mark is a dot's
/// colour, or [`SPIN`] for a spinner while the app waits on the TV.
pub fn playing_line(
    view: &CastView,
    playing: impl FnOnce() -> String,
) -> (String, &'static str) {
    if view.reconnecting {
        return (t::tv_status_reconnecting(&view.name), SPIN);
    }
    match view.waiting {
        // "Waiting for Living room", as a stop on the TV is learned within 4 s; a longer wait for
        // the stream counts its seconds, as the starting line does.
        Some(seconds) if u64::from(seconds) < COUNT_AFTER.as_secs() => {
            (t::tv_waiting_for(&view.name), SPIN)
        }
        Some(seconds) => (t::cast_phase_waiting(&view.name, i64::from(seconds)), SPIN),
        None if view.now.as_ref().is_some_and(|now| now.paused) => {
            (t::cast_status_paused(&view.name), "haze")
        }
        None => (playing(), "water"),
    }
}

/// What a card that ended offers, by the engine's reason.
pub fn offers_for(notice: &Msg) -> Vec<Offer> {
    match notice {
        Msg::NoticeCastRefused { .. }
        | Msg::NoticeCastVideoCodec { .. }
        | Msg::NoticeCastAudioCodec { .. }
        | Msg::NoticeCastContainer { .. }
        | Msg::NoticeCastSegment { .. } => vec![Offer::ChooseSource, Offer::PlayHere],
        Msg::NoticeCastNeverFetches { .. } | Msg::NoticeCastLostStream { .. } => {
            vec![Offer::TryAgain, Offer::PlayHere]
        }
        Msg::NoticeCastDeepLink => vec![Offer::ChooseSource],
        _ => vec![Offer::PlayHere, Offer::ChooseAnother],
    }
}

/// The starting line: the engine's phase line, which counts its own seconds; before it has one,
/// "Starting on Living room", with the seconds once it has taken `COUNT_AFTER`.
fn starting_line(card: &CastCard, waited: Duration) -> String {
    if let Some(said) = &card.phase_line {
        return crate::say::msg(said);
    }
    let line = card
        .card
        .phase_line
        .clone()
        .filter(|l| !l.is_empty())
        .unwrap_or_else(|| t::tv_starting_on(&card.card.tv_name));
    if waited >= COUNT_AFTER {
        t::tv_cast_counting(&line, waited.as_secs() as i64)
    } else {
        line
    }
}

impl Tv {
    /// The Cast rows of the picker: TVs with Chromecast built in that a browse found.
    pub fn cast_rows(&self) -> Vec<CastRow> {
        self.cast_rows.borrow().clone()
    }

    /// A cast is running, or ended and kept on the card.
    pub fn casting(&self) -> bool {
        self.cast.borrow().is_some()
    }

    /// The TV shows a picture, so the card's controls reach it.
    pub fn cast_playing(&self) -> bool {
        self.cast
            .borrow()
            .as_ref()
            .is_some_and(|run| stage_of(&run.card) == Stage::Playing && !run.card.reconnecting)
    }

    /// The connection to the TV closed and is being made again.
    pub fn cast_reconnecting(&self) -> bool {
        self.cast_view().is_some_and(|view| view.reconnecting)
    }

    /// This computer sends the TV the episode, so it must stay awake while the cast runs.
    pub fn cast_relaying(&self) -> bool {
        self.cast.borrow().as_ref().is_some_and(|run| {
            !run.card.card.over && run.card.route == Some(CastRouteHint::ThroughThisDevice)
        })
    }

    pub fn cast_view(&self) -> Option<CastView> {
        let cast = self.cast.borrow();
        let run = cast.as_ref()?;
        let stage = stage_of(&run.card);
        let now = run.card.card.now.clone().or_else(|| Some(self.asked_card(run)));
        Some(CastView {
            name: run.card.card.tv_name.clone(),
            stage,
            starting: starting_line(&run.card, run.asked.elapsed()),
            now,
            notice: run.card.notice.clone(),
            offers: run.card.notice.as_ref().map(offers_for).unwrap_or_default(),
            volume: run.card.volume_control,
            can_mute: run.card.can_mute,
            muted: run.card.muted.unwrap_or(false),
            up_next: run.card.up_next.clone().filter(|_| !stage.ended()),
            undo: run.card.undo.filter(|_| stage == Stage::Playing),
            waiting: run.card.waiting.filter(|_| stage == Stage::Playing),
            reconnecting: run.card.reconnecting && !stage.ended(),
        })
    }

    /// What was asked of the TV, for the card to show before the engine has said what plays.
    fn asked_card(&self, run: &CastRun) -> NowCard {
        let request = &run.request;
        NowCard {
            id: request.id,
            title: request.title.clone(),
            episode_line: t::tv_episode(&request.episode),
            episode: request.episode.clone(),
            loading: true,
            position: request.from.unwrap_or(0.0),
            duration: None,
            paused: false,
            speed: 1.0,
            volume: None,
            audio: Vec::new(),
            subtitles: Vec::new(),
            subtitles_off: false,
            skip: None,
        }
    }

    /// Where the TV's playhead is now: moved on from the card on this computer's clock while it
    /// plays, and held where the card last said once it ends.
    pub fn cast_playhead(&self) -> Option<f64> {
        let cast = self.cast.borrow();
        let run = cast.as_ref()?;
        let now = run.card.card.now.as_ref()?;
        // The clock stops while the TV waits, and while the app can't hear it.
        if stage_of(&run.card) != Stage::Playing
            || run.card.waiting.is_some()
            || run.card.reconnecting
        {
            return Some(now.position);
        }
        Some(now.position_after(run.card_at.elapsed().as_secs_f64()))
    }

    /// The engine's cast session, as the header's button draws it.
    pub fn cast_session(&self) -> Option<SessionView> {
        let sender = self.sender.as_ref()?;
        let _entered = self.handle.enter();
        Some(sender.cast_state())
    }

    /// The session's cast when another device started it and this computer joined it, with the
    /// card that says what plays.
    fn joined(&self) -> Option<(Arc<CastSession>, CastCard)> {
        if self.casting() {
            return None;
        }
        let sender = self.sender.as_ref()?;
        let session = {
            let _entered = self.handle.enter();
            sender.cast_session()?
        };
        let card = session.card();
        joined_now(&card)?;
        Some((session, card))
    }

    /// What a cast another device started plays, for the header's popover to open its remote by.
    pub fn cast_joined_now(&self) -> Option<NowCard> {
        self.joined().and_then(|(_, card)| card.card.now)
    }

    /// Take a cast another device started as this computer's own, for its remote to open: false
    /// when there is none, or it doesn't say what plays.
    pub(super) fn join_cast(&self) -> bool {
        let Some((session, card)) = self.joined() else { return false };
        let Some(key) = self.cast_held() else { return false };
        let Some(now) = joined_now(&card) else { return false };
        let request = Request {
            id: now.id,
            title: now.title.clone(),
            episode: now.episode.clone(),
            from: None,
            from_player: false,
        };
        let turn = self.cast_turn.get() + 1;
        self.cast_turn.set(turn);
        let played = stage_of(&card) == Stage::Playing;
        self.cast.replace(Some(CastRun {
            session: Arc::clone(&session),
            key,
            request,
            card,
            asked: Instant::now(),
            card_at: Instant::now(),
            played,
            reopened: false,
        }));
        self.watch_cast(&session, turn);
        true
    }

    /// The Cast row key of a TV held with nothing cast from here yet: connected first from the
    /// header's button.
    pub(super) fn cast_held(&self) -> Option<String> {
        if self.casting() {
            return None;
        }
        let session = self.cast_session()?;
        let held = matches!(session.state, SessionState::Connected | SessionState::Playing);
        session.device.filter(|_| held).map(|d| format!("cast:{}", d.id)) // i18n-skip
    }

    /// A Cast row chosen in the header's chooser: what was asked plays there, else the TV is
    /// connected to with nothing playing yet.
    pub fn cast_connect(&self, row: &CastRow) {
        if self.request.borrow().is_some() {
            self.choose_cast(row);
            return;
        }
        let Some(sender) = self.sender.clone() else { return };
        let connected = {
            let _entered = self.handle.enter();
            sender.connect_cast(&row.key, None)
        };
        if let Err(problem) = connected {
            self.problem.replace(Some(problem));
        }
        self.changed();
    }

    /// Stop casting from the header's popover: the cast from here, or the TV held with
    /// nothing playing.
    pub fn cast_disconnect(&self) {
        if self.casting() {
            self.cast_stop();
            return;
        }
        if let Some(sender) = &self.sender {
            let _entered = self.handle.enter();
            sender.disconnect_cast();
        }
        self.changed();
    }

    /// What the system's media controls show of a cast: the episode and where the TV's
    /// playhead is, while the cast runs and has not ended.
    pub fn cast_media(&self) -> Option<(NowCard, f64)> {
        let view = self.cast_view().filter(|v| !v.stage.ended())?;
        let now = view.now?;
        let at = self.cast_playhead().unwrap_or(now.position);
        Some((now, at))
    }

    /// What the sidebar and the header button say of the cast: the words, the dot, and whether
    /// it is down.
    pub fn cast_standing(&self) -> Option<(String, &'static str, bool)> {
        let view = self.cast_view()?;
        Some(match view.stage {
            Stage::Starting => (view.starting, "haze", false),
            Stage::Playing => {
                let (line, mark) = playing_line(&view, || t::tv_status_playing(&view.name));
                // The session button's dot stays a dot, haze while the app waits on the TV.
                (line, if mark == SPIN { "haze" } else { mark }, false)
            }
            Stage::Failed => (t::tv_cast_couldnt_start(&view.name), "vermilion", true),
            Stage::Lost => (t::tv_cast_stopped(), "vermilion", true),
        })
    }

    /// A Cast row chosen: the request is cast to that TV, and the card is up at once.
    pub fn choose_cast(&self, row: &CastRow) {
        let Some(request) = self.request.borrow().clone() else { return };
        self.start_cast(row.key.clone(), request);
    }

    pub(super) fn start_cast(&self, key: String, request: Request) {
        let Some(sender) = self.sender.clone() else { return };
        let started = {
            let _entered = self.handle.enter();
            sender.cast(&key, request.id, request.episode.clone(), request.from)
        };
        let session = match started {
            Ok(session) => session,
            Err(problem) => {
                self.problem.replace(Some(problem));
                self.changed();
                return;
            }
        };
        self.request.replace(None);
        self.problem.replace(None);
        // What the TV was taken from is over: its notice and Play here go.
        self.taken.replace(None);
        if let Some(started) = self.cast_started.borrow().as_ref() {
            started();
        }
        let turn = self.cast_turn.get() + 1;
        self.cast_turn.set(turn);
        let card = session.card();
        self.cast.replace(Some(CastRun {
            session: Arc::clone(&session),
            key,
            request,
            card,
            asked: Instant::now(),
            card_at: Instant::now(),
            played: false,
            reopened: false,
        }));
        self.sheet.set(Some(Sheet::Remote));
        self.watch_cast(&session, turn);
        self.changed();
    }

    /// Read the card again whenever the engine says it may read differently.
    fn watch_cast(&self, session: &CastSession, turn: u64) {
        let mut changes = session.changes();
        let (tx, rx) = async_channel::unbounded::<()>();
        self.handle.spawn(async move {
            while changes.changed().await.is_ok() {
                if tx.send(()).await.is_err() {
                    return;
                }
            }
        });
        let me = self.me.clone();
        glib::spawn_future_local(async move {
            while rx.recv().await.is_ok() {
                // A burst of changes, read once.
                while rx.try_recv().is_ok() {}
                let Some(tv) = me.upgrade() else { return };
                if !tv.read_cast(turn) {
                    return;
                }
            }
        });
    }

    /// Take the engine's card. False once this cast is not the one shown.
    fn read_cast(&self, turn: u64) -> bool {
        if self.cast_turn.get() != turn {
            return false;
        }
        let (stage, notice, hand_off) = {
            let mut cast = self.cast.borrow_mut();
            let Some(run) = cast.as_mut() else { return false };
            let mut next = run.session.card();
            // A cast that played and ended from afar keeps where the TV got to and the
            // episode's length: the engine's card forgets both once the TV is gone, on every read.
            if stage_of(&next) == Stage::Lost {
                hold_playhead(&run.card, run.card_at.elapsed().as_secs_f64(), &mut next);
            }
            if next.card.now != run.card.card.now {
                run.card_at = Instant::now();
            }
            run.card = next;
            let stage = stage_of(&run.card);
            let hand_off = stage == Stage::Playing && !run.played && run.request.from_player;
            run.played |= stage == Stage::Playing;
            (stage, run.card.notice.clone(), hand_off)
        };
        if hand_off && let Some(handed_off) = self.handed_off.borrow().as_ref() {
            handed_off();
        }
        match (stage, &notice) {
            // Stopped from the card, or played out: nothing to say.
            (Stage::Starting | Stage::Playing, _) if self.cast_over() => self.finish_cast(),
            (_, Some(taken @ Msg::NoticeCastTakenOver { .. })) => self.ended_away(taken),
            // Lost, or stopped on the TV, with the controls closed: the bar goes and the notice
            // offers Play here where it was. With them open, they say it under Casting stopped.
            (Stage::Lost, Some(ended)) if self.sheet.get() != Some(Sheet::Remote) => {
                self.ended_away(ended);
            }
            _ => self.changed(),
        }
        true
    }

    /// The engine ended the cast with nothing to say.
    fn cast_over(&self) -> bool {
        self.cast.borrow().as_ref().is_some_and(|run| run.card.card.over)
    }

    /// Let the cast go, and close the card.
    fn finish_cast(&self) {
        self.drop_cast();
        if self.sheet.get() == Some(Sheet::Remote) {
            self.sheet.set(None);
        }
        self.changed();
    }

    fn drop_cast(&self) -> Option<CastRun> {
        self.cast_turn.set(self.cast_turn.get() + 1);
        self.cast.take()
    }

    /// The cast ended from afar: someone else started something on the TV, it was stopped on
    /// the TV, or the TV was lost. The card closes, and a notice offers to carry on here.
    fn ended_away(&self, notice: &Msg) {
        let Some(run) = self.drop_cast() else { return };
        let (id, episode) = Self::episode_of(&run);
        self.taken.replace(Some((id, episode)));
        if self.sheet.get() == Some(Sheet::Remote) {
            self.sheet.set(None);
        }
        self.changed();
        if let Some(say) = self.on_ended_away.borrow().as_ref() {
            say(&crate::say::msg(notice));
        }
    }

    /// What the notice of a cast that ended from afar offers: the episode, to play on this
    /// computer.
    pub fn take_ended(&self) -> Option<(AnilistId, String)> {
        self.taken.take()
    }

    /// Run `f` with the words of the engine's note on the TV held, each time it reads anew: an
    /// information notice, with no button.
    pub fn on_cast_note(&self, f: impl Fn(&str) + 'static) {
        self.on_cast_note.replace(Some(Box::new(f)));
    }

    /// The engine's note on the TV held, said when it first reads differently: Kaeri's receiver
    /// didn't start, so the TV plays this cast in its own player, or it shows plain subtitles.
    pub(super) fn say_cast_note(&self) {
        // A cast from Play on TV holds no session: its card has the note.
        let card = self.cast.borrow().as_ref().and_then(|run| {
            let _entered = self.handle.enter();
            run.session.card().note
        });
        let note = card.or_else(|| self.cast_session().and_then(|session| session.note));
        if *self.cast_noted.borrow() == note {
            return;
        }
        self.cast_noted.replace(note.clone());
        if let (Some(note), Some(say)) = (note, self.on_cast_note.borrow().as_ref()) {
            say(&crate::say::msg(&note));
        }
    }

    /// Run `f` with the words of the notice when a cast ends from afar.
    pub fn on_ended_away(&self, f: impl Fn(&str) + 'static) {
        self.on_ended_away.replace(Some(Box::new(f)));
    }

    fn episode_of(run: &CastRun) -> (AnilistId, String) {
        run.card.card.now.as_ref().map_or_else(
            || (run.request.id, run.request.episode.clone()),
            |now| (now.id, now.episode.clone()),
        )
    }

    /// Send the TV a command from the card: the engine's playback, or the TV's own volume.
    pub fn cast_send(&self, command: RemoteCommand) {
        if let Some(run) = self.cast.borrow().as_ref() {
            let _entered = self.handle.enter();
            run.session.command(command);
        }
    }

    /// Stop casting from the card. The person chose it, so there is nothing to say.
    pub fn cast_stop(&self) {
        let Some(run) = self.drop_cast() else { return };
        {
            let _entered = self.handle.enter();
            run.session.stop();
        }
        if self.sheet.get() == Some(Sheet::Remote) {
            self.sheet.set(None);
        }
        self.changed();
    }

    /// Play here: the cast stops and the episode is the caller's to play on this computer, from
    /// where the TV had it (`None` near the start, or when the TV never said).
    pub fn cast_play_here(&self) -> Option<(AnilistId, String, Option<f64>)> {
        let run = self.drop_cast()?;
        let (id, episode) = Self::episode_of(&run);
        let at = if stage_of(&run.card).ended() {
            run.card.card.now.as_ref().map(|now| now.position).filter(|p| *p >= CARRY_FROM)
        } else {
            let _entered = self.handle.enter();
            run.session.play_here()
        };
        if self.sheet.get() == Some(Sheet::Remote) {
            self.sheet.set(None);
        }
        self.changed();
        Some((id, episode, at))
    }

    /// Try again: the same episode to the same TV, from where it got to. The card starts over.
    pub fn cast_try_again(&self) {
        let again = {
            let mut cast = self.cast.borrow_mut();
            let Some(run) = cast.as_mut() else { return };
            let _entered = self.handle.enter();
            let again = run.session.try_again();
            if again {
                run.asked = Instant::now();
                run.played = false;
                run.reopened = false;
            }
            again
        };
        if again {
            self.read_cast(self.cast_turn.get());
            return;
        }
        let Some(run) = self.drop_cast() else { return };
        self.start_cast(run.key, run.request);
    }

    /// Up next's Play now: the next episode plays on the TV at once.
    pub fn cast_play_next(&self) {
        if let Some(run) = self.cast.borrow().as_ref() {
            let _entered = self.handle.enter();
            run.session.play_next();
        }
    }

    /// Up next's Cancel: the episode doesn't roll on.
    pub fn cast_cancel_next(&self) {
        if let Some(run) = self.cast.borrow().as_ref() {
            let _entered = self.handle.enter();
            run.session.cancel_next();
        }
    }

    /// Undo on the note an automatic skip leaves: back to where it skipped from.
    pub fn cast_undo_skip(&self) {
        if let Some(run) = self.cast.borrow().as_ref() {
            let _entered = self.handle.enter();
            run.session.undo_skip();
        }
    }

    /// Each tick of the card: while the engine's card counts seconds by itself (the phase line,
    /// the Undo note), it is read again, as nothing else says it changed.
    pub fn cast_tick(&self) {
        let ticking = self.cast.borrow().as_ref().is_some_and(|run| run.session.ticking());
        if ticking {
            self.read_cast(self.cast_turn.get());
        }
    }

    /// Choose another TV: the picker, with the same episode to cast.
    pub fn cast_choose_another(&self) {
        let Some(run) = self.drop_cast() else { return };
        self.request.replace(Some(run.request));
        self.sheet.set(Some(Sheet::Picker));
        self.changed();
    }

    /// Choose source: the card goes, and the episode is the caller's to list sources for.
    pub fn cast_choose_source(&self) -> Option<(AnilistId, String)> {
        let run = self.drop_cast()?;
        if self.sheet.get() == Some(Sheet::Remote) {
            self.sheet.set(None);
        }
        self.changed();
        Some(Self::episode_of(&run))
    }

    /// The card is opened again from the header button.
    pub(super) fn cast_reopened(&self) {
        if let Some(run) = self.cast.borrow_mut().as_mut() {
            run.reopened = true;
        }
    }

    /// The sheet closed with a cast on it: running, it goes on. One lost or stopped on the TV
    /// goes, as its controls said why. One that never started stays for the bar to say so,
    /// until the card has been opened again and closed.
    pub(super) fn cast_sheet_closed(&self) {
        let dismissed = self.cast.borrow().as_ref().is_some_and(|run| {
            let stage = stage_of(&run.card);
            stage == Stage::Lost || (stage.ended() && run.reopened)
        });
        if dismissed {
            self.drop_cast();
        }
    }
}

/// Under a Chromecast row in the picker: the stream plays through this computer. A TV that fetches
/// it by itself has no line yet.
pub fn route_line(row: &CastRow) -> Option<String> {
    (row.route_hint == CastRouteHint::ThroughThisDevice).then(t::tv_cast_through_computer)
}

/// The line over Up next: its count while it runs, else Up next alone.
pub fn up_next_line(next: &CastUpNext) -> String {
    next.seconds_left
        .map_or_else(t::player_up_next, |left| t::tv_cast_up_next_in(i64::from(left)))
}

/// The next episode as Up next names it: "Episode 8 · A Town Without Night".
pub fn up_next_episode(next: &CastUpNext) -> String {
    match next.title.as_deref().filter(|t| !t.is_empty()) {
        Some(title) => t::cast_episode_titled(&next.number, title),
        None => t::cast_episode(&next.number),
    }
}

/// The note an automatic skip leaves on the card.
pub fn skipped_line(undo: &CastUndo) -> String {
    match undo.kind {
        kaeri_model::playback::SkipKind::Opening => t::player_skipped_opening(),
        kaeri_model::playback::SkipKind::Ending => t::player_skipped_ending(),
        kaeri_model::playback::SkipKind::Recap => t::player_skipped_recap(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_card_that_ended_offers_by_why() {
        use Offer::*;
        let name = || "Living room".to_owned();
        let codec = Msg::NoticeCastVideoCodec { name: name(), codec: "AV1".into() };
        assert_eq!(
            offers_for(&Msg::NoticeCastRefused { name: name() }),
            vec![ChooseSource, PlayHere]
        );
        assert_eq!(offers_for(&codec), vec![ChooseSource, PlayHere]);
        assert_eq!(
            offers_for(&Msg::NoticeCastContainer { name: name() }),
            vec![ChooseSource, PlayHere]
        );
        assert_eq!(
            offers_for(&Msg::NoticeCastNeverFetches { name: name() }),
            vec![TryAgain, PlayHere]
        );
        assert_eq!(
            offers_for(&Msg::NoticeCastNotSameNetwork { name: name() }),
            vec![PlayHere, ChooseAnother]
        );
        assert_eq!(
            offers_for(&Msg::NoticeCastSegment { name: name() }),
            vec![ChooseSource, PlayHere]
        );
        assert_eq!(
            offers_for(&Msg::NoticeCastLostStream { name: name() }),
            vec![TryAgain, PlayHere]
        );
        assert_eq!(offers_for(&Msg::NoticeCastNoDevice), vec![PlayHere, ChooseAnother]);
        assert_eq!(offers_for(&Msg::NoticeCastDeepLink), vec![ChooseSource]);
        assert_eq!(
            offers_for(&Msg::NoticeCastLost { name: name() }),
            vec![PlayHere, ChooseAnother]
        );
    }

    fn card(loading: bool, duration: Option<f64>, paused: bool) -> CastCard {
        use kaeri_engine::remote::{protocol::state::Screen, sender::RemoteCard};
        CastCard {
            card: RemoteCard {
                tv_name: "Living room".into(),
                phase: LinkPhase::Connected,
                phase_line: None,
                screen: Screen::Home,
                now: Some(NowCard {
                    id: AnilistId(1),
                    title: "Glasswing Ferry".into(),
                    episode: "7".into(),
                    episode_line: "Episode 7".into(),
                    loading,
                    position: 754.0,
                    duration,
                    paused,
                    speed: 1.0,
                    volume: None,
                    audio: Vec::new(),
                    subtitles: Vec::new(),
                    subtitles_off: false,
                    skip: None,
                }),
                party: None,
                rev: 0,
                over: false,
                reached: true,
            },
            volume_control: VolumeControl::Level,
            can_mute: true,
            muted: None,
            notice: None,
            phase_line: None,
            up_next: None,
            undo: None,
            route: None,
            waiting: None,
            reconnecting: false,
            note: None,
        }
    }

    fn view(paused: bool, waiting: Option<u32>) -> CastView {
        CastView {
            name: "Living room".into(),
            stage: Stage::Playing,
            starting: String::new(),
            now: card(false, Some(1440.0), paused).card.now,
            notice: None,
            offers: Vec::new(),
            volume: VolumeControl::Level,
            can_mute: true,
            muted: false,
            up_next: None,
            undo: None,
            waiting,
            reconnecting: false,
        }
    }

    #[test]
    fn a_paused_cast_reads_as_paused() {
        let playing = || t::tv_status_playing("Living room");
        assert_eq!(playing_line(&view(false, None), playing), (playing(), "water"));
        assert_eq!(
            playing_line(&view(true, None), playing),
            (t::cast_status_paused("Living room"), "haze")
        );
        // The bottom bar's own words while it plays: only the name.
        assert_eq!(
            playing_line(&view(true, None), || "Living room".into()),
            (t::cast_status_paused("Living room"), "haze")
        );
    }

    #[test]
    fn a_cast_waiting_on_the_tv_says_so_and_counts_past_four_seconds() {
        // As after a stop on the TV, while the engine hears why the receiver left: up to 4 s.
        let playing = || t::tv_status_playing("Living room");
        let line = playing_line(&view(false, Some(3)), playing);
        assert_eq!(line, (t::tv_waiting_for("Living room"), SPIN));
        assert_ne!(line.0, playing());
        // A longer wait for the stream counts, as the starting line does.
        assert_eq!(
            playing_line(&view(false, Some(6)), playing),
            (t::cast_phase_waiting("Living room", 6), SPIN)
        );
    }

    #[test]
    fn a_dropped_connection_reads_as_reconnecting_over_every_other_line() {
        let playing = || t::tv_status_playing("Living room");
        for (paused, waiting) in [(false, None), (true, None), (false, Some(2))] {
            let mut view = view(paused, waiting);
            view.reconnecting = true;
            assert_eq!(
                playing_line(&view, playing),
                (t::tv_status_reconnecting("Living room"), SPIN)
            );
        }
        let mut card = card(false, Some(1440.0), false);
        card.reconnecting = true;
        // Still a cast that plays: the engine only waits on the TV.
        assert_eq!(stage_of(&card), Stage::Playing);
    }

    #[test]
    fn a_stop_on_the_tv_ends_as_casting_stopped_with_play_here() {
        let mut card = card(false, Some(1440.0), false);
        card.notice = Some(Msg::NoticeCastStoppedElsewhere { name: "Living room".into() });
        assert_eq!(stage_of(&card), Stage::Lost);
        assert!(Stage::Lost.ended());
        assert_eq!(
            offers_for(card.notice.as_ref().expect("a notice")),
            vec![Offer::PlayHere, Offer::ChooseAnother]
        );
        card.notice = Some(Msg::NoticeCastLost { name: "Living room".into() });
        assert_eq!(stage_of(&card), Stage::Lost);
    }

    #[test]
    fn a_cast_lost_keeps_where_the_tv_got_to_and_the_length() {
        let ended = || {
            let mut card = card(false, None, false);
            card.notice = Some(Msg::NoticeCastLost { name: "Living room".into() });
            if let Some(now) = card.card.now.as_mut() {
                now.position = 492.0;
            }
            card
        };
        let playing = card(false, Some(1440.0), false);
        let mut next = ended();
        hold_playhead(&playing, 5.0, &mut next);
        let now = next.card.now.expect("the episode");
        assert!((now.position - 759.0).abs() < 0.01, "{}", now.position);
        assert_eq!(now.duration, Some(1440.0));
        // Reconnecting, the clock had stopped where the TV was last heard.
        let mut reconnecting = card(false, Some(1440.0), false);
        reconnecting.reconnecting = true;
        let mut next = ended();
        hold_playhead(&reconnecting, 20.0, &mut next);
        assert_eq!(next.card.now.map(|now| now.position), Some(754.0));
        // Each later read of the lost card keeps what the first one held.
        let mut held = ended();
        hold_playhead(&playing, 5.0, &mut held);
        let mut again = ended();
        hold_playhead(&held, 30.0, &mut again);
        let now = again.card.now.expect("the episode");
        assert!((now.position - 759.0).abs() < 0.01, "{}", now.position);
        assert_eq!(now.duration, Some(1440.0));
        // A lost card without the episode takes the one the TV played.
        let mut bare = ended();
        bare.card.now = None;
        hold_playhead(&held, 30.0, &mut bare);
        let now = bare.card.now.expect("the episode");
        assert!((now.position - 759.0).abs() < 0.01, "{}", now.position);
        assert_eq!(now.duration, Some(1440.0));
    }

    #[test]
    fn a_joined_cast_opens_its_remote_only_when_it_says_what_plays() {
        let playing = card(false, Some(1440.0), false);
        assert_eq!(joined_now(&playing).map(|now| now.episode.as_str()), Some("7"));
        let mut unsaid = playing.clone();
        unsaid.card.now = None;
        assert!(joined_now(&unsaid).is_none(), "another app's cast");
        let mut over = playing.clone();
        over.card.over = true;
        assert!(joined_now(&over).is_none());
        let mut taken = playing;
        taken.notice = Some(Msg::NoticeCastTakenOver { name: "Living room".into() });
        assert!(joined_now(&taken).is_none());
    }

    #[test]
    fn a_tv_echo_without_the_length_keeps_the_card_playing() {
        assert_eq!(stage_of(&card(false, Some(1440.0), false)), Stage::Playing);
        // A pause's echo, a seek's or a buffering change's can come without the media object.
        assert_eq!(stage_of(&card(false, None, true)), Stage::Playing);
        assert_eq!(stage_of(&card(false, None, false)), Stage::Playing);
        // Only a start or a reload is starting.
        assert_eq!(stage_of(&card(true, Some(1440.0), false)), Stage::Starting);
    }
}
