//! Wiring playback to history.
//!
//! The policy lives in [`kaeri_playback::PlaybackTracker`], which is pure and tested. This
//! is the part that talks to the world: it opens a player, feeds its events to the tracker, and
//! turns the resulting actions into rows in the local database.
//!
//! The player is whatever the [`Machine`] opens, through a [`PlayerLink`]: mpv's child process
//! on the desktop. So this runs in any build; without the `desktop` feature there is no player
//! to open yet, and nothing starts a playback.
//!
//! History is written **before** anything is pushed anywhere. Local is the source of truth, so
//! if a tracker push fails the watch is still recorded and the outbox can retry.
//!
//! # Layout
//!
//! The playback run is here: the play a front end asks for, where it goes when it cannot play
//! in mpv, and the streams it tries in one window until one plays. One file's events, the
//! viewer's controls, what the companion shows, the picture, what is written down of the watch,
//! skipping, the subtitles fetched ahead and the Discord presence each have a file of their own.

use kaeri_model::{
    Msg,
    party::NoSourceWhy,
    starting::{StartFailure, StartStep},
};
use std::sync::Arc;

use kaeri_core::{
    media::Translation,
    stream::{Stream, StreamKind},
    traits::PlaybackRequest,
};
use kaeri_model::{
    command::PlayerCommand,
    event::Event,
    notice::Notice,
    playback::{Ending, SourceSwitch, Successor, SwitchTo},
};
// What the episode table says of the episodes around this one: the front end sends them ahead
// of a play, so they are the model's, and still named from here for whatever plays.
pub use kaeri_model::playback::{EpisodeEntry, NextEpisode};
use kaeri_net::HttpClient;
use kaeri_playback::{
    AniSkipTime, MarkedSkip, SkipInterval, TrackAction, TrackPreferences, companion,
};
use kaeri_store::{ServerFailure, Store};
use tokio::sync::mpsc;

use crate::{
    host::Host,
    outbox::Outbox,
    playback::{
        PlaybackContext,
        health::Servers,
        link::{FileFacts, LinkError, Overlay, PlayerLink, PlayerOpener},
        picture::HdrKind,
        power::PowerSource,
        seat::{PartyHub, Seat, SeatReport},
        shaders::Look,
        slot::{Player, PlayerSlot},
        switch::SwitchNews,
    },
};

mod companion_state;
mod controls;
mod events;
mod failover;
mod found;
mod marking;
mod presence;
#[cfg(test)]
mod scripted;
mod skips;
mod stats;
mod subtitles;
mod timeline;
mod upscaling;
mod watch;

pub use skips::{SkipTimes, offered, origin, taken, timeline_mark};
pub(crate) use subtitles::localise_ahead;
pub use upscaling::PictureState;

use companion_state::fetch_still;
use subtitles::{SUBTITLE_WAIT, localise_subtitles, sanitise};
use watch::remember_tracks;

/// An unguessable-enough path token for one playback attempt's stream proxy.
///
/// Not a security boundary — binding to loopback is — but it keeps an unrelated local process
/// from stumbling onto the proxy, and differs per attempt so a stale connection from a failed
/// stream cannot address the next one's.
pub(crate) fn proxy_token(context: &PlaybackContext, attempt: usize) -> String {
    // Random, because the parts identifying the playback were all guessable: the AniList id is
    // the show on screen, the episode a small integer, the attempt almost always zero, and the
    // timestamp a few seconds wide. That is a few thousand candidates against a scannable
    // ephemeral port, and the proxy will fetch any URL named in the path — so a guessable
    // token made it a fetch primitive for anything else running on the machine.
    use rand::Rng;
    let mut bytes = [0_u8; 16];
    rand::rng().fill_bytes(&mut bytes);
    let random: String = bytes.iter().map(|b| format!("{b:02x}")).collect();

    // The identifying part is kept in front of it: a token in a log line should say which
    // playback it belonged to, and it costs nothing next to 128 bits.
    format!("{}-{}-{attempt}-{random}", context.anilist_id.get(), sanitise(&context.episode))
}

/// The settings one playback runs under, read from the config when it starts.
#[derive(Debug, Clone)]
pub struct PlaySettings {
    pub threshold: f64,
    pub auto_skip: bool,
    /// Skip a recap by itself, from `playback.skip_recap_auto`. `auto_skip` covers the opening
    /// and the ending only.
    pub auto_skip_recap: bool,
    /// Ask "Was that the opening?" after a skip by hand that looks like one, from
    /// `playback.ask_was_opening`.
    pub ask_was_opening: bool,
    /// Find the opening and the ending from the audio, from `playback.find_skip_times`, where
    /// the player hands the audio over and the device is not saving power or storage: see
    /// [`crate::skip_find::SkipFind::runs`].
    pub find_skip_times: bool,
    /// Whether the viewer's marks and votes go to AniSkip, from `playback.share_skip_times`:
    /// `None` until asked, which the first Save does.
    pub share_skip_times: Option<bool>,
    /// Put the opening and ending on the seek bar where the stream has no chapters of its own,
    /// from `playback.mark_chapters`.
    pub mark_chapters: bool,
    /// Offer a skip button while the opening or ending plays, where the skip is not automatic,
    /// from `playback.skip_button`.
    pub skip_button: bool,
    /// Open mpv fullscreen, from `playback.fullscreen`.
    pub fullscreen: bool,
    pub subtitle_language: Option<String>,
    /// Whether a finished episode rolls on to the next, from `playback.auto_next`.
    pub auto_next: bool,
    pub night_mode: bool,
    /// Keep the viewer's audio and subtitle picks and subtitle delay per series, from
    /// `playback.persist_speed` — which has always promised "speed and track choice".
    pub remember_tracks: bool,
    pub look: Look,
    /// Where the shaders were written. `None` turns runtime upscaling off entirely — the
    /// cache could not be written, or the user passes their own `--glsl-shaders`.
    pub shader_dir: Option<std::path::PathBuf>,
    /// The HDR tone-mapping curve, from `playback.tone_mapping`.
    pub tone_mapping: String,
    /// Tracker ids to queue progress against when the episode completes.
    pub tracker_ids: Vec<String>,
    pub presence: kaeri_core::config::PresenceConfig,
    /// Tells a media server what is watched, when the stream is its own.
    pub reports: crate::servers::Reports,
    /// Where the watch party is, when there is one: a playback takes a seat at it, at the start
    /// or the moment it opens, and plays with the room until it ends. See [`crate::playback::seat`].
    pub party: PartyHub,
    /// How many seconds a hosted player's Up Next counts down before it plays the next
    /// episode, `0` for none: `playback.up_next_countdown`. The terminal's companion keeps its
    /// own ten.
    pub up_next_countdown: u32,
    /// The viewer's own mpv options, as property and value, for a hosted player to set as it
    /// opens: `playback.mpv_args`, which mpv's own process takes on its command line.
    pub mpv_options: Vec<(String, String)>,
    /// Record nothing: no history, resume point, tracker progress, track choices or presence.
    /// An opened link played without tracking. See [`PlaySettings::untracked`].
    pub untracked: bool,
    /// How a torrent stream is doing, for the starting screen until its first frame and for
    /// Stats for nerds' swarm rows.
    pub swarm: crate::playback::starting::Swarm,
    /// Set when the viewer asked to begin at 0: the file then opens there, whatever the resume
    /// point.
    pub begin: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// What was made ready for this episode while the one before ended, and when the roll-on
    /// handed over to it: see [`crate::playback::prefetch`].
    pub ahead: crate::playback::prefetch::Ahead,
    /// Whether what the playback would record is held now, private mode being on or the show
    /// always private: read at every write, so switching applies at once. See
    /// [`crate::private`].
    pub privacy: crate::private::Gate,
    /// The starting screen's Skip, heard until a file shows its first frame, and the sources
    /// beyond the streams found: where Skip and Switch source go once the pool has nothing
    /// left. See [`crate::playback::further`].
    pub further: crate::playback::further::Further,
    /// The playback's end of its door, where a release picked in the player's Sources panel
    /// reaches it: see [`crate::playback::switch`].
    pub switches: crate::playback::switch::SwitchInbox,
}

