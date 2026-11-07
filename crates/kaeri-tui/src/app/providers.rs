//! The Providers table: the sources failover walks and their order, and the header's chips
//! for the sources, the metadata ladder and the torrent proxy.

use kaeri_i18n::t;
use kaeri_model::{
    command::Command,
    settings::SettingKey,
    sources::{
        ProviderRow, ProxyStatus, SourceChip, SourceState, order_with_source_moved,
        source_states,
    },
};

use super::{App, alert};
use crate::{
    nav::{Section, StageView},
    theme::glyph,
};

/// The header's `proxy` chip for the proxy check's result, and whether it alerts; `None` when
/// torrents connect directly, which needs no chip.
///
/// A check still running does not alert: it is not yet failing anyone.
pub fn proxy_badge(status: ProxyStatus) -> Option<(String, bool)> {
    match status {
        ProxyStatus::Checking => {
            Some((t::sources_proxy_checking(&glyph::STATE_UNKNOWN.to_string()), false))
        }
        ProxyStatus::Passed => {
            Some((t::sources_proxy_passed(&glyph::STATE_READY.to_string()), false))
        }
        ProxyStatus::Failed => {
            Some((t::sources_proxy_failed(&glyph::STATE_DOWN.to_string()), true))
        }
        ProxyStatus::Direct => None,
    }
}

impl App {
    /// What the status line should call the source: the provider that last actually served,
    /// before anything has played the first configured source that is ready, and failing that
    /// the first one that is not switched off.
    ///
    /// Not simply the first entry in the order: one switched off would be named over a session
    /// that only the others could serve.
    pub fn source_label(&self) -> String {
        if let Some(active) = &self.active_provider {
            return active.clone();
        }
        let sources = self.source_states();
        sources
            .iter()
            .find(|chip| chip.state == SourceState::Ready)
            .or_else(|| sources.iter().find(|chip| chip.state != SourceState::Off))
            .or_else(|| sources.first())
            .map_or_else(t::sources_none_label, |chip| chip.id.clone())
    }

    /// Each configured source, in the order failover walks them, and how it stands: see
    /// [`source_states`].
    pub fn source_states(&self) -> Vec<SourceChip> {
        source_states(&self.settings.sources, &self.providers)
    }

    /// The Providers table as it stands.
    pub(super) fn apply_providers(&mut self, rows: Vec<ProviderRow>) {
        let had = self.providers.iter().any(|row| row.usable);
        self.providers = rows;
        if !had && self.providers.iter().any(|row| row.usable) {
            self.sources_arrived();
        }
        // The cursor stays in range: a removed plugin shortens the table under it.
        let last = self.providers.len().saturating_sub(1);
        self.cursors.providers.selected = self.cursors.providers.selected.min(last);
    }

    /// The proxy check's result, for the header's chip: `None` when torrents are off.
    pub(super) fn apply_proxy(&mut self, status: Option<ProxyStatus>) {
        let (badge, failed) = status.and_then(proxy_badge).unzip();
        let failed = failed.unwrap_or(false);
        // A check that has just failed is worth interrupting for: torrents wait for it, and
        // the viewer needs to know why playback does not start.
        if failed && !self.proxy_failed {
            self.push_toast(alert(t::sources_proxy_failed_note()));
        }
        self.proxy_badge = badge;
        self.proxy_failed = failed;
    }

    /// Whether metadata is currently served by anything other than AniList.
    ///
    /// The header chip's question. Answered from the last status report, so it holds
    /// between Providers-screen visits.
    pub fn meta_degraded(&self) -> bool {
        self.meta_status.iter().any(|row| row.active && row.source != "anilist")
    }

    /// Providers' own view, where Enter opens the stores.
    pub(super) fn on_providers(&self) -> bool {
        self.nav.section() == Section::Providers
            && matches!(self.nav.current(), StageView::Section(Section::Providers))
    }

