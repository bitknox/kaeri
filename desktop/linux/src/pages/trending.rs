//! Search's Trending rail: the heading is a menu of periods (today, this week, this month),
//! the rail is a row of covers, and See all browses the period shown.
//!
//! Today is the browse's own Trending now rail. A week or a month comes from the engine's
//! `LoadTrending`, which remembers the choice on this device. Nothing here asks at launch: the
//! rail asks when Search is on screen. A change of period keeps the old covers for 600 ms, then
//! shows placeholders for at least 300 ms, then the new covers; each change crossfades over a
//! picture of what was there, so the rail is never empty between.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use adw::prelude::*;
use gtk::{gio, glib};
use kaeri_i18n::t;
use kaeri_model::{
    browsing::TrendPeriod, catalog::Entry, command::Command, event::Event, search::SearchSort,
};

use crate::{
    states::{Shape, States},
    widgets::PosterGrid,
    window::Ctx,
};

/// How many covers the rail shows, as This season's grid does.
const SHOWN: usize = 12;

/// What See all does, given the period shown.
type SeeAll = Box<dyn Fn(TrendPeriod)>;

#[derive(Clone)]
pub struct Rail {
    pub widget: gtk::Box,
    inner: Rc<Inner>,
}

struct Inner {
    me: std::rc::Weak<Inner>,
    ctx: Rc<Ctx>,
    period: Cell<TrendPeriod>,
    /// The viewer picked a period this visit: a remembered one arriving later does not take over.
    chosen: Cell<bool>,
    kept: RefCell<HashMap<&'static str, Vec<Entry>>>,
    heading: gtk::Stack,
    grid: PosterGrid,
    states: Rc<States>,
    actions: gio::SimpleActionGroup,
    see_all: RefCell<Option<SeeAll>>,
    asked: Cell<Option<std::time::Instant>>,
    /// The rail under its ghost: a picture of before, fading away over the change.
    veil: gtk::Overlay,
    /// A period asked for and not yet in: the old covers stay until this fires.
    switching: RefCell<Option<glib::SourceId>>,
    /// The answer waiting out the placeholders' least time.
    landing: RefCell<Option<glib::SourceId>>,
}

fn key(period: TrendPeriod) -> &'static str {
    match period {
        TrendPeriod::Today => "today",
        TrendPeriod::Week => "week",
        TrendPeriod::Month => "month",
    }
}

fn from_key(key: &str) -> TrendPeriod {
    TrendPeriod::ALL.into_iter().find(|p| self::key(*p) == key).unwrap_or(TrendPeriod::Today)
}

impl Rail {
    pub fn new(ctx: &Rc<Ctx>) -> Self {
        let grid = PosterGrid::new(&ctx.hub);
        grid.widget.set_vexpand(false);
        let empty = crate::states::plain(&t::search_linux_nothing_found(), "");
        let states = States::new(
            &ctx.hub,
            &grid.widget,
            Shape::Grid,
            &empty,
            &t::trending_failed(&TrendPeriod::Today.choice().to_lowercase()),
        );
        states.widget.set_vexpand(false);

        // The heading: the period's name in a flat menu button, crossfading as it changes.
        let heading = gtk::Stack::new();
        heading.set_transition_type(gtk::StackTransitionType::Crossfade);
        heading.set_transition_duration(160);
        heading.set_hhomogeneous(false);
        heading.set_vhomogeneous(false);
        for name in ["a", "b"] {
            let label = gtk::Label::new(None);
            label.add_css_class("title-4");
            heading.add_named(&label, Some(name));
        }
        // The heading opens a menu under it: the arrow points down.
        let arrow = gtk::Image::from_icon_name("pan-down-symbolic");
        arrow.add_css_class("dim-label");
        let face = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        face.append(&heading);
        face.append(&arrow);
        let menu_button = gtk::MenuButton::builder().child(&face).build();
        menu_button.add_css_class("flat");
        menu_button.add_css_class("trending-heading");
        menu_button.set_widget_name("trend-period");
        menu_button.set_halign(gtk::Align::Start);

        let menu = gio::Menu::new();
        for period in TrendPeriod::ALL {
            menu.append(
                Some(&period.choice()),
                Some(&format!("trend.period::{}", key(period))),
            );
        }
        menu_button.set_menu_model(Some(&menu));
        let actions = gio::SimpleActionGroup::new();
        let action = gio::SimpleAction::new_stateful(
            "period",
            Some(glib::VariantTy::STRING),
            &key(TrendPeriod::Today).to_variant(),
        );
        actions.add_action(&action);
        menu_button.insert_action_group("trend", Some(&actions));

        let see_all = gtk::Button::with_label(&t::search_see_all());
        see_all.add_css_class("flat");
        see_all.add_css_class("link");
        let top = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        top.append(&menu_button);
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        top.append(&spacer);
        top.append(&see_all);

        let veil = gtk::Overlay::new();
        veil.set_child(Some(&states.widget));
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 8);
        widget.append(&top);
        widget.append(&veil);

