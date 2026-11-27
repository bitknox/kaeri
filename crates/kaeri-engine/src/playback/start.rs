//! Starting a playback: the plays a front end asks for, each with a fresh control channel and the
//! window handed over, the stream found behind the eyecatch, and the settings it runs under.

use kaeri_model::Msg;
use kaeri_model::StoreMsg;
use std::sync::Arc;

use kaeri_core::{config::Config, ids::AnilistId};
use kaeri_model::{
    command::PlayerCommand, episodes::EpisodeRow, event::Event, notice::Notice,
    playback::PlaybackId,
};
use tokio::{
    sync::{mpsc, watch},
    task::JoinHandle,
};

use crate::{
    cache::{PlayHints, PlayKind},
    jobs::JobGuard,
    outbox::{Outbox, Played},
    playback::{
        self,
        autonext::{Follow, Staged},
        further::{Further, Lookup},
        handoff::{Handoff, successor, take_controls},
        link::{PlayerOpener, PlayerTarget, opener},
        prefetch::Warm,
        resolve::{Pick, Unplayable, resolve_for_playback},
        shaders::shader_dir,
        slot::PlayerSlot,
        starting::Starting,
        switch::SwitchInbox,
    },
    services::Services,
    show_settings,
    state::EngineState,
    titles::episodes::{EpisodeLoad, load_episodes},
    tracking::commands::signed_in_tracker_ids,
};

/// What a play of `episode` of `id` takes from the engine's record of the titles: the name the
/// window being handed over announces it by, and what the player's Up Next card and Episodes
/// menu show. See [`TitleCache::hints`]. A staged play's name is remembered for the episode
/// that follows it, which goes by the same name.
///
/// [`TitleCache::hints`]: crate::cache::TitleCache::hints
fn hints_for(
    id: AnilistId,
    episode: &str,
    kind: PlayKind,
    skip_filler: bool,
    engine: &mut EngineState,
) -> PlayHints {
    let hints = engine.cache.hints(id, episode, skip_filler, kind);
    if kind == PlayKind::Staged {
        engine.cache.announce(id, hints.title.clone());
    }
    hints
}

/// What this play opens its player with, from the config in effect: see [`opener`].
///
/// No player handle is kept across the session: every play builds its own from the config in
/// effect, which is the live one the Settings screen edits, as the front end last said. A
/// handle held across the session is exactly how a turned-off upscaler kept running.
pub(crate) fn opener_for(
    engine: &EngineState,
    services: &Services,
) -> Option<Arc<dyn PlayerOpener>> {
    // While casting, every play casts to the same TV, auto-next's included.
    #[cfg(feature = "cast")]
    if let Some(cast) = &engine.casting
        && services.casts.casting()
    {
        return Some(Arc::clone(cast) as Arc<dyn PlayerOpener>);
    }
    // A cast session holds a TV: every play goes to it.
    #[cfg(feature = "cast")]
    if let Some(device) = services.casts.session_device() {
        return Some(Arc::new(cast_opener(services, device)) as Arc<dyn PlayerOpener>);
    }
    let hosted = services.hosted_player.then(|| -> Arc<dyn PlayerOpener> {
        Arc::clone(&services.hosted) as Arc<dyn PlayerOpener>
    });
    opener(engine.config.effective(), &services.paths, &services.player_theme, hosted)
}

/// Play an episode from its finished download, or else from whichever source the resolution
/// ladder chooses, staged for Now Playing
/// as the engine's record of the titles names it, which the front end is told: where it was
/// left, or from the beginning when `from_start`. Refused, with nothing staged, where this build
/// has no player to open.
pub fn play(
    id: AnilistId,
    episode: String,
    from_start: bool,
    job: Option<JobGuard>,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let Some(opener) = opener_for(engine, services) else {
        return playback::refuse(job, tx);
    };
    let billing = named(id, &episode, engine);
    let start = if from_start { Start::Beginning } else { Start::Resume };
    // In a room, an episode with no source here is the room's No source, not a toast.
    #[cfg(feature = "room")]
    let in_room = crate::room::is_going(engine);
    #[cfg(not(feature = "room"))]
    let in_room = false;
    let staged = play_staged(
        id, episode, None, start, in_room, false, billing, opener, job, engine, services, tx,
    );
    let _ = tx.send(staged);
}

/// Cast an episode to the TV with Chromecast built in that `device` names: a play as any other,
/// in a cast to that TV, and every play after it too, until [`stop_casting`]. From `position`
/// when it is carried over from this device, from the top when that is close to it.
#[cfg(feature = "cast")]
#[allow(clippy::too_many_arguments)]
pub fn cast_to(
    device: &str,
    id: AnilistId,
    episode: String,
    position: Option<f64>,
    job: Option<JobGuard>,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let hub = &services.casts;
    let Some(device) = hub.device(device) else {
        let _ = tx.send(Event::Toast(Notice::alert(kaeri_model::Msg::NoticeCastNoDevice)));
        return drop(job);
    };
    // Carried over: written down as where the episode was left, which is where a play opens.
    let from_start = match position {
        Some(at) if at >= CARRY_FROM => {
            let event = kaeri_store::WatchEvent::new(id, episode.clone(), at);
            // While private, held in memory as any private play's place is, and never written.
            if services.privacy.holds_show(id) {
                services.privacy.keep_watch(event);
            } else if let Err(e) = services.store.record_event(&event) {
                tx.not_kept("the position carried over to the cast", &e);
            }
            false
        }
        Some(_) => true,
        None => false,
    };
    hub.begin(device.clone());
    hub.asked(id, episode.clone());
    engine.casting = Some(Arc::new(cast_opener(services, device)));
    play(id, episode, from_start, job, engine, services, tx);
}

/// What opens a cast to `device`, with its scratch files in the runtime directory.
#[cfg(feature = "cast")]
fn cast_opener(
    services: &Services,
    device: kaeri_cast::CastDevice,
) -> crate::playback::link::cast::CastOpener {
    let scratch = services.paths.runtime_dir().join("cast");
    if let Err(e) = std::fs::create_dir_all(&scratch) {
        tracing::warn!(error = %e, "couldn't make the cast's scratch directory");
    }
    crate::playback::link::cast::CastOpener::new(Arc::clone(&services.casts), device, scratch)
}

/// A position closer to the start than this is not carried over to a cast: it starts from the
/// top, as Play on TV does.
#[cfg(feature = "cast")]
const CARRY_FROM: f64 = 5.0;

