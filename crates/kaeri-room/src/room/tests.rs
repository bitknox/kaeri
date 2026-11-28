//! The rules of waits, playing, words and timeouts, as tables.

use super::*;
use crate::wire::{Body, Ep, Message, StateCode, Status, VERSION};

fn ep() -> Ep {
    Ep(4242, "9".into())
}

/// A room seen from `me` ("a"), with `others` said Hello at 0.
fn room(others: &[&str]) -> Room {
    let mut room = Room::new("a", "Ada", "pa", 0);
    for mid in others {
        room.receive(
            body(
                mid,
                Message::Hello {
                    name: mid.to_uppercase(),
                    app: "0.6".into(),
                    caps: vec![],
                    idpk: String::new(),
                    idsig: String::new(),
                },
                Status::default(),
            ),
            0,
        );
    }
    room
}

/// A body from `mid`, its counter above every one before it, as a sender's are.
fn body(mid: &str, m: Message, st: Status) -> Body {
    static CTR: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let ctr = CTR.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    Body { v: VERSION, from: format!("p{mid}"), mid: mid.into(), ctr, t: 0, st, m }
}

/// A body with its counter given: what a member said, in the order they said it.
fn numbered(mid: &str, m: Message, st: Status, ctr: u64) -> Body {
    Body { ctr, ..body(mid, m, st) }
}

fn watching(rdy: Option<u64>) -> Status {
    Status { s: StateCode::Watching, ep: Some(ep()), rdy, play: true, ..Status::default() }
}

fn loading() -> Status {
    Status { s: StateCode::Loading, ep: Some(ep()), ..Status::default() }
}

fn status(room: &mut Room, mid: &str, st: Status, now: i64) {
    room.receive(body(mid, Message::Status, st), now);
}

/// Start the room on the episode, playing, ready here.
fn start(room: &mut Room, now: i64) -> u64 {
    let head = room
        .write(
            now,
            now,
            |h| {
                h.play = true;
                h.cause = Cause::Episode;
            },
            Some(ep()),
        )
        .unwrap();
    room.set_status(watching(Some(head.wseq)), now);
    head.wseq
}

#[test]
fn the_room_waits_for_a_loader_and_plays_when_it_is_ready() {
    let mut room = room(&["r"]);
    let wseq = start(&mut room, 0);
    status(&mut room, "r", loading(), 10);
    assert_eq!(room.waited(10).iter().map(|w| w.mid.as_str()).collect::<Vec<_>>(), ["r"]);
    assert!(!room.playing());
    status(&mut room, "r", watching(Some(wseq)), 6_000);
    assert!(room.waited(6_000).is_empty());
    // Before play, a count of 3 s.
    assert_eq!(room.starts_at(), Some(9_000));
    room.settle(9_000);
    assert!(room.playing());
    assert_eq!(room.position(11_000), Some(2.0));
    assert_eq!(room.word(room.member("r").unwrap(), 11_000), Word::Watching);
}

#[test]
fn a_peer_that_has_not_heard_the_write_is_still_waited_for() {
    let mut room = room(&["r"]);
    status(&mut room, "r", watching(Some(0)), 0);
    start(&mut room, 0);
    assert_eq!(room.waited(100).len(), 1, "Ren still answers the old wait");
}

#[test]
fn carry_on_plays_past_the_waited_and_they_join_quietly() {
    let mut room = room(&["r", "m"]);
    let wseq = start(&mut room, 0);
    status(&mut room, "m", watching(Some(wseq)), 0);
    status(&mut room, "r", loading(), 0);
    room.settle(5_000);
    let waited: Vec<Mid> = room.waited(21_000).into_iter().map(|w| w.mid).collect();
    room.write(
        21_000,
        21_000,
        |h| {
            h.play = true;
            h.cause = Cause::CarryOn;
            h.carry.extend(waited);
        },
        None,
    );
    assert!(room.waited(21_000).is_empty());
    assert!(room.playing());
    assert_eq!(room.carried_past(21_000).len(), 1);
}

