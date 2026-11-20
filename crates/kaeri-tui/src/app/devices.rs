//! Devices, reached from Settings: this device's name, the others it syncs with, pairing both
//! ways, the cloud folder and its recovery key, and the profiles when there are two or more.
//!
//! The engine owns sync. What it last said is kept here as it said it, with what the screen's
//! questions are waiting on: the folder being set up, the code on show, the key to show once.

use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use kaeri_i18n::t;
use kaeri_model::{
    catalog::Entry,
    command::Command,
    group::{
        SyncCloudWay, SyncCommand, SyncCopy, SyncDeviceView, SyncFolderKind, SyncFolderSpec,
        SyncPairProblem, SyncProblem, SyncSignInStep, SyncStatus,
    },
    profiles::{ProfileView, ProfilesView},
    stores::Secret,
};

use kaeri_core::ids::AnilistId;

use super::{App, Cursor, Field, alert, info};
use crate::{
    keymap::Action,
    nav::{Overlay, Section, StageView},
};

/// A pairing code on show: the code, the port this device listens at, and when it runs out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairCode {
    pub code: String,
    pub port: u16,
    /// Unix seconds.
    pub until: i64,
}

/// What the confirm overlay is asking about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceConfirm {
    /// Remove another device from the group.
    Remove { id: String, name: String },
    /// Turn sync off on this device.
    TurnOff,
    /// Make a new recovery key, which stops the old one working.
    NewKey,
    /// Delete the sync data in the folder on the restore screen and set sync up there again,
    /// without the key.
    StartOver,
}

/// How long a start over's work waits before it shows: a quick service goes straight from the
/// confirmation to the key.
const START_OVER_QUIET: Duration = Duration::from_millis(600);

/// How long each step of a start over stays on show, once it has shown.
const START_OVER_HOLD: Duration = Duration::from_millis(400);

/// A step of starting over without the key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartOverStep {
    /// Asked of the engine, with nothing heard yet.
    Asked,
    /// The folder's old history is being deleted.
    Clearing,
    /// The folder is being set up again.
    SettingUp,
    /// It could not be done, in words. Nothing on this device changed.
    Failed(String),
    /// Done: the new key shows next.
    Done,
}

/// A start over without the key, from the confirmation to the new key.
///
/// What the engine said and what is on show are kept apart, so the work shows only after
/// [`START_OVER_QUIET`] and each step stays [`START_OVER_HOLD`] at least.
#[derive(Debug, Clone)]
pub struct StartOver {
    /// The service the folder is in, for the lines that name it.
    pub service: SyncFolderKind,
    /// The folder, to ask again with.
    folder: SyncFolderSpec,
    /// When it was asked.
    began: Instant,
    /// What the engine said last.
    said: StartOverStep,
    /// What is on show, and since when. `Asked` while the content before it holds.
    pub shown: StartOverStep,
    since: Instant,
    /// The new key, held until the step on show has had its time.
    key: Option<String>,
}

impl StartOver {
    /// Whether the screen still shows what was there before the work: the restore content, or
    /// the error a Try again answered.
    fn held(&self) -> bool {
        match &self.shown {
            StartOverStep::Asked => true,
            StartOverStep::Failed(_) => !matches!(self.said, StartOverStep::Failed(_)),
            _ => false,
        }
    }
}

/// A choice on a device's page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DevicePageItem {
    /// Its Sync switch.
    Sync,
    /// Its Remote switch, when it is paired as a remote.
    Remote,
    /// Ask it for its sign-ins.
    CopySignIns,
    Remove,
}

/// A row of the Devices screen the cursor stops on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceLine {
    /// This device's name: change it.
    Name,
    /// Sync is off: turn it on.
    TurnOn,
    /// Another device in the group, by its place in the status's list.
    Device(usize),
    ShowCode,
    EnterCode,
    /// No cloud folder yet: add one.
    AddCloud,
    /// The cloud folder: sync now, or sign in again.
    Folder,
    /// Another device started sync over in the folder: enter the new key.
    EnterNewKey,
    /// Sign out of the cloud drive the folder is in.
    SignOut,
    RecoveryKey,
    /// Make a new recovery key.
    NewKey,
    TurnOff,
    /// A sync note, by its place in the status's list: mark the episode unwatched.
    Note(usize),
    /// A profile, by its place in the list.
    Profile(usize),
    AddProfile,
}

/// How the cloud folder stands, for its row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderState {
    Synced,
    Syncing,
    SettingUp,
    /// The drive signed this device out, or it was signed out here.
    SignedOut,
    Failed,
}

/// The Devices screen's state: see the module.
#[derive(Debug, Default)]
pub struct Devices {
    /// How sync stands, as the engine last said.
    pub status: Option<SyncStatus>,
    /// The profiles, empty for a single user.
    pub profiles: Vec<ProfileView>,
    /// What this computer is called, which sync starts with when it has no name in the group.
    /// Set by whoever runs the app.
    pub this_name: String,
    /// This computer's address on the network, shown under a pairing code. Set by whoever runs
    /// the app once a code is on show.
    pub address: Option<String>,
    /// The pairing code on show.
    pub invite: Option<PairCode>,
    /// The devices in the group when a pairing began, so the one it adds can be named.
    pub pairing_with: Option<Vec<String>>,
    /// The cloud sign-in running in the browser, and its last step.
    pub sign_in: Option<(SyncFolderKind, Option<SyncSignInStep>)>,
    /// Signed in; the folder is being set up.
    pub setting_up: bool,
    /// A set-up, join or pairing is on its way to the engine's answer.
    pub working: bool,
    /// The recovery key, while it is on show. Forgotten when the overlay closes.
    pub recovery_key: Option<String>,
    pub key_copied: bool,
    pub show_qr: bool,
    /// The last problem, said inside the question it answers.
    pub problem: Option<String>,
    /// The kind of folder the forms are asking about.
    pub kind: Option<SyncFolderKind>,
    /// The folder last asked for, without its password: joined with a recovery key when it
    /// holds another group.
    pub pending: Option<SyncFolderSpec>,
    pub confirm: Option<DeviceConfirm>,
    /// The device whose page is open, by id.
    pub device_page: Option<String>,
    /// Why a pairing by code missed, in words, until the problem that follows says it.
    pub pair_failed: Option<String>,
    /// The titles sync notes name, as the engine answered.
    pub note_titles: HashMap<u32, String>,
    /// The titles already asked for.
    pub titles_asked: HashSet<u32>,
    /// The profile switched to, which the terminal starts again on.
    pub restart_as: Option<String>,
    /// The restore overlay's Start over line has the keyboard, rather than the key field.
    pub start_over_focused: bool,
    /// A start over without the key, under way or failed.
    pub start_over: Option<StartOver>,
    /// The key on show was made by starting over in this service.
    pub started_over: Option<SyncFolderKind>,
}

/// A cloud service's name. The brands are not translated.
pub(crate) fn service_name(kind: &SyncFolderKind) -> String {
    match kind {
        SyncFolderKind::GoogleDrive => "Google Drive".into(), // i18n-skip
        SyncFolderKind::Dropbox => "Dropbox".into(),          // i18n-skip
        SyncFolderKind::OneDrive => "OneDrive".into(),        // i18n-skip
        SyncFolderKind::Nextcloud => "Nextcloud".into(),      // i18n-skip
        SyncFolderKind::WebDav => t::cloud_webdav(),
        SyncFolderKind::Directory => t::cloud_folder(&t::sync_tui_kind()),
    }
}

/// The line under a cloud folder's name in the picker.
pub(crate) fn service_line(kind: &SyncFolderKind) -> String {
    match kind {
        SyncFolderKind::GoogleDrive => t::cloud_google_line(),
        SyncFolderKind::Dropbox => t::cloud_dropbox_line(),
        SyncFolderKind::OneDrive => t::cloud_onedrive_line(),
        SyncFolderKind::Nextcloud => t::cloud_nextcloud_line(),
        SyncFolderKind::WebDav => t::cloud_webdav_line(),
        SyncFolderKind::Directory => t::cloud_folder_line(),
    }
}

/// A cloud drive signed in to with a browser, as opposed to a server or a folder.
pub(crate) const fn is_drive(kind: &SyncFolderKind) -> bool {
    matches!(
        kind,
        SyncFolderKind::GoogleDrive | SyncFolderKind::Dropbox | SyncFolderKind::OneDrive
    )
}

/// A recovery key in its six groups of four, whatever spacing it came with.
pub fn key_groups(key: &str) -> String {
    let chars: Vec<char> = key.chars().filter(char::is_ascii_alphanumeric).collect();
    chars.chunks(4).map(|group| group.iter().collect::<String>()).collect::<Vec<_>>().join(" ")
}

/// A pairing code as it is read out: `042 917`.
pub fn code_groups(code: &str) -> String {
    let chars: Vec<char> = code.chars().collect();
    chars.chunks(3).map(|group| group.iter().collect::<String>()).collect::<Vec<_>>().join(" ")
}

/// The text a phone scans to pair: the code, and where this computer listens for it.
pub fn pair_link(code: &str, address: &str, port: u16) -> String {
    format!("kaeri://pair/{code}@{address}:{port}") // i18n-skip
}

impl App {
    /// Whether the Devices screen is on top.
    pub fn in_devices(&self) -> bool {
        self.nav.current() == &StageView::Devices
    }

    /// The name this device has in the group, else what the computer is called.
    pub fn device_name(&self) -> String {
        self.devices
            .status
            .as_ref()
            .and_then(|s| s.device_name.clone())
            .or_else(|| {
                Some(self.devices.this_name.trim().to_owned()).filter(|n| !n.is_empty())
            })
            .unwrap_or_else(|| kaeri_model::stores::COMPUTER.to_owned())
    }

    /// What this computer is called, for sync to start with.
    pub fn set_device_name(&mut self, name: String) {
        self.devices.this_name = name;
    }

    /// This computer's address on the network, for the pairing code's address line.
    pub fn set_lan_address(&mut self, address: Option<String>) {
        self.devices.address = address;
    }

    /// The profile a switch asked for, which the terminal starts again on once it has closed.
    pub fn profile_restart(&self) -> Option<&str> {
        self.devices.restart_as.as_deref()
    }

    /// Whether something on screen counts down by the second: the pairing code's time, or a
    /// check of a missing episode waiting to show Checking or its answer.
    pub fn is_ticking(&self) -> bool {
        (self.nav.overlay() == Some(&Overlay::SyncPairShow) && self.devices.invite.is_some())
            || self.start_over_moving()
            || self.reader_ticking()
            || self.checking_missing()
    }

    /// Whoever runs the app copied the text it was handed, or could not: the store's open
    /// link and the recovery key say which.
    pub fn clipboard_went(&mut self, went: bool) {
        self.open_link_copied = went;
        if self.nav.overlay() == Some(&Overlay::SyncRecoveryKey) {
            self.devices.key_copied = went;
        }
    }

