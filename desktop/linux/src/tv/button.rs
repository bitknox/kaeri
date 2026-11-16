//! The cast button at the end of each page's header bar. Hidden while no TV is found, an
//! outline while one is there, its waves in turn while it connects, filled while connected.
//! Its popover chooses a TV, or says what the TV held plays with Stop casting under it.

use std::{cell::Cell, rc::Rc};

use adw::prelude::*;
use kaeri_engine::cast::{CastButton, SessionState};
use kaeri_i18n::t;

use super::{Tv, cast::Stage, sheet};
use crate::window::Ctx;

/// What the button shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Look {
    Hidden,
    Available,
    Connecting,
    Connected,
}

/// The button's look, from the cast run from here, a TV running Kaeri driven from here, and
/// the engine's session held from the header.
pub fn look(tv: &Tv) -> Look {
    look_of(
        tv.available(),
        tv.cast_view().map(|v| v.stage),
        tv.connected().is_some() && tv.live(),
        tv.cast_session().map(|s| s.button),
        !(tv.rows().is_empty() && tv.cast_rows().is_empty()),
    )
}

fn look_of(
    available: bool,
    cast: Option<Stage>,
    kaeri_live: bool,
    session: Option<CastButton>,
    found: bool,
) -> Look {
    if !available {
        return Look::Hidden;
    }
    match cast {
        Some(Stage::Starting) => return Look::Connecting,
        Some(Stage::Playing) => return Look::Connected,
        // An ended cast keeps its card; the button is free to choose again.
        Some(Stage::Failed | Stage::Lost) => return Look::Available,
        None => {}
    }
    if kaeri_live {
        return Look::Connected;
    }
    match session {
        Some(CastButton::Connecting) => Look::Connecting,
        Some(CastButton::Connected) => Look::Connected,
        _ if found => Look::Available,
        _ => Look::Hidden,
    }
}

/// The glyph for a look: the screen filled inside its frame while a TV is held, else the outline.
fn icon_of(look: Look) -> &'static str {
    match look {
        Look::Connected => "kaeri-cast-connected-symbolic",
        _ => "kaeri-cast-symbolic",
    }
}

/// The name of the TV held, for the tooltip and the popover's heading.
fn held_name(tv: &Tv) -> Option<String> {
    if let Some(view) = tv.cast_view() {
        return Some(view.name);
    }
    if let Some(card) = tv.card().filter(|_| tv.connected().is_some() && tv.live()) {
        return Some(card.tv_name);
    }
    tv.cast_session()?.device.map(|d| d.name)
}

pub fn cast_button(ctx: &Rc<Ctx>) -> gtk::MenuButton {
    let popover = gtk::Popover::new();
    popover.add_css_class("cast-popover");
    let button = gtk::MenuButton::builder()
        .icon_name("kaeri-cast-symbolic")
        .popover(&popover)
        .visible(false)
        .build();
    button.add_css_class("flat");
    button.add_css_class("cast-button");
    // The list is read as the popover opens, and the TVs browsed while it is up.
    popover.connect_show({
        let ctx = Rc::downgrade(ctx);
        move |popover| {
            let Some(ctx) = ctx.upgrade() else { return };
            ctx.tv.start_browsing();
            popover.set_child(Some(&content(&ctx, popover)));
        }
    });
    popover.connect_closed({
        let tv = Rc::downgrade(&ctx.tv);
        move |_| {
            if let Some(tv) = tv.upgrade() {
                tv.stop_browsing();
            }
        }
    });
    let shown = Rc::new(Cell::new(None));
    let draw = {
        let button = button.downgrade();
        let popover = popover.downgrade();
        let tv = Rc::downgrade(&ctx.tv);
        let ctx = Rc::downgrade(ctx);
        move || {
            let (Some(button), Some(tv)) = (button.upgrade(), tv.upgrade()) else { return };
            let look = look(&tv);
            if shown.replace(Some(look)) != Some(look) {
                button.set_visible(look != Look::Hidden);
                button.set_css_classes(&["flat", "cast-button"]);
                match look {
                    Look::Connecting => button.add_css_class("connecting"),
                    Look::Connected => button.add_css_class("connected"),
                    _ => {}
                }
                button.set_icon_name(icon_of(look));
                // What the popover holds follows the look: the chooser, or the TV held.
                if let (Some(popover), Some(ctx)) = (popover.upgrade(), ctx.upgrade())
                    && popover.is_visible()
                {
                    popover.set_child(Some(&content(&ctx, &popover)));
                }
            }
            let name = held_name(&tv);
            let tip = match (look, name) {
                (Look::Connecting, Some(name)) => t::cast_button_connecting(&name),
                (Look::Connected, Some(name)) => t::cast_button_connected(&name),
                _ => t::tv_play_on(),
            };
            button.set_tooltip_text(Some(&tip));
            button.update_property(&[gtk::accessible::Property::Label(&tip)]);
        }
    };
    draw();
    let id = ctx.tv.listen(draw);
    button.connect_destroy({
        let tv = Rc::downgrade(&ctx.tv);
        move |_| {
            if let Some(tv) = tv.upgrade() {
                tv.forget(id);
            }
        }
    });
    button
}

