//! The wire format. `docs/plugin-stores.md` is the prose version of this file.

use serde::{Deserialize, Serialize};

/// The protocol version this crate speaks.
pub const PROTOCOL: u32 = 2;

/// The largest plugin a client downloads or a server accepts.
///
/// The JavaScript reference plugin is about 12 MiB, so this leaves room for a heavier runtime
/// without letting a store fill someone's disk.
pub const MAX_PLUGIN_BYTES: u64 = 32 * 1024 * 1024;

/// How much of a plugin moves in one request, uploading or downloading.
///
/// Small enough for every platform a store runs on — serverless functions cap a request or a
/// response at 4.5 MB — so no request anywhere carries more than this.
pub const PART_SIZE: u64 = 4 * 1024 * 1024;

/// How long an invite lasts unless the owner says otherwise.
pub const INVITE_LIFETIME_SECS: i64 = 7 * 24 * 60 * 60;

/// The `expires_at` of an invite with no end date: the last second of the year 9999, UTC.
///
/// A number rather than nothing, so a client from before end dates reads such an invite as one
/// that ends far off, which is what it is to them, and every rule that compares `expires_at` with
/// the time holds unchanged.
pub const NO_END_DATE: i64 = 253_402_300_799;

/// How long the members list keeps an invite that ended: used up, ran out or cancelled.
pub const ENDED_INVITES_KEPT_SECS: i64 = 30 * 24 * 60 * 60;

/// The longest name a member is shown by, in characters.
pub const MAX_NAME_CHARS: usize = 64;

/// `GET /v1/store`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreInfo {
    pub protocol: u32,
    pub name: String,
    /// The Ed25519 public key, unpadded base64url.
    pub key: String,
    /// Whether nobody owns the store yet and it was deployed with a setup code, so `POST
    /// /v1/setup` can claim it. Left out otherwise.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub claimable: bool,
    /// How to reach whoever runs the store, as its operator gave it. Left out until they do.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact: Option<String>,
    /// Whether an owner can choose how many times an invite works and how long. A store from
    /// before the choices leaves it out, and its invites work once, for seven days.
    #[serde(default)]
    pub invite_choices: bool,
    /// Whether the store takes book sources (`kaeri:books` plugins) as well as show sources. A
    /// store from before them leaves it out, and Kaeri publishes no book source to it: a store
    /// that reads manifests from their section would otherwise list one as a show source.
    #[serde(default)]
    pub books: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    #[default]
    Member,
    Owner,
}

impl Role {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Member => "member",
            Self::Owner => "owner",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "member" => Some(Self::Member),
            "owner" => Some(Self::Owner),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    pub id: String,
    pub name: String,
    pub role: Role,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub label: String,
}

/// `POST /v1/redeem`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedeemRequest {
    pub invite: String,
    pub device: String,
    /// The joiner's own name, for an invite that works more than once. Ignored for one that
    /// works once, which joins as the person it was made for. Left out by clients from before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// `POST /v1/preview`: what an invite leads to, before it is spent. Answered with a
/// [`Previewed`]: the [`SignedIndex`], which holding the invite entitles one to see, and what
/// the invite allows.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviewRequest {
    pub invite: String,
}

/// The answer to `POST /v1/preview`. The signed index's own fields, so a client from before the
/// invite's facts reads it as the [`SignedIndex`] it always was.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Previewed {
    #[serde(flatten)]
    pub signed: SignedIndex,
    /// What the invite allows. Left out by a store from before the choices.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invite: Option<InviteFacts>,
}

/// What an invite allows, for the person about to join with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct InviteFacts {
    /// How many times it works in all, or 0 for any number. One that works more than once asks
    /// whoever joins for their own name.
    pub max_uses: u32,
    /// When it stops working, or [`NO_END_DATE`].
    pub expires_at: i64,
}

impl InviteFacts {
    /// Whether whoever joins gives their own name: the invite works more than once.
    pub fn names_joiner(&self) -> bool {
        self.max_uses != 1
    }
}

/// `POST /v1/open/preview` and `POST /v1/open/join`: the code from a store's open link, which
/// anyone holding the link may use as often as they like while the store lets anyone join.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenJoinRequest {
    pub code: String,
}

/// The code is what lets anyone in, so it never reaches a log line through `{:?}`.
impl std::fmt::Debug for OpenJoinRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OpenJoinRequest(«redacted»)")
    }
}