    /// Whether the profile in use is a kids profile, where Devices is hidden.
    fn in_kids_profile(&self) -> bool {
        self.devices.profiles.iter().any(|p| p.active && p.kids)
    }

    /// Open Devices over Settings, asking how sync and the profiles stand.
    pub(super) fn open_devices(&mut self) -> Option<Command> {
        if self.in_kids_profile() {
            self.push_toast(info(t::profiles_kids_hidden()));
            return None;
        }
        if !self.in_devices() {
            if self.nav.current() != &StageView::Section(Section::Settings)
                && let Some(task) = self.go_to_section(Section::Settings)
            {
                self.commands.push(task);
            }
            self.cursors.devices = Cursor::default();
            self.nav.push(StageView::Devices);
        }
        self.commands.push(Command::LoadProfiles);
        Some(Command::LoadSync)
    }

    /// The Devices screen's keys, and the one Settings has for it. `Some` when one was taken.
    ///
    /// Settings reads `d` as the way in, the key Download has where there is a title. They are
    /// matched before the title commands, which would answer that nothing is selected.
    pub(super) fn devices_key(&mut self, action: Action) -> Option<Option<Command>> {
        let on_settings = self.nav.section() == Section::Settings
            && self.nav.current() == &StageView::Section(Section::Settings);
        if action == Action::ShowDevices || (on_settings && action == Action::Download) {
            return Some(self.open_devices());
        }
        if !self.in_devices() {
            return None;
        }
        Some(match action {
            Action::Open | Action::PlayNext => self.enter_device_line(),
            Action::Refresh => Some(Command::LoadSync),
            _ => return None,
        })
    }

    /// The rows of the Devices screen the cursor stops on, top to bottom.
    pub fn device_lines(&self) -> Vec<DeviceLine> {
        let mut lines = vec![DeviceLine::Name];
        match self.devices.status.as_ref().filter(|s| s.enabled) {
            None => lines.push(DeviceLine::TurnOn),
            Some(status) => {
                for (i, device) in status.devices.iter().enumerate() {
                    if !device.this_device {
                        lines.push(DeviceLine::Device(i));
                    }
                }
                for (i, note) in status.notes.iter().enumerate() {
                    if self.devices.note_titles.contains_key(&note.anilist_id) {
                        lines.push(DeviceLine::Note(i));
                    }
                }
                lines.push(DeviceLine::ShowCode);
                lines.push(DeviceLine::EnterCode);
                if self.folder_kind().is_some() {
                    lines.push(DeviceLine::Folder);
                    if self.group_replaced() {
                        lines.push(DeviceLine::EnterNewKey);
                    } else if self.signed_in_drive().is_some() {
                        lines.push(DeviceLine::SignOut);
                    }
                } else {
                    lines.push(DeviceLine::AddCloud);
                }
                if status.recovery_key && !self.group_replaced() {
                    lines.push(DeviceLine::RecoveryKey);
                    lines.push(DeviceLine::NewKey);
                }
                lines.push(DeviceLine::TurnOff);
            }
        }
        if self.devices.profiles.len() >= 2 {
            lines.extend((0..self.devices.profiles.len()).map(DeviceLine::Profile));
            lines.push(DeviceLine::AddProfile);
        }
        lines
    }

    /// The row under the cursor.
    pub fn device_line(&self) -> Option<DeviceLine> {
        self.device_lines().get(self.cursors.devices.selected).copied()
    }

    /// The kind of folder sync uses, or is being set up in.
    pub fn folder_kind(&self) -> Option<SyncFolderKind> {
        let status = self.devices.status.as_ref();
        status
            .and_then(|s| s.folder.as_ref().map(|f| f.kind.clone()))
            .or_else(|| self.setting_up_kind())
    }

    /// The folder being set up, while it is.
    fn setting_up_kind(&self) -> Option<SyncFolderKind> {
        let busy = self.devices.setting_up || self.devices.working;
        busy.then(|| self.devices.pending.as_ref().map(|p| p.kind.clone())).flatten()
    }

    /// Whether another device started sync over in the folder, so this one has to join with
    /// the new key.
    pub fn group_replaced(&self) -> bool {
        self.devices
            .status
            .as_ref()
            .is_some_and(|s| s.problem == Some(SyncProblem::GroupReplaced))
    }

    /// The cloud drive the folder is in, when this device is signed in to it.
    pub fn signed_in_drive(&self) -> Option<SyncFolderKind> {
        let status = self.devices.status.as_ref()?;
        let kind = status.folder.as_ref()?.kind.clone();
        let cloud = status.clouds.iter().find(|c| c.service == kind)?;
        (is_drive(&kind) && cloud.signed_in).then_some(kind)
    }

    /// How the cloud folder stands.
    pub fn folder_state(&self) -> FolderState {
        let Some(status) = self.devices.status.as_ref() else {
            return FolderState::SettingUp;
        };
        if status.folder.is_none() || self.devices.setting_up {
            return FolderState::SettingUp;
        }
        let kind = status.folder.as_ref().map(|f| f.kind.clone());
        let signed_out = matches!(
            &status.problem,
            Some(
                SyncProblem::SignInNeeded { .. }
                    | SyncProblem::SignInDenied { .. }
                    | SyncProblem::SignInExpired { .. }
            )
        ) || kind.as_ref().is_some_and(|kind| {
            is_drive(kind) && status.clouds.iter().any(|c| c.service == *kind && !c.signed_in)
        });
        if signed_out {
            FolderState::SignedOut
        } else if status.syncing {
            FolderState::Syncing
        } else if status.problem.is_some() || status.error.is_some() {
            FolderState::Failed
        } else {
            FolderState::Synced
        }
    }

    /// What went wrong with the folder, in words, when something did.
    pub fn folder_problem(&self) -> Option<String> {
        let status = self.devices.status.as_ref()?;
        if let Some(problem) = &status.problem {
            return Some(self.problem_text(problem));
        }
        status.error.clone()
    }

    /// The cloud folders the picker offers, in its order: the services by name, then a
    /// WebDAV server and a folder on this computer. A drive this build cannot sign in to is
    /// left out.
    pub fn cloud_choices(&self) -> Vec<SyncFolderKind> {
        let mut clouds: Vec<SyncFolderKind> = self
            .devices
            .status
            .iter()
            .flat_map(|s| &s.clouds)
            .filter(|c| !matches!(c.way, SyncCloudWay::Unavailable { .. }))
            .map(|c| c.service.clone())
            .collect();
        clouds.push(SyncFolderKind::Nextcloud);
        clouds.sort_by_key(|kind| service_name(kind).to_lowercase());
        clouds.dedup();
        clouds.push(SyncFolderKind::WebDav);
        clouds.push(SyncFolderKind::Directory);
        clouds
    }

    /// The name of the folder's service in a line: the one being set up, else the one in use.
    fn folder_service(&self) -> String {
        self.devices
            .kind
            .clone()
            .or_else(|| self.folder_kind())
            .map_or_else(String::new, |k| service_name(&k))
    }

    /// A sync problem in words: inside a pairing, a sign-in, or about the folder.
    pub fn problem_text(&self, problem: &SyncProblem) -> String {
        let pairing = self.devices.pairing_with.is_some()
            || matches!(
                self.nav.overlay(),
                Some(Overlay::SyncPairAddress | Overlay::SyncPairCode | Overlay::SyncPairShow)
            );
        match problem {
            SyncProblem::WrongRecoveryKey => t::rkey_wrong(),
            SyncProblem::NoGroup => t::sync_tui_err_no_group(),
            SyncProblem::GroupExists => t::cloud_err_other_group(),
            SyncProblem::GroupReplaced => t::cloud_err_started_over(&self.folder_service()),
            SyncProblem::ClearRefused => t::rkey_start_over_err_refused(&self.folder_service()),
            SyncProblem::Unreachable if pairing => t::pair_err_not_found(),
            SyncProblem::Unreachable => {
                match self.devices.kind.clone().or_else(|| self.folder_kind()) {
                    Some(SyncFolderKind::Directory) => t::cloud_err_folder_gone(),
                    Some(kind) => t::cloud_err_offline(&service_name(&kind)),
                    None => t::pair_err_not_found(),
                }
            }
            SyncProblem::SignInNeeded { service } => {
                t::cloud_err_signed_out(&service_name(service), &t::sync_tui_kind())
            }
            SyncProblem::SignInDenied { service } => {
                t::cloud_err_denied(&service_name(service))
            }
            SyncProblem::SignInExpired { .. } => t::cloud_err_timeout(),
            SyncProblem::CloudUnavailable { service, .. } => {
                t::sync_tui_err_unavailable(&service_name(service))
            }
            SyncProblem::Other(text) => text.clone(),
        }
    }

    /// Enter on a row of the Devices screen.
    fn enter_device_line(&mut self) -> Option<Command> {
        let line = self.device_line()?;
        self.devices.problem = None;
        match line {
            DeviceLine::Name => {
                let name = self.device_name();
                self.set_text(Field::DeviceName, name);
                self.open_sync_overlay(Overlay::SyncName);
            }
            DeviceLine::TurnOn => self.open_sync_overlay(Overlay::SyncRoad),
            DeviceLine::Device(i) => {
                let id = self.devices.status.as_ref()?.devices.get(i)?.id.clone();
                self.devices.device_page = Some(id);
                self.open_sync_overlay(Overlay::SyncDevice);
            }
            DeviceLine::Note(i) => {
                let note = self.devices.status.as_ref()?.notes.get(i)?;
                self.commands.push(Command::SetWatched {
                    id: kaeri_core::ids::AnilistId::new(note.anilist_id),
                    episodes: vec![note.episode.clone()],
                    watched: false,
                });
                return Some(Command::DismissSyncNote { id: note.id });
            }
            DeviceLine::NewKey => {
                self.devices.confirm = Some(DeviceConfirm::NewKey);
                self.open_sync_overlay(Overlay::SyncConfirm);
            }
            DeviceLine::ShowCode => return self.show_pair_code(),
            DeviceLine::EnterCode => {
                self.field_mut(Field::PairCode).clear();
                self.field_mut(Field::PairAddress).clear();
                self.open_sync_overlay(Overlay::SyncPairCode);
            }
            DeviceLine::AddCloud => self.open_sync_overlay(Overlay::SyncClouds),
            DeviceLine::EnterNewKey => self.enter_new_key(),
            DeviceLine::Folder if self.group_replaced() => self.enter_new_key(),
            DeviceLine::Folder => {
                let kind = self.folder_kind()?;
                return match self.folder_state() {
                    FolderState::SignedOut if is_drive(&kind) => self.sign_in_again(kind),
                    FolderState::SettingUp | FolderState::Syncing => None,
                    _ => Some(Command::SyncFolderNow),
                };
            }
            DeviceLine::SignOut => {
                return Some(Command::SyncSignOut { service: self.signed_in_drive()? });
            }
            DeviceLine::RecoveryKey => return Some(Command::ShowRecoveryKey),
            DeviceLine::TurnOff => {
                self.devices.confirm = Some(DeviceConfirm::TurnOff);
                self.open_sync_overlay(Overlay::SyncConfirm);
            }
            DeviceLine::Profile(i) => {
                let profile = self.devices.profiles.get(i)?;
                return (!profile.active)
                    .then(|| Command::SwitchProfile { id: profile.id.clone() });
            }
            DeviceLine::AddProfile => {
                self.field_mut(Field::ProfileName).clear();
                self.open_sync_overlay(Overlay::ProfileName);
            }
        }
        None
    }

