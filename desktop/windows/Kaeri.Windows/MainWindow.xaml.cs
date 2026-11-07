using System.Globalization;
using System.Runtime.InteropServices;
using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Kaeri.Windows.Pages;
using Kaeri.Windows.Player;
using Microsoft.UI;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Input;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Animation;
using Microsoft.UI.Xaml.Navigation;
using Windows.Graphics;
using VirtualKey = Windows.System.VirtualKey;
using VirtualKeyModifiers = Windows.System.VirtualKeyModifiers;

namespace Kaeri.Windows;

/// <summary>
/// The one window: the title bar with search, the navigation pane, the page, and the player over
/// them. It starts the engine (or, in the testing build, one of its fixture scenes on a scratch
/// root), the player's core, and the loop that brings the engine's events to the pages.
/// </summary>
public sealed partial class MainWindow : Window
{
    static readonly Dictionary<string, Type> Destinations = new()
    {
        ["home"] = typeof(HomePage),
        ["continue"] = typeof(HomePage),
        ["calendar"] = typeof(CalendarPage),
        ["seasonal"] = typeof(SeasonalPage),
        ["library"] = typeof(LibraryPage),
        ["downloads"] = typeof(DownloadsPage),
        ["together"] = typeof(TogetherPage),
        ["providers"] = typeof(ProvidersPage),
        ["stores"] = typeof(StoresPage),
        ["search"] = typeof(SearchPage),
        ["settings"] = typeof(SettingsPage),
        ["read"] = typeof(ReadHomePage),
        ["updates"] = typeof(ReadUpdatesPage),
    };

    readonly Options options;
    /// <summary>The serial of the newest pick whose notice was said.</summary>
    int matchSaid;
    readonly DispatcherQueue ui;
    readonly EngineSession session;
    readonly FfiPlayer? player;
    readonly FfiDirs dirs;
    readonly Ctx ctx;
    readonly NetworkWatch network;
    readonly DeviceWatch device;
    readonly Taskbar? taskbar;
    readonly MediaControls? media;
    Tray? tray;
    readonly ShellStatus status;
    bool selecting, played, closing, askedClose, firstRun;

    public MainWindow(Options options)
    {
        this.options = options;
        InitializeComponent();
        Title = T.AppName;
        // Each destination's tooltip names its shortcut: Ctrl+1 to Ctrl+9, in the sidebar's order.
        // Reading's places come and go with what Kaeri is for; ApplyMedium sets them and the tips.
        ApplyMedium();
        // The built-in Settings item reads the system language, so it takes the catalogue's word.
        Nav.Loaded += (_, _) =>
        {
            if (Nav.SettingsItem is NavigationViewItem settingsItem)
                settingsItem.Content = T.NavSettings;
        };
        ui = DispatcherQueue.GetForCurrentThread();

        ExtendsContentIntoTitleBar = true;
        SetTitleBar(AppTitleBar);
        AppWindow.TitleBar.PreferredHeightOption = TitleBarHeightOption.Tall;
        AppWindow.TitleBar.ButtonBackgroundColor = Colors.Transparent;
        AppWindow.TitleBar.ButtonInactiveBackgroundColor = Colors.Transparent;
        AppWindow.TitleBar.ButtonForegroundColor = Colours.Silk;
        AppWindow.TitleBar.ButtonHoverBackgroundColor = global::Windows.UI.Color.FromArgb(0x18, 0xFF, 0xFF, 0xFF);
        AppWindow.TitleBar.ButtonPressedBackgroundColor = global::Windows.UI.Color.FromArgb(0x10, 0xFF, 0xFF, 0xFF);
        AppWindow.SetIcon(Path.Combine(AppContext.BaseDirectory, "Assets", "kaeri.ico"));
        SystemBackdrop = new TintedMica();

        // The engine's own notices (the launch digest): in the window while it is in front,
        // else as a Windows notification.
        var host = new AppHost(ui, options.Fixture) { Notified = (title, body) => Notified(title, body) };
        (var engine, dirs) = StartEngine(host);
        Player.ShaderCache = Path.Combine(dirs.Cache, "player");
        ChatSetting.Load(dirs.Config);
        SmoothScrolling.Load(dirs.Config);
        FeedFirewall.Load(dirs.Config);
#if KAERI_TESTING
        fixtureInvite = options.Scene == "store-invited" ? engine.TestingStoreInvite() : null;
#endif
        Size();
        session = new EngineSession(engine, act => ui.TryEnqueue(() => act()));
        var art = new Art(session.Send);
        var updater = new Updater(ui, staged: options.Fixture && options.UpdateFrom is null) { Quit = CloseNow };
        updater.Ready += version =>
        {
            ShowToast(InfoBarSeverity.Informational, T.UpdatesReady(version), T.UpdatesRestart, updater.Install);
            readyToast = toastShown;
        };
        // The offer goes once the update moves on, from About or a failure.
        updater.Changed += () =>
        {
            if (updater.State.Step != UpdateStep.Ready && readyToast == toastShown && Toast.IsOpen)
                Toast.IsOpen = false;
        };
        ctx = new Ctx { Session = session, Art = art, Window = this, Options = options, Updater = updater, Tv = tv };
        Ctx.Current = ctx;
        session.Known.SendReadingWith(command => ctx.Send(command));
        session.Known.Reader.BookResource = url => engine.BookResource(url);
        StartReading();
        // Where sync stands and what is running, at the title bar's end.
        status = new ShellStatus(ctx, () => ShowSettings("accounts"), () => ShowSettings("devices"), ShowMessages);
        AppTitleBar.RightHeader = status;
#if KAERI_TESTING
        // The found skip times' shots: finding paused as a fixture says, whatever this machine does.
        session.Known.ForcedPause = options.Show switch
        {
            "found-set-low-power" or "found-panel-paused" => SkipFindPause.LowPower,
            "found-set-storage" => SkipFindPause.Storage,
            _ => null,
        };
#endif
        // A fixture run has no trailers of its own; the testing build gives most titles a made-up one.
        session.Known.FixtureTrailers = options.Fixture;
        session.Batch += batch =>
        {
            Ctx.Guarded("the art", () => art.Take(batch));
            Ctx.Guarded("the player", () => Player.Heard(batch));
            Ctx.Guarded("the media controls", () => media?.Heard(batch));
            Ctx.Guarded("the window", () => Heard(batch));
            Ctx.Guarded("the status", () => status.Heard(batch));
            ctx.Heard(batch);
        };
        session.Stopped += () =>
        {
            if (closing)
                return;
            // The engine stopped on its own: a window that says so, with Open again, rather than
            // the app vanishing.
            stoppedWindow = StartFailedWindow.Stopped();
            stoppedWindow.Activate();
            CloseNow();
        };

        player = StartPlayer(engine);
        Player.Attach(ctx, player);
        Player.FullscreenRequested += Fullscreen;
        Player.FullscreenOn = () => AppWindow.Presenter.Kind == AppWindowPresenterKind.FullScreen;
        Player.ShowingChanged += PlayerShowing;
        CrashGuard.Start(dirs, () => crashWhere);
        Player.ShowingChanged += _ => RememberWhere();
        Player.ControlsChanged += shown =>
        {
            if (Player.Showing)
                AppWindow.TitleBar.PreferredHeightOption = shown ? TitleBarHeightOption.Tall : TitleBarHeightOption.Collapsed;
        };
        Player.MiniRequested += Mini;
        // A dialog doesn't fit the 480 by 270 mini window: the window is its size again first.
        Dialogs.Opening += Player.LeaveMini;
        session.OffThread = e =>
        {
#if KAERI_TESTING
            if (options.Showcase && Showcase.Holds(e))
                return true;
            // --show=sync-*: the made-up group stands in for the fixture engine's own.
            if (options.Show?.StartsWith("sync-", StringComparison.Ordinal) == true && e is Event.SyncStatus or Event.SyncInvite)
                return true;
            // --show=synopsis...: the title's table with synopses and rows ahead (synopsis-ranges: 250 of
            // them); --show=player-episodes...: the playing title's, for the Episodes panel.
            if (options.Show is { } synopsisShot && (synopsisShot.StartsWith("synopsis", StringComparison.Ordinal) || synopsisShot.StartsWith("player-episodes", StringComparison.Ordinal))
                && e is Event.Episodes synopsisTable && synopsisTable.Rows.Length > 0)
            {
                var playerShot = synopsisShot.StartsWith("player-", StringComparison.Ordinal);
                DispatcherQueue.TryEnqueue(() =>
                {
                    var next = playerShot ? ctx.Known.Entry(synopsisTable.Id)?.Progress?.Next ?? 4 : 7;
                    var ranged = synopsisShot is "synopsis-ranges" || synopsisShot.StartsWith("synopsis-go", StringComparison.Ordinal) || synopsisShot.StartsWith("player-episodes-ranges", StringComparison.Ordinal);
                    var shown = new Event[] { Showcase.Synopses(synopsisTable, ranged ? 230 : next, ranged, synopsisShot == "synopsis-go-not-out") };
                    ctx.Known.Take(shown[0]);
                    Heard(shown);
                    ctx.Heard(shown);
                });
                return true;
            }
            // --show=source-unsourced: the title's table, with episodes no source lists.
            if (options.Show is "source-unsourced" or "source-unsourced-end" && e is Event.Episodes table)
            {
                DispatcherQueue.TryEnqueue(() =>
                {
                    var shown = new Event[] { Showcase.Unsourced(table, ctx.Known.Entry(table.Id)?.Progress?.Next ?? 1) };
                    ctx.Known.Take(shown[0]);
                    Heard(shown);
                    ctx.Heard(shown);
                });
                return true;
            }
#endif
            if (e is not Event.PlayerOps ops)
                return false;
            Player.NoteLoad(ops.PlaybackValue, ops.Ops);
            player?.Apply(ops.PlaybackValue, ops.Ops);
            if (ops.Ops.Any(op => op is PlayerOp.Open))
                ui.TryEnqueue(() => Player.ApplySubtitleStyle());
            return true;
        };
        session.Start();
        StartTv(engine);
        StartSync(engine);
        // The network path, said to the engine now and on every change; it answers with one
        // online or offline state (Event.Connectivity) the pages show.
        network = new NetworkWatch(c => ctx.Send(c), ui, options.Offline);
        // What the device says of itself, which with the network decides whether trailers start.
        device = new DeviceWatch(c => ctx.Send(c), ui);
        taskbar = Taskbar.For(WinRT.Interop.WindowNative.GetWindowHandle(this));
        // The media overlay, the media keys and the thumbnail's buttons, while an episode plays.
        // Aimed at the playback on screen, and in a party Next and Previous are the room's pick.
        media = MediaControls.Start(WinRT.Interop.WindowNative.GetWindowHandle(this), taskbar, art, ui, Player.MediaCommand);
        StartNotifications(art);
        Keys();
#if KAERI_TESTING
        if (options.Showcase)
        {
            // After the first batch, so the pages that read the engine's entries have them.
            ui.TryEnqueue(DispatcherQueuePriority.Low, async () =>
            {
                await Task.Delay(1500);
                var events = Showcase.Events();
                foreach (var e in events)
                    session.Known.Take(e);
                ctx.Heard(events);
            });
        }
#endif

        AppWindow.Closing += OnClosing;
        AppWindow.Changed += (_, e) =>
        {
            // Kept half a second after a drag or resize settles, not on every pixel of it.
            if (e.DidPositionChange || e.DidSizeChange || e.DidPresenterChange)
            {
                placeLater ??= PlaceTimer();
                placeLater.Stop();
                placeLater.Start();
            }
        };
        Closed += (_, _) => Shutdown();
        // Hidden or minimised, the tracker pull rests and one pull runs on return; downloads and torrents go on.
        VisibilityChanged += (_, e) => ctx.Send(new Command.SyncVisible(e.Visible));
        Root.Loaded += (_, _) => FirstPage();
        Root.SizeChanged += (_, e) => FitTitleBar(e.NewSize.Width);
    }

