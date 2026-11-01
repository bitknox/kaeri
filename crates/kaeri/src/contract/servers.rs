//! Media servers through the engine: the add flow, the libraries, matching and removal, against
//! the testkit's fake Jellyfin. No real server is involved.

use kaeri_model::{
    command::Command,
    event::Event,
    servers::{ServerCommand, ServerEvent, ServerFailure, ServerRow, ServerStep},
};
use kaeri_testkit::jellyfin::{self as fake, FakeJellyfin};

use crate::harness::Harness;

fn servers(updates: &[Event]) -> Vec<ServerEvent> {
    updates
        .iter()
        .filter_map(|u| match u {
            Event::Server(e) => Some(e.clone()),
            _ => None,
        })
        .collect()
}

fn rows(updates: &[Event]) -> Vec<ServerRow> {
    servers(updates)
        .into_iter()
        .rev()
        .find_map(|e| match e {
            ServerEvent::Servers(rows) => Some(rows),
            _ => None,
        })
        .expect("the saved servers")
}

async fn send(h: &mut Harness, command: ServerCommand) -> Vec<Event> {
    h.send(Command::Server(command));
    h.settle().await
}

async fn sign_in(h: &mut Harness, jellyfin: &FakeJellyfin) -> Vec<Event> {
    send(
        h,
        ServerCommand::SignIn {
            address: jellyfin.address(),
            user: fake::USER.into(),
            password: fake::PASSWORD.into(),
        },
    )
    .await
}

#[tokio::test]
async fn an_address_that_answers_is_checked() {
    let jellyfin = FakeJellyfin::start().await;
    let mut h = Harness::new().await;
    let updates = send(&mut h, ServerCommand::Check { address: jellyfin.address() }).await;
    let [ServerEvent::Checked(checked)] = &servers(&updates)[..] else {
        panic!("a check: {updates:?}");
    };
    assert_eq!(checked.name, fake::SERVER_NAME);
    assert_eq!(checked.id, fake::SERVER_ID);
    assert!(checked.quick_connect);
}

#[tokio::test]
async fn a_stranger_an_old_server_and_a_closed_port_fail_at_the_address() {
    let jellyfin = FakeJellyfin::start().await;
    let mut h = Harness::new().await;
    jellyfin.set_not_jellyfin();
    let updates = send(&mut h, ServerCommand::Check { address: jellyfin.address() }).await;
    assert_eq!(
        servers(&updates),
        [ServerEvent::Failed {
            step: ServerStep::Address,
            failure: ServerFailure::NotJellyfin
        }]
    );
    let jellyfin = FakeJellyfin::start().await;
    jellyfin.set_version("10.7.7");
    let updates = send(&mut h, ServerCommand::Check { address: jellyfin.address() }).await;
    assert_eq!(
        servers(&updates),
        [ServerEvent::Failed {
            step: ServerStep::Address,
            failure: ServerFailure::TooOld { version: "10.7.7".into() }
        }]
    );
    let updates = send(&mut h, ServerCommand::Check { address: "not an address".into() }).await;
    assert!(matches!(
        &servers(&updates)[..],
        [ServerEvent::Failed { failure: ServerFailure::BadAddress, .. }]
    ));
}

#[tokio::test]
async fn a_password_signs_in_and_the_server_is_kept_without_it() {
    let jellyfin = FakeJellyfin::start().await;
    let mut h = Harness::new().await;
    let updates = sign_in(&mut h, &jellyfin).await;
    assert!(
        servers(&updates).contains(&ServerEvent::SignedIn { server: fake::SERVER_ID.into() })
    );
    let saved = rows(&updates);
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].user, fake::USER);
    assert!(!saved[0].signed_out);
    assert!(saved[0].report, "reporting starts on");
    // Nothing the engine wrote under its data directory holds the password.
    fn walk(dir: &std::path::Path, found: &mut bool) {
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, found);
            } else if let Ok(bytes) = std::fs::read(&path) {
                *found |= String::from_utf8_lossy(&bytes).contains(fake::PASSWORD);
            }
        }
    }
    let mut found = false;
    walk(&h.paths().data_dir, &mut found);
    assert!(!found, "the password was written down");
}

#[tokio::test]
async fn a_wrong_password_is_named_and_saves_nothing() {
    let jellyfin = FakeJellyfin::start().await;
    let mut h = Harness::new().await;
    let updates = send(
        &mut h,
        ServerCommand::SignIn {
            address: jellyfin.address(),
            user: fake::USER.into(),
            password: "nope".into(),
        },
    )
    .await;
    assert_eq!(
        servers(&updates),
        [ServerEvent::Failed { step: ServerStep::SignIn, failure: ServerFailure::Credentials }]
    );
    let updates = send(&mut h, ServerCommand::Load).await;
    assert!(rows(&updates).is_empty());
}

