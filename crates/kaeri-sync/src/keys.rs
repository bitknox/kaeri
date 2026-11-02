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
    let key = LessSafeKey::new(UnboundKey::new(&aead::CHACHA20_POLY1305, key).expect("32 bytes"));
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
    let key = LessSafeKey::new(UnboundKey::new(&aead::CHACHA20_POLY1305, key).expect("32 bytes"));
    let mut buf = rest.to_vec();
    let plain = key
        .open_in_place(Nonce::try_assume_unique_for_key(nonce).ok()?, Aad::from(aad), &mut buf)
        .ok()?;
    Some(plain.to_vec())
}

/// Every group key this device holds, by epoch. A rotation adds one; older files stay readable.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Keyring {
    keys: Vec<(u32, String)>,
}

impl std::fmt::Debug for Keyring {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Keyring(epochs {:?})", self.epochs())
    }
}

impl Keyring {
    pub fn generate() -> Self {
        Self { keys: vec![(1, hex::encode(random::<32>()))] }
    }

    /// A ring of one key, for sealing a backup under the recovery key.
    pub(crate) fn single(key: [u8; 32]) -> Self {
        Self { keys: vec![(1, hex::encode(key))] }
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
        let mut keys = self.keys.clone();
        keys.push((self.current_epoch() + 1, hex::encode(random::<32>())));
        Self { keys }
    }

    /// This ring and `other`'s keys together.
    pub fn merged(&self, other: &Self) -> Self {
        let mut keys = self.keys.clone();
        for (e, k) in &other.keys {
            if !keys.iter().any(|(have, _)| have == e) {
                keys.push((*e, k.clone()));
            }
        }
        keys.sort();
        Self { keys }
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
            let v = ALPHABET.iter().position(|a| *a as char == c).ok_or(SyncError::BadRecoveryKey)?;
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
        if wrapped.len() < 32 {
            return Err(SyncError::Damaged("wrapped keys"));
        }
        let (eph, sealed) = wrapped.split_at(32);
        let eph: [u8; 32] = eph.try_into().expect("split at 32");
        let key = wrap_key(&self.shared(&eph), &eph, &self.public());
        let plain = open_with(&key, device_id.as_bytes(), sealed).ok_or(SyncError::KeyMismatch)?;
        Keyring::from_bytes(&plain)
    }
}

fn wrap_key(shared: &[u8; 32], eph_public: &[u8; 32], recipient_public: &[u8; 32]) -> [u8; 32] {
    derive(b"kaeri-sync/1 wrap", shared, &[eph_public, recipient_public])
}

/// Wrap a keyring for the device whose public key is `recipient`.
pub fn wrap_for(recipient: &[u8; 32], device_id: &str, keyring: &Keyring) -> Vec<u8> {
    let eph = DeviceKey { secret: random() };
    let eph_public = eph.public();
    let key = wrap_key(&eph.shared(recipient), &eph_public, recipient);
    let mut out = eph_public.to_vec();
    out.extend(seal_with(&key, device_id.as_bytes(), &keyring.to_bytes()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_recovery_key_reads_back_however_it_is_typed() {
        let key = RecoveryKey::generate();
        let text = key.to_text();
        assert_eq!(text.len(), 24 + 5);
        assert_eq!(RecoveryKey::parse(&text).unwrap(), key);
        assert_eq!(RecoveryKey::parse(&text.to_lowercase().replace('-', " ")).unwrap(), key);
        assert!(RecoveryKey::parse("ABCD-EFGH").is_err());
        assert!(RecoveryKey::parse(&format!("{text}A")).is_err());
        assert!(RecoveryKey::parse("UUUU-UUUU-UUUU-UUUU-UUUU-UUUU").is_err(), "no U in the alphabet");
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

    #[test]
    fn nothing_secret_prints() {
        let ring = Keyring::generate();
        assert!(!format!("{ring:?}").contains(&ring.keys[0].1));
        assert_eq!(format!("{:?}", RecoveryKey::generate()), "RecoveryKey(..)");
    }
}
