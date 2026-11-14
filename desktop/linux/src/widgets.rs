//! Pieces the screens share: covers and banners with their flat placeholders, labels in the
//! app's type, and the poster grid.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use adw::prelude::*;
use gtk::{gdk, gio, glib};
use kaeri_model::catalog::Entry;

use kaeri_i18n::t;

use crate::{fit::Fit, hub::Hub, say};

/// The raised colour, which placeholders stand on.
const RAISED: [u8; 3] = [0x22, 0x26, 0x38];

/// A colour as one pixel, stretched over a placeholder: the cover's own colour at half strength
/// over raised while its art is on the way, and raised when the title has none.
pub fn swatch(hex: Option<&str>) -> gdk::Texture {
    let rgb = hex.and_then(parse_hex).map_or(RAISED, half_over_raised);
    gdk::MemoryTexture::new(
        1,
        1,
        gdk::MemoryFormat::R8g8b8,
        &glib::Bytes::from_owned(rgb.to_vec()),
        3,
    )
    .upcast()
}

/// `rgb` at 50% over raised.
fn half_over_raised(rgb: [u8; 3]) -> [u8; 3] {
    let mix = |at: usize| ((u16::from(rgb[at]) + u16::from(RAISED[at])) / 2) as u8;
    [mix(0), mix(1), mix(2)]
}

fn parse_hex(hex: &str) -> Option<[u8; 3]> {
    let hex = hex.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let byte = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?])
}

/// Art in a rounded box: `url`'s once it is here, fading in over a placeholder in `color`.
/// Give a progress bar a name and a spoken percent, kept in step with its fraction.
pub fn name_progress(bar: &gtk::ProgressBar) {
    let say = |bar: &gtk::ProgressBar| {
        bar.update_property(&[gtk::accessible::Property::ValueText(&t::progress_percent(
            (bar.fraction() * 100.0).round() as i64,
        ))]);
    };
    bar.update_property(&[gtk::accessible::Property::Label(&t::progress_name())]);
    say(bar);
    bar.connect_fraction_notify(say);
}

pub fn art(
    hub: &Hub,
    url: Option<&str>,
    color: Option<&str>,
    class: &str,
) -> (gtk::Box, gtk::Picture) {
    let plate = gtk::Picture::builder()
        .paintable(&swatch(color))
        .accessible_role(gtk::AccessibleRole::Presentation)
        .build();
    plate.set_content_fit(gtk::ContentFit::Fill);
    plate.set_can_shrink(true);
    plate.set_hexpand(true);
    plate.set_vexpand(true);
    let picture =
        gtk::Picture::builder().accessible_role(gtk::AccessibleRole::Presentation).build();
    picture.set_content_fit(gtk::ContentFit::Cover);
    picture.set_can_shrink(true);
    hub.art(&picture, url);
    let layers = gtk::Overlay::new();
    layers.set_child(Some(&plate));
    layers.add_overlay(&picture);
    let frame = gtk::Box::new(gtk::Orientation::Vertical, 0);
    frame.set_overflow(gtk::Overflow::Hidden);
    frame.add_css_class(class);
    frame.append(&layers);
    (frame, picture)
}

/// A title's cover at a fixed size, named for screen readers by its show.
pub fn cover(hub: &Hub, entry: &Entry, width: i32, height: i32) -> Fit {
    let (frame, _) =
        art(hub, entry.cover_url.as_deref(), entry.cover_color.as_deref(), "cover");
    if width <= 40 {
        frame.add_css_class("small");
    }
    let fit = Fit::new(&frame, f64::from(width) / f64::from(height.max(1)), width, true, 0);
    fit.set_halign(gtk::Align::Center);
    fit.set_valign(gtk::Align::Center);
    fit.update_property(&[gtk::accessible::Property::Label(&entry.title)]);
    fit
}

/// A cover that takes the width it is given, at 2:3, asking `width`.
pub fn cover_fill(hub: &Hub, entry: &Entry, width: i32) -> Fit {
    let (frame, _) =
        art(hub, entry.cover_url.as_deref(), entry.cover_color.as_deref(), "cover");
    let fit = Fit::new(&frame, 2.0 / 3.0, width, false, 0);
    fit.set_valign(gtk::Align::Start);
    fit.update_property(&[gtk::accessible::Property::Label(&entry.title)]);
    fit
}