#[tokio::test]
async fn quick_connect_shows_a_code_then_signs_in_once_approved() {
    let jellyfin = FakeJellyfin::start().await;
    let mut h = Harness::new().await;
    h.send(Command::Server(ServerCommand::StartQuickConnect { address: jellyfin.address() }));
    let updates = h
        .settle_until(|e| matches!(e, Event::Server(ServerEvent::QuickConnectCode { .. })))
        .await;
    assert!(
        servers(&updates)
            .contains(&ServerEvent::QuickConnectCode { code: fake::QUICK_CODE.into() })
    );
    jellyfin.approve_quick_connect();
    let updates =
        h.settle_until(|e| matches!(e, Event::Server(ServerEvent::SignedIn { .. }))).await;
    assert!(
        servers(&updates).contains(&ServerEvent::SignedIn { server: fake::SERVER_ID.into() })
    );
}

#[tokio::test]
async fn quick_connect_off_is_named() {
    let jellyfin = FakeJellyfin::start().await;
    jellyfin.set_quick_connect(false);
    let mut h = Harness::new().await;
    h.send(Command::Server(ServerCommand::StartQuickConnect { address: jellyfin.address() }));
    let updates =
        h.settle_until(|e| matches!(e, Event::Server(ServerEvent::Failed { .. }))).await;
    assert!(servers(&updates).contains(&ServerEvent::Failed {
        step: ServerStep::QuickConnect,
        failure: ServerFailure::QuickConnectOff
    }));
}

#[tokio::test]
async fn the_anime_library_starts_ticked_and_the_films_do_not() {
    let jellyfin = FakeJellyfin::start().await;
    let mut h = Harness::new().await;
    sign_in(&mut h, &jellyfin).await;
    let updates =
        send(&mut h, ServerCommand::LoadLibraries { server: fake::SERVER_ID.into() }).await;
    let libraries = servers(&updates)
        .into_iter()
        .find_map(|e| match e {
            ServerEvent::Libraries { libraries, .. } => Some(libraries),
            _ => None,
        })
        .expect("libraries");
    let ticked: Vec<_> = libraries.iter().map(|l| (l.name.as_str(), l.ticked)).collect();
    assert_eq!(ticked, [("Anime Shows", true), ("Films", false)]);
}

#[tokio::test]
async fn choosing_a_library_matches_its_series_by_the_ids_they_carry() {
    let jellyfin = FakeJellyfin::start().await;
    let mut h = Harness::new().await;
    sign_in(&mut h, &jellyfin).await;
    h.send(Command::Server(ServerCommand::SetLibraries {
        server: fake::SERVER_ID.into(),
        libraries: vec!["lib-anime".into()],
    }));
    // The list is said once the choice is kept, and again when the matching is done.
    let updates = h
        .settle_until(
            |e| matches!(e, Event::Server(ServerEvent::Servers(rows)) if rows[0].series > 0),
        )
        .await;
    let saved = rows(&updates);
    assert_eq!(saved[0].series, 3);
    // The series with an AniList id is matched. The AniDB one has no mapping here, and the
    // untagged one has no title like it in AniList's fake, so both are left alone.
    assert_eq!(saved[0].matched, 1);
    assert!(saved[0].libraries.iter().any(|l| l.id == "lib-anime" && l.ticked));
}

#[tokio::test]
async fn reporting_can_be_turned_off_and_a_server_removed() {
    let jellyfin = FakeJellyfin::start().await;
    let mut h = Harness::new().await;
    sign_in(&mut h, &jellyfin).await;
    let updates = send(
        &mut h,
        ServerCommand::SetReport { server: fake::SERVER_ID.into(), report: false },
    )
    .await;
    assert!(!rows(&updates)[0].report);
    let updates = send(&mut h, ServerCommand::Remove { server: fake::SERVER_ID.into() }).await;
    assert!(rows(&updates).is_empty());
}

fn asked(jellyfin: &FakeJellyfin, path: &str) -> usize {
    jellyfin.requests().iter().filter(|r| r.path == path).count()
}

