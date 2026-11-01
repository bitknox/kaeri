//! A plugin store owner's tools, from the store page's menu, as the Mac has them: an invite link for
//! someone new or for another device of yours, the members and the invites not used yet, who can
//! join (only people invited, or anyone with the store's link), and the plugins on this computer
//! to publish there. Each a dialog, which a narrow window shows as a bottom sheet. The engine
//! talks to the store; these only ask it and show what it answers.

use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    rc::Rc,
    time::Duration,
};

use adw::prelude::*;
use gtk::glib;
use kaeri_i18n::t;
use kaeri_model::{
    command::Command,
    event::Event,
    stores::{
        InviteEndReason, InviteLasts, InviteLinkView, InviteUses, LocalPluginRow, OpenJoinView,
        StoreInviteRow, StoreMemberRow,
    },
};

use crate::{widgets, window::Ctx};

/// How long Who Can Join waits, once the store has listed its members, before taking it for a
/// server too old to offer open joining.
const OLDER_SERVER: Duration = Duration::from_secs(4);

/// How long a dialog waits on the store before saying it did not answer.
const NO_ANSWER: Duration = Duration::from_secs(20);

/// A dialog's frame: the header with its buttons, and a column to fill.
struct Frame {
    dialog: adw::Dialog,
    header: adw::HeaderBar,
    body: gtk::Box,
}

/// `title` short, over the store's name, so a narrow window's sheet shows both whole.
fn frame(ctx: &Ctx, name: &str, title: &str, store: &str) -> Frame {
    if let Some(dialog) = ctx.window.visible_dialog()
        && dialog.widget_name() == name
    {
        dialog.force_close();
    }
    let header = adw::HeaderBar::new();
    header.set_show_end_title_buttons(false);
    header.set_show_start_title_buttons(false);
    header.set_title_widget(Some(&adw::WindowTitle::new(title, store)));
    let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
    body.set_margin_top(12);
    body.set_margin_bottom(24);
    body.set_margin_start(18);
    body.set_margin_end(18);
    if ctx.size() != crate::window::Size::Narrow {
        body.set_size_request(460, -1);
    }
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .max_content_height(620)
        .child(&body)
        .build();
    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.set_content(Some(&scroll));
    let dialog = adw::Dialog::builder()
        .name(name)
        .title(t::stores_dialog_title(title, store))
        .follows_content_size(true)
        .child(&view)
        .build();
    Frame { dialog, header, body }
}

/// A Done button at the header's end, which closes the dialog.
fn done(frame: &Frame) -> gtk::Button {
    let done = gtk::Button::with_label(&t::action_done());
    let dialog = frame.dialog.downgrade();
    done.connect_clicked(move |_| {
        if let Some(dialog) = dialog.upgrade() {
            dialog.close();
        }
    });
    frame.header.pack_end(&done);
    frame.dialog.set_default_widget(Some(&done));
    done
}

/// Hear the engine while the dialog is up.
fn listen(ctx: &Ctx, dialog: &adw::Dialog, heard: impl Fn(&Event) + 'static) {
    let listening = ctx.hub.listen(heard);
    let hub = Rc::downgrade(&ctx.hub);
    dialog.connect_closed(move |_| {
        if let Some(hub) = hub.upgrade() {
            hub.forget(listening);
        }
    });
}

/// The engine's alerts, said in the dialog while `claim` takes them, until it closes.
fn claim(ctx: &Rc<Ctx>, dialog: &adw::Dialog, claim: impl Fn(&str) -> bool + 'static) {
    let id = ctx.claim_alerts(move |_, why| claim(why));
    let ctx = Rc::downgrade(ctx);
    dialog.connect_closed(move |_| {
        if let Some(ctx) = ctx.upgrade() {
            ctx.release_alerts(id);
        }
    });
}

/// A dialog's question to the store: a spinner while it is out, and when it goes unanswered,
/// why, with Try again. Offline, it is not sent at all.
struct Asking {
    ctx: Rc<Ctx>,
    body: gtk::Box,
    send: Box<dyn Fn(&Ctx)>,
    waiting: Cell<bool>,
    turn: Cell<u64>,
}

impl Asking {
    fn new(
        ctx: &Rc<Ctx>,
        dialog: &adw::Dialog,
        body: &gtk::Box,
        send: impl Fn(&Ctx) + 'static,
    ) -> Rc<Self> {
        let asking = Rc::new(Self {
            ctx: Rc::clone(ctx),
            body: body.clone(),
            send: Box::new(send),
            waiting: Cell::new(false),
            turn: Cell::new(0),
        });
        claim(ctx, dialog, {
            let asking = Rc::downgrade(&asking);
            move |why| {
                let Some(asking) = asking.upgrade() else { return false };
                if !asking.waiting.get() {
                    return false;
                }
                asking.failed(&t::stores_owner_unreachable(), why);
                true
            }
        });
        listen(ctx, dialog, {
            let asking = Rc::downgrade(&asking);
            move |event| {
                if let (Some(asking), Event::Connectivity { offline: true }) =
                    (asking.upgrade(), event)
                    && asking.waiting.get()
                {
                    asking.offline();
                }
            }
        });
        widgets::keep(dialog, Rc::clone(&asking));
        asking
    }

