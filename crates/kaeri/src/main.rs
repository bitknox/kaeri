//! Kaeri — an anime streaming TUI.

mod cli;
mod covers;
mod preview;
mod tui;

use cli::{
    doctor::{doctor, refresh_data},
    library::{export_cli, import_cli, random_cli, stats_cli},
    login::{forget_token, login, migrate_token, print_login_url},
    play::{open_torrent, play_cli, stream_url},
    plugins::{PluginCommand, list_plugins},
    search::search_cli,
    store::store_command,
    sync::sync_once,
};
use preview::preview;
use tui::run::run;

use anyhow::{Context, Result};
use clap::Parser;
use kaeri_core::config::{Config, Paths, ThemeMode};
use kaeri_net::HttpClient;
use kaeri_store::Store;

#[derive(Parser, Debug)]
#[command(
    name = "kaeri",
    version,
    about = "Kaeri in the terminal. It plays episodes from the sources you add and keeps your trackers in step."
)]
// Nothing here means anything to `store` or `plugin`, and `--stats plugin stamp …` ran the stamp
// without a word about the flag it dropped.
#[command(args_conflicts_with_subcommands = true)]
// One command per run. Resolved silently by the order of the checks in `main`, `--stats
// --random` printed the stats and dropped the rest without a word.
#[command(group(
    clap::ArgGroup::new("command").multiple(false).args([
        "doctor", "update", "refresh_data", "preview", "login", "login_url", "logout", "sync",
        "token_to_file", "search", "stats", "export", "import", "random", "stream_url", "play",
        "continue_watching", "plugins",
    ])
))]
// The options that only mean something to some commands. Alone, each used to start the
// interface as if it had not been typed.
#[command(group(clap::ArgGroup::new("json_command").multiple(true).args(["search", "stats", "random"])))]
#[command(group(clap::ArgGroup::new("episode_command").multiple(true).args(["stream_url", "play"])))]
#[command(group(
    clap::ArgGroup::new("tracker_command")
        .multiple(true)
        .args(["login", "login_url", "logout", "token_to_file"])
))]
struct Cli {
    /// Use this theme mode for this run, whatever the config says.
    #[arg(long, value_parser = ["adaptive", "immersive", "light", "dark"])]
    theme: Option<String>,

    /// Show the app in this language (a tag such as `da`), overriding the `language` setting.
    #[arg(long, value_name = "TAG")]
    lang: Option<String>,

    /// Turn off terminal graphics and use text only.
    #[arg(long)]
    no_images: bool,

    /// Report what the terminal supports and exit.
    #[arg(long)]
    doctor: bool,

    /// Download and install the latest release, then exit.
    #[arg(long)]
    update: bool,

    /// With `--update`, take the newest beta as well as the stable releases, for this run. The
    /// `[updates] beta` setting does it every time.
    #[arg(long, requires = "update")]
    beta: bool,

    /// Refresh the mapping datasets and exit.
    #[arg(long)]
    refresh_data: bool,

    /// Render one frame to stdout as text and exit.
    ///
    /// For looking at the layout without an interactive terminal: at a given size, or over a
    /// connection that isn't a tty.
    #[arg(long, value_name = "WIDTHxHEIGHT", value_parser = parse_size)]
    preview: Option<(u16, u16)>,

    /// Which screen `--preview` renders. Home when left out.
    #[arg(long, value_enum, requires = "preview")]
    screen: Option<PreviewScreen>,

    /// Sign in to a tracker and exit.
    ///
    /// With no value, runs the configured flow: `loopback` opens a browser and catches the
    /// token by itself, and `paste` prints the URL and reads the token from stdin. Pass `-` to
    /// read from stdin either way. A token passed directly also works, but then a year-long
    /// account credential sits in your shell history.
    #[arg(long, value_name = "TOKEN", num_args = 0..=1, default_missing_value = "")]
    login: Option<String>,

    /// Which tracker `--login`, `--login-url`, `--logout` and `--token-to-file` are for.
    /// AniList when left out.
    #[arg(long, requires = "tracker_command")]
    tracker: Option<String>,

    /// Print the authorize URL for `--tracker` and exit.
    #[arg(long)]
    login_url: bool,

    /// Forget a tracker's stored token and exit.
    #[arg(long)]
    logout: bool,

