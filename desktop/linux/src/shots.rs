//! Screenshots of the window, taken by the app itself: each shot puts the window in a state,
//! waits for the engine and the art to settle, and draws the window into a PNG. Run under the
//! fixture scene, so every run draws the same titles.
//!
//!   kaeri-desktop --fixtures --shots <dir> --shot home,title,... [--size 1280x800]

use std::{path::PathBuf, rc::Rc, time::Duration};

use adw::prelude::*;
use gtk::glib;
use kaeri_model::{command::Command, event::Event, input::PadButton, sync::DeviceCodePrompt};

use crate::{hub::SignIn, pages::Dest, window::Shell};

/// The title the title shots open: the fixture scene's finished title, part-way through
/// episode 7.
const TITLE: u32 = 900_001;
/// The title the player shot plays: episode 4 is the next, from its start.
const PLAYING: (u32, &str) = (900_002, "4");

/// Every shot by name, in the order a run takes them.
pub const ALL: [&str; 111] = [
    "home",
    "welcome",
    "offline",
    "calendar",
    "seasonal",
    "search",
    "search-results",
    "browse-trending",
    "browse-period-menu",
    "title-another",
    "search-filters",
    "search-filters-none",
    "search-failed",
    "search-paging",
    "title",
    "title-more-about",
    "trailer-preview",
    "trailer-window",
    "discussion",
    "discussion-none",
    "discussion-waiting",
    "discussion-signed-out",
    "title-discussion",
    "synopsis",
    "synopsis-revealed",
    "synopsis-shown",
    "synopsis-none",
    "synopsis-long",
    "synopsis-ranges",
    "go-dialog",
    "go-dialog-none",
    "go-dialog-not-out",
    "go-typed",
    "go-landed",
    "go-error",
    "title-watch-again",
    "show-settings",
    "library",
    "library-focus",
    "downloads",
    "providers",
    "provider",
    "stores",
    "store",
    "store-menu",
    "store-invite",
    "store-members",
    "store-joining",
    "store-publish",
    "store-invite-name",
    "store-invite-club",
    "store-invite-any",
    "store-invite-older",
    "store-members-uses",
    "store-device",
    "store-members-loading",
    "store-members-empty",
    "store-members-failed",
    "store-joining-closed",
    "store-publish-busy",
    "store-publish-failed",
    "store-publish-empty",
    "join",
    "join-link",
    "join-group",
    "join-ended",
    "settings",
    "settings-general",
    "settings-language",
    "settings-integrations",
    "settings-upscaling",
    "settings-accounts",
    "accounts-kitsu",
    "accounts-code",
    "torrents-setup",
    "torrents-on",
    "torrents-edit-proxy",
    "torrents-checking",
    "torrents-check-failed",
    "torrents-saved",
    "torrent-open-link",
    "torrent-open-confirm",
    "torrent-open-on",
    "torrent-open-change",
    "torrent-open-untracked",
    "first-play-welcome",
    "first-play-sources",
    "first-play-demo",
    "first-play-subtitles",
    "first-play-crashes",
    "first-play-report",
    "settings-updates",
    "update-ready",
    "update-saved",
    "about",
    "player",
    "player-marks",
    "player-tracks",
    "player-episodes",
    "player-episodes-focus",
    "player-episodes-ranges",
    "player-episodes-ranges-menu",
    "first-play-style",
    "player-undo",
    "mini",
    "resumed",
    "close",
    "home-loading",
    "home-arrows",
    "home-notice",
    "home-hidden",
];

/// The watch party's shots, apart from the rest: each holds a party on the scene's own server,
/// and they take long enough that a run of every shot would not fit its time. `--shot party`
/// takes them all.
pub const PARTY: [&str; 32] = [
    "party-form",
    "party-joining",
    "party-refused",
    "party-invite",
    "party-panel",
    "party-ready",
    "party-chat",
    "party-chat-column",
    "party-chat-fold",
    "party-chat-moment",
    "party-chat-field",
    "party-chat-count",
    "party-chat-popover",
    "party-chat-failed",
    "party-chat-off",
    "party-chat-hidden",
    "party-chat-muted",
    "party-chat-lobby",
    "party-left",
    "party-lost",
    "party-close",
    "party-nosource",
    "party-choose",
    "party-unaired",
    "party-nosources",
    "party-title-menu",
    "party-together",
    "party-lobby",
    "party-lobby-invited",
    "party-lobby-waiting",
    "party-player",
    "party-upnext",
];

/// Play on TV's shots, on the scene's two TVs on loopback: `--shot tv` takes them, in this
/// order, as the last ones switch the TVs off.
pub const TV: [&str; 17] = [
    "tv-empty",
    "tv-address",
    "tv-unreachable",
    "tv-slow",
    "tv-picker",
    "tv-code",
    "tv-wrong",
    "tv-expired",
    "tv-remote",
    "tv-bar",
    "tv-settings",
    "tv-player",
    "tv-unpaired",
    "tv-bar-unpaired",
    "tv-lost",
    "tv-bar-lost",
    "tv-connecting",
];

/// Casting's shots, on TVs with Chromecast built in that run on loopback: `--shot cast` takes them.
/// The scene's TV running Kaeri is moved off loopback first, so a Cast TV is not taken for it.
pub const CAST: [&str; 15] = [
    "cast-picker",
    "cast-none-found",
    "cast-starting",
    "cast-counting",
    "cast-playing-fixed",
    "cast-playing-level",
    "cast-refused",
    "cast-never-reads",
    "cast-lost",
    "cast-taken-over",
    "cast-bar",
    "cast-stopped",
    "cast-up-next",
    "settings-integrations",
    "torrents-on",
];

/// The found skip times shots, one for each state of the engine's fixtures: `--shot found`
/// takes them.
pub const FOUND: [&str; 13] = [
    "found-skip",
    "found-controls",
    "found-prompt",
    "found-saved",
    "found-never",
    "found-review",
    "found-nudged",
    "found-undo",
    "found-panel",
    "found-panel-paused",
    "found-set-on",
    "found-set-low-power",
    "found-set-storage",
];

/// The skip times shots, apart from the rest: `--shot skips` takes them.
pub const SKIPS: [&str; 13] = [
    "skip-start",
    "skip-end",
    "skip-review",
    "skip-too-short",
    "skip-too-long",
    "skip-replaces",
    "skip-shares",
    "skip-no-entry",
    "skip-editing",
    "skip-was-opening",
    "skip-wrong",
    "skip-share",
    "skip-times",
];

/// The source picker's shots, apart from the rest: `--shot sources` takes them.
pub const SOURCES: [&str; 7] = [
    "source-picker",
    "source-picker-pinned",
    "source-picker-releases",
    "source-picker-loading",
    "source-pin-unavailable",
    "source-unsourced",
    "source-unsourced-none",
];

/// The match picker's shots, on the unsure scene (`--unsure`), where the finished title's source
/// cannot choose its match: `--shot match` takes them.
pub const MATCH: [&str; 11] = [
    "match-page",
    "match",
    "match-picking",
    "match-source-picker",
    "match-fix-menu",
    "match-menu",
    "match-failed",
    "match-empty",
    "match-results",
    "match-picked",
    "match-cleared",
];

/// The lawful sources' shots: Sources with folders, servers and services, the add-a-server flow,
/// a folder and its match dialog, the title page's three cases. `--shot lawful` takes them.
pub const LAWFUL: [&str; 22] = [
    "lawful-sources-bare",
    "lawful-sources",
    "lawful-sources-matching",
    "lawful-server-find",
    "lawful-server-sign-in",
    "lawful-server-unreachable",
    "lawful-server-credentials",
    "lawful-server-certificate",
    "lawful-server-libraries",
    "lawful-server",
    "lawful-folder",
    "lawful-folder-drive",
    "lawful-folder-fix",
    "lawful-folder-set-episode",
    "lawful-licensed",
    "lawful-title-also-on",
    "lawful-title-on-service",
    "lawful-title-nowhere",
    "lawful-return",
    "lawful-welcome",
    "lawful-demo-offer",
    "lawful-demo-row",
];

/// The game controller's shots: the screens as a controller leaves them, with the focus ring
/// showing. `--shot pad` takes them.
pub const PAD: [&str; 7] = [
    "pad-continue",
    "pad-title",
    "pad-library",
    "pad-sidebar",
    "pad-help",
    "pad-help-player",
    "pad-settings",
];

/// Stats for nerds' shots, apart from the rest so a run of every shot keeps to its time:
/// `--shot stats` takes them.
pub const STATS: [&str; 5] = [
    "player-stats",
    "player-stats-waiting",
    "player-stats-buffering",
    "player-stats-hdr",
    "player-stats-copied",
];

/// Take `shots` into `dir`, each named `<prefix><shot>.png`, then quit.
pub fn run(shell: Rc<Shell>, dir: PathBuf, prefix: String, shots: Vec<String>) {
    glib::spawn_future_local(async move {
        // The first page, its lists and their covers.
        wait(4.0).await;
        if let Some(child) = shell.ctx.window.content() {
            wide_widgets(&child, 0, shell.ctx.window.width());
        }
        for shot in &shots {
            let settle = stage(&shell, shot);
            wait(settle).await;
            let path = dir.join(format!("{prefix}{shot}.png"));
            let mut taken = Err(String::new());
            // A window that has just changed its size or decorations can have no frame drawn
            // yet: asked to draw, and tried again.
            for _ in 0..4 {
                // Taken just after a frame is painted, when the window is laid out: between
                // frames a relayout can be pending, and a widget waiting for one draws nothing.
                taken = painted(&shell, &path, shot).await;
                if taken.is_ok() {
                    break;
                }
                wait(1.5).await;
            }
            match taken {
                Ok(()) => tracing::info!(path = %path.display(), "a shot"),
                Err(e) => tracing::error!(shot, error = %e, "a shot did not save"),
            }
            unstage(&shell, shot);
            wait(0.5).await;
        }
        shell.stop_and_close();
        if let Some(app) = shell.ctx.window.application() {
            app.quit();
        }
    });
}

