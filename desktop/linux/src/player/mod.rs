//! The player: inside the main window, over the page it came from. The video is the bottom
//! layer, mpv drawing into a `GtkGLArea` through its render API; the controls are native
//! widgets over it on a 35% dim of the ground. They fade out after two seconds idle and are then
//! unmapped, so the compositor can scan the video out with nothing over it.
//!
//! The engine drives mpv through `kaeri-mpv-host`: its ops are applied as they come, but
//! only once the GL renderer exists, since a file loaded before that would find no video output.

use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    sync::Arc,
    time::{Duration, Instant},
};

use adw::prelude::*;
use gtk::{gdk, gio, glib};
use kaeri_core::{ids::AnilistId, media::Translation};
use kaeri_engine::engine::Engine;
use kaeri_i18n::t;
use kaeri_model::{
    command::{Command, PlayerCommand},
    event::Event,
    hosted::{
        MenuChoices, MenuEpisode, MenuSource, PlayerChoice, PlayerOp, PlayerUi, TrackKind,
    },
    input::{PadContext, PlayerCard, PlayerFocus, PlayerPanel},
    playback::{PlaybackId, SkipKind, SkipMarkView, SkipPrompt, TimelineMark},
};
use kaeri_mpv_host::{HostEvent, Player, PlayerOptions, PlayerState};

use crate::{
    mpris::{Action as MediaAction, Mpris, Track},
    say, video, widgets,
    window::{Ctx, Shell, Size},
};

mod chat;
mod clock;
mod coming;
mod marking;
#[cfg(feature = "fixtures")]
mod panel_demo;
mod panel_state;
mod side_panel;
mod start_model;
mod start_view;
mod stats;
mod style_panel;
mod timeline;
pub(crate) mod tracks;

use panel_state::{EpisodeAction, PanelTab, Phase, Playhead, SwitchTarget};
use side_panel::{Act, SidePanel};
use timeline::{Marked, Preview};

/// The mini player's size.
const MINI: (i32, i32) = (480, 270);
/// The window's smallest size once the mini player has given it back.
const MINI_MIN: (i32, i32) = (360, 360);

/// The window as it was before the mini player: its size, and whether it filled the screen.
#[derive(Clone, Copy)]
struct Restore {
    width: i32,
    height: i32,
    maximized: bool,
    fullscreen: bool,
}

/// Whether a mini player toggle may go ahead, `since` the last one.
fn mini_ready(since: Option<Duration>) -> bool {
    since.is_none_or(|since| since >= MINI_SETTLE)
}

/// The frame to bring back after the mini player. A quick toggle finds the window still on its
/// way back from the mini size, which GTK reports as the default size and which is no frame
/// worth keeping: the one kept from before stands instead.
fn frame_to_keep(now: Restore, kept: Option<Restore>) -> Restore {
    let small = now.width <= MINI.0 && now.height <= MINI_MIN.1;
    match kept {
        Some(kept) if small => kept,
        _ => now,
    }
}

/// How long a mini player toggle is left to land before the next one counts.
const MINI_SETTLE: Duration = Duration::from_millis(400);
/// How long after a mini player toggle the window may still be on its way to its size.
const SETTLE: Duration = Duration::from_secs(2);

/// How long the controls stay up with nothing happening.
const IDLE: Duration = Duration::from_secs(2);
/// How long after a roll-on its start still counts as one: the engine stages the next episode
/// at once, so a roll-on older than this never led to a start.
const ROLL_ON_FRESH: Duration = Duration::from_secs(10);

/// The controls' fades, in and out: the fade-in and fade-out tokens.
const FADE_IN_MS: u32 = 200;
const FADE_OUT_MS: u32 = 300;
/// The speed menu's speeds, which `[` and `]` step along.
const SPEEDS: [f64; 6] = [0.5, 0.75, 1.0, 1.25, 1.5, 2.0];
/// How long a speed asked for stands in for mpv's report of it.
const SPEED_ASKED: Duration = Duration::from_millis(1500);

/// The speed a step starts from: the one asked for last while it is recent, else mpv's.
fn step_from(asked: Option<(f64, Duration)>, reported: f64) -> f64 {
    match asked {
        Some((speed, ago)) if ago < SPEED_ASKED => speed,
        _ => reported,
    }
}

/// The speed one step faster or slower than `now`, if there is one.
fn next_speed(now: f64, faster: bool) -> Option<f64> {
    if faster {
        SPEEDS.iter().copied().find(|s| *s > now + 0.001)
    } else {
        SPEEDS.iter().rev().copied().find(|s| *s < now - 0.001)
    }
}

/// The player page and what drives it.
pub struct PlayerView {
    pub widget: gtk::Widget,
    me: Weak<Self>,
    ctx: Rc<Ctx>,
    shell: RefCell<Weak<Shell>>,
    player: Option<Rc<Player>>,
    /// Whether mpv can draw: until then the engine's ops wait here.
    ready: Rc<Cell<bool>>,
    waiting: Rc<RefCell<Vec<(PlaybackId, PlayerOp)>>>,
    gl: Rc<Cell<Option<(i32, i32, bool)>>>,
    ui: Ui,
    shown: Cell<bool>,
    idle: RefCell<Option<glib::SourceId>>,
    fade: RefCell<Option<adw::TimedAnimation>>,
    /// The playhead's own updates are held off while a drag on the timeline seeks.
    held_until: Cell<Instant>,
    /// The whole second the seek bar's spoken value was last set for.
    said_second: Cell<i64>,
    state: RefCell<PlayerState>,
    over_bar: Cell<bool>,
    /// A screenshot run's: no Up next card, for a fixture clip short enough to be near its
    /// end from the start.
    pub hold_up_next: Cell<bool>,
    inhibit: Cell<Option<u32>>,
    /// The Up next card's plan, from the engine's word and what the viewer did with it.
    coming: RefCell<Option<coming::Coming>>,
    menus: RefCell<Option<Menus>>,
    /// The chapters changed or a file loaded: read them again once the file has a duration.
    chapters_due: Cell<bool>,
    /// The openings and endings the engine marked, once it has said; until then the file's
    /// chapters stand in.
    engine_marks: RefCell<Option<Vec<timeline::Segment>>>,
    preview: Rc<Preview>,
    /// The window as it was, while the mini player stands in for it.
    mini: Cell<Option<Restore>>,
    /// The frame kept for the last mini player, for a toggle that comes before the window has
    /// settled.
    frame: Cell<Option<(Restore, Instant)>>,
    /// When the mini player last toggled.
    mini_at: Cell<Option<Instant>>,
    /// The start in progress, what the starting screen says of it, since when it has waited
    /// and the clock that moves it on.
    start: RefCell<Option<start_model::Start>>,
    /// On the starting screen's clock: the still clock in a Fixtures build's still shot.
    start_since: Cell<Option<clock::Moment>>,
    start_tick: RefCell<Option<glib::SourceId>>,
    /// When the app or the engine rolled on to the next episode, until that start begins.
    roll_on: Cell<Option<Instant>>,
    /// The roll-on the start on screen came from: its screen waits 600 ms before it shows.
    start_roll: Cell<Option<Instant>>,
    /// Whether that screen is still held back, only its ground showing.
    start_held: Cell<bool>,
    /// When the held screen showed, for its 0.6 s hold.
    start_shown: Cell<Option<Instant>>,
    /// The last frame and Up next fading to the ground.
    start_ground: RefCell<Option<adw::TimedAnimation>>,
    /// The wait before the held screen shows, and a first frame's wait for the screen's hold.
    start_reveal: RefCell<Option<glib::SourceId>>,
    start_leave: RefCell<Option<glib::SourceId>>,
    /// Where the playback resumed, while "Resumed at" shows: it goes 6 s of playback later.
    resumed_from: Cell<Option<f64>>,
    /// The last automatic skip, while Undo is offered for it.
    undo: Cell<Option<Undo>>,
    /// Since when the skip offer has shown, for a controller's A to take it only while it is new.
    skip_since: Cell<Option<Instant>>,
    /// Marking skip times: the bar the engine describes, the prompt up, and the times the
    /// Skip times list shows. See `marking.rs`.
    marking: RefCell<Option<SkipMarkView>>,
    skip_prompt: Cell<Option<SkipPrompt>>,
    prompt_timer: RefCell<Option<glib::SourceId>>,
    /// A shot's prompt, kept up whatever the engine says.
    prompt_held: Cell<bool>,
    timeline_marks: RefCell<Vec<TimelineMark>>,
    sharing: Cell<bool>,
    /// The share question or the Skip times list, while one is up.
    skip_dialog: RefCell<Option<adw::Dialog>>,
    /// The Subtitles and Audio menu's radio actions, set to the file's tracks as it opens.
    subtitle_action: gio::SimpleAction,
    audio_action: gio::SimpleAction,
    /// The other menus' radio and check actions, set to what the engine says is chosen.
    episode_action: gio::SimpleAction,
    source_action: gio::SimpleAction,
    upscaling_action: gio::SimpleAction,
    night_action: gio::SimpleAction,
    /// Stats for nerds, up or away: the More menu's check, I and the panel's close button.
    stats_action: gio::SimpleAction,
    speed_action: gio::SimpleAction,
    /// Sub or Dub, at the top of the Subtitles and Audio menu when the title has a dub.
    translation_action: gio::SimpleAction,
    /// The title playing, and whether it has a dub, once the engine has said.
    title_id: Cell<Option<AnilistId>>,
    /// The playing title's episode table, for the Episodes panel's synopses, and the show's own
    /// spoiler protection when it sets one.
    table: RefCell<Vec<kaeri_model::episodes::EpisodeRow>>,
    own_spoilers: Cell<Option<bool>>,
    /// Whether the episode is far enough on (85%) for the More menu to offer its discussion.
    late: Cell<bool>,
    has_dub: Cell<Option<bool>>,
    /// How far the title's dub goes, for the panel's Dubbed.
    dub: RefCell<Option<kaeri_model::episodes::DubExtent>>,
    /// The episode the panel last asked the sources about.
    panel_asked: RefCell<Option<(AnilistId, String)>>,
    /// The title whose start found its source, and the source's name: a fast start says it
    /// before the player is told the title starts, and the panel's Playing now keeps it.
    found_in: RefCell<Option<(AnilistId, String)>>,
    /// Where a switch to another source loads at, while the picture is held for it.
    hold_at: Cell<Option<f64>>,
    held_fade: RefCell<Option<adw::TimedAnimation>>,
    /// For the panel's scripted switches: the last file the engine loaded, to load it again.
    #[cfg(feature = "fixtures")]
    last_load: RefCell<Option<(PlaybackId, String, Vec<kaeri_model::hosted::MpvOption>)>>,
    /// The episode on screen, and its translation, for its line and the Up Next card's.
    episode_line: RefCell<(String, Option<String>, Option<Translation>)>,
    /// The speed mpv plays at, as the speed button and the media controls show it.
    speed: Cell<f64>,
    /// The speed asked for last and when, until mpv has had time to report it.
    speed_asked: Cell<Option<(f64, Instant)>>,
    /// Since when the stream has stalled, while the card says so, and the card's tick.
    stall_since: Cell<Option<Instant>>,
    stall_tick: RefCell<Option<glib::SourceId>>,
    /// The media controls on the session bus, once served.
    mpris: RefCell<Option<Rc<Mpris>>>,
    /// The watch party's panel, on the people button.
    party: Rc<crate::party::panel::Panel>,
    /// The room around the player: the column beside the video, the strip and the banner.
    pub(crate) room: Rc<crate::room::Room>,
    stats: Rc<stats::Stats>,
}

/// An automatic skip that can still be undone: where it started, when, and where it landed.
#[derive(Clone, Copy)]
struct Undo {
    to: f64,
    since: Instant,
    landed: Option<f64>,
    /// 2 while a screen reader runs, which doubles how long Undo stays, else 1.
    scale: u32,
}

impl Undo {
    /// Undo is offered for 6 s of playback after the skip, and at most 15 s while paused; twice that
    /// with a screen reader.
    fn open(&self, position: Option<f64>) -> bool {
        let played = match (self.landed, position) {
            (Some(landed), Some(now)) => now - landed,
            _ => 0.0,
        };
        played < UNDO_PLAYED * f64::from(self.scale)
            && self.since.elapsed() < UNDO_MOST * self.scale
    }
}

/// How long Undo stays after an automatic skip, in seconds of playback, and at most.
const UNDO_PLAYED: f64 = 6.0;
const UNDO_MOST: Duration = Duration::from_secs(15);

/// How long "Resumed at" stays, in seconds of playback.
const RESUMED_FOR: f64 = 6.0;

/// The widgets the player updates.
/// The episodes, sources and choices the menus offer, and whether they are open.
type Menus = (Vec<MenuEpisode>, Vec<MenuSource>, MenuChoices, bool);

/// What one of the page's actions does.
type Handler = Box<dyn Fn(&PlayerView, Option<&glib::Variant>)>;

struct Ui {
    page: adw::ToolbarView,
    /// The bottom bar's controls that narrow windows leave out: the keys still do them.
    roomy: Vec<gtk::Widget>,
    top: adw::HeaderBar,
    bottom: gtk::Box,
    dim: gtk::Box,
    stage: gtk::Overlay,
    show: gtk::Label,
    episode: gtk::Label,
    /// "Private" after the episode line, while the mode or the show's own flag holds it.
    private: gtk::Box,
    play: gtk::Button,
    timeline: gtk::Scale,
    marked: Marked,
    position: gtk::Label,
    remaining: gtk::Label,
    volume: gtk::Scale,
    starting: gtk::Box,
    start: Rc<start_view::StartView>,
    /// What the header and bottom bar show that the starting screen leaves out: the board keeps
    /// only Back and the window's buttons over the art.
    start_tools: Vec<gtk::Widget>,
    /// The subtitle style panel's button.
    style: gtk::MenuButton,
    cards: gtk::Box,
    chat: Rc<chat::Chat>,
    skip: Card,
    skip_button: gtk::Button,
    /// The marking bar, at the top of the video while skip times are marked.
    mark_bar: gtk::Box,
    /// "Was that the opening?" and "Wrong opening times?", in the corner above Skip.
    skip_prompt: Card,
    /// "Skipped opening · Undo", after an automatic skip.
    undo: Card,
    undo_line: gtk::Label,
    up_next: Card,
    up_next_title: gtk::Label,
    up_next_line: gtk::Label,
    /// Cancel and Play now on the Up Next card, away while the room moves on together.
    up_buttons: gtk::Box,
    up_next_bar: gtk::ProgressBar,
    up_next_still: gtk::Picture,
    up_next_still_frame: crate::fit::Fit,
    stall: Card,
    stall_line: gtk::Label,
    resumed: gtk::Box,
    resumed_line: gtk::Label,
    /// Episodes and Sources: the button, the side panel, the picture held over the video while
    /// a switch loads, and what takes a click on the dimmed video beside the panel.
    episodes: gtk::Button,
    side: Rc<SidePanel>,
    held: gtk::Picture,
    catcher: gtk::Box,
    tracks: gtk::MenuButton,
    /// Watch together: the form that starts a party, or the party's panel once in one, with a
    /// dot on the people while in one.
    people: gtk::Button,
    people_dot: gtk::Box,
    /// Play on TV: hands what plays over to a TV.
    tv: gtk::Button,
    settings: gtk::MenuButton,
    speed: gtk::MenuButton,
    menu_buttons: Vec<gtk::MenuButton>,
    /// The mini player's controls, over the video in place of the bars.
    mini: gtk::Box,
    mini_play: gtk::Button,
}

impl PlayerView {
    pub fn new(ctx: &Rc<Ctx>, options: &PlayerOptions) -> Rc<Self> {
        let (host_tx, host_rx) = async_channel::unbounded::<HostEvent>();
        let engine: std::sync::Weak<Engine> = ctx.hub.engine_link();
        // Reports go to the engine from mpv's thread; the rest to the main loop.
        let sink = Arc::new(move |event: HostEvent| match event {
            HostEvent::Report { playback, report } => {
                if let Some(engine) = engine.upgrade() {
                    engine.send(Command::PlayerReport { playback, report });
                }
            }
            other => {
                let _ = host_tx.send_blocking(other);
            }
        });
        let player = match Player::new(options, sink) {
            Ok(player) => Some(Rc::new(player)),
            Err(e) => {
                tracing::error!(error = %e, "mpv did not start");
                None
            }
        };

        let ready = Rc::new(Cell::new(false));
        let waiting: Rc<RefCell<Vec<(PlaybackId, PlayerOp)>>> = Rc::default();
        let (video_widget, gl) = match &player {
            Some(p) => {
                let area = video::video_area(p.mpv().clone(), {
                    let (ready, waiting, player) =
                        (Rc::clone(&ready), Rc::clone(&waiting), Rc::clone(p));
                    move || {
                        ready.set(true);
                        let ops: Vec<_> = waiting.borrow_mut().drain(..).collect();
                        tracing::info!(waiting = ops.len(), "mpv can draw");
                        for (playback, op) in &ops {
                            player.apply(*playback, op);
                        }
                    }
                });
                if std::env::var_os("KAERI_STATS").is_some() {
                    log_pacing(Rc::clone(&area.pacing), Rc::clone(p));
                }
                (area.widget, area.gl)
            }
            None => (gtk::Box::new(gtk::Orientation::Vertical, 0).upcast(), Rc::default()),
        };

        let ui = build_ui(ctx, &video_widget);
        let preview = Preview::new(&ui.timeline, false);
        let party = crate::party::panel::Panel::new(ctx);
        party.popover.set_parent(&ui.people);
        let room = crate::room::Room::new(&ui.page, &ui.stage);
        ui.top.pack_end(&room.toggle);
        room.set_aside(&ui.people);
        // The column carries the window's buttons while it shows, as a split view's sidebar does.
        room.split.connect_show_sidebar_notify({
            let top = ui.top.clone();
            let toggle = room.toggle.clone();
            move |split| {
                top.set_show_end_title_buttons(!split.shows_sidebar());
                if toggle.is_active() != split.shows_sidebar() && split.shows_sidebar() {
                    toggle.set_active(true);
                }
            }
        });
        ctx.window.connect_fullscreened_notify({
            let room = Rc::clone(&room);
            move |window| room.set_fullscreen(window.is_fullscreen())
        });
        let stats = stats::Stats::new();
        ui.stage.add_overlay(&stats.widget);
        stats.set_narrow(ctx.size() == Size::Narrow);
        ctx.on_size({
            let roomy = ui.roomy.clone();
            let stats = Rc::clone(&stats);
            move |size| {
                for widget in &roomy {
                    widget.set_visible(size != Size::Narrow);
                }
                stats.set_narrow(size == Size::Narrow);
            }
        });
        let view = Rc::new_cyclic(|me| Self {
            widget: room.widget.clone().upcast(),
            me: me.clone(),
            ctx: Rc::clone(ctx),
            shell: RefCell::default(),
            player,
            ready,
            waiting,
            gl,
            ui,
            shown: Cell::new(true),
            idle: RefCell::default(),
            fade: RefCell::default(),
            held_until: Cell::new(Instant::now()),
            said_second: Cell::new(-1),
            state: RefCell::default(),
            over_bar: Cell::new(false),
            hold_up_next: Cell::new(false),
            inhibit: Cell::new(None),
            coming: RefCell::default(),
            menus: RefCell::default(),
            chapters_due: Cell::new(false),
            engine_marks: RefCell::new(None),
            preview,
            mini: Cell::new(None),
            start: RefCell::default(),
            start_since: Cell::new(None),
            start_tick: RefCell::default(),
            roll_on: Cell::new(None),
            start_roll: Cell::new(None),
            start_held: Cell::new(false),
            start_shown: Cell::new(None),
            start_ground: RefCell::default(),
            start_reveal: RefCell::default(),
            start_leave: RefCell::default(),
            resumed_from: Cell::new(None),
            undo: Cell::new(None),
            skip_since: Cell::new(None),
            marking: RefCell::new(None),
            skip_prompt: Cell::new(None),
            prompt_timer: RefCell::new(None),
            prompt_held: Cell::new(false),
            timeline_marks: RefCell::new(Vec::new()),
            sharing: Cell::new(false),
            skip_dialog: RefCell::new(None),
            subtitle_action: radio("subtitle", "off"),
            audio_action: radio("audio", "off"),
            episode_action: radio("episode", ""),
            source_action: radio("source", ""),
            upscaling_action: radio("upscaling", ""),
            night_action: gio::SimpleAction::new_stateful("night", None, &false.to_variant()),
            stats_action: gio::SimpleAction::new_stateful("stats", None, &false.to_variant()),
            speed_action: gio::SimpleAction::new_stateful(
                "speed",
                Some(glib::VariantTy::DOUBLE),
                &1.0f64.to_variant(),
            ),
            translation_action: radio("translation", "sub"),
            title_id: Cell::new(None),
            table: RefCell::default(),
            own_spoilers: Cell::new(None),
            late: Cell::new(false),
            has_dub: Cell::new(None),
            dub: RefCell::default(),
            panel_asked: RefCell::default(),
            found_in: RefCell::default(),
            hold_at: Cell::new(None),
            held_fade: RefCell::default(),
            #[cfg(feature = "fixtures")]
            last_load: RefCell::default(),
            episode_line: RefCell::default(),
            speed: Cell::new(1.0),
            speed_asked: Cell::new(None),
            frame: Cell::new(None),
            mini_at: Cell::new(None),
            stall_since: Cell::new(None),
            stall_tick: RefCell::default(),
            mpris: RefCell::default(),
            party,
            room,
            stats,
        });
        view.wire();
        glib::spawn_future_local({
            let view = Rc::downgrade(&view);
            async move {
                while let Ok(event) = host_rx.recv().await {
                    let Some(view) = view.upgrade() else { return };
                    view.host_event(event);
                }
            }
        });
        view
    }

