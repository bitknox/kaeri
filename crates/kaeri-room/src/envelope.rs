//! A body on the wire: padded to a bucket, sealed under the room key, inside one signed ephemeral
//! Nostr event; and the checks every event passes before the room sees it.
//!
//! A receiver accepts an event only when:
//! 1. its signature is valid;
//! 2. its content opens under the room key (the MAC is checked before anything is parsed);
//! 3. `body.from` is the event's pubkey;
//! 4. the session key is bound to `body.mid` by an identity proof, or the body is the Hello that
//!    binds it (a newcomer's SyncReq is let through unbound);
//! 5. `ctr` is above the last one seen from that session key;
//! 6. `created_at` is within 120 s of the corrected clock.
//!
//! Each sender is also held to 60 events a minute.

use std::collections::{HashMap, VecDeque};

use nostr::{
    event::{Event, EventBuilder, Kind, Tag},
    prelude::FinalizeEvent,
    types::Timestamp,
};

use crate::{
    keys::{RoomKeys, Session, verify_proof},
    wire::{Body, Message, Mid},
};

/// The one ephemeral kind every room message uses.
pub const KIND: u16 = 23517;
/// How long relays may hold an event (NIP-40), seconds.
pub const EXPIRES_AFTER_S: u64 = 60;
/// Events further than this from the corrected clock are dropped, ms.
pub const AGE_LIMIT_MS: i64 = 120_000;
/// The padding buckets, bytes.
pub const BUCKETS: [usize; 5] = [512, 1024, 2048, 4096, 8192];
/// Events one sender may get through in a minute.
pub const SENDER_CAP: usize = 60;

/// The body as JSON, padded with a `_` filler field to the smallest bucket that holds it.
pub fn pad(body: &Body) -> Vec<u8> {
    let mut value = serde_json::to_value(body).expect("a body serialises");
    let plain = serde_json::to_vec(&value).expect("a body serialises");
    // `,"_":""` is 7 bytes before the filler itself.
    let need = plain.len() + 7;
    let bucket = BUCKETS.iter().copied().find(|b| *b >= need).unwrap_or(need);
    if let Some(map) = value.as_object_mut() {
        map.insert("_".into(), serde_json::Value::String("0".repeat(bucket - need)));
    }
    serde_json::to_vec(&value).expect("a body serialises")
}

/// Seal `body` into a signed event on the room's tag for `now_ms` (the corrected clock).
pub fn build(keys: &RoomKeys, session: &Session, body: &Body, now_ms: i64) -> Event {
    let created = (now_ms / 1000).max(0) as u64;
    EventBuilder::new(Kind::Custom(KIND), keys.seal(&pad(body)))
        .tag(Tag::custom("z", [keys.send_tag(now_ms)]))
        .tag(Tag::expiration(Timestamp::from_secs(created + EXPIRES_AFTER_S)))
        .custom_created_at(Timestamp::from_secs(created))
        .finalize(session.keys())
        .expect("a session key signs")
}

/// Why an event was dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dropped {
    Kind,
    Signature,
    Age,
    Seal,
    Body,
    NotItsKey,
    Unbound,
    Replayed,
    Spam,
}

/// Open an event under any of `keys` (the current epoch first; the old one for two minutes after
/// a rekey). Checks 1, 2, 3 and 6; the [`Gate`] does the rest.
pub fn open(keys: &[&RoomKeys], event: &Event, now_ms: i64) -> Result<Body, Dropped> {
    if event.kind != Kind::Custom(KIND) {
        return Err(Dropped::Kind);
    }
    if (event.created_at.as_secs() as i64 * 1000 - now_ms).abs() > AGE_LIMIT_MS {
        return Err(Dropped::Age);
    }
    if event.verify().is_err() {
        return Err(Dropped::Signature);
    }
    let plain = keys.iter().find_map(|k| k.open(&event.content)).ok_or(Dropped::Seal)?;
    let body: Body = serde_json::from_slice(&plain).map_err(|_| Dropped::Body)?;
    if body.from != event.pubkey.to_hex() {
        return Err(Dropped::NotItsKey);
    }
    Ok(body)
}