    /// The restore overlay for the folder in use, to join the group another device started
    /// over there.
    fn enter_new_key(&mut self) {
        let Some(folder) = self.devices.status.as_ref().and_then(|s| s.folder.clone()) else {
            return;
        };
        let address =
            (!is_drive(&folder.kind) && !folder.label.is_empty()).then_some(folder.label);
        self.forget_sync_form();
        self.devices.kind = Some(folder.kind.clone());
        self.devices.pending =
            Some(SyncFolderSpec { kind: folder.kind, address, user: None, password: None });
        self.open_restore();
    }

    /// The restore overlay, its field empty and with the keyboard.
    fn open_restore(&mut self) {
        self.close_sync_overlays(None);
        self.field_mut(Field::RecoveryKey).clear();
        self.devices.problem = None;
        self.devices.start_over_focused = false;
        self.open_sync_overlay(Overlay::SyncRestore);
    }

    /// One of the screen's questions on top, at its first choice.
    fn open_sync_overlay(&mut self, overlay: Overlay) {
        self.cursors.overlay = Cursor::default();
        self.nav.open_overlay(overlay);
    }

    /// The question on top answered by the next one.
    fn next_sync_overlay(&mut self, overlay: Overlay) {
        self.nav.close_overlay();
        self.open_sync_overlay(overlay);
    }

    /// Close every question of the screen's that is open, keeping `keep`.
    fn close_sync_overlays(&mut self, keep: Option<&Overlay>) {
        while let Some(top) = self.nav.overlay() {
            if !top.is_devices() || Some(top) == keep {
                break;
            }
            self.nav.close_overlay();
        }
    }

    /// Forget what a form held: its fields, its problem, and what it was waiting on.
    pub(super) fn forget_sync_form(&mut self) {
        for field in [
            Field::SyncAddress,
            Field::SyncUser,
            Field::SyncPassword,
            Field::RecoveryKey,
            Field::PairCode,
            Field::PairAddress,
            Field::ProfileName,
        ] {
            self.field_mut(field).clear();
        }
        self.devices.problem = None;
        self.devices.working = false;
        self.devices.pending = None;
        self.devices.kind = None;
        self.devices.pairing_with = None;
        self.devices.pair_failed = None;
    }

    /// The ids of the devices in the group now.
    fn known_devices(&self) -> Vec<String> {
        self.devices.status.iter().flat_map(|s| &s.devices).map(|d| d.id.clone()).collect()
    }

    /// Show a pairing code: asked of the engine, with the overlay up while it is made.
    fn show_pair_code(&mut self) -> Option<Command> {
        self.devices.invite = None;
        self.devices.problem = None;
        self.devices.pairing_with = Some(self.known_devices());
        self.close_sync_overlays(None);
        self.open_sync_overlay(Overlay::SyncPairShow);
        Some(Command::PairInvite { device_name: self.device_name() })
    }

    /// Sign in to `kind` again, waiting on the browser.
    fn sign_in_again(&mut self, kind: SyncFolderKind) -> Option<Command> {
        self.devices.sign_in = Some((kind.clone(), None));
        self.open_sync_overlay(Overlay::SyncWait);
        Some(Command::SyncSignIn { service: kind })
    }

    /// Set sync up in `folder`, or move it there when sync is already on.
    fn start_folder(&mut self, folder: SyncFolderSpec) -> Option<Command> {
        self.devices.working = true;
        self.devices.problem = None;
        self.devices.pending = Some(SyncFolderSpec { password: None, ..folder.clone() });
        Some(Command::StartSync { device_name: self.device_name(), folder })
    }

    /// The keys of the screen's questions that are not text fields. `Some` when one was taken.
    ///
    /// Asked ahead of every overlay's shared keys, because leaving some of them says something
    /// to the engine: Esc stops a sign-in or a pairing code, and forgets the recovery key.
    pub(super) fn devices_overlay_key(
        &mut self,
        overlay: &Overlay,
        action: Action,
    ) -> Option<Option<Command>> {
        if !overlay.is_devices() || matches!(action, Action::Help | Action::Quit) {
            return None;
        }
        Some(match (overlay, action) {
            (Overlay::SyncWait, Action::Back) => {
                self.nav.close_overlay();
                self.devices.sign_in = None;
                self.forget_sync_form();
                Some(Command::CancelSyncSignIn)
            }
            (Overlay::SyncWait, Action::Open) => {
                self.wait_url().map(|url| Command::OpenExternal { url })
            }
            // Nextcloud can be signed in to with an app password instead of the page.
            (Overlay::SyncWait, Action::SetListStatus)
                if self.waiting_for() == Some(SyncFolderKind::Nextcloud) =>
            {
                self.nav.close_overlay();
                self.devices.sign_in = None;
                self.devices.working = false;
                self.devices.kind = Some(SyncFolderKind::Nextcloud);
                self.field_mut(Field::SyncUser).clear();
                self.open_sync_overlay(Overlay::SyncUser);
                Some(Command::CancelSyncSignIn)
            }
            (Overlay::SyncRecoveryKey, Action::Back | Action::Open) => {
                self.nav.close_overlay();
                self.devices.recovery_key = None;
                self.devices.key_copied = false;
                self.devices.show_qr = false;
                self.devices.started_over = None;
                None
            }
            (Overlay::SyncStartOver, Action::Open) => self.start_over_again(),
            (Overlay::SyncStartOver, Action::Back) => {
                self.leave_start_over();
                None
            }
            (Overlay::SyncRecoveryKey, Action::ClearCompleted) => {
                self.clipboard = self.devices.recovery_key.as_deref().map(key_groups);
                None
            }
            (Overlay::SyncRecoveryKey, Action::PlayTrailer) => {
                self.devices.show_qr = !self.devices.show_qr;
                None
            }
            (Overlay::SyncPairShow, Action::Back) => {
                self.nav.close_overlay();
                self.devices.invite = None;
                self.devices.pairing_with = None;
                Some(Command::PairStop)
            }
            (Overlay::SyncPairShow, Action::Open) if self.pair_code_expired() => {
                self.show_pair_code()
            }
            (Overlay::SyncConfirm, Action::Open) => self.confirm_device(),
            (Overlay::SyncConfirm, Action::Back) => {
                self.nav.close_overlay();
                self.devices.confirm = None;
                None
            }
            (Overlay::SyncDevice, Action::Open) => self.choose_on_device_page(),
            (Overlay::SyncRoad | Overlay::SyncNearby | Overlay::SyncClouds, Action::Open)
            | (Overlay::ProfileKind, Action::Open) => self.choose_in_sync_overlay(overlay),
            // The rest of the list questions move as every list overlay does.
            (
                Overlay::SyncRoad
                | Overlay::SyncNearby
                | Overlay::SyncClouds
                | Overlay::SyncDevice
                | Overlay::ProfileKind,
                _,
            ) => return None,
            _ => None,
        })
    }

    /// The device whose page is open.
    pub fn page_device(&self) -> Option<&SyncDeviceView> {
        let id = self.devices.device_page.as_deref()?;
        self.devices.status.as_ref()?.devices.iter().find(|d| d.id == id)
    }

    /// The choices on the open device's page, in order.
    pub fn device_page_items(&self) -> Vec<DevicePageItem> {
        let Some(device) = self.page_device() else { return Vec::new() };
        let mut items = vec![DevicePageItem::Sync];
        if device.remote_key.is_some() {
            items.push(DevicePageItem::Remote);
        }
        if !device.trackers.is_empty() || !device.sign_ins.is_empty() {
            items.push(DevicePageItem::CopySignIns);
        }
        items.push(DevicePageItem::Remove);
        items
    }

    /// Enter on a choice of a device's page.
    fn choose_on_device_page(&mut self) -> Option<Command> {
        let item = *self.device_page_items().get(self.cursors.overlay.selected)?;
        let device = self.page_device()?.clone();
        match item {
            DevicePageItem::Sync => Some(Command::SyncGroup(SyncCommand::SetDeviceSync {
                device: device.id,
                on: device.paused,
            })),
            DevicePageItem::Remote => Some(Command::SyncGroup(SyncCommand::SetDeviceRemote {
                device: device.id,
                on: device.remote_off,
            })),
            DevicePageItem::CopySignIns => {
                let mut copy = vec![SyncCopy::Trackers];
                copy.extend(
                    device.sign_ins.iter().cloned().map(|service| SyncCopy::Cloud { service }),
                );
                Some(Command::SyncGroup(SyncCommand::CopySignIns {
                    device: Some(device.id),
                    copy: if device.asked.is_empty() { copy } else { Vec::new() },
                    address: None,
                }))
            }
            DevicePageItem::Remove => {
                self.devices.confirm =
                    Some(DeviceConfirm::Remove { id: device.id, name: device.name });
                self.next_sync_overlay(Overlay::SyncConfirm);
                None
            }
        }
    }

    /// The service the browser is open for.
    pub fn waiting_for(&self) -> Option<SyncFolderKind> {
        self.devices.sign_in.as_ref().map(|(kind, _)| kind.clone())
    }

    /// The page the browser was sent to, to open again.
    pub fn wait_url(&self) -> Option<String> {
        match self.devices.sign_in.as_ref()?.1.as_ref()? {
            SyncSignInStep::Browser { url }
            | SyncSignInStep::Sheet { url, .. }
            | SyncSignInStep::Page { url } => Some(url.clone()),
            SyncSignInStep::Code { verification_url, .. } => Some(verification_url.clone()),
            SyncSignInStep::Finishing
            | SyncSignInStep::Connecting
            | SyncSignInStep::Clearing => None,
        }
    }

    /// Seconds the code on show has left, never below nought.
    pub fn pair_code_left(&self) -> Option<i64> {
        let invite = self.devices.invite.as_ref()?;
        Some((invite.until - self.clock.now_epoch()).max(0))
    }

    /// Whether the code on show has run out.
    pub fn pair_code_expired(&self) -> bool {
        self.pair_code_left() == Some(0)
    }

    /// How many choices a list question of the screen's has.
    pub(super) fn devices_overlay_len(&self, overlay: &Overlay) -> usize {
        match overlay {
            Overlay::SyncRoad | Overlay::SyncNearby | Overlay::ProfileKind => 2,
            Overlay::SyncClouds => self.cloud_choices().len(),
            Overlay::SyncDevice => self.device_page_items().len(),
            _ => 0,
        }
    }

