//! People you follow on AniList: the section at the end of a title page's side column, five of
//! them and Show all, and the marks on its episode rows of who stopped where. Both show only while
//! AniList is signed in and Show people you follow is on; the engine sends nobody otherwise.

use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    time::{Duration, Instant},
};

use adw::prelude::*;
use gtk::glib;
use kaeri_i18n::t;
use kaeri_model::{
    catalog::{Entry, FollowedEntry, LibrarySegment},
    episodes::FollowedMark,
    settings::SettingKey,
};

use crate::{catalog, fit::Fit, hub::Hub, motion, widgets};

/// How many people the section shows before Show all.
const FIRST: usize = 5;
/// How long a title page waits for AniList before it holds the section's place.
const WAIT: Duration = Duration::from_millis(600);
/// How long the placeholders stay once shown, so they never flash.
const HOLD: Duration = Duration::from_millis(300);
/// A person's picture in the section.
const FACE: i32 = 32;
/// A picture on an episode's mark.
pub const MARK: i32 = 24;
/// The column the marks stand in at an episode row's end, the same on every row.
pub const MARKS_WIDTH: i32 = 80;
/// Pictures on a mark before the count.
const PICTURES: usize = 3;

/// Whether people followed show: AniList signed in and the switch on, as it is by default.
pub fn on(hub: &Hub) -> bool {
    let known = hub.known.borrow();
    known.anilist().is_some()
        && known.setting(SettingKey::ShowFollowing).is_none_or(|row| catalog::is_on(&row))
}

/// The line under a person's name: how far they are for Watching, Paused and Dropped, the list
/// alone for Completed and Planning.
pub fn line(person: &FollowedEntry) -> Option<String> {
    use LibrarySegment as S;
    let progress = (person.progress > 0).then(|| t::episode_number(i64::from(person.progress)));
    match (person.status, progress) {
        (Some(status @ (S::Watching | S::Paused | S::Dropped)), Some(progress)) => {
            Some(t::more_followed_progress(&super::library::segment_title(status), &progress))
        }
        (Some(status), _) => Some(super::library::segment_title(status)),
        (None, progress) => progress,
    }
}

/// A score out of 100 as the section says it, out of 10: none when there is none, or it rounds
/// to nought.
pub fn score(person: &FollowedEntry) -> Option<i64> {
    let score = person.score?;
    let tenths = (i64::from(score) + 5) / 10;
    (tenths > 0).then_some(tenths)
}

/// Names as a list: "Ines", "Ines and Lumi", "Ines, Lumi and Dario". The pair is the catalogue's
/// own, and the separator before it is the one the catalogue puts between the first two names.
pub fn join_names(names: &[&str]) -> String {
    match names {
        [] => String::new(),
        [one] => (*one).to_owned(),
        [head @ .., last] => {
            let probe = t::following_names("\u{1}", "\u{2}", 2);
            let separator = probe
                .split_once('\u{1}')
                .and_then(|(_, rest)| rest.split_once('\u{2}'))
                .map_or(", ", |(between, _)| between);
            t::following_names_two(&head.join(separator), last)
        }
    }
}

/// What a mark's tooltip says and an episode row adds for a screen reader. Never a score.
pub fn spoken(marks: &[FollowedMark]) -> String {
    let names: Vec<&str> = marks.iter().map(|m| m.name.as_str()).collect();
    t::following_mark_spoken(&join_names(&names))
}

/// The people on a run of rows, each once, in the order they come.
pub fn union<'a>(marks: impl IntoIterator<Item = &'a FollowedMark>) -> Vec<FollowedMark> {
    let mut all: Vec<FollowedMark> = Vec::new();
    for mark in marks {
        if !all.iter().any(|m| m.name == mark.name) {
            all.push(mark.clone());
        }
    }
    all
}

/// A fade of `widget`'s opacity that plays under Reduce Animation too, where fades are what
/// reduced motion keeps.
fn fade(
    widget: &impl IsA<gtk::Widget>,
    to: f64,
    ms: u32,
    easing: adw::Easing,
) -> adw::TimedAnimation {
    let from = widget.opacity();
    let animation = motion::timed(widget, from, to, ms, motion::opacity(widget));
    animation.set_easing(easing);
    animation.set_follow_enable_animations_setting(false);
    animation
}

