//! The states of the sync screens for the shot run: each puts made-up status into the window
//! the way the engine would say it, and opens the dialog of that state.

use std::{rc::Rc, time::Duration};

use gtk::glib;
use kaeri_model::{
    event::Event,
    group::{
        CloudUnavailable, SyncCloudView, SyncCloudWay, SyncDeviceKind, SyncDeviceView,
        SyncFolderKind, SyncFolderView, SyncNoteView, SyncProblem, SyncStatus,
    },
    profiles::{ProfileView, ProfilesView},
};

use gtk::prelude::*;

use super::dialogs::{self, Origin};
use crate::window::Shell;

pub const NAMES: [&str; 42] = [
    "sync-home-continue",
    "sync-off",
    "sync-picker",
    "sync-picker-error",
    "sync-switch",
    "sync-wait",
    "sync-connecting",
    "sync-setting-up",
    "sync-key-setup",
    "sync-wait-denied",
    "sync-wait-expired",
    "sync-wait-full",
    "sync-more",
    "sync-remove",
    "sync-nextcloud",
    "sync-webdav",
    "sync-restore",
    "sync-restore-wrong",
    "sync-key",
    "sync-on",
    "sync-syncing",
    "sync-problem",
    "sync-offline",
    "sync-pair-show",
    "sync-pair-paired",
    "sync-pair-enter",
    "sync-pair-enter-error",
    "sync-pair-enter-wrong",
    "sync-pair-paired-first",
    "sync-devices-full",
    "sync-device-page",
    "sync-device-page-remote-off",
    "sync-notes",
    "sync-turn-off",
    "sync-first-pair",
    "sync-motion",
    "profiles-one",
    "profiles-many",
    "profiles-add",
    "profiles-delete",
    "profiles-popover",
    "profiles-kids",
];

fn clouds() -> Vec<SyncCloudView> {
    let way = |s: SyncFolderKind, w: SyncCloudWay| SyncCloudView {
        service: s,
        way: w,
        signed_in: false,
        account: None,
    };
    vec![
        way(
            SyncFolderKind::GoogleDrive,
            SyncCloudWay::Unavailable { reason: CloudUnavailable::NoClientId },
        ),
        way(SyncFolderKind::Dropbox, SyncCloudWay::Browser),
        way(SyncFolderKind::OneDrive, SyncCloudWay::Browser),
    ]
}

fn now() -> i64 {
    glib::real_time() / 1_000_000
}

fn device(id: &str, name: &str, this: bool, ago: i64) -> SyncDeviceView {
    SyncDeviceView {
        id: id.into(),
        name: name.into(),
        this_device: this,
        last_seen: Some(now() - ago),
        trackers: Vec::new(),
        sign_ins: Vec::new(),
        folder: Some(SyncFolderKind::Dropbox),
        asked: Vec::new(),
        kind: Some(SyncDeviceKind::Computer),
        paused: false,
        remote_key: None,
        remote_off: false,
    }
}

/// A device that holds sign-ins another device could copy.
fn with_sign_ins(mut d: SyncDeviceView, kind: SyncDeviceKind) -> SyncDeviceView {
    d.kind = Some(kind);
    d.trackers = vec!["anilist".into(), "mal".into()];
    d.sign_ins = vec![SyncFolderKind::Dropbox];
    d
}

/// Every kind of row Devices can have: a phone with sign-ins, a TV with its Remote switch, a
/// device the group holds back, and a kept watch to undo.
fn full() -> SyncStatus {
    let mut tv = device("d", "Living room TV", false, 600);
    tv.kind = Some(SyncDeviceKind::Tv);
    tv.remote_key = Some("0a1b2c3d4e5f60718293a4b5c6d7e8f9".into());
    let mut tablet = device("c", "Living room tablet", false, 3 * 3600);
    tablet.kind = Some(SyncDeviceKind::Tablet);
    tablet.paused = true;
    SyncStatus {
        devices: vec![
            device("a", "Study desktop", true, 0),
            with_sign_ins(device("b", "Mika's phone", false, 120), SyncDeviceKind::Phone),
            tv,
            tablet,
        ],
        notes: vec![SyncNoteView {
            id: 1,
            anilist_id: 900_001,
            episode: "5".into(),
            watch_device: "Living room TV".into(),
            unwatch_device: "Mika's phone".into(),
            at: now() - 3600,
        }],
        ..on()
    }
}