/// Put the window in the state `shot` shows, and say how long to let it settle.
fn stage(shell: &Rc<Shell>, shot: &str) -> f64 {
    #[cfg(feature = "fixtures")]
    if let Some(settle) = crate::shots_reading::stage(shell, shot) {
        return settle;
    }
    #[cfg(feature = "fixtures")]
    if let Some(settle) = crate::shots_read_discovery::stage(shell, shot) {
        return settle;
    }
    #[cfg(feature = "fixtures")]
    if let Some(settle) = crate::shots_prelaunch::stage(shell, shot) {
        return settle;
    }
    let window = &shell.ctx.window;
    match shot {
        "home" => {
            shell.go(Dest::Home);
            1.0
        }
        #[cfg(feature = "fixtures")]
        s if crate::sync::shots::NAMES.contains(&s) => crate::sync::shots::stage(shell, s),
        "welcome" => {
            crate::pages::first_launch::open(&shell.ctx);
            1.5
        }
        // Home's own states. Each Home scene is its own run (`--scene`), so these are taken
        // in the order a run staged them: the notice, then the Hidden titles it adds to.
        "home-loading" => {
            shell.go(Dest::Home);
            0.1
        }
        "home-arrows" => {
            shell.go(Dest::Home);
            show_arrows(shell);
            1.2
        }
        // The slow scene past four seconds, with the activity line, and once its answers land.
        "home-activity" => {
            shell.go(Dest::Home);
            1.5
        }
        "home-settled" => {
            shell.go(Dest::Home);
            // Each part landing, for the recording.
            let window: gtk::Widget = shell.ctx.window.clone().upcast();
            // Home is still waiting when this stage starts, so the first view with fewer parts
            // pending than any before is a landing.
            let waiting = std::cell::Cell::new(usize::MAX);
            let _ = shell.ctx.hub.listen(move |event| {
                if let Event::Home(view) = event {
                    let now = view.pending.len();
                    if now < waiting.replace(now) {
                        mark_step("arrival", &window);
                    }
                }
            });
            14.0
        }
        // Motion, for a recording of the screen (scripts/motion.sh): each logs when its steps
        // fire, so the frames can be found in the video.
        "motion-arrows" => {
            shell.go(Dest::Home);
            let shelf = paging_shelf(shell);
            if let Some(shelf) = &shelf {
                bring_into_view(shelf);
            }
            let steps: [(u64, &str, Option<glib::Variant>); 5] = [
                (500, "shelf.hover", Some(true.to_variant())),
                (1500, "shelf.page", Some(1i32.to_variant())),
                (2500, "shelf.page", Some(1i32.to_variant())),
                (3500, "shelf.page", Some((-1i32).to_variant())),
                (4500, "shelf.hover", Some(false.to_variant())),
            ];
            for (at, action, arg) in steps {
                let shelf = shelf.clone();
                glib::timeout_add_local_once(Duration::from_millis(at), move || {
                    if let Some(shelf) = &shelf {
                        mark_step(action, shelf);
                        let _ = WidgetExt::activate_action(shelf, action, arg.as_ref());
                    }
                });
            }
            5.5
        }
        motion if motion.starts_with("motion-found-") => found_motion(shell, motion),
        "motion-dismiss" => {
            shell.go(Dest::Home);
            // The first card of the last suggestion shelf, so the cards after it close the gap.
            let Some((_, last)) = last_suggestion(shell) else { return 1.0 };
            let shelf_of = |card: &gtk::Widget| {
                let mut shelf = card.parent();
                while let Some(w) = shelf.as_ref().filter(|w| !w.has_css_class("home-shelf")) {
                    shelf = w.parent();
                }
                shelf
            };
            let Some(frame) = shelf_of(&last) else { return 1.0 };
            let Some(id) = descendants(&frame)
                .into_iter()
                .filter(|w| w.has_css_class("home-suggestion"))
                .find_map(|w| {
                    w.downcast::<gtk::Button>().ok()?.action_target_value()?.get::<u32>()
                })
            else {
                return 1.0;
            };
            bring_into_view(&frame);
            let window = shell.ctx.window.clone();
            glib::timeout_add_local_once(Duration::from_millis(500), {
                let (window, frame) = (window.clone(), frame.clone());
                move || {
                    mark_step("dismiss", &frame);
                    let _ = WidgetExt::activate_action(
                        &window,
                        "win.home-dismiss",
                        Some(&id.to_variant()),
                    );
                }
            });
            let notices = shell.ctx.notices.clone();
            glib::timeout_add_local_once(Duration::from_millis(2500), move || {
                mark_step("undo", &frame);
                notices.dismiss_each();
                let _ = WidgetExt::activate_action(
                    &window,
                    "win.restore-up-next",
                    Some(&id.to_string().to_variant()),
                );
            });
            4.5
        }
        "home-notice" => {
            shell.go(Dest::Home);
            dismiss_last_card(shell);
            1.5
        }
        "home-hidden" => {
            shell.ctx.notices.dismiss_each();
            shell.settings_pane("hidden");
            2.0
        }
        "offline" => {
            shell.ctx.hub.send(Command::NetworkPath {
                cellular: false,
                constrained: false,
                offline: true,
            });
            shell.go(Dest::Seasonal);
            2.0
        }
        "resumed" => {
            play(shell);
            #[cfg(feature = "fixtures")]
            {
                let player = Rc::clone(&shell.player);
                glib::timeout_add_local_once(Duration::from_secs(6), move || {
                    player.shot_resumed(724.0);
                });
            }
            7.5
        }
        "calendar" => {
            shell.go(Dest::Calendar);
            3.0
        }
        "seasonal" => {
            shell.go(Dest::Seasonal);
            2.5
        }
        "search" => {
            shell.go(Dest::Search);
            2.0
        }
        #[cfg(feature = "fixtures")]
        "browse-trending" | "browse-period-menu" => {
            use kaeri_model::browsing::TrendPeriod;
            shell.go(Dest::Search);
            let shell = Rc::clone(shell);
            let menu = shot == "browse-period-menu";
            glib::timeout_add_local_once(Duration::from_millis(2500), move || {
                shell.search_page().trending().pick(TrendPeriod::Week);
                if menu {
                    glib::timeout_add_local_once(Duration::from_millis(1500), move || {
                        if let Some(button) = shell.search_page().trending().menu() {
                            button.popup();
                        }
                    });
                }
            });
            5.5
        }
        #[cfg(feature = "fixtures")]
        "title-another" => {
            // Opened by Surprise me: the header says Another.
            shell.go(Dest::Search);
            shell.ctx.pick.replace(Some(crate::window::Pick {
                id: kaeri_core::ids::AnilistId::new(TITLE),
                pool: kaeri_model::browsing::SurprisePool::Filters {
                    query: kaeri_model::search::SearchQuery::default(),
                },
                more: true,
            }));
            let _ =
                WidgetExt::activate_action(window, "win.open-title", Some(&TITLE.to_variant()));
            3.5
        }
        "search-results" => {
            shell.go(Dest::Search);
            shell.search_page().search_for("hoshikuzu");
            3.0
        }
        #[cfg(feature = "fixtures")]
        "search-filters" => {
            // Browsing without a word typed: a genre left out, two formats, a status.
            use kaeri_model::{catalog::AiringStatus, search::*};
            shell.go(Dest::Search);
            let query = SearchQuery::default()
                .picking_genre("Romance", Pick::Exclude)
                .toggling_format(MediaFormat::Tv)
                .toggling_format(MediaFormat::Movie)
                .toggling_status(AiringStatus::Finished);
            shell.search_page().browse(&query);
            3.0
        }
        #[cfg(feature = "fixtures")]
        "search-filters-none" => {
            // Filters nothing matches: two genres wanted, a year.
            use kaeri_model::search::{Pick, SearchQuery};
            shell.go(Dest::Search);
            let query = SearchQuery::default()
                .picking_genre("Action", Pick::Include)
                .picking_genre("Mecha", Pick::Include)
                .in_year(Some(2024));
            shell.search_page().browse(&query);
            3.0
        }
        #[cfg(feature = "fixtures")]
        "search-failed" => {
            // AniList did not answer: the engine's words, in place of the results.
            shell.go(Dest::Search);
            shell.search_page().search_for("hoshikuzu");
            let shell = Rc::clone(shell);
            glib::timeout_add_local_once(Duration::from_millis(1500), move || {
                let Some(query) = shell.search_page().asked() else { return };
                shell.ctx.hub.take(vec![kaeri_model::event::Event::Content {
                    view: kaeri_model::catalog::ContentFor::Search { query },
                    content: kaeri_model::catalog::ContentList::Failed(
                        "AniList didn't answer in time. Check the connection and try again."
                            .into(),
                    ),
                }]);
            });
            3.0
        }
        #[cfg(feature = "fixtures")]
        "search-paging" => {
            // Browsing TV series, the next page on its way at the end of the list.
            use kaeri_model::search::{MediaFormat, SearchQuery};
            crate::showcase::set_mood(crate::showcase::Mood::Silent);
            shell.go(Dest::Search);
            shell
                .search_page()
                .browse(&SearchQuery::default().toggling_format(MediaFormat::Tv));
            let shell = Rc::clone(shell);
            glib::timeout_add_local_once(Duration::from_millis(1800), move || {
                let Some(query) = shell.search_page().asked() else { return };
                shell.ctx.hub.take(vec![kaeri_model::event::Event::SearchPaging {
                    query,
                    has_next: true,
                }]);
                shell.search_page().more();
            });
            3.0
        }
        "title" => {
            shell.go(Dest::Home);
            let _ =
                WidgetExt::activate_action(window, "win.open-title", Some(&TITLE.to_variant()));
            3.5
        }
        "trailer-preview" => {
            // The pointer rests on a cover in Seasonal: its preview opens, and its trailer plays.
            shell.go(Dest::Seasonal);
            let shell = Rc::clone(shell);
            glib::timeout_add_local_once(Duration::from_millis(2500), move || {
                let cover = mapped_grid(shell.ctx.window.upcast_ref())
                    .and_then(|grid| first_cover(grid.upcast_ref()))
                    .and_then(|fit| fit.parent());
                let id = shell
                    .ctx
                    .hub
                    .known
                    .borrow()
                    .entries
                    .values()
                    .find(|entry| entry.title == "The Library Above the Clouds")
                    .map(|entry| entry.id.get());
                if let (Some(cover), Some(id)) = (cover, id) {
                    crate::trailer::show_preview(&shell.ctx.hub, &cover, id);
                }
            });
            6.0
        }
        "trailer-window" => {
            shell.go(Dest::Home);
            if let Some(entry) = shell.ctx.hub.entry(kaeri_core::ids::AnilistId::new(TITLE)) {
                crate::trailer::play_window(&shell.ctx, &entry);
            }
            4.0
        }
        #[cfg(feature = "fixtures")]
        "discussion" | "discussion-none" | "discussion-waiting" | "discussion-signed-out" => {
            use crate::showcase::Thread;
            shell.go(Dest::Home);
            crate::showcase::set_thread(match shot {
                "discussion" => Thread::Comments,
                "discussion-none" => Thread::Nobody,
                "discussion-waiting" => Thread::Waiting,
                _ => Thread::Signed,
            });
            crate::pages::discussion::open(
                &shell.ctx,
                kaeri_core::ids::AnilistId::new(TITLE),
                3,
            );
            3.0
        }
        "title-watch-again" => {
            // Every episode watched: the main button offers episode 1 again.
            shell.go(Dest::Home);
            let _ =
                WidgetExt::activate_action(window, "win.open-title", Some(&TITLE.to_variant()));
            let hub = Rc::clone(&shell.ctx.hub);
            glib::timeout_add_local_once(Duration::from_millis(2500), move || {
                hub.take(vec![Event::Progress {
                    id: kaeri_core::ids::AnilistId::new(TITLE),
                    progress: Some(kaeri_model::progress::TitleProgress { done: 12, next: 1 }),
                    resume: None,
                    episodes: Vec::new(),
                }]);
            });
            4.0
        }
        #[cfg(feature = "fixtures")]
        "title-more-about" => {
            // The dialog behind the page's More about this show button.
            shell.go(Dest::Home);
            let _ =
                WidgetExt::activate_action(window, "win.open-title", Some(&TITLE.to_variant()));
            let ctx = Rc::clone(&shell.ctx);
            glib::timeout_add_local_once(Duration::from_millis(2500), move || {
                if let Some(button) =
                    button_labelled(ctx.window.upcast_ref(), &kaeri_i18n::t::title_more_about())
                {
                    button.emit_clicked();
                }
            });
            4.0
        }
        #[cfg(feature = "fixtures")]
        "synopsis" | "synopsis-revealed" | "synopsis-shown" | "synopsis-none"
        | "synopsis-long" | "synopsis-ranges" | "go-dialog" | "go-dialog-none"
        | "go-dialog-not-out" | "go-typed" | "go-landed" | "go-error" | "motion-go"
        | "motion-typed" => stage_synopsis(shell, shot),
        // Motion B: the period menu opened from the heading, This week picked, the covers
        // crossfading to the week's.
        #[cfg(feature = "fixtures")]
        "motion-period" => {
            use kaeri_model::browsing::TrendPeriod;
            shell.go(Dest::Search);
            let shell = Rc::clone(shell);
            glib::timeout_add_local_once(Duration::from_millis(2500), move || {
                let Some(button) = shell.search_page().trending().menu() else { return };
                mark_step(
                    "period.menu",
                    &shell.search_page().trending().widget.clone().upcast(),
                );
                button.popup();
                glib::timeout_add_local_once(Duration::from_millis(1400), move || {
                    button.popdown();
                    mark_step(
                        "period.week",
                        &shell.search_page().trending().widget.clone().upcast(),
                    );
                    shell.search_page().trending().pick(TrendPeriod::Week);
                });
            });
            7.0
        }
        // Motion C: Another on a title Surprise me opened, crossfading to the next pick.
        #[cfg(feature = "fixtures")]
        "motion-another" => {
            shell.go(Dest::Search);
            shell.ctx.pick.replace(Some(crate::window::Pick {
                id: kaeri_core::ids::AnilistId::new(TITLE),
                pool: kaeri_model::browsing::SurprisePool::Filters {
                    query: kaeri_model::search::SearchQuery::default(),
                },
                more: true,
            }));
            let _ =
                WidgetExt::activate_action(window, "win.open-title", Some(&TITLE.to_variant()));
            let ctx = Rc::clone(&shell.ctx);
            glib::timeout_add_local_once(Duration::from_millis(3000), move || {
                let label = kaeri_i18n::t::surprise_another();
                if let Some(button) = button_labelled(ctx.window.upcast_ref(), &label) {
                    mark_step("another", &ctx.window.clone().upcast());
                    button.emit_clicked();
                }
            });
            7.0
        }
        "title-discussion" => {
            // The episode that has a thread says so under its row.
            shell.go(Dest::Home);
            let _ =
                WidgetExt::activate_action(window, "win.open-title", Some(&TITLE.to_variant()));
            let hub = Rc::clone(&shell.ctx.hub);
            glib::timeout_add_local_once(Duration::from_millis(2500), move || {
                hub.take(vec![Event::DiscussionFound {
                    media: kaeri_core::ids::AnilistId::new(TITLE),
                    episode: 1,
                    comments: 214,
                }]);
            });
            4.0
        }
        "show-settings" => {
            shell.go(Dest::Home);
            let _ =
                WidgetExt::activate_action(window, "win.open-title", Some(&TITLE.to_variant()));
            let ctx = Rc::clone(&shell.ctx);
            glib::timeout_add_local_once(Duration::from_millis(2500), move || {
                if let Some(page) = ctx.nav().and_then(|nav| nav.visible_page()) {
                    let _ = WidgetExt::activate_action(&page, "title.show-settings", None);
                }
            });
            5.0
        }
        "library" => {
            shell.go(Dest::Library);
            3.0
        }
        "library-focus" => {
            // Moved to by keyboard: the first cover holds the focus ring.
            shell.go(Dest::Library);
            let window = shell.ctx.window.clone();
            glib::timeout_add_local_once(Duration::from_millis(2000), move || {
                if let Some(grid) = mapped_grid(window.upcast_ref()) {
                    window.set_focus_visible(true);
                    grid.grab_focus();
                }
            });
            3.0
        }
        "downloads" => {
            shell.go(Dest::Downloads);
            2.0
        }
        "providers" | "first-play-sources" => {
            shell.go(Dest::Providers);
            2.0
        }
        #[cfg(feature = "fixtures")]
        lawful if lawful.starts_with("lawful-") => lawful_stage(shell, lawful),
        "first-play-welcome" => {
            crate::pages::first_launch::open_sources(&shell.ctx);
            1.5
        }
        "first-play-demo" => {
            shell.go(Dest::Providers);
            crate::pages::providers::open_demo(&shell.ctx);
            1.5
        }
        "first-play-subtitles" => {
            shell.settings_pane("playback");
            // Down to the Subtitles group, so its style rows are in frame.
            #[cfg(feature = "fixtures")]
            {
                let window = shell.ctx.window.clone();
                glib::timeout_add_local_once(Duration::from_millis(1200), move || {
                    scroll_to_group(
                        window.upcast_ref(),
                        &kaeri_i18n::t::settings_group_subtitles(),
                    );
                });
            }
            3.0
        }
        "first-play-crashes" => {
            shell.settings_pane("about");
            1.5
        }
        // The demo scenes run with --demo, not in the default list: the demo is the only source.
        "demo-home" => {
            shell.go(Dest::Home);
            3.0
        }
        "demo-search" => {
            shell.go(Dest::Search);
            shell.search_page().search_for("koushisu");
            3.0
        }
        "demo-title" => {
            shell.go(Dest::Home);
            let id = kaeri_core::demo::DEMO_ANILIST_ID.get();
            let _ =
                WidgetExt::activate_action(window, "win.open-title", Some(&id.to_variant()));
            3.5
        }
        "demo-starting" => {
            // The engine says Starting once its source has answered, and the demo's stands at
            // an address that never does, so the screen is raised as the engine would.
            shell.go(Dest::Home);
            shell.ctx.hub.take(vec![Event::PlayerUi {
                playback: kaeri_model::playback::PlaybackId::new(1),
                ui: kaeri_model::hosted::PlayerUi::Starting {
                    id: kaeri_core::demo::DEMO_ANILIST_ID,
                    title: kaeri_core::demo::DEMO_TITLE.to_owned(),
                    episode: "1".to_owned(),
                    episode_title: None,
                    cover_url: None,
                    banner_url: None,
                },
            }]);
            1.5
        }
        // The starting screen, driven by the engine's made-up starts: `start-<state>-<seconds>`
        // plays the state's events on their own clock and is taken that long after the start.
        // With `-details` after the state, Details opens 5 s in; with `-beginning`, Play from the
        // beginning is pressed 4 s in.
        start if start.starts_with("start-") => {
            let details = start.contains("-details-");
            let beginning = start.contains("-beginning-");
            let name = start.replace("-details-", "-").replace("-beginning-", "-");
            let Some((state, seconds)) = start_state(&name) else {
                return 0.1;
            };
            shell.go(Dest::Home);
            if details {
                let player = Rc::clone(&shell.player);
                glib::timeout_add_local_once(Duration::from_secs(5), move || {
                    player.shot_start_details();
                });
            }
            if beginning {
                let player = Rc::clone(&shell.player);
                glib::timeout_add_local_once(Duration::from_secs(4), move || {
                    player.shot_start_from_beginning();
                });
            }
            let hub = Rc::clone(&shell.ctx.hub);
            for fixture in kaeri_engine::playback::starting::fixture(state) {
                let hub = Rc::clone(&hub);
                glib::timeout_add_local_once(Duration::from_millis(fixture.at_ms), move || {
                    hub.take(vec![fixture.event]);
                });
            }
            seconds
        }
        "demo-removed" => {
            shell.go(Dest::Home);
            shell.ctx.hub.send(Command::RemoveDemo);
            2.0
        }
        "first-play-style" => {
            play(shell);
            #[cfg(feature = "fixtures")]
            {
                let player = Rc::clone(&shell.player);
                glib::timeout_add_local_once(Duration::from_secs(4), move || {
                    player.shot_style(Some("/out/media/lantern-road.srt"));
                });
                let player = Rc::clone(&shell.player);
                glib::timeout_add_local_once(Duration::from_millis(8000), move || {
                    player.shot_style(None);
                });
            }
            9.5
        }
        "first-play-report" => {
            crate::pages::report::open(&shell.ctx);
            1.5
        }
        "provider" => {
            shell.go(Dest::Providers);
            crate::pages::plugin::push_provider(&shell.ctx, "mukashi-archive");
            1.5
        }
        "stores" => {
            shell.go(Dest::Stores);
            2.0
        }
        "store" => {
            shell.go(Dest::Stores);
            crate::pages::stores::push_store(&shell.ctx, "hotaru");
            1.5
        }
        #[cfg(feature = "fixtures")]
        owner if owner.starts_with("store-") => store_owner(shell, owner),
        "join" => {
            join(shell);
            2.0
        }
        #[cfg(feature = "fixtures")]
        "join-group" | "join-ended" => {
            let (link, answer) = if shot == "join-group" {
                (crate::showcase::INVITE_GROUP, crate::showcase::invite_group())
            } else {
                (crate::showcase::INVITE, crate::showcase::invite_ended())
            };
            crate::pages::join::open(&shell.ctx, Some(link));
            let hub = Rc::clone(&shell.ctx.hub);
            glib::timeout_add_local_once(Duration::from_millis(900), move || {
                hub.show_instead(vec![answer]);
            });
            2.0
        }
        #[cfg(feature = "fixtures")]
        "join-link" => {
            // The invite opened from a link on a web page: the dialog comes up already checking.
            shell.open_link("kaeri://plugins.hotaru.example/join/fixture#key");
            let hub = Rc::clone(&shell.ctx.hub);
            glib::timeout_add_local_once(Duration::from_millis(900), move || {
                hub.show_instead(vec![crate::showcase::invite()]);
            });
            2.0
        }
        "settings" => {
            shell.settings_pane("playback");
            1.5
        }
        "pad-continue" => {
            shell.go(Dest::Home);
            pad_press(
                shell,
                &[PadButton::DpadDown, PadButton::DpadDown, PadButton::DpadDown],
                1500,
            );
            3.0
        }
        "pad-title" => {
            shell.go(Dest::Home);
            if let Some(pad) = shell.pad() {
                pad.shot_enter();
            }
            let _ =
                WidgetExt::activate_action(window, "win.open-title", Some(&TITLE.to_variant()));
            3.5
        }
        "pad-library" => {
            shell.go(Dest::Library);
            pad_press(
                shell,
                &[PadButton::DpadDown, PadButton::DpadRight, PadButton::DpadRight],
                2200,
            );
            3.5
        }
        "pad-sidebar" => {
            shell.go(Dest::Home);
            pad_press(shell, &[PadButton::DpadDown, PadButton::DpadLeft], 1500);
            3.0
        }
        "pad-help" => {
            shell.go(Dest::Home);
            if let Some(pad) = shell.pad() {
                pad.shot_enter();
                pad.shot_help();
            }
            1.5
        }
        "pad-help-player" => {
            play(shell);
            let shell = Rc::clone(shell);
            glib::timeout_add_local_once(Duration::from_secs(5), move || {
                if let Some(pad) = shell.pad() {
                    pad.shot_enter();
                    pad.shot_help();
                }
            });
            7.0
        }
        "pad-settings" => {
            shell.ctx.prefs.set_controller_seen();
            shell.settings_pane("general");
            1.5
        }
        "settings-general" => {
            shell.settings_pane("general");
            1.5
        }
        // A language chosen, so the closed row shows its flag beside the name.
        "settings-language" => {
            shell.settings_pane("general");
            if let Some(row) = language_row(shell.ctx.window.upcast_ref()) {
                row.set_selected(2);
            }
            1.5
        }
        "settings-integrations" => {
            shell.settings_pane("integrations");
            1.5
        }
        "settings-upscaling" => {
            shell.settings_pane("playback");
            shell.settings_upscaling();
            upscaling(shell, "natural");
            2.0
        }
        "settings-accounts" => {
            shell.settings_pane("accounts");
            1.5
        }
        "torrent-open-link" => {
            crate::pages::opened::open(&shell.ctx);
            1.5
        }
        #[cfg(feature = "fixtures")]
        "torrent-open-confirm" => {
            // The scene's own link: a made-up info hash, its name guessed as episode 3.
            let link = "magnet:?xt=urn:btih:5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e\
                &dn=Hoshikuzu%20no%20Tabibito%20-%2003%20%5B1080p%5D.mkv";
            crate::pages::opened::open_input(&shell.ctx, link);
            3.0
        }
        #[cfg(feature = "fixtures")]
        "torrent-open-on" | "torrent-open-change" => {
            // Torrent playback on (through the proxy, as the engine needs), so Play, Play without
            // tracking and Download are live; the link opens once that has saved.
            by_proxy(shell);
            once_proxy_saved(shell, |ctx| {
                let link = "magnet:?xt=urn:btih:5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e\
                    &dn=Hoshikuzu%20no%20Tabibito%20-%2003%20%5B1080p%5D.mkv";
                crate::pages::opened::open_input(ctx, link);
            });
            if shot == "torrent-open-change" {
                let window = shell.ctx.window.clone();
                glib::timeout_add_local_once(Duration::from_millis(4500), move || {
                    if let Some(dialog) = window.visible_dialog()
                        && let Some(button) = button_labelled(
                            dialog.upcast_ref(),
                            &kaeri_i18n::t::opened_change(),
                        )
                    {
                        button.emit_clicked();
                    }
                });
            }
            6.0
        }
        #[cfg(feature = "fixtures")]
        "torrent-open-untracked" => {
            // No real torrent can start here, so the engine's first player event for a play
            // without tracking is raised by hand: the link's name, and no title behind it.
            shell.ctx.hub.take(vec![Event::PlayerUi {
                playback: kaeri_model::playback::PlaybackId::new(1),
                ui: kaeri_model::hosted::PlayerUi::Starting {
                    id: kaeri_model::opened::UNTRACKED,
                    title: "Hoshikuzu no Tabibito - 03 [1080p].mkv".to_owned(),
                    episode: "3".to_owned(),
                    episode_title: None,
                    cover_url: None,
                    banner_url: None,
                },
            }]);
            1.5
        }
        "torrents-setup" => {
            shell.settings_pane("sources");
            crate::pages::torrents::setup(&shell.ctx, Rc::new(|| {}));
            1.5
        }
        "torrents-on" => {
            by_proxy(shell);
            shell.settings_pane("sources");
            3.0
        }
        #[cfg(feature = "fixtures")]
        "torrents-edit-proxy" => {
            by_proxy(shell);
            shell.settings_pane("sources");
            once_proxy_saved(shell, |ctx| {
                crate::pages::torrents::edit_proxy(ctx);
            });
            3.0
        }
        #[cfg(feature = "fixtures")]
        "torrents-checking" | "torrents-check-failed" | "torrents-saved" => {
            use kaeri_model::sources::ProxyStatus;
            by_proxy(shell);
            shell.settings_pane("sources");
            // Answered here, never through the proxy typed in.
            shell.ctx.hub.check_answer.replace(Some(match shot {
                "torrents-checking" => None,
                "torrents-check-failed" => Some(ProxyStatus::Failed),
                _ => Some(ProxyStatus::Passed),
            }));
            once_proxy_saved(shell, |ctx| {
                let dialog = crate::pages::torrents::edit_proxy(ctx);
                let child = dialog.upcast_ref::<gtk::Widget>();
                for (title, text) in
                    [("Host", "proxy.example"), ("Port", "1080"), ("User name", "viewer")]
                {
                    if let Some(row) = entry_titled(child, title) {
                        row.set_text(text);
                    }
                }
                if let Some(send) = default_button(&dialog) {
                    send.emit_clicked();
                }
            });
            3.0
        }
        "settings-updates" => {
            shell.settings_pane("integrations");
            offer_update(shell);
            // The Updates group is the last on the page: scrolled down to it.
            let window = shell.ctx.window.clone();
            glib::timeout_add_local_once(Duration::from_millis(1000), move || {
                scroll_to_end(window.upcast_ref());
            });
            2.0
        }
        // The download rehearsed and checked: the row offers Restart to Update, and so does the
        // notice.
        "update-ready" => {
            shell.settings_pane("integrations");
            4.5
        }
        // The same from an AppImage in a folder the app can't write to: saved to Downloads, and
        // Show in Files in the row and the notice.
        #[cfg(feature = "fixtures")]
        "update-saved" => {
            shell.settings_pane("integrations");
            shell.ctx.updater.rehearse_unwritable();
            offer_update(shell);
            shell.ctx.updater.download();
            4.5
        }
        "accounts-kitsu" => {
            shell.settings_pane("accounts");
            crate::pages::accounts::kitsu(&shell.ctx, Rc::new(|| {}));
            1.5
        }
        "accounts-code" => {
            shell.ctx.hub.known.borrow_mut().signing.insert("simkl".into(), SignIn::Job(None));
            shell.settings_pane("accounts");
            let prompt = DeviceCodePrompt {
                tracker: "simkl".into(),
                code: "7KQ2M4".into(),
                url: "https://example.test/pin".into(),
                expires_at: Some(glib::real_time() / 1_000_000 + 14 * 60 + 59),
            };
            crate::pages::accounts::device_code(&shell.ctx, Some(&prompt));
            1.5
        }
        "about" => {
            let _ = WidgetExt::activate_action(window, "win.about", None);
            1.5
        }
        "player" => {
            play(shell);
            9.0
        }
        "player-marks" => {
            play(shell);
            #[cfg(feature = "fixtures")]
            {
                let player = Rc::clone(&shell.player);
                glib::timeout_add_local_once(Duration::from_secs(4), move || {
                    player.shot_marks("/out/media/lantern-road.srt");
                });
                let player = Rc::clone(&shell.player);
                // The preview opens the stream the first time it shows and has its frame a
                // moment later; shown again then, so the controls are still up for the shot.
                glib::timeout_add_local_once(Duration::from_millis(6500), move || {
                    player.shot_preview(0.4);
                });
                let player = Rc::clone(&shell.player);
                glib::timeout_add_local_once(Duration::from_millis(8500), move || {
                    player.shot_preview(0.4);
                });
            }
            10.0
        }
        "player-episodes"
        | "player-episodes-focus"
        | "player-episodes-ranges"
        | "player-episodes-ranges-menu" => {
            play(shell);
            #[cfg(feature = "fixtures")]
            stage_player_episodes(shell, shot);
            10.5
        }
        "player-tracks" => {
            play(shell);
            #[cfg(feature = "fixtures")]
            {
                let player = Rc::clone(&shell.player);
                glib::timeout_add_local_once(Duration::from_secs(4), move || {
                    player.shot_tracks(Some("/out/media/lantern-road.srt"));
                });
                let player = Rc::clone(&shell.player);
                glib::timeout_add_local_once(Duration::from_millis(8000), move || {
                    player.shot_tracks(None);
                });
            }
            9.5
        }
        stats if stats.starts_with("player-stats") => {
            play(shell);
            #[cfg(feature = "fixtures")]
            {
                use kaeri_model::hosted::StatsFixture;
                // The live panel reads mpv itself; the others hold the engine's made-up states.
                let (at, fixture, waited, copied) = match stats {
                    "player-stats-waiting" => {
                        (0.3, Some(StatsFixture::Waiting), Some(5.0), false)
                    }
                    "player-stats-buffering" => {
                        (2.0, Some(StatsFixture::Buffering), None, false)
                    }
                    "player-stats-hdr" => (2.0, Some(StatsFixture::Hdr), None, false),
                    "player-stats-copied" => (2.0, Some(StatsFixture::Playing), None, true),
                    _ => (2.0, None, None, false),
                };
                let player = Rc::clone(&shell.player);
                glib::timeout_add_local_once(Duration::from_secs_f64(at), move || {
                    player.shot_stats(fixture, waited, copied);
                });
                let player = Rc::clone(&shell.player);
                glib::timeout_add_local_once(Duration::from_secs(4), move || {
                    player.shot_tracks(Some("/out/media/lantern-road.srt"));
                });
                // The short clip is near its end from the start: no Up Next card over the shot.
                shell.player.hold_up_next.set(true);
            }
            // Opened before the file has: taken while it is still starting.
            if stats == "player-stats-waiting" { 1.0 } else { 9.5 }
        }
        "player-undo" => {
            play(shell);
            #[cfg(feature = "fixtures")]
            {
                let player = Rc::clone(&shell.player);
                glib::timeout_add_local_once(Duration::from_millis(7500), move || {
                    player.shot_auto_skipped();
                });
            }
            9.0
        }
        skip if skip.starts_with("skip-") => skip_stage(shell, skip),
        found if found.starts_with("found-") => found_stage(shell, found),
        "mini" => {
            play(shell);
            let player = Rc::clone(&shell.player);
            glib::timeout_add_local_once(Duration::from_secs(5), move || player.set_mini(true));
            9.0
        }
        "close" => {
            window.close();
            1.5
        }
        party if party.starts_with("party-") => party_stage(shell, party),
        source if source.starts_with("source-") => source_stage(shell, source),
        matched if matched == "match" || matched.starts_with("match-") => {
            match_stage(shell, matched)
        }
        tv if tv.starts_with("tv-") => tv_stage(shell, tv),
        cast if cast.starts_with("cast-") => cast_stage(shell, cast),
        other => {
            tracing::warn!(shot = other, "no such shot");
            0.0
        }
    }
}

