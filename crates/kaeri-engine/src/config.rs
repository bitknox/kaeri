//! The config as the engine owns it: what the file holds, what is in effect, and the one place
//! that writes `config.toml`.

use kaeri_core::{
    config::{Config, Paths},
    settings::SettingValue,
};
use kaeri_model::settings::{self, SettingKey, SettingOverride, SettingsView};

/// The config: the file's, and the one in effect.
///
/// Two copies, because a run can hold what the file does not. Some of it on purpose, for this
/// session only: a theme given with `--theme`, sub or dub chosen with `t` or in the player, and
/// the speed and volume the player remembers when the config says not to save them. The rest
/// for as long as a write takes. So "this session" is not a third copy but the difference
/// between these two, as the Settings rows show it: a third would only be told apart from
/// `effective` by a file edited behind the app's back.
#[derive(Debug, Clone)]
pub struct ConfigState {
    /// What `config.toml` holds, as last read: at launch, and after every write.
    saved: Config,
    /// What is in effect, and what the work reads: the config the app was launched with, and
    /// every change made since put in it, saved or not.
    effective: Config,
    /// How many shows set each setting for themselves, as the database last counted them:
    /// what the Settings rows say beside the config. Kept here so every view of the settings
    /// carries them, counted again whenever a show's settings change.
    overridden: Vec<SettingOverride>,
    /// How many shows switch new-episode notifications on for themselves, counted with
    /// `overridden`: [`SettingsView::notify_switched_on`].
    notify_switched_on: u32,
    /// The engine runs on a phone or a tablet: what the defaults that depend on the device go
    /// by, filled into both copies wherever the file leaves them unset.
    handheld: bool,
    /// The engine runs on a phone, not a tablet: what the defaults that depend on the screen's
    /// size go by.
    phone: bool,
    /// The profile's own `profile.toml`, when a profile other than the first is in use: its
    /// shared keys are read over `config.toml`'s and written here instead of there.
    overlay: Option<std::path::PathBuf>,
    /// The database whose sync log hears of a shared setting written: unset until the engine
    /// starts, and nothing is written to the log while sync is off.
    sync: Option<kaeri_store::Store>,
    /// Why finding skip times from the audio is paused, as the engine last judged it:
    /// [`SettingsView::skip_find_paused`]. See [`crate::skip_find::refresh`].
    skip_find_paused: Option<kaeri_model::playback::SkipFindPause>,
}

impl ConfigState {
    /// The config the app was launched on, in effect, and the file's: `saved` when the front
    /// end has read it, or the file as it is now. A file that does not load is read as startup
    /// reads it, as the defaults.
    pub fn new(launched: Config, saved: Option<Config>, paths: &Paths) -> Self {
        let saved = saved.unwrap_or_else(|| Config::load(paths).unwrap_or_default());
        let mut state = Self {
            saved,
            effective: launched,
            overridden: Vec::new(),
            notify_switched_on: 0,
            handheld: false,
            phone: false,
            overlay: None,
            sync: None,
            skip_find_paused: None,
        };
        state.fill();
        state
    }

    /// Whether the engine runs on a phone or a tablet, for the defaults that depend on it:
    /// torrents while casting is off there, on elsewhere.
    pub fn set_handheld(&mut self, handheld: bool) {
        self.handheld = handheld;
        self.fill();
    }

    /// A phone or a tablet, as the app said.
    pub fn handheld(&self) -> bool {
        self.handheld
    }

    /// Whether the engine runs on a phone, not a tablet, for the defaults that depend on the
    /// screen's size: novel text left-aligned there, justified elsewhere.
    pub fn set_phone(&mut self, phone: bool) {
        self.phone = phone;
    }

    /// A phone, as the app said.
    pub fn phone(&self) -> bool {
        self.phone
    }

    /// Tell `store`'s sync log of every shared setting written from here on.
    pub fn set_sync(&mut self, store: kaeri_store::Store) {
        self.sync = Some(store);
    }

