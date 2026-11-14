//! The reader settings popover from Aa, as the reader board's Linux frames draw it: rows under
//! the heading of where a change applies (This series, then every title of the medium), the
//! label at the start and the control at the end, in silk on Raised. The rest sits under More
//! reader settings.

use std::rc::Rc;

use adw::prelude::*;
use kaeri_core::media::Medium;
use kaeri_i18n::t;
use kaeri_model::reader::{
    ReaderCommand, ReaderMode, ReaderSettingKey as Key, ReaderSettingRow, ReaderValue,
    SettingScope,
};

use super::{Reader, choice_label, setting_name};

/// The popover's width, the board's 330 px.
const WIDTH: i32 = 330;

/// The sheet's width in a window `window` px wide: the board's, or what the window leaves of it.
pub(super) fn sheet_width(window: i32) -> i32 {
    super::chapters_fit((WIDTH, 0), (window, i32::MAX)).0
}

/// The reading mode picker's choices for manga and webtoons.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum ModeChoice {
    Auto,
    RightToLeft,
    LeftToRight,
    Strip,
}

const MODES: [ModeChoice; 4] =
    [ModeChoice::Auto, ModeChoice::RightToLeft, ModeChoice::LeftToRight, ModeChoice::Strip];

/// Which of the four the series reads in: Auto unless it has a choice of its own.
pub(super) fn current_mode(rows: &[ReaderSettingRow]) -> ModeChoice {
    let row = |key: Key| rows.iter().find(|r| r.key == key);
    let overridden = row(Key::Mode).is_some_and(|r| r.overridden)
        || row(Key::Direction).is_some_and(|r| r.overridden);
    if !overridden {
        return ModeChoice::Auto;
    }
    let choice = |key: Key| match row(key).map(|r| &r.value) {
        Some(ReaderValue::Choice { name }) => Some(name.as_str()),
        _ => None,
    };
    if choice(Key::Mode) == Some("strip") {
        ModeChoice::Strip
    } else if choice(Key::Direction) == Some("ltr") {
        ModeChoice::LeftToRight
    } else {
        ModeChoice::RightToLeft
    }
}

/// What the sheet holds, so a change that keeps it can wait for the popover to close.
#[derive(Clone, PartialEq, Debug, Default)]
pub(super) struct Shape {
    medium: Option<Medium>,
    width: i32,
    flows: bool,
    keys: Vec<Key>,
    mode: Option<ModeChoice>,
    detected: bool,
}

impl Reader {
    /// The sheet's shape for the settings as they are now.
    pub(super) fn sheet_shape(&self) -> Shape {
        let state = self.state.borrow();
        let rows = state.settings.as_ref().map(|v| v.rows.clone()).unwrap_or_default();
        Shape {
            medium: state.medium,
            width: sheet_width(self.widget.width()),
            flows: state.mode.is_some_and(ReaderMode::flows),
            keys: rows.iter().map(|r| r.key).collect(),
            mode: (!rows.is_empty()).then(|| current_mode(&rows)),
            detected: rows.iter().any(|r| r.detected),
        }
    }

    /// Fill the popover from the settings. While it is open and only values changed, the
    /// controls already show them, and the rows stay as they are until it closes.
    pub(super) fn fill_sheet(self: &Rc<Self>, force: bool) {
        let shape = self.sheet_shape();
        if !force && self.sheet.is_visible() && *self.sheet_shape_now.borrow() == shape {
            self.sheet_stale.set(true);
            return;
        }
        // A mode just picked waits for its list to slide away before the rows change.
        if self.sheet_held.get() {
            self.sheet_stale.set(true);
            return;
        }
        self.sheet_stale.set(false);
        *self.sheet_shape_now.borrow_mut() = shape;
        // The settings are the first page; Reading mode pushes its list of four over them.
        let nav = adw::NavigationView::new();
        nav.set_animate_transitions(!crate::motion::reduced());
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .propagate_natural_height(true)
            .max_content_height(560)
            .build();
        // The first page added is the one the sheet opens on, so the rows' page goes in before
        // the rows add the mode list behind it.
        nav.add(&adw::NavigationPage::with_tag(
            &scroll,
            &t::linux_reader_settings_open(),
            "main",
        ));
        scroll.set_child(Some(&self.sheet_rows(&nav)));
        self.sheet.set_child(Some(&nav));
    }