/// Leave the state a shot put the window in.
/// The made-up start and the seconds a `start-<state>-<seconds>` shot names.
fn start_state(shot: &str) -> Option<(kaeri_model::starting::StartingFixture, f64)> {
    use kaeri_model::starting::StartingFixture as S;
    let (state, seconds) = shot.strip_prefix("start-")?.rsplit_once('-')?;
    let state = match state {
        "title-lookup" => S::TitleLookup,
        "sources-loading" => S::SourcesLoading,
        "asking-first" => S::AskingFirst,
        "asking-pinned" => S::AskingPinned,
        "source-missed" => S::SourceMissed,
        "source-failed" => S::SourceFailed,
        "sources-resting" => S::SourcesResting,
        "slow" => S::Slow,
        "finding-peers" => S::FindingPeers,
        "peers-known" => S::PeersKnown,
        "found" => S::Found,
        "fetching-subtitles" => S::FetchingSubtitles,
        "another-stream" => S::AnotherStream,
        "buffering-torrent" => S::BufferingTorrent,
        "buffering-direct" => S::BufferingDirect,
        "offline" => S::Offline,
        "offline-timed-out" => S::OfflineTimedOut,
        "failed-no-episode" => S::FailedNoEpisode,
        "failed-no-match" => S::FailedNoMatch,
        "failed-no-sources" => S::FailedNoSources,
        "failed-not-aired" => S::FailedNotAired,
        _ => return None,
    };
    Some((state, seconds.parse().ok()?))
}

fn unstage(shell: &Rc<Shell>, shot: &str) {
    #[cfg(feature = "fixtures")]
    crate::shots_reading::unstage(shell, shot);
    #[cfg(feature = "fixtures")]
    crate::shots_read_discovery::unstage(shell, shot);
    #[cfg(feature = "fixtures")]
    crate::shots_prelaunch::unstage(shell, shot);
    if shot.starts_with("start-") {
        shell.player.close();
    }
    if shot.starts_with("lawful-") {
        close_dialogs(&shell.ctx.window);
        close_dialogs(&shell.ctx.window);
        shell.ctx.notices.dismiss_each();
        let _ = WidgetExt::activate_action(&shell.ctx.window, "win.back", None);
        let _ = WidgetExt::activate_action(&shell.ctx.window, "win.back", None);
    }
    if shot.starts_with("pad-") {
        if let Some(pad) = shell.pad() {
            pad.shot_leave();
        }
        close_dialogs(&shell.ctx.window);
    }
    match shot {
        #[cfg(feature = "fixtures")]
        s if crate::sync::shots::NAMES.contains(&s) => {
            close_dialogs(&shell.ctx.window);
            close_dialogs(&shell.ctx.window);
            shell.ctx.hub.known.borrow_mut().invite = None;
        }
        "title-another" => {
            shell.ctx.pick.replace(None);
            let _ = WidgetExt::activate_action(&shell.ctx.window, "win.back", None);
        }
        "title-more-about" => {
            close_dialogs(&shell.ctx.window);
            let _ = WidgetExt::activate_action(&shell.ctx.window, "win.back", None);
        }
        "title" | "pad-title" | "title-discussion" | "synopsis" | "synopsis-revealed"
        | "synopsis-shown" | "synopsis-none" | "synopsis-long" | "synopsis-ranges"
        | "go-dialog" | "go-dialog-none" | "go-dialog-not-out" | "go-typed" | "go-landed"
        | "go-error" | "motion-go" | "motion-typed" => {
            close_dialogs(&shell.ctx.window);
            let _ = WidgetExt::activate_action(&shell.ctx.window, "win.back", None);
        }
        "trailer-preview" => crate::trailer::close_preview(),
        "trailer-window"
        | "discussion"
        | "discussion-none"
        | "discussion-waiting"
        | "discussion-signed-out" => close_dialogs(&shell.ctx.window),
        "about" | "join" | "join-link" | "join-group" | "join-ended" | "welcome"
        | "first-play-welcome" | "first-play-demo" | "first-play-report" | "accounts-kitsu"
        | "torrents-setup" => close_dialogs(&shell.ctx.window),
        #[cfg(feature = "fixtures")]
        "search-failed" | "search-paging" => {
            crate::showcase::set_mood(crate::showcase::Mood::Usual);
            shell.search_page().browse(&kaeri_model::search::SearchQuery::default());
        }
        "torrent-open-link"
        | "torrent-open-confirm"
        | "torrent-open-on"
        | "torrent-open-change" => {
            close_dialogs(&shell.ctx.window);
            close_dialogs(&shell.ctx.window);
        }
        "torrent-open-untracked" => shell.player.close(),
        "torrents-on"
        | "torrents-edit-proxy"
        | "torrents-checking"
        | "torrents-check-failed"
        | "torrents-saved" => {
            close_dialogs(&shell.ctx.window);
            #[cfg(feature = "fixtures")]
            {
                shell.ctx.hub.check_answer.replace(None);
                crate::pages::torrents::forget_checks();
            }
            shell.ctx.hub.send(Command::SetTorrentProxy { proxy: None });
            shell.ctx.hub.send(Command::SetSetting {
                key: kaeri_model::settings::SettingKey::Torrents,
                value: kaeri_core::settings::SettingValue::Bool(false),
            });
        }
        "accounts-code" => {
            crate::pages::accounts::device_code(&shell.ctx, None);
            shell.ctx.hub.known.borrow_mut().signing.remove("simkl");
        }
        "offline" => shell.ctx.hub.send(Command::NetworkPath {
            cellular: false,
            constrained: false,
            offline: false,
        }),
        "resumed" => shell.player.close(),
        #[cfg(feature = "fixtures")]
        "search-filters" | "search-filters-none" => {
            shell.search_page().browse(&kaeri_model::search::SearchQuery::default())
        }
        #[cfg(feature = "fixtures")]
        stats if stats.starts_with("player-stats") => {
            shell.player.shot_stats_away();
            shell.player.hold_up_next.set(false);
            shell.player.close();
        }
        "show-settings" => {
            close_dialogs(&shell.ctx.window);
            let _ = WidgetExt::activate_action(&shell.ctx.window, "win.back", None);
        }
        "settings-upscaling" => {
            upscaling(shell, "off");
            if let Some(nav) = shell.ctx.nav() {
                nav.pop();
            }
        }
        #[cfg(feature = "fixtures")]
        owner if owner.starts_with("store-") => {
            crate::showcase::set_mood(crate::showcase::Mood::Usual);
            if let Some(more) = more_button(&shell.ctx.window) {
                more.popdown();
            }
            close_dialogs(&shell.ctx.window);
            if let Some(nav) = shell.ctx.nav() {
                nav.pop();
            }
        }
        "provider" | "store" => {
            if let Some(nav) = shell.ctx.nav() {
                nav.pop();
            }
        }
        "player-tracks"
        | "player-episodes"
        | "player-episodes-focus"
        | "player-episodes-ranges"
        | "player-episodes-ranges-menu" => {
            shell.player.close_menus();
            shell.player.close();
        }
        "player" | "player-marks" | "player-undo" | "mini" | "pad-help-player"
        | "demo-starting" => shell.player.close(),
        "first-play-style" => {
            shell.player.close_menus();
            shell.player.close();
        }
        "demo-title" => {
            if let Some(nav) = shell.ctx.nav() {
                nav.pop();
            }
        }
        "home-arrows" | "motion-arrows" => {
            if let Some(shelf) = paging_shelf(shell) {
                let _ = WidgetExt::activate_action(
                    &shelf,
                    "shelf.hover",
                    Some(&false.to_variant()),
                );
            }
        }
        party if party.starts_with("party-") => party_unstage(shell, party),
        source if source.starts_with("source-") => source_unstage(shell),
        matched if matched == "match" || matched.starts_with("match-") => match_unstage(shell),
        tv if tv.starts_with("tv-") => tv_unstage(shell, tv),
        cast if cast.starts_with("cast-") => cast_unstage(shell),
        _ => {}
    }
}

/// Scroll every scrolled window under `widget` to its end.
fn scroll_to_end(widget: &gtk::Widget) {
    if let Some(scrolled) = widget.downcast_ref::<gtk::ScrolledWindow>() {
        let adjustment = scrolled.vadjustment();
        adjustment.set_value(adjustment.upper() - adjustment.page_size());
    }
    let mut child = widget.first_child();
    while let Some(c) = child {
        scroll_to_end(&c);
        child = c.next_sibling();
    }
}

/// A newer release with an AppImage for each machine, as the daily check tells of one: a scene
/// goes through the update's steps, fetching nothing.
fn offer_update(shell: &Rc<Shell>) {
    use kaeri_model::updates::{Release, UpdateStatus};
    // As the daily check tells of a release with an AppImage for each machine: a scene
    // goes through the update's steps, fetching nothing.
    let appimage = |arch: &str| kaeri_model::updates::ReleaseAsset {
        name: format!("kaeri-0.11.0-linux-{arch}.AppImage"),
        url: format!("https://releases.example/v0.11.0/kaeri-0.11.0-linux-{arch}.AppImage"),
        sha256: "5e".repeat(32),
        size: 118_400_000,
    };
    shell.ctx.hub.take(vec![kaeri_model::event::Event::Update(UpdateStatus::Available {
        release: Box::new(Release {
            version: "0.11.0".into(),
            published_at: 0,
            page_url: "https://example.test/releases/v0.11.0".into(),
            notes: String::new(),
            macos: None,
            ios: None,
            android: None,
            android_tv: None,
            android_tv_armv7: None,
            windows_x64: None,
            windows_arm64: None,
            linux_x86_64: Some(appimage("x86_64")),
            linux_aarch64: Some(appimage("aarch64")),
            beta: false,
        }),
        notify: true,
    })]);
}

/// Set upscaling, which the fixture scene keeps off for the player shots.
fn upscaling(shell: &Rc<Shell>, mode: &str) {
    shell.ctx.hub.send(Command::SetSetting {
        key: kaeri_model::settings::SettingKey::Upscaling,
        value: kaeri_core::settings::SettingValue::Str(mode.into()),
    });
}

/// Play the player shots' episode, paused once it plays so the controls stay up.
/// A controller pressing `buttons` one after another, the first `after` milliseconds from now.
fn pad_press(shell: &Rc<Shell>, buttons: &[PadButton], after: u64) {
    let shell = Rc::clone(shell);
    let buttons = buttons.to_vec();
    glib::timeout_add_local_once(Duration::from_millis(after), move || {
        let Some(pad) = shell.pad() else { return };
        pad.shot_enter();
        for button in buttons {
            pad.shot_press(button);
        }
    });
}

fn play(shell: &Rc<Shell>) {
    let (id, episode) = PLAYING;
    let _ = WidgetExt::activate_action(
        &shell.ctx.window,
        "win.play",
        Some(&(id, episode.to_owned()).to_variant()),
    );
    let hub = Rc::clone(&shell.ctx.hub);
    glib::timeout_add_local_once(Duration::from_secs(7), move || {
        hub.send(Command::Player(kaeri_model::command::PlayerCommand::TogglePause));
    });
}

/// The join dialog with an invite pasted, and the showcase's answer to checking it.
#[cfg(feature = "fixtures")]
fn join(shell: &Rc<Shell>) {
    crate::pages::join::open(&shell.ctx, Some(crate::showcase::INVITE));
    let hub = Rc::clone(&shell.ctx.hub);
    glib::timeout_add_local_once(Duration::from_millis(900), move || {
        hub.show_instead(vec![crate::showcase::invite()]);
    });
}

/// The showcase's store that this computer owns, with its owner's tools: the menu, or one of
/// its dialogs answered by the showcase.
#[cfg(feature = "fixtures")]
fn store_owner(shell: &Rc<Shell>, shot: &str) -> f64 {
    use crate::{
        pages::store_owner,
        showcase::{Mood, set_mood},
    };
    let ctx = &shell.ctx;
    shell.go(Dest::Stores);
    crate::pages::stores::push_store(ctx, "mika");
    match shot {
        "store-menu" => {
            let window = ctx.window.clone();
            glib::timeout_add_local_once(Duration::from_millis(800), move || {
                if let Some(more) = more_button(&window) {
                    more.popup();
                }
            });
        }
        "store-invite" => {
            store_owner::invite(ctx, "mika", false);
            let window = ctx.window.clone();
            glib::timeout_add_local_once(Duration::from_millis(600), move || {
                let Some(dialog) = window.visible_dialog() else { return };
                if let Some(field) = first::<adw::EntryRow>(dialog.upcast_ref()) {
                    field.set_text("Kai");
                }
                if let Some(make) = default_button(&dialog) {
                    make.emit_clicked();
                }
            });
        }
        "store-invite-name" => {
            store_owner::invite(ctx, "mika", false);
            let window = ctx.window.clone();
            glib::timeout_add_local_once(Duration::from_millis(600), move || {
                let Some(dialog) = window.visible_dialog() else { return };
                if let Some(field) = first::<adw::EntryRow>(dialog.upcast_ref()) {
                    field.set_text("Kai");
                }
            });
        }
        // A group invite: five uses, a name for it, made.
        "store-invite-club" => {
            store_owner::invite(ctx, "mika", false);
            let window = ctx.window.clone();
            glib::timeout_add_local_once(Duration::from_millis(600), move || {
                let Some(dialog) = window.visible_dialog() else { return };
                if let Some(row) = nth::<adw::ComboRow>(dialog.upcast_ref(), 0) {
                    row.set_selected(1);
                }
                if let Some(field) = first::<adw::EntryRow>(dialog.upcast_ref()) {
                    field.set_text("Book club");
                }
                if let Some(make) = default_button(&dialog) {
                    make.emit_clicked();
                }
            });
        }
        // Any number of times, with no end date, before it is made.
        "store-invite-any" => {
            store_owner::invite(ctx, "mika", false);
            let window = ctx.window.clone();
            glib::timeout_add_local_once(Duration::from_millis(600), move || {
                let Some(dialog) = window.visible_dialog() else { return };
                if let Some(row) = nth::<adw::ComboRow>(dialog.upcast_ref(), 0) {
                    row.set_selected(2);
                }
                if let Some(row) = nth::<adw::ComboRow>(dialog.upcast_ref(), 1) {
                    row.set_selected(3);
                }
                if let Some(field) = first::<adw::EntryRow>(dialog.upcast_ref()) {
                    field.set_text("Open night");
                }
            });
        }
        "store-invite-older" => {
            set_mood(Mood::Older);
            store_owner::invite(ctx, "mika", false);
            let window = ctx.window.clone();
            glib::timeout_add_local_once(Duration::from_millis(900), move || {
                let Some(dialog) = window.visible_dialog() else { return };
                if let Some(field) = first::<adw::EntryRow>(dialog.upcast_ref()) {
                    field.set_text("Kai");
                }
            });
        }
        // The Book club invite's Uses pressed.
        "store-members-uses" => {
            store_owner::members(ctx, "mika");
            let window = ctx.window.clone();
            glib::timeout_add_local_once(Duration::from_millis(900), move || {
                let Some(dialog) = window.visible_dialog() else { return };
                if let Some(uses) = nth_labelled(dialog.upcast_ref(), "Uses", 0) {
                    uses.emit_clicked();
                }
            });
        }
        "store-device" => {
            store_owner::invite(ctx, "mika", true);
            let window = ctx.window.clone();
            glib::timeout_add_local_once(Duration::from_millis(600), move || {
                if let Some(make) = window.visible_dialog().as_ref().and_then(default_button) {
                    make.emit_clicked();
                }
            });
        }
        "store-members" => store_owner::members(ctx, "mika"),
        "store-members-loading" => {
            set_mood(Mood::Silent);
            store_owner::members(ctx, "mika");
        }
        "store-members-empty" => {
            set_mood(Mood::Bare);
            store_owner::members(ctx, "mika");
        }
        "store-members-failed" => {
            set_mood(Mood::Failing);
            store_owner::members(ctx, "mika");
        }
        "store-joining" => store_owner::joining(ctx, "mika"),
        "store-joining-closed" => {
            set_mood(Mood::Closed);
            store_owner::joining(ctx, "mika");
        }
        "store-publish" => store_owner::publish(ctx, "mika"),
        "store-publish-empty" => {
            set_mood(Mood::Bare);
            store_owner::publish(ctx, "mika");
        }
        // The first plugin's Publish pressed: the store still working on it, or refusing it.
        "store-publish-busy" | "store-publish-failed" => {
            if shot == "store-publish-failed" {
                set_mood(Mood::PublishFails);
            }
            store_owner::publish(ctx, "mika");
            let window = ctx.window.clone();
            glib::timeout_add_local_once(Duration::from_millis(1000), move || {
                let Some(dialog) = window.visible_dialog() else { return };
                if let Some(publish) = button_labelled(dialog.upcast_ref(), "Publish") {
                    publish.emit_clicked();
                }
            });
            return 2.5;
        }
        _ => {}
    }
    2.0
}