#[test]
fn a_stall_holds_the_room_only_for_the_staller() {
    let mut room = room(&["r", "m"]);
    let wseq = start(&mut room, 0);
    status(&mut room, "m", watching(Some(wseq)), 0);
    status(&mut room, "r", watching(Some(wseq)), 0);
    room.settle(3_000);
    assert!(room.playing());
    // Ren writes the stall; Mika hasn't answered it, yet only Ren holds the room.
    let mut ren = room.playhead().unwrap().clone();
    ren.seq += 1;
    ren.cause = Cause::Stall;
    ren.wseq = ren.seq;
    ren.by = "r".into();
    room.receive(
        body(
            "r",
            Message::Playhead(ren),
            Status { s: StateCode::Buffering, ..watching(Some(wseq)) },
        ),
        10_000,
    );
    let waited = room.waited(10_000);
    assert_eq!(waited.len(), 1);
    assert_eq!((waited[0].mid.as_str(), waited[0].reason), ("r", Reason::Buffering));
}

#[test]
fn nobody_waits_for_a_dropped_connection() {
    let mut room = room(&["r"]);
    start(&mut room, 0);
    status(&mut room, "r", loading(), 0);
    assert!(!room.waited(39_000).is_empty());
    room.settle(41_000);
    assert!(room.waited(41_000).is_empty());
    assert_eq!(room.word(room.member("r").unwrap(), 41_000), Word::Reconnecting);
    room.settle(181_000);
    assert_eq!(room.word(room.member("r").unwrap(), 181_000), Word::Left);
}

#[test]
fn a_backgrounded_member_reads_reconnecting_later_and_left_after_ten_minutes() {
    let mut room = room(&["r"]);
    start(&mut room, 0);
    status(
        &mut room,
        "r",
        Status { s: StateCode::PlayerClosed, bg: true, hb: 30, ..Status::default() },
        0,
    );
    assert_eq!(room.word(room.member("r").unwrap(), 60_000), Word::PlayerClosed);
    assert_eq!(room.word(room.member("r").unwrap(), 76_000), Word::Reconnecting);
    room.settle(200_000);
    assert_ne!(room.word(room.member("r").unwrap(), 200_000), Word::Left);
    room.settle(601_000);
    assert_eq!(room.word(room.member("r").unwrap(), 601_000), Word::Left);
}

#[test]
fn dont_wait_and_other_states_leave_the_wait() {
    for st in [
        Status { dw: true, ..loading() },
        Status { s: StateCode::Choosing, ..Status::default() },
        Status { s: StateCode::NoSource, ..Status::default() },
        Status { s: StateCode::PlayerClosed, ..Status::default() },
        Status { ep: Some(Ep(4242, "8".into())), ..watching(None) },
        Status { rejoin: true, ..loading() },
    ] {
        let mut room = room(&["r"]);
        start(&mut room, 0);
        status(&mut room, "r", st.clone(), 0);
        assert!(room.waited(0).is_empty(), "{st:?}");
    }
}

#[test]
fn words_follow_the_table() {
    let mut room = room(&["r"]);
    let wseq = start(&mut room, 0);
    status(&mut room, "r", watching(Some(wseq)), 0);
    assert_eq!(room.word(room.member("r").unwrap(), 0), Word::Ready);
    room.settle(3_000);
    room.write(
        5_000,
        5_000,
        |h| {
            h.play = false;
            h.cause = Cause::Pause;
        },
        None,
    );
    assert_eq!(room.word(room.member("r").unwrap(), 5_000), Word::Paused);
    assert!(room.paused_room(room.member("a").unwrap()));
    status(&mut room, "r", Status { ep: Some(Ep(4242, "8".into())), ..watching(None) }, 5_000);
    assert_eq!(room.word(room.member("r").unwrap(), 5_000), Word::OtherEpisode);
}

#[test]
fn concurrent_writes_agree_on_the_earlier_one() {
    let mut room = room(&["m"]);
    start(&mut room, 0);
    room.settle(3_000);
    let base = room.playhead().unwrap().clone();
    let mut pause = base.clone();
    pause.seq += 1;
    pause.t = 100;
    pause.play = false;
    pause.cause = Cause::Pause;
    pause.pk = "pa".into();
    let mut play = base;
    play.seq = pause.seq;
    play.t = 400;
    play.cause = Cause::Play;
    play.pk = "pm".into();
    room.apply(play, 4_000);
    room.apply(pause.clone(), 4_000);
    assert_eq!(room.playhead().unwrap().cause, Cause::Pause);
}

#[test]
fn names_are_made_unique() {
    let room = room(&["r"]);
    assert_eq!(room.free_name("R"), "R 2");
    assert_eq!(room.free_name("Sol"), "Sol");
}

