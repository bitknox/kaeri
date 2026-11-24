//! The player's side panel, with the Episodes and Sources tabs (player panel board, Linux
//! frame). It sits under the player's header bar at the window's right edge, 360 px wide, on the
//! raised fill with a hairline at its left edge, and slides in over 0.3 s and out over 0.2 s.
//!
//! Episodes: a click selects a row, which reveals its synopsis (three lines, with More while it
//! is cut) and its buttons; Play or Resume, Return or a double-click plays it. Sources: the
//! source playing and its streams, Subtitled or Dubbed, then the other sources in the order set
//! in Sources, a source with several releases folding them under it. A pick there switches the
//! episode playing from the playhead, for this episode only; its row says how the switch goes.
//!
//! The panel draws what the player hands it and says what was picked through its `act`; the
//! player sends the commands.

use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::{Rc, Weak},
    time::{Duration, Instant},
};

use adw::prelude::*;
use gtk::glib;
use kaeri_core::{media::Translation, traits::SourceCandidate};
use kaeri_i18n::t;
use kaeri_model::{
    episodes::{DubExtent, EpisodeRow, episode_ranges},
    hosted::MenuSource,
    sources::SourceOption,
    starting::{ListingState, SourceListing},
};

use super::panel_state::{
    self as state, Clock, EpisodeAction, Listing, ListingLine, PanelSwitch, PanelTab, Phase,
    Playhead, SwitchTarget, VersionSwitch,
};
use crate::{hub::Hub, motion, say, synopsis};

/// The panel's width, from the board.
pub const WIDTH: i32 = 360;
/// Opening and closing.
const OPEN_MS: u32 = 300;
const CLOSE_MS: u32 = 200;
/// A reveal, and when its text starts to fade in.
const REVEAL_MS: u32 = 300;
const REVEAL_TEXT_AFTER: u64 = 100;
/// How long a revealed row takes to open before it is brought into view.
const REVEAL_SETTLE: u64 = 320;
/// How many lines a synopsis shows before More.
const SYNOPSIS_LINES: i32 = 3;
/// A series longer than this shows one range of its episodes at a time.
const RANGE: usize = 100;
/// How often a pending switch's row is redrawn: its spinner from 600 ms, its count from 3 s.
const TICK: Duration = Duration::from_millis(250);
/// The fade-in token: a loading source's line crossfading in place.
const FADE_MS: u32 = 200;

/// What the panel asks the player to do.
#[derive(Debug, Clone, PartialEq)]
pub enum Act {
    /// The close button.
    Close,
    /// A tab came up: Sources asks for the episode's releases.
    Tab(PanelTab),
    /// A revealed row's button, or Return and a double-click on a row, for the episode numbered.
    Episode(EpisodeAction, String),
    /// A release from the Sources tab.
    Release(SourceCandidate),
    /// Another stream of the source playing.
    Stream(String),
    /// Subtitled or Dubbed.
    Translation(Translation),
}

/// What the player knows that the panel draws.
#[derive(Debug, Clone, Default)]
pub struct Data {
    pub table: Vec<EpisodeRow>,
    /// The episode playing.
    pub current: Option<String>,
    /// Whether spoiler protection is on for the show.
    pub protect: bool,
    /// The streams of the source playing, from the engine's menus.
    pub streams: Vec<MenuSource>,
    pub translation: Translation,
    /// The title's sources, in the order they are tried, once the engine has said.
    pub options: Option<Vec<SourceOption>>,
    /// The episode's releases across every source, once the engine has said.
    pub releases: Option<Vec<SourceCandidate>>,
    /// The source playing, by name, and the release, when one was picked here and landed.
    pub playing_source: Option<String>,
    pub playing_release: Option<SourceCandidate>,
    /// Whether the title has a dub, and how far it goes.
    pub has_dub: Option<bool>,
    pub dub: Option<DubExtent>,
}

/// One episode row's widgets.
struct EpRow {
    number: String,
    row: gtk::ListBoxRow,
    title: gtk::Label,
    detail: gtk::Label,
    bar: gtk::ProgressBar,
    revealer: gtk::Revealer,
    body: gtk::Box,
    /// The row's actions and their buttons, the amber one first when there is one.
    actions: Vec<(EpisodeAction, gtk::Button)>,
}

/// A stream, source or release row that a switch can be on.
struct Choice {
    target: SwitchTarget,
    on: bool,
    /// A source's row while the tab loads: its detail line is held one line high.
    held: bool,
    check: gtk::Stack,
    spinner: gtk::Spinner,
    line: gtk::Stack,
    starting: gtk::Label,
    alert: gtk::Revealer,
    alert_line: gtk::Label,
}

/// The Subtitled or Dubbed switch's widgets: a spinner over each half, the finding note and a
/// failure under it.
struct VersionRow {
    toggles: adw::ToggleGroup,
    spinners: [gtk::Spinner; 2],
    note: gtk::Revealer,
    note_line: gtk::Label,
    alert: gtk::Revealer,
    alert_line: gtk::Label,
}

/// What a Sources row does when it is activated.
enum RowDoes {
    Release(SourceCandidate),
    Stream(String),
    /// Fold or unfold a source's releases.
    Fold(String),
}

/// Where the panel says what was asked of it.
type ActFn = Rc<dyn Fn(Act)>;

pub struct SidePanel {
    /// What the player lays over the stage: the panel, sliding in from the right.
    pub widget: gtk::Revealer,
    me: Weak<Self>,
    hub: Rc<Hub>,
    tabs: adw::ToggleGroup,
    pub close: gtk::Button,
    stack: gtk::Stack,
    ep_scroll: gtk::ScrolledWindow,
    ep_note: gtk::Label,
    ep_ranges: gtk::DropDown,
    ep_list: gtk::ListBox,
    ep_loading: gtk::Box,
    ep_rows: RefCell<Vec<EpRow>>,
    src_scroll: gtk::ScrolledWindow,
    src_list: gtk::ListBox,
    choices: RefCell<HashMap<String, Choice>>,
    src_does: RefCell<Vec<(gtk::ListBoxRow, RowDoes)>>,
    /// Each folding source's releases, by its id: their rows.
    folds: RefCell<HashMap<String, (gtk::Image, Vec<gtk::ListBoxRow>)>>,
    /// The Sources tab's sources while they answer, from when it asked.
    listing: RefCell<Option<Listing>>,
    /// Their rows, in order, each changed in place as its source answers.
    listing_rows: RefCell<Vec<ListingRow>>,
    /// The source playing as Sources was last drawn, by its id and name: not among the others.
    playing: RefCell<(Option<String>, Option<String>)>,
    translation: RefCell<Option<VersionRow>>,
    data: RefCell<Data>,
    playhead: Cell<Playhead>,
    switch: RefCell<Option<PanelSwitch>>,
    /// The switch to the other version, from the pick until it lands or its failure.
    version: RefCell<Option<VersionSwitch>>,
    tick: RefCell<Option<glib::SourceId>>,
    /// The row revealed, by its number, and the synopses shown whole.
    revealed: RefCell<Option<String>>,
    whole: RefCell<HashSet<String>>,
    /// The sources whose releases are unfolded.
    expanded: RefCell<HashSet<String>>,
    /// The range of a long series shown, and whether the viewer picked it.
    range: Cell<Option<usize>>,
    /// Set while the panel builds or selects by itself: no reveal animates, nothing is said.
    quiet: Cell<bool>,
    /// Where the keyboard waits in the panel while the Sources rows load, to move onto the
    /// first of them once they are in.
    parked: RefCell<Option<gtk::Widget>>,
    act: RefCell<Option<ActFn>>,
}

impl SidePanel {
    pub fn new(hub: &Rc<Hub>) -> Rc<Self> {
        let tabs = adw::ToggleGroup::new();
        tabs.add(adw::Toggle::builder().name("episodes").label(t::player_episodes()).build());
        tabs.add(adw::Toggle::builder().name("sources").label(t::player_sources()).build());
        tabs.set_active_name(Some("episodes"));
        tabs.set_hexpand(true);
        tabs.set_homogeneous(true);
        tabs.add_css_class("panel-tabs");
        let close = gtk::Button::from_icon_name("window-close-symbolic");
        close.add_css_class("circular");
        close.add_css_class("panel-close");
        close.set_valign(gtk::Align::Center);
        close.set_tooltip_text(Some(&t::action_close()));
        close.update_property(&[gtk::accessible::Property::Label(&t::action_close())]);
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        head.add_css_class("panel-head");
        head.append(&tabs);
        head.append(&close);

        // Episodes: the note over a list from the tracker alone, a long series' range, the rows.
        let ep_note = crate::widgets::haze(&t::player_panel_metadata_only(), "panel-note");
        ep_note.set_visible(false);
        let ep_ranges = gtk::DropDown::from_strings(&[] as &[&str]);
        ep_ranges.add_css_class("flat");
        ep_ranges.set_halign(gtk::Align::Start);
        ep_ranges.set_visible(false);
        ep_ranges.set_tooltip_text(Some(&t::episode_range_label()));
        ep_ranges
            .update_property(&[gtk::accessible::Property::Label(&t::episode_range_label())]);
        let ep_list = gtk::ListBox::new();
        ep_list.add_css_class("navigation-sidebar");
        ep_list.add_css_class("panel-episodes");
        ep_list.set_selection_mode(gtk::SelectionMode::Single);
        ep_list.set_activate_on_single_click(false);
        ep_list.update_property(&[gtk::accessible::Property::Label(&t::player_episodes())]);
        let ep_loading = loading();
        let ep_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        ep_box.add_css_class("panel-body");
        ep_box.append(&ep_note);
        ep_box.append(&ep_ranges);
        ep_box.append(&ep_loading);
        ep_box.append(&ep_list);
        let ep_scroll = scroller(&ep_box);

        let src_list = gtk::ListBox::new();
        src_list.add_css_class("navigation-sidebar");
        src_list.add_css_class("panel-sources");
        src_list.set_selection_mode(gtk::SelectionMode::None);
        src_list.set_activate_on_single_click(true);
        src_list.update_property(&[gtk::accessible::Property::Label(&t::player_sources())]);
        let src_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        src_box.add_css_class("panel-body");
        src_box.append(&src_list);
        let src_scroll = scroller(&src_box);

        let stack = gtk::Stack::new();
        stack.set_transition_type(gtk::StackTransitionType::Crossfade);
        stack.set_transition_duration(300);
        stack.set_vexpand(true);
        stack.add_named(&ep_scroll, Some("episodes"));
        stack.add_named(&src_scroll, Some("sources"));

        let panel = gtk::Box::new(gtk::Orientation::Vertical, 0);
        panel.add_css_class("player-side-panel");
        panel.set_width_request(WIDTH);
        panel.set_accessible_role(gtk::AccessibleRole::Dialog);
        panel.update_property(&[gtk::accessible::Property::Label(&t::player_panel())]);
        panel.append(&head);
        panel.append(&stack);

        let widget = gtk::Revealer::new();
        widget.set_transition_type(gtk::RevealerTransitionType::SlideLeft);
        widget.set_halign(gtk::Align::End);
        widget.set_valign(gtk::Align::Fill);
        widget.set_child(Some(&panel));
        widget.set_visible(false);

        let view = Rc::new_cyclic(|me| Self {
            widget,
            me: me.clone(),
            hub: Rc::clone(hub),
            tabs,
            close,
            stack,
            ep_scroll,
            ep_note,
            ep_ranges,
            ep_list,
            ep_loading,
            ep_rows: RefCell::default(),
            src_scroll,
            src_list,
            choices: RefCell::default(),
            src_does: RefCell::default(),
            folds: RefCell::default(),
            listing: RefCell::default(),
            listing_rows: RefCell::default(),
            playing: RefCell::default(),
            translation: RefCell::default(),
            data: RefCell::default(),
            playhead: Cell::default(),
            switch: RefCell::default(),
            version: RefCell::default(),
            tick: RefCell::default(),
            revealed: RefCell::default(),
            whole: RefCell::default(),
            expanded: RefCell::default(),
            range: Cell::new(None),
            quiet: Cell::new(false),
            parked: RefCell::new(None),
            act: RefCell::default(),
        });
        view.wire();
        view
    }

    /// Where the panel's picks go.
    pub fn connect_act(&self, act: impl Fn(Act) + 'static) {
        self.act.replace(Some(Rc::new(act)));
    }

    fn say(&self, act: Act) {
        let act_fn = self.act.borrow().clone();
        if let Some(f) = act_fn {
            f(act);
        }
    }