impl PlaySettings {
    /// These settings for a play that records nothing: nothing written to the history or the
    /// track memory, no tracker queued, no presence published, and nothing after it.
    pub fn untracked(self) -> Self {
        Self {
            untracked: true,
            remember_tracks: false,
            tracker_ids: Vec::new(),
            auto_next: false,
            presence: kaeri_core::config::PresenceConfig { enabled: false, ..self.presence },
            ..self
        }
    }
}

/// How many seconds Up Next counts down, the design's ten: the config's default, and what a
/// playback built without a config says.
pub const UP_NEXT_COUNTDOWN: u32 = 10;

/// How many episodes in a row play with no input from the viewer before a hosted player asks
/// "Still watching?". The app counts; the engine says how far.
pub const STILL_WATCHING_AFTER: u32 = 3;

/// What a playback runs on beside its stream: the player it opens a window in, the slot a window
/// waits in between episodes, the host that opens a link the player cannot play, and what says
/// whether the machine is on battery.
#[derive(Clone)]
pub struct Machine {
    pub opener: Arc<dyn PlayerOpener>,
    pub slot: PlayerSlot,
    pub host: Arc<dyn Host>,
    pub power: Arc<dyn PowerSource>,
    /// Whether the device has no network path, as its front end last said: Stats for nerds'
    /// Network row.
    pub offline: Arc<std::sync::atomic::AtomicBool>,
    /// The sources, for the one a stream came from when it transforms parts of its streams on
    /// the way to the player: see [`kaeri_core::traits::Provider::stream_transforms`]. `None`
    /// serves every stream as its host sent it.
    pub sources: Option<kaeri_providers::ProviderRegistry>,
}

#[cfg(feature = "desktop")]
impl Machine {
    /// A playback on the desktop, as the command line and the probes run one: in `mpv`'s
    /// process, with `slot` for its window, links opened by `host`, and the system's word on
    /// the battery.
    pub fn desktop(mpv: kaeri_player::Mpv, slot: PlayerSlot, host: Arc<dyn Host>) -> Self {
        Self {
            opener: Arc::new(mpv),
            slot,
            host,
            power: Arc::new(super::power::SystemPower),
            offline: Arc::default(),
            sources: None,
        }
    }
}

/// How one file's playback came to an end.
enum FileOutcome {
    /// mpv reached the end, the viewer closed it, or the stream failed.
    Ended {
        /// The tracker's verdict, when it reached one.
        watched: Option<bool>,
        /// mpv played to the end of the file.
        complete: bool,
        /// The window is gone: the viewer closed it, or mpv died.
        exited: bool,
        /// The viewer asked for this from the terminal.
        stopped: bool,
        /// mpv quit, rather than failing or dying: the viewer closed the window or pressed q,
        /// or `stopped` asked it to.
        quit: bool,
        /// Nothing was ever shown.
        no_frames: bool,
        /// The player ended the file in an error, rather than at its end or on a quit.
        errored: bool,
        /// Where the file was when it ended, and how long it is, when known.
        at: f64,
        duration: Option<f64>,
    },
    /// The viewer picked another stream from the menu, or asked for the same one again.
    Switch { to: usize, at: f64 },
    /// The release the viewer picked in the Sources panel was found: its streams, to play from
    /// `at`, where the file was, or where it opened when it has shown no frame yet.
    Swap { to: SwitchTo, streams: Vec<Stream>, at: Option<f64> },
    /// The viewer accepted the stall prompt's offer of another stream.
    Stalled { at: f64 },
    /// The viewer skipped the stream before it showed a frame, from the starting screen.
    Skipped,
    /// A newer playback is taking over this window, for this.
    Handover(Successor),
}

