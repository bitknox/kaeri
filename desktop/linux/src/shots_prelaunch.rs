//! The reading prelaunch shots, one per state the board draws: what came in from each add form,
//! the demo as the only source, first run's join page, the medium questions, and the server
//! progress lines. The `prelaunch` run takes them on the reading scene; the `pl-demo-` ones need
//! the demo as the only source (`--demo --scene reading`).

use std::rc::Rc;

use adw::prelude::*;
use kaeri_core::books::ChapterId;
use kaeri_i18n::t;
use kaeri_model::{
    book_sources::{
        BookPluginRow, BookServerKind, BookServerRow, BookSourceEvent, BookSourcesView,
    },
    came_in::{
        CameIn, FolderReport, FormatCount, NothingCameIn, PluginsInstalled, ServerFound,
        ServerLibrary, StoreReport,
    },
    event::Event,
    group::{FirstRunPage, JoinedSetup, SyncBookServerOffer, SyncStoreOffer},
    medium::{KaeriIsFor, MediumAsk},
    stores::{StorePluginRow, StorePluginStatus, StoreRow, StoreState},
};

use crate::{
    pages::{Dest, first_launch},
    reading::{
        came_in,
        server_progress::{self, EndLine, RowLine, ServerLine, WhyNot},
    },
    window::Shell,
};

/// Every shot of the reading scene's run, in order.
pub const NAMES: [&str; 37] = [
    "pl-added-01",
    "pl-added-02",
    "pl-added-03",
    "pl-added-04",
    "pl-added-05",
    "pl-added-06",
    "pl-added-07",
    "pl-added-08",
    "pl-added-09",
    "pl-added-10",
    "pl-added-11",
    "pl-added-12",
    "pl-added-13",
    "pl-added-14",
    "pl-added-15",
    "pl-added-16",
    "pl-added-first-01",
    "pl-added-first-16",
    "pl-join-01",
    "pl-join-02",
    "pl-join-03",
    "pl-join-04",
    "pl-join-05",
    "pl-join-06",
    "pl-join-07",
    "pl-join-08",
    "pl-medium-first",
    "pl-medium-updated",
    "pl-medium-joined",
    "pl-progress-lines",
    "pl-progress-06",
    "pl-progress-07",
    "pl-progress-08",
    "pl-progress-09",
    "pl-progress-10",
    "pl-sources-plugin",
    "pl-sources-undo",
];

/// Every shot of the demo's run, in order.
pub const DEMO: [&str; 8] = [
    "pl-demo-01",
    "pl-demo-02",
    "pl-demo-03",
    "pl-demo-04",
    "pl-demo-05",
    "pl-demo-06",
    "pl-demo-07",
    "pl-demo-08",
];

/// A Komga server that asks for a username and password.
const KOMGA: &str = "comics.home.example";
const KAVITA: &str = "books.home.example";
const OPDS: &str = "comics.home.example";
const DEVICE: &str = "Kim’s iPhone";
const STORE: &str = "Kim’s shelf";

fn found(empty: bool) -> ServerFound {
    ServerFound {
        pending: "pending-1".into(),
        kind: BookServerKind::Kavita,
        name: "Kavita".into(),
        host: KAVITA.into(),
        address: format!("https://{KAVITA}"),
        series: Some(if empty { 0 } else { 212 }),
        libraries: if empty {
            vec![]
        } else {
            vec![
                ServerLibrary { id: "1".into(), name: "Comics".into(), series: Some(168) },
                ServerLibrary { id: "2".into(), name: "Light novels".into(), series: Some(44) },
            ]
        },
        key_last4: Some("7f3a".into()),
    }
}

fn folder(readable: bool) -> FolderReport {
    FolderReport {
        id: "folder-1".into(),
        name: "Comics".into(),
        path: Some("/home/kim/Comics".into()),
        series: if readable { 41 } else { 0 },
        books: if readable { 286 } else { 0 },
        files: 312,
        matched: if readable { 38 } else { 0 },
        unsupported: vec![
            FormatCount { format: "MOBI".into(), count: 14 },
            FormatCount { format: "AZW3".into(), count: 6 },
        ],
        locked: vec![FormatCount { format: "EPUB".into(), count: 3 }],
        listed: vec!["Harbor Lights 01.mobi".into(), "Harbor Lights 02.mobi".into()],
    }
}

