//! Whether the sources have a title, asked when someone shows interest in one on Home.
//!
//! Every usable source is searched for the title at once, within [`PROBE_WAIT`]. A confident
//! match on any of them is a find, kept as that source's mapping so the play after it asks
//! nothing. A miss is only a miss when every source answered, and none had it; a source that
//! failed or ran out of time leaves the answer unknown, which Home never marks. A miss is kept
//! for a day, so the same title is not asked again each time it is looked at.
//!
//! A miss is kept with a mark of the sources it was asked of, and holds only while those are
//! the sources usable: one added, switched on, updated or gone back to asking anew. A miss kept
//! before misses carried that mark is stale too. A mapping kept since for any usable source,
//! as a title page or a play keeps one, wins over a miss, and a listing or a play from a source
//! turns a kept miss into a find. A find is kept as it is: it costs nothing if its source has
//! gone, since Home offers Play for a find and an unknown alike, and the play asks the sources
//! as they stand.

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    time::Duration,
};

use kaeri_core::{error::ProviderError, ids::AnilistId, media::Translation};
use kaeri_meta::{Meta, title::rank};
use kaeri_model::{event::Event, home::Playable};
use kaeri_providers::ProviderRegistry;
use kaeri_store::{ResolutionRung, Store};
use serde::{Deserialize, Serialize};

use crate::outbox::Outbox;

/// How long the check as a whole may take.
pub const PROBE_WAIT: Duration = Duration::from_secs(4);
const DAY: i64 = 86_400;

/// What the sources check found, as kept under [`probe_key`].
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Probed {
    pub at: i64,
    pub found: bool,
    /// The [`Usable::mark`] of the sources asked. A miss without it was kept before misses
    /// said which sources they were from, and holds no longer.
    #[serde(default)]
    pub sources: Option<u64>,
}

pub(crate) fn probe_key(id: AnilistId) -> String {
    format!("probe:{}", id.get())
}

/// The sources the check asks, as they stand: their ids in preference order, and a mark of
/// which they are and at which version, in no order. Reordering leaves the mark as it was,
/// since a miss is every source saying no, whichever asked first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Usable {
    pub ids: Vec<String>,
    /// A hash of the sorted ids and versions. A toolchain that hashed differently would only
    /// make the kept misses stale, and those are asked again.
    pub mark: u64,
}

impl Usable {
    pub fn of(registry: &ProviderRegistry) -> Self {
        let ids = registry.usable_ids();
        let mut named: Vec<(&str, String)> = ids
            .iter()
            .map(|id| {
                let version = registry.get(id).map(|p| p.manifest().version.clone());
                (id.as_str(), version.unwrap_or_default())
            })
            .collect();
        named.sort();
        let mut hasher = DefaultHasher::new();
        named.hash(&mut hasher);
        let mark = hasher.finish();
        Self { ids, mark }
    }
}

fn record(store: &Store, id: AnilistId) -> Option<Probed> {
    let json = store.get_meta_text(&probe_key(id)).ok().flatten()?;
    serde_json::from_str(&json).ok()
}

fn keep(store: &Store, id: AnilistId, probed: &Probed) {
    if let Ok(json) = serde_json::to_string(probed)
        && let Err(e) = store.set_meta_text(&probe_key(id), &json)
    {
        tracing::warn!(error = %e, "could not keep what the sources said");
    }
}

/// How far after `now` a kept answer may be dated and still hold, as if just given. Home is
/// composed with the moment it was asked for, so a check that ended while it was composed is
/// dated a little after that moment, and it is the newest answer there is. One dated further
/// ahead was kept before the clock was set back, and is asked again.
const AHEAD: i64 = 5 * 60;

fn within_day(probed: &Probed, now: i64) -> bool {
    if probed.at > now {
        return probed.at - now <= AHEAD;
    }
    now - probed.at < DAY
}

/// Whether a mapping is kept for `id` with any of the sources `ids`.
fn mapped(store: &Store, id: AnilistId, ids: &[String]) -> bool {
    ids.iter().any(|p| store.lookup_mapping(id, p).ok().flatten().is_some())
}