/// A banner at 16:9 of the width it is given, cropped to at most `max_height` high.
pub fn banner(hub: &Hub, entry: &Entry, max_height: i32, class: &str) -> Fit {
    let url = entry.banner_url.as_deref().or(entry.cover_url.as_deref());
    let (frame, _) = art(hub, url, entry.cover_color.as_deref(), "banner");
    frame.add_css_class(class);
    Fit::new(&frame, 16.0 / 9.0, max_height * 16 / 9, false, max_height)
}

/// A page's banner, flush to the edges, fading into the ground at its foot so a page's head
/// can stand over its lower part. The fade is drawn with the art, under the same clip.
pub fn faded_banner(hub: &Hub, entry: &Entry, max_height: i32) -> Fit {
    let url = entry.banner_url.as_deref().or(entry.cover_url.as_deref());
    let (frame, picture) = art(hub, url, entry.cover_color.as_deref(), "banner");
    frame.add_css_class("flush");
    if let Some(layers) = picture.parent().and_downcast::<gtk::Overlay>() {
        let fade = gtk::Box::new(gtk::Orientation::Vertical, 0);
        fade.add_css_class("art-fade");
        fade.set_can_target(false);
        layers.add_overlay(&fade);
        // The art and its plate stop short of the edge, so a fractional allocation can't
        // show their last row as a seam below the fade.
        picture.set_margin_bottom(2);
        if let Some(plate) = layers.child() {
            plate.set_margin_bottom(2);
        }
    }
    Fit::new(&frame, 16.0 / 9.0, max_height * 16 / 9, false, max_height)
}

/// An episode's still at a fixed size, 16:9.
pub fn still(hub: &Hub, url: Option<&str>, color: Option<&str>, width: i32) -> (Fit, gtk::Box) {
    let (frame, _) = art(hub, url, color, "still");
    let fit = Fit::new(&frame, 16.0 / 9.0, width, true, 0);
    fit.set_valign(gtk::Align::Center);
    (fit, frame)
}

/// A show's title in Zen Kaku Gothic New.
pub fn show_title(text: &str, size: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("show-title");
    label.add_css_class(size);
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    label
}

/// A line of secondary text.
pub fn haze(text: &str, class: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("haze");
    if !class.is_empty() {
        label.add_css_class(class);
    }
    label.set_xalign(0.0);
    label.set_wrap(true);
    label
}

/// Sets markup with `<a>` links on a label and makes the links' accessibles as soon as the label
/// is in a window.
///
/// GTK makes a label's link accessibles on first use. When a screen reader's walk is that first
/// use, they are made while GTK registers the label, and each link registers the label a second
/// time under a new path. The first path stays in GTK's cache after the label goes, and the next
/// walk reads the freed label and aborts. Made here, before the label is registered, the links
/// register it once. Not before the label is in a window: making them registers the label, and a
/// registered label must not be given its role, which GTK does when it joins a window.
pub fn set_link_markup(label: &gtk::Label, markup: &str) {
    label.set_markup(markup);
    if label.root().is_some() {
        let _ = label.first_accessible_child();
        return;
    }
    let handler = Rc::new(Cell::new(None));
    let id = label.connect_notify_local(Some("root"), {
        let handler = Rc::clone(&handler);
        move |label, _| {
            if label.root().is_some() {
                let _ = label.first_accessible_child();
                if let Some(id) = handler.take() {
                    label.disconnect(id);
                }
            }
        }
    });
    handler.set(Some(id));
}

/// A heading over a section of a page, in sentence case: "New this week".
pub fn heading(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("title-4");
    label.set_xalign(0.0);
    label.set_accessible_role(gtk::AccessibleRole::Heading);
    label
}

/// The button at the start of a content header bar that shows the sidebar while it is
/// collapsed into an overlay.
pub fn sidebar_button(split: &adw::OverlaySplitView) -> gtk::ToggleButton {
    let button = gtk::ToggleButton::new();
    button.set_icon_name("sidebar-show-symbolic");
    button.set_tooltip_text(Some(&t::sidebar_show()));
    split.bind_property("collapsed", &button, "visible").sync_create().build();
    split
        .bind_property("show-sidebar", &button, "active")
        .sync_create()
        .bidirectional()
        .build();
    button
}