    /// Drain the outbox and pull the library once, reporting what happened, then exit.
    ///
    /// In the interface, sync shows only as a badge. This prints what it did.
    #[arg(long)]
    sync: bool,

    /// Move a stored token out of the OS keychain into a `0600` file, then exit.
    ///
    /// It costs one last keychain prompt. macOS ties keychain access to the binary, and every
    /// `cargo build` makes a new one, so "Always Allow" never sticks during development. Set
    /// `trackers.token_storage = "file"` and run this once.
    #[arg(long)]
    token_to_file: bool,

    /// Search AniList and print the results, then exit.
    #[arg(long, value_name = "QUERY")]
    search: Option<String>,

    /// Print watch statistics and exit.
    #[arg(long)]
    stats: bool,

    /// Write a history export to a file, or `-` for stdout, then exit.
    #[arg(long, value_name = "PATH")]
    export: Option<String>,

    /// Merge a history export back in, then exit.
    #[arg(long, value_name = "PATH")]
    import: Option<String>,

    /// Pick something at random from your history and print it, then exit.
    #[arg(long)]
    random: bool,

    /// Emit JSON instead of text: for `--search`, `--stats` and `--random`.
    ///
    /// Pipe it into `jq` or any other tool.
    // Refused by name beside the other commands: `--doctor --json` was told it lacked
    // `--search`, `--stats` or `--random`, as though it had asked for one of them.
    #[arg(long, requires = "json_command", conflicts_with_all = [
        "doctor", "update", "refresh_data", "preview", "login", "login_url", "logout", "sync",
        "token_to_file", "export", "import", "stream_url", "play", "continue_watching", "plugins",
        "open_torrent",
    ])]
    json: bool,

    /// Resolve one episode to a playable URL, print it, and hold it open.
    ///
    /// Point mpv, VLC or curl at the URL by hand to tell a problem in Kaeri from a problem in
    /// the player. A torrent's URL serves only while its session is alive, so this waits for
    /// Ctrl-C instead of exiting.
    #[arg(long, value_name = "ANILIST_ID")]
    stream_url: Option<u32>,

    /// Which episode `--stream-url` and `--play` act on. Accepts non-numeric ids like `OVA`.
    ///
    /// Left unset, `--stream-url` takes episode 1 and `--play` takes the next unwatched.
    #[arg(long, requires = "episode_command")]
    episode: Option<String>,

    /// Play one title in mpv without the interface, add it to your history, then exit.
    ///
    /// It is the same session the app runs, with resume, skips, chapters and tracker updates,
    /// written to stdout instead of drawn. `kaeri --search x --json | jq` finds the id, and this
    /// plays it.
    #[arg(long, value_name = "ANILIST_ID")]
    play: Option<u32>,

    /// Play a magnet link, an info hash or a torrent file you have, in mpv, then exit.
    ///
    /// It goes through the same torrent playback and protection check as a source's torrents.
    #[arg(long, value_name = "LINK_OR_FILE")]
    open_torrent: Option<String>,

    /// Pick up where you left off: resume the most recent episode, or start the next one.
    #[arg(long = "continue")]
    continue_watching: bool,

    /// List installed plugins with what each one is permitted to reach, then exit.
    ///
    /// Plugins are other people's code. This shows what each one may contact, without reading
    /// its source.
    #[arg(long)]
    plugins: bool,

    #[command(subcommand)]
    command: Option<Subcommand>,
}

/// The screens `--preview` can render.
#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
enum PreviewScreen {
    Home,
    Calendar,
    Seasonal,
    Search,
    Library,
    Downloads,
    Providers,
    Accounts,
    Settings,
    /// The first Continue title's page.
    Title,
    /// The first Continue title's episode table.
    Episodes,
    Help,
    Palette,
}

/// `WIDTHxHEIGHT`, refused when it is not one, rather than quietly rendered at 120x34.
fn parse_size(text: &str) -> std::result::Result<(u16, u16), String> {
    let expected = || format!("expected WIDTHxHEIGHT, like 120x34, not {text:?}");
    let (width, height) = text.split_once(['x', 'X']).ok_or_else(expected)?;
    let (width, height): (u16, u16) = (
        width.trim().parse().map_err(|_| expected())?,
        height.trim().parse().map_err(|_| expected())?,
    );
    // Zero rows printed nothing at all; past a thousand a frame is a mistake, not a terminal.
    if !(1..=1000).contains(&width) || !(1..=1000).contains(&height) {
        return Err(format!("{width}x{height} cannot be drawn: each side must be 1 to 1000"));
    }
    Ok((width, height))
}

