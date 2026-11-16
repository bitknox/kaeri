//! The cast states shots, one per frame the board draws for Linux: reconnecting and its two
//! endings, the TV's own player, a stop on the TV, and Disconnect or Stop casting. The clips
//! save each new motion as frames, with animations on and off.

use std::{rc::Rc, time::Duration};

use adw::prelude::*;
use gtk::glib;

use kaeri_cast::fake::{CustomReceiver, FakeOptions};

use crate::{
    pages::Dest,
    tv::{
        Tv,
        fixture::{self, LIVING_ROOM, Scene},
    },
    window::Shell,
};

/// Every still, in the board's order.
pub const STILLS: [&str; 15] = [
    "cast-states-playing",
    "cast-states-reconnecting",
    "cast-states-gave-up",
    "cast-states-playing-remote",
    "cast-states-reconnecting-remote",
    "cast-states-gave-up-remote",
    "cast-states-fallback",
    "cast-states-fallback-remote",
    "cast-states-waiting",
    "cast-states-stopped-tv",
    "cast-states-waiting-remote",
    "cast-states-stopped-tv-remote",
    "cast-states-dialog-playing",
    "cast-states-dialog-idle",
    "cast-states-menu",
];

/// Every motion, as frames under `<shots>/<name>/`; each also with animations off.
pub const CLIPS: [&str; 16] = [
    "cast-states-clip-reconnect",
    "cast-states-clip-reconnect-reduced",
    "cast-states-clip-reconnect-remote",
    "cast-states-clip-reconnect-remote-reduced",
    "cast-states-clip-rejoin",
    "cast-states-clip-rejoin-reduced",
    "cast-states-clip-rejoin-remote",
    "cast-states-clip-rejoin-remote-reduced",
    "cast-states-clip-gave-up",
    "cast-states-clip-gave-up-reduced",
    "cast-states-clip-gave-up-remote",
    "cast-states-clip-gave-up-remote-reduced",
    "cast-states-clip-fallback",
    "cast-states-clip-fallback-reduced",
    "cast-states-clip-stopped-tv",
    "cast-states-clip-stopped-tv-reduced",
];

/// The shots taken with a popover open over the window.
pub const POPOVERS: [&str; 3] =
    ["cast-states-dialog-playing", "cast-states-dialog-idle", "cast-states-menu"];

/// The fixture title the casts play.
const TITLE: u32 = 900_001;

/// When the cast plays: the pick at 1.8 s, the receiver up and the episode loaded by 9 s.
const PLAYING: u64 = 9_500;

/// How many frames a clip keeps: about a second and a half at 60 a second.
const CLIP_FRAMES: u32 = 90;