/// `PUT /v1/open`: who can join. `false`, the default, is only people the owners invite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenSetting {
    pub open: bool,
}

/// `PUT /v1/contact`: how people reach whoever runs the store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContactSetting {
    pub contact: String,
}

/// The longest contact a store keeps, in characters.
pub const MAX_CONTACT_CHARS: usize = 200;

/// Tidy a contact an operator typed, or say what is wrong with it. An email address or a link
/// both fit: the store keeps the text and shows it as it is, so it has to be one plain line.
pub fn clean_contact(text: &str) -> Result<String, &'static str> {
    let text = text.trim();
    if text.is_empty() {
        return Err("give an email address or a link where people can reach you");
    }
    if text.chars().count() > MAX_CONTACT_CHARS {
        return Err("that contact is too long");
    }
    if text.chars().any(char::is_control) {
        return Err("a contact is one plain line");
    }
    Ok(text.to_owned())
}

/// `GET /v1/open`, and the answer to every change of it: who can join, the link anyone can join
/// with while it is open, and the devices that joined that way.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct OpenState {
    /// Whether anyone with the link can join. Otherwise only people the owners invite can.
    pub open: bool,
    /// The contact for the store, which an open link needs before it can be turned on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact: Option<String>,
    /// `{base}/join/{code}?open#{key}`, while the store is open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    /// How many devices that joined with a link still work here, whether or not the store is
    /// still open.
    pub devices: u32,
    /// The most recently joined of them, newest first, up to [`OPEN_DEVICES_LISTED`]. Their
    /// labels are empty: nothing about who joined is kept.
    #[serde(default)]
    pub device_list: Vec<DeviceRow>,
}

/// How many anonymous devices [`OpenState::device_list`] lists at most.
pub const OPEN_DEVICES_LISTED: u32 = 100;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedeemResponse {
    pub token: String,
    pub member: Member,
}

/// The token is a credential, so it never reaches a log line through `{:?}`.
impl std::fmt::Debug for RedeemResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedeemResponse")
            .field("token", &"«redacted»")
            .field("member", &self.member)
            .finish()
    }
}

/// `GET /v1/me`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Me {
    pub member: Member,
    pub device: Device,
}

/// `POST /v1/invites`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum InviteRequest {
    /// Another device for whoever is asking.
    Device,
    /// Someone new. Owners only.
    Member {
        /// Who it is for, or for an invite that works more than once, a name for the invite
        /// itself, such as a group's.
        name: String,
        #[serde(default)]
        role: Role,
        /// How many times it works. An owner invite works once.
        #[serde(default)]
        uses: InviteUses,
        /// How long it works.
        #[serde(default)]
        lasts: InviteLasts,
    },
}

impl InviteRequest {
    /// An invite for someone new that works once, for seven days: what every client asked for
    /// before the choices.
    pub fn member(name: impl Into<String>, role: Role) -> Self {
        Self::Member {
            name: name.into(),
            role,
            uses: InviteUses::default(),
            lasts: InviteLasts::default(),
        }
    }
}

/// How many times an invite works: the choices the screens offer, and no others.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InviteUses {
    #[default]
    Once,
    Five,
    /// Until it ends or an owner cancels it. Needs a contact for the store, since a link like
    /// this can be passed on as the store's own link can.
    Any,
}

impl InviteUses {
    /// The most times it works, or `None` for any number.
    pub const fn max(self) -> Option<u32> {
        match self {
            Self::Once => Some(1),
            Self::Five => Some(5),
            Self::Any => None,
        }
    }
}

/// How long an invite works: the choices the screens offer, and no others.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InviteLasts {
    Day,
    #[default]
    Week,
    Month,
    /// Until it is used up or an owner cancels it.
    Forever,
}

