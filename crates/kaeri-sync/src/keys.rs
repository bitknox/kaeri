//! Group keys, the recovery key and the wrapping of one for a device.

use curve25519_dalek::{constants::X25519_BASEPOINT, montgomery::MontgomeryPoint};
use ring::{
    aead::{self, Aad, LessSafeKey, Nonce, UnboundKey},
    hkdf,
    rand::{SecureRandom, SystemRandom},
};
use serde::{Deserialize, Serialize};

use crate::error::{Result, SyncError};

pub(crate) fn random<const N: usize>() -> [u8; N] {
    let mut out = [0u8; N];
    SystemRandom::new().fill(&mut out).expect("the system's random source");
    out
}

struct Len(usize);
impl hkdf::KeyType for Len {
    fn len(&self) -> usize {
        self.0
    }
}

pub(crate) fn derive<const N: usize>(salt: &[u8], ikm: &[u8], info: &[&[u8]]) -> [u8; N] {
    let mut out = [0u8; N];
    hkdf::Salt::new(hkdf::HKDF_SHA256, salt)
        .extract(ikm)
        .expand(info, Len(N))
        .expect("a length HKDF allows")
        .fill(&mut out)
        .expect("a length HKDF allows");
    out
}

pub(crate) fn seal_with(key: &[u8; 32], aad: &[u8], plain: &[u8]) -> Vec<u8> {
    let nonce: [u8; 12] = random();
    let key =
        LessSafeKey::new(UnboundKey::new(&aead::CHACHA20_POLY1305, key).expect("32 bytes"));
    let mut buf = plain.to_vec();
    key.seal_in_place_append_tag(Nonce::assume_unique_for_key(nonce), Aad::from(aad), &mut buf)
        .expect("sealing in memory");
    let mut out = nonce.to_vec();
    out.extend_from_slice(&buf);
    out
}

pub(crate) fn open_with(key: &[u8; 32], aad: &[u8], sealed: &[u8]) -> Option<Vec<u8>> {
    if sealed.len() < 12 + 16 {
        return None;
    }
    let (nonce, rest) = sealed.split_at(12);
    let key =
        LessSafeKey::new(UnboundKey::new(&aead::CHACHA20_POLY1305, key).expect("32 bytes"));
    let mut buf = rest.to_vec();
    let plain = key
        .open_in_place(Nonce::try_assume_unique_for_key(nonce).ok()?, Aad::from(aad), &mut buf)
        .ok()?;
    Some(plain.to_vec())
}

/// Every group key this device holds, by epoch. A rotation adds one; older files stay readable.
///
/// A rotation also makes a recovery key and says which devices its key was kept from. Both ride
/// in the keyring, so every device it is wrapped for learns them, and a version that knows only
/// `keys` reads the same JSON and leaves the rest out.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Keyring {
    keys: Vec<(u32, String)>,
    /// The recovery keys rotations made, by the epoch each came with, the newest few: the one
    /// that opens `group.bin` is the group's.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    recovery: Vec<(u32, String)>,
    /// For each epoch a rotation made, the devices removed by then, which it was kept from.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    cut: Vec<(u32, Vec<String>)>,
}

/// How many recovery keys a keyring carries: enough for rotations that ran at once to find the
/// one that won.
const RECOVERY_KEPT: usize = 4;
/// A rotation's epoch is this far at most past the last, at random, so two rotations that run at
/// once almost never make the same epoch with different keys.
const EPOCH_SPREAD: u32 = 1 << 16;

impl std::fmt::Debug for Keyring {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Keyring(epochs {:?})", self.epochs())
    }
}

impl Keyring {
    pub fn generate() -> Self {
        Self::of(vec![(1, hex::encode(random::<32>()))])
    }

    fn of(keys: Vec<(u32, String)>) -> Self {
        Self { keys, recovery: Vec::new(), cut: Vec::new() }
    }

    /// A ring of one key, for sealing a backup under the recovery key.
    pub(crate) fn single(key: [u8; 32]) -> Self {
        Self::of(vec![(1, hex::encode(key))])
    }