    fn wire(&self) {
        let me = self.me.clone();
        self.close.connect_clicked(move |_| {
            if let Some(panel) = me.upgrade() {
                panel.say(Act::Close);
            }
        });
        let me = self.me.clone();
        self.tabs.connect_active_name_notify(move |tabs| {
            let Some(panel) = me.upgrade() else { return };
            let tab = PanelTab::from_name(tabs.active_name().as_deref().unwrap_or("episodes"));
            panel.stack.set_visible_child_name(tab.name());
            if !panel.quiet.get() {
                panel.say(Act::Tab(tab));
            }
        });
        let me = self.me.clone();
        self.ep_list.connect_row_selected(move |_, row| {
            if let Some(panel) = me.upgrade() {
                panel.selected(row);
            }
        });
        let me = self.me.clone();
        self.ep_list.connect_row_activated(move |_, row| {
            let Some(panel) = me.upgrade() else { return };
            let primary = panel.ep_rows.borrow().iter().find(|r| &r.row == row).and_then(|r| {
                r.actions
                    .first()
                    .filter(|(a, _)| a.primary())
                    .map(|(a, _)| (*a, r.number.clone()))
            });
            if let Some((action, number)) = primary {
                panel.say(Act::Episode(action, number));
            }
        });
        let me = self.me.clone();
        self.src_list.connect_row_activated(move |_, row| {
            let Some(panel) = me.upgrade() else { return };
            let does =
                panel.src_does.borrow().iter().find(|(r, _)| r == row).map(|(_, d)| match d {
                    RowDoes::Release(c) => RowDoes::Release(c.clone()),
                    RowDoes::Stream(s) => RowDoes::Stream(s.clone()),
                    RowDoes::Fold(p) => RowDoes::Fold(p.clone()),
                });
            match does {
                Some(RowDoes::Release(candidate)) => panel.say(Act::Release(candidate)),
                Some(RowDoes::Stream(id)) => panel.say(Act::Stream(id)),
                Some(RowDoes::Fold(provider)) => panel.fold(&provider),
                None => {}
            }
        });
        let me = self.me.clone();
        self.ep_ranges.connect_selected_notify(move |dropdown| {
            let Some(panel) = me.upgrade() else { return };
            if panel.quiet.get() {
                return;
            }
            panel.range.set(Some(dropdown.selected() as usize));
            panel.fill_episodes(false);
            panel.ep_scroll.vadjustment().set_value(0.0);
            let fade =
                motion::timed(&panel.ep_list, 0.4, 1.0, 200, motion::opacity(&panel.ep_list));
            fade.set_easing(adw::Easing::EaseOutCubic);
            fade.play();
        });
    }

    // Opening and closing

    pub fn is_open(&self) -> bool {
        self.widget.reveals_child()
    }

    /// Slide in on `tab`, the keyboard on the row playing (or the one revealed), or on the
    /// Sources tab's first choice.
    pub fn open(&self, tab: PanelTab) {
        self.show_tab(tab, false);
        if self.is_open() {
            self.focus_in();
            return;
        }
        self.widget.set_visible(true);
        self.widget.set_transition_duration(OPEN_MS);
        self.widget.set_transition_type(if motion::reduced() {
            gtk::RevealerTransitionType::Crossfade
        } else {
            gtk::RevealerTransitionType::SlideLeft
        });
        self.widget.set_reveal_child(true);
        let me = self.me.clone();
        motion::once_mapped(&self.widget, move || {
            if let Some(panel) = me.upgrade() {
                panel.scroll_to_playing();
                panel.focus_in();
            }
        });
        if tab == PanelTab::Sources {
            self.say(Act::Tab(PanelTab::Sources));
        }
    }

    pub fn close(&self) {
        if !self.is_open() {
            return;
        }
        self.widget.set_transition_duration(CLOSE_MS);
        self.widget.set_reveal_child(false);
        let me = self.me.clone();
        glib::timeout_add_local_once(
            Duration::from_millis(u64::from(CLOSE_MS) + 20),
            move || {
                if let Some(panel) = me.upgrade()
                    && !panel.widget.reveals_child()
                {
                    panel.widget.set_visible(false);
                }
            },
        );
    }

    pub fn tab(&self) -> PanelTab {
        PanelTab::from_name(self.tabs.active_name().as_deref().unwrap_or("episodes"))
    }

    /// Bring `tab` up, as Ctrl+1 and Ctrl+2 do, with the keyboard in it when `focus`.
    pub fn show_tab(&self, tab: PanelTab, focus: bool) {
        if self.tab() != tab {
            self.quiet.set(!self.is_open());
            self.tabs.set_active_name(Some(tab.name()));
            self.quiet.set(false);
        }
        if focus {
            self.focus_in();
        }
    }

    /// Whether `widget` is inside the panel.
    pub fn holds(&self, widget: &gtk::Widget) -> bool {
        widget.is_ancestor(&self.widget) || widget == self.widget.upcast_ref::<gtk::Widget>()
    }

    fn focus_in(&self) {
        match self.tab() {
            PanelTab::Episodes => {
                let at = self
                    .revealed
                    .borrow()
                    .clone()
                    .or_else(|| self.data.borrow().current.clone());
                let rows = self.ep_rows.borrow();
                let row = at
                    .and_then(|n| rows.iter().find(|r| r.number == n))
                    .or_else(|| rows.first())
                    .map(|r| r.row.clone());
                drop(rows);
                match row {
                    Some(row) => {
                        row.grab_focus();
                    }
                    None => {
                        self.tabs.grab_focus();
                    }
                }
            }
            PanelTab::Sources => {
                if !self.focus_first_source() {
                    // Nothing to land on yet: wait on the tabs, and move on when the rows fill.
                    self.park();
                }
            }
        }
    }

    /// The keyboard onto the first Sources row that takes it, when there is one.
    fn focus_first_source(&self) -> bool {
        let rows: Vec<gtk::ListBoxRow> =
            self.src_does.borrow().iter().map(|(row, _)| row.clone()).collect();
        let landed =
            rows.iter().any(|row| row.is_visible() && row.is_focusable() && row.grab_focus());
        if landed {
            self.parked.replace(None);
        }
        landed
    }

    /// Keep the keyboard in the panel on its tabs, or on Close, and remember where.
    fn park(&self) {
        let root = self.widget.root();
        let on_tabs = self.tabs.child_focus(gtk::DirectionType::TabForward);
        if !on_tabs {
            self.close.grab_focus();
        }
        let now = root.and_then(|r| gtk::prelude::RootExt::focus(&r));
        self.parked.replace(now.filter(|w| self.holds(w)));
    }

    /// Whether the keyboard still waits where `park` left it.
    fn still_parked(&self) -> bool {
        let Some(parked) = self.parked.borrow().clone() else { return false };
        let now = self.widget.root().and_then(|r| gtk::prelude::RootExt::focus(&r));
        self.is_open() && self.tab() == PanelTab::Sources && now.as_ref() == Some(&parked)
    }

    // What the player hands in

    /// The episodes, the menus and the title's sources changed: the lists are drawn again,
    /// keeping the row revealed, the folds open and the keyboard where it was. Sources is left
    /// as it is when nothing it draws changed, so its rows' fades and spinners run on.
    pub fn set_data(&self, data: Data) {
        let episode_changed = self.data.borrow().current != data.current;
        let (table_changed, sources_changed) = {
            let old = self.data.borrow();
            (
                old.table != data.table || old.protect != data.protect || episode_changed,
                !same_sources(&old, &data),
            )
        };
        if episode_changed {
            // Another episode: its releases and picks are its own.
            self.expanded.borrow_mut().clear();
            self.switch.replace(None);
            self.revealed.replace(None);
            self.range.set(None);
            self.listing.replace(None);
        }
        self.data.replace(data);
        if table_changed {
            self.fill_episodes(episode_changed);
        }
        if sources_changed || episode_changed {
            self.fill_sources();
        }
    }