/// Play the best stream of a resolved list and record what happens.
///
/// `streams` is ordered best-first. A stream that dies without ever producing a frame is not
/// the end: the next one is tried, in the same window, because a provider can only rank what it
/// cannot verify — whether segments actually decode is knowable only here, in front of a player.
/// A stream that *played* and then ended falls over to another only when the player ended it in
/// an error short of the credits, and then at the second it reached, with the viewer's audio,
/// subtitles and delay: anything else would replay what was watched, or override a viewer who
/// closed it. The stream that takes over is the one most like it ([`failover::next_like`]).
///
/// The window is borrowed rather than owned. It comes from the machine's slot when the previous
/// episode left one open, and goes back there when this episode ends with a successor expected
/// — or when a newer playback takes over mid-episode — so a binge is one window from start to
/// finish. A new one is opened by the machine's opener.
///
/// Returns once playback has ended. Runs as its own task so the UI keeps rendering.
#[allow(clippy::too_many_arguments)]
pub async fn play(
    mut streams: Vec<Stream>,
    context: PlaybackContext,
    store: Store,
    http: HttpClient,
    machine: Machine,
    settings: PlaySettings,
    tx: Outbox,
    mut commands: mpsc::UnboundedReceiver<PlayerCommand>,
) {
    let Machine { opener, slot, host, power, offline, sources } = machine;
    // A private play's addresses stay out of the log while it plays, the mode on or not.
    let _private_log = settings.privacy.holds().then(crate::private::redact::open);
    let Some(first) = streams.first() else {
        slot.release();
        // Its controls closed before its ending is said: see `Handoff::live`.
        commands.close();
        let _ = tx.send(Event::playback_ended(false, Ending::Failed));
        return;
    };

    // A licensed stream cannot be decoded locally, so it is handed to whatever is licensed to
    // play it, through the host: the system's handler for the link. No progress is tracked,
    // because we never see any.
    if first.kind == StreamKind::ExternalDeepLink {
        slot.release();
        tracing::info!(url = %first.url, "handing off to the system handler");
        let update = match host.open_url(&first.url) {
            Ok(()) => Event::Toast(Notice::info(Msg::NoticeOpenedInBrowser)),
            Err(e) => {
                Event::Toast(Notice::alert(Msg::NoticeCouldNotOpen { reason: e.to_string() }))
            }
        };
        let _ = tx.send(update);
        // Nothing plays here, so Now Playing comes down rather than waiting on a player that
        // was never going to report.
        commands.close();
        let _ = tx.send(Event::playback_ended(false, Ending::Elsewhere));
        return;
    }

    let aniskip = SkipTimes::shared()
        .for_playback(&http, &settings, context.mal_id, &context.episode, context.runtime_secs)
        .await;
    // Aniskip's times the viewer reported are not used here again, and their own marks go
    // over the rest.
    let aniskip = skips::unreported(&store, aniskip).await;
    let marks = if settings.uses_skip_times() {
        skips::load_marks(&store, context.anilist_id, &context.episode).await
    } else {
        Vec::new()
    };

    let mut run = Run {
        context: &context,
        settings: &settings,
        store: &store,
        tx: &tx,
        skips: aniskip.iter().map(|time| time.interval).collect(),
        aniskip,
        marks,
        found: Vec::new(),
        marking: marking::SkipMarking::new(settings.ask_was_opening, settings.share_skip_times),
        // Cleared when the UI drops its sender, which disables that select branch — otherwise
        // a closed channel returns `None` instantly and spins the loop. Outside the attempt
        // loop, because a channel that closed during one stream is just as closed for the next.
        controls_open: true,
        cancel_next: false,
        night_applied: false,
        night: settings.night_mode,
        prefetch_sent: false,
        length: None,
        look: settings.look,
        tone_mapping: settings.tone_mapping.clone(),
        hdr: None,
        episodes: context.episodes.clone(),
        next: context.next.clone(),
        http: http.clone(),
        scratch: opener.scratch_dir().to_path_buf(),
        power,
        offline,
        still: None,
        // Fetched alongside everything else rather than before it: the card is minutes away
        // and the episode should not wait on a picture.
        still_fetch: context.next.as_ref().and_then(|n| n.thumbnail.clone()).map(|url| {
            tokio::spawn(fetch_still(http.clone(), url, opener.scratch_dir().to_path_buf()))
        }),
        servers: Servers::load(&store, &streams).await,
        seat: None,
        in_party: false,
        failed_over_from: None,
        skipping: settings.further.skips(),
        news: settings.switches.take(),
        finding: None,
        switching: None,
        restoring: None,
        pool_from: 0,
        sources: sources.clone(),
    };

    // Read once: every stream this playback tries is the same series in the same translation.
    // After the first file it is what the viewer chose in the last one, so another stream of
    // the episode starts from their choices, kept for the episode even when Remember tracks
    // keeps nothing for the series.
    let mut memory = if settings.remember_tracks {
        let (store, id, translation) = (store.clone(), context.anilist_id, context.translation);
        tokio::task::spawn_blocking(move || store.track_memory(id, translation))
            .await
            .ok()
            .and_then(|read| {
                read.inspect_err(|e| tracing::debug!(error = %e, "no track memory"))
                    .ok()
                    .flatten()
            })
    } else {
        None
    };

    // Grows when the sources after the pool are asked: see `Further`.
    let mut total = streams.len();
    // Named in the log so "did fallback have anything to work with" is answerable after the
    // fact — a pool of one behaves exactly like the old single-stream path.
    tracing::info!(streams = total, "starting playback");
    let mut tried = vec![false; total];
    let mut localised = vec![false; total];
    let mut index = 0;
    let mut attempt = 0;
    let mut resume_at = context.resume_at;
    let mut player: Option<Player> = None;
    // The quality of the stream the viewer last picked by hand from the sources menu, which a
    // failover keeps to; until then, the quality of the stream that failed.
    let mut target: Option<u32> = None;

    loop {
        tried[index] = true;
        let last = tried.iter().all(|t| *t);

        // Before mpv opens the file, so the tracks are on disk by the time it asks for them,
        // but no longer than a moment: the tracks still on their way follow as they arrive.
        let mut late = None;
        let say = |step, line| {
            crate::playback::starting::say(
                &tx,
                context.anilist_id,
                &context.episode,
                step,
                line,
            );
        };
        if !localised[index] {
            if streams[index].subtitles.iter().any(subtitles::fetches) {
                say(StartStep::FetchingSubtitles, None);
            }
            let scratch = opener.scratch_dir();
            // A subbed episode that plays with none says why, rather than leaving the viewer
            // to wonder whether it has any.
            let told = tx.clone();
            let all_dropped = move || {
                let _ = told.send(Event::Toast(Notice::alert(Msg::NoticeSubtitlesUnavailable)));
            };
            late = localise_subtitles(
                &http,
                scratch,
                &mut streams[index],
                SUBTITLE_WAIT,
                all_dropped,
            )
            .await;
            localised[index] = true;
        }
        let mut stream = streams[index].clone();

        // HLS goes through the stream proxy, which sends the stream's headers with every
        // request and holds playback through a rate limit; segments pass through unchanged
        // unless the stream's source transforms them.
        // Held for the whole attempt — dropping it would kill the proxy mid
        // playback — and replaced on the next, so a stream that failed leaves nothing behind.
        let mut proxies = crate::playback::proxy::Proxies::default();
        if stream.kind == StreamKind::Hls {
            let original = stream.url.clone();
            let source = sources
                .as_ref()
                .and_then(|sources| sources.get(&stream.provider_id))
                .filter(|source| source.stream_transforms().any());
            // The first attempt plays through the proxy started ahead for it, its first
            // seconds already fetched. That proxy transforms nothing, so a stream whose source
            // transforms parts of it gets a proxy of its own.
            let ahead = (attempt == 0 && source.is_none())
                .then(|| settings.ahead.adopt(&original))
                .flatten();
            stream.url = match ahead {
                Some(server) => proxies.adopt(&original, server),
                None => {
                    proxies
                        .route(
                            &http,
                            &original,
                            &stream.headers,
                            source,
                            &proxy_token(&context, attempt),
                        )
                        .await
                }
            };
            // The seek bar's thumbnailer reads the same stream through its own entry, which
            // answers from what playback already fetched and never waits out a rate limit.
            // Unset when the proxy could not start, and thumbfast then opens the stream as
            // the player does.
            stream.thumbnail_url = proxies.thumbnail_url(&original);
        }
        attempt += 1;
        say(
            StartStep::OpeningStream {
                attempt: u32::try_from(attempt).unwrap_or(u32::MAX),
                total: u32::try_from(total).unwrap_or(u32::MAX),
            },
            None,
        );
        tracing::info!(
            attempt,
            of = total,
            provider = %stream.provider_id,
            kind = ?stream.kind,
            quality = ?stream.quality,
            from = %crate::logged::address(&streams[index].url),
            proxied = stream.kind == StreamKind::Hls,
            headers = stream.headers.len(),
            "trying a stream"
        );

        // Asked to begin at 0 while the sources were walked or this file was found: it opens
        // there. The stored position stays as it was until the episode has played.
        let begin = || settings.begin.load(std::sync::atomic::Ordering::SeqCst);
        if begin() {
            resume_at = None;
        }
        let request = PlaybackRequest {
            // While private the window, and the system's media controls that read it, name no
            // show: "Playing in Kaeri".
            title: if settings.privacy.holds() {
                kaeri_i18n::t::now_playing_private()
            } else {
                context.window_title()
            },
            start_at: resume_at,
            subtitle_language: settings.subtitle_language.clone(),
            speed: context.speed,
            volume: context.volume,
            dub: context.translation == Translation::Dub,
            fullscreen: settings.fullscreen,
            mpv_options: settings.mpv_options.clone(),
        };

        // Named per attempt — during failover the header would otherwise credit the stream
        // that just failed.
        let _ = tx.send(Event::ActiveProvider(stream.provider_id.clone()));

        // A window: the one this playback already has, the one the last episode left open,
        // or a new one.
        if player.as_ref().is_none_or(|p| !p.link.is_alive()) {
            player = slot.take(&opener.target());
            run.night_applied = false;
        }
        if player.is_none() {
            tracing::info!("starting the player");
            match opener.open(&request).await {
                Ok((link, events)) => player = Some(Player::new(link, events)),
                Err(e) if last => {
                    let notice = opener.open_failure_notice().unwrap_or_else(|| {
                        Msg::NoticePlayerFailedToStart { reason: e.to_string() }
                    });
                    let _ = tx.send(Event::Toast(Notice::alert(notice.clone())));
                    // The player never existed, but the UI has Now Playing up and a control
                    // surface pointed at it. Saying so is what takes them down.
                    run.servers.settle().await;
                    commands.close();
                    let _ = tx.send(Event::playback_failed(StartFailure {
                        why: NoSourceWhy::NotFound,
                        text: notice,
                        asked: Vec::new(),
                        unreached: Vec::new(),
                        held_back: Vec::new(),
                    }));
                    return;
                }
                Err(e) => {
                    tracing::warn!(error = %e, "mpv would not start; trying the next stream");
                    index =
                        failover::next_like(&streams, &tried, index, target).unwrap_or(index);
                    continue;
                }
            }
        }
        let Some(current) = player.as_mut() else { continue };
        // This playback drives the window from here, the one it opened or the one it took.
        current.link.attach(&tx);

        // The track policy belongs to the process; point it at this series before the file
        // loads, and settle what it still owed the previous one.
        let preferences = TrackPreferences {
            translation: context.translation,
            language: settings.subtitle_language.clone(),
        };
        let owed = current.tracks.prepare(preferences, memory.clone());
        let key = (context.anilist_id, context.translation);
        if let Some(previous) = current.tracks_for.replace(key) {
            for action in owed {
                if let TrackAction::Remember(memory) = action {
                    remember_tracks(
                        &store,
                        settings.remember_tracks && !settings.privacy.holds(),
                        previous,
                        memory,
                    )
                    .await;
                }
            }
        }

        run.publish(current.link.as_ref(), &streams, index).await;
        current.link.prepare(&FileFacts {
            source: streams[index].clone(),
            played: stream.clone(),
            id: context.anilist_id,
            title: context.title.clone(),
            episode: context.episode.clone(),
            episode_title: run
                .episodes
                .iter()
                .find(|e| e.number == context.episode)
                .and_then(|e| e.title.clone()),
            image: context.banner_url.clone().or_else(|| context.cover_url.clone()),
            subtitle_language: settings.subtitle_language.clone(),
            dub: context.translation == Translation::Dub,
            audio_memory: current.tracks.memory().audio.clone(),
        });
        // The audio of this file's head and tail, for finding its skip times: listened for
        // before it loads, so none of it is missed. Never on a cast, which plays elsewhere.
        let finder = (settings.find_skip_times
            && current.link.target() == crate::playback::link::PlayerTarget::Local)
            .then(|| {
                found::Finder::start(
                    store.clone(),
                    context.anilist_id,
                    context.episode.trim().to_owned(),
                )
            });
        let tap = finder.as_ref().map(found::Finder::options).unwrap_or_default();
        if let Err(e) = load(current.link.as_ref(), &stream, &request, tap).await {
            tracing::warn!(error = %e, "could not hand the stream to mpv");
        }
        // mpv is already at the resume point by now — `start` handled it. Saying so is what
        // keeps it from looking like the episode began in the wrong place.
        // Asked while the file was handed over, before it could hear the command: seek there.
        if resume_at.is_some() && begin() {
            resume_at = None;
            let _ = current.link.seek(0.0, true).await;
        }
        if let Some(position) = resume_at {
            let _ = tx.send(Event::Resumed { position });
            let _ = current.link.overlay(Overlay::Resumed { position }).await;
        }

        let outcome = run
            .file(
                current,
                &stream,
                &streams,
                index,
                &mut commands,
                events::Beside { late, finder },
            )
            .await;

        // A release that landed is the pool from here: the streams it replaced are let go, so
        // failover, the Sources menu and the next switch go by what plays now.
        let from = std::mem::take(&mut run.pool_from);
        let outcome = if from > 0 && from <= index {
            streams.drain(..from);
            tried.drain(..from);
            localised.drain(..from);
            total = streams.len();
            index -= from;
            match outcome {
                FileOutcome::Switch { to, at } => {
                    FileOutcome::Switch { to: to.checked_sub(from).unwrap_or(index), at }
                }
                outcome => outcome,
            }
        } else {
            outcome
        };

        // What the viewer chose in this file is where the next file of the episode starts.
        let (carried, owed) = current.tracks.carried();
        for action in owed {
            run.apply_track(action, current.link.as_ref()).await;
        }
        memory = Some(carried);

        // Skipped before a frame, the stream's server is not to blame: the viewer moved on.
        if matches!(outcome, FileOutcome::Skipped) {
            let refused = proxies.refused(&streams[index].url);
            run.servers.unplayed(&store, &streams[index], true, refused);
        }
        // A skip, or Switch source on a stall, with no stream left in the pool to go to: the
        // sources after the ones played from are asked, behind the starting screen, as the walk
        // would have asked them.
        let outcome = match outcome {
            FileOutcome::Skipped | FileOutcome::Stalled { .. }
                if failover::next_like(&streams, &tried, index, target).is_none()
                    && settings.further.beyond(&streams) =>
            {
                let stalled_at = match outcome {
                    FileOutcome::Stalled { at } => Some(at),
                    _ => None,
                };
                let skipped = stalled_at.is_none().then(|| streams[index].provider_id.clone());
                let asked = onward(
                    &mut run,
                    current,
                    &streams,
                    index,
                    skipped.as_deref(),
                    stalled_at,
                    &mut commands,
                )
                .await;
                match asked {
                    Onward::Found(more) => {
                        tracing::info!(
                            streams = more.len(),
                            provider = %more[0].provider_id,
                            "playing on from the sources after the pool"
                        );
                        index = streams.len();
                        streams.extend(more);
                        total = streams.len();
                        tried.resize(total, false);
                        localised.resize(total, false);
                        if let Some(at) = stalled_at {
                            resume_at = Some(at);
                        }
                        run.failed_over_from = None;
                        continue;
                    }
                    Onward::Ended(outcome) => outcome,
                    // A stall has played: the same stream opens again where it was.
                    Onward::Failed(_) if stalled_at.is_some() => {
                        FileOutcome::Stalled { at: stalled_at.unwrap_or_default() }
                    }
                    // Skipped past the last source, the start fails as the walk would have.
                    Onward::Failed(failure) => {
                        tracing::info!("skipped, and no source after it answered");
                        let _ = tx.send(Event::Toast(Notice::alert(failure.text.clone())));
                        if let Some(seat) = run.seat.take() {
                            seat.report(SeatReport::Ended { leave: false, finished: false });
                        }
                        if let Some(player) = player.take() {
                            player.link.shutdown().await;
                        }
                        run.servers.settle().await;
                        // Its controls closed before its ending is said: see `Handoff::live`.
                        commands.close();
                        let _ = tx.send(Event::playback_failed(failure));
                        return;
                    }
                }
            }
            outcome => outcome,
        };

        match outcome {
            FileOutcome::Swap { to: asked, streams: found, at } => {
                let at = at.or(resume_at).unwrap_or_default();
                let from = streams.len();
                streams.extend(found);
                total = streams.len();
                tried.resize(total, false);
                localised.resize(total, false);
                tracing::info!(
                    from = index,
                    provider = %asked.provider_id,
                    at,
                    "switching to another release in place"
                );
                run.begin_switch(asked, at, index, from..total);
                index = from;
                resume_at = Some(at);
                target = streams[index].quality;
                run.failed_over_from = None;
            }
            FileOutcome::Switch { to, at } => {
                tracing::info!(from = index, to, at, "switching stream in place");
                // Picked by the viewer, not the same stream opened again where it was.
                if to != index {
                    let asked = SwitchTo {
                        provider_id: streams[to].provider_id.clone(),
                        name: run.source_name(&streams[to]),
                        release: None,
                        stream: Some(to.to_string()),
                    };
                    run.begin_switch(asked, at, index, to..to + 1);
                }
                index = to;
                resume_at = Some(at);
                // A stream picked by hand sets the quality later failovers keep to.
                target = streams[to].quality;
                run.failed_over_from = None;
                // A switch the viewer asked for is a fresh start for failover: the stream they
                // left may be the one to come back to.
                tried = vec![false; total];
            }
            FileOutcome::Stalled { at } => {
                // Another stream like this one, one not tried yet where there is one. Once
                // every stream has been tried the viewer is still asking for another, so the
                // pool starts again from this one.
                let to = failover::next_like(&streams, &tried, index, target).or_else(|| {
                    tried = vec![false; total];
                    tried[index] = true;
                    failover::next_like(&streams, &tried, index, target)
                });
                // A pool of one has no stall prompt to switch from; were it asked, the same
                // stream opens again where it was.
                let to = to.unwrap_or(index);
                tracing::info!(from = index, to, at, "switching away from a stall");
                index = to;
                resume_at = Some(at);
            }
            FileOutcome::Skipped => {
                // The next stream like this one, the pool from the top once each has been tried.
                let to = failover::next_like(&streams, &tried, index, target).or_else(|| {
                    tried = vec![false; total];
                    tried[index] = true;
                    failover::next_like(&streams, &tried, index, target)
                });
                let to = to.unwrap_or(index);
                tracing::info!(from = index, to, "skipping to another stream");
                index = to;
            }
            FileOutcome::Handover(successor) => {
                // The playback taking the window takes the seat too.
                if let Some(seat) = run.seat.take() {
                    seat.report(SeatReport::Ended { leave: false, finished: false });
                }
                if let Some(player) = player.take() {
                    let card = companion::Status::loading(context.handing_to(&successor), None);
                    // Art the record of the titles did not have, this playback's when it is the
                    // same title's.
                    let ours = |art: &Option<String>| {
                        (successor.id == context.anilist_id).then(|| art.clone()).flatten()
                    };
                    let starting = Overlay::Starting {
                        card,
                        id: successor.id,
                        title: successor.title.clone(),
                        episode: successor.episode.clone(),
                        episode_title: None,
                        cover_url: successor
                            .cover_url
                            .clone()
                            .or_else(|| ours(&context.cover_url)),
                        banner_url: successor
                            .banner_url
                            .clone()
                            .or_else(|| ours(&context.banner_url)),
                    };
                    let _ = player.link.overlay(starting).await;
                    // Silence what was playing while the next stream is found.
                    let _ = player.link.stop().await;
                    slot.park(player);
                }
                run.servers.settle().await;
                return;
            }
            FileOutcome::Ended {
                watched,
                complete,
                exited,
                stopped,
                quit,
                no_frames,
                errored,
                at,
                duration,
            } => {
                let reported = match (&player, no_frames) {
                    (Some(p), true) => p.link.diagnostics().await,
                    _ => None,
                };
                let notice = player.as_ref().and_then(|p| p.link.failure_notice());
                // What the attempt says of its server, written down before anything else is
                // decided, so the next play knows which server to try first.
                if no_frames {
                    let refused = proxies.refused(&streams[index].url);
                    run.servers.unplayed(&store, &streams[index], stopped || quit, refused);
                }

                // A switch that showed no frame: another of its release's streams, and then
                // back to what played before, where it was. The viewer is told once that one
                // shows a frame again.
                if no_frames
                    && !stopped
                    && !exited
                    && !quit
                    && let Some(switching) = run.switching.take()
                {
                    if let Some(next) = switching.streams.clone().find(|i| !tried[*i]) {
                        tracing::warn!(
                            to = next,
                            "the switch showed no frame; trying another of its streams"
                        );
                        index = next;
                        resume_at = Some(switching.at);
                        run.switching = Some(switching);
                        continue;
                    }
                    tracing::warn!(
                        back_to = switching.back_to,
                        "the switch showed no frame; going back"
                    );
                    let text = Msg::PlayerPanelSwitchFailed {
                        source: switching.to.name.clone(),
                        current: run.source_name(&streams[switching.back_to]),
                    };
                    let reason = notice.clone().or_else(|| {
                        reported
                            .clone()
                            .map(|line| Msg::NoticePlayerCouldNotPlay { reason: line })
                    });
                    let Switching { to, at, back_to, .. } = switching;
                    run.restoring = Some(SourceSwitch::Failed { to, at, text, reason });
                    index = back_to;
                    resume_at = Some(at);
                    target = streams[index].quality;
                    continue;
                }
                if no_frames && !stopped && !exited && !last {
                    // The point of carrying alternates: whether segments actually decode is
                    // knowable only in front of a player, so a stream that never showed a frame
                    // fails over — in the same window — unless the viewer stopped it or closed
                    // the window themselves, which is an instruction, not a failure.
                    tracing::warn!(
                        from = %crate::logged::address(&streams[index].url),
                        ?reported,
                        "no frames; trying the next stream"
                    );
                    // Said on the starting screen's step line, in place, and not as a notice.
                    say(
                        StartStep::TryingAnotherStream {
                            next: u32::try_from(attempt + 1).unwrap_or(u32::MAX),
                            total: u32::try_from(total).unwrap_or(u32::MAX),
                        },
                        Some(Msg::StartTryingAnotherStream),
                    );
                    index =
                        failover::next_like(&streams, &tried, index, target).unwrap_or(index);
                    continue;
                }

                // A stream that played and then broke off in an error, in the same window and
                // short of the credits, is a failure of the stream and not the end of the
                // episode: another stream like it takes over at the same second, once per
                // stream, with the viewer's audio, subtitles and delay carried over.
                let broke_off = errored
                    && !no_frames
                    && !complete
                    && !stopped
                    && !quit
                    && !exited
                    && duration.is_none_or(|d| at < d - failover::FAILOVER_TAIL);
                if broke_off {
                    run.servers.failed(&store, &streams[index], ServerFailure::BrokeOff);
                    if let Some(to) = failover::next_like(&streams, &tried, index, target) {
                        tracing::warn!(
                            from = %crate::logged::address(&streams[index].url),
                            to = %crate::logged::address(&streams[to].url),
                            at,
                            "the stream broke off; failing over"
                        );
                        say(
                            StartStep::TryingAnotherStream {
                                next: u32::try_from(attempt + 1).unwrap_or(u32::MAX),
                                total: u32::try_from(total).unwrap_or(u32::MAX),
                            },
                            Some(Msg::StartTryingAnotherStream),
                        );
                        // Said once the stream that takes over shows a frame.
                        if run.failed_over_from.is_none() {
                            run.failed_over_from = Some(streams[index].provider_id.clone());
                        }
                        index = to;
                        resume_at = Some(at);
                        continue;
                    }
                    tracing::warn!(at, "the stream broke off, and no other is left to try");
                }

                // Keep the window for the next episode when one is coming: auto-next is on, the
                // episode table knows what is next, and the viewer did not say no.
                // Not in a party: the room decides what it watches next, and each member's
                // next episode would be found and started on their own.
                let rolls_on = complete
                    && settings.auto_next
                    && !run.cancel_next
                    && !exited
                    && !run.in_party
                    && run.next.is_some();
                // Stopping the player or closing its window asks to leave the party, which only
                // the terminal does; an episode that ended, or failed, stays in it, for the next
                // to play with the room.
                if let Some(seat) = run.seat.take() {
                    seat.report(SeatReport::Ended {
                        leave: stopped || quit,
                        finished: complete && !stopped && !quit && !exited,
                    });
                }
                let next = run.next.as_ref().filter(|_| rolls_on);
                if let Some(player) = player.take() {
                    if let Some(next) = next {
                        let card = companion::Status::loading(
                            format!("Up next · {}", next.label()),
                            Some("Finding sources…".into()),
                        );
                        let starting = Overlay::Starting {
                            card,
                            id: context.anilist_id,
                            title: context.title.clone(),
                            episode: next.number.clone(),
                            episode_title: next.title.clone(),
                            cover_url: context.cover_url.clone(),
                            banner_url: context.banner_url.clone(),
                        };
                        let _ = player.link.overlay(starting).await;
                        slot.park(player);
                    } else {
                        player.link.shutdown().await;
                    }
                }

                // Not for a player the viewer closed or stopped: quitting before the first frame
                // is a choice, and "mpv exited without playing anything" would blame mpv for it.
                // Both are checked, because a quit that reaches mpv before the file has started
                // ends no file, and only the socket closing says anything happened.
                let mut failure = None;
                if no_frames && !stopped && !quit {
                    let message = match (notice, reported) {
                        (Some(notice), _) => notice,
                        (None, Some(line)) => Msg::NoticePlayerCouldNotPlay { reason: line },
                        (None, None) => Msg::NoticePlayerExitedSilently,
                    };
                    tracing::warn!(
                        message = %message,
                        from = %crate::logged::address(&streams[index].url),
                        "playback produced no frames"
                    );
                    let _ = tx.send(Event::Toast(Notice::alert(message.clone())));
                    failure = Some(StartFailure {
                        why: NoSourceWhy::NotFound,
                        text: message,
                        asked: Vec::new(),
                        unreached: Vec::new(),
                        held_back: Vec::new(),
                    });
                }

                // Exactly one ending per playback, sent only now that this attempt is known to
                // be the last: the tracker's verdict when it reached one, and otherwise the
                // honest `false` for a session that never played. The ending names the next
                // episode only when the window was kept for it, which is what auto-next follows.
                // What this playback learned of its servers is in the store first, for the
                // next to read.
                run.servers.settle().await;
                let ending = if complete {
                    Ending::Complete { next: next.map(|n| n.number.clone()) }
                } else if stopped {
                    Ending::Stopped
                } else if exited {
                    Ending::Closed
                } else {
                    Ending::Failed
                };
                let failure = failure.filter(|_| ending == Ending::Failed);
                // Its controls closed before its ending is said: see `Handoff::live`.
                commands.close();
                let _ = tx.send(Event::PlaybackEnded {
                    watched: watched.unwrap_or(false),
                    ending,
                    rolling_on: false,
                    failure,
                });
                return;
            }
        }
    }
}

