//! Sync between a person's own devices, and the profiles on this one: Devices and Profiles in
//! Settings, the cloud folder's sign-in, the recovery key, pairing both ways. The engine holds
//! every rule; these screens show what it last said (`Known::sync_group`) and send its commands.

pub mod devices;
pub mod dialogs;
pub mod lock;
pub mod profiles;

use std::rc::Rc;

use kaeri_i18n::t;
use kaeri_model::{
    command::Command,
    group::{SyncDeviceView, SyncFolderKind, SyncProblem, SyncStatus},
};

use crate::{say, window::Ctx};

/// Wire up the sync dialogs to the engine's events, once, and ask how sync and profiles stand.
pub fn install(ctx: &Rc<Ctx>) {
    dialogs::install(ctx);
    profiles::install(ctx);
    ctx.hub.send(Command::LoadSync);
    ctx.hub.send(Command::LoadProfiles);
}

/// The service name a person reads.
pub fn service_name(kind: &SyncFolderKind) -> String {
    match kind {
        SyncFolderKind::Directory => t::cloud_folder(&t::device_kind_computer()),
        SyncFolderKind::WebDav => t::cloud_webdav(),
        SyncFolderKind::Nextcloud => "Nextcloud".to_owned(), // i18n-skip
        SyncFolderKind::GoogleDrive => "Google Drive".to_owned(), // i18n-skip
        SyncFolderKind::Dropbox => "Dropbox".to_owned(),     // i18n-skip
        SyncFolderKind::OneDrive => "OneDrive".to_owned(),   // i18n-skip
    }
}

/// The word for this device in "Turn off sync on this computer".
pub fn kind() -> String {
    t::device_kind_computer()
}

/// A dot's colour for a line, as `widgets::standing` names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    Water,
    Amber,
    Vermilion,
    Hollow,
    None,
}

impl Mark {
    /// The line with its dot, as Pango markup: the text is escaped.
    pub fn markup(self, text: &str) -> String {
        let text = glib_escape(text);
        let dot = match self {
            Self::Water => "<span foreground=\"#7FC7D9\">\u{25CF}</span> ",
            Self::Amber => "<span foreground=\"#F2A64B\">\u{25CF}</span> ",
            Self::Vermilion => "<span foreground=\"#E06B54\">\u{25CF}</span> ",
            Self::Hollow => "<span foreground=\"#A3A9C4\">\u{25CB}</span> ",
            Self::None => "",
        };
        format!("{dot}{text}")
    }
}

fn glib_escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// How long ago, inside a sentence: "just now", "2 min ago".
pub fn ago_inline(seconds: i64) -> String {
    if seconds < 60 { t::time_just_now_inline() } else { t::time_ago(&say::countdown(seconds)) }
}

/// What a device's row says: when it last synced, or that it has not been seen for a month.
pub fn seen_line(device: &SyncDeviceView, now: i64) -> (String, Mark) {
    match device.last_seen {
        Some(seen) => {
            let days = (now - seen) / 86_400;
            if days >= 30 {
                (t::devices_not_seen(days), Mark::Amber)
            } else {
                (t::devices_row_synced(&ago_inline((now - seen).max(0))), Mark::Water)
            }
        }
        None => (String::new(), Mark::None),
    }
}

/// What the cloud row says: the state and its dot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloudState {
    Synced(String),
    Syncing,
    SettingUp,
    SignedOut,
    /// The service signed this device out, or its folder is gone: the line and its button.
    Problem(String, ProblemFix),
    Offline,
}

/// What to offer when the cloud row has a problem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProblemFix {
    SignInAgain,
    ChooseFolder,
    Nothing,
}

/// The cloud row's state from the engine's status; `by_hand` is that the person pressed Sign
/// out, so a sign-in that is needed is theirs and no error.
pub fn cloud_state(
    status: &SyncStatus,
    now: i64,
    setting_up: bool,
    by_hand: bool,
) -> CloudState {
    let service = status.folder.as_ref().map(|f| service_name(&f.kind)).unwrap_or_default();
    if setting_up {
        return CloudState::SettingUp;
    }
    match &status.problem {
        Some(SyncProblem::SignInNeeded { .. }) if by_hand => return CloudState::SignedOut,
        Some(SyncProblem::SignInNeeded { .. }) => {
            return CloudState::Problem(
                t::cloud_err_signed_out(&service, &kind()),
                ProblemFix::SignInAgain,
            );
        }
        Some(SyncProblem::Unreachable)
            if status.folder.as_ref().is_some_and(|f| f.kind == SyncFolderKind::Directory) =>
        {
            return CloudState::Problem(t::cloud_err_folder_gone(), ProblemFix::ChooseFolder);
        }
        Some(SyncProblem::Unreachable) => return CloudState::Offline,
        Some(_) => {
            return CloudState::Problem(
                status.error.clone().unwrap_or_else(t::devices_row_error),
                ProblemFix::Nothing,
            );
        }
        None => {}
    }
    if status.syncing {
        return CloudState::Syncing;
    }
    match status.last_synced {
        Some(at) => CloudState::Synced(t::devices_row_synced(&ago_inline((now - at).max(0)))),
        None => CloudState::Synced(t::devices_row_synced(&t::time_just_now_inline())),
    }
}

