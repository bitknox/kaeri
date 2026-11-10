//! The rows under a series' head: where it reads from, which trackers follow it and the group in
//! use, and the two sheets those open: Read from, with the sources that have the series and the
//! ones that don't, and the trackers' progress with each one's match.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use gtk::gio;
use kaeri_core::books::WorkId;
use kaeri_i18n::t;
use kaeri_model::{
    book_sources::{BookSourceCommand, BookSourceEvent, EditionMiss, SourceSeriesChoice},
    command::Command,
    event::Event,
    reading::{
        BookEditionRow, EditionListing, ReadingCommand, ReadingEvent, SeriesView,
        TrackerMapping, TrackerReadRow,
    },
};

use super::{listen, send};
use crate::{say, widgets, window::Ctx};

/// What the rows are drawn from, as it has arrived.
#[derive(Default)]
pub struct Memo {
    pub view: Option<SeriesView>,
    pub tracking: Option<Vec<TrackerReadRow>>,
    pub editions: Vec<BookEditionRow>,
}

fn sources(hub: &crate::hub::Hub, command: BookSourceCommand) {
    hub.send(Command::BookSources(command));
}

/// Ask what the rows need: the trackers' progress and the sources that have the series.
pub fn load(ctx: &Rc<Ctx>, work: &WorkId) {
    send(&ctx.hub, ReadingCommand::LoadTracking { work: work.clone() });
    sources(&ctx.hub, BookSourceCommand::LoadForWork { work: work.clone() });
}

/// Keep `memo` up to date from the events, and say when `redraw` is due.
pub fn follow(memo: &Rc<RefCell<Memo>>, work: &WorkId, event: &Event) -> bool {
    match event {
        Event::Reading(ReadingEvent::Series(view)) if view.work == *work => {
            memo.borrow_mut().view = Some((**view).clone());
            true
        }
        Event::Reading(ReadingEvent::Tracking { work: w, rows }) if w == work => {
            memo.borrow_mut().tracking = Some(rows.clone());
            true
        }
        Event::BookSources(BookSourceEvent::ForWork { work: w, editions, .. }) if w == work => {
            memo.borrow_mut().editions = editions.clone();
            true
        }
        _ => false,
    }
}

/// "books.home.example · 1 more": the source, and how many others have it. The group has a row
/// of its own.
fn source_line(edition: &BookEditionRow, others: usize) -> String {
    let mut parts = vec![edition.source_name.clone()];
    if others > 0 {
        parts.push(t::series_sources_more(others as i64));
    }
    parts.join(" \u{b7} ")
}

/// "87 of 88 chapters · volume 9", "Volume 5 of 5 read".
fn progress(row: &TrackerReadRow, view: &SeriesView) -> String {
    if view.medium == kaeri_core::media::Medium::Novel
        && let Some(volumes) = row.volumes
    {
        return t::series_tracker_volumes(
            i64::from(volumes),
            i64::from(view.volumes_total.unwrap_or(volumes)),
        );
    }
    let chapters = match view.chapters_total {
        Some(total) => t::series_tracker_chapters_of(i64::from(row.chapters), i64::from(total)),
        None => t::series_tracker_chapters(i64::from(row.chapters)),
    };
    match row.volumes {
        Some(volume) if volume > 0 => {
            t::series_tracker_with_volume(&chapters, i64::from(volume))
        }
        _ => chapters,
    }
}

fn names(rows: &[&TrackerReadRow]) -> String {
    rows.iter().map(|r| say::tracker_name(&r.tracker)).collect::<Vec<_>>().join(", ")
}

fn row(title: &str, subtitle: &str) -> adw::ActionRow {
    adw::ActionRow::builder().title(glib_escape(title)).subtitle(glib_escape(subtitle)).build()
}

fn glib_escape(text: &str) -> String {
    gtk::glib::markup_escape_text(text).to_string()
}

fn change_button(label: &str) -> gtk::Button {
    let button = gtk::Button::with_label(label);
    button.add_css_class("flat");
    button.set_valign(gtk::Align::Center);
    button
}

/// The name of the veil over the rows, which a group pick marks.
pub const DETAILS_VEIL: &str = "series-details-veil";
/// The mark a pick leaves on the veil until the series comes back with the group in use.
const PICKED_NOW: &str = "group-picked-now";

