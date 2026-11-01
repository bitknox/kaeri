//! Your own magnet link or torrent file: the sheet that takes it, and the confirm step the engine's
//! guess opens. The question is "Is this <title>, episode N?" with Play, Download, Change and Play
//! without tracking. Change lists the titles it could be and searches other words.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use gtk::glib;
use kaeri_core::ids::AnilistId;
use kaeri_i18n::t;
use kaeri_model::{
    catalog::Entry,
    command::Command,
    event::Event,
    opened::{OpenedCommand, OpenedLink},
};

use crate::{widgets, window::Ctx};

const PASTE: &str = "open-link";
const CONFIRM: &str = "opened-link";

/// Take the engine's answers to an opened link, from now on.
pub fn watch(ctx: &Rc<Ctx>) {
    let state: Rc<RefCell<Option<Rc<Confirm>>>> = Rc::default();
    let weak = Rc::downgrade(ctx);
    ctx.hub.listen(move |event| {
        let Some(ctx) = weak.upgrade() else { return };
        match event {
            Event::LinkOpened(link) => {
                if let Some(dialog) = ctx.window.visible_dialog()
                    && matches!(dialog.widget_name().as_str(), PASTE | CONFIRM)
                {
                    dialog.force_close();
                }
                state.replace(Some(Confirm::present(&ctx, link)));
            }
            Event::OpenedChoices { link, query, candidates, failed } => {
                let held = state.borrow().clone();
                if let Some(confirm) = held
                    && confirm.link.link == *link
                {
                    confirm.choices(candidates, failed.as_ref().map(crate::say::msg), query);
                }
            }
            _ => {}
        }
    });
}

/// The sheet that takes a magnet link, an info hash or a torrent file.
pub fn open(ctx: &Rc<Ctx>) {
    let header = adw::HeaderBar::new();
    header.set_show_end_title_buttons(false);
    header.set_show_start_title_buttons(false);
    let cancel = gtk::Button::with_label(&t::action_cancel());
    header.pack_start(&cancel);
    let open = gtk::Button::with_label(&t::opened_open());
    open.add_css_class("suggested-action");
    open.set_sensitive(false);
    header.pack_end(&open);

    let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
    column.set_margin_top(12);
    column.set_margin_bottom(24);
    column.set_margin_start(18);
    column.set_margin_end(18);
    let list = widgets::boxed_list();
    let entry = adw::EntryRow::builder().title(t::opened_paste_placeholder()).build();
    list.append(&entry);
    column.append(&list);
    let file = gtk::Button::with_label(&t::opened_choose_file());
    file.add_css_class("pill");
    file.set_halign(gtk::Align::Start);
    column.append(&file);
    column.set_size_request(400, -1);

    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.set_content(Some(&column));
    let dialog = adw::Dialog::builder()
        .name(PASTE)
        .title(t::opened_paste_title())
        .follows_content_size(true)
        .child(&view)
        .build();

    entry.connect_changed({
        let open = open.clone();
        move |entry| open.set_sensitive(!entry.text().trim().is_empty())
    });
    let send = {
        let (weak, dialog, entry) = (Rc::downgrade(ctx), dialog.clone(), entry.clone());
        move || {
            let input = entry.text().trim().to_owned();
            if input.is_empty() {
                return;
            }
            if let Some(ctx) = weak.upgrade() {
                ctx.hub.send(Command::Opened(OpenedCommand::OpenLink { input }));
            }
            dialog.close();
        }
    };
    open.connect_clicked({
        let send = send.clone();
        move |_| send()
    });
    entry.connect_entry_activated(move |_| send());
    cancel.connect_clicked({
        let dialog = dialog.clone();
        move |_| {
            dialog.close();
        }
    });
    file.connect_clicked({
        let (weak, dialog) = (Rc::downgrade(ctx), dialog.clone());
        move |_| {
            let Some(ctx) = weak.upgrade() else { return };
            let filter = gtk::FileFilter::new();
            filter.add_suffix("torrent");
            filter.add_mime_type("application/x-bittorrent");
            let picker = gtk::FileDialog::builder().title(t::opened_choose_file()).build();
            picker.set_default_filter(Some(&filter));
            let dialog = dialog.clone();
            let window = ctx.window.clone();
            picker.open(Some(&window), gtk::gio::Cancellable::NONE, move |picked| {
                let Ok(file) = picked else { return };
                let Some(path) = file.path() else { return };
                ctx.hub.send(Command::Opened(OpenedCommand::OpenLink {
                    input: path.to_string_lossy().into_owned(),
                }));
                dialog.close();
            });
        }
    });
    dialog.present(Some(&ctx.window));
    entry.grab_focus();
}

