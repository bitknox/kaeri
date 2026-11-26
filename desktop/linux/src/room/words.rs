//! What the room says, worked out from the view alone: the room's line and its second line,
//! the button under them, and each person's word and ring. Pure, so it is tested here; the
//! widgets only draw it.

use super::{
    copy as c,
    model::{Before, MemberState, RoomConnection, RoomMember, RoomView, WaitReason},
};

/// Carry on comes forward at this many seconds.
pub const CAP_SECS: i64 = 20;

/// Seconds since `since_ms`, counted on this device's clock; never below 0.
pub fn secs_since(since_ms: i64, now_ms: i64) -> i64 {
    ((now_ms - since_ms) / 1000).max(0)
}

/// "12:04".
pub fn mmss(secs: f64) -> String {
    let s = secs.max(0.0).round() as i64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// Names joined as the board joins them: "Ren", "Ren and Mika", "3 people".
pub fn join_names(names: &[&str]) -> String {
    match names {
        [] => String::new(),
        [one] => (*one).to_owned(),
        [a, b] => c::names_two(a, b),
        more => c::names_many(more.len()),
    }
}

/// The button under the room's line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineButtons {
    None,
    /// Carry on without … (during play) or Start without … (before play): the one amber.
    CarryOn(String),
    /// Don't wait for me, on the loader's own device.
    DontWait,
    /// Join the room (or Join anyway) and Not now.
    Join {
        anyway: bool,
    },
    /// Go along and Stay here.
    Move,
}

/// The room's line, its second line, and what sits under them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub title: String,
    pub sub: String,
    pub buttons: LineButtons,
    /// The line holds the room (a wait): the strip stays up in full screen.
    pub holds: bool,
}

/// Extra context the view doesn't carry, from this device: a move offer, or the old room after
/// a move.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Moves {
    pub offer_by: Option<String>,
    pub offer_others: Vec<String>,
    pub moved_by: Option<String>,
    pub went: Vec<String>,
}

