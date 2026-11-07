//! The two or three hints at the right of the status line, in place of a footer. They are
//! read from the state the keys are read in, so a hint never names a key that does
//! something else here.

use kaeri_i18n::t;

use super::{Action, Binding, Keymap, said};
use crate::nav::Section;

/// Two keys drawn as one hint.
fn pair(first: &str, second: &str) -> String {
    format!("{first}/{second}") // i18n-skip
}

const ESC: &str = "esc"; // i18n-skip

fn hint_add_source() -> &'static str {
    said(t::keys_hint_add_source())
}

fn hint_back() -> &'static str {
    said(t::keys_hint_back())
}

fn hint_cancel() -> &'static str {
    said(t::keys_hint_cancel())
}

fn hint_change() -> &'static str {
    said(t::keys_hint_change())
}

fn hint_check() -> &'static str {
    said(t::keys_hint_check())
}

fn hint_choose() -> &'static str {
    said(t::keys_hint_choose())
}

fn hint_close() -> &'static str {
    said(t::keys_hint_close())
}

fn hint_confirm() -> &'static str {
    said(t::keys_hint_confirm())
}

fn hint_keep() -> &'static str {
    said(t::keys_hint_keep())
}

fn hint_list() -> &'static str {
    said(t::keys_hint_list())
}

fn hint_make_link() -> &'static str {
    said(t::keys_hint_make_link())
}

fn hint_move() -> &'static str {
    said(t::keys_hint_move())
}

fn hint_next() -> &'static str {
    said(t::keys_hint_next())
}

fn hint_open() -> &'static str {
    said(t::keys_hint_open())
}

fn hint_order() -> &'static str {
    said(t::keys_hint_order())
}

fn hint_page() -> &'static str {
    said(t::keys_hint_page())
}

fn hint_pause() -> &'static str {
    said(t::keys_hint_pause())
}

fn hint_pin() -> &'static str {
    said(t::keys_hint_pin())
}

fn hint_not_interested() -> &'static str {
    said(t::keys_hint_not_interested())
}

fn hint_play() -> &'static str {
    said(t::keys_hint_play())
}

fn hint_publish() -> &'static str {
    said(t::keys_hint_publish())
}

fn hint_queue() -> &'static str {
    said(t::keys_hint_queue())
}

fn hint_rate() -> &'static str {
    said(t::keys_hint_rate())
}

fn hint_resolve() -> &'static str {
    said(t::keys_hint_resolve())
}

fn hint_results() -> &'static str {
    said(t::keys_hint_results())
}

fn hint_resume() -> &'static str {
    said(t::keys_hint_resume())
}

fn hint_run() -> &'static str {
    said(t::keys_hint_run())
}

fn hint_save() -> &'static str {
    said(t::keys_hint_save())
}

fn hint_search() -> &'static str {
    said(t::keys_hint_search())
}

fn hint_set() -> &'static str {
    said(t::keys_hint_set())
}

fn hint_sign_in() -> &'static str {
    said(t::keys_hint_sign_in())
}

fn hint_sign_out() -> &'static str {
    said(t::keys_hint_sign_out())
}

fn hint_ready() -> &'static str {
    said(t::keys_hint_ready())
}

fn hint_copy_link() -> &'static str {
    said(t::keys_hint_copy_link())
}

fn hint_leave_party() -> &'static str {
    said(t::keys_hint_leave_party())
}

fn hint_start_now() -> &'static str {
    said(t::keys_hint_start_now())
}

fn hint_stores() -> &'static str {
    said(t::keys_hint_stores())
}

fn hint_use() -> &'static str {
    said(t::keys_hint_use())
}

/// The two or three contextual hints shown at the right of the status line.
///
/// Deliberately short. A full footer of bindings is the templated look this design avoids;
/// these are only the actions most likely to be wanted right now — and each has to be true
/// where it is shown, so they follow the state the keys are read in, not just the screen:
/// an open overlay shows its own keys, a text field shows no plain keys at all (they would be
/// typed), and Enter is named for what it does here. A narrow status line drops hints from the
/// front, so the last — `? keys`, or an overlay's way out — is the one that always shows.
pub fn status_hints(app: &crate::app::App) -> Vec<(String, &'static str)> {
    status_hints_paged(app, app.help_pages())
}

