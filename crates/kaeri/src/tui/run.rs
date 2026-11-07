//! The interface's event loop: the terminal made ready, the engine started, and every event,
//! key, signal and tick handled until the app quits and the terminal is given back.

use std::{io, sync::Arc, time::Duration};

use anyhow::{Context, Result};
use crossterm::{
    event::{Event as TermEvent, EventStream, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, enable_raw_mode},
};
use futures::StreamExt;
use kaeri_core::config::{Config, Paths};
use kaeri_engine::{engine::Engine, host::DesktopHost, options::EngineOptions};
use kaeri_i18n::t;
use kaeri_model::{command::Command, event::Event};
use kaeri_net::HttpClient;
use kaeri_store::Store;
use kaeri_tui::{
    app::{App, FrameReport},
    keymap::Keymap,
    screens,
    theme::{self, player_theme},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use tokio::sync::mpsc;

use crate::{
    covers,
    tui::{self, clipboard::copy_to_clipboard},
};

/// How long the screen waits on the engine's word before the loop draws and reads keys again
/// regardless: a mark made here, or the staging of the episode an ending rolled on to. The
/// engine writes a mark inline and answers at once, and stages a roll-on with its ending; this
/// only bounds a wait on an engine that has stopped answering.
const ENGINE_WAIT: Duration = Duration::from_millis(250);

#[allow(clippy::too_many_arguments)]
pub async fn run(
    config: Config,
    paths: Paths,
    http: HttpClient,
    store: Store,
    images: bool,
    config_problem: Option<kaeri_core::config::ConfigProblem>,
    set_aside: Option<std::path::PathBuf>,
    party: Option<kaeri_model::party::PartyTarget>,
) -> Result<()> {
    // The colour depth first: every colour the palette hands out is drawn at it.
    theme::set_depth(theme::detect_depth(|key| std::env::var(key).ok()));
    let palette = theme::resolve(config.theme.mode);

    let mut keymap = Keymap::new();
    let keybinding_problems = keymap.apply_overrides(&config.keys);
    for problem in &keybinding_problems {
        eprintln!("warning: keybinding: {problem}");
    }

    let image_engine =
        kaeri_tui::image::ImageEngine::detect(images).with_cache_dir(paths.image_cache());
    tracing::info!(graphics = ?image_engine.graphics(), "image engine ready");

    // What the file says, as distinct from what is in effect: `--theme` changes only this
    // session, and the Settings screen has to be able to tell the two apart.
    // A profile's own file is read over the shared one.
    let saved = match kaeri_engine::profile::overlay_of(&store, &paths) {
        Some(overlay) => kaeri_core::profile::load_with_overlay(&paths, &overlay),
        None => Config::load(&paths),
    }
    .unwrap_or_default();

    // The engine, started as the app launches it: its source chain built behind the interface,
    // and its own work beside the commands. The player is dressed in the immersive palette
    // whatever the terminal is: see `player_theme`. It owns the config from here on.
    let mut options =
        EngineOptions::new(paths.clone(), config.clone(), http, store, player_theme());
    options.saved = Some(saved.clone());
    options.set_aside = set_aside;
    // `syncplay.enabled` sends every play to the party: the terminal's way in, which the apps,
    // starting parties when asked, do without.
    options.every_play_a_party = true;
    // Reading ships dark; `reading.dev` turns it on for development.
    options.reading = config.reading.dev;
    let host = Arc::new(DesktopHost::new(&config, &paths));
    let (engine, mut from_engine) = Engine::start(options, host).await?;
    // The terminal has no network monitor, and a computer is rarely on a metered link, so it
    // says Wi-Fi once at launch. Until some front end says the network, work held to Wi-Fi
    // waits.
    engine.send(Command::NetworkPath { cellular: false, constrained: false, offline: false });

    // The settings as the engine starts on them, which it says again at launch.
    let settings = kaeri_model::settings::view(&config, &saved);
    let mut app = App::with_images(settings, palette, keymap, image_engine);
    // What sync names this computer when it starts without a name in the group.
    app.set_device_name(computer_name());
    // The crash reports saved here, for Report a bug to offer. Saving is off unless asked for,
    // so this is usually empty.
    if let Some(store) = kaeri_engine::crash::armed_store() {
        app.crashes = store
            .list(kaeri_engine::crash::now_ms())
            .iter()
            .filter_map(|summary| store.get(&summary.id))
            .collect();
    }
    // Said inside the app too: the warning above scrolls away behind the alternate screen
    // before anyone reads it, and a refused rebind otherwise looks like a key that does nothing.
    for problem in keybinding_problems {
        tracing::warn!(%problem, "keybinding not applied");
        app.push_toast(kaeri_tui::app::alert(t::toast_keybinding_not_applied(&problem)));
    }

    if let Some(problem) = &config_problem {
        show_config_problem(&mut app, &paths, problem);
    }

    // Both before the terminal changes state, so there is no moment in which a signal or a
    // panic could leave it raw.
    tui::terminal::install_panic_hook();
    let mut signals = tui::terminal::Signals::new().context("listening for signals")?;
    let mut terminal = setup_terminal().context("preparing the terminal")?;

    if let Some(task) = app.reload() {
        engine.send(task);
    }
    // `kaeri party <link or code>`: the Together section, with the lobby opening in it.
    if let Some(target) = party {
        if let Some(task) = app.go_to_section(kaeri_tui::nav::Section::Together) {
            engine.send(task);
        }
        engine.send(kaeri_model::command::Command::Party(
            kaeri_model::party::PartyCommand::Lobby {
                server: target.server,
                room: target.room,
                name: None,
                pick: None,
            },
        ));
    }

    // What was true at launch, applied before the first frame so that it shows the app as it
    // was launched: the joined stores, the trackers' states, and what the check of the player
    // found, which the first frame has always waited for. Nothing the engine has said since
    // is in it; that is read in the loop, after the first frame, as it always was.
    for update in from_engine.launch().await {
        app.apply(update);
    }

    let mut events = EventStream::new();
    let mut ticker = tokio::time::interval(Duration::from_millis(100));
    // Covers come back from the blocking pool on this, decoded, to be handed to the app.
    let (covers_tx, mut covers_rx) = mpsc::unbounded_channel();

    // Painting on change rather than on every tick: see `Repaint`. `idle_tick` is whether
    // nothing but the ticker woke this pass, the one case that may skip it.
    let mut repaint = tui::terminal::Repaint::default();
    let mut idle_tick = false;
    // Until when the screen waits on the engine's word: see `ENGINE_WAIT`.
    let mut engine_due: Option<std::time::Instant> = None;
    let result = loop {
        // A link the screen asked to copy with no event to carry it, as the store's open link
        // is: handed over before the frame that says whether it went.
        if let Some(text) = app.take_clipboard() {
            let went = copy_to_clipboard(terminal.backend_mut(), &text);
            app.clipboard_went(went);
        }
        for id in app.take_deleted_crashes() {
            kaeri_engine::crash::delete_saved(&id);
        }
        let now = std::time::Instant::now();
        // A mark is the engine's to make: the table shows it once the engine has written it
        // and said where the title stands, which it does at once. Until then the next frame
        // and the next key wait, so the frame after `u` shows the mark, as when the interface
        // marked its own rows, and a second `u` reads the first rather than a row not yet
        // marked. An ending that rolls on is the same: the next episode is staged right after
        // it, and the frame after the ending shows it, as when the interface staged it itself.
        let waiting = app.awaiting_engine() && engine_due.is_some_and(|by| now < by);
        // Work on screen keeps its clocks and its pulse moving, the eyecatch its wipe.
        let live = !app.jobs.is_empty() || app.is_animating() || app.is_ticking();
        if !waiting && (!idle_tick || repaint.due(live, now)) {
            // Broken out of rather than returned from: a terminal that can no longer be drawn
            // on still has to be given back, and the player closed.
            let mut report = FrameReport::default();
            if let Err(e) = terminal.draw(|frame| report = screens::render(frame, &app)) {
                break Err(e.into());
            }
            // How the frame laid the overlay out, for the next key to page and scroll by.
            app.took_frame(report);
            repaint.painted(terminal.backend_mut().take_wrote(), now);
        }
        idle_tick = false;
        if app.should_quit {
            break Ok(());
        }

        let size = match terminal.size() {
            Ok(size) => size,
            Err(e) => break Err(e.into()),
        };
        let rows = screens::visible_rows(size.into(), app.nav.section());

        // Prefetch only what is on screen plus a short lookahead. `visible_artwork` claims
        // each URL, so a re-render cannot re-request one already in flight.
        for url in app.visible_artwork(rows) {
            engine.send(Command::FetchArtwork { url });
        }

        // What the viewer asked for without a key to carry it: a scroll's backfill, a
        // section's reload once the rail has settled. All of it, in the order it was asked.
        for task in app.take_commands() {
            engine.send(task);
        }

        tokio::select! {
            // One at a time, each in the order the engine applied it, with the loop's own
            // reactions and the next frame between them, as when the loop read the work itself.
            update = from_engine.recv() => {
                // The engine stops on its own only by panicking: it is shut down after this
                // loop. A panic in its work used to take the loop down with it, and the app
                // still ends rather than going on with nothing to answer it.
                let Some(update) = update else {
                    break Err(anyhow::anyhow!("the engine stopped; the log says why"));
                };
                // Handed to the terminal's clipboard on the way in, so the overlay can say
                // whether it was.
                let copied = match &update {
                    Event::InviteLink(view) => {
                        copy_to_clipboard(terminal.backend_mut(), &view.link.0)
                    }
                    _ => false,
                };
                // An ending that rolls on has the next episode staged right after it.
                let (ended, rolling_on) = match &update {
                    Event::PlaybackEnded { rolling_on, .. } => (true, *rolling_on),
                    _ => (false, false),
                };
                if rolling_on {
                    engine_due = Some(std::time::Instant::now() + ENGINE_WAIT);
                }
                // A pairing code is shown with the address the other device reaches it at.
                if matches!(update, Event::SyncInvite { .. }) {
                    app.set_lan_address(lan_address());
                }
                match update {
                    // Decoded off the loop; it comes back on the arm below.
                    Event::ArtworkReady { url, bytes } => covers::spawn(url, bytes, &covers_tx),
                    update => app.apply(update),
                }
                if copied {
                    app.invite_copied = true;
                }
                // mpv's window is gone; unless the ending rolled on — the next episode plays
                // in the window it left — the viewer's next keypress belongs to the terminal,
                // so ask for focus back rather than leaving it wherever the window manager
                // dropped it.
                if ended && !rolling_on && app.playing.is_none() {
                    tui::focus::refocus_terminal();
                }
            }
            Some(decoded) = covers_rx.recv() => covers::hand_over(&mut app, decoded),
            // Asked to end from outside — a kill, the terminal closing — which leaves the way
            // Q does, so the player is closed rather than orphaned and playing on.
            signal = signals.recv() => {
                tracing::info!(signal, "shutting down");
                tui::terminal::exit_on_second_signal();
                if let Some(task) = app.handle(kaeri_tui::keymap::Action::Quit, rows) {
                    engine.send(task);
                }
                // Whatever an open overlay made of the key, a signal is not a request to close it.
                app.should_quit = true;
            }
            _ = ticker.tick() => {
                // A toast going is a change; one counting down is not.
                let toasts = app.toasts.len();
                app.tick_toasts();
                idle_tick = app.toasts.len() == toasts;
            }
            // The eyecatch is the only thing in this app that animates, so the fast tick only
            // exists while one is running rather than burning a frame budget all the time.
            _ = tokio::time::sleep(Duration::from_millis(kaeri_tui::eyecatch::FRAME_MS)),
                if app.is_animating() =>
            {
                app.tick_animation();
            }
            maybe_event = events.next(), if !waiting => {
                match maybe_event {
                    Some(Ok(TermEvent::Key(key))) if key.kind == KeyEventKind::Press => {
                        if let Some(task) = app.press(key, rows) {
                            if matches!(task, Command::SetWatched { .. }) {
                                engine_due = Some(std::time::Instant::now() + ENGINE_WAIT);
                            }
                            engine.send(task);
                        }
                    }
                    // Arrives whole with bracketed paste on, so a pasted link cannot fire
                    // bindings on its way into a field.
                    Some(Ok(TermEvent::Paste(text))) => app.paste(&text),
                    // The next frame redraws at the new size on its own; the list's window does
                    // not follow by itself, and a shorter terminal would otherwise leave the
                    // highlighted row below the bottom edge.
                    Some(Ok(TermEvent::Resize(width, height))) => {
                        let area = ratatui::layout::Rect::new(0, 0, width, height);
                        app.fit_to(screens::visible_rows(area, app.nav.section()));
                    }
                    Some(Err(e)) => break Err(anyhow::anyhow!(e)),
                    None => break Ok(()),
                    _ => {}
                }
            }
        }
    };

    // A playback that has started, as opposed to one still finding its source, is stopped and
    // given a moment to record where it was left: the engine knows which.
    engine.shutdown().await;
    restore_terminal(&mut terminal)?;
    // Another profile was chosen: the terminal starts again on it, as the apps restart their
    // engine, so nothing of the profile left is kept in memory.
    if let Some(name) = app.profile_restart() {
        result?;
        start_again(name);
        return Ok(());
    }
    result
}

/// Start this program again in place, with the arguments it was started with. Where it cannot,
/// it says how to.
fn start_again(name: &str) {
    #[cfg(unix)]
    if let Ok(program) = std::env::current_exe() {
        use std::os::unix::process::CommandExt;
        let problem =
            std::process::Command::new(program).args(std::env::args_os().skip(1)).exec();
        tracing::warn!(%problem, "could not start again on the profile chosen");
    }
    println!("{}", t::sync_tui_profile_restart(name));
}

/// What this computer is called, as its owner named it: the Mac's computer name, else the host
/// name. Sync starts with it, and it can be changed on another device.
fn computer_name() -> String {
    #[cfg(target_os = "macos")]
    if let Ok(out) =
        std::process::Command::new("scutil").args(["--get", "ComputerName"]).output()
        && out.status.success()
        && let Ok(name) = String::from_utf8(out.stdout)
        && !name.trim().is_empty()
    {
        return name.trim().to_owned();
    }
    ["/proc/sys/kernel/hostname", "/etc/hostname"]
        .iter()
        .filter_map(|file| std::fs::read_to_string(file).ok())
        .map(|name| name.trim().to_owned())
        .find(|name| !name.is_empty())
        .unwrap_or_else(|| kaeri_model::stores::COMPUTER.to_owned())
}

/// This computer's address on the local network: the one its default route leaves from. A UDP
/// connect only picks the route; nothing is sent.
fn lan_address() -> Option<String> {
    let socket = std::net::UdpSocket::bind(("0.0.0.0", 0)).ok()?;
    socket.connect(("192.0.2.1", 9)).ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (!ip.is_unspecified() && !ip.is_loopback()).then(|| ip.to_string())
}

/// Say that the config did not load, on the status line for as long as the session runs and in
/// the log. The interface and its previews both, so a preview shows the frame a person would see.
pub fn show_config_problem(
    app: &mut App,
    paths: &Paths,
    problem: &kaeri_core::config::ConfigProblem,
) {
    app.notice = Some(config_notice(&paths.config_file, problem));
    app.log(
        kaeri_model::notice::NoticeKind::Alert,
        t::tui_config_problem_logged(
            &paths.config_file.display().to_string(),
            &problem.to_string(),
        ),
    );
}

/// The interface's standing notice for a config file that did not load: which file, which line,
/// what is wrong there, and what that means for the session.
///
/// One line, so the parser's list of every key it would have accepted is left to the log.
fn config_notice(
    file: &std::path::Path,
    problem: &kaeri_core::config::ConfigProblem,
) -> String {
    let name = file
        .file_name()
        .map_or_else(|| file.display().to_string(), |n| n.to_string_lossy().into_owned());
    let what = problem.message.split(", expected").next().unwrap_or(&problem.message);
    match problem.line {
        Some(line) => t::tui_config_problem_at_line(&name, line as i64, what),
        None => t::tui_config_problem(&name, what),
    }
}

/// The terminal the interface draws on.
type Tui = Terminal<tui::terminal::Quiet<CrosstermBackend<io::Stdout>>>;

fn setup_terminal() -> Result<Tui> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    // Best effort: the legacy Windows console has no bracketed paste and says so with an error,
    // and a paste arriving as keystrokes is a lesser thing than the app not starting.
    let _ = execute!(stdout, crossterm::event::EnableBracketedPaste);
    let terminal = Terminal::new(tui::terminal::Quiet::new(CrosstermBackend::new(stdout)))?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Tui) -> Result<()> {
    Ok(tui::terminal::restore(terminal.backend_mut())?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_broken_config_is_named_by_file_and_line_on_one_line() {
        let file = std::path::Path::new("/home/someone/.config/kaeri/config.toml");
        let problem = Config::parse("[providers.torrent]\nenabled = 1\n").unwrap_err();
        let notice = config_notice(file, &problem);
        assert!(notice.starts_with("config.toml line 2: invalid type"), "{notice}");
        assert!(notice.ends_with(". Using the defaults until it is fixed."), "{notice}");
        let rule = Config::parse("[playback]\ncommit_threshold = 3.0\n").unwrap_err();
        let notice = config_notice(file, &rule);
        assert!(notice.starts_with("config.toml: playback.commit_threshold"), "{notice}");
    }
}