    pub fn data(&self) -> std::cell::Ref<'_, Data> {
        self.data.borrow()
    }

    /// The tab asked for the episode's releases: every source's row comes with the engine's
    /// first progress, silent for 600 ms.
    pub fn listing_asked(&self) {
        self.listing.replace(Some(Listing::asked(Instant::now())));
        self.data.borrow_mut().releases = None;
        self.fill_sources();
    }

    /// Every source's row as the engine says it now, whole. A row that changed changes in place.
    pub fn set_progress(&self, rows: Vec<SourceListing>) {
        {
            let mut listing = self.listing.borrow_mut();
            let Some(listing) = listing.as_mut() else { return };
            listing.took(rows, Instant::now());
        }
        if self.refresh_listing() {
            self.show_switch();
        } else {
            self.fill_sources();
        }
    }

    /// The engine ended the listing before asking any source, for `why`: the rows still
    /// waiting stop and dim in place, and a note under them says why.
    pub fn sources_failed(&self, why: String) {
        {
            let mut listing = self.listing.borrow_mut();
            let Some(listing) = listing.as_mut() else { return };
            if listing.ended.is_some() {
                return;
            }
            listing.end(why.clone());
        }
        let last = self.listing_rows.borrow().last().map(|r| r.row.index());
        match last {
            Some(at) => self.src_list.insert(&ended_note(&why), at + 1),
            None => {
                self.fill_sources();
                return;
            }
        }
        self.show_switch();
    }

    /// The releases for the episode playing, from the engine. Once the rows came from its
    /// progress they stay drawn from it, and the releases are kept for the picks.
    pub fn set_releases(&self, releases: Option<Vec<SourceCandidate>>) {
        let from_progress = self.listing.borrow().as_ref().is_some_and(|l| l.rows.is_some());
        let answered = releases.is_some();
        self.data.borrow_mut().releases = releases;
        if from_progress && answered {
            return;
        }
        if answered {
            self.listing.replace(None);
        }
        self.fill_sources();
    }

    pub fn set_options(&self, options: Option<Vec<SourceOption>>) {
        self.data.borrow_mut().options = options;
        self.fill_sources();
    }

    /// Where the playhead is: the playing row's line and progress follow it.
    pub fn set_playhead(&self, playhead: Playhead) {
        let old = self.playhead.replace(playhead);
        let minute = |p: Playhead| {
            (
                ((p.duration.unwrap_or(0.0) - p.position) / 60.0).round() as i64,
                p.duration.map(|d| (p.position / d * 200.0) as i64),
            )
        };
        if minute(old) == minute(playhead) {
            return;
        }
        let data = self.data.borrow();
        let Some(current) = &data.current else { return };
        let Some(row) = data.table.iter().find(|r| &r.number == current) else { return };
        let meta = metadata_only(&data.table);
        for ep in self.ep_rows.borrow().iter().filter(|r| &r.number == current) {
            ep.detail.set_label(&self.detail(row, true, meta, &data));
            if let Some(fraction) = state::progress(row, true, meta, playhead) {
                ep.bar.set_fraction(fraction);
                ep.bar.set_visible(fraction > 0.0);
            }
        }
    }

    // Switches

    /// A switch was asked for from the panel.
    pub fn asked(&self, target: SwitchTarget, candidate: Option<SourceCandidate>) {
        self.switch.replace(Some(PanelSwitch::asked(target, candidate, Instant::now())));
        self.show_switch();
    }

    /// The engine says how a switch goes.
    pub fn switching(&self, target: SwitchTarget, phase: Phase) {
        if let Some(release) = &target.release
            && !target.provider_id.is_empty()
        {
            // A release picked elsewhere, or tried again, shows on its row.
            let folds = self.data.borrow().releases.as_ref().is_some_and(|all| {
                all.iter().filter(|c| c.provider_id == target.provider_id).count() > 1
                    && all.iter().any(|c| &c.id == release)
            });
            if folds && self.expanded.borrow_mut().insert(target.provider_id.clone()) {
                self.fill_sources();
            }
        }
        let known = self.switch.take();
        self.switch.replace(Some(PanelSwitch::track(known, target, phase, Instant::now())));
        self.show_switch();
    }

    /// The switch landed: what plays now is at the top of Sources.
    pub fn landed(&self, target: &SwitchTarget) {
        let ours = self.switch.borrow().as_ref().is_none_or(|s| s.on(target));
        if ours {
            self.switch.replace(None);
        }
        self.fill_sources();
        let adjustment = self.src_scroll.vadjustment();
        let from = adjustment.value();
        if from > 0.0 {
            let to_top = adw::CallbackAnimationTarget::new(move |v| adjustment.set_value(v));
            let scroll = motion::timed(&self.src_scroll, from, 0.0, 300, to_top);
            scroll.set_easing(adw::Easing::EaseOutCubic);
            scroll.play();
        }
    }

    /// The other version was picked: false while one is still on its way, as the engine finds
    /// a single version at a time.
    pub fn version_asked(&self, to: Translation) -> bool {
        if self.version.borrow().as_ref().is_some_and(|v| v.failed().is_none()) {
            return false;
        }
        self.version.replace(Some(VersionSwitch::asked(to, Instant::now())));
        self.show_switch();
        true
    }

    /// The engine says how the switch to the other version goes.
    pub fn versioning(&self, to: Translation, phase: Phase) {
        let known = self.version.take();
        self.version.replace(Some(VersionSwitch::track(known, to, phase, Instant::now())));
        self.show_switch();
    }

    /// The other version landed, or the episode started again: no switch of it is on its way.
    pub fn version_done(&self) {
        if self.version.take().is_some() {
            self.fill_sources();
            self.show_switch();
        }
    }

    /// The failed pick, for Try again.
    pub fn failed_pick(&self) -> Option<PanelSwitch> {
        self.switch.borrow().clone().filter(|s| s.failed().is_some())
    }

    fn show_switch(&self) {
        let now = Instant::now();
        let switch = self.switch.borrow().clone();
        for choice in self.choices.borrow().values() {
            let picked = switch.as_ref().filter(|s| s.on(&choice.target));
            let failed = picked.and_then(|s| s.failed());
            let spinning = picked.is_some_and(|s| s.spinning(now));
            let check = if spinning {
                "spin"
            } else if choice.on && picked.is_none_or(|s| s.failed().is_some()) {
                "check"
            } else {
                "none"
            };
            if choice.check.visible_child_name().as_deref() != Some(check) {
                choice.check.set_visible_child_name(check);
            }
            choice.spinner.set_spinning(spinning);
            match picked.and_then(|s| s.counting(now)) {
                Some(seconds) => {
                    choice.starting.set_label(&t::player_panel_starting(seconds as i64));
                    choice.line.set_visible(true);
                    choice.line.set_visible_child_name("starting");
                }
                None => {
                    choice.line.set_visible_child_name("detail");
                    choice.line.set_visible(
                        choice.held
                            || choice.line.child_by_name("detail").is_some_and(|d| {
                                d.downcast_ref::<gtk::Label>()
                                    .is_some_and(|l| !l.label().is_empty())
                            }),
                    );
                }
            }
            if let Some(text) = failed {
                choice.alert_line.set_label(&say::msg(text));
            }
            if choice.alert.reveals_child() != failed.is_some() {
                choice.alert.set_reveal_child(failed.is_some());
                if failed.is_some()
                    && let Some(row) =
                        choice.alert.ancestor(gtk::ListBoxRow::static_type()).and_downcast()
                {
                    let scroll = self.src_scroll.clone();
                    glib::timeout_add_local_once(
                        Duration::from_millis(u64::from(REVEAL_MS) + 20),
                        move || bring_into_view(&scroll, &row, true),
                    );
                }
            }
        }
        self.show_version(now);
        self.show_listing(now);
        // A switch under way redraws its row until it lands or fails, and a source still asked
        // its line.
        let pending = self.pending();
        if pending && self.tick.borrow().is_none() {
            let me = self.me.clone();
            let source = glib::timeout_add_local(TICK, move || {
                let Some(panel) = me.upgrade() else { return glib::ControlFlow::Break };
                let pending = panel.pending();
                if !pending {
                    panel.tick.replace(None);
                    panel.show_switch();
                    return glib::ControlFlow::Break;
                }
                panel.show_switch();
                glib::ControlFlow::Continue
            });
            self.tick.replace(Some(source));
        } else if !pending && let Some(source) = self.tick.take() {
            source.remove();
        }
    }

    /// Whether a switch, of source or version, is on its way, or a source is still asked.
    fn pending(&self) -> bool {
        self.switch.borrow().as_ref().is_some_and(|s| s.failed().is_none())
            || self.version.borrow().as_ref().is_some_and(|v| v.failed().is_none())
            || self.listing.borrow().as_ref().is_some_and(Listing::busy)
    }

    /// The Subtitled or Dubbed switch follows the switch to the other version: the picked half
    /// spins from 600 ms, the note counts from 3 s, and a failure says so under it.
    fn show_version(&self, now: Instant) {
        let version = self.version.borrow().clone();
        let row = self.translation.borrow();
        let Some(row) = row.as_ref() else { return };
        for (spinner, half) in row.spinners.iter().zip([Translation::Sub, Translation::Dub]) {
            let spinning = version.as_ref().is_some_and(|v| v.to == half && v.spinning(now));
            spinner.set_spinning(spinning);
            spinner.set_visible(spinning);
        }
        let episode = self.data.borrow().current.clone().unwrap_or_default();
        let note = version.as_ref().and_then(|v| v.note(&episode, now));
        if let Some(note) = &note {
            row.note_line.set_label(note);
        }
        if row.note.reveals_child() != note.is_some() {
            row.note.set_reveal_child(note.is_some());
        }
        let failed = version.as_ref().and_then(|v| v.failed().cloned());
        if let Some(text) = &failed {
            row.alert_line.set_label(&say::msg(text));
            // The switch goes back to what plays.
            let playing = self.data.borrow().translation;
            if row.toggles.active_name().as_deref() != Some(playing.as_str()) {
                row.toggles.set_active_name(Some(playing.as_str()));
            }
        }
        if row.alert.reveals_child() != failed.is_some() {
            row.alert.set_reveal_child(failed.is_some());
        }
    }

    // Episodes

    fn detail(&self, row: &EpisodeRow, current: bool, meta: bool, _data: &Data) -> String {
        let now = now();
        let airs = |at: i64| airs_words(at, now);
        let date = |at: i64| short_date(at);
        state::detail(
            row,
            current,
            meta,
            self.playhead.get(),
            &Clock { now, airs: &airs, date: &date },
        )
    }

    /// The episode rows, of the range shown for a long series; `fresh` for another episode,
    /// which opens on the range playing.
    fn fill_episodes(&self, fresh: bool) {
        let data = self.data.borrow().clone();
        let focus_was = self.focus_number();
        let revealed = self.revealed.borrow().clone();
        self.quiet.set(true);
        self.ep_rows.borrow_mut().clear();
        while let Some(child) = self.ep_list.first_child() {
            self.ep_list.remove(&child);
        }
        let meta = metadata_only(&data.table);
        self.ep_note.set_visible(meta);
        self.ep_loading.set_visible(data.table.is_empty());
        self.ep_list.set_visible(!data.table.is_empty());

        // A long series: the range playing, or the one picked.
        let now = now();
        let ranges =
            if data.table.len() > RANGE { episode_ranges(&data.table, now) } else { vec![] };
        let range_of = |number: &str| {
            let at = data.table.iter().position(|row| row.number == number)? as u32;
            ranges.iter().position(|r| (r.start..r.end).contains(&at))
        };
        let shown = if ranges.len() > 1 {
            let playing = data.current.as_deref().and_then(range_of).unwrap_or(0);
            let shown = if fresh { playing } else { self.range.get().unwrap_or(playing) };
            let items = synopsis::range_items(&data.table, &ranges);
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            self.ep_ranges.set_model(Some(&gtk::StringList::new(&labels)));
            self.ep_ranges.set_selected(shown as u32);
            self.ep_ranges.set_visible(true);
            Some(ranges[shown].start as usize..ranges[shown].end as usize)
        } else {
            self.ep_ranges.set_visible(false);
            None
        };
        let rows = match &shown {
            Some(range) => {
                &data.table[range.start.min(data.table.len())..range.end.min(data.table.len())]
            }
            None => &data.table[..],
        };
        self.revealed.replace(revealed.clone());
        let mut select = None;
        for row in rows {
            let current = !meta && data.current.as_deref() == Some(row.number.as_str());
            let ep = self.episode_row(row, current, meta, &data, now);
            self.ep_list.append(&ep.row);
            if revealed.as_deref() == Some(row.number.as_str()) {
                select = Some(ep.row.clone());
            }
            self.ep_rows.borrow_mut().push(ep);
        }
        if let Some(row) = select {
            self.ep_list.select_row(Some(&row));
        }
        self.quiet.set(false);
        if let Some((number, button)) = focus_was {
            self.refocus(&number, button);
        }
    }

    /// The row, and the button in it, that has the keyboard, for after the list is drawn again.
    fn focus_number(&self) -> Option<(String, Option<EpisodeAction>)> {
        let root = self.ep_list.root()?;
        let focus = gtk::prelude::RootExt::focus(&root)?;
        if !focus.is_ancestor(&self.ep_list)
            && focus != *self.ep_list.upcast_ref::<gtk::Widget>()
        {
            return None;
        }
        self.ep_rows.borrow().iter().find_map(|ep| {
            if focus == *ep.row.upcast_ref::<gtk::Widget>() {
                return Some((ep.number.clone(), None));
            }
            ep.actions
                .iter()
                .find(|(_, b)| focus == *b.upcast_ref::<gtk::Widget>() || focus.is_ancestor(b))
                .map(|(a, _)| (ep.number.clone(), Some(*a)))
                .or_else(|| focus.is_ancestor(&ep.row).then(|| (ep.number.clone(), None)))
        })
    }

    /// The keyboard back on the row numbered, on the button it was on, else its first one
    /// (Mark watched gone, Play next to where it was), else the row.
    fn refocus(&self, number: &str, button: Option<EpisodeAction>) {
        let rows = self.ep_rows.borrow();
        let Some(ep) = rows.iter().find(|r| r.number == number) else { return };
        let target = button.and_then(|wanted| {
            ep.actions
                .iter()
                .find(|(a, _)| *a == wanted)
                .or_else(|| ep.actions.first())
                .map(|(_, b)| b.clone().upcast::<gtk::Widget>())
        });
        let target = target.unwrap_or_else(|| ep.row.clone().upcast());
        drop(rows);
        target.grab_focus();
    }

    fn episode_row(
        &self,
        row: &EpisodeRow,
        current: bool,
        meta: bool,
        data: &Data,
        now: i64,
    ) -> EpRow {
        let hidden = data.protect && synopsis::protected(row, true, false) && !current;
        let not_out = state::not_out(row, now);

        // The still: the episode's own, on a silk plate until it lands, or the plate alone.
        let plate = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        plate.add_css_class("panel-plate");
        plate.set_size_request(96, 54);
        let picture = gtk::Picture::new();
        picture.set_content_fit(gtk::ContentFit::Cover);
        picture.set_size_request(96, 54);
        picture.set_can_shrink(true);
        picture.add_css_class("panel-still");
        let still = gtk::Overlay::new();
        still.set_child(Some(&plate));
        still.add_overlay(&picture);
        still.set_valign(gtk::Align::Center);
        still.set_halign(gtk::Align::Start);
        still.set_overflow(gtk::Overflow::Hidden);
        still.add_css_class("panel-still-frame");
        if !meta && !hidden {
            self.hub.art(&picture, row.thumbnail.as_deref());
        }
        let bar = gtk::ProgressBar::new();
        bar.add_css_class("still-progress");
        bar.set_valign(gtk::Align::End);
        bar.set_margin_start(4);
        bar.set_margin_end(4);
        bar.set_margin_bottom(4);
        bar.set_can_target(false);
        let fraction = state::progress(row, current, meta, self.playhead.get());
        bar.set_fraction(fraction.unwrap_or(0.0));
        bar.set_visible(fraction.is_some_and(|f| f > 0.0));
        still.add_overlay(&bar);
        if row.completed && !current && !meta {
            still.set_opacity(0.62);
        }

        let title = gtk::Label::new(None);
        let name = glib::markup_escape_text(&state::title(row));
        let titled = row.title.as_deref().is_some_and(|t| !t.trim().is_empty());
        if titled {
            title.set_markup(&format!(
                "<span foreground=\"#A3A9C4\" weight=\"500\">{}</span>  {name}", // i18n-skip
                glib::markup_escape_text(&state::number(&row.number))
            ));
        } else {
            title.set_label(&state::title(row));
        }
        title.add_css_class("panel-row-title");
        clamp(&title, 2);
        let detail =
            crate::widgets::haze(&self.detail(row, current, meta, data), "panel-row-detail");
        clamp(&detail, 2);
        let words = gtk::Box::new(gtk::Orientation::Vertical, 2);
        words.set_valign(gtk::Align::Center);
        words.set_hexpand(true);
        words.append(&title);
        words.append(&detail);
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        head.append(&still);
        head.append(&words);

        // What reveals under it: the synopsis, or that protection holds it back, and the buttons.
        let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
        body.add_css_class("panel-reveal");
        let text = synopsis::text_of(row).filter(|_| !not_out);
        if let Some(text) = text {
            if hidden {
                let held = crate::widgets::haze(&t::episode_spoiler_hidden(), "panel-synopsis");
                held.add_css_class("italic");
                body.append(&held);
            } else {
                body.append(&self.synopsis(&row.number, text));
            }
        }
        let actions = state::actions(row, current, meta, not_out);
        let mut buttons = Vec::new();
        if !actions.is_empty() {
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            for action in &actions {
                let button = gtk::Button::with_label(&action.title());
                button.add_css_class("panel-action");
                if action.primary() {
                    button.add_css_class("suggested-action");
                }
                let (me, action, number) = (self.me.clone(), *action, row.number.clone());
                button.connect_clicked(move |_| {
                    if let Some(panel) = me.upgrade() {
                        panel.say(Act::Episode(action, number.clone()));
                    }
                });
                line.append(&button);
                buttons.push((action, button));
            }
            body.append(&line);
        }
        let revealer = gtk::Revealer::new();
        revealer.set_transition_type(gtk::RevealerTransitionType::SlideDown);
        revealer.set_child(Some(&body));
        let has_body = body.first_child().is_some();
        revealer.set_visible(has_body);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.append(&head);
        content.append(&revealer);
        let item = gtk::ListBoxRow::new();
        item.add_css_class("panel-episode");
        if current {
            item.add_css_class("playing");
        }
        item.set_child(Some(&content));
        let spoken = match (&row.title, row.number.parse::<i64>()) {
            (Some(name), Ok(n)) if !name.trim().is_empty() => t::episode_with_title(n, name),
            _ => state::title(row),
        };
        item.update_property(&[
            gtk::accessible::Property::Label(&spoken),
            gtk::accessible::Property::Description(&detail.label()),
        ]);
        if current {
            item.update_state(&[gtk::accessible::State::Selected(Some(true))]);
        }
        EpRow {
            number: row.number.clone(),
            row: item,
            title,
            detail,
            bar,
            revealer,
            body,
            actions: buttons,
        }
    }

    /// A synopsis cut to three lines, with More to read it whole and Less to fold it back.
    fn synopsis(&self, number: &str, text: &str) -> gtk::Box {
        let label = crate::widgets::haze(text, "panel-synopsis");
        label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        label.set_max_width_chars(1);
        label.set_hexpand(true);
        let more = gtk::Button::new();
        more.add_css_class("flat");
        more.add_css_class("panel-more");
        more.set_halign(gtk::Align::Start);
        let whole = self.whole.borrow().contains(number);
        show_whole(&label, &more, whole);
        let (me, number) = (self.me.clone(), number.to_owned());
        more.connect_clicked(move |more| {
            let Some(panel) = me.upgrade() else { return };
            let whole = {
                let mut shown = panel.whole.borrow_mut();
                if shown.remove(&number) {
                    false
                } else {
                    shown.insert(number.clone());
                    true
                }
            };
            let label = more.prev_sibling().and_downcast::<gtk::Label>();
            if let Some(label) = label {
                show_whole(&label, more, whole);
            }
            // The keyboard stays on the button, now Less or More.
            more.grab_focus();
            // Read whole, the row may now reach past the foot of the list.
            if whole
                && let Some(row) = more.ancestor(gtk::ListBoxRow::static_type()).and_downcast()
            {
                let scroll = panel.ep_scroll.clone();
                glib::idle_add_local_once(move || bring_into_view(&scroll, &row, true));
            }
        });
        let place = gtk::Box::new(gtk::Orientation::Vertical, 2);
        place.append(&label);
        place.append(&more);
        place
    }

    /// The row selected changed: it reveals, and the one before folds.
    fn selected(&self, row: Option<&gtk::ListBoxRow>) {
        // The list drawn again lets go of its rows: the row revealed stays so.
        if self.quiet.get() && row.is_none() {
            return;
        }
        let number = row.and_then(|row| {
            self.ep_rows.borrow().iter().find(|r| &r.row == row).map(|r| r.number.clone())
        });
        let animate = !self.quiet.get();
        let before = self.revealed.replace(number.clone());
        for ep in self.ep_rows.borrow().iter() {
            let open = number.as_deref() == Some(ep.number.as_str());
            if open == ep.revealer.reveals_child() {
                continue;
            }
            self.reveal(ep, open, animate);
        }
        if before != number
            && let Some(row) = row
        {
            let (me, row) = (self.me.clone(), row.clone());
            let settle = if animate { REVEAL_SETTLE } else { 30 };
            glib::timeout_add_local_once(Duration::from_millis(settle), move || {
                if let Some(panel) = me.upgrade() {
                    bring_into_view(&panel.ep_scroll, &row, animate);
                }
            });
        }
    }

    fn reveal(&self, ep: &EpRow, open: bool, animate: bool) {
        let reduced = motion::reduced();
        ep.revealer.set_transition_duration(if animate && !reduced { REVEAL_MS } else { 0 });
        ep.revealer.set_reveal_child(open);
        // Revealed, the title and line show whole; folded, they go back to two lines once shut.
        if open {
            clamp(&ep.title, -1);
            clamp(&ep.detail, -1);
            if animate {
                ep.body.set_opacity(0.0);
                let body = ep.body.clone();
                glib::timeout_add_local_once(
                    Duration::from_millis(REVEAL_TEXT_AFTER),
                    move || {
                        let fade = motion::timed(
                            &body,
                            0.0,
                            1.0,
                            REVEAL_MS - 100,
                            motion::opacity(&body),
                        );
                        fade.set_easing(adw::Easing::EaseOutCubic);
                        fade.play();
                    },
                );
            } else {
                ep.body.set_opacity(1.0);
            }
        } else {
            let (title, detail, revealer) =
                (ep.title.clone(), ep.detail.clone(), ep.revealer.clone());
            let shut = move || {
                if !revealer.reveals_child() {
                    clamp(&title, 2);
                    clamp(&detail, 2);
                }
            };
            if animate {
                glib::timeout_add_local_once(Duration::from_millis(u64::from(REVEAL_MS)), shut);
            } else {
                shut();
            }
        }
    }

    /// The playing row near the top as the list opens, the one before it above.
    fn scroll_to_playing(&self) {
        let current = self.data.borrow().current.clone();
        let rows = self.ep_rows.borrow();
        let Some(at) = current.and_then(|n| rows.iter().position(|r| r.number == n)) else {
            return;
        };
        let above = rows[at.saturating_sub(1)].row.clone();
        drop(rows);
        let (scroll, me) = (self.ep_scroll.clone(), self.me.clone());
        let adjustment = self.ep_scroll.vadjustment();
        let place = move || {
            let child = scroll.child()?;
            let bounds = above.compute_bounds(&child)?;
            me.upgrade()?;
            let adjustment = scroll.vadjustment();
            adjustment.set_value(f64::from(bounds.y()));
            Some(adjustment.value())
        };
        // Once laid out, and again once the slide in has settled the rows' heights, unless the
        // list was scrolled in between.
        glib::idle_add_local_once(move || {
            let Some(first) = place() else { return };
            glib::timeout_add_local_once(
                Duration::from_millis(u64::from(OPEN_MS) + 60),
                move || {
                    if adjustment.value() == first {
                        place();
                    }
                },
            );
        });
    }

    /// Select (and so reveal) the row numbered, as a shot or the keyboard does.
    #[cfg(any(test, feature = "fixtures"))]
    pub fn select(&self, number: &str) {
        let row =
            self.ep_rows.borrow().iter().find(|r| r.number == number).map(|r| r.row.clone());
        if let Some(row) = row {
            self.ep_list.select_row(Some(&row));
            row.grab_focus();
        }
    }

    // Sources

    fn fill_sources(&self) {
        let data = self.data.borrow().clone();
        // The keyboard's row, by its key, to put it back on after.
        let focus_key = self.src_focus_key();
        // The keyboard on a Sources row that is not a pick, such as Try again or a fold.
        let in_list = self
            .src_list
            .root()
            .and_then(|r| gtk::prelude::RootExt::focus(&r))
            .is_some_and(|f| f.is_ancestor(&self.src_list));
        self.choices.borrow_mut().clear();
        self.src_does.borrow_mut().clear();
        self.folds.borrow_mut().clear();
        self.listing_rows.borrow_mut().clear();
        self.translation.replace(None);
        while let Some(child) = self.src_list.first_child() {
            self.src_list.remove(&child);
        }
        let mut keys: Vec<(String, gtk::ListBoxRow)> = Vec::new();
        let now = now();

        self.src_list.append(&heading(&t::player_panel_playing_now()));
        let options = data.options.as_deref();
        let playing_option = options.and_then(|all| {
            data.playing_release
                .as_ref()
                .and_then(|r| all.iter().find(|o| o.provider_id == r.provider_id))
                .or_else(|| {
                    data.playing_source.as_ref().and_then(|n| all.iter().find(|o| &o.name == n))
                })
        });
        let playing_provider = data
            .playing_release
            .as_ref()
            .map(|r| r.provider_id.clone())
            .or_else(|| playing_option.map(|o| o.provider_id.clone()));
        let name =
            data.playing_source.clone().or_else(|| playing_option.map(|o| o.name.clone()));
        if let Some(name) = &name {
            let block = gtk::Box::new(gtk::Orientation::Vertical, 1);
            block.add_css_class("panel-playing");
            let title = gtk::Label::new(Some(name));
            title.add_css_class("panel-playing-name");
            wrap(&title);
            block.append(&title);
            if let Some(option) = playing_option {
                let line = crate::widgets::haze(
                    &crate::choices::detail(option, now),
                    "panel-row-detail",
                );
                wrap(&line);
                block.append(&line);
            }
            self.src_list.append(&still_row(&block));
        }
        match &data.playing_release {
            Some(release) => {
                let target = SwitchTarget::release(release);
                let row = self.choice_row(
                    &release.title,
                    Some(&state::release_detail(release)),
                    target.clone(),
                    Look { on: true, sub: true, ..Look::default() },
                );
                row.set_activatable(false);
                keys.push((target.key(), row.clone()));
                self.src_list.append(&row);
            }
            None => {
                for stream in &data.streams {
                    let target =
                        SwitchTarget::stream(&stream.id, name.as_deref().unwrap_or(""));
                    let line = stream.health.as_deref().or(stream.detail.as_deref());
                    let row = self.choice_row(
                        &stream.label,
                        line,
                        target.clone(),
                        Look { on: stream.current, ..Look::default() },
                    );
                    if stream.current {
                        row.set_activatable(false);
                    } else {
                        self.src_does
                            .borrow_mut()
                            .push((row.clone(), RowDoes::Stream(stream.id.clone())));
                    }
                    keys.push((target.key(), row.clone()));
                    self.src_list.append(&row);
                }
            }
        }
        if data.has_dub != Some(false) || data.translation == Translation::Dub {
            self.src_list.append(&self.translation_row(&data));
        }
        let rule = gtk::Separator::new(gtk::Orientation::Horizontal);
        rule.add_css_class("panel-rule");
        self.src_list.append(&still_row(&rule));
        self.src_list.append(&heading(&t::player_panel_other_sources()));

        // While the tab loads, every other source's row from the engine's progress, each
        // changed in place as it answers.
        self.playing.replace((playing_provider.clone(), name.clone()));
        let others = self.listing_others(&data);
        let loading = others.is_some();
        for (provider_id, source, listing) in others.into_iter().flatten() {
            self.listing_row(provider_id, &source, listing, &mut keys);
        }
        if let Some(why) = self.listing.borrow().as_ref().and_then(|l| l.ended.clone()) {
            self.src_list.append(&ended_note(&why));
        }
        // Every other source, in the order set in Sources; before their lines come, the
        // releases' own sources.
        struct Group {
            provider_id: String,
            name: String,
            option: Option<SourceOption>,
        }
        let releases = data.releases.as_deref();
        let groups: Vec<Group> = match options {
            _ if loading => Vec::new(),
            Some(all) => all
                .iter()
                .filter(|o| Some(&o.provider_id) != playing_provider.as_ref())
                .map(|o| Group {
                    provider_id: o.provider_id.clone(),
                    name: o.name.clone(),
                    option: Some(o.clone()),
                })
                .collect(),
            None => {
                let mut seen = HashSet::new();
                releases
                    .unwrap_or_default()
                    .iter()
                    .filter(|c| Some(&c.provider_id) != playing_provider.as_ref())
                    .filter(|c| seen.insert(c.provider_id.clone()))
                    .map(|c| Group {
                        provider_id: c.provider_id.clone(),
                        name: state::source_name(c),
                        option: None,
                    })
                    .collect()
            }
        };
        let known = releases.is_some();
        for group in groups {
            let own: Vec<&SourceCandidate> = releases
                .unwrap_or_default()
                .iter()
                .filter(|c| c.provider_id == group.provider_id)
                .collect();
            // A source resting or failed before that listed releases now is picked from its
            // releases, and its old note no longer holds.
            let resting =
                own.is_empty() && group.option.as_ref().is_some_and(|o| o.note.is_some());
            let line = group
                .option
                .as_ref()
                .filter(|o| o.note.is_none() || own.is_empty())
                .map(|o| crate::choices::detail(o, now));
            if own.len() > 1 && !resting {
                let open = self.expanded.borrow().contains(&group.provider_id);
                let target = SwitchTarget {
                    provider_id: group.provider_id.clone(),
                    name: group.name.clone(),
                    release: None,
                    stream: None,
                };
                let row = self.choice_row(
                    &group.name,
                    line.as_deref(),
                    target.clone(),
                    Look { fold: Some((own.len(), open)), ..Look::default() },
                );
                row.update_state(&[gtk::accessible::State::Expanded(Some(open))]);
                self.src_does
                    .borrow_mut()
                    .push((row.clone(), RowDoes::Fold(group.provider_id.clone())));
                keys.push((target.key(), row.clone()));
                self.src_list.append(&row);
                let chevron = row_chevron(&row);
                let mut rows = Vec::new();
                for (key, inner) in self.release_rows(own, open, None) {
                    keys.push((key, inner.clone()));
                    rows.push(inner);
                }
                if let Some(chevron) = chevron {
                    self.folds.borrow_mut().insert(group.provider_id.clone(), (chevron, rows));
                }
            } else if own.len() == 1 && !resting {
                // One release: the source's row plays it.
                let release = own[0];
                let target = SwitchTarget::release(release);
                let detail = line.clone().unwrap_or_else(|| state::release_detail(release));
                let row = self.choice_row(
                    &group.name,
                    Some(&detail),
                    target.clone(),
                    Look::default(),
                );
                self.src_does
                    .borrow_mut()
                    .push((row.clone(), RowDoes::Release(release.clone())));
                keys.push((target.key(), row.clone()));
                self.src_list.append(&row);
            } else {
                // Resting, or nothing for this episode: said, and nothing to pick.
                let target = SwitchTarget {
                    provider_id: group.provider_id.clone(),
                    name: group.name.clone(),
                    release: None,
                    stream: None,
                };
                let row = self.choice_row(
                    &group.name,
                    line.as_deref(),
                    target,
                    Look {
                        dim: resting || known,
                        waiting: !known && !resting,
                        ..Look::default()
                    },
                );
                row.set_activatable(false);
                row.set_focusable(false);
                self.src_list.append(&row);
            }
        }
        let foot = crate::widgets::haze(
            &format!("{} {}", t::player_panel_order(), t::player_panel_pick_scope()), // i18n-skip
            "panel-foot",
        );
        wrap(&foot);
        self.src_list.append(&still_row(&foot));
        self.show_switch();
        let kept = focus_key
            .and_then(|key| keys.iter().find(|(k, _)| *k == key).map(|(_, row)| row.clone()))
            .is_some_and(|row| row.grab_focus());
        if !kept && (in_list || self.still_parked()) {
            self.focus_first_source();
        }
    }

    /// A source's releases folded under its row, at `at` in the list or at its end, shown when
    /// `open`: their keys and rows.
    fn release_rows<'a>(
        &self,
        releases: impl IntoIterator<Item = &'a SourceCandidate>,
        open: bool,
        at: Option<i32>,
    ) -> Vec<(String, gtk::ListBoxRow)> {
        let mut made = Vec::new();
        for (n, release) in releases.into_iter().enumerate() {
            let target = SwitchTarget::release(release);
            let inner = self.choice_row(
                &release.title,
                Some(&state::release_detail(release)),
                target.clone(),
                Look { sub: true, inset: true, ..Look::default() },
            );
            // The release row folds: a revealer inside a row that hides once shut.
            let content = inner.child();
            inner.set_child(None::<&gtk::Widget>);
            let fold = gtk::Revealer::new();
            fold.set_transition_type(gtk::RevealerTransitionType::SlideDown);
            fold.set_transition_duration(0);
            fold.set_child(content.as_ref());
            fold.set_reveal_child(open);
            inner.set_child(Some(&fold));
            inner.set_visible(open);
            self.src_does.borrow_mut().push((inner.clone(), RowDoes::Release(release.clone())));
            match at {
                Some(at) => self.src_list.insert(&inner, at + n as i32),
                None => self.src_list.append(&inner),
            }
            made.push((target.key(), inner));
        }
        made
    }

    /// The other sources while the tab loads, in the order set in Sources, without the one
    /// playing: each with what the engine last said of it, or, until it has said, the title's
    /// sources with nothing said yet. `None` when the tab is not loading.
    fn listing_others(
        &self,
        data: &Data,
    ) -> Option<Vec<(String, String, Option<ListingState>)>> {
        let listing = self.listing.borrow();
        let listing = listing.as_ref()?;
        let (playing, name) = self.playing.borrow().clone();
        let other = |id: &str, source: &str| {
            playing.as_deref() != Some(id) && name.as_deref() != Some(source)
        };
        Some(match &listing.rows {
            Some(rows) => rows
                .iter()
                .filter(|r| other(&r.provider_id, &r.name))
                .map(|r| (r.provider_id.clone(), r.name.clone(), Some(r.state.clone())))
                .collect(),
            None => data
                .options
                .iter()
                .flatten()
                .filter(|o| other(&o.provider_id, &o.name))
                .map(|o| (o.provider_id.clone(), o.name.clone(), None))
                .collect(),
        })
    }

    /// A source's row while the tab loads, at the list's end: its name and a detail line held
    /// one line high from the first frame, a spinner in the check slot while it is asked, and
    /// the release count and chevron at its end once it lists more than one.
    fn listing_row(
        &self,
        provider_id: String,
        name: &str,
        listing: Option<ListingState>,
        keys: &mut Vec<(String, gtk::ListBoxRow)>,
    ) {
        let releases = state::listed(listing.as_ref()).to_vec();
        let target = match releases.as_slice() {
            [one] => SwitchTarget::release(one),
            _ => SwitchTarget {
                provider_id: provider_id.clone(),
                name: name.to_owned(),
                release: None,
                stream: None,
            },
        };
        let (row, parts) = self.choice_parts(
            name,
            None,
            target.clone(),
            Look { held: true, ..Look::default() },
        );
        let Some(parts) = parts else { return };
        self.src_list.append(&row);
        keys.push((target.key(), row.clone()));
        keys.extend(self.pickable(&row, &provider_id, &releases, &parts.chevron, None));
        self.listing_rows.borrow_mut().push(ListingRow {
            provider_id,
            name: name.to_owned(),
            row,
            parts,
            state: listing,
            shown: None,
        });
    }

    /// A source's row can be picked once it has listed a release: one plays from the row, more
    /// fold under it, at `at` in the list or at its end.
    fn pickable(
        &self,
        row: &gtk::ListBoxRow,
        provider_id: &str,
        releases: &[SourceCandidate],
        chevron: &gtk::Image,
        at: Option<i32>,
    ) -> Vec<(String, gtk::ListBoxRow)> {
        row.set_activatable(!releases.is_empty());
        row.set_focusable(!releases.is_empty());
        match releases {
            [] => Vec::new(),
            [one] => {
                self.src_does.borrow_mut().push((row.clone(), RowDoes::Release(one.clone())));
                Vec::new()
            }
            all => {
                let open = self.expanded.borrow().contains(provider_id);
                if open {
                    chevron.add_css_class("open");
                }
                row.update_state(&[gtk::accessible::State::Expanded(Some(open))]);
                self.src_does
                    .borrow_mut()
                    .push((row.clone(), RowDoes::Fold(provider_id.to_owned())));
                let made = self.release_rows(all, open, at);
                let rows = made.iter().map(|(_, r)| r.clone()).collect();
                self.folds.borrow_mut().insert(provider_id.to_owned(), (chevron.clone(), rows));
                made
            }
        }
    }

    /// The engine said the rows again: each row that changed changes in place. False when the
    /// rows are not the ones drawn, or a row that had listed changed, so the list is drawn again.
    fn refresh_listing(&self) -> bool {
        let data = self.data.borrow().clone();
        let Some(others) = self.listing_others(&data) else { return false };
        let mut rows = self.listing_rows.borrow_mut();
        if rows.len() != others.len()
            || rows.iter().zip(&others).any(|(row, (id, ..))| &row.provider_id != id)
        {
            return false;
        }
        for (row, (_, _, listing)) in rows.iter_mut().zip(others) {
            if row.state == listing {
                continue;
            }
            if !state::listed(row.state.as_ref()).is_empty() {
                return false;
            }
            let releases = state::listed(listing.as_ref()).to_vec();
            if let [one] = releases.as_slice() {
                // The row plays its one release now: a switch to it shows on this row.
                let mut choices = self.choices.borrow_mut();
                let key = format!("source/{}", row.provider_id); // i18n-skip
                if let Some(mut choice) = choices.remove(&key) {
                    choice.target = SwitchTarget::release(one);
                    choices.insert(choice.target.key(), choice);
                }
            }
            let at = row.row.index() + 1;
            self.pickable(&row.row, &row.provider_id, &releases, &row.parts.chevron, Some(at));
            row.state = listing;
        }
        drop(rows);
        if self.still_parked() {
            self.focus_first_source();
        }
        true
    }

    /// Each loading row shows its line at `at`: held empty for 600 ms, then a spinner and
    /// Asking, the seconds from 3 s, then the answer, each change a crossfade in place.
    fn show_listing(&self, at: Instant) {
        let listing = self.listing.borrow();
        let Some(listing) = listing.as_ref() else { return };
        let data = self.data.borrow();
        let episode = data.current.clone().unwrap_or_default();
        let unix = now();
        for row in self.listing_rows.borrow_mut().iter_mut() {
            let said = row.state.as_ref();
            let (line, spinning) = listing.line(&row.provider_id, said, at);
            let words = match (line, said) {
                (ListingLine::Asking, _) => t::player_start_row_asking(),
                (ListingLine::Counting(seconds), _) => t::player_panel_asking(seconds as i64),
                (ListingLine::Answered, Some(said)) => {
                    let coverage = data
                        .options
                        .iter()
                        .flatten()
                        .find(|o| o.provider_id == row.provider_id && o.note.is_none())
                        .map(|o| crate::choices::detail(o, unix));
                    state::listing_words(said, coverage, &episode)
                }
                (ListingLine::Held | ListingLine::Answered | ListingLine::Ended, _) => {
                    String::new()
                }
            };
            let releases = state::listed(said).len();
            let want = Shown {
                line,
                words,
                spinning,
                dim: line == ListingLine::Ended
                    || line == ListingLine::Answered && state::listing_dim(said),
                count: if releases > 1 { releases } else { 0 },
            };
            if row.shown.as_ref() != Some(&want) {
                let was = row.shown.replace(want.clone());
                row.show(was.as_ref(), &want);
            }
        }
        self.src_list.update_state(&[gtk::accessible::State::Busy(listing.busy())]);
    }

    /// The key of the Sources row with the keyboard, when one has it.
    fn src_focus_key(&self) -> Option<String> {
        let root = self.src_list.root()?;
        let focus = gtk::prelude::RootExt::focus(&root)?;
        if !focus.is_ancestor(&self.src_list) {
            return None;
        }
        self.choices.borrow().iter().find_map(|(key, choice)| {
            let row = choice.check.ancestor(gtk::ListBoxRow::static_type())?;
            (focus == row || focus.is_ancestor(&row)).then(|| key.clone())
        })
    }

    /// Fold or unfold a source's releases: they slide down under it over 0.3 s.
    fn fold(&self, provider: &str) {
        let open = {
            let mut expanded = self.expanded.borrow_mut();
            if expanded.remove(provider) {
                false
            } else {
                expanded.insert(provider.to_owned());
                true
            }
        };
        let folds = self.folds.borrow();
        let Some((chevron, rows)) = folds.get(provider) else { return };
        if open {
            chevron.add_css_class("open");
        } else {
            chevron.remove_css_class("open");
        }
        if let Some(source) = chevron.ancestor(gtk::ListBoxRow::static_type()) {
            source.update_state(&[gtk::accessible::State::Expanded(Some(open))]);
        }
        let duration = if motion::reduced() { 0 } else { REVEAL_MS };
        for row in rows {
            let Some(fold) = row.child().and_downcast::<gtk::Revealer>() else { continue };
            fold.set_transition_duration(duration);
            if open {
                row.set_visible(true);
                fold.set_reveal_child(true);
                if let Some(content) = fold.child() {
                    content.set_opacity(0.0);
                    let fade =
                        motion::timed(&content, 0.0, 1.0, 200, motion::opacity(&content));
                    fade.set_easing(adw::Easing::EaseOutCubic);
                    fade.play();
                }
            } else {
                fold.set_reveal_child(false);
                let (row, fold) = (row.clone(), fold.clone());
                glib::timeout_add_local_once(
                    Duration::from_millis(u64::from(duration) + 10),
                    move || {
                        if !fold.reveals_child() {
                            row.set_visible(false);
                        }
                    },
                );
            }
        }
    }

    fn translation_row(&self, data: &Data) -> gtk::ListBoxRow {
        let toggles = adw::ToggleGroup::new();
        toggles.add_css_class("panel-translation");
        toggles.set_homogeneous(true);
        toggles
            .add(adw::Toggle::builder().name("sub").label(t::player_start_subtitled()).build());
        let dub = adw::Toggle::builder().name("dub").label(t::audio_dubbed()).build();
        // The dub does not reach this episode yet.
        let number = data.current.as_deref().and_then(|n| n.parse::<f64>().ok());
        let past = data
            .dub
            .as_ref()
            .is_some_and(|d| !d.complete && number.is_some_and(|n| n > f64::from(d.through)));
        dub.set_enabled(data.translation == Translation::Dub || !past);
        toggles.add(dub);
        // The half picked shows while that version is found.
        let shown = self
            .version
            .borrow()
            .as_ref()
            .filter(|v| v.failed().is_none())
            .map_or(data.translation, |v| v.to);
        toggles.set_active_name(Some(shown.as_str()));
        toggles
            .upcast_ref::<gtk::Widget>()
            .update_property(&[gtk::accessible::Property::Label(&t::player_translation())]);
        let me = self.me.clone();
        toggles.connect_active_name_notify(move |toggles| {
            let Some(panel) = me.upgrade() else { return };
            let picked = if toggles.active_name().as_deref() == Some("dub") {
                Translation::Dub
            } else {
                Translation::Sub
            };
            if picked != panel.data.borrow().translation {
                panel.say(Act::Translation(picked));
            }
        });
        // A spinner at the leading edge of each half, over the switch, so its labels stay put.
        let spinners = [gtk::Spinner::new(), gtk::Spinner::new()];
        let halves = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        halves.set_homogeneous(true);
        halves.set_can_target(false);
        for spinner in &spinners {
            spinner.set_size_request(14, 14);
            spinner.set_halign(gtk::Align::Start);
            spinner.set_valign(gtk::Align::Center);
            spinner.set_margin_start(12);
            spinner.set_visible(false);
            halves.append(spinner);
        }
        let over = gtk::Overlay::new();
        over.set_child(Some(&toggles));
        over.add_overlay(&halves);
        let note_line = crate::widgets::haze("", "panel-row-detail");
        wrap(&note_line);
        let note = gtk::Revealer::new();
        note.set_transition_type(gtk::RevealerTransitionType::SlideDown);
        note.set_transition_duration(if motion::reduced() { 0 } else { REVEAL_MS });
        note.set_child(Some(&note_line));
        let alert_line = gtk::Label::new(None);
        alert_line.add_css_class("panel-alert");
        wrap(&alert_line);
        alert_line.set_accessible_role(gtk::AccessibleRole::Alert);
        let icon = gtk::Image::from_icon_name("dialog-warning-symbolic");
        icon.add_css_class("panel-alert");
        icon.set_valign(gtk::Align::Start);
        icon.set_margin_top(2);
        let said = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        said.append(&icon);
        said.append(&alert_line);
        let alert = gtk::Revealer::new();
        alert.set_transition_type(gtk::RevealerTransitionType::SlideDown);
        alert.set_transition_duration(if motion::reduced() { 0 } else { REVEAL_MS });
        alert.set_child(Some(&said));
        let block = gtk::Box::new(gtk::Orientation::Vertical, 8);
        block.add_css_class("panel-translation-block");
        block.append(&over);
        block.append(&note);
        block.append(&alert);
        if let Some(line) = say::dub_line(data.dub.as_ref()) {
            let note = crate::widgets::haze(&line, "panel-row-detail");
            wrap(&note);
            block.append(&note);
        }
        self.translation.replace(Some(VersionRow {
            toggles,
            spinners,
            note,
            note_line,
            alert,
            alert_line,
        }));
        // A stop for the arrows, as the other rows are; Tab goes on into the switch.
        let row = still_row(&block);
        row.set_focusable(true);
        row
    }

    /// A stream, source or release row: the check on what plays, a spinner on a switch past
    /// 600 ms, "Starting · 3 s" in place of its line from 3 s, and a failure under it with
    /// Try again.
    fn choice_row(
        &self,
        title: &str,
        detail: Option<&str>,
        target: SwitchTarget,
        look: Look,
    ) -> gtk::ListBoxRow {
        self.choice_parts(title, detail, target, look).0
    }

    /// A choice row, and with `look.held` the parts a loading source's row changes in place.
    fn choice_parts(
        &self,
        title: &str,
        detail: Option<&str>,
        target: SwitchTarget,
        look: Look,
    ) -> (gtk::ListBoxRow, Option<ListingParts>) {
        let check = gtk::Stack::new();
        check.set_transition_type(gtk::StackTransitionType::Crossfade);
        check.set_transition_duration(200);
        check.set_size_request(24, -1);
        check.set_valign(gtk::Align::Center);
        check.add_named(&gtk::Box::new(gtk::Orientation::Horizontal, 0), Some("none"));
        let tick = gtk::Image::from_icon_name("object-select-symbolic");
        tick.add_css_class("panel-check");
        check.add_named(&tick, Some("check"));
        let spinner = gtk::Spinner::new();
        spinner.set_size_request(16, 16);
        spinner.set_halign(gtk::Align::Center);
        check.add_named(&spinner, Some("spin"));
        check.set_visible_child_name(if look.on { "check" } else { "none" });
        // A loading source's spinner, over the check slot while it is asked.
        let asking = gtk::Spinner::new();
        asking.set_size_request(16, 16);
        asking.set_halign(gtk::Align::Center);
        asking.set_valign(gtk::Align::Center);
        asking.set_can_target(false);
        asking.set_visible(false);
        let slot = gtk::Overlay::new();
        slot.set_child(Some(&check));
        if look.held {
            slot.add_overlay(&asking);
        }

        let title_label = || {
            let name = gtk::Label::new(Some(title));
            name.add_css_class(if look.sub {
                "panel-choice-sub"
            } else {
                "panel-choice-title"
            });
            if look.held {
                // A loading source's row keeps its height: its name on one line, whole in its
                // tooltip and its spoken label.
                one_line(&name);
                name.set_tooltip_text(Some(title));
            } else {
                wrap(&name);
            }
            name
        };
        let name = title_label();
        if look.dim {
            name.add_css_class("haze");
        }
        // A loading source's name, plain and in haze, the one crossfading to the other.
        let name_swap = look.held.then(|| Swap::new(name.clone(), title_label()));
        let line = gtk::Stack::new();
        line.set_transition_type(gtk::StackTransitionType::Crossfade);
        line.set_transition_duration(200);
        line.set_hhomogeneous(false);
        line.set_vhomogeneous(false);
        line.set_interpolate_size(true);
        let detail_label = crate::widgets::haze(detail.unwrap_or(""), "panel-row-detail");
        wrap(&detail_label);
        // A loading source's line: two labels in one place, held one line high when empty.
        let line_label = || {
            let label = crate::widgets::haze("", "panel-row-detail");
            label.add_css_class("tabular");
            one_line(&label);
            label
        };
        let line_swap = look.held.then(|| Swap::new(line_label(), line_label()));
        match &line_swap {
            Some(swap) => line.add_named(&swap.grid, Some("detail")),
            None => line.add_named(&detail_label, Some("detail")),
        };
        let starting = crate::widgets::haze("", "panel-row-detail");
        starting.add_css_class("tabular");
        line.add_named(&starting, Some("starting"));
        line.set_visible(look.held || detail.is_some_and(|d| !d.is_empty()));
        let words = gtk::Box::new(gtk::Orientation::Vertical, 1);
        words.set_hexpand(true);
        words.set_valign(gtk::Align::Center);
        match &name_swap {
            Some(swap) => words.append(&swap.grid),
            None => words.append(&name),
        }
        words.append(&line);
        if look.waiting {
            let wait = gtk::Spinner::new();
            wait.set_spinning(true);
            wait.set_size_request(12, 12);
            wait.set_halign(gtk::Align::Start);
            wait.set_margin_top(4);
            wait.set_margin_bottom(4);
            wait.add_css_class("haze");
            words.append(&wait);
        }
        let top = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        top.append(&slot);
        top.append(&words);
        let tail = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        tail.set_valign(gtk::Align::Center);
        let (count, open) = look.fold.unwrap_or_default();
        let releases =
            crate::widgets::haze(&t::player_panel_releases(count as i64), "panel-row-detail");
        releases.set_wrap(false);
        releases.set_single_line_mode(true);
        tail.append(&releases);
        let chevron = gtk::Image::from_icon_name("go-next-symbolic");
        chevron.add_css_class("panel-chevron");
        chevron.add_css_class("haze");
        if open {
            chevron.add_css_class("open");
        }
        tail.append(&chevron);
        if look.fold.is_some() || look.held {
            // A loading source's count and chevron fade in once it has listed.
            tail.set_visible(look.fold.is_some());
            top.append(&tail);
        }

        // A failed switch: the engine's line in vermilion, and Try again.
        let alert_line = gtk::Label::new(None);
        alert_line.add_css_class("panel-alert");
        wrap(&alert_line);
        alert_line.set_accessible_role(gtk::AccessibleRole::Alert);
        let icon = gtk::Image::from_icon_name("dialog-warning-symbolic");
        icon.add_css_class("panel-alert");
        icon.set_valign(gtk::Align::Start);
        icon.set_margin_top(2);
        let said = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        said.append(&icon);
        said.append(&alert_line);
        let again = gtk::Button::with_label(&t::action_try_again());
        again.add_css_class("panel-action");
        again.set_halign(gtk::Align::Start);
        let me = self.me.clone();
        again.connect_clicked(move |_| {
            if let Some(panel) = me.upgrade() {
                panel.try_again();
            }
        });
        let alert_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
        alert_box.set_margin_top(6);
        alert_box.set_margin_start(30);
        alert_box.append(&said);
        alert_box.append(&again);
        let alert = gtk::Revealer::new();
        alert.set_transition_type(gtk::RevealerTransitionType::SlideDown);
        alert.set_transition_duration(REVEAL_MS);
        alert.set_child(Some(&alert_box));

        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.set_valign(gtk::Align::Center);
        content.add_css_class("panel-choice");
        if look.inset {
            content.add_css_class("inset");
        }
        content.append(&top);
        content.append(&alert);
        let row = gtk::ListBoxRow::new();
        row.add_css_class("panel-choice-row");
        row.set_child(Some(&content));
        if look.on {
            row.update_state(&[gtk::accessible::State::Checked(gtk::AccessibleTristate::True)]);
        }
        let spoken = match detail.filter(|d| !d.is_empty()) {
            Some(detail) => format!("{title}, {detail}"), // i18n-skip
            None => title.to_owned(),
        };
        row.update_property(&[gtk::accessible::Property::Label(&spoken)]);
        self.choices.borrow_mut().insert(
            target.key(),
            Choice {
                target,
                on: look.on,
                held: look.held,
                check,
                spinner,
                line,
                starting,
                alert,
                alert_line,
            },
        );
        let parts = name_swap.zip(line_swap).map(|(title, line)| ListingParts {
            spinner: asking,
            spin: motion::Fader::always(),
            title,
            line,
            tail,
            count: releases,
            chevron,
            tail_fade: motion::Fader::always(),
        });
        (row, parts)
    }

    /// Try the failed pick again.
    fn try_again(&self) {
        let Some(failed) = self.failed_pick() else { return };
        if let Some(candidate) = failed.candidate {
            self.say(Act::Release(candidate));
        } else if let Some(stream) = failed.target.stream {
            self.say(Act::Stream(stream));
        } else if let Some(release) = failed.target.release {
            let candidate = self.data.borrow().releases.as_ref().and_then(|all| {
                all.iter()
                    .find(|c| c.provider_id == failed.target.provider_id && c.id == release)
                    .cloned()
            });
            if let Some(candidate) = candidate {
                self.say(Act::Release(candidate));
            }
        }
    }

    // For shots and tests

    /// The Sources tab's choice rows' keys, in no order.
    #[cfg(test)]
    pub fn choice_keys(&self) -> Vec<String> {
        self.choices.borrow().keys().cloned().collect()
    }

    /// Unfold a source's releases, as a click on its row does.
    #[cfg(any(test, feature = "fixtures"))]
    pub fn unfold(&self, provider: &str) {
        if !self.expanded.borrow().contains(provider) {
            self.fold(provider);
        }
    }

    /// Click More on the revealed row's synopsis.
    #[cfg(any(test, feature = "fixtures"))]
    pub fn more(&self) -> bool {
        let rows = self.ep_rows.borrow();
        let Some(ep) = rows.iter().find(|r| Some(&r.number) == self.revealed.borrow().as_ref())
        else {
            return false;
        };
        let mut at = ep.body.first_child();
        while let Some(widget) = at {
            if let Some(button) = find_more(&widget) {
                button.emit_clicked();
                return true;
            }
            at = widget.next_sibling();
        }
        false
    }
}

