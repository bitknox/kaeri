//! The folder road: one directory per device and one writer per directory, so Syncthing and iCloud
//! never make conflict copies and WebDAV never needs locks.
//!
//! ```text
//! <prefix>/group.bin                 format, group id, the keyring wrapped by the recovery key
//! <prefix>/devices/<id>/keys.bin     the keyring, wrapped for this device
//! <prefix>/devices/<id>/000001.seg   this device's ops, sealed
//! <prefix>/devices/<id>/acks.bin     the version vector this device has seen, sealed
//! <prefix>/paused/<id>               present while the group holds back from that device
//! ```
//!
//! A device whose Sync switch is off (the `device_sync` register) is held back from: the others
//! skip its segments, and write its name under `paused/`, where it looks at the start of every
//! pass and, finding itself, neither reads nor writes. The register alone could not say so, since
//! a held-back device would never read the register that lets it back in. Not a lock, which
//! removing a device is (it rotates the key): a device that ignored the marker could still read.

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
    }
}

pub struct FolderRoad {
    target: Arc<dyn FolderTarget>,
    prefix: String,
    store: Store,
    device: String,
    key: DeviceKey,
    ring: std::sync::Mutex<Keyring>,
}

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

fn open_group_file(bytes: &[u8], recovery: &RecoveryKey) -> Result<([u8; 16], Keyring)> {
    if bytes.len() < 20 || &bytes[..4] != GROUP_MAGIC {
        return Err(SyncError::Damaged("group.bin"));
    }
    let group: [u8; 16] = bytes[4..20].try_into().expect("16 bytes");
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
        road.write_own_keys().await?;
        road.write_missing_keys(&[]).await?;
        Ok(road)
    }

    /// Open the group a folder holds as one of its devices, with the keyring another device
    /// wrapped for this one: no recovery key needed. [`SyncError::NoGroup`] when the folder holds
    /// none, and [`SyncError::GroupExists`] when it holds one this device has no key for.
    pub async fn open_as_member(
        target: Arc<dyn FolderTarget>,
        prefix: &str,
        store: Store,
        device: &str,
        key: DeviceKey,
    ) -> Result<Self> {
        if !Self::group_exists(&*target, prefix).await? {
            return Err(SyncError::NoGroup);
        }
        let path = join(prefix, &format!("devices/{device}/keys.bin"));
        let wrapped = match target.read(&path).await {
            Ok(bytes) => bytes,
            Err(FolderError::NotFound(_)) => return Err(SyncError::GroupExists),
            Err(e) => return Err(e.into()),
        };
        let ring = key.unwrap(device, &wrapped).map_err(|_| SyncError::GroupExists)?;
        Ok(Self::open(target, prefix, store, device, key, ring))
    }

    /// Join the group a folder holds, with its recovery key.
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
        let (_, ring) = open_group_file(&bytes, recovery)?;
        let road = Self::open(target, prefix, store, device, key, ring);
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
        }
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
        let mut report = FolderReport::default();
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
        // A rotation by another device reaches this one as a new keys.bin.
        self.refresh_keys().await?;

        let devices = self.target.list(&self.path("devices")).await?;
        let present: Vec<String> =
            devices.iter().filter(|e| e.is_dir).map(|e| e.name.clone()).collect();
        self.write_missing_keys(&present).await?;
        for entry in devices.iter().filter(|e| e.is_dir && e.name != self.device) {
            if removed.contains(&entry.name) || paused.contains(&entry.name) {
                continue;
            }
            report.devices_seen += 1;
            self.read_device(&entry.name, &mut report).await?;
        }
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

    /// Wrap the keyring for each device in the group's list that has no place in the folder yet:
    /// one paired nearby, which can then open the folder once it signs in to it.
    async fn write_missing_keys(&self, present: &[String]) -> Result<()> {
        for (device, _, pubkey, removed) in self.store.sync_peers()? {
            if removed || device == self.device || present.contains(&device) {
                continue;
            }
            let Some(pubkey) = pubkey.as_deref().and_then(crate::keys::public_of) else {
                continue;
            };
            let wrapped = wrap_for(&pubkey, &device, &self.keyring());
            self.replace(&self.path(&format!("devices/{device}/keys.bin")), &wrapped).await?;
        }
        Ok(())
    }

    async fn refresh_keys(&self) -> Result<()> {
        let path = self.path(&format!("devices/{}/keys.bin", self.device));
        match self.target.read(&path).await {
            Ok(bytes) => {
                if let Ok(theirs) = self.key.unwrap(&self.device, &bytes) {
                    self.set_ring(self.keyring().merged(&theirs));
                }
                Ok(())
            }
            Err(FolderError::NotFound(_)) => Ok(()),
            Err(e) => Err(e.into()),
        }
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
                    seal::open(&self.keyring(), &bytes)?
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

    /// Rotate the group key, as after removing a device: a new current key, wrapped for every
    /// device still in the group and under the recovery key. The removed device keeps what it had
    /// and reads nothing written from here on. Needs the recovery key to rewrite `group.bin`.
    pub async fn rotate_group_key(&self, recovery: &RecoveryKey) -> Result<Keyring> {
        let ring = self.keyring().rotated();
        self.set_ring(ring.clone());
        self.write_group(recovery, &ring).await?;
        for (device, _, pubkey, removed) in self.store.sync_peers()? {
            if removed || device == self.device {
                continue;
            }
            let Some(pubkey) = pubkey
                .and_then(|p| hex::decode(p).ok())
                .and_then(|p| <[u8; 32]>::try_from(p).ok())
            else {
                continue;
            };
            let wrapped = wrap_for(&pubkey, &device, &ring);
            self.replace(&self.path(&format!("devices/{device}/keys.bin")), &wrapped).await?;
        }
        self.write_own_keys().await?;
        Ok(ring)
    }

    /// Make a new recovery key: the old one stops working at once.
    pub async fn new_recovery_key(&self) -> Result<RecoveryKey> {
        let recovery = RecoveryKey::generate();
        self.write_group(&recovery, &self.keyring()).await?;
        Ok(recovery)
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
        // Its own keys.bin does not exist, so a first sync lists devices and fails on the first segment.
        let err = other.sync().await.unwrap_err();
        assert!(matches!(err, SyncError::KeyMismatch), "{err:?}");
        assert_eq!(s.resume("1"), None);
    }

    #[tokio::test]
    async fn rotation_reaches_the_others_and_a_removed_device_is_cut_off() {
        let target: Arc<dyn FolderTarget> = Arc::new(MemoryTarget::default());
        let (a, ra, b, rb, recovery) = pair(&target).await;
        let c = dev("00000000000000c3", 1_000);
        let rc = FolderRoad::join(
            target.clone(),
            "kaeri",
            c.store.clone(),
            &c.id,
            c.identity(),
            &recovery,
        )
        .await
        .unwrap();
        for d in [&b, &c] {
            a.store.sync_device_add(&d.id, "device", &d.identity().public_hex()).unwrap();
        }
        ra.sync().await.unwrap();
        rb.sync().await.unwrap();
        rc.sync().await.unwrap();
        a.store.sync_device_remove(&c.id).unwrap();
        ra.rotate_group_key(&recovery).await.unwrap();
        a.watch("1", 500.0);
        ra.sync().await.unwrap();
        rb.sync().await.unwrap();
        assert_eq!(b.resume("1"), Some(500.0));
        assert_eq!(rb.keyring().current_epoch(), 2);
        // The removed device still holds the old key and cannot read what follows.
        assert!(rc.sync().await.is_err());
        assert_eq!(c.resume("1"), None);
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