    fn ask(self: &Rc<Self>) {
        empty(&self.body);
        if self.ctx.hub.known.borrow().offline {
            self.offline();
            return;
        }
        self.body.append(&waiting());
        self.waiting.set(true);
        let turn = self.turn.get() + 1;
        self.turn.set(turn);
        (self.send)(&self.ctx);
        let asking = Rc::downgrade(self);
        glib::timeout_add_local_once(NO_ANSWER, move || {
            if let Some(asking) = asking.upgrade()
                && asking.waiting.get()
                && asking.turn.get() == turn
            {
                asking
                    .failed(&t::stores_owner_no_answer(), &t::stores_owner_no_answer_detail());
            }
        });
    }

    /// The store answered: what it said replaces the spinner.
    fn answered(&self) {
        self.waiting.set(false);
    }

    fn offline(self: &Rc<Self>) {
        self.failed(
            &t::stores_owner_needs_connection(),
            &t::stores_owner_needs_connection_detail(),
        );
    }

    fn failed(self: &Rc<Self>, title: &str, why: &str) {
        self.waiting.set(false);
        self.turn.set(self.turn.get() + 1);
        empty(&self.body);
        let again = gtk::Button::with_label(&t::action_try_again());
        again.add_css_class("pill");
        again.set_halign(gtk::Align::Center);
        let asking = Rc::downgrade(self);
        again.connect_clicked(move |_| {
            if let Some(asking) = asking.upgrade() {
                asking.ask();
            }
        });
        let page = crate::states::plain(title, why);
        page.set_child(Some(&again));
        self.body.append(&page);
    }
}

fn empty(column: &gtk::Box) {
    while let Some(child) = column.first_child() {
        column.remove(&child);
    }
}

/// A spinner in the dialog's middle, while the store is asked.
fn waiting() -> gtk::Widget {
    let spinner = adw::Spinner::new();
    spinner.set_size_request(24, 24);
    spinner.set_halign(gtk::Align::Center);
    spinner.set_margin_top(18);
    spinner.set_margin_bottom(18);
    spinner.upcast()
}

/// The engine's words for what went wrong, in the error colour.
fn problem_line(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("error");
    label.set_xalign(0.0);
    label.set_wrap(true);
    label
}

/// A text button on a row's end.
fn row_button(label: &str) -> gtk::Button {
    let button = gtk::Button::with_label(label);
    button.set_valign(gtk::Align::Center);
    button
}

/// A link on a plate, in monospace, selectable, with Copy Link under it.
fn link_plate(link: &str) -> gtk::Box {
    let text = gtk::Label::new(Some(link));
    text.add_css_class("monospace");
    text.set_xalign(0.0);
    text.set_wrap(true);
    text.set_wrap_mode(gtk::pango::WrapMode::Char);
    // A link broken across lines gains no hyphen, so what is copied by hand stays the link.
    let attrs = gtk::pango::AttrList::new();
    attrs.insert(gtk::pango::AttrInt::new_insert_hyphens(false));
    text.set_attributes(Some(&attrs));
    text.set_selectable(true);
    text.set_hexpand(true);
    let plate = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    plate.add_css_class("link-plate");
    plate.append(&text);
    let copy = gtk::Button::with_label(&t::stores_copy_link());
    copy.set_halign(gtk::Align::End);
    let link = link.to_owned();
    copy.connect_clicked(move |button| {
        button.clipboard().set_text(&link);
        button.set_label(&t::action_copied());
        let button = button.downgrade();
        glib::timeout_add_local_once(Duration::from_secs(2), move || {
            if let Some(button) = button.upgrade() {
                button.set_label(&t::stores_copy_link());
            }
        });
    });
    let column = gtk::Box::new(gtk::Orientation::Vertical, 8);
    column.append(&plate);
    column.append(&copy);
    column
}

/// When an invite stops working, as a date and a time: "28 Oct, 20:15".
pub fn expiry(seconds: i64) -> String {
    let twelve = crate::pages::calendar::twelve_hour();
    glib::DateTime::from_unix_local(seconds)
        .ok()
        .and_then(|when| {
            when.format(if twelve { "%-d %b, %-l:%M %p" } else { "%-d %b, %H:%M" }).ok() // i18n-skip
        })
        .map(|text| text.to_string())
        .unwrap_or_default()
}

/// How long ago, from the store's seconds.
fn since(seconds: i64) -> String {
    crate::say::ago(
        glib::DateTime::now_utc().map(|now| now.to_unix()).unwrap_or(seconds) - seconds,
    )
}

/// A date without a time: "7 Nov".
fn date(seconds: i64) -> String {
    glib::DateTime::from_unix_local(seconds)
        .ok()
        .and_then(|when| when.format("%-d %b").ok()) // i18n-skip
        .map(|text| text.to_string())
        .unwrap_or_default()
}

/// How often an invite works and until when: the line under the choices and under a made link.
fn works(max: u32, until: Option<i64>) -> String {
    match (max, until.map(date)) {
        (0, Some(until)) => t::stores_invite_works_any(&until),
        (0, None) => t::stores_invite_works_any_forever(),
        (1, Some(until)) => t::stores_invite_once_until(&until),
        (1, None) => t::stores_invite_once_forever(),
        (count, Some(until)) => t::stores_invite_works_times(i64::from(count), &until),
        (count, None) => t::stores_invite_works_times_forever(i64::from(count)),
    }
}