/// Fade `widget` in from clear over 0.2 s, once it is on screen.
fn fade_in(widget: &impl IsA<gtk::Widget>) {
    widget.set_opacity(0.0);
    let target = widget.as_ref().clone();
    motion::once_mapped(widget, move || {
        fade(&target, 1.0, 200, adw::Easing::EaseOutCubic).play();
    });
}

/// A person's picture in a circle, or the first letter of their name on the selected raised
/// fill when they have none.
fn face(hub: &Hub, name: &str, url: Option<&str>, size: i32, ringed: bool) -> gtk::Widget {
    let small = size < FACE;
    let widget: gtk::Widget = match url.filter(|u| !u.is_empty()) {
        Some(url) => {
            let (frame, _) = widgets::art(hub, Some(url), None, "followed-face");
            if ringed {
                frame.add_css_class("ringed");
            }
            Fit::new(&frame, 1.0, size, true, 0).upcast()
        }
        None => {
            let letter: String =
                name.chars().next().into_iter().flat_map(char::to_uppercase).collect();
            let label = gtk::Label::new(Some(&letter));
            label.add_css_class("followed-face");
            label.add_css_class("followed-letter");
            if small {
                label.add_css_class("small");
            }
            if ringed {
                label.add_css_class("ringed");
            }
            label.set_size_request(size, size);
            label.upcast()
        }
    };
    widget.set_halign(gtk::Align::Center);
    widget.set_valign(gtk::Align::Center);
    widget.set_can_target(!ringed);
    widget.set_accessible_role(gtk::AccessibleRole::Presentation);
    widget
}

/// The mark for `marks`: three pictures, then +N, with the sentence as its tooltip.
fn mark(hub: &Hub, marks: &[FollowedMark]) -> gtk::Box {
    let stack = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .accessible_role(gtk::AccessibleRole::Presentation)
        .halign(gtk::Align::End)
        .valign(gtk::Align::Center)
        .hexpand(true)
        .build();
    stack.add_css_class("followed-marks");
    for (at, person) in marks.iter().take(PICTURES).enumerate() {
        let picture = face(hub, &person.name, person.avatar_url.as_deref(), MARK, true);
        if at > 0 {
            picture.add_css_class("overlap");
        }
        stack.append(&picture);
    }
    if marks.len() > PICTURES {
        let more = i64::try_from(marks.len() - PICTURES).unwrap_or(i64::MAX);
        let count = gtk::Label::new(Some(&t::following_mark_more(more)));
        count.add_css_class("followed-more");
        count.add_css_class("overlap");
        count.add_css_class("ringed");
        count.add_css_class("numeric");
        count.set_valign(gtk::Align::Center);
        count.set_can_target(false);
        stack.append(&count);
    }
    stack.set_tooltip_text(Some(&spoken(marks)));
    stack
}

/// A slot at an episode row's end for its mark: the fixed column on wide rows, or only as wide as
/// the mark on narrow ones.
pub fn slot(fixed: bool) -> gtk::Box {
    let slot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    slot.set_valign(gtk::Align::Center);
    if fixed {
        slot.set_size_request(MARKS_WIDTH, -1);
    }
    slot
}

/// Put `marks` in `slot`: faded in over 0.2 s when `animate`, faded out over 0.3 s when they go.
pub fn set_marks(hub: &Hub, slot: &gtk::Box, marks: &[FollowedMark], animate: bool) {
    let old = slot.first_child();
    if marks.is_empty() {
        let Some(old) = old else { return };
        if animate && old.is_mapped() {
            let (slot, gone) = (slot.clone(), old.clone());
            let animation = fade(&old, 0.0, 300, adw::Easing::EaseInCubic);
            animation.connect_done(move |_| {
                if gone.parent().as_ref() == Some(slot.upcast_ref::<gtk::Widget>()) {
                    slot.remove(&gone);
                }
            });
            animation.play();
        } else {
            slot.remove(&old);
        }
        return;
    }
    if let Some(old) = old {
        slot.remove(&old);
    }
    let new = mark(hub, marks);
    slot.append(&new);
    if animate {
        fade_in(&new);
    }
}