/// The first button under `widget` reading `label`, depth first.
#[cfg(feature = "fixtures")]
fn button_labelled(widget: &gtk::Widget, label: &str) -> Option<gtk::Button> {
    if let Some(button) = widget.downcast_ref::<gtk::Button>()
        && button.label().as_deref() == Some(label)
    {
        return Some(button.clone());
    }
    let mut child = widget.first_child();
    while let Some(next) = child {
        if let Some(found) = button_labelled(&next, label) {
            return Some(found);
        }
        child = next.next_sibling();
    }
    None
}

/// The first entry row under `widget` titled `title`, depth first.
#[cfg(feature = "fixtures")]
fn entry_titled(widget: &gtk::Widget, title: &str) -> Option<adw::EntryRow> {
    if let Some(row) = widget.downcast_ref::<adw::EntryRow>()
        && row.title() == title
    {
        return Some(row.clone());
    }
    let mut child = widget.first_child();
    while let Some(next) = child {
        if let Some(found) = entry_titled(&next, title) {
            return Some(found);
        }
        child = next.next_sibling();
    }
    None
}

/// Run `then` once the proxy `by_proxy` sets has reached the app, so a form opens filled in.
#[cfg(feature = "fixtures")]
fn once_proxy_saved(shell: &Rc<Shell>, then: impl FnOnce(&Rc<crate::window::Ctx>) + 'static) {
    let ctx = Rc::clone(&shell.ctx);
    let mut then = Some(then);
    let mut tries = 0;
    glib::timeout_add_local(Duration::from_millis(100), move || {
        tries += 1;
        let saved = crate::pages::torrents::current(&ctx)
            .proxy
            .is_some_and(|proxy| proxy.host == "127.0.0.1");
        if !saved && tries < 40 {
            return glib::ControlFlow::Continue;
        }
        if let Some(then) = then.take() {
            then(&ctx);
        }
        glib::ControlFlow::Break
    });
}

/// Torrents on through a proxy nothing listens on: its check fails on this machine, and never
/// looks up where the run's own traffic leaves from.
fn by_proxy(shell: &Rc<Shell>) {
    use kaeri_core::settings::SettingValue;
    use kaeri_model::proxy::TorrentProxy;
    shell.ctx.hub.send(Command::SetTorrentProxy {
        proxy: Some(TorrentProxy {
            host: "127.0.0.1".into(),
            port: 9,
            user: None,
            password: None,
        }),
    });
    shell.ctx.hub.send(Command::SetSetting {
        key: kaeri_model::settings::SettingKey::Torrents,
        value: SettingValue::Bool(true),
    });
}

#[cfg(feature = "fixtures")]
/// Scroll the page on screen so the preferences group titled `title` is at its top.
fn scroll_to_group(root: &gtk::Widget, title: &str) {
    fn groups(widget: &gtk::Widget, out: &mut Vec<adw::PreferencesGroup>) {
        if let Some(group) = widget.downcast_ref::<adw::PreferencesGroup>()
            && group.is_mapped()
        {
            out.push(group.clone());
        }
        let mut child = widget.first_child();
        while let Some(next) = child {
            groups(&next, out);
            child = next.next_sibling();
        }
    }
    let mut found = Vec::new();
    groups(root, &mut found);
    let Some(group) = found.into_iter().find(|g| g.title() == title) else { return };
    let Some(scroll) = group.ancestor(gtk::ScrolledWindow::static_type()) else { return };
    let Some(scroll) = scroll.downcast_ref::<gtk::ScrolledWindow>() else { return };
    if let Some(child) = scroll.child()
        && let Some(point) = group.compute_point(&child, &gtk::graphene::Point::new(0.0, 0.0))
    {
        scroll.vadjustment().set_value(f64::from(point.y()) - 12.0);
    }
}

/// The `n`th `T` under `widget`, in order.
#[cfg(feature = "fixtures")]
fn nth<T: IsA<gtk::Widget>>(widget: &gtk::Widget, n: usize) -> Option<T> {
    fn walk<T: IsA<gtk::Widget>>(
        widget: &gtk::Widget,
        n: usize,
        seen: &mut usize,
    ) -> Option<T> {
        if let Some(found) = widget.downcast_ref::<T>() {
            if *seen == n {
                return Some(found.clone());
            }
            *seen += 1;
        }
        let mut child = widget.first_child();
        while let Some(next) = child {
            if let Some(found) = walk(&next, n, seen) {
                return Some(found);
            }
            child = next.next_sibling();
        }
        None
    }
    walk(widget, n, &mut 0)
}

/// The `n`th button under `widget` labelled `label`.
#[cfg(feature = "fixtures")]
fn nth_labelled(widget: &gtk::Widget, label: &str, n: usize) -> Option<gtk::Button> {
    fn walk(
        widget: &gtk::Widget,
        label: &str,
        n: usize,
        seen: &mut usize,
    ) -> Option<gtk::Button> {
        if let Some(button) = widget.downcast_ref::<gtk::Button>()
            && button.label().as_deref() == Some(label)
        {
            if *seen == n {
                return Some(button.clone());
            }
            *seen += 1;
        }
        let mut child = widget.first_child();
        while let Some(next) = child {
            if let Some(found) = walk(&next, label, n, seen) {
                return Some(found);
            }
            child = next.next_sibling();
        }
        None
    }
    walk(widget, label, n, &mut 0)
}

/// The first widget of type `T` under `widget`, depth first.
#[cfg(feature = "fixtures")]
fn first<T: IsA<gtk::Widget>>(widget: &gtk::Widget) -> Option<T> {
    if let Some(found) = widget.downcast_ref::<T>() {
        return Some(found.clone());
    }
    let mut child = widget.first_child();
    while let Some(next) = child {
        if let Some(found) = first(&next) {
            return Some(found);
        }
        child = next.next_sibling();
    }
    None
}

#[cfg(not(feature = "fixtures"))]
fn join(shell: &Rc<Shell>) {
    crate::pages::join::open(&shell.ctx, None);
}

fn close_dialogs(window: &adw::ApplicationWindow) {
    if let Some(dialog) = window.visible_dialog() {
        dialog.force_close();
    }
}

/// A dialog's default button: what Return presses.
#[cfg(feature = "fixtures")]
fn default_button(dialog: &adw::Dialog) -> Option<gtk::Button> {
    dialog.default_widget()?.downcast().ok()
}

/// Draw the window, dialogs and all, into a PNG at its own scale.
fn capture(
    window: &adw::ApplicationWindow,
    path: &std::path::Path,
    over: Option<(gtk::Widget, f32, f32)>,
) -> Result<(), String> {
    let (width, height) = (window.width(), window.height());
    if width <= 0 || height <= 0 {
        return Err("the window has no size".into());
    }
    // The window on its own first: with a popover on top, an empty window would still give a
    // picture, of the popover alone.
    let paintable = gtk::WidgetPaintable::new(Some(window));
    let drawn = gtk::Snapshot::new();
    paintable.snapshot(&drawn, f64::from(width), f64::from(height));
    let page = drawn.to_node();
    let page = page.ok_or_else(|| {
        format!(
            "the window drew nothing: {width}x{height}, mapped {}, decorated {}, needs {}x{}",
            window.is_mapped(),
            window.is_decorated(),
            window.measure(gtk::Orientation::Horizontal, -1).0,
            window.measure(gtk::Orientation::Vertical, -1).0,
        )
    })?;
    let snapshot = gtk::Snapshot::new();
    snapshot.append_node(&page);
    // A popover is a surface of its own, which the window's drawing leaves out: drawn on top.
    if let Some((widget, x, y)) = over {
        let popover = gtk::WidgetPaintable::new(Some(&widget));
        // The window draws its frame's bounds squeezed into its size: the popover goes through
        // the same, so it lands where it sits over the window.
        let frame = window
            .compute_bounds(window)
            .unwrap_or_else(|| gtk::graphene::Rect::new(0.0, 0.0, width as f32, height as f32));
        snapshot.save();
        snapshot.scale(width as f32 / frame.width(), height as f32 / frame.height());
        snapshot.translate(&gtk::graphene::Point::new(x - frame.x(), y - frame.y()));
        popover.snapshot(&snapshot, f64::from(widget.width()), f64::from(widget.height()));
        snapshot.restore();
    }
    let node = snapshot.to_node().ok_or("nothing drawn")?;
    let renderer = window.renderer().ok_or("no renderer")?;
    let bounds = gtk::graphene::Rect::new(0.0, 0.0, width as f32, height as f32);
    let texture = renderer.render_texture(&node, Some(&bounds));
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    texture.save_to_png(path).map_err(|e| e.to_string())
}

/// The window drawn into `path` just after its next frame is painted, with the shot's popover
/// on top; captured at once if no frame comes within a second.
async fn painted(shell: &Rc<Shell>, path: &std::path::Path, shot: &str) -> Result<(), String> {
    let window = shell.ctx.window.clone();
    let take = {
        let (player, window, path) =
            (Rc::clone(&shell.player), window.clone(), path.to_owned());
        let shot = shot.to_owned();
        move || {
            let over = match shot.as_str() {
                "player-marks" => player.shot_popover(),
                "player-tracks" => player.shot_tracks_popover(),
                "player-episodes" | "player-episodes-focus" | "player-episodes-ranges" => {
                    player.shot_episodes_popover()
                }
                "synopsis-ranges" => range_popover(&window),
                #[cfg(feature = "fixtures")]
                "sync-more" => crate::sync::devices::shot_more_popover(&window),
                #[cfg(feature = "fixtures")]
                "browse-period-menu" => period_popover(&window),
                "first-play-style" => player.shot_style_popover(),
                "party-title-menu" | "store-menu" | "match-fix-menu" => {
                    title_menu_popover(&window)
                }
                "trailer-preview" => crate::trailer::shot_preview(window.upcast_ref()),
                #[cfg(feature = "fixtures")]
                "read-library-filters"
                | "read-search-filters"
                | "read-paged-settings"
                | "read-mode"
                | "read-strip-mode"
                | "read-chapters"
                | "read-chapters-list"
                | "read-novel-paper" => open_popover(&window),
                party if party.starts_with("party-") => player.shot_party_popover(),
                _ => None,
            };
            capture(&window, &path, over)
        }
    };
    let Some(clock) = window.frame_clock() else { return take() };
    let (tx, rx) = async_channel::bounded::<Result<(), String>>(1);
    let once = std::cell::Cell::new(Some(take.clone()));
    let handler = clock.connect_after_paint(move |_| {
        // Drawn outside the frame, so the capture's own rendering does not run inside the
        // window's, but before anything else on the loop can ask for a new layout.
        if let Some(take) = once.take() {
            let tx = tx.clone();
            glib::idle_add_local_full(glib::Priority::HIGH, move || {
                let _ = tx.try_send(take());
                glib::ControlFlow::Break
            });
        }
    });
    window.queue_draw();
    let taken = glib::future_with_timeout(Duration::from_secs(1), rx.recv()).await;
    clock.disconnect(handler);
    match taken {
        Ok(Ok(taken)) => taken,
        _ => take(),
    }
}

async fn wait(seconds: f64) {
    glib::timeout_future(Duration::from_secs_f64(seconds)).await;
}

/// The shots asked for: a comma-separated list, or every one.
pub fn named(list: Option<&str>) -> Vec<String> {
    match list {
        None | Some("all") => ALL.iter().map(|s| (*s).to_owned()).collect(),
        Some(list) => list
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .flat_map(|s| match s {
                "party" => PARTY.iter().map(|p| (*p).to_owned()).collect(),
                "skips" => SKIPS.iter().map(|p| (*p).to_owned()).collect(),
                "found" => FOUND.iter().map(|p| (*p).to_owned()).collect(),
                "sources" => SOURCES.iter().map(|p| (*p).to_owned()).collect(),
                "lawful" => LAWFUL.iter().map(|p| (*p).to_owned()).collect(),
                "pad" => PAD.iter().map(|p| (*p).to_owned()).collect(),
                "stats" => STATS.iter().map(|p| (*p).to_owned()).collect(),
                "tv" => TV.iter().map(|p| (*p).to_owned()).collect(),
                "cast" => CAST.iter().map(|p| (*p).to_owned()).collect(),
                #[cfg(feature = "fixtures")]
                "sync" => crate::sync::shots::NAMES.iter().map(|p| (*p).to_owned()).collect(),
                #[cfg(feature = "fixtures")]
                "reading" => {
                    crate::shots_reading::NAMES.iter().map(|p| (*p).to_owned()).collect()
                }
                #[cfg(feature = "fixtures")]
                "reading-discovery" => {
                    crate::shots_read_discovery::NAMES.iter().map(|p| (*p).to_owned()).collect()
                }
                #[cfg(feature = "fixtures")]
                "prelaunch" => {
                    crate::shots_prelaunch::NAMES.iter().map(|p| (*p).to_owned()).collect()
                }
                #[cfg(feature = "fixtures")]
                "prelaunch-demo" => {
                    crate::shots_prelaunch::DEMO.iter().map(|p| (*p).to_owned()).collect()
                }
                "match" => MATCH.iter().map(|p| (*p).to_owned()).collect(),
                one => vec![one.to_owned()],
            })
            .collect(),
    }
}

/// Warn of each widget whose minimum width is over `limit`, the narrowest of a branch that is:
/// what keeps a window from fitting its size.
fn wide_widgets(widget: &gtk::Widget, depth: usize, limit: i32) {
    let (min, ..) = widget.measure(gtk::Orientation::Horizontal, -1);
    if min <= limit {
        return;
    }
    let mut over = false;
    let mut child = widget.first_child();
    while let Some(c) = child {
        if c.measure(gtk::Orientation::Horizontal, -1).0 > limit {
            over = true;
        }
        wide_widgets(&c, depth + 1, limit);
        child = c.next_sibling();
    }
    if !over {
        tracing::warn!(depth, min, kind = %widget.type_(), name = %widget.widget_name(), css = ?widget.css_classes(), "too wide");
    }
}

/// Every widget under `root`, parents first.
fn descendants(root: &gtk::Widget) -> Vec<gtk::Widget> {
    let mut all = vec![root.clone()];
    let mut at = 0;
    while at < all.len() {
        let mut child = all[at].first_child();
        while let Some(c) = child {
            child = c.next_sibling();
            all.push(c);
        }
        at += 1;
    }
    all
}

/// Home's paging buttons as the pointer over a shelf shows them, a page in so both show.
fn show_arrows(shell: &Rc<Shell>) {
    if let Some(shelf) = paging_shelf(shell) {
        let _ = WidgetExt::activate_action(&shelf, "shelf.hover", Some(&true.to_variant()));
        let _ = WidgetExt::activate_action(&shelf, "shelf.page", Some(&1i32.to_variant()));
    }
}

/// Home's shelves, top to bottom.
fn home_shelves(shell: &Rc<Shell>) -> Vec<gtk::Widget> {
    let Some(root) = shell.ctx.window.content() else { return Vec::new() };
    descendants(root.upcast_ref())
        .into_iter()
        .filter(|w| w.has_css_class("home-shelf"))
        .collect()
}

/// The shelf the paging shots page: the first poster shelf, past Continue.
fn paging_shelf(shell: &Rc<Shell>) -> Option<gtk::Widget> {
    let shelves = home_shelves(shell);
    shelves.get(1).or(shelves.first()).cloned()
}

/// A motion step, logged with the wall clock and where `frame` sits in the window, so a
/// recording's frames can be matched to it and cropped to it.
fn mark_step(step: &str, frame: &gtk::Widget) {
    let window = frame.root().map(|r| r.upcast::<gtk::Widget>());
    let bounds = window.and_then(|w| frame.compute_bounds(&w));
    let (x, y, w, h) =
        bounds.map_or((0.0, 0.0, 0.0, 0.0), |b| (b.x(), b.y(), b.width(), b.height()));
    eprintln!("motion step {step} at {} box {x:.0} {y:.0} {w:.0} {h:.0}", glib::real_time()); // i18n-skip
}

/// Scroll Home so `widget` sits near the top of the page.
fn bring_into_view(widget: &gtk::Widget) {
    let mut at = widget.parent();
    while let Some(w) = at {
        if let Some(scroll) = w.downcast_ref::<gtk::ScrolledWindow>()
            && scroll.vscrollbar_policy() != gtk::PolicyType::Never
            && let Some(child) = scroll.child()
            && let Some(point) =
                widget.compute_point(&child, &gtk::graphene::Point::new(0.0, 0.0))
        {
            scroll.vadjustment().set_value(f64::from(point.y()) - 24.0);
            return;
        }
        at = w.parent();
    }
}

/// The last suggestion card Home shows, and its title's id.
fn last_suggestion(shell: &Rc<Shell>) -> Option<(u32, gtk::Widget)> {
    let root = shell.ctx.window.content()?;
    descendants(root.upcast_ref())
        .into_iter()
        .filter(|w| w.has_css_class("home-suggestion"))
        .filter_map(|w| {
            let id = w.downcast_ref::<gtk::Button>()?.action_target_value()?.get::<u32>()?;
            Some((id, w))
        })
        .last()
}

/// Not interested on the last suggestion card Home shows.
fn dismiss_last_card(shell: &Rc<Shell>) {
    if let Some((id, _)) = last_suggestion(shell) {
        let _ = WidgetExt::activate_action(
            &shell.ctx.window,
            "win.home-dismiss",
            Some(&id.to_variant()),
        );
    }
}

