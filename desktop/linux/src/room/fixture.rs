//! The board's frames as room views: Ada (you), Mika and Ren, episode 9 at 12:04, and every
//! state the board draws. Stand-ins for the engine's fake room scenes until its FFI lands; the
//! tests and the room shots both read them.

use super::model::{
    Before, MemberState, MoveCandidate, PausedFor, RelayHealth, RoomConnection, RoomMember,
    RoomUpNext, RoomView, WaitReason,
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
        waiting: vec![],
        can_carry_on: false,
        can_dont_wait: false,
        carried_on_by: None,
        up_next: None,
        link: Some("https://kaeri.app/r#v1.Q2m4xR9dK7tWbH3nPzL8sYe".into()),
        move_candidates: vec![],
        before: None,
        invited_by: None,
        over_cap: false,
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
    view.waiting.push(PausedFor { name: name.into(), reason, since_ms: NOW - secs * 1000 });
    view.can_carry_on = secs >= 20;
    let state = if reason == WaitReason::Buffering {
        MemberState::Buffering
    } else {
        MemberState::Loading
    };
    set(view, name, state, Some(secs));
}

fn six(view: &mut RoomView) {
    for n in ["Sora", "Kenji", "Noa"] {
        view.members.push(member(n, false, MemberState::Watching));
    }
}