/// The cloud services this build offers, alphabetical, with those it cannot sign in to hidden
/// (a service without a client id, or without a way here). A service that needs a phone is a
/// TV's affair and never listed on a computer.
pub fn offered(status: &SyncStatus) -> Vec<(SyncFolderKind, String)> {
    use kaeri_model::group::SyncCloudWay;
    let mut out: Vec<(SyncFolderKind, String)> = status
        .clouds
        .iter()
        .filter(|c| {
            !matches!(c.way, SyncCloudWay::Unavailable { .. } | SyncCloudWay::DeviceCode)
        })
        .map(|c| (c.service.clone(), service_name(&c.service)))
        .collect();
    out.push((SyncFolderKind::Nextcloud, service_name(&SyncFolderKind::Nextcloud)));
    out.sort_by(|a, b| a.1.cmp(&b.1));
    out
}

/// The six groups of four of a recovery key, in two rows of three.
pub fn key_groups(key: &str) -> Vec<String> {
    let clean: Vec<char> = key
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect();
    clean.chunks(4).map(|c| c.iter().collect()).collect()
}

/// A pairing code as "042 917".
pub fn spaced_code(code: &str) -> String {
    if code.len() == 6 { format!("{} {}", &code[..3], &code[3..]) } else { code.to_owned() }
}

/// "1:48" from seconds.
pub fn minutes_seconds(left: i64) -> String {
    let left = left.max(0);
    format!("{}:{:02}", left / 60, left % 60)
}

/// This device's address on the local network, for the code the other device types or scans.
pub fn local_address() -> Option<String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    // Nothing is sent: connecting a datagram socket only picks the route.
    socket.connect("192.0.2.1:9").ok()?;
    Some(socket.local_addr().ok()?.ip().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaeri_model::group::{CloudUnavailable, SyncCloudView, SyncCloudWay};

    fn cloud(service: SyncFolderKind, way: SyncCloudWay) -> SyncCloudView {
        SyncCloudView { service, way, signed_in: false, account: None }
    }

    #[test]
    fn the_picker_hides_what_this_build_cannot_sign_in_to() {
        let status = SyncStatus {
            clouds: vec![
                cloud(SyncFolderKind::OneDrive, SyncCloudWay::Browser),
                cloud(
                    SyncFolderKind::GoogleDrive,
                    SyncCloudWay::Unavailable { reason: CloudUnavailable::NoClientId },
                ),
                cloud(SyncFolderKind::Dropbox, SyncCloudWay::Browser),
            ],
            ..SyncStatus::default()
        };
        let names: Vec<_> = offered(&status).into_iter().map(|(_, n)| n).collect();
        assert_eq!(names, ["Dropbox", "Nextcloud", "OneDrive"]);
    }

    #[test]
    fn a_key_reads_in_groups_of_four() {
        assert_eq!(
            key_groups("k7qm4xr9tb2h wn6cp3zd8fla"),
            ["K7QM", "4XR9", "TB2H", "WN6C", "P3ZD", "8FLA"]
        );
    }

    #[test]
    fn a_code_is_spaced_and_a_countdown_reads_as_minutes() {
        assert_eq!(spaced_code("042917"), "042 917");
        assert_eq!(minutes_seconds(108), "1:48");
    }

    #[test]
    fn a_signed_out_service_says_so_unless_the_person_did_it() {
        let status = SyncStatus {
            enabled: true,
            folder: Some(kaeri_model::group::SyncFolderView {
                kind: SyncFolderKind::GoogleDrive,
                label: String::new(),
                account: None,
            }),
            problem: Some(SyncProblem::SignInNeeded { service: SyncFolderKind::GoogleDrive }),
            ..SyncStatus::default()
        };
        assert!(matches!(
            cloud_state(&status, 0, false, false),
            CloudState::Problem(_, ProblemFix::SignInAgain)
        ));
        assert_eq!(cloud_state(&status, 0, false, true), CloudState::SignedOut);
    }
}

#[cfg(feature = "fixtures")]
pub mod shots;