    pub fn epochs(&self) -> Vec<u32> {
        self.keys.iter().map(|(e, _)| *e).collect()
    }

    pub fn current_epoch(&self) -> u32 {
        self.keys.iter().map(|(e, _)| *e).max().unwrap_or(0)
    }

    pub(crate) fn key(&self, epoch: u32) -> Option<[u8; 32]> {
        let (_, hex) = self.keys.iter().find(|(e, _)| *e == epoch)?;
        hex::decode(hex).ok()?.try_into().ok()
    }

    pub(crate) fn current(&self) -> [u8; 32] {
        self.key(self.current_epoch()).expect("a keyring holds its current key")
    }

    /// A new current key; the old ones stay for reading what was written under them.
    pub fn rotated(&self) -> Self {
        let mut ring = self.clone();
        ring.keys.push((self.current_epoch() + 1, hex::encode(random::<32>())));
        ring
    }

    /// A new current key kept from the devices in `removed`, with `recovery` as the recovery key
    /// that comes with it. Its epoch is a random step past the last.
    pub fn rotated_from(&self, removed: &[String], recovery: &RecoveryKey) -> Self {
        let step = 1 + u32::from_be_bytes(random()) % EPOCH_SPREAD;
        let epoch = self.current_epoch().saturating_add(step);
        let mut ring = self.clone();
        ring.keys.push((epoch, hex::encode(random::<32>())));
        ring.recovery.push((epoch, recovery.to_text()));
        ring.trim_recovery();
        let mut removed = removed.to_vec();
        removed.sort();
        removed.dedup();
        ring.cut.push((epoch, removed));
        ring
    }

    fn trim_recovery(&mut self) {
        self.recovery.sort();
        self.recovery.dedup();
        let extra = self.recovery.len().saturating_sub(RECOVERY_KEPT);
        self.recovery.drain(..extra);
    }

    /// The recovery keys rotations made, newest first.
    pub fn recovery_keys(&self) -> Vec<RecoveryKey> {
        self.recovery
            .iter()
            .rev()
            .filter_map(|(_, text)| RecoveryKey::parse(text).ok())
            .collect()
    }

    /// Whether a device the log lists as removed may hold the current key: one the rotation that
    /// made it did not know was removed, or, once rotations say whom they kept their key from, a
    /// current key made by a version that does not say.
    pub fn reaches_removed(&self, removed: &[String]) -> bool {
        if removed.is_empty() {
            return false;
        }
        let current = self.current_epoch();
        match self.cut.iter().find(|(e, _)| *e == current) {
            Some((_, cut)) => removed.iter().any(|d| !cut.contains(d)),
            None => !self.cut.is_empty(),
        }
    }

    /// This ring and `other`'s keys together.
    pub fn merged(&self, other: &Self) -> Self {
        let mut ring = self.clone();
        for (e, k) in &other.keys {
            if !ring.keys.iter().any(|(have, _)| have == e) {
                ring.keys.push((*e, k.clone()));
            }
        }
        ring.keys.sort();
        ring.recovery.extend(other.recovery.iter().cloned());
        ring.trim_recovery();
        for (e, cut) in &other.cut {
            if !ring.cut.iter().any(|(have, _)| have == e) {
                ring.cut.push((*e, cut.clone()));
            }
        }
        ring.cut.sort();
        ring
    }

    /// Whether this ring holds every key `other` does.
    pub fn holds(&self, other: &Self) -> bool {
        other.keys.iter().all(|(e, _)| self.keys.iter().any(|(have, _)| have == e))
    }

    /// A short name for this ring's contents, which changes whenever they do.
    pub fn fingerprint(&self) -> String {
        hex::encode(
            &ring::digest::digest(&ring::digest::SHA256, &self.to_bytes()).as_ref()[..8],
        )
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("a keyring serialises")
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        serde_json::from_slice(bytes).map_err(|_| SyncError::Damaged("keyring"))
    }
}