/// A magnet link or torrent file handed to the app by the system: sent as if typed.
pub fn open_input(ctx: &Rc<Ctx>, input: &str) {
    ctx.hub.send(Command::Opened(OpenedCommand::OpenLink { input: input.to_owned() }));
}

/// Whether `link` is one this page takes: a magnet link or a path to a torrent file.
pub fn takes(link: &str) -> bool {
    let lower = link.to_ascii_lowercase();
    lower.starts_with("magnet:") || lower.ends_with(".torrent")
}

/// The confirm step on screen.
struct Confirm {
    ctx: Rc<Ctx>,
    dialog: adw::Dialog,
    link: OpenedLink,
    /// The title chosen, from the guess or from Change.
    chosen: RefCell<Option<(AnilistId, String)>>,
    question: gtk::Label,
    episode: adw::EntryRow,
    change: gtk::Box,
    results: gtk::ListBox,
    note: gtk::Label,
    play: gtk::Button,
    download: gtk::Button,
}

impl Confirm {
    fn present(ctx: &Rc<Ctx>, link: &OpenedLink) -> Rc<Self> {
        let guess = link.guess.as_ref();
        let chosen = guess.map(|g| (g.title.id, g.title.title.clone()));
        let episode_known = guess.and_then(|g| g.episode.clone());

        let header = adw::HeaderBar::new();
        header.set_show_end_title_buttons(false);
        header.set_show_start_title_buttons(false);
        let cancel = gtk::Button::with_label(&t::action_cancel());
        header.pack_start(&cancel);

        let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
        column.set_margin_top(12);
        column.set_margin_bottom(24);
        column.set_margin_start(18);
        column.set_margin_end(18);
        column.set_size_request(400, -1);
        let question = gtk::Label::new(None);
        question.set_wrap(true);
        question.set_xalign(0.0);
        question.add_css_class("title-4");
        column.append(&question);
        if let Some(name) = &link.name {
            let name = widgets::haze(name, "caption");
            name.set_xalign(0.0);
            name.set_wrap(true);
            column.append(&name);
        }
        let episode_list = widgets::boxed_list();
        let episode = adw::EntryRow::builder().title(t::opened_episode()).build();
        if let Some(ep) = &episode_known {
            episode.set_text(ep);
        }
        episode_list.append(&episode);
        episode_list.set_visible(chosen.is_some() && episode_known.is_none());
        column.append(&episode_list);

        let change = gtk::Box::new(gtk::Orientation::Vertical, 8);
        change.set_visible(false);
        let heading = widgets::haze(&t::opened_change_title(), "heading");
        heading.set_xalign(0.0);
        change.append(&heading);
        let search =
            gtk::SearchEntry::builder().placeholder_text(t::opened_change_search()).build();
        change.append(&search);
        let results = widgets::boxed_list();
        results.set_selection_mode(gtk::SelectionMode::None);
        change.append(&results);
        let note = widgets::haze("", "caption");
        note.set_xalign(0.0);
        note.set_visible(false);
        change.append(&note);
        column.append(&change);

        let buttons = adw::WrapBox::builder().child_spacing(8).line_spacing(8).build();
        let play = gtk::Button::with_label(&if link.playback_on {
            t::opened_play()
        } else {
            t::opened_turn_on()
        });
        play.add_css_class("pill");
        play.add_css_class("suggested-action");
        let download = gtk::Button::with_label(&t::opened_download());
        download.add_css_class("pill");
        let change_button = gtk::Button::with_label(&t::opened_change());
        change_button.add_css_class("pill");
        buttons.append(&play);
        if link.playback_on {
            buttons.append(&download);
        }
        buttons.append(&change_button);
        column.append(&buttons);
        let untracked = gtk::Button::with_label(&t::opened_play_untracked());
        untracked.add_css_class("flat");
        untracked.set_halign(gtk::Align::Start);
        untracked.set_sensitive(link.playback_on);
        column.append(&untracked);
        let detail = widgets::haze(&t::opened_untracked_detail(), "caption");
        detail.set_xalign(0.0);
        detail.set_wrap(true);
        column.append(&detail);

        let view = adw::ToolbarView::new();
        view.add_top_bar(&header);
        view.set_content(Some(&column));
        let dialog = adw::Dialog::builder()
            .name(CONFIRM)
            .title(t::opened_open())
            .follows_content_size(true)
            .child(&view)
            .build();

        let this = Rc::new(Self {
            ctx: Rc::clone(ctx),
            dialog: dialog.clone(),
            link: link.clone(),
            chosen: RefCell::new(chosen),
            question,
            episode: episode.clone(),
            change: change.clone(),
            results,
            note,
            play: play.clone(),
            download: download.clone(),
        });
        this.ask(episode_known.as_deref());
        this.results_from(&link.candidates);

        let weak = Rc::downgrade(&this);
        episode.connect_changed({
            let weak = weak.clone();
            move |_| {
                if let Some(this) = weak.upgrade() {
                    this.sensitive();
                }
            }
        });
        cancel.connect_clicked({
            let dialog = dialog.clone();
            move |_| {
                dialog.close();
            }
        });
        change_button.connect_clicked({
            let (change, list, weak) = (change.clone(), episode_list.clone(), weak.clone());
            move |_| {
                change.set_visible(true);
                if let Some(this) = weak.upgrade() {
                    list.set_visible(this.chosen.borrow().is_some());
                }
            }
        });
        search.connect_activate({
            let weak = weak.clone();
            move |search| {
                let query = search.text().trim().to_owned();
                if query.is_empty() {
                    return;
                }
                if let Some(this) = weak.upgrade() {
                    this.ctx.hub.send(Command::Opened(OpenedCommand::Find {
                        link: this.link.link.clone(),
                        query,
                    }));
                }
            }
        });
        play.connect_clicked({
            let weak = weak.clone();
            move |_| {
                if let Some(this) = weak.upgrade() {
                    this.start(false);
                }
            }
        });
        download.connect_clicked({
            let weak = weak.clone();
            move |_| {
                if let Some(this) = weak.upgrade() {
                    this.start(true);
                }
            }
        });
        untracked.connect_clicked({
            let weak = weak.clone();
            move |_| {
                if let Some(this) = weak.upgrade() {
                    let name = this.link.name.clone().unwrap_or_else(|| this.link.link.clone());
                    this.ctx.hub.send(Command::Opened(OpenedCommand::PlayUntracked {
                        link: this.link.link.clone(),
                        name,
                    }));
                    this.dialog.close();
                }
            }
        });
        this.sensitive();
        dialog.present(Some(&ctx.window));
        this
    }