/// Draw the rows again from `memo`. When the series comes back after a group pick, the old rows
/// fade over the new ones (0.3 s, or 0.2 s under Reduce Motion as every crossfade) and the
/// keyboard goes back to the Group row's Change.
pub fn redraw(
    ctx: &Rc<Ctx>,
    veil: &gtk::Overlay,
    details: &gtk::Box,
    memo: &Memo,
    series: bool,
) {
    if !(series && veil.has_css_class(PICKED_NOW)) {
        draw(ctx, details, memo);
        return;
    }
    veil.remove_css_class(PICKED_NOW);
    crate::motion::ghost(veil, details, true, || draw(ctx, details, memo));
    let details = details.downgrade();
    gtk::glib::idle_add_local_once(move || {
        if let Some(change) = details.upgrade().and_then(|d| named(d.upcast_ref(), CHANGE)) {
            change.grab_focus();
        }
    });
}

/// The Group row's Change, by its name.
const CHANGE: &str = "series-group-change";

fn named(widget: &gtk::Widget, name: &str) -> Option<gtk::Widget> {
    if widget.widget_name() == name {
        return Some(widget.clone());
    }
    let mut child = widget.first_child();
    while let Some(next) = child {
        if let Some(found) = named(&next, name) {
            return Some(found);
        }
        child = next.next_sibling();
    }
    None
}

/// Draw the rows into `details` from `memo`.
pub fn draw(ctx: &Rc<Ctx>, details: &gtk::Box, memo: &Memo) {
    super::clear(details);
    let Some(view) = &memo.view else { return };
    let list = widgets::boxed_list();
    if let Some(edition) = &view.edition {
        let line = row(
            &t::series_reading_from(),
            &source_line(edition, memo.editions.len().saturating_sub(1)),
        );
        let button = change_button(&t::series_change());
        button.connect_clicked({
            let (ctx, work, title) =
                (Rc::downgrade(ctx), view.work.clone(), view.titles.display.clone());
            move |_| {
                if let Some(ctx) = ctx.upgrade() {
                    read_from(&ctx, &work, &title);
                }
            }
        });
        line.add_suffix(&button);
        list.append(&line);
    }
    // The group whose version of each chapter the series lists, with one more to pick from.
    if let Some(edition) = view.edition.as_ref().filter(|e| e.groups.len() > 1)
        && let Some(label) = edition.version_label.clone().filter(|l| !l.is_empty())
    {
        let why = if edition.group_chosen {
            t::series_group_picked()
        } else {
            t::series_group_most()
        };
        let line = row(&t::series_group(), &[label, why].join(" \u{b7} "));
        // A menu button holds the picker, so the picker goes with the row when the series is
        // drawn again after a pick.
        let button = gtk::MenuButton::new();
        button.set_child(Some(&gtk::Label::new(Some(&t::series_change()))));
        button.set_always_show_arrow(false);
        button.add_css_class("flat");
        button.set_valign(gtk::Align::Center);
        button.update_property(&[gtk::accessible::Property::Description(
            &t::series_group_hint(),
        )]);
        button.set_widget_name(CHANGE);
        button.set_popover(Some(&group_picker(ctx, &view.work, edition)));
        line.add_suffix(&button);
        list.append(&line);
    }
    let tracking = memo.tracking.as_deref().unwrap_or(&view.tracking);
    let followed: Vec<&TrackerReadRow> =
        tracking.iter().filter(|r| matches!(r.mapping, TrackerMapping::Mapped)).collect();
    if let Some(first) = followed.first() {
        let line = row(&names(&followed), &progress(first, view));
        line.set_activatable(true);
        line.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
        line.connect_activated({
            let (ctx, work, shown) = (Rc::downgrade(ctx), view.work.clone(), view.clone());
            move |_| {
                if let Some(ctx) = ctx.upgrade() {
                    trackers(&ctx, &work, Some(shown.clone()));
                }
            }
        });
        list.append(&line);
    } else if !tracking.is_empty() {
        let all: Vec<&TrackerReadRow> = tracking.iter().collect();
        // The line already asks; the whole row is the one action that answers it.
        let line = row(&names(&all), &t::series_tracker_ask());
        line.set_activatable(true);
        line.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
        line.connect_activated({
            let (ctx, work, shown) = (Rc::downgrade(ctx), view.work.clone(), view.clone());
            move |_| {
                if let Some(ctx) = ctx.upgrade() {
                    trackers(&ctx, &work, Some(shown.clone()));
                }
            }
        });
        list.append(&line);
    }
    if list.first_child().is_some() {
        details.append(&list);
    }
}

// -------------------------------------------------------------------------------------------
// Group

