//! Another release of the episode playing, from where its playhead is: a pick in the player's
//! Sources panel, [`Command::SwitchSource`].
//!
//! The engine finds the release while the episode plays on, and hands its streams to the
//! playback through a door of its own, which every playback has: the playback then loads them
//! where it is, as it does a stream picked from its Sources menu, and goes back to what it played
//! when they show no frame. All of it is said on [`Event::PlayerSwitch`]. Nothing is written of
//! the pick: the title's pinned source and release stay as they are, and the next episode is
//! found as it would have been.
//!
//! [`Command::SwitchSource`]: kaeri_model::command::Command::SwitchSource
//! [`Event::PlayerSwitch`]: kaeri_model::event::Event::PlayerSwitch

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use kaeri_core::{config::PlaybackConfig, ids::AnilistId, media::Translation, stream::Stream};
use kaeri_meta::Meta;
use kaeri_model::{Msg, playback::PlaybackId, playback::SwitchTo};
use kaeri_providers::ProviderRegistry;
use kaeri_store::Store;
use tokio::sync::mpsc;

use crate::{
    jobs::JobGuard,
    outbox::Outbox,
    playback::resolve::{Pick, display_name, resolve_for_playback},
    services::Services,
    state::EngineState,
};

/// What a playback hears of a release the engine finds for it.
#[derive(Debug)]
pub enum SwitchNews {
    /// The release is being found.
    Finding(SwitchTo),
    /// Found: its streams, as the source gave them.
    Found(SwitchTo, Vec<Stream>),
    /// It could not be found, for this reason.
    Failed(SwitchTo, Msg),
}

/// The engine's ends of the playbacks' doors, by playback: where a release found for the episode
/// a playback plays is handed to it.
#[derive(Debug, Default)]
pub struct Doors(HashMap<PlaybackId, mpsc::UnboundedSender<SwitchNews>>);

impl Doors {
    /// A door for the playback `serial`, about to start: the playback's end of it, which it takes
    /// as it starts playing. The doors of playbacks that have ended are let go.
    pub fn open(&mut self, serial: PlaybackId) -> SwitchInbox {
        self.0.retain(|_, door| !door.is_closed());
        let (door, inbox) = mpsc::unbounded_channel();
        self.0.insert(serial, door);
        SwitchInbox(Arc::new(Mutex::new(Some(inbox))))
    }

    /// The door of `serial`, while that playback runs.
    fn of(&self, serial: PlaybackId) -> Option<mpsc::UnboundedSender<SwitchNews>> {
        self.0.get(&serial).filter(|door| !door.is_closed()).cloned()
    }
}

/// A playback's end of its door, taken once as it starts. None for a playback started with no
/// door, as the terminal's probes start one.
#[derive(Debug, Clone, Default)]
pub struct SwitchInbox(Arc<Mutex<Option<mpsc::UnboundedReceiver<SwitchNews>>>>);

impl SwitchInbox {
    /// The news for this playback, once.
    pub(crate) fn take(&self) -> Option<mpsc::UnboundedReceiver<SwitchNews>> {
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take()
    }

    /// A door and its inbox, for a test that speaks for the engine.
    #[cfg(test)]
    pub(crate) fn pair() -> (mpsc::UnboundedSender<SwitchNews>, Self) {
        let (door, inbox) = mpsc::unbounded_channel();
        (door, Self(Arc::new(Mutex::new(Some(inbox)))))
    }
}

/// The door of the playback Now Playing shows, when it plays `episode` of `id` and has started:
/// one still finding its stream has nothing to switch from.
fn door_for(
    engine: &EngineState,
    id: AnilistId,
    episode: &str,
) -> Option<mpsc::UnboundedSender<SwitchNews>> {
    let front = engine.staging.front()?;
    let playing =
        front.id == id && front.episode == episode && engine.cache.under_way(front.serial);
    playing.then(|| engine.switches.of(front.serial)).flatten()
}

