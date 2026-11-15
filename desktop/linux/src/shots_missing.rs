//! The missing episode's shots, on the missing episode scene (`--scene missing-episode`): the
//! title page as the episode has just aired, Checking, still after Check again, and found, with
//! Source for this title's lines. `--shot missing` takes the page's states in order on one run,
//! where each Check again moves the fixture source on a listing; `missing-sources` needs a run of
//! its own, before any check. The found shot also records the window's frames from the press on,
//! into `clip/` beside the shots, with a concat list for ffmpeg.

use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
    time::Duration,
};

use adw::prelude::*;
use gtk::{gdk, glib};
use kaeri_testkit::fixtures::AIRING;

use crate::{pages::Dest, window::Shell};

/// The page's states, in the order one run takes them.
pub const NAMES: [&str; 5] = [
    "missing-aired",
    "missing-checking",
    "missing-still",
    "missing-found",
    "missing-sources-found",
];

thread_local! {
    /// Where the run saves its shots: the clip goes beside them.
    static DIR: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

/// Where the run saves its shots.
pub fn saving_to(dir: PathBuf) {
    DIR.with(|d| d.replace(Some(dir)));
}

/// Put the window in the state `shot` shows, and say how long to let it settle.
pub fn stage(shell: &Rc<Shell>, shot: &str) -> Option<f64> {
    Some(match shot {
        "missing-aired" => {
            open_title(shell);
            4.0
        }
        "missing-sources" => {
            open_title(shell);
            let ctx = Rc::clone(&shell.ctx);
            glib::timeout_add_local_once(Duration::from_millis(2500), move || {
                if let Some(page) = ctx.nav().and_then(|nav| nav.visible_page()) {
                    let _ = WidgetExt::activate_action(&page, "title.source-picker", None);
                }
            });
            // The sheet lists the title again, which takes 1.4 s.
            6.0
        }
        // Checking shows 600 ms after the press, fading in for 200.
        "missing-checking" => {
            check_again(shell);
            1.0
        }
        // The listing takes 1.4 s, then the words crossfade.
        "missing-still" => 1.5,
        "missing-found" => {
            let window = shell.ctx.window.clone();
            record(&window, 4.5);
            let shell = Rc::clone(shell);
            glib::timeout_add_local_once(Duration::from_millis(500), move || {
                check_again(&shell);
            });
            // The recording, then its frames saved.
            6.0
        }
        "missing-sources-found" => {
            if let Some(page) = shell.ctx.nav().and_then(|nav| nav.visible_page()) {
                let _ = WidgetExt::activate_action(&page, "title.source-picker", None);
            }
            3.5
        }
        _ => return None,
    })
}

pub fn unstage(shell: &Rc<Shell>, shot: &str) {
    if shot.starts_with("missing-sources") {
        let window = &shell.ctx.window;
        if let Some(dialog) = window.visible_dialog() {
            dialog.force_close();
        }
    }
}

fn open_title(shell: &Rc<Shell>) {
    shell.ctx.notices.dismiss_each();
    // Anything a first launch asks, out of the way.
    if let Some(dialog) = shell.ctx.window.visible_dialog() {
        dialog.force_close();
    }
    shell.go(Dest::Home);
    let _ = WidgetExt::activate_action(
        &shell.ctx.window,
        "win.open-title",
        Some(&AIRING.get().to_variant()),
    );
}

/// Check again, pressed as a click would.
fn check_again(shell: &Rc<Shell>) {
    let root: gtk::Widget = shell.ctx.window.clone().upcast();
    match named(&root, "missing-check").and_then(|w| w.downcast::<gtk::Button>().ok()) {
        Some(button) => {
            button.grab_focus();
            button.emit_clicked();
        }
        None => tracing::error!("no Check again on the page"),
    }
}

fn named(root: &gtk::Widget, name: &str) -> Option<gtk::Widget> {
    if root.widget_name() == name && root.is_mapped() {
        return Some(root.clone());
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        if let Some(found) = named(&widget, name) {
            return Some(found);
        }
        child = widget.next_sibling();
    }
    None
}

/// The window as it is drawn now.
fn frame(window: &adw::ApplicationWindow) -> Option<gdk::Texture> {
    let (width, height) = (window.width(), window.height());
    let paintable = gtk::WidgetPaintable::new(Some(window));
    let snapshot = gtk::Snapshot::new();
    paintable.snapshot(&snapshot, f64::from(width), f64::from(height));
    let node = snapshot.to_node()?;
    let renderer = window.renderer()?;
    let bounds = gtk::graphene::Rect::new(0.0, 0.0, width as f32, height as f32);
    Some(renderer.render_texture(&node, Some(&bounds)))
}

/// Every frame the window paints for `seconds`, and one at least every 33 ms, kept in memory
/// with its time, then saved as `clip/frame-NNNN.png` with `clip/frames.txt`, an ffconcat list
/// with each frame's duration.
fn record(window: &adw::ApplicationWindow, seconds: f64) {
    let Some(dir) = DIR.with(|d| d.borrow().clone()).map(|d| d.join("clip")) else {
        tracing::error!("no folder for the clip");
        return;
    };
    let Some(clock) = window.frame_clock() else { return };
    let frames: Rc<RefCell<Vec<(i64, gdk::Texture)>>> = Rc::default();
    let start = glib::monotonic_time();
    let busy = Rc::new(Cell::new(false));
    let handler = clock.connect_after_paint({
        let (window, frames, busy) = (window.clone(), Rc::clone(&frames), Rc::clone(&busy));
        move |_| {
            if busy.replace(true) {
                return;
            }
            let (window, frames, busy) = (window.clone(), Rc::clone(&frames), Rc::clone(&busy));
            glib::idle_add_local_full(glib::Priority::HIGH, move || {
                if let Some(texture) = frame(&window) {
                    frames.borrow_mut().push((glib::monotonic_time(), texture));
                }
                busy.set(false);
                glib::ControlFlow::Break
            });
        }
    });
    let pace = glib::timeout_add_local(Duration::from_millis(33), {
        let window = window.clone();
        move || {
            window.queue_draw();
            glib::ControlFlow::Continue
        }
    });
    glib::timeout_add_local_once(Duration::from_secs_f64(seconds), move || {
        clock.disconnect(handler);
        pace.remove();
        save(&dir, start, &frames.borrow());
    });
}

fn save(dir: &std::path::Path, start: i64, frames: &[(i64, gdk::Texture)]) {
    if let Err(e) = std::fs::create_dir_all(dir) {
        tracing::error!(error = %e, "the clip's folder");
        return;
    }
    let mut list = String::from("ffconcat version 1.0\n"); // i18n-skip
    for (n, (at, texture)) in frames.iter().enumerate() {
        let name = format!("frame-{n:04}.png");
        if let Err(e) = texture.save_to_png(dir.join(&name)) {
            tracing::error!(error = %e, "a frame did not save");
            continue;
        }
        let next = frames.get(n + 1).map_or(*at + 33_000, |(next, _)| *next);
        list.push_str(&format!(
            "file '{name}'\nduration {:.4}\n# at {:.3}\n", // i18n-skip
            (next - at) as f64 / 1e6,
            (at - start) as f64 / 1e6
        ));
    }
    if let Some(last) = frames.len().checked_sub(1) {
        // The concat demuxer takes the last frame's duration only when it is named again.
        list.push_str(&format!("file 'frame-{last:04}.png'\n")); // i18n-skip
    }
    match std::fs::write(dir.join("frames.txt"), list) {
        Ok(()) => tracing::info!(frames = frames.len(), "the clip's frames"),
        Err(e) => tracing::error!(error = %e, "the clip's list"),
    }
}