/// Stop casting: the playback on the TV stops, the TV goes back to what it showed, and plays
/// open on this device again.
#[cfg(feature = "cast")]
pub fn stop_casting(engine: &mut EngineState, services: &Services) {
    // A cast session ends with it: the receiver closes, and nothing is left for a later launch
    // to join, after a lost or taken-over cast too.
    let held = services.casts.disconnect();
    services.casts.forget_left();
    if engine.casting.take().is_none() && !services.casts.casting() && !held {
        return;
    }
    services.casts.end();
    control_player(PlayerCommand::Stop, engine);
    // A TV kept for the next episode is let go too.
    services.slot.release();
}

/// Play the release the user picked in the Sources overlay.
#[allow(clippy::too_many_arguments)]
pub fn play_source(
    id: AnilistId,
    episode: String,
    provider_id: String,
    source_id: String,
    job: Option<JobGuard>,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let Some(opener) = opener_for(engine, services) else {
        return playback::refuse(job, tx);
    };
    let billing = named(id, &episode, engine);
    let pick = Some(Pick::Release { provider_id, source_id });
    let staged = play_staged(
        id,
        episode,
        pick,
        Start::Resume,
        false,
        false,
        billing,
        opener,
        job,
        engine,
        services,
        tx,
    );
    let _ = tx.send(staged);
}

/// Play what a playback led to, which the engine starts on its own, in `opener`'s player: staged
/// as `follow` says, under the name the playback before it went by. Returns what it was staged
/// as, for the engine to say right after the event that led to it.
pub(crate) fn play_following(
    follow: Follow,
    opener: Arc<dyn PlayerOpener>,
    job: Option<JobGuard>,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) -> Event {
    let Follow { id, episode, title, episode_title, at } = follow;
    let billing = (title, episode_title);
    let start = at.map_or(Start::Resume, Start::At);
    play_staged(
        id, episode, None, start, false, true, billing, opener, job, engine, services, tx,
    )
}

/// What a play the front end asked for is staged as: the name the engine's record of the titles
/// gives it, and the episode's own name from the table it holds when that is this title's.
fn named(id: AnilistId, episode: &str, engine: &EngineState) -> (String, Option<String>) {
    (engine.cache.name_for(id, PlayKind::Staged), engine.cache.episode_title(id, episode))
}

/// The config a play of `id` runs under: the one in effect with what the show sets for itself
/// put in it, and sub or dub as the player last picked it for this title over the show's own.
pub(crate) fn play_config(
    engine: &EngineState,
    store: &kaeri_store::Store,
    id: AnilistId,
) -> Config {
    picked_over(
        show_settings::for_show(engine.config.effective(), store, id),
        id,
        engine.translation_picked,
    )
}

/// `config` for a play of `id`, with sub or dub as `picked` says when it was picked for `id`.
fn picked_over(
    mut config: Config,
    id: AnilistId,
    picked: Option<(AnilistId, kaeri_core::media::Translation)>,
) -> Config {
    if let Some((_, translation)) = picked.filter(|(picked, _)| *picked == id) {
        config.playback.translation = translation;
    }
    config
}

/// Whether the front end has title `id`'s table already, so a play of it says none of its own:
/// Now Playing shows a play of `id` that this one follows on from (rolled on, picked in the
/// player, or started again in sub or dub), or the title was listed for the front end since
/// the engine started (its page, a card, Continue), even while the record holds another title's
/// table or that listing is still on its way.
///
/// A listing names the rows its source left without a title, a still or a synopsis from another
/// source and the synopsis fill, which a play's own load does not wait for. Said over it, the
/// play's table would take those away from the page and the player's Episodes panel alike.
fn table_told(id: AnilistId, engine: &EngineState) -> bool {
    engine.staging.front().is_some_and(|front| front.id == id) || engine.listed.ever(id)
}

/// Say title `id`'s episode table as the engine's record holds it, with what has been watched
/// since it loaded, for the player's Episodes panel: a play from Continue, a widget or a link
/// opens on a title whose page was never opened. Nothing when the record holds another title's
/// table; the play loads its own and says that.
fn say_table(id: AnilistId, engine: &EngineState, tx: &Outbox) {
    if let Some(rows) = engine.cache.table_now(id) {
        let _ = tx.send(Event::Episodes { id, rows, failed: None });
    }
}

/// Stage a playback for Now Playing as `title`, with `episode_title`: the name the episode
/// after it goes by too, and the engine's record of what Now Playing shows. Returns what the
/// front end is told of it, for the caller to say in its place: a play that `opener` casts to
/// a TV is the cast card's to show, so the player here is not told of it.
fn stage(
    played: &Played,
    title: String,
    episode_title: Option<String>,
    resume: Option<f64>,
    opener: &dyn PlayerOpener,
    engine: &mut EngineState,
    private: bool,
) -> Event {
    engine.cache.announce(played.id, title.clone());
    // The player log's lines go under this playback from now on; a private one keeps none.
    crate::player_log::begin(&played.episode, &title, episode_title.as_deref(), private);
    engine.staging.staged(Staged {
        serial: played.serial,
        id: played.id,
        episode: played.episode.clone(),
        title: title.clone(),
        episode_title: episode_title.clone(),
    });
    let (id, episode) = (played.id, played.episode.clone());
    // What the player starts at when it remembers a speed, and Now Playing until it says.
    let speed = engine.config.effective().playback.carried_speed().unwrap_or(1.0);
    match opener.target() {
        PlayerTarget::Local => Event::PlaybackStaged {
            id,
            title,
            episode,
            episode_title,
            speed,
            resume,
            always_private: false,
        },
        PlayerTarget::Cast(_) => Event::CastStaged { id, title, episode, episode_title, speed },
    }
}

/// Where `episode` of title `id` was left, when it was: best-effort, as every read of the
/// resume point is, since a database hiccup must not stop an episode from playing.
///
/// A place a private play held in memory comes first: it is the newer.
fn resume_of(services: &Services, id: AnilistId, episode: &str) -> Option<f64> {
    if let Some(kept) = services.privacy.resume_kept(id, episode) {
        return Some(kept);
    }
    services
        .store
        .resume_position(id, episode)
        .inspect_err(|e| tracing::warn!(error = %e, "could not read resume position"))
        .unwrap_or(None)
}

/// Play a link the viewer opened as episode `episode` of title `id`, as they confirmed it: a
/// play like any other, with history, resume, trackers and sync, from the link rather than a
/// source. Refused with a notice while torrent playback is off. See [`crate::opened`].
#[allow(clippy::too_many_arguments)]
pub fn play_opened(
    link: String,
    id: AnilistId,
    episode: String,
    title: String,
    job: Option<JobGuard>,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    if !crate::opened::playback_on(engine) {
        let _ =
            tx.send(Event::Toast(Notice::alert(Msg::Store(StoreMsg::TorrentLinkPlaybackOff))));
        return;
    }
    let Some(opener) = opener_for(engine, services) else {
        return playback::refuse(job, tx);
    };
    let episode_title = engine.cache.episode_title(id, &episode);
    let pick = Some(Pick::Link(link));
    let staged = play_staged(
        id,
        episode,
        pick,
        Start::Resume,
        false,
        false,
        (title, episode_title),
        opener,
        job,
        engine,
        services,
        tx,
    );
    let _ = tx.send(staged);
}

