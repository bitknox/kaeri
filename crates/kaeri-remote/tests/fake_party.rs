//! The fake TV in a watch party, as the Fixtures app's remote screen shows it: a phone hears the
//! conversation, and a line it sends as the TV comes back as the TV's own.
#![cfg(feature = "testing")]

use std::time::Duration;

use kaeri_remote::{
    client::{Hello, RemoteClient},
    command::RemoteCommand,
    fake::FakeTv,
    identity::Identity,
    state::{PartyInfo, RemoteState},
};
use tokio::sync::watch;

const WAIT: Duration = Duration::from_secs(5);

/// The first party state that `ok` accepts.
async fn party_where(
    states: &mut watch::Receiver<Option<RemoteState>>,
    ok: impl Fn(&PartyInfo) -> bool,
) -> PartyInfo {
    tokio::time::timeout(WAIT, async {
        loop {
            if let Some(party) =
                states.borrow_and_update().as_ref().and_then(|s| s.party.as_deref())
                && ok(party)
            {
                return party.clone();
            }
            states.changed().await.unwrap();
        }
    })
    .await
    .expect("the party state arrives")
}

#[tokio::test]
async fn a_phone_hears_the_tvs_party_and_its_line_comes_back_as_the_tvs() {
    let tv = FakeTv::start("Living room", Vec::new()).await.unwrap();
    let phone = Identity::generate().unwrap();
    let hello = Hello { name: "Hall phone".into(), app: "test/0".into() };
    let mut client = RemoteClient::connect(tv.addr(), &phone, None, &hello).await.unwrap();
    client.pair(&tv.open_code()).await.unwrap();
    let mut states = client.state();

    tv.join_party("quiet-harbor", "Stardust Wayfarer", "7");
    let party = party_where(&mut states, |p| !p.messages.is_empty()).await;
    assert_eq!(party.room, "quiet-harbor");
    assert_eq!(party.members, ["Mika", "Ren", "Sol"]);
    assert!(party.messages.iter().any(|line| line.you && line.from == "Living room"));

    client
        .send(RemoteCommand::PartyChat { text: "pause at the end, tea break".into() })
        .await
        .unwrap();
    let party = party_where(&mut states, |p| {
        p.messages.last().and_then(|line| line.text.as_deref())
            == Some("pause at the end, tea break")
    })
    .await;
    assert!(party.messages.last().unwrap().you);

    client.send(RemoteCommand::PartyMute { member: "Ren".into(), muted: true }).await.unwrap();
    let party = party_where(&mut states, |p| p.muted == ["Ren"]).await;
    assert!(party.messages.iter().all(|line| line.from != "Ren"));
}
