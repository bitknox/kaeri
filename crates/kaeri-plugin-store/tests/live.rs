//! A whole store journey against a store that is already running, wherever it runs: the reference
//! server, `wrangler dev`, a function on a laptop, a real deployment.
//!
//! Runs only when told where:
//!
//! ```sh
//! KAERI_TEST_STORE=http://127.0.0.1:8787 KAERI_TEST_SETUP_CODE=… \
//!   cargo test -p kaeri-plugin-store --test live -- --nocapture
//! ```
//!
//! The store must be fresh — nobody owns it yet — and deployed with that setup code, because the
//! journey starts by claiming it. Along the way it opens the store to anyone with its link, has a
//! device join that way, and closes it again.
//!
//! Plugins are published as stamped stand-ins (a WebAssembly component carrying a manifest and
//! padding), which a store that cannot run plugins accepts; a store that can run them would refuse
//! the stand-in, so this is for serverless stores. Set `KAERI_TEST_PLUGIN` to a real stamped
//! plugin to use against the reference server.

use kaeri_plugin_store::{
    ClientError, Invite, PluginInfo, StoreClient, manifest,
    protocol::{
        Claimed, EndReason, InviteLasts, InviteRequest, InviteUses, NO_END_DATE, PART_SIZE,
        RedeemResponse, Role,
    },
};

fn client() -> StoreClient {
    StoreClient::new(std::time::Duration::from_secs(30)).unwrap()
}

/// A component that is nothing but a manifest and enough padding to need three parts.
fn stand_in(id: &str, version: &str) -> Vec<u8> {
    let mut wasm = b"\0asm\x0d\x00\x01\x00".to_vec();
    let padding = vec![b'x'; PART_SIZE as usize * 2 + 4_321];
    let name = b"padding";
    let mut body = vec![name.len() as u8];
    body.extend_from_slice(name);
    body.extend_from_slice(&padding);
    wasm.push(0);
    let mut size = body.len() as u32;
    loop {
        let byte = (size & 0x7f) as u8;
        size >>= 7;
        if size == 0 {
            wasm.push(byte);
            break;
        }
        wasm.push(byte | 0x80);
    }
    wasm.extend_from_slice(&body);
    let info = PluginInfo {
        id: id.into(),
        name: format!("{id} stand-in"),
        version: version.into(),
        allowed_hosts: vec!["api.example".into()],
        translations: vec!["sub".into()],
        kind: None,
        media: Vec::new(),
    };
    manifest::stamp(&wasm, &info).unwrap()
}

