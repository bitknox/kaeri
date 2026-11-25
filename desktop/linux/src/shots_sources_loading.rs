//! The player panel's Sources tab while it loads, on the sources loading scene
//! (`--scene sourcesloading`): episode 7 of the finished title plays, the tab opens 5 s in, and
//! each shot is taken at its moment after the tab asked, 0.3 s to 21 s, as the six sources
//! answer in their own time. `--shot sources-loading` takes them in order on one run, after a
//! first shot of the episode playing: a run's first capture is slow, and the tab's first
//! moments are too short to wait for one. Saving the 0.3 s shot outlasts the wait to 1 s, so
//! `scripts/shots.sh` takes 0.3 s on a run of its own and the rest on a second.
//!
//! Three more, each on a run of its own as each dresses the engine's events: a source with a
//! name longer than the panel is wide (`sources-loading-long`), a listing the engine ends
//! before asking any source (`sources-loading-ended`), and a source resting since before that
//! lists a release when the rows come from the final answer alone
//! (`sources-loading-resting`).

use std::{cell::Cell, rc::Rc, time::Duration, time::Instant};

use adw::prelude::*;
use gtk::glib;
use kaeri_model::{
    Msg,
    command::{Command, PlayerCommand},
    event::Event,
    sources::SourceNote,
};
use kaeri_testkit::fixtures::FINISHED;

use crate::window::Shell;

/// The tab's states, in the order one run takes them, after the episode playing.
pub const NAMES: [&str; 6] = [
    "sources-loading-playing",
    "sources-loading-0s",
    "sources-loading-1s",
    "sources-loading-4s",
    "sources-loading-12s",
    "sources-loading-21s",
];

/// The dressed shots, each on a run of its own after the episode playing, and its moment after
/// the tab asked: the long name once its source answered, the ended listing past when the
/// rows would count, and the resting source once the final answer came.
const DRESSED: [(&str, f64); 3] = [
    ("sources-loading-long", 4.3),
    ("sources-loading-ended", 4.3),
    ("sources-loading-resting", 23.0),
];

/// The made-up long name the long shot gives Media folder.
const LONG_NAME: &str = "Media folder on the shared drive in the hallway cupboard";

/// Each shot's moment after the tab asked, in seconds. The counting ones are 0.3 s past the
/// whole second: the count is redrawn every 250 ms, so at 4.0 s it can still say 3 s.
const MOMENTS: [f64; 5] = [0.3, 1.0, 4.3, 12.3, 21.0];

/// How long after the episode is asked to play the tab opens: playback up and the bar settled.
const OPEN_AFTER: f64 = 5.0;

thread_local! {
    /// When the tab opens, once the run has asked for the episode.
    static OPENS: Cell<Option<Instant>> = const { Cell::new(None) };
}

/// Put the window in the state `shot` shows, and say how long to let it settle.
pub fn stage(shell: &Rc<Shell>, shot: &str) -> Option<f64> {
    let dressed = DRESSED.iter().find(|(name, _)| *name == shot);
    let at = match dressed {
        Some(_) => None,
        None => Some(NAMES.iter().position(|n| *n == shot)?),
    };
    let opens = OPENS.with(Cell::get).unwrap_or_else(|| {
        let opens = Instant::now() + Duration::from_secs_f64(OPEN_AFTER);
        OPENS.with(|o| o.set(Some(opens)));
        play_and_open(shell);
        opens
    });
    if let Some((name, _)) = dressed {
        dress(shell, name);
    }
    let moment = match (dressed, at.and_then(|at| at.checked_sub(1))) {
        (Some((_, after)), _) => opens + Duration::from_secs_f64(*after),
        (None, Some(at)) => opens + Duration::from_secs_f64(MOMENTS[at]),
        // The episode playing, before the tab opens.
        (None, None) => opens - Duration::from_millis(1500),
    };
    Some(moment.saturating_duration_since(Instant::now()).as_secs_f64())
}

/// Episode 7 played, paused once it shows, and the Sources tab opened [`OPEN_AFTER`] later.
fn play_and_open(shell: &Rc<Shell>) {
    shell.ctx.notices.dismiss_each();
    let _ = WidgetExt::activate_action(
        &shell.ctx.window,
        "win.play",
        Some(&(FINISHED.get(), "7".to_owned()).to_variant()),
    );
    let hub = Rc::clone(&shell.ctx.hub);
    glib::timeout_add_local_once(Duration::from_secs(4), move || {
        hub.send(Command::Player(PlayerCommand::TogglePause));
    });
    let player = Rc::clone(&shell.player);
    glib::timeout_add_local_once(Duration::from_secs_f64(OPEN_AFTER), move || {
        player.shot_panel("sources");
    });
}

/// Dress the engine's events for the shot `name`.
fn dress(shell: &Rc<Shell>, name: &str) {
    let dress: crate::hub::Dress = match name {
        "sources-loading-long" => Rc::new(|event: &mut Event| match event {
            Event::SourcesProgress { rows, .. } => {
                for row in rows.iter_mut().filter(|r| r.name == "Media folder") {
                    row.name = LONG_NAME.into();
                }
            }
            Event::SourceOptions { options, .. } => {
                for option in options.iter_mut().filter(|o| o.name == "Media folder") {
                    option.name = LONG_NAME.into();
                }
            }
            _ => {}
        }),
        // As when the title's details could not be read: no row is said, the listing ends, and
        // no final answer follows (the scene's own is sent to another episode).
        "sources-loading-ended" => Rc::new(|event: &mut Event| match event {
            Event::SourcesProgress { id, episode, .. } => {
                *event = Event::SourcesFailed {
                    id: *id,
                    episode: episode.clone(),
                    why: Msg::verbatim("The title's details didn't load: connection reset"), // i18n-skip
                };
            }
            Event::Sources { episode, .. } => *episode = "0".into(),
            _ => {}
        }),
        // Home server rests since an earlier failure, and the rows come from the final answer
        // alone (the progress is sent to another episode).
        "sources-loading-resting" => Rc::new(|event: &mut Event| match event {
            Event::SourcesProgress { episode, .. } => *episode = "0".into(),
            Event::SourceOptions { options, .. } => {
                for option in options.iter_mut().filter(|o| o.name == "Home server") {
                    option.note = Some(SourceNote::Resting);
                }
            }
            _ => {}
        }),
        _ => return,
    };
    shell.ctx.hub.dress.replace(Some(dress));
}
