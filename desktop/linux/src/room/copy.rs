//! The room's words, as the board writes them, until the engine branch's catalogue carries
//! them. Each function is named for the catalogue id it stands in for (`room_*`), so the swap
//! to `kaeri_i18n::t` is one `use` per caller. Nothing here may ship: the report lists every id.

pub fn room_title() -> String {
    "Watch together".into() // i18n-skip
}
pub fn room_in_room(episode: &str, count: usize) -> String {
    format!("{episode} · {count} in the room") // i18n-skip
}
pub fn room_show_room() -> String {
    "Show the room".into() // i18n-skip
}
pub fn room_invite() -> String {
    "Invite".into() // i18n-skip
}
pub fn room_details() -> String {
    "Details".into() // i18n-skip
}
pub fn room_leave() -> String {
    "Leave".into() // i18n-skip
}
pub fn room_say() -> String {
    "Say something".into() // i18n-skip
}
pub fn room_people() -> String {
    "People in the room".into() // i18n-skip
}

// The room's line.
pub fn line_watching() -> String {
    "Watching together".into() // i18n-skip
}
pub fn line_at(episode: &str, at: &str) -> String {
    format!("{episode} · {at}") // i18n-skip
}
pub fn line_paused(by: &str) -> String {
    format!("{by} paused") // i18n-skip
}
pub fn line_you_paused() -> String {
    "You paused".into() // i18n-skip
}
pub fn line_paused_at(at: &str) -> String {
    format!("at {at}") // i18n-skip
}
pub fn line_paused_for(names: &str) -> String {
    format!("Paused for {names}") // i18n-skip
}
pub fn line_why(why: &str, secs: i64) -> String {
    format!("{why} · {secs} s") // i18n-skip
}
pub fn why_loading() -> String {
    "Loading".into() // i18n-skip
}
pub fn why_buffering() -> String {
    "Buffering".into() // i18n-skip
}
pub fn why_seeking() -> String {
    "Seeking".into() // i18n-skip
}
pub fn line_still_loading(names: &str, many: bool) -> String {
    if many {
        format!("{names} are still loading") // i18n-skip
    } else {
        format!("{names} is still loading") // i18n-skip
    }
}
pub fn line_waited() -> String {
    "The room has waited 20 s".into() // i18n-skip
}
pub fn line_carried(by: &str, names: &str, many: bool) -> String {
    if many {
        format!("{by} carried on · they join when ready") // i18n-skip
    } else {
        format!("{by} carried on · {names} joins when ready") // i18n-skip
    }
}
pub fn line_paused_for_you() -> String {
    "Paused for you".into() // i18n-skip
}
pub fn line_waiting_on_you(names: &str, many: bool, secs: i64) -> String {
    if many {
        format!("{names} are waiting · {secs} s") // i18n-skip
    } else {
        format!("{names} is waiting · {secs} s") // i18n-skip
    }
}
pub fn line_behind(secs: i64) -> String {
    format!("You're {secs} s behind · catching up") // i18n-skip
}
pub fn line_reconnecting() -> String {
    "Reconnecting to the room".into() // i18n-skip
}
pub fn line_no_relay() -> String {
    "No relay is answering · your episode keeps playing".into() // i18n-skip
}
pub fn line_relay_trouble(down: usize, of: usize) -> String {
    if down == 1 {
        format!("1 of {of} relays isn't answering · the room carries on") // i18n-skip
    } else {
        format!("{down} of {of} relays aren't answering · the room carries on") // i18n-skip
    }
}
pub fn line_not_started() -> String {
    "Not started yet".into() // i18n-skip
}
pub fn line_play_starts() -> String {
    "Play starts it for everyone".into() // i18n-skip
}
pub fn line_starting_when(names: &str, many: bool) -> String {
    if many {
        format!("Starting when {names} are ready") // i18n-skip
    } else {
        format!("Starting when {names} is ready") // i18n-skip
    }
}
pub fn line_starting_in(secs: u32) -> String {
    format!("Starting in {secs}") // i18n-skip
}
pub fn line_everyone_ready() -> String {
    "Everyone is ready".into() // i18n-skip
}
pub fn line_invited(by: &str) -> String {
    format!("{by} invited you") // i18n-skip
}
pub fn line_over_cap(count: usize) -> String {
    format!("{count} people are here already · rooms work best with 10 or fewer") // i18n-skip
}
pub fn line_up_next(secs: i64) -> String {
    format!("Up next in {secs} s") // i18n-skip
}
pub fn line_up_next_for_everyone(episode: &str) -> String {
    format!("Episode {episode} plays for everyone") // i18n-skip
}
pub fn line_moved(by: &str) -> String {
    format!("{by} moved the room") // i18n-skip
}
pub fn line_go_along(others: &str, count: usize, at: &str) -> String {
    match count {
        0 => format!("Go along? The episode carries on from {at}."), // i18n-skip
        1 => format!("Go along? {others} is asked too. The episode carries on from {at}."), // i18n-skip
        _ => format!("Go along? {others} are asked too. The episode carries on from {at}."), // i18n-skip
    }
}
pub fn line_moved_away(by: &str) -> String {
    format!("{by} moved to a new room") // i18n-skip
}
pub fn line_went_along(names: &str) -> String {
    format!("{names} went along · this room carries on") // i18n-skip
}