    /// <summary>
    /// The title bar at the smallest widths: the name gives way and the search box shrinks, so
    /// the box stays whole beside the caption buttons instead of running under them.
    /// </summary>
    void FitTitleBar(double width)
    {
        var narrow = width < 760;
        SearchBox.MinWidth = narrow ? 140 : 320;
        AppTitleBar.Title = narrow ? "" : T.AppName;
        status?.Fit(width < 1120);
    }

    /// <summary>
    /// The window where it was last left (size, position and maximised), when that still lies on
    /// a display; otherwise 80% of the display's work area (at least 1280 by 800, at most the work area), centred.
    /// The minimum is 500 by 420.
    /// </summary>
    void Size()
    {
        var area = DisplayArea.GetFromWindowId(AppWindow.Id, DisplayAreaFallback.Primary).WorkArea;
        var scale = GetDpiForWindow(WinRT.Interop.WindowNative.GetWindowHandle(this)) / 96.0;
        if (AppWindow.Presenter is OverlappedPresenter overlapped)
        {
            overlapped.PreferredMinimumWidth = (int)(500 * scale);
            overlapped.PreferredMinimumHeight = (int)(420 * scale);
        }
        if (options.Width is null && Place.Read(PlacePath) is { } place && Place.OnADisplay(place.Frame))
        {
            AppWindow.MoveAndResize(place.Frame);
            if (place.Maximised && AppWindow.Presenter is OverlappedPresenter p)
                p.Maximize();
            placed = true;
            return;
        }
        var width = options.Width is { } w ? (int)(w * scale) : Math.Min(Math.Max((int)(area.Width * 0.8), (int)(1280 * scale)), area.Width);
        var height = options.Height is { } h ? (int)(h * scale) : Math.Min(Math.Max((int)(area.Height * 0.8), (int)(800 * scale)), area.Height);
        AppWindow.MoveAndResize(new RectInt32(
            area.X + Math.Max(0, (area.Width - width) / 2), area.Y + Math.Max(0, (area.Height - height) / 2), width, height));
        placed = true;
    }

    bool placed;

    string PlacePath => Path.Combine(dirs.Config, "windows-window.json");

    DispatcherQueueTimer? placeLater;

    DispatcherQueueTimer PlaceTimer()
    {
        var timer = ui.CreateTimer();
        timer.Interval = TimeSpan.FromMilliseconds(500);
        timer.IsRepeating = false;
        timer.Tick += (_, _) => RememberPlace();
        return timer;
    }

    /// <summary>A frame waiting to be kept is kept now: the window is closing or going to the notification area.</summary>
    void KeepPlaceNow()
    {
        if (placeLater is not { IsRunning: true } waiting)
            return;
        waiting.Stop();
        RememberPlace();
    }

    /// <summary>Keeps the window's frame for the next launch: the normal frame, and whether it was maximised.</summary>
    void RememberPlace()
    {
        if (!placed || options.Width is not null || AppWindow.Presenter is not OverlappedPresenter overlapped || !AppWindow.IsVisible)
            return;
        var maximised = overlapped.State == OverlappedPresenterState.Maximized;
        if (overlapped.State == OverlappedPresenterState.Minimized)
            return;
        var frame = maximised && Place.Read(PlacePath) is { } was
            ? was.Frame
            : new RectInt32(AppWindow.Position.X, AppWindow.Position.Y, AppWindow.Size.Width, AppWindow.Size.Height);
        Place.Write(PlacePath, new Place(frame, maximised));
    }

    [System.Runtime.InteropServices.LibraryImport("user32.dll")]
    private static partial uint GetDpiForWindow(nint hwnd);

    (FfiEngine, FfiDirs) StartEngine(FfiHost host)
    {
        var start = EngineSession.AppStart();
#if KAERI_TESTING
        // --show=start-failed: the window a refused start shows, for a screenshot.
        if (options.Show == "start-failed")
            throw new FfiException.Config("unknown key", new Msg.StartFailedConfig(@"C:\Users\viewer\AppData\Roaming\kaeri\config.toml", "unknown field `player.speeed`"));
        if (options.Scene is { } name)
        {
            var root = AppDirs.NewScratchRoot("app");
            var scratch = AppDirs.Under(root);
            KaeriFfiMethods.StartLog(scratch.Cache);
            if (options.Downloads && options.Media is { } clip)
                KaeriFfiMethods.SeedFixtureDownloads(scratch, clip);
            FfiScene scene = name switch
            {
                "fresh" => new FfiScene.Fresh(),
                "sourceless" => new FfiScene.Sourceless(),
                "unsure" => new FfiScene.Unsure(),
                "link-opened" => new FfiScene.LinkOpened(),
                "servers" => new FfiScene.Servers(),
                "library" => new FfiScene.Library(),
                "sources-busy" => new FfiScene.SourcesBusy(),
                "available" => new FfiScene.Available(),
                "licensed" => new FfiScene.Licensed(),
                "demoonly" => new FfiScene.DemoOnly(),
                "demoplayed" => new FfiScene.DemoPlayed(),
                "demoandstore" => new FfiScene.DemoAndStore(),
                "demoremoved" => new FfiScene.DemoRemoved(),
                "subtitlestyled" => new FfiScene.SubtitleStyled(),
                "crashoff" => new FfiScene.CrashOff(),
                "crashsaved" => new FfiScene.CrashSaved(),
                "crashheld" => new FfiScene.CrashHeld(),
                "skipmarked" => new FfiScene.SkipMarked(options.Media ?? @"C:\setup\sdr-testsrc.mp4", 60),
                "store-owner" => new FfiScene.StoreOwner(),
                "store-invited" => new FfiScene.StoreInvited(),
                "playing" => new FfiScene.Playing(options.Media ?? @"C:\setup\sdr-testsrc.mp4"),
                "demo-only" => new FfiScene.DemoOnly(),
                "demo-played" => new FfiScene.DemoPlayed(),
                "demo-and-store" => new FfiScene.DemoAndStore(),
                "demo-removed" => new FfiScene.DemoRemoved(),
                "subtitle-styled" => new FfiScene.SubtitleStyled(),
                "crash-off" => new FfiScene.CrashOff(),
                "crash-saved" => new FfiScene.CrashSaved(),
                "crash-held" => new FfiScene.CrashHeld(),
                "profiles" => new FfiScene.Profiles(),
                "reading-first-run" => new FfiScene.ReadingFirstRun(),
                "reading-update-sheet" => new FfiScene.ReadingUpdateSheet(),
                "reading-join-ask" => new FfiScene.ReadingJoinAsk(),
                "reading-home" => new FfiScene.ReadingHome(),
                "reading-series" => new FfiScene.ReadingSeries(),
                "reader-manga-rtl" => new FfiScene.ReaderMangaRtl(),
                "reader-strip" => new FfiScene.ReaderStrip(),
                "reader-novel" => new FfiScene.ReaderNovel(),
                "reading-downloads" => new FfiScene.ReadingDownloads(),
                "reader-moved" => new FfiScene.ReaderMoved(),
                "reading-dark" => new FfiScene.ReadingDark(),
                _ => new FfiScene.Watching(),
            };
            return (FfiEngine.StartScene(scratch, host, scene, EngineSession.AppStart(signIn: false)), scratch);
        }
#endif
        var user = AppDirs.User();
        // A first launch: no config yet. Existing installs never see the welcome.
        firstRun = !Directory.Exists(user.Config) || !Directory.EnumerateFileSystemEntries(user.Config).Any();
        KaeriFfiMethods.StartLog(user.Cache);
        return (EngineSession.StartUser(host, start), user);
    }

    FfiPlayer? StartPlayer(FfiEngine engine)
    {
        try
        {
            var opts = new FfiPlayerOptions(
                Video: new FfiVideo.Composition(1280, 720),
                CacheDir: Path.Combine(dirs.Cache, "player"));
            return new FfiPlayer(opts, engine, new Sink(this));
        }
        catch (Exception e)
        {
            KaeriFfiMethods.LogLine(FfiLogLevel.Error, "windows", $"the player did not start: {e.Message}");
            return null;
        }
    }

    /// <summary>The player's events, from mpv's threads to the UI's.</summary>
    sealed class Sink(MainWindow window) : FfiPlayerSink
    {
        public void Event(FfiPlayerEvent e) => window.ui.TryEnqueue(() =>
        {
            window.Player.Heard(e);
            window.media?.Heard(e);
        });
    }

    // The first page, and what the command line asks for once the engine has a list to show.

