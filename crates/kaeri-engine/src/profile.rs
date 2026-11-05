//! Profiles in the running engine: which one a launch enters, the commands that change the
//! list, and the picker's view of it.
//!
//! Optional throughout: with no `profiles.json` every function here is a no-op and the install
//! is a single user's.

use std::path::PathBuf;

use kaeri_core::config::{Config, ConfigProblem, Paths};
use kaeri_model::profiles::{ProfileView, ProfilesView};
use kaeri_store::{
    Store,
    profile::{self, ProfileList},
};
use kaeri_track::secret::TokenStore;

/// The trackers and cloud drives whose tokens belong to a profile.
pub const TRACKER_TOKENS: [&str; 8] = [
    "anilist",
    "mal",
    "kitsu",
    "simkl",
    "trakt",
    crate::cloud::TOKEN_IDS[0],
    crate::cloud::TOKEN_IDS[1],
    crate::cloud::TOKEN_IDS[2],
];

/// Enter a profile at launch: `requested` when it exists, else the one used last. Does nothing
/// for a single user. Answers the profile entered.
pub fn enter(
    store: &Store,
    paths: &Paths,
    requested: Option<&str>,
) -> Result<Option<String>, String> {
    let Some(list) = ProfileList::load(&paths.data_dir).map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    let id = requested
        .filter(|id| list.get(id).is_some())
        .map(str::to_owned)
        .or_else(|| list.profiles.iter().max_by_key(|p| p.last_used).map(|p| p.id.clone()))
        .unwrap_or_else(|| list.primary.clone());
    store.use_profile(&paths.data_dir, &id, kaeri_store::now()).map_err(|e| e.to_string())?;
    Ok(Some(id))
}

/// The overlay file of the profile in use, when it is not the first.
pub fn overlay_of(store: &Store, paths: &Paths) -> Option<PathBuf> {
    let active = store.active_profile()?;
    let list = ProfileList::load(&paths.data_dir).ok().flatten()?;
    (active != list.primary)
        .then(|| ProfileList::dir(&paths.data_dir, &active).join("profile.toml"))
}

/// The config with the profile in use's shared settings read over it.
pub fn read_config(store: &Store, paths: &Paths) -> Result<Config, ConfigProblem> {
    match overlay_of(store, paths) {
        Some(overlay) => kaeri_core::profile::load_with_overlay(paths, &overlay)
            .map_err(|e| ConfigProblem { line: None, message: e.to_string() }),
        None => Config::read(paths),
    }
}

/// The tokens the profile in use signs in through: the bare names for the first profile and a
/// single user, `p-<id>/` ones for the rest.
pub fn scope_tokens(store: &Store, paths: &Paths, tokens: TokenStore) -> TokenStore {
    match (store.active_profile(), ProfileList::load(&paths.data_dir).ok().flatten()) {
        (Some(active), Some(list)) if active != list.primary => tokens.scoped(&active),
        _ => tokens,
    }
}

/// The profiles as the picker shows them. Empty for a single user.
pub fn view(store: &Store, paths: &Paths) -> ProfilesView {
    let Ok(Some(list)) = ProfileList::load(&paths.data_dir) else {
        return ProfilesView::default();
    };
    let active = store.active_profile();
    ProfilesView {
        profiles: list
            .profiles
            .iter()
            .map(|p| ProfileView {
                id: p.id.clone(),
                name: p.name.clone(),
                initials: p.initials.clone(),
                kids: p.kids,
                active: active.as_deref() == Some(p.id.as_str()),
            })
            .collect(),
    }
}

/// Add a profile. The first one added makes today's install the first profile, named
/// `first_name`.
pub fn add(
    store: &Store,
    paths: &Paths,
    name: &str,
    kids: bool,
    first_name: &str,
) -> Result<(), String> {
    let first = !store.has_profiles() && store.active_profile().is_none();
    profile::add(&paths.data_dir, name, kids, first_name, kaeri_store::now())
        .map_err(|e| e.to_string())?;
    if first {
        // The store takes its second connection and its guards now, as the first profile.
        store
            .use_profile(&paths.data_dir, "p1", kaeri_store::now())
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Rename a profile or change whether it is a kids profile.
pub fn edit(paths: &Paths, id: &str, name: &str, kids: bool) -> Result<(), String> {
    profile::edit(&paths.data_dir, id, name, kids).map_err(|e| e.to_string())
}

/// Delete a profile with its data. Its tracker tokens go too, and when the first profile is
/// deleted the one in use takes its place: its tokens and shared settings move over.
pub fn delete(
    store: &Store,
    paths: &Paths,
    tokens: &TokenStore,
    id: &str,
) -> Result<(), String> {
    let active = store.active_profile();
    let overlay =
        active.as_ref().map(|a| ProfileList::dir(&paths.data_dir, a).join("profile.toml"));
    let deleted = store.delete_profile(&paths.data_dir, id).map_err(|e| e.to_string())?;
    match (&deleted.promoted, &active) {
        (Some(_), Some(active)) => {
            tokens.promote(active, &TRACKER_TOKENS).map_err(|e| e.to_string())?;
            if let Some(overlay) = overlay {
                kaeri_core::profile::promote(paths, &overlay).map_err(|e| e.to_string())?;
            }
        }
        _ => {
            tokens.clear_profile(id, &TRACKER_TOKENS).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Record that a profile is the next one to enter. The front end stops the engine, which saves
/// what plays, and starts it again on this profile.
pub fn choose(paths: &Paths, id: &str) -> Result<(), String> {
    let list = ProfileList::load(&paths.data_dir).map_err(|e| e.to_string())?;
    if list.is_none_or(|l| l.get(id).is_none()) {
        return Err(format!("no profile {id}"));
    }
    profile::touch(&paths.data_dir, id, kaeri_store::now()).map_err(|e| e.to_string())
}
