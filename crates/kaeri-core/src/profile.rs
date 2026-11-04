//! A profile's own settings: `profile.toml` beside its database, holding the keys of
//! [`crate::shared_settings::SHARED`] and nothing else.
//!
//! The first profile keeps its keys in `config.toml` as today, so a one-person install has no
//! overlay. A second profile's keys are read over the config's: a key present in the overlay wins,
//! and a key absent from it is the device's.

use std::path::Path;

use crate::{
    config::{Config, Paths},
    settings::{self, SettingValue},
    shared_settings,
};

/// The config as `paths` holds it, with `overlay`'s shared keys read over it.
pub fn load_with_overlay(paths: &Paths, overlay: &Path) -> Result<Config, crate::Error> {
    let base = match std::fs::read_to_string(&paths.config_file) {
        Ok(src) => src,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => {
            return Err(crate::Error::Config(format!(
                "cannot read {}: {e}",
                paths.config_file.display()
            )));
        }
    };
    let over = std::fs::read_to_string(overlay).unwrap_or_default();
    if over.trim().is_empty() {
        return Config::load(paths);
    }
    Config::from_toml(&merged(&base, &over)?)
}

/// `base` with the shared keys `over` sets written over it.
fn merged(base: &str, over: &str) -> Result<String, crate::Error> {
    let mut doc = base
        .parse::<toml_edit::DocumentMut>()
        .map_err(|e| crate::Error::Config(format!("config.toml is not valid TOML: {e}")))?;
    let over = over
        .parse::<toml_edit::DocumentMut>()
        .map_err(|e| crate::Error::Config(format!("profile.toml is not valid TOML: {e}")))?;
    for (table, key) in shared_settings::SHARED {
        let mut from = Some(over.as_table());
        for segment in *table {
            from = from.and_then(|t| t.get(segment)).and_then(|i| i.as_table());
        }
        let Some(item) = from.and_then(|t| t.get(key)) else { continue };
        let mut to = doc.as_table_mut();
        for segment in *table {
            to = to
                .entry(segment)
                .or_insert_with(|| toml_edit::Item::Table(toml_edit::Table::new()))
                .as_table_mut()
                .ok_or_else(|| crate::Error::Config(format!("{segment} is not a table")))?;
        }
        to.insert(key, item.clone());
    }
    Ok(doc.to_string())
}

/// Write a shared setting into a profile's overlay, never into the device's config. `false` when
/// the key is not shared, so the caller writes it to the config as it always did.
pub fn write_shared(
    overlay: &Path,
    table: &[&str],
    key: &str,
    value: SettingValue,
) -> Result<bool, crate::Error> {
    if !shared_settings::is_shared(table, key) {
        return Ok(false);
    }
    if let Some(dir) = overlay.parent() {
        std::fs::create_dir_all(dir).map_err(|e| {
            crate::Error::Config(format!("cannot create {}: {e}", dir.display()))
        })?;
    }
    let as_paths = Paths {
        config_file: overlay.to_path_buf(),
        data_dir: overlay.parent().unwrap_or(overlay).to_path_buf(),
        cache_dir: overlay.parent().unwrap_or(overlay).to_path_buf(),
    };
    settings::write_key(&as_paths, table, key, value)?;
    Ok(true)
}

/// Move a profile's overlay into `config.toml` and remove it: the profile takes the first one's
/// place. Its keys replace the config's, and a shared key it never set is left as the config has it.
pub fn promote(paths: &Paths, overlay: &Path) -> Result<(), crate::Error> {
    let over = match std::fs::read_to_string(overlay) {
        Ok(src) => src,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => {
            return Err(crate::Error::Config(format!(
                "cannot read {}: {e}",
                overlay.display()
            )));
        }
    };
    let base = std::fs::read_to_string(&paths.config_file).unwrap_or_default();
    let merged = merged(&base, &over)?;
    Config::from_toml(&merged)?;
    if let Some(dir) = paths.config_file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::write(&paths.config_file, merged)
        .map_err(|e| crate::Error::Config(format!("cannot write config.toml: {e}")))?;
    let _ = std::fs::remove_file(overlay);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("kaeri-profile-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn paths(dir: &Path) -> Paths {
        Paths {
            config_file: dir.join("config.toml"),
            data_dir: dir.join("data"),
            cache_dir: dir.join("cache"),
        }
    }

    #[test]
    fn a_profiles_shared_keys_read_over_the_device_config_and_the_rest_stays() {
        let dir = scratch("overlay");
        let paths = paths(&dir);
        std::fs::write(&paths.config_file, "[playback]\nskip_opening = false\nquality = 720\n")
            .unwrap();
        let overlay = dir.join("profiles/p2/profile.toml");
        assert!(
            write_shared(&overlay, &["playback"], "skip_opening", SettingValue::Bool(true))
                .unwrap()
        );
        // Not shared: refused, so the caller writes the config.
        assert!(
            !write_shared(&overlay, &["playback"], "quality", SettingValue::Int(1080)).unwrap()
        );

        let config = load_with_overlay(&paths, &overlay).unwrap();
        assert!(config.playback.skip_opening, "the profile's own");
        assert_eq!(config.playback.quality, 720, "the device's");
        let plain = Config::load(&paths).unwrap();
        assert!(!plain.playback.skip_opening, "the config file itself is untouched");
    }

    #[test]
    fn promotion_moves_the_overlay_into_the_config() {
        let dir = scratch("promote");
        let paths = paths(&dir);
        std::fs::write(&paths.config_file, "# mine\n[playback]\nquality = 720\n").unwrap();
        let overlay = dir.join("profiles/p2/profile.toml");
        write_shared(&overlay, &["playback"], "auto_next", SettingValue::Bool(false)).unwrap();
        promote(&paths, &overlay).unwrap();
        assert!(!overlay.exists());
        let text = std::fs::read_to_string(&paths.config_file).unwrap();
        assert!(text.contains("# mine") && text.contains("auto_next = false"), "{text}");
    }
}