    void FirstPage()
    {
        // Later launches, and kaeri:// links clicked while the app runs, come here; the one
        // this launch came with opens once the welcome, if it shows, is up to take it.
        Program.Listen(link => ui.TryEnqueue(() =>
        {
            Forward();
            if (link is not null)
                OpenLink(link);
        }));
        // After Restart to update: whether the new version is the one running.
        if (ctx.Updater.AfterRestart() is { } restarted)
            ui.TryEnqueue(DispatcherQueuePriority.Low, () =>
            {
                if (restarted.Installed)
                    ShowToast(InfoBarSeverity.Success, T.UpdatesUpdated(Updater.Current));
                else
                    ShowToast(InfoBarSeverity.Warning, T.UpdatesDidNotInstall(restarted.Version), T.UpdatesOpenAbout,
                        () => OpenSettings("about"));
            });
#if KAERI_TESTING
        // --show=update: a newer release with this machine's installer, as the engine's check
        // tells of one, which a fixture run stages without fetching.
        // --update-from=<url>,<sha256>,<size>,<version> offers that installer instead, fetched
        // and installed for real; --show=update-install presses Restart to update once it is ready.
        if (options.Show is "update" or "update-install" || options.UpdateFrom is not null)
            ui.TryEnqueue(DispatcherQueuePriority.Low, async () =>
            {
                await Task.Delay(1200);
                var events = new Event[] { new Event.Update(new UpdateStatus.Available(Showcase.Release(options.UpdateFrom), true)) };
                ctx.Known.Take(events[0]);
                Heard(events);
                ctx.Heard(events);
                if (options.Show != "update-install")
                    return;
                while (ctx.Updater.State.Step is not (UpdateStep.Ready or UpdateStep.Failed))
                    await Task.Delay(250);
                await Task.Delay(1500);
                ctx.Updater.Install();
            });
#endif
        if (options.Opens is { } opens)
            ui.TryEnqueue(DispatcherQueuePriority.Low, async () =>
            {
                await Task.Delay(900);
                OpenLink(opens);
            });
        var page = options.Page ?? "home";
        if (options.Query is not null)
            ShowSearch(options.Query);
        else
            Navigate(page, options.Pane);
        if (options.Title is { } id)
            OpenTitle(id);
        // --show=opened: Open link or file with the fixture's made-up magnet link, for a screenshot.
        if (options.Show == "opened")
            ui.TryEnqueue(DispatcherQueuePriority.Low, async () =>
            {
                await Task.Delay(1800);
                OpenLink("magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567&dn=Hoshikuzu+no+Tabibito+-+03+%5B1080p%5D.mkv");
            });
        // --show=opened-unreadable: a magnet link the engine can't read, to see the dialog say so and go back.
        if (options.Show == "opened-unreadable")
            ui.TryEnqueue(DispatcherQueuePriority.Low, async () =>
            {
                await Task.Delay(1800);
                OpenLink("magnet:?xt=urn:btih:not-a-hash");
            });
        if (options.Show == "open-link")
            ui.TryEnqueue(DispatcherQueuePriority.Low, async () =>
            {
                await Task.Delay(1800);
                ShowOpenLink();
            });
        if (options.Join)
            ui.TryEnqueue(DispatcherQueuePriority.Low, async () =>
            {
                await Task.Delay(1800);
                ShowJoinStore(fixtureInvite ?? "https://plugins.hotaru.example/join/fixture#key");
            });
        if (options.AskClose)
            ui.TryEnqueue(DispatcherQueuePriority.Low, async () =>
            {
                await Task.Delay(2500);
                AskBeforeClosing();
            });
#if KAERI_TESTING
        // --show=accounts-states: Settings › Accounts with an expired AniList sign-in, a
        // MyAnimeList pull that failed and Simkl synced, as the engine would say them.
        if (options.Show == "accounts-states")
            ui.TryEnqueue(DispatcherQueuePriority.Low, async () =>
            {
                await Task.Delay(2500);
                ctx.Inject(new Event.Sync(new SyncState("anilist", true, 0, 0, User: "mika", NeedsReauth: true)));
                ctx.Inject(new Event.Sync(new SyncState("mal", true, 0, 0, User: "mika", Last: "pulled", PullError: new PullError("the server did not answer", 0))));
                ctx.Inject(new Event.Sync(new SyncState("simkl", true, 0, 0, User: "mika", Last: "pulled")));
            });
        // --show=jobs[-open]: two jobs past 600 ms, one held up on a wait, as the engine would say
        // them; -open opens the status flyout. --show=messages: a notice and an alert, then Messages.
        if (options.Show is "jobs" or "jobs-open")
            ui.TryEnqueue(DispatcherQueuePriority.Low, async () =>
            {
                await Task.Delay(2000);
                ctx.Inject(new Event.JobStarted(9_100_001, "checking sources")); // i18n-skip
                await Task.Delay(400);
                ctx.Inject(new Event.JobStarted(9_100_002, "syncing with AniList")); // i18n-skip
                await Task.Delay(1200);
                ctx.Inject(new Event.JobNote(9_100_002, new Msg.StatusWaitingForRedirect("AniList"), TimeSpan.FromSeconds(12)));
                if (options.Show == "jobs-open")
                    status.Open();
                await Task.Delay(6000);
                ctx.Inject(new Event.JobDone(9_100_001));
                await Task.Delay(3000);
                ctx.Inject(new Event.JobDone(9_100_002));
            });
        // --show=conflicts[-open]: two titles that differ from AniList, as a pull would find them,
        // under Accounts' AniList card; -open opens the list to settle them.
        if (options.Show is "conflicts" or "conflicts-open")
            ui.TryEnqueue(DispatcherQueuePriority.Low, async () =>
            {
                await Task.Delay(2500);
                (uint Id, string Title)[] titles = [(910_001, "Stardust Wayfarer"), (910_002, "The Steel Gardener")]; // i18n-skip
                ctx.Inject(new Event.Conflicts([.. titles.Select((t, i) => new ConflictRow(t.Id, t.Title, i == 0 ? "progress" : "score", i == 0 ? "7" : "8", i == 0 ? "5" : "6"))])); // i18n-skip
                if (options.Show == "conflicts-open")
                {
                    await Task.Delay(1000);
                    await Sync.ConflictsDialog.Show(ctx, Root.XamlRoot);
                }
            });
        // --show=hidden: three hidden titles in Settings › General, for Restore to take away one by one.
        if (options.Show == "hidden")
            ui.TryEnqueue(DispatcherQueuePriority.Low, async () =>
            {
                await Task.Delay(3000);
                var now = DateTimeOffset.UtcNow.ToUnixTimeSeconds();
                ctx.Inject(new Event.HiddenTitles(
                [
                    new HiddenTitle(new Entry(920_001, "Lantern Harbor", Genres: [], AvailableOn: []), HiddenAt: now - 3 * 86_400), // i18n-skip
                    new HiddenTitle(new Entry(920_002, "Moss and Iron", Genres: [], AvailableOn: []), UntilAirs: true, HiddenAt: now - 86_400), // i18n-skip
                    new HiddenTitle(new Entry(920_003, "The Quiet Orbit", Genres: [], AvailableOn: [])), // i18n-skip
                ]));
            });
        if (options.Show == "messages")
            ui.TryEnqueue(DispatcherQueuePriority.Low, async () =>
            {
                // Heard by the window as the engine's own notices are, so they show and are kept.
                void Notice(NoticeKind kind, Msg msg)
                {
                    Event[] said = [new Event.Toast(new Notice(kind, "", Msg: msg))];
                    Heard(said);
                    ctx.Heard(said);
                }
                await Task.Delay(1500);
                Notice(NoticeKind.Info, new Msg.StatusWaitingForRedirect("AniList")); // i18n-skip
                Notice(NoticeKind.Alert, new Msg.NoticeSignInFailed("Kitsu", "invalid credentials")); // i18n-skip
                await Task.Delay(1000);
                ShowMessages();
                await Task.Delay(2000);
                Notice(NoticeKind.Info, new Msg.NoticeSignInNotWaiting("AniList")); // i18n-skip
            });
        if (options.Show?.StartsWith("party-", StringComparison.Ordinal) == true)
            RehearseParty(options.Show);
        if (options.Show?.StartsWith("tv-", StringComparison.Ordinal) == true)
            RehearseTv(options.Show);
        if (options.Show?.StartsWith("cast-", StringComparison.Ordinal) == true)
            RehearseCast(options.Show);
        // --show=lawful-*: the add-a-server dialog (lawful-server-*), or the licensed services page (lawful-licensed).
        if (options.Show?.StartsWith("lawful-server", StringComparison.Ordinal) == true)
            ui.TryEnqueue(DispatcherQueuePriority.Low, async () => { await Task.Delay(900); ShowAddServer(); });
        // --show=folder and --show=server[-N]: the first folder's page, or the nth server's, once the scene's sources are read.
        if (options.Show == "folder")
            ui.TryEnqueue(DispatcherQueuePriority.Low, async () =>
            {
                ctx.Send(new Command.Local(new LocalCommand.Load()));
                await Task.Delay(2500);
                if (ctx.Known.Local?.Folders.FirstOrDefault() is { } folder)
                    Push(typeof(Pages.FolderPage), folder.Path);
            });
        // --show=fix-match: Fix this match for the Hoshikuzu folder, searched by the title's other name.
        if (options.Show == "fix-match")
            ui.TryEnqueue(DispatcherQueuePriority.Low, async () =>
            {
                ctx.Send(new Command.Local(new LocalCommand.Load()));
                await Task.Delay(2500);
                if (ctx.Known.Local?.Groups.FirstOrDefault(g => g.Name.StartsWith("Hoshikuzu", StringComparison.Ordinal)) is { } group)
                    await new Pages.FixMatchDialog(ctx, Root.XamlRoot, group with { Name = "Hoshikuzu no Tabibito" }).ShowAsync();
            });
        if (options.Show?.StartsWith("server", StringComparison.Ordinal) == true && options.Show != "server-cert")
            ui.TryEnqueue(DispatcherQueuePriority.Low, async () =>
            {
                ctx.Send(new Command.Server(new ServerCommand.Load()));
                await Task.Delay(2500);
                var nth = options.Show.Length > 7 && int.TryParse(options.Show[7..], out var n) ? n : 0;
                if (ctx.Known.Servers.ElementAtOrDefault(nth) is { } server)
                    Push(typeof(Pages.ServerPage), server.Id);
            });
        if (options.Show == "lawful-licensed")
            ui.TryEnqueue(DispatcherQueuePriority.Low, async () => { await Task.Delay(900); Push(typeof(Pages.LicensedPage), null); });
        // Home's first content is the usual trigger; this covers a run where it never comes.
        ui.TryEnqueue(DispatcherQueuePriority.Low, async () =>
        {
            await Task.Delay(4000);
            if (!crashAsked)
            {
                crashAsked = true;
                AskAboutCrash();
            }
        });
        if (options.CrashProbe)
            ui.TryEnqueue(DispatcherQueuePriority.Low, async () =>
            {
                await Task.Delay(3000);
                CrashGuard.Caught(new InvalidOperationException("crash probe"));
            });
        // --show=invite: the invite sheet with a made-up link, for a screenshot.
        if (options.Show == "invite")
            ui.TryEnqueue(DispatcherQueuePriority.Low, async () =>
            {
                await Task.Delay(1500);
                Pages.InviteDialog.Show(ctx, Root.XamlRoot, "https://store.example/join/Kq3vT9xLw2", "Mira");
            });
        // --show=starting-<state>[-<seconds>][-reduced]: the player's starting screen on the engine's
        // made-up start, with no media: the player opens for it and nothing plays.
        if (options.Show?.StartsWith("starting-", StringComparison.Ordinal) == true && !options.Play)
            ui.TryEnqueue(DispatcherQueuePriority.Low, async () =>
            {
                await Task.Delay(1500);
                await Player.StartDemo(options.Show);
            });
        // --show=devicecode: Trakt's sign-in code, for a screenshot.
        if (options.Show == "devicecode")
            ui.TryEnqueue(DispatcherQueuePriority.Low, async () =>
            {
                await Task.Delay(1500);
                Pages.DeviceCodeDialog.Heard(Root.XamlRoot, new DeviceCodePrompt("trakt", "7F3K9QX2", "https://tracker.example/activate",
                    DateTimeOffset.UtcNow.ToUnixTimeSeconds() + 15 * 60 - 1));
            });
#endif
        if (options.Shortcuts)
            ui.TryEnqueue(DispatcherQueuePriority.Low, async () =>
            {
                await Task.Delay(1500);
                ShowShortcuts();
            });
        // First launch: the three pages, once. A fixture run shows them only when asked.
        var welcomed = Path.Combine(dirs.Config, "windows-welcomed");
        if (options.Welcome || (!options.Fixture && firstRun && !File.Exists(welcomed)))
            ui.TryEnqueue(DispatcherQueuePriority.Low, async () =>
            {
                await Task.Delay(600);
                // Not kept on a disk that refuses it: it shows again next time, which is harmless.
                SmallFiles.Write(welcomed, "");
                var welcome = welcoming = new WelcomeDialog(ctx, Root.XamlRoot);
                if (options.Welcome && options.Show is { } step)
                    welcome.Step(step);
                await welcome.ShowAsync();
                welcoming = null;
                if (welcome.Invite is { } invite)
                    ShowJoinStore(invite);
                else if (welcome.Then == "folder")
                    AddFolder();
                else if (welcome.Then == "server")
                    ShowAddServer();
                else if (welcome.Then == "pair")
                    Sync.SyncDialogs.PairShow(Sync.Origin.FirstLaunch);
                else if (welcome.Then == "restore")
                    Sync.SyncDialogs.Picker();
                else if (welcome.Then == "book-folder")
                    await CameInDesk.PickFolder(ctx);
                else if (welcome.Then == "book-server")
                    await new BookServerDialog(ctx, Root.XamlRoot).ShowAsync();
            });
    }