/// Who may speak as whom: each member id bound to its identity key, each session key to its
/// member id, and the last counter from each session.
#[derive(Debug, Default)]
pub struct Gate {
    room_id: [u8; 16],
    /// mid → identity key.
    ids: HashMap<Mid, String>,
    /// session → mid.
    sessions: HashMap<String, Mid>,
    /// session → last ctr.
    ctrs: HashMap<String, u64>,
    /// session → arrival times in the last minute.
    rate: HashMap<String, VecDeque<i64>>,
}

impl Gate {
    pub fn new(room_id: [u8; 16]) -> Self {
        Self { room_id, ..Self::default() }
    }

    /// After a rekey, proofs name the new room id; bindings made so far hold.
    pub fn set_room_id(&mut self, room_id: [u8; 16]) {
        self.room_id = room_id;
    }

    /// Whether `session` is bound to `mid`.
    pub fn speaks_for(&self, session: &str, mid: &str) -> bool {
        self.sessions.get(session).is_some_and(|m| m == mid)
    }

    /// The session currently bound to `mid`, the latest proved.
    pub fn session_of(&self, mid: &str) -> Option<&str> {
        self.sessions.iter().find(|(_, m)| *m == mid).map(|(s, _)| s.as_str())
    }

    fn bind(&mut self, session: &str, mid: &str, idpk: &str, idsig: &str) -> bool {
        if verify_proof(idpk, idsig, session, &self.room_id).as_deref() != Some(mid) {
            return false;
        }
        match self.ids.get(mid) {
            Some(known) if known != idpk => return false,
            Some(_) => {}
            None => {
                self.ids.insert(mid.to_owned(), idpk.to_owned());
            }
        }
        // A session speaks for one mid only.
        if self.sessions.get(session).is_some_and(|m| m != mid) {
            return false;
        }
        // A newer session for the same mid replaces the old (a restart).
        self.sessions.retain(|s, m| m != mid || s == session);
        self.sessions.insert(session.to_owned(), mid.to_owned());
        true
    }

    /// Checks 4 and 5 and the sender cap. A sync answer's roster entries that don't prove
    /// themselves are taken out of it; the rest bind.
    pub fn admit(&mut self, body: &mut Body, now_ms: i64) -> Result<(), Dropped> {
        let session = body.from.clone();
        let times = self.rate.entry(session.clone()).or_default();
        while times.front().is_some_and(|t| now_ms - t >= 60_000) {
            times.pop_front();
        }
        if times.len() >= SENDER_CAP {
            return Err(Dropped::Spam);
        }
        if self.ctrs.get(&session).is_some_and(|last| body.ctr <= *last) {
            return Err(Dropped::Replayed);
        }
        match &body.m {
            Message::Hello { idpk, idsig, .. } => {
                let (idpk, idsig) = (idpk.clone(), idsig.clone());
                if !self.bind(&session, &body.mid, &idpk, &idsig) {
                    return Err(Dropped::Unbound);
                }
            }
            Message::SyncReq { .. } if !self.sessions.contains_key(&session) => {
                // A newcomer asks before its Hello. It can't speak as anyone with it.
            }
            // A sync answer carries its sender's own proof in its roster: a newcomer who missed
            // the sender's Hello binds it from there.
            Message::SyncAns { roster, .. } if !self.speaks_for(&session, &body.mid) => {
                let own = roster.iter().find(|e| e.mid == body.mid && e.pk == session);
                let proved = own.is_some_and(|e| {
                    let (mid, idpk, idsig) = (e.mid.clone(), e.idpk.clone(), e.idsig.clone());
                    self.bind(&session, &mid, &idpk, &idsig)
                });
                if !proved {
                    return Err(Dropped::Unbound);
                }
            }
            _ if !self.speaks_for(&session, &body.mid) => return Err(Dropped::Unbound),
            _ => {}
        }
        if let Message::SyncAns { roster, .. } = &mut body.m {
            let mut kept = Vec::with_capacity(roster.len());
            for entry in roster.drain(..) {
                // Only someone not bound here yet binds from another's word: a member's own
                // Hello names their live session, and an answer may hold an older one.
                if entry.mid == body.mid
                    || self.speaks_for(&entry.pk, &entry.mid)
                    || (self.session_of(&entry.mid).is_none()
                        && self.bind(&entry.pk, &entry.mid, &entry.idpk, &entry.idsig))
                {
                    kept.push(entry);
                }
            }
            *roster = kept;
        }
        self.ctrs.insert(session.clone(), body.ctr);
        self.rate.entry(session).or_default().push_back(now_ms);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        keys::Identity,
        wire::{RosterEntry, Status, VERSION},
    };

