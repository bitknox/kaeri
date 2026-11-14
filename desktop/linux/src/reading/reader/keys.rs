//! The keyboard stays with the reader while it covers the window. The pages behind it stay in
//! the window's stack, so anything there that takes focus (a field a shortcut focuses, a dialog
//! closing and putting focus back where it was opened from) would take the keys with it: Esc
//! and the arrows would go to an unseen control and the reader would not hear them.
//!
//! When the reader closes, the keyboard goes back to the control that opened it.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

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

/// The control that had the keyboard when the reader opened, which has it back when the reader
/// closes. A series page draws its main action and chapter rows again as the reader closes, to
/// show the new place, so when that control has gone, the one drawn in its place takes the
/// keyboard: the same kind of control with the same action on the same page, the one with the
/// same target first.
#[derive(Default)]
pub struct Opener {
    control: glib::WeakRef<gtk::Widget>,
    /// The page the control is on.
    page: glib::WeakRef<gtk::Widget>,
    kind: Cell<Option<glib::Type>>,
    action: RefCell<Option<(glib::GString, Option<glib::Variant>)>>,
    /// Given the keyboard back, and nothing else has taken it since.
    giving: Cell<bool>,
}

impl Opener {
    /// Note what has the keyboard as `reader` opens over the window. Focus in a dialog, or none,
    /// leaves nothing to go back to. Opened again straight after a close, the keyboard is still
    /// in the reader, or nowhere, until the close's give-back lands: the control from before
    /// still holds then.
    pub fn note(&self, window: &impl IsA<gtk::Window>, reader: &impl IsA<gtk::Widget>) {
        let reader = reader.as_ref();
        let focus = focused(window.as_ref());
        let at = place(focus.as_ref(), reader);
        let control = focus.filter(|_| at == Focus::Behind);
        let pending = self.giving.get() || at == Focus::Inside;
        self.giving.set(false);
        if control.is_none() && pending && self.control.upgrade().is_some() {
            return;
        }
        self.remember(control.as_ref(), reader);
    }

    fn remember(&self, control: Option<&gtk::Widget>, reader: &gtk::Widget) {
        self.control.set(control);
        self.page.set(control.and_then(|control| page_of(control, reader)).as_ref());
        self.kind.set(control.map(|control| control.type_()));
        self.action.replace(control.and_then(|control| {
            let actionable = control.dynamic_cast_ref::<gtk::Actionable>()?;
            Some((actionable.action_name()?, actionable.action_target_value()))
        }));
    }

    /// Give the keyboard back as `reader` closes, and keep giving it to the control drawn in its
    /// place while the page redraws, until something else takes it.
    pub fn give_back(
        self: &Rc<Self>,
        window: &impl IsA<gtk::Window>,
        reader: &impl IsA<gtk::Widget>,
    ) {
        self.giving.set(true);
        self.later(window.as_ref(), reader.as_ref(), true);
    }

    /// Follow the window's focus for [`give_back`](Self::give_back).
    pub fn watch(
        self: &Rc<Self>,
        window: &impl IsA<gtk::Window>,
        reader: &impl IsA<gtk::Widget>,
    ) {
        window.as_ref().connect_notify_local(Some("focus-widget"), {
            let (this, reader) = (Rc::downgrade(self), reader.as_ref().downgrade());
            move |window, _| {
                let (Some(this), Some(reader)) = (this.upgrade(), reader.upgrade()) else {
                    return;
                };
                if !this.giving.get() || covers(&reader) {
                    return;
                }
                let control = this.control.upgrade();
                if control.is_some() && focused(window) == control {
                    return;
                }
                this.later(window, &reader, false);
            }
        });
    }

    /// After the change under way has finished: the control again if it is still on show, one
    /// drawn in its place if it has gone, and nothing if the person has moved on since `first`.
    fn later(self: &Rc<Self>, window: &gtk::Window, reader: &gtk::Widget, first: bool) {
        let (this, window, reader) =
            (Rc::downgrade(self), window.downgrade(), reader.downgrade());
        glib::idle_add_local_once(move || {
            if let (Some(this), Some(window), Some(reader)) =
                (this.upgrade(), window.upgrade(), reader.upgrade())
            {
                this.give(&window, &reader, first);
            }
        });
    }