#[cfg(any(test, feature = "fixtures"))]
fn find_more(widget: &gtk::Widget) -> Option<gtk::Button> {
    if let Some(button) = widget.downcast_ref::<gtk::Button>()
        && button.has_css_class("panel-more")
    {
        return Some(button.clone());
    }
    let mut at = widget.first_child();
    while let Some(child) = at {
        if let Some(found) = find_more(&child) {
            return Some(found);
        }
        at = child.next_sibling();
    }
    None
}

/// How a choice row looks.
#[derive(Default)]
struct Look {
    /// What plays: the check.
    on: bool,
    /// A stream or release under its source: the smaller title.
    sub: bool,
    /// Indented under its source by the check's width.
    inset: bool,
    /// Resting, or nothing for this episode.
    dim: bool,
    /// Its releases have not come yet.
    waiting: bool,
    /// A source's row while the tab loads: its line held one line high, its parts changed in
    /// place.
    held: bool,
    /// A source that folds its releases: how many, and whether they show.
    fold: Option<(usize, bool)>,
}

/// Two labels in one place, one shown: a change crossfades from the one shown to the other on
/// the fade-in token, old and new on screen together, and the place is as tall as the taller
/// while both show. The fades play under Reduce Animation too, where a fade is what reduced
/// motion keeps.
struct Swap {
    grid: gtk::Grid,
    labels: [gtk::Label; 2],
    /// The one shown, or coming in.
    front: Cell<usize>,
    fades: RefCell<Vec<adw::TimedAnimation>>,
}