/// Every staged view by name, in the board's order.
pub const ALL: [&str; 34] = [
    "watching",
    "paused",
    "wait",
    "cap",
    "carry",
    "self-wait",
    "buffering",
    "buffering-six",
    "choosing",
    "nosource",
    "closed",
    "reconnecting",
    "behind",
    "other",
    "self-choosing",
    "self-nosource",
    "self-behind",
    "self-conn",
    "relay-trouble",
    "upnext",
    "six",
    "before-idle",
    "before-wait",
    "before-cap",
    "before-count",
    "invited",
    "invited-over",
    "move-start",
    "move-offer",
    "move-after",
    "left",
    "ready-alone",
    "full-wait",
    "full-cap",
];

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
            v.waiting.push(PausedFor {
                name: "Ren".into(),
                reason: WaitReason::Loading,
                since_ms: NOW - 24_000,
            });
            v.carried_on_by = Some("Mika".into());
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
            v.members[2].episode = Some("8".into());
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
                episode: "10".into(),
                ends_at_ms: Some(NOW + 8_000),
                waiting_for: vec![],
                cancelled_by: None,
            });
        }
        "six" => six(&mut v),
        "before-idle" => {
            v.before = Some(Before::Idle);
            v.playing = false;
            v.position = 0.0;
            set(&mut v, "Ren", MemberState::Loading, Some(4));
        }
        "before-wait" | "before-cap" => {
            let secs = if name == "before-cap" { 20 } else { 6 };
            v.before = Some(Before::Waiting);
            v.playing = false;
            v.position = 0.0;
            v.waiting.push(PausedFor {
                name: "Ren".into(),
                reason: WaitReason::Loading,
                since_ms: NOW - secs * 1000,
            });
            v.can_carry_on = secs >= 20;
            set(&mut v, "Ren", MemberState::Loading, Some(secs));
        }
        "before-count" => {
            v.before = Some(Before::Count(3));
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

/// The conversation each staged view shows under the people.
pub fn conversation(name: &str) -> Vec<super::model::ConvLine> {
    use super::model::ConvLine::{Said, System};
    let said =
        |who: &str, text: &str| Said { who: who.into(), text: text.into(), you: who == "Ada" };
    let sys = |text: &str| System(text.into());
    if name.starts_with("before") {
        return vec![
            sys("Mika started the room · Episode 9"),
            said("Mika", "everyone here?"),
            said("Ren", "two minutes, making tea"),
            said("Ada", "ready when you are"),
        ];
    }
    if name.starts_with("invited") {
        return vec![sys("The chat opens when you join")];
    }
    if name == "move-after" {
        return vec![
            sys("Mika started the room · Episode 9"),
            sys("Mika moved to a new room"),
            sys("Ada went to the new room"),
            sys("Ren went to the new room"),
        ];
    }
    let mut lines = vec![
        sys("Mika started the room · Episode 9"),
        said("Mika", "everyone here?"),
        said("Ren", "yes, go"),
        sys("Ren paused at 3:10"),
        said("Ren", "back in two, kettle"),
        sys("Ren resumed"),
        said("Ada", "the relay handoff at the bridge"),
        said("Mika", "every week this song"),
    ];
    match name {
        "move-offer" => {
            lines.truncate(5);
            lines.push(sys("Mika moved to a new room"));
        }
        "paused" => lines.push(sys("Mika paused at 12:04")),
        "wait" | "cap" | "full-wait" | "full-cap" => {
            lines.push(sys("Paused for Ren · loading"))
        }
        "buffering" => lines.push(sys("Paused for Ren · buffering")),
        "self-wait" => lines.push(sys("Paused for you · loading")),
        "carry" => {
            lines.push(sys("Paused for Ren · loading"));
            lines.push(sys("Mika carried on without Ren"));
        }
        "upnext" => lines.push(sys("Up next: episode 10 for everyone")),
        "skip" => lines.push(sys("Mika skipped the opening")),
        _ => {}
    }
    lines
}

/// The room's shots, one per board frame on Linux.
#[cfg(feature = "fixtures")]
pub const SHOTS: [&str; 47] = [
    "room-morph",
    "room-watching",
    "room-paused",
    "room-wait",
    "room-cap",
    "room-carry",
    "room-self-wait",
    "room-buffering",
    "room-buffering-six",
    "room-choosing",
    "room-nosource",
    "room-closed",
    "room-reconnecting",
    "room-behind",
    "room-other",
    "room-left",
    "room-self-choosing",
    "room-self-nosource",
    "room-self-behind",
    "room-self-conn",
    "room-relay-trouble",
    "room-upnext",
    "room-six",
    "room-before-idle",
    "room-before-wait",
    "room-before-cap",
    "room-before-count",
    "room-invited",
    "room-invited-over",
    "room-invite",
    "room-details",
    "room-details-trouble",
    "room-edit-relays",
    "room-leave",
    "room-close",
    "room-move-start",
    "room-move-pick",
    "room-move-offer",
    "room-move-after",
    "room-alone",
    "room-narrow",
    "room-toggle-off",
    "room-full-watching",
    "room-full-wait",
    "room-full-cap",
    "room-full-choosing",
    "room-full-controls",
];

/// A shot's view, what it opens over the room, and whether it is full screen.
#[cfg(feature = "fixtures")]
pub fn frame(shot: &str) -> Option<(&'static str, Option<&'static str>, bool)> {
    let frame = shot.strip_prefix("room-")?;
    Some(match frame {
        "invite" => ("watching", Some("invite"), false),
        "details" => ("watching", Some("details"), false),
        "details-trouble" => ("relay-trouble", Some("details"), false),
        "edit-relays" => ("watching", Some("edit-relays"), false),
        "leave" => ("watching", Some("leave"), false),
        "close" => ("watching", Some("close"), false),
        "move-start" => ("move-start", Some("invite"), false),
        "move-pick" => ("move-start", Some("move"), false),
        "full-watching" => ("watching", None, true),
        "full-wait" => ("full-wait", None, true),
        "full-cap" => ("full-cap", None, true),
        "full-choosing" => ("choosing", None, true),
        "full-controls" => ("watching", None, true),
        "alone" => ("ready-alone", None, false),
        "narrow" => ("six", None, false),
        "toggle-off" => ("watching", Some("toggle-off"), false),
        "morph" => ("choosing", None, false),
        other => (ALL.iter().find(|n| **n == other).copied()?, None, false),
    })
}

/// The staged view for the player's own fixture episode: The Library Above the Clouds,
/// episode 4, so the column and the header agree.
#[cfg(feature = "fixtures")]
pub fn for_player(name: &str) -> (RoomView, Vec<super::model::ConvLine>, super::words::Moves) {
    use super::model::ConvLine;
    let mut v = view(name);
    v.title = "The Library Above the Clouds".into();
    v.episode = "4".into();
    v.episode_title = None;
    for m in &mut v.members {
        if m.episode.is_some() {
            m.episode = Some("3".into());
        }
    }
    if let Some(up) = &mut v.up_next {
        up.episode = "5".into();
    }
    let conv = conversation(name)
        .into_iter()
        .map(|l| match l {
            ConvLine::System(t) => ConvLine::System(
                t.replace("Episode 9", "Episode 4").replace("episode 10", "episode 5"),
            ),
            other => other,
        })
        .collect();
    let mut moves = super::words::Moves::default();
    match name {
        "move-offer" => {
            moves.offer_by = Some("Mika".into());
            moves.offer_others = vec!["Ren".into()];
        }
        "move-after" => {
            moves.moved_by = Some("Mika".into());
            moves.went = vec!["Ada".into(), "Ren".into()];
        }
        _ => {}
    }
    (v, conv, moves)
}

/// The place's shots: no room yet, in a room with the player closed, a link whose relays don't
/// answer, and a Rejoin nobody answered.
#[cfg(feature = "fixtures")]
pub const PLACE_SHOTS: [&str; 4] =
    ["room-place", "room-place-in", "room-join-fail", "room-recent-fail"];

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
            failed: shot == "room-recent-fail",
        },
        Recent {
            title: "Stardust Wayfarer".into(),
            line: "Episode 3 · with Mika · Tuesday".into(),
            failed: false,
        },
        Recent {
            title: "The Steel Gardener".into(),
            line: "Episode 1 · 18 November".into(),
            failed: false,
        },
    ];
    let room = (shot == "room-place-in").then(|| {
        let mut v = for_player("closed").0;
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
