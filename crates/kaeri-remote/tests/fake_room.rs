//! The fake TV in a room, as the Fixtures app's remote screen shows it: a phone hears the
//! conversation, and a line it sends as the TV comes back as the TV's own.
#![cfg(feature = "testing")]

use std::time::Duration;

use kaeri_remote::{
    client::{Hello, RemoteClient},
    command::RemoteCommand,
    fake::FakeTv,
    identity::Identity,
    state::{RemoteState, RoomInfo},
};
use tokio::sync::watch;

const WAIT: Duration = Duration::from_secs(5);

/// The first room state that `ok` accepts.
async fn room_where(
    states: &mut watch::Receiver<Option<RemoteState>>,
    ok: impl Fn(&RoomInfo) -> bool,
) -> RoomInfo {
    tokio::time::timeout(WAIT, async {
        loop {
            if let Some(room) =
                states.borrow_and_update().as_ref().and_then(|s| s.room.as_deref())
                && ok(room)
            {
                return room.clone();
            }
            states.changed().await.unwrap();
        }
    })
    .await
    .expect("the room state arrives")
}

#[tokio::test]
async fn a_phone_hears_the_tvs_room_and_its_line_comes_back_as_the_tvs() {
    let tv = FakeTv::start("Living room", Vec::new()).await.unwrap();
    let phone = Identity::generate().unwrap();
    let hello = Hello { name: "Hall phone".into(), app: "test/0".into() };
    let mut client = RemoteClient::connect(tv.addr(), &phone, None, &hello).await.unwrap();
    client.pair(&tv.open_code()).await.unwrap();
    let mut states = client.state();

    tv.join_room("Stardust Wayfarer", "7");
    let room = room_where(&mut states, |p| !p.messages.is_empty()).await;
    assert_eq!(room.title.as_deref(), Some("Stardust Wayfarer"));
    let names: Vec<&str> = room.members.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(names, ["Mika", "Ren", "Sol"]);
    assert!(room.messages.iter().any(|line| line.you && line.from == "Living room"));

    client
        .send(RemoteCommand::RoomChat { text: "pause at the end, tea break".into() })
        .await
        .unwrap();
    let room = room_where(&mut states, |p| {
        p.messages.last().and_then(|line| line.text.as_deref())
            == Some("pause at the end, tea break")
    })
    .await;
    assert!(room.messages.last().unwrap().you);

    let ren = room.members.iter().find(|m| m.name == "Ren").unwrap().id.clone();
    client.send(RemoteCommand::RoomMute { member: ren.clone(), muted: true }).await.unwrap();
    let room = room_where(&mut states, |p| p.muted == [ren.clone()]).await;
    assert!(room.messages.iter().all(|line| line.from != "Ren"));
}