    /// Say a write that landed to the sync log: a shared setting, the source order, a source
    /// switched off or on. The one place every config write passes, so none can be missed.
    fn announce(&self, table: &[&str], key: &str, value: &SettingValue, was: &Config) {
        use kaeri_store::sync::kinds;
        let Some(store) = &self.sync else { return };
        let told = if kaeri_core::shared_settings::is_shared(table, key) {
            let id = kaeri_core::shared_settings::id(table, key);
            store.sync_register(
                kinds::SETTING,
                &id,
                Some(&kaeri_core::shared_settings::encode(value)),
                None,
            )
        } else if (table, key) == (&["providers"][..], "order") {
            store.sync_register(
                kinds::SOURCE_ORDER,
                "order",
                Some(&kaeri_core::shared_settings::encode(value)),
                None,
            )
        } else if let ((&["providers"], "disabled"), SettingValue::Strings(now)) =
            ((table, key), value)
        {
            let before = &was.providers.disabled;
            let mut result = Ok(());
            for id in before.iter().filter(|id| !now.contains(id)) {
                result = result.and(store.sync_register(kinds::SOURCE_OFF, id, None, None));
            }
            for id in now.iter().filter(|id| !before.contains(id)) {
                result =
                    result.and(store.sync_register(kinds::SOURCE_OFF, id, Some("1"), None));
            }
            result
        } else {
            return;
        };
        if let Err(e) = told {
            tracing::warn!(error = %e, "could not tell the sync log of a setting");
        }
    }

    /// [`Self::write`] for a value that arrived from another device: written and read back as
    /// any is, and not announced again.
    pub fn write_received(
        &mut self,
        paths: &Paths,
        table: &[&str],
        key: &str,
        value: SettingValue,
    ) -> Result<(), kaeri_core::Error> {
        let sync = self.sync.take();
        let written = self.write(paths, table, key, value);
        self.sync = sync;
        written
    }

    /// Read the file as it is now, with the profile's overlay over it when there is one.
    fn load(&self, paths: &Paths) -> Config {
        match &self.overlay {
            Some(overlay) => kaeri_core::profile::load_with_overlay(paths, overlay),
            None => Config::load(paths),
        }
        .unwrap_or_default()
    }

    /// Use `overlay` as the profile's `profile.toml` (`None` for the first profile, whose keys
    /// are in `config.toml`), and put its shared settings in effect, over whatever the last
    /// profile had.
    pub fn use_overlay(&mut self, overlay: Option<std::path::PathBuf>, paths: &Paths) {
        self.overlay = overlay;
        self.saved = self.load(paths);
        self.fill();
        self.adopt_shared();
    }

    /// Put the file's shared settings in effect: what a profile switch or a setting received
    /// from another device changed. Everything not on the shared list is left as it is, the
    /// session's own choices included.
    pub fn adopt_shared(&mut self) {
        let (Ok(from), Ok(mut to)) =
            (serde_json::to_value(&self.saved), serde_json::to_value(&self.effective))
        else {
            return;
        };
        for (table, key) in kaeri_core::shared_settings::SHARED {
            let mut src = Some(&from);
            for segment in *table {
                src = src.and_then(|v| v.get(segment));
            }
            let value = src.and_then(|v| v.get(key)).cloned();
            let mut dst = Some(&mut to);
            for segment in *table {
                dst = dst.and_then(|v| v.get_mut(segment));
            }
            if let Some(object) = dst.and_then(|v| v.as_object_mut()) {
                match value {
                    Some(value) => {
                        object.insert((*key).to_owned(), value);
                    }
                    None => {
                        object.remove(*key);
                    }
                }
            }
        }
        if let Ok(adopted) = serde_json::from_value::<Config>(to) {
            self.effective = adopted;
            self.fill();
        }
    }

    /// Write `table`.`key` where the active profile keeps it: its overlay for a shared key
    /// while another profile than the first is in use, else the config.
    fn put_key(
        &self,
        paths: &Paths,
        table: &[&str],
        key: &str,
        value: SettingValue,
    ) -> Result<(), kaeri_core::Error> {
        if let Some(overlay) = &self.overlay
            && kaeri_core::profile::write_shared(overlay, table, key, value.clone())?
        {
            return Ok(());
        }
        kaeri_core::settings::write_key(paths, table, key, value)
    }

    /// The defaults that depend on the device, where a copy leaves them unset. Unset reads as
    /// on, so only a phone or a tablet fills anything in.
    fn fill(&mut self) {
        if !self.handheld {
            return;
        }
        for config in [&mut self.saved, &mut self.effective] {
            config.providers.torrent.while_casting.get_or_insert(false);
        }
    }

    /// The config in effect.
    pub fn effective(&self) -> &Config {
        &self.effective
    }

    /// The config in effect, for a change that is put in effect without a write: the session's
    /// own choices, and the player's.
    pub fn effective_mut(&mut self) -> &mut Config {
        &mut self.effective
    }

    /// The file, as last read.
    pub fn saved(&self) -> &Config {
        &self.saved
    }

    /// What a front end is told of it, with the counts of shows that set each setting.
    pub fn view(&self) -> SettingsView {
        SettingsView {
            overridden: self.overridden.clone(),
            notify_switched_on: self.notify_switched_on,
            // What this build holds, not what the config asks: a phone's engine has no Discord.
            presence_available: cfg!(feature = "presence"),
            skip_find_paused: self.skip_find_paused,
            ..settings::view(&self.effective, &self.saved)
        }
    }