    /// The question the guess asks, or says there is none.
    fn ask(&self, episode: Option<&str>) {
        let line = match (&*self.chosen.borrow(), episode) {
            (Some((_, title)), Some(ep)) => t::opened_confirm(title, ep),
            (Some((_, title)), None) => t::opened_confirm_title(title),
            (None, _) => t::opened_no_guess(),
        };
        self.question.set_label(&line);
        if self.chosen.borrow().is_none() {
            self.change.set_visible(true);
        }
    }

    /// Play and Download wait for a title and an episode.
    fn sensitive(&self) {
        let ready = self.chosen.borrow().is_some() && !self.episode.text().trim().is_empty();
        self.play.set_sensitive(ready || !self.link.playback_on);
        self.download.set_sensitive(ready);
    }

    /// Play, or Download, as the title and episode shown. While playback is off, Play is Turn on
    /// torrent playback instead, which opens its setting.
    fn start(&self, download: bool) {
        if !download && !self.link.playback_on {
            self.dialog.close();
            let _ = WidgetExt::activate_action(
                &self.ctx.window,
                "win.settings-pane",
                Some(&"torrents".to_variant()),
            );
            return;
        }
        let Some((id, title)) = self.chosen.borrow().clone() else { return };
        let episode = self.episode.text().trim().to_owned();
        let link = self.link.link.clone();
        let command = if download {
            OpenedCommand::Download { link, id, episode }
        } else {
            OpenedCommand::Play { link, id, episode, title }
        };
        self.ctx.hub.send(Command::Opened(command));
        self.dialog.close();
    }

    /// The titles Change offers, and what a search that found nothing says.
    fn choices(self: &Rc<Self>, candidates: &[Entry], failed: Option<String>, query: &str) {
        self.results_from(candidates);
        let line =
            failed.or_else(|| candidates.is_empty().then(|| t::search_nothing_found(query)));
        self.note.set_visible(line.is_some());
        self.note.set_label(&line.unwrap_or_default());
    }

    fn results_from(self: &Rc<Self>, candidates: &[Entry]) {
        while let Some(row) = self.results.first_child() {
            self.results.remove(&row);
        }
        self.results.set_visible(!candidates.is_empty());
        for entry in candidates {
            let row = adw::ActionRow::builder()
                .title(glib::markup_escape_text(&entry.title))
                .subtitle(crate::say::cover_caption(entry))
                .activatable(true)
                .build();
            row.set_title_lines(1);
            let (weak, id, title) = (Rc::downgrade(self), entry.id, entry.title.clone());
            row.connect_activated(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.pick(id, &title);
                }
            });
            self.results.append(&row);
        }
    }

    /// A title from Change: asked about again, with the episode to fill in.
    fn pick(&self, id: AnilistId, title: &str) {
        self.chosen.replace(Some((id, title.to_owned())));
        let episode = self.episode.text().trim().to_owned();
        self.ask((!episode.is_empty()).then_some(episode.as_str()));
        if let Some(list) = self.episode.parent() {
            list.set_visible(true);
        }
        self.sensitive();
    }
}