    /// `K` or `J` on Providers: the selected source one place up or down the chain, saved.
    ///
    /// The rows move at once and the cursor with its row; the run loop puts the live chain in
    /// the same order once the file has it. At either end nothing moves and nothing is saved.
    pub(super) fn move_source(&mut self, step: isize) -> Option<Command> {
        let id = self.providers.get(self.cursors.providers.selected)?.id.clone();
        let order =
            order_with_source_moved(&self.settings.sources.order, &self.providers, &id, step)?;
        // Every row is named in the new order, so its place there is its place in the chain.
        let place = |row: &ProviderRow| order.iter().position(|entry| *entry == row.id);
        self.providers.sort_by_key(|row| place(row).unwrap_or(order.len()));
        self.cursors.providers.selected =
            self.providers.iter().position(|row| row.id == id).unwrap_or(0);
        // Taken as the order here before the engine says it is, so a second press moves on
        // from this one rather than from the order before it. The engine's answer is the same
        // order, or on a refusal the file's, with the chain's own rows.
        self.settings.sources.order.clone_from(&order);
        Some(Command::SetSetting {
            key: SettingKey::SourceOrder,
            value: kaeri_core::settings::SettingValue::Strings(order),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::fixtures::*;

    #[test]
    fn the_source_named_is_one_that_is_not_switched_off() {
        let mut a = app();
        assert_eq!(a.source_label(), "plugins");
        assert_eq!(a.source_states(), chips(&[("plugins", SourceState::Pending)]));
        a.apply(Event::Providers(vec![ProviderRow {
            id: "someplugin".into(),
            kind: "plugin".into(),
            state: "ready".into(),
            latency_ms: None,
            last_error: None,
            usable: true,
            held_back: false,
            ..Default::default()
        }]));
        assert_eq!(a.source_states(), chips(&[("plugins", SourceState::Ready)]));
        // What actually served wins once something has played.
        a.apply(Event::ActiveProvider("someplugin".into()));
        assert_eq!(a.source_label(), "someplugin");
    }

    fn chips(states: &[(&str, SourceState)]) -> Vec<SourceChip> {
        states
            .iter()
            .map(|(id, state)| SourceChip { id: (*id).into(), state: *state })
            .collect()
    }

    fn source_row(id: &str, kind: &str) -> ProviderRow {
        ProviderRow {
            id: id.into(),
            kind: kind.into(),
            state: "ready".into(),
            latency_ms: None,
            last_error: None,
            usable: true,
            held_back: false,
            ..Default::default()
        }
    }

    fn strings(entries: &[&str]) -> Vec<String> {
        entries.iter().map(|e| (*e).to_owned()).collect()
    }

    #[test]
    fn a_named_plugin_adds_no_chip_of_its_own() {
        // The `"plugins"` chip speaks for every plugin, named in the order or not.
        let mut a = app();
        // `torrent`, the torrent search that is no longer built in, adds none either.
        a.settings.sources.order = strings(&["alpha", "torrent", "plugins"]);
        a.apply(Event::Providers(vec![source_row("alpha", "plugin")]));
        assert_eq!(a.source_states(), chips(&[("plugins", SourceState::Ready)]));
    }

    /// An app on Providers with the stage focused, showing a remote source and two plugins.
    fn at_providers() -> App {
        let mut a = app();
        a.settings.sources.order = strings(&["remote", "plugins"]);
        a.go_to_section(Section::Providers);
        a.nav.focus_stage();
        a.apply(Event::Providers(vec![
            source_row("remote", "remote"),
            source_row("alpha", "plugin"),
            source_row("beta", "plugin"),
        ]));
        a
    }

    fn press(a: &mut App, ch: char) -> Option<Command> {
        a.press(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE), 20)
    }

    /// The order a task saves, for a task that must be the order being saved.
    fn saved_order(task: Option<Command>) -> Vec<String> {
        match task {
            Some(Command::SetSetting {
                key: SettingKey::SourceOrder,
                value: SettingValue::Strings(order),
            }) => order,
            other => panic!("not an order save: {other:?}"),
        }
    }

    fn row_ids(a: &App) -> Vec<&str> {
        a.providers.iter().map(|row| row.id.as_str()).collect()
    }

    #[test]
    fn shift_k_and_j_move_the_selected_source_and_save_the_order() {
        let mut a = at_providers();
        press(&mut a, 'j');
        assert_eq!(a.cursor(), 1, "j moves the cursor, as on every table");

        let order = saved_order(press(&mut a, 'K'));
        assert_eq!(order, strings(&["alpha", "remote", "beta", "plugins"]));
        assert_eq!(row_ids(&a), ["alpha", "remote", "beta"], "the rows move at once");
        assert_eq!(a.cursor(), 0, "the cursor goes with its row");
        assert_eq!(a.settings.sources.order, order, "the next press starts from it");

        // At the top: nothing moves and nothing is saved.
        assert_eq!(press(&mut a, 'K'), None);
        assert_eq!(row_ids(&a), ["alpha", "remote", "beta"]);

        let order = saved_order(press(&mut a, 'J'));
        assert_eq!(order, strings(&["remote", "alpha", "beta", "plugins"]));
        assert_eq!(row_ids(&a), ["remote", "alpha", "beta"]);
        assert_eq!(a.cursor(), 1);
    }

    #[test]
    fn the_order_keys_work_while_something_plays() {
        // The playback table takes neither key, so they still reach the Providers table.
        let mut a = at_providers();
        a.playing = Some(NowPlaying::default());
        let order = saved_order(press(&mut a, 'J'));
        assert_eq!(order, strings(&["alpha", "remote", "beta", "plugins"]));
    }

    #[test]
    fn the_order_keys_move_nothing_off_the_providers_table() {
        let mut a = at_providers();
        a.nav.push(StageView::Stores);
        assert_eq!(press(&mut a, 'J'), None);
        assert_eq!(row_ids(&a), ["remote", "alpha", "beta"]);

        let mut a = app();
        assert_eq!(a.handle(Action::RaiseSource, 20), None);
        assert!(
            a.toasts.iter().any(|t| t.notice.text
                == kaeri_i18n::t::toast_works_elsewhere(
                    Action::RaiseSource.label(),
                    Action::RaiseSource.where_it_works()
                )),
            "{:?}",
            a.toasts
        );
    }

    #[test]
    fn the_order_is_put_back_when_the_write_fails() {
        let mut a = at_providers();
        let mut owner = Owner::new(Config::default());
        owner.effective.providers.order = a.settings.sources.order.clone();
        owner.saved.providers.order = a.settings.sources.order.clone();
        let before = a.settings.sources.order.clone();
        let task = press(&mut a, 'J');
        assert_ne!(a.settings.sources.order, before);
        owner.refuse(&mut a, task);
        assert_eq!(a.settings.sources.order, before);
    }

    #[test]
    fn a_new_order_does_not_say_the_sources_are_reloading() {
        // Nothing is rebuilt for a reorder, so the status line must not claim a reload. All a
        // reorder sends the interface is the chain's own rows, in their new order.
        let mut a = at_providers();
        let before = a.status.clone();
        a.apply(Event::Providers(vec![
            source_row("alpha", "plugin"),
            source_row("remote", "remote"),
            source_row("beta", "plugin"),
        ]));
        assert_eq!(a.status, before);
        assert_eq!(row_ids(&a), ["alpha", "remote", "beta"]);
    }

    #[test]
    fn a_shorter_providers_table_keeps_the_cursor_on_a_row() {
        let mut a = at_providers();
        a.handle(Action::Bottom, 20);
        assert_eq!(a.cursor(), 2);
        a.apply(Event::Providers(vec![source_row("remote", "remote")]));
        assert_eq!(a.cursor(), 0);
    }

    #[test]
    fn the_meta_chip_appears_only_while_a_fallback_is_serving() {
        let mut a = app();
        assert!(!a.meta_degraded(), "no status yet is not degraded");

        let row = |source: &str, active: bool| MetaSourceRow {
            source: source.into(),
            state: String::new(),
            healthy: true,
            active,
            detail: None,
            retry_at: None,
        };
        a.apply(Event::MetaStatus(vec![row("anilist", true), row("tenrai", false)]));
        assert!(!a.meta_degraded(), "anilist serving is the resting state");

        a.apply(Event::MetaStatus(vec![row("anilist", false), row("tenrai", true)]));
        assert!(a.meta_degraded());
    }

    fn demo_state(on: bool, only: bool, played: bool) -> kaeri_model::sources::DemoState {
        kaeri_model::sources::DemoState { on, offer: !on, only, played, ..Default::default() }
    }

    #[test]
    fn the_first_run_asks_once_and_enter_plays_the_demo() {
        let mut a = app();
        a.apply(Event::Demo(demo_state(true, true, false)));
        assert_eq!(a.nav.overlay(), Some(&crate::nav::Overlay::FirstRun));
        let played = a.press(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), 20);
        assert_eq!(played, Some(Command::PlayDemo));
        assert!(!a.nav.has_overlay());
        a.apply(Event::Demo(demo_state(true, true, false)));
        assert!(!a.nav.has_overlay(), "asked once a session");
    }

    #[test]
    fn the_first_run_can_be_skipped_or_go_to_an_invite() {
        let mut skipped = app();
        skipped.apply(Event::Demo(demo_state(true, true, false)));
        assert_eq!(press(&mut skipped, 's'), None);
        assert!(!skipped.nav.has_overlay());

        let mut invited = app();
        invited.apply(Event::Demo(demo_state(true, true, false)));
        assert_eq!(press(&mut invited, 'i'), None);
        assert_eq!(invited.nav.overlay(), Some(&crate::nav::Overlay::JoinStore));
    }

    #[test]
    fn nothing_asks_when_the_demo_is_not_the_only_source_or_was_played() {
        for state in [
            demo_state(true, false, false),
            demo_state(true, true, true),
            demo_state(false, false, false),
        ] {
            let mut a = app();
            a.apply(Event::Demo(state));
            assert!(!a.nav.has_overlay());
        }
    }

    #[test]
    fn the_demo_keys_play_it_and_take_it_out_or_bring_it_back() {
        let mut a = at_providers();
        a.apply(Event::Demo(demo_state(true, false, false)));
        assert_eq!(press(&mut a, 'B'), Some(Command::PlayDemo));
        assert_eq!(press(&mut a, 'Y'), Some(Command::RemoveDemo));
        a.apply(Event::Demo(demo_state(false, false, false)));
        assert_eq!(press(&mut a, 'Y'), Some(Command::AddDemo));
    }
}