    const NOW: i64 = 1_790_000_000_000;

    fn body(session: &Session, mid: &str, ctr: u64, m: Message) -> Body {
        Body {
            v: VERSION,
            from: session.pubkey(),
            mid: mid.into(),
            ctr,
            t: NOW,
            st: Status::default(),
            m,
        }
    }

    fn hello(id: &Identity, session: &Session, keys: &RoomKeys, ctr: u64) -> Body {
        body(
            session,
            &id.mid(),
            ctr,
            Message::Hello {
                name: "Mika".into(),
                app: "0.6".into(),
                caps: vec![],
                idpk: id.pubkey(),
                idsig: id.prove(&session.pubkey(), &keys.room_id()),
            },
        )
    }

    #[test]
    fn bodies_pad_to_their_bucket() {
        let s = Session::new();
        let small = pad(&body(&s, "m", 1, Message::Heartbeat));
        assert_eq!(small.len(), 512);
        let chat =
            pad(&body(&s, "m", 1, Message::Chat { id: "c".into(), text: "a".repeat(700) }));
        assert_eq!(chat.len(), 1024);
        let back: Body = serde_json::from_slice(&small).unwrap();
        assert_eq!(back.m, Message::Heartbeat);
    }

    #[test]
    fn an_event_opens_for_the_room_and_nobody_else() {
        let keys = RoomKeys::new([1; 16]);
        let s = Session::new();
        let sent = body(&s, "m", 1, Message::Heartbeat);
        let event = build(&keys, &s, &sent, NOW);
        assert_eq!(event.kind, Kind::Custom(KIND));
        assert_eq!(open(&[&keys], &event, NOW), Ok(sent.clone()));
        // An outsider at the relay holds no secret.
        assert_eq!(open(&[&RoomKeys::new([2; 16])], &event, NOW), Err(Dropped::Seal));
        // Too old, or from the future.
        assert_eq!(open(&[&keys], &event, NOW + 121_000), Err(Dropped::Age));
        // Nothing readable on the wire.
        let json = serde_json::to_string(&event).unwrap();
        assert!(!json.contains("heartbeat"), "{json}");
        assert!(json.contains(&keys.send_tag(NOW)));
    }

    #[test]
    fn a_member_cant_replay_anothers_body_under_its_own_key() {
        let keys = RoomKeys::new([1; 16]);
        let (ada, mallory) = (Session::new(), Session::new());
        let sent = body(&ada, "m", 1, Message::Heartbeat);
        let event = build(&keys, &mallory, &sent, NOW);
        assert_eq!(open(&[&keys], &event, NOW), Err(Dropped::NotItsKey));
    }

    #[test]
    fn a_tampered_event_fails_its_signature() {
        let keys = RoomKeys::new([1; 16]);
        let s = Session::new();
        let mut event = build(&keys, &s, &body(&s, "m", 1, Message::Heartbeat), NOW);
        event.content = keys.seal(&pad(&body(&s, "m", 2, Message::Leave)));
        assert_eq!(open(&[&keys], &event, NOW), Err(Dropped::Signature));
    }

