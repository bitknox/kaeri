//! Asking a plugin what it is.
//!
//! Where a WebAssembly runtime is available — the reference server, the app — an [`Inspector`]
//! loads the plugin with no capabilities and asks. Where none is, a store reads the manifest the
//! plugin carries instead (see [`crate::manifest`]).

pub use crate::manifest::PluginInfo;
use crate::{
    MaybeSendSync,
    protocol::{IndexEntry, host_set},
};

#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
pub trait Inspector: MaybeSendSync {
    /// Load `wasm` with no capabilities at all and return its manifest.
    ///
    /// An error is a plugin that does not load, said in words a person can act on.
    async fn inspect(&self, wasm: &[u8]) -> Result<PluginInfo, String>;
}

/// Where a plugin's own description and a listing disagree, if they do.
///
/// The listing is what the user was shown and agreed to. A plugin that declares a different id,
/// version, kind or set of hosts is not the thing they agreed to, whatever the cause. The kind
/// is what keeps a book source listed as a show source, by a store from before book sources,
/// from being installed as one.
pub fn mismatch(info: &PluginInfo, entry: &IndexEntry) -> Option<String> {
    if info.kind() != entry.kind() {
        return Some(format!(
            "{} is a source of {} but the store lists it as a source of {}",
            entry.id,
            info.kind(),
            entry.kind()
        ));
    }
    differs(info, &entry.id, &entry.version, &entry.allowed_hosts)
}

/// Where two descriptions of the same plugin disagree, if they do: what a plugin's manifest
/// section says against what the plugin says when asked.
pub fn disagrees(carried: &PluginInfo, asked: &PluginInfo) -> Option<String> {
    if carried.kind() != asked.kind() {
        return Some(format!(
            "it says it is a source of {} but is a source of {}",
            carried.kind(),
            asked.kind()
        ));
    }
    differs(carried, &asked.id, &asked.version, &asked.allowed_hosts)
}

/// Where two descriptions of the same plugin disagree on id, version or hosts, if they do.
pub fn differs(info: &PluginInfo, id: &str, version: &str, hosts: &[String]) -> Option<String> {
    if info.id != id {
        return Some(format!("the plugin calls itself {} but the store lists {id}", info.id));
    }
    if info.version != version {
        return Some(format!("{id} is version {} but the store lists {version}", info.version));
    }
    if host_set(&info.allowed_hosts) != host_set(hosts) {
        return Some(format!(
            "{id} asks to contact {} but the store lists {}",
            describe_hosts(&info.allowed_hosts),
            describe_hosts(hosts)
        ));
    }
    None
}

fn describe_hosts(hosts: &[String]) -> String {
    if hosts.is_empty() { "nothing".into() } else { hosts.join(", ") }
}

#[cfg(any(test, feature = "testing"))]
pub mod fake {
    //! An inspector that reads a manifest out of a JSON "plugin", for tests.

    use super::*;

    /// Bytes standing in for a book source: the JSON of a manifest with its kind and media.
    pub fn book_plugin(id: &str, version: &str, hosts: &[&str], media: &[&str]) -> Vec<u8> {
        serde_json::json!({
            "id": id,
            "name": format!("{id} plugin"),
            "version": version,
            "hosts": hosts,
            "kind": "books",
            "media": media,
        })
        .to_string()
        .into_bytes()
    }

    /// Bytes standing in for a component: the JSON of a manifest.
    pub fn plugin(id: &str, version: &str, hosts: &[&str]) -> Vec<u8> {
        serde_json::json!({
            "id": id,
            "name": format!("{id} plugin"),
            "version": version,
            "hosts": hosts,
        })
        .to_string()
        .into_bytes()
    }

    pub struct FakeInspector;

    #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
    #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
    impl Inspector for FakeInspector {
        async fn inspect(&self, wasm: &[u8]) -> Result<PluginInfo, String> {
            let value: serde_json::Value =
                serde_json::from_slice(wasm).map_err(|_| "not a plugin".to_owned())?;
            let text = |key: &str| value[key].as_str().unwrap_or_default().to_owned();
            let list = |key: &str| -> Vec<String> {
                value[key]
                    .as_array()
                    .map(|a| a.iter().filter_map(|h| h.as_str().map(str::to_owned)).collect())
                    .unwrap_or_default()
            };
            let kind = value["kind"].as_str().map(str::to_owned);
            Ok(PluginInfo {
                id: text("id"),
                name: text("name"),
                version: text("version"),
                allowed_hosts: list("hosts"),
                translations: if kind.is_some() { Vec::new() } else { vec!["sub".into()] },
                media: list("media"),
                kind,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(hosts: &[&str]) -> IndexEntry {
        IndexEntry {
            id: "example".into(),
            name: "Example".into(),
            version: "1.0.0".into(),
            allowed_hosts: hosts.iter().map(|h| (*h).to_owned()).collect(),
            translations: Vec::new(),
            sha256: String::new(),
            size: 0,
            published_at: 0,
            kind: None,
            media: Vec::new(),
        }
    }

    fn info(id: &str, version: &str, hosts: &[&str]) -> PluginInfo {
        PluginInfo {
            id: id.into(),
            name: String::new(),
            version: version.into(),
            allowed_hosts: hosts.iter().map(|h| (*h).to_owned()).collect(),
            translations: Vec::new(),
            kind: None,
            media: Vec::new(),
        }
    }

    #[test]
    fn a_plugin_matching_its_listing_passes() {
        let listed = entry(&["api.example", "cdn.example"]);
        assert_eq!(
            mismatch(&info("example", "1.0.0", &["CDN.example", "api.example"]), &listed),
            None
        );
    }

    #[test]
    fn an_extra_host_is_a_mismatch() {
        let listed = entry(&["api.example"]);
        let found =
            mismatch(&info("example", "1.0.0", &["api.example", "else.example"]), &listed);
        assert!(found.unwrap().contains("else.example"));
    }

    #[test]
    fn a_book_source_listed_as_a_show_source_is_a_mismatch() {
        // What a store from before book sources would list, reading only the section's id,
        // version and hosts.
        let listed = entry(&["api.example"]);
        let books = PluginInfo {
            kind: Some("books".into()),
            ..info("example", "1.0.0", &["api.example"])
        };
        assert!(mismatch(&books, &listed).unwrap().contains("books"));
        let listed = IndexEntry { kind: Some("books".into()), ..listed };
        assert_eq!(mismatch(&books, &listed), None);
        assert!(mismatch(&info("example", "1.0.0", &["api.example"]), &listed).is_some());
    }

    #[test]
    fn a_different_id_or_version_is_a_mismatch() {
        let listed = entry(&[]);
        assert!(mismatch(&info("other", "1.0.0", &[]), &listed).is_some());
        assert!(mismatch(&info("example", "1.0.1", &[]), &listed).is_some());
    }
}