    /// Serve the media controls as `name` on the session bus: what they ask comes back here.
    pub fn serve_media_controls(self: &Rc<Self>, name: &str) {
        let view = Rc::downgrade(self);
        let mpris = Mpris::serve(name, move |action| {
            if let Some(view) = view.upgrade() {
                view.media_control(action);
            }
        });
        self.mpris.replace(Some(mpris));
        // A TV driven from here is what the media controls show and press while nothing
        // plays on this computer.
        let view = Rc::downgrade(self);
        self.ctx.tv.listen(move || {
            if let Some(view) = view.upgrade() {
                view.tv_media();
            }
        });
    }

    /// The TV is what plays: driven from here, playing something, and nothing open in the
    /// player on this computer.
    fn tv_drives(&self) -> bool {
        let tv = &self.ctx.tv;
        (tv.cast_media().is_some() || (tv.live() && tv.now().is_some()))
            && !self.shell().is_some_and(|s| s.is_player())
    }

    /// Show the TV's episode to the media controls, or nothing once the TV has stopped.
    fn tv_media(&self) {
        let Some(media) = self.media() else { return };
        if self.shell().is_some_and(|s| s.is_player()) {
            return;
        }
        let tv = &self.ctx.tv;
        // A cast running is what the controls show; else a TV running Kaeri, driven from here.
        let cast = tv.cast_media();
        let playhead = match &cast {
            Some((_, at)) => Some(*at),
            None => tv.playhead(),
        };
        let Some(now) =
            cast.map(|(now, _)| now).or_else(|| tv.now().filter(|_| tv.connected().is_some()))
        else {
            media.stopped();
            return;
        };
        let entry = self.ctx.hub.entry(now.id);
        let show = match &entry {
            _ if !now.title.is_empty() => now.title.clone(),
            Some(entry) => entry.title.clone(),
            None => String::new(),
        };
        media.private(self.ctx.hub.known.borrow().holds_show(now.id));
        media.track(Track {
            show,
            title: now.episode_line.clone(),
            art: entry.and_then(|e| e.banner_url.or(e.cover_url)),
        });
        media.state(now.paused, playhead.or(Some(now.position)), now.duration);
        media.neighbours(true, false);
    }

    /// What the media controls asked, sent to the TV.
    fn tv_media_control(&self, action: MediaAction) {
        use kaeri_engine::remote::protocol::command::RemoteCommand;
        let tv = &self.ctx.tv;
        let cast = tv.cast_media().map(|(now, _)| now);
        let paused = cast.as_ref().or(tv.now().as_ref()).is_some_and(|n| n.paused);
        let command = match action {
            MediaAction::Raise => {
                self.ctx.window.present();
                tv.open_remote();
                return;
            }
            // Stop on a cast stops casting, as Stop casting does.
            MediaAction::Stop if cast.is_some() => {
                tv.cast_stop();
                return;
            }
            MediaAction::PlayPause => RemoteCommand::TogglePause,
            MediaAction::Play if paused => RemoteCommand::TogglePause,
            MediaAction::Pause if !paused => RemoteCommand::TogglePause,
            MediaAction::Next => RemoteCommand::Next,
            MediaAction::SeekBy(by) => RemoteCommand::Seek { by },
            MediaAction::SeekTo(position) => RemoteCommand::SeekTo { position },
            MediaAction::Volume(level) => {
                let now =
                    cast.as_ref().or(tv.now().as_ref()).and_then(|n| n.volume).unwrap_or(100.0);
                RemoteCommand::Volume { step: level * 100.0 - now }
            }
            // The TV has no episode before, nor speeds, and Stop leaves it playing.
            MediaAction::Play
            | MediaAction::Pause
            | MediaAction::Previous
            | MediaAction::Stop
            | MediaAction::Rate(_) => return,
        };
        if cast.is_some() {
            tv.cast_send(command);
        } else {
            tv.send(command);
        }
    }

    /// What GNOME's media controls, the lock screen or a media key asked.
    fn media_control(&self, action: MediaAction) {
        if self.tv_drives() {
            self.tv_media_control(action);
            return;
        }
        let paused = self.state.borrow().paused;
        match action {
            MediaAction::Raise => self.ctx.window.present(),
            MediaAction::PlayPause => self.send(PlayerCommand::TogglePause),
            MediaAction::Play if paused => self.send(PlayerCommand::TogglePause),
            MediaAction::Pause if !paused => self.send(PlayerCommand::TogglePause),
            MediaAction::Play | MediaAction::Pause => {}
            MediaAction::Stop => self.close(),
            MediaAction::Next if self.in_party() => {}
            MediaAction::Next => self.send(PlayerCommand::Step(1)),
            MediaAction::Previous => self.send(PlayerCommand::Step(-1)),
            MediaAction::SeekBy(by) => self.send(PlayerCommand::Seek(by)),
            MediaAction::SeekTo(to) => self.send(PlayerCommand::SeekTo(to)),
            MediaAction::Rate(rate) => self
                .send(PlayerCommand::SpeedTo(rate.clamp(SPEEDS[0], SPEEDS[SPEEDS.len() - 1]))),
            MediaAction::Volume(level) => {
                let now = self
                    .player
                    .as_ref()
                    .and_then(|p| p.mpv().get("volume"))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(100.0);
                self.send(PlayerCommand::Volume(level * 100.0 - now));
            }
        }
    }

    /// "Private" in the bar and the neutral media controls, while the mode or the playing
    /// show's own flag holds what plays. A link played with no title behind it follows the mode.
    fn follow_private(&self) {
        let held = {
            let known = self.ctx.hub.known.borrow();
            match self.title_id.get() {
                Some(id) => known.holds_show(id),
                None => known.private,
            }
        };
        self.ui.private.set_visible(held);
        if self.tv_drives() {
            return;
        }
        if let Some(media) = self.media() {
            media.private(held);
        }
    }

    /// While the TV's episode is on the media controls, they follow its show.
    fn tv_media_private(&self) {
        if !self.tv_drives() {
            return;
        }
        let Some(now) = self.ctx.tv.now() else { return };
        if let Some(media) = self.media() {
            media.private(self.ctx.hub.known.borrow().holds_show(now.id));
        }
    }

    fn media(&self) -> Option<Rc<Mpris>> {
        self.mpris.borrow().clone()
    }

    /// The window the player shows in, once it is built.
    pub fn attach(&self, shell: Weak<Shell>) {
        self.shell.replace(shell);
    }

    fn shell(&self) -> Option<Rc<Shell>> {
        self.shell.borrow().upgrade()
    }

    fn send(&self, command: PlayerCommand) {
        self.ctx.hub.send(Command::Player(command));
    }

    pub fn focus(&self) {
        self.ui.play.grab_focus();
        self.wake();
    }

    /// The viewer closed the player: Esc, the back button, Alt+Left.
    pub fn close(&self) {
        if let Some(player) = &self.player {
            player.viewer_closed();
        }
        self.closed();
    }

    /// The window is closing: whatever plays ends as if the viewer closed it.
    pub fn window_closed(&self) {
        if let Some(player) = &self.player {
            player.viewer_closed();
        }
    }

    fn closed(&self) {
        self.set_mini(false);
        self.preview.forget();
        self.ui.marked.set(Vec::new(), 0.0);
        self.ui.marked.set_buffered(Vec::new());
        self.engine_marks.replace(None);
        self.timeline_marks.replace(Vec::new());
        self.skip_mark(None);
        self.show_prompt(None);
        if let Some(shell) = self.shell() {
            shell.show_player(false);
        }
        if self.ctx.window.is_fullscreen() {
            self.ctx.window.unfullscreen();
        }
        if let (Some(cookie), Some(app)) = (self.inhibit.take(), self.ctx.window.application())
        {
            app.uninhibit(cookie);
        }
        self.clear_cards();
        self.forget_undo();
        if let Some(media) = self.media() {
            media.stopped();
        }
        self.tv_media();
        self.end_start();
        self.roll_on.set(None);
        if let Some(ground) = self.start_ground.take() {
            ground.pause();
        }
        self.ui.start.unhold();
        self.ui.starting.set_visible(true);
        self.ui.starting.set_opacity(1.0);
    }

    fn open(&self) {
        if let Some(shell) = self.shell()
            && !shell.is_player()
        {
            shell.show_player(true);
        }
    }

    /// What the engine said, as the player hears it.
    pub fn engine_event(&self, event: &Event) {
        if crate::private::changes(event) {
            self.follow_private();
            self.tv_media_private();
        }
        match event {
            Event::PlayerOps { playback, ops } => {
                if ops.iter().any(|op| matches!(op, PlayerOp::Open { .. })) {
                    self.open();
                    // The engine's player starts with its stats off: asked again, and the
                    // panel waits for the new file's figures.
                    if self.stats.shown() {
                        self.stats.restart();
                        self.send(PlayerCommand::Stats(true));
                    }
                }
                let Some(player) = &self.player else {
                    self.ctx.toast(&t::player_could_not_start());
                    return;
                };
                for op in ops {
                    match op {
                        PlayerOp::Load { url, options } => {
                            #[cfg(feature = "fixtures")]
                            self.last_load.replace(Some((
                                *playback,
                                url.clone(),
                                options.clone(),
                            )));
                            let options = options
                                .iter()
                                .map(|o| (o.name.clone(), o.value.clone()))
                                .collect();
                            self.preview.stream(url, options);
                            self.ui.marked.set(Vec::new(), 0.0);
                            self.ui.marked.set_buffered(Vec::new());
                            self.engine_marks.replace(None);
                            self.chapters_due.set(true);
                        }
                        PlayerOp::Set { property, .. } if property == "chapter-list" => {
                            self.chapters_due.set(true);
                        }
                        _ => {}
                    }
                    if self.ready.get() {
                        player.apply(*playback, op);
                    } else {
                        self.waiting.borrow_mut().push((*playback, op.clone()));
                    }
                }
            }
            Event::PlayerUi { ui, .. } => self.player_ui(ui),
            Event::Settings(_) => self.style_subtitles(),
            Event::PartyJoined(_) | Event::PartyLeft { .. } => {
                self.show_party_button();
                self.ui.chat.heard(event, true);
            }
            Event::PartyChat(_)
            | Event::PartyChatState(_)
            | Event::PartyReaction(_)
            | Event::PartyChatRefused(_) => self.ui.chat.heard(event, true),
            Event::PlayerSkipMark { view, .. } => self.skip_mark(view.clone()),
            Event::PlayerSkipPrompt { prompt, .. } => {
                if !self.prompt_held.get() || prompt.is_some() {
                    self.show_prompt(*prompt);
                }
            }
            Event::PlayerSkipNotice { notice, .. } => {
                self.ctx.toast(&marking::notice_text(notice))
            }
            Event::PlayerTimeline { marks, .. } => {
                self.timeline_marks.replace(marks.clone());
                // The engine's marks are the ones skipping uses; they replace the chapters'.
                self.engine_marks.replace(Some(timeline::from_marks(marks)));
                self.chapters_due.set(false);
                let duration = self.state.borrow().duration.unwrap_or(0.0);
                self.show_marks(duration);
            }
            Event::TitleDub { id, dub } if Some(*id) == self.title_id.get() => {
                let had = self.has_dub.replace(Some(dub.is_some()));
                self.dub.replace(*dub);
                if had != Some(dub.is_some()) && self.ui.tracks.is_active() {
                    self.fill_tracks();
                }
                self.refresh_panel();
            }
            Event::Episodes { id, rows, failed: None } if Some(*id) == self.title_id.get() => {
                self.table.replace(rows.clone());
                self.rebuild_menus();
            }
            Event::Sources { id, episode, candidates, .. }
                if self.panel_asked.borrow().as_ref() == Some(&(*id, episode.clone())) =>
            {
                self.ui.side.set_releases(Some(candidates.clone()));
            }
            Event::SourcesProgress { id, episode, rows }
                if self.panel_asked.borrow().as_ref() == Some(&(*id, episode.clone())) =>
            {
                self.ui.side.set_progress(rows.clone());
            }
            Event::SourcesFailed { id, episode, why }
                if self.panel_asked.borrow().as_ref() == Some(&(*id, episode.clone())) =>
            {
                // Said in the tab as well as the alert; opening the tab again asks again.
                self.panel_asked.take();
                self.ui.side.sources_failed(crate::say::msg(why));
            }
            Event::SourceOptions { id, options } if Some(*id) == self.title_id.get() => {
                self.ui.side.set_options(Some(options.clone()));
            }
            Event::PlayerSwitch { state, .. } => self.switched(state),
            Event::ShowSettings(view) if Some(view.id) == self.title_id.get() => {
                let own = view
                    .rows
                    .iter()
                    .find(|row| row.key == kaeri_model::show::ShowSettingKey::SpoilerProtection)
                    .map(|row| row.value == "on");
                if own != self.own_spoilers.replace(own) {
                    self.rebuild_menus();
                }
            }
            Event::PlayerStats { stats, .. } => self.stats.took(stats),
            Event::Clipboard { text } => {
                self.ctx.window.clipboard().set_text(text);
                self.stats.copied();
            }
            Event::PlayerBuffered { ranges, .. } => {
                self.ui.marked.set_buffered(ranges.iter().map(|r| (r.start, r.end)).collect());
            }
            Event::PlaybackStaged { id, title, episode, episode_title, resume, .. } => {
                self.staged(*id, title, episode, episode_title.as_deref(), *resume);
            }
            Event::StartingStep { id, episode, step, line } => {
                self.start_step(*id, episode, step, line.as_ref());
            }
            Event::PlaybackEnded { failure: Some(failure), rolling_on: false, .. } => {
                self.start_failed(failure);
            }
            // The engine rolled on by itself: the next episode's start is a roll-on.
            Event::PlaybackEnded { rolling_on: true, .. } => self.rolled_on(),
            _ => {}
        }
    }

    /// Whether a start is on screen: before the first frame, and after a failure.
    pub fn is_starting(&self) -> bool {
        self.start.borrow().is_some()
    }

    /// The bars while a start is on screen: Back and the window's buttons only.
    fn start_chrome(&self, on: bool) {
        self.ui.bottom.set_visible(!on);
        for widget in &self.ui.start_tools {
            widget.set_opacity(if on { 0.0 } else { 1.0 });
            widget.set_can_target(!on);
            widget.set_focusable(!on);
        }
    }

    /// The start is over: it played, or the player closed.
    fn end_start(&self) {
        if self.start.replace(None).is_some() {
            self.start_chrome(false);
        }
        self.start_since.set(None);
        self.stop_start_timers();
        self.start_roll.set(None);
        self.start_held.set(false);
        self.start_shown.set(None);
    }

    fn stop_start_timers(&self) {
        for timer in [&self.start_reveal, &self.start_leave] {
            if let Some(source) = timer.borrow_mut().take() {
                source.remove();
            }
        }
    }

    /// A roll-on began: Up next's countdown ended, Play now, or the engine rolled on.
    fn rolled_on(&self) {
        if self.roll_on.get().is_none_or(|at| at.elapsed() >= ROLL_ON_FRESH) {
            self.roll_on.set(Some(Instant::now()));
        }
    }

    /// A roll-on's start: the last frame and Up next fade to the ground (ease-in, 0.25 s), and
    /// the starting screen shows over it only when the picture has not come 600 ms after the
    /// roll-on. A start begun again for the same episode (`carried`) keeps where it had got to.
    fn hold_start(&self, at: Instant, carried: bool) {
        self.start_roll.set(Some(at));
        if carried && !self.start_held.get() {
            // Its screen showed already, and keeps its hold.
            self.ui.starting.set_opacity(1.0);
            return;
        }
        if !carried {
            self.start_shown.set(None);
            self.start_held.set(true);
            self.ui.start.hold();
            self.ui.starting.set_opacity(0.0);
            let ground = crate::motion::timed(
                &self.ui.starting,
                0.0,
                1.0,
                250,
                crate::motion::opacity(&self.ui.starting),
            );
            ground.set_easing(adw::Easing::EaseInCubic);
            ground.play();
            self.start_ground.replace(Some(ground));
        }
        let wait = start_model::ROLL_ON_WAIT.saturating_sub(at.elapsed());
        let me = self.me.clone();
        let source = glib::timeout_add_local_once(wait, move || {
            let Some(view) = me.upgrade() else { return };
            view.start_reveal.borrow_mut().take();
            view.reveal_start();
        });
        self.start_reveal.replace(Some(source));
    }

    /// The held screen shows over its ground: the wait ran out, or the start failed.
    fn reveal_start(&self) {
        if let Some(source) = self.start_reveal.borrow_mut().take() {
            source.remove();
        }
        if !self.start_held.replace(false) || self.start.borrow().is_none() {
            return;
        }
        self.start_shown.set(Some(Instant::now()));
        self.ui.start.reveal();
    }

    /// The first frame came: the starting screen fades to the picture, at once or, for a
    /// roll-on's screen that showed, once it has held for its 0.6 s.
    fn leave_start(&self) {
        let wait = self.start_roll.get().map_or(Duration::ZERO, |roll| {
            let picture = roll.elapsed();
            let shown = self.start_shown.get().map(|shown| shown.duration_since(roll));
            start_model::roll_on_leaves(shown, picture).saturating_sub(picture)
        });
        if wait.is_zero() {
            self.start_gone();
            return;
        }
        let me = self.me.clone();
        let source = glib::timeout_add_local_once(wait, move || {
            let Some(view) = me.upgrade() else { return };
            view.start_leave.borrow_mut().take();
            if view.ui.starting.is_visible() {
                view.start_gone();
            }
        });
        self.start_leave.replace(Some(source));
    }

    fn start_gone(&self) {
        if let Some(ground) = self.start_ground.take() {
            ground.pause();
        }
        fade(&self.ui.starting, false);
        self.end_start();
    }

    fn start_elapsed(&self) -> Duration {
        self.start_since.get().map_or(Duration::ZERO, |since| since.elapsed())
    }

    fn start_episode(&self) -> String {
        self.start.borrow().as_ref().map(|s| s.episode.clone()).unwrap_or_default()
    }

    /// A play was staged: the starting screen opens at once, with the title's art where it is
    /// already known.
    fn staged(
        &self,
        id: AnilistId,
        title: &str,
        episode: &str,
        episode_title: Option<&str>,
        resume: Option<f64>,
    ) {
        // The episode started again, as it does in the other version when nothing played yet:
        // no switch of it is on its way.
        self.ui.side.version_done();
        self.begin_start(id, title, episode, episode_title, resume);
        let entry = self.ctx.hub.entry(id);
        let art = entry.and_then(|e| e.banner_url.or(e.cover_url));
        self.ctx.hub.art(&self.ui.start.art, art.as_deref());
        self.open();
    }