/// When a link made with `lasts` would end, for the line under the choices.
fn lasts_until(lasts: InviteLasts) -> Option<i64> {
    let days = match lasts {
        InviteLasts::Day => 1,
        InviteLasts::Week => 7,
        InviteLasts::Month => 30,
        InviteLasts::Forever => return None,
    };
    glib::DateTime::now_utc().ok().map(|now| now.to_unix() + days * 86_400)
}

fn store_name(ctx: &Ctx, store: &str) -> String {
    ctx.hub
        .known
        .borrow()
        .stores
        .iter()
        .find(|s| s.id == store)
        .map(|s| s.name.clone())
        .unwrap_or_else(|| store.to_owned())
}

// Invite.

/// An invite link to make: for someone new, by name, or for another device of yours.
pub fn invite(ctx: &Rc<Ctx>, store: &str, device: bool) {
    let name = store_name(ctx, store);
    let title = if device { t::stores_link_device_title() } else { t::stores_invite_title() };
    let frame = frame(ctx, "store-invite", &title, &name); // i18n-skip
    let cancel = gtk::Button::with_label(&t::action_cancel());
    frame.header.pack_start(&cancel);
    let make = gtk::Button::with_label(&t::stores_make_link());
    make.add_css_class("suggested-action");
    frame.header.pack_end(&make);
    let spinner = adw::Spinner::new();
    spinner.set_visible(false);
    frame.header.pack_end(&spinner);

    let field = adw::EntryRow::builder().title(t::stores_their_name()).build();
    // Read when Make Link is pressed, so kept with the dialog even where it is not shown.
    widgets::keep(&frame.dialog, field.clone());
    let problem = problem_line("");
    problem.set_visible(false);
    if device {
        frame.body.append(&widgets::haze(&t::stores_link_device_detail(), ""));
    } else {
        let list = widgets::boxed_list();
        list.append(&field);
        frame.body.append(&list);
        make.set_sensitive(false);
    }
    // How many times it works and for how long, for someone new. A store on an older server only
    // makes links that work once for a week, and says so in their place.
    let uses = Rc::new(Cell::new(InviteUses::Once));
    let lasts = Rc::new(Cell::new(InviteLasts::Week));
    let choices = Rc::new(Cell::new(true));
    let pickers = widgets::boxed_list();
    let uses_row = adw::ComboRow::builder()
        .title(t::stores_invite_uses())
        .model(&gtk::StringList::new(&[
            t::stores_invite_uses_once().as_str(),
            t::stores_invite_uses_five().as_str(),
            t::stores_invite_uses_any().as_str(),
        ]))
        .build();
    let lasts_row = adw::ComboRow::builder()
        .title(t::stores_invite_lasts())
        .model(&gtk::StringList::new(&[
            t::stores_invite_lasts_day().as_str(),
            t::stores_invite_lasts_week().as_str(),
            t::stores_invite_lasts_month().as_str(),
            t::stores_invite_lasts_forever().as_str(),
        ]))
        .selected(1)
        .build();
    pickers.append(&uses_row);
    pickers.append(&lasts_row);
    let works_line = widgets::haze("", "caption");
    let group_line = widgets::haze(&t::stores_invite_group_detail(), "caption");
    let older_line = widgets::haze(&t::stores_server_older_invites(), "caption");
    // Everything the choices change: the name field's wording and need, the lines under them.
    let refresh: Rc<dyn Fn()> = Rc::new({
        let (field, make) = (field.downgrade(), make.downgrade());
        let (uses, lasts, choices) = (Rc::clone(&uses), Rc::clone(&lasts), Rc::clone(&choices));
        let (pickers, works_line, group_line, older_line) = (
            pickers.downgrade(),
            works_line.downgrade(),
            group_line.downgrade(),
            older_line.downgrade(),
        );
        move || {
            let (
                Some(field),
                Some(make),
                Some(pickers),
                Some(works_line),
                Some(group_line),
                Some(older_line),
            ) = (
                field.upgrade(),
                make.upgrade(),
                pickers.upgrade(),
                works_line.upgrade(),
                group_line.upgrade(),
                older_line.upgrade(),
            )
            else {
                return;
            };
            let choices = choices.get();
            let group = choices && uses.get() != InviteUses::Once;
            field.set_title(&if group {
                t::stores_invite_group_name()
            } else {
                t::stores_their_name()
            });
            pickers.set_visible(choices);
            works_line.set_visible(choices);
            group_line.set_visible(group);
            older_line.set_visible(!choices);
            let max = match uses.get() {
                InviteUses::Once => 1,
                InviteUses::Five => 5,
                InviteUses::Any => 0,
            };
            works_line.set_label(&works(max, lasts_until(lasts.get())));
            // A name is needed for a link that works once; a group's name is optional.
            if !device {
                make.set_sensitive(group || !field.text().trim().is_empty());
            }
        }
    });
    if !device {
        uses_row.connect_selected_notify({
            let (uses, refresh) = (Rc::clone(&uses), Rc::clone(&refresh));
            move |row| {
                uses.set(match row.selected() {
                    1 => InviteUses::Five,
                    2 => InviteUses::Any,
                    _ => InviteUses::Once,
                });
                refresh();
            }
        });
        lasts_row.connect_selected_notify({
            let (lasts, refresh) = (Rc::clone(&lasts), Rc::clone(&refresh));
            move |row| {
                lasts.set(match row.selected() {
                    0 => InviteLasts::Day,
                    2 => InviteLasts::Month,
                    3 => InviteLasts::Forever,
                    _ => InviteLasts::Week,
                });
                refresh();
            }
        });
        field.connect_changed({
            let refresh = Rc::clone(&refresh);
            move |_| refresh()
        });
        frame.body.append(&pickers);
        frame.body.append(&works_line);
        frame.body.append(&group_line);
        frame.body.append(&older_line);
        refresh();
    }
    frame.body.append(&problem);
    frame.dialog.set_default_widget(Some(&make));
    field.connect_entry_activated({
        let make = make.downgrade();
        move |_| {
            if let Some(make) = make.upgrade()
                && make.is_sensitive()
            {
                make.emit_clicked();
            }
        }
    });
    cancel.connect_clicked({
        let dialog = frame.dialog.downgrade();
        move |_| {
            if let Some(dialog) = dialog.upgrade() {
                dialog.close();
            }
        }
    });
    let asked = Rc::new(Cell::new(false));
    make.connect_clicked({
        let ctx = Rc::downgrade(ctx);
        let (store, field, spinner, asked, problem) = (
            store.to_owned(),
            field.downgrade(),
            spinner.downgrade(),
            Rc::clone(&asked),
            problem.downgrade(),
        );
        let (uses, lasts, choices) = (Rc::clone(&uses), Rc::clone(&lasts), Rc::clone(&choices));
        move |make| {
            let (Some(ctx), Some(field), Some(spinner), Some(problem)) =
                (ctx.upgrade(), field.upgrade(), spinner.upgrade(), problem.upgrade())
            else {
                return;
            };
            // A link that works once is for a named person; a group's name is optional.
            let name = (!device).then(|| field.text().trim().to_owned()).filter(|name| {
                !name.is_empty() || (!device && choices.get() && uses.get() == InviteUses::Once)
            });
            let (uses, lasts) = match (device, choices.get()) {
                (false, true) => (Some(uses.get()), Some(lasts.get())),
                _ => (None, None),
            };
            if ctx.hub.known.borrow().offline {
                problem.set_label(&t::stores_owner_link_offline());
                problem.set_visible(true);
                return;
            }
            problem.set_visible(false);
            asked.set(true);
            make.set_visible(false);
            spinner.set_visible(true);
            field.set_sensitive(false);
            ctx.hub.send(Command::CreateInvite { store: store.clone(), name, uses, lasts });
        }
    });
    listen(ctx, &frame.dialog, {
        let (store, body, header) =
            (store.to_owned(), frame.body.downgrade(), frame.header.downgrade());
        let (make, cancel, spinner) =
            (make.downgrade(), cancel.downgrade(), spinner.downgrade());
        let dialog = frame.dialog.downgrade();
        let asked = Rc::clone(&asked);
        let (choices, refresh) = (Rc::clone(&choices), Rc::clone(&refresh));
        move |event| {
            // The store says whether its server offers the choices.
            if let Event::StoreMembers { store: of, invite_choices, .. } = event
                && *of == store
            {
                choices.set(*invite_choices);
                refresh();
                return;
            }
            let Event::InviteLink(link) = event else { return };
            if link.store != store || !asked.get() {
                return;
            }
            let (
                Some(body),
                Some(header),
                Some(make),
                Some(cancel),
                Some(spinner),
                Some(dialog),
            ) = (
                body.upgrade(),
                header.upgrade(),
                make.upgrade(),
                cancel.upgrade(),
                spinner.upgrade(),
                dialog.upgrade(),
            )
            else {
                return;
            };
            asked.set(false);
            header.remove(&make);
            header.remove(&cancel);
            header.remove(&spinner);
            let frame = Frame { dialog, header, body: body.clone() };
            done(&frame);
            show_link(&body, link, device);
        }
    });
    // Why the engine could not, said here and not in a toast under the dialog: the dialog is
    // as it was, the name kept, to try again.
    claim(ctx, &frame.dialog, {
        let (make, spinner, field, problem) =
            (make.downgrade(), spinner.downgrade(), field.downgrade(), problem.downgrade());
        move |why| {
            if !asked.get() {
                return false;
            }
            asked.set(false);
            if let (Some(make), Some(spinner), Some(field), Some(problem)) =
                (make.upgrade(), spinner.upgrade(), field.upgrade(), problem.upgrade())
            {
                spinner.set_visible(false);
                make.set_visible(true);
                field.set_sensitive(true);
                problem.set_label(why);
                problem.set_visible(true);
            }
            true
        }
    });
    frame.dialog.present(Some(&ctx.window));
    if !device {
        field.grab_focus();
        if !ctx.hub.known.borrow().offline {
            ctx.hub.send(Command::LoadMembers { store: store.to_owned() });
        }
    }
}

