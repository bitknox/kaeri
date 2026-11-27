//! Choose source: what each of this device's sources lists for an episode, loading in place;
//! the one picked plays.

use std::{cell::Cell, rc::Rc};

use adw::prelude::*;
use kaeri_core::{ids::AnilistId, traits::SourceCandidate};
use kaeri_i18n::t;
use kaeri_model::{
    command::Command,
    event::Event,
    notice::NoticeKind,
    starting::{ListingMiss, MissedListing, SourceHold},
};

use crate::{
    widgets,
    window::{Ctx, Size},
};

/// An episode as the catalogue counts it. The engine names one by its number as text.
pub fn number(episode: &str) -> i64 {
    episode.trim().parse().unwrap_or(0)
}

/// "Episode 7 of Stardust Wayfarer", or "Episode 7" before the title is known.
fn episode_of(ctx: &Ctx, id: AnilistId, episode: &str) -> String {
    match ctx.hub.entry(id) {
        Some(entry) => t::party_form_episode_of(number(episode), &entry.title),
        None => t::party_form_episode(number(episode)),
    }
}

/// What each source lists for the episode, loading in place; the one picked plays with the
/// room.
struct Choose {
    ctx: Rc<Ctx>,
    dialog: adw::Dialog,
    id: AnilistId,
    episode: String,
    spinner: adw::Spinner,
    mark: gtk::Image,
    line: gtk::Label,
    list: gtk::ListBox,
    again: gtk::Button,
    providers: gtk::Button,
    /// Waiting on the list: an alert in the meantime is why it found nothing.
    looking: Cell<bool>,
}

/// The second step: the sources' list for the episode.
pub fn choose(ctx: &Rc<Ctx>, id: AnilistId, episode: &str) {
    let header = adw::HeaderBar::new();
    header.set_show_end_title_buttons(false);
    header.set_show_start_title_buttons(false);
    let cancel = gtk::Button::with_label(&t::action_cancel());
    header.pack_start(&cancel);
    let again = gtk::Button::with_label(&t::action_try_again());
    again.add_css_class("suggested-action");
    again.set_visible(false);
    header.pack_end(&again);
    let providers = gtk::Button::with_label(&t::party_open_sources());
    providers.add_css_class("suggested-action");
    providers.set_visible(false);
    header.pack_end(&providers);

    let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
    column.set_margin_top(12);
    column.set_margin_bottom(24);
    column.set_margin_start(18);
    column.set_margin_end(18);
    let what = gtk::Label::new(Some(&episode_of(ctx, id, episode)));
    what.add_css_class("heading");
    what.set_xalign(0.0);
    what.set_wrap(true);
    column.append(&what);
    let spinner = adw::Spinner::new();
    spinner.set_size_request(16, 16);
    let mark = gtk::Image::from_icon_name("dialog-error-symbolic");
    mark.add_css_class("error");
    mark.set_visible(false);
    let line = widgets::haze(&t::party_looking_sources(), "");
    line.set_hexpand(true);
    let waiting = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    waiting.append(&spinner);
    waiting.append(&mark);
    waiting.append(&line);
    column.append(&waiting);
    let list = widgets::boxed_list();
    list.set_visible(false);
    list.update_property(&[gtk::accessible::Property::Label(&t::party_sources_label())]);
    column.append(&list);
    if ctx.size() != Size::Narrow {
        column.set_size_request(460, -1);
    }
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .max_content_height(520)
        .child(&column)
        .build();
    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.set_content(Some(&scroll));
    let dialog = adw::Dialog::builder()
        .title(t::party_choose_a_source())
        .follows_content_size(true)
        .child(&view)
        .build();

    let state = Rc::new(Choose {
        ctx: Rc::clone(ctx),
        dialog: dialog.clone(),
        id,
        episode: episode.to_owned(),
        spinner,
        mark,
        line,
        list,
        again: again.clone(),
        providers: providers.clone(),
        looking: Cell::new(false),
    });
    cancel.connect_clicked({
        let dialog = dialog.clone();
        move |_| {
            dialog.close();
        }
    });
    again.connect_clicked({
        let weak = Rc::downgrade(&state);
        move |_| {
            if let Some(state) = weak.upgrade() {
                state.look();
            }
        }
    });
    providers.connect_clicked({
        let weak = Rc::downgrade(&state);
        move |_| {
            let Some(state) = weak.upgrade() else { return };
            state.dialog.close();
            let _ = WidgetExt::activate_action(
                &state.ctx.window,
                "win.go",
                Some(&"providers".to_variant()),
            );
        }
    });
    let listening = ctx.hub.listen({
        let weak = Rc::downgrade(&state);
        move |event| {
            let Some(state) = weak.upgrade() else { return };
            match event {
                Event::Sources { id, episode, candidates, missed }
                    if state.looking.get() && *id == state.id && *episode == state.episode =>
                {
                    state.listed(candidates, missed);
                }
                // The engine says why it found nothing as an alert, not as an empty list.
                Event::Toast(notice)
                    if state.looking.get() && notice.kind == NoticeKind::Alert =>
                {
                    state.failed(&notice.msg.as_ref().map(crate::say::msg).unwrap_or_default());
                }
                _ => {}
            }
        }
    });
    let hub = Rc::downgrade(&ctx.hub);
    dialog.connect_closed(move |_| {
        if let Some(hub) = hub.upgrade() {
            hub.forget(listening);
        }
    });
    widgets::keep(&dialog, Rc::clone(&state));
    state.look();
    dialog.present(Some(&ctx.window));
}