#[tokio::test]
async fn a_whole_journey_against_a_running_store() {
    let (Ok(base), Ok(code)) =
        (std::env::var("KAERI_TEST_STORE"), std::env::var("KAERI_TEST_SETUP_CODE"))
    else {
        eprintln!("skipping: set KAERI_TEST_STORE and KAERI_TEST_SETUP_CODE to run this");
        return;
    };
    let base = base.trim_end_matches('/').to_owned();
    let http = reqwest::Client::new();
    let client = client();

    let info = client.info(&base).await.unwrap();
    eprintln!("store: {} (protocol {})", info.name, info.protocol);
    let health = http.get(format!("{base}/healthz")).send().await.unwrap();
    assert_eq!(health.status(), 200, "health check");

    // Claim it with the setup code; the answer is the owner invite.
    let wrong = http
        .post(format!("{base}/v1/setup"))
        .json(&serde_json::json!({ "code": "not the code" }))
        .send()
        .await
        .unwrap();
    assert_eq!(wrong.status(), 403, "a wrong setup code is refused");
    let claimed: Claimed = http
        .post(format!("{base}/v1/setup"))
        .json(&serde_json::json!({ "code": code, "contact": "owner@store.example" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .expect("claiming: is the store fresh, and deployed with this setup code?");
    let owner_invite = Invite::parse(&claimed.link).unwrap();
    assert_eq!(client.preview(&owner_invite).await.unwrap().name, info.name);
    let owner = client.redeem(&owner_invite, "live test").await.unwrap();
    assert_eq!(owner.member.role, Role::Owner);
    let key = owner_invite.key;

    // Publish in parts, then read it back in ranges.
    let wasm = match std::env::var("KAERI_TEST_PLUGIN") {
        Ok(path) => std::fs::read(path).unwrap(),
        Err(_) => stand_in("live-test", "1.0.0"),
    };
    let entry = client.publish(&base, &owner.token, wasm.clone()).await.unwrap();
    eprintln!("published {} {} ({} bytes)", entry.id, entry.version, entry.size);
    let index = client.index(&base, &owner.token, &key, 0).await.unwrap();
    assert!(index.sequence >= 1);
    let listed = index.plugin(&entry.id).unwrap();
    let bytes = client.download(&base, &owner.token, listed).await.unwrap();
    assert_eq!(bytes, wasm, "the download is what was published");

    // Someone new joins, sees it, and is removed.
    let invite = client
        .create_invite(&base, &owner.token, &InviteRequest::member("Live member", Role::Member))
        .await
        .unwrap();
    let member =
        client.redeem(&Invite::parse(&invite.link).unwrap(), "elsewhere").await.unwrap();
    let seen = client.index(&base, &member.token, &key, index.sequence).await.unwrap();
    assert!(seen.plugin(&entry.id).is_some());
    let again =
        client.redeem(&Invite::parse(&invite.link).unwrap(), "twice").await.unwrap_err();
    assert_eq!(again.code(), Some("invite_used_up"));

    let members = client.members(&base, &owner.token).await.unwrap();
    assert_eq!(members.members.len(), 2);
    client.remove_member(&base, &owner.token, &member.member.id).await.unwrap();
    assert_eq!(
        client.index(&base, &member.token, &key, 0).await.unwrap_err(),
        ClientError::Unauthorized
    );

    // Open to anyone with its link: a device joins without a name, through the same signed
    // list and downloads, and an older Kaeri given the link is told to update.
    let opened = client.set_open(&base, &owner.token, true).await.unwrap();
    let open_link =
        Invite::parse(opened.link.as_deref().expect("an open store's link")).unwrap();
    assert!(open_link.open && open_link.key == key);
    let preview = client.preview_index(&open_link).await.unwrap().expect("a preview");
    assert!(preview.open && preview.plugin(&entry.id).is_some());
    let anonymous = client.redeem(&open_link, "").await.unwrap();
    assert_eq!(anonymous.member.name, "");
    let listed = client.index(&base, &anonymous.token, &key, 0).await.unwrap();
    let fetched = client
        .download(&base, &anonymous.token, listed.plugin(&entry.id).unwrap())
        .await
        .unwrap();
    assert_eq!(fetched, wasm);
    let as_invite = Invite { open: false, ..open_link.clone() };
    let old = client.redeem(&as_invite, "older Kaeri").await.unwrap_err();
    assert_eq!(old.code(), Some("open_link"));
    assert_eq!(client.open_state(&base, &owner.token).await.unwrap().devices, 1);
    assert_eq!(client.members(&base, &owner.token).await.unwrap().members.len(), 1);

    // A second device joins and is revoked on its own: it stops, and the first does not.
    let second = client.redeem(&open_link, "").await.unwrap();
    let device = client.me(&base, &second.token).await.unwrap().device.id;
    client.revoke_device(&base, &owner.token, &device).await.unwrap();
    assert_eq!(client.me(&base, &second.token).await.unwrap_err(), ClientError::Unauthorized);
    assert!(client.me(&base, &anonymous.token).await.is_ok());
    assert_eq!(client.open_state(&base, &owner.token).await.unwrap().devices, 1);

    // Invite-only again: the link stops, the device stays until the owner removes it.
    let closed = client.set_open(&base, &owner.token, false).await.unwrap();
    assert!(!closed.open && closed.link.is_none());
    assert_eq!(client.redeem(&open_link, "").await.unwrap_err().code(), Some("open_unusable"));
    assert!(client.me(&base, &anonymous.token).await.is_ok());
    let cleared = client.remove_anonymous(&base, &owner.token).await.unwrap();
    assert_eq!(cleared.devices, 0);
    assert_eq!(
        client.me(&base, &anonymous.token).await.unwrap_err(),
        ClientError::Unauthorized
    );

    // The admin page: a sign-in link, a session cookie, and the overview.
    let link = client.admin_link(&base, &owner.token).await.unwrap();
    let login = link.link.rsplit("#login=").next().unwrap().to_owned();
    let origin = url::Url::parse(&base).unwrap().origin().ascii_serialization();
    let session = http
        .post(format!("{base}/v1/admin/session"))
        .header("origin", &origin)
        .header("x-kaeri-admin", "1")
        .json(&serde_json::json!({ "code": login }))
        .send()
        .await
        .unwrap();
    assert_eq!(session.status(), 200);
    let cookie =
        session.headers()["set-cookie"].to_str().unwrap().split(';').next().unwrap().to_owned();
    let overview = http
        .get(format!("{base}/v1/admin/overview"))
        .header("cookie", &cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(overview.status(), 200);
    let page = http.get(format!("{base}/admin/")).send().await.unwrap();
    assert_eq!(page.status(), 200);
    assert!(
        page.headers()["content-security-policy"]
            .to_str()
            .unwrap()
            .contains("script-src 'self'")
    );

    // An invite for a group: five uses, no end date, each joiner a member of their own.
    let group = InviteRequest::Member {
        name: "Live group".into(),
        role: Role::Member,
        uses: InviteUses::Five,
        lasts: InviteLasts::Forever,
    };
    let made = client.create_invite(&base, &owner.token, &group).await.unwrap();
    assert_eq!((made.max_uses, made.expires_at), (5, NO_END_DATE));
    let group_link = Invite::parse(&made.link).unwrap();
    let (_, facts) = client.preview_invite(&group_link).await.unwrap().expect("a preview");
    assert_eq!(facts.map(|f| f.max_uses), Some(5));
    let named = client.redeem_as(&group_link, "first", Some("Live joiner")).await.unwrap();
    assert_eq!(named.member.name, "Live joiner");
    // As a client from before the joiner's name redeems: no name, so the store numbers it.
    let older: RedeemResponse = http
        .post(format!("{base}/v1/redeem"))
        .json(&serde_json::json!({ "invite": group_link.code, "device": "older" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(older.member.name, "Live group 2");
    client.redeem(&group_link, "third").await.unwrap();
    client.redeem(&group_link, "fourth").await.unwrap();
    // The last use, raced by two devices: the claim is one `UPDATE … RETURNING`, so one wins.
    let (a, b) = tokio::join!(
        client.redeem_as(&group_link, "racing one", Some("One")),
        client.redeem_as(&group_link, "racing two", Some("Two")),
    );
    assert_eq!(a.is_ok() as u8 + b.is_ok() as u8, 1, "{a:?} {b:?}");
    let lost = if a.is_ok() { b } else { a }.unwrap_err();
    assert_eq!(lost.code(), Some("invite_used_up"));
    let list = client.members(&base, &owner.token).await.unwrap();
    let ended = list.ended_invites.iter().find(|i| i.id == made.id).expect("listed as ended");
    assert_eq!((ended.uses, ended.joined.len()), (5, 5));
    assert_eq!(ended.ended.map(|e| e.reason), Some(EndReason::UsedUp));
    for joined in &ended.joined {
        client.remove_member(&base, &owner.token, &joined.member).await.unwrap();
    }

    // Any number of times needs a contact for the store, as its own link does.
    let any = InviteRequest::Member {
        name: "Live anyone".into(),
        role: Role::Member,
        uses: InviteUses::Any,
        lasts: InviteLasts::Day,
    };
    let store_info: serde_json::Value =
        http.get(format!("{base}/v1/store")).send().await.unwrap().json().await.unwrap();
    let made = client.create_invite(&base, &owner.token, &any).await;
    if store_info.get("contact").is_some() {
        let made = made.unwrap();
        assert_eq!(made.max_uses, 0);
        client.cancel_invite(&base, &owner.token, &made.id).await.unwrap();
    } else {
        assert_eq!(made.unwrap_err().code(), Some("contact_needed"));
    }

    // Leave it tidy for another run's eyes: withdraw the plugin.
    client.unpublish(&base, &owner.token, &entry.id).await.unwrap();
    eprintln!("journey complete");
}