    fn sheet_rows(self: &Rc<Self>, nav: &adw::NavigationView) -> gtk::Box {
        let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
        column.add_css_class("reader-sheet-column");
        column.set_width_request(sheet_width(self.widget.width()));
        let (view, medium, flows) = {
            let state = self.state.borrow();
            (state.settings.clone(), state.medium, state.mode.is_some_and(ReaderMode::flows))
        };
        let (Some(view), Some(medium)) = (view, medium) else { return column };
        let rows = view.rows;
        let row = |key: Key| rows.iter().find(|r| r.key == key).cloned();
        let novel = medium == Medium::Novel;

        // This series: the reading mode, guessed or chosen.
        if !novel && (row(Key::Mode).is_some() || row(Key::Direction).is_some()) {
            column.append(&heading(&t::linux_reader_this_series()));
            column.append(&self.mode_row(&rows, nav));
            let overridden = current_mode(&rows) != ModeChoice::Auto;
            if overridden {
                let reset = gtk::Button::with_label(&t::linux_reader_use_default());
                reset.add_css_class("flat");
                reset.add_css_class("reader-sheet-link");
                reset.set_halign(gtk::Align::Start);
                let this = Rc::downgrade(self);
                reset.connect_clicked(move |_| {
                    if let Some(this) = this.upgrade() {
                        this.choose_mode(ModeChoice::Auto);
                    }
                });
                column.append(&reset);
            } else if row(Key::Direction).is_some_and(|r| r.detected)
                || row(Key::Mode).is_some_and(|r| r.detected)
            {
                let note = crate::widgets::haze(&t::linux_reader_mode_detected(), "caption");
                note.set_xalign(0.0);
                note.add_css_class("reader-sheet-note");
                column.append(&note);
            }
        }

        // Every title of the medium: the rows the board puts up front.
        let main: &[Key] = if novel {
            &[Key::TextSize, Key::Look, Key::LineSpacing]
        } else if flows {
            &[Key::StripWidth, Key::StripGap, Key::Background]
        } else {
            &[Key::PageLayout, Key::Fit, Key::CropBorders, Key::Background]
        };
        let more: &[Key] = if novel {
            &[Key::Font, Key::Margins, Key::Justify, Key::Mode]
        } else if flows {
            &[]
        } else {
            &[Key::Spreads]
        };
        let shown: Vec<ReaderSettingRow> = main.iter().filter_map(|k| row(*k)).collect();
        if !shown.is_empty() {
            let all_here = shown.iter().all(|r| r.scope == SettingScope::Device);
            let title = match (novel, flows, all_here) {
                (true, _, _) => t::linux_reader_all_novels(),
                (false, true, true) => t::linux_reader_all_strips_here(),
                (false, true, false) => t::linux_reader_all_strips(),
                (false, false, true) => t::linux_reader_all_manga_here(),
                (false, false, false) => t::linux_reader_all_manga(),
            };
            column.append(&heading(&title));
            for setting in &shown {
                column.append(&self.setting_row(setting, medium));
            }
        }
        let extra: Vec<ReaderSettingRow> = more.iter().filter_map(|k| row(*k)).collect();
        if !extra.is_empty() {
            // Hidden rather than an unrevealed revealer, which without a slide keeps its
            // child's height and leaves the sheet that much taller than its rows.
            let rest = gtk::Box::new(gtk::Orientation::Vertical, 0);
            for setting in &extra {
                rest.append(&self.setting_row(setting, medium));
            }
            rest.set_visible(false);
            let open = gtk::Image::from_icon_name("pan-end-symbolic");
            open.add_css_class("haze");
            let words = gtk::Label::new(Some(&t::linux_reader_more_settings()));
            words.set_xalign(0.0);
            words.set_hexpand(true);
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            line.append(&words);
            line.append(&open);
            let button = gtk::Button::builder().child(&line).build();
            button.add_css_class("flat");
            button.add_css_class("reader-sheet-more");
            button.update_property(&[gtk::accessible::Property::Label(
                &t::linux_reader_more_settings(),
            )]);
            button.update_state(&[gtk::accessible::State::Expanded(Some(false))]);
            button.connect_clicked({
                let rest = rest.clone();
                move |button| {
                    let on = !rest.is_visible();
                    rest.set_visible(on);
                    open.set_icon_name(Some(if on {
                        "pan-down-symbolic"
                    } else {
                        "pan-end-symbolic"
                    }));
                    button.update_state(&[gtk::accessible::State::Expanded(Some(on))]);
                }
            });
            column.append(&button);
            column.append(&rest);
        }
        column
    }