    void Heard(IReadOnlyList<Event> batch)
    {
        PartyHeard(batch);
        foreach (var e in batch)
        {
            // --play: Home's spotlight, the title Continue leads with, once the engine has it. Before
            // the switch, whose first Continue case, the crash question, would take the event.
            if (options.Play && !played)
            {
                if (e is Event.Home { V1.Spotlight: { Case: SpotlightCase.Continuing } spot })
                {
                    played = true;
                    ctx.Play(spot.Entry.Id, spot.Episode.ToString(CultureInfo.InvariantCulture));
                }
                else if (e is Event.Content { View: ContentFor.Continue, ContentValue: ContentList.Entries })
                {
                    var entries = ctx.Known.ListEntries(new ContentFor.Continue());
                    var queue = Wording.Said(KaeriFfiMethods.ContinueQueue(entries, [.. ctx.Known.HiddenFromContinue], Casing.Sentence));
                    if (queue.InProgress.Concat(queue.NewThisWeek).FirstOrDefault(q => q.Playable) is { } first)
                    {
                        played = true;
                        ctx.Play(first.Entry.Id, first.Episode.ToString(CultureInfo.InvariantCulture));
                    }
                }
            }
            switch (e)
            {
                case Event.Sync or Event.SyncStatus or Event.Profiles:
                    ShowAccount();
                    break;
                case Event.Content { View: ContentFor.Continue } when !crashAsked:
                    crashAsked = true;
                    AskAboutCrash();
                    break;
                case Event.Settings:
                    CrashGuard.Follow(SettingsCatalog.IsOn(ctx.Known.Setting(SettingKey.SaveCrashReports)));
                    // Motion off in Settings reduces motion, as Windows' own Animation effects off does.
                    Motion.Chosen = ctx.Known.Setting(SettingKey.Motion) is not { } motion || SettingsCatalog.IsOn(motion);
                    RememberWhere();
                    break;
                case Event.TitlesChanged:
                    // Every title is named another way now: what has been shown is asked for again.
                    foreach (var command in ctx.Known.AskAgain())
                        ctx.Send(command);
                    break;
                // A cast's reason is on its card, and someone taking the TV has its own notice.
                case Event.Toast t when TvWords.OnCastCard(t.V1.Msg):
                    Keep(t.V1);
                    break;
                case Event.LicensedReturn back:
                    ShowReturn(back.V1);
                    break;
                // A start that failed says why on its own screen, not in a notice over it.
                case Event.Toast { V1.Kind: NoticeKind.Alert } quiet when Player.IsStarting:
                    Keep(quiet.V1);
                    break;
                case Event.Toast t when !Lines.SettingSaved(t.V1):
                    ShowToast(t.V1);
                    break;
                case Event.PinUnavailable pin when ctx.Known.SourceChoices.Say(new PinUnavailableNotice(pin.Id, pin.ProviderId, pin.Name)):
                    // The source kept for a title is off, removed or resting: said once, and the order is used.
                    ShowToast(InfoBarSeverity.Informational, T.SourcePinUnavailable(pin.Name));
                    break;
                case Event.TitleMatch when ctx.Known.Matches.Settled is { } settled && settled.Serial > matchSaid:
                    // A pick settled: said once, wherever the person is by then.
                    matchSaid = settled.Serial;
                    ShowToast(InfoBarSeverity.Informational, T.MatchPicked(settled.Title, settled.Source));
                    break;
                case Event.DeviceCode code:
                    Pages.DeviceCodeDialog.Heard(Root.XamlRoot, code.V1);
                    break;
                case Event.Authorize authorize:
                    // AniList and MyAnimeList: their page in the browser, which comes back as
                    // kaeri://auth/<tracker> (OpenLink).
                    OpenSignIn(authorize.Url);
                    break;
                case Event.Update update:
                    ctx.Updater.Heard(update.V1);
                    break;
                case Event.Downloads d:
                    taskbar?.Downloads(ctx.Known.Downloads);
                    foreach (var notice in Notices.Downloaded(downloadsSeen, d.V1))
                        toasts?.Post(notice);
                    downloadsSeen = d.V1;
                    break;
                case Event.NewEpisodes n:
                    foreach (var notice in Notices.Aired(n.V1))
                        toasts?.Post(notice);
                    break;
            }
        }
    }

    /// <summary>A tracker's sign-in page in the browser; a toast when no browser takes it.</summary>
    async void OpenSignIn(string url)
    {
        var opened = false;
        try
        {
            opened = Uri.TryCreate(url, UriKind.Absolute, out var uri) && await global::Windows.System.Launcher.LaunchUriAsync(uri);
        }
        catch (Exception)
        {
            // Said below.
        }
        if (!opened)
            ShowToast(InfoBarSeverity.Warning, T.AccountsBrowserFailed);
    }

    /// <summary>The tracker the window syncs with, by its own name, which no language changes.</summary>
    const string Tracker = "AniList";

    /// <summary>
    /// The account in the pane's foot. Water is calm state: changes waiting to sync. A sign-in
    /// that needs renewing is a problem, so it says so in words beside the error glyph.
    /// </summary>
    void ShowAccount()
    {
        var anilist = ctx.Known.Anilist;
        var reauth = anilist is { NeedsReauth: true };
        // A cloud folder that signed Kaeri out marks the account too, until it is signed in again.
        var cloud = ctx.Known.SyncGroup is { Problem: SyncProblem.SignInNeeded needed } && !Sync.SyncDialogs.ByHand
            ? T.CloudErrSignedOut(SyncWords.ServiceName(needed.Service), SyncWords.Kind)
            : null;
        var text = anilist is null ? T.AccountsStateSignedOut : reauth ? T.AccountsPaneSignInAgain : anilist.User ?? Tracker;
        var dot = anilist is { Outbox: > 0 } && !reauth ? Visibility.Visible : Visibility.Collapsed;
        var alert = reauth || cloud is not null ? Visibility.Visible : Visibility.Collapsed;
        var tip = cloud is not null && !reauth ? cloud
            : anilist is null ? T.LibrarySignedOutAction
            : reauth ? T.AccountsStateExpired(Tracker)
            : anilist.Outbox > 0 ? T.AccountsStateSignedInWaiting(Tracker, anilist.User ?? Tracker, anilist.Outbox)
            : T.AccountsStateSignedIn(Tracker, anilist.User ?? Tracker);
        // With two or more profiles the item names the one in use and opens the flyout to switch.
        if (ProfileWords.Active(ctx.Known.Profiles) is { } active)
        {
            text = active.Name;
            tip = T.ProfilesAvatarDesc(active.Name);
        }
        // Each sync event lands here: only what changed is set, so the pane's foot never redraws.
        if (AccountText.Text != text)
            AccountText.Text = text;
        if (AccountDot.Visibility != dot)
            AccountDot.Visibility = dot;
        if (AccountAlert.Visibility != alert)
            AccountAlert.Visibility = alert;
        var picks = !ProfileWords.Picking(ctx.Known.Profiles);
        if (AccountItem.SelectsOnInvoked != picks)
            AccountItem.SelectsOnInvoked = picks;
        if (ToolTipService.GetToolTip(AccountItem) as string != tip)
            ToolTipService.SetToolTip(AccountItem, tip);
        var named = T.AccountsPaneName(tip);
        if (AutomationProperties.GetName(AccountItem) != named)
            AutomationProperties.SetName(AccountItem, named);
    }

    /// <summary>
    /// Settings open on one pane, such as Profiles from the account flyout. With Settings already
    /// open, the pane changes there: no second Settings page, and no Back step to the same page.
    /// </summary>
    internal void OpenSettings(string pane)
    {
        if (ContentFrame.Content is SettingsPage open)
        {
            open.ShowPane(pane);
            selecting = true;
            Nav.SelectedItem = pane == "accounts" ? AccountItem : Nav.SettingsItem;
            selecting = false;
            return;
        }
        ContentFrame.Navigate(typeof(SettingsPage), pane, new SuppressNavigationTransitionInfo());
    }

    void OnNavInvoked(NavigationView sender, NavigationViewItemInvokedEventArgs args)
    {
        if (args.InvokedItemContainer == AccountItem && ProfileWords.Picking(ctx.Known.Profiles))
            ShowProfileFlyout();
    }

    internal void ShowProfileFlyout() => Sync.Profiles.Flyout().ShowAt(AccountItem);

    // Notices: the engine's words with the first letter capitalised, for as long as they take to
    // read (4 s, 8 s with a button or for an alert, and 1 s for every 10 words past 12), paused
    // under the pointer, with the hint as the one button.

    int toastShown;
    int readyToast = -1;
    bool toastHeld;
    DateTimeOffset toastUntil;

    /// <summary>A notice from a page, with one button when it has something to undo.</summary>
    public void Say(string text, string? button = null, Action? act = null) =>
        ShowToast(InfoBarSeverity.Informational, text, button, act);

    /// <summary>A plain notice of the app's own: Matching automatically again.</summary>
    public void Say(string text) => ShowToast(InfoBarSeverity.Informational, text);

    /// <summary>A notice said elsewhere than the notice bar still goes to Messages.</summary>
    void Keep(Notice notice) => ctx.Known.Messages.Add(notice.Msg?.Text() ?? "", notice.Kind == NoticeKind.Alert, DateTimeOffset.Now);

