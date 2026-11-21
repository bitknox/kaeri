//! The folder road: one directory per device and one writer per directory, so Syncthing and iCloud
//! never make conflict copies and WebDAV never needs locks.
//!
//! ```text
//! <prefix>/group.bin                 format, group id, the keyring wrapped by the recovery key
//! <prefix>/devices/<id>/keys.bin     the keyring, wrapped for this device
//! <prefix>/devices/<id>/000001.seg   this device's ops, sealed
//! <prefix>/devices/<id>/acks.bin     the version vector this device has seen, sealed
//! <prefix>/keys/<id>/<from>-<fp>.bin the keyring as device <from> holds it, wrapped for <id>
//! <prefix>/paused/<id>               present while the group holds back from that device
//! <prefix>/removed/<id>              present once the group removed that device
//! ```
//!
//! A device whose Sync switch is off (the `device_sync` register) is held back from: the others
//! skip its segments, and write its name under `paused/`, where it looks at the start of every
//! pass and, finding itself, neither reads nor writes. The register alone could not say so, since
//! a held-back device would never read the register that lets it back in. Not a lock, which
//! removing a device is (it rotates the key): a device that ignored the marker could still read.
//!
//! Removing a device rotates: a new group key and a new recovery key, both in the keyring, so
//! every device it is wrapped for learns them, and `group.bin` under the new recovery key. A
//! removed device that knew the old recovery key opens nothing written after. Each rotation says
//! whom its key was kept from; a device that finds the current key was not kept from a device the
//! log lists as removed, as when two removals ran at once, rotates again. Every device wraps its
//! keyring for the others whenever it changes, `keys.bin` for every version and a file of its own
//! under `keys/`, which no other device writes over, so the keys of rotations that ran at once
//! reach everyone.
//!
//! Starting over ([`FolderRoad::clear`]) removes everything the road owns, `group.bin` last, so
//! a clear that stopped part way still reads as another group's folder and runs again. Each
//! device keeps the id of the group it is in (the 16 plain bytes after the magic in `group.bin`)
//! and reads it at the start of every pass: a folder with no `group.bin`, or another group's, is
//! [`SyncError::GroupReplaced`], and the pass writes nothing.
//!
//! A removed device cannot read the removal, which is sealed under the key it no longer holds, so
//! the device that removes it also writes its name under `removed/`. The removed device finds it
//! at the start of its next pass and says it was removed, rather than that it holds no key, and a
//! join or set-up under that id says so too: it joins again under a new id.

use std::{collections::BTreeMap, sync::Arc};

use kaeri_store::{
    Store,
    sync::{Applied, Effect, Op, SyncNote, VersionVector, kinds},
};

use crate::{
    error::{Result, SyncError},
    folder::{FolderError, FolderTarget},
    keys::{DeviceKey, Keyring, RecoveryKey, open_with, random, seal_with, wrap_for},
    seal::{self, Kind},
};

const GROUP_MAGIC: &[u8; 4] = b"KSG1";
/// A segment holds at most this much plain text.
pub const SEGMENT_BYTES: usize = 1024 * 1024;

/// What one pass through the folder did.
#[derive(Debug, Default)]
pub struct FolderReport {
    pub segments_read: usize,
    pub ops_applied: usize,
    pub segments_written: usize,
    pub ops_written: usize,
    pub devices_seen: usize,
    pub compacted: usize,
    /// For the engine to carry out: shared settings, the source order and the like.
    pub effects: Vec<Effect>,
    pub notes: Vec<SyncNote>,
    pub titles: Vec<u32>,
    /// Completed watches that arrived, for the tracker rules.
    pub watches: Vec<kaeri_store::sync::ReceivedWatch>,
    /// Works whose reading changed, by WorkId: an open reader may move on.
    pub works: Vec<String>,
    /// Reads kept against an unread they had not seen, for the note on each device.
    pub read_notes: Vec<kaeri_store::sync::ReadNote>,
    /// The group holds back from this device (its name is under `paused/`): the pass read and
    /// wrote nothing.
    pub paused: bool,
    /// This pass rotated the group key, and this is the recovery key it made.
    pub rotated: Option<RecoveryKey>,
    /// The recovery key that opens `group.bin`, when this pass looked: after the keyring changed.
    pub recovery: Option<RecoveryKey>,
    /// Another copy of this device writes under its id: the pass stopped before writing, for
    /// the device to carry on under a new id.
    pub second_copy: bool,
}

impl FolderReport {
    fn absorb(&mut self, applied: Applied) {
        self.ops_applied += applied.ops;
        self.effects.extend(applied.effects);
        self.notes.extend(applied.notes);
        self.titles.extend(applied.titles);
        self.watches.extend(applied.watches);
        self.works.extend(applied.works);
        self.read_notes.extend(applied.read_notes);
        self.second_copy |= applied.second_copy;
    }
}

pub struct FolderRoad {
    target: Arc<dyn FolderTarget>,
    prefix: String,
    store: Store,
    device: String,
    key: DeviceKey,
    ring: std::sync::Mutex<Keyring>,
    /// The group this device is in, once known: from creating, joining or opening it, else
    /// from what the store kept.
    group: std::sync::Mutex<Option<[u8; 16]>>,
}

/// What the road owns in a folder, besides `group.bin`.
const OWNED_DIRS: [&str; 4] = ["devices", "keys", "paused", "removed"];

impl std::fmt::Debug for FolderRoad {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FolderRoad({} at {}/)", self.device, self.prefix)
    }
}

fn group_aad(group: &[u8; 16]) -> Vec<u8> {
    let mut aad = GROUP_MAGIC.to_vec();
    aad.extend_from_slice(group);
    aad
}

fn group_file(group: &[u8; 16], recovery: &RecoveryKey, ring: &Keyring) -> Vec<u8> {
    let mut out = group_aad(group);
    out.extend(seal_with(&recovery.wrap_key(), &group_aad(group), &ring.to_bytes()));
    out
}

/// The group id in `group.bin`, which is plain.
fn group_id(bytes: &[u8]) -> Result<[u8; 16]> {
    if bytes.len() < 20 || &bytes[..4] != GROUP_MAGIC {
        return Err(SyncError::Damaged("group.bin"));
    }
    Ok(bytes[4..20].try_into().expect("16 bytes"))
}

fn open_group_file(bytes: &[u8], recovery: &RecoveryKey) -> Result<([u8; 16], Keyring)> {
    let group = group_id(bytes)?;
    let plain = open_with(&recovery.wrap_key(), &group_aad(&group), &bytes[20..])
        .ok_or(SyncError::WrongRecoveryKey)?;
    Ok((group, Keyring::from_bytes(&plain)?))
}

impl FolderRoad {
    fn path(&self, rest: &str) -> String {
        join(&self.prefix, rest)
    }

    fn meta(&self, what: &str) -> String {
        format!("folder.{}.{what}", self.prefix)
    }

    fn meta_u64(&self, what: &str) -> Result<u64> {
        Ok(self
            .store
            .sync_meta_get(&self.meta(what))?
            .and_then(|v| v.parse().ok())
            .unwrap_or(0))
    }