thread_local! {
    /// The offline banners on screen, and whether they show.
    static BANNERS: RefCell<Vec<glib::WeakRef<adw::Banner>>> = RefCell::default();
    static OFFLINE: Cell<bool> = const { Cell::new(false) };
}

/// The banner under a page's header bar that says the device is offline: shown on every page
/// that has one while it is, and gone once the network is back.
pub fn offline_banner() -> adw::Banner {
    banner_saying(&t::state_offline_line())
}

/// The offline banner on Read's pages, which speaks of chapters.
pub fn read_offline_banner() -> adw::Banner {
    banner_saying(&t::read_state_offline_line())
}

fn banner_saying(line: &str) -> adw::Banner {
    let banner = adw::Banner::new(line);
    banner.add_css_class("offline");
    banner.set_revealed(OFFLINE.get());
    BANNERS.with_borrow_mut(|banners| {
        banners.retain(|b| b.upgrade().is_some());
        banners.push(banner.downgrade());
    });
    banner
}

/// Whether the device is offline, as the engine last said.
pub fn is_offline() -> bool {
    OFFLINE.get()
}

/// Show or hide every offline banner.
pub fn set_offline(offline: bool) {
    OFFLINE.set(offline);
    BANNERS.with_borrow_mut(|banners| {
        banners.retain(|b| b.upgrade().is_some());
        for banner in banners.iter().filter_map(glib::WeakRef::upgrade) {
            banner.set_revealed(offline);
        }
    });
}

/// A destination's page: its header bar and the offline banner over `content`.
pub fn content_page(
    title: &str,
    header: &adw::HeaderBar,
    content: &impl IsA<gtk::Widget>,
) -> adw::NavigationPage {
    page_with_banner(title, header, &offline_banner(), content)
}

/// A page of Read: its header bar and Read's offline banner over `content`.
pub fn read_content_page(
    title: &str,
    header: &adw::HeaderBar,
    content: &impl IsA<gtk::Widget>,
) -> adw::NavigationPage {
    page_with_banner(title, header, &read_offline_banner(), content)
}

fn page_with_banner(
    title: &str,
    header: &adw::HeaderBar,
    banner: &adw::Banner,
    content: &impl IsA<gtk::Widget>,
) -> adw::NavigationPage {
    let view = adw::ToolbarView::new();
    view.add_top_bar(header);
    view.add_top_bar(banner);
    view.set_content(Some(content));
    adw::NavigationPage::builder().title(title).child(&view).build()
}

/// A page of a destination: its header bar over `content`.
pub fn page(
    title: &str,
    header: &adw::HeaderBar,
    content: &impl IsA<gtk::Widget>,
) -> adw::NavigationPage {
    let view = adw::ToolbarView::new();
    view.add_top_bar(header);
    view.set_content(Some(content));
    adw::NavigationPage::builder().title(title).child(&view).build()
}

/// How a row stands, beside its name: a mark and a word or two. `mark` is a dot's colour,
/// "water", "vermilion" or "haze", or "ring" for something switched off or held back.
pub fn standing(text: &str, mark: &str) -> gtk::Box {
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    line.set_valign(gtk::Align::Center);
    let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    dot.add_css_class("mark");
    dot.add_css_class(mark);
    dot.set_valign(gtk::Align::Center);
    line.append(&dot);
    if !text.is_empty() {
        let label = gtk::Label::new(Some(text));
        label.add_css_class(if matches!(mark, "haze" | "ring") { "haze" } else { "standing" });
        // A long standing, as a TV's "pair again", wraps rather than widening its row.
        label.set_xalign(0.0);
        label.set_wrap(true);
        label.set_natural_wrap_mode(gtk::NaturalWrapMode::Word);
        line.append(&label);
    }
    line
}

/// An empty boxed list, nothing selectable.
pub fn boxed_list() -> gtk::ListBox {
    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);
    list
}

/// A column of groups, clamped to `max` and scrolling down.
pub fn column_page(column: &gtk::Box, max: i32) -> gtk::ScrolledWindow {
    column.set_margin_top(18);
    column.set_margin_bottom(24);
    column.set_margin_start(12);
    column.set_margin_end(12);
    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&adw::Clamp::builder().maximum_size(max).child(column).build())
        .build()
}