    /// A new start: the model, the screen and the clock.
    fn begin_start(
        &self,
        id: AnilistId,
        title: &str,
        episode: &str,
        episode_title: Option<&str>,
        resume: Option<f64>,
    ) {
        if let Some(tick) = self.start_tick.borrow_mut().take() {
            tick.remove();
        }
        // A roll-on's screen is held back. The same start begun again (staged, then opened)
        // keeps the hold it had.
        let again =
            self.start.borrow().as_ref().is_some_and(|s| s.id == id && s.episode == episode);
        let carried = self.start_roll.get().filter(|_| again);
        let rolled = self.roll_on.take().filter(|at| at.elapsed() < ROLL_ON_FRESH);
        self.stop_start_timers();
        self.start.replace(Some(start_model::Start::new(id, episode, resume)));
        self.start_chrome(true);
        if let Some(start) = self.start.borrow_mut().as_mut() {
            start.details_open = self.ctx.prefs.starting_details();
        }
        self.start_since.set(Some(clock::Moment::now()));
        self.ui.show.set_label(title);
        self.late.set(false);
        let translation = self.episode_line.borrow().2;
        self.episode_line.replace((
            episode.to_owned(),
            episode_title.map(str::to_owned),
            translation,
        ));
        let line = self.show_episode_line();
        let demo = id == kaeri_core::demo::DEMO_ANILIST_ID;
        let resumes = self.start.borrow().as_ref().is_some_and(start_model::Start::resumes);
        self.ui.start.begin(title, &line, demo, resumes);
        self.ui.starting.set_visible(true);
        match (rolled, carried) {
            (Some(at), _) => self.hold_start(at, false),
            (None, Some(at)) => self.hold_start(at, true),
            (None, None) => {
                self.start_roll.set(None);
                self.start_held.set(false);
                self.start_shown.set(None);
                self.ui.start.unhold();
                self.ui.starting.set_opacity(1.0);
            }
        }
        if self.start_roll.get().is_some() {
            // Up next fades with the last frame rather than vanish under the ground.
            self.ui.up_next.set(false);
            self.coming.replace(None);
            for card in [&self.ui.skip, &self.ui.undo, &self.ui.stall] {
                card.hide_now();
            }
            self.stall_over();
        } else {
            self.clear_cards();
        }
        self.resumed_done();
        self.announce(&t::player_start_announce(episode, title));
        self.show_start();
        let me = self.me.clone();
        let tick = glib::timeout_add_local(Duration::from_millis(100), move || {
            let Some(view) = me.upgrade() else { return glib::ControlFlow::Break };
            if view.start.borrow().is_none() || !view.ui.starting.is_visible() {
                view.start_tick.borrow_mut().take();
                return glib::ControlFlow::Break;
            }
            view.tick_start();
            glib::ControlFlow::Continue
        });
        self.start_tick.replace(Some(tick));
    }

    /// The starting screen's clock moved: a waiting line whose turn has come shows, and the
    /// screen is drawn for now.
    fn tick_start(&self) {
        let at = self.start_elapsed();
        if let Some(start) = self.start.borrow_mut().as_mut() {
            start.tick(at);
        }
        let slow = self.start.borrow_mut().as_mut().and_then(|s| s.slow_announcement(at));
        if let Some(slow) = slow {
            self.announce(&slow);
        }
        self.show_start();
    }

    /// Draw what the model says now.
    fn show_start(&self) {
        let at = self.start_elapsed();
        let drawn = self.start.borrow().as_ref().map(|s| (s.frame(at), s.details_open));
        if let Some((frame, open)) = drawn {
            self.ui.start.show(&frame, open);
        }
    }

    fn start_step(
        &self,
        id: AnilistId,
        episode: &str,
        step: &kaeri_model::starting::StartStep,
        line: Option<&kaeri_model::Msg>,
    ) {
        let at = self.start_elapsed();
        {
            let mut start = self.start.borrow_mut();
            let Some(start) = start.as_mut().filter(|s| s.id == id && s.episode == episode)
            else {
                return;
            };
            start.step(step, line, at);
        }
        if let kaeri_model::starting::StartStep::Found { source: Some(source), .. } = step {
            self.found_in.replace(Some((id, source.clone())));
            self.playing_from(source);
        }
        if matches!(step, kaeri_model::starting::StartStep::WaitingForConnection) {
            self.announce(&t::player_start_offline_detail(&self.start_episode()));
        }
        self.show_start();
    }

    /// The start ended without playing, and the engine says why: the failure screen.
    fn start_failed(&self, failure: &kaeri_model::starting::StartFailure) {
        if self.start.borrow().is_none() {
            return;
        }
        if let Some(start) = self.start.borrow_mut().as_mut() {
            start.fail(failure.clone());
        }
        // A failure shows at once, whatever a roll-on's wait has left.
        self.reveal_start();
        self.show_start();
        let frame = self.start.borrow().as_ref().map(|s| s.frame(self.start_elapsed()));
        if let Some(Some((title, reason))) = frame.map(|f| f.failure) {
            self.announce_high(&t::player_start_failed_announce(&title, &reason));
        }
        self.ui.start.focus_primary();
        self.open();
    }

    /// What a button on the starting screen does.
    fn start_action(&self, action: start_model::Action) {
        use start_model::Action as A;
        let Some((id, episode)) =
            self.start.borrow().as_ref().map(|s| (s.id, s.episode.clone()))
        else {
            return;
        };
        match action {
            A::Cancel | A::Close => self.close(),
            A::FromBeginning => {
                self.send(PlayerCommand::PlayFromBeginning);
                if let Some(start) = self.start.borrow_mut().as_mut() {
                    start.begin_from_start();
                }
                self.show_start();
            }
            A::Skip => self.send(PlayerCommand::SkipSource),
            A::ChooseSource | A::ChooseAnother => {
                crate::party::source::choose(&self.ctx, id, &episode);
            }
            A::Details => {
                let open = {
                    let mut start = self.start.borrow_mut();
                    let Some(start) = start.as_mut() else { return };
                    start.details_open = !start.details_open;
                    start.details_open
                };
                self.ctx.prefs.set_starting_details(open);
                self.show_start();
            }
            A::ShowDownloads => {
                self.close();
                let _ = WidgetExt::activate_action(
                    &self.ctx.window,
                    "win.go",
                    Some(&"downloads".to_variant()),
                );
            }
            A::TryAgain => {
                self.ctx.hub.send(Command::Play { id, episode, from_start: false });
            }
            A::FixMatch => {
                self.close();
                let _ = crate::pages::match_picker::open(
                    &self.ctx,
                    &self.ctx.window,
                    id,
                    true,
                    None,
                );
            }
            A::AddStore => {
                self.close();
                let _ = WidgetExt::activate_action(
                    &self.ctx.window,
                    "win.go",
                    Some(&"stores".to_variant()),
                );
            }
        }
    }

    /// "Resumed at 12:04" with Start Over, in the top leading corner.
    fn resumed(&self, position: f64) {
        self.ui.resumed_line.set_label(&t::player_resumed_at(&say::clock(position)));
        self.announce(&self.ui.resumed_line.label());
        self.resumed_from.set(Some(position));
        self.ui.resumed.set_visible(true);
        crate::motion::fade_in(&self.ui.resumed);
    }

    /// Say `text` to a screen reader once.
    fn announce(&self, text: &str) {
        self.ui
            .page
            .upcast_ref::<gtk::Widget>()
            .announce(text, gtk::AccessibleAnnouncementPriority::Medium);
    }

    /// Say `text` to a screen reader at once: a failure.
    fn announce_high(&self, text: &str) {
        self.ui
            .page
            .upcast_ref::<gtk::Widget>()
            .announce(text, gtk::AccessibleAnnouncementPriority::High);
    }

    fn resumed_done(&self) {
        if self.resumed_from.take().is_some() && self.ui.resumed.is_visible() {
            crate::motion::fade_out(&self.ui.resumed);
        }
    }

    fn player_ui(&self, ui: &PlayerUi) {
        match ui {
            PlayerUi::Starting {
                id,
                title,
                episode,
                episode_title,
                banner_url,
                cover_url,
                ..
            } => {
                // Whether the title has a dub, for the Sub and Dub switch: asked once a title.
                // An opened link played without tracking has no title behind it to ask about.
                self.close_panel();
                if self.panel_asked.take().is_some() {
                    self.ui.side.listing_dropped();
                }
                if self.title_id.replace(Some(*id)) != Some(*id) {
                    self.has_dub.set(None);
                    self.dub.take();
                    self.table.take();
                    self.own_spoilers.set(None);
                    let found = self.found_in.take().filter(|(of, _)| of == id);
                    self.ui.side.set_data(side_panel::Data {
                        playing_source: found.map(|(_, source)| source),
                        ..side_panel::Data::default()
                    });
                    if *id != kaeri_model::opened::UNTRACKED {
                        self.ctx.hub.send(Command::LoadDub(*id));
                        // The synopses for the Episodes panel, and the show's own protection.
                        self.ctx.hub.send(Command::LoadEpisodes(*id));
                        self.ctx.hub.send(Command::LoadShowSettings { id: *id });
                    }
                }
                self.ui.show.set_label(title);
                self.late.set(false);
                let translation = self.episode_line.borrow().2;
                self.episode_line.replace((
                    episode.clone(),
                    episode_title.clone(),
                    translation,
                ));
                let line = self.show_episode_line();
                // Held or not before the controls hear of it, so its name never reaches them.
                self.follow_private();
                if let Some(media) = self.media() {
                    media.track(Track {
                        show: title.clone(),
                        title: line.clone(),
                        art: banner_url.clone().or_else(|| cover_url.clone()),
                    });
                }
                // The play was staged already, and its screen has been counting: the same
                // start goes on. Else (a start that was not staged) the screen opens now.
                let same = self
                    .start
                    .borrow()
                    .as_ref()
                    .is_some_and(|s| s.id == *id && s.episode == *episode);
                if same {
                    self.ui.start.set_show(title);
                    self.ui.start.set_episode_line(&line);
                } else {
                    self.begin_start(*id, title, episode, episode_title.as_deref(), None);
                }
                self.ui.starting.set_visible(true);
                if self.start_roll.get().is_none() {
                    self.ui.starting.set_opacity(1.0);
                }
                let art = banner_url.as_deref().or(cover_url.as_deref());
                self.ctx.hub.art(&self.ui.start.art, art);
                self.open();
            }
            PlayerUi::Episode { number, title, show, translation, .. } => {
                self.ui.show.set_label(show);
                if self.episode_line.borrow().0 != *number {
                    self.late.set(false);
                }
                self.episode_line.replace((number.clone(), title.clone(), Some(*translation)));
                self.translation_action.set_state(&translation.as_str().to_variant());
                let line = self.show_episode_line();
                if let Some(media) = self.media() {
                    media.retitle(show, &line);
                }
            }
            PlayerUi::Menus {
                episodes, sources, upscaling, night_mode, translation, ..
            } => {
                self.translation_action.set_state(&translation.as_str().to_variant());
                if *translation == Translation::Dub {
                    self.has_dub.set(Some(true));
                }
                if let Some(media) = self.media() {
                    let at = episodes.iter().position(|e| e.current);
                    media.neighbours(
                        at.is_some_and(|at| at + 1 < episodes.len()),
                        at.is_some_and(|at| at > 0),
                    );
                }
                self.menus.replace(Some((
                    episodes.clone(),
                    sources.clone(),
                    upscaling.clone(),
                    *night_mode,
                )));
                self.rebuild_menus();
            }
            PlayerUi::Skip { kind, .. } => {
                self.ui.skip_button.set_label(&match kind {
                    SkipKind::Opening => t::player_skip_opening(),
                    SkipKind::Ending => t::player_skip_ending(),
                    SkipKind::Recap => t::player_skip_recap(),
                });
                if !self.ui.skip.shown() {
                    self.skip_since.set(Some(Instant::now()));
                    self.announce(&t::player_announce_skip(
                        &self.ui.skip_button.label().unwrap_or_default(),
                    ));
                }
                self.ui.skip.set(true);
            }
            PlayerUi::SkipCleared => {
                self.skip_since.set(None);
                self.ui.skip.set(false);
            }
            PlayerUi::AutoSkipped { kind, undo_to } => self.auto_skipped(*kind, *undo_to),
            PlayerUi::UpNext { .. } if self.hold_up_next.get() => {}
            PlayerUi::UpNext { next, countdown, auto, ending_at, .. } => {
                let translation = self.episode_line.borrow().2;
                self.ui.up_next_title.set_label(&episode_line(
                    &next.number,
                    translation,
                    next.title.as_deref(),
                ));
                self.ui.up_next_title.set_tooltip_text(Some(&self.ui.up_next_title.label()));
                self.ui.up_next_still.set_paintable(Some(&widgets::swatch(None)));
                self.ui.up_next_still_frame.set_visible(next.thumbnail.is_some());
                self.ctx.hub.art(&self.ui.up_next_still, next.thumbnail.as_deref());
                self.coming.replace(Some(coming::Coming::new(
                    *auto && !crate::window::screen_reader_running(),
                    *ending_at,
                    *countdown,
                )));
                self.show_up_next();
            }
            PlayerUi::UpNextCleared => {
                self.coming.replace(None);
                self.ui.up_next.set(false);
            }
            PlayerUi::Stall { seconds } => {
                self.stalled(*seconds);
                self.wake();
            }
            PlayerUi::StallCleared => self.stall_over(),
            PlayerUi::Alert { text, .. } => {
                let shown = crate::catalog::shown(text);
                // A chat line has its own place on the video, in the column.
                if !self.ui.chat.is_echo(&shown) {
                    self.ctx.alert(&shown);
                }
            }
            PlayerUi::Thumbnails { url } => self.preview.thumbnails(url.as_deref()),
            PlayerUi::Resumed { position } => self.resumed(*position),
            _ => {}
        }
    }

    /// The episode on screen's line, in the top bar and as the media controls title it:
    /// "Episode 7 · Dub · The Lantern Road".
    fn show_episode_line(&self) -> String {
        let (number, title, translation) = self.episode_line.borrow().clone();
        let line = episode_line(&number, translation, title.as_deref());
        self.ui.episode.set_label(&line);
        line
    }

    /// "Source is buffering · 9 s", counting up from what the engine says, with Keep Waiting
    /// and Switch Source.
    fn stalled(&self, seconds: u64) {
        self.stall_since.set(Some(Instant::now() - Duration::from_secs(seconds)));
        let say = |view: &Self| {
            let held = view.stall_since.get().map_or(0, |since| since.elapsed().as_secs());
            view.ui.stall_line.set_label(&t::player_stall_title(held as i64));
        };
        say(self);
        if !self.ui.stall.shown() {
            self.announce(&self.ui.stall_line.label());
        }
        self.ui.stall.set(true);
        if self.stall_tick.borrow().is_some() {
            return;
        }
        let me = self.me.clone();
        let tick = glib::timeout_add_seconds_local(1, move || {
            let Some(view) = me.upgrade() else { return glib::ControlFlow::Break };
            if view.stall_since.get().is_none() {
                view.stall_tick.borrow_mut().take();
                return glib::ControlFlow::Break;
            }
            say(&view);
            glib::ControlFlow::Continue
        });
        self.stall_tick.replace(Some(tick));
    }

    fn stall_over(&self) {
        self.stall_since.set(None);
        if let Some(tick) = self.stall_tick.borrow_mut().take() {
            tick.remove();
        }
        self.ui.stall.set(false);
    }

    fn clear_cards(&self) {
        for card in [&self.ui.skip, &self.ui.undo, &self.ui.up_next, &self.ui.stall] {
            card.hide_now();
        }
        self.stall_over();
        self.coming.replace(None);
    }

    /// The Up next card for the playhead: it shows from where the ending starts, "Plays in 8 s"
    /// counts down on the playhead, and at zero this sends `PlayNext`, once.
    fn show_up_next(&self) {
        let (position, duration) = {
            let state = self.state.borrow();
            (state.position.unwrap_or(0.0), state.duration.unwrap_or(0.0))
        };
        let (card, due) = match self.coming.borrow().as_ref() {
            Some(plan) => (plan.card(position, duration), plan.due(position, duration)),
            None => (coming::Card::Away, false),
        };
        // A screen reader hears the card once, as it appears.
        if !matches!(card, coming::Card::Away) && !self.ui.up_next.shown() {
            let title = self.ui.up_next_title.label();
            self.announce(&match card {
                coming::Card::Counting { left, .. } => {
                    t::player_announce_up_next_counting(&title, left.ceil() as i64)
                }
                _ => t::player_announce_up_next_waiting(&title),
            });
        }
        match card {
            coming::Card::Away => self.ui.up_next.set(false),
            coming::Card::Waiting => {
                self.ui.up_next_line.set_visible(false);
                self.ui.up_next_bar.set_visible(false);
                self.ui.up_next.set(true);
            }
            coming::Card::Counting { left, fraction } => {
                let secs = left.ceil() as i64;
                self.ui.up_next_line.set_label(&if self.in_party() {
                    t::party_next_line(secs)
                } else {
                    t::player_plays_in(secs)
                });
                self.ui.up_next_line.set_visible(true);
                self.ui.up_next_bar.set_fraction(fraction);
                self.ui.up_next_bar.set_visible(true);
                self.ui.up_next.set(true);
            }
        }
        if due {
            if let Some(plan) = self.coming.borrow_mut().as_mut() {
                plan.played = true;
            }
            self.rolled_on();
            self.send(PlayerCommand::PlayNext);
        }
    }

    /// Whether this device is in a watch party, where the room moves on together.
    fn in_party(&self) -> bool {
        self.ctx.hub.known.borrow().party.is_some()
    }

    /// Cancel on the card, Escape, or the pad's B: the card goes and the engine stays.
    fn cancel_next(&self) {
        if self.in_party() {
            return;
        }
        if let Some(plan) = self.coming.borrow_mut().as_mut() {
            plan.cancelled = true;
        }
        self.ui.up_next.set(false);
        self.send(PlayerCommand::CancelNext);
    }

    /// Play now on the card, Enter, or the pad's A.
    fn play_next(&self) {
        if self.in_party() {
            return;
        }
        if let Some(plan) = self.coming.borrow_mut().as_mut() {
            plan.played = true;
        }
        self.rolled_on();
        self.send(PlayerCommand::PlayNext);
    }

    /// "Skipped opening · Undo" in the corner, for 6 s of playback: the button, U, or Ctrl+Z.
    fn auto_skipped(&self, kind: SkipKind, undo_to: f64) {
        self.forget_undo();
        self.undo.set(Some(Undo {
            to: undo_to,
            since: Instant::now(),
            landed: None,
            scale: if crate::window::screen_reader_running() { 2 } else { 1 },
        }));
        self.ui.undo_line.set_label(&match kind {
            SkipKind::Opening => t::player_skipped_opening(),
            SkipKind::Ending => t::player_skipped_ending(),
            SkipKind::Recap => t::player_skipped_recap(),
        });
        self.ui.undo.set(true);
        self.announce(&t::player_announce_undo(&self.ui.undo_line.label()));
        // At most 15 s, even while paused, when no playback counts the 6 s down.
        let me = self.me.clone();
        let most = UNDO_MOST * self.undo.get().map_or(1, |undo| undo.scale);
        glib::timeout_add_local_once(most, move || {
            if let Some(view) = me.upgrade()
                && view.undo.get().is_some_and(|undo| undo.since.elapsed() >= most)
            {
                view.forget_undo();
            }
        });
    }

    /// Back to where the last automatic skip started, while Undo is offered. True when it was.
    fn undo_skip(&self) -> bool {
        let position = self.state.borrow().position;
        let Some(undo) = self.undo.get().filter(|u| u.open(position)) else {
            self.forget_undo();
            return false;
        };
        self.send(PlayerCommand::UndoSkip { to: undo.to });
        self.forget_undo();
        true
    }

    fn forget_undo(&self) {
        self.undo.set(None);
        self.ui.undo.set(false);
    }