impl Swap {
    fn new(first: gtk::Label, second: gtk::Label) -> Self {
        let grid = gtk::Grid::new();
        grid.set_hexpand(true);
        // Both in the one cell: a grid draws them over each other.
        grid.attach(&first, 0, 0, 1, 1);
        grid.attach(&second, 0, 0, 1, 1);
        second.set_visible(false);
        Self { grid, labels: [first, second], front: Cell::new(0), fades: RefCell::default() }
    }

    /// The label shown, or coming in.
    fn shown(&self) -> &gtk::Label {
        &self.labels[self.front.get()]
    }

    /// `change` made to the other label, which then takes the place: crossfading when `fade`
    /// and the place is on screen, else at once.
    fn to(&self, fade: bool, change: impl FnOnce(&gtk::Label)) {
        for running in self.fades.borrow_mut().drain(..) {
            running.pause();
        }
        let old = self.labels[self.front.get()].clone();
        let new = self.labels[1 - self.front.get()].clone();
        self.front.set(1 - self.front.get());
        new.set_opacity(0.0);
        change(&new);
        new.set_visible(true);
        if !fade || !self.grid.is_mapped() || !old.is_visible() {
            new.set_opacity(1.0);
            old.set_visible(false);
            old.set_opacity(1.0);
            return;
        }
        let fade = |label: &gtk::Label, from: f64, to: f64| {
            let animation = motion::timed(label, from, to, FADE_MS, motion::opacity(label));
            animation.set_easing(adw::Easing::EaseOutCubic);
            animation.set_follow_enable_animations_setting(false);
            animation
        };
        let going = fade(&old, old.opacity(), 0.0);
        going.connect_done(move |_| {
            old.set_visible(false);
            old.set_opacity(1.0);
        });
        let coming = fade(&new, 0.0, 1.0);
        going.play();
        coming.play();
        self.fades.replace(vec![going, coming]);
    }
}