/// A menu item for `action` with a string target.
pub fn menu_item(label: &str, action: &str, target: &str) -> gio::MenuItem {
    let item = gio::MenuItem::new(Some(label), None);
    item.set_action_and_target_value(Some(action), Some(&target.to_variant()));
    item
}

/// Mark the items for `action` in `button`'s menu with `class`, once the menu is built: the
/// Episodes and Sources lists check their pick in amber, and every other menu in silk.
pub fn mark_items(button: &gtk::MenuButton, action: &str, class: &str) {
    fn walk(widget: &gtk::Widget, action: &str, class: &str) {
        if let Some(item) = widget.dynamic_cast_ref::<gtk::Actionable>()
            && item.action_name().is_some_and(|name| name == action)
        {
            widget.add_css_class(class);
        }
        let mut child = widget.first_child();
        while let Some(next) = child {
            walk(&next, action, class);
            child = next.next_sibling();
        }
    }
    if let Some(popover) = button.popover() {
        walk(popover.upcast_ref(), action, class);
    }
}

/// A row's chevron: it opens a page.
pub fn chevron() -> gtk::Image {
    let image = gtk::Image::from_icon_name("go-next-symbolic");
    image.add_css_class("dim-label");
    image
}

/// Keep `value` as long as `widget` lives: a screen's state, which its handlers hold weakly.
pub fn keep<T: 'static>(widget: &impl IsA<gtk::Widget>, value: T) {
    let value = std::cell::RefCell::new(Some(value));
    widget.as_ref().connect_destroy(move |_| drop(value.borrow_mut().take()));
}

/// Present a dialog with nothing to lose (Light dismiss, native-ux.md): a click on the dimmed
/// window around it closes it, as Esc does. libadwaita's bottom sheet closes on a click on its
/// dimming; this gives the floating dialog the same. A dialog that holds input never takes it.
pub fn present_light_dismiss(dialog: &impl IsA<adw::Dialog>, parent: &impl IsA<gtk::Widget>) {
    let dialog = dialog.as_ref().clone();
    dialog.present(Some(parent));
    let Some(window) = dialog.root().map(|root| root.upcast::<gtk::Widget>()) else {
        return;
    };
    let click = gtk::GestureClick::new();
    click.set_propagation_phase(gtk::PropagationPhase::Capture);
    click.connect_pressed({
        let dialog = dialog.downgrade();
        move |gesture, _, x, y| {
            let (Some(dialog), Some(window)) = (dialog.upgrade(), gesture.widget()) else {
                return;
            };
            // The dialog's own content, not the dialog: the dialog widget spans its dimming too.
            let Some(card) = dialog.child().and_then(|child| child.compute_bounds(&window))
            else {
                return;
            };
            if !card.contains_point(&gtk::graphene::Point::new(x as f32, y as f32)) {
                dialog.close();
            }
        }
    });
    window.add_controller(click.clone());
    let window = window.downgrade();
    dialog.connect_closed(move |_| {
        if let Some(window) = window.upgrade() {
            window.remove_controller(&click);
        }
    });
}

/// A title in a grid, kept in the grid's model.
pub type Item = glib::BoxedAnyObject;

/// A poster grid: covers 140 to 180 wide, as many columns as fit, the show title in Zen Kaku and
/// one caption line under each. A cover opens its title; right-click offers the title's menu.
pub struct PosterGrid {
    pub widget: gtk::GridView,
    store: gio::ListStore,
}

/// What a poster says under its title.
#[derive(Clone, Copy)]
enum Lines {
    /// Search and the rails: the count first, then the format and where it plays.
    Cell,
    /// One line, as Library says how far along a title is.
    One(fn(&Entry) -> String),
}

impl PosterGrid {
    pub fn new(hub: &Rc<Hub>) -> Self {
        Self::build(hub, Lines::Cell)
    }

    /// A grid whose caption line under each cover is `caption`'s: Library's says how far.
    pub fn with_caption(hub: &Rc<Hub>, caption: fn(&Entry) -> String) -> Self {
        Self::build(hub, Lines::One(caption))
    }