#[derive(clap::Subcommand, Debug)]
enum Subcommand {
    /// Run a plugin store, or look after one from the command line.
    #[command(subcommand)]
    Store(kaeri_store_server::StoreCommand),
    /// Work with plugin files.
    #[command(subcommand)]
    Plugin(PluginCommand),
    /// Join a watch party: an invite link, or a code, opens its lobby.
    Party {
        /// The invite link or the code a friend sent.
        target: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // macOS hands every process 256 file descriptors — less than one healthy torrent
    // swarm, and the first casualty was mpv failing to *spawn* mid-binge. Ask for a
    // real allowance up front; the OS hard limit still has the final word.
    #[cfg(unix)]
    let _ = rlimit::increase_nofile_limit(8192);

    let args = Cli::parse();
    let paths = Paths::resolve().context("resolving config paths")?;
    // A store server is a long-running service in someone's terminal or a unit file, so it logs
    // where they are looking rather than to the app's log file.
    let mut party = None;
    match args.command {
        Some(Subcommand::Party { target }) => {
            // Read before anything starts, so a mistyped link is said at the prompt.
            party = Some(
                kaeri_model::party::PartyTarget::parse(&target)
                    .with_context(|| "that is not a watch party link or code")?,
            );
        }
        Some(command) => return store_command(command, &paths).await,
        None => {}
    }
    init_logging(&paths);

    // A malformed config must not stop the app from starting, but it must be loud — silently
    // ignoring it could mean silently ignoring a proxy setting. Logged, printed for the commands
    // that print, and kept for the interface, which says so for as long as it runs.
    let mut config_problem = None;
    let mut config = Config::read(&paths).unwrap_or_else(|problem| {
        let file = paths.config_file.display();
        tracing::error!(%file, %problem, "config not loaded; using the defaults");
        eprintln!("warning: {file}, {problem}\nfalling back to defaults");
        config_problem = Some(problem);
        Config::default()
    });
    // The language, once: `--lang` over the config key, both over the system's. KAERI_LANG
    // (for trying a language or the pseudo-locale) still comes first.
    kaeri_i18n::init(args.lang.as_deref().or(config.language.as_deref()));

    let http = HttpClient::new(&config.network).context("building http client")?;
    // A damaged database is moved aside for a new one, and the interface says where it went.
    let (store, set_aside) =
        Store::open_or_set_aside(paths.database()).context("opening the local database")?;
    // An install with profiles enters the one used last, and reads its own settings over the
    // shared file. A single user has none, and nothing changes.
    match kaeri_engine::profile::enter(&store, &paths, None) {
        Ok(Some(_)) if kaeri_engine::profile::overlay_of(&store, &paths).is_some() => {
            match kaeri_engine::profile::read_config(&store, &paths) {
                Ok(profiled) => config = profiled,
                Err(problem) => {
                    tracing::error!(%problem, "the profile's settings did not load");
                    config_problem.get_or_insert(problem);
                }
            }
        }
        Ok(_) => {}
        Err(reason) => tracing::warn!(%reason, "could not enter the profile used last"),
    }
    if let Some(mode) = &args.theme {
        config.theme.mode = match mode.as_str() {
            "immersive" => ThemeMode::Immersive,
            "light" => ThemeMode::Light,
            "dark" => ThemeMode::Dark,
            _ => ThemeMode::Adaptive,
        };
    }

    if args.update {
        return cli::update::self_update(&http, args.beta || config.updates.beta).await;
    }
    if args.doctor {
        return doctor(&config, &paths, &store, !args.no_images).await;
    }
    if args.refresh_data {
        return refresh_data(&store, &http).await;
    }
    let tracker = args.tracker.as_deref().unwrap_or("anilist");
    if args.login_url {
        return print_login_url(&config, tracker);
    }
    if let Some(token) = &args.login {
        return login(&config, &paths, tracker, token, &http).await;
    }
    if args.logout {
        return forget_token(&config, &paths, tracker);
    }
    if args.sync {
        return sync_once(&config, &paths, &store, &http).await;
    }
    if args.token_to_file {
        return migrate_token(&config, &paths, tracker);
    }
    if args.plugins {
        return list_plugins(&config, &paths).await;
    }
    if let Some(id) = args.stream_url {
        let episode = args.episode.as_deref().unwrap_or("1");
        return stream_url(&config, &http, &store, &paths, id, episode).await;
    }
    if let Some(id) = args.play {
        let id = kaeri_core::ids::AnilistId::new(id);
        // No `--episode` means the next unwatched, which is almost always the ask.
        let episode = match &args.episode {
            Some(episode) => episode.clone(),
            None => (store.completed_episode_count(id).unwrap_or(0) + 1).to_string(),
        };
        return play_cli(&config, &http, &store, &paths, id, episode).await;
    }
    if let Some(input) = &args.open_torrent {
        return open_torrent(&config, &http, &store, &paths, input).await;
    }
    if args.continue_watching {
        let Some(top) = store.continue_list(1).unwrap_or_default().into_iter().next() else {
            anyhow::bail!("nothing in your history yet; watch something first");
        };
        // Half-finished resumes; anything else moves on to the episode after the last one
        // that counted. The same reading of history the home screen's CONTINUE rail uses.
        let episode = if top.is_resumable(kaeri_store::RESUME_CEILING) {
            top.last_episode.clone()
        } else if top.episodes_done > 0 {
            (top.episodes_done + 1).to_string()
        } else {
            top.last_episode.clone()
        };
        return play_cli(&config, &http, &store, &paths, top.anilist_id, episode).await;
    }
    if let Some(query) = &args.search {
        return search_cli(&config, &http, &store, query, args.json).await;
    }
    if args.stats {
        return stats_cli(&store, args.json);
    }
    if let Some(path) = &args.export {
        return export_cli(&store, path);
    }
    if let Some(path) = &args.import {
        return import_cli(&store, path);
    }
    if args.random {
        return random_cli(&store, args.json);
    }
    if let Some(size) = args.preview {
        let screen = args.screen.unwrap_or(PreviewScreen::Home);
        return preview(
            config,
            http,
            store,
            &paths,
            size,
            screen,
            !args.no_images,
            config_problem,
        )
        .await;
    }

    run(config, paths, http, store, !args.no_images, config_problem, set_aside, party).await
}

/// What the terminal's graphics support amounts to, in words.
fn describe_graphics(graphics: kaeri_tui::image::Graphics) -> &'static str {
    match graphics {
        kaeri_tui::image::Graphics::Native => "a graphics protocol, so covers are pictures",
        kaeri_tui::image::Graphics::Halfblocks => {
            "no graphics protocol, so covers are drawn in half blocks"
        }
        kaeri_tui::image::Graphics::Disabled => "off (--no-images)",
    }
}