/// The parts of a source's row that change while the tab loads.
struct ListingParts {
    /// In the check slot while the source is asked.
    spinner: gtk::Spinner,
    spin: motion::Fader,
    /// The name, plain or in haze once it answered with nothing to pick.
    title: Swap,
    line: Swap,
    /// The release count and chevron, once it lists more than one.
    tail: gtk::Box,
    count: gtk::Label,
    chevron: gtk::Image,
    tail_fade: motion::Fader,
}

/// What a loading source's row shows.
#[derive(Debug, Clone, PartialEq)]
struct Shown {
    line: ListingLine,
    words: String,
    spinning: bool,
    dim: bool,
    /// Its releases when it folds them, else 0.
    count: usize,
}

/// A source's row while the Sources tab loads: drawn from the first frame, in the order set in
/// Sources, and changed in place as the source answers, so no row moves.
struct ListingRow {
    provider_id: String,
    name: String,
    row: gtk::ListBoxRow,
    parts: ListingParts,
    /// What the engine last said of it; `None` before it has said.
    state: Option<ListingState>,
    /// What it shows; `None` until first drawn, which is at once.
    shown: Option<Shown>,
}

impl ListingRow {
    /// Show `want`, from `was`: each change one crossfade in place, the spinner fading out as
    /// the answer fades in. The first drawing is at once.
    fn show(&self, was: Option<&Shown>, want: &Shown) {
        let parts = &self.parts;
        let fade = was.is_some();
        let kind = |s: &Shown| match s.line {
            ListingLine::Counting(_) => ListingLine::Counting(0),
            line => line,
        };
        let same_line = was.is_some_and(|was| {
            kind(was) == kind(want)
                && (want.line != ListingLine::Answered || was.words == want.words)
        });
        if !same_line {
            parts.line.to(fade, |label| label.set_label(&want.words));
        } else if parts.line.shown().label() != want.words {
            // The seconds count on in place.
            parts.line.shown().set_label(&want.words);
        }
        match was {
            Some(was) if was.spinning != want.spinning => {
                if want.spinning {
                    parts.spinner.set_spinning(true);
                }
                parts.spin.set(&parts.spinner, want.spinning);
            }
            Some(_) => {}
            None => {
                parts.spinner.set_spinning(want.spinning);
                parts.spinner.set_visible(want.spinning);
            }
        }
        if was.is_none_or(|was| was.dim != want.dim) {
            parts.title.to(fade, |label| {
                if want.dim {
                    label.add_css_class("haze");
                } else {
                    label.remove_css_class("haze");
                }
            });
        }
        if want.count > 0 {
            parts.count.set_label(&t::player_panel_releases(want.count as i64));
        }
        let listed = want.count > 0;
        match was {
            Some(was) if (was.count > 0) != listed => parts.tail_fade.set(&parts.tail, listed),
            Some(_) => {}
            None => parts.tail.set_visible(listed),
        }
        let spoken = if want.words.is_empty() {
            self.name.clone()
        } else {
            format!("{}, {}", self.name, want.words) // i18n-skip
        };
        self.row.update_property(&[gtk::accessible::Property::Label(&spoken)]);
    }
}