pub fn line(view: &RoomView, moves: &Moves, now_ms: i64) -> Line {
    let plain = |title: String, sub: String| Line {
        title,
        sub,
        buttons: LineButtons::None,
        holds: false,
    };
    let at = mmss(view.position_at(now_ms));
    if let Some(by) = &moves.offer_by {
        let others: Vec<&str> = moves.offer_others.iter().map(String::as_str).collect();
        return Line {
            title: c::line_moved(by),
            sub: c::line_go_along(&join_names(&others), others.len(), &at),
            buttons: LineButtons::Move,
            holds: true,
        };
    }
    if let Some(by) = &moves.moved_by {
        let went: Vec<&str> = moves.went.iter().map(String::as_str).collect();
        return plain(c::line_moved_away(by), c::line_went_along(&join_names(&went)));
    }
    if let Some(by) = &view.invited_by {
        let sub = if view.over_cap {
            c::line_over_cap(view.members.len())
        } else {
            c::line_invited_detail(&view.title, &view.episode)
        };
        return Line {
            title: c::line_invited(by),
            sub,
            buttons: LineButtons::Join { anyway: view.over_cap },
            holds: false,
        };
    }
    let you = view.you();
    if matches!(view.connection, RoomConnection::Reconnecting { .. }) {
        return plain(c::line_reconnecting(), c::line_no_relay());
    }
    let shows = view.wait_shows(now_ms);
    let waited: Vec<&str> =
        view.waiting.iter().filter(|w| shows && !w.you).map(|w| w.name.as_str()).collect();
    let waits_for_you = shows && view.waiting.iter().any(|w| w.you);
    let oldest = view.waiting.iter().map(|w| w.since_ms).min().map(|s| secs_since(s, now_ms));
    if waits_for_you {
        let others: Vec<&str> = view
            .members
            .iter()
            .filter(|m| !m.you && m.state != MemberState::Left)
            .map(|m| m.name.as_str())
            .collect();
        return Line {
            title: c::line_paused_for_you(),
            sub: c::line_waiting_on_you(
                &join_names(&others),
                others.len(),
                oldest.unwrap_or(0),
            ),
            buttons: LineButtons::DontWait,
            holds: true,
        };
    }
    let names = join_names(&waited);
    let many = waited.len() > 1;
    let without = (!many).then_some(names.as_str());
    let carry = view.carry_on_now(now_ms);
    match view.before(now_ms) {
        Some(Before::Idle) => return plain(c::line_not_started(), c::line_play_starts()),
        Some(Before::Count(n)) => {
            return plain(c::line_starting_in(n), c::line_everyone_ready());
        }
        Some(Before::Waiting) if !waited.is_empty() => {
            let secs = oldest.unwrap_or(0);
            if carry {
                return Line {
                    title: c::line_still_loading(&names, waited.len()),
                    sub: c::line_waited(secs),
                    buttons: LineButtons::CarryOn(c::start_without(without)),
                    holds: true,
                };
            }
            return Line {
                title: c::line_starting_when(&names, waited.len()),
                sub: c::line_why(WaitReason::Loading, secs),
                buttons: LineButtons::None,
                holds: true,
            };
        }
        _ => {}
    }
    if let Some(up) = &view.up_next
        && up.cancelled_by.is_none()
        && let Some(ends) = up.ends_at_ms
    {
        let secs = ((ends - now_ms) as f64 / 1000.0).ceil().max(0.0) as i64;
        return plain(c::line_up_next(secs), c::line_up_next_for_everyone(&up.episode.episode));
    }
    if !waited.is_empty() {
        if let Some(carried) = &view.carried_on {
            let past: Vec<&str> = carried.past.iter().map(String::as_str).collect();
            let (names, many) = if past.is_empty() {
                (names.clone(), many)
            } else {
                (join_names(&past), past.len() > 1)
            };
            return plain(c::line_watching(), c::line_carried(&carried.by, &names, many));
        }
        let secs = oldest.unwrap_or(0);
        if carry {
            return Line {
                title: c::line_still_loading(&names, waited.len()),
                sub: c::line_waited(secs),
                buttons: LineButtons::CarryOn(c::carry_on_without(without)),
                holds: true,
            };
        }
        let why = match (
            waited.len(),
            view.waiting.iter().find(|w| w.name == waited[0]).map(|w| w.reason),
        ) {
            (1, Some(reason)) => reason,
            _ => WaitReason::Loading,
        };
        return Line {
            title: c::line_paused_for(&names),
            sub: c::line_why(why, secs),
            buttons: LineButtons::None,
            holds: true,
        };
    }
    if let Some(me) = you
        && me.state == MemberState::Behind
    {
        return plain(
            c::line_watching(),
            c::line_behind(me.behind_s.unwrap_or(0.0).round() as i64),
        );
    }
    let down = view.relays.len() - view.relays_up();
    if !view.relays.is_empty() && down > 0 {
        return plain(c::line_watching(), c::line_relay_trouble(down, view.relays.len()));
    }
    if !view.playing {
        let title = match &view.paused_by {
            Some(by) if you.is_some_and(|y| &y.name == by) => c::line_you_paused(),
            Some(by) => c::line_paused(by),
            None => c::state_paused(),
        };
        return plain(title, c::line_paused_at(&at));
    }
    plain(c::line_watching(), c::line_at(&view.episode, &at))
}

/// How a face's ring draws.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Ring {
    /// With the room: a solid water ring.
    Water,
    /// The room is paused: a solid haze ring.
    Haze,
    /// The room waits for them: an arc filling toward the cap.
    Progress(f64),
    /// Not with the room: no ring, the face dimmed.
    None,
}

/// What a person's row shows.
#[derive(Debug, Clone, PartialEq)]
pub struct Face {
    pub word: String,
    pub ring: Ring,
    /// The face's opacity.
    pub dim: f64,
    /// The pause badge: they paused the room.
    pub badge: bool,
    /// The vermilion dot: an error on their side.
    pub error: bool,
    /// The word isn't the quiet default: it reads in silk, not haze.
    pub loud: bool,
}

pub fn face(view: &RoomView, member: &RoomMember, now_ms: i64) -> Face {
    let secs = member.since_ms.map(|s| secs_since(s, now_ms));
    let progress = |s: Option<i64>| {
        Ring::Progress((s.unwrap_or(0) as f64 / CAP_SECS as f64).clamp(0.04, 1.0))
    };
    let before = view.before_play;
    let (word, ring, dim) = match member.state {
        MemberState::Ready => (c::state_ready(), Ring::Water, 1.0),
        MemberState::Watching if before => (c::state_ready(), Ring::Water, 1.0),
        MemberState::Watching => (c::state_watching(), Ring::Water, 1.0),
        MemberState::Paused if member.paused_room => (c::state_paused_it(), Ring::Haze, 1.0),
        MemberState::Paused => (c::state_paused(), Ring::Haze, 1.0),
        MemberState::Behind => {
            (c::state_behind(member.behind_s.unwrap_or(0.0).round() as i64), Ring::Water, 1.0)
        }
        MemberState::Loading => (c::state_loading(secs.unwrap_or(0)), progress(secs), 1.0),
        MemberState::Buffering => (c::state_buffering(secs.unwrap_or(0)), progress(secs), 1.0),
        MemberState::OtherEpisode => {
            let episode = member.episode.as_ref().map_or("", |e| e.episode.as_str());
            (c::state_other_episode(episode), Ring::None, 0.5)
        }
        MemberState::Choosing => (c::state_choosing(), Ring::None, 0.5),
        MemberState::NoSource => (c::state_no_source(&view.episode), Ring::None, 0.5),
        MemberState::PlayerClosed => (c::state_closed(), Ring::None, 0.5),
        MemberState::Reconnecting => (c::state_reconnecting(), Ring::None, 0.5),
        MemberState::Left => (c::state_left(), Ring::None, 0.35),
    };
    let word = if member.muted { c::state_muted(&word) } else { word };
    Face {
        loud: !matches!(member.state, MemberState::Watching | MemberState::Ready),
        badge: member.state == MemberState::Paused && member.paused_room,
        error: member.state == MemberState::NoSource,
        word,
        ring,
        dim,
    }
}