    /// Why finding skip times is paused, as last judged.
    pub const fn skip_find_paused(&self) -> Option<kaeri_model::playback::SkipFindPause> {
        self.skip_find_paused
    }

    pub const fn set_skip_find_paused(
        &mut self,
        paused: Option<kaeri_model::playback::SkipFindPause>,
    ) {
        self.skip_find_paused = paused;
    }

    /// The counts of shows that set each setting for themselves, and of those that switch
    /// new-episode notifications on, as the database has them now.
    pub fn set_overridden(
        &mut self,
        overridden: Vec<SettingOverride>,
        notify_switched_on: u32,
    ) {
        self.overridden = overridden;
        self.notify_switched_on = notify_switched_on;
    }

    /// How many shows set `key` for themselves.
    pub fn shows_setting(&self, key: SettingKey) -> u32 {
        self.overridden
            .iter()
            .find(|counted| counted.key == key)
            .map_or(0, |counted| counted.shows)
    }

    /// A new install reads as having the demo on while there is no config file. A write that
    /// made the file says so too, or the next launch would read the file as the demo off.
    fn remember_demo(&self, paths: &Paths, existed: bool) {
        if !existed && paths.config_file.exists() && self.effective.sources.demo {
            let _ = kaeri_core::settings::write_key(
                paths,
                &["sources"],
                "demo",
                SettingValue::Bool(true),
            );
        }
    }

    /// Write `value` at `table`.`key`, then read the file back, whatever the write did: what
    /// the file holds is what the Settings screen shows. A refused write — a value the config's
    /// rules refuse, a file that did not load, a disk that said no — puts that one setting in effect
    /// back to what the file holds, and leaves every other alone, a change made a moment later
    /// whose own write worked included.
    ///
    /// Synchronous on purpose — one small file, and a settings change that silently lost a race
    /// with the next one would be worse than a paused frame.
    pub fn write(
        &mut self,
        paths: &Paths,
        table: &[&str],
        key: &str,
        value: SettingValue,
    ) -> Result<(), kaeri_core::Error> {
        let existed = paths.config_file.exists();
        let was = self.saved.clone();
        let announced = value.clone();
        let written = self.put_key(paths, table, key, value);
        self.remember_demo(paths, existed);
        self.saved = self.load(paths);
        if written.is_ok() {
            self.announce(table, key, &announced, &was);
        }
        self.fill();
        if written.is_err()
            && let Some(setting) = SettingKey::at(table, key)
        {
            setting.copy(&self.saved, &mut self.effective);
        }
        written
    }

    /// Write `edits` and `removals` as one change, then read the file back, as [`Self::write`]
    /// does, and put in effect what the file then holds of the torrent proxy, whether the
    /// change was refused or not.
    pub fn write_proxy(
        &mut self,
        paths: &Paths,
        edits: Vec<kaeri_model::proxy::Edit>,
        removals: Vec<kaeri_model::proxy::Removal>,
    ) -> Result<(), kaeri_core::Error> {
        let existed = paths.config_file.exists();
        let written = kaeri_core::settings::write_changes(paths, edits, removals);
        self.remember_demo(paths, existed);
        self.saved = self.load(paths);
        self.fill();
        self.effective.providers.torrent.proxy.clone_from(&self.saved.providers.torrent.proxy);
        written
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths_in(dir: &std::path::Path) -> Paths {
        Paths {
            config_file: dir.join("config.toml"),
            data_dir: dir.join("data"),
            cache_dir: dir.join("cache"),
        }
    }

    #[test]
    fn a_refused_write_puts_back_only_its_own_setting() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let mut config = ConfigState::new(Config::default(), None, &paths);

        config.effective_mut().playback.quality = 2160;
        config.write(&paths, &["playback"], "quality", SettingValue::Int(2160)).unwrap();
        assert_eq!(config.saved().playback.quality, 2160, "read back from the file");

        // Marked watched at 200%: refused by the config's own rules.
        config.effective_mut().playback.commit_threshold = 2.0;
        let refused =
            config.write(&paths, &["playback"], "commit_threshold", SettingValue::Float(2.0));
        assert!(refused.is_err());
        assert!(
            config.effective().playback.commit_threshold <= 1.0,
            "back to what the file holds"
        );
        assert_eq!(config.effective().playback.quality, 2160, "and nothing else");
    }

