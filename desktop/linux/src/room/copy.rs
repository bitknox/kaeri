//! The room's words from the catalogue (`i18n/en/ui-room.ftl`), in the shapes the room's widgets
//! ask for: episode numbers as the room carries them, counts as `usize`, and the board's choice
//! between one name and many.

use kaeri_i18n::t;
use kaeri_model::room::{Reaction, WaitReason};

/// An episode number as the catalogue counts it: "9" is 9; anything else reads as 0.
fn ep(episode: &str) -> i64 {
    episode.trim().parse::<f64>().map_or(0, |n| n as i64)
}

fn n(count: usize) -> i64 {
    i64::try_from(count).unwrap_or(i64::MAX)
}

pub fn room_title() -> String {
    t::nav_watch_together()
}
pub fn room_show_room() -> String {
    t::room_show()
}
pub fn room_invite() -> String {
    t::room_invite()
}
pub fn room_details() -> String {
    t::room_details()
}
pub fn room_leave() -> String {
    t::action_leave()
}
pub fn room_say() -> String {
    t::room_chat_field()
}
/// The people list's name for screen readers, "3 people"; the catalogue has no "People in the
/// room".
pub fn room_people(count: usize) -> String {
    t::room_names_people(n(count))
}

// The room's line.
pub fn line_watching() -> String {
    t::room_line_watching()
}
pub fn line_at(episode: &str, at: &str) -> String {
    t::room_line_watching_detail(ep(episode), at)
}
pub fn line_paused(by: &str) -> String {
    t::room_line_paused_by(by)
}
pub fn line_you_paused() -> String {
    t::room_line_you_paused()
}
pub fn line_paused_at(at: &str) -> String {
    t::room_line_paused_at(at)
}
pub fn line_paused_for(names: &str) -> String {
    t::room_line_paused_for(names)
}
/// "Loading · 6 s", "Buffering · 3 s": why the room waits, and how long so far.
pub fn line_why(reason: WaitReason, secs: i64) -> String {
    match reason {
        WaitReason::Buffering => t::room_state_buffering(secs),
        // The catalogue has no "Seeking · N s": a seek waits on loading.
        WaitReason::Loading | WaitReason::Seeking => t::room_state_loading(secs),
    }
}
pub fn line_still_loading(names: &str, count: usize) -> String {
    t::room_line_still_loading(names, n(count))
}
pub fn line_waited(secs: i64) -> String {
    t::room_line_waited(secs)
}
pub fn line_carried(by: &str, names: &str, many: bool) -> String {
    if many { t::room_line_carried_on_many(by) } else { t::room_line_carried_on_one(by, names) }
}
pub fn line_paused_for_you() -> String {
    t::room_line_paused_for_you()
}
pub fn line_waiting_on_you(names: &str, count: usize, secs: i64) -> String {
    t::room_line_waiting_for_you(names, n(count), secs)
}
pub fn line_behind(secs: i64) -> String {
    t::room_line_you_behind(secs)
}
pub fn line_reconnecting() -> String {
    t::room_line_reconnecting()
}
pub fn line_no_relay() -> String {
    t::room_line_reconnecting_detail()
}
pub fn line_relay_trouble(down: usize, of: usize) -> String {
    t::room_line_relays_down(n(down), n(of))
}
pub fn line_not_started() -> String {
    t::room_line_not_started()
}
pub fn line_play_starts() -> String {
    t::room_line_play_starts()
}
pub fn line_starting_when(names: &str, count: usize) -> String {
    t::room_line_starting_when(names, n(count))
}
pub fn line_starting_in(secs: u32) -> String {
    t::room_line_starting_in(i64::from(secs))
}
pub fn line_everyone_ready() -> String {
    t::room_line_everyone_ready()
}
pub fn line_invited(by: &str) -> String {
    t::room_invited_by(by)
}
pub fn line_invited_detail(title: &str, episode: &str) -> String {
    t::room_invited_detail(title, ep(episode))
}
pub fn line_over_cap(count: usize) -> String {
    t::room_invited_over(n(count))
}
pub fn line_up_next(secs: i64) -> String {
    t::room_line_up_next(secs)
}
pub fn line_up_next_for_everyone(episode: &str) -> String {
    t::room_line_up_next_detail(ep(episode))
}
pub fn line_moved(by: &str) -> String {
    t::room_line_moved(by)
}
pub fn line_go_along(others: &str, count: usize, at: &str) -> String {
    if count == 0 {
        t::room_line_moved_detail_alone(at)
    } else {
        t::room_line_moved_detail(others, n(count), at)
    }
}
pub fn line_moved_away(by: &str) -> String {
    t::room_line_moved_away(by)
}
pub fn line_went_along(names: &str) -> String {
    t::room_line_went_along(names)
}
/// Carry on without one person, or without them.
pub fn carry_on_without(name: Option<&str>) -> String {
    name.map_or_else(t::room_carry_on_without_them, t::room_carry_on_without)
}
pub fn start_without(name: Option<&str>) -> String {
    name.map_or_else(t::room_start_without_them, t::room_start_without)
}
pub fn join_room() -> String {
    t::room_join()
}
pub fn join_anyway() -> String {
    t::room_join_anyway()
}
pub fn not_now() -> String {
    t::action_not_now()
}
pub fn go_along() -> String {
    t::room_go_along()
}
pub fn stay_here() -> String {
    t::room_stay()
}
pub fn dont_wait() -> String {
    t::room_dont_wait()
}
pub fn names_two(a: &str, b: &str) -> String {
    t::room_names_and(a, b)
}
pub fn names_many(count: usize) -> String {
    t::room_names_people(n(count))
}
pub fn you() -> String {
    t::room_you()
}
pub fn you_tag() -> String {
    t::room_you_marker()
}