/// Whether Sources draws the same from `a` as from `b`.
fn same_sources(a: &Data, b: &Data) -> bool {
    a.current == b.current
        && a.streams == b.streams
        && a.translation == b.translation
        && a.options == b.options
        && a.releases == b.releases
        && a.playing_source == b.playing_source
        && a.playing_release == b.playing_release
        && a.has_dub == b.has_dub
        && a.dub == b.dub
}

/// The chevron of a folding source's row.
fn row_chevron(row: &gtk::ListBoxRow) -> Option<gtk::Image> {
    fn find(widget: &gtk::Widget) -> Option<gtk::Image> {
        if let Some(image) = widget.downcast_ref::<gtk::Image>()
            && image.has_css_class("panel-chevron")
        {
            return Some(image.clone());
        }
        let mut at = widget.first_child();
        while let Some(child) = at {
            if let Some(found) = find(&child) {
                return Some(found);
            }
            at = child.next_sibling();
        }
        None
    }
    find(row.upcast_ref())
}

/// Whether the table comes from the tracker alone, with nothing to play.
fn metadata_only(table: &[EpisodeRow]) -> bool {
    table.iter().any(|r| r.metadata_only)
}

/// A section heading in the Sources list: not a row the keyboard stops on.
fn heading(text: &str) -> gtk::ListBoxRow {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("panel-heading");
    label.set_xalign(0.0);
    label.set_accessible_role(gtk::AccessibleRole::Heading);
    still_row(&label)
}

/// Why the listing ended before any source was asked, under the sources' rows.
fn ended_note(why: &str) -> gtk::ListBoxRow {
    let note = crate::widgets::haze(why, "panel-row-detail");
    wrap(&note);
    still_row(&note)
}

/// A row that only says something, or holds controls of its own.
fn still_row(child: &impl IsA<gtk::Widget>) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::builder()
        .activatable(false)
        .selectable(false)
        .focusable(false)
        .build();
    row.add_css_class("panel-still-row");
    row.set_child(Some(child));
    row
}

/// A label that wraps inside the panel, never widening it.
fn wrap(label: &gtk::Label) {
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    label.set_max_width_chars(1);
    label.set_hexpand(true);
}

/// A label held to one line, cut at its end when the panel is too narrow for it.
fn one_line(label: &gtk::Label) {
    label.set_xalign(0.0);
    label.set_wrap(false);
    label.set_single_line_mode(true);
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    label.set_hexpand(true);
}

/// A label cut to `lines` with an ellipsis, or whole with -1.
fn clamp(label: &gtk::Label, lines: i32) {
    wrap(label);
    label.set_lines(lines);
    label.set_ellipsize(if lines > 0 {
        gtk::pango::EllipsizeMode::End
    } else {
        gtk::pango::EllipsizeMode::None
    });
}

/// The synopsis whole with Less under it, or cut to three lines with More under it while it is
/// cut, read once it is laid out.
fn show_whole(label: &gtk::Label, more: &gtk::Button, whole: bool) {
    if whole {
        label.set_lines(-1);
        label.set_ellipsize(gtk::pango::EllipsizeMode::None);
        more.set_label(&t::player_panel_less());
        more.set_visible(true);
        return;
    }
    label.set_lines(SYNOPSIS_LINES);
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    more.set_label(&t::player_panel_more());
    let more = more.downgrade();
    label.add_tick_callback(move |label, _| {
        if label.width() == 0 {
            return glib::ControlFlow::Continue;
        }
        if let Some(more) = more.upgrade() {
            more.set_visible(label.layout().is_ellipsized());
        }
        glib::ControlFlow::Break
    });
}

/// Scroll so the whole of `row` shows, its buttons included, or its top when it is taller
/// than the list.
fn bring_into_view(scroll: &gtk::ScrolledWindow, row: &gtk::ListBoxRow, animate: bool) {
    let Some(child) = scroll.child() else { return };
    let Some(bounds) = row.compute_bounds(&child) else { return };
    let adjustment = scroll.vadjustment();
    let (top, bottom) = (f64::from(bounds.y()), f64::from(bounds.y() + bounds.height()));
    let (value, page) = (adjustment.value(), adjustment.page_size());
    let to = if bottom > value + page {
        (bottom - page).min(top)
    } else if top < value {
        top
    } else {
        return;
    };
    if !animate || motion::reduced() {
        adjustment.set_value(to);
        return;
    }
    let to_value = adw::CallbackAnimationTarget::new(move |v| adjustment.set_value(v));
    let scroll_to = motion::timed(scroll, value, to, 250, to_value);
    scroll_to.set_easing(adw::Easing::EaseOutCubic);
    scroll_to.play();
}

fn scroller(child: &impl IsA<gtk::Widget>) -> gtk::ScrolledWindow {
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroll.set_child(Some(child));
    scroll.set_vexpand(true);
    scroll
}

/// Seven rows the shape of the ones to come, and after 4 s the line that they are coming.
fn loading() -> gtk::Box {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    column.add_css_class("panel-loading");
    column.update_property(&[gtk::accessible::Property::Label(
        &t::player_sheet_episodes_loading(),
    )]);
    for width in [0.72, 0.58, 0.8, 0.64, 0.7, 0.52, 0.76] {
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        line.add_css_class("panel-loading-row");
        let still = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        still.add_css_class("panel-plate");
        still.set_size_request(96, 54);
        let bars = gtk::Box::new(gtk::Orientation::Vertical, 8);
        bars.set_valign(gtk::Align::Center);
        let words_width = WIDTH - 24 - 16 - 96 - 12;
        for (fraction, height) in [(width, 14), (0.34, 11)] {
            let bar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            bar.add_css_class("panel-loading-bar");
            bar.set_size_request((f64::from(words_width) * fraction) as i32, height);
            bar.set_halign(gtk::Align::Start);
            bars.append(&bar);
        }
        line.append(&still);
        line.append(&bars);
        column.append(&line);
    }
    let late = crate::widgets::haze(&t::player_sheet_episodes_loading(), "panel-row-detail");
    late.set_margin_start(8);
    late.set_margin_top(8);
    late.set_opacity(0.0);
    column.append(&late);
    column.connect_map(move |_| {
        late.set_opacity(0.0);
        let late = late.clone();
        glib::timeout_add_local_once(Duration::from_secs(4), move || {
            if late.is_mapped() {
                motion::fade_in(&late);
            }
        });
    });
    column
}

fn now() -> i64 {
    glib::DateTime::now_utc().map_or(0, |t| t.to_unix())
}

/// A day as the rows write it: "7 Nov 2026".
fn short_date(at: i64) -> String {
    glib::DateTime::from_unix_local(at)
        .ok()
        .and_then(|d| d.format("%-d %b %Y").ok()) // i18n-skip
        .map_or_else(String::new, |d| d.to_string())
}

