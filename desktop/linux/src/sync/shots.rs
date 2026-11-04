//! The states of the sync screens for the shot run: each puts made-up status into the window
//! the way the engine would say it, and opens the dialog of that state.

use std::{rc::Rc, time::Duration};

use gtk::glib;
use kaeri_model::{
    event::Event,
    group::{
        CloudUnavailable, SyncCloudView, SyncCloudWay, SyncDeviceView, SyncFolderKind,
        SyncFolderView, SyncProblem, SyncSignInStep, SyncStatus,
    },
    profiles::{ProfileView, ProfilesView},
};

use gtk::prelude::*;

use super::dialogs::{self, Origin};
use crate::window::Shell;

pub const NAMES: [&str; 27] = [
    "sync-off",
    "sync-road",
    "sync-picker",
    "sync-picker-error",
    "sync-wait",
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
    }
}

pub fn finished() -> SyncStatus {
    on()
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
            device("b", "Mika's phone", false, 120),
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
    match shot {
        "sync-off" => say(vec![Event::SyncStatus(off())]),
        "sync-road" | "sync-picker" | "sync-picker-error" | "sync-wait" | "sync-nextcloud"
        | "sync-webdav" | "sync-restore" | "sync-restore-wrong" => {
            say(vec![Event::SyncStatus(off())]);
        }
        "sync-on"
        | "sync-key"
        | "sync-pair-show"
        | "sync-pair-paired"
        | "sync-pair-enter"
        | "sync-pair-enter-error"
        | "sync-turn-off" => say(vec![Event::SyncStatus(on())]),
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
        return 7.0;
    }
    2.0
}

fn open(ctx: &Rc<crate::window::Ctx>, shot: &str) {
    let open_url = "https://auth.example/authorize";
    match shot {
        // For the recording: the picker, Dropbox chosen (the list crossfades to the wait), then
        // the engine's step arrives and the wait gives way to the finished folder.
        "sync-motion" => {
            dialogs::picker(ctx);
            let ctx = Rc::clone(ctx);
            glib::timeout_add_local_once(Duration::from_millis(2000), move || {
                dialogs::picker_wait(&ctx, &SyncFolderKind::Dropbox, open_url);
                let ctx2 = Rc::clone(&ctx);
                glib::timeout_add_local_once(Duration::from_millis(2500), move || {
                    ctx2.hub.take(vec![Event::SyncStatus(super::shots::finished())]);
                    dialogs::finish_for_shot(&ctx2);
                });
            });
        }
        "sync-road" => dialogs::road(ctx),
        "sync-picker" => dialogs::picker(ctx),
        "sync-picker-error" => {
            ctx.hub.take(vec![Event::SyncProblem(SyncProblem::Unreachable)]);
            dialogs::picker(ctx);
        }
        "sync-wait" => dialogs::picker_wait(ctx, &SyncFolderKind::Dropbox, open_url),
        "sync-nextcloud" => dialogs::picker_page(ctx, "nextcloud"),
        "sync-webdav" => dialogs::picker_page(ctx, "webdav"),
        "sync-restore" => dialogs::restore(ctx, &SyncFolderKind::Dropbox),
        "sync-restore-wrong" => dialogs::restore_wrong(ctx),
        "sync-key" => {
            ctx.hub.take(vec![Event::SyncRecoveryKey("ABCDEFGHJKLMNPQRSTUVWXYZ".into())]);
            dialogs::key_dialog(ctx);
        }
        "sync-pair-show" | "sync-pair-paired" | "sync-first-pair" => {
            ctx.hub.known.borrow_mut().invite = Some(("482913".into(), 47_800, now() + 270));
            let origin =
                if shot == "sync-first-pair" { Origin::FirstLaunch } else { Origin::Devices };
            if shot == "sync-pair-paired" {
                dialogs::pair_show_paired(ctx, "Mika's phone", origin);
            } else {
                dialogs::present_pair_show(ctx, origin);
            }
        }
        "sync-pair-enter" => dialogs::pair_enter(ctx),
        "sync-pair-enter-error" => {
            dialogs::pair_enter_error(ctx, &kaeri_i18n::t::pair_err_not_found());
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
    let _ = SyncSignInStep::Finishing;
}
