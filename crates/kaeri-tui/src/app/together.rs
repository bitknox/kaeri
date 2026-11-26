//! Together: the room this terminal is in, a line to join another by its invite link, and the
//! last rooms.

use kaeri_i18n::t;
use kaeri_model::{
    command::Command,
    room::{
        Reaction, RoomChatRefusal, RoomCommand, RoomConversation, RoomLeftWhy, RoomMember,
        RoomRecent, RoomTarget, RoomView,
    },
};

use super::{App, Field, alert, info, say};
use crate::{
    keymap::Action,
    nav::{Focus, Overlay, Section, StageView},
};

impl App {
    /// Whether the Together rows have focus.
    pub(super) fn in_together_stage(&self) -> bool {
        self.nav.section() == Section::Together
            && self.nav.focus() == Focus::Stage
            && matches!(self.nav.current(), StageView::Section(Section::Together))
    }

    /// The rows the cursor walks: the room's other members, the Join line, then the recent
    /// rooms.
    pub(super) fn together_rows(&self) -> usize {
        self.others().count() + 1 + self.room_recent.len()
    }

    /// The room's members other than this device, in the order the rows list them. They are
    /// rows so `M` can mute one.
    pub fn others(&self) -> impl Iterator<Item = &RoomMember> {
        self.room_members.iter().filter(|m| !m.you && self.room.is_some())
    }

    /// The other member under the cursor.
    pub fn selected_member(&self) -> Option<&RoomMember> {
        self.others().nth(self.cursors.together.selected)
    }

    /// The row of the Join line.
    pub fn join_row(&self) -> usize {
        self.others().count()
    }

    /// The recent room under the cursor; `None` on the Join line or a member.
    pub fn selected_room(&self) -> Option<&RoomRecent> {
        self.room_recent.get(self.cursors.together.selected.checked_sub(self.join_row() + 1)?)
    }

    /// Whether Hide chat is on.
    pub fn chat_hidden(&self) -> bool {
        self.room_chat_state.as_ref().is_some_and(|s| s.hidden)
    }

    /// What `s` does in the room now, when it does anything: Carry on once it is offered, Play
    /// for everyone before play, or Don't wait for me while the room waits for this device.
    pub fn room_go(&self) -> Option<RoomGo> {
        let room = self.room.as_ref()?;
        if room.can_carry_on {
            Some(RoomGo::CarryOn)
        } else if room.can_dont_wait {
            Some(RoomGo::DontWait)
        } else if room.before_play && room.starts_at_ms.is_none() {
            Some(RoomGo::PlayForEveryone)
        } else {
            None
        }
    }

    /// The keys of the section. `Some` when one was taken, with what it asks of the engine.
    ///
    /// The section reads the keys the rest of the app already has for its own screens, the way
    /// Downloads does: `c` copy the link, `r` a new invite, `s` carry on or play, `x` leave or
    /// forget a recent room. They are matched before the title commands, which would otherwise
    /// answer that nothing is selected.
    pub(super) fn together_key(&mut self, action: Action) -> Option<Option<Command>> {
        if !self.in_together_stage() {
            return None;
        }
        let in_room = self.room.is_some();
        Some(match action {
            Action::Open => self.open_together_row(),
            Action::HideFromContinue if self.selected_room().is_some() => self.forget_room(),
            Action::Refresh if in_room => Some(Command::Room(RoomCommand::NewInvite)),
            Action::ClearCompleted if in_room => self.room_copy_link(),
            Action::ShowSources if in_room => self.room_go().map(|go| {
                Command::Room(match go {
                    RoomGo::CarryOn => RoomCommand::CarryOn,
                    RoomGo::DontWait => RoomCommand::DontWait,
                    RoomGo::PlayForEveryone => RoomCommand::Play { play: true },
                })
            }),
            Action::HideFromContinue if in_room => {
                self.nav.open_overlay(Overlay::LeaveRoom);
                None
            }
            Action::FixMapping if in_room => self.open_say(),
            Action::MarkAllPrevious if in_room => self.room_mute(),
            Action::ToggleSynopsis if in_room => {
                Some(Command::Room(RoomCommand::HideChat { hidden: !self.chat_hidden() }))
            }
            _ => return None,
        })
    }