/// What came of asking the sources after the pool.
enum Onward {
    /// The streams of the next source that answered.
    Found(Vec<Stream>),
    /// None answered, as the start would have failed.
    Failed(StartFailure),
    /// The viewer stopped, closed the window, or a newer playback took it over meanwhile.
    Ended(FileOutcome),
}

/// Ask the sources after `streams` for the episode, with the starting screen up over the
/// player, which stops playing the stream passed. `skipped` is the source the viewer skipped,
/// and `stalled_at` where a stalled file was when Switch source was pressed. A Stop, a closed
/// window or a newer playback ends the asking.
async fn onward(
    run: &mut Run<'_>,
    player: &mut Player,
    streams: &[Stream],
    index: usize,
    skipped: Option<&str>,
    stalled_at: Option<f64>,
    commands: &mut mpsc::UnboundedReceiver<PlayerCommand>,
) -> Onward {
    let link = player.link.as_ref();
    run.publish(link, streams, index).await;
    if stalled_at.is_some() {
        let _ = link.overlay(Overlay::StallCleared).await;
    }
    let _ = link.stop().await;
    let ended = |stopped: bool, exited: bool, quit: bool| FileOutcome::Ended {
        watched: None,
        complete: false,
        exited,
        stopped,
        quit,
        no_frames: stalled_at.is_none(),
        errored: false,
        at: stalled_at.unwrap_or_default(),
        duration: None,
    };
    let settings = run.settings;
    let asking = settings.further.after(streams, skipped);
    tokio::pin!(asking);
    loop {
        tokio::select! {
            asked = &mut asking => {
                return match asked {
                    Ok(more) if !more.is_empty() => Onward::Found(more),
                    Ok(_) => Onward::Failed(StartFailure {
                        why: NoSourceWhy::NotFound,
                        text: Msg::NoticeReason { reason: kaeri_model::Cause::NoSources },
                        asked: Vec::new(),
                        unreached: Vec::new(),
                        held_back: Vec::new(),
                    }),
                    Err(failure) => Onward::Failed(failure),
                };
            }
            command = commands.recv(), if run.controls_open => match command {
                None => run.controls_open = false,
                Some(PlayerCommand::Stop) => return Onward::Ended(ended(true, false, true)),
                Some(PlayerCommand::Handover(successor)) => {
                    if let Some(at) = stalled_at {
                        let context = run.context;
                        let _ = run.tx.send(Event::EpisodeLeft {
                            id: context.anilist_id,
                            episode: context.episode.clone(),
                            position: at,
                            duration: None,
                            watched: false,
                        });
                    }
                    return Onward::Ended(FileOutcome::Handover(successor));
                }
                // The walk hears a skip itself; the rest waits for a stream to play.
                Some(_) => {}
            },
            event = player.events.recv() => {
                if event.is_none() {
                    return Onward::Ended(ended(false, true, link.has_quit()));
                }
            }
        }
    }
}