    /// Enter on a list question: the road, the nearby choice, the cloud, the profile's kind.
    fn choose_in_sync_overlay(&mut self, overlay: &Overlay) -> Option<Command> {
        let at = self.cursors.overlay.selected;
        match overlay {
            Overlay::SyncRoad if at == 0 => self.next_sync_overlay(Overlay::SyncNearby),
            Overlay::SyncRoad => self.next_sync_overlay(Overlay::SyncClouds),
            Overlay::SyncNearby if at == 0 => return self.show_pair_code(),
            Overlay::SyncNearby => {
                self.field_mut(Field::PairCode).clear();
                self.field_mut(Field::PairAddress).clear();
                self.next_sync_overlay(Overlay::SyncPairCode);
            }
            Overlay::SyncClouds => {
                let kind = self.cloud_choices().get(at)?.clone();
                self.devices.kind = Some(kind.clone());
                self.devices.problem = None;
                if is_drive(&kind) {
                    self.close_sync_overlays(None);
                    self.devices.sign_in = Some((kind.clone(), None));
                    self.open_sync_overlay(Overlay::SyncWait);
                    return self.start_folder(SyncFolderSpec {
                        kind,
                        address: None,
                        user: None,
                        password: None,
                    });
                }
                self.field_mut(Field::SyncAddress).clear();
                self.next_sync_overlay(Overlay::SyncAddress);
            }
            Overlay::ProfileKind => {
                let name = self.field_mut(Field::ProfileName).take().trim().to_owned();
                self.nav.close_overlay();
                if name.is_empty() {
                    return None;
                }
                return Some(Command::AddProfile { name, kids: at == 1, first_name: None });
            }
            _ => {}
        }
        None
    }

    /// Enter on the confirm overlay: the device removed, or sync turned off here.
    fn confirm_device(&mut self) -> Option<Command> {
        self.nav.close_overlay();
        match self.devices.confirm.take()? {
            DeviceConfirm::Remove { id, .. } => Some(Command::RemoveSyncDevice { device: id }),
            DeviceConfirm::TurnOff => Some(Command::LeaveSync),
            DeviceConfirm::NewKey => Some(Command::NewRecoveryKey),
            DeviceConfirm::StartOver => self.start_over(),
        }
    }

    /// Start over without the key in the folder the restore overlay is for. The restore content
    /// stays until the work has run a moment.
    fn start_over(&mut self) -> Option<Command> {
        let folder = self.devices.pending.clone()?;
        let now = self.clock.now_instant();
        self.devices.problem = None;
        self.devices.sign_in = None;
        self.devices.start_over = Some(StartOver {
            service: folder.kind.clone(),
            folder: folder.clone(),
            began: now,
            said: StartOverStep::Asked,
            shown: StartOverStep::Asked,
            since: now,
            key: None,
        });
        Some(Command::SyncGroup(SyncCommand::ClearFolderAndStart {
            device_name: self.device_name(),
            folder,
        }))
    }

    /// Try again, on a start over's error: the error stays until the work has run a moment.
    fn start_over_again(&mut self) -> Option<Command> {
        let device_name = self.device_name();
        let now = self.clock.now_instant();
        let so = self.devices.start_over.as_mut()?;
        if !matches!(so.said, StartOverStep::Failed(_)) {
            return None;
        }
        so.said = StartOverStep::Asked;
        so.began = now;
        so.key = None;
        Some(Command::SyncGroup(SyncCommand::ClearFolderAndStart {
            device_name,
            folder: so.folder.clone(),
        }))
    }

    /// Esc on a start over. Closed while the work runs, it carries on and the key opens by
    /// itself. Cancelled on an error, the start over is let go and the restore overlay is back
    /// with its key field, for the same folder.
    fn leave_start_over(&mut self) {
        let failed =
            self.devices.start_over.take_if(|so| matches!(so.said, StartOverStep::Failed(_)));
        match (failed, self.devices.start_over.is_some()) {
            (Some(so), _) => {
                self.devices.kind = Some(so.service);
                self.devices.pending = Some(so.folder);
                self.open_restore();
            }
            (None, true) => self.nav.close_overlay(),
            (None, false) => {
                self.nav.close_overlay();
                self.forget_sync_form();
            }
        }
    }

    /// What the engine said about a start over, kept until it is due on screen.
    fn start_over_said(&mut self, step: StartOverStep) {
        if let Some(so) = self.devices.start_over.as_mut() {
            so.said = step;
        }
        self.advance_start_over();
    }

    /// The idle tick: a start over's next step, once it is due.
    pub fn tick_start_over(&mut self) {
        self.advance_start_over();
    }

    /// Whether a start over has a step waiting for its time, or a spinner turning.
    fn start_over_moving(&self) -> bool {
        self.devices.start_over.as_ref().is_some_and(|so| {
            so.held() || matches!(so.shown, StartOverStep::Clearing | StartOverStep::SettingUp)
        })
    }

    /// Put the step the engine said on show when it is due: the work after
    /// [`START_OVER_QUIET`], each step after [`START_OVER_HOLD`] on show, and an end at once
    /// when nothing of the work has shown.
    fn advance_start_over(&mut self) {
        let now = self.clock.now_instant();
        let Some(so) = self.devices.start_over.as_ref() else { return };
        let ended = matches!(so.said, StartOverStep::Done | StartOverStep::Failed(_));
        let next = if so.held() {
            if ended {
                so.said.clone()
            } else if now.saturating_duration_since(so.began) >= START_OVER_QUIET {
                match &so.said {
                    StartOverStep::Asked => StartOverStep::Clearing,
                    said => said.clone(),
                }
            } else {
                return;
            }
        } else if so.said != so.shown
            && now.saturating_duration_since(so.since) >= START_OVER_HOLD
        {
            so.said.clone()
        } else {
            return;
        };
        self.show_start_over(next, now);
    }

    /// Put `step` on show, in the overlay the restore began, unless it was closed.
    fn show_start_over(&mut self, step: StartOverStep, now: Instant) {
        let ours =
            matches!(self.nav.overlay(), Some(Overlay::SyncRestore | Overlay::SyncStartOver));
        match step {
            StartOverStep::Done => {
                let Some(so) = self.devices.start_over.take() else { return };
                match so.key {
                    Some(key) => {
                        self.apply_recovery_key(key);
                        self.devices.started_over = Some(so.service);
                    }
                    None => {
                        if ours {
                            self.close_sync_overlays(None);
                        }
                        self.forget_sync_form();
                    }
                }
            }
            step => {
                let failed = matches!(step, StartOverStep::Failed(_));
                let Some(so) = self.devices.start_over.as_mut() else { return };
                so.shown = step;
                so.since = now;
                if self.nav.overlay() == Some(&Overlay::SyncRestore) {
                    self.next_sync_overlay(Overlay::SyncStartOver);
                } else if failed && !ours {
                    // Closed while it worked, an error opens it again with Try again, as the
                    // key would have.
                    let (kind, folder) = (so.service.clone(), so.folder.clone());
                    self.devices.kind = Some(kind);
                    self.devices.pending = Some(folder);
                    self.close_sync_overlays(None);
                    self.open_sync_overlay(Overlay::SyncStartOver);
                }
            }
        }
    }

    /// Keys the restore overlay reads before its field: Tab between the key and Start over,
    /// and a letter typed on Start over going back to the key. `Some` when one was taken.
    pub(super) fn restore_key(
        &mut self,
        key: &crossterm::event::KeyEvent,
    ) -> Option<Option<Command>> {
        use crossterm::event::KeyCode;
        if self.nav.overlay() != Some(&Overlay::SyncRestore)
            || self.devices.working
            || self.devices.start_over.is_some()
        {
            return None;
        }
        let on_start_over = self.devices.start_over_focused;
        match key.code {
            KeyCode::Tab | KeyCode::BackTab => self.devices.start_over_focused = !on_start_over,
            KeyCode::Down if !on_start_over => self.devices.start_over_focused = true,
            KeyCode::Up if on_start_over => self.devices.start_over_focused = false,
            KeyCode::Char(_) | KeyCode::Backspace if on_start_over => {
                self.devices.start_over_focused = false;
                return None;
            }
            _ => return None,
        }
        Some(None)
    }

    /// Enter in one of the screen's text fields. An empty one stays put.
    pub(super) fn submit_devices_field(&mut self) -> Option<Command> {
        let overlay = self.nav.overlay()?.clone();
        if overlay == Overlay::SyncRestore && self.devices.start_over_focused {
            if !self.devices.working && self.devices.start_over.is_none() {
                self.devices.confirm = Some(DeviceConfirm::StartOver);
                self.open_sync_overlay(Overlay::SyncConfirm);
            }
            return None;
        }
        let field = self.focused_field()?;
        let text = self.text(field).trim().to_owned();
        if text.is_empty() || self.devices.working {
            return None;
        }
        self.devices.problem = None;
        match overlay {
            Overlay::SyncAddress => match self.devices.kind.clone()? {
                SyncFolderKind::Nextcloud => {
                    self.close_sync_overlays(None);
                    self.devices.sign_in = Some((SyncFolderKind::Nextcloud, None));
                    self.open_sync_overlay(Overlay::SyncWait);
                    self.start_folder(SyncFolderSpec {
                        kind: SyncFolderKind::Nextcloud,
                        address: Some(text),
                        user: None,
                        password: None,
                    })
                }
                SyncFolderKind::WebDav => {
                    self.field_mut(Field::SyncUser).clear();
                    self.next_sync_overlay(Overlay::SyncUser);
                    None
                }
                kind => self.start_folder(SyncFolderSpec {
                    kind,
                    address: Some(text),
                    user: None,
                    password: None,
                }),
            },
            Overlay::SyncUser => {
                self.field_mut(Field::SyncPassword).clear();
                self.next_sync_overlay(Overlay::SyncPassword);
                None
            }
            Overlay::SyncPassword => {
                let kind = self.devices.kind.clone()?;
                let password = Secret(self.field_mut(Field::SyncPassword).take());
                let address = self.text(Field::SyncAddress).trim().to_owned();
                let user = self.text(Field::SyncUser).trim().to_owned();
                self.start_folder(SyncFolderSpec {
                    kind,
                    address: Some(address),
                    user: Some(user),
                    password: Some(password),
                })
            }
            Overlay::SyncRestore => {
                let folder = self.devices.pending.clone()?;
                let key = key_groups(&self.field_mut(Field::RecoveryKey).take());
                self.devices.working = true;
                Some(Command::JoinSync {
                    device_name: self.device_name(),
                    folder,
                    recovery_key: Secret(key),
                })
            }
            Overlay::SyncPairCode => {
                if text.len() != 6 {
                    return None;
                }
                self.field_mut(Field::PairAddress).clear();
                self.next_sync_overlay(Overlay::SyncPairAddress);
                None
            }
            Overlay::SyncPairAddress => {
                self.devices.working = true;
                self.devices.pairing_with = Some(self.known_devices());
                Some(Command::PairJoin {
                    device_name: self.device_name(),
                    address: text,
                    code: self.text(Field::PairCode).to_owned(),
                })
            }
            Overlay::SyncName => {
                self.nav.close_overlay();
                self.field_mut(Field::DeviceName).clear();
                self.devices.this_name.clone_from(&text);
                Some(Command::SyncGroup(SyncCommand::RenameDevice { name: text }))
            }
            Overlay::ProfileName => {
                self.next_sync_overlay(Overlay::ProfileKind);
                None
            }
            _ => None,
        }
    }