    fn build(hub: &Rc<Hub>, caption: Lines) -> Self {
        let store = gio::ListStore::new::<Item>();
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_bind({
            let hub = Rc::clone(hub);
            move |_, item| {
                let Some(item) = item.downcast_ref::<gtk::ListItem>() else { return };
                let Some(object) = item.item().and_downcast::<Item>() else { return };
                let entry = object.borrow::<Entry>();
                item.set_child(Some(&poster(&hub, &entry, caption)));
            }
        });
        factory.connect_unbind(|_, item| {
            if let Some(item) = item.downcast_ref::<gtk::ListItem>() {
                item.set_child(gtk::Widget::NONE);
            }
        });
        let selection = gtk::NoSelection::new(Some(store.clone()));
        let widget = gtk::GridView::new(Some(selection), Some(factory));
        widget.set_max_columns(12);
        widget.set_min_columns(2);
        widget.set_single_click_activate(true);
        widget.add_css_class("posters");
        widget.update_property(&[gtk::accessible::Property::Label(&t::poster_grid_name())]);
        widget.set_vexpand(true);
        widget.connect_activate({
            let store = store.clone();
            move |grid, position| {
                if let Some(object) = store.item(position).and_downcast::<Item>() {
                    let id = object.borrow::<Entry>().id.get();
                    let _ = grid.activate_action("win.open-title", Some(&id.to_variant()));
                }
            }
        });
        // Focus sits on the grid's own cell, around the poster: the menu keys open the
        // focused poster's menu from here.
        let keys = gtk::ShortcutController::new();
        keys.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<Shift>F10|Menu"),
            Some(gtk::CallbackAction::new(|grid, _| {
                let poster = grid.focus_child().and_then(|cell| cell.first_child());
                match poster {
                    Some(poster) => {
                        let _ = poster.activate_action("context.popup", None);
                        glib::Propagation::Stop
                    }
                    None => glib::Propagation::Proceed,
                }
            })),
        ));
        widget.add_controller(keys);
        Self { widget, store }
    }

    pub fn set(&self, entries: &[Entry]) {
        let items: Vec<Item> = entries.iter().cloned().map(Item::new).collect();
        self.store.splice(0, self.store.n_items(), &items);
    }
}

fn poster(hub: &Rc<Hub>, entry: &Entry, lines: Lines) -> gtk::Box {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 6);
    column.set_size_request(120, -1);
    column.append(&cover_fill(hub, entry, 148));
    let title = show_title(&entry.title, "row");
    title.set_lines(2);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title.set_tooltip_text(Some(&entry.title));
    title.set_valign(gtk::Align::Start);
    column.append(&title);
    match lines {
        Lines::One(caption) => {
            let line = haze(&caption(entry), "caption");
            line.set_wrap(false);
            line.set_ellipsize(gtk::pango::EllipsizeMode::End);
            column.append(&line);
            if let Some(at) = &entry.available_at {
                let line = haze(&crate::say::available_line(at), "caption");
                line.set_wrap(false);
                line.set_ellipsize(gtk::pango::EllipsizeMode::End);
                column.append(&line);
            }
            column.update_property(&[gtk::accessible::Property::Label(&entry.title)]);
        }
        Lines::Cell => {
            let mut said = vec![entry.title.clone()];
            if let Some(count) = &entry.count_line {
                // The short form until the cell is on screen and its text can be measured: then the
                // long one when it fits, so the dub part is never cut off.
                let line = haze(&count.short, "caption");
                line.set_wrap(false);
                line.set_ellipsize(gtk::pango::EllipsizeMode::End);
                line.add_css_class("count-line");
                line.connect_realize({
                    let count = count.clone();
                    move |line| {
                        let room = 148;
                        let text = say::count_text(&count, room, |text| {
                            line.create_pango_layout(Some(text)).pixel_size().0
                        });
                        line.set_label(&text);
                    }
                });
                column.append(&line);
                said.push(count.spoken.clone());
            }
            let detail = say::cell_detail(entry);
            if !detail.is_empty() {
                let line = haze(&detail, "caption");
                line.set_wrap(false);
                line.set_ellipsize(gtk::pango::EllipsizeMode::End);
                column.append(&line);
                said.push(detail);
            }
            column.update_property(&[gtk::accessible::Property::Label(&said.join(", "))]);
        }
    }
    title_menu(&column, entry.id.get(), true);
    crate::trailer::hover_preview(hub, &column, entry.id.get());
    column
}