const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// 24 characters in groups of four, about 120 random bits: random rather than chosen, so it needs
/// no slow derivation and cannot be guessed.
#[derive(Clone, PartialEq, Eq)]
pub struct RecoveryKey([u8; 15]);

impl std::fmt::Debug for RecoveryKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RecoveryKey(..)")
    }
}

impl RecoveryKey {
    pub fn generate() -> Self {
        Self(random())
    }

    /// `ABCD-EFGH-...`, for the sheet.
    pub fn to_text(&self) -> String {
        let mut bits = 0u32;
        let mut n = 0;
        let mut chars = Vec::new();
        for byte in self.0 {
            bits = (bits << 8) | u32::from(byte);
            n += 8;
            while n >= 5 {
                n -= 5;
                chars.push(ALPHABET[((bits >> n) & 31) as usize]);
            }
        }
        chars
            .chunks(4)
            .map(|c| String::from_utf8_lossy(c).into_owned())
            .collect::<Vec<_>>()
            .join("-")
    }

    /// What a person typed: any case, with or without dashes and spaces, and the letters that
    /// look like digits read as the digits (O as 0, I and L as 1).
    pub fn parse(text: &str) -> Result<Self> {
        let mut bits = 0u32;
        let mut n = 0;
        let mut bytes = Vec::new();
        let mut count = 0;
        for c in text.chars().filter(|c| !matches!(c, '-' | ' ' | '\n' | '\t')) {
            let c = match c.to_ascii_uppercase() {
                'O' => '0',
                'I' | 'L' => '1',
                other => other,
            };
            let v = ALPHABET
                .iter()
                .position(|a| *a as char == c)
                .ok_or(SyncError::BadRecoveryKey)?;
            count += 1;
            bits = (bits << 5) | v as u32;
            n += 5;
            if n >= 8 {
                n -= 8;
                bytes.push(((bits >> n) & 0xff) as u8);
            }
        }
        if count != 24 {
            return Err(SyncError::BadRecoveryKey);
        }
        Ok(Self(bytes.try_into().map_err(|_| SyncError::BadRecoveryKey)?))
    }

    /// The key the group's keyring is wrapped under.
    pub(crate) fn wrap_key(&self) -> [u8; 32] {
        derive(b"kaeri-sync/1 recovery", &self.0, &[b"wrap"])
    }
}

/// A device's X25519 pair, derived from its sync identity so that nothing more is kept.
#[derive(Clone)]
pub struct DeviceKey {
    secret: [u8; 32],
}

impl std::fmt::Debug for DeviceKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DeviceKey(..)")
    }
}

impl DeviceKey {
    /// From the identity's private key bytes (the PKCS#8 document the host keeps).
    pub fn from_identity(identity_secret: &[u8]) -> Self {
        Self { secret: derive(b"kaeri-sync/1 device", identity_secret, &[b"x25519"]) }
    }

    pub fn public(&self) -> [u8; 32] {
        X25519_BASEPOINT.mul_clamped(self.secret).to_bytes()
    }

    pub fn public_hex(&self) -> String {
        hex::encode(self.public())
    }

    fn shared(&self, their_public: &[u8; 32]) -> [u8; 32] {
        MontgomeryPoint(*their_public).mul_clamped(self.secret).to_bytes()
    }

    /// Open a keyring wrapped for this device.
    pub fn unwrap(&self, device_id: &str, wrapped: &[u8]) -> Result<Keyring> {
        let plain = self.open_sealed(WRAP, device_id.as_bytes(), wrapped)?;
        Keyring::from_bytes(&plain)
    }

    /// Open what [`seal_for`] sealed for this device under `label` and `context`.
    fn open_sealed(&self, label: &[u8], context: &[u8], sealed: &[u8]) -> Result<Vec<u8>> {
        if sealed.len() < 32 {
            return Err(SyncError::Damaged("sealed for a device"));
        }
        let (eph, sealed) = sealed.split_at(32);
        let eph: [u8; 32] = eph.try_into().expect("split at 32");
        let key = wrap_key(label, &self.shared(&eph), &eph, &self.public());
        open_with(&key, context, sealed).ok_or(SyncError::KeyMismatch)
    }
}