#[test]
fn a_late_joiner_starts_at_the_answered_place() {
    let mut mika = room(&[]);
    start(&mut mika, 0);
    mika.settle(3_000);
    let answer = mika.sync_answer("n", 13_000);
    let mut ren = Room::new("r", "Ren", "pr", 13_000);
    ren.receive(body("a", answer, watching(Some(1))), 13_100);
    assert!(ren.playing());
    assert_eq!(ren.position(13_100), Some(10.0));
    assert!(!ren.before_play());
    assert_eq!(ren.members().len(), 2);
}

#[test]
fn a_members_older_word_coming_late_changes_nothing_of_theirs() {
    // Their events come by several relays, each in its own time: a Status said before another
    // can arrive after it, and a receiver takes it (it wasn't seen yet).
    let n = 1_000_000;
    let mut room = room(&["m"]);
    room.receive(numbered("m", Message::Status, watching(Some(1)), n + 2), 100);
    room.receive(numbered("m", Message::Status, loading(), n + 1), 200);
    assert_eq!(room.member("m").unwrap().status.s, StateCode::Watching);
    // Gone: something said before the Leave doesn't bring them back.
    room.receive(numbered("m", Message::Leave, watching(Some(1)), n + 4), 300);
    room.receive(numbered("m", Message::Status, watching(Some(1)), n + 3), 400);
    assert!(room.member("m").unwrap().left_at.is_some());
    // Back with something newer.
    room.receive(numbered("m", Message::Status, watching(Some(1)), n + 5), 500);
    assert!(room.member("m").unwrap().left_at.is_none());
    // A restart is a new session, its counter from the start again: newer all the same.
    room.receive(
        Body { from: "pm-again".into(), ..numbered("m", Message::Status, loading(), 1) },
        600,
    );
    assert_eq!(room.member("m").unwrap().status.s, StateCode::Loading);
}

#[test]
fn an_older_answer_coming_late_keeps_the_newer_place() {
    let mut mika = room(&[]);
    start(&mut mika, 0);
    mika.settle(3_000);
    let older = mika.sync_answer("n1", 13_000);
    let newer = mika.sync_answer("n2", 23_000);
    let mut ren = Room::new("r", "Ren", "pr", 23_000);
    ren.receive(numbered("a", newer, watching(Some(1)), 1_000_002), 23_100);
    ren.receive(numbered("a", older, watching(Some(1)), 1_000_001), 23_200);
    let at = ren.position(23_200).unwrap();
    assert!(at > 19.0, "{at}");
}

/// Three relays, health bits per member (bit i: relay i works there).
fn relayed(others: &[&str]) -> Room {
    let mut room = room(others);
    room.set_relays(
        vec![
            "wss://one.example".into(),
            "wss://two.example".into(),
            "wss://three.example".into(),
        ],
        1,
    );
    room
}

fn health(room: &mut Room, mid: &str, ok: u8, now: i64) {
    if mid == "a" {
        let st = Status { ok, ..room.my_status().clone() };
        room.set_status(st, now);
    } else {
        status(room, mid, Status { ok, ..watching(None) }, now);
    }
}

#[test]
fn a_relay_the_majority_cant_reach_for_a_minute_is_swapped_for_a_spare() {
    let spares = ["wss://two.example".to_owned(), "wss://spare.example".to_owned()];
    let mut room = relayed(&["r", "s"]);
    // Relay 1 (two.example) fails for Ada and Ren; Sol still has it. Two of three: a majority.
    for now in [0, 30_000] {
        health(&mut room, "a", 0b101, now);
        health(&mut room, "r", 0b101, now);
        health(&mut room, "s", 0b111, now);
        assert_eq!(room.relay_change(now, &spares), None);
    }
    health(&mut room, "a", 0b101, 60_000);
    health(&mut room, "r", 0b101, 60_000);
    health(&mut room, "s", 0b111, 60_000);
    assert_eq!(
        room.relay_change(60_000, &spares),
        Some(vec![
            "wss://one.example".into(),
            "wss://spare.example".into(),
            "wss://three.example".into()
        ])
    );
}

