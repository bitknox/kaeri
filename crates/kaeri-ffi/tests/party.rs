//! Watch parties as the bindings drive them: an invite written and read, and a party joined on
//! the fake server, steered by a member it plays, and left, with no network.
#![cfg(all(feature = "testing", feature = "party"))]

use std::{
    sync::{Arc, mpsc},
    thread,
    time::Duration,
};

use kaeri_core::ids::AnilistId;
use kaeri_ffi::{
    FfiChatFixture, FfiDirs, FfiEngine, FfiHost, FfiStart, parse_party_invite,
    party_invite_link,
};
use kaeri_model::{
    command::Command,
    event::Event,
    party::{
        PartyChatBody, PartyChatRefusal, PartyCommand, PartyInvite, PartyMessageKind, Reaction,
    },
};

const PATIENCE: Duration = Duration::from_secs(30);

struct Quiet;

impl FfiHost for Quiet {
    fn open_url(&self, _url: String) -> bool {
        false
    }

    fn notify(&self, _title: String, _body: String) {}

    fn secret_get(&self, _id: String) -> Option<String> {
        None
    }

    fn secret_set(&self, _id: String, _value: String) -> bool {
        false
    }

    fn secret_delete(&self, _id: String) -> bool {
        true
    }
}

fn read_until(
    engine: &Arc<FfiEngine>,
    until: impl Fn(&Event) -> bool + Send + 'static,
) -> Vec<Event> {
    let engine = Arc::clone(engine);
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut read = Vec::new();
        let found = loop {
            let batch = futures::executor::block_on(engine.next_events());
            if batch.is_empty() {
                break false;
            }
            let found = batch.iter().any(&until);
            read.extend(batch);
            if found {
                break true;
            }
        };
        let _ = tx.send((read, found));
    });
    let (read, found) = rx.recv_timeout(PATIENCE).expect("the event in time");
    assert!(found, "the engine stopped first, having said: {read:?}");
    read
}

#[test]
fn an_invite_reads_back_as_it_was_written() {
    let invite = PartyInvite {
        server: "syncplay.example:8999".into(),
        room: "amber anchor".into(),
        id: AnilistId::new(21),
        episode: "3".into(),
    };
    let link = party_invite_link(invite.clone());
    assert!(link.starts_with("kaeri://party?"), "{link}");
    assert_eq!(parse_party_invite(link), Some(invite));
    assert_eq!(parse_party_invite("kaeri://party?room=amber".into()), None);
}

#[test]
fn a_party_is_joined_hears_its_members_and_is_left() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let dir = |name: &str| root.path().join(name).display().to_string();
    let dirs = FfiDirs { config: dir("config"), data: dir("data"), cache: dir("cache") };
    let engine = FfiEngine::start_testing(dirs, Arc::new(Quiet), FfiStart::default())
        .expect("the engine starts");
    let party = engine.start_fake_party().expect("the fake server starts");

    engine.send(Command::PartyJoin {
        server: party.server(),
        room: "amber-anchor".into(),
        name: Some("Ada".into()),
    });
    let read = read_until(&engine, |e| matches!(e, Event::PartyMembers(_)));
    assert!(read.iter().any(|e| matches!(e, Event::PartyJoined(view) if view.name == "Ada")));

    party.add_member("amber-anchor".into(), "Mika".into());
    party.member_says("Mika".into(), "starting soon".into());
    let read = read_until(
        &engine,
        |e| matches!(e, Event::PartyMessage(m) if matches!(&m.kind, PartyMessageKind::Chat { .. })),
    );
    let said = read.iter().find_map(|e| match e {
        Event::PartyMessage(m) if matches!(&m.kind, PartyMessageKind::Chat { from } if from == "Mika") => {
            Some(m.text.clone())
        }
        _ => None,
    });
    assert!(said.is_some_and(|text| text.contains("starting soon")), "{read:?}");

    engine.send(Command::PartyLeave);
    read_until(&engine, |e| matches!(e, Event::PartyLeft { reason: None }));
    party.stop();
    engine.shutdown();
}

fn testing_engine(root: &tempfile::TempDir) -> Arc<FfiEngine> {
    let dir = |name: &str| root.path().join(name).display().to_string();
    let dirs = FfiDirs { config: dir("config"), data: dir("data"), cache: dir("cache") };
    FfiEngine::start_testing(dirs, Arc::new(Quiet), FfiStart::default())
        .expect("the engine starts")
}

/// Join as Noa; what was said on the way, which a later read would not see again.
fn join(engine: &Arc<FfiEngine>, party: &kaeri_ffi::FfiFakeParty, room: &str) -> Vec<Event> {
    engine.send(Command::PartyJoin {
        server: party.server(),
        room: room.into(),
        name: Some("Noa".into()),
    });
    read_until(engine, |e| matches!(e, Event::PartyMembers(_)))
}

