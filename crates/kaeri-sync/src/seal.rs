//! The sealed file format every file in the folder shares.
//!
//! `KSY1`, a kind byte, the key epoch, the writing device, a number, a nonce and the sealed body.
//! The header is the associated data, so a file cannot be swapped for another's or replayed under
//! another name; its key comes from HKDF over the group key and the same header fields.

use crate::{
    error::{Result, SyncError},
    keys::{Keyring, derive, open_with, seal_with},
};

pub const MAGIC: &[u8; 4] = b"KSY1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Kind {
    Segment = 1,
    Acks = 2,
    Backup = 3,
}

fn header(kind: Kind, epoch: u32, device: &[u8; 8], number: u64) -> Vec<u8> {
    let mut h = MAGIC.to_vec();
    h.push(kind as u8);
    h.extend_from_slice(&epoch.to_be_bytes());
    h.extend_from_slice(device);
    h.extend_from_slice(&number.to_be_bytes());
    h
}

pub(crate) fn device_bytes(id: &str) -> Result<[u8; 8]> {
    hex::decode(id)
        .ok()
        .and_then(|b| b.try_into().ok())
        .ok_or(SyncError::Damaged("device id"))
}

fn file_key(group_key: &[u8; 32], header: &[u8]) -> [u8; 32] {
    derive(b"kaeri-sync/1 file", group_key, &[header])
}

/// Seal `plain` as file `number` of `device`, under the keyring's current key.
pub fn seal(ring: &Keyring, kind: Kind, device: &str, number: u64, plain: &[u8]) -> Result<Vec<u8>> {
    let epoch = ring.current_epoch();
    let h = header(kind, epoch, &device_bytes(device)?, number);
    let mut out = h.clone();
    out.extend(seal_with(&file_key(&ring.current(), &h), &h, plain));
    Ok(out)
}

/// What a sealed file says it is, before opening it.
pub struct Opened {
    pub kind: Kind,
    pub epoch: u32,
    pub device: String,
    pub number: u64,
    pub plain: Vec<u8>,
}

/// Open a sealed file. `KeyMismatch` when its epoch is not in the ring or the key does not open
/// it; `Damaged` when it is not a sealed file at all.
pub fn open(ring: &Keyring, bytes: &[u8]) -> Result<Opened> {
    const HEADER: usize = 4 + 1 + 4 + 8 + 8;
    if bytes.len() < HEADER || &bytes[..4] != MAGIC {
        return Err(SyncError::Damaged("not a sealed file"));
    }
    let (h, sealed) = bytes.split_at(HEADER);
    let kind = match h[4] {
        1 => Kind::Segment,
        2 => Kind::Acks,
        3 => Kind::Backup,
        _ => return Err(SyncError::Damaged("unknown kind")),
    };
    let epoch = u32::from_be_bytes(h[5..9].try_into().expect("4 bytes"));
    let device = hex::encode(&h[9..17]);
    let number = u64::from_be_bytes(h[17..25].try_into().expect("8 bytes"));
    let key = ring.key(epoch).ok_or(SyncError::KeyMismatch)?;
    let plain = open_with(&file_key(&key, h), h, sealed).ok_or(SyncError::KeyMismatch)?;
    Ok(Opened { kind, epoch, device, number, plain })
}

/// The epoch a sealed file was written under, without opening it.
pub fn epoch_of(bytes: &[u8]) -> Option<u32> {
    (bytes.len() >= 9 && &bytes[..4] == MAGIC)
        .then(|| u32::from_be_bytes(bytes[5..9].try_into().expect("4 bytes")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_opens_only_as_what_it_was_sealed_as() {
        let ring = Keyring::generate();
        let sealed = seal(&ring, Kind::Segment, "aaaa0000aaaa0000", 7, b"ops").unwrap();
        let opened = open(&ring, &sealed).unwrap();
        assert_eq!((opened.number, opened.device.as_str(), opened.plain.as_slice()), (7, "aaaa0000aaaa0000", &b"ops"[..]));
        // The name is in the header: change the number or the device and it no longer opens.
        let mut other = sealed.clone();
        other[24] ^= 1;
        assert!(matches!(open(&ring, &other), Err(SyncError::KeyMismatch)));
        let mut other = sealed.clone();
        other[10] ^= 1;
        assert!(matches!(open(&ring, &other), Err(SyncError::KeyMismatch)));
        // A flipped bit of the body, and another group's key.
        let mut flipped = sealed.clone();
        *flipped.last_mut().unwrap() ^= 1;
        assert!(open(&ring, &flipped).is_err());
        assert!(matches!(open(&Keyring::generate(), &sealed), Err(SyncError::KeyMismatch)));
        assert!(matches!(open(&ring, b"hello"), Err(SyncError::Damaged(_))));
    }

    #[test]
    fn a_rotated_ring_reads_old_files_and_an_old_ring_cannot_read_new_ones() {
        let old = Keyring::generate();
        let new = old.rotated();
        let before = seal(&old, Kind::Segment, "aaaa0000aaaa0000", 1, b"a").unwrap();
        let after = seal(&new, Kind::Segment, "aaaa0000aaaa0000", 2, b"b").unwrap();
        assert_eq!(open(&new, &before).unwrap().plain, b"a");
        assert!(matches!(open(&old, &after), Err(SyncError::KeyMismatch)));
        assert_eq!(epoch_of(&after), Some(2));
    }

    #[test]
    fn a_file_shows_a_stranger_no_titles() {
        let ring = Keyring::generate();
        let sealed = seal(&ring, Kind::Segment, "aaaa0000aaaa0000", 1, b"Made Up Show episode 4").unwrap();
        assert!(!sealed.windows(7).any(|w| w == b"Made Up"));
    }
}