// Buttons under the line.
pub fn carry_on_without(names: &str) -> String {
    format!("Carry on without {names}") // i18n-skip
}
pub fn start_without(names: &str) -> String {
    format!("Start without {names}") // i18n-skip
}
pub fn them() -> String {
    "them".into() // i18n-skip
}
pub fn join_room() -> String {
    "Join the room".into() // i18n-skip
}
pub fn join_anyway() -> String {
    "Join anyway".into() // i18n-skip
}
pub fn not_now() -> String {
    "Not now".into() // i18n-skip
}
pub fn go_along() -> String {
    "Go along".into() // i18n-skip
}
pub fn stay_here() -> String {
    "Stay here".into() // i18n-skip
}
pub fn dont_wait() -> String {
    "Don't wait for me".into() // i18n-skip
}

// Names joined.
pub fn names_two(a: &str, b: &str) -> String {
    format!("{a} and {b}") // i18n-skip
}
pub fn names_many(count: usize) -> String {
    format!("{count} people") // i18n-skip
}
pub fn you() -> String {
    "You".into() // i18n-skip
}
pub fn you_tag() -> String {
    "(you)".into() // i18n-skip
}

// A person's word.
pub fn state_watching() -> String {
    "Watching".into() // i18n-skip
}
pub fn state_ready() -> String {
    "Ready".into() // i18n-skip
}
pub fn state_paused_it() -> String {
    "Paused it".into() // i18n-skip
}
pub fn state_paused() -> String {
    "Paused".into() // i18n-skip
}
pub fn state_behind(secs: i64) -> String {
    format!("Behind · {secs} s") // i18n-skip
}
pub fn state_loading(secs: i64) -> String {
    format!("Loading · {secs} s") // i18n-skip
}
pub fn state_buffering(secs: i64) -> String {
    format!("Buffering · {secs} s") // i18n-skip
}
pub fn state_other_episode(episode: &str) -> String {
    format!("On episode {episode}") // i18n-skip
}
pub fn state_choosing() -> String {
    "Choosing a source".into() // i18n-skip
}
pub fn state_no_source(episode: &str) -> String {
    format!("No source for episode {episode}") // i18n-skip
}
pub fn state_closed() -> String {
    "Player closed".into() // i18n-skip
}
pub fn state_reconnecting() -> String {
    "Reconnecting".into() // i18n-skip
}
pub fn state_left() -> String {
    "Left".into() // i18n-skip
}
pub fn state_muted(word: &str) -> String {
    format!("{word} · Muted") // i18n-skip
}

// Invite.
pub fn invite_body() -> String {
    "Anyone with this link can join. The link carries the room's key, so only people who have it can read the room.".into() // i18n-skip
}
pub fn copy_link() -> String {
    "Copy link".into() // i18n-skip
}
pub fn link_copied() -> String {
    "Copied".into() // i18n-skip
}
pub fn new_invite() -> String {
    "New invite".into() // i18n-skip
}
pub fn new_invite_note() -> String {
    "New invite stops the old link working. Everyone in the room moves to the new one.".into() // i18n-skip
}
pub fn move_room_button() -> String {
    "Move to a new room…".into() // i18n-skip
}
pub fn move_room_note() -> String {
    "For someone already here: start a new room and pick who comes along.".into() // i18n-skip
}

// Details.
pub fn encryption() -> String {
    "Encryption".into() // i18n-skip
}
pub fn end_to_end() -> String {
    "End to end".into() // i18n-skip
}
pub fn relays() -> String {
    "Relays".into() // i18n-skip
}
pub fn relays_answering(up: usize, of: usize) -> String {
    if up == of {
        format!("{of} answering") // i18n-skip
    } else {
        format!("{up} of {of} answering") // i18n-skip
    }
}
pub fn relay_rtt(ms: u32) -> String {
    format!("{ms} ms") // i18n-skip
}
pub fn relay_down() -> String {
    "Not answering".into() // i18n-skip
}
pub fn relays_note() -> String {
    "Relays pass the room's messages along and can't read them. These are public relays run by others; Kaeri doesn't run any of them. If one stops answering, the room swaps in a spare.".into() // i18n-skip
}
pub fn edit_relays_button() -> String {
    "Edit relays…".into() // i18n-skip
}
pub fn edit_relays() -> String {
    "Edit relays".into() // i18n-skip
}
pub fn edit_relays_body() -> String {
    "Changes this room's relays for everyone in it. New invites carry the list.".into() // i18n-skip
}
pub fn remove_relay(url: &str) -> String {
    format!("Remove {url}") // i18n-skip
}
pub fn add() -> String {
    "Add".into() // i18n-skip
}
pub fn relays_settings_note() -> String {
    "Settings ▸ Playback ▸ Relays sets the list for new rooms.".into() // i18n-skip
}
pub fn done() -> String {
    "Done".into() // i18n-skip
}