/// Open `stream` in the player, in place of whatever was playing.
///
/// Everything that belongs to the file travels with it, so the previous file's never leak into
/// this one: headers, resume point, title and track languages as its options, and before them
/// where the seek bar's thumbnails are read, so the thumbnailer sees the right address when it
/// starts on this file. A torrent gives a second address that reads only what is already
/// downloaded, so thumbnails never pull bandwidth away from the playhead. External subtitles are
/// attached once the file has loaded.
async fn load(
    link: &dyn PlayerLink,
    stream: &Stream,
    request: &PlaybackRequest,
    // The audio tap's options, when its skip times are found.
    tap: Vec<(String, String)>,
) -> Result<(), LinkError> {
    link.overlay(Overlay::ThumbnailUrl(stream.thumbnail_url.clone())).await?;
    let mut options = kaeri_playback::protocol::file_options(
        &stream.headers,
        request.start_at,
        &request.title,
        request.subtitle_language.as_deref(),
        request.dub,
    );
    options.extend(tap);
    // A torrent fills the player's cache before the first frame, so that the starting screen
    // has a real percentage to show; a direct stream starts as soon as it can.
    if stream.kind == StreamKind::TorrentHttp {
        options.push(("cache-pause-initial".to_owned(), "yes".to_owned()));
    }
    link.load(&stream.url, options).await
}