fn show_link(body: &gtk::Box, link: &InviteLinkView, device: bool) {
    empty(body);
    let line = if device {
        t::stores_link_once_until(&expiry(link.expires_at))
    } else {
        works(link.max, link.until)
    };
    body.append(&widgets::haze(&line, ""));
    body.append(&link_plate(&link.link.0));
}

// Members.

/// A store's members and the invites not used yet.
pub fn members(ctx: &Rc<Ctx>, store: &str) {
    let name = store_name(ctx, store);
    let frame = frame(ctx, "store-members", &t::stores_members(), &name); // i18n-skip
    done(&frame);
    let asking = Asking::new(ctx, &frame.dialog, &frame.body, {
        let store = store.to_owned();
        move |ctx| ctx.hub.send(Command::LoadMembers { store: store.clone() })
    });
    // The invites whose uses are showing, kept over a refresh of the list.
    let opened: Rc<RefCell<HashSet<String>>> = Rc::default();
    listen(ctx, &frame.dialog, {
        let ctx = Rc::downgrade(ctx);
        let (store, body) = (store.to_owned(), frame.body.downgrade());
        let asking = Rc::downgrade(&asking);
        move |event| {
            let (Some(ctx), Some(body)) = (ctx.upgrade(), body.upgrade()) else { return };
            if let Event::StoreMembers { store: of, members, invites, ended, .. } = event
                && *of == store
            {
                if let Some(asking) = asking.upgrade() {
                    asking.answered();
                }
                show_members(&ctx, &body, &store, members, invites, ended, &opened);
            }
        }
    });
    asking.ask();
    frame.dialog.present(Some(&ctx.window));
}