    /// A pairing by code missed: the words wait for the problem that follows.
    pub(super) fn apply_pair_failed(
        &mut self,
        problem: SyncPairProblem,
        device: Option<String>,
    ) {
        self.devices.pair_failed = Some(match problem {
            SyncPairProblem::WrongCode => {
                t::pair_err_code(&device.unwrap_or_else(t::pair_other_device))
            }
            SyncPairProblem::NotFound => t::pair_err_not_found(),
        });
    }

    /// Titles for the sync notes, which name them by id.
    pub(super) fn apply_note_titles(&mut self, entries: &[Entry]) {
        for entry in entries {
            self.devices.note_titles.insert(entry.id.get(), entry.title.clone());
        }
        let last = self.device_lines().len().saturating_sub(1);
        self.cursors.devices.selected = self.cursors.devices.selected.min(last);
    }

    /// How sync stands now: a folder set up closes its form, and a pairing that added a device
    /// closes the code and says who.
    pub(super) fn apply_sync_status(&mut self, status: SyncStatus) {
        let paired = self.devices.pairing_with.as_ref().and_then(|known| {
            status
                .devices
                .iter()
                .find(|d| !d.this_device && !known.contains(&d.id))
                .map(|d| d.name.clone())
        });
        let folder_up = status.enabled && status.folder.is_some();
        // A start over closes its own overlay, once its key has had its turn.
        if folder_up && self.devices.start_over.is_none() {
            self.devices.setting_up = false;
            if self.devices.pending.is_some() && self.devices.recovery_key.is_none() {
                self.close_sync_overlays(Some(&Overlay::SyncRecoveryKey));
            }
            if self.devices.pending.is_some() {
                self.forget_sync_form();
            }
        }
        if let Some(name) = paired {
            self.close_sync_overlays(None);
            self.devices.invite = None;
            self.forget_sync_form();
            self.push_toast(info(t::pair_ok(&name)));
        }
        let unasked: Vec<AnilistId> = status
            .notes
            .iter()
            .map(|n| n.anilist_id)
            .filter(|id| !self.devices.note_titles.contains_key(id))
            .filter(|id| self.devices.titles_asked.insert(*id))
            .map(AnilistId::new)
            .collect();
        if !unasked.is_empty() {
            self.commands.push(Command::LoadCards { ids: unasked });
        }
        self.devices.status = Some(status);
        let last = self.device_lines().len().saturating_sub(1);
        self.cursors.devices.selected = self.cursors.devices.selected.min(last);
    }

    /// A pairing code is on show.
    pub(super) fn apply_sync_invite(&mut self, code: String, port: u16, seconds: u32) {
        let until = self.clock.now_epoch() + i64::from(seconds);
        self.devices.invite = Some(PairCode { code, port, until });
    }

    /// The recovery key, to show once.
    pub(super) fn apply_recovery_key(&mut self, key: String) {
        if let Some(so) = self.devices.start_over.as_mut() {
            so.key = Some(key);
            self.start_over_said(StartOverStep::Done);
            return;
        }
        self.devices.started_over = None;
        self.close_sync_overlays(None);
        self.devices.recovery_key = Some(key);
        self.devices.key_copied = false;
        self.devices.show_qr = false;
        self.devices.setting_up = false;
        self.forget_sync_form();
        self.open_sync_overlay(Overlay::SyncRecoveryKey);
    }

    /// A cloud sign-in moved on.
    pub(super) fn apply_sync_sign_in(
        &mut self,
        service: SyncFolderKind,
        step: Option<SyncSignInStep>,
    ) {
        // A start over says its steps as a sign-in's. The sign-in that found the other group can
        // end after it began, and says nothing about it.
        if let Some(so) = self.devices.start_over.as_ref() {
            let said = match step {
                Some(SyncSignInStep::Clearing) => StartOverStep::Clearing,
                Some(SyncSignInStep::Finishing) => StartOverStep::SettingUp,
                // The end, with no key made: this device brought its own group along.
                None if matches!(
                    so.said,
                    StartOverStep::Clearing | StartOverStep::SettingUp
                ) =>
                {
                    StartOverStep::Done
                }
                _ => {
                    self.devices.sign_in = None;
                    return;
                }
            };
            self.devices.sign_in = None;
            self.start_over_said(said);
            return;
        }
        let waiting = self.nav.overlay() == Some(&Overlay::SyncWait);
        match step {
            None | Some(SyncSignInStep::Finishing) => {
                if self.devices.sign_in.take().is_some() && waiting {
                    self.nav.close_overlay();
                }
                if step.is_some() && self.devices.pending.is_some() {
                    self.devices.setting_up = true;
                }
            }
            Some(step) => {
                if self.devices.sign_in.is_none() {
                    return;
                }
                self.devices.sign_in = Some((service, Some(step)));
                if !waiting {
                    self.close_sync_overlays(None);
                    self.open_sync_overlay(Overlay::SyncWait);
                }
            }
        }
    }

    /// A set-up, join or pairing did not work. A folder that holds another group asks for its
    /// recovery key; the rest are said in the question that asked, or as a toast.
    pub(super) fn apply_sync_problem(&mut self, problem: SyncProblem) {
        self.devices.working = false;
        self.devices.setting_up = false;
        if let Some(so) = self.devices.start_over.as_ref() {
            let service = service_name(&so.service);
            let text = match &problem {
                SyncProblem::Unreachable if so.service == SyncFolderKind::Directory => {
                    t::cloud_err_folder_gone()
                }
                SyncProblem::Unreachable => t::cloud_err_offline(&service),
                SyncProblem::ClearRefused => t::rkey_start_over_err_refused(&service),
                other => self.problem_text(other),
            };
            self.start_over_said(StartOverStep::Failed(text));
            return;
        }
        if problem == SyncProblem::GroupExists && self.devices.pending.is_some() {
            self.open_restore();
            return;
        }
        let text =
            self.devices.pair_failed.take().unwrap_or_else(|| self.problem_text(&problem));
        if self.nav.overlay() == Some(&Overlay::SyncWait) {
            self.nav.close_overlay();
            self.devices.sign_in = None;
        }
        let asked_here = self.nav.overlay().is_some_and(|o| {
            o.is_devices() && (o.takes_text_input() || *o == Overlay::SyncPairShow)
        });
        if asked_here {
            self.devices.problem = Some(text);
        } else {
            self.devices.pending = None;
            self.devices.pairing_with = None;
            self.push_toast(alert(text));
        }
    }

    /// The profiles now. In a kids profile Devices is hidden, so it closes.
    pub(super) fn apply_profiles(&mut self, view: ProfilesView) {
        self.devices.profiles = view.profiles;
        if self.in_kids_profile() && self.in_devices() {
            self.close_sync_overlays(None);
            self.nav.pop();
            self.push_toast(info(t::profiles_kids_hidden()));
        }
        let last = self.device_lines().len().saturating_sub(1);
        self.cursors.devices.selected = self.cursors.devices.selected.min(last);
    }