/// The popover's face: the TV held, or the chooser.
fn content(ctx: &Rc<Ctx>, popover: &gtk::Popover) -> gtk::Widget {
    let tv = &ctx.tv;
    let held = matches!(look(tv), Look::Connecting | Look::Connected);
    if held { connected(ctx, popover) } else { chooser(ctx, popover) }
}

fn heading(text: &str) -> gtk::Label {
    let label = gtk::Label::builder().label(text).xalign(0.0).wrap(true).build();
    label.add_css_class("heading");
    label.set_margin_start(4);
    label.set_margin_bottom(4);
    label
}

fn group(text: &str) -> gtk::Label {
    let label = gtk::Label::builder().label(text).xalign(0.0).build();
    label.add_css_class("caption-heading");
    label.add_css_class("haze");
    label.set_margin_start(4);
    label.set_margin_top(8);
    label.set_margin_bottom(4);
    label
}

fn boxed() -> gtk::ListBox {
    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);
    list
}

/// Play on TV: the TVs with Chromecast built in, then the TVs running Kaeri.
fn chooser(ctx: &Rc<Ctx>, popover: &gtk::Popover) -> gtk::Widget {
    let tv = &ctx.tv;
    let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    column.set_width_request(300);
    column.append(&heading(&t::tv_play_on()));
    let casts = tv.cast_rows();
    if !casts.is_empty() {
        column.append(&group(&t::tv_cast_group()));
        let list = boxed();
        for row in &casts {
            list.append(&sheet::cast_row(row));
        }
        list.connect_row_activated({
            let tv = Rc::downgrade(tv);
            let popover = popover.downgrade();
            move |_, row| {
                let Some(tv) = tv.upgrade() else { return };
                let key = row.widget_name();
                if let Some(chosen) = tv.cast_rows().into_iter().find(|r| r.key == key) {
                    if let Some(popover) = popover.upgrade() {
                        popover.popdown();
                    }
                    tv.cast_connect(&chosen);
                }
            }
        });
        column.append(&list);
    }
    let kaeri = tv.rows();
    if !kaeri.is_empty() {
        column.append(&group(&t::tv_network_group()));
        let list = boxed();
        for row in &kaeri {
            list.append(&sheet::tv_row(row, false));
        }
        list.connect_row_activated({
            let tv = Rc::downgrade(tv);
            let popover = popover.downgrade();
            move |_, row| {
                let Some(tv) = tv.upgrade() else { return };
                let key = row.widget_name();
                if let Some(chosen) = tv.rows().into_iter().find(|r| r.key == key) {
                    if let Some(popover) = popover.upgrade() {
                        popover.popdown();
                    }
                    tv.open_picker();
                    tv.choose(&chosen);
                }
            }
        });
        column.append(&list);
    }
    if casts.is_empty() && kaeri.is_empty() {
        let none = gtk::Label::builder().label(t::tv_looking()).xalign(0.0).build();
        none.add_css_class("haze");
        none.set_margin_start(4);
        column.append(&none);
    }
    column.upcast()
}