/// Log to a file, never to the terminal.
///
/// Anything written to stdout would corrupt the alternate screen, so diagnostics go to disk
/// and the Logs overlay reads them back.
fn init_logging(paths: &Paths) {
    use tracing_subscriber::{EnvFilter, fmt};

    let dir = paths.cache_dir.join("logs");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let appender = tracing_appender::rolling::daily(&dir, "kaeri.log");
    let _ = fmt()
        .with_writer(appender)
        .with_ansi(false)
        .with_env_filter(
            EnvFilter::try_from_env("KAERI_LOG").unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .try_init();
}

#[cfg(test)]
mod contract;
#[cfg(test)]
mod harness;

#[cfg(test)]
mod tests {
    use super::*;
    use kaeri_engine::{
        data,
        playback::{
            self,
            player::{player, shader_dir},
        },
        settings::setting_label,
        sources,
    };
    use kaeri_model::settings::{SettingKey, SettingsView};
    use kaeri_tui::{
        app::App,
        keymap::Keymap,
        theme::{self, player_theme},
    };

    fn test_app() -> App {
        App::new(SettingsView::default(), theme::Palette::dark(), Keymap::new())
    }

    #[test]
    fn the_player_wears_the_immersive_palette_whatever_the_terminal_is() {
        // What the companion is handed, which mpv's arguments are spelled from word for word:
        // `--script-opts-append=kaeri-<key>=<value>`, and the font as `--osd-font` too.
        let theme: Vec<(&str, &str)> = vec![
            ("text", "#E4E0D6"),
            ("dim", "#9298B5"),
            ("obi", "#F2A64B"),
            ("state", "#7FC7D9"),
            ("alert", "#E06B54"),
            ("ground", "#161A2E"),
            ("font", "Zen Kaku Gothic New"),
        ];
        let given = player_theme().options();
        let given: Vec<(&str, &str)> =
            given.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        assert_eq!(given, theme);
    }

    #[test]
    fn the_player_reflects_the_config_as_it_stands_now() {
        // Shaders are set per file over IPC and never ride the command line, but what does,
        // the HDR curve, must follow the settings screen as it stands now.
        // All three under a temporary directory: filling the rest from `Paths::resolve()`
        // handed the player the real data directory of whoever ran the tests.
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths {
            config_file: dir.path().join("config.toml"),
            data_dir: dir.path().join("data"),
            cache_dir: dir.path().join("cache"),
        };
        let mut config = Config::default();

        config.playback.upscaling = kaeri_core::config::Upscaling::Sharp;
        config.playback.tone_mapping = "bt.2390".into();
        let player_now = player(&config, &paths, &player_theme());
        let args = player_now.args_for_test();
        assert!(!args.iter().any(|a| a.starts_with("--glsl-shaders")), "{args:?}");
        assert!(args.iter().any(|a| a == "--tone-mapping=bt.2390"), "{args:?}");

        config.playback.tone_mapping = "auto".into();
        let args = player(&config, &paths, &player_theme()).args_for_test().to_vec();
        assert!(!args.iter().any(|a| a.starts_with("--tone-mapping")), "auto is mpv's own");

        // A chain of the user's own is theirs: no runtime chain may replace it.
        assert!(shader_dir(&config, &paths).is_some());
        config.playback.mpv_args = vec!["--glsl-shaders=~~/mine.glsl".into()];
        assert!(shader_dir(&config, &paths).is_none());
    }

    #[test]
    fn one_command_per_run_and_no_option_without_its_command() {
        use clap::error::ErrorKind;
        let parse = |args: &[&str]| {
            Cli::try_parse_from(std::iter::once("kaeri").chain(args.iter().copied()))
        };
        let refused = |args: &[&str]| parse(args).map(|_| ()).unwrap_err().kind();

        // Two commands were resolved by the order of the checks in `main`, silently.
        assert_eq!(refused(&["--stats", "--random"]), ErrorKind::ArgumentConflict);
        assert_eq!(refused(&["--doctor", "--plugins"]), ErrorKind::ArgumentConflict);
        assert_eq!(refused(&["--continue", "--play", "1"]), ErrorKind::ArgumentConflict);

        // Alone, these started the interface as if they had not been typed.
        for alone in [
            &["--json"][..],
            &["--screen", "title"],
            &["--episode", "3"],
            &["--tracker", "mal"],
        ] {
            assert_eq!(refused(alone), ErrorKind::MissingRequiredArgument, "{alone:?}");
        }

        // `--json` means nothing to these, and the refusal names the command it was given with.
        for (args, command) in
            [(&["--doctor", "--json"][..], "--doctor"), (&["--plugins", "--json"], "--plugins")]
        {
            let error = parse(args).unwrap_err();
            assert_eq!(error.kind(), ErrorKind::ArgumentConflict, "{args:?}");
            assert!(error.to_string().contains(command), "{error}");
        }

        // A command's flags beside a subcommand were dropped without a word.
        assert_eq!(
            refused(&["--stats", "plugin", "stamp", "a.wasm"]),
            ErrorKind::ArgumentConflict
        );
        assert!(parse(&["plugin", "stamp", "a.wasm"]).is_ok());
        for fine in [
            &["--stats", "--json"][..],
            &["--search", "summer", "--json"],
            &["--preview", "80x24", "--screen", "seasonal"],
            &["--play", "1", "--episode", "OVA"],
            &["--login", "--tracker", "mal"],
            &["--logout", "--tracker", "kitsu"],
            &[],
        ] {
            assert!(parse(fine).is_ok(), "{fine:?}: {:?}", parse(fine).err());
        }
    }

    #[test]
    fn a_preview_must_name_a_size_and_a_screen_that_exist() {
        use clap::error::ErrorKind;
        let parse = |args: &[&str]| {
            Cli::try_parse_from(std::iter::once("kaeri").chain(args.iter().copied()))
        };
        // Each of these rendered Home at 120x34, or nothing, with exit status 0.
        for wrong in [&["--preview", "abc"][..], &["--preview", "0x0"], &["--preview", "120x"]]
        {
            assert_eq!(
                parse(wrong).unwrap_err().kind(),
                ErrorKind::ValueValidation,
                "{wrong:?}"
            );
        }
        assert_eq!(
            parse(&["--preview", "80x24", "--screen", "bogus"]).unwrap_err().kind(),
            ErrorKind::InvalidValue
        );
        let cli = parse(&["--preview", "80X24", "--screen", "episodes"]).unwrap();
        assert_eq!(cli.preview, Some((80, 24)));
        assert_eq!(cli.screen, Some(PreviewScreen::Episodes));
    }

    // The work's words held to the interface's. Only the binary sees both sides, so these live
    // here rather than beside the work they check.

    #[test]
    fn a_proxy_check_reads_on_the_badge_as_the_check_words_it() {
        use kaeri_providers::proxy::ProxyState;

        // Every result a check can reach. Not yet checked is said before any check runs, and
        // does not alert: nothing runs until it passes.
        for state in [ProxyState::Passed, ProxyState::Failed { reason: "refused".into() }] {
            let (badge, alerts) = kaeri_tui::app::proxy_badge(sources::proxy_status(&state))
                .expect("a proxy has a chip");
            // The words are the terminal's own; what must agree is whether it alerts.
            assert!(badge.to_lowercase().starts_with("proxy"), "{state:?}: {badge}");
            assert_eq!(alerts, !state.is_ready(), "{state:?}");
        }
        let checking =
            kaeri_tui::app::proxy_badge(sources::proxy_status(&ProxyState::Unchecked));
        assert!(!checking.expect("a proxy has a chip").1);
        // Connecting directly has no chip at all.
        assert_eq!(
            kaeri_tui::app::proxy_badge(sources::proxy_status(&ProxyState::Direct)),
            None
        );
    }

    #[test]
    fn every_kind_the_filler_index_knows_reads_as_the_index_words_it() {
        for kind in kaeri_meta::filler::EpisodeKind::ALL {
            let shown = kaeri_tui::screens::episode_kind_label(data::row_kind(kind));
            assert!(shown.eq_ignore_ascii_case(kind.label()), "{kind:?}: {shown}");
        }
        assert_eq!(
            kaeri_tui::screens::episode_kind_label(kaeri_model::episodes::EpisodeKind::Filler),
            "Filler"
        );
    }

    #[test]
    fn a_saved_setting_is_named_as_the_settings_screen_names_it() {
        let app = test_app();
        // Every row that writes, and the order, which Providers writes.
        let mut written: Vec<(&[&str], &str)> = app
            .setting_rows()
            .iter()
            .filter_map(|row| row.editable.then(|| row.key.path()).flatten())
            .collect();
        written.push((&["providers"], "order"));
        for (table, key) in &written {
            let named = setting_label(table, key);
            assert!(named.is_some(), "{table:?}.{key} is saved without a word");
        }
        // And nothing named that the interface does not write.
        for setting in SettingKey::ROWS.into_iter().chain([SettingKey::SourceOrder]) {
            if let Some((table, key)) = setting.path() {
                assert!(written.contains(&(table, key)), "{table:?}.{key} has no row");
            }
        }
        // Saved in passing, from the player.
        for key in ["persisted_speed", "persisted_volume"] {
            assert_eq!(setting_label(&["playback"], key), None, "{key}");
        }
    }

    #[test]
    fn a_skip_offered_in_the_terminal_reads_as_the_player_words_it() {
        use kaeri_player::SkipKind;

        for kind in SkipKind::ALL {
            let prompt = kaeri_tui::screens::skip_prompt(playback::offered(kind), "S");
            assert!(prompt.to_lowercase().contains(kind.label()), "{prompt}");
        }
    }
}