    /// <summary>Ctrl+Shift+L and the status flyout: every notice this run.</summary>
    internal void ShowMessages() => Messages.Show(ctx, Root.XamlRoot);

    void ShowToast(Notice notice)
    {
        var text = notice.Msg?.Text() ?? "";
        if (notice.Kind == NoticeKind.Alert && text.Length > 0)
            ctx.Known.LastError = new BugError("message", text);
        var (button, act) = notice.Hint switch
        {
            NoticeHint.StopPlayback => (T.ActionStop, (Action)(() => { if (Player.Showing) Player.Close(); })),
            NoticeHint.OpenStores => (T.ToastOpenStores, () => Go("stores")),
            NoticeHint.KitsuCredentials => (T.ActionSignIn, () => KitsuDialog.Show(ctx, Root.XamlRoot)),
            NoticeHint.UndoRemoveDemo => (T.ActionUndo, () => ctx.Send(new Command.AddDemo())),
            NoticeHint.RemoveDemo => (T.SourcesDemoRemove, () => ctx.Send(new Command.RemoveDemo())),
            NoticeHint.UndoSubtitleReset => (T.ActionUndo, () =>
            {
                if (ctx.Known.StyleBeforeReset is { } before)
                    Kaeri.Windows.Player.SubtitleStyling.Restore(ctx, before);
            }),
            NoticeHint.ReportCrash => (T.CrashNoticeReport, () => ReportDialog.Show(ctx, Root.XamlRoot, latestCrash: true)),
            _ => (null, null),
        };
        ShowToast(notice.Kind == NoticeKind.Alert ? InfoBarSeverity.Warning : InfoBarSeverity.Informational, text, button, act);
    }

    void ShowToast(InfoBarSeverity severity, string text, string? button = null, Action? act = null)
    {
        var shown = ++toastShown;
        var alert = severity == InfoBarSeverity.Warning;
        ctx.Known.Messages.Add(text, alert, DateTimeOffset.Now);
        // A newer notice crossfades over the last; a first one fades in.
        if (Toast.IsOpen)
            Motion.FadeIn(Toast);
        else
        {
            Toast.Opacity = 0;
            Motion.FadeTo(Toast, 1, Motion.FadeInTime);
        }
        // An alert takes the vermilion mark, as on the Mac; Warning's icon is haze.
        Toast.Severity = alert ? InfoBarSeverity.Error : severity;
        Toast.Message = text;
        if (button is not null && act is not null)
        {
            var b = new Button { Content = button };
            b.Click += (_, _) =>
            {
                Toast.IsOpen = false;
                act();
            };
            Toast.ActionButton = b;
        }
        else
            Toast.ActionButton = null;
        Toast.IsOpen = true;
        // An alert is read out at once, over what Narrator was saying; the others politely.
        if (alert && (FrameworkElementAutomationPeer.FromElement(Toast) ?? FrameworkElementAutomationPeer.CreatePeerForElement(Toast)) is { } peer)
            peer.RaiseNotificationEvent(AutomationNotificationKind.Other, AutomationNotificationProcessing.ImportantMostRecent, text, "kaeri-alert"); // i18n-skip
        // A notice with a button stays until it is dismissed or acted on; the others stay twice as
        // long while a screen reader runs.
        var stays = NoticeTime.Stays(text, alert, button is not null && act is not null, DeviceWatch.ScreenReaderRunning());
        if (stays is not { } time)
        {
            toastUntil = DateTimeOffset.MaxValue;
            return;
        }
        toastUntil = DateTimeOffset.UtcNow.Add(time);
        _ = CloseToastLater(shown);
    }

    async Task CloseToastLater(int shown)
    {
        while (shown == toastShown)
        {
            await Task.Delay(250);
            if (toastHeld)
            {
                toastUntil = toastUntil.AddMilliseconds(250);
                continue;
            }
            if (DateTimeOffset.UtcNow >= toastUntil)
                break;
        }
        if (shown != toastShown)
            return;
        // Fades out, then closes, unless a newer notice came meanwhile.
        Motion.FadeTo(Toast, 0, Motion.FadeOutTime);
        await Task.Delay(Motion.FadeOutTime);
        if (shown == toastShown)
        {
            Toast.IsOpen = false;
            Toast.Opacity = 1;
        }
    }

    void OnToastPointerEntered(object sender, PointerRoutedEventArgs e) => toastHeld = true;

    void OnToastPointerExited(object sender, PointerRoutedEventArgs e) => toastHeld = false;

    // Moving between pages.

    void Navigate(string tag, object? parameter = null)
    {
        if (PageFor(tag) is not { } page)
            return;
        if (ContentFrame.CurrentSourcePageType == page && parameter is null && tag != "search")
            return;
        ContentFrame.Navigate(page, parameter, new SuppressNavigationTransitionInfo());
    }

    /// <summary>Sources with one source selected, as Details on the demo asks. The player closes first.</summary>
    internal void ShowSource(string id)
    {
        if (Player.Showing)
            Player.Close();
        Navigate("providers", id);
    }

    /// <summary>Settings on a pane, as About's crash link and the report sheet ask. The player closes first.</summary>
    internal void ShowSettings(string pane)
    {
        if (Player.Showing)
            Player.Close();
        OpenSettings(pane);
    }

    /// <summary>Goes to a destination by its tag, as the pane and Ctrl+1 to Ctrl+9 do.</summary>
    internal void Go(string tag)
    {
        if (Player.Showing)
            Player.Close();
        Navigate(tag);
    }

    /// <summary>
    /// A title's page, as the Mac opens one: the page crossfades over the one it leaves on the
    /// standard token, and the cover flies from where it was clicked when one was (a connected
    /// animation, on the standard spring). With animation effects off, only the crossfade.
    /// </summary>
    internal void OpenTitle(uint id)
    {
        if (Player.Showing)
            Player.Close();
        if (!Motion.Enabled)
            ConnectedAnimationService.GetForCurrentView().GetAnimation(TitlePage.CoverAnimation)?.Cancel();
        Crossfade.Swap(ContentFrame, () => ContentFrame.Navigate(typeof(TitlePage), id, new SuppressNavigationTransitionInfo()));
    }

    /// <summary>
    /// Another: this title's page is swapped for the next pick's, so Back goes to where Surprise me
    /// was pressed. The page as it stood fades away over the new one on the standard token.
    /// </summary>
    internal void SwapTitle(uint id)
    {
        Crossfade.Swap(ContentFrame, () =>
        {
            ContentFrame.Navigate(typeof(TitlePage), id, new SuppressNavigationTransitionInfo());
            // The page it replaces is not a place to come back to.
            if (ContentFrame.BackStack.Count > 0)
                ContentFrame.BackStack.RemoveAt(ContentFrame.BackStack.Count - 1);
        });
    }

    /// <summary>A browse from Home: the query's genres and sort set, nothing typed.</summary>
    internal void ShowBrowse(SearchQuery query)
    {
        SearchBox.Text = query.Text;
        ContentFrame.Navigate(typeof(SearchPage), query, new SuppressNavigationTransitionInfo());
    }

    internal void ShowSearch(string query)
    {
        SearchBox.Text = query;
        // Enter on Search searches there, as typing does, rather than adding a page for Back.
        if (ContentFrame.Content is SearchPage page)
        {
            page.Submit(query);
            return;
        }
        ContentFrame.Navigate(typeof(SearchPage), query, new SuppressNavigationTransitionInfo());
    }

    /// <summary>The join-store dialog, with an invite already pasted when one is given.</summary>
    internal async void ShowJoinStore(string? pasted = null)
    {
        var dialog = joining = new JoinStoreDialog(ctx, Root.XamlRoot, pasted);
        try
        {
            await dialog.ShowAsync(options.Show == "preview");
        }
        catch (COMException)
        {
            // Another dialog came up first: the link waits for the viewer to open it again.
            return;
        }
        finally
        {
            if (joining == dialog)
                joining = null;
        }
        // The store just joined, or the one joined before: Plugin stores opens on it.
        if ((dialog.Store ?? ctx.Known.Joined) is { } store)
        {
            ctx.Known.Joined = null;
            if (ContentFrame.Content is StoresPage stores)
                stores.Select(store);
            else
                ContentFrame.Navigate(typeof(StoresPage), store, new SuppressNavigationTransitionInfo());
        }
    }

    void OnNavSelectionChanged(NavigationView sender, NavigationViewSelectionChangedEventArgs args)
    {
        if (selecting)
            return;
        if (args.IsSettingsSelected)
            Navigate("settings");
        else if (args.SelectedItem is NavigationViewItem { Tag: string tag })
        {
            if (tag == "account")
                OpenSettings("accounts");
            else if (tag == "search")
            {
                // Search in the sidebar, as on the Mac: the page with what the box holds, and the
                // box ready to type in.
                var query = SearchBox.Text.Trim();
                if (ContentFrame.CurrentSourcePageType != typeof(SearchPage))
                    Navigate("search", query.Length > 0 ? query : null);
                SearchBox.Focus(FocusState.Programmatic);
            }
            else
                Navigate(tag);
        }
    }

    object? framePage;

    /// <summary>Where the app is, as a crash report would word it. Kept for a thread that cannot ask the window.</summary>
    volatile CrashGuard.Where crashWhere = new(CrashPlace.Home, "", 0, false);

    bool crashAsked;

    /// <summary>
    /// A second after Home's first content, the notice for a crash the last run saved: only while
    /// saving and asking are both on (the engine says so by giving nothing otherwise). It stays
    /// with Report… for the usual time, and is shown once per crash.
    /// </summary>
    DispatcherQueueTimer? crashTimer;

    void AskAboutCrash()
    {
        // Held in a field: a timer nothing refers to can be collected before it fires.
        var timer = crashTimer = ui.CreateTimer();
        timer.Interval = TimeSpan.FromSeconds(1);
        timer.IsRepeating = false;
        timer.Tick += (_, _) =>
        {
            if (KaeriFfiMethods.CrashPendingNotice() is not null)
                ShowToast(InfoBarSeverity.Informational, T.CrashNotice, T.CrashNoticeReport, () => ReportDialog.Show(ctx, Root.XamlRoot, latestCrash: true));
        };
        timer.Start();
    }

    void RememberWhere()
    {
        var page = ContentFrame.CurrentSourcePageType;
        var place = Player.Showing ? CrashPlace.Player
            : page == typeof(HomePage) ? CrashPlace.Home
            : page == typeof(TitlePage) ? CrashPlace.Title
            : page == typeof(SearchPage) ? CrashPlace.Search
            : page == typeof(SettingsPage) ? CrashPlace.Settings
            : CrashPlace.Other;
        var sources = (uint)ctx.Known.Providers.Count(p => !p.IsDemo);
        crashWhere = new(place, Player.Showing ? "playing" : "browsing", sources, ctx.Known.Demo.On);
    }