// Each person's word.
pub fn state_watching() -> String {
    t::room_state_watching()
}
pub fn state_ready() -> String {
    t::room_state_ready()
}
pub fn state_paused_it() -> String {
    t::room_state_paused_it()
}
pub fn state_paused() -> String {
    t::room_state_paused()
}
pub fn state_behind(secs: i64) -> String {
    t::room_state_behind(secs)
}
pub fn state_loading(secs: i64) -> String {
    t::room_state_loading(secs)
}
pub fn state_buffering(secs: i64) -> String {
    t::room_state_buffering(secs)
}
pub fn state_other_episode(episode: &str) -> String {
    t::room_state_other_episode(ep(episode))
}
pub fn state_choosing() -> String {
    t::room_state_choosing()
}
pub fn state_no_source(episode: &str) -> String {
    t::room_state_no_source(ep(episode))
}
pub fn state_closed() -> String {
    t::room_state_player_closed()
}
pub fn state_reconnecting() -> String {
    t::room_state_reconnecting()
}
pub fn state_left() -> String {
    t::room_state_left()
}
pub fn state_muted(word: &str) -> String {
    t::room_state_and_muted(word)
}

// Invite and Details.
pub fn invite_body() -> String {
    format!("{} {}", t::room_invite_anyone(), t::room_invite_key()) // i18n-skip
}
pub fn copy_link() -> String {
    t::room_copy_link()
}
pub fn link_copied() -> String {
    t::room_link_copied()
}
pub fn new_invite() -> String {
    t::room_new_invite()
}
pub fn new_invite_note() -> String {
    t::room_new_invite_line()
}
pub fn move_room_button() -> String {
    t::room_move_open()
}
pub fn move_room_note() -> String {
    t::room_move_open_line()
}
pub fn encryption() -> String {
    t::room_details_encryption()
}
pub fn end_to_end() -> String {
    t::room_details_end_to_end()
}
pub fn relays() -> String {
    t::room_relays()
}
pub fn relays_answering(up: usize, of: usize) -> String {
    if up == of { t::room_relays_answering(n(of)) } else { t::room_relays_answering_of(n(up), n(of)) }
}
pub fn relay_rtt(ms: u32) -> String {
    t::room_relay_latency(i64::from(ms))
}
pub fn relay_down() -> String {
    t::room_relay_not_answering()
}
pub fn relays_note() -> String {
    t::room_details_relays_line()
}
pub fn edit_relays_button() -> String {
    t::room_edit_relays()
}
pub fn edit_relays() -> String {
    t::room_edit_relays_title()
}
pub fn edit_relays_body() -> String {
    t::room_edit_relays_line()
}
pub fn remove_relay(url: &str) -> String {
    t::room_relay_remove(url)
}
pub fn add() -> String {
    t::room_relay_add()
}
pub fn relays_settings_note() -> String {
    t::room_edit_relays_settings()
}
pub fn done() -> String {
    t::action_done()
}

