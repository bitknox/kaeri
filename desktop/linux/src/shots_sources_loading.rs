//! The player panel's Sources tab while it loads, on the sources loading scene
//! (`--scene sourcesloading`): episode 7 of the finished title plays, the tab opens 5 s in, and
//! each shot is taken at its moment after the tab asked, 0.3 s to 21 s, as the six sources
//! answer in their own time. `--shot sources-loading` takes them in order on one run, after a
//! first shot of the episode playing: a run's first capture is slow, and the tab's first
//! moments are too short to wait for one. Saving the 0.3 s shot outlasts the wait to 1 s, so
//! `scripts/shots.sh` takes 0.3 s on a run of its own and the rest on a second.

use std::{cell::Cell, rc::Rc, time::Duration, time::Instant};

use adw::prelude::*;
use gtk::glib;
use kaeri_model::command::{Command, PlayerCommand};
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
    let at = NAMES.iter().position(|n| *n == shot)?;
    let opens = OPENS.with(Cell::get).unwrap_or_else(|| {
        let opens = Instant::now() + Duration::from_secs_f64(OPEN_AFTER);
        OPENS.with(|o| o.set(Some(opens)));
        play_and_open(shell);
        opens
    });
    let moment = match at.checked_sub(1) {
        Some(at) => opens + Duration::from_secs_f64(MOMENTS[at]),
        // The episode playing, before the tab opens.
        None => opens - Duration::from_millis(1500),
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