/// What a party is answered with where this build has no watch parties: see the `party`
/// feature.
pub const NO_PARTY: Msg = Msg::NoticeNoParty;

/// One playback's state across the streams it tries.
struct Run<'a> {
    context: &'a PlaybackContext,
    settings: &'a PlaySettings,
    store: &'a Store,
    tx: &'a Outbox,
    /// The skips as they stand, as the tracker last settled them.
    skips: Vec<SkipInterval>,
    /// Aniskip's times for the episode, less the ones the viewer reported.
    aniskip: Vec<AniSkipTime>,
    /// The viewer's own marks for the episode.
    marks: Vec<MarkedSkip>,
    /// What was found from the audio of the file playing, by kind.
    found: Vec<SkipInterval>,
    /// The marking bar, its prompts and the seeks they watch.
    marking: marking::SkipMarking,
    controls_open: bool,
    /// The viewer cancelled Up Next, so the end of this episode is the end.
    cancel_next: bool,
    /// Night mode as the viewer last set it, and whether this window has it applied.
    night: bool,
    night_applied: bool,
    /// Whether the next episode has been asked to be found ahead.
    prefetch_sent: bool,
    /// The file's length, once the player has said it: where the episode's own ending can
    /// start goes by it.
    length: Option<f64>,
    /// The upscaling look and HDR curve as the viewer last chose them in the player.
    look: Look,
    tone_mapping: String,
    /// What the current file is, for the tone-mapping menu.
    hdr: Option<HdrKind>,
    /// The next episode's still once written out, and the fetch while it is still under way.
    still: Option<companion::Still>,
    still_fetch: Option<tokio::task::JoinHandle<Option<companion::Still>>>,
    /// The episode list and what follows, from the context or learned once playback started.
    episodes: Vec<EpisodeEntry>,
    next: Option<NextEpisode>,
    /// For fetching a still learned late, and where to write it.
    http: HttpClient,
    scratch: std::path::PathBuf,
    /// What says whether the machine is on battery, which the upscaling chain goes by.
    power: Arc<dyn PowerSource>,
    /// Whether the device has no network path, for Stats for nerds.
    offline: Arc<std::sync::atomic::AtomicBool>,
    /// How the servers of the streams tried have fared, and the attempt under way.
    servers: Servers,
    /// The seat at the watch party, between files; the file playing holds it while it plays.
    seat: Option<Seat>,
    /// Whether this playback plays with a party now: what it writes down and whether it rolls
    /// on go by it.
    in_party: bool,
    /// The source of the stream that broke off, while another is failing over for it: the
    /// notice naming the stream that took over is said once that one shows a frame.
    failed_over_from: Option<String>,
    /// The starting screen's Skip, heard until a file shows its first frame: see
    /// [`PlaySettings::further`].
    skipping: Option<tokio::sync::watch::Receiver<u64>>,
    /// What the engine says of a release it finds for this playback, and the release it is
    /// finding: see [`crate::playback::switch`].
    news: Option<mpsc::UnboundedReceiver<SwitchNews>>,
    finding: Option<SwitchTo>,
    /// A switch loading, until its first frame: what it is and what it goes back to.
    switching: Option<Switching>,
    /// A switch that failed, said once the stream it went back to shows a frame again.
    restoring: Option<SourceSwitch>,
    /// Where the streams of a release that landed start: the streams before it are let go once
    /// its file is over, and the Sources menu lists only the release's streams until then.
    pool_from: usize,
    /// The sources, for the names a switch says.
    sources: Option<kaeri_providers::ProviderRegistry>,
}