fn off() -> SyncStatus {
    SyncStatus {
        device_name: Some("Study desktop".into()),
        clouds: clouds(),
        ..SyncStatus::default()
    }
}

fn on() -> SyncStatus {
    SyncStatus {
        enabled: true,
        device_name: Some("Study desktop".into()),
        folder: Some(SyncFolderView {
            kind: SyncFolderKind::Dropbox,
            label: "Dropbox".into(),
            account: Some("mika@example.invalid".into()),
        }),
        devices: vec![
            device("a", "Study desktop", true, 0),
            with_sign_ins(device("b", "Mika's phone", false, 120), SyncDeviceKind::Phone),
            device("c", "Living room tablet", false, 3 * 3600),
        ],
        last_synced: Some(now() - 120),
        recovery_key: true,
        recovery_key_made: Some(now() - 9 * 86_400),
        clouds: clouds(),
        group_folder: Some(SyncFolderKind::Dropbox),
        ..SyncStatus::default()
    }
}

fn profile(id: &str, name: &str, kids: bool, active: bool) -> ProfileView {
    ProfileView {
        id: id.into(),
        name: name.into(),
        initials: name.chars().take(1).collect(),
        kids,
        active,
    }
}

fn many() -> Vec<ProfileView> {
    vec![
        profile("p1", "Mika", false, true),
        profile("p2", "Sora", false, false),
        profile("p3", "Kids", true, false),
    ]
}

/// Put the window in `shot`'s state. Answers how long to wait before the picture.
pub fn stage(shell: &Rc<Shell>, shot: &str) -> f64 {
    let ctx = &shell.ctx;
    let hub = &ctx.hub;
    let say = |events: Vec<Event>| hub.show_instead(events);
    let pane = if shot.starts_with("profiles") { "profiles" } else { "devices" };
    if shot == "sync-home-continue" {
        // Home as the engine sends it when the titles under way were last played on a paired
        // device: its Continue cards and the spotlight say so. Each Home that arrives is sent
        // again once, with the device set.
        shell.go(crate::pages::Dest::Home);
        let hub_again = Rc::clone(hub);
        let hub = Rc::clone(hub);
        let _ = hub.clone().listen(move |event| {
            let Event::Home(view) = event else { return };
            let stamped =
                view.sections.iter().flat_map(|s| &s.items).any(|i| i.last_device.is_some());
            if stamped {
                return;
            }
            let mut view = view.clone();
            let device = Some("Mika's phone".to_owned());
            if let Some(spotlight) = view.spotlight.as_mut()
                && spotlight.resume.is_some()
            {
                spotlight.last_device = device.clone();
            }
            for section in &mut view.sections {
                if section.kind == kaeri_model::home::SectionKind::Continue {
                    for item in section.items.iter_mut().take(2) {
                        if item.resume.is_some() {
                            item.last_device = device.clone();
                        }
                    }
                }
            }
            let hub = Rc::clone(&hub);
            glib::idle_add_local_once(move || hub.take(vec![Event::Home(view)]));
        });
        // Home was sent before this listened: ask for it again.
        hub_again.send(kaeri_model::command::Command::LoadHome { refresh: false });
        return 5.0;
    }
    match shot {
        "sync-off" => say(vec![Event::SyncStatus(off())]),
        "sync-picker" | "sync-picker-error" | "sync-wait" | "sync-connecting"
        | "sync-setting-up" | "sync-key-setup" | "sync-wait-denied" | "sync-wait-expired"
        | "sync-wait-full" | "sync-nextcloud" | "sync-webdav" | "sync-restore"
        | "sync-restore-wrong" => {
            say(vec![Event::SyncStatus(off())]);
        }
        "sync-on"
        | "sync-switch"
        | "sync-more"
        | "sync-remove"
        | "sync-key"
        | "sync-pair-show"
        | "sync-pair-paired"
        | "sync-pair-enter"
        | "sync-pair-enter-error"
        | "sync-pair-enter-wrong"
        | "sync-turn-off" => say(vec![Event::SyncStatus(on())]),
        // First launch: the phone that paired made an invite for one plugin store.
        "sync-pair-paired-first" => say(vec![Event::SyncStatus(SyncStatus {
            stores_to_join: vec![kaeri_model::group::SyncStoreOffer {
                address: "https://club.example".into(),
                name: "Reading club".into(),
                invite_from: Some("b".into()),
            }],
            ..on()
        })]),
        "sync-devices-full"
        | "sync-device-page"
        | "sync-device-page-remote-off"
        | "sync-notes" => {
            // The kept watch's title comes from the engine's cards, as it does for a person.
            say(vec![Event::SyncStatus(full())]);
        }
        "sync-syncing" => say(vec![Event::SyncStatus(SyncStatus { syncing: true, ..on() })]),
        "sync-problem" => say(vec![Event::SyncStatus(SyncStatus {
            problem: Some(SyncProblem::SignInNeeded { service: SyncFolderKind::Dropbox }),
            ..on()
        })]),
        "sync-offline" => say(vec![Event::SyncStatus(SyncStatus {
            error: Some("offline".into()),
            problem: Some(SyncProblem::Unreachable),
            ..on()
        })]),
        "profiles-one" | "profiles-add" => say(vec![Event::Profiles(ProfilesView::default())]),
        "profiles-many" | "profiles-delete" | "profiles-popover" | "profiles-kids" => {
            say(vec![Event::Profiles(ProfilesView { profiles: many() })]);
        }
        _ => {}
    }
    if shot == "sync-motion" {
        say(vec![Event::SyncStatus(off())]);
    }
    if shot != "sync-first-pair" {
        shell.settings_pane(pane);
    }
    let ctx = Rc::clone(ctx);
    let owned = shot.to_owned();
    glib::timeout_add_local_once(Duration::from_millis(500), move || open(&ctx, &owned));
    if shot == "sync-motion" {
        return 16.0;
    }
    2.0
}

