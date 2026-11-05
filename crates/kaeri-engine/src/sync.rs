//! What the engine does with the sync log: carries out the settings, source order and switched-off
//! sources that arrived from other devices, fills the trackers a watch's origin lacked, and lists
//! the plugin stores another device joined that this one has not.
//!
//! Writes the other way (a setting changed here) are announced by [`crate::config::ConfigState`],
//! the one place every config write passes.

use kaeri_core::{settings::SettingValue, shared_settings};
use kaeri_model::event::Event;
use kaeri_store::sync::{Effect, kinds};

use crate::{
    outbox::{Internal, Outbox},
    services::Services,
    state::EngineState,
};

/// Carry out what received ops asked of the config. A key that is not on the shared list is
/// refused, so nothing a peer sends can write a device-bound setting. Written and read back as
/// any setting is, and not announced again.
pub fn apply_effects(
    effects: Vec<Effect>,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    if effects.is_empty() {
        return;
    }
    let paths = &services.paths;
    let (mut reordered, mut sources_changed, mut wrote) = (false, false, false);
    for Effect { kind, key, value } in effects {
        match kind.as_str() {
            kinds::SETTING => {
                let Some((table, file_key)) = shared_settings::parse(&key) else {
                    tracing::debug!(key, "a received setting that is not shared; ignored");
                    continue;
                };
                let Some(value) = value.as_deref().and_then(shared_settings::decode) else {
                    continue;
                };
                wrote |= engine.config.write_received(paths, table, file_key, value).is_ok();
            }
            kinds::SOURCE_ORDER => {
                let Some(SettingValue::Strings(order)) =
                    value.as_deref().and_then(shared_settings::decode)
                else {
                    continue;
                };
                let written = engine.config.write_received(
                    paths,
                    &["providers"],
                    "order",
                    SettingValue::Strings(order.clone()),
                );
                if written.is_ok() {
                    engine.config.effective_mut().providers.order = order;
                    reordered = true;
                    wrote = true;
                }
            }
            kinds::SOURCE_OFF => {
                let mut off = engine.config.effective().providers.disabled.clone();
                off.retain(|id| *id != key);
                if value.is_some() {
                    off.push(key);
                }
                let written = engine.config.write_received(
                    paths,
                    &["providers"],
                    "disabled",
                    SettingValue::Strings(off.clone()),
                );
                if written.is_ok() {
                    engine.config.effective_mut().providers.disabled = off;
                    sources_changed = true;
                    wrote = true;
                }
            }
            kinds::MEDIUM_ACCOUNT => {
                let answer = value.as_deref().and_then(kaeri_model::medium::KaeriIsFor::parse);
                let writer = services.store.sync_reg_writer(&kind, &key).ok().flatten();
                if engine.reading.account_received(answer, writer) {
                    let _ = tx.send(Event::Medium(engine.reading.view()));
                }
            }
            // Stores are read from the log when asked: see `stores_to_join`.
            _ => {}
        }
    }
    if !wrote {
        return;
    }
    engine.config.adopt_shared();
    let _ = tx.send(Event::Settings(engine.config.view()));
    let _ = tx.internal(Internal::SettingsSaved);
    if sources_changed {
        services
            .registry
            .set_disabled(engine.config.effective().providers.disabled.iter().cloned());
        let _ = tx.internal(Internal::SourcesChanged);
    } else if reordered {
        let _ = tx.internal(Internal::SourcesReordered);
    }
}

/// The plugin stores another device joined that this one has not: `(address, name)`, for the
/// Plugin stores list's "Join on this device". `joined` is the addresses this device is a member of.
pub fn stores_to_join(
    store: &kaeri_store::Store,
    joined: &[String],
) -> Result<Vec<(String, String)>, kaeri_store::StoreError> {
    Ok(store
        .sync_regs(kinds::STORE)?
        .into_iter()
        .filter(|(address, _)| !joined.contains(address))
        .collect())
}

/// The trackers this device is signed in to, as the other devices need them for the lead rule.
pub fn announce_trackers(services: &Services, signed_in: &[String]) {
    if let Err(e) = services.store.sync_set_my_trackers(signed_in) {
        tracing::warn!(error = %e, "could not tell the sync log which trackers this device has");
    }
}

/// Queue the progress this device must send for the trackers a received watch's origin lacked.
/// With sync off, or when this device is not the lead, nothing is queued.
pub fn fill_tracker_gaps(
    watches: &[kaeri_store::sync::ReceivedWatch],
    my_trackers: &[String],
    store: &kaeri_store::Store,
) {
    if watches.is_empty() || my_trackers.is_empty() {
        return;
    }
    let now = kaeri_store::now();
    let gaps = match store.sync_tracker_gaps(my_trackers, watches, now) {
        Ok(gaps) => gaps,
        Err(e) => {
            tracing::warn!(error = %e, "could not work out which trackers to fill");
            return;
        }
    };
    for (tracker, anilist_id) in gaps {
        kaeri_track::sync::queue_progress_for(
            store,
            std::slice::from_ref(&tracker),
            kaeri_core::ids::AnilistId::new(anilist_id),
            now,
        );
    }
}

/// Whether this device has a list of titles with statuses: the Library's tracker is signed in
/// (its list wins), or statuses a person set on a device came through sync and fill in.
pub fn has_list(services: &crate::services::Services) -> bool {
    services.sync.trackers.first().is_some_and(|tracker| tracker.is_authenticated())
        || services.store.has_synced_statuses().unwrap_or(false)
}