    fn give(&self, window: &gtk::Window, reader: &gtk::Widget, first: bool) {
        if !self.giving.get() || covers(reader) {
            return;
        }
        let focus = focused(window);
        let control = self.control.upgrade();
        if let Some(control) = control.as_ref().filter(|control| control.root().is_some()) {
            if focus.as_ref() == Some(control) {
                return;
            }
            // Still there, and the focus went elsewhere since: that was the person.
            if (focus.is_some() && !first) || !usable(control) {
                self.giving.set(false);
                return;
            }
            control.grab_focus();
            return;
        }
        // Gone: the page drew it again.
        match self.stand_in() {
            Some(stand_in) => {
                stand_in.grab_focus();
                self.remember(Some(&stand_in), reader);
            }
            None => self.giving.set(false),
        }
    }

    /// The control drawn in the place of the one that went: on the same page, still shown.
    fn stand_in(&self) -> Option<gtk::Widget> {
        let page = self.page.upgrade().filter(shown)?;
        let kind = self.kind.get()?;
        let (name, target) = self.action.borrow().clone()?;
        let mut alike = Vec::new();
        descendants(&page, &mut |widget| {
            if widget.type_() == kind
                && usable(widget)
                && let Some(actionable) = widget.dynamic_cast_ref::<gtk::Actionable>()
                && actionable.action_name().as_ref() == Some(&name)
            {
                alike.push((actionable.action_target_value() == target, widget.clone()));
            }
        });
        let at = alike.iter().position(|(same, _)| *same).unwrap_or(0);
        (!alike.is_empty()).then(|| alike.swap_remove(at).1)
    }
}

/// The page `control` is on: its navigation page, or the window's page if it has none.
fn page_of(control: &gtk::Widget, reader: &gtk::Widget) -> Option<gtk::Widget> {
    let stack = reader.parent()?;
    let mut at = Some(control.clone());
    while let Some(widget) = at {
        let parent = widget.parent();
        if widget.is::<adw::NavigationPage>() || parent.as_ref() == Some(&stack) {
            return Some(widget);
        }
        at = parent;
    }
    None
}

/// Whether `page` is on show: the page its navigation view shows, or mapped.
fn shown(page: &gtk::Widget) -> bool {
    if !page.is_mapped() {
        return false;
    }
    let Some(page) = page.downcast_ref::<adw::NavigationPage>() else { return true };
    page.ancestor(adw::NavigationView::static_type())
        .and_downcast::<adw::NavigationView>()
        .is_none_or(|view| view.visible_page().as_ref() == Some(page))
}

fn usable(widget: &gtk::Widget) -> bool {
    widget.is_mapped() && widget.is_sensitive() && widget.is_focusable()
}

