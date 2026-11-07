//! The screens: one module each, and what the window shows for destinations not built yet.

pub mod accounts;
pub mod calendar;
pub mod demo;
pub mod discussion;
pub mod downloads;
pub mod first_launch;
pub mod first_party;
pub mod folders;
pub mod home_page;
pub mod join;
pub mod library;
pub mod licensed;
pub mod lobby;
pub mod match_picker;
pub mod more_about;
pub mod opened;
pub mod plugin;
pub mod providers;
pub mod report;
pub mod search;
pub mod search_filters;
pub mod seasonal;
pub mod servers;
pub mod settings;
pub mod settings_preview;
pub mod show_settings;
pub mod source_picker;
pub mod store_owner;
pub mod stores;
pub mod title;
pub mod together;
pub mod torrents;
pub mod trending;

use std::rc::Rc;

use adw::prelude::*;
use kaeri_i18n::t;

use crate::{
    prefs::CloseChoice,
    window::{Ctx, Shell},
};

/// The sidebar's destinations, in its order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dest {
    Home,
    Calendar,
    Seasonal,
    Search,
    Library,
    Downloads,
    Together,
    Providers,
    Stores,
    Settings,
    /// Reading's Home.
    Read,
    /// New chapters from the series being followed.
    Updates,
    /// Search, Library, Downloads and Your sources with Reading chosen: not rows of the
    /// sidebar, the other side of the row that shares their name.
    ReadSearch,
    ReadLibrary,
    ReadDownloads,
    ReadSources,
}

thread_local! {
    /// What Kaeri is for, as the window last heard: the names of Home and Read follow it.
    static ANSWER: std::cell::Cell<kaeri_model::medium::KaeriIsFor> =
        const { std::cell::Cell::new(kaeri_model::medium::KaeriIsFor::Watch) };
}

/// What Kaeri is for on this device, as last said.
pub fn answer() -> kaeri_model::medium::KaeriIsFor {
    ANSWER.with(std::cell::Cell::get)
}

pub fn set_answer(answer: kaeri_model::medium::KaeriIsFor) {
    ANSWER.with(|a| a.set(answer));
}

impl Dest {
    /// The destinations watching has, in the sidebar's order.
    pub const ALL: [Self; 10] = [
        Self::Home,
        Self::Calendar,
        Self::Seasonal,
        Self::Search,
        Self::Library,
        Self::Downloads,
        Self::Together,
        Self::Providers,
        Self::Stores,
        Self::Settings,
    ];

    /// The ones reading adds.
    pub const READING: [Self; 6] = [
        Self::Read,
        Self::Updates,
        Self::ReadSearch,
        Self::ReadLibrary,
        Self::ReadDownloads,
        Self::ReadSources,
    ];

    /// The sidebar's rows for `answer`, in order: watching alone is today's list; reading
    /// alone has Home, Updates, Search, Library and Downloads; both puts Watch with Calendar
    /// and Seasonal, then Read with Updates, then the shared places.
    pub fn rows(answer: kaeri_model::medium::KaeriIsFor) -> Vec<Self> {
        use kaeri_model::medium::KaeriIsFor::{Both, Read, Watch};
        let mut rows = match answer {
            Watch => vec![
                Self::Home,
                Self::Calendar,
                Self::Seasonal,
                Self::Search,
                Self::Library,
                Self::Downloads,
                Self::Together,
            ],
            Read => {
                vec![Self::Read, Self::Updates, Self::Search, Self::Library, Self::Downloads]
            }
            Both => vec![
                Self::Home,
                Self::Calendar,
                Self::Seasonal,
                Self::Read,
                Self::Updates,
                Self::Search,
                Self::Library,
                Self::Downloads,
                Self::Together,
            ],
        };
        rows.extend([Self::Providers, Self::Stores, Self::Settings]);
        rows
    }

    /// The destination that shares this row's name with the other medium, Reading's side.
    pub fn reading_side(self) -> Option<Self> {
        match self {
            Self::Search => Some(Self::ReadSearch),
            Self::Library => Some(Self::ReadLibrary),
            Self::Downloads => Some(Self::ReadDownloads),
            Self::Providers => Some(Self::ReadSources),
            _ => None,
        }
    }