impl Choose {
    fn look(&self) {
        self.looking.set(true);
        self.again.set_visible(false);
        self.providers.set_visible(false);
        self.spinner.set_visible(true);
        self.mark.set_visible(false);
        self.line.add_css_class("haze");
        self.line.set_label(&t::party_looking_sources());
        self.list.set_visible(false);
        self.ctx.hub.send(Command::LoadSources { id: self.id, episode: self.episode.clone() });
    }

    fn failed(&self, why: &str) {
        self.looking.set(false);
        self.spinner.set_visible(false);
        self.mark.set_visible(true);
        // The reason in silk beside the mark.
        self.line.remove_css_class("haze");
        self.line.set_label(why);
        self.again.set_visible(true);
    }

    fn listed(self: &Rc<Self>, found: &[SourceCandidate], missed: &[MissedListing]) {
        self.looking.set(false);
        self.spinner.set_visible(false);
        while let Some(row) = self.list.first_child() {
            self.list.remove(&row);
        }
        if found.is_empty() && !missed.is_empty() {
            // What each source said: one that failed or was held back is not one without the
            // episode, and Try again asks it anew. The line over them claims only what the
            // sources that answered said.
            self.line.set_label(&none_lists_line(missed, &self.episode));
            for miss in missed {
                let row = adw::ActionRow::builder()
                    .title(gtk::glib::markup_escape_text(&missed_line(miss, &self.episode)))
                    .activatable(false)
                    .build();
                row.set_title_lines(2);
                self.list.append(&row);
            }
            self.list.set_visible(true);
            let reachable = missed.iter().any(|m| unreached(&m.miss));
            self.again.set_visible(reachable);
            self.providers.set_visible(!reachable);
            return;
        }
        if found.is_empty() {
            self.line.set_label(&t::party_sources_none_listed(number(&self.episode)));
            self.providers.set_visible(true);
            return;
        }
        // In a room the pick plays for the room; outside one, for this player or the TV.
        let in_room = self.ctx.hub.known.borrow().room.is_some();
        self.line.set_label(&if in_room {
            t::room_sources_found_line(found.len() as i64)
        } else {
            t::player_sources_found_line(found.len() as i64)
        });
        for candidate in found {
            let row = adw::ActionRow::builder()
                .title(gtk::glib::markup_escape_text(&candidate.title))
                .subtitle(gtk::glib::markup_escape_text(&detail(candidate)))
                .activatable(true)
                .build();
            row.set_title_lines(1);
            row.set_subtitle_lines(1);
            row.add_suffix(&widgets::chevron());
            let pick = candidate.clone();
            let weak = Rc::downgrade(self);
            row.connect_activated(move |_| {
                let Some(state) = weak.upgrade() else { return };
                // In the room already: this play plays with it.
                state.ctx.hub.send(Command::PlaySource {
                    id: state.id,
                    episode: state.episode.clone(),
                    provider_id: pick.provider_id.clone(),
                    source_id: pick.id.clone(),
                });
                state.dialog.close();
            });
            self.list.append(&row);
        }
        self.list.set_visible(true);
        crate::motion::fade_in(&self.list);
    }
}