/// Connected to the TV: what it plays, or that nothing plays yet, then Stop casting.
fn connected(ctx: &Rc<Ctx>, popover: &gtk::Popover) -> gtk::Widget {
    let tv = &ctx.tv;
    let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
    column.set_width_request(320);
    let name = held_name(tv).unwrap_or_default();
    let connecting = look(tv) == Look::Connecting;
    column.append(&heading(&if connecting {
        t::cast_button_connecting(&name)
    } else {
        t::cast_button_connected(&name)
    }));
    let list = boxed();
    let now = tv.cast_view().and_then(|v| v.now).or_else(|| tv.now());
    let idle = tv.cast_session().is_some_and(|s| s.state == SessionState::Connected);
    let row = match now {
        Some(now) => {
            let private = ctx.hub.known.borrow().holds_show(now.id);
            let title = if private { t::now_playing_private() } else { now.title.clone() };
            let line = if private { String::new() } else { now.episode_line.clone() };
            let row = adw::ActionRow::builder()
                .title(title)
                .subtitle(line)
                .use_markup(false)
                .activatable(true)
                .build();
            row.set_tooltip_text(Some(&t::cast_mini_opens()));
            row
        }
        None => {
            let row = adw::ActionRow::builder()
                .title(if idle { t::cast_idle() } else { t::tv_starting() })
                .use_markup(false)
                .build();
            row.add_prefix(&gtk::Image::from_icon_name("kaeri-cast-symbolic"));
            row
        }
    };
    list.append(&row);
    // The row playing opens the controls.
    list.connect_row_activated({
        let tv = Rc::downgrade(tv);
        let popover = popover.downgrade();
        move |_, _| {
            if let Some(popover) = popover.upgrade() {
                popover.popdown();
            }
            if let Some(tv) = tv.upgrade() {
                tv.open_remote();
            }
        }
    });
    column.append(&list);
    let stop = gtk::Button::with_label(&t::cast_stop());
    stop.connect_clicked({
        let tv = Rc::downgrade(tv);
        let popover = popover.downgrade();
        move |_| {
            if let Some(popover) = popover.upgrade() {
                popover.popdown();
            }
            let Some(tv) = tv.upgrade() else { return };
            // A TV running Kaeri lets go of the link; a cast stops.
            if tv.casting() || tv.cast_session().is_some_and(|s| s.device.is_some()) {
                tv.cast_disconnect();
            } else {
                tv.disconnect();
            }
        }
    });
    column.append(&stop);
    column.upcast()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_button_hides_without_a_tv_and_fills_while_connected() {
        assert_eq!(look_of(false, Some(Stage::Playing), true, None, true), Look::Hidden);
        assert_eq!(look_of(true, None, false, None, false), Look::Hidden);
        assert_eq!(look_of(true, None, false, Some(CastButton::Hidden), true), Look::Available);
        assert_eq!(
            look_of(true, None, false, Some(CastButton::Connecting), false),
            Look::Connecting
        );
        assert_eq!(
            look_of(true, None, false, Some(CastButton::Connected), false),
            Look::Connected
        );
        assert_eq!(look_of(true, None, true, None, false), Look::Connected);
    }

    #[test]
    fn only_a_held_tv_draws_the_filled_screen() {
        assert_eq!(icon_of(Look::Connected), "kaeri-cast-connected-symbolic");
        for look in [Look::Hidden, Look::Available, Look::Connecting] {
            assert_eq!(icon_of(look), "kaeri-cast-symbolic");
        }
    }

    #[test]
    fn a_cast_from_here_leads_and_an_ended_one_frees_the_button() {
        assert_eq!(look_of(true, Some(Stage::Starting), false, None, true), Look::Connecting);
        assert_eq!(look_of(true, Some(Stage::Playing), false, None, true), Look::Connected);
        let gone = Some(CastButton::Connected);
        assert_eq!(look_of(true, Some(Stage::Lost), false, gone, true), Look::Available);
        assert_eq!(look_of(true, Some(Stage::Failed), false, None, true), Look::Available);
    }
}
