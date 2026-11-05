//! What one paired device hands another over the paired link besides the log: sign-ins it asked
//! for (a tracker's, a Nextcloud or WebDAV login, a Dropbox sign-in). Each answer is sealed for the
//! asking device's own key from the group's device list, so only that device can open it, and it
//! travels inside the link's mutual TLS as well.
//!
//! The crate moves bytes and nothing else: what is asked and what an answer holds is the engine's,
//! through [`Handoff`].

use crate::{
    error::{Result, SyncError},
    keys::{DeviceKey, open_sealed_with, public_of, seal_for},
    lan::Paired,
};

/// The label a handoff is sealed under, apart from the keyrings' own.
const LABEL: &[u8] = b"kaeri-sync/1 handoff";

/// The engine's side of a handoff, asked during each session with a paired device.
#[async_trait::async_trait]
pub trait Handoff: Send + Sync {
    /// What this device asks `peer` for now: ids the engine understands, such as
    /// `tracker:anilist`. Empty for nothing. `just_paired` when the session began by pairing.
    fn asks(&self, peer: &Paired, just_paired: bool) -> Vec<String>;

    /// The answer to what `peer` asked, as plain bytes the crate seals for it, or `None` to send
    /// nothing. It may ask a server first (a plugin store making an invite), but the other side
    /// waits for it only so long: keep it well under [`crate::lan::ANSWER`].
    async fn answer(&self, peer: &Paired, asks: &[String]) -> Option<Vec<u8>>;
}

/// What a peer sent for what this device asked.
#[derive(Clone)]
pub struct Received {
    pub from: Paired,
    /// What this device asked for in the session.
    pub asked: Vec<String>,
    /// The answer, opened.
    pub plain: Vec<u8>,
}

impl std::fmt::Debug for Received {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never the answer: it holds sign-ins.
        f.debug_struct("Received")
            .field("from", &self.from.device)
            .field("asked", &self.asked)
            .finish_non_exhaustive()
    }
}

fn context(from: &str, to: &str) -> Vec<u8> {
    format!("{from}>{to}").into_bytes()
}

/// Seal `plain` from `from` for `to`, which only `to`'s device key opens.
pub fn seal(to: &Paired, from: &str, plain: &[u8]) -> Result<Vec<u8>> {
    let recipient = public_of(&to.pubkey).ok_or(SyncError::Damaged("a device's public key"))?;
    Ok(seal_for(&recipient, LABEL, &context(from, &to.device), plain))
}

/// Open what `from` sealed for this device, `me`.
pub fn open(key: &DeviceKey, me: &str, from: &str, sealed: &[u8]) -> Result<Vec<u8>> {
    open_sealed_with(key, LABEL, &context(from, me), sealed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paired(device: &str, key: &DeviceKey) -> Paired {
        Paired { device: device.into(), name: "Phone".into(), pubkey: key.public_hex() }
    }

    #[test]
    fn only_the_asking_device_opens_what_was_sealed_for_it() {
        let (tv, other) = (DeviceKey::from_identity(b"tv"), DeviceKey::from_identity(b"other"));
        let sealed =
            seal(&paired("00000000000000a1", &tv), "00000000000000b2", b"a sign-in").unwrap();
        assert_eq!(
            open(&tv, "00000000000000a1", "00000000000000b2", &sealed).unwrap(),
            b"a sign-in"
        );
        // Another device's key, another sender, or a keyring's label: nothing opens.
        assert!(open(&other, "00000000000000a1", "00000000000000b2", &sealed).is_err());
        assert!(open(&tv, "00000000000000a1", "00000000000000c3", &sealed).is_err());
        assert!(tv.unwrap("00000000000000a1", &sealed).is_err());
        assert!(
            !format!(
                "{:?}",
                Received { from: paired("x", &tv), asked: vec![], plain: b"secret".to_vec() }
            )
            .contains("secret")
        );
    }
}