/// The room the party shots hold their party in.
#[cfg(feature = "fixtures")]
const ROOM: &str = "quiet-harbor-k7m2xq9ad4tnwe3p";

#[cfg(feature = "fixtures")]
thread_local! {
    /// The party shot's server, kept until the shot is taken.
    static PARTY_SERVER: std::cell::RefCell<Option<crate::fixtures::FakeParty>> =
        const { std::cell::RefCell::new(None) };
    /// The TV the slow pairing shot waits on, on this computer, which never answers.
    static SILENT_TV: std::cell::RefCell<Option<std::net::TcpListener>> =
        const { std::cell::RefCell::new(None) };
    /// The address the joining shot waits on, which never answers.
    static SILENT: std::cell::RefCell<Option<std::net::TcpListener>> =
        const { std::cell::RefCell::new(None) };
}

/// Put the window in the party state `shot` shows, and say how long to let it settle.
#[cfg(feature = "fixtures")]
fn party_stage(shell: &Rc<Shell>, shot: &str) -> f64 {
    use crate::{
        fixtures::FakeParty,
        party::{form, source},
    };
    use kaeri_core::ids::AnilistId;
    use kaeri_model::party::{NoSourceWhy, PartyInvite};

    let ctx = Rc::clone(&shell.ctx);
    let title = AnilistId::new(TITLE);
    let open_title = || {
        shell.go(Dest::Home);
        let _ = WidgetExt::activate_action(
            &shell.ctx.window,
            "win.open-title",
            Some(&TITLE.to_variant()),
        );
    };
    let serve = |refuse: Option<&str>| {
        let party = FakeParty::start(refuse)?;
        let server = party.server();
        PARTY_SERVER.with(|p| p.replace(Some(party)));
        Some(server)
    };
    // Each scene's own notices, not the last one's still queued.
    shell.ctx.notices.dismiss_each();
    match shot {
        "party-form" => {
            open_title();
            let ctx = Rc::clone(&ctx);
            glib::timeout_add_local_once(Duration::from_millis(1500), move || {
                form::open(&ctx, form::For::Episode(title, "7".into()));
            });
            3.5
        }
        "party-together" => {
            shell.go(Dest::Together);
            2.5
        }
        "party-lobby" | "party-lobby-invited" | "party-lobby-waiting" | "party-chat-lobby" => {
            let Some(server) = serve(None) else { return 0.0 };
            let file = |episode: &str| {
                kaeri_party::file::file_name("Stardust Wayfarer", TITLE, episode) // i18n-skip
            };
            if shot != "party-lobby-invited" {
                ctx.hub.send(Command::Party(kaeri_model::party::PartyCommand::Lobby {
                    server,
                    room: ROOM.into(),
                    name: Some("Ada".into()),
                    pick: Some(kaeri_model::party::PartyEpisode {
                        id: title,
                        episode: "9".into(),
                    }),
                }));
                crate::pages::lobby::push(&ctx, None);
                let chatty = shot == "party-chat-lobby";
                glib::timeout_add_local_once(Duration::from_millis(2500), move || {
                    PARTY_SERVER.with(|p| {
                        let p = p.borrow();
                        let Some(party) = p.as_ref() else { return };
                        party.add(ROOM, "Mika");
                        party.opens("Mika", &file("9"), 1440.0);
                        party.ready("Mika", true);
                        party.add(ROOM, "Ren");
                        party.opens("Ren", &file("9"), 1440.0);
                        party.ready("Ren", false);
                        if chatty {
                            party.says("Mika", "snacks ready?");
                            party.says("Ren", "two minutes, making tea");
                            party.says("Mika", "the soundtrack is at music.example/ost");
                        }
                    });
                });
                if chatty {
                    // The conversation sits under the room: scrolled down to it.
                    let window = ctx.window.clone();
                    glib::timeout_add_local_once(Duration::from_millis(5500), move || {
                        scroll_to_end(window.upcast_ref());
                    });
                }
                if shot == "party-lobby-waiting" {
                    // Everyone but Ren is ready: the room waits for him, then offers Start anyway.
                    let hub = Rc::clone(&ctx.hub);
                    glib::timeout_add_local_once(Duration::from_millis(4000), move || {
                        hub.send(Command::PartyReady { ready: true });
                    });
                    return 17.0;
                }
                return 7.0;
            }
            PARTY_SERVER.with(|p| {
                let p = p.borrow();
                let Some(party) = p.as_ref() else { return };
                party.add(ROOM, "Mika");
                party.opens("Mika", &file("7"), 1440.0);
                party.playlist("Mika", &[&file("7"), &file("8")]);
            });
            let ctx = Rc::clone(&ctx);
            glib::timeout_add_local_once(Duration::from_millis(1200), move || {
                // The link names episode 5 and Mika; the room says 7, which wins.
                crate::pages::together::open_target(
                    &ctx,
                    &kaeri_model::party::PartyTarget {
                        server: server.clone(),
                        room: ROOM.into(),
                        by: Some("Mika".into()),
                        title: Some("Stardust Wayfarer".into()), // i18n-skip
                        episode: Some("5".into()),
                        id: None,
                        from_code: false,
                    },
                );
            });
            6.0
        }
        "party-title-menu" => {
            open_title();
            let window = shell.ctx.window.clone();
            glib::timeout_add_local_once(Duration::from_millis(2000), move || {
                if let Some(more) = more_button(&window) {
                    more.popup();
                }
            });
            3.5
        }
        "party-joining" | "party-refused" => {
            let server = if shot == "party-joining" {
                crate::fixtures::silent_server().map(|(listener, address)| {
                    SILENT.with(|s| s.replace(Some(listener)));
                    address
                })
            } else {
                serve(Some("this room takes members by invitation only"))
            };
            open_title();
            let ctx = Rc::clone(&ctx);
            glib::timeout_add_local_once(Duration::from_millis(1500), move || {
                form::open(&ctx, form::For::Episode(title, "7".into()));
                form::rehearse(server, true);
            });
            4.5
        }
        "party-invite" | "party-unaired" => {
            let Some(server) = serve(None) else { return 0.0 };
            let (id, episode) =
                if shot == "party-invite" { (TITLE, "7") } else { (PLAYING.0, "6") };
            let link = PartyInvite {
                server,
                room: ROOM.into(),
                id: AnilistId::new(id),
                episode: episode.into(),
            }
            .link();
            shell.go(Dest::Home);
            shell.open_link(&link);
            if shot == "party-unaired" {
                glib::timeout_add_local_once(Duration::from_millis(2500), || {
                    form::rehearse(None, true);
                });
                return 7.0;
            }
            3.5
        }
        "party-nosource" | "party-nosources" | "party-choose" => {
            let Some(server) = serve(None) else { return 0.0 };
            ctx.hub.send(Command::PartyJoin {
                server,
                room: ROOM.into(),
                name: Some("Ada".into()),
            });
            let ctx = Rc::clone(&ctx);
            let shot = shot.to_owned();
            glib::spawn_future_local(async move {
                for _ in 0..50 {
                    if ctx.hub.known.borrow().party.is_some() {
                        break;
                    }
                    wait(0.2).await;
                }
                match shot.as_str() {
                    "party-nosources" => source::open(
                        &ctx,
                        title,
                        "7",
                        NoSourceWhy::NoSources,
                        "no source is set up",
                    ),
                    "party-nosource" => source::open(
                        &ctx,
                        title,
                        "7",
                        NoSourceWhy::NotFound,
                        "could not match this title: sample-source has no episode 7",
                    ),
                    _ => {
                        source::rehearse_choose(&ctx, title, "7");
                        // Before the engine's own answer, which has no sources to give.
                        ctx.hub.take(vec![kaeri_model::event::Event::Sources {
                            id: title,
                            episode: "7".into(),
                            candidates: choices(),
                        }]);
                    }
                }
            });
            4.0
        }
        playing => {
            let Some(server) = serve(None) else { return 0.0 };
            party_playing(shell, server, playing.to_owned());
            match playing {
                "party-lost" => 18.0,
                "party-upnext" => 12.0,
                "party-left" | "party-ready" => 16.0,
                "party-chat-failed" => 19.0,
                _ => 14.0,
            }
        }
    }
}

/// A party playing the title's episode 7, held paused at the start as a room waiting to be
/// ready is, with Mika ready and Ren not, then the state `shot` shows.
#[cfg(feature = "fixtures")]
fn party_playing(shell: &Rc<Shell>, server: String, shot: String) {
    use kaeri_core::ids::AnilistId;
    use kaeri_model::command::PlayerCommand;
    let shell = Rc::clone(shell);
    shell.player.hold_up_next.set(shot != "party-upnext");
    let started = std::time::Instant::now();
    // Until `at` seconds into the scene, so a notice is still up when the shot is taken.
    let until = move |at: f64| wait((at - started.elapsed().as_secs_f64()).max(0.0));
    glib::spawn_future_local(async move {
        let hub = Rc::clone(&shell.ctx.hub);
        match shot.as_str() {
            "party-chat-count" => party_chat_setup(|p| p.set_chat(true, Some(40))),
            "party-chat-off" => party_chat_setup(|p| p.set_chat(false, None)),
            "party-chat-failed" => party_chat_setup(|p| p.swallow_chat(true)),
            "party-chat-hidden" => shell.ctx.prefs.set_hide_chat(true),
            _ => {}
        }
        hub.send(Command::PartyJoin { server, room: ROOM.into(), name: Some("Ada".into()) });
        hub.send(Command::Play {
            id: AnilistId::new(TITLE),
            episode: "7".into(),
            from_start: true,
        });
        // Until this device's member has the episode open, for the others to open the same.
        let mut you = None;
        for _ in 0..100 {
            wait(0.2).await;
            you = hub
                .known
                .borrow()
                .party_members
                .as_ref()
                .and_then(|m| m.iter().find(|m| m.you).cloned());
            if you.as_ref().is_some_and(|y| y.file.is_some() && y.duration.is_some()) {
                break;
            }
        }
        let file = you.as_ref().and_then(|y| y.file.clone()).unwrap_or_default();
        let length = you.and_then(|y| y.duration).unwrap_or(60.0);
        hub.send(Command::Player(PlayerCommand::TogglePause));
        PARTY_SERVER.with(|p| {
            let p = p.borrow();
            let Some(party) = p.as_ref() else { return };
            party.add(ROOM, "Mika");
            party.opens("Mika", &file, length);
            party.add(ROOM, "Ren");
            let other = file.replace("Episode 7", "Episode 8");
            party.opens("Ren", if shot == "party-panel" { &other } else { &file }, length);
            party.ready("Mika", true);
            party.ready("Ren", false);
        });
        wait(0.8).await;
        // The joins said, so the shot shows its own notice.
        shell.ctx.notices.dismiss_each();
        let party = |act: &dyn Fn(&crate::fixtures::FakeParty)| {
            PARTY_SERVER.with(|p| {
                if let Some(party) = p.borrow().as_ref() {
                    act(party);
                }
            });
        };
        match shot.as_str() {
            "party-panel" => shell.player.show_party(),
            "party-ready" => {
                party(&|p| p.ready("Ren", true));
                wait(0.4).await;
                hub.send(Command::PartyReady { ready: true });
                wait(0.6).await;
                shell.player.show_party();
            }
            "party-chat" => {
                until(11.5).await;
                party(&|p| p.says("Mika", "Starting in a minute, grabbing tea"));
            }
            "party-chat-column" | "party-chat-fold" | "party-chat-moment"
            | "party-chat-field" | "party-chat-count" | "party-chat-popover"
            | "party-chat-failed" | "party-chat-off" | "party-chat-hidden"
            | "party-chat-muted" => {
                chat_scene(&shell, &shot, &party).await;
            }
            "party-left" => {
                until(11.5).await;
                shell.player.show_party();
                until(13.0).await;
                hub.send(Command::PartyLeave);
            }
            "party-lost" => {
                party(&|p| p.go_down(true));
                wait(2.5).await;
                shell.player.show_party();
            }
            "party-close" => shell.ctx.window.close(),
            // Held paused at the start, as a room waiting to be ready is: the bar as it is in a party.
            "party-player" => {}
            "party-upnext" => {
                // Everyone ready and the room playing, then the playhead near the end.
                party(&|p| {
                    p.ready("Ren", true);
                    p.playlist("Mika", &[&file, &file.replace("Episode 7", "Episode 8")]);
                });
                hub.send(Command::PartyReady { ready: true });
                until(7.0).await;
                hub.send(Command::Player(PlayerCommand::TogglePause));
                wait(0.5).await;
                hub.send(Command::Player(PlayerCommand::SeekTo(50.5)));
            }
            _ => {}
        }
    });
}

/// Set the server's chat up before the shot's party joins.
#[cfg(feature = "fixtures")]
fn party_chat_setup(act: impl FnOnce(&crate::fixtures::FakeParty)) {
    PARTY_SERVER.with(|p| {
        if let Some(party) = p.borrow().as_ref() {
            act(party);
        }
    });
}

/// Runs an act on the shot's server.
#[cfg(feature = "fixtures")]
type PartyAct = dyn Fn(&dyn Fn(&crate::fixtures::FakeParty));

/// The chat states the watch together chat board draws, over the playing party. Everything lands
/// at about 11 s and the shot is taken at 14 s, while the lines still hold.
#[cfg(feature = "fixtures")]
async fn chat_scene(shell: &Rc<Shell>, shot: &str, party: &PartyAct) {
    use kaeri_party::Reaction;
    wait(10.0).await;
    party(&|p| p.add(ROOM, "Sol"));
    wait(0.6).await;
    let lines = [
        ("Mika", "ok starting"),
        ("Ren", "the handoff in the rain, I knew it"),
        ("Sol", "wait, who is the new runner"),
    ];
    match shot {
        "party-chat-fold" => {
            for (who, text) in lines.iter().chain(&[
                ("Mika", "this part again"),
                ("Ren", "ready"),
                ("Sol", "one sec"),
            ]) {
                party(&|p| p.says(who, text));
                wait(0.15).await;
            }
        }
        "party-chat-moment" => {
            party(&|p| p.says("Mika", "ok starting"));
            wait(0.3).await;
            party(&|p| {
                p.reacts("Ren", Reaction::Gasp, 1.5);
                p.reacts("Mika", Reaction::Laugh, 3.0);
                p.reacts("Sol", Reaction::Laugh, 3.1);
            });
        }
        "party-chat-off" | "party-chat-hidden" => {
            party(&|p| p.says("Mika", "ok starting"));
            wait(0.4).await;
            shell.player.chat().shot_pop();
        }
        "party-chat-field" => {
            party(&|p| p.says("Mika", lines[0].1));
            wait(0.3).await;
            shell.player.chat().shot_type("the handoff in the rain");
        }
        "party-chat-count" => {
            wait(0.3).await;
            shell.player.chat().shot_type("the handoff in the rain, I knew it, so good");
        }
        "party-chat-failed" => {
            wait(0.3).await;
            shell.player.chat().shot_type("anyone there");
            shell.player.chat().shot_send();
            wait(6.0).await;
            shell.player.show_party();
        }
        "party-chat-popover" | "party-chat-muted" => {
            for (who, text) in lines {
                party(&|p| p.says(who, text));
                wait(0.2).await;
            }
            party(&|p| p.reacts("Ren", Reaction::Laugh, 3.0));
            if shot == "party-chat-muted" {
                shell.ctx.hub.send(Command::Party(kaeri_model::party::PartyCommand::Mute {
                    name: "Sol".into(),
                    muted: true,
                }));
            }
            wait(0.4).await;
            shell.player.show_party();
        }
        _ => {
            for (who, text) in lines {
                party(&|p| p.says(who, text));
                wait(0.3).await;
            }
        }
    }
}

/// Leave the party state a shot put the window in.
#[cfg(feature = "fixtures")]
fn party_unstage(shell: &Rc<Shell>, shot: &str) {
    shell.player.close_menus();
    if let Some(more) = more_button(&shell.ctx.window) {
        more.popdown();
    }
    // Twice: the alert, and the dialog it opened.
    close_dialogs(&shell.ctx.window);
    close_dialogs(&shell.ctx.window);
    shell.ctx.hub.send(Command::PartyLeave);
    shell.ctx.prefs.set_hide_chat(false);
    if shell.is_player() {
        shell.player.close();
    }
    if !matches!(shot, "party-nosource" | "party-nosources" | "party-choose") {
        let _ = WidgetExt::activate_action(&shell.ctx.window, "win.back", None);
    }
    shell.player.hold_up_next.set(false);
    shell.go(Dest::Home);
    let notices = shell.ctx.notices.clone();
    glib::timeout_add_local_once(Duration::from_millis(400), move || {
        PARTY_SERVER.with(|p| p.replace(None));
        SILENT.with(|s| s.replace(None));
        notices.dismiss_each();
    });
}

/// What the sources list for the choose shot's episode.
#[cfg(feature = "fixtures")]
fn choices() -> Vec<kaeri_core::traits::SourceCandidate> {
    use kaeri_core::traits::SourceCandidate;
    let choice =
        |id: &str, title: &str, quality, size: &str, seeders: u32, auto_pick| SourceCandidate {
            id: id.into(),
            provider_id: "sample-source".into(),
            title: title.into(),
            quality: Some(quality),
            label: Some(format!("{seeders} seeders")),
            size: Some(size.into()),
            dual_audio: false,
            dubbed: false,
            auto_pick,
        };
    vec![
        choice("a", "Stardust Wayfarer - 07 (1080p)", 1080, "1.4 GiB", 42, true),
        choice("b", "Stardust Wayfarer - 07 (720p)", 720, "690 MiB", 18, false),
        choice("c", "Stardust Wayfarer - 07 (480p)", 480, "310 MiB", 5, false),
    ]
}

#[cfg(not(feature = "fixtures"))]
fn party_stage(_: &Rc<Shell>, _: &str) -> f64 {
    0.0
}

#[cfg(not(feature = "fixtures"))]
fn party_unstage(_: &Rc<Shell>, _: &str) {}

/// The poster grid on screen.
/// The first cover under `widget`.
fn first_cover(widget: &gtk::Widget) -> Option<gtk::Widget> {
    if widget.is::<crate::fit::Fit>() {
        return Some(widget.clone());
    }
    let mut child = widget.first_child();
    while let Some(next) = child {
        if let Some(found) = first_cover(&next) {
            return Some(found);
        }
        child = next.next_sibling();
    }
    None
}

fn mapped_grid(widget: &gtk::Widget) -> Option<gtk::GridView> {
    if let Some(grid) = widget.downcast_ref::<gtk::GridView>()
        && grid.is_mapped()
    {
        return Some(grid.clone());
    }
    let mut child = widget.first_child();
    while let Some(next) = child {
        if let Some(found) = mapped_grid(&next) {
            return Some(found);
        }
        child = next.next_sibling();
    }
    None
}