/// Switch the episode playing to `source_id` of `provider_id`: found while it plays on, then
/// handed to its playback. With no playback of `episode` of `id` under way, a play of that
/// release, as [`Command::PlaySource`](kaeri_model::command::Command::PlaySource) is.
#[allow(clippy::too_many_arguments)]
pub fn ask(
    id: AnilistId,
    episode: String,
    provider_id: String,
    source_id: String,
    job: Option<JobGuard>,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let Some(door) = door_for(engine, id, &episode) else {
        // As the dispatch of `PlaySource` does before it plays.
        engine.dubs.stop();
        engine.names.stop();
        #[cfg(feature = "party")]
        crate::party::join_by_default(engine, services, tx);
        return crate::playback::start::play_source(
            id,
            episode,
            provider_id,
            source_id,
            job,
            engine,
            services,
            tx,
        );
    };
    let to = SwitchTo {
        name: display_name(&services.registry, &provider_id),
        provider_id: provider_id.clone(),
        release: Some(source_id.clone()),
        ..SwitchTo::default()
    };
    let _ = door.send(SwitchNews::Finding(to.clone()));
    // Sub or dub as the playback runs it: the show's own, or what the player last picked.
    let playback = crate::playback::start::play_config(engine, &services.store, id).playback;
    let Services { meta, store, registry, .. } = services;
    let (meta, store, registry) = (meta.clone(), store.clone(), registry.clone());
    tokio::spawn(async move {
        let _job = job;
        let release = (provider_id.as_str(), source_id.as_str());
        let news = match find(&meta, &store, &registry, id, &episode, release, &playback).await
        {
            Ok(streams) => SwitchNews::Found(to, streams),
            Err(why) => SwitchNews::Failed(to, why),
        };
        let _ = door.send(news);
    });
}

/// Switch the episode playing to `translation`, sub or dub picked in the player: found while
/// it plays on, then handed to its playback, which loads it at the playhead as it does a
/// release. False when no playback of the episode Now Playing shows is under way, for the
/// caller to start the episode again in `translation` instead.
///
/// The version is found as a play of the episode would find it, from the title's sources in
/// their order, with `translation` in place of the title's own. The source is not known until
/// it is found: `Finding` names none.
pub fn translate(translation: Translation, engine: &EngineState, services: &Services) -> bool {
    let Some(front) = engine.staging.front() else { return false };
    let (id, episode) = (front.id, front.episode.clone());
    let Some(door) = door_for(engine, id, &episode) else { return false };
    let to = SwitchTo { translation: Some(translation), ..SwitchTo::default() };
    let _ = door.send(SwitchNews::Finding(to.clone()));
    let mut playback =
        crate::playback::start::play_config(engine, &services.store, id).playback;
    playback.translation = translation;
    let Services { meta, store, registry, .. } = services;
    let (meta, store, registry) = (meta.clone(), store.clone(), registry.clone());
    tokio::spawn(async move {
        let resolved = resolve_for_playback(
            &meta, &store, &registry, id, &episode, None, &playback, None, None,
        )
        .await;
        let news = match resolved {
            Ok((streams, _)) if !streams.is_empty() => SwitchNews::Found(to, streams),
            Ok(_) => SwitchNews::Failed(
                to,
                crate::playback::resolve::explained(kaeri_model::Cause::NoSources),
            ),
            Err(unplayable) => {
                tracing::info!(reason = %unplayable, "the other version found nothing");
                SwitchNews::Failed(to, unplayable.text)
            }
        };
        let _ = door.send(news);
    });
    true
}

/// The streams of release `source_id` of source `provider_id` for `episode` of `id`, from that
/// source alone, or why there are none. Written down nowhere: a pick in the player is for this
/// episode only.
pub(crate) async fn find(
    meta: &Meta,
    store: &Store,
    registry: &ProviderRegistry,
    id: AnilistId,
    episode: &str,
    (provider_id, source_id): (&str, &str),
    playback: &PlaybackConfig,
) -> Result<Vec<Stream>, Msg> {
    let pick =
        Pick::Release { provider_id: provider_id.to_owned(), source_id: source_id.to_owned() };
    let resolved = resolve_for_playback(
        meta,
        store,
        registry,
        id,
        episode,
        Some(pick),
        playback,
        None,
        None,
    )
    .await;
    match resolved {
        Ok((streams, _)) => Ok(streams),
        Err(unplayable) => {
            tracing::info!(provider = %provider_id, reason = %unplayable, "a switch found nothing");
            Err(unplayable.text)
        }
    }
}
