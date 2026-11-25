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

fn body(mid: &str, m: Message, st: Status) -> Body {
    Body { v: VERSION, from: format!("p{mid}"), mid: mid.into(), ctr: 1, t: 0, st, m }
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