/// The menu a title's cover or row opens on right-click, Shift+F10 and the Menu key.
pub fn title_menu(widget: &impl IsA<gtk::Widget>, id: u32, planning: bool) {
    let menu = gio::Menu::new();
    let action = |name: &str| format!("win.{name}(uint32 {id})"); // i18n-skip
    if planning {
        menu.append(Some(&t::library_add_planning()), Some(&action("plan"))); // i18n-skip
    }
    menu.append(Some(&t::title_open()), Some(&action("open-title"))); // i18n-skip
    menu.append(Some(&t::title_open_anilist()), Some(&action("open-anilist"))); // i18n-skip
    context_menu(widget, &menu);
}

/// `menu` as `widget`'s context menu.
pub fn context_menu(widget: &impl IsA<gtk::Widget>, menu: &gio::Menu) {
    let widget = widget.as_ref();
    let popover = gtk::PopoverMenu::from_model(Some(menu));
    popover.set_parent(widget);
    popover.set_has_arrow(false);
    popover.set_halign(gtk::Align::Start);
    let click = gtk::GestureClick::new();
    click.set_button(gdk::BUTTON_SECONDARY);
    click.connect_pressed({
        let popover = popover.clone();
        move |_, _, x, y| {
            popover.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
            popover.popup();
        }
    });
    widget.add_controller(click);
    let press = gtk::GestureLongPress::new();
    press.set_touch_only(true);
    press.connect_pressed({
        let popover = popover.clone();
        move |_, x, y| {
            popover.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
            popover.popup();
        }
    });
    widget.add_controller(press);
    let keys = gtk::ShortcutController::new();
    keys.add_shortcut(gtk::Shortcut::new(
        gtk::ShortcutTrigger::parse_string("<Shift>F10|Menu"),
        Some(gtk::CallbackAction::new({
            let popover = popover.clone();
            move |_, _| {
                popover.set_pointing_to(None);
                popover.popup();
                glib::Propagation::Stop
            }
        })),
    ));
    widget.add_controller(keys);
    // The same from outside, for a container whose focus sits around the widget: a grid's cell.
    let popup = gio::SimpleAction::new("popup", None);
    popup.connect_activate({
        let popover = popover.clone();
        move |_, _| {
            popover.set_pointing_to(None);
            popover.popup();
        }
    });
    let group = gio::SimpleActionGroup::new();
    group.add_action(&popup);
    widget.insert_action_group("context", Some(&group));
    // The popover goes with its widget.
    widget.connect_destroy(move |_| popover.unparent());
}