    /// The Subtitles and Audio menu, from the file's tracks as they are now.
    fn fill_tracks(&self) {
        let list = self.player.as_ref().and_then(|p| p.mpv().get("track-list"));
        let tracks = list.as_ref().map(tracks::tracks).unwrap_or_default();
        let playing = !tracks.is_empty() && !self.ui.starting.is_visible();
        self.subtitle_action
            .set_state(&tracks::chosen(&tracks, TrackKind::Subtitle).to_variant());
        self.audio_action.set_state(&tracks::chosen(&tracks, TrackKind::Audio).to_variant());
        let dub = self.has_dub.get() == Some(true);
        self.ui.tracks.set_menu_model(Some(&tracks::menu(&tracks, playing, dub)));
        if dub
            && let Some(popover) = self.ui.tracks.popover().and_downcast::<gtk::PopoverMenu>()
        {
            popover.add_child(&tracks::translation_switch(), tracks::TRANSLATION_SLOT);
        }
    }

    /// A track picked from the menu: the engine selects it and keeps it for the series.
    fn choose_track(&self, kind: TrackKind, target: &str) {
        let id = if target == "off" { None } else { target.parse().ok() };
        if id.is_none() && (target != "off" || kind == TrackKind::Audio) {
            return;
        }
        let action = match kind {
            TrackKind::Subtitle => &self.subtitle_action,
            TrackKind::Audio => &self.audio_action,
        };
        action.set_state(&target.to_variant());
        self.send(PlayerCommand::ChooseTrack { kind, id });
    }

    /// Load a subtitle or audio track from a file, through the file chooser portal, and
    /// select it. mpv's alone: the next episode does not carry it.
    fn load_track(&self, kind: TrackKind) {
        let title = match kind {
            TrackKind::Subtitle => t::player_load_subtitle_file(),
            TrackKind::Audio => t::player_load_audio_file(),
        };
        let dialog = gtk::FileDialog::builder()
            .title(title.trim_end_matches('…'))
            .modal(true)
            .filters(&tracks::filter(kind))
            .build();
        let me = self.me.clone();
        dialog.open(Some(&self.ctx.window), gio::Cancellable::NONE, move |picked| {
            let Some(view) = me.upgrade() else { return };
            let file = match picked {
                Ok(file) => file,
                // Dismissed: nothing to say.
                Err(e) if e.matches(gtk::DialogError::Dismissed) => return,
                Err(e) => {
                    tracing::warn!(error = %e, "the file chooser failed");
                    view.ctx.toast(&t::player_file_picker_failed());
                    return;
                }
            };
            let Some(path) = file.path() else {
                view.ctx.toast(&t::player_file_unreadable());
                return;
            };
            let path = path.to_string_lossy().into_owned();
            let added = view.player.as_ref().map(|player| {
                player.mpv().command(&[tracks::add_command(kind), &path, "select"])
            });
            match added {
                Some(Ok(())) => view.fill_tracks(),
                Some(Err(e)) => {
                    tracing::warn!(error = %e, "mpv did not load the track");
                    view.ctx.toast(&match kind {
                        TrackKind::Subtitle => t::player_subtitle_file_failed(),
                        TrackKind::Audio => t::player_audio_file_failed(),
                    });
                }
                None => view.ctx.toast(&t::player_could_not_start()),
            }
            view.wake();
        });
    }

    fn host_event(&self, event: HostEvent) {
        match event {
            HostEvent::Opened { fullscreen } => {
                self.open();
                self.report_display();
                let start_fullscreen = self
                    .ctx
                    .hub
                    .known
                    .borrow()
                    .setting(kaeri_model::settings::SettingKey::Fullscreen)
                    .is_some_and(|r| crate::catalog::is_on(&r));
                if fullscreen || start_fullscreen {
                    self.ctx.window.fullscreen();
                }
                if self.inhibit.get().is_none()
                    && let Some(app) = self.ctx.window.application()
                {
                    let cookie = app.inhibit(
                        Some(&self.ctx.window),
                        gtk::ApplicationInhibitFlags::IDLE
                            | gtk::ApplicationInhibitFlags::SUSPEND,
                        Some(&t::player_inhibit_reason()),
                    );
                    // Zero when there is no session to ask, as in a headless run.
                    self.inhibit.set((cookie > 0).then_some(cookie));
                }
                self.wake();
                self.style_subtitles();
            }
            HostEvent::Closed => self.closed(),
            HostEvent::Fullscreen(on) => self.fullscreen(on),
            HostEvent::State(state) => self.show_state(state),
            HostEvent::Report { .. } => {}
        }
    }

    fn fullscreen(&self, on: Option<bool>) {
        let window = &self.ctx.window;
        if on.unwrap_or(!window.is_fullscreen()) {
            window.fullscreen();
        } else {
            window.unfullscreen();
        }
    }

    fn discussions_on(&self) -> bool {
        self.ctx
            .hub
            .known
            .borrow()
            .setting(kaeri_model::settings::SettingKey::EpisodeDiscussions)
            .is_some_and(|row| crate::catalog::is_on(&row))
    }

    fn show_state(&self, state: PlayerState) {
        // From 85% of the episode, the point it counts as watched.
        if !self.late.get()
            && let (Some(position), Some(duration)) = (state.position, state.duration)
            && duration > 0.0
            && position >= duration * 0.85
        {
            self.late.set(true);
            self.rebuild_menus();
        }
        if let Some(media) = self.media() {
            media.state(state.paused, state.position, state.duration);
        }
        let ui = &self.ui;
        let (icon, tip) = if state.paused {
            ("media-playback-start-symbolic", t::action_play())
        } else {
            ("media-playback-pause-symbolic", t::action_pause())
        };
        for button in [&ui.play, &ui.mini_play] {
            button.set_icon_name(icon);
            button.set_tooltip_text(Some(&tip));
        }
        if let Some(duration) = state.duration {
            ui.timeline.set_range(0.0, duration.max(0.1));
        }
        // While a switch loads, the clock stays where the held picture is.
        let position = self.hold_at.get().or(state.position);
        if let Some(position) = position {
            ui.side.set_playhead(Playhead { position, duration: state.duration });
        }
        if let Some(position) = position {
            ui.position.set_label(&say::clock(position));
            if let Some(duration) = state.duration {
                let left = (duration - position).max(0.0);
                // A minus sign, not a hyphen, and said as time left.
                ui.remaining.set_label(&format!("\u{2212}{}", say::clock(left)));
                ui.remaining.update_property(&[gtk::accessible::Property::Label(
                    &t::player_time_left(&say::spoken_duration(left)),
                )]);
            }
            if Instant::now() >= self.held_until.get() {
                ui.timeline.set_value(position);
                ui.marked.moved();
            }
            // The seek bar says a time, not a number of seconds, and only when the second changes.
            if let Some(duration) = state.duration {
                let second = position as i64;
                if self.said_second.replace(second) != second {
                    ui.timeline.update_property(&[gtk::accessible::Property::ValueText(
                        &t::player_position_spoken(
                            &say::spoken_duration(position),
                            &say::spoken_duration(duration),
                        ),
                    )]);
                }
            }
        }
        if state.has_picture && ui.starting.is_visible() && self.start_leave.borrow().is_none()
        {
            self.leave_start();
        }
        // Six seconds of playback on, or a seek back past it, and "Resumed at" goes.
        if let (Some(from), Some(position)) = (self.resumed_from.get(), state.position)
            && (position > from + RESUMED_FOR || position + 1.0 < from)
        {
            self.resumed_done();
        }
        self.update_provisional();
        if let (Some(player), Some(duration)) = (&self.player, state.duration) {
            if self.engine_marks.borrow().is_some() {
                self.show_marks(duration);
            } else if state.has_picture && self.chapters_due.replace(false) {
                let chapters = player.mpv().get("chapter-list").map(|v| timeline::chapters(&v));
                ui.marked
                    .set(timeline::segments(&chapters.unwrap_or_default(), duration), duration);
            }
        }
        // Where an automatic skip landed, which Undo's 6 s of playback count from.
        if let (Some(mut undo), Some(position)) = (self.undo.get(), state.position)
            && undo.landed.is_none()
        {
            undo.landed = Some(position);
            self.undo.set(Some(undo));
        }
        if self.undo.get().is_some_and(|undo| !undo.open(state.position)) {
            self.forget_undo();
        }
        self.read_speed();
        let paused_now = state.paused && !self.state.borrow().paused;
        let moved = {
            let before = self.state.borrow();
            before.subtitle_frame != state.subtitle_frame
                || before.subtitle_placement != state.subtitle_placement
        };
        self.state.replace(state);
        // A new subtitle track, size or full screen: the lift is worked out again.
        if moved {
            self.lift(self.shown.get());
        }
        self.show_up_next();
        // Controls stay up while paused.
        if paused_now {
            self.wake();
        }
    }

    /// The speed mpv plays at, onto the speed button, its menu and the media controls.
    fn read_speed(&self) {
        let Some(speed) =
            self.player.as_ref().and_then(|p| p.mpv().get("speed")).and_then(|v| v.as_f64())
        else {
            return;
        };
        if (speed - self.speed.replace(speed)).abs() < 0.001 && self.ui.speed.label().is_some()
        {
            return;
        }
        let label = speed_label(speed);
        self.ui.speed.set_label(&label);
        self.ui.speed.set_tooltip_text(Some(&t::player_speed_is(&label)));
        self.speed_action.set_state(&speed.to_variant());
        if let Some(media) = self.media() {
            media.rate(speed);
        }
    }

    /// One step along the speed menu's speeds, faster or slower: `]` and `[`. Quick presses step
    /// from the speed asked for last, since mpv reports the change back a moment later.
    fn step_speed(&self, faster: bool) {
        let asked = self.speed_asked.get().map(|(speed, at)| (speed, at.elapsed()));
        if let Some(speed) = next_speed(step_from(asked, self.speed.get()), faster) {
            self.ask_speed(speed);
        }
    }

    fn ask_speed(&self, speed: f64) {
        self.speed_asked.set(Some((speed, Instant::now())));
        self.send(PlayerCommand::SpeedTo(speed));
    }

    /// Draw the engine's marks on a timeline `duration` long.
    fn show_marks(&self, duration: f64) {
        if let Some(marks) = self.engine_marks.borrow().as_ref() {
            self.ui.marked.set(marks.clone(), duration);
        }
    }

    /// The display the window is on, for the engine's upscaling.
    fn report_display(&self) {
        let Some(player) = &self.player else { return };
        let window = &self.ctx.window;
        let Some(surface) = window.surface() else { return };
        let Some(monitor) = WidgetExt::display(window).monitor_at_surface(&surface) else {
            return;
        };
        let geometry = monitor.geometry();
        let scale = monitor.scale();
        let refresh = monitor.refresh_rate();
        let compute =
            self.gl.get().is_some_and(|(major, minor, gles)| !gles && (major, minor) >= (4, 3));
        player.display(
            (f64::from(geometry.width()) * scale) as u32,
            (f64::from(geometry.height()) * scale) as u32,
            scale,
            (refresh > 0).then(|| f64::from(refresh) / 1000.0),
            compute,
        );
    }

    /// Show the controls, and hide them again after the idle time unless something holds them.
    fn wake(&self) {
        if !self.shown.replace(true) {
            for widget in self.chrome() {
                widget.set_visible(true);
            }
            self.ui.page.set_cursor(None);
            self.lift(true);
            self.animate(true);
            self.room.set_controls(true);
        }
        if let Some(source) = self.idle.borrow_mut().take() {
            source.remove();
        }
        let weak = self.self_weak();
        let source = glib::timeout_add_local_once(IDLE, move || {
            if let Some(view) = weak.upgrade() {
                view.idle.borrow_mut().take();
                view.rest();
            }
        });
        self.idle.replace(Some(source));
    }

    /// Whether keyboard focus is on one of the controls, which then stay up for it.
    fn focus_on_controls(&self) -> bool {
        let Some(focus) = GtkWindowExt::focus(&self.ctx.window) else { return false };
        let chrome = self.chrome();
        let mut at = Some(focus);
        while let Some(widget) = at {
            if chrome.iter().any(|c| c == &widget) {
                return true;
            }
            at = widget.parent();
        }
        false
    }

    /// Hide the controls, unless paused, scrubbing, a menu is open, the pointer is on a bar,
    /// keyboard focus is on a control or a screen reader runs.
    fn rest(&self) {
        let menu_open = self.ui.menu_buttons.iter().any(|b| b.is_active())
            || self.party.popover.is_visible()
            || self.ui.chat.holds_controls()
            || self.ui.side.is_open();
        let card = self.ui.stall.shown() || self.marking.borrow().is_some();
        if self.state.borrow().paused
            || self.over_bar.get()
            || menu_open
            || card
            || self.focus_on_controls()
            || crate::window::screen_reader_running()
            || self.start.borrow().is_some()
            || !self.shown.get()
        {
            if self.shown.get() {
                self.wake();
            }
            return;
        }
        self.hide_controls();
    }

    /// Take the controls away now, whatever holds them up.
    fn hide_controls(&self) {
        if !self.shown.replace(false) {
            return;
        }
        if let Some(source) = self.idle.borrow_mut().take() {
            source.remove();
        }
        self.ui.page.set_cursor_from_name(Some("none"));
        self.preview.hide();
        self.lift(false);
        self.animate(false);
        self.room.set_controls(false);
    }

    /// Subtitles above the bottom bar while it shows, as far as they need to clear it, at once.
    fn lift(&self, up: bool) {
        let Some(player) = &self.player else { return };
        let bar = if up && self.mini.get().is_none() {
            let height = self.ui.bottom.height();
            if height > 0 {
                height
            } else {
                self.ui.bottom.measure(gtk::Orientation::Vertical, -1).1
            }
        } else {
            0
        };
        let style = self.subtitle_style();
        let state = self.state.borrow();
        timeline::lift_subtitles(
            player.mpv(),
            bar,
            self.ui.stage.height(),
            style.as_ref(),
            (state.subtitle_frame.as_ref(), &state.subtitle_placement),
        );
    }

    /// The viewer's subtitle style, as the engine last said Settings.
    fn subtitle_style(&self) -> Option<kaeri_model::subtitle_style::SubtitleStyle> {
        self.ctx.hub.known.borrow().settings.as_ref().and_then(|s| s.subtitle_style.clone())
    }

    /// Draw subtitles in the viewer's style: the engine's options, the size, font, colour, edge,
    /// background and raise, over the lift above the controls.
    fn style_subtitles(&self) {
        let (Some(player), Some(style)) = (&self.player, self.subtitle_style()) else {
            return;
        };
        timeline::style_subtitles(player.mpv(), &style, 0.0);
        self.lift(self.shown.get());
    }

    /// Stats for nerds up or away, for the rest of this window's playbacks.
    fn toggle_stats(&self) {
        let shown = !self.stats.shown();
        self.stats.set_shown(shown);
        self.stats_action.set_state(&shown.to_variant());
        self.send(PlayerCommand::Stats(shown));
    }

    /// For a shot: Stats for nerds up with the engine's `fixture` held in place of its figures,
    /// `waited` before the file opened, and "Copied" up when `copied`.
    #[cfg(feature = "fixtures")]
    pub fn shot_stats(
        &self,
        fixture: Option<kaeri_model::hosted::StatsFixture>,
        waited: Option<f64>,
        copied: bool,
    ) {
        if !self.stats.shown() {
            self.toggle_stats();
        }
        if let Some(fixture) = fixture {
            self.stats.stage(
                kaeri_engine::playback::stats::fixture(fixture),
                waited.map(Duration::from_secs_f64),
                copied,
            );
        }
    }

    /// For a shot: Stats for nerds away again.
    #[cfg(feature = "fixtures")]
    pub fn shot_stats_away(&self) {
        if self.stats.shown() {
            self.toggle_stats();
        }
    }

    /// P and the mini button: the other way. The window takes a moment to change size and
    /// decoration, and a toggle before it has landed would read a size that is on its way
    /// somewhere else, so it waits.
    fn toggle_mini(&self) {
        if !mini_ready(self.mini_at.get().map(|at| at.elapsed())) {
            return;
        }
        self.set_mini(self.mini.get().is_none());
    }

    /// The mini player: the window itself, small, undecorated and with three controls, or back
    /// to what it was.
    pub fn set_mini(&self, on: bool) {
        if on {
            self.close_panel();
        }
        let window = &self.ctx.window;
        self.mini_at.set(Some(Instant::now()));
        match (on, self.mini.get()) {
            (true, None) => {
                let (width, height) = window.default_size();
                let now = Restore {
                    width: if width > 0 { width } else { window.width() },
                    height: if height > 0 { height } else { window.height() },
                    maximized: window.is_maximized(),
                    fullscreen: window.is_fullscreen(),
                };
                let kept = self
                    .frame
                    .get()
                    .filter(|(_, at)| at.elapsed() < SETTLE)
                    .map(|(frame, _)| frame);
                let frame = frame_to_keep(now, kept);
                self.frame.set(Some((frame, Instant::now())));
                self.mini.set(Some(frame));
                // The bars go; the mini controls stand in for them.
                for widget in self.chrome() {
                    widget.set_visible(false);
                }
                self.ui.top.set_visible(false);
                self.ui.bottom.set_visible(false);
                // The corner cards would cover the small picture: Up Next still plays on.
                self.ui.cards.set_visible(false);
                self.stats.set_mini(true);
                window.unfullscreen();
                window.unmaximize();
                window.set_decorated(false);
                window.set_size_request(MINI.0, MINI.1);
                window.set_default_size(MINI.0, MINI.1);
                self.shown.set(false);
                self.preview.hide();
                self.lift(false);
                self.wake();
            }
            (false, Some(restore)) => {
                self.mini.set(None);
                self.ui.mini.set_visible(false);
                self.ui.cards.set_visible(true);
                self.stats.set_mini(false);
                window.set_decorated(true);
                window.set_size_request(360, 360);
                window.set_default_size(restore.width, restore.height);
                if restore.maximized {
                    window.maximize();
                }
                if restore.fullscreen {
                    window.fullscreen();
                }
                self.shown.set(false);
                self.wake();
            }
            _ => {}
        }
    }

    fn chrome(&self) -> Vec<gtk::Widget> {
        if self.mini.get().is_some() {
            return vec![self.ui.mini.clone().upcast(), self.ui.dim.clone().upcast()];
        }
        // The panel keeps the header and the dim; the bottom bar stays away under it.
        if self.start.borrow().is_some() || self.ui.side.is_open() {
            return vec![self.ui.top.clone().upcast(), self.ui.dim.clone().upcast()];
        }
        vec![
            self.ui.top.clone().upcast(),
            self.ui.bottom.clone().upcast(),
            self.ui.dim.clone().upcast(),
        ]
    }

    /// Fade the bars and the dim in or out, from wherever a fade under way had got to: in over
    /// 200 ms ease-out, out over 300 ms ease-in. Faded out, they are unmapped.
    fn animate(&self, show: bool) {
        if let Some(old) = self.fade.borrow_mut().take() {
            old.pause();
        }
        let chrome = self.chrome();
        let now = chrome.first().map_or(if show { 0.0 } else { 1.0 }, |w| w.opacity());
        let reduced = !gtk::Settings::default().is_some_and(|s| s.is_gtk_enable_animations());
        let target = adw::CallbackAnimationTarget::new({
            let chrome = chrome.clone();
            move |value| {
                for widget in &chrome {
                    widget.set_opacity(value);
                }
            }
        });
        let (to, ms, easing) = if show {
            (1.0, FADE_IN_MS, adw::Easing::EaseOutCubic)
        } else {
            (0.0, FADE_OUT_MS, adw::Easing::EaseInCubic)
        };
        let animation =
            crate::motion::timed(&self.ui.page, now, to, if reduced { 0 } else { ms }, target);
        animation.set_easing(easing);
        if !show {
            let weak = self.self_weak();
            animation.connect_done(move |_| {
                if let Some(view) = weak.upgrade()
                    && !view.shown.get()
                {
                    for widget in view.chrome() {
                        widget.set_visible(false);
                    }
                }
            });
        }
        animation.play();
        self.fade.replace(Some(animation));
    }

    fn self_weak(&self) -> Weak<Self> {
        self.me.clone()
    }