    /// Whether the folder already holds a group: what decides between setting one up and asking
    /// for the recovery key.
    pub async fn group_exists(target: &dyn FolderTarget, prefix: &str) -> Result<bool> {
        match target.read(&join(prefix, "group.bin")).await {
            Ok(_) => Ok(true),
            Err(FolderError::NotFound(_)) => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    /// Start a group in an empty folder: its keys, its recovery key (to show once, now), and this
    /// device's place in it.
    pub async fn create(
        target: Arc<dyn FolderTarget>,
        prefix: &str,
        store: Store,
        device: &str,
        key: DeviceKey,
    ) -> Result<(Self, RecoveryKey)> {
        let recovery = RecoveryKey::generate();
        let road = Self::create_with(
            target,
            prefix,
            store,
            device,
            key,
            Keyring::generate(),
            &recovery,
        )
        .await?;
        Ok((road, recovery))
    }

    /// Put a group that already exists into an empty folder: a group that synced only nearby
    /// gaining its first folder, or one moving to another folder. Its keyring and recovery key
    /// are the ones given, and the keyring is wrapped for every device in the group's list, so
    /// each can open the folder once it signs in to it.
    pub async fn create_with(
        target: Arc<dyn FolderTarget>,
        prefix: &str,
        store: Store,
        device: &str,
        key: DeviceKey,
        ring: Keyring,
        recovery: &RecoveryKey,
    ) -> Result<Self> {
        if Self::group_exists(&*target, prefix).await? {
            return Err(SyncError::GroupExists);
        }
        let group: [u8; 16] = random();
        target
            .write_new(&join(prefix, "group.bin"), &group_file(&group, recovery, &ring))
            .await?;
        let road = Self::open(target, prefix, store, device, key, ring);
        road.know_group(group)?;
        road.write_own_keys().await?;
        road.share_keys(&[]).await?;
        Ok(road)
    }

    /// Remove everything the road owns in a folder, so a new group can be set up there: every
    /// device's files, the keys, the markers, and `group.bin` last. Only files are removed that
    /// the road writes, so a folder the person picked keeps anything else in it. Removing what is
    /// already gone is done, so a clear that stopped part way finishes when run again, and until
    /// it does the folder still reads as another group's.
    pub async fn clear(target: &dyn FolderTarget, prefix: &str) -> Result<()> {
        for dir in OWNED_DIRS {
            target.remove_dir(&join(prefix, dir)).await?;
        }
        let group = join(prefix, "group.bin");
        // A rewrite of `group.bin` that never moved into place.
        for entry in target.list(prefix.trim_matches('/')).await? {
            if !entry.is_dir
                && entry.name.starts_with("group.bin.")
                && entry.name.ends_with(".tmp")
            {
                remove_file(target, &join(prefix, &entry.name)).await?;
            }
        }
        remove_file(target, &group).await
    }

    /// Whether the folder names `device` under `removed/`: the group removed it.
    pub async fn is_removed(
        target: &dyn FolderTarget,
        prefix: &str,
        device: &str,
    ) -> Result<bool> {
        Ok(target
            .list(&join(prefix, "removed"))
            .await?
            .iter()
            .any(|e| !e.is_dir && e.name == device))
    }

    /// Open the group a folder holds as one of its devices, with the keyring another device
    /// wrapped for this one: no recovery key needed. [`SyncError::NoGroup`] when the folder holds
    /// none, [`SyncError::GroupExists`] when it holds one this device has no key for, and
    /// [`SyncError::Removed`] when the group removed this device.
    pub async fn open_as_member(
        target: Arc<dyn FolderTarget>,
        prefix: &str,
        store: Store,
        device: &str,
        key: DeviceKey,
    ) -> Result<Self> {
        let group = match target.read(&join(prefix, "group.bin")).await {
            Ok(bytes) => group_id(&bytes)?,
            Err(FolderError::NotFound(_)) => return Err(SyncError::NoGroup),
            Err(e) => return Err(e.into()),
        };
        if Self::is_removed(&*target, prefix, device).await? {
            return Err(SyncError::Removed);
        }
        let path = join(prefix, &format!("devices/{device}/keys.bin"));
        let wrapped = match target.read(&path).await {
            Ok(bytes) => bytes,
            Err(FolderError::NotFound(_)) => return Err(SyncError::GroupExists),
            Err(e) => return Err(e.into()),
        };
        let ring = key.unwrap(device, &wrapped).map_err(|_| SyncError::GroupExists)?;
        let road = Self::open(target, prefix, store, device, key, ring);
        road.know_group(group)?;
        Ok(road)
    }

    /// Join the group a folder holds, with its recovery key. [`SyncError::Removed`] when the
    /// group removed a device of this id: it joins under a new one.
    pub async fn join(
        target: Arc<dyn FolderTarget>,
        prefix: &str,
        store: Store,
        device: &str,
        key: DeviceKey,
        recovery: &RecoveryKey,
    ) -> Result<Self> {
        let bytes = match target.read(&join(prefix, "group.bin")).await {
            Ok(b) => b,
            Err(FolderError::NotFound(_)) => return Err(SyncError::NoGroup),
            Err(e) => return Err(e.into()),
        };
        let (group, ring) = open_group_file(&bytes, recovery)?;
        if Self::is_removed(&*target, prefix, device).await? {
            return Err(SyncError::Removed);
        }
        let road = Self::open(target, prefix, store, device, key, ring);
        road.know_group(group)?;
        road.write_own_keys().await?;
        Ok(road)
    }

    /// Carry on with a group this device is already in, from the keyring the host kept.
    pub fn open(
        target: Arc<dyn FolderTarget>,
        prefix: &str,
        store: Store,
        device: &str,
        key: DeviceKey,
        ring: Keyring,
    ) -> Self {
        Self {
            target,
            prefix: prefix.trim_matches('/').to_owned(),
            store,
            device: device.to_owned(),
            key,
            ring: std::sync::Mutex::new(ring),
            group: std::sync::Mutex::new(None),
        }
    }

    /// Keep `group` as the one this device is in, here and in the store. What was kept of
    /// another group in this folder (cursors, numbers published) means nothing in this one and
    /// goes, so this device reads the new group from its start and writes all it holds again.
    fn know_group(&self, group: [u8; 16]) -> Result<()> {
        *self.group.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(group);
        let key = self.meta("group");
        let hexed = hex::encode(group);
        match self.store.sync_meta_get(&key)? {
            Some(kept) if kept == hexed => return Ok(()),
            Some(_) => {
                self.store.sync_meta_clear(&self.meta(""))?;
            }
            None => {}
        }
        self.store.sync_meta_set(&key, &hexed)?;
        Ok(())
    }

    fn known_group(&self) -> Result<Option<[u8; 16]>> {
        if let Some(group) =
            *self.group.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
        {
            return Ok(Some(group));
        }
        Ok(self
            .store
            .sync_meta_get(&self.meta("group"))?
            .and_then(|h| hex::decode(h).ok())
            .and_then(|b| b.try_into().ok()))
    }

    /// Whether the folder still holds the group this device is in, read before a pass writes
    /// anything: [`SyncError::GroupReplaced`] when `group.bin` is gone or another group's. A
    /// device that never kept the id (set up by an older version) takes the folder's when the
    /// folder still holds this device's keys, and finds the group replaced when it does not.
    async fn check_same_group(&self) -> Result<()> {
        let found = match self.target.read(&self.path("group.bin")).await {
            Ok(bytes) => group_id(&bytes)?,
            Err(FolderError::NotFound(_)) => return Err(SyncError::GroupReplaced),
            Err(e) => return Err(e.into()),
        };
        match self.known_group()? {
            Some(known) if known == found => {
                // The store's copy goes when the host takes a folder: put it back.
                if self.store.sync_meta_get(&self.meta("group"))?.is_none() {
                    self.know_group(found)?;
                }
                Ok(())
            }
            Some(_) => Err(SyncError::GroupReplaced),
            None if self.member_without_id(found).await? => self.know_group(found),
            None => Err(SyncError::GroupReplaced),
        }
    }

    /// For a device that kept no group id: whether the folder's group is the one it is in.
    /// Yes when the folder holds keys for it, or one of its recovery keys opens `group.bin`, or
    /// its keyring opens what the other devices said they have seen. No when others said so and
    /// none of it opens. Yes when nothing says either way, as before ids were kept.
    async fn member_without_id(&self, found: [u8; 16]) -> Result<bool> {
        let path = self.path(&format!("devices/{}/keys.bin", self.device));
        match self.target.read(&path).await {
            Ok(bytes) if self.key.unwrap(&self.device, &bytes).is_ok() => return Ok(true),
            Ok(_) | Err(FolderError::NotFound(_)) => {}
            Err(e) => return Err(e.into()),
        }
        let ring = self.keyring();
        if !ring.recovery_keys().is_empty() {
            let bytes = self.target.read(&self.path("group.bin")).await?;
            if ring
                .recovery_keys()
                .iter()
                .any(|r| open_group_file(&bytes, r).is_ok_and(|(group, _)| group == found))
            {
                return Ok(true);
            }
        }
        let mut others = 0;
        for entry in self.target.list(&self.path("devices")).await? {
            if !entry.is_dir || entry.name == self.device {
                continue;
            }
            let path = self.path(&format!("devices/{}/acks.bin", entry.name));
            match self.target.read(&path).await {
                Ok(bytes) if seal::open(&ring, &bytes).is_ok() => return Ok(true),
                Ok(_) => others += 1,
                Err(FolderError::NotFound(_)) => {}
                Err(e) => return Err(e.into()),
            }
        }
        Ok(others == 0)
    }

    /// The keyring now, for the host to keep: it changes when another device rotates the key.
    pub fn keyring(&self) -> Keyring {
        self.ring.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
    }

    fn set_ring(&self, ring: Keyring) {
        *self.ring.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = ring;
    }

    async fn write_own_keys(&self) -> Result<()> {
        let wrapped = wrap_for(&self.key.public(), &self.device, &self.keyring());
        self.replace(&self.path(&format!("devices/{}/keys.bin", self.device)), &wrapped).await
    }

    /// Write a file that is rewritten: a temporary name, then a move over the old one.
    async fn replace(&self, path: &str, bytes: &[u8]) -> Result<()> {
        let tmp = format!("{path}.{}.tmp", hex_id());
        self.target.write_new(&tmp, bytes).await?;
        if let Err(e) = self.target.rename(&tmp, path).await {
            let _ = self.target.remove(&tmp).await;
            return Err(e.into());
        }
        Ok(())
    }

    /// Read what other devices wrote, write what this one has, say what has been seen, and
    /// compact what everyone has seen.
    pub async fn sync(&self) -> Result<FolderReport> {
        self.sync_rotating(false).await
    }

    /// A pass that also rotates the group key when `rotate` (a device was removed here, or a new
    /// recovery key was asked for) or when the current key may have reached a removed device.
    /// The recovery key a rotation made comes back in [`FolderReport::rotated`].
    pub async fn sync_rotating(&self, rotate: bool) -> Result<FolderReport> {
        let mut report = FolderReport::default();
        self.check_same_group().await?;
        let removed: Vec<String> = self
            .store
            .sync_peers()?
            .into_iter()
            .filter(|(_, _, _, removed)| *removed)
            .map(|(d, ..)| d)
            .collect();
        if removed.contains(&self.device) {
            return Err(SyncError::Removed);
        }
        let named_removed = self.target.list(&self.path("removed")).await?;
        if named_removed.iter().any(|e| !e.is_dir && e.name == self.device) {
            return Err(SyncError::Removed);
        }
        let marked: Vec<String> = self
            .target
            .list(&self.path("paused"))
            .await?
            .into_iter()
            .filter(|e| !e.is_dir)
            .map(|e| e.name)
            .collect();
        if marked.contains(&self.device) {
            report.paused = true;
            return Ok(report);
        }
        let paused: Vec<String> = self
            .store
            .sync_regs(kinds::DEVICE_SYNC)?
            .into_iter()
            .filter(|(_, value)| value == "off")
            .map(|(device, _)| device)
            .collect();
        // A rotation by another device reaches this one as new keys.
        self.refresh_keys().await?;

        let devices = self.target.list(&self.path("devices")).await?;
        let present: Vec<String> =
            devices.iter().filter(|e| e.is_dir).map(|e| e.name.clone()).collect();
        for entry in devices.iter().filter(|e| e.is_dir && e.name != self.device) {
            if removed.contains(&entry.name) || paused.contains(&entry.name) {
                continue;
            }
            report.devices_seen += 1;
            self.read_device(&entry.name, &mut report).await?;
        }
        // What was read may be this device's own removal: it writes nothing more.
        let removed_now = self.removed_devices()?;
        if removed_now.contains(&self.device) {
            return Err(SyncError::Removed);
        }
        if rotate || self.keyring().reaches_removed(&removed_now) {
            report.rotated = Some(self.rotate(&removed_now).await?);
        }
        self.share_keys(&present).await?;
        report.recovery = self.check_group().await?;
        // Again before the segments: a group started over while this pass read writes nothing
        // of this device's under the old key into the new group's folder.
        self.check_same_group().await?;
        self.know_writer()?;
        self.write_segments(&mut report).await?;
        self.write_acks().await?;
        // After the segments, so a device that finds a marker can read the register behind it.
        self.mark_paused(&paused, &marked, &removed).await?;
        report.compacted = self
            .compact(&devices.iter().map(|e| e.name.clone()).collect::<Vec<_>>(), &removed)
            .await?;
        Ok(report)
    }

    /// Bring `paused/` in line with the `device_sync` registers as this device has them: a
    /// marker for each device held back, none for the others. Devices that knew less put a
    /// marker back, or take one away, until they read the newer register.
    async fn mark_paused(
        &self,
        paused: &[String],
        marked: &[String],
        removed: &[String],
    ) -> Result<()> {
        let group: Vec<String> = self
            .store
            .sync_peers()?
            .into_iter()
            .filter(|(device, _, _, gone)| !gone && *device != self.device)
            .map(|(device, ..)| device)
            .collect();
        for device in group.iter().filter(|d| !removed.contains(d)) {
            let path = self.path(&format!("paused/{device}"));
            match (paused.contains(device), marked.contains(device)) {
                (true, false) => match self.target.write_new(&path, b"off").await {
                    Ok(()) | Err(FolderError::Exists(_)) => {}
                    Err(e) => return Err(e.into()),
                },
                (false, true) => match self.target.remove(&path).await {
                    Ok(()) | Err(FolderError::NotFound(_)) => {}
                    Err(e) => return Err(e.into()),
                },
                _ => {}
            }
        }
        Ok(())
    }

    /// Whether the group's list names `device`, removed or not.
    fn listed(&self, device: &str) -> Result<bool> {
        Ok(self.store.sync_peers()?.iter().any(|(d, ..)| d == device))
    }

    fn removed_devices(&self) -> Result<Vec<String>> {
        Ok(self
            .store
            .sync_peers()?
            .into_iter()
            .filter(|(_, _, _, removed)| *removed)
            .map(|(d, ..)| d)
            .collect())
    }

    /// Wrap the keyring for each other device in the group's list whenever it is not the one
    /// last wrapped for it, and for one with no place in the folder yet (paired nearby, which can
    /// then open the folder once it signs in to it). `keys.bin` is what every version reads; the
    /// file under `keys/` is this device's alone, so no rotation that ran at once writes over it.
    async fn share_keys(&self, present: &[String]) -> Result<()> {
        let ring = self.keyring();
        let fp = ring.fingerprint();
        for (device, _, pubkey, removed) in self.store.sync_peers()? {
            if removed || device == self.device {
                continue;
            }
            let Some(pubkey) = pubkey.as_deref().and_then(crate::keys::public_of) else {
                continue;
            };
            let shared = self.meta(&format!("keys.{device}"));
            let was = self.store.sync_meta_get(&shared)?;
            if present.contains(&device) && was.as_deref() == Some(fp.as_str()) {
                continue;
            }
            let wrapped = wrap_for(&pubkey, &device, &ring);
            self.replace(&self.path(&format!("devices/{device}/keys.bin")), &wrapped).await?;
            if was.as_deref() == Some(fp.as_str()) {
                continue;
            }
            let mine = format!("keys/{device}/{}-{fp}.bin", self.device);
            match self.target.write_new(&self.path(&mine), &wrapped).await {
                Ok(()) | Err(FolderError::Exists(_)) => {}
                Err(e) => return Err(e.into()),
            }
            if let Some(old) = was.filter(|old| *old != fp) {
                let old = format!("keys/{device}/{}-{old}.bin", self.device);
                match self.target.remove(&self.path(&old)).await {
                    Ok(()) | Err(FolderError::NotFound(_)) => {}
                    Err(e) => return Err(e.into()),
                }
            }
            self.store.sync_meta_set(&shared, &fp)?;
        }
        Ok(())
    }

    /// Merge the keyrings wrapped for this device: `keys.bin`, and each file under `keys/` not
    /// read before.
    async fn refresh_keys(&self) -> Result<()> {
        let mut found = Vec::new();
        let path = self.path(&format!("devices/{}/keys.bin", self.device));
        match self.target.read(&path).await {
            Ok(bytes) => found.push(bytes),
            Err(FolderError::NotFound(_)) => {}
            Err(e) => return Err(e.into()),
        }
        let dir = format!("keys/{}", self.device);
        let names: Vec<String> = self
            .target
            .list(&self.path(&dir))
            .await?
            .into_iter()
            .filter(|e| !e.is_dir && e.name.ends_with(".bin"))
            .map(|e| e.name)
            .collect();
        let seen_key = self.meta("keys.read");
        let seen: Vec<String> = self
            .store
            .sync_meta_get(&seen_key)?
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();
        for name in names.iter().filter(|n| !seen.contains(n)) {
            match self.target.read(&self.path(&format!("{dir}/{name}"))).await {
                Ok(bytes) => found.push(bytes),
                Err(FolderError::NotFound(_)) => {}
                Err(e) => return Err(e.into()),
            }
        }
        for bytes in found {
            if let Ok(theirs) = self.key.unwrap(&self.device, &bytes) {
                self.set_ring(self.keyring().merged(&theirs));
            }
        }
        if names != seen {
            let json =
                serde_json::to_string(&names).map_err(|_| SyncError::Damaged("encoding"))?;
            self.store.sync_meta_set(&seen_key, &json)?;
        }
        Ok(())
    }

    /// When the keyring changed since the last look: which of its recovery keys opens
    /// `group.bin`, and, when one does and `group.bin` lacks a key this device holds, the
    /// keyring written there again under that same recovery key. A recovery key none of them
    /// matches (made by a version that does not share it) is left alone.
    async fn check_group(&self) -> Result<Option<RecoveryKey>> {
        let ring = self.keyring();
        let candidates = ring.recovery_keys();
        let checked = self.meta("group.checked");
        let fp = ring.fingerprint();
        if candidates.is_empty()
            || self.store.sync_meta_get(&checked)?.as_deref() == Some(fp.as_str())
        {
            return Ok(None);
        }
        let bytes = self.target.read(&self.path("group.bin")).await?;
        let mut found = None;
        for recovery in candidates {
            if let Ok((_, held)) = open_group_file(&bytes, &recovery) {
                if !held.holds(&ring) {
                    self.write_group(&recovery, &held.merged(&ring)).await?;
                }
                found = Some(recovery);
                break;
            }
        }
        self.store.sync_meta_set(&checked, &fp)?;
        Ok(found)
    }

    async fn read_device(&self, device: &str, report: &mut FolderReport) -> Result<()> {
        let dir = self.path(&format!("devices/{device}"));
        let cursor_key = format!("cur.{device}");
        let mut cursor = self.meta_u64(&cursor_key)?;
        let mut numbers: Vec<u64> = self
            .target
            .list(&dir)
            .await?
            .iter()
            .filter(|e| !e.is_dir)
            .filter_map(|e| e.name.strip_suffix(".seg")?.parse().ok())
            .filter(|n| *n > cursor)
            .collect();
        numbers.sort_unstable();
        for n in numbers {
            let bytes = self.target.read(&format!("{dir}/{n:06}.seg")).await?;
            let opened = match seal::open(&self.keyring(), &bytes) {
                Ok(o) => o,
                Err(SyncError::KeyMismatch) => {
                    // Written under a key this device may be sent in a moment.
                    self.refresh_keys().await?;
                    match seal::open(&self.keyring(), &bytes) {
                        Ok(o) => o,
                        // A device the group never listed, writing under a key it never had:
                        // one of a group this folder held before, still writing. Skipped, so
                        // the group's own devices go on syncing.
                        Err(SyncError::KeyMismatch) if !self.listed(device)? => return Ok(()),
                        Err(e) => return Err(e),
                    }
                }
                Err(e) => return Err(e),
            };
            if opened.kind != Kind::Segment || opened.device != device || opened.number != n {
                return Err(SyncError::Damaged("a segment under another name"));
            }
            let ops: Vec<Op> = serde_json::from_slice(&opened.plain)
                .map_err(|_| SyncError::Damaged("segment contents"))?;
            let applied = self.store.apply_ops(ops)?;
            let complete = applied.gaps == 0;
            report.segments_read += 1;
            report.absorb(applied);
            if !complete {
                // A segment this one builds on has not arrived yet; read on a later pass.
                break;
            }
            cursor = n;
            self.store.sync_meta_set(&self.meta(&cursor_key), &cursor.to_string())?;
        }
        self.store.sync_touch_peer(device, now_secs())?;
        Ok(())
    }

    /// What this device wrote is counted for the id it wrote under: a device that moved to a new
    /// id (another copy of it writes under the old one) starts its own directory from the start.
    fn know_writer(&self) -> Result<()> {
        let key = self.meta("writer");
        match self.store.sync_meta_get(&key)? {
            Some(writer) if writer == self.device => return Ok(()),
            Some(_) => {
                for what in ["published", "segno", "acked"] {
                    self.store.sync_meta_remove(&self.meta(what))?;
                }
            }
            None => {}
        }
        self.store.sync_meta_set(&key, &self.device)?;
        Ok(())
    }

    async fn write_segments(&self, report: &mut FolderReport) -> Result<()> {
        loop {
            let published = self.meta_u64("published")?;
            let mut batch = self.store.ops_of(&self.device, published, 5_000)?;
            if batch.is_empty() {
                return Ok(());
            }
            // Cut to the segment size.
            let mut plain_len = 2;
            let mut keep = 0;
            for op in &batch {
                plain_len += serde_json::to_vec(op).map_or(0, |v| v.len()) + 1;
                if plain_len > SEGMENT_BYTES && keep > 0 {
                    break;
                }
                keep += 1;
            }
            batch.truncate(keep);
            let last = batch.last().map_or(published, |o| o.seq);
            let plain =
                serde_json::to_vec(&batch).map_err(|_| SyncError::Damaged("encoding"))?;
            let mut number = self.meta_u64("segno")? + 1;
            loop {
                let sealed =
                    seal::seal(&self.keyring(), Kind::Segment, &self.device, number, &plain)?;
                let path = self.path(&format!("devices/{}/{number:06}.seg", self.device));
                match self.target.write_new(&path, &sealed).await {
                    Ok(()) => break,
                    Err(FolderError::Exists(_)) => number += 1,
                    Err(e) => return Err(e.into()),
                }
            }
            self.store.sync_meta_set(&self.meta("segno"), &number.to_string())?;
            self.store.sync_meta_set(&self.meta("published"), &last.to_string())?;
            report.segments_written += 1;
            report.ops_written += batch.len();
        }
    }

    async fn write_acks(&self) -> Result<()> {
        let vector = self.store.version_vector()?;
        let plain = serde_json::to_vec(&vector).map_err(|_| SyncError::Damaged("encoding"))?;
        if self.store.sync_meta_get(&self.meta("acked"))?.as_deref()
            == Some(&hex::encode(&plain))
        {
            return Ok(());
        }
        let sealed = seal::seal(&self.keyring(), Kind::Acks, &self.device, 0, &plain)?;
        self.replace(&self.path(&format!("devices/{}/acks.bin", self.device)), &sealed).await?;
        self.store.sync_meta_set(&self.meta("acked"), &hex::encode(&plain))?;
        Ok(())
    }

    /// Compact what every active device has acknowledged. Skipped when any of them has not yet
    /// said what it has seen.
    async fn compact(&self, devices: &[String], removed: &[String]) -> Result<usize> {
        let mut floor: Option<VersionVector> = None;
        let mut counted = 0;
        for device in devices.iter().filter(|d| !removed.contains(d)) {
            let vector: VersionVector = if *device == self.device {
                self.store.version_vector()?
            } else {
                let path = self.path(&format!("devices/{device}/acks.bin"));
                let bytes = match self.target.read(&path).await {
                    Ok(bytes) => bytes,
                    // A device with only the keys another wrapped for it has never read the
                    // folder: it counts as not here yet, as one with no place at all does.
                    Err(FolderError::NotFound(_)) if self.only_keys(device).await => continue,
                    Err(_) => return Ok(0),
                };
                let Ok(opened) = seal::open(&self.keyring(), &bytes) else { return Ok(0) };
                serde_json::from_slice(&opened.plain).unwrap_or_default()
            };
            counted += 1;
            floor = Some(match floor {
                None => vector,
                Some(mut min) => {
                    let keys: Vec<String> =
                        min.keys().cloned().chain(vector.keys().cloned()).collect();
                    for k in keys {
                        let a = min.get(&k).copied().unwrap_or(0);
                        let b = vector.get(&k).copied().unwrap_or(0);
                        min.insert(k, a.min(b));
                    }
                    min
                }
            });
        }
        match floor {
            Some(floor) if counted > 1 => Ok(self.store.compact(&floor)?),
            _ => Ok(0),
        }
    }

    async fn only_keys(&self, device: &str) -> bool {
        self.target
            .list(&self.path(&format!("devices/{device}")))
            .await
            .is_ok_and(|entries| entries.iter().all(|e| e.name == "keys.bin"))
    }

    /// Rotate, as after removing a device: a new group key kept from every device in `removed`,
    /// and a new recovery key, both in the keyring, with `group.bin` under the new recovery key.
    /// No old recovery key is needed. The others get the keyring as the pass goes on
    /// ([`Self::share_keys`]); a removed device keeps what it had, reads nothing written from
    /// here on, and finds its name under `removed/`.
    async fn rotate(&self, removed: &[String]) -> Result<RecoveryKey> {
        let recovery = RecoveryKey::generate();
        let ring = self.keyring().rotated_from(removed, &recovery);
        self.set_ring(ring.clone());
        // This device's own copy first, so the new keys are never only in memory.
        self.write_own_keys().await?;
        self.write_group(&recovery, &ring).await?;
        for device in removed.iter().filter(|d| **d != self.device) {
            match self.target.write_new(&self.path(&format!("removed/{device}")), b"").await {
                Ok(()) | Err(FolderError::Exists(_)) => {}
                Err(e) => return Err(e.into()),
            }
        }
        Ok(recovery)
    }

    /// Make a new recovery key, rotating the group key with it, in a pass: the old one stops
    /// working at once, and the others learn the new one on their next pass.
    pub async fn new_recovery_key(&self) -> Result<RecoveryKey> {
        self.sync_rotating(true)
            .await?
            .rotated
            .ok_or(SyncError::Damaged("a rotation that made no key"))
    }

    async fn write_group(&self, recovery: &RecoveryKey, ring: &Keyring) -> Result<()> {
        let path = self.path("group.bin");
        let old = self.target.read(&path).await?;
        let group: [u8; 16] = old
            .get(4..20)
            .and_then(|g| g.try_into().ok())
            .ok_or(SyncError::Damaged("group.bin"))?;
        self.replace(&path, &group_file(&group, recovery, ring)).await
    }
}

/// Everything a device holds, as one sealed file under the recovery key: Export for people with
/// no folder. Importing it applies the ops, so history, choices and the device list come back.
pub fn export_backup(store: &Store, recovery: &RecoveryKey) -> Result<Vec<u8>> {
    let ops = store.ops_after(&BTreeMap::new(), usize::MAX)?;
    let plain = serde_json::to_vec(&ops).map_err(|_| SyncError::Damaged("encoding"))?;
    seal::seal(
        &Keyring::single(recovery.wrap_key()),
        Kind::Backup,
        "0000000000000000",
        0,
        &plain,
    )
}

/// Read a backup back. A wrong recovery key is `WrongRecoveryKey`, and nothing is applied.
pub fn import_backup(store: &Store, recovery: &RecoveryKey, bytes: &[u8]) -> Result<Applied> {
    let opened =
        seal::open(&Keyring::single(recovery.wrap_key()), bytes).map_err(|e| match e {
            SyncError::KeyMismatch => SyncError::WrongRecoveryKey,
            other => other,
        })?;
    let ops: Vec<Op> = serde_json::from_slice(&opened.plain)
        .map_err(|_| SyncError::Damaged("backup contents"))?;
    Ok(store.apply_ops(ops)?)
}

/// Remove one file; one already gone is done.
async fn remove_file(target: &dyn FolderTarget, path: &str) -> Result<()> {
    match target.remove(path).await {
        Ok(()) | Err(FolderError::NotFound(_)) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

fn join(prefix: &str, rest: &str) -> String {
    if prefix.is_empty() { rest.to_owned() } else { format!("{prefix}/{rest}") }
}

fn hex_id() -> String {
    hex::encode(random::<4>())
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicI64, Ordering};

    use kaeri_core::ids::AnilistId;
    use kaeri_store::WatchEvent;

    use super::*;
    use crate::folder::MemoryTarget;

    const SHOW: AnilistId = AnilistId::new(7_001);

    struct Dev {
        store: Store,
        clock: Arc<AtomicI64>,
        id: String,
        key: [u8; 32],
    }

    fn dev(id: &str, secs: i64) -> Dev {
        let store = Store::open_in_memory().unwrap();
        let clock = Arc::new(AtomicI64::new(secs * 1000));
        let c = clock.clone();
        store.set_sync_clock(move || c.load(Ordering::SeqCst));
        store.sync_enable(id).unwrap();
        Dev { store, clock, id: id.to_owned(), key: random() }
    }

    impl Dev {
        fn identity(&self) -> DeviceKey {
            DeviceKey::from_identity(&self.key)
        }

        fn watch(&self, ep: &str, pos: f64) {
            let now = self.clock.fetch_add(1000, Ordering::SeqCst) + 1000;
            let mut e = WatchEvent::new(SHOW, ep, pos);
            e.duration_secs = Some(1440.0);
            e.at = now / 1000;
            self.store.record_event(&e).unwrap();
        }

        fn resume(&self, ep: &str) -> Option<f64> {
            self.store.resume_position(SHOW, ep).unwrap()
        }
    }

    async fn pair(
        target: &Arc<dyn FolderTarget>,
    ) -> (Dev, FolderRoad, Dev, FolderRoad, RecoveryKey) {
        let a = dev("00000000000000a1", 1_000);
        let b = dev("00000000000000b2", 1_000);
        let (ra, recovery) =
            FolderRoad::create(target.clone(), "kaeri", a.store.clone(), &a.id, a.identity())
                .await
                .unwrap();
        let rb = FolderRoad::join(
            target.clone(),
            "kaeri",
            b.store.clone(),
            &b.id,
            b.identity(),
            &recovery,
        )
        .await
        .unwrap();
        (a, ra, b, rb, recovery)
    }

    #[tokio::test]
    async fn a_device_held_back_neither_reads_nor_is_read_until_its_sync_is_back_on() {
        let target: Arc<dyn FolderTarget> = Arc::new(MemoryTarget::default());
        let (a, ra, b, rb, _) = pair(&target).await;
        ra.sync().await.unwrap();
        rb.sync().await.unwrap();
        ra.sync().await.unwrap();

        // A turns B's Sync off: the register, then a pass that leaves B's marker.
        a.store.sync_register(kinds::DEVICE_SYNC, &b.id, Some("off"), None).unwrap();
        ra.sync().await.unwrap();
        assert!(target.read("kaeri/paused/00000000000000b2").await.is_ok());
        a.watch("1", 300.0);
        ra.sync().await.unwrap();
        b.watch("2", 600.0);
        let r = rb.sync().await.unwrap();
        assert!(r.paused);
        assert_eq!((r.ops_applied, r.ops_written), (0, 0));
        assert_eq!(b.resume("1"), None);
        ra.sync().await.unwrap();
        assert_eq!(a.resume("2"), None);

        // Back on: the marker goes with A's next pass, and both catch up.
        a.store.sync_register(kinds::DEVICE_SYNC, &b.id, None, None).unwrap();
        ra.sync().await.unwrap();
        assert!(target.read("kaeri/paused/00000000000000b2").await.is_err());
        let r = rb.sync().await.unwrap();
        assert!(!r.paused);
        assert_eq!(b.resume("1"), Some(300.0));
        ra.sync().await.unwrap();
        assert_eq!(a.resume("2"), Some(600.0));
        // B read the register that let it back in, and leaves no marker of its own.
        assert!(!b.store.sync_device_paused(&b.id));
    }

    #[tokio::test]
    async fn two_devices_meet_through_the_folder() {
        let target: Arc<dyn FolderTarget> = Arc::new(MemoryTarget::default());
        let (a, ra, b, rb, _) = pair(&target).await;
        a.watch("1", 300.0);
        let r = ra.sync().await.unwrap();
        assert!(r.ops_written > 0);
        let r = rb.sync().await.unwrap();
        assert!(r.ops_applied > 0);
        assert_eq!(b.resume("1"), Some(300.0));
        b.watch("1", 900.0);
        rb.sync().await.unwrap();
        ra.sync().await.unwrap();
        assert_eq!(a.resume("1"), Some(900.0));
        // Nothing new: nothing written, nothing applied.
        let r = ra.sync().await.unwrap();
        assert_eq!((r.ops_written, r.ops_applied), (0, 0));
    }

    /// Two devices that know each other from pairing nearby: each in the other's list, with its
    /// key, and no folder yet.
    fn nearby() -> (Dev, Dev) {
        let (a, b) = (dev("00000000000000a1", 1_000), dev("00000000000000b2", 1_000));
        for d in [&a, &b] {
            for other in [&a, &b] {
                d.store
                    .sync_device_add(&other.id, "x", &other.identity().public_hex())
                    .unwrap();
            }
        }
        (a, b)
    }

    #[tokio::test]
    async fn a_nearby_group_gains_a_folder_and_its_other_device_opens_it_without_the_key() {
        let target: Arc<dyn FolderTarget> = Arc::new(MemoryTarget::default());
        let (a, b) = nearby();
        let (ra, _recovery) =
            FolderRoad::create(target.clone(), "kaeri", a.store.clone(), &a.id, a.identity())
                .await
                .unwrap();
        a.watch("1", 300.0);
        ra.sync().await.unwrap();
        // The other device was wrapped for: it opens the folder with its own key.
        let rb = FolderRoad::open_as_member(
            target.clone(),
            "kaeri",
            b.store.clone(),
            &b.id,
            b.identity(),
        )
        .await
        .unwrap();
        assert_eq!(rb.keyring(), ra.keyring());
        rb.sync().await.unwrap();
        assert_eq!(b.resume("1"), Some(300.0));
        // A device outside the group finds a group it has no key for; an empty folder has none.
        let c = dev("00000000000000c3", 1_000);
        let stranger = FolderRoad::open_as_member(
            target.clone(),
            "kaeri",
            c.store.clone(),
            &c.id,
            c.identity(),
        )
        .await;
        assert!(matches!(stranger, Err(SyncError::GroupExists)), "{stranger:?}");
        let empty: Arc<dyn FolderTarget> = Arc::new(MemoryTarget::default());
        let none =
            FolderRoad::open_as_member(empty, "kaeri", c.store.clone(), &c.id, c.identity())
                .await;
        assert!(matches!(none, Err(SyncError::NoGroup)), "{none:?}");
    }

    #[tokio::test]
    async fn a_device_paired_later_gets_its_keys_on_the_next_pass_and_compaction_waits_for_none()
     {
        let target: Arc<dyn FolderTarget> = Arc::new(MemoryTarget::default());
        let a = dev("00000000000000a1", 1_000);
        let (ra, _) =
            FolderRoad::create(target.clone(), "kaeri", a.store.clone(), &a.id, a.identity())
                .await
                .unwrap();
        let b = dev("00000000000000b2", 1_000);
        a.store.sync_device_add(&b.id, "Phone", &b.identity().public_hex()).unwrap();
        ra.sync().await.unwrap();
        let rb = FolderRoad::open_as_member(
            target.clone(),
            "kaeri",
            b.store.clone(),
            &b.id,
            b.identity(),
        )
        .await
        .unwrap();
        assert_eq!(rb.keyring(), ra.keyring());
        // A device with only its keys in the folder does not hold compaction back.
        let c = dev("00000000000000c3", 1_000);
        a.store.sync_device_add(&c.id, "TV", &c.identity().public_hex()).unwrap();
        ra.sync().await.unwrap();
        rb.sync().await.unwrap();
        ra.sync().await.unwrap();
        assert!(ra.only_keys(&c.id).await);
        a.store.sync_register("setting", "theme", Some("\"dark\""), None).unwrap();
        a.store.sync_register("setting", "theme", Some("\"light\""), None).unwrap();
        ra.sync().await.unwrap();
        rb.sync().await.unwrap();
        let r = ra.sync().await.unwrap();
        assert!(r.compacted > 0, "{r:?}");
    }

    #[tokio::test]
    async fn a_group_moves_to_another_folder_with_the_same_keys_and_recovery_key() {
        let first: Arc<dyn FolderTarget> = Arc::new(MemoryTarget::default());
        let (a, _, b, _, _) = pair(&first).await;
        let target: Arc<dyn FolderTarget> = Arc::new(MemoryTarget::default());
        let ring = Keyring::generate();
        let recovery = RecoveryKey::generate();
        let ra = FolderRoad::create_with(
            target.clone(),
            "kaeri",
            a.store.clone(),
            &a.id,
            a.identity(),
            ring.clone(),
            &recovery,
        )
        .await
        .unwrap();
        a.watch("4", 50.0);
        ra.sync().await.unwrap();
        // The recovery key opens the new folder, and creating over it is refused.
        let joined = FolderRoad::join(
            target.clone(),
            "kaeri",
            b.store.clone(),
            &b.id,
            b.identity(),
            &recovery,
        )
        .await
        .unwrap();
        assert_eq!(joined.keyring(), ring);
        joined.sync().await.unwrap();
        assert_eq!(b.resume("4"), Some(50.0));
        let again = FolderRoad::create_with(
            target,
            "kaeri",
            a.store.clone(),
            &a.id,
            a.identity(),
            ring,
            &recovery,
        )
        .await;
        assert!(matches!(again, Err(SyncError::GroupExists)));
    }

    #[tokio::test]
    async fn a_device_offline_catches_up_in_order() {
        let target: Arc<dyn FolderTarget> = Arc::new(MemoryTarget::default());
        let (a, ra, b, rb, _) = pair(&target).await;
        for n in 1..=5 {
            a.watch(&n.to_string(), 100.0);
            ra.sync().await.unwrap();
        }
        rb.sync().await.unwrap();
        for n in 1..=5 {
            assert_eq!(b.resume(&n.to_string()), Some(100.0));
        }
    }

    #[tokio::test]
    async fn a_wrong_recovery_key_is_refused_and_nothing_is_read() {
        let target: Arc<dyn FolderTarget> = Arc::new(MemoryTarget::default());
        let (_a, _ra, _b, _rb, _) = pair(&target).await;
        let c = dev("00000000000000c3", 1_000);
        let err = FolderRoad::join(
            target.clone(),
            "kaeri",
            c.store.clone(),
            &c.id,
            c.identity(),
            &RecoveryKey::generate(),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, SyncError::WrongRecoveryKey));
        let err = FolderRoad::join(
            target.clone(),
            "empty",
            c.store.clone(),
            &c.id,
            c.identity(),
            &RecoveryKey::generate(),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, SyncError::NoGroup));
    }

    /// Every file in the folder, for a test to see that nothing was written.
    async fn target_paths(target: &Arc<dyn FolderTarget>) -> Vec<String> {
        let mut out = Vec::new();
        let mut dirs = vec![String::new()];
        while let Some(dir) = dirs.pop() {
            for e in target.list(&dir).await.unwrap() {
                let path = if dir.is_empty() { e.name } else { format!("{dir}/{}", e.name) };
                if e.is_dir { dirs.push(path) } else { out.push(path) }
            }
        }
        out.sort();
        out
    }

    /// A folder whose removes start failing after `left` of them, as a service that stops
    /// answering part way through.
    struct Stopping {
        inner: MemoryTarget,
        left: std::sync::atomic::AtomicUsize,
    }

    #[async_trait::async_trait]
    impl FolderTarget for Stopping {
        async fn list(
            &self,
            dir: &str,
        ) -> std::result::Result<Vec<crate::folder::Entry>, FolderError> {
            self.inner.list(dir).await
        }
        async fn read(&self, path: &str) -> std::result::Result<Vec<u8>, FolderError> {
            self.inner.read(path).await
        }
        async fn write_new(
            &self,
            path: &str,
            bytes: &[u8],
        ) -> std::result::Result<(), FolderError> {
            self.inner.write_new(path, bytes).await
        }
        async fn rename(&self, from: &str, to: &str) -> std::result::Result<(), FolderError> {
            self.inner.rename(from, to).await
        }
        async fn remove(&self, path: &str) -> std::result::Result<(), FolderError> {
            let left = self.left.load(Ordering::SeqCst);
            if left == 0 {
                return Err(FolderError::Unreachable("stopped".into()));
            }
            self.left.store(left - 1, Ordering::SeqCst);
            self.inner.remove(path).await
        }
        fn describe(&self) -> String {
            "stopping".into()
        }
    }

    #[tokio::test]
    async fn clearing_removes_what_the_road_owns_with_the_group_last_and_resumes() {
        let memory = MemoryTarget::default();
        let target: Arc<dyn FolderTarget> = Arc::new(memory.clone());
        let (a, ra, b, rb, _) = pair(&target).await;
        announce(&[&a, &b]);
        a.watch("1", 300.0);
        ra.sync().await.unwrap();
        rb.sync().await.unwrap();
        a.store.sync_register(kinds::DEVICE_SYNC, &b.id, Some("off"), None).unwrap();
        ra.sync().await.unwrap();
        memory.put_raw("kaeri/group.bin.0badf00d.tmp", b"half".to_vec());
        memory.put_raw("kaeri/notes.txt", b"the person's".to_vec());
        memory.put_raw("elsewhere/keep.seg", b"not ours".to_vec());
        let owned = target_paths(&target).await;
        assert!(owned.iter().any(|p| p.starts_with("kaeri/paused/")), "{owned:?}");

        // It stops part way: the group is still there, so the folder still reads as a group's.
        let stopping = Stopping { inner: memory.clone(), left: 3.into() };
        let err = FolderRoad::clear(&stopping, "kaeri").await.unwrap_err();
        assert!(matches!(err, SyncError::Folder(FolderError::Unreachable(_))), "{err:?}");
        assert!(FolderRoad::group_exists(&*target, "kaeri").await.unwrap());
        assert!(target_paths(&target).await.len() < owned.len());

        // Run again, it finishes, and only what the road never wrote is left.
        FolderRoad::clear(&*target, "kaeri").await.unwrap();
        assert_eq!(
            target_paths(&target).await,
            ["elsewhere/keep.seg", "kaeri/notes.txt"].map(String::from)
        );
        assert!(!FolderRoad::group_exists(&*target, "kaeri").await.unwrap());
        FolderRoad::clear(&*target, "kaeri").await.unwrap();
    }

    #[tokio::test]
    async fn a_device_of_a_group_started_over_stops_and_joins_the_new_one_with_its_history() {
        let target: Arc<dyn FolderTarget> = Arc::new(MemoryTarget::default());
        let (a, ra, b, rb, _) = pair(&target).await;
        announce(&[&a, &b]);
        a.watch("1", 300.0);
        b.watch("2", 600.0);
        ra.sync().await.unwrap();
        rb.sync().await.unwrap();
        ra.sync().await.unwrap();

        // C lost the key: it clears the folder. Until the new group is there, B writes nothing.
        let c = dev("00000000000000c3", 1_000);
        FolderRoad::clear(&*target, "kaeri").await.unwrap();
        b.watch("3", 900.0);
        assert!(matches!(rb.sync().await.unwrap_err(), SyncError::GroupReplaced));
        assert!(target_paths(&target).await.is_empty());

        // C sets a new group up. A and B find it replaced and write nothing into it.
        let (rc, recovery) =
            FolderRoad::create(target.clone(), "kaeri", c.store.clone(), &c.id, c.identity())
                .await
                .unwrap();
        announce(&[&c]);
        c.watch("4", 120.0);
        rc.sync().await.unwrap();
        let theirs = target_paths(&target).await;
        assert!(matches!(rb.sync().await.unwrap_err(), SyncError::GroupReplaced));
        assert!(matches!(ra.sync().await.unwrap_err(), SyncError::GroupReplaced));
        assert_eq!(target_paths(&target).await, theirs);
        // Opened again from what the store kept, it still knows its old group.
        let reopened = FolderRoad::open(
            target.clone(),
            "kaeri",
            b.store.clone(),
            &b.id,
            b.identity(),
            rb.keyring(),
        );
        assert!(matches!(reopened.sync().await.unwrap_err(), SyncError::GroupReplaced));

        // B joins with the new key, as a device of the new group, and brings what it holds.
        let rb = join_with(&target, &b, &recovery).await;
        rb.sync().await.unwrap();
        assert_eq!(b.resume("4"), Some(120.0));
        rc.sync().await.unwrap();
        assert_eq!(c.resume("2"), Some(600.0));
        assert_eq!(c.resume("3"), Some(900.0));
        // What B had read from A came with A's own segments, which the new group never had.
        assert_eq!(c.resume("1"), None);
    }

    #[tokio::test]
    async fn a_device_that_kept_no_group_id_takes_its_own_and_not_a_new_one() {
        let target: Arc<dyn FolderTarget> = Arc::new(MemoryTarget::default());
        let (a, ra, b, rb, _) = pair(&target).await;
        announce(&[&a, &b]);
        ra.sync().await.unwrap();
        rb.sync().await.unwrap();
        // As a version that kept no id left it.
        b.store.sync_meta_remove("folder.kaeri.group").unwrap();
        let open = |d: &Dev, ring: Keyring| {
            FolderRoad::open(
                target.clone(),
                "kaeri",
                d.store.clone(),
                &d.id,
                d.identity(),
                ring,
            )
        };
        open(&b, rb.keyring()).sync().await.unwrap();
        assert!(b.store.sync_meta_get("folder.kaeri.group").unwrap().is_some());

        // Started over by C: without an id, B still sees the group is not its own.
        b.store.sync_meta_remove("folder.kaeri.group").unwrap();
        FolderRoad::clear(&*target, "kaeri").await.unwrap();
        let c = dev("00000000000000c3", 1_000);
        let (rc, _) =
            FolderRoad::create(target.clone(), "kaeri", c.store.clone(), &c.id, c.identity())
                .await
                .unwrap();
        rc.sync().await.unwrap();
        let err = open(&b, rb.keyring()).sync().await.unwrap_err();
        assert!(matches!(err, SyncError::GroupReplaced), "{err:?}");
    }

    #[tokio::test]
    async fn segments_a_listed_device_never_wrote_under_the_groups_key_are_skipped() {
        let target: Arc<dyn FolderTarget> = Arc::new(MemoryTarget::default());
        let (a, ra, b, rb, _) = pair(&target).await;
        announce(&[&a, &b]);
        a.watch("1", 300.0);
        ra.sync().await.unwrap();
        // A device of some other group wrote into this folder.
        let stray =
            seal::seal(&Keyring::generate(), Kind::Segment, "00000000000000e5", 1, b"[]")
                .unwrap();
        target.write_new("kaeri/devices/00000000000000e5/000001.seg", &stray).await.unwrap();
        rb.sync().await.unwrap();
        assert_eq!(b.resume("1"), Some(300.0));
    }

    #[tokio::test]
    async fn creating_over_a_group_is_refused() {
        let target: Arc<dyn FolderTarget> = Arc::new(MemoryTarget::default());
        let (_a, _ra, ..) = pair(&target).await;
        let c = dev("00000000000000c3", 1_000);
        let err =
            FolderRoad::create(target.clone(), "kaeri", c.store.clone(), &c.id, c.identity())
                .await
                .unwrap_err();
        assert!(matches!(err, SyncError::GroupExists));
    }

    #[tokio::test]
    async fn a_stranger_with_the_wrong_key_reads_nothing() {
        let target: Arc<dyn FolderTarget> = Arc::new(MemoryTarget::default());
        let (a, ra, ..) = pair(&target).await;
        a.watch("1", 300.0);
        ra.sync().await.unwrap();
        // The folder holds sealed bytes only: no title id, no episode in any file name or body.
        for entry in target.list("kaeri/devices").await.unwrap() {
            assert!(!entry.name.contains("7001"));
        }
        // A device holding some other group's keyring reads the folder and gets a mismatch.
        let s = dev("00000000000000d4", 1_000);
        let other = FolderRoad::open(
            target.clone(),
            "kaeri",
            s.store.clone(),
            &s.id,
            s.identity(),
            Keyring::generate(),
        );
        // The folder holds no keys for it, and nothing the others wrote opens with its keyring:
        // the group is not its own, and it reads and writes nothing.
        let before = target_paths(&target).await;
        let err = other.sync().await.unwrap_err();
        assert!(matches!(err, SyncError::GroupReplaced), "{err:?}");
        assert_eq!(target_paths(&target).await, before);
        assert_eq!(s.resume("1"), None);
    }

    /// Each device in the group's list with its key, as the engine says it on joining.
    fn announce(devices: &[&Dev]) {
        for d in devices {
            d.store.sync_device_add(&d.id, "device", &d.identity().public_hex()).unwrap();
        }
    }

    async fn join_with(
        target: &Arc<dyn FolderTarget>,
        d: &Dev,
        key: &RecoveryKey,
    ) -> FolderRoad {
        FolderRoad::join(target.clone(), "kaeri", d.store.clone(), &d.id, d.identity(), key)
            .await
            .unwrap()
    }

    /// The newest segment `device` wrote, as the folder holds it.
    async fn newest_segment(target: &Arc<dyn FolderTarget>, device: &str) -> Vec<u8> {
        let dir = format!("kaeri/devices/{device}");
        let last = target
            .list(&dir)
            .await
            .unwrap()
            .into_iter()
            .filter(|e| e.name.ends_with(".seg"))
            .map(|e| e.name)
            .max()
            .expect("a segment");
        target.read(&format!("{dir}/{last}")).await.unwrap()
    }

    #[tokio::test]
    async fn a_removal_rotates_both_keys_and_the_removed_device_reads_nothing_new() {
        let target: Arc<dyn FolderTarget> = Arc::new(MemoryTarget::default());
        let (a, ra, b, rb, old) = pair(&target).await;
        let c = dev("00000000000000c3", 1_000);
        let rc = join_with(&target, &c, &old).await;
        announce(&[&a, &b, &c]);
        for _ in 0..2 {
            for r in [&ra, &rb, &rc] {
                r.sync().await.unwrap();
            }
        }
        // B removes C: B never needs the recovery key for it.
        b.store.sync_device_remove(&c.id).unwrap();
        let fresh = rb.sync_rotating(true).await.unwrap().rotated.expect("a new recovery key");
        b.watch("1", 500.0);
        rb.sync().await.unwrap();
        // A learns the new keys and the new recovery key on its next pass.
        let seen = ra.sync().await.unwrap();
        assert_eq!(seen.recovery.as_ref(), Some(&fresh));
        assert_eq!(a.resume("1"), Some(500.0));
        assert_eq!(ra.keyring().current_epoch(), rb.keyring().current_epoch());
        // C is told it was removed, cannot open what B wrote since, and the recovery key it knew
        // no longer opens the folder.
        assert!(matches!(rc.sync().await, Err(SyncError::Removed)));
        assert_eq!(c.resume("1"), None);
        let after = newest_segment(&target, &b.id).await;
        assert!(matches!(seal::open(&rc.keyring(), &after), Err(SyncError::KeyMismatch)));
        let d = dev("00000000000000d4", 1_000);
        let joined = FolderRoad::join(
            target.clone(),
            "kaeri",
            d.store.clone(),
            &d.id,
            d.identity(),
            &old,
        );
        assert!(matches!(joined.await, Err(SyncError::WrongRecoveryKey)));
        join_with(&target, &d, &fresh).await;
        // Joining or opening the folder again under the id the group removed is refused.
        let again = FolderRoad::join(
            target.clone(),
            "kaeri",
            c.store.clone(),
            &c.id,
            c.identity(),
            &fresh,
        );
        assert!(matches!(again.await, Err(SyncError::Removed)));
        let open = FolderRoad::open_as_member(
            target.clone(),
            "kaeri",
            c.store.clone(),
            &c.id,
            c.identity(),
        );
        assert!(matches!(open.await, Err(SyncError::Removed)));
    }

    #[tokio::test]
    async fn two_removals_at_once_end_on_one_key_neither_removed_device_holds() {
        let target: Arc<dyn FolderTarget> = Arc::new(MemoryTarget::default());
        let (a, ra, b, rb, old) = pair(&target).await;
        let c = dev("00000000000000c3", 1_000);
        let d = dev("00000000000000d4", 1_000);
        let rc = join_with(&target, &c, &old).await;
        let rd = join_with(&target, &d, &old).await;
        announce(&[&a, &b, &c, &d]);
        for _ in 0..2 {
            for r in [&ra, &rb, &rc, &rd] {
                r.sync().await.unwrap();
            }
        }
        // A removes C and B removes D, and both rotate before either reads the other's removal.
        a.store.sync_device_remove(&c.id).unwrap();
        b.store.sync_device_remove(&d.id).unwrap();
        ra.rotate(std::slice::from_ref(&c.id)).await.unwrap();
        rb.rotate(std::slice::from_ref(&d.id)).await.unwrap();
        // A few passes: each reads the other's removal and keys, and one rotates again.
        let mut found = (None, None);
        for _ in 0..3 {
            if let Some(key) = ra.sync().await.unwrap().recovery {
                found.0 = Some(key);
            }
            if let Some(key) = rb.sync().await.unwrap().recovery {
                found.1 = Some(key);
            }
        }
        let (ka, kb) = (ra.keyring(), rb.keyring());
        assert_eq!(ka.current_epoch(), kb.current_epoch());
        let removed = vec![c.id.clone(), d.id.clone()];
        assert!(!ka.reaches_removed(&removed), "the current key was kept from both");
        // Both hold the recovery key that opens the folder.
        let group = target.read("kaeri/group.bin").await.unwrap();
        let held = ka.recovery_keys().into_iter().find(|k| open_group_file(&group, k).is_ok());
        assert!(held.is_some());
        assert_eq!(found.0.as_ref().or(held.as_ref()), held.as_ref());
        assert_eq!(found.1.as_ref().or(held.as_ref()), held.as_ref());
        // What each writes now, neither removed device can open.
        a.watch("2", 100.0);
        b.watch("3", 100.0);
        ra.sync().await.unwrap();
        rb.sync().await.unwrap();
        ra.sync().await.unwrap();
        assert_eq!((a.resume("3"), b.resume("2")), (Some(100.0), Some(100.0)));
        for gone in [&rc, &rd] {
            for writer in [&a.id, &b.id] {
                let bytes = newest_segment(&target, writer).await;
                assert!(matches!(
                    seal::open(&gone.keyring(), &bytes),
                    Err(SyncError::KeyMismatch)
                ));
            }
        }
    }

    #[tokio::test]
    async fn a_device_away_through_two_rotations_catches_up() {
        let target: Arc<dyn FolderTarget> = Arc::new(MemoryTarget::default());
        let (a, ra, b, rb, old) = pair(&target).await;
        let c = dev("00000000000000c3", 1_000);
        let rc = join_with(&target, &c, &old).await;
        let d = dev("00000000000000d4", 1_000);
        let rd = join_with(&target, &d, &old).await;
        announce(&[&a, &b, &c, &d]);
        for _ in 0..2 {
            for r in [&ra, &rb, &rc, &rd] {
                r.sync().await.unwrap();
            }
        }
        // B is away while A removes C, then D, writing between.
        a.store.sync_device_remove(&c.id).unwrap();
        ra.sync_rotating(true).await.unwrap();
        a.watch("1", 100.0);
        ra.sync().await.unwrap();
        a.store.sync_device_remove(&d.id).unwrap();
        ra.sync_rotating(true).await.unwrap();
        a.watch("2", 200.0);
        ra.sync().await.unwrap();
        let back = rb.sync().await.unwrap();
        assert_eq!((b.resume("1"), b.resume("2")), (Some(100.0), Some(200.0)));
        assert_eq!(rb.keyring().current_epoch(), ra.keyring().current_epoch());
        assert!(back.recovery.is_some());
    }

    #[tokio::test]
    async fn an_unreachable_folder_leaves_local_work_intact() {
        let mem = Arc::new(MemoryTarget::default());
        let target: Arc<dyn FolderTarget> = mem.clone();
        let (a, ra, b, rb, _) = pair(&target).await;
        mem.set_down(true);
        a.watch("1", 200.0);
        assert!(matches!(ra.sync().await, Err(SyncError::Folder(FolderError::Unreachable(_)))));
        assert_eq!(a.resume("1"), Some(200.0));
        mem.set_down(false);
        ra.sync().await.unwrap();
        rb.sync().await.unwrap();
        assert_eq!(b.resume("1"), Some(200.0));
    }

    #[tokio::test]
    async fn the_backup_round_trips_and_a_wrong_key_applies_nothing() {
        let target: Arc<dyn FolderTarget> = Arc::new(MemoryTarget::default());
        let (a, _ra, b, _rb, recovery) = pair(&target).await;
        a.watch("1", 700.0);
        let file = export_backup(&a.store, &recovery).unwrap();
        assert!(matches!(
            import_backup(&b.store, &RecoveryKey::generate(), &file),
            Err(SyncError::WrongRecoveryKey)
        ));
        assert_eq!(b.resume("1"), None);
        import_backup(&b.store, &recovery, &file).unwrap();
        assert_eq!(b.resume("1"), Some(700.0));
    }

    #[tokio::test]
    async fn a_new_recovery_key_replaces_the_old() {
        let target: Arc<dyn FolderTarget> = Arc::new(MemoryTarget::default());
        let (_a, ra, ..) = pair(&target).await;
        let fresh = ra.new_recovery_key().await.unwrap();
        let c = dev("00000000000000c3", 1_000);
        let old = RecoveryKey::generate();
        assert!(
            FolderRoad::join(
                target.clone(),
                "kaeri",
                c.store.clone(),
                &c.id,
                c.identity(),
                &old
            )
            .await
            .is_err()
        );
        FolderRoad::join(target.clone(), "kaeri", c.store.clone(), &c.id, c.identity(), &fresh)
            .await
            .unwrap();
    }
}