    /// Enter: the Join line asks for an invite link, or joins a full room anyway once it said
    /// so; a recent room is joined again. A member's row opens nothing.
    fn open_together_row(&mut self) -> Option<Command> {
        if self.selected_member().is_some() {
            return None;
        }
        match self.selected_room() {
            None => {
                if let Some(invite) = self.room_full.take() {
                    self.room_joining = Some(invite.clone());
                    return Some(Command::Room(RoomCommand::Lobby {
                        invite: Some(invite),
                        name: None,
                        pick: None,
                        anyway: true,
                    }));
                }
                self.field_mut(Field::RoomLink).clear();
                self.nav.open_overlay(Overlay::JoinRoom);
                None
            }
            Some(recent) => {
                if self.room.as_ref().is_some_and(|r| r.room_id == recent.room_id) {
                    return None;
                }
                let room_id = recent.room_id.clone();
                Some(Command::Room(RoomCommand::Rejoin { room_id }))
            }
        }
    }

    /// Enter on a pasted link: the room it is the invite to.
    pub(super) fn submit_room_link(&mut self) -> Option<Command> {
        let text: String =
            self.text(Field::RoomLink).chars().filter(|c| !c.is_whitespace()).collect();
        if text.is_empty() {
            return None;
        }
        let Some(target) = RoomTarget::parse(&text) else {
            self.push_toast(alert(t::tui_room_not_a_link()));
            return None;
        };
        if target.old.is_some() {
            self.push_toast(alert(t::room_error_old_invite()));
            return None;
        }
        self.nav.close_overlay();
        self.field_mut(Field::RoomLink).clear();
        Some(self.join_room(target.link))
    }

    /// Join the room `invite` leads to.
    pub fn join_room(&mut self, invite: String) -> Command {
        self.room_full = None;
        self.room_joining = Some(invite.clone());
        Command::Room(RoomCommand::Lobby {
            invite: Some(invite),
            name: None,
            pick: None,
            anyway: false,
        })
    }

    /// `x` on a recent room: off the list here, and off the device.
    fn forget_room(&mut self) -> Option<Command> {
        let room_id = self.selected_room()?.room_id.clone();
        self.room_recent.retain(|r| r.room_id != room_id);
        self.keep_together_cursor();
        Some(Command::Room(RoomCommand::ForgetRoom { room_id }))
    }

    /// Enter in the Leave question: out of the room. What plays goes on as a private session.
    pub(super) fn confirm_leave_room(&mut self) -> Option<Command> {
        self.nav.close_overlay();
        self.room.as_ref()?;
        Some(Command::Room(RoomCommand::Leave))
    }

    /// `c`: the invite link to the terminal's clipboard.
    fn room_copy_link(&mut self) -> Option<Command> {
        let link = self.room.as_ref().and_then(|r| r.link.clone())?;
        self.clipboard = Some(link);
        self.push_toast(info(t::room_link_copied()));
        None
    }

    /// `M` on a member's row: mute them on this device, or unmute them.
    fn room_mute(&mut self) -> Option<Command> {
        let member = self.selected_member()?;
        let (id, muted) = (member.id.clone(), !member.muted);
        Some(Command::Room(RoomCommand::Mute { id, muted }))
    }

    /// `m`: the field for saying something, unless Hide chat is on.
    fn open_say(&mut self) -> Option<Command> {
        if self.chat_hidden() {
            self.push_toast(info(t::tui_room_chat_hidden()));
            return None;
        }
        self.nav.open_overlay(Overlay::Say);
        None
    }

