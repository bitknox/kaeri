//! The book source plugins this device has loaded.
//!
//! Book plugins come from the same plugin directory as show sources and are compiled by the
//! same background load (see [`crate::sources::spawn_plugin_load`]), but only while reading is
//! available: until then a `kaeri:books` component in the directory is left alone, as Kaeri
//! 0.3 leaves it. They are kept here rather than in the show chain, which only takes
//! providers.
//!
//! Ordering, switching off and the chain that asks them in turn belong to the book registry,
//! with folders and servers beside them. This holds the loaded plugins and their rows.

use std::sync::{Arc, RwLock};

use kaeri_core::traits::BookSource;
use kaeri_model::book_sources::BookPluginRow;
use kaeri_plugin::WasmBookSource;

/// The loaded book plugins, shared by every clone. Empty while reading is off.
#[derive(Clone)]
pub struct BookPlugins {
    loaded: Arc<RwLock<Vec<Arc<WasmBookSource>>>>,
    /// Whether a load from the plugin directory is under way: a series a plugin holds is not
    /// "not found" while its plugin is still compiling.
    arriving: Arc<tokio::sync::watch::Sender<bool>>,
    /// Whether a screen was drawn from a series held in a plugin still on its way, so the end of
    /// the load has to draw it again however the load went.
    leaned: Arc<std::sync::atomic::AtomicBool>,
}

impl Default for BookPlugins {
    fn default() -> Self {
        Self {
            loaded: Arc::default(),
            arriving: Arc::new(tokio::sync::watch::Sender::new(false)),
            leaned: Arc::default(),
        }
    }
}

impl std::fmt::Debug for BookPlugins {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.ids()).finish()
    }
}

impl BookPlugins {
    fn read(&self) -> std::sync::RwLockReadGuard<'_, Vec<Arc<WasmBookSource>>> {
        self.loaded.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, Vec<Arc<WasmBookSource>>> {
        self.loaded.write().unwrap_or_else(|e| e.into_inner())
    }

    /// A load from the plugin directory has started.
    pub fn expect_arrivals(&self) {
        self.arriving.send_replace(true);
    }

    /// The load has ended, loaded or not.
    pub fn arrivals_done(&self) {
        self.arriving.send_replace(false);
    }

    /// Whether a load from the plugin directory is under way.
    pub fn is_arriving(&self) -> bool {
        *self.arriving.borrow()
    }

    /// What `find` gives while a load is under way, noted when it gives something so the end of
    /// the load draws again what was drawn from it. None when no load is under way.
    pub fn lean_on_arrivals<T>(&self, find: impl FnOnce() -> Option<T>) -> Option<T> {
        if !self.is_arriving() {
            return None;
        }
        let found = find();
        if found.is_some() {
            self.leaned.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        found
    }

    /// Whether something was drawn from a load under way since this was last asked.
    pub fn take_leaned(&self) -> bool {
        self.leaned.swap(false, std::sync::atomic::Ordering::Relaxed)
    }

    /// Wait for a load under way to end, for at most `limit`. Returns at once when none is.
    pub async fn arrived(&self, limit: std::time::Duration) {
        let mut arriving = self.arriving.subscribe();
        let _ = tokio::time::timeout(limit, arriving.wait_for(|a| !*a)).await;
    }

    /// Add plugins from a directory load. A plugin with the id of one already here replaces it.
    pub fn extend(&self, plugins: impl IntoIterator<Item = WasmBookSource>) {
        for plugin in plugins {
            self.put(Arc::new(plugin));
        }
    }

    /// Add one plugin, or swap it for the loaded one with the same id: an update installed
    /// while the app runs.
    pub fn put(&self, plugin: Arc<WasmBookSource>) {
        let mut plugins = self.write();
        let id = plugin.manifest().id.clone();
        match plugins.iter_mut().find(|p| p.manifest().id == id) {
            Some(slot) => *slot = plugin,
            None => plugins.push(plugin),
        }
        plugins.sort_by(|a, b| a.manifest().id.cmp(&b.manifest().id));
    }

    /// Take a plugin out, for one that was removed while the app runs.
    pub fn remove(&self, id: &str) {
        self.write().retain(|p| p.manifest().id != id);
    }

    /// The plugin with this id.
    pub fn get(&self, id: &str) -> Option<Arc<WasmBookSource>> {
        self.read().iter().find(|p| p.manifest().id == id).cloned()
    }

    pub fn ids(&self) -> Vec<String> {
        self.read().iter().map(|p| p.manifest().id.clone()).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.read().is_empty()
    }

    /// The Sources screen's rows, by id. `store` names the store a plugin came from.
    pub fn rows(&self, store: impl Fn(&str) -> Option<String>) -> Vec<BookPluginRow> {
        self.read()
            .iter()
            .map(|plugin| {
                let manifest = plugin.manifest();
                BookPluginRow {
                    id: manifest.id.clone(),
                    name: manifest.name.clone(),
                    version: manifest.version.clone(),
                    media: manifest.media.clone(),
                    languages: manifest.languages.clone(),
                    off: false,
                    store: store(&manifest.id),
                }
            })
            .collect()
    }
}