    /// The row of the sidebar this destination sits under.
    pub fn row(self) -> Self {
        match self {
            Self::ReadSearch => Self::Search,
            Self::ReadLibrary => Self::Library,
            Self::ReadDownloads => Self::Downloads,
            Self::ReadSources => Self::Providers,
            other => other,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Calendar => "calendar",
            Self::Seasonal => "seasonal",
            Self::Search => "search",
            Self::Library => "library",
            Self::Downloads => "downloads",
            Self::Together => "together",
            Self::Providers => "providers",
            Self::Stores => "stores",
            Self::Settings => "settings",
            Self::Read => "read",
            Self::Updates => "updates",
            Self::ReadSearch => "read-search",
            Self::ReadLibrary => "read-library",
            Self::ReadDownloads => "read-downloads",
            Self::ReadSources => "read-sources",
        }
    }

    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().chain(Self::READING).find(|d| d.name() == name)
    }

    pub fn title(self) -> String {
        use kaeri_model::medium::KaeriIsFor::Both;
        match self {
            Self::Home if answer() == Both => t::nav_watch(),
            Self::Home => t::home_title(),
            Self::Read if answer() == Both => t::nav_read(),
            Self::Read => t::home_title(),
            Self::Updates => t::nav_updates(),
            Self::Calendar => t::nav_calendar(),
            Self::Seasonal => t::nav_seasonal(),
            Self::Search | Self::ReadSearch => t::nav_search(),
            Self::Library | Self::ReadLibrary => t::nav_library(),
            Self::Downloads | Self::ReadDownloads => t::nav_downloads(),
            Self::Together => t::nav_watch_together(),
            Self::Providers | Self::ReadSources => t::sources_title(),
            Self::Stores => t::nav_plugin_stores(),
            Self::Settings => t::nav_settings(),
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::Home => "go-home-symbolic",
            Self::Calendar => "x-office-calendar-symbolic",
            Self::Seasonal => "view-grid-symbolic",
            Self::Search | Self::ReadSearch => "system-search-symbolic",
            Self::Library | Self::ReadLibrary => "view-list-symbolic",
            Self::Downloads | Self::ReadDownloads => "folder-download-symbolic",
            Self::Together => "system-users-symbolic",
            Self::Providers | Self::ReadSources => "network-server-symbolic",
            Self::Stores => "application-x-addon-symbolic",
            Self::Settings => "emblem-system-symbolic",
            Self::Read => "accessories-dictionary-symbolic",
            Self::Updates => "preferences-system-notifications-symbolic",
        }
    }
}