    void OnFrameNavigated(object sender, NavigationEventArgs e)
    {
#if KAERI_TESTING
        // For Kaeri.Drive: when a navigation is done, and when the new page has been laid out.
        Probe.Mark($"navigated {e.SourcePageType.Name}");
        if (e.Content is FrameworkElement shown)
        {
            void Laid(object s, RoutedEventArgs a)
            {
                shown.Loaded -= Laid;
                Probe.Mark($"loaded {e.SourcePageType.Name}");
            }
            shown.Loaded += Laid;
        }
#endif
        RememberWhere();
        if (framePage != e.Content)
            ctx.Left(framePage);
        framePage = e.Content;
        HoverPreview.Close();
        AppTitleBar.IsBackButtonEnabled = ContentFrame.CanGoBack;
        selecting = true;
        if (e.SourcePageType == typeof(SettingsPage))
            Nav.SelectedItem = e.Parameter as string == "accounts" ? AccountItem : Nav.SettingsItem;
        else if (e.SourcePageType == typeof(FolderPage) || e.SourcePageType == typeof(ServerPage) || e.SourcePageType == typeof(LicensedPage))
            Nav.SelectedItem = Nav.MenuItems.OfType<NavigationViewItem>().FirstOrDefault(i => (string?)i.Tag == "providers");
        else if (TagOf(e.SourcePageType) is { } tag)
            Nav.SelectedItem = Nav.MenuItems.OfType<NavigationViewItem>().FirstOrDefault(i => (string?)i.Tag == tag);
        else
            Nav.SelectedItem = null;
        selecting = false;
        UpdatePartyBar();
        if (!Player.Showing)
            FocusPage();
    }

    /// <summary>Where focus was on each page the frame keeps for Back, so a return lands there and the page does not scroll.</summary>
    readonly System.Runtime.CompilerServices.ConditionalWeakTable<object, WeakReference<Control>> focusKept = [];

    void OnFrameNavigating(object sender, NavigatingCancelEventArgs e)
    {
        if (ContentFrame.Content is Page { NavigationCacheMode: not NavigationCacheMode.Disabled } kept && Content?.XamlRoot is { } root)
        {
            var focused = FocusManager.GetFocusedElement(root) as Control;
            focusKept.AddOrUpdate(kept, new WeakReference<Control>(focused is not null && Within(focused, kept) ? focused : null!));
        }
    }

    static bool Within(DependencyObject at, DependencyObject outer)
    {
        for (DependencyObject? up = at; up is not null; up = VisualTreeHelper.GetParent(up))
        {
            if (up == outer)
                return true;
        }
        return false;
    }

    /// <summary>
    /// After a page change, focus goes to the page's first control and the screen reader hears
    /// its title, instead of staying on the pane item. Typing in the search box is left alone. A
    /// page the frame kept gets focus back where it was, or keeps none, rather than its first
    /// control: that one can be above where the page was scrolled to, and focusing it would scroll.
    /// </summary>
    void FocusPage()
    {
        var shown = ContentFrame.Content;
        var kept = shown is not null && focusKept.TryGetValue(shown, out var weak) ? weak : null;
        DispatcherQueue.TryEnqueue(DispatcherQueuePriority.Low, () =>
        {
            if (Player.Showing || shown != ContentFrame.Content || shown is not UIElement page || Content?.XamlRoot is not { } root)
                return;
            if (FocusManager.GetFocusedElement(root) is DependencyObject at)
            {
                for (DependencyObject? up = at; up is not null; up = VisualTreeHelper.GetParent(up))
                {
                    if (up == SearchBox)
                        return;
                }
            }
            if (Heading(page) is { } heading && FrameworkElementAutomationPeer.FromElement(heading) is { } peer)
                peer.RaiseNotificationEvent(AutomationNotificationKind.ActionCompleted, AutomationNotificationProcessing.MostRecent, heading.Text, "page");
            if (kept?.TryGetTarget(out var again) == true && again.IsLoaded && Within(again, page))
                again.Focus(FocusState.Programmatic);
            else if (kept is null)
                (FocusManager.FindFirstFocusableElement(page) as Control)?.Focus(FocusState.Programmatic);
        });
    }

    static TextBlock? Heading(DependencyObject at)
    {
        if (at is TextBlock text && AutomationProperties.GetHeadingLevel(text) == AutomationHeadingLevel.Level1 && text.Text.Length > 0)
            return text;
        for (var i = 0; i < VisualTreeHelper.GetChildrenCount(at); i++)
        {
            if (Heading(VisualTreeHelper.GetChild(at, i)) is { } found)
                return found;
        }
        return null;
    }

    void OnBackRequested(TitleBar sender, object args) => GoBack();

    void GoBack()
    {
        if (ReaderLayer.Showing)
            ReaderLayer.Close();
        else if (Player.Showing)
            Player.Close();
        else if (ContentFrame.CanGoBack)
            // Back crossfades as the page came in.
            Crossfade.Swap(ContentFrame, () =>
            {
                if (ContentFrame.CanGoBack)
                    ContentFrame.GoBack();
            });
    }

    void OnPaneToggleRequested(TitleBar sender, object args) => Nav.IsPaneOpen = !Nav.IsPaneOpen;

    // Search in the title bar: suggestions from the titles the app already knows, Enter searches.

    void OnSearchTextChanged(AutoSuggestBox sender, AutoSuggestBoxTextChangedEventArgs args)
    {
        if (args.Reason != AutoSuggestionBoxTextChangeReason.UserInput)
            return;
        var text = sender.Text.Trim();
        sender.ItemsSource = text.Length < 2
            ? null
            : ctx.Known.Entries.Values
                .Where(e => e.Title.Contains(text, StringComparison.CurrentCultureIgnoreCase)
                    || e.Secondary?.Contains(text, StringComparison.CurrentCultureIgnoreCase) == true)
                .OrderBy(e => e.Title)
                .Take(6)
                .Select(e => new Suggestion(e.Id, e.Title))
                .ToList();
    }

    sealed record Suggestion(uint Id, string Title)
    {
        public override string ToString() => Title;
    }

    void OnSuggestionChosen(AutoSuggestBox sender, AutoSuggestBoxSuggestionChosenEventArgs args) { }

    void OnSearchSubmitted(AutoSuggestBox sender, AutoSuggestBoxQuerySubmittedEventArgs args)
    {
        if (args.ChosenSuggestion is Suggestion s)
        {
            sender.Text = "";
            OpenTitle(s.Id);
        }
        else if (PageFor("search") == typeof(ReadSearchPage))
        {
            // Reading's Search takes the words when Search is on reading.
            ContentFrame.Navigate(typeof(ReadSearchPage), args.QueryText.Trim(), new SuppressNavigationTransitionInfo());
        }
        else
            ShowSearch(args.QueryText.Trim());
    }

    void OnFocusSearch(Microsoft.UI.Xaml.Input.KeyboardAccelerator sender, Microsoft.UI.Xaml.Input.KeyboardAcceleratorInvokedEventArgs args)
    {
        if (Player.Showing)
            return;
        args.Handled = true;
        // On Settings, Ctrl+F finds a setting, as the Mac's toolbar field does there.
        if (ContentFrame.Content is SettingsPage settings)
            settings.FocusFind();
        else
            SearchBox.Focus(FocusState.Keyboard);
    }

    // The player over the window.

    void PlayerShowing(bool showing)
    {
        // The player's top bar is the title bar while it is up: its title drags the window, and
        // the caption buttons follow its controls.
        if (showing)
            TrailerHub.StopAll();
        SetTitleBar(showing ? Player.DragArea : AppTitleBar);
        AppTitleBar.Visibility = showing ? Visibility.Collapsed : Visibility.Visible;
        Nav.Visibility = showing ? Visibility.Collapsed : Visibility.Visible;
        AppWindow.TitleBar.PreferredHeightOption = TitleBarHeightOption.Tall;
        if (!showing && AppWindow.Presenter.Kind == AppWindowPresenterKind.FullScreen)
            Fullscreen(false);
        if (!showing)
            DispatcherQueue.TryEnqueue(DispatcherQueuePriority.Low, () =>
            {
                if (!Player.RestoreFocus())
                    FocusPage();
            });
    }

    RectInt32? beforeMini;
    /// <summary>How the window was before the mini player: maximised, full screen or neither.</summary>
    bool miniFromMaximised, miniFromFullscreen;
    /// <summary>Where the mini player was last, so it opens there and at that size again.</summary>
    RectInt32? lastMini;

    /// <summary>
    /// The mini player: the window as a small one that stays on top of the others, the video and
    /// its few controls in it, and the way back to the window as it was, maximised and full
    /// screen included. It opens where and at the size it was last left.
    /// </summary>
    void Mini(bool on)
    {
        var now = AppWindow.Presenter.Kind == AppWindowPresenterKind.CompactOverlay;
        if (on == now)
            return;
        if (on)
        {
            miniFromFullscreen = AppWindow.Presenter.Kind == AppWindowPresenterKind.FullScreen;
            if (miniFromFullscreen)
                Fullscreen(false);
            miniFromMaximised = AppWindow.Presenter is OverlappedPresenter { State: OverlappedPresenterState.Maximized };
            // A maximised window keeps its restored place: the size it goes back to when it is
            // made smaller again.
            if (miniFromMaximised && AppWindow.Presenter is OverlappedPresenter maximised)
                maximised.Restore();
            beforeMini = new RectInt32(AppWindow.Position.X, AppWindow.Position.Y, AppWindow.Size.Width, AppWindow.Size.Height);
            AppWindow.SetPresenter(AppWindowPresenterKind.CompactOverlay);
            if (lastMini is { } at)
                AppWindow.MoveAndResize(at);
            else
            {
                var scale = GetDpiForWindow(WinRT.Interop.WindowNative.GetWindowHandle(this)) / 96.0;
                AppWindow.Resize(new SizeInt32((int)(480 * scale), (int)(270 * scale)));
            }
        }
        else
        {
            lastMini = new RectInt32(AppWindow.Position.X, AppWindow.Position.Y, AppWindow.Size.Width, AppWindow.Size.Height);
            AppWindow.SetPresenter(AppWindowPresenterKind.Default);
            if (beforeMini is { } was)
                AppWindow.MoveAndResize(was);
            beforeMini = null;
            if (miniFromMaximised && AppWindow.Presenter is OverlappedPresenter overlapped)
                overlapped.Maximize();
            else if (miniFromFullscreen && !Player.Closing)
                Fullscreen(true);
            miniFromMaximised = miniFromFullscreen = false;
        }
    }

    void Fullscreen(bool? on)
    {
        var now = AppWindow.Presenter.Kind == AppWindowPresenterKind.FullScreen;
        var want = on ?? !now;
        if (want == now)
            return;
        AppWindow.SetPresenter(want ? AppWindowPresenterKind.FullScreen : AppWindowPresenterKind.Default);
        Player.ShowFullscreen(want);
    }

    // Closing: downloads and seeding carry on only if the viewer says so.