/// Whether a source's miss was one where it gave no real answer: Try again may reach it.
/// The line over the reasons when no source lists the episode: "No source lists episode 7."
/// only when every source answered, "No source that answered lists episode 7." when any failed,
/// took too long or wasn't asked.
pub(crate) fn none_lists_line(missed: &[MissedListing], episode: &str) -> String {
    if missed.iter().any(|m| unreached(&m.miss)) {
        t::source_picker_none_answered_lists(episode)
    } else {
        t::source_picker_none_lists(episode)
    }
}

fn unreached(miss: &ListingMiss) -> bool {
    matches!(miss, ListingMiss::Failed | ListingMiss::TimedOut | ListingMiss::HeldBack { .. })
}

/// Why one source listed no release: its name once, as it goes by.
pub(crate) fn missed_line(missed: &MissedListing, episode: &str) -> String {
    let name = missed.name.as_str();
    match &missed.miss {
        ListingMiss::NoMatch => t::player_start_miss_no_match(name),
        ListingMiss::Ambiguous => t::player_start_miss_ambiguous(name),
        ListingMiss::NoEpisode => t::player_start_miss_no_episode(name, episode),
        ListingMiss::Failed => t::player_start_miss_failed(name),
        ListingMiss::TimedOut => t::player_start_miss_timed_out(name),
        ListingMiss::HeldBack { why } => match why {
            SourceHold::TorrentsOff => t::source_picker_held_torrents_off(name),
            SourceHold::TorrentsStarting => t::source_picker_held_torrents_starting(name),
            SourceHold::TorrentsFailed => t::source_picker_held_torrents_failed(name),
            SourceHold::ProxyCheck => t::source_picker_held_proxy(name),
            SourceHold::Cellular => t::source_picker_held_cellular(name),
            SourceHold::Other => t::source_picker_held_other(name),
        },
    }
}

/// The name a release's source goes by, and its id when the engine gave none.
fn source_name(candidate: &SourceCandidate) -> String {
    if candidate.provider_name.is_empty() {
        candidate.provider_id.clone()
    } else {
        candidate.provider_name.clone()
    }
}

/// "Sample Source · 1080p · 1.4 GiB · 12 seeders", the last being the source's own label.
fn detail(candidate: &SourceCandidate) -> String {
    let mut parts = vec![source_name(candidate)];
    if let Some(quality) = candidate.quality {
        parts.push(format!("{quality}p"));
    }
    if let Some(size) = candidate.size.as_deref().filter(|s| !s.is_empty()) {
        parts.push(size.to_owned());
    }
    if let Some(label) = candidate.label.as_deref().filter(|l| !l.is_empty()) {
        parts.push(label.to_owned());
    }
    parts.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn miss(name: &str, miss: ListingMiss) -> MissedListing {
        MissedListing { name: name.into(), miss }
    }

    #[test]
    fn the_line_over_the_reasons_claims_only_what_the_sources_that_answered_said() {
        let answered =
            [miss("Brook", ListingMiss::NoEpisode), miss("Kumo", ListingMiss::NoMatch)];
        assert_eq!(none_lists_line(&answered, "7"), "No source lists episode 7.");
        let failed =
            [miss("Alpha", ListingMiss::Failed), miss("Brook", ListingMiss::NoEpisode)];
        assert_eq!(none_lists_line(&failed, "7"), "No source that answered lists episode 7.");
        let held = [
            miss("Brook", ListingMiss::NoEpisode),
            miss("Harbor", ListingMiss::HeldBack { why: SourceHold::TorrentsOff }),
        ];
        assert_eq!(none_lists_line(&held, "7"), "No source that answered lists episode 7.");
    }
}