    /// Reading mode: a row that pushes the list of four, Auto, right to left, left to right
    /// and strip, kept for this series. The row says what is in use: "Auto · Right to left".
    fn mode_row(
        self: &Rc<Self>,
        rows: &[ReaderSettingRow],
        nav: &adw::NavigationView,
    ) -> gtk::Widget {
        let current = current_mode(rows);
        let flows = self.state.borrow().mode.is_some_and(ReaderMode::flows);
        let auto_now = auto_now_of(flows, self.rtl());
        let name = |mode: ModeChoice| match mode {
            ModeChoice::Auto => t::linux_reader_mode_auto(),
            ModeChoice::RightToLeft => t::reader_mode_rtl(),
            ModeChoice::LeftToRight => t::reader_mode_ltr(),
            ModeChoice::Strip => t::reader_mode_strip(),
        };
        let value = match current {
            ModeChoice::Auto => {
                [t::linux_reader_mode_auto(), auto_now.clone()].join(" \u{b7} ")
            }
            other => name(other),
        };

        let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        list.add_css_class("reader-sheet-column");
        list.set_width_request(sheet_width(self.widget.width()));
        let back = gtk::Button::from_icon_name("go-previous-symbolic");
        back.add_css_class("flat");
        back.set_tooltip_text(Some(&t::action_back()));
        back.update_property(&[gtk::accessible::Property::Label(&t::action_back())]);
        back.connect_clicked({
            let nav = nav.downgrade();
            move |_| {
                if let Some(nav) = nav.upgrade() {
                    nav.pop();
                }
            }
        });
        let title = gtk::Label::new(Some(&t::linux_reader_reading_mode()));
        title.add_css_class("heading");
        title.set_xalign(0.0);
        let top = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        top.add_css_class("reader-sheet-pushed-head");
        top.append(&back);
        top.append(&title);
        list.append(&top);
        for mode in MODES {
            // Auto says what it chose while it is in use.
            let words =
                gtk::Label::new(Some(&if mode == ModeChoice::Auto && current == mode {
                    value.clone()
                } else {
                    name(mode)
                }));
            words.set_xalign(0.0);
            words.set_hexpand(true);
            let check = gtk::Image::from_icon_name("object-select-symbolic");
            check.set_opacity(if mode == current { 1.0 } else { 0.0 });
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            line.append(&words);
            line.append(&check);
            let button = gtk::Button::builder().child(&line).build();
            button.add_css_class("flat");
            button.add_css_class("reader-sheet-choice");
            button.update_state(&[gtk::accessible::State::Checked(if mode == current {
                gtk::AccessibleTristate::True
            } else {
                gtk::AccessibleTristate::False
            })]);
            button.connect_clicked({
                let (this, nav) = (Rc::downgrade(self), nav.downgrade());
                move |_| {
                    let Some(this) = this.upgrade() else { return };
                    if mode != current {
                        // The rows wait until the list has gone, so the slide is not cut short.
                        this.sheet_held.set(true);
                        this.choose_mode(mode);
                    }
                    if let Some(nav) = nav.upgrade() {
                        nav.pop();
                    }
                }
            });
            list.append(&button);
        }
        let page =
            adw::NavigationPage::with_tag(&list, &t::linux_reader_reading_mode(), "mode");
        page.connect_hidden({
            let this = Rc::downgrade(self);
            move |_| {
                let Some(this) = this.upgrade() else { return };
                if this.sheet_held.replace(false) && this.sheet_stale.get() {
                    this.fill_sheet(true);
                }
            }
        });
        nav.add(&page);

        let label = gtk::Label::new(Some(&t::linux_reader_reading_mode()));
        label.set_xalign(0.0);
        label.set_hexpand(true);
        let shows = gtk::Label::new(Some(&value));
        shows.add_css_class("haze");
        shows.set_ellipsize(gtk::pango::EllipsizeMode::End);
        let open = gtk::Image::from_icon_name("go-next-symbolic");
        open.add_css_class("haze");
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        line.append(&label);
        line.append(&shows);
        line.append(&open);
        let row = gtk::Button::builder().child(&line).build();
        row.add_css_class("flat");
        row.add_css_class("reader-sheet-push");
        row.update_property(&[
            gtk::accessible::Property::Label(&t::linux_reader_reading_mode()),
            gtk::accessible::Property::Description(&value),
        ]);
        row.connect_clicked({
            let nav = nav.downgrade();
            move |_| {
                if let Some(nav) = nav.upgrade() {
                    nav.push_by_tag("mode");
                }
            }
        });
        row.upcast()
    }