/// What is kept of `id` that still holds at `now` with the sources `usable`: a find from the
/// last day, or a miss from the last day asked of these same sources, none of which has a
/// mapping for it since. `None` when nothing holds, which Home shows as unknown.
pub(crate) fn kept(
    store: &Store,
    id: AnilistId,
    usable: &Usable,
    now: i64,
) -> Option<Playable> {
    let probed = record(store, id).filter(|p| within_day(p, now))?;
    if probed.found {
        return Some(Playable::Found);
    }
    if probed.sources != Some(usable.mark) {
        return None;
    }
    Some(if mapped(store, id, &usable.ids) { Playable::Found } else { Playable::NotFound })
}

/// Title `id` was listed or played from a source: a miss kept for it is a find now, and said,
/// so a Home on screen stops saying it is not found. Nothing is said without a miss kept.
pub(crate) fn listed(store: &Store, tx: &Outbox, id: AnilistId) {
    let Some(miss) = record(store, id).filter(|p| !p.found) else { return };
    // For as long as the miss would have held: the source's mapping is kept by then.
    keep(store, id, &Probed { found: true, ..miss });
    let _ = tx.send(Event::TitlePlayable { id, playable: Playable::Found });
}

/// What `id` is now, when the apps were told it was not found and that no longer holds with
/// the sources `usable`: unknown, or a find where a mapping has been kept since.
pub(crate) fn lapsed(
    store: &Store,
    id: AnilistId,
    usable: &Usable,
    now: i64,
) -> Option<Playable> {
    record(store, id).filter(|p| !p.found && within_day(p, now))?;
    match kept(store, id, usable, now) {
        Some(Playable::NotFound) => None,
        other => Some(other.unwrap_or(Playable::Unknown)),
    }
}

/// What the sources say of one title, as far as one source can tell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Said {
    Has,
    HasNot,
    CouldNotTell,
}

/// Ask the sources whether they have `id`, and send what they said. A find or a miss kept from
/// less than a day ago is sent again without asking.
pub async fn probe(
    meta: &Meta,
    store: &Store,
    registry: &ProviderRegistry,
    tx: &Outbox,
    id: AnilistId,
    translation: Translation,
    now: i64,
) -> Playable {
    let playable = check(meta, store, registry, id, translation, now).await;
    let _ = tx.send(Event::TitlePlayable { id, playable });
    playable
}