/// Play a link the viewer opened with nothing recorded, staged as `name`: its playback goes by
/// [`kaeri_model::opened::UNTRACKED`]. Refused with a notice while torrent playback is off.
pub fn play_opened_untracked(
    link: String,
    name: String,
    job: Option<JobGuard>,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    if !crate::opened::playback_on(engine) {
        let _ =
            tx.send(Event::Toast(Notice::alert(Msg::Store(StoreMsg::TorrentLinkPlaybackOff))));
        return;
    }
    let Some(opener) = opener_for(engine, services) else {
        return playback::refuse(job, tx);
    };
    // The episode the name states picks the file in a season's torrent; none plays the
    // torrent's own choice.
    let episode = kaeri_providers::torrent::names::episode_in(&name)
        .map(|n| n.to_string())
        .unwrap_or_default();
    let pick = Some(Pick::Untracked { link, name: name.clone() });
    let id = kaeri_model::opened::UNTRACKED;
    let staged = play_staged(
        id,
        episode,
        pick,
        Start::Beginning,
        false,
        false,
        (name, None),
        opener,
        job,
        engine,
        services,
        tx,
    );
    let _ = tx.send(staged);
}

/// Where a play opens its episode.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Start {
    /// Where the episode was left.
    Resume,
    /// At the beginning, whatever position it was left at.
    Beginning,
    /// At this second: the playhead of the playback it takes over from, as a sub or dub picked
    /// in the player does.
    At(f64),
}

impl Start {
    /// The second the player opens at: `stored` is where the store says the episode was left,
    /// `kept` where a private play left it, held in memory and newer than any row.
    pub(crate) fn opens_at(self, stored: Option<f64>, kept: Option<f64>) -> Option<f64> {
        match self {
            Self::Resume => kept.or(stored),
            Self::Beginning => None,
            Self::At(at) => Some(at),
        }
    }
}

/// Play an episode staged for Now Playing as `billing` says, its title and its episode's own
/// name, from the release `pick` names, its finished download, or whichever the resolution
/// ladder chooses, and from where `start` says. Returns what it was staged as.
#[allow(clippy::too_many_arguments)]
fn play_staged(
    id: AnilistId,
    episode: String,
    // The release the user picked in the Sources overlay, or the link they opened.
    pick: Option<Pick>,
    start: Start,
    // Asked for by a party: an episode with no source here says so for the party's dialog.
    party: bool,
    // Led to by a playback of the same title, whose player has the title's table: rolled on,
    // picked in the player, or started again in sub or dub.
    follows: bool,
    (title, episode_title): (String, Option<String>),
    opener: Arc<dyn PlayerOpener>,
    job: Option<JobGuard>,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) -> Event {
    // A finished download plays from disk, offline too: unless a release was picked by hand,
    // the party plays, or the play goes to a TV.
    if pick.is_none()
        && !party
        && !casting(engine, services)
        && let Some(path) = downloaded(services, id, &episode)
    {
        let billing = (title, episode_title);
        return play_file(
            id, episode, path, start, follows, billing, opener, job, engine, services, tx,
        );
    }
    let Services { paths, sync, prefetch, store, .. } = services;
    // In a room, never this device's own resume point: the room's place, or the start.
    #[cfg(feature = "room")]
    let start = match (start, crate::room::opens_at(id, &episode, engine)) {
        (Start::Resume, Some(at)) => at.map_or(Start::Beginning, Start::At),
        (start, _) => start,
    };
    let played = engine.next_playback(id, &episode);
    let told = follows || table_told(id, engine);
    // Where the episode opens, for the starting screen to say it resumes there. Not for a play
    // from the beginning, or one that records nothing.
    let resume = match start {
        _ if id == kaeri_model::opened::UNTRACKED => None,
        Start::Resume => resume_of(services, id, &episode),
        Start::Beginning => None,
        Start::At(at) => Some(at),
    };
    let private = services.privacy.holds_show(played.id);
    let staged = stage(&played, title, episode_title, resume, opener.as_ref(), engine, private);
    // What the show sets for itself, over the config in effect: sub or dub, quality, what is
    // skipped.
    let for_show = play_config(engine, store, id);
    let hints =
        hints_for(id, &episode, PlayKind::Staged, for_show.playback.skip_filler, engine);
    // The player's Episodes panel has the title's whole table from the start.
    if !told && !Pick::untracked(pick.as_ref()) {
        say_table(id, engine, tx);
    }
    let switches = engine.switches.open(played.serial);
    let EngineState { player_tx, finding, skipping, beginning, .. } = engine;
    let config = &for_show;
    // A fresh channel per playback. The one playing gives up its window only once
    // this one has something to play — see `Handoff`.
    let (prx, handoff) = take_controls(player_tx, successor(id, &episode, &hints, config));
    let stopped = watch_for_stop(finding);
    let skip = watch_for_skip(skipping);
    let begin = watch_for_begin(beginning);
    // Never from the streams found ahead for a release picked by hand: the pick names its
    // own. Otherwise the same fresh channel, the same session-replacement semantics.
    let prefetched = match pick {
        None => prefetch.take(id, &episode, config.playback.translation),
        Some(_) => None,
    };
    // A roll-on handed over to this play: its first frame says how long that took.
    let rolled_at = prefetch.handover_for(id, &episode);
    // Tracker ids rather than trackers: playback only needs to know which queues to
    // append to, and passing the set would couple the player to the sync layer.
    let tracker_ids = signed_in_tracker_ids(sync);
    // An untracked play has no title to list the episodes of.
    if hints.episodes.is_empty() && !Pick::untracked(pick.as_ref()) {
        fill_episodes(id, &episode, told, services, player_tx, tx);
    }
    spawn_playback(
        id,
        episode,
        pick,
        start,
        party,
        prx,
        stopped,
        skip,
        (begin, switches),
        handoff,
        config,
        services,
        opener,
        tracker_ids,
        hints,
        write_shaders(config, paths),
        (prefetched, rolled_at),
        job,
        &tx.for_playback(played),
    );
    staged
}

