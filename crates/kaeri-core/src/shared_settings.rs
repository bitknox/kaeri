//! The settings that travel with a person, not with a device.
//!
//! One list answers two questions: which settings sync between a person's devices, and which are
//! a profile's own when a device has several profiles. A key is added here in code, with the test
//! below, so that nothing bound to a device (VPN, torrents, mpv, paths, quality on a cellular
//! connection, key bindings, the controller, casting) reaches it by accident.

/// `(table path, key)` as `settings::write_key` takes them.
pub const SHARED: &[(&[&str], &str)] = &[
    (&["playback"], "translation"),
    (&["playback"], "subtitle_language"),
    (&["playback"], "commit_threshold"),
    (&["playback"], "auto_next"),
    (&["playback"], "skip_opening"),
    (&["playback"], "skip_filler"),
    (&["playback"], "skip_button"),
    (&["playback"], "up_next_countdown"),
    (&["playback"], "spoiler_protection"),
    (&["playback"], "mark_chapters"),
    (&["meta"], "title_language"),
    (&["meta"], "adult"),
    (&["meta"], "discussions"),
    (&["meta"], "calendar_mine"),
    (&["notifications"], "episodes"),
    (&["syncplay"], "name"),
    (&["syncplay"], "recent_servers"),
];

/// Whether `table.key` is a shared setting.
pub fn is_shared(table: &[&str], key: &str) -> bool {
    SHARED.iter().any(|(t, k)| *t == table && *k == key)
}

/// The register key a shared setting syncs under: `playback.translation`.
pub fn id(table: &[&str], key: &str) -> String {
    let mut out = table.join(".");
    out.push('.');
    out.push_str(key);
    out
}

/// The table path and key a register key names, when it is a shared setting. The reverse of [`id`],
/// so that a setting received from another device can only ever write a key on this list.
pub fn parse(id: &str) -> Option<(&'static [&'static str], &'static str)> {
    SHARED.iter().copied().find(|(t, k)| self::id(t, k) == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_bound_to_a_device_is_shared() {
        let bound = [
            "vpn", "torrent", "mpv", "directory", "dir", "path", "binary", "key", "bind",
            "controller", "cast", "quality", "cellular", "token", "port", "upload", "seed",
            "fullscreen", "upscaling", "player_ui", "night_mode", "presence", "hook", "on_complete",
        ];
        for (table, key) in SHARED {
            let name = id(table, key);
            for word in bound {
                assert!(!name.contains(word), "{name} looks bound to a device ({word})");
            }
        }
    }

    #[test]
    fn ids_round_trip_and_unknown_keys_are_refused() {
        for (table, key) in SHARED {
            assert_eq!(parse(&id(table, key)), Some((*table, *key)));
        }
        assert_eq!(parse("providers.torrent.enabled"), None);
        assert_eq!(parse("downloads.directory"), None);
        assert!(is_shared(&["playback"], "auto_next"));
        assert!(!is_shared(&["playback"], "mpv_args"));
    }

    #[test]
    fn keys_exist_in_the_config() {
        // A typo in the list would sync a key nothing reads.
        let defaults = toml::to_string(&crate::config::Config::default()).unwrap();
        let doc: toml::Table = defaults.parse().unwrap();
        for (table, key) in SHARED {
            let mut at = &doc;
            for part in *table {
                at = at.get(*part).and_then(|v| v.as_table()).unwrap_or_else(|| panic!("{part}"));
            }
            // Optional keys serialise only when set; the table has to exist.
            let _ = (at, key);
        }
    }
}
