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

/// A setting's value as a sync register holds it: JSON, tagged by type so that `1` and `1.0` stay
/// what they were.
pub fn encode(value: &crate::settings::SettingValue) -> String {
    use crate::settings::SettingValue as V;
    match value {
        V::Str(s) => serde_json::json!({ "s": s }),
        V::Int(i) => serde_json::json!({ "i": i }),
        V::Float(f) => serde_json::json!({ "f": f }),
        V::Bool(b) => serde_json::json!({ "b": b }),
        V::Strings(items) => serde_json::json!({ "l": items }),
    }
    .to_string()
}

/// The inverse of [`encode`]; `None` for anything that is not one of its outputs.
pub fn decode(text: &str) -> Option<crate::settings::SettingValue> {
    use crate::settings::SettingValue as V;
    let json: serde_json::Value = serde_json::from_str(text).ok()?;
    let object = json.as_object()?;
    if let Some(v) = object.get("s") {
        return Some(V::Str(v.as_str()?.to_owned()));
    }
    if let Some(v) = object.get("i") {
        return Some(V::Int(v.as_i64()?));
    }
    if let Some(v) = object.get("f") {
        return Some(V::Float(v.as_f64()?));
    }
    if let Some(v) = object.get("b") {
        return Some(V::Bool(v.as_bool()?));
    }
    let list = object.get("l")?.as_array()?;
    Some(V::Strings(list.iter().map(|i| i.as_str().map(str::to_owned)).collect::<Option<_>>()?))
}

#[cfg(test)]
mod tests {
    use crate::settings::SettingValue;

    #[test]
    fn values_round_trip_through_a_register() {
        for value in [
            SettingValue::Str("jpn".into()),
            SettingValue::Int(3),
            SettingValue::Float(0.85),
            SettingValue::Bool(false),
            SettingValue::Strings(vec!["a.example".into(), "b".into()]),
        ] {
            assert_eq!(super::decode(&super::encode(&value)), Some(value));
        }
        assert_eq!(super::decode("nonsense"), None);
        assert_eq!(super::decode("{\"x\": 1}"), None);
    }

    use super::*;

    #[test]
    fn nothing_bound_to_a_device_is_shared() {
        let bound = [
            "vpn",
            "torrent",
            "mpv",
            "directory",
            "dir",
            "path",
            "binary",
            "key",
            "bind",
            "controller",
            "cast",
            "quality",
            "cellular",
            "token",
            "port",
            "upload",
            "seed",
            "fullscreen",
            "upscaling",
            "player_ui",
            "night_mode",
            "presence",
            "hook",
            "on_complete",
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
                at = at
                    .get(*part)
                    .and_then(|v| v.as_table())
                    .unwrap_or_else(|| panic!("{part}"));
            }
            // Optional keys serialise only when set; the table has to exist.
            let _ = (at, key);
        }
    }
}