/// The group picker: a popover with an arrow on the Group row's Change, naming the source it
/// sits inside. Most chapters first, then each group with how many chapters it has and its
/// language; a check on the one in use. A pick closes it, and the chapters follow the group.
pub fn group_picker(ctx: &Rc<Ctx>, work: &WorkId, edition: &BookEditionRow) -> gtk::Popover {
    let popover = gtk::Popover::new();
    popover.add_css_class("group-picker");
    popover.set_position(gtk::PositionType::Bottom);
    let title = gtk::Label::new(Some(&t::series_group()));
    title.add_css_class("heading");
    title.set_xalign(0.0);
    let within = widgets::haze(&t::group_picker_in(&edition.source_name), "caption");
    within.set_xalign(0.0);
    let head = gtk::Box::new(gtk::Orientation::Vertical, 2);
    head.add_css_class("group-picker-head");
    head.append(&title);
    head.append(&within);
    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::None);
    list.add_css_class("group-picker-list");
    list.update_property(&[gtk::accessible::Property::Label(&t::series_group())]);
    let most = edition.groups.first().map(|g| g.label.clone()).unwrap_or_default();
    let mut picks: Vec<Option<String>> = vec![None];
    list.append(&choice_row(
        &t::group_picker_most(),
        &t::group_picker_most_now(&most),
        !edition.group_chosen,
    ));
    let rule = gtk::ListBoxRow::new();
    rule.set_child(Some(&gtk::Separator::new(gtk::Orientation::Horizontal)));
    rule.add_css_class("group-picker-rule");
    rule.set_activatable(false);
    rule.set_selectable(false);
    rule.set_focusable(false);
    list.append(&rule);
    picks.push(None);
    for group in &edition.groups {
        let mut said = vec![t::group_picker_count(
            i64::from(group.chapters),
            i64::from(edition.chapters.max(group.chapters)),
        )];
        said.extend(
            group
                .language
                .as_deref()
                .filter(|l| !l.is_empty())
                .map(|l| say::language(l.split(['-', '_']).next().unwrap_or(l))),
        );
        list.append(&choice_row(
            &group.label,
            &said.join(" \u{b7} "),
            edition.group_chosen && group.in_use,
        ));
        picks.push(Some(group.label.clone()));
    }
    let fallback = widgets::haze(&t::group_picker_fallback(), "caption");
    fallback.set_wrap(true);
    fallback.set_xalign(0.0);
    fallback.set_max_width_chars(36);
    fallback.add_css_class("group-picker-foot");
    let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    column.set_width_request(320);
    column.append(&head);
    column.append(&list);
    column.append(&fallback);
    popover.set_child(Some(&column));
    list.connect_row_activated({
        let (ctx, work, popover) = (Rc::downgrade(ctx), work.clone(), popover.downgrade());
        move |_, row| {
            let index = row.index();
            if index == 1 {
                return;
            }
            if let Some(popover) = popover.upgrade() {
                // The series that comes back with the pick redraws the rows by a crossfade.
                let mut up = popover.parent();
                while let Some(widget) = up {
                    if widget.widget_name() == DETAILS_VEIL {
                        widget.add_css_class(PICKED_NOW);
                        break;
                    }
                    up = widget.parent();
                }
                popover.popdown();
            }
            if let Some(ctx) = ctx.upgrade()
                && let Some(group) = usize::try_from(index).ok().and_then(|i| picks.get(i))
            {
                send(
                    &ctx.hub,
                    ReadingCommand::ChooseGroup { work: work.clone(), group: group.clone() },
                );
            }
        }
    });
    // The keys start on the choice in use.
    popover.connect_show({
        let list = list.downgrade();
        move |_| {
            let Some(list) = list.upgrade() else { return };
            let mut row = list.first_child();
            while let Some(widget) = row {
                if widget.has_css_class("group-picked") {
                    widget.grab_focus();
                    break;
                }
                row = widget.next_sibling();
            }
        }
    });
    popover
}

/// A choice in the group picker: its name, its line, and a check when it is the one in use.
fn choice_row(title: &str, subtitle: &str, picked: bool) -> gtk::ListBoxRow {
    // Long names and lines wrap at large sizes; none is cut.
    let name = gtk::Label::new(Some(title));
    name.set_xalign(0.0);
    name.set_wrap(true);
    name.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    let line = widgets::haze(subtitle, "caption");
    line.set_xalign(0.0);
    line.set_wrap(true);
    line.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    let words = gtk::Box::new(gtk::Orientation::Vertical, 2);
    words.set_hexpand(true);
    words.set_valign(gtk::Align::Center);
    words.append(&name);
    words.append(&line);
    let check = gtk::Image::from_icon_name("object-select-symbolic");
    check.set_opacity(if picked { 1.0 } else { 0.0 });
    check.set_valign(gtk::Align::Center);
    check.set_accessible_role(gtk::AccessibleRole::Presentation);
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    content.add_css_class("group-picker-row");
    content.append(&words);
    content.append(&check);
    let row = gtk::ListBoxRow::new();
    row.set_child(Some(&content));
    if picked {
        row.add_css_class("group-picked");
        row.update_state(&[gtk::accessible::State::Selected(Some(true))]);
    }
    row.update_property(&[
        gtk::accessible::Property::Label(title),
        gtk::accessible::Property::Description(subtitle),
    ]);
    row
}