fn store(reading: usize, watching: usize, other: u32) -> StoreReport {
    StoreReport {
        store: "store-1".into(),
        name: STORE.into(),
        owner: Some("Kim".into()),
        for_reading: (0..reading).map(|n| format!("plugin-{n}")).collect(),
        for_watching: (0..watching).map(|n| format!("watch-{n}")).collect(),
        other_medium: other,
        other_is_reading: false,
        install_all: reading >= 2,
    }
}

const KAVITA_LINK: &str = "https://books.home.example/api/opds/9d2c41e07b5e7f3a";
const INVITE: &str =
    "https://store.home.example/join/kq83-fz2m-7q2m#c2hlbGYta2V5LWZvci1zaG90cw";

/// Your sources as the board draws it: the scene's folder, two servers and a plugin.
fn sources_view(shell: &Rc<Shell>) -> BookSourcesView {
    let mut view = shell.ctx.hub.known.borrow().book_sources.clone().unwrap_or_default();
    let server = |id: &str, kind, name: &str, host: &str, series, libraries| BookServerRow {
        id: id.into(),
        kind,
        name: name.into(),
        address: format!("https://{host}"),
        series,
        report_progress: kind != BookServerKind::Opds,
        off: false,
        signed_out: false,
        libraries,
        empty: false,
        rechecks_at: None,
        key_last4: None,
        progress_pending: 0,
        progress_sending: false,
        progress_saved_at: None,
        progress_problem: None,
        progress_problem_at: None,
    };
    view.servers = vec![
        server("server-kavita", BookServerKind::Kavita, "Kavita", KAVITA, 212, 2),
        server("server-opds", BookServerKind::Opds, "OPDS", OPDS, 38, 1),
    ];
    view.plugins = vec![BookPluginRow {
        id: "tsuru".into(),
        name: "Tsuru".into(),
        version: "1.4".into(),
        media: vec![],
        languages: vec!["en".into()],
        off: false,
        store: Some(STORE.into()),
    }];
    view
}

fn show_sources(shell: &Rc<Shell>) {
    let view = sources_view(shell);
    shell.ctx.hub.show_instead(vec![Event::BookSources(BookSourceEvent::View(view))]);
    shell.go(Dest::ReadSources);
}

fn row_line(shell: &Rc<Shell>, line: RowLine) {
    let model = server_progress::model(&shell.ctx);
    model.stage_row("server-kavita", Some(line), false);
}

fn setup(stores: bool) -> JoinedSetup {
    JoinedSetup {
        device: "device-kim".into(),
        device_name: DEVICE.into(),
        medium: Some(KaeriIsFor::Read),
        stores: if stores {
            vec![SyncStoreOffer {
                address: "https://store.home.example".into(),
                name: STORE.into(),
                invite_from: Some(DEVICE.into()),
            }]
        } else {
            vec![]
        },
        servers: vec![SyncBookServerOffer {
            from: "device-kim".into(),
            id: "server-kavita".into(),
            kind: BookServerKind::Kavita,
            name: "Kavita".into(),
            host: KAVITA.into(),
            address: format!("https://{KAVITA}"),
            user: None,
            key_last4: Some("7f3a".into()),
            libraries: vec!["Comics".into(), "Light novels".into()],
            added: false,
        }],
        signed_in: vec!["AniList".into(), "MangaUpdates".into()],
        folders: 1,
        answered: vec![FirstRunPage::Medium, FirstRunPage::Lists, FirstRunPage::Sources],
    }
}

fn trackers() -> Vec<String> {
    vec!["AniList".into(), "MangaUpdates".into()]
}

/// The server form, over Your sources, standing as `came` left it.
fn server_form(shell: &Rc<Shell>, address: &str, came: Option<CameIn>, looking: bool) -> f64 {
    shell.go(Dest::ReadSources);
    came_in::shot_server(&shell.ctx, address, came, looking);
    3.0
}

