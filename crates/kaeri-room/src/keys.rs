//! Every key a room uses, from the invite's 128-bit secret:
//!
//! ```text
//! room_id  = HKDF-SHA256(S, salt "kaeri-room-v1", info "id")[..16]   local only, never sent
//! K_msg    = HKDF(S, salt "kaeri-room-v1", info "msg")              seals every body
//! K_tag    = HKDF(S, salt "kaeri-room-v1", info "tag")
//! tag(d)   = hex(HMAC-SHA256(K_tag, "tag" ‖ d)[..16])                d = days since epoch, UTC
//! ```
//!
//! Bodies are sealed with the NIP-44 v2 construction, `K_msg` standing in for the conversation
//! key. Each device signs with a fresh session key per join, and proves its member id with an
//! identity key kept per room. Pairwise payloads (Move, Rekey) are NIP-44 v2 between two session
//! keys. See `docs/room-protocol.md`, "Room identity and keys".

use base64::{Engine as _, engine::general_purpose::STANDARD};
use hmac::{Hmac, KeyInit, Mac};
use nostr::{
    key::{Keys, PublicKey, SecretKey},
    nips::nip44::v2::{self, ConversationKey},
};
use secp256k1::{Secp256k1, VerifyOnly, XOnlyPublicKey, schnorr::Signature};
use sha2::{Digest, Sha256};

use crate::wire::Mid;

type HmacSha256 = Hmac<Sha256>;

const SALT: &[u8] = b"kaeri-room-v1";
/// What an identity key signs, after the session key and the room id.
const ID_LABEL: &[u8] = b"kaeri-room-id";
/// One UTC day in ms.
pub const DAY_MS: i64 = 86_400_000;
/// Around midnight UTC (23:58 to 00:02) devices listen on both days' tags.
pub const MIDNIGHT_MARGIN_MS: i64 = 120_000;

fn hmac(key: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC takes any key length");
    for part in parts {
        mac.update(part);
    }
    mac.finalize().into_bytes().into()
}

/// HKDF-SHA256 for one 32-byte block (RFC 5869).
fn hkdf(secret: &[u8], info: &[u8]) -> [u8; 32] {
    let prk = hmac(SALT, &[secret]);
    hmac(&prk, &[info, &[1]])
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}

/// Fresh random bytes from the OS.
pub fn random<const N: usize>() -> [u8; N] {
    let mut out = [0u8; N];
    getrandom::fill(&mut out).expect("the OS gives random bytes");
    out
}

/// A room's keys, from its secret.
#[derive(Clone)]
pub struct RoomKeys {
    secret: [u8; 16],
    room_id: [u8; 16],
    msg: [u8; 32],
    tag: [u8; 32],
}

impl std::fmt::Debug for RoomKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never the secret.
        f.debug_struct("RoomKeys").field("room_id", &self.room_id_hex()).finish_non_exhaustive()
    }
}

impl RoomKeys {
    pub fn new(secret: [u8; 16]) -> Self {
        let id = hkdf(&secret, b"id");
        let mut room_id = [0u8; 16];
        room_id.copy_from_slice(&id[..16]);
        Self { secret, room_id, msg: hkdf(&secret, b"msg"), tag: hkdf(&secret, b"tag") }
    }

    pub fn secret(&self) -> [u8; 16] {
        self.secret
    }

    pub fn room_id(&self) -> [u8; 16] {
        self.room_id
    }

    /// The room id as recent rooms keep it.
    pub fn room_id_hex(&self) -> String {
        hex(&self.room_id)
    }

    /// The room's tag on UTC day `day` (days since the epoch).
    pub fn tag(&self, day: i64) -> String {
        let mac = hmac(&self.tag, &[b"tag", day.to_string().as_bytes()]);
        hex(&mac[..16])
    }

    /// The tags to listen on at `now_ms`: today's, and the other day's within two minutes of
    /// midnight.
    pub fn tags_at(&self, now_ms: i64) -> Vec<String> {
        days_at(now_ms).into_iter().map(|d| self.tag(d)).collect()
    }

    /// The tag to send on at `now_ms`.
    pub fn send_tag(&self, now_ms: i64) -> String {
        self.tag(now_ms.div_euclid(DAY_MS))
    }

    fn conversation(&self) -> ConversationKey {
        ConversationKey::new(self.msg)
    }

    /// Seal `plain` under `K_msg`: base64 of the NIP-44 v2 payload.
    pub fn seal(&self, plain: &[u8]) -> String {
        let payload = v2::encrypt_to_bytes_with_nonce(&self.conversation(), plain, random())
            .expect("bodies are far below NIP-44's limit");
        STANDARD.encode(payload)
    }

    /// Open a sealed body: the MAC is checked before anything is parsed.
    pub fn open(&self, sealed: &str) -> Option<Vec<u8>> {
        let payload = STANDARD.decode(sealed).ok()?;
        v2::decrypt_to_bytes(&self.conversation(), &payload).ok()
    }
}