/// A switch the viewer asked for, loading.
struct Switching {
    to: SwitchTo,
    /// Where the episode was, which it plays from.
    at: f64,
    /// The stream it played before, which it goes back to when the switch shows no frame.
    back_to: usize,
    /// The streams the switch may play, the release's own: another of them is tried before
    /// going back.
    streams: std::ops::Range<usize>,
}

impl Run<'_> {
    /// The name the source of `stream` goes by.
    fn source_name(&self, stream: &Stream) -> String {
        match &self.sources {
            Some(sources) => {
                crate::playback::resolve::display_name(sources, &stream.provider_id)
            }
            None => stream.provider_id.clone(),
        }
    }

    /// Say how the switch goes, for this playback.
    fn say_switch(&self, state: SourceSwitch) {
        let playback = self.tx.playback().unwrap_or(kaeri_model::playback::PlaybackId::new(0));
        let _ = self.tx.send(Event::PlayerSwitch { playback, state });
    }

    /// A switch to `to`, at `at`, from the stream `back_to`, about to load the first of
    /// `streams`: held from here until it lands or fails.
    fn begin_switch(
        &mut self,
        to: SwitchTo,
        at: f64,
        back_to: usize,
        streams: std::ops::Range<usize>,
    ) {
        // One that failed and has not yet said so is over: a newer switch takes its place.
        if let Some(failed) = self.restoring.take() {
            self.say_switch(failed);
        }
        self.say_switch(SourceSwitch::Loading { to: to.clone(), at });
        self.switching = Some(Switching { to, at, back_to, streams });
    }
}