// -------------------------------------------------------------------------------------------
// Read from

/// The sheet that lists the sources with the series, picks one, and searches the others.
pub fn read_from(ctx: &Rc<Ctx>, work: &WorkId, title: &str) {
    read_from_then(ctx, work, title, None);
}

/// [`read_from`], running `chosen` once a source is picked and the sheet has closed.
pub fn read_from_then(ctx: &Rc<Ctx>, work: &WorkId, title: &str, chosen: Option<Rc<dyn Fn()>>) {
    let dialog = adw::Dialog::new();
    dialog.set_title(&t::series_read_from_title());
    dialog.set_content_width(560);
    dialog.set_follows_content_size(true);
    let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
    column.set_margin_top(12);
    column.set_margin_bottom(18);
    column.set_margin_start(18);
    column.set_margin_end(18);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
    column.append(&body);
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .max_content_height(520)
        .child(&column)
        .build();
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::new(&t::series_read_from_title(), title)));
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&scroll));
    dialog.set_child(Some(&toolbar));

    let found: Rc<RefCell<Found>> = Rc::default();
    if let Some(chosen) = chosen {
        let picked = Rc::clone(&found.borrow().picked);
        dialog.connect_closed(move |_| {
            if picked.get() {
                chosen();
            }
        });
    }
    let redraw: Rc<dyn Fn()> = Rc::new({
        let (ctx, work, body, found, dialog, title) = (
            Rc::downgrade(ctx),
            work.clone(),
            body.clone(),
            Rc::clone(&found),
            dialog.downgrade(),
            title.to_owned(),
        );
        move || {
            let (Some(ctx), Some(dialog)) = (ctx.upgrade(), dialog.upgrade()) else { return };
            draw_read_from(&ctx, &dialog, &work, &title, &body, &found);
        }
    });
    found.borrow_mut().redraw = Some(Rc::downgrade(&redraw));
    listen(ctx, &dialog, {
        let (work, found, redraw) = (work.clone(), Rc::clone(&found), Rc::clone(&redraw));
        move |event| {
            let Event::BookSources(event) = event else { return };
            match event {
                BookSourceEvent::View(view) => {
                    found.borrow_mut().sources = Some(view.clone());
                }
                BookSourceEvent::ForWork { work: w, editions, asking, missing }
                    if *w == work =>
                {
                    let mut found = found.borrow_mut();
                    found.editions = editions.clone();
                    found.asking = *asking;
                    found.missing = missing.clone();
                    found.loaded = true;
                }
                BookSourceEvent::SourceSeries {
                    work: w, source_id, choices, failure, ..
                } if *w == work => {
                    let mut found = found.borrow_mut();
                    found.searched = Some((source_id.clone(), choices.clone()));
                    found.failure = failure.as_ref().map(super::failure);
                }
                _ => return,
            }
            redraw();
        }
    });
    sources(&ctx.hub, BookSourceCommand::Load);
    sources(&ctx.hub, BookSourceCommand::LoadForWork { work: work.clone() });
    redraw();
    dialog.present(Some(&ctx.window));
}

#[derive(Default)]
struct Found {
    /// The sources themselves, for the kind of each: a server's, a store's plugin, a folder.
    sources: Option<kaeri_model::book_sources::BookSourcesView>,
    loaded: bool,
    asking: bool,
    editions: Vec<BookEditionRow>,
    missing: Vec<EditionMiss>,
    /// The source whose "Search there" field is open.
    open: Option<String>,
    searched: Option<(String, Vec<SourceSeriesChoice>)>,
    failure: Option<String>,
    redraw: Option<std::rc::Weak<dyn Fn()>>,
    /// A source was picked before the sheet closed.
    picked: Rc<std::cell::Cell<bool>>,
}