    /// Auto drops the series' choice; the others set its mode and direction, which follow the
    /// person.
    pub(super) fn choose_mode(&self, mode: ModeChoice) {
        let (medium, work) = {
            let state = self.state.borrow();
            (state.medium.unwrap_or(Medium::Manga), state.work.clone())
        };
        let Some(work) = work else { return };
        match mode {
            ModeChoice::Auto => {
                for key in [Key::Mode, Key::Direction] {
                    self.send(ReaderCommand::UseDefault { medium, work: work.clone(), key });
                }
            }
            ModeChoice::RightToLeft | ModeChoice::LeftToRight => {
                let direction = if mode == ModeChoice::RightToLeft { "rtl" } else { "ltr" };
                self.set_setting(Key::Mode, choice("paged"), SettingScope::Series);
                self.set_setting(Key::Direction, choice(direction), SettingScope::Series);
            }
            ModeChoice::Strip => {
                self.set_setting(Key::Mode, choice("strip"), SettingScope::Series);
            }
        }
    }

    /// Change a setting where it is kept: a series' own choice names the work.
    pub(super) fn set_setting(&self, key: Key, value: ReaderValue, scope: SettingScope) {
        let (session, medium, work) = {
            let state = self.state.borrow();
            (state.session, state.medium.unwrap_or(Medium::Manga), state.work.clone())
        };
        let work = matches!(scope, SettingScope::Series | SettingScope::SeriesHere)
            .then_some(work)
            .flatten();
        self.send(ReaderCommand::SetSetting {
            session: (session != 0).then_some(session),
            medium,
            work,
            key,
            scope,
            value,
        });
    }

    /// A novel's text a step larger (`1`), smaller (`-1`) or back to its size (`0`).
    pub(super) fn step_text_size(&self, by: i32) {
        if self.state.borrow().medium != Some(Medium::Novel) {
            return;
        }
        let now = self.amount(Key::TextSize).unwrap_or(1.0);
        let to = match by {
            0 => 1.0,
            by => ((now + f64::from(by) * 0.1) * 10.0).round() / 10.0,
        }
        .clamp(0.5, 3.0);
        if (to - now).abs() > f64::EPSILON {
            self.set_setting(
                Key::TextSize,
                ReaderValue::Amount { value: to },
                Key::TextSize.home_scope(),
            );
        }
    }