impl InviteLasts {
    /// When an invite made `now` stops working, or [`NO_END_DATE`].
    pub const fn expires_at(self, now: i64) -> i64 {
        const DAY: i64 = 24 * 60 * 60;
        match self {
            Self::Day => now + DAY,
            Self::Week => now + INVITE_LIFETIME_SECS,
            Self::Month => now + 30 * DAY,
            Self::Forever => NO_END_DATE,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InviteCreated {
    pub id: String,
    pub link: String,
    /// When it stops working, or [`NO_END_DATE`].
    pub expires_at: i64,
    /// How many times it works in all, or 0 for any number. 1 from a store from before the
    /// choices.
    #[serde(default = "one")]
    pub max_uses: u32,
}

fn one() -> u32 {
    1
}

/// `GET /v1/members`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct MemberList {
    pub members: Vec<MemberRow>,
    /// The invites that still work.
    pub invites: Vec<PendingInvite>,
    /// The invites that ended in the last 30 days, the latest first. Apart from `invites`, so a
    /// client from before reads only the ones that still work. Absent from servers before it.
    #[serde(default)]
    pub ended_invites: Vec<PendingInvite>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberRow {
    pub id: String,
    pub name: String,
    pub role: Role,
    pub joined_at: i64,
    pub last_seen: Option<i64>,
    pub devices: u32,
    /// Each device, for the admin page. Absent from servers that predate it.
    #[serde(default)]
    pub device_list: Vec<DeviceRow>,
}

/// One device of a member, for the owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceRow {
    pub id: String,
    pub label: String,
    pub created_at: i64,
    pub last_seen: Option<i64>,
}

/// `PATCH /v1/members/{id}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberChange {
    /// A new role. Not for yourself.
    #[serde(default)]
    pub role: Option<Role>,
    /// A new name, as the members list shows it. Yours included.
    #[serde(default)]
    pub name: Option<String>,
}

/// `POST /v1/admin/links`: a one-time link that signs a browser in to the admin page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminLink {
    pub link: String,
    pub expires_at: i64,
}

/// `POST /v1/admin/session`.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminLogin {
    pub code: String,
}

impl std::fmt::Debug for AdminLogin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AdminLogin(«redacted»)")
    }
}

/// `GET /v1/admin/overview`: everything the admin page draws, in one answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminOverview {
    pub store: StoreInfo,
    pub url: String,
    pub fingerprint: String,
    pub me: Member,
    pub sequence: u64,
    pub plugins: Vec<IndexEntry>,
    pub members: Vec<MemberRow>,
    pub invites: Vec<PendingInvite>,
    /// As [`MemberList::ended_invites`].
    #[serde(default)]
    pub ended_invites: Vec<PendingInvite>,
    /// Who can join, and the devices that joined with the open link. Absent from servers that
    /// predate open joining, which read as invite-only.
    #[serde(default)]
    pub open: OpenState,
    /// What the store runs. Absent from servers that predate it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<ServerInfo>,
}

/// What a store runs, for its owners: the overview's `server`, which the admin page compares with
/// the newest release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerInfo {
    /// The release it was built from, such as `0.10.8`. A store built from the workspace without a
    /// release carries the workspace's version.
    pub version: String,
    /// For a store compiled to WebAssembly, the first 16 hex digits of the SHA-256 of
    /// `deploy/store-wasm/SOURCE` it was built from. Its `version` is then only the major and
    /// minor version, since the package is not rebuilt for each release, and this is what tells
    /// one build from another.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<String>,
    /// The schema version its database is at.
    pub schema: u32,
    /// Where it runs, as far as it can tell: `docker`, `fly`, `render`, `railway`, `heroku`,
    /// `cloudflare`, `vercel`, `netlify`, `kaeri` for one that `kaeri store serve` runs,
    /// or `binary` for `kaeri-store` run by hand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
}

impl ServerInfo {
    /// A store built from this workspace, on a host it names if it can.
    pub fn new(version: &str, host: Option<&str>) -> Self {
        Self {
            version: version.to_owned(),
            build: None,
            schema: crate::schema::VERSION,
            host: host.map(str::to_owned),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InviteKind {
    Member,
    Device,
}

impl InviteKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Member => "member",
            Self::Device => "device",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "member" => Some(Self::Member),
            "device" => Some(Self::Device),
            _ => None,
        }
    }
}

/// An invite link, for the owners: one that still works, or one that ended lately.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingInvite {
    pub id: String,
    pub name: String,
    pub role: Role,
    pub kind: InviteKind,
    /// When it stops working, or [`NO_END_DATE`].
    pub expires_at: i64,
    /// How many times it works in all, or 0 for any number. 1 from servers before the choices.
    #[serde(default = "one")]
    pub max_uses: u32,
    /// How many times it was used.
    #[serde(default)]
    pub uses: u32,
    /// Who joined with it, the first first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub joined: Vec<InviteUse>,
    /// Why and when it stopped working, for one that ended.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended: Option<InviteEnded>,
}

/// One use of an invite: who joined with it, and when.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InviteUse {
    /// The member who joined, or for a device invite, who the device was added to.
    pub member: String,
    /// Their name as the members list shows it now.
    pub name: String,
    pub used_at: i64,
    /// They were removed since.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub removed: bool,
}

