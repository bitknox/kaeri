//! What a remote shows: the TV's screen, and what is playing on it.
//!
//! [`StateFolder`] reads it off what the TV already has, the engine's own [`Event`]s and the
//! hosted player's [`PlayerReport`]s, so the TV app keeps nothing of its own for its remotes. It
//! says which changes are worth a message: every change of what is playing, of pause, speed,
//! volume, tracks or skips, and the playhead only when it jumps or has moved [`POSITION_STEP`].
//! Between those, a phone moves its own playhead on its own clock, from `position`, `paused` and
//! `speed`. That keeps a playing episode to a message every few seconds per phone, and none while
//! paused.

use kaeri_core::ids::AnilistId;
use kaeri_model::{
    event::Event,
    hosted::{PlayerReport, PlayerUi},
    playback::SkipKind,
};
use serde::{Deserialize, Serialize};

use crate::command::Track;

/// How far the playhead moves, in media seconds, before a phone is told again.
pub const POSITION_STEP: f64 = 5.0;

/// The whole of what a remote shows.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RemoteState {
    /// Counts up with every state sent, so a phone can tell a stale one.
    pub rev: u64,
    pub screen: Screen,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub now_playing: Option<NowPlaying>,
    /// The watch party the TV is in, while it is in one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub party: Option<Box<PartyInfo>>,
}

/// The watch party the TV is in: what a phone shows on the TV's remote page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartyInfo {
    pub room: String,
    /// The room's code, when it is one on the suggested server.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// The https invite, which the phone can share on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    /// The title and episode the room is set to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub episode: Option<String>,
    pub ready: bool,
    pub playing: bool,
    /// The last [`MESSAGES`] of the room's conversation, the newest last, without the muted
    /// members' lines. In memory on the TV for the party.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub messages: Vec<PartyLine>,
    /// The members in the room beside the TV, for a phone's Mute.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub members: Vec<String>,
    /// The members muted on the TV.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub muted: Vec<String>,
    /// The room's server has chat turned off: a phone hides its field.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub chat_off: bool,
    /// The most characters in a message, when the TV knows; 0 is 150.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub chat_max: u32,
}

/// How many lines of the conversation a remote is told.
pub const MESSAGES: usize = 20;

fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// One line of the party's conversation, for a phone's remote page. A typed line has `text`; a
/// reaction has `reaction`, one of `laughs`, `gasps`, `claps` or `cries`, and `at`, the seconds
/// into the episode, and `with`, the others who reacted the same at the same moment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartyLine {
    pub from: String,
    /// Said as the TV.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub you: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reaction: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub with: Vec<String>,
}