/// Surprise me, or Another: a flat header bar button with its word. It asks the engine for a pick
/// from `pool` and opens it (or swaps the page on screen for it when `swap`; a book from reading's
/// filters opens as a series, with no Another), showing a spinner in
/// place of the glyph once the pick takes more than 600 ms. Pressed again while it waits, it does
/// nothing. An empty pool or no connection is said by the engine's own notice.
pub fn surprise_button(
    ctx: &Rc<crate::window::Ctx>,
    label: &str,
    hint: &str,
    swap: bool,
    pool: impl Fn() -> Option<kaeri_model::browsing::SurprisePool> + 'static,
) -> gtk::Button {
    use kaeri_model::{browsing::SurpriseOutcome, command::Command, event::Event};
    let (face, icon, words) = surprise_face(label);
    let button = gtk::Button::builder().child(&face).tooltip_text(hint).build();
    button.add_css_class("flat");
    ctx.on_size({
        let words = words.downgrade();
        move |size| {
            if let Some(words) = words.upgrade() {
                lay_surprise(&words, size);
            }
        }
    });
    button.update_property(&[
        gtk::accessible::Property::Label(label),
        gtk::accessible::Property::Description(hint),
    ]);
    let asked: Rc<RefCell<Option<kaeri_model::browsing::SurprisePool>>> = Rc::default();
    let slow: Rc<RefCell<Option<glib::SourceId>>> = Rc::default();
    button.connect_clicked({
        let (ctx, asked, slow, icon) =
            (Rc::clone(ctx), asked.clone(), slow.clone(), icon.clone());
        move |_| {
            if asked.borrow().is_some() {
                return;
            }
            let Some(pool) = pool() else { return };
            asked.replace(Some(pool.clone()));
            let icon = icon.clone();
            let id = glib::timeout_add_local_once(
                std::time::Duration::from_millis(600),
                move || {
                    icon.set_visible_child_name("wait");
                },
            );
            slow.replace(Some(id));
            ctx.hub.send(Command::Surprise(pool));
        }
    });
    let hub_ctx = Rc::downgrade(ctx);
    let weak = button.downgrade();
    ctx.hub.listen(move |event| {
        let Event::Surprise { pool, outcome } = event else { return };
        let (Some(ctx), Some(button)) = (hub_ctx.upgrade(), weak.upgrade()) else { return };
        if asked.borrow().as_ref() != Some(pool) {
            return;
        }
        asked.replace(None);
        if let Some(id) = slow.take() {
            id.remove();
        }
        icon.set_visible_child_name("dice");
        if let SurpriseOutcome::Picked { id, .. } = outcome
            && matches!(pool, kaeri_model::browsing::SurprisePool::Reading { .. })
        {
            ctx.pick.replace(None);
            let _ = button
                .activate_action("win.open-series", Some(&format!("a:{id}").to_variant()));
        } else if let SurpriseOutcome::Picked { id, more } = outcome {
            ctx.pick.replace(Some(crate::window::Pick {
                id: *id,
                pool: pool.clone(),
                more: *more,
            }));
            let _ = button.activate_action(
                if swap { "win.swap-title" } else { "win.open-title" },
                Some(&id.get().to_variant()),
            );
        }
    });
    button
}

/// Surprise me's face: the dice, or a spinner while it picks, and its name.
fn surprise_face(label: &str) -> (gtk::Box, gtk::Stack, gtk::Label) {
    let icon = gtk::Stack::new();
    icon.add_named(&gtk::Image::from_icon_name("kaeri-dice-symbolic"), Some("dice"));
    icon.add_named(&adw::Spinner::new(), Some("wait"));
    let face = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    face.append(&icon);
    let words = gtk::Label::new(Some(label));
    face.append(&words);
    (face, icon, words)
}

/// A narrow window's header has room for the dice alone; the name stays in the tooltip and for
/// screen readers.
fn lay_surprise(words: &gtk::Label, size: crate::window::Size) {
    words.set_visible(size != crate::window::Size::Narrow);
}