#[test]
fn a_swap_heard_from_another_member_starts_the_minute_again() {
    // Ren swapped the dead relay first. The reports Ada holds still describe the old list, so
    // her own minute, already up, must not swap the fresh spare straight out for the next one.
    let spares = ["wss://spare.example".to_owned(), "wss://next.example".to_owned()];
    let mut room = relayed(&["r", "s"]);
    for now in [0, 59_000] {
        health(&mut room, "a", 0b011, now);
        health(&mut room, "r", 0b011, now);
        health(&mut room, "s", 0b011, now);
        assert_eq!(room.relay_change(now, &spares), None);
    }
    let swapped: Vec<String> = vec![
        "wss://one.example".into(),
        "wss://two.example".into(),
        "wss://spare.example".into(),
    ];
    room.set_relays(swapped.clone(), 2);
    assert_eq!(room.relay_change(60_000, &spares), None, "the old reports don't count");
    assert_eq!(room.relay_change(119_000, &spares), None);
    // Still unhealthy for everyone a minute after the change: then it goes too.
    let changed = room.relay_change(120_000, &spares).unwrap();
    assert_eq!(changed[2], "wss://next.example");
    // The same for adding one: a minute with too few relays before the change doesn't carry.
    let mut room = relayed(&["r", "s"]);
    health(&mut room, "a", 0b001, 0);
    health(&mut room, "r", 0b111, 0);
    health(&mut room, "s", 0b111, 0);
    assert_eq!(room.relay_change(0, &spares), None);
    room.set_relays(swapped, 2);
    assert_eq!(room.relay_change(60_000, &spares), None);
}

#[test]
fn one_members_trouble_never_evicts_a_relay_and_only_adds_one() {
    let spares = ["wss://spare.example".to_owned()];
    let mut room = relayed(&["r", "s"]);
    // Only Ada can't reach two relays: no majority, so nothing is evicted, but after a minute
    // with fewer than two healthy relays her device adds the spare.
    for now in [0, 59_000] {
        health(&mut room, "a", 0b001, now);
        health(&mut room, "r", 0b111, now);
        health(&mut room, "s", 0b111, now);
        assert_eq!(room.relay_change(now, &spares), None);
    }
    health(&mut room, "a", 0b001, 60_000);
    let changed = room.relay_change(60_000, &spares).unwrap();
    assert_eq!(changed.len(), 4);
    assert_eq!(changed[3], "wss://spare.example");
    // Members who never reported health don't count.
    let mut room = relayed(&["r"]);
    health(&mut room, "r", 0, 0);
    health(&mut room, "a", 0b111, 0);
    assert_eq!(room.relay_change(120_000, &spares), None);
}

#[test]
fn before_play_and_while_paused_the_room_waits_for_no_one() {
    // Nobody pressed Play: Ren still loading is a row, not a wait ("Not started yet").
    let mut room = room(&["r"]);
    let head = room
        .write(
            0,
            0,
            |h| {
                h.play = false;
                h.cause = Cause::Episode;
            },
            Some(ep()),
        )
        .unwrap();
    room.set_status(watching(Some(head.wseq)), 0);
    status(&mut room, "r", loading(), 10);
    assert!(room.waited(5_000).is_empty());
    assert_eq!(room.word(room.member("r").unwrap(), 5_000), Word::Loading);
    // Play for everyone: now the room waits for Ren.
    room.write(
        6_000,
        6_000,
        |h| {
            h.play = true;
            h.cause = Cause::Play;
        },
        None,
    )
    .unwrap();
    assert_eq!(room.waited(6_000).iter().map(|w| w.mid.as_str()).collect::<Vec<_>>(), ["r"]);
}

#[test]
fn a_member_loading_after_a_seek_is_waited_for_as_loading_as_its_row_says() {
    let mut room = room(&["r", "m"]);
    let wseq = start(&mut room, 0);
    status(&mut room, "m", watching(Some(wseq)), 0);
    status(&mut room, "r", watching(Some(wseq)), 0);
    room.settle(3_000);
    room.write(
        5_000,
        5_000,
        |h| {
            h.pos = 35.0;
            h.cause = Cause::Seek;
        },
        None,
    )
    .unwrap();
    // Mika seeks; Ada (here) is on it. Ren's player still seeks: Seeking.
    let wseq = room.playhead().unwrap().wseq;
    room.set_status(watching(Some(wseq)), 5_000);
    status(&mut room, "m", watching(Some(wseq)), 5_000);
    let waited = room.waited(5_500);
    assert_eq!((waited[0].mid.as_str(), waited[0].reason), ("r", Reason::Seeking));
    // Ren's player reopens the episode: Loading, in the wait and on the row alike.
    status(&mut room, "r", loading(), 6_000);
    let waited = room.waited(6_500);
    assert_eq!((waited[0].mid.as_str(), waited[0].reason), ("r", Reason::Loading));
    assert_eq!(room.word(room.member("r").unwrap(), 6_500), Word::Loading);
}