    /// The episodes menu and the settings menu, from what the engine offers.
    fn rebuild_menus(&self) {
        let Some((episodes, sources, upscaling, night)) = self.menus.borrow().clone() else {
            return;
        };
        self.refresh_panel_with(&episodes, self.protecting());
        let current = episodes.iter().find(|e| e.current).map(|e| e.number.clone());
        self.episode_action.set_state(&current.unwrap_or_default().to_variant());

        let menu = gio::Menu::new();
        let skips_menu = gio::Menu::new();
        skips_menu.append(Some(&t::player_mark_skip_times()), Some("player.mark-skips"));
        skips_menu.append(Some(&t::player_skip_times()), Some("player.skip-times"));
        menu.append_section(None, &skips_menu);
        let source_menu = gio::Menu::new();
        for source in &sources {
            source_menu.append_item(&widgets::menu_item(
                &source.label,
                "player.source",
                &source.id,
            ));
        }
        if !sources.is_empty() {
            menu.append_section(Some(&t::show_settings_source()), &source_menu);
        }
        let current = sources.iter().find(|s| s.current).map(|s| s.id.clone());
        self.source_action.set_state(&current.unwrap_or_default().to_variant());
        let picture = gio::Menu::new();
        for choice in &upscaling.options {
            picture.append_item(&widgets::menu_item(
                &choice.label,
                "player.upscaling",
                &choice.id,
            ));
        }
        self.upscaling_action.set_state(&upscaling.current.to_variant());
        menu.append_section(Some(&t::player_upscaling()), &picture);
        let night_menu = gio::Menu::new();
        night_menu.append(Some(&t::player_night_mode()), Some("player.night"));
        self.night_action.set_state(&night.to_variant());
        menu.append_section(None, &night_menu);
        if self.ctx.tv.available() {
            let tv_menu = gio::Menu::new();
            tv_menu.append(Some(&t::tv_play_on_menu()), Some("player.tv"));
            menu.append_section(None, &tv_menu);
        }
        if self.late.get() && self.discussions_on() {
            let talk = gio::Menu::new();
            talk.append(Some(&t::discussion_open()), Some("player.discussion"));
            menu.append_section(None, &talk);
        }
        let stats_menu = gio::Menu::new();
        stats_menu.append(Some(&t::player_stats()), Some("player.stats"));
        menu.append_section(None, &stats_menu);
        self.ui.settings.set_menu_model(Some(&menu));
        widgets::mark_items(&self.ui.settings, "player.source", "amber-check");
    }

    fn wire(self: &Rc<Self>) {
        let ui = &self.ui;
        ui.start.connect_action({
            let weak = Rc::downgrade(self);
            Rc::new(move |action| {
                if let Some(view) = weak.upgrade() {
                    view.start_action(action);
                }
            })
        });
        ui.start.connect_translation({
            let weak = Rc::downgrade(self);
            Rc::new(move |translation| {
                if let Some(view) = weak.upgrade() {
                    view.send(PlayerCommand::Choose(PlayerChoice::Translation(translation)));
                }
            })
        });
        // The Episodes and Sources panel: its button, what it asks for, and a click on the dimmed
        // video beside it, which closes it.
        ui.side.connect_act({
            let weak = Rc::downgrade(self);
            move |act| {
                if let Some(view) = weak.upgrade() {
                    view.panel_act(act);
                }
            }
        });
        ui.episodes.connect_clicked({
            let weak = Rc::downgrade(self);
            move |_| {
                if let Some(view) = weak.upgrade() {
                    view.toggle_panel();
                }
            }
        });
        let catch = gtk::GestureClick::new();
        catch.connect_released({
            let weak = Rc::downgrade(self);
            move |gesture, _, _, _| {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                if let Some(view) = weak.upgrade() {
                    view.close_panel();
                }
            }
        });
        ui.catcher.add_controller(catch);
        let weak = Rc::downgrade(self);
        let act = move |f: fn(&PlayerView)| {
            let weak = weak.clone();
            move || {
                if let Some(view) = weak.upgrade() {
                    f(&view);
                }
            }
        };

        // The page's actions, for its buttons and menus.
        let group = gio::SimpleActionGroup::new();
        let add = |name: &str, kind: Option<&glib::VariantTy>, f: Handler| {
            let action = gio::SimpleAction::new(name, kind);
            let weak = Rc::downgrade(self);
            action.connect_activate(move |_, arg| {
                if let Some(view) = weak.upgrade() {
                    f(&view, arg);
                    view.wake();
                }
            });
            group.add_action(&action);
        };
        add("back", None, Box::new(|v, _| v.close()));
        add("toggle", None, Box::new(|v, _| v.send(PlayerCommand::TogglePause)));
        add("back-10", None, Box::new(|v, _| v.send(PlayerCommand::Seek(-10.0))));
        add("forward-10", None, Box::new(|v, _| v.send(PlayerCommand::Seek(10.0))));
        add("fullscreen", None, Box::new(|v, _| v.fullscreen(None)));
        add("mini", None, Box::new(|v, _| v.toggle_mini()));
        add("skip", None, Box::new(|v, _| v.send(PlayerCommand::TakeSkip)));
        add(
            "undo-skip",
            None,
            Box::new(|v, _| {
                v.undo_skip();
            }),
        );
        add("mark-skips", None, Box::new(|v, _| v.toggle_marking()));
        add("skip-times", None, Box::new(|v, _| v.show_skip_times()));
        add("load-subtitle", None, Box::new(|v, _| v.load_track(TrackKind::Subtitle)));
        add("load-audio", None, Box::new(|v, _| v.load_track(TrackKind::Audio)));
        for (action, kind) in [
            (&self.subtitle_action, TrackKind::Subtitle),
            (&self.audio_action, TrackKind::Audio),
        ] {
            let weak = Rc::downgrade(self);
            action.connect_activate(move |_, arg| {
                if let (Some(view), Some(target)) =
                    (weak.upgrade(), arg.and_then(String::from_variant))
                {
                    view.choose_track(kind, &target);
                    view.wake();
                }
            });
            group.add_action(action);
        }
        add("party", None, Box::new(|v, _| v.show_party()));
        add("tv", None, Box::new(|v, _| v.play_on_tv()));
        self.ui.tv.set_visible(self.ctx.tv.available());
        // Once the TV has it, the player here closes.
        self.ctx.tv.on_handed_off({
            let weak = Rc::downgrade(self);
            move || {
                if let Some(view) = weak.upgrade()
                    && view.shell().is_some_and(|shell| shell.is_player())
                {
                    view.close();
                }
            }
        });
        self.party.popover.connect_map({
            let weak = Rc::downgrade(self);
            move |_| {
                if let Some(view) = weak.upgrade() {
                    view.party.refresh_chat();
                    view.ui.chat.popover(true);
                }
            }
        });
        self.ui.chat.on_marks({
            let marked = self.ui.marked.clone_handle();
            move |marks| marked.set_reactions(marks)
        });
        self.ui.chat.on_open_conversation({
            let weak = Rc::downgrade(self);
            move || {
                if let Some(view) = weak.upgrade() {
                    view.show_party();
                }
            }
        });
        self.party.popover.connect_closed({
            let weak = Rc::downgrade(self);
            move |_| {
                if let Some(view) = weak.upgrade() {
                    view.ui.chat.popover(false);
                    view.wake();
                }
            }
        });
        add(
            "copy-stats",
            None,
            Box::new(|v, _| v.send(PlayerCommand::CopyStats { about: stats::about() })),
        );
        // Discussion: playback pauses where it is, and stays paused when the sheet closes.
        add(
            "discussion",
            None,
            Box::new(|v, _| {
                let (Some(id), number) = (v.title_id.get(), v.episode_line.borrow().0.clone())
                else {
                    return;
                };
                if !v.state.borrow().paused {
                    v.send(PlayerCommand::TogglePause);
                }
                let _ = v.widget.activate_action(
                    "win.open-discussion",
                    Some(&(id.get(), number).to_variant()),
                );
            }),
        );
        add("play-next", None, Box::new(|v, _| v.play_next()));
        add("cancel-next", None, Box::new(|v, _| v.cancel_next()));
        add("stall-switch", None, Box::new(|v, _| v.send(PlayerCommand::StallSwitch)));
        add("stall-wait", None, Box::new(|v, _| v.send(PlayerCommand::StallWait)));
        add(
            "start-over",
            None,
            Box::new(|v, _| {
                v.send(PlayerCommand::SeekTo(0.0));
                v.resumed_done();
            }),
        );
        // The stateful ones: the menus show their state as radio items and a check, and the
        // state follows what the engine and mpv then say, not the click.
        let stateful = |action: &gio::SimpleAction, f: Handler| {
            let weak = Rc::downgrade(self);
            action.connect_activate(move |_, arg| {
                if let Some(view) = weak.upgrade() {
                    f(&view, arg);
                    view.wake();
                }
            });
            group.add_action(action);
        };
        stateful(&self.stats_action, Box::new(|v, _| v.toggle_stats()));
        stateful(
            &self.night_action,
            Box::new(|v, _| {
                let on = v.menus.borrow().as_ref().is_some_and(|m| m.3);
                v.send(PlayerCommand::NightMode(!on));
            }),
        );
        stateful(
            &self.speed_action,
            Box::new(|v, a| {
                if let Some(speed) = a.and_then(f64::from_variant) {
                    v.ask_speed(speed);
                }
            }),
        );
        stateful(
            &self.episode_action,
            Box::new(|v, a| {
                if let Some(n) = a.and_then(String::from_variant) {
                    v.send(PlayerCommand::Choose(PlayerChoice::Episode(n)));
                }
            }),
        );
        stateful(
            &self.source_action,
            Box::new(|v, a| {
                if let Some(id) = a.and_then(String::from_variant) {
                    v.send(PlayerCommand::Choose(PlayerChoice::Source(id)));
                }
            }),
        );
        stateful(
            &self.upscaling_action,
            Box::new(|v, a| {
                if let Some(id) = a.and_then(String::from_variant) {
                    v.send(PlayerCommand::Upscaling(id));
                }
            }),
        );
        stateful(
            &self.translation_action,
            Box::new(|v, a| {
                let Some(target) = a.and_then(String::from_variant) else { return };
                let translation =
                    if target == "dub" { Translation::Dub } else { Translation::Sub };
                if v.translation_action.state().and_then(|s| s.get::<String>()).as_deref()
                    != Some(translation.as_str())
                {
                    v.send(PlayerCommand::Choose(PlayerChoice::Translation(translation)));
                }
                v.ui.tracks.popdown();
            }),
        );
        ui.page.insert_action_group("player", Some(&group));
        // The track menu is read from the file each time it opens.
        ui.tracks.set_create_popup_func({
            let weak = Rc::downgrade(self);
            move |_| {
                if let Some(view) = weak.upgrade() {
                    view.fill_tracks();
                }
            }
        });

        // A drag on the timeline seeks.
        ui.timeline.connect_change_value({
            let weak = Rc::downgrade(self);
            move |_, _, value| {
                if let Some(view) = weak.upgrade() {
                    view.held_until.set(Instant::now() + Duration::from_millis(400));
                    view.send(PlayerCommand::SeekTo(value));
                    view.wake();
                }
                glib::Propagation::Proceed
            }
        });
        // Hovering the timeline shows the frame there.
        let hover = gtk::EventControllerMotion::new();
        hover.connect_motion({
            let weak = Rc::downgrade(self);
            move |_, x, _| {
                let Some(view) = weak.upgrade() else { return };
                let has_duration = view.state.borrow().duration.is_some_and(|d| d > 0.0);
                if let Some(seconds) = timeline::time_at(&view.ui.timeline, x)
                    && has_duration
                {
                    let duration = view.state.borrow().duration.unwrap_or(0.0);
                    let tick = view.ui.marked.tick_label(seconds, duration);
                    view.preview.show(x, seconds, view.ui.marked.at(seconds), tick);
                }
            }
        });
        hover.connect_leave({
            let weak = Rc::downgrade(self);
            move |_| {
                if let Some(view) = weak.upgrade() {
                    view.preview.hide();
                }
            }
        });
        ui.timeline.add_controller(hover);
        ui.volume.connect_change_value({
            let weak = Rc::downgrade(self);
            move |_, _, value| {
                if let Some(view) = weak.upgrade() {
                    let now = view
                        .player
                        .as_ref()
                        .and_then(|p| p.mpv().get("volume"))
                        .and_then(|v| v.as_f64())
                        .unwrap_or(100.0);
                    view.send(PlayerCommand::Volume(value - now));
                }
                glib::Propagation::Proceed
            }
        });

        // The pointer shows the controls; over a bar it holds them.
        let motion = gtk::EventControllerMotion::new();
        let last = Cell::new((0.0, 0.0));
        let wake = act(|v| v.wake());
        motion.connect_motion(move |_, x, y| {
            // GTK reports a motion when the cursor changes; only a real move wakes.
            if last.replace((x, y)) != (x, y) {
                wake();
            }
        });
        ui.page.add_controller(motion);
        for bar in [ui.top.clone().upcast::<gtk::Widget>(), ui.bottom.clone().upcast()] {
            let hover = gtk::EventControllerMotion::new();
            let weak = Rc::downgrade(self);
            hover.connect_enter(move |_, _, _| {
                if let Some(view) = weak.upgrade() {
                    view.over_bar.set(true);
                }
            });
            let weak = Rc::downgrade(self);
            hover.connect_leave(move |_| {
                if let Some(view) = weak.upgrade() {
                    view.over_bar.set(false);
                    view.wake();
                }
            });
            bar.add_controller(hover);
        }

        // Click plays or pauses; double-click toggles fullscreen.
        let click = gtk::GestureClick::new();
        click.set_button(gdk::BUTTON_PRIMARY);
        let weak = Rc::downgrade(self);
        click.connect_pressed(move |gesture, presses, x, y| {
            let Some(view) = weak.upgrade() else { return };
            // Only on the video, not on a card or the panel: a double-click that plays a row
            // closes the panel, and its second press is not the video's.
            let picked = view.ui.stage.pick(x, y, gtk::PickFlags::DEFAULT);
            if picked.is_some_and(|w| {
                w.is_ancestor(&view.ui.cards) || w == view.ui.cards || view.ui.side.holds(&w)
            }) {
                return;
            }
            if gesture.current_event().is_some_and(|e| {
                e.device().is_some_and(|d| d.source() == gdk::InputSource::Touchscreen)
            }) {
                // A tap toggles the controls instead.
                if view.shown.get() {
                    view.rest_now()
                } else {
                    view.wake()
                }
                return;
            }
            if view.mini.get().is_some() {
                // The mini player is dragged by its picture; a double-click is the full window.
                if presses == 2 {
                    view.set_mini(false);
                } else {
                    view.wake();
                }
                return;
            }
            match presses {
                1 => view.send(PlayerCommand::TogglePause),
                2 => {
                    view.send(PlayerCommand::TogglePause);
                    view.fullscreen(None);
                }
                _ => {}
            }
        });
        ui.stage.add_controller(click);
        let drag = gtk::GestureDrag::new();
        let weak = Rc::downgrade(self);
        drag.connect_drag_begin(move |gesture, x, y| {
            let Some(view) = weak.upgrade() else { return };
            if view.mini.get().is_none() {
                return;
            }
            let window = &view.ctx.window;
            let (Some(surface), Some(device)) = (window.surface(), gesture.device()) else {
                return;
            };
            let Ok(toplevel) = surface.downcast::<gdk::Toplevel>() else { return };
            let point = view
                .ui
                .stage
                .compute_point(window, &gtk::graphene::Point::new(x as f32, y as f32))
                .unwrap_or_else(|| gtk::graphene::Point::new(x as f32, y as f32));
            let (dx, dy) = window.surface_transform();
            toplevel.begin_move(
                &device,
                gesture.current_button() as i32,
                f64::from(point.x()) + dx,
                f64::from(point.y()) + dy,
                gesture.current_event_time(),
            );
        });
        ui.stage.add_controller(drag);

        // The wheel steps the volume; horizontal scrolling seeks.
        let scroll =
            gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::BOTH_AXES);
        let weak = Rc::downgrade(self);
        scroll.connect_scroll(move |_, dx, dy| {
            let Some(view) = weak.upgrade() else { return glib::Propagation::Proceed };
            if dy.abs() > dx.abs() {
                view.send(PlayerCommand::Volume(-dy.signum() * 5.0));
            } else if dx != 0.0 {
                view.send(PlayerCommand::Seek(dx.signum() * 5.0));
            }
            view.wake();
            glib::Propagation::Stop
        });
        ui.page.add_controller(scroll);