/// The keyboard shortcuts, as rows: Ctrl+? opens them.
pub fn shortcuts(ctx: &Ctx) {
    // One at a time: asking again with it up leaves it where it is.
    if ctx.window.visible_dialog().is_some_and(|d| d.widget_name() == "shortcuts") {
        return;
    }
    let page = adw::PreferencesPage::new();
    let groups: [(String, Vec<(String, &str)>); 3] = [
        (
            t::settings_group_general(),
            vec![
                (t::shortcuts_go_to_section(), "<Control>1...<Control>7"), // i18n-skip
                (t::nav_search(), "<Control>f"),                           // i18n-skip
                (t::nav_settings(), "<Control>comma"),                     // i18n-skip
                (t::accounts_sync_now(), "<Control>r"),                    // i18n-skip
                (t::action_back(), "<Alt>Left"),                           // i18n-skip
                (t::shortcuts_show(), "<Control>question"),                // i18n-skip
                (t::shortcuts_close_window(), "<Control>w"),               // i18n-skip
                (t::shortcuts_quit(), "<Control>q"),                       // i18n-skip
            ],
        ),
        (
            t::shortcuts_group_lists(),
            vec![
                (t::shortcuts_play_selected(), "Return"), // i18n-skip
                (t::episode_mark_watched(), "<Control><Shift>m"), // i18n-skip
                (t::continue_hide(), "Delete"),           // i18n-skip
                (t::episode_go_to(), "<Control>g"),       // i18n-skip
                (
                    t::shortcuts_episode_ranges(),
                    "<Control><Shift>bracketleft <Control><Shift>bracketright", // i18n-skip
                ),
                (t::shortcuts_context_menu(), "<Shift>F10"), // i18n-skip
            ],
        ),
        (
            t::settings_group_player(),
            vec![
                (t::shortcuts_play_pause(), "space"),  // i18n-skip
                (t::shortcuts_seek_5(), "Left Right"), // i18n-skip
                (t::shortcuts_seek_30(), "<Shift>Left <Shift>Right"), // i18n-skip
                (t::shortcuts_seek_10(), "j l"),       // i18n-skip
                (t::shortcuts_volume(), "Up Down"),    // i18n-skip
                (t::shortcuts_mute(), "m"),            // i18n-skip
                (t::player_fullscreen(), "f"),         // i18n-skip
                (t::player_mini_player(), "p"),        // i18n-skip
                (t::shortcuts_skip(), "s"),            // i18n-skip
                (t::shortcuts_undo_skip(), "u <Control>z"), // i18n-skip
                (t::player_subtitles_and_audio(), "t"), // i18n-skip
                (t::shortcuts_next_previous(), "n <Shift>n"), // i18n-skip
                (t::shortcuts_speed(), "bracketright bracketleft"), // i18n-skip
                (t::player_speed_normal(), "BackSpace"), // i18n-skip
                (t::player_episodes(), "e"),           // i18n-skip
                (t::player_stats(), "i"),              // i18n-skip
                (t::shortcuts_answer_card(), "Return"), // i18n-skip
                (t::shortcuts_escape(), "Escape"),     // i18n-skip
            ],
        ),
    ];
    for (title, rows) in groups {
        let group = adw::PreferencesGroup::builder().title(title).build();
        for (name, accel) in rows {
            let row = adw::ActionRow::builder().title(name).build();
            let label = gtk::ShortcutLabel::new(accel);
            label.set_valign(gtk::Align::Center);
            row.add_suffix(&label);
            group.add(&row);
        }
        page.add(&group);
    }
    let view = adw::ToolbarView::new();
    view.add_top_bar(&adw::HeaderBar::new());
    view.set_content(Some(&page));
    let dialog = adw::Dialog::builder()
        .name("shortcuts") // i18n-skip
        .title(t::shortcuts_title())
        .content_width(520)
        .content_height(640)
        .child(&view)
        .build();
    crate::widgets::present_light_dismiss(&dialog, &ctx.window);
}

/// Closing the window while episodes download or seed: keep them going, stop and close, or
/// cancel.
pub fn close_dialog(shell: &Rc<Shell>, downloading: usize, seeding: usize) {
    let body = running_body(downloading, seeding);
    let dialog = adw::AlertDialog::new(Some(&t::downloads_background_question()), Some(&body));
    dialog.add_responses(&[
        ("cancel", &t::action_cancel()),
        ("stop", &t::close_stop_and_close()),
        ("keep", &t::close_keep_going()),
    ]);
    dialog.set_response_appearance("keep", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("keep"));
    dialog.set_close_response("cancel");
    let again = gtk::CheckButton::with_label(&t::close_dont_ask());
    dialog.set_extra_child(Some(&again));
    let weak = Rc::downgrade(shell);
    dialog.connect_response(None, move |_, response| {
        let Some(shell) = weak.upgrade() else { return };
        let remember = |choice| {
            if again.is_active() {
                shell.ctx.prefs.set_close_choice(choice);
            }
        };
        match response {
            "keep" => {
                remember(CloseChoice::KeepGoing);
                shell.keep_going();
            }
            "stop" => {
                remember(CloseChoice::Stop);
                shell.stop_and_close();
            }
            _ => {}
        }
    });
    dialog.present(Some(&shell.ctx.window));
}

/// "Episodes: 2 downloading or waiting and 1 seeding. Keep them going after the window closes, or stop
/// them?"
pub fn running_body(downloading: usize, seeding: usize) -> String {
    let count = |n: usize| i64::try_from(n).unwrap_or(i64::MAX);
    match (downloading, seeding) {
        (d, 0) => t::close_body_downloading(count(d)),
        (0, s) => t::close_body_seeding(count(s)),
        (d, s) => t::close_body_both(count(d), count(s)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_close_question_counts_what_runs() {
        assert_eq!(
            running_body(2, 1),
            "Episodes: 2 downloading or waiting and 1 seeding. Keep them going after the window closes, \
             or stop them?"
        );
        assert_eq!(
            running_body(1, 0),
            "One episode is downloading or waiting to. Keep it going after the window closes, or stop it?"
        );
    }

    #[test]
    fn destinations_are_found_by_name() {
        for dest in Dest::ALL {
            assert_eq!(Dest::named(dest.name()), Some(dest));
        }
    }
}