/// "Used 2 of 5 · Invited, until 7 Nov": how much of a link is spent, and when it ends.
fn facts(invite: &StoreInviteRow) -> String {
    let used = if invite.max == 1 || invite.used == 0 {
        if invite.used == 0 {
            t::stores_invite_not_used()
        } else {
            t::stores_invite_used(i64::from(invite.used))
        }
    } else if invite.max == 0 {
        t::stores_invite_used(i64::from(invite.used))
    } else {
        t::stores_invite_used_of(i64::from(invite.used), i64::from(invite.max))
    };
    match invite.until {
        Some(until) => format!("{used} · {}", t::stores_invited_until(&date(until))), // i18n-skip
        None => used,
    }
}

/// "Used up 7 Nov", "Ended 7 Nov" or "Cancelled 7 Nov", for an invite in the ended list.
fn ended_facts(invite: &StoreInviteRow) -> String {
    match &invite.ended {
        Some(e) => match e.reason {
            InviteEndReason::UsedUp => t::stores_invite_used_up(&date(e.at)),
            InviteEndReason::Cancelled => t::stores_invite_cancelled_on(&date(e.at)),
            InviteEndReason::RanOut => t::stores_invite_ran_out(&date(e.at)),
        },
        None => String::new(),
    }
}

fn show_members(
    ctx: &Rc<Ctx>,
    body: &gtk::Box,
    store: &str,
    members: &[StoreMemberRow],
    invites: &[StoreInviteRow],
    ended: &[StoreInviteRow],
    opened: &Rc<RefCell<HashSet<String>>>,
) {
    empty(body);
    let list = widgets::boxed_list();
    for member in members {
        let devices = t::stores_device_count(i64::from(member.devices));
        let row = adw::ActionRow::builder()
            .title(if member.you {
                t::party_member_you(&member.name)
            } else {
                member.name.clone()
            })
            .subtitle(if member.owner { t::stores_owner_devices(&devices) } else { devices })
            .build();
        if !member.you {
            let remove = row_button(&t::action_remove());
            remove.set_tooltip_text(Some(&t::stores_remove_member()));
            let ctx = Rc::downgrade(ctx);
            let (store, member) = (store.to_owned(), member.clone());
            remove.connect_clicked(move |button| {
                if let Some(ctx) = ctx.upgrade() {
                    confirm_remove(&ctx, &store, &member, button);
                }
            });
            row.add_suffix(&remove);
        }
        list.append(&row);
    }
    body.append(&list);
    let heading = widgets::heading(&t::stores_invite_links());
    heading.set_margin_top(12);
    body.append(&heading);
    if invites.is_empty() {
        body.append(&widgets::haze(&t::stores_no_invites(), "caption"));
    }
    for invite in invites {
        body.append(&invite_row(ctx, store, invite, &facts(invite), false, opened));
    }
    if ended.is_empty() {
        return;
    }
    let heading = widgets::heading(&t::stores_invites_ended());
    heading.set_margin_top(12);
    body.append(&heading);
    for invite in ended {
        body.append(&invite_row(ctx, store, invite, &ended_facts(invite), true, opened));
    }
}