/// Open what [`seal_for`] sealed for `key`'s device under `label` and `context`.
pub(crate) fn open_sealed_with(
    key: &DeviceKey,
    label: &[u8],
    context: &[u8],
    sealed: &[u8],
) -> Result<Vec<u8>> {
    key.open_sealed(label, context, sealed)
}

/// The label a keyring is wrapped under.
const WRAP: &[u8] = b"kaeri-sync/1 wrap";

fn wrap_key(
    label: &[u8],
    shared: &[u8; 32],
    eph_public: &[u8; 32],
    recipient_public: &[u8; 32],
) -> [u8; 32] {
    derive(label, shared, &[eph_public, recipient_public])
}

/// Seal `plain` so that only the device whose public key is `recipient` can open it: a key agreed
/// with a one-time key pair, under `label` (what kind of thing it is) and bound to `context`.
pub(crate) fn seal_for(
    recipient: &[u8; 32],
    label: &[u8],
    context: &[u8],
    plain: &[u8],
) -> Vec<u8> {
    let eph = DeviceKey { secret: random() };
    let eph_public = eph.public();
    let key = wrap_key(label, &eph.shared(recipient), &eph_public, recipient);
    let mut out = eph_public.to_vec();
    out.extend(seal_with(&key, context, plain));
    out
}

/// Wrap a keyring for the device whose public key is `recipient`.
pub fn wrap_for(recipient: &[u8; 32], device_id: &str, keyring: &Keyring) -> Vec<u8> {
    seal_for(recipient, WRAP, device_id.as_bytes(), &keyring.to_bytes())
}

/// A public key as the device list keeps it, hex, read back.
pub fn public_of(hex_key: &str) -> Option<[u8; 32]> {
    hex::decode(hex_key).ok()?.try_into().ok()
}

/// A new device's secret seed, from which its X25519 key is derived: kept in the host's secrets.
pub fn new_device_seed() -> [u8; 32] {
    random()
}