    /// Enter in the Say field: the line goes to the room and the field stays open for the
    /// next. A typed `*laughs*`, `*gasps*`, `*claps*` or `*cries*` is the reaction.
    pub(super) fn submit_say(&mut self) -> Option<Command> {
        let text = self.text(Field::Say).trim().to_owned();
        if text.is_empty() {
            return None;
        }
        self.field_mut(Field::Say).clear();
        let reaction = match text.as_str() {
            "*laughs*" => Some(Reaction::Laugh),
            "*gasps*" => Some(Reaction::Gasp),
            "*claps*" => Some(Reaction::Clap),
            "*cries*" => Some(Reaction::Cry),
            _ => None,
        };
        Some(Command::Room(match reaction {
            Some(reaction) => RoomCommand::React { reaction },
            None => RoomCommand::Say { text },
        }))
    }

    /// The conversation changed. A line no relay echoed comes back into the field, once, when
    /// the field is open and empty.
    pub(super) fn set_room_chat(&mut self, conversation: RoomConversation) {
        let failed = conversation.failed.clone();
        let fresh = failed != self.room_chat.failed;
        self.room_chat = conversation;
        if let Some(text) = failed
            && fresh
            && self.nav.overlay() == Some(&Overlay::Say)
            && self.text(Field::Say).is_empty()
        {
            self.set_text(Field::Say, text);
        }
    }

    /// The engine turned a line or a reaction down.
    pub(super) fn chat_refused(&mut self, refusal: RoomChatRefusal) {
        match refusal {
            RoomChatRefusal::Wait { seconds } => {
                self.push_toast(info(t::tui_room_chat_wait(i64::from(seconds))));
            }
            RoomChatRefusal::NotPlaying => {}
        }
    }

    /// In a room: said once, wherever the terminal is, since a room started from a title page
    /// shows on Together.
    pub(super) fn room_joined(&mut self, view: RoomView) {
        self.room_joining = None;
        self.room_full = None;
        if self.nav.section() != Section::Together {
            let key = self.key_for(Action::JumpSection(10));
            self.push_toast(info(t::tui_room_joined(&key)));
        }
        self.set_room(view);
    }

    /// Take the engine's word for how the room stands.
    pub(super) fn set_room(&mut self, view: RoomView) {
        self.room = Some(view);
    }

    /// Out of the room, and why, when it wasn't on purpose.
    pub(super) fn room_left(&mut self, why: RoomLeftWhy) {
        let joining = self.room_joining.take();
        // Moved: the new room's `RoomJoined` follows.
        if why == RoomLeftWhy::Moved {
            return;
        }
        self.end_room();
        let said = match why {
            RoomLeftWhy::Left | RoomLeftWhy::Moved => return,
            RoomLeftWhy::Unreachable => t::room_error_unreachable(),
            RoomLeftWhy::Full => {
                self.room_full = joining;
                t::tui_room_full(&self.key_for(Action::Open))
            }
            RoomLeftWhy::OldInvite => t::room_error_old_invite(),
            RoomLeftWhy::Failed { reason } => say(&reason),
        };
        self.push_toast(alert(said));
    }

    /// The room is over here: everything kept for it goes.
    fn end_room(&mut self) {
        self.room = None;
        self.room_members.clear();
        self.room_chat = Default::default();
        self.room_chat_state = None;
        if self.nav.overlay() == Some(&Overlay::LeaveRoom)
            || self.nav.overlay() == Some(&Overlay::Say)
        {
            self.nav.close_overlay();
        }
        self.keep_together_cursor();
    }

    /// The last rooms arrived: keep the cursor on a row that exists.
    pub(super) fn set_recent_rooms(&mut self, rooms: Vec<RoomRecent>) {
        self.room_recent = rooms;
        self.keep_together_cursor();
    }

    /// The members changed: keep the cursor on a row that exists.
    pub(super) fn set_room_members(&mut self, members: Vec<RoomMember>) {
        self.room_members = members;
        self.keep_together_cursor();
    }

    fn keep_together_cursor(&mut self) {
        let last = self.together_rows() - 1;
        self.cursors.together.selected = self.cursors.together.selected.min(last);
    }
}