/// An invite link of the store: who it is for, what is left of it, Uses with the people who
/// joined, and Cancel while it works. An ended one is dimmed.
fn invite_row(
    ctx: &Rc<Ctx>,
    store: &str,
    invite: &StoreInviteRow,
    facts: &str,
    ended: bool,
    opened: &Rc<RefCell<HashSet<String>>>,
) -> gtk::Box {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 4);
    if ended {
        column.set_opacity(0.6);
    }
    let list = widgets::boxed_list();
    let title = if invite.device || invite.name.is_empty() {
        t::stores_invite_unnamed()
    } else {
        t::stores_invite_for_name(&invite.name)
    };
    let row = adw::ActionRow::builder().title(&title).subtitle(facts).build();
    let uses = gtk::Revealer::new();
    uses.set_reveal_child(opened.borrow().contains(&invite.id));
    if !invite.uses.is_empty() {
        let toggle = gtk::ToggleButton::with_label(&t::stores_invite_show_uses());
        toggle.set_valign(gtk::Align::Center);
        toggle.set_active(uses.reveals_child());
        toggle.connect_toggled({
            let (uses, opened, id) = (uses.downgrade(), Rc::clone(opened), invite.id.clone());
            move |toggle| {
                if let Some(uses) = uses.upgrade() {
                    uses.set_reveal_child(toggle.is_active());
                }
                if toggle.is_active() {
                    opened.borrow_mut().insert(id.clone());
                } else {
                    opened.borrow_mut().remove(&id);
                }
            }
        });
        row.add_suffix(&toggle);
    }
    if !ended {
        let cancel = row_button(&t::stores_cancel_invite());
        let ctx = Rc::downgrade(ctx);
        let (store, invite) = (store.to_owned(), invite.clone());
        cancel.connect_clicked(move |button| {
            if let Some(ctx) = ctx.upgrade() {
                confirm_cancel(&ctx, &store, &invite, button);
            }
        });
        row.add_suffix(&cancel);
    }
    list.append(&row);
    column.append(&list);
    let people = widgets::boxed_list();
    for joined in &invite.uses {
        let line = adw::ActionRow::builder()
            .title(t::stores_invite_joined(&joined.name, &date(joined.at)))
            .build();
        if !joined.removed {
            let remove = row_button(&t::action_remove());
            let ctx = Rc::downgrade(ctx);
            let (store, member, name) =
                (store.to_owned(), joined.member.clone(), joined.name.clone());
            remove.connect_clicked(move |button| {
                if let Some(ctx) = ctx.upgrade() {
                    let who = StoreMemberRow {
                        id: member.clone(),
                        name: name.clone(),
                        owner: false,
                        devices: 0,
                        last_seen: None,
                        you: false,
                    };
                    confirm_remove(&ctx, &store, &who, button);
                }
            });
            line.add_suffix(&remove);
        }
        people.append(&line);
    }
    uses.set_child(Some(&people));
    column.append(&uses);
    column
}

/// Cancelling a link that people joined with says they stay members; one nobody used is
/// cancelled at once.
fn confirm_cancel(ctx: &Rc<Ctx>, store: &str, invite: &StoreInviteRow, button: &gtk::Button) {
    let send = {
        let (ctx, store, id, button) =
            (Rc::downgrade(ctx), store.to_owned(), invite.id.clone(), button.downgrade());
        move || {
            let Some(ctx) = ctx.upgrade() else { return };
            if let Some(button) = button.upgrade() {
                button.set_sensitive(false);
            }
            ctx.hub.send(Command::CancelInvite { store: store.clone(), invite: id.clone() });
        }
    };
    if invite.used == 0 {
        send();
        return;
    }
    let title = if invite.device || invite.name.is_empty() {
        t::stores_invite_unnamed()
    } else {
        t::stores_invite_for_name(&invite.name)
    };
    let dialog = adw::AlertDialog::new(
        Some(&title),
        Some(&t::stores_cancel_invite_detail(i64::from(invite.used))),
    );
    dialog.add_response("keep", &t::action_cancel());
    dialog.add_response("cancel-invite", &t::stores_cancel_invite());
    dialog.set_response_appearance("cancel-invite", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("keep"));
    dialog.set_close_response("keep");
    dialog.connect_response(None, move |_, response| {
        if response == "cancel-invite" {
            send();
        }
    });
    dialog.present(Some(&ctx.window));
}