/// Why an invite stopped working, and when.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct InviteEnded {
    pub reason: EndReason,
    pub at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndReason {
    /// Used as many times as it allows.
    UsedUp,
    /// Its end date passed.
    RanOut,
    /// An owner cancelled it, or removed the member who made it.
    Cancelled,
}

/// `GET /v1/index`: the index document as a string, and its signature.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedIndex {
    pub index: String,
    pub signature: String,
}

/// The document inside a [`SignedIndex`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Index {
    pub protocol: u32,
    pub store: String,
    pub key: String,
    pub sequence: u64,
    pub issued_at: i64,
    pub plugins: Vec<IndexEntry>,
    /// Who runs the store: its owners' names, the longest-standing first, for "Run by …". Like
    /// `store`, it describes the store as it is now and moves no `sequence`, which counts the
    /// plugin list. Left out by a store from before it was, and then read as none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub owners: Vec<String>,
    /// Whether anyone with the store's link can join it, rather than only people its owners
    /// invite: something a member should know about who else is there. Like `owners`, it
    /// describes the store as it is now and moves no `sequence`. Left out while it is false, and
    /// by a store from before it was.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub open: bool,
    /// How to reach whoever runs the store, shown on its page. Like `owners`, it describes the
    /// store as it is now and moves no `sequence`. Left out until the operator gives one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact: Option<String>,
}

impl Index {
    pub fn plugin(&self, id: &str) -> Option<&IndexEntry> {
        self.plugins.iter().find(|p| p.id == id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexEntry {
    pub id: String,
    pub name: String,
    pub version: String,
    pub allowed_hosts: Vec<String>,
    #[serde(default)]
    pub translations: Vec<String>,
    pub sha256: String,
    pub size: u64,
    pub published_at: i64,
    /// What the plugin is a source of: `books` for a book source. Absent, or `shows`, is a
    /// show source, so an entry for one is the same bytes it was before book sources. Kaeri
    /// 0.3.1 and later leave out entries of a kind they cannot use before reading anything
    /// else of them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// A book source's media, from its manifest: `manga`, `novel`. Absent for a show source.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub media: Vec<String>,
}

impl IndexEntry {
    /// The kind of plugin it offers, `shows` when it names none.
    pub fn kind(&self) -> &str {
        crate::manifest::kind_name(self.kind.as_deref())
    }

    /// Whether it offers a book source.
    pub fn is_books(&self) -> bool {
        self.kind() == crate::manifest::KIND_BOOKS
    }
}

/// `POST /v1/uploads`: start sending a plugin in parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UploadRequest {
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UploadStarted {
    pub id: String,
    pub part_size: u64,
    pub parts: u64,
}

/// `POST /v1/plugins`: publish what an upload carried.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishRequest {
    pub upload: String,
    /// The plugin's SHA-256, from whoever sent it. A store with CPU to spare computes it anyway;
    /// one that runs under a tight CPU budget, as a Worker on Cloudflare's free plan does, may
    /// take it instead. Members verify their downloads against it either way.
    #[serde(default)]
    pub sha256: Option<String>,
}

/// `POST /v1/setup`: claim a store that has no owner yet, with the code it was deployed with.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimRequest {
    pub code: String,
    /// How people reach whoever runs the store: an email address or a link. Required, and shown
    /// on the store's page.
    #[serde(default)]
    pub contact: String,
}

impl std::fmt::Debug for ClaimRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ClaimRequest(«redacted»)")
    }
}

/// The owner invite a claim produces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claimed {
    pub link: String,
    pub expires_at: i64,
}

/// Every error body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorBody {
    pub error: String,
    pub message: String,
    /// How to reach whoever runs the store, with the errors of an invite that ended, when they
    /// gave one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact: Option<String>,
    /// Who runs the store, the longest-standing owner first, with the same errors: who to ask
    /// for a new invite.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub owners: Vec<String>,
    /// When the invite stopped working, with `invite_ended`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<i64>,
}

/// Tidy the name a member gives themselves: one plain line of at most [`MAX_NAME_CHARS`]
/// characters, with runs of spaces and control characters made one space. `None` when nothing
/// is left.
pub fn clean_name(text: &str) -> Option<String> {
    let name: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(MAX_NAME_CHARS)
        .collect();
    let name = name.trim_end().to_owned();
    (!name.is_empty()).then_some(name)
}

