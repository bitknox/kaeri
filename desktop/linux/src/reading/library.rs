//! Library, Reading side: the medium and the list over a grid of covers. The list is a menu
//! button that says the list on screen and how many it holds; the order sits in the header.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use adw::prelude::*;
use gtk::{gio, glib};
use kaeri_i18n::t;
use kaeri_model::{
    event::Event,
    reading::{LibrarySegmentRead as Segment, ReadingCommand, ReadingEvent, WorkTile},
};

use super::{Dest, OnScreen, WorkGrid, listen, scope, send};
use crate::{
    states::{Shape, States},
    widgets,
    window::Ctx,
};

const SEGMENTS: [Segment; 7] = [
    Segment::Reading,
    Segment::Planning,
    Segment::Completed,
    Segment::Paused,
    Segment::Dropped,
    Segment::Rereading,
    Segment::YourFiles,
];

pub fn name(segment: Segment) -> String {
    match segment {
        Segment::Reading => t::read_list_reading(),
        Segment::Planning => t::read_list_planning(),
        Segment::Completed => t::read_list_completed(),
        Segment::Paused => t::read_list_paused(),
        Segment::Dropped => t::read_list_dropped(),
        Segment::Rereading => t::read_list_rereading(),
        Segment::YourFiles => t::read_list_your_files(),
    }
}

/// The order of a list: as the engine sends it, most recently read first, or by title.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Sort {
    LastRead,
    Title,
}

impl Sort {
    fn name(self) -> &'static str {
        match self {
            Self::LastRead => "last-read",
            Self::Title => "title",
        }
    }

    fn parse(name: &str) -> Option<Self> {
        [Self::LastRead, Self::Title].into_iter().find(|s| s.name() == name)
    }

    fn label(self) -> String {
        match self {
            Self::LastRead => t::read_sort_last_read(),
            Self::Title => t::library_sort_title(),
        }
    }

    fn sorted(self, tiles: &[WorkTile]) -> Vec<WorkTile> {
        let mut tiles = tiles.to_vec();
        if self == Self::Title {
            tiles.sort_by_key(|tile| tile.title.to_lowercase());
        }
        tiles
    }
}

/// The order menu: an icon button in the header, as the Shows side has, over a radio action.
fn sort_menu(on_change: impl Fn(Sort) + 'static) -> (gtk::MenuButton, Rc<Cell<Sort>>) {
    let current = Rc::new(Cell::new(Sort::LastRead));
    let orders = gio::Menu::new();
    for sort in [Sort::LastRead, Sort::Title] {
        let item = gio::MenuItem::new(Some(&sort.label()), None);
        item.set_action_and_target_value(Some("sort.order"), Some(&sort.name().to_variant()));
        orders.append_item(&item);
    }
    let menu = gio::Menu::new();
    menu.append_section(Some(&t::library_linux_order()), &orders);
    let button = gtk::MenuButton::builder()
        .icon_name("view-sort-descending-symbolic")
        .tooltip_text(t::library_linux_order())
        .menu_model(&menu)
        .build();
    let action = gio::SimpleAction::new_stateful(
        "order",
        Some(glib::VariantTy::STRING),
        &Sort::LastRead.name().to_variant(),
    );
    action.connect_activate({
        let current = Rc::clone(&current);
        move |action, target| {
            let Some(sort) =
                target.and_then(String::from_variant).and_then(|n| Sort::parse(&n))
            else {
                return;
            };
            action.set_state(&sort.name().to_variant());
            current.set(sort);
            on_change(sort);
        }
    });
    let group = gio::SimpleActionGroup::new();
    group.add_action(&action);
    button.insert_action_group("sort", Some(&group));
    (button, current)
}

/// The row over the covers: the medium, then the list menu, which goes under it when a narrow
/// window or large text leaves no room beside it.
fn bar(scope: &impl IsA<gtk::Widget>, menu: &impl IsA<gtk::Widget>) -> adw::WrapBox {
    let bar = adw::WrapBox::builder().child_spacing(12).line_spacing(8).build();
    bar.set_margin_top(12);
    bar.set_margin_bottom(6);
    bar.set_margin_start(18);
    bar.set_margin_end(18);
    bar.append(scope);
    bar.append(menu);
    bar
}

/// The list menu: "Reading 14" with a chevron, opening every list with its count.
struct ListMenu {
    button: gtk::MenuButton,
    name: gtk::Label,
    count: gtk::Label,
    rows: Vec<(Segment, gtk::Label, gtk::Image)>,
}

