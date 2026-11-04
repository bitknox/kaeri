//! Together: the watch party this terminal is in, a line to join another, and the last rooms.

use kaeri_i18n::t;
use kaeri_model::{
    command::Command,
    party::{
        PartyChatRefusal, PartyCommand, PartyMember, PartyRecentRoom, PartyTarget, Reaction,
    },
};

use super::{App, Field, alert, info};
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

    /// The rows the cursor walks: the party's other members, the Join line, then the recent
    /// rooms.
    pub(super) fn together_rows(&self) -> usize {
        self.others().count() + 1 + self.party_recent.len()
    }

    /// The party's members other than this device's, in the order the rows list them. They are
    /// rows so `M` can mute one.
    pub fn others(&self) -> impl Iterator<Item = &PartyMember> {
        self.party_members.iter().filter(|m| !m.you && self.party.is_some())
    }

    /// The other member under the cursor.
    pub fn selected_member(&self) -> Option<&PartyMember> {
        self.others().nth(self.cursors.together.selected)
    }

    /// The row of the Join line.
    pub fn join_row(&self) -> usize {
        self.others().count()
    }

    /// The recent room under the cursor; `None` on the Join line or a member.
    pub fn selected_room(&self) -> Option<&PartyRecentRoom> {
        self.party_recent.get(self.cursors.together.selected.checked_sub(self.join_row() + 1)?)
    }

    /// Whether this member's lines and reactions are muted for the party.
    pub fn is_muted(&self, name: &str) -> bool {
        self.party_chat_state.as_ref().is_some_and(|s| s.muted.iter().any(|m| m == name))
    }

    /// Whether Hide chat is on.
    pub fn chat_hidden(&self) -> bool {
        self.party_chat_state.as_ref().is_some_and(|s| s.hidden)
    }

    /// The keys of the section. `Some` when one was taken, with what it asks of the engine.
    ///
    /// The section reads the keys the rest of the app already has for its own screens, the way
    /// Downloads does: `r` ready, `c` copy the link, `s` start, `x` leave. They are matched
    /// before the title commands, which would otherwise answer that nothing is selected.
    pub(super) fn together_key(&mut self, action: Action) -> Option<Option<Command>> {
        if !self.in_together_stage() {
            return None;
        }
        let party = self.party.is_some();
        Some(match action {
            Action::Open => self.open_together_row(),
            Action::Refresh if party => self.party_toggle_ready(),
            Action::ClearCompleted if party => self.party_copy_link(),
            Action::ShowSources if party => self.party_start(),
            Action::HideFromContinue if party => Some(Command::PartyLeave),
            Action::FixMapping if party => self.open_say(),
            Action::MarkAllPrevious if party => self.party_mute(),
            Action::ToggleSynopsis if party => {
                Some(Command::Party(PartyCommand::HideChat { hidden: !self.chat_hidden() }))
            }
            _ => return None,
        })
    }

    /// Enter: the Join line asks for a link or code; a recent room is joined again. A member's
    /// row opens nothing.
    fn open_together_row(&mut self) -> Option<Command> {
        if self.selected_member().is_some() {
            return None;
        }
        match self.selected_room() {
            None => {
                self.field_mut(Field::PartyLink).clear();
                self.nav.open_overlay(Overlay::JoinParty);
                None
            }
            Some(room) => {
                if self.party.as_ref().is_some_and(|p| p.room == room.room) {
                    return None;
                }
                let (server, room) = (room.server.clone(), room.room.clone());
                Some(Command::Party(PartyCommand::Lobby {
                    server,
                    room,
                    name: None,
                    pick: None,
                }))
            }
        }
    }

    /// Enter on a typed link or code: the lobby it names.
    pub(super) fn submit_party_link(&mut self) -> Option<Command> {
        let text: String =
            self.text(Field::PartyLink).chars().filter(|c| !c.is_whitespace()).collect();
        if text.is_empty() {
            return None;
        }
        let Some(target) = PartyTarget::parse(&text) else {
            self.push_toast(alert(t::tui_together_not_a_link()));
            return None;
        };
        self.nav.close_overlay();
        self.field_mut(Field::PartyLink).clear();
        Some(Command::Party(PartyCommand::Lobby {
            server: target.server,
            room: target.room,
            name: None,
            pick: None,
        }))
    }

    /// `r`: ready, or not ready.
    fn party_toggle_ready(&mut self) -> Option<Command> {
        let ready = !self.party.as_ref()?.ready;
        Some(Command::PartyReady { ready })
    }

    /// `c`: the invite link to the terminal's clipboard.
    fn party_copy_link(&mut self) -> Option<Command> {
        let link =
            self.party.as_ref().and_then(|p| p.link.clone().or_else(|| p.invite.clone()))?;
        self.clipboard = Some(link);
        self.push_toast(info(t::party_copy_invite()));
        None
    }

    /// `s`: start now, for whoever may.
    fn party_start(&mut self) -> Option<Command> {
        if self.party.as_ref()?.can_start_anyway {
            return Some(Command::Party(PartyCommand::StartAnyway));
        }
        None
    }

    /// `M` on a member's row: mute them for the party, or unmute them.
    fn party_mute(&mut self) -> Option<Command> {
        let name = self.selected_member()?.name.clone();
        let muted = !self.is_muted(&name);
        Some(Command::Party(PartyCommand::Mute { name, muted }))
    }

    /// `m`: the field for saying something, unless the room's server has no chat or Hide chat
    /// is on.
    fn open_say(&mut self) -> Option<Command> {
        if self.party_chat_state.as_ref().is_some_and(|s| !s.allowed) {
            self.push_toast(info(t::party_chat_off()));
            return None;
        }
        if self.chat_hidden() {
            self.push_toast(info(t::party_chat_hidden()));
            return None;
        }
        self.nav.open_overlay(Overlay::Say);
        None
    }

    /// Enter in the Say field: the line goes to the party and the field stays open for the
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
        Some(Command::Party(match reaction {
            Some(reaction) => PartyCommand::React { reaction },
            None => PartyCommand::Say { text },
        }))
    }

    /// The conversation changed. A line the server never echoed comes back into the field,
    /// once, when the field is open and empty.
    pub(super) fn set_party_chat(
        &mut self,
        conversation: kaeri_model::party::PartyConversation,
    ) {
        let failed = conversation.failed.clone();
        let fresh = failed != self.party_chat.failed;
        self.party_chat = conversation;
        if let Some(text) = failed
            && fresh
            && self.nav.overlay() == Some(&Overlay::Say)
            && self.text(Field::Say).is_empty()
        {
            self.set_text(Field::Say, text);
        }
    }

    /// The engine turned a line or a reaction down.
    pub(super) fn chat_refused(&mut self, refusal: PartyChatRefusal) {
        match refusal {
            PartyChatRefusal::Off => self.push_toast(info(t::party_chat_off())),
            PartyChatRefusal::Wait { seconds } => {
                self.push_toast(info(t::party_chat_wait(i64::from(seconds))));
            }
            PartyChatRefusal::NotPlaying => {}
        }
    }

    /// Take the lobby's word for how the party stands.
    pub(super) fn set_party(&mut self, view: kaeri_model::party::PartyView) {
        self.party = Some(view);
    }

    /// The party is over.
    pub(super) fn end_party(&mut self) {
        self.party = None;
        self.party_members.clear();
        self.keep_together_cursor();
        self.party_chat = Default::default();
        self.party_chat_state = None;
    }

    /// The last rooms arrived: keep the cursor on a row that exists.
    pub(super) fn set_recent_rooms(&mut self, rooms: Vec<PartyRecentRoom>) {
        self.party_recent = rooms;
        self.keep_together_cursor();
    }

    /// The members changed: keep the cursor on a row that exists.
    pub(super) fn set_party_members(&mut self, members: Vec<PartyMember>) {
        self.party_members = members;
        self.keep_together_cursor();
    }

    fn keep_together_cursor(&mut self) {
        let last = self.together_rows() - 1;
        self.cursors.together.selected = self.cursors.together.selected.min(last);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::fixtures::*;
    use kaeri_core::ids::AnilistId;
    use kaeri_model::{
        event::Event,
        party::{PartyConnection, PartyMember, PartyView},
    };

    fn view(can_start_anyway: bool) -> PartyView {
        PartyView {
            server: "party.example:8999".into(),
            room: "kite-7qm4-xr9d".into(),
            name: "Ada".into(),
            connection: PartyConnection::Connected,
            encrypted: true,
            ready: false,
            episode: None,
            invite: None,
            motd: None,
            latency_ms: None,
            room_episode: None,
            next_episode: None,
            starter: None,
            starter_present: false,
            can_start_anyway,
            code: Some("kite-7qm4-xr9d".into()),
            link: Some(
                "https://kaeri.app/party#s=party.example%3A8999&r=kite-7qm4-xr9d".into(),
            ),
            playing: false,
            starting_in: None,
            sources: None,
            guesses: Vec::new(),
            looking: false,
            moves_on: false,
        }
    }

    fn room(name: &str) -> PartyRecentRoom {
        PartyRecentRoom {
            server: "party.example:8999".into(),
            room: name.into(),
            id: AnilistId::new(104_321),
            title: "Natsukaze Relay".into(),
            episode: "8".into(),
            names: vec!["Mika".into()],
            last_at: 0,
            cover_url: None,
        }
    }

    #[test]
    fn zero_is_the_tenth_section_and_asks_for_the_rooms() {
        let mut a = app();
        assert_eq!(a.key_for(Action::JumpSection(10)), "0");
        let task = a.handle(Action::JumpSection(10), 5);
        assert_eq!(a.nav.section(), Section::Together);
        assert_eq!(task, Some(Command::Party(PartyCommand::LoadRecent)));
    }

    #[test]
    fn a_typed_code_or_link_joins_its_lobby_and_anything_else_is_refused() {
        let mut a = app();
        a.handle(Action::JumpSection(10), 5);
        a.nav.focus_stage();
        a.handle(Action::Open, 5);
        assert_eq!(a.nav.overlay(), Some(&Overlay::JoinParty));

        for ch in "nonsense".chars() {
            a.type_char(ch);
        }
        assert_eq!(a.handle(Action::Open, 5), None);
        assert_eq!(a.nav.overlay(), Some(&Overlay::JoinParty), "kept, to be corrected");

        a.dismiss_field();
        a.handle(Action::Open, 5);
        a.paste("https://kaeri.app/party#s=party.example%3A8999&r=kite-7qm4-xr9d&e=9");
        let task = a.handle(Action::Open, 5);
        assert_eq!(
            task,
            Some(Command::Party(PartyCommand::Lobby {
                server: "party.example:8999".into(),
                room: "kite-7qm4-xr9d".into(),
                name: None,
                pick: None,
            }))
        );
        assert!(a.nav.overlay().is_none());
        assert!(a.text(Field::PartyLink).is_empty(), "the spent link is not kept");
    }

    #[test]
    fn a_recent_room_is_joined_again_from_its_row() {
        let mut a = app();
        a.handle(Action::JumpSection(10), 5);
        a.nav.focus_stage();
        a.apply(Event::PartyRecent(vec![room("kite-7qm4-xr9d"), room("ember-2hv8-kd4p")]));
        a.handle(Action::Down, 5);
        a.handle(Action::Down, 5);
        assert_eq!(a.selected_room().map(|r| r.room.as_str()), Some("ember-2hv8-kd4p"));
        let task = a.handle(Action::Open, 5);
        assert!(matches!(
            task,
            Some(Command::Party(PartyCommand::Lobby { room, .. })) if room == "ember-2hv8-kd4p"
        ));
    }

    #[test]
    fn the_partys_keys_work_only_in_a_party_and_start_only_for_who_may() {
        let mut a = app();
        a.handle(Action::JumpSection(10), 5);
        a.nav.focus_stage();
        assert_eq!(a.handle(Action::HideFromContinue, 5), None);
        assert!(a.take_clipboard().is_none());

        a.apply(Event::PartyJoined(view(false)));
        a.apply(Event::PartyMembers(vec![PartyMember {
            name: "Ada".into(),
            ready: Some(false),
            file: None,
            duration: None,
            file_matches: None,
            you: true,
        }]));
        assert_eq!(a.handle(Action::Refresh, 5), Some(Command::PartyReady { ready: true }));
        assert_eq!(a.handle(Action::ShowSources, 5), None, "not the starter");
        assert_eq!(a.handle(Action::ClearCompleted, 5), None);
        assert_eq!(
            a.take_clipboard().as_deref(),
            Some("https://kaeri.app/party#s=party.example%3A8999&r=kite-7qm4-xr9d")
        );

        a.apply(Event::PartyState(view(true)));
        assert_eq!(
            a.handle(Action::ShowSources, 5),
            Some(Command::Party(PartyCommand::StartAnyway))
        );
        assert_eq!(a.handle(Action::HideFromContinue, 5), Some(Command::PartyLeave));
        a.apply(Event::PartyLeft { reason: None });
        assert!(a.party.is_none() && a.party_members.is_empty());
    }

    fn in_party() -> App {
        let mut a = app();
        a.apply(Event::PartyJoined(view(false)));
        a.handle(Action::JumpSection(10), 5);
        a.nav.focus_stage();
        a
    }

    #[test]
    fn m_opens_the_say_field_and_enter_sends_and_keeps_it_open() {
        let mut a = in_party();
        a.handle(Action::FixMapping, 5);
        assert_eq!(a.nav.overlay(), Some(&Overlay::Say));
        for ch in "ok starting".chars() {
            a.type_char(ch);
        }
        assert_eq!(
            a.handle(Action::Open, 5),
            Some(Command::Party(PartyCommand::Say { text: "ok starting".into() }))
        );
        assert_eq!(a.nav.overlay(), Some(&Overlay::Say), "the field stays for the next line");
        assert!(a.text(Field::Say).is_empty());
    }

    #[test]
    fn a_typed_reaction_word_is_the_reaction() {
        use kaeri_model::party::Reaction;
        let mut a = in_party();
        a.handle(Action::FixMapping, 5);
        for ch in "*laughs*".chars() {
            a.type_char(ch);
        }
        assert_eq!(
            a.handle(Action::Open, 5),
            Some(Command::Party(PartyCommand::React { reaction: Reaction::Laugh }))
        );
    }

    #[test]
    fn a_server_without_chat_says_so_instead_of_opening_the_field() {
        use kaeri_model::party::PartyChatState;
        let mut a = in_party();
        a.apply(Event::PartyChatState(PartyChatState {
            allowed: false,
            max_length: 150,
            muted: Vec::new(),
            wait_seconds: 0,
            react_wait_seconds: 0,
            hidden: false,
        }));
        a.handle(Action::FixMapping, 5);
        assert_eq!(a.nav.overlay(), None);
    }

    fn member(name: &str, you: bool) -> PartyMember {
        PartyMember {
            name: name.into(),
            ready: Some(false),
            file: None,
            duration: None,
            file_matches: None,
            you,
        }
    }

    fn chat_state(muted: &[&str], hidden: bool) -> kaeri_model::party::PartyChatState {
        kaeri_model::party::PartyChatState {
            allowed: true,
            max_length: 150,
            muted: muted.iter().map(|m| (*m).to_owned()).collect(),
            wait_seconds: 0,
            react_wait_seconds: 0,
            hidden,
        }
    }

    #[test]
    fn the_cursor_walks_the_other_members_then_join_and_shift_m_mutes_the_one_under_it() {
        let mut a = in_party();
        a.apply(Event::PartyRecent(vec![room("ember-2hv8-kd4p")]));
        a.apply(Event::PartyMembers(vec![
            member("Ada", true),
            member("Mika", false),
            member("Ren", false),
        ]));
        assert_eq!(a.together_rows(), 4, "Mika, Ren, Join, one room");
        assert_eq!(a.selected_member().map(|m| m.name.as_str()), Some("Mika"));
        assert_eq!(a.handle(Action::Open, 5), None, "Enter opens nothing on a member");
        assert_eq!(a.nav.overlay(), None);
        a.handle(Action::Down, 5);
        assert_eq!(
            a.handle(Action::MarkAllPrevious, 5),
            Some(Command::Party(PartyCommand::Mute { name: "Ren".into(), muted: true }))
        );
        a.apply(Event::PartyChatState(chat_state(&["Ren"], false)));
        assert_eq!(
            a.handle(Action::MarkAllPrevious, 5),
            Some(Command::Party(PartyCommand::Mute { name: "Ren".into(), muted: false }))
        );
        a.handle(Action::Down, 5);
        assert_eq!(a.handle(Action::MarkAllPrevious, 5), None, "the Join line has no member");
        a.handle(Action::Down, 5);
        assert_eq!(a.selected_room().map(|r| r.room.as_str()), Some("ember-2hv8-kd4p"));

        // Someone leaves: the cursor stays on a row that exists.
        a.apply(Event::PartyMembers(vec![member("Ada", true)]));
        assert_eq!(a.cursors.together.selected, 1);
        assert_eq!(a.selected_room().map(|r| r.room.as_str()), Some("ember-2hv8-kd4p"));
    }

    #[test]
    fn i_turns_hide_chat_over_and_m_says_chat_is_hidden_while_it_is_on() {
        let mut a = in_party();
        assert_eq!(
            a.handle(Action::ToggleSynopsis, 5),
            Some(Command::Party(PartyCommand::HideChat { hidden: true }))
        );
        a.apply(Event::PartyChatState(chat_state(&[], true)));
        assert_eq!(
            a.handle(Action::ToggleSynopsis, 5),
            Some(Command::Party(PartyCommand::HideChat { hidden: false }))
        );
        a.handle(Action::FixMapping, 5);
        assert_eq!(a.nav.overlay(), None, "no field while chat is hidden");
    }

    #[test]
    fn a_line_that_never_came_back_returns_to_the_open_field_and_leaving_forgets_the_chat() {
        use kaeri_model::party::PartyConversation;
        let mut a = in_party();
        a.handle(Action::FixMapping, 5);
        a.apply(Event::PartyChat(PartyConversation {
            failed: Some("anyone there".into()),
            ..Default::default()
        }));
        assert_eq!(a.text(Field::Say), "anyone there");
        a.apply(Event::PartyLeft { reason: None });
        assert!(a.party_chat.failed.is_none() && a.party_chat.lines.is_empty());
    }
}