    #[test]
    fn only_a_proved_session_speaks_for_a_mid() {
        let keys = RoomKeys::new([1; 16]);
        let mut gate = Gate::new(keys.room_id());
        let (id, s) = (Identity::new(), Session::new());
        // Unbound: dropped, except a newcomer's SyncReq.
        let mut beat = body(&s, &id.mid(), 1, Message::Heartbeat);
        assert_eq!(gate.admit(&mut beat, NOW), Err(Dropped::Unbound));
        let mut ask =
            body(&s, &id.mid(), 2, Message::SyncReq { nonce: "n".into(), peek: false });
        assert_eq!(gate.admit(&mut ask, NOW), Ok(()));
        assert_eq!(gate.admit(&mut hello(&id, &s, &keys, 3), NOW), Ok(()));
        assert_eq!(gate.admit(&mut body(&s, &id.mid(), 4, Message::Heartbeat), NOW), Ok(()));
        // Counters only rise.
        assert_eq!(
            gate.admit(&mut body(&s, &id.mid(), 4, Message::Heartbeat), NOW),
            Err(Dropped::Replayed)
        );
        // Someone else claiming the mid, with a fresh session and a fresh identity key: dropped.
        let (other_id, other) = (Identity::new(), Session::new());
        let mut claim = hello(&other_id, &other, &keys, 1);
        claim.mid = id.mid();
        assert_eq!(gate.admit(&mut claim, NOW), Err(Dropped::Unbound));
        assert_eq!(
            gate.admit(&mut body(&other, &id.mid(), 2, Message::Leave), NOW),
            Err(Dropped::Unbound)
        );
        // Copying the real proof onto another session doesn't hold either.
        let mut copied = hello(&id, &s, &keys, 1);
        copied.from = other.pubkey();
        assert_eq!(gate.admit(&mut copied, NOW), Err(Dropped::Unbound));
        // The same identity after a restart: a new session proves the same mid at once.
        let again = Session::new();
        assert_eq!(gate.admit(&mut hello(&id, &again, &keys, 1), NOW), Ok(()));
        assert!(gate.speaks_for(&again.pubkey(), &id.mid()));
        assert!(!gate.speaks_for(&s.pubkey(), &id.mid()));
    }

    #[test]
    fn a_sync_answer_keeps_only_proved_roster_entries() {
        let keys = RoomKeys::new([1; 16]);
        let mut gate = Gate::new(keys.room_id());
        let (ada, ada_s) = (Identity::new(), Session::new());
        gate.admit(&mut hello(&ada, &ada_s, &keys, 1), NOW).unwrap();
        let (ren, ren_s) = (Identity::new(), Session::new());
        let entry = |mid: String, pk: String, idpk: String, idsig: String| RosterEntry {
            mid,
            name: "x".into(),
            pk,
            idpk,
            idsig,
            st: Status::default(),
            joined_ms_ago: 0,
            gone: false,
        };
        let good = entry(
            ren.mid(),
            ren_s.pubkey(),
            ren.pubkey(),
            ren.prove(&ren_s.pubkey(), &keys.room_id()),
        );
        let forged = entry(
            "feedfacefeedface".into(),
            Session::new().pubkey(),
            ren.pubkey(),
            good.idsig.clone(),
        );
        let mut answer = body(
            &ada_s,
            &ada.mid(),
            2,
            Message::SyncAns {
                nonce: "n".into(),
                playhead: None,
                relays: vec![],
                relays_seq: 0,
                epoch: 0,
                now_pos: None,
                playing_now: false,
                started: false,
                roster: vec![good, forged],
            },
        );
        gate.admit(&mut answer, NOW).unwrap();
        let Message::SyncAns { roster, .. } = &answer.m else { unreachable!() };
        assert_eq!(roster.len(), 1);
        assert!(gate.speaks_for(&ren_s.pubkey(), &ren.mid()));
    }