/// Play a downloaded episode from disk, in the full playback session all the same.
#[allow(clippy::too_many_arguments)]
pub fn play_local(
    id: AnilistId,
    episode: String,
    title: String,
    path: String,
    job: Option<JobGuard>,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let Some(opener) = opener_for(engine, services) else {
        return playback::refuse(job, tx);
    };
    // Named by the download it plays: the Downloads screen says what its rows are called.
    let billing = (title, engine.cache.episode_title(id, &episode));
    let staged = play_file(
        id,
        episode,
        path,
        Start::Resume,
        false,
        billing,
        opener,
        job,
        engine,
        services,
        tx,
    );
    let _ = tx.send(staged);
}

/// The file a finished download of `episode` of `id` left on disk, if it is still there.
fn downloaded(services: &Services, id: AnilistId, episode: &str) -> Option<String> {
    let download = services.store.download_for(id, episode).ok().flatten()?;
    if download.state != kaeri_store::DownloadState::Done {
        return None;
    }
    let path = download.path?;
    std::path::Path::new(&path).exists().then_some(path)
}

/// Whether plays go to a TV now, which a file on this device's disk does not reach.
fn casting(engine: &EngineState, services: &Services) -> bool {
    #[cfg(feature = "cast")]
    {
        engine.casting.is_some()
            || services.casts.casting()
            || services.casts.session_device().is_some()
    }
    #[cfg(not(feature = "cast"))]
    {
        let _ = (engine, services);
        false
    }
}

/// Play the file at `path`, a downloaded episode, staged as `billing` says and from where
/// `start` says: the playback session a streamed play has, without the lookup. Returns what it
/// was staged as.
#[allow(clippy::too_many_arguments)]
fn play_file(
    id: AnilistId,
    episode: String,
    path: String,
    start: Start,
    // Led to by a playback of the same title, whose player has the title's table.
    follows: bool,
    (title, episode_title): (String, Option<String>),
    opener: Arc<dyn PlayerOpener>,
    job: Option<JobGuard>,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) -> Event {
    let Services { paths, http, store, sync, .. } = services;
    let played = engine.next_playback(id, &episode);
    let told = follows || table_told(id, engine);
    let for_show = play_config(engine, store, id);
    let hints =
        engine.cache.hints(id, &episode, for_show.playback.skip_filler, PlayKind::Staged);
    let resume = match start {
        Start::Resume => resume_of(services, id, &episode),
        Start::Beginning => None,
        Start::At(at) => Some(at),
    };
    let private = services.privacy.holds_show(played.id);
    let staged =
        stage(&played, title.clone(), episode_title, resume, opener.as_ref(), engine, private);
    if !told {
        say_table(id, engine, tx);
    }
    let switches = engine.switches.open(played.serial);
    let EngineState { player_tx, finding, skipping, beginning, .. } = engine;
    // Nothing to find: it opens at once, and a Stop is its session's from the start.
    *finding = None;
    *skipping = None;
    *beginning = None;
    let config = &for_show;
    // No provider walk and no torrent session — the file is on disk — but the full
    // playback session all the same: a downloaded episode records history, resumes,
    // and syncs exactly like a streamed one. It used to go straight to mpv with no
    // context, which made downloads second-class in every way that matters.
    // Nothing to look up, so nothing can fail before it plays.
    // Named by the download, as it is staged.
    let hints = PlayHints { title: title.clone(), ..hints };
    let (prx, handoff) = take_controls(player_tx, successor(id, &episode, &hints, config));
    let PlayHints { next, episodes, cover_url, banner_url, episode_total, .. } = hints;
    let tracker_ids = signed_in_tracker_ids(sync);
    if episodes.is_empty() {
        fill_episodes(id, &episode, told, services, player_tx, tx);
    }
    let shaders = write_shaders(config, paths);
    let party = services.party.clone();
    // Private as any play is: what it would record of the show is held while the show is, and
    // nothing is listened for to find skip times.
    let privacy = services.privacy.gate(id);
    let find = !services.privacy.holds_show(id) && services.skip_find.runs(config);
    let (config, store, http, machine, tx) = (
        config.clone(),
        store.clone(),
        http.clone(),
        machine(opener, services),
        tx.for_playback(played),
    );
    tokio::spawn(async move {
        let _job = job;
        let playback = config.playback.clone();
        let stream = kaeri_core::stream::Stream::new(path, kaeri_core::stream::StreamKind::Mp4);
        // Best-effort, exactly as the streaming path treats it.
        let mal_id = store.mapping_for(id).ok().flatten().and_then(|m| m.mal_id);
        let context = playback::PlaybackContext {
            anilist_id: id,
            mal_id,
            episode,
            title,
            translation: playback.translation,
            // Where it was staged from: a place a private play kept in memory comes first.
            resume_at: resume,
            // A local file's real length is knowable, but the hint only serves
            // the aniskip fallback and a lie would filter its answers.
            runtime_secs: None,
            speed: playback.carried_speed(),
            volume: playback.persist_volume.then_some(playback.persisted_volume).flatten(),
            next,
            episodes,
            cover_url,
            banner_url,
            episode_total,
        };
        let settings = playback::PlaySettings {
            privacy,
            shader_dir: shaders.await.ok().flatten(),
            find_skip_times: find,
            switches,
            ..play_settings(&config, tracker_ids, party)
        };
        // The window is asked for and waited on, as a streamed play's is: claimed while the
        // playback before still opened its own, it opened a second one beside it.
        handoff.complete().await;
        playback::play(vec![stream], context, store, http, machine, settings, tx, prx).await;
    });
    staged
}

/// Write the shaders out for a playback, on the blocking pool rather than on the engine's own
/// task: the first play in a new cache writes every one of them, and while a call blocks that
/// task the engine routes no command and passes on no event. Awaited once the playback has its
/// streams, so the writing overlaps the lookup. A write that panics leaves upscaling off, as
/// one that fails does.
fn write_shaders(
    config: &Config,
    paths: &kaeri_core::config::Paths,
) -> JoinHandle<Option<std::path::PathBuf>> {
    let (config, paths) = (config.clone(), paths.clone());
    tokio::task::spawn_blocking(move || shader_dir(&config, &paths))
}

/// What a playback runs on: `opener`'s player, and the engine's slot, host and power source.
fn machine(opener: Arc<dyn PlayerOpener>, services: &Services) -> playback::Machine {
    #[cfg(feature = "testing")]
    let opener: Arc<dyn PlayerOpener> = match &services.windows {
        Some(opened) => {
            Arc::new(crate::playback::link::Counted { inner: opener, opened: opened.clone() })
        }
        None => opener,
    };
    playback::Machine {
        opener,
        slot: services.slot.clone(),
        host: services.host.clone(),
        power: services.power.clone(),
        offline: services.offline.clone(),
        sources: Some(services.registry.clone()),
    }
}