        let inner = Rc::new_cyclic(|me: &std::rc::Weak<Inner>| Inner {
            me: me.clone(),
            ctx: Rc::clone(ctx),
            period: Cell::new(TrendPeriod::Today),
            chosen: Cell::new(false),
            kept: RefCell::default(),
            heading,
            grid,
            states,
            actions,
            see_all: RefCell::new(None),
            asked: Cell::new(None),
            veil,
            switching: RefCell::default(),
            landing: RefCell::default(),
        });
        inner.say_period(TrendPeriod::Today, false);

        action.connect_activate({
            let inner = Rc::downgrade(&inner);
            move |_, target| {
                if let (Some(inner), Some(value)) =
                    (inner.upgrade(), target.and_then(String::from_variant))
                {
                    inner.choose(from_key(&value));
                }
            }
        });
        see_all.connect_clicked({
            let inner = Rc::downgrade(&inner);
            move |_| {
                if let Some(inner) = inner.upgrade()
                    && let Some(go) = inner.see_all.borrow().as_ref()
                {
                    go(inner.period.get());
                }
            }
        });
        inner.states.on_retry({
            let inner = Rc::downgrade(&inner);
            move || {
                if let Some(inner) = inner.upgrade() {
                    inner.ask();
                }
            }
        });
        ctx.hub.listen({
            let inner = Rc::downgrade(&inner);
            move |event| {
                if let Some(inner) = inner.upgrade() {
                    inner.heard(event);
                }
            }
        });
        // The rail asks when Search is on screen, never at launch.
        widget.connect_map({
            let inner = Rc::downgrade(&inner);
            move |_| {
                if let Some(inner) = inner.upgrade() {
                    inner.visit();
                }
            }
        });
        Self { widget, inner }
    }

    /// What See all does: browse the period shown.
    pub fn on_see_all(&self, go: impl Fn(TrendPeriod) + 'static) {
        self.inner.see_all.replace(Some(Box::new(go)));
    }

    /// For a shot: pick `period` as the menu would.
    #[cfg(feature = "fixtures")]
    pub fn pick(&self, period: TrendPeriod) {
        self.inner.choose(period);
    }

    /// For a shot: the heading's menu open.
    #[cfg(feature = "fixtures")]
    pub fn menu(&self) -> Option<gtk::MenuButton> {
        let top = self.widget.first_child()?;
        top.first_child().and_downcast::<gtk::MenuButton>()
    }
}

impl Inner {
    /// Search came on screen: the browse (and with it the remembered period) is asked for, at
    /// most once a minute.
    fn visit(&self) {
        let recent = self.asked.get().is_some_and(|at| at.elapsed().as_secs() < 60);
        if recent {
            return;
        }
        self.asked.set(Some(std::time::Instant::now()));
        if !self.kept.borrow().contains_key(key(self.period.get())) {
            self.states.loading(None);
        }
        self.ctx.hub.send(Command::LoadBrowse);
    }

    fn ask(&self) {
        self.states.loading(None);
        self.ctx.hub.send(match self.period.get() {
            TrendPeriod::Today => Command::LoadBrowse,
            period => Command::LoadTrending(period),
        });
    }

    /// The viewer chose a period: what is kept shows at once and is asked for again behind it.
    fn choose(&self, period: TrendPeriod) {
        if self.chosen.replace(true) && self.period.get() == period {
            return;
        }
        let changed = self.period.get() != period;
        self.say_period(period, changed);
        self.period.set(period);
        let kept = self.kept.borrow().get(key(period)).cloned();
        self.settle();
        match kept {
            Some(entries) if changed => self.crossfade(|| self.show_now(&entries)),
            Some(entries) => self.show(&entries),
            None if self.states.has_answer() => self.switch(),
            None => self.states.loading(None),
        }
        // Today is the browse's own rail; a week and a month are the period's.
        match period {
            TrendPeriod::Today => {
                if self.kept.borrow().get(key(period)).is_none() {
                    self.ctx.hub.send(Command::LoadBrowse);
                }
                // Remembered as the choice, asked or not.
                self.ctx.hub.send(Command::LoadTrending(period));
            }
            _ => self.ctx.hub.send(Command::LoadTrending(period)),
        }
    }