fn draw_read_from(
    ctx: &Rc<Ctx>,
    dialog: &adw::Dialog,
    work: &WorkId,
    title: &str,
    body: &gtk::Box,
    found: &Rc<RefCell<Found>>,
) {
    super::clear(body);
    let state = found.borrow();
    if !state.editions.is_empty() {
        let list = widgets::boxed_list();
        for edition in &state.editions {
            let line =
                row(&edition.source_name, &edition_line(edition, state.sources.as_ref()));
            if edition.in_use {
                let check = gtk::Image::from_icon_name("object-select-symbolic");
                line.add_suffix(&check);
            } else {
                line.set_activatable(true);
                line.connect_activated({
                    let (ctx, work, edition, dialog, picked) = (
                        Rc::downgrade(ctx),
                        work.clone(),
                        edition.clone(),
                        dialog.downgrade(),
                        Rc::clone(&state.picked),
                    );
                    move |_| {
                        let Some(ctx) = ctx.upgrade() else { return };
                        picked.set(true);
                        send(
                            &ctx.hub,
                            ReadingCommand::ChooseEdition {
                                work: work.clone(),
                                source_id: edition.source_id.clone(),
                                version_label: edition.version_label.clone(),
                                language: edition.language.clone(),
                            },
                        );
                        if let Some(dialog) = dialog.upgrade() {
                            dialog.close();
                        }
                    }
                });
            }
            list.append(&line);
        }
        body.append(&list);
        let note = widgets::haze(&t::series_read_from_note(), "caption");
        note.set_wrap(true);
        body.append(&note);
    } else if !state.loaded || state.asking {
        body.append(&widgets::haze(&t::series_read_from_asking(), "caption"));
    } else if state.missing.is_empty() {
        body.append(&crate::states::plain(
            &t::series_read_from_none_title(),
            &t::series_read_from_none_line(),
        ));
    }
    if !state.missing.is_empty() {
        let heading = gtk::Label::new(Some(&t::read_from_not_found()));
        heading.add_css_class("heading");
        heading.set_xalign(0.0);
        body.append(&heading);
        let list = widgets::boxed_list();
        for miss in &state.missing {
            let line = row(&miss.source_name, &say::msg(&miss.reason));
            let open = state.open.as_deref() == Some(miss.source_id.as_str());
            if !open {
                let button = change_button(&t::read_from_search_there());
                button.connect_clicked({
                    let (found, source) = (Rc::clone(found), miss.source_id.clone());
                    let ctx = Rc::downgrade(ctx);
                    let (work, query) =
                        (work.clone(), miss.query.clone().unwrap_or_else(|| title.to_owned()));
                    move |_| {
                        let Some(ctx) = ctx.upgrade() else { return };
                        {
                            let mut state = found.borrow_mut();
                            state.open = Some(source.clone());
                            state.searched = None;
                            state.failure = None;
                        }
                        sources(
                            &ctx.hub,
                            BookSourceCommand::SearchSourceForWork {
                                work: work.clone(),
                                source_id: source.clone(),
                                query: query.clone(),
                            },
                        );
                        let redraw = found.borrow().redraw.clone();
                        if let Some(redraw) = redraw.and_then(|r| r.upgrade()) {
                            redraw();
                        }
                    }
                });
                line.add_suffix(&button);
            }
            list.append(&line);
            if open {
                list.append(&search_there(ctx, dialog, work, title, miss, &state));
            } else if let Some(possible) = &miss.possible {
                // Another edition the source has, which may be it: one press confirms it.
                list.append(&possible_row(
                    ctx,
                    dialog,
                    work,
                    &miss.source_id,
                    possible,
                    &state,
                ));
            }
        }
        body.append(&list);
    }
}

/// "Kavita · chapters 1 to 88", "Chapters 1 to 60 · on this computer": what kind of source it
/// is, its group and language, and how far it goes.
fn edition_line(
    edition: &BookEditionRow,
    view: Option<&kaeri_model::book_sources::BookSourcesView>,
) -> String {
    use kaeri_model::book_sources::BookServerKind;
    let mut lead: Vec<String> = Vec::new();
    let mut folder = false;
    if let Some(view) = view {
        if let Some(server) = view.servers.iter().find(|s| s.id == edition.source_id) {
            lead.push(match server.kind {
                BookServerKind::Komga => "Komga".to_owned(), // i18n-skip
                BookServerKind::Kavita => "Kavita".to_owned(), // i18n-skip
                BookServerKind::Opds => t::book_sources_opds(),
            });
        } else if let Some(from) = view
            .plugins
            .iter()
            .find(|p| p.id == edition.source_id)
            .and_then(|p| p.store.as_deref())
        {
            lead.push(t::book_sources_from_store(from));
        } else if view.folders.iter().any(|f| f.id == edition.source_id) {
            folder = true;
        }
    }
    lead.extend(edition.language.as_deref().filter(|l| !l.is_empty()).map(say::language));
    let mut parts = lead.clone();
    parts.push(edition_chapters(edition, lead.is_empty()));
    if folder {
        parts.push(t::series_read_from_on_device(&t::device_kind_computer()));
    }
    parts.join(" \u{b7} ")
}