/// Steer whatever is playing, through the controls of the playback started last.
///
/// A Stop is that playback's whether or not it has opened yet: one still finding its stream
/// reads its controls only once it plays, so it is told at once as well, and opens nothing.
pub fn control_player(command: PlayerCommand, engine: &EngineState) {
    let EngineState { player_tx, finding, skipping, beginning, .. } = engine;
    if command == PlayerCommand::PlayFromBeginning
        && let Some(beginning) = beginning
    {
        // Read when the file is opened; a file already open hears the command as well.
        beginning.store(true, std::sync::atomic::Ordering::SeqCst);
    }
    if command == PlayerCommand::Stop
        && let Some(finding) = finding
    {
        finding.send_replace(true);
    }
    // Heard by the walk through the sources while one is under way, and by the playback until
    // its first frame.
    if command == PlayerCommand::SkipSource {
        if let Some(skipping) = skipping.as_ref().filter(|skipping| !skipping.is_closed()) {
            skipping.send_modify(|count| *count += 1);
        }
        return;
    }
    if let Some(sender) = player_tx.as_ref() {
        let _ = sender.send(command);
    }
}

/// Steer `playback`, when it is the playback started last, which holds the controls. A command
/// aimed at one another has followed since is let go: a Play Now or an Undo that a hosted
/// player sent as its episode rolled on to the next was meant for the episode it showed.
///
/// But for a Stop while the playback started last is still finding its stream. The viewer
/// stopped what was on screen, which was that one's starting screen, or the episode it is
/// about to take the window from: the player it would open is what they turned down.
pub fn control_playback(playback: PlaybackId, command: PlayerCommand, engine: &EngineState) {
    let current = PlaybackId::new(engine.playbacks);
    let finding = engine.finding.as_ref().is_some_and(|finding| !finding.is_closed());
    if current != playback && !(command == PlayerCommand::Stop && playback < current && finding)
    {
        tracing::debug!(
            playback = playback.get(),
            current = engine.playbacks,
            ?command,
            "a command for a playback that is no longer current"
        );
        return;
    }
    if current != playback {
        tracing::info!(
            playback = playback.get(),
            current = engine.playbacks,
            "a stop for the playback before, while this one finds its stream: stopping it"
        );
    }
    control_player(command, engine);
}

/// A fresh watch for a Stop that reaches the playback starting now before it opens, kept as the
/// engine's `finding` in place of the one before it. See [`control_player`].
fn watch_for_stop(finding: &mut Option<watch::Sender<bool>>) -> watch::Receiver<bool> {
    let (stop, stopped) = watch::channel(false);
    *finding = Some(stop);
    stopped
}

/// A fresh count for the skip that reaches the playback starting now, kept as the engine's
/// `skipping` in place of the one before it.
fn watch_for_skip(skipping: &mut Option<watch::Sender<u64>>) -> watch::Receiver<u64> {
    let (skip, skipped) = watch::channel(0);
    *skipping = Some(skip);
    skipped
}

/// A fresh flag for the begin-at-0 that reaches the playback starting now, kept as the engine's
/// `beginning` in place of the one before it.
fn watch_for_begin(
    beginning: &mut Option<Arc<std::sync::atomic::AtomicBool>>,
) -> Arc<std::sync::atomic::AtomicBool> {
    let begin = Arc::new(std::sync::atomic::AtomicBool::new(false));
    *beginning = Some(begin.clone());
    begin
}

/// What ended a playback's wait for its stream before the stream was found.
enum Overtaken {
    /// The viewer stopped it.
    Stopped,
    /// A newer playback was asked for, which took its controls and dropped its watch.
    Superseded,
}

/// Until a Stop reaches the playback `stopped` watches for, or a newer playback replaces the
/// watch, which only a newer one does.
async fn stop_asked(stopped: &mut watch::Receiver<bool>) -> Overtaken {
    match stopped.wait_for(|stop| *stop).await {
        Ok(_) => Overtaken::Stopped,
        Err(_) => Overtaken::Superseded,
    }
}

/// Whether a newer playback has replaced the watch `stopped` reads.
fn superseded(stopped: &watch::Receiver<bool>) -> bool {
    stopped.has_changed().is_err() && !*stopped.borrow()
}

/// A playback stopped before it opened a player: it says it ended, stopped, and nothing opens.
/// A window parked for it would wait for nothing.
fn stopped_unopened(slot: &PlayerSlot, tx: &Outbox) {
    tracing::info!("stopped while finding its stream; no player opens for it");
    slot.release();
    let _ = tx.send(Event::playback_ended(false, kaeri_model::playback::Ending::Stopped));
}

