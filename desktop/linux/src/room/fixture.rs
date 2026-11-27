//! The board's frames as room views: Ada (you), Mika and Ren, episode 9 at 12:04, and every
//! state the board draws, built from the engine's records by hand. The words' tests read them,
//! and the Watch together place's shots; the player's room shots stage the engine's fake room
//! instead ([`super::scenes`]).

use kaeri_core::ids::AnilistId;

use super::model::{
    MemberState, MoveCandidate, PausedFor, RelayHealth, RoomCarriedOn, RoomConnection,
    RoomEpisode, RoomEpisodeInfo, RoomMember, RoomUpNext, RoomView, WaitReason,
};

/// The clock every staged view is read at.
pub const NOW: i64 = 1_800_000_000_000;

pub const RELAYS: [&str; 3] =
    ["wss://relay-one.example", "wss://relay-two.example", "wss://relay-three.example"];

fn member(name: &str, you: bool, state: MemberState) -> RoomMember {
    RoomMember {
        id: format!("mid-{}", name.to_lowercase()),
        name: name.into(),
        you,
        state,
        since_ms: None,
        behind_s: None,
        episode: None,
        paused_room: false,
        waited: false,
        background: false,
        muted: false,
    }
}

fn base() -> RoomView {
    RoomView {
        room_id: "room-k7m2xq9a".into(),
        title: "Natsukaze Relay".into(),
        episode: "9".into(),
        episode_title: Some("The Long Leg".into()),
        minutes: Some(24),
        name: "Ada".into(),
        connection: RoomConnection::Connected,
        relays: RELAYS
            .iter()
            .map(|u| RelayHealth { url: (*u).into(), healthy: true, rtt_ms: Some(40) })
            .collect(),
        members: vec![
            member("Ada", true, MemberState::Watching),
            member("Mika", false, MemberState::Watching),
            member("Ren", false, MemberState::Watching),
        ],
        playing: true,
        paused_by: None,
        position: 724.0,
        link: Some("https://kaeri.app/r#v1.Q2m4xR9dK7tWbH3nPzL8sYe".into()),
        ..RoomView::default()
    }
}

fn set(view: &mut RoomView, name: &str, state: MemberState, secs: Option<i64>) {
    if let Some(m) = view.members.iter_mut().find(|m| m.name == name) {
        m.state = state;
        m.since_ms = secs.map(|s| NOW - s * 1000);
    }
}

fn wait_for(view: &mut RoomView, name: &str, reason: WaitReason, secs: i64) {
    view.playing = false;
    let you = view.members.iter().any(|m| m.you && m.name == name);
    view.waiting.push(paused_for(name, you, reason, secs));
    view.can_carry_on = secs >= 20;
    let state = if reason == WaitReason::Buffering {
        MemberState::Buffering
    } else {
        MemberState::Loading
    };
    set(view, name, state, Some(secs));
}

fn paused_for(name: &str, you: bool, reason: WaitReason, secs: i64) -> PausedFor {
    PausedFor {
        id: format!("mid-{}", name.to_lowercase()),
        name: name.into(),
        you,
        reason,
        since_ms: NOW - secs * 1000,
    }
}

fn six(view: &mut RoomView) {
    for n in ["Sora", "Kenji", "Noa"] {
        view.members.push(member(n, false, MemberState::Watching));
    }
}