/// What `s` does in a room, as [`App::room_go`] finds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomGo {
    CarryOn,
    DontWait,
    PlayForEveryone,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::fixtures::*;
    use kaeri_core::ids::AnilistId;
    use kaeri_model::{
        event::Event,
        room::{MemberState, RoomChatState, RoomConnection, RoomShare},
    };

    /// A real invite, as another device would have made it.
    fn invite() -> String {
        RoomShare {
            secret: [7; 16],
            relays: Vec::new(),
            by: Some("Mika".into()),
            title: Some("Natsukaze Relay".into()),
            episode: Some("9".into()),
        }
        .https_link()
    }

    fn view() -> RoomView {
        RoomView {
            room_id: "room-1".into(),
            name: "Ada".into(),
            connection: RoomConnection::Connected,
            link: Some(invite()),
            ..Default::default()
        }
    }

    fn recent(room_id: &str) -> RoomRecent {
        RoomRecent {
            room_id: room_id.into(),
            id: AnilistId::new(104_321),
            title: "Natsukaze Relay".into(),
            episode: "8".into(),
            names: vec!["Mika".into()],
            last_at: 0,
            cover_url: None,
        }
    }

    fn member(id: &str, you: bool, muted: bool) -> RoomMember {
        RoomMember {
            id: id.into(),
            name: id.into(),
            you,
            state: MemberState::Watching,
            since_ms: None,
            behind_s: None,
            episode: None,
            paused_room: false,
            waited: false,
            background: false,
            muted,
        }
    }

    fn chat_state(hidden: bool) -> RoomChatState {
        RoomChatState {
            max_length: 500,
            muted: Vec::new(),
            wait_seconds: 0,
            react_wait_seconds: 0,
            hidden,
        }
    }

    fn in_room() -> App {
        let mut a = app();
        a.apply(Event::RoomJoined(view()));
        a.handle(Action::JumpSection(10), 5);
        a.nav.focus_stage();
        a
    }

    #[test]
    fn zero_is_the_tenth_section_and_asks_for_the_rooms() {
        let mut a = app();
        assert_eq!(a.key_for(Action::JumpSection(10)), "0");
        let task = a.handle(Action::JumpSection(10), 5);
        assert_eq!(a.nav.section(), Section::Together);
        assert_eq!(task, Some(Command::Room(RoomCommand::LoadRecent)));
    }

    #[test]
    fn a_pasted_invite_joins_its_room_and_anything_else_is_refused() {
        let mut a = app();
        a.handle(Action::JumpSection(10), 5);
        a.nav.focus_stage();
        a.handle(Action::Open, 5);
        assert_eq!(a.nav.overlay(), Some(&Overlay::JoinRoom));

        for ch in "nonsense".chars() {
            a.type_char(ch);
        }
        assert_eq!(a.handle(Action::Open, 5), None);
        assert_eq!(a.nav.overlay(), Some(&Overlay::JoinRoom), "kept, to be corrected");

        // An old room's link can't be joined: said, and kept to be replaced.
        a.dismiss_field();
        a.handle(Action::Open, 5);
        a.paste("https://kaeri.app/party#s=room.example%3A8999&r=kite-7qm4-xr9d");
        assert_eq!(a.handle(Action::Open, 5), None);
        assert_eq!(a.nav.overlay(), Some(&Overlay::JoinRoom));

        a.dismiss_field();
        a.handle(Action::Open, 5);
        a.paste(&invite());
        let task = a.handle(Action::Open, 5);
        assert_eq!(
            task,
            Some(Command::Room(RoomCommand::Lobby {
                invite: Some(invite()),
                name: None,
                pick: None,
                anyway: false,
            }))
        );
        assert!(a.nav.overlay().is_none());
        assert!(a.text(Field::RoomLink).is_empty(), "the spent link is not kept");
    }

    #[test]
    fn a_full_room_offers_join_anyway_on_the_join_line() {
        let mut a = app();
        a.handle(Action::JumpSection(10), 5);
        a.nav.focus_stage();
        a.handle(Action::Open, 5);
        a.paste(&invite());
        a.handle(Action::Open, 5);
        a.apply(Event::RoomLeft { why: RoomLeftWhy::Full });
        assert_eq!(a.room_full.as_deref(), Some(invite().as_str()));
        assert_eq!(
            a.handle(Action::Open, 5),
            Some(Command::Room(RoomCommand::Lobby {
                invite: Some(invite()),
                name: None,
                pick: None,
                anyway: true,
            }))
        );
        assert!(a.room_full.is_none(), "asked once");
    }

    #[test]
    fn a_recent_room_is_joined_again_from_its_row_and_x_forgets_it() {
        let mut a = app();
        a.handle(Action::JumpSection(10), 5);
        a.nav.focus_stage();
        a.apply(Event::RoomRecent(vec![recent("room-1"), recent("room-2")]));
        a.handle(Action::Down, 5);
        a.handle(Action::Down, 5);
        assert_eq!(a.selected_room().map(|r| r.room_id.as_str()), Some("room-2"));
        assert_eq!(
            a.handle(Action::Open, 5),
            Some(Command::Room(RoomCommand::Rejoin { room_id: "room-2".into() }))
        );
        assert_eq!(
            a.handle(Action::HideFromContinue, 5),
            Some(Command::Room(RoomCommand::ForgetRoom { room_id: "room-2".into() }))
        );
        assert_eq!(a.room_recent.len(), 1);
        assert_eq!(a.selected_room().map(|r| r.room_id.as_str()), Some("room-1"));
    }

    #[test]
    fn the_rooms_keys_work_only_in_a_room() {
        let mut a = app();
        a.handle(Action::JumpSection(10), 5);
        a.nav.focus_stage();
        assert_eq!(a.handle(Action::HideFromContinue, 5), None);
        assert_eq!(
            a.handle(Action::Refresh, 5),
            Some(Command::Room(RoomCommand::LoadRecent)),
            "outside a room, r reloads the rooms"
        );
        assert_eq!(a.handle(Action::ClearCompleted, 5), None);
        assert!(a.take_clipboard().is_none());

        a.apply(Event::RoomJoined(view()));
        a.apply(Event::RoomMembers(vec![member("Ada", true, false)]));
        assert_eq!(a.handle(Action::ClearCompleted, 5), None);
        assert_eq!(a.take_clipboard(), Some(invite()));
        assert_eq!(a.handle(Action::Refresh, 5), Some(Command::Room(RoomCommand::NewInvite)));
        assert_eq!(a.handle(Action::ShowSources, 5), None, "nothing to carry on from");

        // Leaving is asked first.
        assert_eq!(a.handle(Action::HideFromContinue, 5), None);
        assert_eq!(a.nav.overlay(), Some(&Overlay::LeaveRoom));
        assert_eq!(a.handle(Action::Open, 5), Some(Command::Room(RoomCommand::Leave)));
        a.apply(Event::RoomLeft { why: RoomLeftWhy::Left });
        assert!(a.room.is_none() && a.room_members.is_empty());
    }

    #[test]
    fn s_plays_for_everyone_before_play_and_carries_on_once_offered() {
        let mut a = in_room();
        a.apply(Event::RoomState(RoomView { before_play: true, ..view() }));
        assert_eq!(
            a.handle(Action::ShowSources, 5),
            Some(Command::Room(RoomCommand::Play { play: true }))
        );
        a.apply(Event::RoomState(RoomView { playing: true, can_carry_on: true, ..view() }));
        assert_eq!(a.handle(Action::ShowSources, 5), Some(Command::Room(RoomCommand::CarryOn)));
        a.apply(Event::RoomState(RoomView { can_dont_wait: true, ..view() }));
        assert_eq!(
            a.handle(Action::ShowSources, 5),
            Some(Command::Room(RoomCommand::DontWait))
        );
    }

    #[test]
    fn m_opens_the_say_field_and_enter_sends_and_keeps_it_open() {
        let mut a = in_room();
        a.handle(Action::FixMapping, 5);
        assert_eq!(a.nav.overlay(), Some(&Overlay::Say));
        for ch in "ok starting".chars() {
            a.type_char(ch);
        }
        assert_eq!(
            a.handle(Action::Open, 5),
            Some(Command::Room(RoomCommand::Say { text: "ok starting".into() }))
        );
        assert_eq!(a.nav.overlay(), Some(&Overlay::Say), "the field stays for the next line");
        assert!(a.text(Field::Say).is_empty());
    }

    #[test]
    fn a_typed_reaction_word_is_the_reaction() {
        let mut a = in_room();
        a.handle(Action::FixMapping, 5);
        for ch in "*laughs*".chars() {
            a.type_char(ch);
        }
        assert_eq!(
            a.handle(Action::Open, 5),
            Some(Command::Room(RoomCommand::React { reaction: Reaction::Laugh }))
        );
    }

    #[test]
    fn the_cursor_walks_the_other_members_then_join_and_shift_m_mutes_the_one_under_it() {
        let mut a = in_room();
        a.apply(Event::RoomRecent(vec![recent("room-2")]));
        a.apply(Event::RoomMembers(vec![
            member("Ada", true, false),
            member("Mika", false, false),
            member("Ren", false, false),
        ]));
        assert_eq!(a.together_rows(), 4, "Mika, Ren, Join, one room");
        assert_eq!(a.selected_member().map(|m| m.name.as_str()), Some("Mika"));
        assert_eq!(a.handle(Action::Open, 5), None, "Enter opens nothing on a member");
        assert_eq!(a.nav.overlay(), None);
        a.handle(Action::Down, 5);
        assert_eq!(
            a.handle(Action::MarkAllPrevious, 5),
            Some(Command::Room(RoomCommand::Mute { id: "Ren".into(), muted: true }))
        );
        a.apply(Event::RoomMembers(vec![
            member("Ada", true, false),
            member("Mika", false, false),
            member("Ren", false, true),
        ]));
        assert_eq!(
            a.handle(Action::MarkAllPrevious, 5),
            Some(Command::Room(RoomCommand::Mute { id: "Ren".into(), muted: false }))
        );
        a.handle(Action::Down, 5);
        assert_eq!(a.handle(Action::MarkAllPrevious, 5), None, "the Join line has no member");
        a.handle(Action::Down, 5);
        assert_eq!(a.selected_room().map(|r| r.room_id.as_str()), Some("room-2"));

        // Someone leaves: the cursor stays on a row that exists.
        a.apply(Event::RoomMembers(vec![member("Ada", true, false)]));
        assert_eq!(a.cursors.together.selected, 1);
        assert_eq!(a.selected_room().map(|r| r.room_id.as_str()), Some("room-2"));
    }

    #[test]
    fn i_turns_hide_chat_over_and_m_says_chat_is_hidden_while_it_is_on() {
        let mut a = in_room();
        assert_eq!(
            a.handle(Action::ToggleSynopsis, 5),
            Some(Command::Room(RoomCommand::HideChat { hidden: true }))
        );
        a.apply(Event::RoomChatState(chat_state(true)));
        assert_eq!(
            a.handle(Action::ToggleSynopsis, 5),
            Some(Command::Room(RoomCommand::HideChat { hidden: false }))
        );
        a.handle(Action::FixMapping, 5);
        assert_eq!(a.nav.overlay(), None, "no field while chat is hidden");
    }

    #[test]
    fn a_line_that_never_came_back_returns_to_the_open_field_and_leaving_forgets_the_chat() {
        let mut a = in_room();
        a.handle(Action::FixMapping, 5);
        a.apply(Event::RoomChat(RoomConversation {
            failed: Some("anyone there".into()),
            ..Default::default()
        }));
        assert_eq!(a.text(Field::Say), "anyone there");
        a.apply(Event::RoomLeft { why: RoomLeftWhy::Left });
        assert!(a.room_chat.failed.is_none() && a.room_chat.lines.is_empty());
        assert_eq!(a.nav.overlay(), None, "the Say line goes with the room");
    }
}