#[cfg(all(test, feature = "cast"))]
mod cast_tests;
#[cfg(test)]
mod tests {
    use kaeri_core::ids::AnilistId;

    use super::*;

    fn context_for(episode: &str) -> PlaybackContext {
        PlaybackContext {
            anilist_id: AnilistId::new(1),
            mal_id: None,
            episode: episode.into(),
            title: "A Show".into(),
            translation: Translation::Sub,
            resume_at: None,
            runtime_secs: None,
            speed: None,
            volume: None,
            next: None,
            episodes: Vec::new(),
            cover_url: None,
            banner_url: None,
            episode_total: None,
        }
    }

    #[test]
    fn a_window_changing_hands_says_what_is_coming() {
        let now = context_for("6");
        let to = |id: u32, title: &str, episode: &str, translation| Successor {
            id: AnilistId::new(id),
            title: title.into(),
            episode: episode.into(),
            translation,
            cover_url: None,
            banner_url: None,
        };
        let sub = Translation::Sub;
        assert_eq!(now.handing_to(&to(1, "A Show", "7", sub)), "Opening the next episode…");
        assert_eq!(now.handing_to(&to(1, "A Show", "5", sub)), "Opening the previous episode…");
        assert_eq!(now.handing_to(&to(1, "A Show", "11", sub)), "Opening Episode 11…");
        assert_eq!(
            now.handing_to(&to(1, "A Show", "6", Translation::Dub)),
            "Switching to the dub…"
        );
        assert_eq!(now.handing_to(&to(1, "A Show", "6", sub)), "Opening Episode 6 again…");
        assert_eq!(
            now.handing_to(&to(2, "Another Show", "1", sub)),
            "Opening Another Show · Episode 1…"
        );
    }

    #[test]
    fn a_playback_context_carries_what_history_needs() {
        // Provider and translation are the fields no tracker can supply, and the reason local
        // history is richer than anything that can be synced.
        let context = PlaybackContext {
            anilist_id: AnilistId::new(154_587),
            mal_id: Some(52_991),
            episode: "12".into(),
            title: "Hoshikuzu".into(),
            translation: Translation::Sub,
            resume_at: Some(600.0),
            runtime_secs: Some(1_440),
            speed: Some(1.25),
            volume: Some(80.0),
            next: None,
            episodes: Vec::new(),
            cover_url: None,
            banner_url: None,
            episode_total: None,
        };
        assert_eq!(context.mal_id, Some(52_991));
        assert_eq!(context.resume_at, Some(600.0));
        assert_eq!(context.runtime_secs, Some(1_440));
    }
}