/// The joined store's plugins, as the store form lists them: two for reading, two for watching.
fn seed_store(shell: &Rc<Shell>, installed: bool) {
    let plugin =
        |id: &str, name: &str, version: &str, hosts: usize, books: bool| StorePluginRow {
            id: id.into(),
            name: name.into(),
            version: version.into(),
            hosts: (0..hosts).map(|n| format!("cdn{n}.pages.example")).collect(),
            status: if installed {
                StorePluginStatus::Installed
            } else {
                StorePluginStatus::Available
            },
            books,
            media: vec![],
        };
    let row = StoreRow {
        id: "store-1".into(),
        name: STORE.into(),
        url: "https://store.home.example".into(),
        host: "store.home.example".into(),
        fingerprint: "7Q2M 4KD1 9XLA 0PE3".into(),
        member: "kim-laptop".into(),
        owner: false,
        state: StoreState::Ready,
        checked_at: None,
        plugins: vec![
            plugin("plugin-0", "Tsuru", "1.4", 2, true),
            plugin("plugin-1", "Lantern Pages", "0.9", 1, true),
            plugin("watch-0", "Kawa", "2.1", 3, false),
            plugin("watch-1", "Hotaru", "1.0", 2, false),
        ],
        owners: vec!["Kim".into()],
        open: false,
        anonymous: false,
    };
    let mut known = shell.ctx.hub.known.borrow_mut();
    known.stores.retain(|s| s.id != row.id);
    known.stores.push(row);
}

fn store_form(shell: &Rc<Shell>, came: Option<CameIn>) -> f64 {
    shell.go(Dest::ReadSources);
    came_in::shot_store(&shell.ctx, INVITE, came);
    3.0
}

/// The five lines the page between chapters can show, one under another, as a component: the
/// end page that holds one is the reader's.
fn progress_lines(shell: &Rc<Shell>) -> f64 {
    let ctx = &shell.ctx;
    let model = server_progress::model(ctx);
    let column = gtk::Box::new(gtk::Orientation::Vertical, 22);
    column.set_margin_top(28);
    column.set_margin_bottom(28);
    column.set_margin_start(28);
    column.set_margin_end(28);
    let lines = [
        ServerLine::Saved(KAVITA.into()),
        ServerLine::Sending(KAVITA.into()),
        ServerLine::Offline(KAVITA.into()),
        ServerLine::Retrying(KAVITA.into()),
        ServerLine::Failed(KAVITA.into(), WhyNot::Key),
    ];
    let mut kept = Vec::new();
    for (at, line) in lines.into_iter().enumerate() {
        let chapter = ChapterId(format!("shot-chapter-{at}"));
        model.stage_line(&chapter, "server-kavita", Some(line));
        let end = EndLine::new(ctx, &chapter);
        let head = gtk::Label::new(Some(&t::reader_end_of_chapter_number("12")));
        head.add_css_class("haze");
        head.set_xalign(0.0);
        let block = gtk::Box::new(gtk::Orientation::Vertical, 4);
        block.append(&head);
        block.append(&end.widget);
        column.append(&block);
        kept.push(end);
    }
    let dialog = adw::Dialog::new();
    dialog.set_content_width(420);
    dialog.set_child(Some(&column));
    crate::widgets::keep(&column, kept);
    dialog.present(Some(&ctx.window));
    2.5
}