/// The UTC days to listen on at `now_ms`.
pub fn days_at(now_ms: i64) -> Vec<i64> {
    let day = now_ms.div_euclid(DAY_MS);
    let into = now_ms.rem_euclid(DAY_MS);
    let mut days = vec![day];
    if into < MIDNIGHT_MARGIN_MS {
        days.insert(0, day - 1);
    } else if DAY_MS - into <= MIDNIGHT_MARGIN_MS {
        days.push(day + 1);
    }
    days
}

/// A device's key for one join of one room: signs its events, never stored.
#[derive(Clone)]
pub struct Session {
    keys: Keys,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session").field("pk", &self.pubkey()).finish_non_exhaustive()
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    pub fn new() -> Self {
        Self { keys: Keys::generate() }
    }

    pub fn keys(&self) -> &Keys {
        &self.keys
    }

    /// The session public key, hex (x-only, as Nostr writes it).
    pub fn pubkey(&self) -> String {
        self.keys.public_key().to_hex()
    }

    fn pair(&self, peer: &str) -> Option<ConversationKey> {
        let peer = PublicKey::from_hex(peer).ok()?;
        ConversationKey::derive(self.keys.secret_key(), &peer).ok()
    }

    /// Seal `plain` to the member whose session key is `peer` (NIP-44 v2, pairwise).
    pub fn seal_to(&self, peer: &str, plain: &[u8]) -> Option<String> {
        let payload =
            v2::encrypt_to_bytes_with_nonce(&self.pair(peer)?, plain, random()).ok()?;
        Some(STANDARD.encode(payload))
    }

    /// Open what `peer` sealed to this session.
    pub fn open_from(&self, peer: &str, sealed: &str) -> Option<Vec<u8>> {
        let payload = STANDARD.decode(sealed).ok()?;
        v2::decrypt_to_bytes(&self.pair(peer)?, &payload).ok()
    }

    /// The first 8 bytes of this session key, hex: how a pairwise payload names its recipient.
    pub fn pk8(&self) -> String {
        pk8(&self.pubkey())
    }
}

/// The first 8 bytes of a session key, hex.
pub fn pk8(pubkey: &str) -> String {
    pubkey.chars().take(16).collect()
}

/// A device's key in one room, kept with the recent room: it gives the member id and proves each
/// session speaks for it. Relays never see it.
#[derive(Clone)]
pub struct Identity {
    keys: Keys,
}

impl std::fmt::Debug for Identity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Identity").field("mid", &self.mid()).finish_non_exhaustive()
    }
}

impl Default for Identity {
    fn default() -> Self {
        Self::new()
    }
}

impl Identity {
    pub fn new() -> Self {
        Self { keys: Keys::generate() }
    }

    /// The identity key a recent room kept (hex), or `None` when it doesn't read.
    pub fn from_secret_hex(secret: &str) -> Option<Self> {
        let key = SecretKey::from_hex(secret).ok()?;
        Some(Self { keys: Keys::new(key) })
    }

    /// The secret, hex, for the recent room. A membership token: never logged.
    pub fn secret_hex(&self) -> String {
        self.keys.secret_key().to_secret_hex()
    }

    pub fn pubkey(&self) -> String {
        self.keys.public_key().to_hex()
    }

    pub fn mid(&self) -> Mid {
        mid_of(&self.pubkey()).unwrap_or_default()
    }

    /// `idsig`: a Schnorr signature over `session ‖ room_id ‖ "kaeri-room-id"`.
    pub fn prove(&self, session: &str, room_id: &[u8; 16]) -> String {
        let digest = proof_digest(session, room_id);
        hex(&self.keys.sign_schnorr(digest).to_bytes())
    }
}

fn proof_digest(session: &str, room_id: &[u8; 16]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(unhex(session).unwrap_or_default());
    hash.update(room_id);
    hash.update(ID_LABEL);
    hash.finalize().into()
}

/// A member id: the first 8 bytes of SHA-256 of the identity public key, hex.
pub fn mid_of(idpk: &str) -> Option<Mid> {
    let bytes = unhex(idpk)?;
    if bytes.len() != 32 {
        return None;
    }
    Some(hex(&Sha256::digest(&bytes)[..8]))
}