/// When an episode airs: "today at 16:30", "tomorrow at 16:30", "on Thursday at 16:30" within
/// six days, and "on 29 Nov at 16:30" later, as the title page says it.
fn airs_words(at: i64, now: i64) -> String {
    let (Ok(day), Ok(today)) =
        (glib::DateTime::from_unix_local(at), glib::DateTime::from_unix_local(now))
    else {
        return String::new();
    };
    let time = crate::pages::calendar::time(at);
    let year_days = glib::DateTime::from_local(today.year(), 12, 31, 12, 0, 0.0)
        .map_or(365, |d| d.day_of_year());
    let days = state::days_between(
        (today.year(), today.day_of_year()),
        (day.year(), day.day_of_year()),
        year_days,
    );
    let format = |pattern: &str| day.format(pattern).map(|s| s.to_string()).unwrap_or_default();
    match days {
        0 => t::cause_airs_today(&time),
        1 => t::cause_airs_tomorrow(&time),
        2..=6 => t::cause_airs_on_date(&format("%A"), &time), // i18n-skip
        _ => t::cause_airs_on_date(&format("%-d %b"), &time), // i18n-skip
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gtk_test::run;

    fn pump(ms: u64) {
        let until = Instant::now() + Duration::from_millis(ms);
        let context = glib::MainContext::default();
        while Instant::now() < until {
            context.iteration(false);
        }
    }

    fn table() -> Vec<EpisodeRow> {
        (1..=9)
            .map(|n| EpisodeRow {
                number: n.to_string(),
                title: Some(format!("Lamp number {n}")),
                duration_secs: Some(1420),
                description: Some(
                    "The lamp seller refuses a sale for the first time. Mina wants to know why, \
                     and the answer takes them out past the last house on the road, where the \
                     lamps have never been lit and the road goes on in the dark for a long way."
                        .into(),
                ),
                completed: n < 7,
                watched: if n < 7 { 1.0 } else { 0.0 },
                ..EpisodeRow::default()
            })
            .collect()
    }

    fn release(id: &str, provider: &str) -> SourceCandidate {
        SourceCandidate {
            id: id.into(),
            provider_id: provider.into(),
            provider_name: format!("{provider} name"),
            title: format!("The Lamp Road 07 {id}"),
            quality: Some(1080),
            label: None,
            size: Some("1.4 GiB".into()),
            dual_audio: false,
            dubbed: false,
            auto_pick: id == "r1",
        }
    }

    fn panel_in_window() -> (Rc<SidePanel>, gtk::Window, Rc<RefCell<Vec<Act>>>) {
        let hub = Hub::new(std::sync::Weak::new());
        let panel = SidePanel::new(&hub);
        let acts: Rc<RefCell<Vec<Act>>> = Rc::default();
        panel.connect_act({
            let acts = Rc::clone(&acts);
            move |act| acts.borrow_mut().push(act)
        });
        let window = gtk::Window::builder().default_width(1280).default_height(800).build();
        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&gtk::Box::new(gtk::Orientation::Vertical, 0)));
        overlay.add_overlay(&panel.widget);
        window.set_child(Some(&overlay));
        window.present();
        panel.set_data(Data {
            table: table(),
            current: Some("7".into()),
            streams: vec![
                MenuSource {
                    id: "s2".into(),
                    label: "1080p · server 2".into(),
                    detail: None,
                    current: true,
                    health: None,
                },
                MenuSource {
                    id: "s1".into(),
                    label: "1080p · server 1".into(),
                    detail: None,
                    current: false,
                    health: Some("Failed the last 3 times".into()),
                },
            ],
            playing_source: Some("Example source".into()),
            has_dub: Some(true),
            ..Data::default()
        });
        panel.set_playhead(Playhead { position: 724.0, duration: Some(1420.0) });
        (panel, window, acts)
    }

    #[test]
    fn a_click_reveals_and_return_plays_the_amber_action() {
        run(|| {
            let (panel, window, acts) = panel_in_window();
            panel.open(PanelTab::Episodes);
            pump(400);
            panel.select("8");
            pump(500);
            let rows = panel.ep_rows.borrow();
            let eight = rows.iter().find(|r| r.number == "8").expect("row 8");
            assert!(eight.revealer.reveals_child());
            assert_eq!(
                eight.actions.iter().map(|(a, _)| *a).collect::<Vec<_>>(),
                vec![EpisodeAction::Play, EpisodeAction::MarkWatched]
            );
            assert!(
                !rows.iter().find(|r| r.number == "7").expect("row 7").revealer.reveals_child()
            );
            let row = eight.row.clone();
            drop(rows);
            row.emit_activate();
            assert_eq!(
                acts.borrow().last(),
                Some(&Act::Episode(EpisodeAction::Play, "8".into()))
            );
            window.destroy();
        });
    }

    #[test]
    fn more_shows_a_cut_synopsis_whole_and_less_folds_it() {
        run(|| {
            let (panel, window, _) = panel_in_window();
            panel.open(PanelTab::Episodes);
            pump(400);
            panel.select("8");
            pump(600);
            assert!(panel.more(), "More was there to click");
            pump(100);
            assert!(panel.whole.borrow().contains("8"));
            assert!(panel.more(), "Less was there to click");
            assert!(!panel.whole.borrow().contains("8"));
            window.destroy();
        });
    }

    #[test]
    fn a_source_with_several_releases_folds_them_and_a_pick_spins_then_fails() {
        run(|| {
            let (panel, window, acts) = panel_in_window();
            panel.open(PanelTab::Sources);
            panel.set_releases(Some(vec![
                release("r1", "index"),
                release("r2", "index"),
                release("h1", "home"),
            ]));
            pump(400);
            let keys = panel.choice_keys();
            assert!(keys.contains(&"source/index".to_owned()), "{keys:?}");
            assert!(keys.contains(&"release/index/r2".to_owned()));
            // One release: the source's own row plays it.
            assert!(keys.contains(&"release/home/h1".to_owned()));
            panel.unfold("index");
            pump(400);
            let candidate = release("r2", "index");
            let target = SwitchTarget::release(&candidate);
            panel.asked(target.clone(), Some(candidate.clone()));
            {
                let choices = panel.choices.borrow();
                let choice = &choices["release/index/r2"];
                assert_eq!(choice.check.visible_child_name().as_deref(), Some("none"));
            }
            pump(800);
            {
                let choices = panel.choices.borrow();
                let choice = &choices["release/index/r2"];
                assert_eq!(choice.check.visible_child_name().as_deref(), Some("spin"));
            }
            panel.switching(
                target,
                Phase::Failed(kaeri_model::Msg::PlayerPanelSwitchFailed {
                    source: "index name".into(),
                    current: "Example source".into(),
                }),
            );
            pump(400);
            {
                let choices = panel.choices.borrow();
                let choice = &choices["release/index/r2"];
                assert!(choice.alert.reveals_child());
                assert_eq!(
                    choice.alert_line.label(),
                    "Couldn't start from index name. Still playing from Example source."
                );
            }
            panel.try_again();
            assert_eq!(acts.borrow().last(), Some(&Act::Release(candidate)));
            window.destroy();
        });
    }

    fn listing(id: &str, name: &str, state: ListingState) -> SourceListing {
        SourceListing { provider_id: id.into(), name: name.into(), state }
    }

    /// The other sources as the engine says them, the one playing first.
    fn said(
        home: ListingState,
        index: ListingState,
        archive: ListingState,
    ) -> Vec<SourceListing> {
        use kaeri_model::starting::ListingMiss;
        vec![
            listing(
                "example",
                "Example source",
                ListingState::Missed { miss: ListingMiss::NoEpisode },
            ),
            listing("home", "Home server", home),
            listing("index", "Example index", index),
            listing("archive", "Archive example", archive),
            listing("old", "Old example", ListingState::Off),
        ]
    }

    /// Each loading row: its source, what its line shows, whether its spinner shows, whether
    /// its name is in haze, and its height.
    fn loading_rows(panel: &SidePanel) -> Vec<(String, String, bool, bool, i32)> {
        panel
            .listing_rows
            .borrow()
            .iter()
            .map(|r| {
                (
                    r.provider_id.clone(),
                    r.parts.line.shown().label().to_string(),
                    r.parts.spinner.is_visible() && r.parts.spinner.opacity() > 0.99,
                    r.parts.title.shown().has_css_class("haze"),
                    r.row.height(),
                )
            })
            .collect()
    }

    fn age(panel: &SidePanel, by: Duration) {
        panel.listing.borrow_mut().as_mut().expect("loading").age(by);
        panel.show_switch();
    }

    #[test]
    fn each_source_answers_in_its_own_row_which_never_moves() {
        use kaeri_model::starting::ListingMiss;
        run(|| {
            let (panel, window, acts) = panel_in_window();
            panel.open(PanelTab::Sources);
            panel.listing_asked();
            let asking = || ListingState::Asking;
            panel.set_progress(said(asking(), asking(), asking()));
            pump(400);
            let first = loading_rows(&panel);
            let ids: Vec<&str> = first.iter().map(|r| r.0.as_str()).collect();
            assert_eq!(
                ids,
                ["home", "index", "archive", "old"],
                "in order, the one playing apart"
            );
            // Silent for 600 ms: no spinner and an empty line; switched off says so at once.
            assert_eq!(first[0].1, "");
            assert!(!first[0].2 && !first[0].3);
            assert_eq!(first[3].1, "Switched off");
            assert!(!first[3].2 && first[3].3, "off: no spinner, its name in haze");
            let heights: Vec<i32> = first.iter().map(|r| r.4).collect();
            assert!(heights.iter().all(|h| *h > 0), "{heights:?}");

            // From 600 ms a spinner and Asking, in rows of the same height.
            age(&panel, Duration::from_millis(700));
            pump(400);
            let now = loading_rows(&panel);
            assert_eq!(now[0].1, "Asking");
            assert!(now[0].2, "the spinner shows");
            assert!(!now[3].2, "never one for a source switched off");
            assert_eq!(now.iter().map(|r| r.4).collect::<Vec<_>>(), heights);
            let home = panel.listing_rows.borrow()[0].row.clone();
            assert!(!home.is_activatable() && !home.is_focusable(), "nothing to pick yet");

            // Home server lists one release and Example index three: the one plays from its
            // row, the three fold under theirs, its count and chevron at the row's end.
            let index =
                vec![release("r1", "index"), release("r2", "index"), release("r3", "index")];
            panel.set_progress(said(
                ListingState::Listed { releases: vec![release("h1", "home")] },
                ListingState::Listed { releases: index },
                asking(),
            ));
            pump(400);
            let now = loading_rows(&panel);
            assert_eq!(now[0].1, "1080p · 1.4 GiB", "the one release's own detail");
            assert!(!now[0].2, "the spinner went as the answer came");
            assert_eq!(now.iter().map(|r| r.4).collect::<Vec<_>>(), heights, "no row moved");
            assert!(home.is_activatable() && home.is_focusable());
            {
                let rows = panel.listing_rows.borrow();
                let index = &rows[1];
                assert!(index.parts.tail.is_visible());
                assert_eq!(index.parts.count.label(), "3 releases");
            }
            let keys = panel.choice_keys();
            assert!(keys.contains(&"release/home/h1".to_owned()), "{keys:?}");
            assert!(keys.contains(&"release/index/r2".to_owned()), "{keys:?}");
            let shown = panel
                .folds
                .borrow()
                .get("index")
                .map(|(_, rows)| rows.iter().any(|r| r.is_visible()));
            assert_eq!(shown, Some(false), "releases never open by themselves");
            home.emit_activate();
            assert_eq!(acts.borrow().last(), Some(&Act::Release(release("h1", "home"))));

            // From 3 s the one still asked counts its seconds, then it fails.
            age(&panel, Duration::from_millis(2500));
            pump(300);
            let counting = loading_rows(&panel)[2].1.clone();
            assert!(
                counting.starts_with("Asking · ") && counting.ends_with(" s"),
                "{counting}"
            );
            panel.set_progress(said(
                ListingState::Listed { releases: vec![release("h1", "home")] },
                ListingState::Listed {
                    releases: vec![
                        release("r1", "index"),
                        release("r2", "index"),
                        release("r3", "index"),
                    ],
                },
                ListingState::Missed { miss: ListingMiss::Failed },
            ));
            pump(400);
            let now = loading_rows(&panel);
            assert_eq!(now[2].1, "Didn't answer");
            assert!(!now[2].2 && now[2].3, "no spinner, its name in haze");
            assert_eq!(now.iter().map(|r| r.4).collect::<Vec<_>>(), heights);
            assert!(!panel.pending(), "nothing is asked now");

            // The final answer leaves the rows as they are drawn.
            let drawn = panel.listing_rows.borrow()[0].row.clone();
            panel.set_releases(Some(vec![release("h1", "home"), release("r1", "index")]));
            assert_eq!(panel.listing_rows.borrow()[0].row, drawn);
            window.destroy();
        });
    }

    /// Every label's text in Sources, top to bottom, the hidden ones too.
    fn src_texts(panel: &SidePanel) -> Vec<String> {
        fn walk(w: &gtk::Widget, out: &mut Vec<String>) {
            if let Some(label) = w.downcast_ref::<gtk::Label>() {
                out.push(label.label().to_string());
            }
            let mut child = w.first_child();
            while let Some(c) = child {
                walk(&c, out);
                child = c.next_sibling();
            }
        }
        let mut out = Vec::new();
        walk(panel.src_list.upcast_ref(), &mut out);
        out
    }

    #[test]
    fn a_listing_the_engine_ends_stops_asking_and_says_why() {
        run(|| {
            let (panel, window, _) = panel_in_window();
            panel.open(PanelTab::Sources);
            panel.listing_asked();
            let asking = || ListingState::Asking;
            panel.set_progress(said(asking(), asking(), asking()));
            pump(300);
            let heights: Vec<i32> = loading_rows(&panel).iter().map(|r| r.4).collect();
            age(&panel, Duration::from_millis(3500));
            pump(300);
            assert!(loading_rows(&panel)[0].1.starts_with("Asking · "));

            let why = "Episode 7 hasn't aired yet";
            panel.sources_failed(why.to_owned());
            pump(400);
            let now = loading_rows(&panel);
            assert_eq!(now.iter().map(|r| r.4).collect::<Vec<_>>(), heights, "no row moved");
            for row in &now[..3] {
                assert_eq!(row.1, "", "{row:?} no longer asks");
                assert!(!row.2 && row.3, "{row:?}: no spinner, its name in haze");
            }
            assert_eq!(now[3].1, "Switched off");
            assert!(!panel.pending(), "nothing counts on");
            let texts = src_texts(&panel);
            assert_eq!(texts.iter().filter(|t| *t == why).count(), 1, "{texts:?}");

            // Drawn again, the rows and the note stay as they are.
            panel.set_options(None);
            pump(100);
            assert_eq!(src_texts(&panel).iter().filter(|t| *t == why).count(), 1);
            assert!(!panel.pending());
            window.destroy();
        });
    }

    #[test]
    fn a_source_resting_before_that_lists_releases_now_can_be_picked() {
        use kaeri_model::sources::{SourceNote, SourceOption};
        run(|| {
            let (panel, window, acts) = panel_in_window();
            panel.open(PanelTab::Sources);
            let option = |id: &str, name: &str, note| SourceOption {
                provider_id: id.into(),
                name: name.into(),
                covered: 12,
                of: 12,
                pinned: false,
                note,
                releases: Vec::new(),
                pinned_release: None,
                first: Some(1),
                last: Some(12),
                lacking: Vec::new(),
                lacking_since: None,
            };
            panel.set_options(Some(vec![
                option("home", "Home server", Some(SourceNote::Resting)),
                option("archive", "Archive example", Some(SourceNote::Resting)),
            ]));
            // The final answer alone, without progress: Home server listed a release.
            panel.set_releases(Some(vec![release("h1", "home")]));
            pump(300);
            let texts = src_texts(&panel);
            let resting = kaeri_i18n::t::source_note_resting();
            assert_eq!(
                texts.iter().filter(|t| **t == resting).count(),
                1,
                "only Archive example, which listed nothing, rests: {texts:?}"
            );
            assert!(texts.iter().any(|t| t == "1080p · 1.4 GiB"), "{texts:?}");
            let keys = panel.choice_keys();
            assert!(keys.contains(&"release/home/h1".to_owned()), "{keys:?}");
            let row = panel
                .src_does
                .borrow()
                .iter()
                .find(|(_, does)| matches!(does, RowDoes::Release(r) if r.id == "h1"))
                .map(|(row, _)| row.clone())
                .expect("Home server's row plays its release");
            assert!(row.is_activatable());
            row.emit_activate();
            assert_eq!(acts.borrow().last(), Some(&Act::Release(release("h1", "home"))));
            window.destroy();
        });
    }

    #[test]
    fn mark_watched_keeps_the_keyboard_on_the_row() {
        run(|| {
            let (panel, window, _) = panel_in_window();
            panel.open(PanelTab::Episodes);
            pump(400);
            panel.select("8");
            pump(400);
            let button =
                panel.ep_rows.borrow().iter().find(|r| r.number == "8").expect("row 8").actions
                    [1]
                .1
                .clone();
            button.grab_focus();
            // The engine answers Mark watched with the table again, 8 now watched.
            let mut data = panel.data().clone();
            data.table[7].completed = true;
            data.table[7].watched = 1.0;
            panel.set_data(data);
            pump(100);
            let rows = panel.ep_rows.borrow();
            let eight = rows.iter().find(|r| r.number == "8").expect("row 8");
            assert!(eight.revealer.reveals_child(), "the row stays revealed");
            assert_eq!(
                eight.actions.iter().map(|(a, _)| *a).collect::<Vec<_>>(),
                vec![EpisodeAction::Play]
            );
            assert!(eight.actions[0].1.has_focus(), "the keyboard went to Play");
            drop(rows);
            window.destroy();
        });
    }
}