/// Put the window in the state `shot` shows, and say how long to let it settle, when it is one.
pub fn stage(shell: &Rc<Shell>, shot: &str) -> Option<f64> {
    let ctx = &shell.ctx;
    Some(match shot {
        "pl-added-01" => server_form(shell, KAVITA_LINK, None, false),
        "pl-added-02" => server_form(shell, KAVITA_LINK, None, true),
        "pl-added-03" => {
            server_form(shell, KAVITA_LINK, Some(CameIn::Found(found(false))), false)
        }
        "pl-added-04" => server_form(
            shell,
            KOMGA,
            Some(CameIn::Empty(NothingCameIn::Unreachable { host: KOMGA.into() })),
            false,
        ),
        "pl-added-05" => {
            shell.go(Dest::ReadSources);
            came_in::shot_server_refused(
                ctx,
                KOMGA,
                ("kim", "lantern-ferry"),
                CameIn::Empty(NothingCameIn::WrongPassword {
                    host: KOMGA.into(),
                    kind: BookServerKind::Komga,
                }),
            );
            3.0
        }
        "pl-added-06" => server_form(
            shell,
            KAVITA_LINK,
            Some(CameIn::Empty(NothingCameIn::WrongKey { host: KAVITA.into() })),
            false,
        ),
        "pl-added-07" => server_form(
            shell,
            KAVITA_LINK,
            Some(CameIn::Empty(NothingCameIn::EmptyCatalogue { found: found(true) })),
            false,
        ),
        "pl-added-08" => server_form(
            shell,
            "photos.home.example",
            Some(CameIn::Empty(NothingCameIn::NotABookServer {
                host: "photos.home.example".into(),
            })),
            false,
        ),
        "pl-added-09" => {
            shell.go(Dest::ReadSources);
            came_in::shot_folder(ctx, CameIn::Folder(folder(true)));
            3.0
        }
        "pl-added-10" => {
            shell.go(Dest::ReadSources);
            came_in::shot_folder(
                ctx,
                CameIn::Empty(NothingCameIn::UnsupportedFiles { folder: folder(false) }),
            );
            3.0
        }
        "pl-added-11" => {
            seed_store(shell, false);
            store_form(shell, Some(CameIn::Store(store(2, 0, 1))))
        }
        "pl-added-12" => {
            seed_store(shell, false);
            let settle = store_form(shell, Some(CameIn::Store(store(2, 0, 1))));
            seed_store(shell, true);
            came_in::shot_store_answer(&CameIn::Installed(PluginsInstalled {
                store: "store-1".into(),
                name: STORE.into(),
                installed: 2,
                failed: vec![],
            }));
            settle
        }
        "pl-added-13" => {
            seed_store(shell, false);
            store_form(
                shell,
                Some(CameIn::Empty(NothingCameIn::OtherMedium { store: store(0, 0, 2) })),
            )
        }
        "pl-added-14" => store_form(
            shell,
            Some(CameIn::Empty(NothingCameIn::EmptyStore { store: store(0, 0, 0) })),
        ),
        "pl-added-15" => store_form(
            shell,
            Some(CameIn::Empty(NothingCameIn::StoreUnreachable {
                host: "store.home.example".into(),
            })),
        ),
        "pl-added-16" => {
            came_in::shot_just_added("server-kavita");
            show_sources(shell);
            ctx.toast(&t::source_added(KAVITA, 212));
            3.0
        }
        "pl-added-first-01" => {
            first_launch::shot(ctx, "sources", Some(KaeriIsFor::Read))?;
            came_in::shot_server(ctx, KAVITA_LINK, None, false);
            3.5
        }
        "pl-added-first-16" => {
            first_launch::shot(ctx, "sources", Some(KaeriIsFor::Read))?;
            first_launch::shot_added(
                ctx,
                came_in::Added {
                    icon: "network-server-symbolic",
                    title: KAVITA.into(),
                    line: t::source_row_server(
                        &came_in::kind_words(BookServerKind::Kavita),
                        212,
                        2,
                    ),
                },
            )?;
            first_launch::shot_added(
                ctx,
                came_in::Added {
                    icon: "folder-symbolic",
                    title: "Comics".into(),
                    line: t::source_row_folder("/home/kim/Comics", 41, 23),
                },
            )?;
            3.5
        }
        "pl-join-01" => {
            first_launch::shot(ctx, "join", None)?;
            3.0
        }
        "pl-join-02" => {
            first_launch::shot_code(ctx, "482913", 4, 116)?;
            3.0
        }
        "pl-join-03" => {
            first_launch::shot_code(ctx, "482913", 34, 86)?;
            3.0
        }
        "pl-join-04" => {
            first_launch::shot_code(ctx, "482913", 125, -5)?;
            3.0
        }
        "pl-join-05" => {
            first_launch::shot_paired(ctx, DEVICE, trackers(), None, vec![])?;
            3.0
        }
        "pl-join-06" => {
            first_launch::shot_paired(ctx, DEVICE, trackers(), Some(setup(true)), vec![])?;
            3.0
        }
        "pl-join-07" => {
            first_launch::shot_paired(
                ctx,
                DEVICE,
                trackers(),
                Some(setup(true)),
                vec![STORE.into()],
            )?;
            3.0
        }
        "pl-join-08" => {
            shell.go(Dest::Read);
            ctx.toast(&t::first_join_syncing(DEVICE));
            3.0
        }
        "pl-medium-first" => {
            first_launch::shot(ctx, "medium", None)?;
            3.0
        }
        "pl-medium-updated" => {
            shell.go(Dest::Home);
            crate::reading::medium::shot_sheet(ctx, &MediumAsk::Updated);
            2.5
        }
        "pl-medium-joined" => {
            shell.go(Dest::Home);
            crate::reading::medium::shot_sheet(
                ctx,
                &MediumAsk::Joined { devices: vec![DEVICE.into()], answer: KaeriIsFor::Both },
            );
            2.5
        }
        "pl-progress-lines" => progress_lines(shell),
        "pl-progress-06" => {
            show_sources(shell);
            row_line(shell, RowLine::Saved(glib_now() - 120));
            3.0
        }
        "pl-progress-07" => {
            show_sources(shell);
            row_line(shell, RowLine::Offline(3));
            3.0
        }
        "pl-progress-08" => {
            show_sources(shell);
            server_progress::model(ctx).stage_row(
                "server-kavita",
                Some(RowLine::Retrying(3)),
                true,
            );
            3.0
        }
        "pl-progress-09" => {
            show_sources(shell);
            row_line(shell, RowLine::Failed(3, WhyNot::Key));
            3.0
        }
        "pl-progress-10" => {
            show_sources(shell);
            row_line(shell, RowLine::Off);
            if let Some(nav) = ctx.nav() {
                nav.push(&crate::reading::sources::shot_server_page(ctx, "server-kavita"));
            }
            3.0
        }
        "pl-sources-plugin" => {
            show_sources(shell);
            if let Some(nav) = ctx.nav() {
                nav.push(&crate::reading::sources::shot_plugin_page(ctx, "tsuru"));
            }
            3.0
        }
        "pl-sources-undo" => {
            show_sources(shell);
            crate::reading::sources::shot_removed(ctx);
            // The folder leaves the list while the notice holds Undo, as the engine would say.
            let mut view = sources_view(shell);
            view.folders.clear();
            shell.ctx.hub.show_instead(vec![Event::BookSources(BookSourceEvent::View(view))]);
            3.0
        }
        // The demo, the only source.
        "pl-demo-01" => {
            first_launch::shot(ctx, "sources", Some(KaeriIsFor::Read))?;
            3.5
        }
        "pl-demo-02" => {
            first_launch::shot(ctx, "sources", Some(KaeriIsFor::Both))?;
            3.5
        }
        "pl-demo-03" => {
            shell.go(Dest::Read);
            4.0
        }
        "pl-demo-04" => {
            shell.go(Dest::Read);
            let comic = crate::reading::demo::comic()?;
            let _ = WidgetExt::activate_action(
                &ctx.window,
                "win.open-series",
                Some(&comic.work.0.to_variant()),
            );
            4.0
        }
        "pl-demo-05" => {
            crate::reading::demo::read(&ctx.window);
            // The credit capsule sits in the header, so the chrome stays up as the board draws it.
            let reader = Rc::clone(&shell.reader);
            gtk::glib::timeout_add_local_once(
                std::time::Duration::from_millis(2500),
                move || {
                    reader.shot_chrome(true);
                },
            );
            6.0
        }
        "pl-demo-06" => {
            crate::reading::demo::read(&ctx.window);
            let reader = Rc::clone(&shell.reader);
            gtk::glib::timeout_add_local_once(
                std::time::Duration::from_millis(4000),
                move || {
                    reader.shot_end();
                },
            );
            6.5
        }
        "pl-demo-07" => {
            shell.go(Dest::ReadSources);
            if let Some(nav) = ctx.nav() {
                nav.push(&crate::reading::demo::about_page(ctx));
            }
            3.0
        }
        "pl-demo-08" => {
            shell.go(Dest::ReadSearch);
            // The demo comic's own title, so the demo's row is what comes back.
            let comic = crate::reading::demo::comic()?;
            let word: String = comic.series.chars().take(6).collect();
            type_in_search(ctx.window.upcast_ref::<gtk::Widget>(), &word);
            3.5
        }
        _ => return None,
    })
}