/// Put the window in the state `shot` shows, and say how long to let it settle.
pub fn stage(shell: &Rc<Shell>, shot: &str) -> Option<f64> {
    if !shot.starts_with("cast-states-") {
        return None;
    }
    let scene = fixture::scene()?;
    let reduced = shot.ends_with("-reduced");
    if reduced && let Some(settings) = gtk::Settings::default() {
        settings.set_gtk_enable_animations(false);
    }
    let clip: &'static str = CLIPS.iter().find(|c| **c == shot).copied().unwrap_or_default();
    let base = shot.trim_end_matches("-reduced");
    let remote = base.ends_with("-remote");
    let tv = Rc::clone(&shell.ctx.tv);
    shell.ctx.notices.dismiss_each();
    scene.apart();
    scene.cast_clear();
    // The board lists only Bedroom on the network; the Living room there is the Cast one.
    scene.switch_off(LIVING_ROOM);
    shell.go(Dest::Home);
    let settle = match base.trim_end_matches("-remote") {
        "cast-states-playing" => {
            cast(shell, &scene, plays());
            hide_remote(shell, remote, PLAYING);
            11.0
        }
        // The connection closes at 10 s and the TV takes a minute to answer again: the engine
        // tries for 20 s, then gives up.
        "cast-states-reconnecting" | "cast-states-gave-up" => {
            cast(shell, &scene, plays());
            hide_remote(shell, remote, PLAYING);
            drop_at(&scene, 10_000, Duration::from_secs(60));
            if base.contains("gave-up") { 33.5 } else { 13.0 }
        }
        "cast-states-clip-reconnect" => {
            cast(shell, &scene, plays());
            hide_remote(shell, remote, PLAYING);
            // The first frames of a run can come a second late, so the drop waits 1.2 s.
            later(10_000, {
                let window = shell.ctx.window.clone();
                move || crate::shots::frames(&window, clip, CLIP_FRAMES + 60)
            });
            drop_at(&scene, 11_200, Duration::from_secs(60));
            14.0
        }
        // The TV answers 4 s after the connection closed, and the cast goes on: the clip starts
        // well before.
        "cast-states-clip-rejoin" => {
            cast(shell, &scene, plays());
            hide_remote(shell, remote, PLAYING);
            drop_at(&scene, 10_000, Duration::from_secs(4));
            later(11_500, {
                let window = shell.ctx.window.clone();
                move || crate::shots::frames(&window, clip, CLIP_FRAMES + 180)
            });
            18.0
        }
        "cast-states-clip-gave-up" => {
            cast(shell, &scene, plays());
            hide_remote(shell, remote, PLAYING);
            drop_at(&scene, 10_000, Duration::from_secs(60));
            film_before_end(shell, clip, 10_500);
            34.0
        }
        // Kaeri's receiver isn't published to the TV, so it plays in its own player. The
        // receiver takes 3 s for each launch, so the notice comes about 8 s in, as the episode
        // starts, and stays 4 s.
        "cast-states-fallback" | "cast-states-clip-fallback" => {
            cast(
                shell,
                &scene,
                FakeOptions {
                    custom: CustomReceiver::Unknown,
                    launch: Duration::from_secs(3),
                    ..plays()
                },
            );
            hide_remote(shell, remote, 4_000);
            if !clip.is_empty() {
                later(7_000, {
                    let window = shell.ctx.window.clone();
                    move || crate::shots::frames(&window, clip, CLIP_FRAMES + 60)
                });
            }
            if clip.is_empty() { 9.6 } else { 12.0 }
        }
        // Stopped on the TV at 10 s: the engine hears the TV for up to 4 s before it ends.
        "cast-states-waiting" | "cast-states-stopped-tv" | "cast-states-clip-stopped-tv" => {
            cast(shell, &scene, plays());
            hide_remote(shell, remote, PLAYING);
            let stopping = Rc::clone(&scene);
            later(10_000, move || stopping.cast_stop_app(LIVING_ROOM));
            if !clip.is_empty() {
                film_before_end(shell, clip, 10_000);
            }
            match base.trim_end_matches("-remote") {
                "cast-states-waiting" => 11.5,
                "cast-states-stopped-tv" => 16.0,
                _ => 17.0,
            }
        }
        // The TV held from the header's chooser first, then Play on TV plays there. A cast from
        // Play on TV alone holds no session, and the chooser's browse would join it again.
        "cast-states-dialog-playing" => {
            scene.cast(LIVING_ROOM, plays());
            tv.start_browsing();
            connect_when_listed(Rc::clone(&tv), 12);
            later(4_000, {
                let tv = Rc::clone(&tv);
                move || tv.stop_browsing()
            });
            cast_after(shell, 4_000);
            hide_remote(shell, false, 14_000);
            later(15_000, {
                let window = shell.ctx.window.clone();
                move || pop_up(&window, |b| b.has_css_class("cast-button"))
            });
            16.0
        }
        // The TV chosen from the header's chooser with nothing asked to play.
        "cast-states-dialog-idle" => {
            scene.cast(LIVING_ROOM, plays());
            tv.start_browsing();
            connect_when_listed(Rc::clone(&tv), 12);
            later(6_000, {
                let window = shell.ctx.window.clone();
                move || pop_up(&window, |b| b.has_css_class("cast-button"))
            });
            7.0
        }
        "cast-states-menu" => {
            cast(shell, &scene, plays());
            later(10_500, {
                let window = shell.ctx.window.clone();
                move || pop_up(&window, |b| in_remote(b.upcast_ref()))
            });
            11.5
        }
        _ => 0.0,
    };
    Some(settle)
}

/// Put back what a shot changed. The cast itself ends in the cast shots' own unstage.
pub fn unstage(shell: &Rc<Shell>, shot: &str) {
    if !shot.starts_with("cast-states-") {
        return;
    }
    pop_down(shell.ctx.window.upcast_ref());
    shell.ctx.notices.dismiss_each();
    let tv = &shell.ctx.tv;
    tv.stop_browsing();
    // A TV held from the header's chooser is let go as well.
    if tv.cast_session().is_some_and(|s| s.device.is_some()) {
        if tv.casting() {
            tv.cast_stop();
        }
        tv.cast_disconnect();
    }
    if let Some(settings) = gtk::Settings::default() {
        settings.set_gtk_enable_animations(true);
    }
}

/// Close every menu button's popover under `widget`.
fn pop_down(widget: &gtk::Widget) {
    if let Some(button) = widget.downcast_ref::<gtk::MenuButton>() {
        button.popdown();
    }
    let mut child = widget.first_child();
    while let Some(next) = child {
        pop_down(&next);
        child = next.next_sibling();
    }
}

/// What the episodes stream is a test pattern the fake plays as though it had read it; the
/// TV keeps its app across connections, as a real one does.
fn plays() -> FakeOptions {
    FakeOptions {
        plays_unfetched: true,
        duration: 1440.0,
        control: "fixed",
        remembers: true,
        ..FakeOptions::default()
    }
}