/// Resolve a stream and start playing it.
///
/// Everything slow happens inside the spawned task, behind the eyecatch: matching the title,
/// walking the provider chain, and reading back where you left off.
#[allow(clippy::too_many_arguments)]
fn spawn_playback(
    id: kaeri_core::ids::AnilistId,
    episode: String,
    // The release the user picked in the Sources overlay, or the link they opened; `None`
    // lets the resolution ladder choose.
    pick: Option<Pick>,
    // Where the player opens the episode.
    start: Start,
    // Asked for by a party: an episode with no source here says so, for the party's dialog
    // that offers Choose source.
    party: bool,
    commands: mpsc::UnboundedReceiver<kaeri_model::command::PlayerCommand>,
    // Whether a Stop reached this playback before it opened: see `control_player`.
    mut stopped: watch::Receiver<bool>,
    // The count the walk through the sources watches for a skip of the source it is asking.
    skip: watch::Receiver<u64>,
    // Whether the viewer asked to begin at 0: see `PlayerCommand::PlayFromBeginning`; and the
    // playback's end of its door, where a release picked in the player reaches it.
    (begin, switches): (Arc<std::sync::atomic::AtomicBool>, SwitchInbox),
    handoff: Handoff,
    config: &Config,
    services: &Services,
    opener: Arc<dyn PlayerOpener>,
    tracker_ids: Vec<String>,
    // What the player's Up Next card, its Episodes menu and its starting screen show, from the
    // engine's record of the titles.
    hints: PlayHints,
    // The shaders being written for it, while it looks for its streams.
    shaders: JoinHandle<Option<std::path::PathBuf>>,
    // This episode as it was made ready while the previous one's ending played, and when the
    // roll-on handed over to it.
    (prefetched, rolled_at): (Option<Warm>, Option<std::time::Instant>),
    job: Option<JobGuard>,
    tx: &Outbox,
) {
    let untracked = Pick::untracked(pick.as_ref());
    let from_sources = matches!(pick, None | Some(Pick::Release { .. }));
    // An untracked play keeps no seat at a party: the room would be told of a title it is not.
    let hub = if untracked { Default::default() } else { services.party.clone() };
    // A cast with Torrents while casting off passes torrents over, and the next source plays.
    let no_torrents = matches!(opener.target(), crate::playback::link::PlayerTarget::Cast(_))
        && !kaeri_model::settings::while_casting(config);
    let registry = if no_torrents {
        services.registry.without_torrents()
    } else {
        services.registry.clone()
    };
    let prefetched =
        if no_torrents { prefetched.and_then(Warm::without_torrents) } else { prefetched };
    // From here the playback owns what was started ahead: the proxy its first attempt adopts.
    let (prefetched, proxy) = match prefetched.map(Warm::into_parts) {
        Some((resolved, proxy)) => (Some(resolved), proxy),
        None => (None, None),
    };
    let ahead =
        crate::playback::prefetch::Ahead::new(rolled_at, proxy, services.prefetch.clone());
    let reports = services.servers.reports();
    // An untracked play is of no show whose episodes could be compared, and a private one keeps
    // no prints of what it played.
    let find =
        !untracked && !services.privacy.holds_show(id) && services.skip_find.runs(config);
    let privacy = services.privacy.clone();
    let mut link = services.connection.subscribe();
    let torrents = services.registry.torrents().clone();
    let unreached = services.unreached.clone();
    let (config, meta, store, registry, http, machine, slot, tx) = (
        config.clone(),
        services.meta.clone(),
        services.store.clone(),
        registry,
        services.http.clone(),
        machine(opener, services),
        services.slot.clone(),
        tx.clone(),
    );

    tokio::spawn(async move {
        let playback = config.playback.clone();
        // The skip as the playback hears it once its streams are found, beside the walk's.
        let skips = skip.clone();
        let starting = Starting::new(&tx, id, &episode, Some(skip));
        // Try again: what the last failed start of this episode couldn't reach is asked anew.
        unreached.retry(id, &episode, &registry);
        // A torrent looked up ahead on a limited network starts now, as it would have had it
        // been found now. One that does not start sends the play looking again.
        let prefetched = match prefetched {
            Some((streams, context))
                if streams
                    .iter()
                    .any(|s| s.kind == kaeri_core::stream::StreamKind::Torrent) =>
            {
                match torrents.start_first(streams, &episode).await {
                    Ok(streams) => Some((streams, context)),
                    Err(error) => {
                        tracing::info!(%error, "the torrent looked up ahead did not start");
                        None
                    }
                }
            }
            prefetched => prefetched,
        };
        let resolving = async {
            match prefetched {
                Some(found) => {
                    tracing::info!(episode = %episode, "playing from the streams found ahead");
                    Ok(found)
                }
                None => {
                    // Offline, or offline again before any source is asked, the start waits
                    // for a connection for up to two minutes and carries on by itself when it
                    // is back. Stopped meanwhile, the select below ends it at once.
                    let waited = starting
                        .when_connected(
                            &mut link,
                            crate::playback::starting::OFFLINE_WAIT,
                            || {
                                resolve_for_playback(
                                    &meta,
                                    &store,
                                    &registry,
                                    id,
                                    &episode,
                                    pick.clone(),
                                    &playback,
                                    job.as_ref(),
                                    Some(&starting),
                                )
                            },
                        )
                        .await;
                    match waited {
                        Some(resolved) => resolved,
                        None => {
                            tracing::info!(episode = %episode, "no connection came back in time");
                            Err(Unplayable::no_connection())
                        }
                    }
                }
            }
        };
        // Stopped while it looks: the viewer closed its starting screen, or stopped the episode
        // it was to take the window from. Read in the controls, it would open the player first.
        let resolved = tokio::select! {
            biased;
            overtaken = stop_asked(&mut stopped) => Err(overtaken),
            resolved = resolving => Ok(resolved),
        };
        let resolved = match resolved {
            Ok(resolved) => resolved,
            // The episode it was to take over stops with it: what the viewer stopped.
            Err(Overtaken::Stopped) => {
                handoff.stop();
                // Its controls closed before its ending is said: see `Handoff::live`.
                drop(commands);
                stopped_unopened(&slot, &tx);
                return;
            }
            // Asked past, as N pressed again: the newer playback takes the window from the
            // episode on screen through this one's controls, and this one finds nothing more.
            // It says no ending, since its successor is the playback the apps now show.
            Err(Overtaken::Superseded) => {
                tracing::info!(episode = %episode, "a newer playback was asked for; giving way");
                drop(job);
                handoff.pass_on(commands).await;
                return;
            }
        };
        // Try again gives what this start couldn't reach another chance.
        let context = resolved.inspect_err(|reason| {
            unreached.note(id, &episode, reason.unreached.clone());
        });
        let context = match context {
            Ok(pair) => pair,
            // The episode being watched carries on, and the window with it: only the lookup
            // failed, and closing what was playing over it would punish the viewer for asking.
            Err(reason) if handoff.live() => {
                // In a party the dialog says why, and a toast would say it twice.
                // The kind comes as the lookup decided it.
                if party {
                    let _ = tx.send(Event::RoomNoSource {
                        id,
                        episode: episode.clone(),
                        why: reason.why,
                        reason: reason.text.clone(),
                    });
                } else {
                    let _ = tx.send(Event::Toast(Notice::alert(reason.text.clone())));
                }
                let _ = tx.send(Event::PlaybackKept);
                handoff.keep(commands, reason.text.english()).await;
                return;
            }
            Err(reason) => {
                // In the room all the same, with nothing playing: the party's dialog offers
                // Choose source.
                // The failure ladder's first rung: name the failure rather than showing an
                // empty screen. Releasing the eyecatch is what the alert does for us. In a
                // party the dialog names it instead, and the ending below releases it.
                let failure = kaeri_model::starting::StartFailure {
                    why: reason.why,
                    text: reason.text.clone(),
                    asked: starting.asked(),
                    unreached: reason
                        .unreached
                        .iter()
                        .map(|provider_id| {
                            crate::playback::resolve::display_name(&registry, provider_id)
                        })
                        .collect(),
                    held_back: starting.held_back(),
                };
                if party {
                    let _ = tx.send(Event::RoomNoSource {
                        id,
                        episode: episode.clone(),
                        why: reason.why,
                        reason: reason.text,
                    });
                } else {
                    let _ = tx.send(Event::Toast(Notice::alert(reason.text)));
                }
                // A window left open for this episode would wait for nothing.
                slot.release();
                // Its controls closed before its ending is said: a play asked for on hearing
                // the ending takes this one for over only once they are (`Handoff::live`).
                drop(commands);
                let _ = tx.send(Event::playback_failed(failure));
                return;
            }
        };
        let (streams, mut context) = context;
        // A skip, or Switch source on a stall, with no stream of these left goes on to the
        // sources after the one that answered, as the walk would have. Not past a release the
        // viewer picked or a link they opened: those are the stream they asked for.
        let further = if pick.is_none() {
            let onward = Onward {
                meta: meta.clone(),
                store: store.clone(),
                registry: registry.clone(),
                id,
                episode: episode.clone(),
                playback: playback.clone(),
                starting: starting.clone(),
            };
            Further::new(skips, Arc::new(onward))
        } else {
            Further::skips_only(skips)
        };
        // Streams from a source, not a link the viewer opened: a miss Home kept is a find now.
        if from_sources && !streams.is_empty() {
            crate::catalog::probe::listed(&store, &tx, id);
        }
        // Found, from the ladder just now or from ahead: the screen's step is now the player
        // opening the first stream.
        if let Some(first) = streams.first() {
            let source =
                registry.get(&first.provider_id).map(|s| s.manifest().display_name.clone());
            starting.step(
                kaeri_model::starting::StartStep::Found {
                    count: u32::try_from(streams.len()).unwrap_or(u32::MAX),
                    source,
                    quality: first.quality,
                    translation: context.translation,
                },
                Some(Msg::StartOpeningStream),
            );
        }
        // Played from the beginning: no resume point, so the player opens at nought, nothing
        // says it resumed, and the history it writes as it plays is where it is left next.
        context.resume_at =
            start.opens_at(context.resume_at, privacy.resume_kept(id, &context.episode));
        let PlayHints { next, episodes, cover_url, banner_url, episode_total, .. } = hints;
        (context.next, context.episodes) = (next, episodes);
        // The art the title's lookup found, else what its page or a list showed.
        context.cover_url = context.cover_url.or(cover_url);
        context.episode_total = context.episode_total.or(episode_total);
        context.banner_url = context.banner_url.or(banner_url);
        // Resolved: from here the wait is mpv opening, which Now Playing shows itself, and the
        // session lasts the episode — far too long to hold a "starting" job open for.
        drop(job);
        // Say why this release, so an automatic pick is never a mystery. The active-provider
        // header is sent from playback itself, per attempt, so failover never credits the
        // stream that just failed.
        if let Some(note) = streams.first().and_then(|s| s.pick_note.clone()) {
            let _ = tx.send(Event::Toast(Notice::info(Msg::verbatim(note))));
        }

        let shader_dir = shaders.await.ok().flatten();
        let settings = playback::PlaySettings {
            shader_dir,
            reports,
            find_skip_times: find,
            swarm: crate::playback::starting::Swarm::of(torrents),
            begin,
            switches,
            ahead,
            privacy: privacy.gate(id),
            further,
            ..play_settings(&config, tracker_ids, hub)
        };
        let settings = if untracked { settings.untracked() } else { settings };
        handoff.complete().await;
        // Stopped while the window was handed over: the one parked for it closes.
        if *stopped.borrow() {
            stopped_unopened(&slot, &tx);
            return;
        }
        // Asked past while the window was handed over: the window stays parked for the newer
        // playback, which is waiting for these controls to close.
        if superseded(&stopped) {
            tracing::info!(episode = %episode, "a newer playback was asked for; leaving it the window");
            return;
        }
        // Opening from here: a Stop is the session's, read off its controls.
        drop(stopped);
        playback::play(streams, context, store, http, machine, settings, tx, commands).await;
    });
}