/// [`status_hints`], with the help table running to `help_pages`: the frame being drawn says,
/// once it has laid the table out, where the App still holds what the last frame said.
pub(crate) fn status_hints_paged(
    app: &crate::app::App,
    help_pages: usize,
) -> Vec<(String, &'static str)> {
    use crate::nav::StageView as V;
    if let Some(overlay) = app.nav.overlay() {
        return overlay_hints(app, overlay, help_pages);
    }
    if app.is_typing() {
        // Search's field: letters are the query, so only the keys that cannot be typed are
        // offered. Esc and Enter are fixed for every field; see `modal_action`.
        let enter = if app.search_opens_on_enter() { hint_open() } else { hint_search() };
        let mut hints = vec![("↵".to_string(), enter)];
        // ↓ is how the keyboard gets from the field into the results; with none it does nothing.
        if !app.content.is_empty() {
            hints.push(("↓".to_string(), hint_results()));
        }
        if let Some(key) = app.keymap.keys_for(Action::FilterSearch).first() {
            hints.push((key.render(), Action::FilterSearch.hint()));
        }
        hints.push((ESC.to_string(), hint_back()));
        return hints;
    }

    let keymap = &app.keymap;
    // By the table the keys are read from now, so a hint is never a key the player has taken.
    let context = app.key_context();
    let key = |action: Action| keymap.keys_in(action, context).first().map(Binding::render);
    // Left and Right as one hint: the arrows, as every overlay names them, while they do it,
    // and the live keys when the player has taken the arrows to seek.
    let sideways = || {
        use crossterm::event::KeyCode;
        let bound = |action: Action, code: KeyCode| {
            keymap
                .keys_in(action, context)
                .iter()
                .any(|b| b.code == code && b.modifiers.is_empty())
        };
        if bound(Action::Left, KeyCode::Left) && bound(Action::Right, KeyCode::Right) {
            return Some("←→".to_string());
        }
        Some(pair(&key(Action::Left)?, &key(Action::Right)?))
    };
    let in_stage = app.nav.focus() == crate::nav::Focus::Stage;
    let wanted: Vec<(Action, &'static str)> = match app.nav.current() {
        // Now Playing has its own vocabulary, and it is the one view where the browsing
        // hints would be actively wrong.
        // Skip only while a skip is offered: at any other moment the key does nothing.
        V::NowPlaying => {
            let offered = app.playing.as_ref().is_some_and(|playing| playing.skip.is_some());
            let mut wanted = vec![(Action::PlayPause, Action::PlayPause.hint())];
            if offered {
                wanted.push((Action::SkipOpening, Action::SkipOpening.hint()));
            }
            wanted.push((Action::Detach, Action::Detach.hint()));
            wanted
        }
        // Enter plays in the table; that is the table's whole point. A row from AniList alone
        // has nothing to play it from, and Enter there goes to add a source.
        V::Episodes(_) if !app.go_count.is_empty() || app.go_error.is_some() => {
            // A number is being typed: the key that takes it, drawn beside what it will take,
            // and Esc to let go of it. After one went nowhere, only Esc: the line says why.
            let mut hints = Vec::new();
            if app.go_error.is_none() {
                let go = key(Action::Bottom).unwrap_or_default();
                hints.push((
                    format!("{} {go}", app.go_count),
                    said(t::keys_hint_go_to_episode()),
                ));
            }
            hints.push((ESC.to_string(), said(t::keys_hint_clear())));
            return hints;
        }
        V::Episodes(_) => vec![
            (
                Action::Open,
                if app.selected_unlisted() { hint_add_source() } else { hint_play() },
            ),
            (Action::ToggleWatched, Action::ToggleWatched.hint()),
            (Action::Back, Action::Back.hint()),
        ],
        // Enter on a title page goes one deeper, into its episodes. Space is play next only
        // while nothing plays; then it pauses the player, as on every other screen.
        V::Title(id) => [
            // Another, on the page Surprise me opened, while its pool has one to pick.
            app.surprised_here()
                .filter(|picked| picked.more)
                .map(|_| (Action::Surprise, said(t::keys_hint_another()))),
            Some(if app.playing.is_some() {
                (Action::PlayPause, Action::PlayPause.hint())
            } else if app.unlisted(*id) {
                (Action::PlayNext, hint_add_source())
            } else if app.target().is_some_and(crate::app::watched_all) {
                (Action::PlayNext, said(t::title_button_watch_again()))
            } else {
                (Action::PlayNext, Action::PlayNext.hint())
            }),
            Some((Action::Open, Action::ShowEpisodes.hint())),
            Some((Action::Back, Action::Back.hint())),
        ]
        .into_iter()
        .flatten()
        .collect(),
        V::Stores | V::Store(_) | V::StoreMembers(_) => {
            vec![(Action::Open, Action::Open.hint()), (Action::Back, Action::Back.hint())]
        }
        // Focus is on the rail: the arrows walk it, which is exactly the thing nobody could
        // discover when they did not.
        V::Section(_) if !in_stage => {
            vec![(Action::Down, Action::Down.hint()), (Action::Right, Action::Right.hint())]
        }
        V::Section(Section::Home) if app.content.is_empty() => Vec::new(),
        // On a suggestion `x` is Not interested, the one thing a suggestion offers besides
        // opening; on a Continue row it is named in help.
        V::Section(Section::Home) if app.on_discovery() => {
            let mut hints = vec![
                (Action::HideFromContinue, hint_not_interested()),
                (Action::Open, hint_open()),
            ];
            if app.last_dismissed.is_some() {
                hints.push((Action::ToggleWatched, said(t::keys_hint_undo())));
            }
            hints
        }
        V::Section(Section::Home) => vec![
            // A part-watched row resumes on Enter rather than opening.
            (Action::Open, if app.resumes_on_enter() { hint_play() } else { hint_open() }),
            (Action::ShowEpisodes, Action::ShowEpisodes.hint()),
        ],
        // The segment keys go first so they are dropped first; Enter is what a row is for.
        V::Section(Section::Library) => {
            let mut hints: Vec<(String, &'static str)> = Vec::new();
            hints.extend(sideways().map(|keys| (keys, hint_list())));
            if !app.content.is_empty() {
                // Planning has Surprise me: a title picked from the list.
                if app.library_segment == kaeri_model::catalog::LibrarySegment::Planning {
                    hints.extend(key(Action::Surprise).map(|k| (k, Action::Surprise.hint())));
                }
                hints.extend(key(Action::Open).map(|k| (k, hint_open())));
                hints.extend(
                    key(Action::ShowEpisodes).map(|k| (k, Action::ShowEpisodes.hint())),
                );
            }
            return with_help(keymap, hints);
        }
        // Enter plays a finished download and starts a waiting one now, ahead of the queue.
        // Nothing on this screen is an "open". Space and `x`
        // pause and cancel the highlighted row — the play-next and remove keys while idle, the
        // player's pause and stop while something plays — so they are named by whichever table
        // the keys are read from now, and only where the row can take them.
        V::Section(Section::Downloads) => {
            use kaeri_model::downloads::DownloadState;
            let playing = app.playing.is_some();
            let (pause, cancel) = if playing {
                (Action::PlayPause, Action::StopPlayback)
            } else {
                (Action::PlayNext, Action::HideFromContinue)
            };
            let clear = (Action::ClearCompleted, Action::ClearCompleted.hint());
            match app.downloads.get(app.cursors.downloads.selected).map(|row| row.state) {
                Some(DownloadState::Done) => vec![
                    clear,
                    (Action::DeleteDownload, Action::DeleteDownload.hint()),
                    (Action::Open, hint_play()),
                ],
                Some(DownloadState::Failed) => vec![clear, (cancel, hint_cancel())],
                Some(DownloadState::Paused) => vec![
                    clear,
                    (cancel, hint_cancel()),
                    (pause, hint_resume()),
                    (Action::Open, hint_start_now()),
                ],
                Some(DownloadState::Queued) => vec![
                    clear,
                    (cancel, hint_cancel()),
                    (pause, hint_pause()),
                    (Action::Open, hint_start_now()),
                ],
                Some(_) => vec![clear, (cancel, hint_cancel()), (pause, hint_pause())],
                None => Vec::new(),
            }
        }
        // Enter on Providers leads to the plugin stores; the table itself opens nothing. The
        // order keys are one hint, the way Settings names its pair, offered once there are two
        // sources to order, and first so a narrow line drops them before the way into the stores.
        V::Section(Section::Providers) => {
            let mut hints: Vec<(String, &'static str)> = Vec::new();
            if app.providers.len() > 1
                && let (Some(up), Some(down)) =
                    (key(Action::RaiseSource), key(Action::LowerSource))
            {
                hints.push((pair(&up, &down), hint_order()));
            }
            hints.extend(key(Action::Refresh).map(|k| (k, Action::Refresh.hint())));
            hints.extend(key(Action::Open).map(|k| (k, hint_stores())));
            return with_help(keymap, hints);
        }
        // Together: Enter joins the highlighted room or asks for a link, and on a member's row
        // `M` mutes them instead. The party's own keys are named only while there is a party
        // to use them on. A narrow line drops from the front, so Hide chat goes first and the
        // row's own key and Say last.
        V::Section(Section::Together) => {
            let mut hints = Vec::new();
            if let Some(party) = &app.party {
                hints.push(if app.chat_hidden() {
                    (Action::ToggleSynopsis, said(t::keys_hint_show_chat()))
                } else {
                    (Action::ToggleSynopsis, said(t::keys_hint_hide_chat()))
                });
                hints.push((Action::Refresh, hint_ready()));
                hints.push((Action::ClearCompleted, hint_copy_link()));
                if party.can_start_anyway {
                    hints.push((Action::ShowSources, hint_start_now()));
                }
                hints.push((Action::HideFromContinue, hint_leave_party()));
                if !app.chat_hidden() {
                    hints.push((Action::FixMapping, said(t::keys_hint_say())));
                }
            }
            hints.push(match app.selected_member() {
                Some(member) if app.is_muted(&member.name) => {
                    (Action::MarkAllPrevious, said(t::keys_hint_unmute()))
                }
                Some(_) => (Action::MarkAllPrevious, said(t::keys_hint_mute())),
                None => (Action::Open, hint_open()),
            });
            hints
        }
        // Enter signs the highlighted tracker in or out, so it is named for which.
        V::Section(Section::Accounts) => match app.sync.get(app.cursors.accounts.selected) {
            Some(tracker) if tracker.connected && !tracker.needs_reauth => {
                vec![(Action::Open, hint_sign_out())]
            }
            Some(_) => vec![(Action::Open, hint_sign_in())],
            None => Vec::new(),
        },
        // Settings has nothing to open and no episodes, so offering those keys would be a lie.
        // Left and Right are what actually do something here.
        // `d` leads to Devices, first so a narrow line drops it before the arrows.
        V::Section(Section::Settings) => {
            let mut hints: Vec<_> = key(Action::Download)
                .map(|k| (k, said(t::devices_title())))
                .into_iter()
                .collect();
            hints.extend(sideways().map(|keys| (keys, hint_change())));
            return with_help(keymap, hints);
        }
        // Enter is named for what it does on the row under the cursor.
        V::Devices => {
            use crate::app::{DeviceLine as L, FolderState};
            let enter = match app.device_line() {
                Some(L::Device(_)) => said(t::sync_tui_hint_remove()),
                Some(L::RecoveryKey) => said(t::sync_tui_hint_show()),
                Some(L::TurnOff) => said(t::sync_tui_hint_turn_off()),
                Some(L::SignOut) => hint_sign_out(),
                Some(L::Folder) if app.folder_state() == FolderState::SignedOut => {
                    hint_sign_in()
                }
                Some(L::Folder) => said(t::cloud_sync_now()),
                Some(L::Profile(_)) => hint_use(),
                _ => hint_open(),
            };
            vec![(Action::Open, enter), (Action::Back, Action::Back.hint())]
        }
        // An empty or failed list has no row to open.
        V::Section(_) if app.content.is_empty() => Vec::new(),
        // Search's browse picks at random from its filters, where a search with words opens a
        // row's episodes.
        V::Section(Section::Search) if app.searched_query.is_browse() => vec![
            (Action::Open, Action::Open.hint()),
            (Action::Surprise, Action::Surprise.hint()),
        ],
        V::Section(_) => vec![
            (Action::Open, Action::Open.hint()),
            (Action::ShowEpisodes, Action::ShowEpisodes.hint()),
        ],
    };
    let hints =
        wanted.into_iter().filter_map(|(action, label)| Some((key(action)?, label))).collect();
    with_help(keymap, hints)
}

/// `? keys` last, so it is the hint a narrow status line keeps.
fn with_help(
    keymap: &Keymap,
    mut hints: Vec<(String, &'static str)>,
) -> Vec<(String, &'static str)> {
    if let Some(key) = keymap.keys_for(Action::Help).first() {
        hints.push((key.render(), Action::Help.hint()));
    }
    hints
}

/// An overlay's own hints. Its keys are the fixed ones from [`modal_action`], so they hold
/// whatever `[keys]` says. Esc goes last: it is the one a narrow line must keep.
///
/// [`modal_action`]: crate::keymap::modal_action
fn overlay_hints(
    app: &crate::app::App,
    overlay: &crate::nav::Overlay,
    help_pages: usize,
) -> Vec<(String, &'static str)> {
    use crate::nav::Overlay as O;
    let hint = |key: &str, label: &'static str| (key.to_string(), label);
    match overlay {
        O::Help if help_pages > 1 => vec![hint("↑↓", hint_page()), hint(ESC, hint_close())],
        O::Help => vec![hint(ESC, hint_close())],
        O::InviteLink | O::OpenLink => vec![hint("↵", hint_close())],
        O::FirstRun => vec![
            hint("↵", super::said(kaeri_i18n::t::onboarding_play_demo())),
            hint("i", super::said(kaeri_i18n::t::onboarding_invite_heading())),
            hint("s", super::said(kaeri_i18n::t::onboarding_skip())),
        ],
        O::Medium
            if app.medium_question() == Some(&kaeri_model::medium::MediumAsk::FirstRun) =>
        {
            vec![hint("↑↓", hint_move()), hint("↵", hint_choose())]
        }
        O::Medium => {
            vec![hint("↑↓", hint_move()), hint("↵", hint_choose()), hint(ESC, hint_close())]
        }
        O::MediumOff => {
            vec![hint("↑↓", hint_move()), hint("↵", hint_choose()), hint(ESC, hint_cancel())]
        }
        O::BugReport => vec![
            hint("↑↓", hint_move()),
            hint("↵", super::said(kaeri_i18n::t::report_open_github())),
            hint("C", super::said(kaeri_i18n::t::report_copy())),
            hint("E", super::said(kaeri_i18n::t::settings_crash_delete())),
            hint(ESC, hint_close()),
        ],
        O::Logs if app.logs.is_empty() => vec![hint(ESC, hint_close())],
        O::Logs => vec![hint("↑↓", hint_move()), hint(ESC, hint_close())],
        O::SearchFilters => {
            vec![hint("←→", hint_change()), hint("↵", hint_search()), hint(ESC, hint_keep())]
        }
        O::CommandPalette => {
            vec![hint("↑↓", hint_move()), hint("↵", hint_run()), hint(ESC, hint_close())]
        }
        O::SyncDevice => {
            vec![hint("↑↓", hint_move()), hint("↵", hint_change()), hint(ESC, hint_close())]
        }
        O::SyncWait => vec![hint("↵", hint_open()), hint(ESC, hint_cancel())],
        O::SyncRecoveryKey => vec![
            hint("c", said(t::sync_tui_hint_copy())),
            hint("v", said(t::rkey_qr_short())),
            hint("↵", hint_close()),
        ],
        O::SyncPairShow if app.pair_code_expired() => {
            vec![hint("↵", said(t::pair_show_code())), hint(ESC, hint_close())]
        }
        O::SyncPairShow => vec![hint(ESC, hint_close())],
        O::SyncConfirm => {
            let verb = match &app.devices.confirm {
                Some(crate::app::DeviceConfirm::Remove { .. }) => t::sync_tui_hint_remove(),
                Some(crate::app::DeviceConfirm::NewKey) => t::rkey_new(),
                _ => t::sync_tui_hint_turn_off(),
            };
            vec![hint("↵", said(verb)), hint(ESC, hint_cancel())]
        }
        other => {
            let verb = match other {
                O::ListStatus => hint_set(),
                O::Rate => hint_rate(),
                O::Sources => hint_play(),
                O::SourceProvider => hint_pin(),
                O::SourceRelease => hint_pin(),
                O::Disambiguate => hint_use(),
                O::WatchOrder | O::Recommendations => hint_open(),
                O::Accounts => hint_choose(),
                O::Conflicts => hint_resolve(),
                O::PluginAction => hint_choose(),
                O::PublishPick => hint_publish(),
                O::StoreConfirm => hint_confirm(),
                O::ManualQuery => hint_search(),
                O::DownloadRange => hint_queue(),
                O::EditSetting => hint_save(),
                O::KitsuUsername => hint_next(),
                O::KitsuPassword => hint_sign_in(),
                O::JoinStore => hint_check(),
                O::JoinParty => hint_open(),
                O::Say => said(t::party_chat_send()),
                O::InviteName => hint_make_link(),
                O::SyncAddress
                    if app.devices.kind != Some(kaeri_model::group::SyncFolderKind::WebDav) =>
                {
                    said(t::cloud_connect())
                }
                O::SyncAddress | O::SyncUser | O::SyncPairCode | O::ProfileName => hint_next(),
                O::SyncPassword => hint_sign_in(),
                O::SyncRestore => said(t::rkey_restore()),
                O::SyncPairAddress => said(t::pair_button()),
                _ => hint_choose(),
            };
            let leave = if matches!(other, O::Say) { hint_close() } else { hint_cancel() };
            vec![hint("↵", verb), hint(ESC, leave)]
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn status_hints_stay_short_and_always_offer_help() {
        // The whole point of not having a footer: at most a few hints, never a wall.
        use crate::nav::StageView;
        let mut app = crate::app::App::new(
            kaeri_model::settings::SettingsView::default(),
            crate::theme::Palette::dark(),
            Keymap::new(),
        );
        let check = |app: &crate::app::App, what: &str| {
            let hints = status_hints(app);
            assert!(hints.len() <= 4, "{what} produced {} hints", hints.len());
            assert!(
                hints.last().is_some_and(|(k, _)| k == "?"),
                "{what}: discoverability depends on ? always being offered, last: {hints:?}"
            );
        };
        for section in Section::ALL {
            app.go_to_section(section);
            app.nav.focus_rail();
            check(&app, &format!("{section:?} rail"));
            app.nav.focus_stage();
            // Search's own view is a text field, where `?` would be typed; see the typing test.
            if !app.is_typing() {
                check(&app, &format!("{section:?} stage"));
            }
        }
        for view in [
            StageView::NowPlaying,
            StageView::Episodes(kaeri_core::ids::AnilistId::new(1)),
            StageView::Title(kaeri_core::ids::AnilistId::new(1)),
        ] {
            app.go_to_section(Section::Home);
            app.nav.push(view.clone());
            check(&app, &format!("{view:?}"));
        }
    }

    #[test]
    fn providers_offers_the_order_keys_as_one_hint_once_there_is_something_to_order() {
        let mut app = crate::app::App::new(
            kaeri_model::settings::SettingsView::default(),
            crate::theme::Palette::dark(),
            Keymap::new(),
        );
        app.go_to_section(Section::Providers);
        app.nav.focus_stage();
        let row = |id: &str| kaeri_model::sources::ProviderRow {
            id: id.into(),
            kind: "plugin".into(),
            state: "ready".into(),
            latency_ms: None,
            last_error: None,
            usable: true,
            held_back: false,
            ..Default::default()
        };
        app.apply(kaeri_model::event::Event::Providers(vec![row("alpha")]));
        let hints = status_hints(&app);
        assert!(hints.iter().all(|(_, label)| *label != "Order"), "{hints:?}");

        app.apply(kaeri_model::event::Event::Providers(vec![row("alpha"), row("beta")]));
        let hints = status_hints(&app);
        assert_eq!(
            hints,
            [
                ("K/J".to_string(), "Order"),
                ("r".to_string(), "Refresh"),
                ("↵".to_string(), "Plugin stores"),
                ("?".to_string(), "Keys"),
            ]
        );

        // Named by the live table, so a rebind shows.
        app.keymap.apply_overrides(&BTreeMap::from([("raise_source".into(), "U".into())]));
        assert_eq!(status_hints(&app)[0], ("U/J".to_string(), "Order"));
    }

    #[test]
    fn a_title_page_under_playback_hints_the_keys_the_player_leaves() {
        // Space pauses and `q` detaches while something plays, so "space play next" and
        // "q back" were both wrong there.
        let mut app = crate::app::App::new(
            kaeri_model::settings::SettingsView::default(),
            crate::theme::Palette::dark(),
            Keymap::new(),
        );
        app.nav.push(crate::nav::StageView::Title(kaeri_core::ids::AnilistId::new(1)));
        let hints = status_hints(&app);
        assert!(hints.contains(&("space".into(), "Play next")), "{hints:?}");

        app.playing = Some(crate::app::NowPlaying::default());
        let hints = status_hints(&app);
        assert!(hints.contains(&("space".into(), "Pause")), "{hints:?}");
        assert!(hints.iter().all(|(_, label)| *label != "Play next"), "{hints:?}");
        assert!(hints.contains(&(ESC.into(), "Back")), "{hints:?}");
    }
}