fn said_lines(event: &Event) -> Option<Vec<String>> {
    let Event::PartyChat(conversation) = event else { return None };
    Some(
        conversation
            .lines
            .iter()
            .filter_map(|l| match &l.body {
                PartyChatBody::Said { text } => Some(format!("{}: {text}", l.from)),
                PartyChatBody::Reacted { .. } => None,
            })
            .collect(),
    )
}

#[test]
fn chat_is_sent_shown_on_its_echo_and_a_reaction_is_a_moment() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let engine = testing_engine(&root);
    let party = engine.start_fake_party().expect("the fake server starts");
    let read = join(&engine, &party, "amber-anchor");
    assert!(
        read.iter()
            .any(|e| matches!(e, Event::PartyChatState(s) if s.allowed && s.max_length == 150)),
        "{read:?}"
    );

    engine.send(Command::Party(PartyCommand::Say { text: "back in 2".into() }));
    let read = read_until(&engine, |e| {
        said_lines(e).is_some_and(|l| l.contains(&"Noa: back in 2".to_owned()))
    });
    let own = read.iter().rev().find_map(|e| match e {
        Event::PartyChat(c) => c.lines.iter().find(|l| l.you).cloned(),
        _ => None,
    });
    assert!(own.is_some_and(|line| line.from == "Noa"), "{read:?}");
    assert_eq!(party.chat_taken(), vec!["Noa: back in 2".to_owned()]);

    party.chat_fixture("amber-anchor".into(), FfiChatFixture::Conversation);
    let read = read_until(
        &engine,
        |e| matches!(e, Event::PartyChat(c) if c.lines.iter().any(|l| matches!(&l.body, PartyChatBody::Said { text } if text == "this part again"))),
    );
    assert!(read.iter().any(|e| matches!(e, Event::PartyReaction(r) if r.reaction == Reaction::Clap && r.at == 204.0 && r.from == "Sol")));
    let conversation = read.iter().rev().find_map(|e| match e {
        Event::PartyChat(c) => Some(c.clone()),
        _ => None,
    });
    let conversation = conversation.expect("a conversation");
    assert_eq!(conversation.marks.len(), 4);
    assert!(conversation.lines.iter().any(|l| matches!(&l.body, PartyChatBody::Reacted { with, .. } if with == &["Mika".to_owned()])));

    // Muted: Ren's lines are marked, and nothing of theirs reaches the video.
    engine.send(Command::Party(PartyCommand::Mute { name: "Ren".into(), muted: true }));
    let read =
        read_until(&engine, |e| matches!(e, Event::PartyChatState(s) if s.muted == ["Ren"]));
    assert!(read.iter().any(|e| matches!(e, Event::PartyChat(c) if c.lines.iter().filter(|l| l.from == "Ren").all(|l| l.muted))));

    engine.send(Command::PartyLeave);
    read_until(&engine, |e| matches!(e, Event::PartyLeft { reason: None }));
    party.stop();
    engine.shutdown();
}

#[test]
fn a_server_with_chat_off_refuses_and_a_lost_line_comes_back_as_failed() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let engine = testing_engine(&root);
    let party = engine.start_fake_party().expect("the fake server starts");
    party.chat_fixture("amber-anchor".into(), FfiChatFixture::ChatOff);
    let read = join(&engine, &party, "amber-anchor");
    assert!(
        read.iter().any(|e| matches!(e, Event::PartyChatState(s) if !s.allowed)),
        "{read:?}"
    );
    engine.send(Command::Party(PartyCommand::Say { text: "anyone".into() }));
    read_until(&engine, |e| matches!(e, Event::PartyChatRefused(PartyChatRefusal::Off)));
    engine.send(Command::PartyLeave);
    read_until(&engine, |e| matches!(e, Event::PartyLeft { reason: None }));

    party.chat_fixture("amber-anchor".into(), FfiChatFixture::NotSent);
    join(&engine, &party, "amber-anchor");
    engine.send(Command::Party(PartyCommand::Say { text: "back in 2".into() }));
    let read = read_until(&engine, |e| matches!(e, Event::PartyChat(c) if c.failed.is_some()));
    assert!(
        read.iter().any(
            |e| matches!(e, Event::PartyChat(c) if c.failed.as_deref() == Some("back in 2"))
        )
    );
    // Rate limit: three in five seconds, then Wait.
    for text in ["one", "two", "three", "four"] {
        engine.send(Command::Party(PartyCommand::Say { text: text.into() }));
    }
    read_until(&engine, |e| {
        matches!(e, Event::PartyChatRefused(PartyChatRefusal::Wait { .. }))
    });
    // Reactions need a playing episode.
    engine.send(Command::Party(PartyCommand::React { reaction: Reaction::Laugh }));
    read_until(&engine, |e| matches!(e, Event::PartyChatRefused(PartyChatRefusal::NotPlaying)));
    engine.send(Command::PartyLeave);
    read_until(&engine, |e| matches!(e, Event::PartyLeft { reason: None }));
    party.stop();
    engine.shutdown();
}