/// Play on TV's scenes. Each starts from the window as it is between shots: no sheet up and no
/// TV driven.
#[cfg(feature = "fixtures")]
fn tv_stage(shell: &Rc<Shell>, shot: &str) -> f64 {
    use crate::tv::{
        fixture::{self, BEDROOM, LIVING_ROOM},
        sheet,
    };
    let Some(scene) = fixture::scene() else { return 0.0 };
    let tv = Rc::clone(&shell.ctx.tv);
    let later = |ms: u64, f: Box<dyn FnOnce()>| {
        glib::timeout_add_local_once(Duration::from_millis(ms), f);
    };
    // A paired TV, then Play on TV for episode 3 from 5:12, chosen in the picker: the remote.
    let drive = |name: &'static str| {
        if let Some(link) = scene.pair_link(name) {
            tv.pair_quietly(&link);
        }
        let window = shell.ctx.window.clone();
        later(
            1500,
            Box::new(move || {
                let _ = WidgetExt::activate_action(
                    &window,
                    "win.play-on-tv",
                    Some(&(TITLE, "3".to_owned(), 312.0).to_variant()),
                );
            }),
        );
        let tv = Rc::clone(&tv);
        later(
            2500,
            Box::new(move || {
                if let Some(row) = tv.rows().into_iter().find(|r| r.name == name) {
                    tv.choose(&row);
                }
            }),
        );
    };
    shell.go(Dest::Home);
    match shot {
        "tv-empty" => {
            scene.silence(true);
            tv.open_picker();
            11.5
        }
        "tv-address" => {
            scene.silence(true);
            tv.open_picker();
            later(800, Box::new(|| sheet::shot_address("192.168.1.20:47811")));
            2.0
        }
        "tv-unreachable" => {
            scene.silence(true);
            tv.open_picker();
            // Nothing listens on port 1 of this computer: refused at once.
            let tv = Rc::clone(&tv);
            later(500, Box::new(move || tv.begin("127.0.0.1:1")));
            3.0
        }
        "tv-slow" => {
            // A TV on this computer that takes the connection and never answers, as one on a
            // slow network: nothing leaves the machine.
            scene.silence(true);
            tv.open_picker();
            if let Some((listener, address)) = crate::fixtures::silent_server() {
                SILENT_TV.with(|s| s.replace(Some(listener)));
                let tv = Rc::clone(&tv);
                later(500, Box::new(move || tv.begin(&address)));
            }
            3.0
        }
        "tv-picker" => {
            if let Some(link) = scene.pair_link(BEDROOM) {
                tv.pair_quietly(&link);
            }
            let _ = WidgetExt::activate_action(
                &shell.ctx.window,
                "win.open-title",
                Some(&TITLE.to_variant()),
            );
            let window = shell.ctx.window.clone();
            later(
                1500,
                Box::new(move || {
                    let _ = WidgetExt::activate_action(
                        &window,
                        "win.play-on-tv",
                        Some(&(TITLE, "7".to_owned(), 0.0).to_variant()),
                    );
                }),
            );
            4.0
        }
        "tv-code" | "tv-wrong" | "tv-expired" => {
            tv.open_picker();
            // Chosen once the browse lists it, as a click on its row would.
            let begin = Rc::clone(&tv);
            later(
                1000,
                Box::new(move || {
                    if let Some(row) = begin.rows().into_iter().find(|r| r.name == LIVING_ROOM)
                    {
                        begin.choose(&row);
                    }
                }),
            );
            let tries = match shot {
                "tv-code" => {
                    later(2000, Box::new(|| sheet::shot_code("482")));
                    return 3.0;
                }
                "tv-wrong" => 1,
                _ => 4,
            };
            for n in 0..tries {
                later(2000 + n * 1500, Box::new(|| sheet::shot_code("000000")));
            }
            2.5 + tries as f64 * 1.5
        }
        "tv-remote" => {
            drive(LIVING_ROOM);
            8.0
        }
        "tv-bar" => {
            drive(LIVING_ROOM);
            let window = shell.ctx.window.clone();
            later(7000, Box::new(move || close_dialogs(&window)));
            8.5
        }
        "tv-settings" => {
            drive(LIVING_ROOM);
            let window = shell.ctx.window.clone();
            let settings = Rc::clone(shell);
            later(
                6000,
                Box::new(move || {
                    close_dialogs(&window);
                    settings.settings_pane("integrations");
                }),
            );
            8.0
        }
        "tv-player" => {
            play(shell);
            9.0
        }
        "tv-unpaired" => {
            drive(LIVING_ROOM);
            later(6000, Box::new(move || scene.revoke_all(LIVING_ROOM)));
            9.0
        }
        "tv-bar-unpaired" | "tv-bar-lost" => {
            // Driven, the sheet closed, then the TV forgets this computer or goes away: the
            // sidebar says so.
            let lost = shot == "tv-bar-lost";
            let name = if lost { BEDROOM } else { LIVING_ROOM };
            drive(name);
            let window = shell.ctx.window.clone();
            later(6000, Box::new(move || close_dialogs(&window)));
            later(
                6500,
                Box::new(move || {
                    if lost {
                        scene.switch_off(name);
                    } else {
                        scene.revoke_all(name);
                    }
                }),
            );
            9.5
        }
        "tv-lost" => {
            drive(LIVING_ROOM);
            later(6000, Box::new(move || scene.switch_off(LIVING_ROOM)));
            9.0
        }
        "tv-connecting" => {
            // Paired before, and away now: the remote waits on it, holding what was asked.
            if let Some(link) = scene.pair_link(BEDROOM) {
                tv.pair_quietly(&link);
            }
            later(1000, Box::new(move || scene.switch_off(BEDROOM)));
            let window = shell.ctx.window.clone();
            later(
                1800,
                Box::new(move || {
                    let _ = WidgetExt::activate_action(
                        &window,
                        "win.play-on-tv",
                        Some(&(TITLE, "3".to_owned(), 312.0).to_variant()),
                    );
                }),
            );
            let tv = Rc::clone(&tv);
            later(
                2600,
                Box::new(move || {
                    if let Some(row) = tv.rows().into_iter().find(|r| r.name == BEDROOM) {
                        tv.choose(&row);
                    }
                }),
            );
            5.0
        }
        _ => 0.0,
    }
}

#[cfg(feature = "fixtures")]
fn tv_unstage(shell: &Rc<Shell>, shot: &str) {
    SILENT_TV.with(|s| s.replace(None));
    // As Done would: the model lets the sheet go, then anything else up closes.
    shell.ctx.tv.sheet_closed();
    close_dialogs(&shell.ctx.window);
    shell.ctx.tv.cancel_pairing();
    shell.ctx.tv.disconnect();
    if let Some(scene) = crate::tv::fixture::scene() {
        scene.silence(false);
    }
    match shot {
        "tv-picker" => {
            let _ = WidgetExt::activate_action(&shell.ctx.window, "win.back", None);
        }
        "tv-player" => shell.player.close(),
        _ => {}
    }
}

/// Casting's scenes: Play on TV for episode 3 from 8:12, with TVs with Chromecast built in on
/// loopback, each set up as its scene needs: the pick, then how long the cast takes to show.
#[cfg(feature = "fixtures")]
fn cast_stage(shell: &Rc<Shell>, shot: &str) -> f64 {
    use crate::tv::fixture::{self, LIVING_ROOM};
    use kaeri_cast::fake::{FakeOptions, Refusal, Refuse};
    const KITCHEN: &str = "Kitchen display";
    let Some(scene) = fixture::scene() else { return 0.0 };
    let tv = Rc::clone(&shell.ctx.tv);
    let later = |ms: u64, f: Box<dyn FnOnce()>| {
        glib::timeout_add_local_once(Duration::from_millis(ms), f);
    };
    // What the episodes stream is a test pattern the fake plays as though it had read it.
    let plays = |control: &'static str| FakeOptions {
        plays_unfetched: true,
        duration: 1440.0,
        control,
        ..FakeOptions::default()
    };
    scene.apart();
    scene.cast_clear();
    // Play on TV for episode 3, from 8:12, once the window has settled.
    let open = {
        let window = shell.ctx.window.clone();
        move || {
            let window = window.clone();
            later(
                600,
                Box::new(move || {
                    let _ = WidgetExt::activate_action(
                        &window,
                        "win.play-on-tv",
                        Some(&(TITLE, "3".to_owned(), 492.0).to_variant()),
                    );
                }),
            );
        }
    };
    // Choose the cast row once it is listed, asking again until it is.
    fn choose(tv: Rc<crate::tv::Tv>, name: &'static str, tries: u32) {
        if let Some(row) = tv.cast_rows().into_iter().find(|r| r.name == name) {
            return tv.choose_cast(&row);
        }
        if tries > 0 {
            glib::timeout_add_local_once(Duration::from_millis(500), move || {
                choose(tv, name, tries - 1);
            });
        }
    }
    let pick = |name: &'static str, after: u64| {
        let tv = Rc::clone(&tv);
        later(after, Box::new(move || choose(tv, name, 10)));
    };
    // The board lists only Bedroom on the network; the Living room there is the Cast one.
    scene.switch_off(LIVING_ROOM);
    shell.go(Dest::Home);
    match shot {
        "cast-picker" => {
            scene.cast(LIVING_ROOM, plays("fixed"));
            scene.cast(KITCHEN, plays("attenuation"));
            open();
            2.5
        }
        "cast-none-found" => {
            scene.silence(true);
            open();
            11.5
        }
        // The TV takes about 7 s to open the receiver: the line before and after its 4 s.
        "cast-starting" | "cast-counting" => {
            scene.cast(
                LIVING_ROOM,
                FakeOptions { launch: Duration::from_secs(7), ..plays("fixed") },
            );
            open();
            pick(LIVING_ROOM, 1800);
            if shot == "cast-starting" { 2.7 } else { 7.2 }
        }
        "cast-playing-fixed" => {
            scene.cast(LIVING_ROOM, plays("fixed"));
            open();
            pick(LIVING_ROOM, 1800);
            11.0
        }
        "cast-playing-level" => {
            scene.cast(KITCHEN, plays("attenuation"));
            open();
            pick(KITCHEN, 1800);
            11.0
        }
        "cast-refused" => {
            scene.cast(
                LIVING_ROOM,
                FakeOptions {
                    refuse: Refuse {
                        content_types: vec![
                            "application/x-mpegurl".into(),
                            "video/mp4".into(),
                            "video/x-matroska".into(),
                            "video/webm".into(),
                        ],
                        bytes: vec![
                            b"ftyp".to_vec(),
                            b"moof".to_vec(),
                            vec![0x1a, 0x45, 0xdf, 0xa3],
                        ],
                        how: Some(Refusal::LoadFailed),
                        ..Refuse::default()
                    },
                    ..plays("fixed")
                },
            );
            open();
            pick(LIVING_ROOM, 1800);
            12.0
        }
        "cast-never-reads" => {
            scene.cast(
                LIVING_ROOM,
                FakeOptions { fetches: false, plays_unfetched: false, ..plays("fixed") },
            );
            open();
            pick(LIVING_ROOM, 1800);
            // The engine gives a TV a minute to start reading before it says why it didn't.
            70.0
        }
        "cast-lost" => {
            scene.cast(LIVING_ROOM, plays("fixed"));
            open();
            pick(LIVING_ROOM, 1800);
            later(9000, Box::new(move || scene.cast_go_silent(LIVING_ROOM)));
            27.0
        }
        "cast-taken-over" => {
            scene.cast(LIVING_ROOM, plays("fixed"));
            open();
            pick(LIVING_ROOM, 1800);
            later(9000, Box::new(move || scene.cast_take_over(LIVING_ROOM)));
            11.0
        }
        "cast-bar" => {
            scene.cast(LIVING_ROOM, plays("fixed"));
            open();
            pick(LIVING_ROOM, 1800);
            let window = shell.ctx.window.clone();
            later(9500, Box::new(move || close_dialogs(&window)));
            11.0
        }
        "cast-stopped" => {
            scene.cast(LIVING_ROOM, plays("fixed"));
            open();
            pick(LIVING_ROOM, 1800);
            let tv = Rc::clone(&tv);
            later(9000, Box::new(move || tv.cast_stop()));
            10.5
        }
        // Near the end, so Up next counts down to the next episode.
        "cast-up-next" => {
            scene.cast(LIVING_ROOM, plays("fixed"));
            open();
            pick(LIVING_ROOM, 1800);
            let tv = Rc::clone(&tv);
            later(
                9000,
                Box::new(move || {
                    tv.cast_send(
                        kaeri_engine::remote::protocol::command::RemoteCommand::SeekTo {
                            position: 1440.0 - 26.0,
                        },
                    )
                }),
            );
            15.0
        }
        _ => 0.0,
    }
}

#[cfg(feature = "fixtures")]
fn cast_unstage(shell: &Rc<Shell>) {
    let tv = &shell.ctx.tv;
    tv.cast_stop();
    tv.sheet_closed();
    close_dialogs(&shell.ctx.window);
    if let Some(scene) = crate::tv::fixture::scene() {
        scene.cast_clear();
        scene.silence(false);
    }
}

#[cfg(not(feature = "fixtures"))]
fn cast_stage(_: &Rc<Shell>, _: &str) -> f64 {
    0.0
}

#[cfg(not(feature = "fixtures"))]
fn cast_unstage(_: &Rc<Shell>) {}

#[cfg(not(feature = "fixtures"))]
fn tv_stage(_: &Rc<Shell>, _: &str) -> f64 {
    0.0
}

#[cfg(not(feature = "fixtures"))]
fn tv_unstage(_: &Rc<Shell>, _: &str) {}

/// Settings' Language row, on the page on screen.
fn language_row(widget: &gtk::Widget) -> Option<adw::ComboRow> {
    if let Some(row) = widget.downcast_ref::<adw::ComboRow>()
        && row.title() == kaeri_i18n::t::settings_language()
    {
        return Some(row.clone());
    }
    let mut child = widget.first_child();
    while let Some(next) = child {
        if let Some(found) = language_row(&next) {
            return Some(found);
        }
        child = next.next_sibling();
    }
    None
}

/// The title page's More button, on the page on screen.
fn more_button(window: &adw::ApplicationWindow) -> Option<gtk::MenuButton> {
    fn find(widget: &gtk::Widget) -> Option<gtk::MenuButton> {
        if let Some(button) = widget.downcast_ref::<gtk::MenuButton>()
            && button.tooltip_text().as_deref() == Some("More")
            && button.is_mapped()
        {
            return Some(button.clone());
        }
        let mut child = widget.first_child();
        while let Some(next) = child {
            if let Some(found) = find(&next) {
                return Some(found);
            }
            child = next.next_sibling();
        }
        None
    }
    find(window.upcast_ref())
}

/// The title page's range picker, while it is on screen.
fn range_button(window: &adw::ApplicationWindow) -> Option<gtk::MenuButton> {
    fn find(widget: &gtk::Widget, tip: &str) -> Option<gtk::MenuButton> {
        if let Some(button) = widget.downcast_ref::<gtk::MenuButton>()
            && button.tooltip_text().as_deref() == Some(tip)
            && button.is_mapped()
        {
            return Some(button.clone());
        }
        let mut child = widget.first_child();
        while let Some(next) = child {
            if let Some(found) = find(&next, tip) {
                return Some(found);
            }
            child = next.next_sibling();
        }
        None
    }
    find(window.upcast_ref(), &kaeri_i18n::t::episode_range_label())
}

/// The Trending rail's period menu, under its heading, over the window.
#[cfg(feature = "fixtures")]
fn period_popover(window: &adw::ApplicationWindow) -> Option<(gtk::Widget, f32, f32)> {
    fn find(widget: &gtk::Widget) -> Option<gtk::MenuButton> {
        if let Some(button) = widget.downcast_ref::<gtk::MenuButton>()
            && button.widget_name() == "trend-period"
            && button.is_mapped()
        {
            return Some(button.clone());
        }
        let mut child = widget.first_child();
        while let Some(next) = child {
            if let Some(found) = find(&next) {
                return Some(found);
            }
            child = next.next_sibling();
        }
        None
    }
    let button = find(window.upcast_ref())?;
    let popover = button.popover()?;
    if !popover.is_visible() {
        return None;
    }
    let bounds = button.compute_bounds(window)?;
    let width = popover.width() as f32;
    let x = (bounds.x() - 4.0).clamp(0.0, (window.width() as f32 - width).max(0.0));
    Some((popover.upcast(), x, bounds.y() + bounds.height()))
}

/// Whichever menu button has its popover open, the popover under it, over the window.
#[cfg(feature = "fixtures")]
fn open_popover(window: &adw::ApplicationWindow) -> Option<(gtk::Widget, f32, f32)> {
    fn find(widget: &gtk::Widget) -> Option<gtk::MenuButton> {
        if let Some(button) = widget.downcast_ref::<gtk::MenuButton>()
            && button.is_mapped()
            && button.popover().is_some_and(|p| p.is_visible())
        {
            return Some(button.clone());
        }
        let mut child = widget.first_child();
        while let Some(next) = child {
            if let Some(found) = find(&next) {
                return Some(found);
            }
            child = next.next_sibling();
        }
        None
    }
    let button = find(window.upcast_ref())?;
    let popover = button.popover()?;
    let bounds = button.compute_bounds(window)?;
    let width = popover.width() as f32;
    let x = (bounds.x() + bounds.width() / 2.0 - width / 2.0)
        .clamp(0.0, (window.width() as f32 - width).max(0.0));
    // Under the button, or over it for one that opens upwards, as the reader's do: over the
    // place the popover points at, when it points somewhere of its own.
    let y = if button.direction() == gtk::ArrowType::Up {
        let (aimed, at) = popover.pointing_to();
        let top = if aimed { at.y() as f32 } else { 0.0 };
        bounds.y() + top - popover.height() as f32
    } else {
        bounds.y() + bounds.height()
    };
    Some((popover.upcast(), x, y.max(0.0)))
}

/// The first text field under `widget`.
#[cfg(feature = "fixtures")]
fn find_entry(widget: &gtk::Widget) -> Option<gtk::Entry> {
    if let Some(entry) = widget.downcast_ref::<gtk::Entry>() {
        return Some(entry.clone());
    }
    let mut child = widget.first_child();
    while let Some(next) = child {
        if let Some(found) = find_entry(&next) {
            return Some(found);
        }
        child = next.next_sibling();
    }
    None
}

/// The range picker's popover, under its button, over the window.
fn range_popover(window: &adw::ApplicationWindow) -> Option<(gtk::Widget, f32, f32)> {
    let button = range_button(window)?;
    let popover = button.popover()?;
    if !popover.is_visible() {
        return None;
    }
    let bounds = button.compute_bounds(window)?;
    let width = popover.width() as f32;
    let x = (bounds.x() + bounds.width() / 2.0 - width / 2.0)
        .clamp(0.0, (window.width() as f32 - width).max(0.0));
    Some((popover.upcast(), x, bounds.y() + bounds.height()))
}

/// The More menu's popover, under its button, over the window.
fn title_menu_popover(window: &adw::ApplicationWindow) -> Option<(gtk::Widget, f32, f32)> {
    let button = more_button(window)?;
    let popover = button.popover()?;
    if !popover.is_visible() {
        return None;
    }
    let bounds = button.compute_bounds(window)?;
    let width = popover.width() as f32;
    let x = (bounds.x() + bounds.width() / 2.0 - width / 2.0)
        .clamp(0.0, (window.width() as f32 - width).max(0.0));
    Some((popover.upcast(), x, bounds.y() + bounds.height()))
}