    void OnClosing(AppWindow sender, AppWindowClosingEventArgs args)
    {
        KeepPlaceNow();
        if (closing)
            return;
        if (AskLeavingParty())
        {
            args.Cancel = true;
            return;
        }
        if (HoldForDownloads())
            args.Cancel = true;
    }

    /// <summary>
    /// Whether running downloads keep the window from closing now: asked about, or carried on
    /// hidden, as Settings › Downloads says.
    /// </summary>
    bool HoldForDownloads()
    {
        var (downloading, seeding) = ctx.Known.Running();
        if (downloading + seeding == 0 || askedClose)
            return false;
        switch (WhenClosing)
        {
            case CloseChoice.Ask:
                AskBeforeClosing();
                return true;
            case CloseChoice.KeepGoing:
                KeepGoingHidden();
                return true;
            default:
                return false;
        }
    }

    async void AskBeforeClosing()
    {
        var (downloading, seeding) = ctx.Known.Running();
        if (downloading + seeding == 0)
            (downloading, seeding) = options.AskClose ? (2, 1) : (0, 0);
        askedClose = true;
        var dontAsk = new CheckBox { Content = T.CloseDontAsk };
        var body = new StackPanel { Spacing = 16 };
        body.Children.Add(new TextBlock { Text = CloseLine(downloading, seeding), TextWrapping = TextWrapping.WrapWholeWords });
        body.Children.Add(dontAsk);
        var dialog = new ContentDialog
        {
            XamlRoot = Root.XamlRoot,
            Title = T.CloseTitle,
            Content = body,
            PrimaryButtonText = T.CloseKeepGoing,
            SecondaryButtonText = T.CloseStop,
            CloseButtonText = T.ActionCancel,
            DefaultButton = ContentDialogButton.Primary,
        };
        // The question comes first: a dialog already up closes for it.
        var answer = await Dialogs.Show(dialog, first: true);
        askedClose = false;
        if (answer == ContentDialogResult.None)
            return;
        if (dontAsk.IsChecked == true)
            WhenClosing = answer == ContentDialogResult.Primary ? CloseChoice.KeepGoing : CloseChoice.Stop;
        if (answer == ContentDialogResult.Primary)
            KeepGoingHidden();
        else
            CloseNow();
    }

    static string CloseLine(int downloading, int seeding)
    {
        var lines = new List<string>();
        if (downloading > 0)
            lines.Add(T.CloseDownloading(downloading));
        if (seeding > 0)
            lines.Add(T.CloseSeeding(seeding));
        lines.Add(T.CloseAsk);
        return string.Join(" ", lines);
    }

    DispatcherQueueTimer? keepGoing;

    /// <summary>
    /// Keep going: the window goes and the notification area icon comes, the first time with a
    /// notification saying where Kaeri went. The engine stays until nothing is left running,
    /// or until the icon's Quit; the icon's Open brings the window back. Downloads paused from
    /// the icon keep it there too, to be resumed from the same menu.
    /// </summary>
    void KeepGoingHidden()
    {
        if (Player.Showing)
            Player.Close();
        AppWindow.Hide();
        tray ??= new Tray(WinRT.Interop.WindowNative.GetWindowHandle(this), Path.Combine(AppContext.BaseDirectory, "Assets", "kaeri.ico"))
        {
            Open = () => ui.TryEnqueue(ShowAgain),
            PauseAll = () => ui.TryEnqueue(() => trayPaused.UnionWith(ctx.PauseAllDownloads())),
            ResumeAll = () => ui.TryEnqueue(ctx.ResumeAllDownloads),
            Quit = () => ui.TryEnqueue(CloseNow),
            AnyRunning = () => ctx.Known.Downloads.Any(r => r.State is DownloadState.Active or DownloadState.Queued),
            AnyPaused = () => ctx.Known.Downloads.Any(r => r.State is DownloadState.Paused),
        };
        var told = Path.Combine(dirs.Config, "windows-told-tray");
        tray.Show(tell: !File.Exists(told));
        // Told again next time when it can't be kept: harmless.
        SmallFiles.Write(told, "");
        keepGoing?.Stop();
        keepGoing = ui.CreateTimer();
        keepGoing.Interval = TimeSpan.FromSeconds(5);
        keepGoing.Tick += (_, _) =>
        {
            var (d, s) = ctx.Known.Running();
            // Paused from the icon: still there to resume, so Kaeri stays for them.
            trayPaused.IntersectWith(ctx.Known.Downloads.Where(r => r.State is DownloadState.Paused or DownloadState.Active or DownloadState.Queued).Select(r => r.Id));
            if (d + s == 0 && !ctx.Known.Downloads.Any(r => r.State is DownloadState.Paused && trayPaused.Contains(r.Id)))
                CloseNow();
        };
        keepGoing.Start();
    }

    /// <summary>The downloads the icon's Pause all paused, which keep Kaeri in the notification area.</summary>
    readonly HashSet<long> trayPaused = [];

    // Windows notifications: new episodes from the engine's check, finished downloads, and the
    // engine's own notices while the window is not in front.

    Toasts? toasts;
    DownloadRow[]? downloadsSeen;
    DispatcherQueueTimer? newEpisodes;
    bool inFront = true;

    /// <summary>The window is the active one.</summary>
    internal bool InFront => inFront;

    /// <summary>The player is up over the pages.</summary>
    internal bool PlayerUp => Player.Showing;

    /// <summary>How often the open app asks the engine what aired; the engine asks AniList at most every five minutes.</summary>
    static readonly TimeSpan CheckEvery = TimeSpan.FromMinutes(15);

    void StartNotifications(Art art)
    {
        Activated += (_, e) =>
        {
            inFront = e.WindowActivationState != WindowActivationState.Deactivated;
            // Back in front: the keys go to the dialog that is up, not the page under it.
            if (inFront)
                DispatcherQueue.TryEnqueue(DispatcherQueuePriority.Low, Dialogs.Refocus);
            // The system's caption style has no change event: read it again on coming back.
            if (inFront)
                Kaeri.Windows.Player.SubtitleStyling.Refresh();
            // A trailer plays only while the window is in front.
            if (!inFront)
                TrailerHub.StopAll();
            else
                // Back from a licensed service: the engine says whether it was long enough to have watched an episode.
                ctx.Send(new Command.Licensed(new LicensedCommand.Returned()));
        };
        var asked = options.Show == "toast";
        if (options.Fixture && !asked)
            return;
        toasts = Toasts.Start(Path.Combine(dirs.Cache, "notification-art"), art, (action, show, episode) => ui.TryEnqueue(() => FromNotification(action, show, episode)));
#if KAERI_TESTING
        if (asked)
        {
            ui.TryEnqueue(DispatcherQueuePriority.Low, async () =>
            {
                await Task.Delay(2500);
                var banner = ctx.Known.Entry(900001)?.BannerUrl ?? ctx.Known.Entry(900001)?.CoverUrl;
                foreach (var notice in Notices.Aired([new NewEpisode(900001, ctx.Known.Entry(900001)?.Title ?? "Stardust Wayfarer", 8, 0, NextForYou: 7, BannerUrl: banner)]))
                    toasts?.Post(notice);
            });
            return;
        }
#endif
        if (options.Fixture)
            return;
        // Soon after launch, then while the app runs, window or notification area.
        newEpisodes = ui.CreateTimer();
        newEpisodes.Interval = TimeSpan.FromSeconds(20);
        newEpisodes.Tick += (_, _) =>
        {
            newEpisodes.Interval = CheckEvery;
            ctx.Send(new Command.CheckNewEpisodes());
        };
        newEpisodes.Start();
    }

    /// <summary>A notification clicked: the window to the front, then the episode playing or the show open.</summary>
    void FromNotification(string action, uint show, string? episode)
    {
        if (!AppWindow.IsVisible)
            ShowAgain();
        else
            Activate();
        if (show == 0)
            return;
        if (action == "play" && episode is not null)
            ctx.Play(show, episode);
        else
            OpenTitle(show);
    }

    void Notified(string title, string body)
    {
        if ((inFront && AppWindow.IsVisible) || toasts is null)
            ShowToast(InfoBarSeverity.Informational, T.ToastTitled(title, body));
        else
            toasts.Post(new SystemNotice($"engine-{title}", title, body, 0, null, null));
    }

    // kaeri:// links, as the Mac opens them (Link).

    /// <summary>Open link or file: the dialog that asks for a magnet link or a torrent file, then confirms what the engine guessed.</summary>
    internal async void ShowOpenLink() => await new OpenedDialog(ctx, Root.XamlRoot, null).ShowAsync();

    void OnKeyOpenLink(Microsoft.UI.Xaml.Input.KeyboardAccelerator sender, Microsoft.UI.Xaml.Input.KeyboardAcceleratorInvokedEventArgs args)
    {
        if (Player.Showing || Dialogs.Open)
            return;
        args.Handled = true;
        ShowOpenLink();
    }

    /// <summary>The window that says the engine stopped, kept while it is up.</summary>
    static StartFailedWindow? stoppedWindow;

    JoinStoreDialog? joining;
    string? fixtureInvite;
    WelcomeDialog? welcoming;

    /// <summary>
    /// The window to the front for another launch: back from the notification area, or up from
    /// the taskbar, and given the foreground the launch handed over.
    /// </summary>
    void Forward()
    {
        if (!AppWindow.IsVisible)
        {
            ShowAgain();
            return;
        }
        if (AppWindow.Presenter is OverlappedPresenter { State: OverlappedPresenterState.Minimized } overlapped)
            overlapped.Restore();
        Activate();
        SetForegroundWindow(WinRT.Interop.WindowNative.GetWindowHandle(this));
    }

    /// <summary>
    /// A link the app was opened with. An invite fills in the welcome or the join dialog when one
    /// is up, and opens the join dialog otherwise, over any other dialog; the widgets' links go
    /// where the Mac takes them; any other link says it can't be opened.
    /// </summary>
    internal async void OpenLink(string text)
    {
        var link = Link.Parse(text);
        if (link is Link.Opened opened)
        {
            if (Player.Showing)
                Player.Close();
            if (CloseDialogs())
                await Task.Delay(250);
            await new OpenedDialog(ctx, Root.XamlRoot, opened.Input).ShowAsync();
            return;
        }
        if (link is Link.Party party)
        {
            OpenPartyInvite(party.Text);
            return;
        }
        if (link is Link.Invite invite)
        {
            if (welcoming is { } welcome)
                welcome.Fill(invite.Https);
            else if (joining is { } join)
                join.Fill(invite.Https);
            else
            {
                if (Player.Showing)
                    Player.Close();
                if (CloseDialogs())
                    await Task.Delay(250);
                ShowJoinStore(invite.Https);
            }
            return;
        }
        switch (link)
        {
            case Link.Title title:
                OpenTitle(title.Id);
                break;
            case Link.Play play:
                OpenTitle(play.Id);
                ctx.Play(play.Id, play.Episode.ToString(CultureInfo.InvariantCulture));
                break;
            case Link.Calendar:
                Go("calendar");
                break;
            case Link.Accounts:
                if (Player.Showing)
                    Player.Close();
                OpenSettings("accounts");
                break;
            case Link.Auth auth:
                ctx.Known.SignInStarted(auth.Tracker, ctx.Send(new Command.FinishSignIn(auth.Tracker, auth.Callback)));
                if (Player.Showing)
                    Player.Close();
                OpenSettings("accounts");
                break;
            case Link.Download download:
                // A finished download plays, from its title's page; otherwise Downloads, where it is.
                if (ctx.Known.Downloads.FirstOrDefault(r => r.Id == download.Id && r.State == DownloadState.Done) is { Path: { } path } row)
                {
                    OpenTitle(row.AnilistId);
                    ctx.Send(new Command.PlayLocal(row.AnilistId, row.Episode, row.Title, path));
                }
                else
                    Go("downloads");
                break;
            default:
                KaeriFfiMethods.LogLine(FfiLogLevel.Info, "windows", "a link this app does not open");
                ShowToast(InfoBarSeverity.Warning, T.LinkCantOpen);
                break;
        }
    }