/// A ring of how far something has come, 16 px: a silk arc on the rule, with a stop square
/// inside when it can be stopped.
pub fn ring(fraction: f64) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.set_content_width(16);
    area.set_content_height(16);
    area.set_valign(gtk::Align::Center);
    area.set_margin_end(4);
    area.set_draw_func(move |area, cr, width, height| {
        let color = area.color();
        let (r, g, b) =
            (f64::from(color.red()), f64::from(color.green()), f64::from(color.blue()));
        let (cx, cy) = (f64::from(width) / 2.0, f64::from(height) / 2.0);
        let radius = cx.min(cy) - 1.0;
        cr.set_line_width(2.0);
        cr.set_source_rgba(r, g, b, 0.22);
        cr.arc(cx, cy, radius, 0.0, std::f64::consts::TAU);
        let _ = cr.stroke();
        cr.set_source_rgb(r, g, b);
        let start = -std::f64::consts::FRAC_PI_2;
        cr.arc(cx, cy, radius, start, start + std::f64::consts::TAU * fraction.clamp(0.0, 1.0));
        let _ = cr.stroke();
        cr.rectangle(cx - 2.5, cy - 2.5, 5.0, 5.0);
        let _ = cr.fill();
    });
    area
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        gtk_test::{pump, run},
        window::Size,
    };

    /// Search's header in a 360 px window: the sidebar button, the title, the field's toggle,
    /// Surprise me and the close button fit the 350 px the content has.
    #[test]
    fn the_search_header_fits_a_narrow_window() {
        run(|| {
            let header = adw::HeaderBar::new();
            header.set_decoration_layout(Some(":close"));
            header.pack_start(&gtk::Button::from_icon_name("sidebar-show-symbolic"));
            header.set_title_widget(Some(&adw::WindowTitle::new("Search", "")));
            header.pack_end(
                &gtk::ToggleButton::builder().icon_name("edit-find-symbolic").build(),
            );
            let (face, _, words) = surprise_face("Surprise me");
            header.pack_end(&gtk::Button::builder().child(&face).css_classes(["flat"]).build());
            let window = adw::Window::builder().default_width(1280).default_height(800).build();
            let view = adw::ToolbarView::new();
            view.add_top_bar(&header);
            window.set_content(Some(&view));
            window.present();
            let min_width = || header.measure(gtk::Orientation::Horizontal, -1).0;

            lay_surprise(&words, Size::Wide);
            pump(50);
            let wide = min_width();
            lay_surprise(&words, Size::Narrow);
            pump(50);
            let narrow = min_width();
            assert!(narrow < wide, "the name still takes room: {narrow} px against {wide}");
            assert!(narrow <= 330, "the header needs {narrow} px");
            window.close();
        });
    }

    #[test]
    fn cover_colours_read_from_anilists_hex() {
        assert_eq!(parse_hex("#e4a15d"), Some([0xE4, 0xA1, 0x5D]));
        assert_eq!(parse_hex("e4a15d"), Some([0xE4, 0xA1, 0x5D]));
        assert_eq!(parse_hex("#fff"), None);
        assert_eq!(parse_hex("#zzzzzz"), None);
    }

    #[test]
    fn a_placeholder_is_its_cover_colour_at_half_over_raised() {
        assert_eq!(half_over_raised([0xFF, 0xFF, 0xFF]), [0x90, 0x92, 0x9B]);
        assert_eq!(half_over_raised(RAISED), RAISED);
    }

    /// Read's pages speak of chapters offline, the shows' pages of episodes, and both follow
    /// the network.
    #[test]
    fn reads_offline_banner_speaks_of_chapters() {
        crate::gtk_test::run(|| {
            let shows = offline_banner();
            let read = read_offline_banner();
            assert_eq!(shows.title(), t::state_offline_line());
            assert_eq!(read.title(), t::read_state_offline_line());
            set_offline(true);
            assert!(shows.is_revealed() && read.is_revealed());
            set_offline(false);
            assert!(!shows.is_revealed() && !read.is_revealed());
        });
    }
}

#[cfg(test)]
mod link_markup_tests {
    use std::path::Path;

    /// Every `.rs` file under `dir`, with its text.
    fn sources(dir: &Path, out: &mut Vec<(String, String)>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                sources(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push((path.display().to_string(), std::fs::read_to_string(&path).unwrap()));
            }
        }
    }

    /// The names that call `method` in `text`: `label` for `label.method(`.
    fn callers(text: &str, method: &str) -> Vec<String> {
        text.match_indices(method)
            .map(|(at, _)| {
                let before = &text[..at];
                let start = before
                    .rfind(|c: char| !(c.is_alphanumeric() || c == '_'))
                    .map_or(0, |i| i + 1);
                before[start..].to_string()
            })
            .filter(|name| !name.is_empty())
            .collect()
    }

    /// A label whose links a screen reader can reach gets its markup through `set_link_markup`,
    /// so a walk never makes the links while GTK registers the label (R9L-1).
    #[test]
    fn link_labels_set_their_markup_through_the_helper() {
        let (link, markup) =
            ([".connect", "_activate_link("].concat(), [".set", "_markup("].concat());
        let mut files = Vec::new();
        sources(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
        let mut direct = Vec::new();
        for (path, text) in &files {
            let mut function = String::new();
            for line in text.lines().chain(["fn end"]) {
                let head = line
                    .trim_start()
                    .trim_start_matches("pub(crate) ")
                    .trim_start_matches("pub ");
                if head.starts_with("fn ") {
                    for name in callers(&function, &link) {
                        if function.contains(&format!("{name}{markup}")) {
                            direct.push(format!("{path}: {name}"));
                        }
                    }
                    function.clear();
                }
                function.push_str(line);
                function.push('\n');
            }
        }
        assert!(direct.is_empty(), "link labels set without set_link_markup: {direct:?}");
    }
}