fn confirm_remove(ctx: &Rc<Ctx>, store: &str, member: &StoreMemberRow, button: &gtk::Button) {
    let dialog = adw::AlertDialog::new(
        Some(&t::stores_remove_member_title(&member.name)),
        Some(&t::stores_remove_member_detail()),
    );
    dialog.add_response("cancel", &t::action_cancel());
    dialog.add_response("remove", &t::stores_remove_member());
    dialog.set_response_appearance("remove", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    let weak = Rc::downgrade(ctx);
    let (store, id, button) = (store.to_owned(), member.id.clone(), button.downgrade());
    dialog.connect_response(None, move |_, response| {
        let (Some(ctx), "remove") = (weak.upgrade(), response) else { return };
        if let Some(button) = button.upgrade() {
            button.set_sensitive(false);
        }
        ctx.hub.send(Command::RemoveMember { store: store.clone(), member: id.clone() });
    });
    dialog.present(Some(&ctx.window));
}

// Who can join.

struct Joining {
    ctx: Rc<Ctx>,
    store: String,
    body: gtk::Box,
    /// Set while the switch is moved to show the store's answer, not by the viewer.
    showing: Cell<bool>,
    /// Whether the store has answered, so the older server's words never replace an answer.
    answered: Cell<bool>,
    asking: RefCell<Option<Rc<Asking>>>,
}

/// Who can join a store: only people invited, or anyone with the store's link, and the devices
/// that joined with it.
pub fn joining(ctx: &Rc<Ctx>, store: &str) {
    let name = store_name(ctx, store);
    let frame = frame(ctx, "store-joining", &t::stores_who_can_join(), &name); // i18n-skip
    done(&frame);
    let state = Rc::new(Joining {
        ctx: Rc::clone(ctx),
        store: store.to_owned(),
        body: frame.body.clone(),
        showing: Cell::new(false),
        answered: Cell::new(false),
        asking: RefCell::default(),
    });
    // The store lists its members first, then says who can join: a store that answers the
    // first and stays silent on the second runs a server from before open joining.
    let asking = Asking::new(ctx, &frame.dialog, &frame.body, {
        let state = Rc::downgrade(&state);
        move |ctx| {
            if let Some(state) = state.upgrade() {
                state.answered.set(false);
                ctx.hub.send(Command::LoadMembers { store: state.store.clone() });
            }
        }
    });
    state.asking.replace(Some(Rc::clone(&asking)));
    listen(ctx, &frame.dialog, {
        let state = Rc::downgrade(&state);
        move |event| {
            let Some(state) = state.upgrade() else { return };
            match event {
                Event::OpenJoin { store, view } if *store == state.store => {
                    if let Some(asking) = state.asking.borrow().as_ref() {
                        asking.answered();
                    }
                    state.show(view);
                }
                Event::StoreMembers { store, .. } if *store == state.store => {
                    state.wait_for_open_join();
                }
                _ => {}
            }
        }
    });
    widgets::keep(&frame.dialog, Rc::clone(&state));
    asking.ask();
    frame.dialog.present(Some(&ctx.window));
}

impl Joining {
    /// The members came; who can join follows at once from a server that knows open joining.
    fn wait_for_open_join(self: &Rc<Self>) {
        let state = Rc::downgrade(self);
        glib::timeout_add_local_once(OLDER_SERVER, move || {
            let Some(state) = state.upgrade() else { return };
            let waiting = state.asking.borrow().as_ref().is_some_and(|a| a.waiting.get());
            if state.answered.get() || !waiting {
                return;
            }
            if let Some(asking) = state.asking.borrow().as_ref() {
                asking.answered();
            }
            empty(&state.body);
            state.body.append(&widgets::haze(&t::stores_server_older(), ""));
        });
    }

    fn show(self: &Rc<Self>, view: &OpenJoinView) {
        self.answered.set(true);
        empty(&self.body);
        let list = widgets::boxed_list();
        let open = adw::SwitchRow::builder()
            .title(t::stores_anyone_with_store_link())
            .subtitle(if view.open {
                t::stores_open_on_detail()
            } else {
                t::stores_open_off_detail()
            })
            .build();
        self.showing.set(true);
        open.set_active(view.open);
        self.showing.set(false);
        open.connect_active_notify({
            let state = Rc::downgrade(self);
            move |open| {
                let Some(state) = state.upgrade() else { return };
                if state.showing.get() {
                    return;
                }
                open.set_sensitive(false);
                state.ctx.hub.send(Command::SetOpenJoin {
                    store: state.store.clone(),
                    open: open.is_active(),
                });
            }
        });
        list.append(&open);
        self.body.append(&list);
        if let Some(link) = &view.link {
            self.body.append(&link_plate(&link.0));
        }
        if view.devices == 0 {
            return;
        }
        let devices = widgets::boxed_list();
        let summary = adw::ActionRow::builder()
            .title(t::stores_devices_joined(i64::from(view.devices)))
            .subtitle(if view.open {
                t::stores_open_no_names()
            } else {
                t::stores_open_keep_working()
            })
            .build();
        let all = row_button(&if view.devices == 1 {
            t::action_remove()
        } else {
            t::stores_remove_all()
        });
        all.connect_clicked({
            let state = Rc::downgrade(self);
            move |_| {
                if let Some(state) = state.upgrade() {
                    state.confirm_remove_all();
                }
            }
        });
        summary.add_suffix(&all);
        devices.append(&summary);
        for device in &view.recent {
            let row = adw::ActionRow::builder()
                .title(t::stores_device_joined_at(&since(device.joined_at)))
                .subtitle(device.last_seen.map_or_else(t::stores_device_never_seen, |at| {
                    t::stores_device_seen(&since(at))
                }))
                .build();
            let remove = row_button(&t::action_remove());
            let state = Rc::downgrade(self);
            let id = device.id.clone();
            remove.connect_clicked(move |button| {
                if let Some(state) = state.upgrade() {
                    button.set_sensitive(false);
                    state.ctx.hub.send(Command::RemoveAnonymous {
                        store: state.store.clone(),
                        device: Some(id.clone()),
                    });
                }
            });
            row.add_suffix(&remove);
            devices.append(&row);
        }
        devices.set_margin_top(12);
        self.body.append(&devices);
        if view.devices as usize > view.recent.len() {
            self.body.append(&widgets::haze(
                &t::stores_newest_listed(view.recent.len() as i64),
                "caption",
            ));
        }
    }

    fn confirm_remove_all(self: &Rc<Self>) {
        let dialog = adw::AlertDialog::new(
            Some(&t::stores_remove_devices_title()),
            Some(&t::stores_remove_devices_detail(&store_name(&self.ctx, &self.store))),
        );
        dialog.add_response("cancel", &t::action_cancel());
        dialog.add_response("remove", &t::action_remove());
        dialog.set_response_appearance("remove", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let state = Rc::downgrade(self);
        dialog.connect_response(None, move |_, response| {
            let (Some(state), "remove") = (state.upgrade(), response) else { return };
            state
                .ctx
                .hub
                .send(Command::RemoveAnonymous { store: state.store.clone(), device: None });
        });
        dialog.present(Some(&self.ctx.window));
    }
}

// Publish.

/// The plugins on this computer, to publish to the store or bring up to date there.
pub fn publish(ctx: &Rc<Ctx>, store: &str) {
    let name = store_name(ctx, store);
    let frame = frame(ctx, "store-publish", &t::stores_publish_short(), &name); // i18n-skip
    done(&frame);
    let publishing = Rc::new(Publishing::default());
    let asking = Asking::new(ctx, &frame.dialog, &frame.body, {
        let store = store.to_owned();
        move |ctx| ctx.hub.send(Command::LoadLocalPlugins { store: store.clone() })
    });
    listen(ctx, &frame.dialog, {
        let ctx = Rc::downgrade(ctx);
        let (store, body) = (store.to_owned(), frame.body.downgrade());
        let publishing = Rc::clone(&publishing);
        let asking = Rc::downgrade(&asking);
        move |event| {
            let (Some(ctx), Some(body)) = (ctx.upgrade(), body.upgrade()) else { return };
            match event {
                Event::LocalPlugins { store: of, plugins } if *of == store => {
                    if let Some(asking) = asking.upgrade() {
                        asking.answered();
                    }
                    publishing.plugin.replace(None);
                    show_local(&ctx, &body, &store, plugins, &publishing);
                }
                // Published: the store's list changed, so what it offers is asked again.
                Event::Stores(_) if publishing.plugin.borrow().is_some() => {
                    ctx.hub.send(Command::LoadLocalPlugins { store: store.clone() });
                }
                _ => {}
            }
        }
    });
    // Not published: why shows over the list, and the list is asked again.
    claim(ctx, &frame.dialog, {
        let ctx = Rc::downgrade(ctx);
        let store = store.to_owned();
        let publishing = Rc::clone(&publishing);
        move |why| {
            let Some(ctx) = ctx.upgrade() else { return false };
            if publishing.plugin.borrow().is_none() {
                return false;
            }
            // The engine names the plugin by its id: here it goes by the name its row shows.
            let id = publishing.plugin.borrow().clone().unwrap_or_default();
            let named = why
                .strip_prefix(&crate::catalog::shown(&id))
                .map(|rest| format!("{}{rest}", publishing.name.borrow()));
            publishing.problem.replace(Some(named.unwrap_or_else(|| why.to_owned())));
            ctx.hub.send(Command::LoadLocalPlugins { store: store.clone() });
            true
        }
    });
    asking.ask();
    frame.dialog.present(Some(&ctx.window));
}

/// The plugin being published, by id and name, and why the last one was not.
#[derive(Default)]
struct Publishing {
    plugin: RefCell<Option<String>>,
    name: RefCell<String>,
    problem: RefCell<Option<String>>,
}

fn show_local(
    ctx: &Rc<Ctx>,
    body: &gtk::Box,
    store: &str,
    plugins: &[LocalPluginRow],
    publishing: &Rc<Publishing>,
) {
    empty(body);
    if let Some(why) = publishing.problem.take() {
        body.append(&problem_line(&why));
    }
    if plugins.is_empty() {
        body.append(&widgets::haze(&t::stores_publish_none(), ""));
        body.append(&widgets::haze(&t::stores_publish_none_hint(), "caption"));
        return;
    }
    let list = widgets::boxed_list();
    for plugin in plugins {
        let row = adw::ActionRow::builder()
            .title(t::stores_plugin_name_version(&plugin.name, &plugin.version))
            .subtitle(
                plugin
                    .published
                    .as_ref()
                    .map_or_else(t::stores_not_published, |v| t::stores_store_offers(v)),
            )
            .build();
        // This version already in the store: nothing to do, and said so.
        if plugin.published.as_deref() == Some(plugin.version.as_str()) {
            row.set_subtitle(&t::stores_published_current());
            row.add_suffix(&widgets::haze(&t::stores_published_state(), ""));
            list.append(&row);
            continue;
        }
        let button = row_button(&t::action_publish());
        button.set_tooltip_text(Some(&t::stores_publish_label(&plugin.name, &plugin.version)));
        let ctx = Rc::downgrade(ctx);
        let (store, id, name, publishing) =
            (store.to_owned(), plugin.id.clone(), plugin.name.clone(), Rc::clone(publishing));
        button.connect_clicked(move |button| {
            let Some(ctx) = ctx.upgrade() else { return };
            button.set_sensitive(false);
            let spinner = adw::Spinner::new();
            button.set_child(Some(&spinner));
            publishing.plugin.replace(Some(id.clone()));
            publishing.name.replace(name.clone());
            ctx.hub.send(Command::PublishPlugin { store: store.clone(), plugin: id.clone() });
        });
        row.add_suffix(&button);
        list.append(&row);
    }
    body.append(&list);
}