fn later(ms: u64, f: impl FnOnce() + 'static) {
    glib::timeout_add_local_once(Duration::from_millis(ms), f);
}

/// Play on TV for episode 3, from 8:12, on the Living room TV set up as `options` say.
fn cast(shell: &Rc<Shell>, scene: &Rc<Scene>, options: FakeOptions) {
    scene.cast(LIVING_ROOM, options);
    cast_after(shell, 0);
}

/// Play on TV for episode 3 `after` ms from now, and the Living room chosen.
fn cast_after(shell: &Rc<Shell>, after: u64) {
    let window = shell.ctx.window.clone();
    later(after + 600, move || {
        let _ = WidgetExt::activate_action(
            &window,
            "win.play-on-tv",
            Some(&(TITLE, "3".to_owned(), 492.0).to_variant()),
        );
    });
    let tv = Rc::clone(&shell.ctx.tv);
    later(after + 1_800, move || choose(tv, 10));
}

/// Choose the cast row once it is listed, asking again until it is.
fn choose(tv: Rc<Tv>, tries: u32) {
    if let Some(row) = tv.cast_rows().into_iter().find(|r| r.name == LIVING_ROOM) {
        return tv.choose_cast(&row);
    }
    if tries > 0 {
        later(500, move || choose(tv, tries - 1));
    }
}

/// Connect to the TV from the header's chooser once it is listed.
fn connect_when_listed(tv: Rc<Tv>, tries: u32) {
    if let Some(row) = tv.cast_rows().into_iter().find(|r| r.name == LIVING_ROOM) {
        return tv.cast_connect(&row);
    }
    if tries > 0 {
        later(500, move || connect_when_listed(tv, tries - 1));
    }
}

/// Close the remote at `ms` for the shots with only the bar, with the first launch's
/// question and the picker gone too.
fn hide_remote(shell: &Rc<Shell>, remote: bool, ms: u64) {
    if remote {
        return;
    }
    let window = shell.ctx.window.clone();
    let tv = Rc::clone(&shell.ctx.tv);
    later(ms, move || {
        crate::shots::close_dialogs(&window);
        tv.sheet_closed();
        crate::shots::close_dialogs(&window);
    });
}

/// At `ms`, the TV answers new connections only after `answer` and closes the one held.
fn drop_at(scene: &Rc<Scene>, ms: u64, answer: Duration) {
    let scene = Rc::clone(scene);
    later(ms, move || {
        scene.cast_answer_after(LIVING_ROOM, answer);
        scene.cast_close_connection(LIVING_ROOM);
    });
}

/// From `from` ms, look every 20 ms for `ready`, then save the clip's frames.
fn film_when(
    shell: &Rc<Shell>,
    clip: &'static str,
    from: u64,
    ready: impl Fn(&Tv) -> bool + 'static,
) {
    let window = shell.ctx.window.clone();
    let tv = Rc::clone(&shell.ctx.tv);
    later(from, move || {
        let mut left = 2_000u32;
        glib::timeout_add_local(Duration::from_millis(20), move || {
            left = left.saturating_sub(1);
            if ready(&tv) {
                crate::shots::frames(&window, clip, CLIP_FRAMES);
                return glib::ControlFlow::Break;
            }
            if left == 0 { glib::ControlFlow::Break } else { glib::ControlFlow::Continue }
        });
    });
}

/// The clip of a cast's end: from the moment it ends from afar.
fn film_before_end(shell: &Rc<Shell>, clip: &'static str, from: u64) {
    film_when(shell, clip, from, |tv| tv.cast_view().is_none_or(|view| view.stage.ended()));
}

/// Open the popover of the first shown menu button `pick` takes.
fn pop_up(window: &adw::ApplicationWindow, pick: impl Fn(&gtk::MenuButton) -> bool) {
    fn find(
        widget: &gtk::Widget,
        pick: &dyn Fn(&gtk::MenuButton) -> bool,
    ) -> Option<gtk::MenuButton> {
        if let Some(button) = widget.downcast_ref::<gtk::MenuButton>()
            && button.is_mapped()
            && pick(button)
        {
            return Some(button.clone());
        }
        let mut child = widget.first_child();
        while let Some(next) = child {
            if let Some(found) = find(&next, pick) {
                return Some(found);
            }
            child = next.next_sibling();
        }
        None
    }
    match find(window.upcast_ref(), &pick) {
        Some(button) => button.popup(),
        None => tracing::warn!("no menu button to open for the shot"),
    }
}

/// Whether `widget` sits in the remote.
fn in_remote(widget: &gtk::Widget) -> bool {
    let mut up = widget.parent();
    while let Some(parent) = up {
        if parent.has_css_class("tv-remote") {
            return true;
        }
        up = parent.parent();
    }
    false
}