/// The section on a title page: nothing until AniList answers, two placeholder rows from 0.6 s
/// while it is slow, then the people, or nothing when nobody followed has the show.
pub struct Section {
    hub: Rc<Hub>,
    /// The placeholders and the people in one cell, so one crossfades into the other in place.
    pub widget: gtk::Grid,
    holding: gtk::Box,
    people: gtk::Box,
    /// The people drawn, once the page has answered.
    shown: RefCell<Option<Vec<FollowedEntry>>>,
    /// When the placeholders came, while they show.
    held_since: Cell<Option<Instant>>,
    /// The answer waiting for the placeholders' least time to pass.
    pending: RefCell<Option<Vec<FollowedEntry>>>,
    waiting: RefCell<Option<glib::SourceId>>,
    me: RefCell<Weak<Section>>,
}

impl Section {
    pub fn new(hub: &Rc<Hub>) -> Rc<Self> {
        let widget = gtk::Grid::new();
        widget.set_visible(false);
        let holding = placeholders();
        let people = gtk::Box::new(gtk::Orientation::Vertical, 0);
        holding.set_visible(false);
        people.set_visible(false);
        widget.attach(&holding, 0, 0, 1, 1);
        widget.attach(&people, 0, 0, 1, 1);
        let section = Rc::new(Self {
            hub: Rc::clone(hub),
            widget,
            holding,
            people,
            shown: RefCell::default(),
            held_since: Cell::new(None),
            pending: RefCell::default(),
            waiting: RefCell::default(),
            me: RefCell::default(),
        });
        section.me.replace(Rc::downgrade(&section));
        section
    }

    /// The page opened: from 0.6 s on, placeholders hold the place while AniList has not
    /// answered.
    pub fn wait(&self) {
        let me = self.me.borrow().clone();
        let id = glib::timeout_add_local_once(WAIT, move || {
            let Some(section) = me.upgrade() else { return };
            section.waiting.replace(None);
            if section.shown.borrow().is_none() && on(&section.hub) {
                section.hold();
            }
        });
        self.waiting.replace(Some(id));
    }

    fn hold(&self) {
        self.held_since.set(Some(Instant::now()));
        self.widget.set_visible(true);
        self.holding.set_visible(true);
        fade_in(&self.holding);
    }

    /// What the page knows now: the people of a whole answer, or nothing yet.
    pub fn show(&self, entry: &Entry) {
        let Some(detail) = entry.detail.as_ref().filter(|d| !d.partial) else { return };
        if let Some(id) = self.waiting.take() {
            id.remove();
        }
        let people = detail.following.clone();
        if let Some(since) = self.held_since.get() {
            // The placeholders stay their least time, then crossfade to the latest answer.
            let first = self.pending.replace(Some(people)).is_none();
            if first {
                let me = self.me.borrow().clone();
                let left = HOLD.saturating_sub(since.elapsed());
                glib::timeout_add_local_once(left, move || {
                    if let Some(section) = me.upgrade() {
                        section.settle();
                    }
                });
            }
            return;
        }
        if self.shown.borrow().as_ref() == Some(&people) {
            return;
        }
        let before = self.shown.replace(Some(people.clone()));
        let was = before.as_ref().is_some_and(|p| !p.is_empty());
        if people.is_empty() {
            if was && self.people.is_mapped() {
                // Switched off on an open page: the section fades out where it stands.
                let (widget, list) = (self.widget.clone(), self.people.clone());
                let animation = fade(&self.people, 0.0, 300, adw::Easing::EaseInCubic);
                animation.connect_done(move |_| {
                    list.set_visible(false);
                    list.set_opacity(1.0);
                    widget.set_visible(false);
                });
                animation.play();
            } else {
                self.people.set_visible(false);
                self.widget.set_visible(false);
            }
            return;
        }
        self.fill(&people);
        self.widget.set_visible(true);
        self.people.set_visible(true);
        if was {
            // A newer answer: the rows change in place.
            self.people.set_opacity(1.0);
        } else {
            fade_in(&self.people);
        }
    }