/// Whether a plugin id is safe to use as a file name and a path segment.
///
/// Checked by the server before it publishes and by the client before it installs, because an id
/// becomes `{id}.wasm` on every member's disk and a `../` in it would be a path traversal into
/// someone's home directory. The same alphabet the plugin settings table can be keyed by.
pub fn is_valid_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
        && !id.starts_with(['-', '_'])
}

/// Ids Kaeri's own sources already use. A plugin by one of these names would take a built-in
/// source's place in the chain, so a server will not publish one and a client will not install one.
/// `demo` is the built-in demo source's id, which no store may register or repoint. `folders`
/// and `servers` are the book sources' own.
pub const RESERVED_IDS: &[&str] =
    &["torrent", "remote", "plugins", "demo", "folders", "servers"];

/// Whether a version string is safe to put in a path segment and a file name.
pub fn is_valid_version(version: &str) -> bool {
    (1..=64).contains(&version.len())
        && version
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'+' | b'_'))
        && !version.starts_with('.')
}

/// Hosts compared the way the sandbox compares them: trimmed, lowercase, as a set.
pub fn host_set(hosts: &[String]) -> std::collections::BTreeSet<String> {
    hosts.iter().map(|h| h.trim().to_ascii_lowercase()).filter(|h| !h.is_empty()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_that_could_escape_a_directory_are_invalid() {
        for good in ["example", "my-plugin", "plugin_2", "a"] {
            assert!(is_valid_id(good), "{good}");
        }
        for bad in ["", "../evil", "a/b", "Example", "-dash", "dot.ted", "with space", "é"] {
            assert!(!is_valid_id(bad), "{bad}");
        }
        assert!(!is_valid_id(&"a".repeat(65)));
    }

    #[test]
    fn versions_that_could_escape_a_directory_are_invalid() {
        for good in ["1.4.0", "0.10.0-beta.1", "1.0.0+build.5", "v3"] {
            assert!(is_valid_version(good), "{good}");
        }
        for bad in ["", "..", "../1", "1/2", "1 2", ".hidden"] {
            assert!(!is_valid_version(bad), "{bad}");
        }
    }

    #[test]
    fn an_invite_request_is_tagged_by_kind() {
        let device: InviteRequest = serde_json::from_str(r#"{"kind":"device"}"#).unwrap();
        assert_eq!(device, InviteRequest::Device);
        // As a client from before the choices sends it: once, for seven days.
        let member: InviteRequest =
            serde_json::from_str(r#"{"kind":"member","name":"Alex"}"#).unwrap();
        assert_eq!(member, InviteRequest::member("Alex", Role::Member));
        let owner: InviteRequest =
            serde_json::from_str(r#"{"kind":"member","name":"Sam","role":"owner"}"#).unwrap();
        assert_eq!(owner, InviteRequest::member("Sam", Role::Owner));
        let group: InviteRequest = serde_json::from_str(
            r#"{"kind":"member","name":"Book club","uses":"five","lasts":"forever"}"#,
        )
        .unwrap();
        assert_eq!(
            group,
            InviteRequest::Member {
                name: "Book club".into(),
                role: Role::Member,
                uses: InviteUses::Five,
                lasts: InviteLasts::Forever,
            }
        );
    }

    #[test]
    fn only_the_offered_choices_are_accepted() {
        for bad in [r#""uses":"ten""#, r#""uses":5"#, r#""lasts":"year""#, r#""lasts":3600"#] {
            let body = format!(r#"{{"kind":"member","name":"Alex",{bad}}}"#);
            assert!(serde_json::from_str::<InviteRequest>(&body).is_err(), "{body}");
        }
    }

    #[test]
    fn each_choice_has_its_number_and_end() {
        assert_eq!(InviteUses::Once.max(), Some(1));
        assert_eq!(InviteUses::Five.max(), Some(5));
        assert_eq!(InviteUses::Any.max(), None);
        assert_eq!(InviteLasts::Day.expires_at(100), 100 + 86_400);
        assert_eq!(InviteLasts::Week.expires_at(100), 100 + INVITE_LIFETIME_SECS);
        assert_eq!(InviteLasts::Month.expires_at(100), 100 + 30 * 86_400);
        assert_eq!(InviteLasts::Forever.expires_at(100), NO_END_DATE);
    }

    #[test]
    fn an_invite_from_an_older_server_works_once() {
        let older =
            r#"{"id":"1","name":"Alex","role":"member","kind":"member","expires_at":9}"#;
        let invite: PendingInvite = serde_json::from_str(older).unwrap();
        assert_eq!((invite.max_uses, invite.uses, invite.ended), (1, 0, None));
        let made: InviteCreated =
            serde_json::from_str(r#"{"id":"1","link":"x","expires_at":9}"#).unwrap();
        assert_eq!(made.max_uses, 1);
        let info: StoreInfo =
            serde_json::from_str(r#"{"protocol":1,"name":"Ours","key":"k"}"#).unwrap();
        assert!(!info.invite_choices);
    }

    #[test]
    fn a_preview_reads_as_the_signed_index_it_always_was() {
        let previewed = Previewed {
            signed: SignedIndex { index: "{}".into(), signature: "s".into() },
            invite: Some(InviteFacts { max_uses: 5, expires_at: NO_END_DATE }),
        };
        let text = serde_json::to_string(&previewed).unwrap();
        let older: SignedIndex = serde_json::from_str(&text).unwrap();
        assert_eq!(older, previewed.signed);
        assert_eq!(serde_json::from_str::<Previewed>(&text).unwrap(), previewed);
        let from_older: Previewed =
            serde_json::from_str(r#"{"index":"{}","signature":"s"}"#).unwrap();
        assert_eq!(from_older.invite, None);
        assert!(previewed.invite.unwrap().names_joiner());
        assert!(!InviteFacts { max_uses: 1, expires_at: 0 }.names_joiner());
        assert!(InviteFacts { max_uses: 0, expires_at: 0 }.names_joiner());
    }

    #[test]
    fn a_name_is_one_plain_line() {
        assert_eq!(clean_name("  Sam \n Lee\t"), Some("Sam Lee".into()));
        assert_eq!(clean_name(" \u{7} "), None);
        assert_eq!(clean_name(&"é".repeat(80)).unwrap().chars().count(), MAX_NAME_CHARS);
    }

    #[test]
    fn a_redeemed_token_never_prints() {
        let response = RedeemResponse {
            token: "ast1_secret".into(),
            member: Member { id: "1".into(), name: "Alex".into(), role: Role::Member },
        };
        assert!(!format!("{response:?}").contains("secret"));
    }

    #[test]
    fn an_index_says_it_is_open_only_when_it_is() {
        // Left out while false, so an invite-only store signs the same bytes it did before open
        // joining existed, and an index from such a store reads as invite-only.
        let index = Index {
            protocol: PROTOCOL,
            store: "Ours".into(),
            key: String::new(),
            sequence: 1,
            issued_at: 0,
            plugins: Vec::new(),
            owners: Vec::new(),
            open: false,
            contact: None,
        };
        let closed = serde_json::to_string(&index).unwrap();
        assert!(!closed.contains("open"), "{closed}");
        assert!(!serde_json::from_str::<Index>(&closed).unwrap().open);
        let open = serde_json::to_string(&Index { open: true, ..index }).unwrap();
        assert!(serde_json::from_str::<Index>(&open).unwrap().open);
    }

    #[test]
    fn a_show_source_s_entry_is_the_bytes_it_was_before_kinds() {
        let entry = IndexEntry {
            id: "a".into(),
            name: "A".into(),
            version: "1".into(),
            allowed_hosts: Vec::new(),
            translations: Vec::new(),
            sha256: String::new(),
            size: 0,
            published_at: 0,
            kind: None,
            media: Vec::new(),
        };
        let shows = serde_json::to_string(&entry).unwrap();
        assert!(!shows.contains("kind") && !shows.contains("media"), "{shows}");
        assert!(!entry.is_books());
        let books =
            IndexEntry { kind: Some("books".into()), media: vec!["novel".into()], ..entry };
        let books = serde_json::to_string(&books).unwrap();
        assert!(books.contains(r#""kind":"books""#) && books.contains(r#""media":["novel"]"#));
        assert!(serde_json::from_str::<IndexEntry>(&books).unwrap().is_books());
    }

    #[test]
    fn an_open_code_never_prints() {
        let request = OpenJoinRequest { code: "k3TzQe9WbA1xLr7uVn2c0g".into() };
        assert!(!format!("{request:?}").contains("k3Tz"));
    }

    #[test]
    fn host_sets_ignore_case_order_and_blanks() {
        let a = host_set(&["API.example".into(), "cdn.example".into(), " ".into()]);
        let b = host_set(&["cdn.example".into(), "api.example ".into()]);
        assert_eq!(a, b);
    }
}