    fn setting_row(self: &Rc<Self>, row: &ReaderSettingRow, medium: Medium) -> gtk::Widget {
        let key = row.key;
        let scope = if key == Key::Mode { SettingScope::Series } else { key.home_scope() };
        let send = {
            let this = Rc::downgrade(self);
            move |value: ReaderValue| {
                if let Some(this) = this.upgrade() {
                    this.set_setting(key, value, scope);
                }
            }
        };
        let _ = medium;
        match (&row.value, key) {
            (ReaderValue::Amount { value }, Key::TextSize) => text_size(*value, send).upcast(),
            (ReaderValue::Amount { value }, Key::LineSpacing) => sheet_row(
                &setting_name(key),
                &amounts(
                    *value,
                    &[
                        (t::linux_reader_line_tight(), 1.2),
                        (t::linux_reader_line_normal(), 1.55),
                        (t::linux_reader_line_loose(), 1.8),
                    ],
                    send,
                ),
            )
            .upcast(),
            (ReaderValue::Amount { value }, Key::Margins) => sheet_row(
                &setting_name(key),
                &amounts(
                    *value,
                    &[
                        (t::reader_margins_narrow(), 0.6),
                        (t::linux_reader_margins_normal(), 1.0),
                        (t::reader_margins_wide(), 1.6),
                    ],
                    send,
                ),
            )
            .upcast(),
            (ReaderValue::Amount { value }, _) => {
                let (low, high) = match key {
                    Key::StripWidth => (0.3, 1.0),
                    _ => (0.5, 3.0),
                };
                let scale =
                    gtk::Scale::with_range(gtk::Orientation::Horizontal, low, high, 0.05);
                scale.set_value(*value);
                scale.set_size_request(150, -1);
                scale.add_css_class("reader-sheet-scale");
                scale.update_property(&[gtk::accessible::Property::Label(&setting_name(key))]);
                scale.connect_value_changed(move |scale| {
                    send(ReaderValue::Amount { value: scale.value() });
                });
                sheet_row(&setting_name(key), &scale).upcast()
            }
            (ReaderValue::Toggle { on }, _) => {
                let switch = gtk::Switch::new();
                switch.set_active(*on);
                switch.set_valign(gtk::Align::Center);
                switch.update_property(&[gtk::accessible::Property::Label(&setting_name(key))]);
                switch.connect_active_notify(move |switch| {
                    send(ReaderValue::Toggle { on: switch.is_active() });
                });
                sheet_row(&setting_name(key), &switch).upcast()
            }
            (ReaderValue::Choice { name }, Key::Look) => {
                swatches(name, &row.choices, send).upcast()
            }
            (ReaderValue::Choice { name }, _) => {
                let group = adw::ToggleGroup::new();
                for value in &row.choices {
                    if let ReaderValue::Choice { name } = value {
                        group.add(
                            adw::Toggle::builder()
                                .name(name.as_str())
                                .label(choice_label(name))
                                .build(),
                        );
                    }
                }
                group.set_active_name(Some(name));
                group.add_css_class("reader-sheet-segments");
                group.connect_active_name_notify(move |group| {
                    if let Some(name) = group.active_name() {
                        send(ReaderValue::Choice { name: name.to_string() });
                    }
                });
                sheet_row(&setting_name(key), &group).upcast()
            }
        }
    }
}

/// What Auto reads in now.
fn auto_now_of(flows: bool, rtl: bool) -> String {
    if flows {
        t::reader_mode_strip()
    } else if rtl {
        t::reader_mode_rtl()
    } else {
        t::reader_mode_ltr()
    }
}

fn choice(name: &str) -> ReaderValue {
    ReaderValue::Choice { name: name.to_owned() }
}

fn heading(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_xalign(0.0);
    label.add_css_class("reader-sheet-heading");
    label.add_css_class("haze");
    label
}

/// A row: the setting's name at the start, its control at the end.
fn sheet_row(name: &str, control: &impl IsA<gtk::Widget>) -> gtk::Box {
    let label = gtk::Label::new(Some(name));
    label.set_xalign(0.0);
    label.set_hexpand(true);
    label.set_wrap(true);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.add_css_class("reader-sheet-row");
    row.append(&label);
    control.as_ref().set_valign(gtk::Align::Center);
    row.append(control);
    row
}

/// Named amounts as segments; the nearest is on.
fn amounts(
    value: f64,
    options: &[(String, f64)],
    send: impl Fn(ReaderValue) + 'static,
) -> adw::ToggleGroup {
    let group = adw::ToggleGroup::new();
    for (i, (name, _)) in options.iter().enumerate() {
        group.add(adw::Toggle::builder().name(i.to_string()).label(name.as_str()).build());
    }
    let nearest = options
        .iter()
        .enumerate()
        .min_by(|a, b| (a.1.1 - value).abs().total_cmp(&(b.1.1 - value).abs()))
        .map_or(0, |(i, _)| i);
    group.set_active(nearest as u32);
    group.add_css_class("reader-sheet-segments");
    let values: Vec<f64> = options.iter().map(|(_, v)| *v).collect();
    group.connect_active_notify(move |group| {
        if let Some(value) = values.get(group.active() as usize) {
            send(ReaderValue::Amount { value: *value });
        }
    });
    group
}