/// The chapters part of an edition's line. A list not asked for yet, or still on its way, says
/// it is being checked; an empty one says so; one that failed says why. Only a list the source
/// gave has a range, so it never reads "1 to 0".
fn edition_chapters(edition: &BookEditionRow, first: bool) -> String {
    let checking = || {
        if first {
            t::series_read_from_checking()
        } else {
            t::series_read_from_checking_later()
        }
    };
    let empty =
        || if first { t::series_read_from_empty() } else { t::series_read_from_empty_later() };
    let has = edition.chapters > 0 || edition.latest.is_some();
    match &edition.listing {
        Some(EditionListing::Failed { failure }) => super::failure(failure),
        Some(EditionListing::Empty) => empty(),
        Some(EditionListing::Asking) => checking(),
        None if !has => checking(),
        Some(EditionListing::Listed) if !has => empty(),
        Some(EditionListing::Listed) | None => {
            let latest = edition.latest.clone().unwrap_or_else(|| edition.chapters.to_string());
            if first {
                t::series_read_from_chapters(&latest)
            } else {
                t::series_read_from_chapters_later(&latest)
            }
        }
    }
}

/// A series a source has in another edition: its title with year and language, chosen with a press.
fn possible_row(
    ctx: &Rc<Ctx>,
    dialog: &adw::Dialog,
    work: &WorkId,
    source_id: &str,
    choice: &SourceSeriesChoice,
    state: &Found,
) -> adw::ActionRow {
    let mut parts: Vec<String> = Vec::new();
    parts.extend(choice.year.map(|y| y.to_string()));
    parts.extend(choice.language.as_deref().filter(|l| !l.is_empty()).map(say::language));
    let line = row(&choice.title, &parts.join(" \u{b7} "));
    line.set_activatable(true);
    line.connect_activated({
        let (ctx, work, source, series_ref, dialog, picked) = (
            Rc::downgrade(ctx),
            work.clone(),
            source_id.to_owned(),
            choice.series_ref.clone(),
            dialog.downgrade(),
            Rc::clone(&state.picked),
        );
        move |_| {
            let Some(ctx) = ctx.upgrade() else { return };
            picked.set(true);
            sources(
                &ctx.hub,
                BookSourceCommand::ChooseSourceSeries {
                    work: work.clone(),
                    source_id: source.clone(),
                    series_ref: series_ref.clone(),
                },
            );
            if let Some(dialog) = dialog.upgrade() {
                dialog.close();
            }
        }
    });
    line
}

/// The field under a source that has no match, and what that source finds.
fn search_there(
    ctx: &Rc<Ctx>,
    dialog: &adw::Dialog,
    work: &WorkId,
    title: &str,
    miss: &EditionMiss,
    state: &Found,
) -> gtk::ListBoxRow {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 8);
    column.set_margin_top(8);
    column.set_margin_bottom(8);
    column.set_margin_start(12);
    column.set_margin_end(12);
    let entry = gtk::SearchEntry::new();
    entry.set_placeholder_text(Some(&t::read_from_search_placeholder(&miss.source_name)));
    entry.set_text(miss.query.as_deref().unwrap_or(title));
    column.append(&entry);
    let go = {
        let (ctx, work, source, entry) =
            (Rc::downgrade(ctx), work.clone(), miss.source_id.clone(), entry.clone());
        move || {
            let query = entry.text().trim().to_owned();
            if query.is_empty() {
                return;
            }
            if let Some(ctx) = ctx.upgrade() {
                sources(
                    &ctx.hub,
                    BookSourceCommand::SearchSourceForWork {
                        work: work.clone(),
                        source_id: source.clone(),
                        query,
                    },
                );
            }
        }
    };
    entry.connect_activate({
        let go = go.clone();
        move |_| go()
    });
    let search = gtk::Button::with_label(&t::read_from_search());
    search.add_css_class("pill");
    search.set_halign(gtk::Align::Start);
    search.connect_clicked(move |_| go());
    column.append(&search);
    if let Some(failure) = &state.failure {
        column.append(&widgets::haze(failure, "caption"));
    } else if let Some((source, choices)) = &state.searched
        && *source == miss.source_id
    {
        if choices.is_empty() {
            column.append(&widgets::haze(&t::read_from_search_none(), "caption"));
        }
        for choice in choices {
            let button = gtk::Button::new();
            button.add_css_class("flat");
            let text = gtk::Label::new(Some(&choice.title));
            text.set_xalign(0.0);
            text.set_ellipsize(gtk::pango::EllipsizeMode::End);
            button.set_child(Some(&text));
            button.connect_clicked({
                let (ctx, work, source, series_ref, dialog, picked) = (
                    Rc::downgrade(ctx),
                    work.clone(),
                    miss.source_id.clone(),
                    choice.series_ref.clone(),
                    dialog.downgrade(),
                    Rc::clone(&state.picked),
                );
                move |_| {
                    let Some(ctx) = ctx.upgrade() else { return };
                    picked.set(true);
                    sources(
                        &ctx.hub,
                        BookSourceCommand::ChooseSourceSeries {
                            work: work.clone(),
                            source_id: source.clone(),
                            series_ref: series_ref.clone(),
                        },
                    );
                    if let Some(dialog) = dialog.upgrade() {
                        dialog.close();
                    }
                }
            });
            column.append(&button);
        }
    }
    gtk::ListBoxRow::builder().child(&column).activatable(false).selectable(false).build()
}