/// A new device id: random, not the machine's, so it says nothing about it.
pub fn new_device_id() -> String {
    hex::encode(random::<8>())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// X25519 against RFC 7748's test vectors, a device's public key, and a keyring wrapped by an
    /// earlier build: a new curve library must reproduce every one of them byte for byte.
    #[test]
    fn device_keys_and_wrapped_keyrings_stay_byte_identical() {
        let hex32 = |h: &str| -> [u8; 32] { hex::decode(h).unwrap().try_into().unwrap() };
        let alice = DeviceKey {
            secret: hex32("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a"),
        };
        let bob = DeviceKey {
            secret: hex32("5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb"),
        };
        assert_eq!(
            alice.public_hex(),
            "8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a"
        );
        assert_eq!(
            bob.public_hex(),
            "de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f"
        );
        let shared = hex32("4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742");
        assert_eq!(alice.shared(&bob.public()), shared);
        assert_eq!(bob.shared(&alice.public()), shared);

        let device = DeviceKey::from_identity(b"one");
        assert_eq!(
            device.public_hex(),
            "c42c9a0c8bdd68f921507a18021e093a71221894c12b74063c93eb0db7c4c62b"
        );
        let wrapped = hex::decode(concat!(
            "8c3c84493e02b6f1a6b9caec34e963e9a7b03245b0ecf48d1170f09444114e2d",
            "379897f3bf280f16f74bfafc36cb0844f703b1071ab2ac30b3914f12950e1cb9",
            "c908d889ebe2e618c04be672d37770a90fe3535791ee1d036db14f7bc0645435",
            "517d4b0b310d975238bc2bbaf3840f1078a7174dffb77ee89177f00684fc5382",
            "c062e0c14009b23c56cac3b9a7",
        ))
        .unwrap();
        assert_eq!(
            device.unwrap("aaaa0000aaaa0000", &wrapped).unwrap(),
            Keyring::single([7; 32])
        );
    }

    #[test]
    fn a_recovery_key_reads_back_however_it_is_typed() {
        let key = RecoveryKey::generate();
        let text = key.to_text();
        assert_eq!(text.len(), 24 + 5);
        assert_eq!(RecoveryKey::parse(&text).unwrap(), key);
        assert_eq!(RecoveryKey::parse(&text.to_lowercase().replace('-', " ")).unwrap(), key);
        assert!(RecoveryKey::parse("ABCD-EFGH").is_err());
        assert!(RecoveryKey::parse(&format!("{text}A")).is_err());
        assert!(
            RecoveryKey::parse("UUUU-UUUU-UUUU-UUUU-UUUU-UUUU").is_err(),
            "no U in the alphabet"
        );
        assert_ne!(RecoveryKey::generate(), key);
    }

    #[test]
    fn a_keyring_wraps_for_one_device_only() {
        let (a, b) = (DeviceKey::from_identity(b"one"), DeviceKey::from_identity(b"two"));
        let ring = Keyring::generate().rotated();
        let wrapped = wrap_for(&a.public(), "aaaa0000aaaa0000", &ring);
        assert_eq!(a.unwrap("aaaa0000aaaa0000", &wrapped).unwrap(), ring);
        assert!(matches!(b.unwrap("aaaa0000aaaa0000", &wrapped), Err(SyncError::KeyMismatch)));
        assert!(matches!(a.unwrap("bbbb0000bbbb0000", &wrapped), Err(SyncError::KeyMismatch)));
        assert_eq!(ring.epochs(), vec![1, 2]);
    }

    /// What a version that knows only the keys reads and writes.
    #[derive(Serialize, Deserialize)]
    struct OldRing {
        keys: Vec<(u32, String)>,
    }

    #[test]
    fn a_rotated_keyring_reads_in_a_version_that_knows_only_the_keys() {
        let recovery = RecoveryKey::generate();
        let ring = Keyring::generate().rotated_from(&["00000000000000c3".into()], &recovery);
        let old: OldRing =
            serde_json::from_slice(&ring.to_bytes()).expect("the old shape reads it");
        assert_eq!(old.keys, ring.keys);
        // What it writes back carries the keys, and merges back without losing the rest.
        let back = Keyring::from_bytes(&serde_json::to_vec(&old).unwrap()).unwrap();
        assert_eq!(back.current_epoch(), ring.current_epoch());
        assert_eq!(ring.merged(&back), ring);
        assert_eq!(back.merged(&ring).recovery_keys(), vec![recovery]);
    }

    #[test]
    fn a_rotation_says_whom_its_key_was_kept_from() {
        let (c, d) = ("00000000000000c3".to_owned(), "00000000000000d4".to_owned());
        let start = Keyring::generate();
        assert!(!start.reaches_removed(std::slice::from_ref(&c)), "no rotation has said");
        let one = start.rotated_from(std::slice::from_ref(&c), &RecoveryKey::generate());
        assert!(one.current_epoch() > 1);
        assert!(!one.reaches_removed(std::slice::from_ref(&c)));
        assert!(one.reaches_removed(&[c.clone(), d.clone()]));
        // Another made at once from the same start, kept from D only: together, the current key
        // was not kept from both, until a rotation after them is.
        let other = start.rotated_from(std::slice::from_ref(&d), &RecoveryKey::generate());
        let both = one.merged(&other);
        assert_eq!(both.epochs().len(), 3);
        assert!(both.reaches_removed(&[c.clone(), d.clone()]));
        let after = both.rotated_from(&[c.clone(), d.clone()], &RecoveryKey::generate());
        assert!(!after.reaches_removed(&[c, d]));
        assert_eq!(after.recovery_keys().len(), 3);
    }

    #[test]
    fn nothing_secret_prints() {
        let ring = Keyring::generate();
        assert!(!format!("{ring:?}").contains(&ring.keys[0].1));
        assert_eq!(format!("{:?}", RecoveryKey::generate()), "RecoveryKey(..)");
    }
}