    /// The placeholders have had their time: one crossfade to the people, or a fade out when
    /// nobody followed has the show. The section stands at the column's end, so nothing moves.
    fn settle(&self) {
        let Some(people) = self.pending.take() else { return };
        self.held_since.set(None);
        self.shown.replace(Some(people.clone()));
        let holding = self.holding.clone();
        let out = fade(&self.holding, 0.0, 300, adw::Easing::EaseInOutCubic);
        if people.is_empty() {
            let widget = self.widget.clone();
            out.connect_done(move |_| {
                holding.set_visible(false);
                holding.set_opacity(1.0);
                widget.set_visible(false);
            });
            out.play();
            return;
        }
        out.connect_done(move |_| {
            holding.set_visible(false);
            holding.set_opacity(1.0);
        });
        self.fill(&people);
        self.people.set_opacity(0.0);
        self.people.set_visible(true);
        fade(&self.people, 1.0, 300, adw::Easing::EaseInOutCubic).play();
        out.play();
    }

    /// The group: its heading and note, five people, and Show all for the rest.
    fn fill(&self, people: &[FollowedEntry]) {
        while let Some(child) = self.people.first_child() {
            self.people.remove(&child);
        }
        let group = adw::PreferencesGroup::builder()
            .title(t::more_following())
            .description(t::more_following_note())
            .build();
        let card = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .accessible_role(gtk::AccessibleRole::List)
            .build();
        card.add_css_class("card");
        card.add_css_class("following-card");
        card.set_overflow(gtk::Overflow::Hidden);
        card.update_property(&[gtk::accessible::Property::Label(&t::more_following())]);
        for (at, person) in people.iter().take(FIRST).enumerate() {
            card.append(&self.row(person, at > 0));
        }
        if people.len() > FIRST {
            let rest = gtk::Box::new(gtk::Orientation::Vertical, 0);
            for person in &people[FIRST..] {
                rest.append(&self.row(person, true));
            }
            let grows = gtk::Revealer::builder()
                .transition_type(gtk::RevealerTransitionType::SlideDown)
                .transition_duration(300)
                .child(&rest)
                .build();
            let count = i64::try_from(people.len()).unwrap_or(i64::MAX);
            let all = gtk::Button::with_label(&t::following_show_all(count));
            all.add_css_class("flat");
            all.add_css_class("following-all");
            let goes = gtk::Revealer::builder()
                .transition_type(gtk::RevealerTransitionType::SlideUp)
                .transition_duration(300)
                .reveal_child(true)
                .child(&all)
                .build();
            all.connect_clicked({
                let (grows, goes, rest) = (grows.clone(), goes.clone(), rest.clone());
                move |all| {
                    // The list grows in place: the five rows fade in, the button fades out.
                    rest.set_opacity(0.0);
                    grows.set_reveal_child(true);
                    fade(&rest, 1.0, 200, adw::Easing::EaseOutCubic).play();
                    fade(all, 0.0, 300, adw::Easing::EaseInCubic).play();
                    goes.set_reveal_child(false);
                    all.set_can_focus(false);
                    if let Some(first) = rest.first_child() {
                        first.grab_focus();
                    }
                }
            });
            card.append(&grows);
            card.append(&goes);
        }
        group.add(&card);
        self.people.append(&group);
    }

    /// One person: picture, name, how far they are, and their score at the end.
    fn row(&self, person: &FollowedEntry, ruled: bool) -> gtk::Box {
        let row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .accessible_role(gtk::AccessibleRole::ListItem)
            .focusable(true)
            .build();
        row.add_css_class("following-row");
        if ruled {
            row.add_css_class("ruled");
        }
        row.append(&face(&self.hub, &person.name, person.avatar_url.as_deref(), FACE, false));
        let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
        text.set_hexpand(true);
        text.set_valign(gtk::Align::Center);
        let name = gtk::Label::new(Some(&person.name));
        name.set_xalign(0.0);
        name.set_ellipsize(gtk::pango::EllipsizeMode::End);
        text.append(&name);
        let line = line(person);
        if let Some(line) = &line {
            let label = widgets::haze(line, "following-line");
            label.set_wrap(false);
            label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            text.append(&label);
        }
        row.append(&text);
        let score = score(person);
        if let Some(score) = score {
            let label = widgets::haze(&t::more_followed_score(score), "numeric");
            label.set_wrap(false);
            label.set_valign(gtk::Align::Center);
            row.append(&label);
        }
        // Said as one: the name, the line, and the score in words.
        let spoken: Vec<String> = std::iter::once(person.name.clone())
            .chain(line)
            .chain(score.map(t::following_score_spoken))
            .collect();
        row.update_property(&[gtk::accessible::Property::Label(&spoken.join(", "))]);
        row
    }
}

