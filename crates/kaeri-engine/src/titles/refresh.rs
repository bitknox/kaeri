//! Titles on screen, listed again when the sources they were listed from change.
//!
//! A front end asks for a title's episodes once and keeps the answer, so a source that was
//! switched on, moved up, pinned or installed would otherwise show only after the app was
//! restarted. The engine knows when the sources change, so it lists the titles the viewer
//! has been looking at again and says the new tables, which a front end takes as it takes any
//! table.

use std::{
    collections::HashMap,
    hash::Hash,
    sync::{Arc, Mutex},
};

use kaeri_core::ids::AnilistId;
use kaeri_model::event::Event;
use tokio::sync::Notify;

use crate::{dispatch, outbox::Outbox, services::Services, state::EngineState};

/// How many titles are listed again: the ones the viewer has just been looking at. A page opened
/// later asks for itself and finds the sources as they now are.
const RECENT: usize = 3;

/// The titles whose episodes were listed lately, newest last, and which listing of each is the
/// current one: of its episodes, of its sources' coverage, and of an episode's releases.
#[derive(Debug, Default)]
pub struct Listed(Vec<AnilistId>, Generations, Generations, Generations<(AnilistId, String)>);

/// Which listing of each title (or of each title's episode) is the newest. A load, a pin and a
/// source change each start one, and a slow earlier one must not land over a newer table.
#[derive(Debug, Clone)]
struct Generations<K = AnilistId>(Arc<Mutex<HashMap<K, u64>>>, Arc<Notify>);

impl<K> Default for Generations<K> {
    fn default() -> Self {
        Self(Arc::default(), Arc::default())
    }
}

impl<K: Eq + Hash + Clone> Generations<K> {
    /// Start a listing of `key`, which puts every earlier one out of date.
    fn start(&self, key: K) -> Ticket<K> {
        let n = {
            let mut map = self.0.lock().unwrap();
            let n = map.entry(key.clone()).or_insert(0);
            *n += 1;
            *n
        };
        // An earlier listing still waiting learns it was superseded.
        self.1.notify_waiters();
        Ticket { key, n, generations: self.clone() }
    }
}

/// A listing's claim to speak for its title: good until a newer listing of it starts.
#[derive(Debug, Clone)]
pub struct Ticket<K = AnilistId> {
    key: K,
    n: u64,
    generations: Generations<K>,
}

impl<K: Eq + Hash> Ticket<K> {
    /// No newer listing of the title has started.
    pub fn current(&self) -> bool {
        self.generations.0.lock().unwrap().get(&self.key) == Some(&self.n)
    }

    /// Send `event` if this listing is still the newest and someone still reads. Returns
    /// whether it was sent.
    pub fn send(&self, tx: &Outbox, event: Event) -> bool {
        self.current() && tx.send(event).is_ok()
    }

    /// Send every one of `events`, or none if this listing is no longer the newest: a pair
    /// that belongs together is never split by a newer listing starting between the two.
    pub fn send_all(&self, tx: &Outbox, events: impl IntoIterator<Item = Event>) -> bool {
        self.current() && events.into_iter().all(|event| tx.send(event).is_ok())
    }

    /// Done once a newer listing has started; never done for the newest. A listing races
    /// its work against this to stop at once, rather than when its next answer comes.
    pub async fn superseded(&self) {
        loop {
            let notified = self.generations.1.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if !self.current() {
                return;
            }
            notified.await;
        }
    }
}

impl Ticket {
    /// A ticket that is never current: a listing that says nothing, such as a quiet recheck of a
    /// title nobody has open.
    pub fn muted(id: AnilistId) -> Self {
        Self { key: id, n: 0, generations: Generations::default() }
    }

    /// A ticket that is always current, for a caller with no newer listing to lose to.
    #[cfg(test)]
    pub fn standalone(id: AnilistId) -> Self {
        let generations = Generations::default();
        generations.0.lock().unwrap().insert(id, 0);
        Self { key: id, n: 0, generations }
    }
}

impl Listed {
    /// Title `id`'s episodes were asked for.
    pub fn note(&mut self, id: AnilistId) {
        self.0.retain(|seen| *seen != id);
        self.0.push(id);
        if self.0.len() > RECENT {
            self.0.remove(0);
        }
    }

    /// Start a listing of title `id`, which puts every earlier one out of date.
    pub fn ticket(&self, id: AnilistId) -> Ticket {
        self.1.start(id)
    }

    /// Start a source-options listing of title `id`: only the newest answers, since the
    /// slowest source sets how long one takes and an older one may come back after a newer.
    pub fn options_ticket(&self, id: AnilistId) -> Ticket {
        self.2.start(id)
    }

