//! The player panel's Sources tab while it loads, on the sources loading scene
//! (`--scene sourcesloading`): episode 7 of the finished title plays, the tab opens 5 s in, and
//! each shot is taken at its moment after the tab asked, 0.3 s to 21 s, as the six sources
//! answer in their own time. `--shot sources-loading` takes them in order on one run.

use std::{cell::Cell, rc::Rc, time::Duration, time::Instant};

use adw::prelude::*;
use gtk::glib;
use kaeri_model::command::{Command, PlayerCommand};
use kaeri_testkit::fixtures::FINISHED;

use crate::window::Shell;

/// The tab's states, in the order one run takes them, each with its moment after the tab asks.
pub const NAMES: [&str; 5] = [
    "sources-loading-0s",
    "sources-loading-1s",
    "sources-loading-4s",
    "sources-loading-12s",
    "sources-loading-21s",
];

/// Each shot's moment after the tab asked, in seconds.
const MOMENTS: [f64; 5] = [0.3, 1.0, 4.0, 12.0, 21.0];

/// How long after the episode is asked to play the tab opens: playback up and the bar settled.
const OPEN_AFTER: f64 = 5.0;

thread_local! {
    /// When the tab opens, once the run has asked for the episode.
    static OPENS: Cell<Option<Instant>> = const { Cell::new(None) };
}

/// Put the window in the state `shot` shows, and say how long to let it settle.
pub fn stage(shell: &Rc<Shell>, shot: &str) -> Option<f64> {
    let at = NAMES.iter().position(|n| *n == shot)?;
    let opens = match OPENS.with(Cell::get) {
        Some(opens) => opens,
        None => {
            let opens = Instant::now() + Duration::from_secs_f64(OPEN_AFTER);
            OPENS.with(|o| o.set(Some(opens)));
            play_and_open(shell);
            opens
        }
    };
    let moment = opens + Duration::from_secs_f64(MOMENTS[at]);
    tracing::info!(shot, "sources-loading: staged");
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
        tracing::info!("sources-loading: the tab opens");
        player.shot_panel("sources");
    });
}