        // The keys.
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(self);
        keys.connect_key_pressed(move |_, key, _, modifiers| {
            let Some(view) = weak.upgrade() else { return glib::Propagation::Proceed };
            if view.key(key, modifiers) {
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        ui.page.add_controller(keys);
    }

    /// For a shot: "Resumed at" as a resumed playback shows it, `at` said and the six seconds
    /// counted from where the test pattern is.
    #[cfg(feature = "fixtures")]
    pub fn shot_resumed(&self, at: f64) {
        self.resumed(at);
        self.resumed_from.set(self.state.borrow().position);
    }

    /// For a shot: Details on the starting screen, opened as the viewer opens it.
    pub fn shot_start_details(&self) {
        self.start_action(start_model::Action::Details);
    }

    /// For a shot: Play from the beginning on the starting screen, pressed as the viewer presses it.
    pub fn shot_start_from_beginning(&self) {
        self.start_action(start_model::Action::FromBeginning);
    }

    /// For a still shot: the starting screen as its clock's tick leaves it, at once, after the
    /// shot has moved the still clock on.
    #[cfg(feature = "fixtures")]
    pub fn shot_start_tick(&self) {
        if self.start.borrow().is_some() && self.ui.starting.is_visible() {
            self.tick_start();
        }
    }

    /// For a shot: the subtitle style panel, or with a subtitle, that on screen first.
    #[cfg(feature = "fixtures")]
    pub fn shot_style(&self, subtitle: Option<&str>) {
        if let (Some(player), Some(subtitle)) = (&self.player, subtitle) {
            let _ =
                player.mpv().command(&["sub-add", subtitle, "select", "Lantern Road", "eng"]); // i18n-skip
            return;
        }
        self.ui.style.popup();
        self.wake();
    }

    /// For a shot: the style panel's popover, under its button, over the window.
    pub fn shot_style_popover(&self) -> Option<(gtk::Widget, f32, f32)> {
        let popover = self.ui.style.popover()?;
        if !popover.is_visible() {
            return None;
        }
        let window = &self.ctx.window;
        let button = self.ui.style.compute_bounds(window)?;
        let width = popover.width() as f32;
        let x = (button.x() + button.width() / 2.0 - width / 2.0)
            .clamp(0.0, (window.width() as f32 - width).max(0.0));
        Some((popover.upcast(), x, button.y() + button.height()))
    }

    /// For a shot: an opening and an ending on the timeline, and `subtitle` on screen.
    #[cfg(feature = "fixtures")]
    pub fn shot_marks(&self, subtitle: &str) {
        let Some(player) = &self.player else { return };
        let list = serde_json::json!([
            { "title": "Episode", "time": 0.0 }, // i18n-skip
            { "title": "Opening", "time": 4.0 }, // i18n-skip
            { "title": "Episode", "time": 16.0 }, // i18n-skip
            { "title": "Ending", "time": 50.0 }, // i18n-skip
        ]);
        let _ = player.mpv().set("chapter-list", &list);
        let _ = player.mpv().command(&["sub-add", subtitle, "select"]);
        self.chapters_due.set(true);
    }

    /// For a shot: the Subtitles and Audio menu open, with `subtitle` added as a track.
    #[cfg(feature = "fixtures")]
    pub fn shot_tracks(&self, subtitle: Option<&str>) {
        if let (Some(player), Some(subtitle)) = (&self.player, subtitle) {
            let _ =
                player.mpv().command(&["sub-add", subtitle, "select", "Lantern Road", "eng"]); // i18n-skip
            return;
        }
        self.ui.tracks.popup();
        self.wake();
    }

    /// For a shot or the walkthrough: the board's show in the panel, the 7th of 13 playing from
    /// Example source, and the engine's side of its sources and switches scripted here. With
    /// `long`, 250 rows and the 230th playing, so the Episodes tab shows its ranges.
    #[cfg(feature = "fixtures")]
    pub fn shot_panel_board(&self, long: bool) {
        use kaeri_model::playback::SwitchTo;
        // The first launch's question would sit over the panel.
        crate::shots::close_dialogs(&self.ctx.window);
        let Some(id) = self.title_id.get() else { return };
        let stills: Vec<Option<String>> =
            self.table.borrow().iter().map(|row| row.thumbnail.clone()).collect();
        let mut rows = panel_demo::table(&stills);
        if long {
            let base = rows.clone();
            rows = (1..=250)
                .map(|n| {
                    let mut row = base[(n - 1) % 9].clone();
                    row.number = n.to_string();
                    row.title = Some(format!("Lamp number {n}")); // i18n-skip
                    row.completed = n < 230;
                    row.watched = if n < 230 {
                        1.0
                    } else if n == 230 {
                        0.5
                    } else {
                        0.0
                    };
                    row.past_progress = n > 230;
                    row.spoiler_ahead = n > 230;
                    row.airs_at = None;
                    row.delayed_from = None;
                    row
                })
                .collect();
        }
        let now = if long { "230".to_owned() } else { panel_demo::NOW.to_string() };
        let mut menu = panel_demo::menu(&rows);
        if long {
            menu = rows
                .iter()
                .map(|row| MenuEpisode {
                    number: row.number.clone(),
                    label: t::episode_number(row.number.parse().unwrap_or(0)),
                    title: row.title.clone(),
                    watched: row.completed,
                    filler: false,
                    current: row.number == now,
                })
                .collect();
        }
        let rows = Rc::new(RefCell::new(rows));
        self.table.replace(rows.borrow().clone());
        self.has_dub.set(Some(true));
        self.dub
            .replace(Some(kaeri_model::episodes::DubExtent { through: 8, complete: false }));
        {
            let mut line = self.episode_line.borrow_mut();
            line.0 = now.clone();
            line.1 =
                rows.borrow().iter().find(|r| r.number == now).and_then(|r| r.title.clone());
        }
        self.ui.show.set_label(panel_demo::SHOW);
        self.show_episode_line();
        if let Some(menus) = self.menus.borrow_mut().as_mut() {
            menus.0 = menu;
            menus.1 = panel_demo::streams("s2");
        }
        self.playing_from(panel_demo::PLAYING);
        self.rebuild_menus();

        // The engine's side: the listing and the releases, a watched mark, and each switch as
        // the script has it. A landing loads the same file again from where it was, as the
        // engine does, behind the held picture.
        let attempts: Rc<RefCell<std::collections::HashMap<String, u32>>> = Rc::default();
        let weak = self.self_weak();
        let answer = move |command: &Command| -> Option<Vec<(u64, Event)>> {
            let view = weak.upgrade()?;
            let playback = view.last_load.borrow().as_ref().map(|(p, ..)| *p)?;
            let at = view.state.borrow().position.unwrap_or(panel_demo::AT);
            let to = match command {
                Command::LoadSourceOptions(_) => {
                    return Some(vec![(
                        300,
                        Event::SourceOptions { id, options: panel_demo::options() },
                    )]);
                }
                Command::LoadSources { episode, .. } => {
                    return Some(vec![(
                        700,
                        Event::Sources {
                            id,
                            episode: episode.clone(),
                            candidates: panel_demo::releases(),
                            missed: Vec::new(),
                        },
                    )]);
                }
                Command::SetWatched { episodes, watched, .. } => {
                    for row in rows.borrow_mut().iter_mut() {
                        if episodes.contains(&row.number) {
                            row.completed = *watched;
                            row.watched = if *watched { 1.0 } else { 0.0 };
                        }
                    }
                    return Some(vec![(
                        200,
                        Event::Episodes { id, rows: rows.borrow().clone(), failed: None },
                    )]);
                }
                Command::SwitchSource { provider_id, source_id, .. } => SwitchTo {
                    provider_id: provider_id.clone(),
                    name: panel_demo::releases()
                        .into_iter()
                        .find(|c| &c.id == source_id)
                        .map(|c| c.provider_name)
                        .unwrap_or_default(),
                    release: Some(source_id.clone()),
                    stream: None,
                    translation: None,
                },
                Command::Player(PlayerCommand::Choose(PlayerChoice::Source(stream))) => {
                    SwitchTo {
                        provider_id: "example".into(),
                        name: panel_demo::PLAYING.into(),
                        release: None,
                        stream: Some(stream.clone()),
                        translation: None,
                    }
                }
                _ => return None,
            };
            let key = SwitchTarget::from(&to).key();
            let attempt = {
                let mut attempts = attempts.borrow_mut();
                let n = attempts.entry(key.clone()).or_default();
                *n += 1;
                *n
            };
            let ending = panel_demo::ending(&key, attempt);
            let mut events = Vec::new();
            for (ms, state) in panel_demo::script(&to, at, ending) {
                let lands = matches!(state, kaeri_model::playback::SourceSwitch::Landed { .. });
                if lands {
                    // The engine loads the new stream, and it lands once it plays.
                    if let Some((_, url, options)) = view.last_load.borrow().clone() {
                        let mut options = options;
                        options.retain(|o| o.name != "start");
                        options.push(kaeri_model::hosted::MpvOption {
                            name: "start".into(),
                            value: format!("{at:.3}"),
                        });
                        events.push((
                            ms.saturating_sub(900),
                            Event::PlayerOps {
                                playback,
                                ops: vec![PlayerOp::Load { url, options }],
                            },
                        ));
                    }
                    if let Some(stream) = &to.stream {
                        let (weak, stream) = (view.self_weak(), stream.clone());
                        glib::timeout_add_local_once(Duration::from_millis(ms), move || {
                            if let Some(view) = weak.upgrade() {
                                if let Some(menus) = view.menus.borrow_mut().as_mut() {
                                    menus.1 = panel_demo::streams(&stream);
                                }
                                view.rebuild_menus();
                            }
                        });
                    }
                }
                events.push((ms, Event::PlayerSwitch { playback, state }));
            }
            Some(events)
        };
        self.ctx.hub.panel_answer.replace(Some(Rc::new(answer)));
    }

    /// For the walkthrough: while `KAERI_QA_WIDGETS` names a file, the window's visible
    /// buttons, labels and rows with their bounds, and the focus, written to it twice a second,
    /// so a pointer can click what the app itself says is where. AT-SPI's extents sit off by up
    /// to 25 px in this GTK.
    #[cfg(feature = "fixtures")]
    pub fn qa_widgets(&self) {
        let Some(path) = std::env::var_os("KAERI_QA_WIDGETS") else { return };
        let window = self.ctx.window.clone();
        glib::timeout_add_local(Duration::from_millis(500), move || {
            let mut out = String::new();
            // Bounds are from the window widget; X clicks are from the surface, shadow included.
            let (sx, sy) = window.surface_transform();
            let focus = GtkWindowExt::focus(&window);
            if let Some(focus) = &focus {
                out.push_str(&format!("focus\t{}\t{}\n", focus.type_().name(), qa_text(focus)));
            }
            let mut stack = vec![window.clone().upcast::<gtk::Widget>()];
            while let Some(widget) = stack.pop() {
                if !widget.is_mapped() || !widget.is_child_visible() {
                    continue;
                }
                let kind = if widget.is::<gtk::ListBoxRow>() {
                    Some("row")
                } else if widget.is::<gtk::Button>() || widget.is::<gtk::ToggleButton>() {
                    Some("button")
                } else if widget.is::<gtk::Label>() {
                    Some("label")
                } else {
                    None
                };
                if let (Some(kind), Some(b)) = (kind, widget.compute_bounds(&window)) {
                    out.push_str(&format!(
                        "{kind}\t{:.0}\t{:.0}\t{:.0}\t{:.0}\t{}\t{}\n",
                        b.x() as f64 + sx,
                        b.y() as f64 + sy,
                        b.width(),
                        b.height(),
                        widget.widget_name(),
                        qa_text(&widget),
                    ));
                }
                let mut child = widget.last_child();
                while let Some(next) = child {
                    stack.push(next.clone());
                    child = next.prev_sibling();
                }
            }
            let _ = std::fs::write(&path, out);
            glib::ControlFlow::Continue
        });
    }

    /// For a shot or the walkthrough: the panel open on `tab`.
    #[cfg(feature = "fixtures")]
    pub fn shot_panel(&self, tab: &str) {
        let tab = PanelTab::from_name(tab);
        if self.ui.side.is_open() {
            self.ui.side.show_tab(tab, true);
        } else {
            self.open_panel(tab);
        }
    }

    /// For a shot: the panel itself, to stage its states.
    #[cfg(feature = "fixtures")]
    pub fn shot_side(&self) -> Rc<SidePanel> {
        Rc::clone(&self.ui.side)
    }

    /// For a shot: a pick in the Sources tab, as a click on it says it.
    #[cfg(feature = "fixtures")]
    pub fn shot_pick_release(&self, id: &str) {
        if let Some(candidate) = panel_demo::releases().into_iter().find(|c| c.id == id) {
            self.panel_act(Act::Release(candidate));
        }
    }

    /// For a shot: the notice an automatic skip of the opening shows, with its Undo.
    #[cfg(feature = "fixtures")]
    pub fn shot_auto_skipped(&self) {
        self.auto_skipped(SkipKind::Opening, 2.0);
        self.wake();
    }

    /// For a shot: the Subtitles and Audio menu's popover, under its button, over the window.
    pub fn shot_tracks_popover(&self) -> Option<(gtk::Widget, f32, f32)> {
        let popover = self.ui.tracks.popover()?;
        if !popover.is_visible() {
            return None;
        }
        let window = &self.ctx.window;
        let button = self.ui.tracks.compute_bounds(window)?;
        let width = popover.width() as f32;
        let x = (button.x() + button.width() / 2.0 - width / 2.0)
            .clamp(0.0, (window.width() as f32 - width).max(0.0));
        Some((popover.upcast(), x, button.y() + button.height()))
    }

    /// For a shot: the seek preview's popover and where it is over the window.
    pub fn shot_popover(&self) -> Option<(gtk::Widget, f32, f32)> {
        self.preview.placed(&self.ctx.window)
    }

    /// For a shot: the seek preview `fraction` of the way along, as hovering there shows it.
    #[cfg(feature = "fixtures")]
    pub fn shot_preview(&self, fraction: f64) {
        let Some(duration) = self.state.borrow().duration else { return };
        let Some(x) = timeline::x_at(&self.ui.timeline, fraction) else { return };
        let seconds = duration * fraction;
        let tick = self.ui.marked.tick_label(seconds, duration);
        self.preview.show(x, seconds, self.ui.marked.at(seconds), tick);
        self.wake();
    }

    /// Close whichever of the player's menus is open.
    pub fn close_menus(&self) {
        self.close_menus_only();
        self.close_panel();
    }

    /// Play on TV from the player: paused here, and carried on from here on the TV once it has
    /// started the episode.
    fn play_on_tv(&self) {
        let Some(id) = self.title_id.get() else { return };
        let episode = self.episode_line.borrow().0.clone();
        if episode.is_empty() {
            return;
        }
        let state = self.state.borrow().clone();
        if !state.paused {
            self.send(PlayerCommand::TogglePause);
        }
        if self.mini.get().is_some() {
            self.set_mini(false);
        }
        self.ctx.tv.offer(crate::tv::Request {
            id,
            title: self.ui.show.label().to_string(),
            episode,
            from: state.position.filter(|p| *p > 5.0),
            from_player: true,
        });
    }

    /// The party's panel when in one; the form that starts one when not.
    pub fn show_party(&self) {
        if self.ctx.hub.known.borrow().party.is_none() {
            crate::party::form::open(&self.ctx, crate::party::form::For::Playing);
            return;
        }
        self.wake();
        self.party.popover.popup();
    }

    /// The chat over the video, for the shots.
    #[cfg(feature = "fixtures")]
    pub fn chat(&self) -> Rc<chat::Chat> {
        Rc::clone(&self.ui.chat)
    }

    /// The people button as the party stands: a dot while in one, and its name.
    fn show_party_button(&self) {
        let in_party = self.ctx.hub.known.borrow().party.is_some();
        // The room moves on together: no Next, no Episodes and Sources panel, and the card has
        // no buttons.
        self.ui.up_buttons.set_visible(!in_party);
        if in_party {
            self.close_panel();
        }
        self.ui.episodes.set_sensitive(!in_party);
        self.ui.episodes.set_tooltip_text(Some(&if in_party {
            t::party_next_off()
        } else {
            t::player_panel()
        }));
        // Faded rather than added, so nothing moves.
        self.ui.people_dot.set_opacity(if in_party { 1.0 } else { 0.0 });
        let name = t::party_title();
        self.ui.people.set_tooltip_text(Some(&name));
        self.ui.people.update_property(&[gtk::accessible::Property::Label(&name)]);
    }

    /// For a shot: the room as the board frame `shot` draws it, over the episode playing.
    #[cfg(feature = "fixtures")]
    pub fn shot_room(&self, shot: &str) {
        use crate::room::fixture;
        let Some((name, open, full)) = fixture::frame(shot) else { return };
        let (view, conv, moves) = fixture::for_player(name);
        self.room.hold_clock(fixture::NOW);
        self.room.show(view, moves);
        self.room.show_conversation(&conv);
        if full {
            self.ctx.window.fullscreen();
            // Xvfb has no window manager to make the window fill the screen: the board's size.
            self.ctx.window.set_default_size(1280, 720);
        }
        // Nor to say the window went full screen.
        self.room.set_fullscreen(full);
        if !full && shot != "room-morph" {
            if self.ctx.window.default_width() < 1500 {
                self.ctx.window.set_default_size(1616, 767);
            }
            self.wake();
        }
        if shot == "room-morph" {
            // The recording: the room comes in, goes full screen at 3 s and back at 7 s.
            let (room, window) = (Rc::clone(&self.room), self.ctx.window.clone());
            glib::timeout_add_local_once(Duration::from_secs(3), move || {
                window.fullscreen();
                window.set_default_size(1280, 720);
                room.set_fullscreen(true);
                let room = Rc::clone(&room);
                glib::timeout_add_local_once(Duration::from_secs(4), move || {
                    window.unfullscreen();
                    window.set_default_size(1616, 767);
                    room.set_fullscreen(false);
                });
            });
        }
        if shot == "room-full-controls" {
            self.wake();
        } else if full {
            self.hide_controls();
        }
        if let Some(open) = open {
            let room = Rc::clone(&self.room);
            glib::timeout_add_local_once(Duration::from_millis(1500), move || {
                if open == "toggle-off" {
                    room.toggle.set_active(false);
                } else {
                    room.shot_open(open);
                }
            });
        }
    }

    /// For a shot: the room's open popover, under its button, over the window.
    pub fn shot_room_popover(&self) -> Option<(gtk::Widget, f32, f32)> {
        let window = &self.ctx.window;
        for button in [&self.room.column.invite, &self.room.column.details] {
            let Some(popover) = button.popover() else { continue };
            if !popover.is_visible() {
                continue;
            }
            let at = button.compute_bounds(window)?;
            let width = popover.width() as f32;
            let x = (at.x() + at.width() / 2.0 - width / 2.0)
                .clamp(0.0, (window.width() as f32 - width).max(0.0));
            return Some((popover.upcast(), x, at.y() + at.height()));
        }
        None
    }

    /// For a shot: the party's panel, under the people button, over the window.
    pub fn shot_party_popover(&self) -> Option<(gtk::Widget, f32, f32)> {
        let popover = &self.party.popover;
        if !popover.is_visible() {
            return None;
        }
        let window = &self.ctx.window;
        let button = self.ui.people.compute_bounds(window)?;
        let width = popover.width() as f32;
        let x = (button.x() + button.width() / 2.0 - width / 2.0)
            .clamp(0.0, (window.width() as f32 - width).max(0.0));
        Some((popover.clone().upcast(), x, button.y() + button.height()))
    }

    fn rest_now(&self) {
        if let Some(source) = self.idle.borrow_mut().take() {
            source.remove();
        }
        self.rest();
    }

    /// A key on the player. True when it was the player's.
    fn key(&self, key: gdk::Key, modifiers: gdk::ModifierType) -> bool {
        use gdk::Key;
        let shift = modifiers.contains(gdk::ModifierType::SHIFT_MASK);
        let step = if shift { 30.0 } else { 5.0 };
        // Arrows and S act without showing the controls; every other key shows them.
        let quiet =
            matches!(key, Key::Left | Key::Right | Key::Up | Key::Down | Key::s | Key::S);
        let control = modifiers.contains(gdk::ModifierType::CONTROL_MASK);
        if control && matches!(key, Key::z | Key::Z) && !shift {
            // Edit's Undo, for the last automatic skip.
            return self.undo_skip();
        }
        if modifiers == gdk::ModifierType::ALT_MASK && matches!(key, Key::k | Key::K) {
            self.toggle_marking();
            self.wake();
            return true;
        }
        // Ctrl+1 and Ctrl+2: the panel on Episodes or on Sources, opened or switched to.
        if modifiers == gdk::ModifierType::CONTROL_MASK
            && let Some(tab) = match key {
                Key::_1 | Key::KP_1 => Some(PanelTab::Episodes),
                Key::_2 | Key::KP_2 => Some(PanelTab::Sources),
                _ => None,
            }
        {
            self.panel_shortcut(tab);
            return true;
        }
        if modifiers.intersects(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::ALT_MASK) {
            return false;
        }
        // Tab and F6 bring the controls up before focus moves onto them.
        if matches!(key, Key::Tab | Key::ISO_Left_Tab | Key::F6) {
            self.wake();
            return false;
        }
        // The keys of a text field, an open party form or a focused button are its own.
        if self.party.popover.is_visible() {
            return false;
        }
        // With the panel open, Escape and E close it first. Inside it, the arrows, Enter and
        // the paging keys move through its lists; Space on a row still plays and pauses.
        if self.ui.side.is_open() {
            if matches!(key, Key::Escape | Key::e | Key::E) {
                self.close_panel();
                return true;
            }
            if let Some(focus) = GtkWindowExt::focus(&self.ctx.window)
                && self.ui.side.holds(&focus)
            {
                if key == Key::space {
                    if !focus.is::<gtk::ListBoxRow>() {
                        return false;
                    }
                    self.send(PlayerCommand::TogglePause);
                    return true;
                }
                if matches!(
                    key,
                    Key::Up
                        | Key::Down
                        | Key::Left
                        | Key::Right
                        | Key::Return
                        | Key::KP_Enter
                        | Key::Home
                        | Key::End
                        | Key::Page_Up
                        | Key::Page_Down
                ) {
                    return false;
                }
            }
        }
        if let Some(focus) = GtkWindowExt::focus(&self.ctx.window) {
            let button_key = matches!(key, Key::space | Key::Return | Key::KP_Enter);
            let mut at = Some(focus);
            while let Some(widget) = at {
                if widget.is::<gtk::Editable>()
                    || (button_key
                        && (widget.is::<gtk::Button>() || widget.is::<gtk::MenuButton>()))
                {
                    return false;
                }
                at = widget.parent();
            }
        }
        // While the bar is up on its first two steps the arrows move the playhead by its edge,
        // and Enter marks it or, on the review, saves.
        if let Some(handled) = self.marking_key(key, shift) {
            if handled {
                self.wake();
            }
            return handled;
        }
        // With a menu open its keys are its own: arrows move through it, Space and Enter pick,
        // and Escape closes it before anything else. T and E close the menu they opened.
        if let Some(open) = self.ui.menu_buttons.iter().find(|b| b.is_active()) {
            let toggles = match key {
                Key::t | Key::T => open == &self.ui.tracks,
                _ => false,
            };
            if toggles {
                open.popdown();
                return true;
            }
            return false;
        }
        match key {
            Key::space | Key::k | Key::K => self.send(PlayerCommand::TogglePause),
            Key::Left => self.send(PlayerCommand::Seek(-step)),
            Key::Right => self.send(PlayerCommand::Seek(step)),
            Key::j | Key::J => self.send(PlayerCommand::Seek(-10.0)),
            Key::l | Key::L => self.send(PlayerCommand::Seek(10.0)),
            Key::Up => self.send(PlayerCommand::Volume(5.0)),
            Key::Down => self.send(PlayerCommand::Volume(-5.0)),
            Key::m | Key::M => {
                if let Some(player) = &self.player {
                    let _ = player.mpv().command(&["cycle", "mute"]);
                }
            }
            Key::f | Key::F | Key::F11 if self.mini.get().is_none() => self.fullscreen(None),
            Key::p | Key::P => self.toggle_mini(),
            Key::s | Key::S if self.ui.skip.shown() => self.send(PlayerCommand::TakeSkip),
            Key::n => self.play_next(),
            Key::N if self.in_party() => {}
            Key::N => self.send(PlayerCommand::Step(-1)),
            Key::bracketleft => self.step_speed(false),
            Key::bracketright => self.step_speed(true),
            Key::BackSpace => self.ask_speed(1.0),
            Key::e | Key::E if self.in_party() => {}
            Key::e | Key::E => self.toggle_panel(),
            Key::c | Key::C if self.in_party() => {
                if !self.ui.chat.open_field() {
                    return false;
                }
            }
            Key::t | Key::T => self.ui.tracks.popup(),
            Key::i | Key::I => self.toggle_stats(),
            Key::u | Key::U => {
                if !self.undo_skip() {
                    return false;
                }
            }
            Key::Return | Key::KP_Enter => {
                if self.ui.skip.shown() {
                    self.send(PlayerCommand::TakeSkip);
                } else if self.ui.up_next.shown() {
                    self.play_next();
                } else if self.ui.stall.shown() {
                    self.send(PlayerCommand::StallSwitch);
                } else {
                    return false;
                }
            }
            Key::Escape => {
                if self.ui.up_next.shown() && self.mini.get().is_none() {
                    self.cancel_next();
                } else if self.ui.stall.shown() && self.mini.get().is_none() {
                    self.send(PlayerCommand::StallWait);
                } else if self.mini.get().is_some() {
                    self.set_mini(false);
                } else if self.ctx.window.is_fullscreen() {
                    self.ctx.window.unfullscreen();
                } else {
                    self.close();
                }
            }
            _ => return false,
        }
        if !quiet {
            self.wake();
        }
        true
    }
}

impl PlayerView {
    /// The player as a game controller's map reads it: whether the controls show, and the card
    /// over the video that takes A and B.
    pub fn pad_context(&self) -> PadContext {
        let card = if self.ui.up_next.shown() {
            Some(PlayerCard::UpNext)
        } else if self.ui.stall.shown() {
            Some(PlayerCard::Stall)
        } else if self.ui.skip.shown()
            && self.skip_since.get().is_some_and(|at| at.elapsed() < Duration::from_secs(8))
        {
            Some(PlayerCard::SkipFresh)
        } else {
            None
        };
        let menu_open = self.ui.menu_buttons.iter().any(|b| b.is_active());
        PadContext::Player { controls_shown: self.shown.get() || menu_open, card }
    }

    /// A controller moved along the controls: they stay up a while longer.
    pub fn pad_touch(&self) {
        if self.shown.get() {
            self.wake();
        }
    }

    /// B: the controls away, or the player closed when they are already away.
    pub fn pad_back(&self) {
        if self.shown.get() {
            self.hide_controls();
        } else {
            self.key(gdk::Key::Escape, gdk::ModifierType::empty());
        }
    }

    pub fn pad_command(&self, command: PlayerCommand) {
        self.send(command);
    }

    pub fn pad_panel(&self, panel: PlayerPanel) {
        match panel {
            PlayerPanel::Tracks => self.ui.tracks.popup(),
            PlayerPanel::Episodes => self.open_panel(PanelTab::Episodes),
            PlayerPanel::More => self.ui.settings.popup(),
            PlayerPanel::Stats => self.toggle_stats(),
        }
        self.wake();
    }

    pub fn pad_reveal(&self, target: PlayerFocus) {
        self.wake();
        match target {
            PlayerFocus::SeekLine => self.ui.timeline.grab_focus(),
            PlayerFocus::Episodes => self.ui.episodes.grab_focus(),
        };
    }

    pub fn pad_play_pause(&self) {
        self.send(PlayerCommand::TogglePause);
        self.wake();
    }

    /// The right trigger held: twice the speed. What it was is handed back to restore.
    pub fn pad_hold_speed(&self) -> f64 {
        let was = self.speed.get();
        self.send(PlayerCommand::SpeedTo(2.0));
        self.wake();
        was
    }

    pub fn pad_restore_speed(&self, speed: f64) {
        self.send(PlayerCommand::SpeedTo(speed));
    }
}

/// Every two seconds, how evenly frames were drawn and what mpv says of its own timing.
fn log_pacing(pacing: Rc<video::Pacing>, player: Rc<Player>) {
    glib::timeout_add_seconds_local(2, move || {
        let (fps, worst) = pacing.take();
        let mpv = player.mpv();
        let get = |p: &str| mpv.get(p).map(|v| v.to_string()).unwrap_or_else(|| "-".into());
        tracing::info!(
            drawn_fps = format!("{fps:.1}"),
            worst_gap_ms = format!("{worst:.1}"),
            video_fps = get("estimated-vf-fps"),
            drops = get("frame-drop-count"),
            hwdec = get("hwdec-current"),
            "pacing"
        );
        glib::ControlFlow::Continue
    });
}

/// "Episode 7 · Dub · The Lantern Road": a dub marked on the episode line, a sub not, and the
/// name when there is one.
fn episode_line(number: &str, translation: Option<Translation>, title: Option<&str>) -> String {
    let line = say::marked(
        t::episode_number(crate::party::number(number)),
        translation == Some(Translation::Dub),
    );
    match title {
        Some(title) if !title.is_empty() => [line.as_str(), title].join(" · "),
        _ => line,
    }
}

/// A speed as its button and menu say it: "1×", "1.25×".
fn speed_label(speed: f64) -> String {
    let text = format!("{speed:.2}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    format!("{text}×")
}

/// A radio action on string targets, standing on `state`.
fn radio(name: &str, state: &str) -> gio::SimpleAction {
    gio::SimpleAction::new_stateful(name, Some(glib::VariantTy::STRING), &state.to_variant())
}

/// A corner card over the video, faded in and out with the fade tokens.
struct Card {
    widget: gtk::Box,
    fade: crate::motion::Fader,
}

impl Card {
    fn new() -> Self {
        Self { widget: card(), fade: crate::motion::Fader::default() }
    }

    fn shown(&self) -> bool {
        self.fade.shown()
    }

    fn set(&self, show: bool) {
        self.fade.set(&self.widget, show);
    }

    fn hide_now(&self) {
        self.fade.hide_now(&self.widget);
    }
}

/// Fade `widget` in or out, unmapping it once it is out.
fn fade(widget: &gtk::Box, show: bool) {
    let target = adw::PropertyAnimationTarget::new(widget, "opacity");
    let (from, to) = if show { (0.0, 1.0) } else { (widget.opacity(), 0.0) };
    let animation = crate::motion::timed(widget, from, to, 300, target);
    if !show {
        let widget = widget.clone();
        animation.connect_done(move |_| widget.set_visible(false));
    } else {
        widget.set_visible(true);
    }
    animation.play();
}

fn icon_button(icon: &str, tip: &str, action: &str) -> gtk::Button {
    let button = gtk::Button::from_icon_name(icon);
    button.set_tooltip_text(Some(tip));
    button.set_action_name(Some(action));
    button.add_css_class("flat");
    button.add_css_class("circular");
    button.set_valign(gtk::Align::Center);
    button
}

fn card() -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 8);
    card.add_css_class("corner-card");
    card.set_visible(false);
    card
}

/// The Episodes and Sources panel: opening and closing it, what it is handed, what it asks
/// for, and a switch to another source with the picture held while it loads.
impl PlayerView {
    fn toggle_panel(&self) {
        if self.ui.side.is_open() {
            self.close_panel();
        } else {
            self.open_panel(PanelTab::Episodes);
        }
    }