#[test]
fn a_stale_sync_answer_never_rewinds_the_room() {
    // Neon: Mika, not yet told of Ada's seek, answers a newcomer with the old playhead and its
    // clock; Ada took that answer's place and rewound by minutes.
    let mut ada = room(&[]);
    start(&mut ada, 0);
    ada.settle(3_000);
    let stale = ada.sync_answer("n", 13_000);
    ada.write(
        14_000,
        14_000,
        |h| {
            h.pos = 900.0;
            h.cause = Cause::Seek;
        },
        None,
    )
    .unwrap();
    ada.receive(body("m", stale, watching(Some(1))), 14_100);
    assert_eq!(ada.position(14_100), Some(900.0));
}

#[test]
fn a_sync_request_is_answered_in_turn_when_the_ones_before_stay_quiet() {
    // Neon: only rank 0 answered; one unreachable member and a newcomer heard nothing.
    let mut ada = room(&["m", "r"]);
    let nonce = (0..)
        .map(|i| format!("n{i}"))
        .find(|n| ada.rank(n, 0) == 2)
        .expect("a nonce that puts Ada last");
    ada.will_answer(&nonce, 0);
    assert!(ada.answers_due(0).is_empty());
    assert!(ada.answers_due(799).is_empty());
    assert_eq!(ada.answers_due(800), std::slice::from_ref(&nonce));
    // Someone before Ada answered: she stays quiet.
    ada.will_answer(&nonce, 1_000);
    let answer = Room::new("m", "Mika", "pm", 0).sync_answer(&nonce, 1_100);
    ada.receive(body("m", answer, watching(None)), 1_100);
    assert!(ada.answers_due(2_000).is_empty());
}

#[test]
fn a_sync_answer_whose_playhead_lost_the_tie_applies_no_place() {
    // Neon: Mika's seek tied Ada's pause on seq and lost on time; a sync answer carrying it
    // still moved Ada's place, because only the seq was compared.
    let mut ada = room(&[]);
    start(&mut ada, 0);
    ada.settle(3_000);
    let mut answer = ada.sync_answer("n", 13_000);
    let mine = ada
        .write(
            14_000,
            14_000,
            |h| {
                h.pos = 720.0;
                h.play = false;
                h.cause = Cause::Pause;
            },
            None,
        )
        .unwrap();
    if let Message::SyncAns { playhead: Some(h), now_pos, playing_now, .. } = &mut answer {
        (h.seq, h.t, h.by) = (mine.seq, 14_500, "m".into());
        (h.pos, h.play) = (900.0, true);
        (*now_pos, *playing_now) = (Some(900.0), true);
    }
    ada.receive(body("m", answer, watching(Some(1))), 14_600);
    assert_eq!(ada.position(15_600), Some(720.0));
    assert!(!ada.playing());
}

#[test]
fn a_sync_answer_never_rebinds_a_member_heard_here() {
    // Neon: Mika still held Ren's session from before his restart; her roster put it back over
    // the one Ren had just said Hello with.
    let mut ada = room(&["r"]);
    let mut mika = Room::new("m", "Mika", "pm", 0);
    mika.receive(body("r", Message::Heartbeat, Status::default()), 0);
    let mut answer = mika.sync_answer("n", 100);
    if let Message::SyncAns { roster, .. } = &mut answer {
        let ren = roster.iter_mut().find(|e| e.mid == "r");
        if let Some(ren) = ren {
            ren.pk = "old-session".into();
        } else {
            roster.push(crate::wire::RosterEntry {
                mid: "r".into(),
                name: "R".into(),
                pk: "old-session".into(),
                idpk: String::new(),
                idsig: String::new(),
                st: Status::default(),
                joined_ms_ago: 0,
                gone: false,
            });
        }
    }
    ada.receive(body("m", answer, Status::default()), 200);
    assert_eq!(ada.member("r").unwrap().session, "pr");
}

#[test]
fn a_newcomer_takes_the_rooms_key_epoch_from_a_sync_answer() {
    // Neon: a newcomer by a new invite stayed at epoch 0, so its own New invite went out at
    // seq 1, lost to the room's, and split it.
    let mut ada = room(&[]);
    ada.take_epoch(3, 50, "pa");
    let answer = ada.sync_answer("n", 100);
    let mut ren = Room::new("r", "Ren", "pr", 0);
    ren.receive(body("a", answer, Status::default()), 200);
    assert_eq!(ren.epoch().0, 3);
    // Its own new invite goes past the room's, and the room takes it.
    assert!(ada.take_epoch(ren.epoch().0 + 1, 300, "pr"));
}