/// The sources a playback asks once the viewer has moved on from every stream it found: the
/// same lookup its start made, past the sources it has already asked or played from.
struct Onward {
    meta: kaeri_meta::Meta,
    store: kaeri_store::Store,
    registry: kaeri_providers::ProviderRegistry,
    id: AnilistId,
    episode: String,
    playback: kaeri_core::config::PlaybackConfig,
    starting: Starting,
}

impl Onward {
    /// The sources passed over: those `streams` came from, and those already asked.
    fn passed(&self, streams: &[kaeri_core::stream::Stream]) -> Vec<String> {
        let asked = self.starting.asked();
        let mut passed: Vec<String> = streams.iter().map(|s| s.provider_id.clone()).collect();
        passed.extend(self.registry.ids().into_iter().filter(|id| {
            self.registry
                .get(id)
                .is_some_and(|source| asked.contains(&source.manifest().display_name))
        }));
        passed
    }

    /// A source by the name it goes by on screen.
    fn name(&self, provider_id: &str) -> String {
        self.registry
            .get(provider_id)
            .map_or_else(|| provider_id.to_owned(), |s| s.manifest().display_name.clone())
    }
}

#[async_trait::async_trait]
impl Lookup for Onward {
    fn beyond(&self, streams: &[kaeri_core::stream::Stream]) -> bool {
        !self.registry.without(&self.passed(streams)).usable_ids().is_empty()
    }

    async fn after(
        &self,
        streams: &[kaeri_core::stream::Stream],
        skipped: Option<&str>,
    ) -> Result<Vec<kaeri_core::stream::Stream>, kaeri_model::starting::StartFailure> {
        use kaeri_model::starting::{SourceMiss, StartStep};
        if let Some(skipped) = skipped {
            let name = self.name(skipped);
            self.starting
                .step(StartStep::SourceMissed { name, miss: SourceMiss::Skipped }, None);
        }
        let registry = self.registry.without(&self.passed(streams));
        tracing::info!(sources = ?registry.usable_ids(), "asking the sources after the streams passed");
        let found = resolve_for_playback(
            &self.meta,
            &self.store,
            &registry,
            self.id,
            &self.episode,
            None,
            &self.playback,
            None,
            Some(&self.starting),
        )
        .await;
        match found {
            Ok((streams, context)) if !streams.is_empty() => {
                let first = &streams[0];
                self.starting.step(
                    StartStep::Found {
                        count: u32::try_from(streams.len()).unwrap_or(u32::MAX),
                        source: Some(self.name(&first.provider_id)),
                        quality: first.quality,
                        translation: context.translation,
                    },
                    Some(Msg::StartOpeningStream),
                );
                Ok(streams)
            }
            Ok(_) => Err(kaeri_model::starting::StartFailure {
                why: kaeri_model::room::NoSourceWhy::NotFound,
                text: crate::playback::resolve::explained(kaeri_model::Cause::NoSources),
                asked: self.starting.asked(),
                unreached: Vec::new(),
                held_back: self.starting.held_back(),
            }),
            Err(reason) => Err(kaeri_model::starting::StartFailure {
                why: reason.why,
                text: reason.text,
                asked: self.starting.asked(),
                unreached: reason
                    .unreached
                    .iter()
                    .map(|provider_id| {
                        crate::playback::resolve::display_name(&self.registry, provider_id)
                    })
                    .collect(),
                held_back: self.starting.held_back(),
            }),
        }
    }
}

