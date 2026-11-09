//! The keyboard stays with the reader while it covers the window. The pages behind it stay in
//! the window's stack, so anything there that takes focus (a field a shortcut focuses, a dialog
//! closing and putting focus back where it was opened from) would take the keys with it: Esc
//! and the arrows would go to an unseen control and the reader would not hear them.

use std::rc::Rc;

use adw::prelude::*;
use gtk::{gdk, glib};

/// Where the window's focus is, seen from the reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// In the reader, its popovers included.
    Inside,
    /// On a page behind the reader, or nowhere.
    Behind,
    /// Over the reader: a dialog or a notice on top of the window.
    Over,
}

/// Where `focus` is, for a reader that is a page of `stack`.
pub fn place(focus: Option<&gtk::Widget>, reader: &gtk::Widget) -> Focus {
    let Some(focus) = focus else { return Focus::Behind };
    let stack = reader.parent();
    let mut at = Some(focus.clone());
    while let Some(widget) = at {
        if &widget == reader {
            return Focus::Inside;
        }
        let parent = widget.parent();
        if parent.is_some() && parent == stack {
            return Focus::Behind;
        }
        at = parent;
    }
    Focus::Over
}

/// The widget with the keyboard in `window`.
fn focused(window: &gtk::Window) -> Option<gtk::Widget> {
    gtk::prelude::GtkWindowExt::focus(window)
}

/// Whether `reader` is the page its stack shows.
fn covers(reader: &gtk::Widget) -> bool {
    reader
        .parent()
        .and_downcast::<gtk::Stack>()
        .and_then(|stack| stack.visible_child())
        .is_some_and(|shown| &shown == reader)
}

/// Keep the keys with `reader` while it covers `window`: focus that lands behind it comes back,
/// and a key that finds the focus behind it goes to `take` and no further. Dialogs and notices
/// over the reader keep theirs.
pub fn hold(
    window: &impl IsA<gtk::Window>,
    reader: &impl IsA<gtk::Widget>,
    take: Rc<dyn Fn(gdk::Key, gdk::ModifierType) -> bool>,
) {
    let window = window.as_ref();
    let reader = reader.as_ref();
    window.connect_notify_local(Some("focus-widget"), {
        let reader = reader.downgrade();
        move |window, _| {
            let Some(reader) = reader.upgrade() else { return };
            if covers(&reader) && place(focused(window).as_ref(), &reader) == Focus::Behind {
                // After the change that moved it has finished.
                let (window, reader) = (window.downgrade(), reader.downgrade());
                glib::idle_add_local_once(move || {
                    if let (Some(window), Some(reader)) = (window.upgrade(), reader.upgrade())
                        && covers(&reader)
                        && place(focused(&window).as_ref(), &reader) == Focus::Behind
                    {
                        reader.grab_focus();
                    }
                });
            }
        }
    });
    // Before the focus widget hears the key. The application's shortcuts on the window run
    // first, so Ctrl+W, Ctrl+Q and Alt+Left still work.
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.connect_key_pressed({
        let (reader, window) = (reader.downgrade(), window.downgrade());
        move |_, key, _, modifiers| {
            let (Some(reader), Some(window)) = (reader.upgrade(), window.upgrade()) else {
                return glib::Propagation::Proceed;
            };
            if !covers(&reader) || place(focused(&window).as_ref(), &reader) != Focus::Behind {
                return glib::Propagation::Proceed;
            }
            reader.grab_focus();
            take(key, modifiers);
            glib::Propagation::Stop
        }
    });
    window.add_controller(keys);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gtk_test::{pump, run};

    struct Scene {
        window: gtk::Window,
        stack: gtk::Stack,
        behind: gtk::Button,
        reader: gtk::Overlay,
        inside: gtk::Button,
    }

    /// A window with a page and a reader in one stack, as the app's window has them.
    fn scene() -> Scene {
        let behind = gtk::Button::with_label("Continue");
        let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
        page.append(&behind);
        let inside = gtk::Button::with_label("Start over");
        let reader = gtk::Overlay::new();
        reader.set_child(Some(&gtk::Label::new(Some("page 1"))));
        reader.add_overlay(&inside);
        reader.set_focusable(true);
        let stack = gtk::Stack::new();
        stack.add_named(&page, Some("main"));
        stack.add_named(&reader, Some("reader"));
        let window = gtk::Window::builder().default_width(400).default_height(300).build();
        window.set_child(Some(&stack));
        window.present();
        pump(100);
        Scene { window, stack, behind, reader, inside }
    }

    #[test]
    fn focus_is_placed_by_the_page_it_is_on() {
        run(|| {
            let s = scene();
            let reader = s.reader.upcast_ref::<gtk::Widget>();
            assert_eq!(place(Some(s.behind.upcast_ref()), reader), Focus::Behind);
            assert_eq!(place(Some(reader), reader), Focus::Inside);
            assert_eq!(place(Some(s.inside.upcast_ref()), reader), Focus::Inside);
            assert_eq!(place(None, reader), Focus::Behind);
            assert_eq!(place(Some(s.window.upcast_ref()), reader), Focus::Over);
            s.window.destroy();
        });
    }

    /// A control behind the reader that takes focus while the reader covers the window, as
    /// Search's field does on Ctrl+F or a closing dialog's opener does, gives it back.
    #[test]
    fn focus_taken_behind_the_reader_comes_back_to_it() {
        run(|| {
            let s = scene();
            hold(&s.window, &s.reader, Rc::new(|_, _| true));
            s.behind.grab_focus();
            s.stack.set_visible_child(&s.reader);
            s.reader.grab_focus();
            pump(50);
            s.behind.grab_focus();
            pump(50);
            let focus = focused(&s.window);
            assert_eq!(
                place(focus.as_ref(), s.reader.upcast_ref()),
                Focus::Inside,
                "focus stayed on {focus:?}"
            );
            // A control of the reader's own keeps it.
            s.inside.grab_focus();
            pump(50);
            assert_eq!(focused(&s.window).as_ref(), Some(s.inside.upcast_ref()));
            // With the reader closed, the page behind has it back.
            s.stack.set_visible_child_name("main");
            s.behind.grab_focus();
            pump(50);
            assert_eq!(focused(&s.window).as_ref(), Some(s.behind.upcast_ref()));
            s.window.destroy();
        });
    }
}