impl ListMenu {
    fn new(on_pick: impl Fn(Segment) + 'static) -> Rc<Self> {
        let name = gtk::Label::new(Some(&self::name(Segment::Reading)));
        let count = gtk::Label::new(None);
        count.add_css_class("haze");
        count.add_css_class("numeric");
        let label = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        label.append(&name);
        label.append(&count);
        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::None);
        list.add_css_class("navigation-sidebar");
        let mut rows = Vec::new();
        for segment in SEGMENTS {
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            line.set_margin_start(6);
            line.set_margin_end(6);
            let check = gtk::Image::from_icon_name("object-select-symbolic");
            check.set_opacity(0.0);
            let title = gtk::Label::new(Some(&self::name(segment)));
            title.set_xalign(0.0);
            title.set_hexpand(true);
            let n = gtk::Label::new(None);
            n.add_css_class("haze");
            n.add_css_class("numeric");
            line.append(&check);
            line.append(&title);
            line.append(&n);
            let row = gtk::ListBoxRow::builder().child(&line).build();
            row.set_widget_name(segment.as_str());
            list.append(&row);
            rows.push((segment, n, check));
        }
        let popover = gtk::Popover::builder().child(&list).build();
        popover.set_size_request(240, -1);
        let button = gtk::MenuButton::builder()
            .child(&label)
            .popover(&popover)
            .always_show_arrow(true)
            .build();
        button.update_property(&[gtk::accessible::Property::Label(
            &t::linux_read_library_list(),
        )]);
        list.connect_row_activated({
            let popover = popover.clone();
            move |_, row| {
                popover.popdown();
                if let Some(segment) = Segment::parse(&row.widget_name()) {
                    on_pick(segment);
                }
            }
        });
        Rc::new(Self { button, name, count, rows })
    }

    /// Show `current` on the button and the counts beside every list.
    fn show(&self, current: Segment, counts: &HashMap<Segment, usize>) {
        let n = counts.get(&current).copied();
        self.name.set_label(&name(current));
        self.count.set_label(&n.map(|n| n.to_string()).unwrap_or_default());
        self.button.update_property(&[gtk::accessible::Property::Description(
            &t::read_library_list_value(&name(current), n.unwrap_or(0) as i64),
        )]);
        for (segment, label, check) in &self.rows {
            label.set_label(&counts.get(segment).map(|n| n.to_string()).unwrap_or_default());
            check.set_opacity(if *segment == current { 1.0 } else { 0.0 });
        }
    }
}

pub fn build(ctx: &Rc<Ctx>) -> adw::NavigationPage {
    let grid = Rc::new(WorkGrid::new(&ctx.hub));
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&grid.widget)
        .build();
    grid.follow(&scroll);
    let empty = crate::states::plain(
        &t::read_library_files_empty_title(),
        &t::read_library_files_empty_line(),
    );
    let states = States::new(&ctx.hub, &scroll, Shape::Grid, &empty, &t::error_generic());

    let current = Rc::new(Cell::new(Segment::Reading));
    let kept: Rc<RefCell<HashMap<Segment, Vec<WorkTile>>>> = Rc::default();
    let (order, sort) = sort_menu({
        let (grid, kept, current) = (Rc::clone(&grid), Rc::clone(&kept), Rc::clone(&current));
        move |sort| {
            if let Some(tiles) = kept.borrow().get(&current.get()) {
                grid.set(&sort.sorted(tiles));
            }
        }
    });
    // Every list is asked for, so the menu can say how many each holds.
    let ask_all = {
        let hub = Rc::downgrade(&ctx.hub);
        let (states, kept, current) = (states.clone(), Rc::clone(&kept), Rc::clone(&current));
        move || {
            let Some(hub) = hub.upgrade() else { return };
            if !kept.borrow().contains_key(&current.get()) {
                states.loading(None);
            }
            for segment in SEGMENTS {
                send(&hub, ReadingCommand::LoadLibrary { segment });
            }
        }
    };
    // The grid for the list on screen: its covers, or its empty or loading state.
    let show: Rc<dyn Fn()> = Rc::new({
        let (states, grid, kept, current, sort) = (
            states.clone(),
            Rc::clone(&grid),
            Rc::clone(&kept),
            Rc::clone(&current),
            Rc::clone(&sort),
        );
        move || match kept.borrow().get(&current.get()) {
            Some(tiles) if tiles.is_empty() => states.empty(),
            Some(tiles) => {
                grid.set(&sort.get().sorted(tiles));
                states.content();
            }
            None => states.loading(None),
        }
    });
    let menu: Rc<RefCell<Option<Rc<ListMenu>>>> = Rc::default();
    let counts = {
        let kept = Rc::clone(&kept);
        move || kept.borrow().iter().map(|(s, t)| (*s, t.len())).collect::<HashMap<_, _>>()
    };
    let list_menu = ListMenu::new({
        let (current, show, menu, counts) =
            (Rc::clone(&current), Rc::clone(&show), Rc::clone(&menu), counts.clone());
        move |segment| {
            current.set(segment);
            if let Some(menu) = menu.borrow().as_ref() {
                menu.show(segment, &counts());
            }
            show();
        }
    });
    menu.replace(Some(Rc::clone(&list_menu)));

    let bar = bar(&scope(ctx, Dest::Library, true), &list_menu.button);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    body.append(&bar);
    states.widget.set_vexpand(true);
    body.append(&states.widget);

    let header = adw::HeaderBar::new();
    header.pack_start(&widgets::sidebar_button(&ctx.split));
    header.pack_end(&order);
    let page = widgets::read_content_page(&t::nav_library(), &header, &body);
    states.on_retry({
        let ask_all = ask_all.clone();
        move || ask_all()
    });

    // Not on screen, the lists wait: the place asks for them each time it shows.
    let shown = OnScreen::new(&page);
    listen(ctx, &page, {
        let (kept, current, show, list_menu) =
            (Rc::clone(&kept), Rc::clone(&current), Rc::clone(&show), Rc::clone(&list_menu));
        let ask_all = ask_all.clone();
        let hub = Rc::downgrade(&ctx.hub);
        let shown = Rc::clone(&shown);
        move |event| match event {
            Event::Reading(ReadingEvent::Library { segment, tiles }) => {
                kept.borrow_mut().insert(*segment, tiles.clone());
                list_menu.show(current.get(), &counts());
                if *segment == current.get() {
                    let show = Rc::clone(&show);
                    shown.run("draw", move || show());
                }
            }
            Event::Medium(_)
                if shown.now() && hub.upgrade().is_some_and(|h| super::reads(&h)) =>
            {
                ask_all()
            }
            _ => {}
        }
    });
    page.connect_map({
        let (ask_all, hub) = (ask_all.clone(), Rc::downgrade(&ctx.hub));
        move |_| {
            if hub.upgrade().is_some_and(|h| super::reads(&h)) {
                ask_all();
            }
        }
    });
    if shown.now() && super::reads(&ctx.hub) {
        ask_all();
    }
    page
}

