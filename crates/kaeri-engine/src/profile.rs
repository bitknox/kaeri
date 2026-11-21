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
    // When the first profile goes, the store moves the one in use into its place and removes
    // that one's directory, `profile.toml` with it: its shared settings are read before that.
    let settings = match &active {
        Some(a) => kaeri_core::profile::read_overlay(
            &ProfileList::dir(&paths.data_dir, a).join("profile.toml"),
        )
        .map_err(|e| e.to_string())?,
        None => None,
    };
    let deleted = store.delete_profile(&paths.data_dir, id).map_err(|e| e.to_string())?;
    match (&deleted.promoted, &active) {
        (Some(_), Some(active)) => {
            if let Some(settings) = settings {
                kaeri_core::profile::promote_text(paths, &settings)
                    .map_err(|e| e.to_string())?;
            }
            tokens.promote(active, &TRACKER_TOKENS).map_err(|e| e.to_string())?;
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

#[cfg(test)]
mod tests {
    use kaeri_core::settings::SettingValue;
    use kaeri_track::secret::Backend;

    use super::*;

    #[test]
    fn deleting_the_first_profile_keeps_the_settings_of_the_one_taking_its_place() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths {
            config_file: dir.path().join("config.toml"),
            data_dir: dir.path().join("data"),
            cache_dir: dir.path().join("cache"),
        };
        std::fs::create_dir_all(&paths.data_dir).unwrap();
        std::fs::write(&paths.config_file, "[syncplay]\nname = \"Mika\"\n").unwrap();
        let store = Store::open(paths.data_dir.join("kaeri.db")).unwrap();
        let tokens = TokenStore::with_backend(dir.path().join("tokens"), Backend::File);

        add(&store, &paths, "Aki", false, "Mika").unwrap();
        assert_eq!(enter(&store, &paths, Some("p2")).unwrap().as_deref(), Some("p2"));
        let overlay = overlay_of(&store, &paths).expect("Aki's settings are her own");
        let set = |table: &[&str], key: &str, value: SettingValue| {
            assert!(kaeri_core::profile::write_shared(&overlay, table, key, value).unwrap());
        };
        set(&["syncplay"], "name", SettingValue::Str("Aki".into()));
        set(&["meta"], "adult", SettingValue::Bool(true));
        assert_eq!(read_config(&store, &paths).unwrap().syncplay.name, "Aki");

        // Aki takes Mika's place, and with it the config's keys.
        delete(&store, &paths, &tokens, "p1").unwrap();
        assert_eq!(overlay_of(&store, &paths), None);
        let config = read_config(&store, &paths).unwrap();
        assert_eq!(config.syncplay.name, "Aki", "not the deleted profile's name");
        assert!(config.meta.adult);
        assert!(!overlay.exists());
    }
}
