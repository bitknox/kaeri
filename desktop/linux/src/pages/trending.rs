//! Search's Trending rail: the heading is a menu of periods (today, this week, this month),
//! the rail is a row of covers, and See all browses the period shown.
//!
//! Today is the browse's own Trending now rail. A week or a month comes from the engine's
//! `LoadTrending`, which remembers the choice on this device. Nothing here asks at launch: the
//! rail asks when Search is on screen. A change of period keeps the old covers for 600 ms, then
//! shows placeholders, then crossfades to the new covers.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use adw::prelude::*;
use gtk::{gio, glib};
use kaeri_i18n::t;
use kaeri_model::{
    browsing::TrendPeriod,
    catalog::Entry,
    command::Command,
    event::Event,
    search::SearchSort,
};

use crate::{
    states::{Shape, States},
    widgets::{self, PosterGrid},
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
        let arrow = widgets::chevron();
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
            menu.append(Some(&period.choice()), Some(&format!("trend.period::{}", key(period))));
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

        let widget = gtk::Box::new(gtk::Orientation::Vertical, 8);
        widget.append(&top);
        widget.append(&states.widget);

        let inner = Rc::new(Inner {
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
        match kept {
            Some(entries) => self.show(&entries),
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
        let name = if self.heading.visible_child_name().as_deref() == Some("a") { "b" } else { "a" };
        let label = self.heading.child_by_name(name).and_downcast::<gtk::Label>();
        if let Some(label) = label {
            label.set_label(&period.label());
        }
        if !animate {
            self.heading.set_transition_duration(0);
        }
        self.heading.set_visible_child_name(name);
        self.heading.set_transition_duration(160);
        if let Some(action) = self.actions.lookup_action("period").and_downcast::<gio::SimpleAction>() {
            action.set_state(&key(period).to_variant());
        }
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
                let Some(rail) = rails.iter().find(|r| r.query.sort == Some(SearchSort::Trending))
                else {
                    return;
                };
                self.kept.borrow_mut().insert(key(TrendPeriod::Today), rail.entries.clone());
                if self.period.get() == TrendPeriod::Today && !rail.entries.is_empty() {
                    self.show(&rail.entries);
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
                    self.states
                        .failed_title(&t::trending_failed(&period.choice().to_lowercase()));
                    self.states.failed("");
                } else {
                    self.show(entries);
                }
            }
            _ => {}
        }
    }
}