fn open(ctx: &Rc<crate::window::Ctx>, shot: &str) {
    let open_url = "https://auth.example/authorize";
    match shot {
        // For the recording: the picker, Dropbox chosen (the list crossfades to the wait), then
        // the engine's step arrives and the wait gives way to the finished folder.
        "sync-motion" => {
            // The list, Nextcloud's form (the title crossfades with the page), the list again,
            // then Dropbox's wait.
            dialogs::picker(ctx);
            let ctx = Rc::clone(ctx);
            glib::timeout_add_local_once(Duration::from_millis(1500), move || {
                dialogs::picker_page(&ctx, "nextcloud");
                let ctx = Rc::clone(&ctx);
                glib::timeout_add_local_once(Duration::from_millis(2000), move || {
                    dialogs::picker_page(&ctx, "list");
                    let ctx = Rc::clone(&ctx);
                    glib::timeout_add_local_once(Duration::from_millis(1500), move || {
                        // Dropbox chosen: the wait, the return, the folder being made, the key.
                        let dropbox = SyncFolderKind::Dropbox;
                        dialogs::picker_state(&ctx, &dropbox, "wait", open_url);
                        let ctx2 = Rc::clone(&ctx);
                        glib::timeout_add_local_once(Duration::from_millis(2500), move || {
                            dialogs::picker_state(&ctx2, &dropbox, "connecting", open_url);
                            let ctx3 = Rc::clone(&ctx2);
                            glib::timeout_add_local_once(
                                Duration::from_millis(1500),
                                move || {
                                    dialogs::picker_state(&ctx3, &dropbox, "setting", open_url);
                                    let ctx4 = Rc::clone(&ctx3);
                                    glib::timeout_add_local_once(
                                        Duration::from_millis(1500),
                                        move || {
                                            dialogs::picker_state(
                                                &ctx4, &dropbox, "key", open_url,
                                            );
                                        },
                                    );
                                },
                            );
                        });
                    });
                });
            });
        }
        "sync-picker" => dialogs::picker(ctx),
        "sync-picker-error" => {
            ctx.hub.take(vec![Event::SyncProblem(SyncProblem::Unreachable)]);
            dialogs::picker(ctx);
        }
        "sync-switch" => dialogs::picker_change(ctx, &SyncFolderKind::Dropbox),
        "sync-wait" => {
            dialogs::picker_state(ctx, &SyncFolderKind::GoogleDrive, "wait", open_url)
        }
        "sync-connecting" => {
            dialogs::picker_state(ctx, &SyncFolderKind::GoogleDrive, "connecting", open_url)
        }
        "sync-setting-up" => {
            dialogs::picker_state(ctx, &SyncFolderKind::GoogleDrive, "setting", open_url)
        }
        "sync-key-setup" => {
            dialogs::picker_state(ctx, &SyncFolderKind::GoogleDrive, "key", open_url)
        }
        "sync-wait-denied" => {
            dialogs::picker_state(ctx, &SyncFolderKind::GoogleDrive, "denied", open_url)
        }
        "sync-wait-expired" => {
            dialogs::picker_state(ctx, &SyncFolderKind::GoogleDrive, "expired", open_url)
        }
        "sync-wait-full" => {
            dialogs::picker_state(ctx, &SyncFolderKind::GoogleDrive, "full", open_url)
        }
        "sync-more" => super::devices::pop_more(),
        "sync-remove" => dialogs::remove_folder(ctx, &SyncFolderKind::Dropbox),
        "sync-nextcloud" => dialogs::picker_page(ctx, "nextcloud"),
        "sync-webdav" => dialogs::picker_page(ctx, "webdav"),
        "sync-restore" => dialogs::restore(ctx, &SyncFolderKind::Dropbox),
        "sync-restore-wrong" => dialogs::restore_wrong(ctx),
        "sync-key" => {
            ctx.hub.take(vec![Event::SyncRecoveryKey("ABCDEFGHJKLMNPQRSTUVWXYZ".into())]);
            dialogs::key_dialog(ctx);
        }
        "sync-pair-show"
        | "sync-pair-paired"
        | "sync-pair-paired-first"
        | "sync-first-pair" => {
            ctx.hub.known.borrow_mut().invite = Some(("482913".into(), 47_800, now() + 120));
            let origin = if matches!(shot, "sync-first-pair" | "sync-pair-paired-first") {
                Origin::FirstLaunch
            } else {
                Origin::Devices
            };
            if matches!(shot, "sync-pair-paired" | "sync-pair-paired-first") {
                dialogs::pair_show_paired(ctx, "Mika's phone", origin);
            } else {
                dialogs::present_pair_show(ctx, origin);
            }
        }
        "sync-pair-enter" => dialogs::pair_enter(ctx),
        "sync-pair-enter-error" => {
            dialogs::pair_enter_error(ctx, &kaeri_i18n::t::pair_err_not_found());
        }
        "sync-pair-enter-wrong" => {
            dialogs::pair_enter_error(ctx, &kaeri_i18n::t::pair_err_code("Mika's phone"));
        }
        "sync-device-page" => {
            if let Some(d) = full().devices.into_iter().find(|d| d.id == "d") {
                dialogs::device_page(ctx, &d);
            }
        }
        "sync-device-page-remote-off" => {
            if let Some(mut d) = full().devices.into_iter().find(|d| d.id == "d") {
                d.remote_off = true;
                dialogs::device_page(ctx, &d);
            }
        }
        "sync-turn-off" => dialogs::turn_off(ctx),
        "profiles-add" => super::profiles::edit_dialog(ctx, None),
        "profiles-kids" => {
            super::profiles::edit_dialog(ctx, Some(profile("p3", "Kids", true, false)))
        }
        "profiles-popover" => {
            let pop = super::profiles::popover(ctx);
            pop.set_parent(&ctx.window);
            pop.popup();
        }
        _ => {}
    }
}