    #[test]
    fn a_sync_answers_roster_never_rebinds_a_member_to_an_older_session() {
        // Neon: Ren restarted with a new session; Mika's answer still named his old one, and
        // binding it took the live session from him.
        let keys = RoomKeys::new([1; 16]);
        let mut gate = Gate::new(keys.room_id());
        let (mika, mika_s) = (Identity::new(), Session::new());
        gate.admit(&mut hello(&mika, &mika_s, &keys, 1), NOW).unwrap();
        let (ren, old_s, new_s) = (Identity::new(), Session::new(), Session::new());
        gate.admit(&mut hello(&ren, &new_s, &keys, 1), NOW).unwrap();
        let stale = RosterEntry {
            mid: ren.mid(),
            name: "Ren".into(),
            pk: old_s.pubkey(),
            idpk: ren.pubkey(),
            idsig: ren.prove(&old_s.pubkey(), &keys.room_id()),
            st: Status::default(),
            joined_ms_ago: 0,
            gone: false,
        };
        let mut answer = body(
            &mika_s,
            &mika.mid(),
            2,
            Message::SyncAns {
                nonce: "n".into(),
                playhead: None,
                relays: vec![],
                relays_seq: 0,
                epoch: 0,
                now_pos: None,
                playing_now: false,
                started: false,
                roster: vec![stale],
            },
        );
        gate.admit(&mut answer, NOW).unwrap();
        let Message::SyncAns { roster, .. } = &answer.m else { unreachable!() };
        assert!(roster.is_empty(), "the stale entry is taken out");
        assert!(gate.speaks_for(&new_s.pubkey(), &ren.mid()));
        assert!(!gate.speaks_for(&old_s.pubkey(), &ren.mid()));
    }

    #[test]
    fn a_newcomer_takes_an_answer_whose_sender_proves_itself_in_the_roster() {
        let keys = RoomKeys::new([1; 16]);
        let (ada, ada_s) = (Identity::new(), Session::new());
        let own = RosterEntry {
            mid: ada.mid(),
            name: "Ada".into(),
            pk: ada_s.pubkey(),
            idpk: ada.pubkey(),
            idsig: ada.prove(&ada_s.pubkey(), &keys.room_id()),
            st: Status::default(),
            joined_ms_ago: 0,
            gone: false,
        };
        let answer = |roster: Vec<RosterEntry>| {
            body(
                &ada_s,
                &ada.mid(),
                5,
                Message::SyncAns {
                    nonce: "n".into(),
                    playhead: None,
                    relays: vec![],
                    relays_seq: 0,
                    epoch: 0,
                    now_pos: None,
                    playing_now: false,
                    started: false,
                    roster,
                },
            )
        };
        // Without her own entry, or with one that doesn't prove her: dropped.
        assert_eq!(
            Gate::new(keys.room_id()).admit(&mut answer(vec![]), NOW),
            Err(Dropped::Unbound)
        );
        let forged = RosterEntry {
            idsig: Identity::new().prove(&ada_s.pubkey(), &keys.room_id()),
            ..own.clone()
        };
        assert_eq!(
            Gate::new(keys.room_id()).admit(&mut answer(vec![forged]), NOW),
            Err(Dropped::Unbound)
        );
        // With it: taken, and Ada is bound from then on.
        let mut gate = Gate::new(keys.room_id());
        assert_eq!(gate.admit(&mut answer(vec![own]), NOW), Ok(()));
        assert!(gate.speaks_for(&ada_s.pubkey(), &ada.mid()));
    }

    #[test]
    fn a_sender_gets_sixty_a_minute() {
        let keys = RoomKeys::new([1; 16]);
        let mut gate = Gate::new(keys.room_id());
        let (id, s) = (Identity::new(), Session::new());
        gate.admit(&mut hello(&id, &s, &keys, 1), NOW).unwrap();
        for ctr in 2..=60 {
            gate.admit(&mut body(&s, &id.mid(), ctr, Message::Heartbeat), NOW).unwrap();
        }
        assert_eq!(
            gate.admit(&mut body(&s, &id.mid(), 61, Message::Heartbeat), NOW),
            Err(Dropped::Spam)
        );
        assert_eq!(
            gate.admit(&mut body(&s, &id.mid(), 62, Message::Heartbeat), NOW + 60_000),
            Ok(())
        );
    }
}