async fn check(
    meta: &Meta,
    store: &Store,
    registry: &ProviderRegistry,
    id: AnilistId,
    translation: Translation,
    now: i64,
) -> Playable {
    let usable = Usable::of(registry);
    if let Some(playable) = kept(store, id, &usable, now) {
        return playable;
    }
    let ids = &usable.ids;
    if ids.is_empty() {
        return Playable::Unknown;
    }
    // A mapping already kept for any source is a find without asking.
    if mapped(store, id, ids) {
        return Playable::Found;
    }
    let Some(media) = meta.cached(&[id]).into_iter().next() else {
        return Playable::Unknown;
    };
    let target = media.match_target();
    let Some(query) = target.titles.first().cloned() else {
        return Playable::Unknown;
    };
    // Each source has until the same moment: one that hangs leaves the others' answers standing.
    let deadline = tokio::time::Instant::now() + PROBE_WAIT;
    let asked = ids.iter().map(|provider_id| {
        let (target, query) = (&target, &query);
        let one = async move {
            let Some(source) = registry.pinned(provider_id) else {
                return Said::CouldNotTell;
            };
            let attempt = source.search(query, translation, &target.hints, now).await;
            match attempt.value {
                Some(hits) if hits.is_empty() => Said::HasNot,
                Some(hits) => {
                    let ranked = rank(target, &hits);
                    match ranked.first() {
                        Some(best) if best.is_confident() => {
                            let _ = store.cache_resolution(
                                id,
                                provider_id,
                                &best.hit.key,
                                best.score,
                                ResolutionRung::ProviderSearch,
                                now,
                            );
                            Said::Has
                        }
                        // Something like it, but not sure: not a miss to mark.
                        Some(_) => Said::CouldNotTell,
                        None => Said::HasNot,
                    }
                }
                None if !attempt.failures.is_empty()
                    && attempt.failures.iter().all(|(_, e)| matches!(e, ProviderError::NotFound)) =>
                {
                    Said::HasNot
                }
                None => Said::CouldNotTell,
            }
        };
        async move { tokio::time::timeout_at(deadline, one).await.unwrap_or(Said::CouldNotTell) }
    });
    let said = futures::future::join_all(asked).await;
    // Sources still compiling have not answered, so no miss is said before they are in.
    let playable = if said.contains(&Said::Has) {
        Playable::Found
    } else if said.iter().all(|s| *s == Said::HasNot) && !registry.is_arriving() {
        Playable::NotFound
    } else {
        Playable::Unknown
    };
    if playable != Playable::Unknown {
        let found = playable == Playable::Found;
        keep(store, id, &Probed { at: now, found, sources: Some(usable.mark) });
    }
    playable
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use kaeri_core::{config::NetworkConfig, ids::ProviderKey, media::SearchHit};
    use kaeri_meta::AniList;
    use kaeri_providers::MockProvider;
    use tokio::sync::mpsc;

    use super::*;

    const ID: u32 = 900_301;

    fn title() -> AnilistId {
        AnilistId::new(ID)
    }

    fn now() -> i64 {
        crate::now_epoch()
    }

    /// The metadata and a store, offline, with one made-up title cached.
    fn cached() -> (Meta, Store) {
        let store = Store::open_in_memory().unwrap();
        let media = serde_json::json!({
            "id": ID,
            "title": { "romaji": "Kumo no Ato", "english": "After the Clouds" },
            "format": "TV",
            "status": "FINISHED",
            "episodes": 12,
        });
        store.cache_media(title(), &media.to_string(), now()).unwrap();
        let http = kaeri_net::HttpClient::offline(&NetworkConfig::default()).unwrap();
        (Meta::new(AniList::new(http, 30), None, store.clone()), store)
    }

    /// A source that has the title.
    fn has_it(id: &str) -> MockProvider {
        MockProvider::new(id)
            .with_hits(vec![SearchHit::new("after-the-clouds", "After the Clouds")])
    }

    async fn ask(
        meta: &Meta,
        store: &Store,
        registry: &ProviderRegistry,
        now: i64,
    ) -> Playable {
        check(meta, store, registry, title(), Translation::Sub, now).await
    }

    #[tokio::test]
    async fn a_miss_holds_only_for_the_sources_it_was_asked_of() {
        let (meta, store) = cached();
        let alpha = MockProvider::new("alpha");
        let asked = alpha.call_count();
        let registry = ProviderRegistry::new(vec![alpha.arc()]);
        let at = now();
        assert_eq!(ask(&meta, &store, &registry, at).await, Playable::NotFound);
        // Within the day, the same sources are not asked again.
        assert_eq!(ask(&meta, &store, &registry, at + 60).await, Playable::NotFound);
        assert_eq!(asked.load(Ordering::SeqCst), 1);
        let before = Usable::of(&registry);
        assert_eq!(kept(&store, title(), &before, at + 60), Some(Playable::NotFound));

        // A source joins that has it: the miss was not asked of it, so it holds no longer.
        registry.put(has_it("beta").arc());
        let after = Usable::of(&registry);
        assert_ne!(before.mark, after.mark);
        assert_eq!(kept(&store, title(), &after, at + 120), None, "Home shows it unknown");
        assert_eq!(lapsed(&store, title(), &after, at + 120), Some(Playable::Unknown));
        assert_eq!(ask(&meta, &store, &registry, at + 120).await, Playable::Found);
        assert_eq!(kept(&store, title(), &after, at + 120), Some(Playable::Found));
    }

    #[tokio::test]
    async fn the_order_of_the_sources_is_not_which_sources_they_are() {
        let registry = ProviderRegistry::new(vec![
            MockProvider::new("alpha").arc(),
            MockProvider::new("beta").arc(),
        ]);
        let before = Usable::of(&registry);
        registry.rank_by(|p| std::cmp::Reverse(p.manifest().id.clone()));
        let after = Usable::of(&registry);
        assert_eq!(after.ids, ["beta", "alpha"]);
        assert_eq!(before.mark, after.mark);
    }

    #[tokio::test]
    async fn a_mapping_kept_since_wins_over_a_kept_miss() {
        let (meta, store) = cached();
        let registry = ProviderRegistry::new(vec![MockProvider::new("alpha").arc()]);
        let at = now();
        assert_eq!(ask(&meta, &store, &registry, at).await, Playable::NotFound);
        // The title page matched it by hand, or a play found it, and kept the mapping.
        let key = ProviderKey::new("after-the-clouds");
        store
            .cache_resolution(title(), "alpha", &key, 0.95, ResolutionRung::ProviderSearch, at)
            .unwrap();
        let usable = Usable::of(&registry);
        assert_eq!(kept(&store, title(), &usable, at + 60), Some(Playable::Found));
        assert_eq!(lapsed(&store, title(), &usable, at + 60), Some(Playable::Found));
        assert_eq!(ask(&meta, &store, &registry, at + 60).await, Playable::Found);
    }

    #[tokio::test]
    async fn a_listing_from_a_source_turns_a_kept_miss_into_a_find() {
        let (meta, store) = cached();
        let alpha = MockProvider::new("alpha");
        let asked = alpha.call_count();
        let registry = ProviderRegistry::new(vec![alpha.arc()]);
        let (tx, mut rx) = mpsc::unbounded_channel();
        let tx = Outbox::from(tx);
        let at = now();
        // Nothing kept: a listing says nothing.
        listed(&store, &tx, title());
        assert!(rx.try_recv().is_err());

        assert_eq!(ask(&meta, &store, &registry, at).await, Playable::NotFound);
        listed(&store, &tx, title());
        assert!(matches!(
            rx.try_recv(),
            Ok(Event::TitlePlayable { id, playable: Playable::Found }) if id == title()
        ));
        let usable = Usable::of(&registry);
        assert_eq!(kept(&store, title(), &usable, at + 60), Some(Playable::Found));
        assert_eq!(ask(&meta, &store, &registry, at + 60).await, Playable::Found);
        assert_eq!(asked.load(Ordering::SeqCst), 1, "the find is kept, and nothing asked");
        // A find kept already is not said again.
        listed(&store, &tx, title());
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn a_miss_kept_before_misses_named_their_sources_is_stale() {
        let (meta, store) = cached();
        let registry = ProviderRegistry::new(vec![has_it("alpha").arc()]);
        let at = now();
        let old = serde_json::json!({ "at": at - 600, "found": false }).to_string();
        store.set_meta_text(&probe_key(title()), &old).unwrap();
        let usable = Usable::of(&registry);
        assert_eq!(kept(&store, title(), &usable, at), None);
        assert_eq!(lapsed(&store, title(), &usable, at), Some(Playable::Unknown));
        assert_eq!(ask(&meta, &store, &registry, at).await, Playable::Found);
    }

    #[tokio::test]
    async fn a_miss_dated_after_the_home_asked_still_holds() {
        let (meta, store) = cached();
        let registry = ProviderRegistry::new(vec![MockProvider::new("alpha").arc()]);
        let usable = Usable::of(&registry);
        let asked = now();
        // The check ends a second after Home was asked for, while Home is still composed.
        assert_eq!(ask(&meta, &store, &registry, asked + 1).await, Playable::NotFound);
        assert_eq!(kept(&store, title(), &usable, asked), Some(Playable::NotFound));
        assert_eq!(lapsed(&store, title(), &usable, asked), None, "the miss still holds");

        // A find likewise, a minute on.
        let find = Probed { at: asked + 60, found: true, sources: Some(usable.mark) };
        keep(&store, title(), &find);
        assert_eq!(kept(&store, title(), &usable, asked), Some(Playable::Found));

        // An hour ahead, it was kept before the clock was set back.
        let miss = Probed { at: asked + 3_600, found: false, sources: Some(usable.mark) };
        keep(&store, title(), &miss);
        assert_eq!(kept(&store, title(), &usable, asked), None, "Home shows it unknown");
        assert_eq!(lapsed(&store, title(), &usable, asked), None);
        assert_eq!(kept(&store, title(), &usable, asked + 3_600), Some(Playable::NotFound));
    }

    #[tokio::test]
    async fn no_miss_is_kept_while_sources_are_on_their_way() {
        let (meta, store) = cached();
        let registry = ProviderRegistry::new(vec![MockProvider::new("alpha").arc()]);
        registry.expect_arrivals();
        let at = now();
        assert_eq!(ask(&meta, &store, &registry, at).await, Playable::Unknown);
        assert!(store.get_meta_text(&probe_key(title())).unwrap().is_none());
        registry.arrivals_done();
        assert_eq!(ask(&meta, &store, &registry, at).await, Playable::NotFound);
    }
}