    /// <summary>Closes the dialog that is up, if one is: whether one was.</summary>
    bool CloseDialogs()
    {
        var closed = InfoCard.HideOpen();
        // Closed even when it would hold itself open, as a sign-in's wait does.
        if (Dialogs.Open)
        {
            Dialogs.HideOpen();
            closed = true;
        }
        foreach (var popup in VisualTreeHelper.GetOpenPopupsForXamlRoot(Root.XamlRoot))
            if (popup.Child is ContentDialog dialog)
            {
                dialog.Hide();
                closed = true;
            }
        return closed;
    }

    [LibraryImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static partial bool SetForegroundWindow(nint hwnd);

    /// <summary>The window back from the notification area; the icon goes with the window's return.</summary>
    void ShowAgain()
    {
        keepGoing?.Stop();
        trayPaused.Clear();
        tray?.Hide();
        AppWindow.Show(true);
        Activate();
    }

    internal void CloseNow()
    {
        closing = true;
        Close();
    }

    /// <summary>
    /// What closing the window does while downloads run: Settings › Downloads, "When the window
    /// closes with downloads running", kept beside the engine's own config. "Don't ask again" in
    /// the dialog keeps the answer given. The old flag meant closing without asking, which stops.
    /// </summary>
    internal CloseChoice WhenClosing
    {
        get
        {
            if (SmallFiles.Read(WhenClosingPath) is { } kept)
                return kept switch
                {
                    "keep-going" => CloseChoice.KeepGoing,
                    "stop" => CloseChoice.Stop,
                    _ => CloseChoice.Ask,
                };
            return File.Exists(Path.Combine(dirs.Config, "windows-close-without-asking")) ? CloseChoice.Stop : CloseChoice.Ask;
        }
        set
        {
            // A choice the disk won't keep is asked again next time.
            SmallFiles.Write(WhenClosingPath, value switch
            {
                CloseChoice.KeepGoing => "keep-going",
                CloseChoice.Stop => "stop",
                _ => "ask",
            });
            SmallFiles.Delete(Path.Combine(dirs.Config, "windows-close-without-asking"));
        }
    }

    string WhenClosingPath => Path.Combine(dirs.Config, "windows-when-closing");

    void Shutdown()
    {
        closing = true;
        Player.Quiet();
        keepGoing?.Stop();
        newEpisodes?.Stop();
        toasts?.Stop();
        tray?.Dispose();
        network.Dispose();
        device.Dispose();
        StopTv();
        taskbar?.Clear();
        // The engine first: on its way out it stops what plays and waits for the player to say
        // where it ended, so the player must still be there to hear the stop.
        var clock = System.Diagnostics.Stopwatch.StartNew();
        session.Dispose();
        var engineMs = clock.ElapsedMilliseconds;
        player?.Shutdown();
        player?.Dispose();
        // A slow close, with where the time went, beside the crash log: the engine's log has
        // closed by now.
        if (clock.ElapsedMilliseconds > 500)
        {
            try
            {
                var dir = Path.Combine(Path.GetTempPath(), "kaeri-windows");
                Directory.CreateDirectory(dir);
                File.AppendAllText(Path.Combine(dir, "closing.txt"), $"{DateTime.Now:O} closing took {clock.ElapsedMilliseconds} ms: the engine {engineMs} ms, the player {clock.ElapsedMilliseconds - engineMs} ms" + Environment.NewLine);
            }
            catch (IOException)
            {
            }
        }
    }

    // The window's keys, after the brief's table. The player's own keys are on the player; these
    // stand aside while it is up, and while a text box has the focus for the keys it uses.

    void Keys()
    {
        // Ctrl+1 to Ctrl+9 in the sidebar's order, as Command+1 to 9 on the Mac.
        // The places shown change with what Kaeri is for, so the key looks the place up when pressed.
        for (var i = 0; i < 9; i++)
        {
            var n = i;
            Key(VirtualKey.Number1 + i, VirtualKeyModifiers.Control, () =>
            {
                if (Places().ElementAtOrDefault(n) is { Tag: string tag })
                    Go(tag);
            });
        }
        Key(VirtualKey.Left, VirtualKeyModifiers.Menu, GoBack, inPlayer: true);
        Key((VirtualKey)0xBC, VirtualKeyModifiers.Control, () => Navigate("settings"));
        Key(VirtualKey.R, VirtualKeyModifiers.Control, SyncNow);
        Key(VirtualKey.F5, VirtualKeyModifiers.None, SyncNow);
        Key(VirtualKey.M, VirtualKeyModifiers.Control | VirtualKeyModifiers.Shift, () => (ContentFrame.Content as IPageKeys)?.MarkWatched(), typing: false);
        Key(VirtualKey.Delete, VirtualKeyModifiers.None, () => (ContentFrame.Content as IPageKeys)?.Delete(), typing: false);
        // Ctrl+/ is the slash key on US layouts only; F1 opens the list on every layout.
        Key((VirtualKey)0xBF, VirtualKeyModifiers.Control, ShowShortcuts);
        Key(VirtualKey.F1, VirtualKeyModifiers.None, ShowShortcuts, inPlayer: true);
        Key(VirtualKey.L, VirtualKeyModifiers.Control | VirtualKeyModifiers.Shift, ShowMessages);
        // The mouse's back button, as Alt+Left.
        Root.AddHandler(UIElement.PointerPressedEvent, new PointerEventHandler((_, e) =>
        {
            if (e.GetCurrentPoint(Root).Properties.IsXButton1Pressed)
            {
                GoBack();
                e.Handled = true;
            }
        }), true);
    }

    void Key(VirtualKey key, VirtualKeyModifiers modifiers, Action act, bool inPlayer = false, bool typing = true)
    {
        var accelerator = new KeyboardAccelerator { Key = key, Modifiers = modifiers };
        accelerator.Invoked += (_, args) =>
        {
            if ((Player.Showing || ReaderLayer.Showing) && !inPlayer)
                return;
            if (!typing && FocusManager.GetFocusedElement(Root.XamlRoot) is TextBox or AutoSuggestBox or PasswordBox)
                return;
            args.Handled = true;
            act();
        };
        Root.KeyboardAccelerators.Add(accelerator);
    }

    void SyncNow()
    {
        if (ctx.Known.Anilist is null)
            return;
        ctx.Send(new Command.SyncNow());
        ShowToast(InfoBarSeverity.Informational, T.AccountsSyncRunningWith(Tracker));
    }

    /// <summary>Ctrl+/: every key the window and the player answer, in a dialog.</summary>
    internal async void ShowShortcuts()
    {
        if (InfoCard.IsOpen)
            return;
        var grid = new Grid { ColumnSpacing = 24, RowSpacing = 6 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var row = 0;
        void Section(string name)
        {
            grid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
            var heading = new TextBlock { Text = name, Style = Ui.Res<Style>("BodyStrongTextBlockStyle"), Margin = new Thickness(0, row == 0 ? 0 : 12, 0, 2) };
            AutomationProperties.SetHeadingLevel(heading, AutomationHeadingLevel.Level2);
            Grid.SetRow(heading, row++);
            Grid.SetColumnSpan(heading, 2);
            grid.Children.Add(heading);
        }
        void Line(string what, string keys)
        {
            grid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
            var name = new TextBlock { Text = what };
            var key = new TextBlock { Text = keys, Style = Ui.Res<Style>("HazeBodyStyle") };
            Grid.SetRow(name, row);
            Grid.SetRow(key, row++);
            Grid.SetColumn(key, 1);
            grid.Children.Add(name);
            grid.Children.Add(key);
        }
        Section(T.ShortcutsWindow);
        Line(T.ShortcutsSections, T.ShortcutsKeysRange("Ctrl+1", "Ctrl+9"));
        Line(T.NavSearch, "Ctrl+F, Ctrl+E");
        Line(T.ActionBack, T.ShortcutsKeysBack("Alt+Left"));
        Line(T.NavSettings, "Ctrl+,");
        Line(T.AccountsSyncNow, "Ctrl+R, F5");
        Line(T.EpisodeMarkWatched, "Ctrl+Shift+M");
        Line(T.ContinueHide, "Delete");
        Line(T.ShortcutsContextMenu, T.ShortcutsKeysMenu("Shift+F10"));
        Line(T.ShortcutsTitle, "Ctrl+/");
        Line(T.MessagesTitle, "Ctrl+Shift+L");
        Section(T.SettingsGroupPlayer);
        Line(T.ShortcutsPlayPause, "Space, K");
        Line(T.ShortcutsSeek, "Left, Right");
        Line(T.ShortcutsSeekTen, "J, L");
        Line(T.PlayerVolume, "Up, Down");
        Line(T.ShortcutsMute, "M");
        Line(T.PlayerFullscreen, "F, F11");
        Line(T.ShortcutsSkipUndo, "S, U");
        Line(T.ShortcutsNextPrevious, "N, Shift+N");
        Line(T.PlayerEpisodes, "E");
        Line(T.PlayerSubtitlesAndAudio, "T");
        Line(T.PlayerStats, "I");
        Line(T.ShortcutsSpeedReset, "[, ], Backspace");
        Line(T.PlayerMiniPlayer, "P");
        Line(T.ShortcutsCornerCard, "Enter");
        Line(T.ShortcutsClosePlayer, "Esc, Alt+Left");
        // Nothing to lose, so a click outside closes it too (Light dismiss).
        await InfoCard.Show(
            Root.XamlRoot,
            Ui.Text(T.ShortcutsTitle, "SubtitleTextBlockStyle"),
            new ScrollViewer { Content = grid, MaxHeight = 480, VerticalScrollBarVisibility = ScrollBarVisibility.Auto, Padding = new Thickness(0, 0, 16, 0) },
            548,
            640);
    }
}

/// <summary>What closing the window does while downloads run.</summary>
enum CloseChoice
{
    Ask,
    KeepGoing,
    Stop,
}