// -------------------------------------------------------------------------------------------
// Trackers

/// The trackers that count reading.
const READING_TRACKERS: [&str; 4] = ["anilist", "mangaupdates", "kitsu", "myanimelist"];

/// The trackers: each one's list and progress with its score, the match to choose when it isn't
/// sure, and which place to keep when they differ. Then the trackers that don't track it.
pub fn trackers(ctx: &Rc<Ctx>, work: &WorkId, view: Option<SeriesView>) {
    let dialog = adw::Dialog::new();
    dialog.set_title(&t::series_tracking());
    dialog.set_content_width(460);
    dialog.set_follows_content_size(true);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
    body.set_margin_top(12);
    body.set_margin_bottom(18);
    body.set_margin_start(18);
    body.set_margin_end(18);
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .max_content_height(520)
        .child(&body)
        .build();
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    let subtitle = view.as_ref().map(|v| v.titles.display.clone()).unwrap_or_default();
    header.set_title_widget(Some(&adw::WindowTitle::new(&t::series_tracking(), &subtitle)));
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&scroll));
    dialog.set_child(Some(&toolbar));
    if let Some(view) = &view {
        draw_trackers(ctx, &dialog, work, view, &view.tracking, &body);
    }
    listen(ctx, &dialog, {
        let (ctx, work, body) = (Rc::downgrade(ctx), work.clone(), body.clone());
        let dialog = dialog.downgrade();
        move |event| {
            let Event::Reading(ReadingEvent::Tracking { work: w, rows }) = event else {
                return;
            };
            if *w != work {
                return;
            }
            let (Some(ctx), Some(dialog), Some(view)) =
                (ctx.upgrade(), dialog.upgrade(), &view)
            else {
                return;
            };
            draw_trackers(&ctx, &dialog, &work, view, rows, &body);
        }
    });
    send(&ctx.hub, ReadingCommand::LoadTracking { work: work.clone() });
    dialog.present(Some(&ctx.window));
}

fn draw_trackers(
    ctx: &Rc<Ctx>,
    dialog: &adw::Dialog,
    work: &WorkId,
    view: &SeriesView,
    rows: &[TrackerReadRow],
    body: &gtk::Box,
) {
    super::clear(body);
    let tracked: Vec<&TrackerReadRow> =
        rows.iter().filter(|r| !matches!(r.mapping, TrackerMapping::Unmapped)).collect();
    let unmapped: Vec<&TrackerReadRow> =
        rows.iter().filter(|r| matches!(r.mapping, TrackerMapping::Unmapped)).collect();
    let away: Vec<&str> = READING_TRACKERS
        .into_iter()
        .filter(|id| !rows.iter().any(|r| r.tracker == *id))
        .collect();
    // The place the trackers agree on, that one behind catches up to.
    let newest = tracked
        .iter()
        .filter(|r| !r.divergent)
        .map(|r| r.chapters)
        .max()
        .or_else(|| tracked.iter().map(|r| r.chapters).max())
        .unwrap_or(0);
    if !tracked.is_empty() {
        let list = widgets::boxed_list();
        for tracker in &tracked {
            tracker_rows(ctx, work, view, tracker, newest, &list);
        }
        body.append(&list);
    }
    if let Some(note) = tracked.iter().find_map(|r| r.split_note.as_ref()) {
        let note = widgets::haze(&say::msg(note), "caption");
        note.set_wrap(true);
        body.append(&note);
    }
    if away.is_empty() && unmapped.is_empty() {
        return;
    }
    let heading = gtk::Label::new(Some(&t::series_not_tracked()));
    heading.add_css_class("heading");
    heading.set_xalign(0.0);
    heading.set_margin_top(6);
    body.append(&heading);
    let list = widgets::boxed_list();
    for tracker in unmapped {
        let line = row(say::tracker_name(&tracker.tracker), &t::series_tracker_not_there());
        line.add_css_class("dim-label");
        list.append(&line);
    }
    for id in away {
        let line = row(say::tracker_name(id), &t::series_tracker_connect());
        line.set_activatable(true);
        line.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
        line.connect_activated({
            let dialog = dialog.downgrade();
            move |line| {
                if let Some(dialog) = dialog.upgrade() {
                    dialog.close();
                }
                let _ =
                    line.activate_action("win.settings-pane", Some(&"accounts".to_variant()));
            }
        });
        list.append(&line);
    }
    body.append(&list);
}