    /// Ctrl+1 and Ctrl+2: the panel on `tab`, opened or switched to, with the keyboard in it.
    fn panel_shortcut(&self, tab: PanelTab) {
        if self.ui.side.is_open() {
            self.ui.side.show_tab(tab, true);
        } else {
            self.open_panel(tab);
        }
    }

    /// The window's Ctrl and a number, which pick a section outside the player: here 1 and 2
    /// bring up the panel's tabs, and the others do nothing.
    pub fn section_shortcut(&self, n: i32) {
        match n {
            1 => self.panel_shortcut(PanelTab::Episodes),
            2 => self.panel_shortcut(PanelTab::Sources),
            _ => {}
        }
    }

    /// Slide the panel in on `tab`: the header stays, the bottom bar goes, the video dims.
    fn open_panel(&self, tab: PanelTab) {
        if self.in_party() || self.start.borrow().is_some() || self.mini.get().is_some() {
            return;
        }
        self.close_menus_only();
        let side = &self.ui.side;
        if !side.is_open() {
            self.refresh_panel();
            // Under the header bar, which stays: its natural height, as a hidden bar has none.
            let head = self.ui.top.measure(gtk::Orientation::Vertical, -1).1;
            side.widget.set_margin_top(head.max(self.ui.top.height()));
            self.ui.catcher.set_visible(true);
            self.ui.episodes.update_state(&[gtk::accessible::State::Expanded(Some(true))]);
            self.wake();
            // The bottom bar fades out; the controls stay up while the panel is open.
            let bottom = self.ui.bottom.clone();
            let fade = crate::motion::timed(
                &bottom,
                bottom.opacity(),
                0.0,
                FADE_OUT_MS,
                crate::motion::opacity(&bottom),
            );
            fade.connect_done(move |_| bottom.set_visible(false));
            fade.play();
            self.ui.side.widget.set_visible(true);
        }
        side.open(tab);
    }

    pub fn close_panel(&self) {
        let side = &self.ui.side;
        if !side.is_open() {
            return;
        }
        let focus_was_in =
            GtkWindowExt::focus(&self.ctx.window).is_some_and(|f| side.holds(&f));
        side.close();
        self.ui.catcher.set_visible(false);
        self.ui.episodes.update_state(&[gtk::accessible::State::Expanded(Some(false))]);
        if self.shown.get() && self.mini.get().is_none() && self.start.borrow().is_none() {
            self.ui.bottom.set_visible(true);
            let to = self.ui.top.opacity();
            let fade = crate::motion::timed(
                &self.ui.bottom,
                0.0,
                to,
                FADE_IN_MS,
                crate::motion::opacity(&self.ui.bottom),
            );
            fade.play();
        }
        if focus_was_in {
            self.ui.episodes.grab_focus();
        }
        self.wake();
    }

    /// The other menus closed, the panel left as it is.
    fn close_menus_only(&self) {
        for button in &self.ui.menu_buttons {
            button.popdown();
        }
        self.party.popover.popdown();
        self.room.column.invite.popdown();
        self.room.column.details.popdown();
    }

    /// Whether spoiler protection is on for the show playing.
    fn protecting(&self) -> bool {
        self.own_spoilers.get().unwrap_or_else(|| {
            self.ctx
                .hub
                .known
                .borrow()
                .setting(kaeri_model::settings::SettingKey::SpoilerProtection)
                .is_none_or(|r| crate::catalog::is_on(&r))
        })
    }

    fn refresh_panel(&self) {
        let episodes = self.menus.borrow().as_ref().map(|m| m.0.clone()).unwrap_or_default();
        self.refresh_panel_with(&episodes, self.protecting());
    }

    /// Hand the panel the table, the menus and what is known of the title's sources.
    fn refresh_panel_with(&self, episodes: &[MenuEpisode], protect: bool) {
        let old = self.ui.side.data().clone();
        let current =
            episodes.iter().find(|e| e.current).map(|e| e.number.clone()).or_else(|| {
                Some(self.episode_line.borrow().0.clone()).filter(|n| !n.is_empty())
            });
        let streams = self.menus.borrow().as_ref().map(|m| m.1.clone()).unwrap_or_default();
        let translation = match self.translation_action.state().and_then(|s| s.get::<String>())
        {
            Some(name) if name == "dub" => Translation::Dub,
            _ => Translation::Sub,
        };
        let episode_changed = old.current != current;
        self.ui.side.set_data(side_panel::Data {
            table: self.table.borrow().clone(),
            current,
            protect,
            streams,
            translation,
            options: old.options,
            releases: if episode_changed { None } else { old.releases },
            playing_source: old.playing_source,
            playing_release: if episode_changed { None } else { old.playing_release },
            has_dub: self.has_dub.get(),
            dub: *self.dub.borrow(),
        });
        if episode_changed && self.ui.side.is_open() && self.ui.side.tab() == PanelTab::Sources
        {
            self.load_panel_sources();
        }
    }

    /// The Sources tab came up: the title's sources and the episode's releases, asked once an
    /// episode.
    fn load_panel_sources(&self) {
        let Some(id) = self.title_id.get() else { return };
        let episode = self.episode_line.borrow().0.clone();
        if episode.is_empty() || id == kaeri_model::opened::UNTRACKED {
            return;
        }
        let asked = (id, episode.clone());
        if self.panel_asked.borrow().as_ref() == Some(&asked) {
            return;
        }
        self.panel_asked.replace(Some(asked));
        self.ui.side.listing_asked();
        self.ctx.hub.send(Command::LoadSourceOptions(id));
        self.ctx.hub.send(Command::LoadSources { id, episode });
    }

    /// What the panel asks for.
    fn panel_act(&self, act: Act) {
        let id = self.title_id.get();
        match act {
            Act::Close => self.close_panel(),
            Act::Tab(PanelTab::Sources) => self.load_panel_sources(),
            Act::Tab(PanelTab::Episodes) => {}
            Act::Episode(action, number) => {
                match action {
                    EpisodeAction::MarkWatched => {
                        if let Some(id) = id {
                            self.ctx.hub.send(Command::SetWatched {
                                id,
                                episodes: vec![number],
                                watched: true,
                            });
                        }
                    }
                    EpisodeAction::FromBeginningPlaying => {
                        self.send(PlayerCommand::SeekTo(0.0));
                    }
                    EpisodeAction::Play | EpisodeAction::Resume => {
                        self.send(PlayerCommand::Choose(PlayerChoice::Episode(number)));
                    }
                    EpisodeAction::FromBeginning => match id {
                        Some(id) => self.ctx.hub.send(Command::Play {
                            id,
                            episode: number,
                            from_start: true,
                        }),
                        None => self.send(PlayerCommand::Choose(PlayerChoice::Episode(number))),
                    },
                }
                if action.plays() {
                    self.close_panel();
                }
            }
            Act::Release(candidate) => {
                let Some(id) = id else { return };
                let episode = self.episode_line.borrow().0.clone();
                self.ui.side.asked(SwitchTarget::release(&candidate), Some(candidate.clone()));
                self.ctx.hub.send(Command::SwitchSource {
                    id,
                    episode,
                    provider_id: candidate.provider_id,
                    source_id: candidate.id,
                });
            }
            Act::Stream(stream) => {
                let name = self.ui.side.data().playing_source.clone().unwrap_or_default();
                self.ui.side.asked(SwitchTarget::stream(&stream, &name), None);
                self.send(PlayerCommand::Choose(PlayerChoice::Source(stream)));
            }
            Act::Translation(translation) => {
                // The engine finds the other version while the episode plays on.
                if self.ui.side.version_asked(translation) {
                    self.send(PlayerCommand::Choose(PlayerChoice::Translation(translation)));
                }
            }
        }
    }

    /// What the engine says of a switch to another source: the panel's row follows it, and the
    /// picture holds from its load until it lands or fails.
    fn switched(&self, state: &kaeri_model::playback::SourceSwitch) {
        use kaeri_model::playback::SourceSwitch;
        match state {
            SourceSwitch::Finding { to } => {
                tracing::info!(provider = %to.provider_id, "switch: finding");
                match to.translation {
                    Some(version) => self.ui.side.versioning(version, Phase::Finding),
                    None => self.ui.side.switching(SwitchTarget::from(to), Phase::Finding),
                }
            }
            SourceSwitch::Loading { to, at } => {
                tracing::info!(provider = %to.provider_id, at, "switch: loading, holding the picture");
                match to.translation {
                    Some(version) => self.ui.side.versioning(version, Phase::Loading),
                    None => self.ui.side.switching(SwitchTarget::from(to), Phase::Loading),
                }
                self.hold_picture(*at);
            }
            SourceSwitch::Landed { to, at } => {
                tracing::info!(provider = %to.provider_id, at, "switch: landed");
                let target = SwitchTarget::from(to);
                {
                    let mut data = self.ui.side.data().clone();
                    data.playing_source = Some(to.name.clone());
                    if let Some(release) = &to.release {
                        data.playing_release = self.ui.side.release(&to.provider_id, release);
                    } else if !panel_state::keeps_release(to, data.playing_release.as_ref()) {
                        // The other version, or a stream of another source the engine moved
                        // to: the release picked before is not what plays.
                        data.playing_release = None;
                    }
                    self.ui.side.set_data(data);
                }
                if to.translation.is_some() {
                    self.ui.side.version_done();
                    // The other version's releases are the ones to list now.
                    if self.panel_asked.take().is_some() {
                        self.load_panel_sources();
                    }
                } else {
                    self.ui.side.landed(&target);
                }
                self.release_hold(*at);
                if !self.ui.side.is_open() && to.release.is_some() {
                    self.ctx.toast(&t::notice_playing_from_source(&to.name, &say::clock(*at)));
                }
            }
            SourceSwitch::Failed { to, at, text, .. } => {
                tracing::info!(provider = %to.provider_id, at, "switch: did not start");
                match to.translation {
                    Some(version) => {
                        self.ui.side.versioning(version, Phase::Failed(text.clone()));
                        // Nothing else says it while the panel is closed.
                        if !self.ui.side.is_open() {
                            self.ctx.toast(&say::msg(text));
                        }
                    }
                    None => self
                        .ui
                        .side
                        .switching(SwitchTarget::from(to), Phase::Failed(text.clone())),
                }
                self.release_hold(*at);
            }
        }
    }

    /// Copy the picture on screen over the video: mpv clears it as the switch loads.
    fn hold_picture(&self, at: f64) {
        self.hold_at.set(Some(at));
        if let Some(fade) = self.held_fade.take() {
            fade.pause();
        }
        let Some(video) = self.ui.stage.child() else { return };
        let (width, height) = (video.width(), video.height());
        if width <= 0 || height <= 0 {
            return;
        }
        let paintable = gtk::WidgetPaintable::new(Some(&video));
        let snapshot = gtk::Snapshot::new();
        paintable.snapshot(&snapshot, f64::from(width), f64::from(height));
        let Some(node) = snapshot.to_node() else { return };
        let Some(renderer) = video.native().and_then(|n| n.renderer()) else { return };
        let bounds = gtk::graphene::Rect::new(0.0, 0.0, width as f32, height as f32);
        let texture = renderer.render_texture(&node, Some(&bounds));
        self.ui.held.set_paintable(Some(&texture));
        self.ui.held.set_opacity(1.0);
        self.ui.held.set_visible(true);
    }

    /// The new picture is up, or the old one again: the held one crossfades away over it.
    fn release_hold(&self, _at: f64) {
        self.hold_at.set(None);
        if !self.ui.held.is_visible() {
            return;
        }
        let held = self.ui.held.clone();
        let fade = crate::motion::timed(
            &held,
            held.opacity(),
            0.0,
            300,
            crate::motion::opacity(&held),
        );
        fade.set_easing(adw::Easing::EaseOutCubic);
        let weak = self.self_weak();
        fade.connect_done(move |_| {
            if let Some(view) = weak.upgrade()
                && view.hold_at.get().is_none()
            {
                view.ui.held.set_visible(false);
                view.ui.held.set_paintable(None::<&gdk::Paintable>);
            }
        });
        fade.play();
        self.held_fade.replace(Some(fade));
    }