    /// Start a listing of `episode`'s releases of title `id`, for the Sources tab: only the
    /// newest of that episode says anything, so a tab closed and opened again is not drawn by
    /// two at once. Another episode's listing is apart: a player's tab and a title page's may
    /// each wait on their own.
    pub fn sources_ticket(&self, id: AnilistId, episode: &str) -> Ticket<(AnilistId, String)> {
        self.3.start((id, episode.to_owned()))
    }

    pub fn has(&self, id: AnilistId) -> bool {
        self.0.contains(&id)
    }

    /// Title `id` was listed at some point since the engine started, not only lately: a front
    /// end may hold its table.
    pub fn ever(&self, id: AnilistId) -> bool {
        self.1.0.lock().unwrap().contains_key(&id)
    }

    pub fn ids(&self) -> Vec<AnilistId> {
        self.0.clone()
    }
}

/// The sources changed: order, which are on, one installed, updated or removed, or the torrent
/// source joined. Whatever was found against the old sources is dropped, the titles shown
/// are listed again, and Home's marks are brought up to date.
pub fn sources_changed(engine: &mut EngineState, services: &Services, tx: &Outbox) {
    // A next episode found ahead was found from the old sources.
    services.prefetch.clear();
    for id in engine.listed.ids() {
        relist(id, false, engine, services, tx);
    }
    dispatch::home_after_sources(engine, services, tx);
}

/// Title `id` was pinned to a source or one of its releases, or given back to automatic. A
/// native app keeps the table of every title it loaded, not only the last few the engine
/// counts as on screen, and opens a page on the table it holds. So a title listed at any time
/// before is listed again now and counted among the recent ones, and its page shows the pick
/// as a fresh opening would. One never listed has no table to bring up to date and finds the
/// pick when it is asked for.
pub fn pinned(id: AnilistId, engine: &mut EngineState, services: &Services, tx: &Outbox) {
    if engine.listed.ever(id) {
        engine.listed.note(id);
    }
    relist(id, true, engine, services, tx);
}

/// Title `id` is listed again, with its dub and episode names asked afresh, when the viewer
/// has been looking at it. One never opened has nothing on screen to bring up to date, and
/// asks for itself when it is.
pub fn relist(
    id: AnilistId,
    announce: bool,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    services.prefetch.clear();
    engine.dubs.forget(id, &services.store);
    engine.names.forget(id);
    if !engine.listed.has(id) {
        return;
    }
    dispatch::list_episodes_again(id, announce, engine, services, tx);
    engine.dubs.ask(id, engine.network.offline, services, tx);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_newest_listing_of_a_title_may_speak() {
        let listed = Listed::default();
        let id = AnilistId::new(1);
        let slow = listed.ticket(id);
        assert!(slow.current());
        let pinned = listed.ticket(id);
        assert!(!slow.current(), "a newer listing started");
        assert!(pinned.current());
        assert!(listed.ticket(AnilistId::new(2)).current());
        assert!(pinned.current(), "another title's listing is no one's to supersede");
        assert!(listed.ever(id) && !listed.ever(AnilistId::new(3)), "listed at any time");
        // Options listings are counted apart from episode listings.
        let options = listed.options_ticket(id);
        assert!(pinned.current() && options.current());
        assert!(!Ticket::muted(id).current(), "a muted listing never speaks");
    }

    #[test]
    fn each_episode_s_releases_are_listed_apart() {
        let listed = Listed::default();
        let id = AnilistId::new(1);
        let seventh = listed.sources_ticket(id, "7");
        let third = listed.sources_ticket(id, "3");
        assert!(seventh.current() && third.current(), "another episode supersedes nothing");
        let again = listed.sources_ticket(id, "7");
        assert!(!seventh.current() && again.current(), "the same episode asked again does");
        assert!(third.current());
    }

    #[tokio::test]
    async fn a_listing_learns_at_once_that_it_was_superseded() {
        let listed = Listed::default();
        let id = AnilistId::new(1);
        let first = listed.sources_ticket(id, "7");
        let waiting = tokio::spawn(async move { first.superseded().await });
        tokio::task::yield_now().await;
        assert!(!waiting.is_finished(), "the newest is never superseded");
        let _newer = listed.sources_ticket(id, "7");
        tokio::time::timeout(std::time::Duration::from_secs(5), waiting)
            .await
            .expect("woken by the newer listing")
            .unwrap();
    }

    #[test]
    fn a_listing_nobody_reads_stops_saying() {
        let (events, rx) = tokio::sync::mpsc::unbounded_channel();
        let tx = Outbox::from(events);
        let ticket = Listed::default().sources_ticket(AnilistId::new(1), "7");
        assert!(ticket.send(
            &tx,
            Event::Toast(kaeri_model::notice::Notice::alert(
                kaeri_model::Msg::NoticeTitleLoadFailed
            ))
        ));
        drop(rx);
        assert!(!ticket.send(
            &tx,
            Event::Toast(kaeri_model::notice::Notice::alert(
                kaeri_model::Msg::NoticeTitleLoadFailed
            ))
        ));
    }
}