/// Text size as the board draws it: a small A, the slider, a large A.
fn text_size(value: f64, send: impl Fn(ReaderValue) + 'static) -> gtk::Box {
    let small = gtk::Label::new(Some("A")); // i18n-skip
    small.add_css_class("reader-sheet-a-small");
    small.update_property(&[gtk::accessible::Property::Label(&t::linux_reader_text_smaller())]);
    let large = gtk::Label::new(Some("A")); // i18n-skip
    large.add_css_class("reader-sheet-a-large");
    large.update_property(&[gtk::accessible::Property::Label(&t::linux_reader_text_larger())]);
    let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.5, 3.0, 0.1);
    scale.set_value(value);
    scale.set_hexpand(true);
    scale.set_round_digits(1);
    scale.add_css_class("reader-sheet-scale");
    scale.update_property(&[gtk::accessible::Property::Label(&t::linux_reader_text_size())]);
    scale.connect_value_changed(move |scale| {
        send(ReaderValue::Amount { value: (scale.value() * 10.0).round() / 10.0 });
    });
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.add_css_class("reader-sheet-row");
    row.append(&small);
    row.append(&scale);
    row.append(&large);
    row
}

/// The looks as swatches: each one's page colour with Aa in its ink, its name under it.
fn swatches(
    active: &str,
    choices: &[ReaderValue],
    send: impl Fn(ReaderValue) + 'static,
) -> gtk::Box {
    let send = Rc::new(send);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.add_css_class("reader-sheet-swatches");
    row.set_homogeneous(true);
    let mut buttons: Vec<gtk::ToggleButton> = Vec::new();
    for value in choices {
        let ReaderValue::Choice { name } = value else { continue };
        let tile = gtk::Label::new(Some("Aa")); // i18n-skip
        tile.add_css_class("reader-swatch");
        tile.add_css_class(&format!("look-{name}"));
        tile.set_halign(gtk::Align::Center);
        let words = gtk::Label::new(Some(&choice_label(name)));
        words.add_css_class("caption");
        let column = gtk::Box::new(gtk::Orientation::Vertical, 6);
        column.append(&tile);
        column.append(&words);
        let button = gtk::ToggleButton::builder().child(&column).build();
        button.add_css_class("flat");
        button.add_css_class("reader-swatch-button");
        button.update_property(&[gtk::accessible::Property::Label(&choice_label(name))]);
        if let Some(first) = buttons.first() {
            button.set_group(Some(first));
        }
        button.set_active(name == active);
        let (send, name) = (Rc::clone(&send), name.clone());
        button.connect_toggled(move |button| {
            if button.is_active() {
                send(ReaderValue::Choice { name: name.clone() });
            }
        });
        row.append(&button);
        buttons.push(button);
    }
    row.update_property(&[gtk::accessible::Property::Label(&t::linux_reader_look())]);
    row
}

#[cfg(test)]
mod tests {
    use kaeri_model::reader::{
        ReaderSettingKey as Key, ReaderSettingRow, ReaderValue, SettingScope,
    };

    use super::{ModeChoice, current_mode};

    fn row(key: Key, name: &str, overridden: bool) -> ReaderSettingRow {
        ReaderSettingRow {
            key,
            value: ReaderValue::Choice { name: name.to_owned() },
            scope: SettingScope::Series,
            detected: false,
            overridden,
            choices: Vec::new(),
        }
    }

    #[test]
    fn the_mode_is_auto_until_the_series_chooses() {
        let rows = [row(Key::Mode, "paged", false), row(Key::Direction, "ltr", false)];
        assert_eq!(current_mode(&rows), ModeChoice::Auto);
        let rows = [row(Key::Mode, "paged", false), row(Key::Direction, "ltr", true)];
        assert_eq!(current_mode(&rows), ModeChoice::LeftToRight);
        let rows = [row(Key::Mode, "paged", true), row(Key::Direction, "rtl", false)];
        assert_eq!(current_mode(&rows), ModeChoice::RightToLeft);
        let rows = [row(Key::Mode, "strip", true), row(Key::Direction, "rtl", false)];
        assert_eq!(current_mode(&rows), ModeChoice::Strip);
    }
}