    #[test]
    fn torrents_while_casting_is_off_on_phones_and_tablets_unless_the_file_says() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let config = ConfigState::new(Config::default(), None, &paths);
        assert!(kaeri_model::settings::while_casting(config.effective()));

        let mut phone = ConfigState::new(Config::default(), None, &paths);
        phone.set_handheld(true);
        assert_eq!(phone.effective().providers.torrent.while_casting, Some(false));
        phone
            .write(&paths, &["providers", "torrent"], "while_casting", SettingValue::Bool(true))
            .unwrap();
        assert_eq!(phone.saved().providers.torrent.while_casting, Some(true), "the file's own");
    }

    fn with_sync(dir: &std::path::Path) -> (ConfigState, kaeri_store::Store, Paths) {
        let paths = paths_in(dir);
        let store = kaeri_store::Store::open_in_memory().unwrap();
        store.sync_enable("aaaa").unwrap();
        let mut config = ConfigState::new(Config::default(), None, &paths);
        config.set_sync(store.clone());
        (config, store, paths)
    }

    #[test]
    fn a_shared_setting_is_announced_and_a_device_bound_one_is_not() {
        let dir = tempfile::tempdir().unwrap();
        let (mut config, store, paths) = with_sync(dir.path());
        config.write(&paths, &["playback"], "skip_opening", SettingValue::Bool(true)).unwrap();
        config.write(&paths, &["playback"], "quality", SettingValue::Int(720)).unwrap();
        let regs = store.sync_regs("setting").unwrap();
        assert_eq!(regs.len(), 1, "{regs:?}");
        assert_eq!(regs[0].0, "playback.skip_opening");
        assert_eq!(
            kaeri_core::shared_settings::decode(&regs[0].1),
            Some(SettingValue::Bool(true))
        );
    }

    #[test]
    fn the_source_order_and_a_source_switched_off_are_announced() {
        let dir = tempfile::tempdir().unwrap();
        let (mut config, store, paths) = with_sync(dir.path());
        let order = vec!["plugins".to_owned(), "torrent".to_owned()];
        config
            .write(&paths, &["providers"], "order", SettingValue::Strings(order.clone()))
            .unwrap();
        config
            .write(
                &paths,
                &["providers"],
                "disabled",
                SettingValue::Strings(vec!["one".into()]),
            )
            .unwrap();
        config
            .write(&paths, &["providers"], "disabled", SettingValue::Strings(vec![]))
            .unwrap();
        assert_eq!(store.sync_regs("source_order").unwrap().len(), 1);
        // Switched off, then on again: the register is cleared, so it reads as on everywhere.
        assert!(store.sync_regs("source_off").unwrap().is_empty());
        assert!(store.sync_reg_value("source_off", "one").unwrap().is_none());
    }

    #[test]
    fn a_received_setting_is_written_and_never_announced_again() {
        let dir = tempfile::tempdir().unwrap();
        let (mut config, store, paths) = with_sync(dir.path());
        config
            .write_received(&paths, &["playback"], "auto_next", SettingValue::Bool(false))
            .unwrap();
        assert!(!config.saved().playback.auto_next);
        assert!(store.sync_regs("setting").unwrap().is_empty());
        // And the announcing is back for the next one.
        config.write(&paths, &["playback"], "auto_next", SettingValue::Bool(true)).unwrap();
        assert_eq!(store.sync_regs("setting").unwrap().len(), 1);
    }

    #[test]
    fn a_second_profile_keeps_its_shared_settings_in_its_overlay() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let mut config = ConfigState::new(Config::default(), None, &paths);
        config.write(&paths, &["playback"], "skip_opening", SettingValue::Bool(false)).unwrap();
        let overlay = dir.path().join("profiles/p2/profile.toml");
        config.use_overlay(Some(overlay.clone()), &paths);

        config.write(&paths, &["playback"], "skip_opening", SettingValue::Bool(true)).unwrap();
        config.write(&paths, &["playback"], "quality", SettingValue::Int(480)).unwrap();
        assert!(config.saved().playback.skip_opening, "the profile's own");
        let file = std::fs::read_to_string(&paths.config_file).unwrap();
        assert!(
            file.contains("skip_opening = false"),
            "config.toml is the first profile's: {file}"
        );
        assert!(file.contains("quality = 480"), "device-bound keys stay in the config: {file}");
        assert!(std::fs::read_to_string(&overlay).unwrap().contains("skip_opening = true"));

        // Back to the first profile: its own value is in effect again.
        config.use_overlay(None, &paths);
        assert!(!config.effective().playback.skip_opening);
        assert_eq!(config.saved().playback.quality, 480, "the device's is untouched");
    }
}