pub fn view(name: &str) -> RoomView {
    let mut v = base();
    match name {
        "paused" => {
            v.playing = false;
            v.paused_by = Some("Mika".into());
            for m in &mut v.members {
                m.state = MemberState::Paused;
                m.paused_room = m.name == "Mika";
            }
        }
        "wait" | "full-wait" => wait_for(&mut v, "Ren", WaitReason::Loading, 6),
        "cap" | "full-cap" => wait_for(&mut v, "Ren", WaitReason::Loading, 20),
        "carry" => {
            set(&mut v, "Ren", MemberState::Loading, Some(24));
            v.waiting.push(paused_for("Ren", false, WaitReason::Loading, 24));
            v.carried_on = Some(RoomCarriedOn {
                by: "Mika".into(),
                by_you: false,
                past: vec!["Ren".into()],
            });
        }
        "self-wait" => {
            v.name = "Ren".into();
            for m in &mut v.members {
                m.you = m.name == "Ren";
            }
            wait_for(&mut v, "Ren", WaitReason::Loading, 6);
            v.can_dont_wait = true;
        }
        "buffering" => wait_for(&mut v, "Ren", WaitReason::Buffering, 3),
        "buffering-six" => {
            six(&mut v);
            set(&mut v, "Ren", MemberState::Buffering, Some(3));
        }
        "choosing" => set(&mut v, "Ren", MemberState::Choosing, None),
        "nosource" => set(&mut v, "Ren", MemberState::NoSource, None),
        "closed" => set(&mut v, "Ren", MemberState::PlayerClosed, None),
        "reconnecting" => set(&mut v, "Ren", MemberState::Reconnecting, None),
        "left" => set(&mut v, "Ren", MemberState::Left, None),
        "behind" => {
            set(&mut v, "Ren", MemberState::Behind, None);
            v.members[2].behind_s = Some(4.0);
        }
        "other" => {
            set(&mut v, "Ren", MemberState::OtherEpisode, None);
            v.members[2].episode =
                Some(RoomEpisode { id: AnilistId::new(1), episode: "8".into() });
        }
        "self-choosing" => set(&mut v, "Ada", MemberState::Choosing, None),
        "self-nosource" => set(&mut v, "Ada", MemberState::NoSource, None),
        "self-behind" => {
            set(&mut v, "Ada", MemberState::Behind, None);
            v.members[0].behind_s = Some(4.0);
        }
        "self-conn" => {
            v.connection = RoomConnection::Reconnecting { relays_up: 0 };
            set(&mut v, "Ada", MemberState::Reconnecting, None);
            for r in &mut v.relays {
                r.healthy = false;
                r.rtt_ms = None;
            }
        }
        "relay-trouble" => {
            v.relays[1].healthy = false;
            v.relays[1].rtt_ms = None;
        }
        "upnext" => {
            v.position = 1396.0;
            v.up_next = Some(RoomUpNext {
                episode: RoomEpisodeInfo {
                    id: AnilistId::new(1),
                    episode: "10".into(),
                    title: "Natsukaze Relay".into(),
                    name: None,
                    banner_url: None,
                    cover_url: None,
                },
                ends_at_ms: Some(NOW + 8_000),
                waiting_for: vec![],
                cancelled_by: None,
            });
        }
        "six" => six(&mut v),
        "before-idle" => {
            v.before_play = true;
            v.playing = false;
            v.position = 0.0;
            set(&mut v, "Ren", MemberState::Loading, Some(4));
        }
        "before-wait" | "before-cap" => {
            let secs = if name == "before-cap" { 20 } else { 6 };
            v.before_play = true;
            v.playing = false;
            v.position = 0.0;
            v.waiting.push(paused_for("Ren", false, WaitReason::Loading, secs));
            v.can_carry_on = secs >= 20;
            set(&mut v, "Ren", MemberState::Loading, Some(secs));
        }
        "before-count" => {
            v.before_play = true;
            v.starts_at_ms = Some(NOW + 3_000);
            v.playing = false;
            v.position = 0.0;
        }
        "invited" | "invited-over" => {
            v.invited_by = Some("Mika".into());
            v.members.retain(|m| !m.you);
            if name == "invited-over" {
                v.over_cap = true;
                for n in ["Sora", "Kenji", "Noa", "Yui", "Taro", "Hana", "Kai", "Rio"] {
                    v.members.push(member(n, false, MemberState::Watching));
                }
            }
        }
        "move-start" => {
            v.name = "Mika".into();
            for m in &mut v.members {
                m.you = m.name == "Mika";
            }
            let mut taro = member("Taro", false, MemberState::Watching);
            taro.muted = true;
            v.members.push(taro);
            v.move_candidates = vec![
                MoveCandidate {
                    id: "mid-ada".into(),
                    name: "Ada".into(),
                    muted: false,
                    ticked: true,
                },
                MoveCandidate {
                    id: "mid-ren".into(),
                    name: "Ren".into(),
                    muted: false,
                    ticked: true,
                },
                MoveCandidate {
                    id: "mid-taro".into(),
                    name: "Taro".into(),
                    muted: true,
                    ticked: false,
                },
            ];
        }
        "move-offer" => {
            v.members = vec![
                member("Ada", true, MemberState::Watching),
                member("Ren", false, MemberState::Watching),
                member("Taro", false, MemberState::Watching),
                member("Mika", false, MemberState::Left),
            ];
        }
        "move-after" => {
            v.name = "Taro".into();
            v.members = vec![
                member("Taro", true, MemberState::Watching),
                member("Ada", false, MemberState::Left),
                member("Ren", false, MemberState::Left),
            ];
        }
        "ready-alone" => {
            v.members.truncate(1);
        }
        _ => {}
    }
    v
}

/// The place's shots: no room yet, in a room with the player closed, a link whose relays don't
/// answer, and a Rejoin nobody answered.
#[cfg(feature = "fixtures")]
pub const PLACE_SHOTS: [&str; 5] =
    ["room-place", "room-place-in", "room-join-fail", "room-recent-fail", "room-recent-look"];

#[cfg(feature = "fixtures")]
fn cover(id: u32) -> String {
    format!("{}cover/{id}.jpg", kaeri_testkit::fixtures::ART_HOST) // i18n-skip
}

#[cfg(feature = "fixtures")]
pub fn place(shot: &str) -> Option<super::place::Place> {
    use super::place::{Place, Recent};
    if !PLACE_SHOTS.contains(&shot) {
        return None;
    }
    let recent = vec![
        Recent {
            title: "The Library Above the Clouds".into(),
            line: "Episode 4 · with Mika and Ren · 19:25".into(),
            cover: Some(cover(900_002)),
            here: None,
            failed: shot == "room-recent-fail",
        },
        Recent {
            title: "Stardust Wayfarer".into(),
            line: "Episode 3 · with Mika · Tuesday".into(),
            cover: Some(cover(900_001)),
            here: (shot == "room-recent-look").then(|| super::copy::here(&["Mika"])),
            failed: false,
        },
        Recent {
            title: "The Steel Gardener".into(),
            line: "Episode 1 · 18 November".into(),
            cover: Some(cover(900_005)),
            here: (shot == "room-recent-look").then(|| super::copy::here(&[])),
            failed: false,
        },
    ];
    let room = (shot == "room-place-in").then(|| {
        let mut v = view("closed");
        v.title = "The Library Above the Clouds".into();
        v.episode = "4".into();
        set(&mut v, "Ada", MemberState::PlayerClosed, None);
        v
    });
    Some(Place {
        room,
        join_failed: (shot == "room-join-fail")
            .then(|| "kaeri.app/r#v1.Q2m4xR9dK7tWbH3nPzL8sYe".into()),
        recent,
    })
}