    /// A profile was chosen: the terminal closes, to start again on it.
    pub(super) fn apply_profile_switch(&mut self, id: String) {
        let name =
            self.devices.profiles.iter().find(|p| p.id == id).map_or(id, |p| p.name.clone());
        self.devices.restart_as = Some(name);
        self.should_quit = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::fixtures::*;
    use kaeri_model::{
        event::Event,
        group::{CloudUnavailable, SyncCloudView, SyncDeviceView, SyncFolderView},
    };

    fn device(id: &str, name: &str, this_device: bool) -> SyncDeviceView {
        SyncDeviceView {
            id: id.into(),
            name: name.into(),
            this_device,
            last_seen: None,
            trackers: Vec::new(),
            sign_ins: Vec::new(),
            folder: None,
            asked: Vec::new(),
            kind: None,
            paused: false,
            remote_key: None,
            remote_off: false,
        }
    }

    fn cloud(service: SyncFolderKind, way: SyncCloudWay, signed_in: bool) -> SyncCloudView {
        SyncCloudView { service, way, signed_in, account: None }
    }

    fn clouds() -> Vec<SyncCloudView> {
        vec![
            cloud(SyncFolderKind::GoogleDrive, SyncCloudWay::Browser, false),
            cloud(
                SyncFolderKind::Dropbox,
                SyncCloudWay::Unavailable { reason: CloudUnavailable::NoClientId },
                false,
            ),
            cloud(SyncFolderKind::OneDrive, SyncCloudWay::Browser, false),
        ]
    }

    fn off() -> SyncStatus {
        SyncStatus { clouds: clouds(), ..SyncStatus::default() }
    }

    fn on(folder: Option<SyncFolderKind>) -> SyncStatus {
        SyncStatus {
            enabled: true,
            device_name: Some("Kim's laptop".into()),
            folder: folder.map(|kind| SyncFolderView {
                kind,
                label: "cloud".into(),
                account: None,
            }),
            devices: vec![
                device("a", "Kim's laptop", true),
                device("b", "Kim's iPhone", false),
            ],
            clouds: clouds(),
            recovery_key: true,
            ..SyncStatus::default()
        }
    }

    /// Settings, then `d`: Devices, asking how sync and the profiles stand.
    fn at_devices(status: SyncStatus) -> App {
        let mut a = app();
        // Stopped, so a pairing code's time left is the same however long the test takes.
        a.clock = crate::clock::Clock::fixed(1_793_000_000, crate::clock::Zone::utc());
        a.set_device_name("Kim's laptop".into());
        a.handle(Action::JumpSection(9), 5);
        assert_eq!(a.handle(Action::Download, 5), Some(Command::LoadSync));
        assert_eq!(a.take_commands(), vec![Command::LoadProfiles]);
        assert!(a.in_devices());
        a.apply(Event::SyncStatus(status));
        // On the first row under the name, where these tests start.
        a.cursors.devices.selected = 1;
        a
    }

    fn type_text(a: &mut App, text: &str) {
        text.chars().for_each(|c| a.type_char(c));
    }

    #[test]
    fn settings_d_and_the_palette_open_devices() {
        let mut a = at_devices(off());
        assert_eq!(a.device_lines(), vec![DeviceLine::Name, DeviceLine::TurnOn]);
        a.handle(Action::Back, 5);
        assert_eq!(a.nav.current(), &StageView::Section(Section::Settings));

        let mut b = app();
        assert_eq!(b.handle(Action::ShowDevices, 5), Some(Command::LoadSync));
        assert!(b.in_devices());
        assert_eq!(b.nav.section(), Section::Settings);
    }

    #[test]
    fn the_cloud_picker_hides_what_this_build_cannot_sign_in_to() {
        let a = at_devices(off());
        assert_eq!(
            a.cloud_choices(),
            vec![
                SyncFolderKind::GoogleDrive,
                SyncFolderKind::Nextcloud,
                SyncFolderKind::OneDrive,
                SyncFolderKind::WebDav,
                SyncFolderKind::Directory,
            ]
        );
    }

    #[test]
    fn a_cloud_drive_waits_on_the_browser_and_esc_cancels_it() {
        let mut a = at_devices(off());
        a.handle(Action::Open, 5);
        assert_eq!(a.nav.overlay(), Some(&Overlay::SyncRoad));
        a.handle(Action::Down, 5);
        a.handle(Action::Open, 5);
        assert_eq!(a.nav.overlay(), Some(&Overlay::SyncClouds));
        let task = a.handle(Action::Open, 5);
        let Some(Command::StartSync { device_name, folder }) = task else { panic!("{task:?}") };
        assert_eq!(device_name, "Kim's laptop");
        assert_eq!(folder.kind, SyncFolderKind::GoogleDrive);
        assert_eq!(a.nav.overlay(), Some(&Overlay::SyncWait));

        a.apply(Event::SyncSignIn {
            service: SyncFolderKind::GoogleDrive,
            step: Some(SyncSignInStep::Browser { url: "https://auth.example/x".into() }),
        });
        assert_eq!(
            a.handle(Action::Open, 5),
            Some(Command::OpenExternal { url: "https://auth.example/x".into() }),
            "Enter opens the page again"
        );
        assert_eq!(a.handle(Action::Back, 5), Some(Command::CancelSyncSignIn));
        assert!(!a.nav.has_overlay());
        assert!(a.devices.pending.is_none());
    }

    #[test]
    fn signed_in_the_folder_is_set_up_and_the_key_shown_once() {
        let mut a = at_devices(off());
        a.handle(Action::Open, 5);
        a.handle(Action::Down, 5);
        a.handle(Action::Open, 5);
        a.handle(Action::Open, 5);
        a.apply(Event::SyncSignIn {
            service: SyncFolderKind::GoogleDrive,
            step: Some(SyncSignInStep::Finishing),
        });
        assert!(!a.nav.has_overlay());
        assert!(a.devices.setting_up);
        a.apply(Event::SyncRecoveryKey("ABCD2345EFGH6789JKLM2345".into()));
        assert_eq!(a.nav.overlay(), Some(&Overlay::SyncRecoveryKey));
        a.apply(Event::SyncStatus(on(Some(SyncFolderKind::GoogleDrive))));
        assert_eq!(a.nav.overlay(), Some(&Overlay::SyncRecoveryKey), "the key stays up");

        a.handle(Action::ClearCompleted, 5);
        assert_eq!(a.take_clipboard().as_deref(), Some("ABCD 2345 EFGH 6789 JKLM 2345"));
        a.clipboard_went(true);
        assert!(a.devices.key_copied);
        a.handle(Action::PlayTrailer, 5);
        assert!(a.devices.show_qr);
        a.handle(Action::Open, 5);
        assert!(!a.nav.has_overlay());
        assert!(a.devices.recovery_key.is_none(), "the key is not kept once it is closed");
        assert!(!a.devices.setting_up);
    }

    #[test]
    fn a_folder_with_another_group_asks_for_its_key_and_a_wrong_one_is_said_there() {
        let mut a = at_devices(off());
        a.handle(Action::Open, 5);
        a.handle(Action::Down, 5);
        a.handle(Action::Open, 5);
        a.handle(Action::Bottom, 5);
        a.handle(Action::Open, 5);
        assert_eq!(a.nav.overlay(), Some(&Overlay::SyncAddress));
        type_text(&mut a, "/home/kim/Synced");
        let task = a.handle(Action::Open, 5);
        assert!(matches!(task, Some(Command::StartSync { .. })), "{task:?}");
        assert!(a.handle(Action::Open, 5).is_none(), "one at a time");

        a.apply(Event::SyncProblem(SyncProblem::GroupExists));
        assert_eq!(a.nav.overlay(), Some(&Overlay::SyncRestore));
        type_text(&mut a, "abcd2345efgh6789jklm2345");
        let task = a.handle(Action::Open, 5);
        let Some(Command::JoinSync { folder, recovery_key, .. }) = task else {
            panic!("{task:?}")
        };
        assert_eq!(folder.kind, SyncFolderKind::Directory);
        assert_eq!(folder.address.as_deref(), Some("/home/kim/Synced"));
        assert_eq!(recovery_key.0, "abcd 2345 efgh 6789 jklm 2345");
        assert!(a.text(Field::RecoveryKey).is_empty(), "the key is not kept");

        a.apply(Event::SyncProblem(SyncProblem::WrongRecoveryKey));
        assert_eq!(a.nav.overlay(), Some(&Overlay::SyncRestore));
        assert_eq!(a.devices.problem, Some(t::rkey_wrong()));
    }

    #[test]
    fn a_webdav_server_takes_its_password_masked_and_keeps_none_of_it() {
        let mut a = at_devices(off());
        a.handle(Action::Open, 5);
        a.handle(Action::Down, 5);
        a.handle(Action::Open, 5);
        let at = a.cloud_choices().iter().position(|k| *k == SyncFolderKind::WebDav).unwrap();
        for _ in 0..at {
            a.handle(Action::Down, 5);
        }
        a.handle(Action::Open, 5);
        type_text(&mut a, "https://dav.example/kaeri");
        a.handle(Action::Open, 5);
        assert_eq!(a.nav.overlay(), Some(&Overlay::SyncUser));
        type_text(&mut a, "kim");
        a.handle(Action::Open, 5);
        assert_eq!(a.nav.overlay(), Some(&Overlay::SyncPassword));
        assert!(Field::SyncPassword.masked());
        type_text(&mut a, "hunter2");
        let task = a.handle(Action::Open, 5);
        let Some(Command::StartSync { folder, .. }) = task else { panic!("{task:?}") };
        assert_eq!(folder.user.as_deref(), Some("kim"));
        assert_eq!(folder.password.map(|p| p.0).as_deref(), Some("hunter2"));
        assert!(a.text(Field::SyncPassword).is_empty());
        assert!(a.devices.pending.as_ref().is_some_and(|p| p.password.is_none()));

        a.apply(Event::SyncProblem(SyncProblem::Unreachable));
        assert_eq!(a.devices.problem, Some(t::cloud_err_offline(&t::cloud_webdav())));
        a.handle(Action::Back, 5);
        assert!(a.text(Field::SyncAddress).is_empty() && a.text(Field::SyncUser).is_empty());
        assert!(a.devices.problem.is_none());
    }

    #[test]
    fn a_pairing_code_counts_down_and_closes_on_the_new_device() {
        let mut a = at_devices(on(None));
        let at = a.device_lines().iter().position(|l| *l == DeviceLine::ShowCode).unwrap();
        a.cursors.devices.selected = at;
        let task = a.handle(Action::Open, 5);
        assert_eq!(task, Some(Command::PairInvite { device_name: "Kim's laptop".into() }));
        assert_eq!(a.nav.overlay(), Some(&Overlay::SyncPairShow));
        a.apply(Event::SyncInvite { code: "042917".into(), port: 47811, seconds: 120 });
        assert_eq!(a.pair_code_left(), Some(120));
        assert!(a.is_ticking());
        a.clock.advance(std::time::Duration::from_secs(12));
        assert_eq!(a.pair_code_left(), Some(108));

        let mut paired = on(None);
        paired.devices.push(device("c", "Living room TV", false));
        a.apply(Event::SyncStatus(paired));
        assert!(!a.nav.has_overlay());
        assert!(a.toasts.iter().any(|t| t.notice.text == t::pair_ok("Living room TV")));
    }

    #[test]
    fn esc_stops_the_code_and_a_spent_one_is_made_again() {
        let mut a = at_devices(on(None));
        a.cursors.devices.selected =
            a.device_lines().iter().position(|l| *l == DeviceLine::ShowCode).unwrap();
        a.handle(Action::Open, 5);
        a.apply(Event::SyncInvite { code: "042917".into(), port: 47811, seconds: 120 });
        a.clock.advance(std::time::Duration::from_secs(200));
        assert!(a.pair_code_expired());
        assert!(matches!(a.handle(Action::Open, 5), Some(Command::PairInvite { .. })));
        assert_eq!(a.handle(Action::Back, 5), Some(Command::PairStop));
        assert!(a.devices.invite.is_none());
    }

    #[test]
    fn a_code_is_six_digits_then_the_address_and_a_miss_is_said_there() {
        let mut a = at_devices(off());
        a.handle(Action::Open, 5);
        a.handle(Action::Open, 5);
        assert_eq!(a.nav.overlay(), Some(&Overlay::SyncNearby));
        a.handle(Action::Down, 5);
        a.handle(Action::Open, 5);
        assert_eq!(a.nav.overlay(), Some(&Overlay::SyncPairCode));
        type_text(&mut a, "04a2 9");
        assert_eq!(a.text(Field::PairCode), "0429");
        assert_eq!(a.handle(Action::Open, 5), None);
        assert_eq!(a.nav.overlay(), Some(&Overlay::SyncPairCode), "four digits are not a code");
        type_text(&mut a, "17999");
        assert_eq!(a.text(Field::PairCode), "042917");
        a.handle(Action::Open, 5);
        assert_eq!(a.nav.overlay(), Some(&Overlay::SyncPairAddress));
        type_text(&mut a, "192.168.1.20:47811");
        let task = a.handle(Action::Open, 5);
        assert_eq!(
            task,
            Some(Command::PairJoin {
                device_name: "Kim's laptop".into(),
                address: "192.168.1.20:47811".into(),
                code: "042917".into(),
            })
        );
        a.apply(Event::SyncProblem(SyncProblem::Unreachable));
        assert_eq!(a.devices.problem, Some(t::pair_err_not_found()));
        assert_eq!(a.nav.overlay(), Some(&Overlay::SyncPairAddress));
    }

    #[test]
    fn removing_a_device_and_turning_off_ask_first() {
        let mut a = at_devices(on(Some(SyncFolderKind::GoogleDrive)));
        assert_eq!(a.device_line(), Some(DeviceLine::Device(1)));
        assert_eq!(a.handle(Action::Open, 5), None);
        assert_eq!(a.nav.overlay(), Some(&Overlay::SyncDevice));
        a.handle(Action::Bottom, 5);
        assert_eq!(a.handle(Action::Open, 5), None);
        assert_eq!(a.nav.overlay(), Some(&Overlay::SyncConfirm));
        assert_eq!(a.handle(Action::Down, 5), None, "a stray key answers nothing");
        assert_eq!(
            a.handle(Action::Open, 5),
            Some(Command::RemoveSyncDevice { device: "b".into() })
        );

        a.handle(Action::Bottom, 5);
        assert_eq!(a.device_line(), Some(DeviceLine::TurnOff));
        a.handle(Action::Open, 5);
        a.handle(Action::Back, 5);
        assert!(!a.nav.has_overlay() && a.devices.confirm.is_none());
        a.handle(Action::Open, 5);
        assert_eq!(a.handle(Action::Open, 5), Some(Command::LeaveSync));
    }

    #[test]
    fn a_devices_page_turns_its_switches_and_the_remote_shows_only_when_paired() {
        let mut status = on(Some(SyncFolderKind::GoogleDrive));
        status.devices[1].sign_ins = vec![SyncFolderKind::Nextcloud];
        let mut a = at_devices(status.clone());
        a.handle(Action::Open, 5);
        assert_eq!(
            a.device_page_items(),
            vec![DevicePageItem::Sync, DevicePageItem::CopySignIns, DevicePageItem::Remove]
        );
        assert_eq!(
            a.handle(Action::Open, 5),
            Some(Command::SyncGroup(SyncCommand::SetDeviceSync {
                device: "b".into(),
                on: false,
            }))
        );
        a.handle(Action::Down, 5);
        let task = a.handle(Action::Open, 5);
        let Some(Command::SyncGroup(SyncCommand::CopySignIns { device, copy, .. })) = task
        else {
            panic!("{task:?}")
        };
        assert_eq!(device.as_deref(), Some("b"));
        assert_eq!(copy.len(), 2);

        status.devices[1].remote_key = Some("ab12".into());
        status.devices[1].remote_off = true;
        a.apply(Event::SyncStatus(status));
        assert!(a.device_page_items().contains(&DevicePageItem::Remote));
        a.cursors.overlay.selected = 1;
        assert_eq!(
            a.handle(Action::Open, 5),
            Some(Command::SyncGroup(SyncCommand::SetDeviceRemote {
                device: "b".into(),
                on: true,
            }))
        );
    }

    #[test]
    fn the_name_is_renamed_and_a_new_key_asks_first() {
        let mut a = at_devices(on(Some(SyncFolderKind::GoogleDrive)));
        a.cursors.devices.selected = 0;
        a.handle(Action::Open, 5);
        assert_eq!(a.nav.overlay(), Some(&Overlay::SyncName));
        assert_eq!(a.text(Field::DeviceName), "Kim's laptop");
        a.set_text(Field::DeviceName, "Den".into());
        assert_eq!(
            a.submit_devices_field(),
            Some(Command::SyncGroup(SyncCommand::RenameDevice { name: "Den".into() }))
        );

        a.cursors.devices.selected =
            a.device_lines().iter().position(|l| *l == DeviceLine::NewKey).unwrap();
        assert_eq!(a.handle(Action::Open, 5), None);
        assert_eq!(a.handle(Action::Open, 5), Some(Command::NewRecoveryKey));
    }

    #[test]
    fn a_note_waits_for_its_title_then_marks_the_episode_unwatched() {
        use kaeri_model::group::SyncNoteView;
        let mut status = on(Some(SyncFolderKind::GoogleDrive));
        status.notes = vec![SyncNoteView {
            id: 7,
            anilist_id: 900_001,
            episode: "5".into(),
            watch_device: "Kim's laptop".into(),
            unwatch_device: "Kim's iPhone".into(),
            at: 1_792_000_000,
        }];
        let mut a = at_devices(status);
        assert!(!a.device_lines().iter().any(|l| matches!(l, DeviceLine::Note(_))));
        assert!(a.take_commands().contains(&Command::LoadCards {
            ids: vec![kaeri_core::ids::AnilistId::new(900_001)]
        }));
        a.devices.note_titles.insert(900_001, "Made-up Show".into());
        a.cursors.devices.selected =
            a.device_lines().iter().position(|l| *l == DeviceLine::Note(0)).unwrap();
        assert_eq!(a.handle(Action::Open, 5), Some(Command::DismissSyncNote { id: 7 }));
        assert!(a.take_commands().contains(&Command::SetWatched {
            id: kaeri_core::ids::AnilistId::new(900_001),
            episodes: vec!["5".into()],
            watched: false,
        }));
    }

    #[test]
    fn a_wrong_code_is_said_by_the_device_that_refused_it() {
        let mut a = at_devices(off());
        a.cursors.overlay = Cursor::default();
        a.nav.open_overlay(Overlay::SyncPairCode);
        a.apply(Event::SyncPairFailed {
            problem: SyncPairProblem::WrongCode,
            device: Some("Kim's iPhone".into()),
        });
        a.apply(Event::SyncProblem(SyncProblem::Other("raw".into())));
        assert_eq!(a.devices.problem, Some(t::pair_err_code("Kim's iPhone")));
    }

    #[test]
    fn a_revoked_drive_signs_in_again_from_its_row() {
        let mut status = on(Some(SyncFolderKind::GoogleDrive));
        status.problem =
            Some(SyncProblem::SignInNeeded { service: SyncFolderKind::GoogleDrive });
        let mut a = at_devices(status);
        assert_eq!(a.folder_state(), FolderState::SignedOut);
        assert!(!a.device_lines().contains(&DeviceLine::SignOut));
        a.cursors.devices.selected =
            a.device_lines().iter().position(|l| *l == DeviceLine::Folder).unwrap();
        assert_eq!(
            a.handle(Action::Open, 5),
            Some(Command::SyncSignIn { service: SyncFolderKind::GoogleDrive })
        );
        assert_eq!(a.nav.overlay(), Some(&Overlay::SyncWait));
        assert_eq!(
            a.folder_problem(),
            Some(t::cloud_err_signed_out("Google Drive", &t::sync_tui_kind()))
        );
    }

    #[test]
    fn a_problem_with_no_question_open_is_a_toast() {
        let mut a = at_devices(on(Some(SyncFolderKind::OneDrive)));
        a.apply(Event::SyncProblem(SyncProblem::Other("the disk is full".into())));
        assert!(a.toasts.iter().any(|t| t.notice.text == "the disk is full"));
    }

    fn profile(id: &str, name: &str, active: bool, kids: bool) -> ProfileView {
        ProfileView {
            id: id.into(),
            name: name.into(),
            initials: name[..1].into(),
            kids,
            active,
        }
    }

    #[test]
    fn one_profile_shows_nothing_and_two_can_be_switched_and_added_to() {
        let mut a = at_devices(off());
        a.apply(Event::Profiles(ProfilesView { profiles: Vec::new() }));
        assert_eq!(a.device_lines(), vec![DeviceLine::Name, DeviceLine::TurnOn]);

        a.apply(Event::Profiles(ProfilesView {
            profiles: vec![
                profile("p1", "Kim", true, false),
                profile("p2", "Sam", false, true),
            ],
        }));
        assert_eq!(
            a.device_lines(),
            vec![
                DeviceLine::Name,
                DeviceLine::TurnOn,
                DeviceLine::Profile(0),
                DeviceLine::Profile(1),
                DeviceLine::AddProfile
            ]
        );
        a.handle(Action::Down, 5);
        assert_eq!(a.handle(Action::Open, 5), None, "the one in use");
        a.handle(Action::Down, 5);
        assert_eq!(a.handle(Action::Open, 5), Some(Command::SwitchProfile { id: "p2".into() }));
        a.handle(Action::Down, 5);
        a.handle(Action::Open, 5);
        assert_eq!(a.nav.overlay(), Some(&Overlay::ProfileName));
        type_text(&mut a, "Alex");
        a.handle(Action::Open, 5);
        assert_eq!(a.nav.overlay(), Some(&Overlay::ProfileKind));
        a.handle(Action::Down, 5);
        assert_eq!(
            a.handle(Action::Open, 5),
            Some(Command::AddProfile { name: "Alex".into(), kids: true, first_name: None })
        );

        a.apply(Event::ProfileSwitch { id: "p2".into() });
        assert_eq!(a.profile_restart(), Some("Sam"));
        assert!(a.should_quit);
    }

    #[test]
    fn a_kids_profile_has_no_devices() {
        let mut a = at_devices(off());
        a.apply(Event::Profiles(ProfilesView {
            profiles: vec![
                profile("p1", "Kim", false, false),
                profile("p2", "Sam", true, true),
            ],
        }));
        assert!(!a.in_devices());
        assert_eq!(a.handle(Action::ShowDevices, 5), None);
        assert!(!a.in_devices());
    }

    fn press(a: &mut App, code: crossterm::event::KeyCode) -> Option<Command> {
        a.press(crossterm::event::KeyEvent::new(code, crossterm::event::KeyModifiers::NONE), 5)
    }

    fn clearing() -> Event {
        Event::SyncSignIn {
            service: SyncFolderKind::GoogleDrive,
            step: Some(SyncSignInStep::Clearing),
        }
    }

    fn finishing() -> Event {
        Event::SyncSignIn {
            service: SyncFolderKind::GoogleDrive,
            step: Some(SyncSignInStep::Finishing),
        }
    }

    fn ended() -> Event {
        Event::SyncSignIn { service: SyncFolderKind::GoogleDrive, step: None }
    }

    /// Google Drive signed in to and found holding another group: the restore overlay.
    fn at_restore() -> App {
        let mut a = at_devices(off());
        a.handle(Action::Open, 5);
        a.handle(Action::Down, 5);
        a.handle(Action::Open, 5);
        let task = a.handle(Action::Open, 5);
        assert!(matches!(task, Some(Command::StartSync { .. })), "{task:?}");
        a.apply(finishing());
        a.apply(Event::SyncProblem(SyncProblem::GroupExists));
        assert_eq!(a.nav.overlay(), Some(&Overlay::SyncRestore));
        a
    }

    /// Tab to Start over, Enter, and Enter on the confirmation: the command it sends.
    fn confirm_start_over(a: &mut App) -> Option<Command> {
        press(a, crossterm::event::KeyCode::Tab);
        assert!(a.devices.start_over_focused);
        assert_eq!(a.handle(Action::Open, 5), None);
        assert_eq!(a.nav.overlay(), Some(&Overlay::SyncConfirm));
        a.handle(Action::Open, 5)
    }

    /// Time passes, and the idle tick comes.
    fn after(a: &mut App, millis: u64) {
        a.clock.advance(std::time::Duration::from_millis(millis));
        a.tick_toasts();
    }

    fn shown(a: &App) -> Option<StartOverStep> {
        a.devices.start_over.as_ref().map(|so| so.shown.clone())
    }

    #[test]
    fn tab_moves_between_the_key_and_start_over_and_a_letter_goes_back_to_the_key() {
        let mut a = at_restore();
        type_text(&mut a, "k7qm");
        press(&mut a, crossterm::event::KeyCode::Tab);
        assert!(a.devices.start_over_focused);
        assert_eq!(a.text(Field::RecoveryKey), "k7qm", "Tab is not typed");
        press(&mut a, crossterm::event::KeyCode::Char('4'));
        assert!(!a.devices.start_over_focused);
        assert_eq!(a.text(Field::RecoveryKey), "k7qm4");
        press(&mut a, crossterm::event::KeyCode::Down);
        assert!(a.devices.start_over_focused);
        press(&mut a, crossterm::event::KeyCode::BackTab);
        assert!(!a.devices.start_over_focused);
    }

    #[test]
    fn start_over_asks_first_and_esc_goes_back_to_the_restore_overlay() {
        let mut a = at_restore();
        press(&mut a, crossterm::event::KeyCode::Tab);
        a.handle(Action::Open, 5);
        assert_eq!(a.devices.confirm, Some(DeviceConfirm::StartOver));
        assert_eq!(a.handle(Action::Back, 5), None);
        assert_eq!(a.nav.overlay(), Some(&Overlay::SyncRestore));
        assert!(a.devices.start_over.is_none());
    }

    #[test]
    fn start_over_holds_the_restore_content_then_shows_each_step_and_the_key_with_its_line() {
        let mut a = at_restore();
        let task = confirm_start_over(&mut a);
        let Some(Command::SyncGroup(SyncCommand::ClearFolderAndStart { device_name, folder })) =
            task
        else {
            panic!("{task:?}")
        };
        assert_eq!(device_name, "Kim's laptop");
        assert_eq!(folder.kind, SyncFolderKind::GoogleDrive);

        // The sign-in that found the group ends late, and says nothing about the start over.
        a.apply(ended());
        a.apply(clearing());
        assert_eq!(a.nav.overlay(), Some(&Overlay::SyncRestore), "under 600 ms");
        assert!(a.is_ticking());
        after(&mut a, 600);
        assert_eq!(a.nav.overlay(), Some(&Overlay::SyncStartOver));
        assert_eq!(shown(&a), Some(StartOverStep::Clearing));

        a.apply(finishing());
        assert_eq!(shown(&a), Some(StartOverStep::Clearing), "each step stays 400 ms");
        after(&mut a, 400);
        assert_eq!(shown(&a), Some(StartOverStep::SettingUp));

        a.apply(Event::SyncRecoveryKey("ABCD2345EFGH6789JKLM2345".into()));
        a.apply(Event::SyncStatus(on(Some(SyncFolderKind::GoogleDrive))));
        assert_eq!(a.nav.overlay(), Some(&Overlay::SyncStartOver), "the status closes nothing");
        after(&mut a, 400);
        assert_eq!(a.nav.overlay(), Some(&Overlay::SyncRecoveryKey));
        assert_eq!(a.devices.started_over, Some(SyncFolderKind::GoogleDrive));
        assert_eq!(a.devices.recovery_key.as_deref(), Some("ABCD2345EFGH6789JKLM2345"));
        a.apply(ended());
        assert_eq!(a.nav.overlay(), Some(&Overlay::SyncRecoveryKey));
        assert!(a.devices.start_over.is_none() && a.devices.pending.is_none());

        a.handle(Action::Open, 5);
        assert!(!a.nav.has_overlay());
        assert!(a.devices.started_over.is_none());
    }

    #[test]
    fn a_quick_service_goes_straight_from_the_confirmation_to_the_key() {
        let mut a = at_restore();
        confirm_start_over(&mut a);
        a.apply(clearing());
        a.apply(finishing());
        a.apply(Event::SyncRecoveryKey("ABCD2345EFGH6789JKLM2345".into()));
        assert_eq!(a.nav.overlay(), Some(&Overlay::SyncRecoveryKey));
        assert_eq!(a.devices.started_over, Some(SyncFolderKind::GoogleDrive));
    }

    #[test]
    fn closed_while_it_works_it_carries_on_and_the_key_opens_by_itself() {
        let mut a = at_restore();
        confirm_start_over(&mut a);
        a.apply(clearing());
        after(&mut a, 600);
        assert_eq!(a.handle(Action::Back, 5), None);
        assert!(!a.nav.has_overlay());
        a.apply(finishing());
        after(&mut a, 400);
        assert!(!a.nav.has_overlay(), "the work is not opened again");
        a.apply(Event::SyncRecoveryKey("ABCD2345EFGH6789JKLM2345".into()));
        after(&mut a, 400);
        assert_eq!(a.nav.overlay(), Some(&Overlay::SyncRecoveryKey));
        assert_eq!(a.devices.started_over, Some(SyncFolderKind::GoogleDrive));
    }

    #[test]
    fn with_no_key_made_the_end_closes_the_overlay() {
        let mut a = at_restore();
        confirm_start_over(&mut a);
        a.apply(clearing());
        a.apply(finishing());
        a.apply(Event::SyncStatus(on(Some(SyncFolderKind::GoogleDrive))));
        a.apply(ended());
        assert!(!a.nav.has_overlay());
        assert!(a.devices.start_over.is_none());
    }

    #[test]
    fn a_refused_delete_says_so_and_try_again_asks_again() {
        let mut a = at_restore();
        confirm_start_over(&mut a);
        a.apply(clearing());
        after(&mut a, 600);
        a.apply(Event::SyncProblem(SyncProblem::ClearRefused));
        a.apply(Event::SyncStatus(off()));
        after(&mut a, 400);
        assert_eq!(a.nav.overlay(), Some(&Overlay::SyncStartOver));
        assert_eq!(
            shown(&a),
            Some(StartOverStep::Failed(t::rkey_start_over_err_refused("Google Drive")))
        );
        let task = a.handle(Action::Open, 5);
        assert!(
            matches!(task, Some(Command::SyncGroup(SyncCommand::ClearFolderAndStart { .. }))),
            "{task:?}"
        );
        assert_eq!(a.handle(Action::Open, 5), None, "one at a time");
        a.apply(clearing());
        assert!(matches!(shown(&a), Some(StartOverStep::Failed(_))), "the error holds 600 ms");
        after(&mut a, 600);
        assert_eq!(shown(&a), Some(StartOverStep::Clearing));
    }

    #[test]
    fn offline_before_the_work_shows_is_said_at_once_and_esc_goes_back_to_the_key() {
        let mut a = at_restore();
        confirm_start_over(&mut a);
        a.apply(Event::SyncProblem(SyncProblem::Unreachable));
        assert_eq!(a.nav.overlay(), Some(&Overlay::SyncStartOver));
        assert_eq!(
            shown(&a),
            Some(StartOverStep::Failed(t::cloud_err_offline("Google Drive")))
        );
        assert_eq!(a.handle(Action::Back, 5), None);
        assert_eq!(a.nav.overlay(), Some(&Overlay::SyncRestore));
        assert!(a.devices.start_over.is_none());
        assert!(!a.devices.start_over_focused, "the key field has the keyboard");
        assert_eq!(
            a.devices.pending.as_ref().map(|p| p.kind.clone()),
            Some(SyncFolderKind::GoogleDrive)
        );
        // Start over is still there, for the same folder.
        let task = confirm_start_over(&mut a);
        assert!(
            matches!(task, Some(Command::SyncGroup(SyncCommand::ClearFolderAndStart { .. }))),
            "{task:?}"
        );
    }

    #[test]
    fn an_error_after_the_overlay_closed_opens_it_again_with_try_again() {
        let mut a = at_restore();
        confirm_start_over(&mut a);
        a.apply(clearing());
        after(&mut a, 600);
        a.handle(Action::Back, 5);
        assert!(!a.nav.has_overlay());
        a.apply(Event::SyncProblem(SyncProblem::ClearRefused));
        after(&mut a, 400);
        assert!(a.toasts.is_empty());
        assert_eq!(a.nav.overlay(), Some(&Overlay::SyncStartOver));
        assert_eq!(
            shown(&a),
            Some(StartOverStep::Failed(t::rkey_start_over_err_refused("Google Drive")))
        );
        let task = a.handle(Action::Open, 5);
        assert!(
            matches!(task, Some(Command::SyncGroup(SyncCommand::ClearFolderAndStart { .. }))),
            "{task:?}"
        );
        a.apply(Event::SyncProblem(SyncProblem::Unreachable));
        after(&mut a, 600);
        a.handle(Action::Back, 5);
        assert_eq!(a.nav.overlay(), Some(&Overlay::SyncRestore), "Esc goes back to the key");
        assert_eq!(
            a.devices.pending.as_ref().map(|p| p.kind.clone()),
            Some(SyncFolderKind::GoogleDrive)
        );
    }

    #[test]
    fn closed_in_the_quiet_moment_an_error_still_opens_on_its_folder() {
        let mut a = at_restore();
        confirm_start_over(&mut a);
        a.handle(Action::Back, 5);
        assert!(!a.nav.has_overlay());
        assert!(a.devices.pending.is_none(), "Esc on the restore overlay forgets the form");
        a.apply(Event::SyncProblem(SyncProblem::Unreachable));
        assert_eq!(a.nav.overlay(), Some(&Overlay::SyncStartOver));
        a.handle(Action::Back, 5);
        assert_eq!(a.nav.overlay(), Some(&Overlay::SyncRestore));
        assert_eq!(
            a.devices.pending.as_ref().map(|p| p.kind.clone()),
            Some(SyncFolderKind::GoogleDrive)
        );
    }

    #[test]
    fn started_over_elsewhere_the_folder_row_opens_the_restore_overlay_to_join() {
        let mut status = on(Some(SyncFolderKind::GoogleDrive));
        status.clouds[0].signed_in = true;
        status.problem = Some(SyncProblem::GroupReplaced);
        // The engine forgets the old key; a status that still says it holds one hides it too.
        status.recovery_key = true;
        let mut a = at_devices(status);
        let lines = a.device_lines();
        let at = lines.iter().position(|l| *l == DeviceLine::Folder).unwrap();
        assert_eq!(
            lines[at..],
            [DeviceLine::Folder, DeviceLine::EnterNewKey, DeviceLine::TurnOff],
            "Enter new key is the only way on"
        );
        assert_eq!(a.folder_state(), FolderState::Failed);
        assert_eq!(a.folder_problem(), Some(t::cloud_err_started_over("Google Drive")));

        a.cursors.devices.selected = at;
        assert_eq!(a.handle(Action::Open, 5), None);
        assert_eq!(a.nav.overlay(), Some(&Overlay::SyncRestore));
        type_text(&mut a, "abcd2345efgh6789jklm2345");
        let task = a.handle(Action::Open, 5);
        let Some(Command::JoinSync { folder, .. }) = task else { panic!("{task:?}") };
        assert_eq!(folder.kind, SyncFolderKind::GoogleDrive);
        assert_eq!(folder.address, None);
    }

    #[test]
    fn keys_and_codes_are_grouped_as_they_are_read() {
        assert_eq!(key_groups("abcd-2345 EFGH6789jklm2345"), "abcd 2345 EFGH 6789 jklm 2345");
        assert_eq!(code_groups("042917"), "042 917");
        assert_eq!(
            pair_link("042917", "192.168.1.20", 47811),
            "kaeri://pair/042917@192.168.1.20:47811"
        );
    }
}