/// Type `text` in the first search field on screen.
fn type_in_search(widget: &gtk::Widget, text: &str) -> bool {
    if let Some(entry) = widget.downcast_ref::<gtk::SearchEntry>()
        && widget.is_mapped()
    {
        entry.set_text(text);
        return true;
    }
    let mut child = widget.first_child();
    while let Some(next) = child {
        if type_in_search(&next, text) {
            return true;
        }
        child = next.next_sibling();
    }
    false
}

fn glib_now() -> i64 {
    gtk::glib::real_time() / 1_000_000
}

/// Leave what `shot` opened.
pub fn unstage(shell: &Rc<Shell>, shot: &str) {
    if !shot.starts_with("pl-") {
        return;
    }
    for _ in 0..3 {
        if let Some(dialog) = shell.ctx.window.visible_dialog() {
            dialog.force_close();
        }
    }
    shell.ctx.notices.dismiss_each();
    if shot.starts_with("pl-demo-0") {
        shell.reader.close();
    }
    if let Some(nav) = shell.ctx.nav()
        && let Some(root) = nav.navigation_stack().item(0).and_downcast::<adw::NavigationPage>()
    {
        nav.pop_to_page(&root);
    }
    let model = server_progress::model(&shell.ctx);
    model.stage_row("server-kavita", None, false);
    shell.go(Dest::Read);
}