    fn say_period(&self, period: TrendPeriod, animate: bool) {
        self.period.set(period);
        let name =
            if self.heading.visible_child_name().as_deref() == Some("a") { "b" } else { "a" };
        let label = self.heading.child_by_name(name).and_downcast::<gtk::Label>();
        if let Some(label) = label {
            label.set_label(&period.label());
        }
        if !animate {
            self.heading.set_transition_duration(0);
        }
        self.heading.set_visible_child_name(name);
        self.heading.set_transition_duration(160);
        if let Some(action) =
            self.actions.lookup_action("period").and_downcast::<gio::SimpleAction>()
        {
            action.set_state(&key(period).to_variant());
        }
    }

    /// A period not kept, with covers on screen: they stay for 600 ms, then the placeholders.
    fn switch(&self) {
        let me = self.me.clone();
        let timer = glib::timeout_add_local_once(crate::states::SILENT, move || {
            if let Some(inner) = me.upgrade() {
                inner.switching.borrow_mut().take();
                inner.crossfade(|| inner.states.placeholders_now());
            }
        });
        self.switching.replace(Some(timer));
    }

    /// The covers of the period chosen are in: they crossfade over what is there once the
    /// placeholders have stood their least time.
    fn land(&self, entries: &[Entry]) {
        if let Some(timer) = self.switching.borrow_mut().take() {
            // In before the 600 ms were up: straight from the old covers.
            timer.remove();
            self.crossfade(|| self.show_now(entries));
            return;
        }
        let Some(since) = self.states.placeholders_since() else {
            self.show(entries);
            return;
        };
        let left = crate::states::AT_LEAST.saturating_sub(since.elapsed());
        if left.is_zero() {
            self.crossfade(|| self.show_now(entries));
            return;
        }
        let me = self.me.clone();
        let entries = entries.to_vec();
        let timer = glib::timeout_add_local_once(left, move || {
            if let Some(inner) = me.upgrade() {
                inner.landing.borrow_mut().take();
                inner.crossfade(|| inner.show_now(&entries));
            }
        });
        if let Some(old) = self.landing.replace(Some(timer)) {
            old.remove();
        }
    }

    /// Any switch or landing under way is called off: a newer choice takes over.
    fn settle(&self) {
        if let Some(timer) = self.switching.borrow_mut().take() {
            timer.remove();
        }
        if let Some(timer) = self.landing.borrow_mut().take() {
            timer.remove();
        }
    }

    /// The change crossfades over a picture of the rail as it stood, under Reduce Animation too.
    fn crossfade(&self, change: impl FnOnce()) {
        crate::motion::ghost(&self.veil, &self.states.widget, true, change);
    }

    /// The covers at once, for a crossfade to draw over.
    fn show_now(&self, entries: &[Entry]) {
        self.grid.set(&entries[..entries.len().min(SHOWN)]);
        self.states.answer_now(entries.is_empty());
    }

    fn show(&self, entries: &[Entry]) {
        self.grid.set(&entries[..entries.len().min(SHOWN)]);
        if entries.is_empty() {
            self.states.empty();
        } else {
            self.states.content();
        }
    }

    fn heard(&self, event: &Event) {
        match event {
            // Today is the first rail of the browse.
            Event::Browse(rails) => {
                let Some(rail) =
                    rails.iter().find(|r| r.query.sort == Some(SearchSort::Trending))
                else {
                    return;
                };
                self.kept.borrow_mut().insert(key(TrendPeriod::Today), rail.entries.clone());
                if self.period.get() == TrendPeriod::Today && !rail.entries.is_empty() {
                    self.land(&rail.entries);
                }
            }
            Event::Trending { period, entries, failed, .. } => {
                if !*failed {
                    self.kept.borrow_mut().insert(key(*period), entries.clone());
                }
                // A period remembered from an earlier visit comes before the viewer has chosen.
                if !self.chosen.get() && *period != self.period.get() && !*failed {
                    self.say_period(*period, false);
                }
                if *period != self.period.get() {
                    return;
                }
                if *failed {
                    if self.kept.borrow().contains_key(key(*period)) {
                        return;
                    }
                    self.settle();
                    self.states
                        .failed_title(&t::trending_failed(&period.choice().to_lowercase()));
                    self.states.failed("");
                } else {
                    self.land(entries);
                }
            }
            _ => {}
        }
    }
}