/// What the sources have of the title the source picker's shots open: all of it, part of it,
/// failed, off, and a torrent source with a batch and singles. `pinned` is the source pinned, and
/// `release` the one of its releases.
#[cfg(feature = "fixtures")]
fn source_options(
    pinned: Option<&str>,
    release: Option<&str>,
) -> Vec<kaeri_model::sources::SourceOption> {
    use kaeri_core::traits::SourceCandidate;
    use kaeri_model::sources::{SourceNote, SourceOption};
    let candidate =
        |id: &str, title: &str, quality, seeders: u32, size: &str| SourceCandidate {
            id: id.into(),
            provider_id: "torrent".into(),
            title: title.into(),
            quality: Some(quality),
            label: Some(format!("{seeders} seeders")),
            size: Some(size.into()),
            dual_audio: false,
            dubbed: false,
            auto_pick: false,
        };
    let option = |id: &str, name: &str, covered, note, releases| SourceOption {
        provider_id: id.into(),
        name: name.into(),
        covered,
        of: 13,
        pinned: pinned == Some(id),
        note,
        releases,
        pinned_release: release.filter(|_| pinned == Some(id)).map(str::to_owned),
    };
    vec![
        option("hotaru-open-archive", "Hotaru Open Archive", 13, None, Vec::new()),
        option("mukashi-archive", "Mukashi Archive", 9, None, Vec::new()),
        option(
            "torrent",
            "Torrents",
            6,
            None,
            vec![
                candidate(
                    "batch-1080",
                    "Hoshikuzu no Tabibito (Batch) 1080p",
                    1080,
                    84,
                    "14.2 GiB",
                ),
                candidate(
                    "batch-720",
                    "Hoshikuzu no Tabibito (Batch) 720p",
                    720,
                    31,
                    "6.8 GiB",
                ),
                candidate(
                    "single-1080",
                    "Hoshikuzu no Tabibito - 01 1080p",
                    1080,
                    12,
                    "1.1 GiB",
                ),
            ],
        ),
        option(
            "machi-library-films",
            "Machi Library Films",
            0,
            Some(SourceNote::Failed { reason: "the source did not answer in 15 s.".into() }),
            Vec::new(),
        ),
        option(
            "sora-festival",
            "Sora Animation Festival",
            0,
            Some(SourceNote::Off),
            Vec::new(),
        ),
    ]
}

/// The source picker's scenes: the title's page, then the picker over it.
#[cfg(feature = "fixtures")]
fn source_stage(shell: &Rc<Shell>, shot: &str) -> f64 {
    use kaeri_core::ids::AnilistId;
    use kaeri_model::event::Event;

    let hub = Rc::clone(&shell.ctx.hub);
    shell.ctx.notices.dismiss_each();
    let unsourced = shot.starts_with("source-unsourced");
    let title = if unsourced { PLAYING.0 } else { TITLE };
    hub.known.borrow_mut().source_options.clear();
    let listing = match shot {
        "source-picker-pinned" => Some((source_options(Some("mukashi-archive"), None), false)),
        "source-picker-releases" => {
            Some((source_options(Some("torrent"), Some("batch-720")), false))
        }
        "source-picker-loading" => Some((Vec::new(), true)),
        _ => Some((source_options(None, None), false)),
    };
    hub.listing.replace(listing);
    if unsourced {
        // The next episode, or it and every one after, is listed by no source.
        hub.unsourced.set(Some((title, 4, if shot == "source-unsourced" { 4 } else { 99 })));
    }
    shell.go(Dest::Home);
    let _ = WidgetExt::activate_action(
        &shell.ctx.window,
        "win.open-title",
        Some(&title.to_variant()),
    );
    let ctx = Rc::clone(&shell.ctx);
    let shot = shot.to_owned();
    glib::timeout_add_local_once(Duration::from_millis(2500), move || match shot.as_str() {
        "first-play-subtitles" => {
            // Down to the Subtitles group, which sits below Watching, Player and Picture.
            if let Some(page) = first::<adw::PreferencesPage>(ctx.window.upcast_ref())
                && let Some(scroll) = first::<gtk::ScrolledWindow>(page.upcast_ref())
            {
                scroll.vadjustment().set_value(scroll.vadjustment().upper());
            }
        }
        "source-pin-unavailable" => ctx.hub.take(vec![Event::PinUnavailable {
            id: AnilistId::new(title),
            provider_id: "mukashi-archive".into(),
            name: "Mukashi Archive".into(),
        }]),
        "source-unsourced" | "source-unsourced-none" => {
            // The page scrolled to the dimmed episode.
            if let Some(scroll) = ctx
                .nav()
                .and_then(|nav| nav.visible_page())
                .and_then(|page| first::<gtk::ScrolledWindow>(page.upcast_ref()))
            {
                scroll.vadjustment().set_value(330.0);
            }
        }
        _ => {
            if let Some(page) = ctx.nav().and_then(|nav| nav.visible_page()) {
                let _ = WidgetExt::activate_action(&page, "title.source-picker", None);
            }
        }
    });
    5.0
}

/// Leave the source picker's scene: the picker closed, the title's page left, the engine's own
/// answers back.
#[cfg(feature = "fixtures")]
fn source_unstage(shell: &Rc<Shell>) {
    close_dialogs(&shell.ctx.window);
    shell.ctx.hub.listing.replace(None);
    shell.ctx.hub.unsourced.set(None);
    shell.ctx.notices.dismiss_each();
    let _ = WidgetExt::activate_action(&shell.ctx.window, "win.back", None);
}

/// The match picker's scenes, on the unsure scene: the title's page asking for its match, then
/// the picker over it in each of its states.
#[cfg(feature = "fixtures")]
fn match_stage(shell: &Rc<Shell>, shot: &str) -> f64 {
    use crate::hub::MatchHold;

    let hub = Rc::clone(&shell.ctx.hub);
    shell.ctx.notices.dismiss_each();
    hub.known.borrow_mut().source_options.clear();
    hub.listing.replace(Some((source_options(None, None), false)));
    hub.match_hold.set(match shot {
        "match-menu" => MatchHold::SearchWaits,
        "match-failed" => MatchHold::SearchFails,
        "match-picking" => MatchHold::PickWaits,
        _ => MatchHold::Nothing,
    });
    shell.go(Dest::Home);
    // The last scene's page is still leaving; opening the title again waits for it to go.
    let (ctx, name) = (Rc::clone(&shell.ctx), shot.to_owned());
    glib::timeout_add_local_once(Duration::from_millis(900), move || {
        let _ = WidgetExt::activate_action(
            &ctx.window,
            "win.open-title",
            Some(&TITLE.to_variant()),
        );
        match_wait(Rc::clone(&ctx), name, 0);
    });
    match shot {
        "match-cleared" => 13.0,
        "match-picked" => 9.0,
        _ => 7.0,
    }
}

/// Wait until the engine has said what the title is matched to, or 20 s, then stage `shot`.
#[cfg(feature = "fixtures")]
fn match_wait(ctx: Rc<crate::window::Ctx>, shot: String, tries: u32) {
    use kaeri_core::ids::AnilistId;
    let id = AnilistId::new(TITLE);
    // At least 2.5 s, so the page has opened under the picker.
    let pushed = ctx.nav().is_some_and(|nav| nav.navigation_stack().n_items() > 1);
    if (!ctx.hub.known.borrow().matches.is_known(id) || !pushed || tries < 10) && tries < 80 {
        glib::timeout_add_local_once(Duration::from_millis(250), move || {
            match_wait(ctx, shot, tries + 1)
        });
        return;
    }
    match_run(&ctx, &shot);
}

/// A match in place from the source, as the menu's states show the picker.
#[cfg(feature = "fixtures")]
fn match_in_place(ctx: &Rc<crate::window::Ctx>, by_you: bool) {
    use kaeri_core::ids::AnilistId;
    use kaeri_model::{event::Event, sources::CurrentMatch};
    ctx.hub.take(vec![Event::TitleMatch {
        id: AnilistId::new(TITLE),
        current: Some(CurrentMatch {
            provider_id: crate::fixtures::UNSURE_SOURCE.into(),
            source: "Hotaru Open Archive".into(),
            title: Some("Hoshi Tabi Diaries".into()),
            chosen_by_you: by_you,
        }),
    }]);
}

#[cfg(feature = "fixtures")]
fn match_open(
    ctx: &Rc<crate::window::Ctx>,
    from_menu: bool,
    query: Option<&str>,
) -> adw::Dialog {
    use kaeri_core::ids::AnilistId;
    crate::pages::match_picker::open(ctx, &ctx.window, AnilistId::new(TITLE), from_menu, query)
}

/// Pick the first row of the picker `dialog` shows.
#[cfg(feature = "fixtures")]
fn match_pick_first(dialog: &adw::Dialog) {
    if let Some(row) = first::<adw::ActionRow>(dialog.upcast_ref()) {
        ActionRowExt::activate(&row);
    }
}

#[cfg(feature = "fixtures")]
fn match_run(ctx: &Rc<crate::window::Ctx>, shot: &str) {
    use kaeri_i18n::t;
    match shot {
        "match" => {
            match_open(ctx, false, None);
        }
        "match-picking" => {
            // The best row picked, with no answer to come.
            let dialog = match_open(ctx, false, None);
            glib::timeout_add_local_once(Duration::from_millis(600), move || {
                match_pick_first(&dialog)
            });
        }
        "match-source-picker" => {
            if let Some(page) = ctx.nav().and_then(|nav| nav.visible_page()) {
                let _ = WidgetExt::activate_action(&page, "title.source-picker", None);
            }
        }
        "match-fix-menu" => {
            if let Some(more) = more_button(&ctx.window) {
                more.popup();
            }
        }
        "match-menu" | "match-failed" => {
            match_in_place(ctx, false);
            match_open(ctx, true, None);
        }
        "match-empty" => {
            match_in_place(ctx, false);
            match_open(ctx, true, Some("hoshikuzu tabibto"));
        }
        "match-results" => {
            match_in_place(ctx, true);
            match_open(ctx, true, Some("Stardust Wayfarer"));
        }
        "match-picked" | "match-cleared" => {
            let dialog = match_open(ctx, true, Some("Stardust Wayfarer"));
            let (ctx, cleared) = (Rc::clone(ctx), shot == "match-cleared");
            glib::timeout_add_local_once(Duration::from_millis(1500), move || {
                match_pick_first(&dialog);
                if !cleared {
                    return;
                }
                // Reopened over the page as a person's choice stands, to forget it.
                glib::timeout_add_local_once(Duration::from_millis(2500), move || {
                    match_open(&ctx, true, None);
                    glib::timeout_add_local_once(Duration::from_millis(1200), move || {
                        let label = t::match_automatic();
                        if let Some(dialog) = ctx.window.visible_dialog()
                            && let Some(button) = button_labelled(dialog.upcast_ref(), &label)
                        {
                            button.emit_clicked();
                        }
                    });
                });
            });
        }
        _ => {}
    }
}

/// Leave the match picker's scene: the dialogs closed, the title's page left, and the engine
/// asked to match the title again so the next scene starts with its question.
#[cfg(feature = "fixtures")]
fn match_unstage(shell: &Rc<Shell>) {
    use crate::hub::MatchHold;
    use kaeri_core::ids::AnilistId;
    let id = AnilistId::new(TITLE);
    close_dialogs(&shell.ctx.window);
    shell.ctx.hub.match_hold.set(MatchHold::Nothing);
    shell.ctx.hub.listing.replace(None);
    shell.ctx.notices.dismiss_each();
    shell.ctx.hub.known.borrow_mut().matches.cleared(id);
    shell.ctx.hub.send(kaeri_model::command::Command::ClearMatch { id });
    let _ = WidgetExt::activate_action(&shell.ctx.window, "win.back", None);
}

#[cfg(not(feature = "fixtures"))]
fn match_stage(_: &Rc<Shell>, _: &str) -> f64 {
    0.0
}

#[cfg(not(feature = "fixtures"))]
fn match_unstage(_: &Rc<Shell>) {}

/// Stand the lawful sources showcase up as `flow` and `case` answer, with `bare` for nothing
/// added yet.
#[cfg(feature = "fixtures")]
fn lawful_begin(
    shell: &Rc<Shell>,
    flow: crate::showcase::Flow,
    case: crate::showcase::Case,
    bare: bool,
) {
    crate::showcase::lawful(flow, case, bare);
    shell.ctx.hub.show_instead(crate::showcase::lawful_events());
    shell.go(Dest::Providers);
}

/// Every text row under `widget`, in tree order: the add-a-server dialog's address, user and
/// password.
#[cfg(feature = "fixtures")]
fn entry_rows(widget: &gtk::Widget, out: &mut Vec<adw::EntryRow>) {
    if let Some(row) = widget.downcast_ref::<adw::EntryRow>() {
        out.push(row.clone());
    }
    let mut child = widget.first_child();
    while let Some(c) = child {
        entry_rows(&c, out);
        child = c.next_sibling();
    }
}

/// Type the showcase server's address and press Return in it.
#[cfg(feature = "fixtures")]
fn type_address(window: &adw::ApplicationWindow) {
    let Some(dialog) = window.visible_dialog() else { return };
    let mut rows = Vec::new();
    entry_rows(dialog.upcast_ref(), &mut rows);
    if let Some(address) = rows.first() {
        address.set_text("https://jelly.example:8920");
        address.emit_by_name::<()>("entry-activated", &[]);
    }
}

/// Type the showcase user and a password, and press Return in the password.
#[cfg(feature = "fixtures")]
fn type_sign_in(window: &adw::ApplicationWindow, password: &str) {
    let Some(dialog) = window.visible_dialog() else { return };
    let mut rows = Vec::new();
    entry_rows(dialog.upcast_ref(), &mut rows);
    if let [_, user, pass, ..] = rows.as_slice() {
        user.set_text("mika");
        pass.set_text(password);
        pass.emit_by_name::<()>("entry-activated", &[]);
    }
}

/// The lawful sources shots.
#[cfg(feature = "fixtures")]
fn lawful_stage(shell: &Rc<Shell>, shot: &str) -> f64 {
    use crate::{
        pages::{folders, licensed, servers},
        showcase::{Case, Flow},
    };
    let ctx = &shell.ctx;
    let (flow, case) = match shot {
        "lawful-server-unreachable" => (Flow::Unreachable, Case::PlaysAlsoOn),
        "lawful-server-credentials" => (Flow::Credentials, Case::PlaysAlsoOn),
        "lawful-server-certificate" => (Flow::Certificate, Case::PlaysAlsoOn),
        "lawful-title-on-service" => (Flow::Usual, Case::OnService),
        "lawful-title-nowhere" => (Flow::Usual, Case::NotInSources),
        _ => (Flow::Usual, Case::PlaysAlsoOn),
    };
    let bare = matches!(
        shot,
        "lawful-sources-bare"
            | "lawful-demo-offer"
            | "lawful-server-find"
            | "lawful-server-sign-in"
            | "lawful-server-unreachable"
            | "lawful-server-credentials"
            | "lawful-server-certificate"
            | "lawful-server-libraries"
    );
    lawful_begin(shell, flow, case, bare);
    let later = |ms: u64, f: Box<dyn Fn(&adw::ApplicationWindow)>| {
        let window = ctx.window.clone();
        glib::timeout_add_local_once(Duration::from_millis(ms), move || f(&window));
    };
    match shot {
        "lawful-sources-bare" | "lawful-sources" => {}
        "lawful-demo-offer" => {
            use kaeri_model::sources::DemoState;
            ctx.hub.show_instead(vec![
                Event::Providers(Vec::new()),
                Event::Demo(DemoState { offer: true, ..DemoState::default() }),
            ]);
        }
        "lawful-demo-row" => {
            use kaeri_model::sources::DemoState;
            ctx.hub.show_instead(vec![
                Event::Providers(crate::showcase::providers_with_demo()),
                Event::Demo(DemoState { on: true, ..DemoState::default() }),
            ]);
        }
        "lawful-server-find" => servers::open_add(ctx, None),
        "lawful-sources-matching" => {
            crate::showcase::lawful_matching();
            ctx.hub.send(Command::Local(kaeri_model::local::LocalCommand::Load));
            ctx.hub.send(Command::Server(kaeri_model::servers::ServerCommand::Load));
        }
        "lawful-server-sign-in" | "lawful-server-unreachable" | "lawful-server-certificate" => {
            servers::open_add(ctx, Some("https://jelly.example:8920"));
            later(800, Box::new(type_address));
        }
        "lawful-server-credentials" | "lawful-server-libraries" => {
            servers::open_add(ctx, Some("https://jelly.example:8920"));
            let bad = shot == "lawful-server-credentials";
            later(800, Box::new(type_address));
            later(
                2200,
                Box::new(move |window| {
                    type_sign_in(window, if bad { "wrong" } else { "lantern-tea" });
                }),
            );
        }
        "lawful-server" => servers::push(ctx, "mikas-jellyfin"),
        "lawful-folder" => folders::push(ctx, "/media/shows"),
        "lawful-folder-drive" => folders::push(ctx, "/run/media/mika/travel-drive/anime"),
        "lawful-folder-fix" => {
            folders::push(ctx, "/media/shows");
            let ctx = Rc::clone(ctx);
            later(
                800,
                Box::new(move |_| {
                    let group = ctx.hub.known.borrow().local.as_ref().and_then(|view| {
                        view.groups.iter().find(|g| g.name.starts_with("harbour")).cloned()
                    });
                    if let Some(group) = group {
                        folders::fix_match(&ctx, &group);
                    }
                }),
            );
            later(
                2200,
                Box::new(|window| {
                    if let Some(dialog) = window.visible_dialog()
                        && let Some(field) = first::<gtk::SearchEntry>(dialog.upcast_ref())
                    {
                        field.set_text("stardust");
                    }
                }),
            );
        }
        "lawful-folder-set-episode" => {
            folders::push(ctx, "/media/shows");
            later(
                900,
                Box::new(|window| {
                    if let Some(button) = button_labelled(
                        window.upcast_ref(),
                        &kaeri_i18n::t::library_set_episode(),
                    ) {
                        button.emit_clicked();
                    }
                }),
            );
        }
        "lawful-licensed" => licensed::push(ctx),
        "lawful-title-also-on" | "lawful-title-on-service" | "lawful-title-nowhere" => {
            shell.go(Dest::Home);
            let _ = WidgetExt::activate_action(
                &ctx.window,
                "win.open-title",
                Some(&TITLE.to_variant()),
            );
            return 3.5;
        }
        "lawful-return" => {
            ctx.hub.send(Command::Licensed(kaeri_model::command::LicensedCommand::Returned));
        }
        "lawful-welcome" => {
            crate::pages::first_launch::open(ctx);
            later(
                600,
                Box::new(|window| {
                    if let Some(dialog) = window.visible_dialog()
                        && let Some(pages) = first::<adw::NavigationView>(dialog.upcast_ref())
                    {
                        pages.push_by_tag("sources");
                    }
                }),
            );
        }
        _ => {}
    }
    3.0
}

#[cfg(not(feature = "fixtures"))]
fn source_stage(_: &Rc<Shell>, _: &str) -> f64 {
    0.0
}

#[cfg(not(feature = "fixtures"))]
fn source_unstage(_: &Rc<Shell>) {}

/// The skip times shots: the marking bar at each step, the prompts, the sharing question and the
/// Skip times list, over the paused clip.
#[cfg(feature = "fixtures")]
fn skip_stage(shell: &Rc<Shell>, name: &str) -> f64 {
    use kaeri_model::playback::{SkipKind, SkipMarkFixture as F, SkipPrompt};
    shell.player.shot_reset();
    play(shell);
    let player = Rc::clone(&shell.player);
    let name = name.to_owned();
    glib::timeout_add_local_once(Duration::from_millis(8000), move || match name.as_str() {
        "skip-start" => player.shot_marking(F::Start),
        "skip-end" => player.shot_marking(F::End),
        "skip-review" => player.shot_marking(F::Review),
        "skip-too-short" => player.shot_marking(F::TooShort),
        "skip-too-long" => player.shot_marking(F::TooLong),
        "skip-replaces" => player.shot_marking(F::Replaces),
        "skip-shares" => player.shot_marking(F::Shares),
        "skip-no-entry" => player.shot_marking(F::NoEntry),
        "skip-editing" => player.shot_marking(F::Editing),
        "skip-was-opening" => {
            player.shot_prompt(SkipPrompt::WasOpening { start: 10.0, end: 34.0 })
        }
        "skip-wrong" => player.shot_prompt(SkipPrompt::WrongTimes { kind: SkipKind::Opening }),
        "skip-share" => player.shot_prompt(SkipPrompt::Share),
        "skip-times" => player.shot_skip_times(),
        _ => {}
    });
    11.0
}