    /// The source a start found, named for the panel's Playing now.
    fn playing_from(&self, source: &str) {
        let mut data = self.ui.side.data().clone();
        if data.playing_source.as_deref() != Some(source) {
            data.playing_source = Some(source.to_owned());
            data.playing_release = None;
            self.ui.side.set_data(data);
        }
    }
}

/// For the walkthrough: what a widget says, its own text or its labels', and its tooltip.
#[cfg(feature = "fixtures")]
fn qa_text(widget: &gtk::Widget) -> String {
    let mut words = Vec::new();
    let mut stack = vec![widget.clone()];
    while let Some(w) = stack.pop() {
        if let Some(label) = w.downcast_ref::<gtk::Label>()
            && w.is_visible()
        {
            words.push(label.text().to_string());
        }
        let mut child = w.last_child();
        while let Some(next) = child {
            stack.push(next.clone());
            child = next.prev_sibling();
        }
    }
    if let Some(tip) = widget.tooltip_text() {
        words.push(tip.to_string());
    }
    words.join(" | ").replace(['\n', '\t'], " ")
}

fn build_ui(ctx: &Rc<Ctx>, video: &gtk::Widget) -> Ui {
    // The top bar: back, the show and the episode, and at the end episodes, settings and
    // fullscreen, with the window's own close button.
    let back = icon_button("go-previous-symbolic", &t::action_back(), "player.back");
    let show = widgets::show_title("", "row");
    show.set_xalign(0.5);
    show.set_wrap(false);
    show.set_ellipsize(gtk::pango::EllipsizeMode::End);
    let episode = gtk::Label::new(None);
    episode.add_css_class("caption");
    episode.set_ellipsize(gtk::pango::EllipsizeMode::End);
    let titles = gtk::Box::new(gtk::Orientation::Vertical, 0);
    titles.set_valign(gtk::Align::Center);
    titles.append(&show);
    // The episode line, and after it "Private" while what plays is held.
    let private = crate::private::player_label();
    let episode_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    episode_row.set_halign(gtk::Align::Center);
    episode_row.append(&episode);
    episode_row.append(&private);
    titles.append(&episode_row);
    let episodes = gtk::Button::builder()
        .icon_name("view-list-symbolic")
        .tooltip_text(t::player_panel())
        .name("player-panel-button")
        .valign(gtk::Align::Center)
        .build();
    episodes.add_css_class("flat");
    episodes.update_property(&[gtk::accessible::Property::Label(&t::player_panel())]);
    episodes.update_state(&[gtk::accessible::State::Expanded(Some(false))]);
    let tracks = gtk::MenuButton::builder()
        .icon_name("media-view-subtitles-symbolic")
        .tooltip_text(t::player_subtitles_and_audio())
        .valign(gtk::Align::Center)
        .build();
    tracks.add_css_class("flat");
    let people_dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    people_dot.add_css_class("party-dot");
    people_dot.set_halign(gtk::Align::End);
    people_dot.set_valign(gtk::Align::Start);
    people_dot.set_opacity(0.0);
    people_dot.set_can_target(false);
    let people_icon = gtk::Overlay::new();
    people_icon.set_child(Some(&gtk::Image::from_icon_name("kaeri-party-symbolic")));
    people_icon.add_overlay(&people_dot);
    let people = gtk::Button::builder()
        .child(&people_icon)
        .tooltip_text(t::party_title())
        .action_name("player.party")
        .valign(gtk::Align::Center)
        .build();
    people.add_css_class("flat");
    people.add_css_class("image-button");
    let tv = icon_button("video-display-symbolic", &t::tv_play_on(), "player.tv");
    let full =
        icon_button("view-fullscreen-symbolic", &t::player_fullscreen(), "player.fullscreen");
    let mini_button =
        icon_button("kaeri-mini-player-symbolic", &t::player_mini_player(), "player.mini");
    let top = adw::HeaderBar::new();
    top.add_css_class("osd");
    top.add_css_class("player-top");
    top.pack_start(&back);
    top.set_title_widget(Some(&titles));
    top.pack_end(&full);
    top.pack_end(&mini_button);
    top.pack_end(&episodes);
    top.pack_end(&tracks);
    let style = style_panel::button(ctx);
    top.pack_end(&style);
    top.pack_end(&people);
    top.pack_end(&tv);

    // The bottom bar.
    let play =
        icon_button("media-playback-pause-symbolic", &t::action_pause(), "player.toggle");
    play.add_css_class("play");
    let back10 =
        icon_button("media-seek-backward-symbolic", &t::player_back_10(), "player.back-10");
    let forward10 = icon_button(
        "media-seek-forward-symbolic",
        &t::player_forward_10(),
        "player.forward-10",
    );
    let position = gtk::Label::new(Some("0:00"));
    let remaining = gtk::Label::new(Some("\u{2212}0:00"));
    for label in [&position, &remaining] {
        label.add_css_class("numeric");
        label.add_css_class("caption");
    }
    let timeline = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 1.0);
    timeline.set_hexpand(true);
    timeline.set_draw_value(false);
    timeline.add_css_class("timeline");
    timeline.update_property(&[gtk::accessible::Property::Label(&t::player_position())]);
    timeline.set_restrict_to_fill_level(false);
    let marked = Marked::new(&timeline);

    let volume = gtk::Scale::with_range(gtk::Orientation::Vertical, 0.0, 100.0, 5.0);
    volume.set_inverted(true);
    volume.set_value(100.0);
    volume.set_size_request(-1, 140);
    volume.set_draw_value(false);
    volume.update_property(&[gtk::accessible::Property::Label(&t::player_volume())]);
    volume.update_property(&[gtk::accessible::Property::ValueText(&t::player_volume_percent(
        100,
    ))]);
    volume.connect_value_changed(|scale| {
        scale.update_property(&[gtk::accessible::Property::ValueText(
            &t::player_volume_percent(scale.value().round() as i64),
        )]);
    });
    let volume_popover = gtk::Popover::builder().child(&volume).build();
    let volume_button = gtk::MenuButton::builder()
        .icon_name("audio-volume-high-symbolic")
        .tooltip_text(t::player_volume())
        .popover(&volume_popover)
        .valign(gtk::Align::Center)
        .build();
    volume_button.add_css_class("flat");
    let speeds = gio::Menu::new();
    for speed in SPEEDS {
        let item = gio::MenuItem::new(Some(&speed_label(speed)), None);
        item.set_action_and_target_value(Some("player.speed"), Some(&speed.to_variant()));
        speeds.append_item(&item);
    }
    // The speed as the button's own label, "1×", as mpv and the Mac show it.
    let speed_button = gtk::MenuButton::builder()
        .label(speed_label(1.0))
        .tooltip_text(t::player_speed_is(&speed_label(1.0)))
        .menu_model(&speeds)
        .valign(gtk::Align::Center)
        .always_show_arrow(false)
        .build();
    speed_button.add_css_class("flat");
    speed_button.add_css_class("numeric");
    speed_button.add_css_class("speed");
    let settings = gtk::MenuButton::builder()
        .icon_name("emblem-system-symbolic")
        .tooltip_text(t::player_playback_settings())
        .valign(gtk::Align::Center)
        .build();
    settings.add_css_class("flat");

    let bottom = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    bottom.add_css_class("player-bar");
    bottom.add_css_class("osd");
    for widget in [
        play.upcast_ref::<gtk::Widget>(),
        back10.upcast_ref(),
        forward10.upcast_ref(),
        position.upcast_ref(),
    ] {
        bottom.append(widget);
    }
    bottom.append(&marked.widget);
    bottom.append(&remaining);
    bottom.append(&volume_button);
    bottom.append(&speed_button);
    bottom.append(&settings);

    // The starting screen: the art under its ground, Kaeri's line and the tools by time.
    let start = start_view::StartView::new();
    let starting = start.root.clone();

    // The corner cards.
    let skip = Card::new();
    let skip_button = gtk::Button::with_label(&t::player_skip_opening());
    skip_button.add_css_class("pill");
    skip_button.set_action_name(Some("player.skip"));
    skip.widget.append(&skip_button);

    // The marking bar over the top of the video, and the prompts in the corner.
    let mark_bar = gtk::Box::new(gtk::Orientation::Vertical, 10);
    mark_bar.add_css_class("corner-card");
    mark_bar.add_css_class("mark-bar");
    mark_bar.set_halign(gtk::Align::Center);
    mark_bar.set_valign(gtk::Align::Start);
    mark_bar.set_visible(false);
    let skip_prompt = Card::new();
    skip_prompt.widget.set_orientation(gtk::Orientation::Horizontal);
    skip_prompt.widget.set_spacing(8);
    skip_prompt.widget.add_css_class("prompt-card");

    // "Skipped opening · Undo", a small card in the same corner as Skip.
    let undo = Card::new();
    undo.widget.set_orientation(gtk::Orientation::Horizontal);
    undo.widget.set_spacing(6);
    undo.widget.add_css_class("undo-card");
    let undo_line = gtk::Label::new(Some(&t::player_skipped_opening()));
    let undo_dot = widgets::haze("·", "");
    let undo_button = gtk::Button::with_label(&t::action_undo());
    undo_button.add_css_class("flat");
    undo_button.add_css_class("undo-button");
    undo_button.set_action_name(Some("player.undo-skip"));
    undo_button.set_tooltip_text(Some(&t::player_undo_tooltip()));
    undo.widget.append(&undo_line);
    undo.widget.append(&undo_dot);
    undo.widget.append(&undo_button);

    // Up next as Windows draws it: the episode's still, "Up next", the episode and "Plays in
    // 8 s", then the countdown, and Cancel with an amber Play now.
    let up_next = Card::new();
    up_next.widget.add_css_class("up-next-card");
    let up_next_caption = widgets::haze(&t::player_up_next(), "caption");
    up_next_caption.set_xalign(0.0);
    let up_next_title = gtk::Label::new(None);
    up_next_title.set_xalign(0.0);
    up_next_title.set_wrap(true);
    up_next_title.set_lines(3);
    up_next_title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    up_next_title.set_max_width_chars(24);
    up_next_title.add_css_class("heading");
    let up_next_line = widgets::haze("", "caption");
    up_next_line.set_xalign(0.0);
    up_next_line.set_margin_top(4);
    up_next_line.set_visible(false);
    up_next_line.add_css_class("numeric");
    let up_next_still =
        gtk::Picture::builder().accessible_role(gtk::AccessibleRole::Presentation).build();
    up_next_still.set_content_fit(gtk::ContentFit::Cover);
    up_next_still.set_can_shrink(true);
    let still_plate = gtk::Box::new(gtk::Orientation::Vertical, 0);
    still_plate.add_css_class("up-next-still");
    still_plate.set_overflow(gtk::Overflow::Hidden);
    still_plate.append(&up_next_still);
    let up_next_still_frame = crate::fit::Fit::new(&still_plate, 16.0 / 9.0, 160, true, 0);
    up_next_still_frame.set_valign(gtk::Align::Start);
    up_next_still_frame.set_visible(false);
    let up_next_text = gtk::Box::new(gtk::Orientation::Vertical, 2);
    up_next_text.set_valign(gtk::Align::Center);
    up_next_text.set_hexpand(true);
    up_next_text.append(&up_next_caption);
    up_next_text.append(&up_next_title);
    up_next_text.append(&up_next_line);
    let up_next_top = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    up_next_top.append(&up_next_still_frame);
    up_next_top.append(&up_next_text);
    // The card is announced once; the bar only draws the countdown.
    let up_next_bar =
        gtk::ProgressBar::builder().accessible_role(gtk::AccessibleRole::Presentation).build();
    up_next_bar.add_css_class("countdown");
    let up_buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    up_buttons.set_halign(gtk::Align::End);
    let play_now = gtk::Button::with_label(&t::action_play_now());
    play_now.add_css_class("suggested-action");
    play_now.set_action_name(Some("player.play-next"));
    let cancel = gtk::Button::with_label(&t::action_cancel());
    cancel.set_action_name(Some("player.cancel-next"));
    up_buttons.append(&cancel);
    up_buttons.append(&play_now);
    up_next.widget.set_size_request(380, -1);
    up_next.widget.append(&up_next_top);
    up_next.widget.append(&up_next_bar);
    up_next.widget.append(&up_buttons);

    // "Source is buffering · 9 s", with Keep Waiting and Switch Source, the suggested one last.
    let stall = Card::new();
    stall.widget.set_size_request(320, -1);
    let stall_line = gtk::Label::new(Some(&t::player_stall_title(0)));
    stall_line.add_css_class("heading");
    stall_line.add_css_class("numeric");
    stall_line.set_xalign(0.0);
    let stall_buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    stall_buttons.set_halign(gtk::Align::End);
    let switch = gtk::Button::with_label(&t::player_switch_source());
    switch.add_css_class("pill");
    switch.add_css_class("suggested-action");
    switch.set_action_name(Some("player.stall-switch"));
    let wait = gtk::Button::with_label(&t::player_keep_waiting());
    wait.add_css_class("pill");
    wait.set_action_name(Some("player.stall-wait"));
    stall_buttons.append(&wait);
    stall_buttons.append(&switch);
    stall.widget.append(&stall_line);
    stall.widget.append(&stall_buttons);

    // "Resumed at 12:04" and Start Over, top leading, under the top bar.
    let resumed = card();
    resumed.set_orientation(gtk::Orientation::Horizontal);
    resumed.set_spacing(12);
    resumed.set_halign(gtk::Align::Start);
    resumed.set_valign(gtk::Align::Start);
    resumed.set_margin_top(72);
    let resumed_line = gtk::Label::new(None);
    resumed_line.add_css_class("numeric");
    resumed_line.set_valign(gtk::Align::Center);
    let start_over = gtk::Button::with_label(&t::player_start_over());
    start_over.add_css_class("pill");
    start_over.set_action_name(Some("player.start-over"));
    resumed.append(&resumed_line);
    resumed.append(&start_over);

    let cards = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let chat = chat::Chat::new(ctx, &cards);
    for button in chat.buttons.iter().rev() {
        top.pack_end(button);
    }

    cards.set_halign(gtk::Align::End);
    cards.set_valign(gtk::Align::End);
    cards.set_margin_end(24);
    cards.set_margin_bottom(96);
    cards.append(&stall.widget);
    cards.append(&undo.widget);
    cards.append(&skip_prompt.widget);
    cards.append(&up_next.widget);
    cards.append(&skip.widget);

    let dim = gtk::Box::new(gtk::Orientation::Vertical, 0);
    dim.add_css_class("player-dim");
    dim.set_can_target(false);

    // The mini player's controls: play and pause, back and forward 10 s in the middle, and the
    // way back to the full window in the corner.
    let mini = gtk::Box::new(gtk::Orientation::Vertical, 0);
    mini.set_visible(false);
    let full_window =
        icon_button("kaeri-mini-player-symbolic", &t::player_mini_back(), "player.mini");
    full_window.set_halign(gtk::Align::End);
    full_window.set_margin_top(8);
    full_window.set_margin_end(8);
    full_window.add_css_class("osd");
    mini.append(&full_window);
    let middle_row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    middle_row.set_halign(gtk::Align::Center);
    middle_row.set_valign(gtk::Align::Center);
    middle_row.set_vexpand(true);
    middle_row.add_css_class("mini-controls");
    let mini_play =
        icon_button("media-playback-pause-symbolic", &t::action_pause(), "player.toggle");
    for button in [
        icon_button("media-seek-backward-symbolic", &t::player_back_10(), "player.back-10"),
        mini_play.clone(),
        icon_button(
            "media-seek-forward-symbolic",
            &t::player_forward_10(),
            "player.forward-10",
        ),
    ] {
        button.add_css_class("osd");
        middle_row.append(&button);
    }
    mini.append(&middle_row);

    // The picture held over the video while a switch to another source loads.
    let held = gtk::Picture::new();
    held.set_content_fit(gtk::ContentFit::Fill);
    held.set_can_target(false);
    held.set_visible(false);
    // The side panel, and beside it what closes it on a click on the dimmed video.
    let side = SidePanel::new(&ctx.hub);
    let catcher = gtk::Box::new(gtk::Orientation::Vertical, 0);
    catcher.add_css_class("panel-catcher");
    catcher.set_visible(false);

    let stage = gtk::Overlay::new();
    stage.add_css_class("player");
    stage.set_child(Some(video));
    stage.add_overlay(&held);
    stage.add_overlay(&dim);
    stage.add_overlay(&starting);
    stage.add_overlay(&cards);
    stage.add_overlay(&chat.column);
    stage.add_overlay(&chat.moments);
    stage.add_overlay(&chat.field);
    stage.add_overlay(&mark_bar);
    stage.add_overlay(&resumed);
    stage.add_overlay(&catcher);
    stage.add_overlay(&side.widget);
    stage.add_overlay(&mini);

    // The bars over the video, which fills the page under them.
    let page = adw::ToolbarView::new();
    page.add_css_class("player-page");
    page.add_top_bar(&top);
    page.add_bottom_bar(&bottom);
    page.set_content(Some(&stage));
    page.set_extend_content_to_top_edge(true);
    page.set_extend_content_to_bottom_edge(true);
    page.set_focusable(true);

    let menu_buttons = vec![
        tracks.clone(),
        settings.clone(),
        style.clone(),
        speed_button.clone(),
        volume_button,
    ];
    Ui {
        // The mini player's button too, so a narrow window's title keeps room beside the people
        // button.
        roomy: vec![
            back10.upcast(),
            forward10.upcast(),
            speed_button.clone().upcast(),
            mini_button.clone().upcast(),
            tv.clone().upcast(),
        ],
        page,
        top,
        bottom,
        dim,
        stage,
        show,
        episode,
        private,
        play,
        timeline,
        marked,
        position,
        remaining,
        volume,
        starting,
        start,
        start_tools: vec![
            titles.clone().upcast(),
            full.clone().upcast(),
            mini_button.clone().upcast(),
            episodes.clone().upcast(),
            tracks.clone().upcast(),
            style.clone().upcast(),
            people.clone().upcast(),
            tv.clone().upcast(),
        ],
        style,
        cards,
        chat,
        skip,
        skip_button,
        mark_bar,
        skip_prompt,
        undo,
        undo_line,
        up_next,
        up_next_title,
        up_next_line,
        up_buttons: up_buttons.clone(),
        up_next_bar,
        up_next_still,
        up_next_still_frame,
        stall,
        stall_line,
        resumed,
        resumed_line,
        episodes,
        side,
        held,
        catcher,
        tracks,
        people,
        people_dot,
        tv,
        settings,
        speed: speed_button,
        menu_buttons,
        mini,
        mini_play,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_dub_is_marked_on_the_episode_line() {
        assert_eq!(
            episode_line("10", Some(Translation::Dub), Some("The Lantern Road")),
            "Episode 10 · Dubbed · The Lantern Road"
        );
        assert_eq!(episode_line("10", Some(Translation::Sub), None), "Episode 10");
        assert_eq!(episode_line("3", None, Some("")), "Episode 3");
    }

    #[test]
    fn a_mini_toggle_waits_for_the_last_one_to_land() {
        assert!(mini_ready(None));
        assert!(!mini_ready(Some(Duration::from_millis(5))));
        assert!(mini_ready(Some(Duration::from_secs(1))));
    }

    #[test]
    fn a_quick_mini_toggle_keeps_the_frame_from_before() {
        let big = Restore { width: 1280, height: 800, maximized: false, fullscreen: false };
        let mid = Restore { width: 480, height: 360, maximized: false, fullscreen: false };
        assert_eq!(frame_to_keep(mid, Some(big)).width, 1280);
        // A first entry, or a window the viewer resized, is its own frame.
        assert_eq!(frame_to_keep(mid, None).width, 480);
        let other = Restore { width: 900, height: 600, ..big };
        assert_eq!(frame_to_keep(other, Some(big)).width, 900);
    }

    #[test]
    fn a_step_starts_from_the_speed_asked_for_not_the_one_mpv_last_reported() {
        let mut asked = None;
        for want in [1.25, 1.5, 2.0] {
            let next = next_speed(step_from(asked, 1.0), true).unwrap();
            assert_eq!(next, want);
            asked = Some((next, Duration::from_millis(80)));
        }
        let down = next_speed(step_from(asked, 1.0), false).unwrap();
        assert_eq!(down, 1.5);
        // Once mpv has had time to answer, its speed stands again.
        assert_eq!(step_from(Some((2.0, Duration::from_secs(3))), 1.0), 1.0);
    }

    #[test]
    fn quick_steps_walk_the_speeds_one_after_the_other() {
        // Four presses of `]` before mpv reports anything: each steps from the last ask.
        let mut at = 1.0;
        for want in [1.25, 1.5, 2.0] {
            at = next_speed(at, true).unwrap();
            assert_eq!(at, want);
        }
        assert_eq!(next_speed(at, true), None);
        // `]` then `[` comes back to where it began.
        assert_eq!(next_speed(next_speed(1.0, true).unwrap(), false), Some(1.0));
        assert_eq!(next_speed(0.5, false), None);
    }

    #[test]
    fn speeds_read_without_trailing_zeros() {
        assert_eq!(speed_label(1.0), "1×");
        assert_eq!(speed_label(1.25), "1.25×");
        assert_eq!(speed_label(0.5), "0.5×");
    }
}