/// What one playback runs under, from the config as it stands now.
pub fn play_settings(
    config: &Config,
    tracker_ids: Vec<String>,
    party: playback::seat::PartyHub,
) -> playback::PlaySettings {
    let playback = &config.playback;
    playback::PlaySettings {
        threshold: playback.commit_threshold,
        auto_skip: playback.skip_opening,
        auto_skip_recap: playback.skip_recap_auto,
        ask_was_opening: playback.ask_was_opening,
        // Whether the player can hand the audio over, and the device can spare the work, is
        // the engine's to say: see `SkipFind::runs`.
        find_skip_times: false,
        share_skip_times: playback.share_skip_times,
        mark_chapters: playback.mark_chapters,
        skip_button: playback.skip_button,
        fullscreen: playback.fullscreen,
        subtitle_language: Some(playback.subtitle_language.clone()),
        auto_next: playback.auto_next,
        night_mode: playback.night_mode,
        remember_tracks: playback.persist_speed,
        look: playback.upscaling.into(),
        // Filled in by the caller, which knows where the cache is.
        shader_dir: None,
        tone_mapping: playback.tone_mapping.clone(),
        tracker_ids,
        presence: config.presence.clone(),
        reports: Default::default(),
        party,
        up_next_countdown: playback.up_next_countdown,
        mpv_options: kaeri_playback::protocol::option_pairs(&playback.mpv_args),
        untracked: false,
        swarm: Default::default(),
        begin: Default::default(),
        ahead: Default::default(),
        privacy: Default::default(),
        further: Default::default(),
        switches: Default::default(),
    }
}

/// Load a title's episode list for a playback the episode table on screen could not describe.
///
/// The table belongs to whichever title is open, so a playback started from Home or Continue
/// had no episode list: the player's Episodes menu was empty and, with no next episode known,
/// there was no Up Next card, no fetching ahead and no keeping the window for the binge.
/// Loaded in the background after playback starts, never in front of it, and handed to the
/// playback through its controls, and said to the front end unless `told`, as [`table_told`]
/// has it.
fn fill_episodes(
    id: kaeri_core::ids::AnilistId,
    episode: &str,
    told: bool,
    services: &Services,
    controls: &Option<mpsc::UnboundedSender<kaeri_model::command::PlayerCommand>>,
    tx: &Outbox,
) {
    let Some(controls) = controls.clone() else { return };
    let (meta, store, registry, episode, tx) = (
        services.meta.clone(),
        services.store.clone(),
        services.registry.clone(),
        episode.to_owned(),
        tx.clone(),
    );
    tokio::spawn(async move {
        match load_episodes(&meta, &store, &registry, id).await {
            Ok(EpisodeLoad::Rows(rows)) if !rows.is_empty() => {
                // The front end's table too, for the player's Episodes panel, unless the
                // play follows on from one that has it.
                if !told {
                    let _ = tx.send(Event::Episodes { id, rows: rows.clone(), failed: None });
                }
                let next = following(&rows, &episode);
                let _ =
                    controls.send(kaeri_model::command::PlayerCommand::Episodes { rows, next });
            }
            Ok(_) => {}
            Err(reason) => tracing::debug!(%reason, "no episode list for the player"),
        }
    });
}

/// The episode after `episode` in a list, when the list has it.
fn following(rows: &[EpisodeRow], episode: &str) -> Option<String> {
    let next = episode.trim().parse::<u32>().ok()?.checked_add(1)?.to_string();
    rows.iter().any(|row| row.number == next && !row.unsourced).then_some(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_play_opens_where_it_was_left_at_the_beginning_or_at_a_playhead_it_takes_over() {
        let (stored, kept) = (Some(300.0), Some(420.0));
        // A private play's point, held in memory, is newer than the store's.
        assert_eq!(Start::Resume.opens_at(stored, kept), Some(420.0));
        assert_eq!(Start::Resume.opens_at(stored, None), Some(300.0));
        assert_eq!(Start::Beginning.opens_at(stored, kept), None);
        // A sub or dub picked in the player opens at the playhead it left, whatever was stored.
        assert_eq!(Start::At(612.5).opens_at(stored, kept), Some(612.5));
        assert_eq!(Start::At(612.5).opens_at(None, None), Some(612.5));
    }

    #[test]
    fn sub_or_dub_picked_in_the_player_holds_over_the_show_s_own_for_that_title_only() {
        use kaeri_core::media::Translation;
        let (id, other) = (AnilistId::new(7), AnilistId::new(8));
        let store = kaeri_store::Store::open_in_memory().unwrap();
        store.set_show_setting(id, "translation", Some("dub"), 0).unwrap();
        let show = show_settings::for_show(&Config::default(), &store, id);
        assert_eq!(show.playback.translation, Translation::Dub, "the show's own setting");

        let picked = Some((id, Translation::Sub));
        assert_eq!(
            picked_over(show.clone(), id, picked).playback.translation,
            Translation::Sub
        );
        // A pick for another title leaves this one's as the show has it.
        let elsewhere = Some((other, Translation::Sub));
        assert_eq!(
            picked_over(show.clone(), id, elsewhere).playback.translation,
            Translation::Dub
        );
        assert_eq!(picked_over(show, id, None).playback.translation, Translation::Dub);
    }

    #[test]
    fn the_following_episode_is_only_offered_when_the_list_has_it() {
        let rows: Vec<EpisodeRow> = ["1", "2", "3"]
            .into_iter()
            .map(|n| EpisodeRow {
                number: n.into(),
                title: None,
                thumbnail: None,
                description: None,
                duration_secs: None,
                watched: 0.0,
                completed: false,
                kind: None,
                skippable: false,
                airs_at: None,
                delayed_from: None,
                metadata_only: false,
                source: None,
                unsourced: false,
                past_progress: false,
                spoiler_ahead: false,
                synopsis_pending: false,
                followed: Vec::new(),
            })
            .collect();
        assert_eq!(following(&rows, "1").as_deref(), Some("2"));
        assert_eq!(following(&rows, "3"), None, "a finale has no next");
        assert_eq!(following(&rows, "OVA"), None);
    }

    #[test]
    fn up_next_counts_down_what_the_setting_says() {
        let mut config = Config::default();
        assert_eq!(
            play_settings(&config, Vec::new(), Default::default()).up_next_countdown,
            playback::UP_NEXT_COUNTDOWN,
        );
        config.playback.up_next_countdown = 20;
        assert_eq!(
            play_settings(&config, Vec::new(), Default::default()).up_next_countdown,
            20
        );
        config.playback.up_next_countdown = 0;
        assert_eq!(
            play_settings(&config, Vec::new(), Default::default()).up_next_countdown,
            0,
            "off waits for Play now"
        );
    }
}