/// One tracker's row, and under it the guesses to choose from when it isn't sure.
fn tracker_rows(
    ctx: &Rc<Ctx>,
    work: &WorkId,
    view: &SeriesView,
    tracker: &TrackerReadRow,
    newest: u32,
    list: &gtk::ListBox,
) {
    let name = say::tracker_name(&tracker.tracker).to_owned();
    let line = match &tracker.mapping {
        TrackerMapping::Looking => t::series_tracker_looking(),
        TrackerMapping::Ask { .. } => t::series_tracker_ask(),
        _ if tracker.divergent && tracker.chapters < newest => {
            t::series_tracker_behind(i64::from(tracker.chapters), i64::from(newest))
        }
        _ if tracker.divergent => t::series_tracker_ahead(i64::from(tracker.chapters)),
        _ => {
            let mut parts: Vec<String> = Vec::new();
            parts.extend(tracker.status.map(super::library::name));
            parts.push(progress(tracker, view));
            parts.join(" \u{b7} ")
        }
    };
    let head = row(&name, &line);
    if tracker.divergent {
        head.add_css_class("divergent");
        let menu = gio::Menu::new();
        let target = |keep: bool| (work.0.clone(), tracker.tracker.clone(), keep).to_variant();
        let keep = gio::MenuItem::new(Some(&t::series_tracker_keep_kaeri()), None);
        keep.set_action_and_target_value(Some("trackers.resolve"), Some(&target(true)));
        menu.append_item(&keep);
        let take = gio::MenuItem::new(Some(&t::series_tracker_take_theirs(&name)), None);
        take.set_action_and_target_value(Some("trackers.resolve"), Some(&target(false)));
        menu.append_item(&take);
        let more = gtk::MenuButton::builder()
            .icon_name("view-more-symbolic")
            .valign(gtk::Align::Center)
            .tooltip_text(t::action_more())
            .menu_model(&menu)
            .build();
        more.add_css_class("flat");
        let actions = gio::SimpleActionGroup::new();
        let resolve = gio::SimpleAction::new(
            "resolve",
            Some(gtk::glib::VariantTy::new("(ssb)").expect("a type")),
        );
        resolve.connect_activate({
            let ctx = Rc::downgrade(ctx);
            move |_, arg| {
                let (Some(ctx), Some((work, tracker, keep_local))) =
                    (ctx.upgrade(), arg.and_then(|a| a.get::<(String, String, bool)>()))
                else {
                    return;
                };
                send(
                    &ctx.hub,
                    ReadingCommand::ResolveConflict { work: WorkId(work), tracker, keep_local },
                );
            }
        });
        actions.add_action(&resolve);
        more.insert_action_group("trackers", Some(&actions));
        head.add_suffix(&more);
    } else if let Some(score) = tracker.score.filter(|s| *s > 0.0) {
        let score = widgets::haze(&t::series_tracker_score(&format_score(score)), "numeric");
        head.add_suffix(&score);
    }
    list.append(&head);
    let TrackerMapping::Ask { candidates } = &tracker.mapping else { return };
    let choose = |tracker_id: Option<String>| {
        let (ctx, work, id) = (Rc::downgrade(ctx), work.clone(), tracker.tracker.clone());
        move |_: &adw::ActionRow| {
            if let Some(ctx) = ctx.upgrade() {
                send(
                    &ctx.hub,
                    ReadingCommand::ChooseMapping {
                        work: work.clone(),
                        tracker: id.clone(),
                        tracker_id: tracker_id.clone(),
                    },
                );
            }
        }
    };
    for candidate in candidates {
        let year = candidate.year.map(|y| y.to_string()).unwrap_or_default();
        let line = row(&candidate.title, &year);
        line.set_activatable(true);
        line.add_suffix(&widgets::haze(
            &t::library_match_percent(i64::from(candidate.confidence)),
            "caption",
        ));
        line.connect_activated(choose(Some(candidate.tracker_id.clone())));
        list.append(&line);
    }
    let none = row(&t::series_tracker_none_of_these(), "");
    none.set_activatable(true);
    none.add_css_class("dim-label");
    none.connect_activated(choose(None));
    list.append(&none);
}

/// "8" or "7.5": a score with one decimal only when it has one.
fn format_score(score: f32) -> String {
    if score.fract().abs() < 0.05 { format!("{score:.0}") } else { format!("{score:.1}") }
}