/// What the TV has on screen, as its front end says: see [`StateFolder::screen`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Screen {
    #[default]
    Home,
    /// A search, with the text in its field: what a phone's field mirrors.
    Search {
        query: String,
    },
    Title {
        id: AnilistId,
    },
    Player,
    Settings,
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NowPlaying {
    pub id: AnilistId,
    pub title: String,
    pub episode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub episode_title: Option<String>,
    /// Still finding the stream: nothing has played yet.
    pub loading: bool,
    pub position: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    pub paused: bool,
    pub speed: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tracks: Vec<TrackInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skip: Option<SkipOffer>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackInfo {
    pub id: i64,
    pub kind: Track,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkipOffer {
    pub kind: SkipPart,
    pub to: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipPart {
    Opening,
    Ending,
}

impl SkipPart {
    /// The part a phone is told of, for a skip the TV offers. A recap is not told: a phone on
    /// protocol 1 reads only these two, and a `skip` it cannot read would cost it the whole
    /// state. The TV shows its own Skip recap button.
    pub const fn of(kind: SkipKind) -> Option<Self> {
        match kind {
            SkipKind::Opening => Some(Self::Opening),
            SkipKind::Ending => Some(Self::Ending),
            SkipKind::Recap => None,
        }
    }
}

fn party_line(line: &kaeri_model::party::PartyChatLine) -> PartyLine {
    use kaeri_model::party::{PartyChatBody, Reaction};
    let mut out = PartyLine {
        from: line.from.clone(),
        you: line.you,
        text: None,
        reaction: None,
        at: None,
        with: Vec::new(),
    };
    match &line.body {
        PartyChatBody::Said { text } => out.text = Some(text.clone()),
        PartyChatBody::Reacted { reaction, at, with } => {
            let word = match reaction {
                Reaction::Laugh => "laughs",
                Reaction::Gasp => "gasps",
                Reaction::Clap => "claps",
                Reaction::Cry => "cries",
            };
            out.reaction = Some(word.to_owned());
            out.at = Some(*at);
            out.with.clone_from(with);
        }
    }
    out
}

/// [`RemoteState`], kept up from what the TV's engine and player say.
#[derive(Debug, Default)]
pub struct StateFolder {
    state: RemoteState,
    /// What was playing before the play just staged, for when that play finds nothing and the
    /// old one keeps playing ([`Event::PlaybackKept`]).
    replaced: Option<NowPlaying>,
    /// The playhead and pause last told, which the next position is weighed against.
    told: Option<(f64, bool)>,
    /// The TV's own volume, once its front end has said it: what a remote's volume shows in
    /// place of the player's.
    system_volume: Option<f64>,
}

impl StateFolder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn state(&self) -> &RemoteState {
        &self.state
    }

    /// The state to send, counted: call it after a change this folder said was worth sending.
    pub fn publish(&mut self) -> RemoteState {
        self.state.rev += 1;
        if let Some(now) = &self.state.now_playing {
            self.told = Some((now.position, now.paused));
        }
        self.state.clone()
    }

    /// What the TV front end has on screen now. True when it changed.
    pub fn screen(&mut self, screen: Screen) -> bool {
        if self.state.screen == screen {
            return false;
        }
        self.state.screen = screen;
        true
    }

    /// The TV's own volume, out of 100, as its front end reads it from the system. From then on
    /// it is the volume a remote shows, not the player's. True when a remote should hear of it.
    pub fn volume(&mut self, percent: f64) -> bool {
        self.system_volume = Some(percent);
        self.with_now(|now| replace(&mut now.volume, Some(percent)))
    }

    /// An engine event, as the TV front end reads it. True when a remote should hear of it.
    pub fn event(&mut self, event: &Event) -> bool {
        match event {
            // A cast's card is folded as a TV's state is: the play staged for the TV it casts to.
            Event::PlaybackStaged { id, title, episode, episode_title, speed, .. }
            | Event::CastStaged { id, title, episode, episode_title, speed } => {
                let carried = self
                    .system_volume
                    .or_else(|| self.state.now_playing.as_ref().and_then(|now| now.volume));
                self.replaced = self.state.now_playing.take();
                self.told = None;
                self.state.now_playing = Some(NowPlaying {
                    id: *id,
                    title: title.clone(),
                    episode: episode.clone(),
                    episode_title: episode_title.clone(),
                    loading: true,
                    position: 0.0,
                    duration: None,
                    paused: false,
                    speed: *speed,
                    volume: carried,
                    tracks: Vec::new(),
                    skip: None,
                });
                true
            }
            Event::PlaybackKept => {
                self.state.now_playing = self.replaced.take();
                self.told = None;
                true
            }
            Event::PlaybackEnded { rolling_on, .. } => {
                if *rolling_on {
                    return false;
                }
                self.replaced = None;
                self.told = None;
                self.state.now_playing.take().is_some()
            }
            Event::Playback { position, duration, paused } => {
                let told = self.told;
                let Some(now) = &mut self.state.now_playing else { return false };
                let was_loading = std::mem::replace(&mut now.loading, false);
                let duration_changed = now.duration != *duration;
                now.position = *position;
                now.duration = *duration;
                now.paused = *paused;
                was_loading || duration_changed || Self::moved(told, *position, *paused)
            }
            Event::Resumed { position } => self.with_now(|now| {
                now.position = *position;
                true
            }),
            Event::PlaybackSpeed(speed) => self.with_now(|now| replace(&mut now.speed, *speed)),
            Event::PlaybackVolume(volume) if self.system_volume.is_none() => {
                self.with_now(|now| replace(&mut now.volume, Some(*volume)))
            }
            Event::SkipAvailable { kind, to } => self.with_now(|now| {
                let offer = SkipPart::of(*kind).map(|kind| SkipOffer { kind, to: *to });
                replace(&mut now.skip, offer)
            }),
            Event::SkipCleared => self.with_now(|now| replace(&mut now.skip, None)),
            Event::PlayerUi { ui: PlayerUi::Episode { number, title, show, .. }, .. } => self
                .with_now(|now| {
                    let a = replace(&mut now.episode, number.clone());
                    let b = replace(&mut now.episode_title, title.clone());
                    let c = replace(&mut now.title, show.clone());
                    a || b || c
                }),
            Event::PartyJoined(view) | Event::PartyState(view) => {
                let kept = self.state.party.take();
                let mut info = PartyInfo {
                    room: view.room.clone(),
                    code: view.code.clone(),
                    link: view.link.clone(),
                    title: view.room_episode.as_ref().map(|r| r.title.clone()),
                    episode: view.room_episode.as_ref().map(|r| r.episode.clone()),
                    ready: view.ready,
                    playing: view.playing,
                    messages: Vec::new(),
                    members: Vec::new(),
                    muted: Vec::new(),
                    chat_off: false,
                    chat_max: 0,
                };
                // The conversation, members and chat state come in their own events.
                if let Some(kept) = &kept {
                    info.messages.clone_from(&kept.messages);
                    info.members.clone_from(&kept.members);
                    info.muted.clone_from(&kept.muted);
                    (info.chat_off, info.chat_max) = (kept.chat_off, kept.chat_max);
                }
                let changed = kept.as_deref() != Some(&info);
                self.state.party = Some(Box::new(info));
                changed
            }
            Event::PartyMembers(members) => self.with_party(|info| {
                let names: Vec<String> =
                    members.iter().filter(|m| !m.you).map(|m| m.name.clone()).collect();
                replace(&mut info.members, names)
            }),
            Event::PartyChatState(chat) => self.with_party(|info| {
                let off = !chat.allowed;
                let a = replace(&mut info.chat_off, off);
                let b = replace(&mut info.chat_max, chat.max_length);
                let c = replace(&mut info.muted, chat.muted.clone());
                a || b || c
            }),
            Event::PartyChat(conversation) => self.with_party(|info| {
                let lines: Vec<PartyLine> = conversation
                    .lines
                    .iter()
                    .filter(|line| !line.muted)
                    .map(party_line)
                    .collect();
                let from = lines.len().saturating_sub(MESSAGES);
                replace(&mut info.messages, lines[from..].to_vec())
            }),
            Event::PartyLeft { .. } => self.state.party.take().is_some(),
            _ => false,
        }
    }

    fn with_party(&mut self, change: impl FnOnce(&mut PartyInfo) -> bool) -> bool {
        self.state.party.as_deref_mut().is_some_and(change)
    }

    /// What the TV's hosted player reported, as the front end hands it to the engine: only its
    /// track list matters here. True when it changed.
    pub fn report(&mut self, report: &PlayerReport) -> bool {
        let PlayerReport::Property { name, json } = report else { return false };
        if name != "track-list" {
            return false;
        }
        let tracks = parse_track_list(json);
        self.with_now(|now| replace(&mut now.tracks, tracks))
    }

    fn with_now(&mut self, change: impl FnOnce(&mut NowPlaying) -> bool) -> bool {
        self.state.now_playing.as_mut().is_some_and(change)
    }

    fn moved(told: Option<(f64, bool)>, position: f64, paused: bool) -> bool {
        let Some((at, was_paused)) = told else { return true };
        paused != was_paused || position < at || position - at >= POSITION_STEP
    }
}

fn replace<T: PartialEq>(slot: &mut T, value: T) -> bool {
    if *slot == value {
        return false;
    }
    *slot = value;
    true
}

/// The audio and subtitle tracks in mpv's `track-list`, as the hosted player reports it.
fn parse_track_list(json: &str) -> Vec<TrackInfo> {
    let Ok(serde_json::Value::Array(tracks)) = serde_json::from_str(json) else {
        return Vec::new();
    };
    tracks
        .iter()
        .filter_map(|track| {
            let kind = match track.get("type")?.as_str()? {
                "audio" => Track::Audio,
                "sub" => Track::Subtitle,
                _ => return None,
            };
            let text = |key| track.get(key).and_then(|v| v.as_str()).map(str::to_owned);
            Some(TrackInfo {
                id: track.get("id")?.as_i64()?,
                kind,
                title: text("title"),
                language: text("lang"),
                selected: track.get("selected").and_then(|v| v.as_bool()).unwrap_or(false),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn staged() -> Event {
        Event::PlaybackStaged {
            id: AnilistId::new(42),
            title: "Harbour Lights".into(),
            episode: "3".into(),
            episode_title: None,
            speed: 1.0,
            resume: None,
        }
    }

    fn at(position: f64, paused: bool) -> Event {
        Event::Playback { position, duration: Some(1440.0), paused }
    }

    #[test]
    fn the_playhead_is_told_when_it_jumps_not_every_tick() {
        let mut folder = StateFolder::new();
        assert!(folder.event(&staged()));
        folder.publish();
        assert!(folder.event(&at(1.0, false)), "the first position ends the loading");
        folder.publish();
        assert!(!folder.event(&at(2.0, false)));
        assert!(!folder.event(&at(5.5, false)));
        assert!(folder.event(&at(6.0, false)), "five seconds on");
        folder.publish();
        assert!(folder.event(&at(6.5, true)), "paused");
        folder.publish();
        assert!(folder.event(&at(2.0, true)), "a seek back");
        assert_eq!(folder.state().now_playing.as_ref().unwrap().position, 2.0);
    }

    #[test]
    fn an_ending_that_rolls_on_keeps_now_playing() {
        let mut folder = StateFolder::new();
        folder.event(&staged());
        let ended = |rolling_on| Event::PlaybackEnded {
            watched: true,
            ending: kaeri_model::playback::Ending::Complete { next: None },
            rolling_on,
            failure: None,
        };
        assert!(!folder.event(&ended(true)));
        assert!(folder.state().now_playing.is_some());
        assert!(folder.event(&ended(false)));
        assert!(folder.state().now_playing.is_none());
    }

    #[test]
    fn the_tvs_own_volume_is_shown_over_the_players() {
        let mut folder = StateFolder::new();
        folder.volume(30.0);
        folder.event(&staged());
        assert_eq!(folder.state().now_playing.as_ref().unwrap().volume, Some(30.0));
        assert!(!folder.event(&Event::PlaybackVolume(100.0)));
        assert!(folder.volume(35.0));
        assert_eq!(folder.state().now_playing.as_ref().unwrap().volume, Some(35.0));
    }

    #[test]
    fn a_kept_playback_comes_back() {
        let mut folder = StateFolder::new();
        folder.event(&staged());
        folder.event(&Event::PlaybackStaged {
            id: AnilistId::new(43),
            title: "Paper Orchard".into(),
            episode: "1".into(),
            episode_title: None,
            speed: 1.0,
            resume: None,
        });
        folder.event(&Event::PlaybackKept);
        assert_eq!(folder.state().now_playing.as_ref().unwrap().id, AnilistId::new(42));
    }

    #[test]
    fn tracks_come_from_the_players_track_list() {
        let mut folder = StateFolder::new();
        folder.event(&staged());
        let json = r#"[{"id":1,"type":"video"},{"id":1,"type":"audio","lang":"jpn","selected":true},
            {"id":2,"type":"sub","lang":"eng","title":"Full","selected":false}]"#;
        let report = PlayerReport::Property { name: "track-list".into(), json: json.into() };
        assert!(folder.report(&report));
        assert!(!folder.report(&report), "the same list again is no change");
        let tracks = &folder.state().now_playing.as_ref().unwrap().tracks;
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[1].kind, Track::Subtitle);
        assert_eq!(tracks[1].title.as_deref(), Some("Full"));
    }

    fn party_view() -> kaeri_model::party::PartyView {
        use kaeri_model::party::{PartyConnection, PartyView};
        PartyView {
            server: "party.example:8999".into(),
            room: "kite-7qm4-xr9d".into(),
            name: "Living room TV".into(),
            connection: PartyConnection::Connected,
            encrypted: false,
            ready: false,
            episode: None,
            invite: None,
            motd: None,
            latency_ms: None,
            room_episode: None,
            next_episode: None,
            starter: None,
            starter_present: false,
            can_start_anyway: false,
            code: None,
            link: None,
            playing: false,
            starting_in: None,
            sources: None,
            guesses: Vec::new(),
            looking: false,
            moves_on: false,
        }
    }

    fn line(id: u64, from: &str, text: &str, muted: bool) -> kaeri_model::party::PartyChatLine {
        use kaeri_model::party::{PartyChatBody, PartyChatLine};
        PartyChatLine {
            id,
            from: from.into(),
            you: false,
            body: PartyChatBody::Said { text: text.into() },
            muted,
        }
    }

    #[test]
    fn the_party_carries_the_last_twenty_unmuted_lines_and_keeps_them_across_states() {
        use kaeri_model::party::{PartyChatBody, PartyChatState, PartyConversation, Reaction};
        let mut folder = StateFolder::new();
        assert!(
            !folder.event(&Event::PartyChat(PartyConversation::default())),
            "no party, nothing to tell"
        );
        assert!(folder.event(&Event::PartyJoined(party_view())));
        let mut lines: Vec<_> =
            (1..=25).map(|i| line(i, "Mika", &format!("line {i}"), false)).collect();
        lines.push(line(26, "Ren", "muted words", true));
        lines.push(kaeri_model::party::PartyChatLine {
            id: 27,
            from: "Sol".into(),
            you: false,
            body: PartyChatBody::Reacted {
                reaction: Reaction::Laugh,
                at: 724.0,
                with: vec!["Ren".into()],
            },
            muted: false,
        });
        let conversation = PartyConversation { lines, ..PartyConversation::default() };
        assert!(folder.event(&Event::PartyChat(conversation.clone())));
        assert!(!folder.event(&Event::PartyChat(conversation)), "the same conversation");
        let party = folder.state().party.clone().unwrap();
        assert_eq!(party.messages.len(), MESSAGES);
        assert_eq!(party.messages[0].text.as_deref(), Some("line 7"));
        let last = party.messages.last().unwrap();
        assert_eq!(last.reaction.as_deref(), Some("laughs"));
        assert_eq!((last.at, last.with.clone()), (Some(724.0), vec!["Ren".to_owned()]));
        assert!(party.messages.iter().all(|m| m.text.as_deref() != Some("muted words")));

        let state = PartyChatState {
            allowed: false,
            max_length: 300,
            muted: vec!["Ren".into()],
            wait_seconds: 0,
            react_wait_seconds: 0,
            hidden: false,
        };
        assert!(folder.event(&Event::PartyChatState(state)));
        folder.event(&Event::PartyState(party_view()));
        let party = folder.state().party.clone().unwrap();
        assert_eq!(party.messages.len(), MESSAGES, "a new state keeps the conversation");
        assert_eq!(
            (party.chat_off, party.chat_max, party.muted.clone()),
            (true, 300, vec!["Ren".into()])
        );

        let json = serde_json::to_value(&*party).unwrap();
        assert!(json.get("members").is_none(), "empty lists are left out: {json}");
        folder.event(&Event::PartyLeft { reason: None });
        assert!(folder.state().party.is_none());
    }
}