/// Whether anyone isn't watching: the full screen strip stays while this holds.
pub fn anyone_not_watching(view: &RoomView) -> bool {
    !view.waiting.is_empty()
        || view.members.iter().any(|m| {
            !matches!(
                m.state,
                MemberState::Watching
                    | MemberState::Ready
                    | MemberState::Paused
                    | MemberState::Left
            )
        })
}

/// The members in the column's order: you first, then join order.
pub fn ordered(view: &RoomView) -> Vec<&RoomMember> {
    let mut list: Vec<&RoomMember> = view.members.iter().collect();
    list.sort_by_key(|m| !m.you);
    list
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::room::fixture;

    #[test]
    fn watching_says_where() {
        let v = fixture::view("watching");
        let l = line(&v, &Moves::default(), fixture::NOW);
        assert_eq!(l.title, "Watching together");
        assert_eq!(l.sub, "Episode 9 · 12:04");
        assert_eq!(l.buttons, LineButtons::None);
    }

    #[test]
    fn a_wait_counts_then_offers_carry_on() {
        let v = fixture::view("wait");
        let l = line(&v, &Moves::default(), fixture::NOW);
        assert_eq!(l.title, "Paused for Ren");
        assert_eq!(l.sub, "Loading · 6 s");
        assert!(l.holds);
        let v = fixture::view("cap");
        let l = line(&v, &Moves::default(), fixture::NOW);
        assert_eq!(l.title, "Ren is still loading");
        assert_eq!(l.buttons, LineButtons::CarryOn("Carry on without Ren".into()));
    }

    #[test]
    fn before_play_starts_without() {
        let l = line(&fixture::view("before-cap"), &Moves::default(), fixture::NOW);
        assert_eq!(l.buttons, LineButtons::CarryOn("Start without Ren".into()));
        let l = line(&fixture::view("before-wait"), &Moves::default(), fixture::NOW);
        assert_eq!(l.title, "Starting when Ren is ready");
    }

    #[test]
    fn paused_names_who() {
        let l = line(&fixture::view("paused"), &Moves::default(), fixture::NOW);
        assert_eq!((l.title.as_str(), l.sub.as_str()), ("Mika paused", "at 12:04"));
        let v = fixture::view("paused");
        let mika = v.members.iter().find(|m| m.name == "Mika").unwrap();
        let f = face(&v, mika, fixture::NOW);
        assert_eq!(f.word, "Paused it");
        assert!(f.badge);
    }

    #[test]
    fn own_wait_offers_dont_wait() {
        let l = line(&fixture::view("self-wait"), &Moves::default(), fixture::NOW);
        assert_eq!(l.title, "Paused for you");
        assert_eq!(l.sub, "Ada and Mika are waiting · 6 s");
        assert_eq!(l.buttons, LineButtons::DontWait);
    }

    #[test]
    fn words_and_rings() {
        let v = fixture::view("nosource");
        let ren = v.members.iter().find(|m| m.name == "Ren").unwrap();
        let f = face(&v, ren, fixture::NOW);
        assert_eq!(f.word, "No source for episode 9");
        assert!(f.error);
        assert_eq!(f.ring, Ring::None);
        let v = fixture::view("wait");
        let ren = v.members.iter().find(|m| m.name == "Ren").unwrap();
        assert_eq!(face(&v, ren, fixture::NOW).ring, Ring::Progress(0.3));
    }

    #[test]
    fn names_join() {
        assert_eq!(join_names(&["Ren"]), "Ren");
        assert_eq!(join_names(&["Ren", "Mika"]), "Ren and Mika");
        assert_eq!(join_names(&["Ren", "Mika", "Ada"]), "3 people");
    }
}