// Leaving.
pub fn leave_title() -> String {
    "Leave the room?".into() // i18n-skip
}
pub fn leave_body() -> String {
    "The others keep watching.".into() // i18n-skip
}
pub fn leave_close_body() -> String {
    "The others keep watching. Keep the room closes the player and keeps your place.".into() // i18n-skip
}
pub fn keep_room() -> String {
    "Keep the room".into() // i18n-skip
}
pub fn cancel() -> String {
    "Cancel".into() // i18n-skip
}

// Move to a new room.
pub fn move_title() -> String {
    "Move to a new room".into() // i18n-skip
}
pub fn move_body() -> String {
    "Everyone you tick is asked to go along. Nobody moves without saying yes, and anyone not ticked stays in this room.".into() // i18n-skip
}
pub fn move_foot(at: &str) -> String {
    format!(
        "The new room picks up at {at} with a new key. Its link reaches only the people ticked."
    ) // i18n-skip
}
pub fn move_always() -> String {
    "Always goes".into() // i18n-skip
}
pub fn move_muted() -> String {
    "Muted, so not ticked".into() // i18n-skip
}
pub fn move_with(others: usize) -> String {
    match others {
        1 => "Move with 1 other".into(),      // i18n-skip
        n => format!("Move with {n} others"), // i18n-skip
    }
}

// The video area before play, and when your own player has nothing to show.
pub fn up_in_room() -> String {
    "Up in the room".into() // i18n-skip
}
pub fn room_is_watching() -> String {
    "The room is watching".into() // i18n-skip
}
pub fn episode_minutes(episode: &str, minutes: u32) -> String {
    format!("{episode} · {minutes} min") // i18n-skip
}
pub fn plays_from_yours() -> String {
    "Plays from your sources".into() // i18n-skip
}
pub fn play_for_everyone() -> String {
    "Play for everyone".into() // i18n-skip
}
pub fn anyone_can_start() -> String {
    "Anyone in the room can start it".into() // i18n-skip
}
pub fn waiting_first_frames() -> String {
    "Waiting for everyone's first frame".into() // i18n-skip
}
pub fn choose_source_for(episode: &str) -> String {
    format!("Choose a source for episode {episode}") // i18n-skip
}
pub fn choose_note() -> String {
    "The room keeps watching; you join at its place.".into() // i18n-skip
}
pub fn play_in_room() -> String {
    "Play in the room".into() // i18n-skip
}
pub fn none_has(episode: &str) -> String {
    format!("None of your sources has episode {episode}") // i18n-skip
}
pub fn none_has_note(count: usize) -> String {
    format!(
        "Kaeri looked in your {count} sources. You stay in the room and the chat while the others watch."
    ) // i18n-skip
}
pub fn open_sources() -> String {
    "Open sources".into() // i18n-skip
}
pub fn search_titles() -> String {
    "Search titles".into() // i18n-skip
}
pub fn paused_until_starts(secs: i64) -> String {
    format!("The room is paused until this starts · {secs} s") // i18n-skip
}

// Up next and Skip opening.
pub fn up_next_for_room() -> String {
    "Up next for the room".into() // i18n-skip
}
pub fn plays_for_everyone_in(secs: i64) -> String {
    format!("Plays for everyone in {secs} s") // i18n-skip
}

// Watch together, the place.
pub fn place_intro() -> String {
    "Each of you plays the episode from your own sources; the room keeps you in step. Start a room from any episode with Watch together, in the player or on its page.".into() // i18n-skip
}
pub fn your_room() -> String {
    "Your room".into() // i18n-skip
}
pub fn open_player() -> String {
    "Open the player".into() // i18n-skip
}
pub fn leave_ellipsis() -> String {
    "Leave…".into() // i18n-skip
}
pub fn join_a_room() -> String {
    "Join a room".into() // i18n-skip
}
pub fn invite_link() -> String {
    "Invite link".into() // i18n-skip
}
pub fn paste() -> String {
    "Paste".into() // i18n-skip
}
pub fn join() -> String {
    "Join".into() // i18n-skip
}
pub fn try_again() -> String {
    "Try again".into() // i18n-skip
}
pub fn unreachable() -> String {
    "Couldn't reach this room's relays.".into() // i18n-skip
}
pub fn unreachable_note(link: usize, spares: usize) -> String {
    format!(
        "Ask for a new invite. Kaeri tried the link's {link} relays and {spares} spares for 8 s each."
    ) // i18n-skip
}
pub fn recent_rooms() -> String {
    "Recent rooms".into() // i18n-skip
}
pub fn recent_note() -> String {
    "The last ten, kept on this computer".into() // i18n-skip
}
pub fn rejoin() -> String {
    "Rejoin".into() // i18n-skip
}
pub fn nobody_answered() -> String {
    "Nobody answered in this room.".into() // i18n-skip
}
pub fn nobody_answered_note() -> String {
    "If someone made a new invite, the old one stopped working; ask them for the new link."
        .into() // i18n-skip
}
pub fn recent_with(episode: &str, names: &str, when: &str) -> String {
    format!("Episode {episode} · with {names} · {when}") // i18n-skip
}