fn descendants(widget: &gtk::Widget, visit: &mut impl FnMut(&gtk::Widget)) {
    let mut child = widget.first_child();
    while let Some(next) = child {
        visit(&next);
        descendants(&next, visit);
        child = next.next_sibling();
    }
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

    /// A page of buttons that open the reader, as a series page has its main action and its
    /// chapter rows.
    fn openers(s: &Scene) -> gtk::Box {
        let group = gtk::gio::SimpleActionGroup::new();
        group.add_action(&gtk::gio::SimpleAction::new("read", Some(glib::VariantTy::STRING)));
        s.window.insert_action_group("t", Some(&group));
        let page = s.behind.parent().and_downcast::<gtk::Box>().expect("the page");
        let head = gtk::Box::new(gtk::Orientation::Vertical, 0);
        page.append(&head);
        head
    }

    fn opener(label: &str, chapter: &str) -> gtk::Button {
        let button = gtk::Button::with_label(label);
        button.set_action_target_value(Some(&chapter.to_variant()));
        button.set_action_name(Some("t.read"));
        button
    }

    /// The focus once the window has settled on `expected`, or after a second. GTK moves the
    /// focus off a removed control at the next frame, and the stand-in takes it after that.
    fn settled(window: &gtk::Window, expected: &impl IsA<gtk::Widget>) -> Option<gtk::Widget> {
        for _ in 0..100 {
            pump(10);
            if focused(window).as_ref() == Some(expected.upcast_ref()) {
                break;
            }
        }
        focused(window)
    }

    /// Open the reader from `control` and close it again, as `Reader::open` and `close` do.
    fn open_and_close(s: &Scene, held: &Rc<Opener>, control: &impl IsA<gtk::Widget>) {
        control.grab_focus();
        pump(20);
        held.note(&s.window, &s.reader);
        s.stack.set_visible_child(&s.reader);
        s.reader.grab_focus();
        pump(50);
        s.stack.set_visible_child_name("main");
        held.give_back(&s.window, &s.reader);
        pump(50);
    }

    #[test]
    fn closing_gives_the_keyboard_back_to_what_opened_the_reader() {
        run(|| {
            let s = scene();
            let head = openers(&s);
            let read = opener("Continue chapter 3", "3");
            head.append(&read);
            pump(50);
            let held = Rc::new(Opener::default());
            held.watch(&s.window, &s.reader);
            open_and_close(&s, &held, &read);
            assert_eq!(focused(&s.window).as_ref(), Some(read.upcast_ref()));
            s.window.destroy();
        });
    }

    /// The series page draws its main action again as the reader closes, with the new place.
    #[test]
    fn a_control_drawn_again_takes_the_keyboard_in_place_of_the_one_that_went() {
        run(|| {
            let s = scene();
            let head = openers(&s);
            let read = opener("Continue chapter 3", "3");
            head.append(&read);
            pump(50);
            let held = Rc::new(Opener::default());
            held.watch(&s.window, &s.reader);
            open_and_close(&s, &held, &read);
            // Drawn again: the main action now goes on from chapter 4, and a link to chapter 3
            // of another kind is no stand-in.
            head.remove(&read);
            let link = gtk::LinkButton::new("https://kaeri.example");
            link.set_action_target_value(Some(&"3".to_variant()));
            link.set_action_name(Some("t.read"));
            head.prepend(&link);
            let again = opener("Continue chapter 4", "4");
            head.append(&again);
            assert_eq!(settled(&s.window, &again).as_ref(), Some(again.upcast_ref()));
            // And again: the same target first.
            head.remove(&again);
            let other = opener("Chapter 2", "2");
            let same = opener("Chapter 4", "4");
            head.append(&other);
            head.append(&same);
            assert_eq!(settled(&s.window, &same).as_ref(), Some(same.upcast_ref()));
            s.window.destroy();
        });
    }

    /// Esc and an open at once: the keyboard is still in the reader when it opens again, and
    /// the control from the first open has it back after the second close.
    #[test]
    fn opened_again_before_the_give_back_the_first_control_still_holds() {
        run(|| {
            let s = scene();
            let head = openers(&s);
            let read = opener("Continue chapter 3", "3");
            head.append(&read);
            pump(50);
            let held = Rc::new(Opener::default());
            held.watch(&s.window, &s.reader);
            read.grab_focus();
            pump(20);
            held.note(&s.window, &s.reader);
            s.stack.set_visible_child(&s.reader);
            s.reader.grab_focus();
            pump(50);
            // Closed, and opened again before the give-back has run.
            s.stack.set_visible_child_name("main");
            held.give_back(&s.window, &s.reader);
            held.note(&s.window, &s.reader);
            s.stack.set_visible_child(&s.reader);
            pump(50);
            s.stack.set_visible_child_name("main");
            held.give_back(&s.window, &s.reader);
            assert_eq!(settled(&s.window, &read).as_ref(), Some(read.upcast_ref()));
            s.window.destroy();
        });
    }

    /// Once the person has put the keyboard elsewhere, a redraw doesn't take it back.
    #[test]
    fn the_keyboard_stays_where_the_person_put_it_after_closing() {
        run(|| {
            let s = scene();
            let head = openers(&s);
            let read = opener("Continue chapter 3", "3");
            head.append(&read);
            pump(50);
            let held = Rc::new(Opener::default());
            held.watch(&s.window, &s.reader);
            open_and_close(&s, &held, &read);
            s.behind.grab_focus();
            pump(50);
            head.remove(&read);
            head.append(&opener("Continue chapter 4", "4"));
            pump(50);
            assert_eq!(focused(&s.window).as_ref(), Some(s.behind.upcast_ref()));
            s.window.destroy();
        });
    }
}