// Leave, and Move to a new room.
pub fn leave_title() -> String {
    t::room_leave_title()
}
pub fn leave_body() -> String {
    t::room_leave_line()
}
pub fn leave_close_body() -> String {
    t::room_leave_line_keep()
}
pub fn keep_room() -> String {
    t::room_keep()
}
pub fn cancel() -> String {
    t::action_cancel()
}
pub fn move_title() -> String {
    t::room_move_title()
}
pub fn move_body() -> String {
    t::room_move_body()
}
pub fn move_foot(at: &str) -> String {
    t::room_move_foot(at)
}
pub fn move_always() -> String {
    t::room_move_always_goes()
}
pub fn move_muted() -> String {
    t::room_move_muted()
}
pub fn move_with(others: usize) -> String {
    t::room_move_with(n(others))
}

// The banner over the video.
pub fn up_in_room() -> String {
    t::room_up_in_room()
}
pub fn room_is_watching() -> String {
    t::room_room_is_watching()
}
/// "Episode 9 · The Long Leg · 24 min", or as much of it as is known.
pub fn banner_episode(episode: &str, name: Option<&str>, minutes: Option<u32>) -> String {
    match (name, minutes) {
        (Some(name), Some(min)) => {
            t::room_banner_episode(ep(episode), name, &t::room_minutes(i64::from(min)))
        }
        (Some(name), None) => t::room_up_next_episode(ep(episode), name),
        _ => t::room_up_next_episode_plain(ep(episode)),
    }
}
pub fn plays_from_yours() -> String {
    t::room_plays_from_sources()
}
pub fn play_for_everyone() -> String {
    t::room_play_for_everyone()
}
pub fn anyone_can_start() -> String {
    t::room_anyone_can_start()
}
pub fn choose_source_for(episode: &str) -> String {
    t::room_choose_source(ep(episode))
}
pub fn choose_note() -> String {
    t::room_choose_source_line()
}
pub fn play_in_room() -> String {
    t::room_play_in_room()
}
pub fn none_has(episode: &str) -> String {
    t::room_no_source_title(ep(episode))
}
pub fn none_has_note(count: usize) -> String {
    t::room_no_source_line(n(count))
}
pub fn open_sources() -> String {
    t::room_open_sources()
}
pub fn search_titles() -> String {
    t::room_search_titles()
}

// Watch together, the place in the main window.
pub fn place_intro() -> String {
    t::room_place_line()
}
pub fn your_room() -> String {
    t::room_place_yours()
}
pub fn open_player() -> String {
    t::room_open_player()
}
pub fn leave_ellipsis() -> String {
    t::room_leave_ellipsis()
}
pub fn join_a_room() -> String {
    t::room_place_join()
}
pub fn invite_link() -> String {
    t::room_place_link()
}
pub fn paste() -> String {
    t::room_place_paste()
}
pub fn join() -> String {
    t::action_join()
}
pub fn try_again() -> String {
    t::action_try_again()
}
pub fn unreachable() -> String {
    t::room_error_unreachable_title()
}
pub fn unreachable_note() -> String {
    format!("{} {}", t::room_error_ask_new_invite(), t::room_error_tried(8)) // i18n-skip
}
pub fn recent_rooms() -> String {
    t::room_recent()
}
pub fn recent_note() -> String {
    t::room_recent_note()
}
pub fn rejoin() -> String {
    t::room_rejoin()
}
pub fn nobody_answered() -> String {
    t::room_error_nobody_answered()
}
pub fn nobody_answered_note() -> String {
    t::room_error_nobody_answered_line()
}

/// "Ren laughed at 7:48".
pub fn reacted(reaction: Reaction, name: &str, at: f64) -> String {
    let at = super::words::mmss(at);
    match reaction {
        Reaction::Laugh => t::room_reacted_laugh(name, &at),
        Reaction::Gasp => t::room_reacted_gasp(name, &at),
        Reaction::Clap => t::room_reacted_clap(name, &at),
        Reaction::Cry => t::room_reacted_cry(name, &at),
    }
}