#[tokio::test]
async fn signing_out_keeps_the_row_and_ends_the_sign_in() {
    let jellyfin = FakeJellyfin::start().await;
    let mut h = Harness::new().await;
    sign_in(&mut h, &jellyfin).await;
    let updates = send(&mut h, ServerCommand::SignOut { server: fake::SERVER_ID.into() }).await;
    let saved = rows(&updates);
    assert_eq!(saved.len(), 1, "the row stays");
    assert!(saved[0].signed_out && saved[0].signed_out_here);
    assert_eq!(asked(&jellyfin, "/Sessions/Logout"), 1, "ended on the server too");
    // With no token kept, nothing more can be asked of the server.
    let updates =
        send(&mut h, ServerCommand::LoadLibraries { server: fake::SERVER_ID.into() }).await;
    assert!(servers(&updates).contains(&ServerEvent::Failed {
        step: ServerStep::Libraries,
        failure: ServerFailure::SignedOut
    }));
    let updates = sign_in(&mut h, &jellyfin).await;
    let saved = rows(&updates);
    assert!(!saved[0].signed_out && !saved[0].signed_out_here, "signed in again");
}

#[tokio::test]
async fn undo_brings_a_removed_server_back_without_a_password() {
    let jellyfin = FakeJellyfin::start().await;
    let mut h = Harness::new().await;
    sign_in(&mut h, &jellyfin).await;
    let updates = send(&mut h, ServerCommand::Remove { server: fake::SERVER_ID.into() }).await;
    assert!(rows(&updates).is_empty());
    let updates = send(&mut h, ServerCommand::Restore { server: fake::SERVER_ID.into() }).await;
    let saved = rows(&updates);
    assert_eq!(saved.len(), 1);
    assert!(!saved[0].signed_out);
    // Still signed in: the token was held, and no sign-in was asked for again.
    let updates =
        send(&mut h, ServerCommand::LoadLibraries { server: fake::SERVER_ID.into() }).await;
    assert!(servers(&updates).iter().any(|e| matches!(e, ServerEvent::Libraries { .. })));
    assert_eq!(asked(&jellyfin, "/Users/AuthenticateByName"), 1);
}

#[tokio::test]
async fn the_row_counts_the_series_as_they_are_matched() {
    let jellyfin = FakeJellyfin::start().await;
    let mut h = Harness::new().await;
    sign_in(&mut h, &jellyfin).await;
    h.send(Command::Server(ServerCommand::SetLibraries {
        server: fake::SERVER_ID.into(),
        libraries: vec!["lib-anime".into()],
    }));
    let updates = h
        .settle_until(
            |e| matches!(e, Event::Server(ServerEvent::Servers(rows)) if rows[0].series > 0),
        )
        .await;
    let counted: Vec<(u32, u32)> = servers(&updates)
        .into_iter()
        .filter_map(|e| match e {
            ServerEvent::Servers(rows) => rows[0].matching.clone().map(|m| (m.done, m.total)),
            _ => None,
        })
        .collect();
    assert!(counted.contains(&(0, 3)) && counted.contains(&(2, 3)), "{counted:?}");
    assert_eq!(rows(&updates)[0].matching, None, "done");
}

#[tokio::test]
async fn an_allowed_certificate_is_checked_again() {
    // The fake speaks plain HTTP, so this covers the call and the check after it; the
    // fingerprint pinning itself is tested in the Jellyfin crate.
    let jellyfin = FakeJellyfin::start().await;
    let mut h = Harness::new().await;
    let updates = send(
        &mut h,
        ServerCommand::TrustCertificate {
            address: jellyfin.address(),
            fingerprint: "ab:12:cd:34".into(),
        },
    )
    .await;
    assert!(matches!(&servers(&updates)[..], [ServerEvent::Checked(_)]), "{updates:?}");
}

#[tokio::test]
async fn a_server_that_stops_answering_says_when_it_was_last_seen_and_keeps_it() {
    let jellyfin = FakeJellyfin::start().await;
    let mut h = Harness::new().await;
    let before = kaeri_engine::now_epoch();
    let saved = rows(&sign_in(&mut h, &jellyfin).await);
    let seen = saved[0].last_seen.expect("seen as it signed in");
    assert!(seen >= before && !saved[0].not_answering);

    // Loading the servers asks each one: this one answers.
    let updates = send(&mut h, ServerCommand::Load).await;
    assert!(!rows(&updates)[0].not_answering);
    assert!(asked(&jellyfin, "/System/Info/Public") >= 2, "asked again on load");

    // Switched off: the next load finds it not answering, and when it was last seen stays.
    drop(jellyfin);
    h.send(Command::Server(ServerCommand::Load));
    let updates = h
        .settle_until(|e| {
            matches!(e, Event::Server(ServerEvent::Servers(rows))
                if rows.first().is_some_and(|r| r.not_answering))
        })
        .await;
    let row = &rows(&updates)[0];
    assert!(row.last_seen >= Some(seen), "{row:?}");
    // Kept with the server: what the engine reads when it starts again.
    let kept = kaeri_engine::servers::saved(h.store());
    assert!(kept[0].not_answering);
    assert_eq!(kept[0].last_seen, row.last_seen);
}