/// The member id `idpk` proves for `session` in this room, when `idsig` holds.
pub fn verify_proof(idpk: &str, idsig: &str, session: &str, room_id: &[u8; 16]) -> Option<Mid> {
    let key = XOnlyPublicKey::from_slice(&unhex(idpk)?).ok()?;
    let sig = Signature::from_slice(&unhex(idsig)?).ok()?;
    let digest = proof_digest(session, room_id);
    static SECP: std::sync::LazyLock<Secp256k1<VerifyOnly>> =
        std::sync::LazyLock::new(Secp256k1::verification_only);
    SECP.verify_schnorr(&sig, &digest, &key).ok()?;
    mid_of(idpk)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: [u8; 16] = [7; 16];

    #[test]
    fn the_ladder_and_the_tag_are_fixed() {
        // Golden values, checked against a Python HMAC: a change breaks rooms between versions.
        let keys = RoomKeys::new(SECRET);
        assert_eq!(keys.room_id_hex(), GOLDEN_ROOM_ID);
        assert_eq!(keys.tag(20_000), GOLDEN_TAG);
        assert_ne!(keys.tag(20_000), keys.tag(20_001));
        assert_ne!(RoomKeys::new([8; 16]).tag(20_000), keys.tag(20_000));
        assert_eq!(keys.tag(20_000).len(), 32);
    }

    const GOLDEN_ROOM_ID: &str = "19bd1d0b7bfcf778c686f2a0ac9130f1";
    const GOLDEN_TAG: &str = "a742dd452e711dee449c5fa6ac9936c5";

    #[test]
    fn hkdf_matches_rfc_5869() {
        // RFC 5869 test case 1, with our salt swapped for its: checks extract-then-expand.
        let prk = hmac(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12], &[&[0x0b; 22]]);
        assert_eq!(
            hex(&prk),
            "077709362c2e32df0ddc3f0dc47bba6390b6c73bb50f9c3122ec844ad7c2b3e5"
        );
        let okm =
            hmac(&prk, &[&[0xf0, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8, 0xf9], &[1]]);
        assert_eq!(hex(&okm[..16]), "3cb25f25faacd57a90434f64d0362f2a");
    }

    #[test]
    fn near_midnight_both_days_are_heard() {
        let day = 20_000 * DAY_MS;
        assert_eq!(days_at(day + 12 * 3_600_000), vec![20_000]);
        assert_eq!(days_at(day + 60_000), vec![19_999, 20_000]);
        assert_eq!(days_at(day + DAY_MS - 60_000), vec![20_000, 20_001]);
        assert_eq!(
            RoomKeys::new(SECRET).send_tag(day + DAY_MS - 60_000),
            RoomKeys::new(SECRET).tag(20_000)
        );
    }

    #[test]
    fn a_sealed_body_opens_only_under_its_room() {
        let keys = RoomKeys::new(SECRET);
        let sealed = keys.seal(b"{\"v\":1}");
        assert_eq!(keys.open(&sealed).as_deref(), Some(&b"{\"v\":1}"[..]));
        assert_eq!(RoomKeys::new([8; 16]).open(&sealed), None);
        // A flipped byte fails the MAC.
        let mut raw = STANDARD.decode(&sealed).unwrap();
        raw[40] ^= 1;
        assert_eq!(keys.open(&STANDARD.encode(raw)), None);
        // Two seals of the same body differ (fresh nonce).
        assert_ne!(keys.seal(b"x"), keys.seal(b"x"));
    }

    #[test]
    fn nip44_v2_matches_its_published_vector() {
        // From the NIP-44 v2 test vectors (encrypt_decrypt, first entry).
        let conversation = ConversationKey::from_slice(
            &unhex("c41c775356fd92eadc63ff5a0dc1da211b268cbea22316767095b2871ea1412d").unwrap(),
        )
        .unwrap();
        let nonce: [u8; 32] =
            unhex("0000000000000000000000000000000000000000000000000000000000000001")
                .unwrap()
                .try_into()
                .unwrap();
        let payload = v2::encrypt_to_bytes_with_nonce(&conversation, b"a", nonce).unwrap();
        assert_eq!(
            STANDARD.encode(payload),
            "AgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABee0G5VSK0/9YypIObAtDKfYEAjD35uVkHyB0F4DwrcNaCXlCWZKaArsGrY6M9wnuTMxWfp1RTN9Xga8no+kF5Vsb"
        );
    }

    #[test]
    fn pairwise_payloads_open_only_for_the_two() {
        let (a, b, c) = (Session::new(), Session::new(), Session::new());
        let sealed = a.seal_to(&b.pubkey(), b"secret").unwrap();
        assert_eq!(b.open_from(&a.pubkey(), &sealed).as_deref(), Some(&b"secret"[..]));
        assert_eq!(c.open_from(&a.pubkey(), &sealed), None);
        assert_eq!(a.pk8().len(), 16);
    }

    #[test]
    fn an_identity_proves_its_mid_for_one_session_in_one_room() {
        let keys = RoomKeys::new(SECRET);
        let id = Identity::new();
        let session = Session::new();
        let sig = id.prove(&session.pubkey(), &keys.room_id());
        assert_eq!(
            verify_proof(&id.pubkey(), &sig, &session.pubkey(), &keys.room_id()),
            Some(id.mid())
        );
        // Not for another session, another room, or another identity.
        assert_eq!(
            verify_proof(&id.pubkey(), &sig, &Session::new().pubkey(), &keys.room_id()),
            None
        );
        assert_eq!(verify_proof(&id.pubkey(), &sig, &session.pubkey(), &[0; 16]), None);
        assert_eq!(
            verify_proof(&Identity::new().pubkey(), &sig, &session.pubkey(), &keys.room_id()),
            None
        );
        // It survives the recent room.
        let back = Identity::from_secret_hex(&id.secret_hex()).unwrap();
        assert_eq!(back.mid(), id.mid());
        assert_eq!(id.mid().len(), 16);
    }
}