/// Open the list menu for the shot.
#[cfg(feature = "fixtures")]
pub fn shot_filters(shell: &Rc<crate::window::Shell>) {
    let window = shell.ctx.window.clone();
    glib::timeout_add_local_once(std::time::Duration::from_millis(1500), move || {
        if let Some(button) = find_list(window.upcast_ref()) {
            button.popup();
        }
    });
}

#[cfg(feature = "fixtures")]
fn find_list(widget: &gtk::Widget) -> Option<gtk::MenuButton> {
    if let Some(button) = widget.downcast_ref::<gtk::MenuButton>()
        && widget.is_mapped()
        && button.popover().is_some_and(|p| p.child().is_some_and(|c| c.is::<gtk::ListBox>()))
    {
        return Some(button.clone());
    }
    let mut child = widget.first_child();
    while let Some(next) = child {
        if let Some(found) = find_list(&next) {
            return Some(found);
        }
        child = next.next_sibling();
    }
    None
}

#[cfg(test)]
mod tests {
    use adw::prelude::*;
    use kaeri_i18n::t;

    use super::{ListMenu, bar};
    use crate::gtk_test::{pump, run};

    /// The bar in the smallest window, 350 px inside: the list menu goes under the medium
    /// when both don't fit beside each other, at twice the text size too.
    #[test]
    fn the_bar_fits_a_narrow_window_at_any_text_size() {
        run(|| {
            let toggles = adw::ToggleGroup::new();
            for (name, label) in
                [("shows", t::library_scope_shows()), ("reading", t::library_scope_reading())]
            {
                toggles.add(adw::Toggle::builder().name(name).label(label).build());
            }
            toggles.add_css_class("round");
            let menu = ListMenu::new(|_| {});
            menu.count.set_label("1240");
            let bar = bar(&toggles, &menu.button);
            let window = gtk::Window::builder().default_width(1280).default_height(800).build();
            window.set_child(Some(&bar));
            window.present();
            pump(100);
            let normal = bar.measure(gtk::Orientation::Horizontal, -1).0;
            let settings = gtk::Settings::default().expect("a display");
            let dpi = settings.gtk_xft_dpi();
            settings.set_gtk_xft_dpi(2 * 96 * 1024);
            pump(150);
            let large = bar.measure(gtk::Orientation::Horizontal, -1).0;
            settings.set_gtk_xft_dpi(dpi);
            window.destroy();

            assert!(normal <= 350, "the bar needs {normal} px at the normal text size");
            assert!(large <= 350, "the bar needs {large} px at twice the size");
        });
    }
}