/// Two rows held for people while AniList is slow: bars for the heading and note, then a
/// picture's circle and two lines each, in the section's own box.
fn placeholders() -> gtk::Box {
    let bar = |width: i32, height: i32| {
        let bar = gtk::Box::new(gtk::Orientation::Vertical, 0);
        bar.add_css_class("plate");
        bar.add_css_class("bar");
        bar.set_size_request(width, height);
        bar.set_halign(gtk::Align::Start);
        bar
    };
    let column = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .accessible_role(gtk::AccessibleRole::Presentation)
        .build();
    column.add_css_class("following-holding");
    let heading = bar(130, 11);
    heading.set_margin_top(4);
    heading.set_margin_bottom(10);
    let note = bar(76, 9);
    note.set_margin_bottom(12);
    column.append(&heading);
    column.append(&note);
    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    card.add_css_class("card");
    card.add_css_class("following-card");
    for at in 0..2 {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        row.add_css_class("following-row");
        if at > 0 {
            row.add_css_class("ruled");
        }
        let circle = gtk::Box::new(gtk::Orientation::Vertical, 0);
        circle.add_css_class("plate");
        circle.add_css_class("followed-face");
        circle.set_size_request(FACE, FACE);
        circle.set_valign(gtk::Align::Center);
        let lines = gtk::Box::new(gtk::Orientation::Vertical, 6);
        lines.set_valign(gtk::Align::Center);
        lines.append(&bar(96, 9));
        lines.append(&bar(140, 8));
        row.append(&circle);
        row.append(&lines);
        card.append(&row);
    }
    column.append(&card);
    column
}

#[cfg(test)]
mod tests {
    use super::*;

    fn person(
        status: Option<LibrarySegment>,
        progress: u32,
        score: Option<u32>,
    ) -> FollowedEntry {
        FollowedEntry { name: "Ines".into(), avatar_url: None, status, score, progress }
    }

    #[test]
    fn the_line_says_how_far_only_for_lists_that_are_part_way() {
        use LibrarySegment as S;
        assert_eq!(line(&person(Some(S::Watching), 8, None)).unwrap(), "Watching · Episode 8");
        assert_eq!(line(&person(Some(S::Paused), 5, None)).unwrap(), "Paused · Episode 5");
        assert_eq!(line(&person(Some(S::Dropped), 3, None)).unwrap(), "Dropped · Episode 3");
        assert_eq!(line(&person(Some(S::Completed), 12, None)).unwrap(), "Completed");
        assert_eq!(line(&person(Some(S::Planning), 0, None)).unwrap(), "Planning");
        assert_eq!(line(&person(Some(S::Watching), 0, None)).unwrap(), "Watching");
    }

    #[test]
    fn a_score_out_of_100_is_said_out_of_10_and_nought_is_left_out() {
        assert_eq!(score(&person(None, 0, Some(80))), Some(8));
        assert_eq!(score(&person(None, 0, Some(75))), Some(8));
        assert_eq!(score(&person(None, 0, Some(74))), Some(7));
        assert_eq!(score(&person(None, 0, Some(4))), None);
        assert_eq!(score(&person(None, 0, Some(0))), None);
        assert_eq!(score(&person(None, 0, None)), None);
    }

    #[test]
    fn names_are_joined_as_a_list() {
        assert_eq!(join_names(&[]), "");
        assert_eq!(join_names(&["Ines"]), "Ines");
        assert_eq!(join_names(&["Ines", "Lumi"]), "Ines and Lumi");
        assert_eq!(
            join_names(&["Ines", "Lumi", "Dario", "Oskar"]),
            "Ines, Lumi, Dario and Oskar"
        );
    }

    #[test]
    fn a_folded_run_names_each_person_once_in_order() {
        let mark = |name: &str| FollowedMark { name: name.into(), avatar_url: None };
        let rows = [mark("TeoVale"), mark("Ines"), mark("TeoVale")];
        let names: Vec<String> = union(&rows).into_iter().map(|m| m.name).collect();
        assert_eq!(names, ["TeoVale", "Ines"]);
        assert_eq!(spoken(&union(&rows)), "TeoVale and Ines watched up to here.");
    }
}