/// The found skip times shots: the player or Settings in one of the engine's fixture states.
#[cfg(feature = "fixtures")]
fn found_stage(shell: &Rc<Shell>, name: &str) -> f64 {
    use kaeri_model::playback::{FoundSkipFixture as F, found_skip_scene};
    let state = match name {
        "found-skip" => F::Skip,
        "found-controls" => F::Controls,
        "found-prompt" => F::Prompt,
        "found-saved" => F::Saved,
        "found-never" => F::Never,
        "found-review" => F::Review,
        "found-nudged" => F::Nudged,
        "found-undo" => F::Undo,
        "found-panel" => F::Panel,
        "found-panel-paused" => F::PanelPaused,
        "found-set-on" => F::SettingsOn,
        "found-set-low-power" => F::SettingsLowPower,
        _ => F::SettingsStorage,
    };
    let scene = found_skip_scene(state);
    if scene.settings {
        // The engine says why finding is paused with its Settings: say it as the fixture does.
        if let Some(view) = shell.ctx.hub.known.borrow_mut().settings.as_mut() {
            view.skip_find_paused = scene.paused;
        }
        shell.player.shot_reset();
        shell.player.close();
        shell.settings_pane("playback");
        // The page builds its rows from the engine's Settings: send them as they now stand.
        let view = shell.ctx.hub.known.borrow().settings.clone();
        if let Some(view) = view {
            shell.ctx.hub.take(vec![kaeri_model::event::Event::Settings(view)]);
        }
        let window = shell.ctx.window.clone();
        glib::timeout_add_local_once(Duration::from_millis(900), move || {
            reveal_row(window.upcast_ref(), &kaeri_i18n::t::settings_find_skips());
        });
        return 2.8;
    }
    shell.player.shot_reset();
    play(shell);
    let player = Rc::clone(&shell.player);
    glib::timeout_add_local_once(Duration::from_millis(8000), move || player.shot_found(scene));
    11.0
}

/// The found skip times' motion, for a recording of the screen: the card comes in over the video
/// and answers by crossfading in place (`yes`, `never`), Adjust closes it for the review
/// (`adjust`), and Settings' paused line opens and closes (`settings`).
#[cfg(feature = "fixtures")]
fn found_motion(shell: &Rc<Shell>, name: &str) -> f64 {
    use kaeri_model::playback::{
        FoundSkipFixture as F, SkipFindPause, SkipKind, SkipPrompt, found_skip_scene,
    };
    let at = |millis: u64, step: Box<dyn Fn() + 'static>| {
        glib::timeout_add_local_once(Duration::from_millis(millis), step);
    };
    if name == "motion-found-settings" {
        shell.settings_pane("playback");
        let (hub, window) = (Rc::clone(&shell.ctx.hub), shell.ctx.window.clone());
        // The row in the middle of the page, again after each time the engine's Settings
        // rebuild the page.
        for millis in [600, 1800, 4800] {
            at(
                millis,
                Box::new({
                    let window = window.clone();
                    move || {
                        reveal_row(window.upcast_ref(), &kaeri_i18n::t::settings_find_skips())
                    }
                }),
            );
        }
        for (millis, pause) in [(1500, Some(SkipFindPause::LowPower)), (4500, None)] {
            let hub = Rc::clone(&hub);
            at(
                millis,
                Box::new(move || {
                    let mut view = hub.known.borrow().settings.clone();
                    if let Some(view) = view.as_mut() {
                        view.skip_find_paused = pause;
                    }
                    if let Some(view) = view {
                        hub.take(vec![kaeri_model::event::Event::Settings(view)]);
                    }
                }),
            );
        }
        return 7.0;
    }
    shell.player.shot_reset();
    play(shell);
    let player = Rc::clone(&shell.player);
    // The player takes a few seconds to come up: the script starts after them, as found_stage's does.
    const START: u64 = 14000;
    let (kind, episode) = (SkipKind::Opening, 3);
    // The card lands 3 s after a skip; the recording starts as the clip does.
    at(START + 1500, {
        let player = Rc::clone(&player);
        Box::new(move || {
            // The Prompt scene: the card's question over the video, no Skip button beside it.
            player.shot_found(found_skip_scene(F::Prompt));
        })
    });
    match name {
        "motion-found-yes" => {
            at(START + 4500, {
                let player = Rc::clone(&player);
                Box::new(move || player.shot_prompt(SkipPrompt::FoundSaved { kind, episode }))
            });
            at(START + 8800, {
                let player = Rc::clone(&player);
                Box::new(move || player.shot_prompt_gone())
            });
        }
        "motion-found-never" => {
            at(START + 4500, {
                let player = Rc::clone(&player);
                Box::new(move || player.shot_prompt(SkipPrompt::FoundNever))
            });
            at(START + 8800, {
                let player = Rc::clone(&player);
                Box::new(move || player.shot_prompt_gone())
            });
        }
        _ => {
            // Adjust: the card goes and the review comes in with the found stretch.
            at(START + 4000, {
                let (player, window) = (Rc::clone(&player), shell.ctx.window.clone());
                Box::new(move || {
                    if let Some(button) = button_labelled(
                        window.upcast_ref(),
                        &kaeri_i18n::t::player_found_adjust(),
                    ) {
                        button.emit_clicked();
                    }
                    player.shot_review(found_skip_scene(F::Review));
                })
            });
        }
    }
    25.0
}

/// Scrolls the row with this title into the middle of its page, as a viewer's own scrolling
/// would: focusing a row alone leaves it at the page's edge.
#[cfg(feature = "fixtures")]
fn reveal_row(window: &gtk::Widget, title: &str) {
    let Some(row) = row_titled(window, title) else { return };
    // Focus brings the row into view first; then the scroller puts it in the page's middle.
    row.grab_focus();
    let mut parent = row.parent();
    while let Some(widget) = parent {
        if let Some(scroller) = widget.downcast_ref::<gtk::ScrolledWindow>() {
            let scroller = scroller.clone();
            glib::timeout_add_local_once(Duration::from_millis(600), move || {
                let Some(viewport) = scroller.child() else { return };
                // Measured against the scrolled content, not the viewport that moves over it.
                let content = viewport
                    .downcast_ref::<gtk::Viewport>()
                    .and_then(gtk::Viewport::child)
                    .unwrap_or(viewport);
                let Some(bounds) = row.compute_bounds(&content) else { return };
                let adjustment = scroller.vadjustment();
                let middle = f64::from(bounds.y()) + f64::from(bounds.height()) / 2.0;
                adjustment.set_value(middle - adjustment.page_size() / 2.0);
            });
            return;
        }
        parent = widget.parent();
    }
}

/// The preferences row on the page on screen with this title.
#[cfg(feature = "fixtures")]
fn row_titled(widget: &gtk::Widget, title: &str) -> Option<adw::PreferencesRow> {
    if let Some(row) = widget.downcast_ref::<adw::PreferencesRow>()
        && row.title() == title
    {
        return Some(row.clone());
    }
    let mut child = widget.first_child();
    while let Some(next) = child {
        if let Some(found) = row_titled(&next, title) {
            return Some(found);
        }
        child = next.next_sibling();
    }
    None
}

#[cfg(not(feature = "fixtures"))]
fn found_stage(_: &Rc<Shell>, _: &str) -> f64 {
    0.0
}

#[cfg(not(feature = "fixtures"))]
fn found_motion(_: &Rc<Shell>, _: &str) -> f64 {
    0.0
}

#[cfg(not(feature = "fixtures"))]
fn skip_stage(_: &Rc<Shell>, _: &str) -> f64 {
    0.0
}

/// The synopses of the title page's list, made up: one line per episode of the finished title.
#[cfg(feature = "fixtures")]
const LINES: [&str; 11] = [
    "A lamplighter wakes on the star road with a lantern and no name.",
    "The crew take on a passenger who pays in maps, and the first map is of a place that has not been built yet.",
    "A storm closes the pass with the mail still aboard. The pilot has to choose between turning back and flying the old route under the cloud line, the one her mentor never came back from.",
    "Ryo repairs the engine by lamplight while the others argue over where the next island lies.",
    "A festival of paper lanterns drifts up from the sea of cloud, and one of them carries a message for the youngest pilot.",
    "The crew wait out a calm at a floating harbour where nobody has docked in years.",
    "A storm closes Kiriya Pass with the mail still aboard. Toma has to choose between turning back and flying the old route under the cloud line.",
    "Ryo finds the letter Toma has carried since the first night, and reads it before she can stop him. Neither of them speaks again until the lamps of Hoshimi come up out of the cloud.",
    "On an island with one lamp left, the crew meet the keeper who lights it every night for a ship that stopped coming. Ryo stays behind to help, and nearly misses the morning departure. The keeper gives Toma a map that was never meant to leave the island, and asks only that she carry it past the edge of the cloud sea.",
    "The crew of a patched-up airship carry letters and strangers between islands that float above an endless sea of cloud.",
    "A clock that runs four minutes slow is the only witness to a theft on Lantern Street.",
];

/// The player's Episodes panel with synopses: the playing title's table is sent again with text
/// and the rows ahead, and the panel opened. With `focus`, the keyboard then moves to the second
/// row, and the footer follows it.
#[cfg(feature = "fixtures")]
fn stage_player_episodes(shell: &Rc<Shell>, shot: &str) {
    let focus = shot == "player-episodes-focus";
    let ranged = shot.starts_with("player-episodes-ranges");
    let menu = shot == "player-episodes-ranges-menu";
    use kaeri_model::episodes::EpisodeRow;
    let hub = Rc::clone(&shell.ctx.hub);
    let seen: Rc<std::cell::RefCell<Vec<EpisodeRow>>> = Rc::default();
    let listening = hub.listen({
        let seen = Rc::clone(&seen);
        move |event| {
            if let Event::Episodes { id, rows, .. } = event
                && id.get() == PLAYING.0
            {
                seen.replace(rows.clone());
            }
        }
    });
    let player = Rc::clone(&shell.player);
    glib::timeout_add_local_once(Duration::from_secs(4), move || {
        hub.forget(listening);
        let base = seen.borrow().clone();
        // A long series for the ranges: 250 rows, 230 playing.
        let mut rows: Vec<EpisodeRow> = if ranged && !base.is_empty() {
            (1..=250)
                .map(|n| {
                    let mut row = base[(n - 1) % base.len()].clone();
                    row.number = n.to_string();
                    row.title = Some(format!("Letter number {n}"));
                    row
                })
                .collect()
        } else {
            base
        };
        let next = if ranged { 230 } else { 4 };
        for (i, row) in rows.iter_mut().enumerate() {
            let n = i + 1;
            row.completed = n < next;
            row.watched = if n == next { 0.42 } else { 0.0 };
            row.past_progress = n > next;
            row.spoiler_ahead = n > next;
            row.description = Some(LINES[i % LINES.len()].to_owned());
        }
        hub.take(vec![Event::Episodes {
            id: kaeri_core::ids::AnilistId::new(PLAYING.0),
            rows,
            failed: None,
        }]);
        glib::timeout_add_local_once(Duration::from_millis(1200), move || {
            if ranged {
                player.shot_ranged_rows();
            }
            player.shot_episodes();
            if menu {
                let player = Rc::clone(&player);
                glib::timeout_add_local_once(Duration::from_millis(900), move || {
                    player.shot_episodes_ranges()
                });
            }
            if focus {
                glib::timeout_add_local_once(Duration::from_millis(800), move || {
                    player.shot_episodes_focus(1)
                });
            }
        });
    });
}

/// The title page with synopses on its episodes, as the fixture scene has none of its own: the
/// table the page got is sent again with text, and which rows are ahead. `synopsis-long` sends a
/// table of 250, and `synopsis-ranges` the same with the range picker open; `synopsis-none` opens a
/// title whose table has no text at all.
#[cfg(feature = "fixtures")]
fn stage_synopsis(shell: &Rc<Shell>, shot: &str) -> f64 {
    use kaeri_model::episodes::EpisodeRow;
    let window = &shell.ctx.window;
    shell.go(Dest::Home);
    let title = if shot == "synopsis-none" { PLAYING.0 } else { TITLE };
    let seen: Rc<std::cell::RefCell<Vec<EpisodeRow>>> = Rc::default();
    let hub = Rc::clone(&shell.ctx.hub);
    let listening = hub.listen({
        let seen = Rc::clone(&seen);
        move |event| {
            if let Event::Episodes { id, rows, .. } = event
                && id.get() == title
            {
                seen.replace(rows.clone());
            }
        }
    });
    let _ = WidgetExt::activate_action(window, "win.open-title", Some(&title.to_variant()));
    if shot == "synopsis-none" {
        return 3.5;
    }
    let wait = if shot.starts_with("motion-") { 8.0 } else { 6.0 };
    let ctx = Rc::clone(&shell.ctx);
    let shot = shot.to_owned();
    glib::timeout_add_local_once(Duration::from_millis(2500), move || {
        hub.forget(listening);
        let base = seen.borrow().clone();
        let next = 7;
        let long =
            !matches!(shot.as_str(), "synopsis" | "synopsis-revealed" | "synopsis-shown");
        let mut rows: Vec<EpisodeRow> = if long {
            (1..=250)
                .map(|n| {
                    let mut row = base[(n - 1) % base.len()].clone();
                    row.number = n.to_string();
                    row.title = Some(format!("Letter number {n}"));
                    row
                })
                .collect()
        } else {
            base
        };
        let next = if long { 230 } else { next };
        for (i, row) in rows.iter_mut().enumerate() {
            let n = i + 1;
            row.completed = n < next;
            row.watched = if n == next { 0.42 } else { 0.0 };
            row.past_progress = n > next;
            row.spoiler_ahead = n > next;
            row.description = (n != 10 || long).then(|| LINES[i % LINES.len()].to_owned());
            row.kind = (n == 10).then_some(kaeri_model::episodes::EpisodeKind::Filler);
            // The not-out shot: the last ten have no date, after 240 that came out.
            if shot == "go-dialog-not-out" {
                row.airs_at = (n <= 240).then_some(1_000_000_000 + n as i64 * 604_800);
            }
        }
        hub.take(vec![Event::Episodes {
            id: kaeri_core::ids::AnilistId::new(TITLE),
            rows,
            failed: None,
        }]);
        let for_press = Rc::clone(&ctx);
        let press = move |label: String| {
            if let Some(button) = button_labelled(for_press.window.upcast_ref(), &label) {
                button.emit_clicked();
            }
        };
        let ctx2 = Rc::clone(&ctx);
        glib::timeout_add_local_once(Duration::from_millis(800), move || match shot.as_str() {
            "synopsis-revealed" => {
                if let Some(page) = ctx2.nav().and_then(|nav| nav.visible_page()) {
                    let _ = WidgetExt::activate_action(
                        &page,
                        "title.show-synopsis",
                        Some(&"9".to_variant()),
                    );
                }
            }
            "synopsis-shown" => press(kaeri_i18n::t::episode_protection_show()),
            "go-dialog" | "go-dialog-none" | "go-dialog-not-out" => {
                let page = ctx2.nav().and_then(|nav| nav.visible_page());
                let window = ctx2.window.clone();
                if let Some(page) = page {
                    let _ = WidgetExt::activate_action(
                        &page,
                        "title.shot-go",
                        Some(&"dialog".to_variant()),
                    );
                    let text = match shot.as_str() {
                        "go-dialog-none" => "537",
                        "go-dialog-not-out" => "245",
                        _ => "",
                    };
                    glib::timeout_add_local_once(Duration::from_millis(500), move || {
                        if let Some(entry) =
                            window.visible_dialog().and_then(|d| find_entry(d.upcast_ref()))
                        {
                            entry.set_text(text);
                            entry.set_position(-1);
                        }
                    });
                }
            }
            "go-typed" | "go-landed" | "go-error" => {
                if let Some(page) = ctx2.nav().and_then(|nav| nav.visible_page()) {
                    let arg = match shot.as_str() {
                        "go-typed" => "type:12",
                        "go-landed" => "go:120",
                        _ => "err:537",
                    };
                    let _ = WidgetExt::activate_action(
                        &page,
                        "title.shot-go",
                        Some(&arg.to_variant()),
                    );
                }
            }
            // Motion D: Go to episode from 230 to 128, another range, typed one digit at a
            // time and taken with Return.
            "motion-go" => {
                let Some(page) = ctx2.nav().and_then(|nav| nav.visible_page()) else { return };
                mark_step("go.dialog", &ctx2.window.clone().upcast());
                let _ = WidgetExt::activate_action(
                    &page,
                    "title.shot-go",
                    Some(&"dialog".to_variant()),
                );
                let window = ctx2.window.clone();
                for (i, text) in ["1", "12", "128"].into_iter().enumerate() {
                    let window = window.clone();
                    glib::timeout_add_local_once(
                        Duration::from_millis(700 + 260 * i as u64),
                        move || {
                            if let Some(entry) =
                                window.visible_dialog().and_then(|d| find_entry(d.upcast_ref()))
                            {
                                entry.set_text(text);
                                entry.set_position(-1);
                            }
                        },
                    );
                }
                glib::timeout_add_local_once(Duration::from_millis(2000), move || {
                    if let Some(entry) =
                        window.visible_dialog().and_then(|d| find_entry(d.upcast_ref()))
                    {
                        mark_step("go.land", &window.clone().upcast());
                        entry.emit_activate();
                    }
                });
            }
            // Motion E: 2, 3, 6 typed on the list, which goes to 236 in the same range once the
            // typing pauses.
            "motion-typed" => {
                let Some(page) = ctx2.nav().and_then(|nav| nav.visible_page()) else { return };
                // The list scrolled to the rows around the next episode first.
                let _ = WidgetExt::activate_action(
                    &page,
                    "title.shot-go",
                    Some(&"go:226".to_variant()),
                );
                let window = ctx2.window.clone();
                for (i, key) in ["2", "3", "6"].into_iter().enumerate() {
                    let page = page.clone();
                    let window = window.clone();
                    glib::timeout_add_local_once(
                        Duration::from_millis(1200 + 300 * i as u64),
                        move || {
                            if i == 0 {
                                mark_step("typed", &window.upcast());
                            }
                            let _ = WidgetExt::activate_action(
                                &page,
                                "title.shot-go",
                                Some(&format!("key:{key}").to_variant()),
                            );
                        },
                    );
                }
            }
            "synopsis-ranges" => {
                // Focused first, so the page scrolls the picker into view, then opened.
                if let Some(button) = range_button(&ctx2.window) {
                    button.grab_focus();
                    glib::timeout_add_local_once(Duration::from_millis(600), move || {
                        button.popup()
                    });
                }
            }
            _ => {}
        });
    });
    wait
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shots_are_named_or_all() {
        assert_eq!(named(Some("home, title")), ["home", "title"]);
        assert_eq!(named(None).len(), ALL.len());
        assert_eq!(named(Some("home,party")).len(), PARTY.len() + 1);
    }
}
