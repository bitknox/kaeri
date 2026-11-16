using System.Runtime.InteropServices;
using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Kaeri.Windows.Casting;
using Kaeri.Windows.Pages;
using Kaeri.Windows.Tv;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Windows.Networking.Connectivity;

namespace Kaeri.Windows;

/// <summary>
/// Play on TV, the window's side: the engine's sender, browsing with Windows' DNS-SD while the
/// window is up, the title bar's cast button, the cast's mini and controller on the page pane,
/// the dialog that comes up whenever the remote has a sheet to show, and trying the TV again as
/// the network or the window comes back.
/// </summary>
public sealed partial class MainWindow
{
    readonly TvRemote tv = new();
    FfiTvSender? tvSender;
    CastButton? castButton;
    CastLayer? castLayer;
    bool openingTv, browsingForButton;

    /// <summary>The sender, when the engine was built with the remote; without one Play on TV stays out of sight.</summary>
    void StartTv(FfiEngine engine)
    {
        try
        {
            tvSender = engine.TvSender(Sync.SyncDialogs.KeptName(dirs.Config) ?? TvWords.Device, $"kaeri-windows/{Updater.Current}");
        }
        catch (FfiException e)
        {
            KaeriFfiMethods.LogLine(FfiLogLevel.Info, "tv", $"no Play on TV: {e.Message}");
            return;
        }
        FfiTvBrowser browser = new DnssdBrowser();
        FfiCastBrowser casts = new CastBrowser();
#if KAERI_TESTING
        // A fixture run never looks on the real network: the fake TVs beside the engine, or none.
        if (options.Fixture)
        {
            browser = FixtureTvs(engine);
            casts = FixtureCasts(engine);
        }
#endif
        tv.HandedOff = () =>
        {
            if (Player.Showing)
                Player.Close();
        };
        // Windows' firewall asks the first time the cast's relay listens; once a cast has played,
        // the picker stops saying so.
        var served = Path.Combine(dirs.Config, "windows-cast-served");
        tv.CastServed = File.Exists(served);
#if KAERI_TESTING
        if (options.Show is not null && options.Show.StartsWith("cast-", StringComparison.Ordinal))
            tv.CastServed = CastSceneName == "cast-picker-allowed";
#endif
        tv.Served = () =>
        {
            try
            {
                Directory.CreateDirectory(dirs.Config);
                File.WriteAllText(served, "");
            }
            catch (Exception e) when (e is IOException or UnauthorizedAccessException)
            {
                KaeriFfiMethods.LogLine(FfiLogLevel.Warn, "cast", $"the cast mark was not kept: {e.Message}");
            }
        };
        // The cast session: plays go to the TV it holds; the player here hands its episode over.
        tv.PlayOnTv = play => ctx.Play(play.Id, play.Episode);
        tv.PlayingHere = () => Player.Showing ? Player.PlayingOnTv() : null;
        castButton = new CastButton(ctx, tv, () => castLayer?.Expand());
        castLayer = new CastLayer(ctx, tv, PaneGrid, new CastHost
        {
            OpenShow = OpenTitle,
            PlayHere = play => ctx.Play(play.Id, play.Episode),
            ChooseSource = ChooseCastSource,
            // Over the mini while it shows; at the foot, over the remote, while that is open.
            MiniShown = shown => Toast.Margin = new Thickness(24, 0, 24, shown ? 92 : 16),
            Remembered = RememberedCast,
        });
        AppTitleBar.RightHeader = TitleBarEnd();
        // The TV went away, was stopped there or was taken: the mini goes, and the notice offers
        // to carry on here.
        tv.TakenOver = CastEnded;
        tv.SessionEnded = CastEnded;
        tv.Changed += TvChanged;
        tv.Attach(tvSender, browser, casts);
        // The button knows whether TVs exist, and a cast left running is joined again, only by
        // browsing: from launch while the window is up.
        BrowseForButton(true);
        Root.PreviewKeyDown += CastVolumeKeys;
        // The link waits between tries while the TV is away; a network back, or the window
        // brought to the front, tries now.
        NetworkInformation.NetworkStatusChanged += TvNetworkChanged;
        Activated += (_, e) =>
        {
            if (e.WindowActivationState != WindowActivationState.Deactivated)
                tv.RetryNow();
        };
    }

    void TvNetworkChanged(object sender) => ui.TryEnqueue(tv.RetryNow);

    void StopTv()
    {
        NetworkInformation.NetworkStatusChanged -= TvNetworkChanged;
        if (tvSender is null)
            return;
        tv.Changed -= TvChanged;
        // Quitting lets the TV go: what plays there stops.
        if (tv.SessionHeld)
            tv.DisconnectCast();
        BrowseForButton(false);
        tv.Disconnect();
#if KAERI_TESTING
        foreach (var fake in fakeTvs)
            fake.Stop();
        foreach (var fake in fakeCasts)
            fake.Stop();
        StopSilentTv();
#endif
    }

    /// <summary>The toast that offered Play here after a cast ended, while it may still show.</summary>
    int? castEndToast;
    (string Text, DateTimeOffset At)? lastCastEnd;

    /// <summary>
    /// A cast ended from outside: with the remote open it says so in place, and the message is
    /// kept; otherwise the notice, 8 s with Play here, takes the mini's place.
    /// </summary>
    void CastEnded(TvPlay play, Msg notice)
    {
        var text = notice.Text();
        // The card and the session both tell of a TV taken: one notice.
        if (lastCastEnd is { } last && last.Text == text && DateTimeOffset.Now - last.At < TimeSpan.FromSeconds(3))
            return;
        lastCastEnd = (text, DateTimeOffset.Now);
        if (castLayer?.Expanded == true)
        {
            ctx.Known.Messages.Add(text, false, DateTimeOffset.Now, false);
            castLayer.Refresh();
            return;
        }
        ShowToast(InfoBarSeverity.Informational, text, T.CastPlayHere, () => ctx.Play(play.Id, play.Episode), hold: NoticeTime.Timed(text, screenReader: false));
        castEndToast = toastShown;
        castLayer?.Refresh();
    }

    /// <summary>Closes the notice now, fading out, unless a newer one comes meanwhile.</summary>
    void CloseToast()
    {
        var shown = ++toastShown;
        Motion.FadeOutThen(Toast, () =>
        {
            if (shown != toastShown)
                return;
            Toast.IsOpen = false;
            Toast.Opacity = 1;
        });
    }

    void TvChanged()
    {
        castButton?.Update();
        castLayer?.Refresh();
        // A new cast on its way: the offer to play the last one here goes.
        if (castEndToast is { } shown && shown == toastShown && Toast.IsOpen && tv.SessionLoaded && tv.Session?.State != FfiCastSessionState.Error)
        {
            castEndToast = null;
            CloseToast();
        }
        RememberCast();
        ToastCastNote();
        // While this PC relays the stream, it doesn't sleep.
        KeepAwake(tv.CastRelaying || (tv.SessionLoaded && tv.Session?.Item is not null));
        // While a TV plays, the media keys and Windows' media controls drive it: the cast first,
        // as the TV held gets every play.
        var cast = tv.SessionLoaded ? tv.Drawn() : null;
        media?.Cast(cast, T.CastNotificationLine(tv.SessionName), cast?.Image ?? (cast?.Id is { } castId ? PictureOf(castId) : null), DriveCast);
        var now = tv.Connected is null ? null : tv.Now;
        media?.Tv(now, now is null ? 0 : tv.Playhead() ?? now.Position, tv.Live, Picture(now), c => _ = tv.Send(c));
        if (tv.Sheet is not null && TvDialog.Showing is null && !openingTv)
            OpenTvDialog();
    }

    string? castNoteShown;

    /// <summary>
    /// The cast's note, from the held TV or a cast from Play on TV, as a toast each time it comes:
    /// the TV's own player took over, so the notice says so each cast it happens, and Messages
    /// keeps it.
    /// </summary>
    void ToastCastNote()
    {
        var note = tv.CastNote?.Text();
        if (note == castNoteShown)
            return;
        castNoteShown = note;
        if (!string.IsNullOrEmpty(note))
            ShowToast(InfoBarSeverity.Informational, note);
    }

    bool keptAwake;

    /// <summary>
    /// While this PC sends a TV the episode, Windows doesn't sleep: that would stop the cast. The
    /// display may still turn off. Always on the UI thread, which the request belongs to.
    /// </summary>
    void KeepAwake(bool awake)
    {
        if (awake == keptAwake)
            return;
        keptAwake = awake;
        const uint continuous = 0x80000000, system = 0x00000001;
        SetThreadExecutionState(awake ? continuous | system : continuous);
    }

    [LibraryImport("kernel32.dll")]
    private static partial uint SetThreadExecutionState(uint flags);

    async void OpenTvDialog()
    {
        openingTv = true;
        try
        {
            if (CloseDialogs())
                await Task.Delay(250);
            if (tv.Sheet is not null)
                await TvDialog.Show(ctx, Root.XamlRoot);
        }
        catch (COMException)
        {
            // Another dialog came up first.
            tv.SheetClosed();
        }
        finally
        {
            openingTv = false;
        }
    }

    /// <summary>The show's banner or cover for the media overlay, when the catalogue knows the show.</summary>
    string? Picture(FfiNowCard? now) => now is null ? null : PictureOf(now.Id);

    string? PictureOf(uint id) => ctx.Known.Entry(id) is not { } entry ? null : entry.BannerUrl ?? entry.CoverUrl;

    /// <summary>The media keys and the overlay on the held TV: play and pause and seeks are shown at once, as the mini's are.</summary>
    void DriveCast(FfiTvCommand command)
    {
        switch (command)
        {
            case FfiTvCommand.TogglePause:
                tv.SessionTogglePause();
                break;
            case FfiTvCommand.Seek seek:
                tv.SessionSeekBy(seek.By);
                break;
            case FfiTvCommand.SeekTo to:
                tv.SessionSeekTo(to.Position);
                break;
            default:
                tv.SessionCommand(command);
                break;
        }
        castLayer?.Refresh();
    }

    /// <summary>
    /// The keyboard's volume keys while a TV is held: the TV's volume moves too. Windows still
    /// moves this PC's own, as it does for every app.
    /// </summary>
    // The keyboard's volume keys, which VirtualKey has no names for.
    const global::Windows.System.VirtualKey VolumeMuteKey = (global::Windows.System.VirtualKey)0xAD,
        VolumeDownKey = (global::Windows.System.VirtualKey)0xAE, VolumeUpKey = (global::Windows.System.VirtualKey)0xAF;

    void CastVolumeKeys(object sender, Microsoft.UI.Xaml.Input.KeyRoutedEventArgs e)
    {
        if (!tv.SessionHeld)
            return;
        switch (e.Key)
        {
            case VolumeUpKey:
                tv.SessionCommand(new FfiTvCommand.Volume(5));
                break;
            case VolumeDownKey:
                tv.SessionCommand(new FfiTvCommand.Volume(-5));
                break;
            case VolumeMuteKey:
                tv.SessionCommand(new FfiTvCommand.Mute(!(tv.Session?.Muted ?? false)));
                break;
        }
    }

    /// <summary>The title bar's end: the cast button in a slot of its own, then where sync stands and the private badge.</summary>
    StackPanel TitleBarEnd()
    {
        // The status and the private badge are there from the start; the cast button goes first.
        AppTitleBar.RightHeader = null;
        titleEnd.Children.Insert(0, castButton!);
        return titleEnd;
    }

    /// <summary>Browsing for TVs from launch while the window is up; off while it sits in the notification area.</summary>
    void BrowseForButton(bool on)
    {
        if (on == browsingForButton)
            return;
        browsingForButton = on;
        if (on)
            tv.StartBrowsing();
        else
            tv.StopBrowsing();
    }

    /// <summary>The episode the TV couldn't play: its sources, here.</summary>
    async void ChooseCastSource(TvPlay play)
    {
        try
        {
            await PartySourceDialog.ForEpisode(ctx, Root.XamlRoot, play.Id, play.Episode).ShowAsync();
        }
        catch (COMException)
        {
            // Another dialog came up first.
        }
    }

    string CastLastPath => Path.Combine(dirs.Config, "windows-cast-last");
    uint? rememberedId;

    /// <summary>The show on the TV, kept for the next launch: the mini names it while the cast is joined again.</summary>
    void RememberCast()
    {
        if (tv.Session?.Item is not { Id: { } id } item || id == rememberedId)
            return;
        rememberedId = id;
        SmallFiles.Write(CastLastPath, $"{id}\t{item.Title}");
    }

    (uint Id, string Title)? RememberedCast()
    {
        if (SmallFiles.Read(CastLastPath) is not { } kept || kept.Split('\t', 2) is not [var id, var title] || !uint.TryParse(id, out var known))
            return null;
        return (known, title);
    }

#if KAERI_TESTING
    readonly List<FfiFakeTv> fakeTvs = [];
    FixtureTvBrowser? fixtureTvs;

    /// <summary>
    /// The fixture run's TVs: two made-up ones on the loopback interface, each a real remote
    /// server with a scripted player (<c>StartFakeTv</c>), found as DNS-SD would find them. For
    /// tv-slow, a third whose address is a loopback listener that takes the connection and never
    /// answers, so nothing leaves the machine; for tv-empty and tv-looking, a network where no TV
    /// answers.
    /// </summary>
    FfiTvBrowser FixtureTvs(FfiEngine engine)
    {
        try
        {
            // A cast scene's Living room has Chromecast built in, so only Bedroom runs the app.
            if (CastScene)
            {
                fakeTvs.Add(engine.StartFakeTv("Bedroom"));
                fixtureTvs = new FixtureTvBrowser(fakeTvs.ToArray(), null, 0, silent: options.Show == "cast-none-found", apart: true);
                return fixtureTvs;
            }
            fakeTvs.Add(engine.StartFakeTv("Living room"));
            fakeTvs.Add(engine.StartFakeTv("Bedroom"));
            FfiFakeTv? slow = options.Show == "tv-slow" ? engine.StartFakeTv("Kitchen") : null;
            if (slow is not null)
                fakeTvs.Add(slow);
            var slowPort = slow is null ? (ushort)0 : StartSilentTv();
            fixtureTvs = new FixtureTvBrowser(fakeTvs.Take(2).ToArray(), slow, slowPort, silent: options.Show is "tv-empty" or "tv-looking");
        }
        catch (FfiException e)
        {
            KaeriFfiMethods.LogLine(FfiLogLevel.Warn, "tv", $"the fake TVs did not start: {e.Message}");
            fixtureTvs = new FixtureTvBrowser([], null, 0, silent: true);
        }
        return fixtureTvs;
    }

    bool CastScene => options.Show?.StartsWith("cast-", StringComparison.Ordinal) == true;

    /// <summary>The cast scene without its -reduced, which runs it with animation effects off.</summary>
    string CastSceneName => options.Show is { } show && show.EndsWith("-reduced", StringComparison.Ordinal) ? show[..^"-reduced".Length] : options.Show ?? "";

    readonly List<FfiFakeCast> fakeCasts = [];
    FixtureCastBrowser? fixtureCasts;

    /// <summary>
    /// The fixture run's TVs with Chromecast built in: receivers on the loopback interface
    /// (<c>StartFakeCast</c>), found as a <c>_googlecast._tcp</c> browse would find them. Living
    /// room has fixed volume and Kitchen display a level; each scene sets how Living room behaves.
    /// Only a cast scene has any, so the other scenes' pickers stay as they were.
    /// </summary>
    FfiCastBrowser FixtureCasts(FfiEngine engine)
    {
        var scene = CastSceneName;
        fixtureCasts = new FixtureCastBrowser(fakeCasts);
        if (!CastScene || scene == "cast-none-found")
            return fixtureCasts;
        try
        {
            var living = new FfiFakeCastOptions(FixedVolume: true, Fetches: true, Refuses: false, LaunchMs: 0);
            living = scene switch
            {
                "cast-starting" or "cast-counting" => living with { LaunchMs = 7000 },
                "cast-refused" => living with { Refuses = true },
                "cast-never-reads" => living with { Fetches = false },
                "cast-fallback" or "cast-fallback-remote" or "cast-messages" => living with { OwnReceiverFails = true },
                _ when scene.StartsWith("cast-reconnecting", StringComparison.Ordinal) || scene.StartsWith("cast-rejoined", StringComparison.Ordinal)
                    || scene.StartsWith("cast-gave-up", StringComparison.Ordinal) => living with { Remembers = true },
                _ => living,
            };
            fakeCasts.Add(engine.StartFakeCast("Living room", living));
            fakeCasts.Add(engine.StartFakeCast("Kitchen display", new FfiFakeCastOptions(FixedVolume: false, Fetches: true, Refuses: false, LaunchMs: 0)));
        }
        catch (FfiException e)
        {
            KaeriFfiMethods.LogLine(FfiLogLevel.Warn, "cast", $"the fake TVs with Chromecast built in did not start: {e.Message}");
        }
        return fixtureCasts;
    }

    sealed class FixtureCastBrowser(List<FfiFakeCast> casts) : FfiCastBrowser
    {
        public void Start(FfiCastSink sink)
        {
            foreach (var fake in casts)
            {
                var advert = fake.Advert();
                sink.Found(advert.Instance, [fake.Host()], advert.Port, advert.Txt);
            }
        }

        public void Stop()
        {
        }
    }

    /// <summary>
    /// --show=cast-*: Play on TV with the fake TVs with Chromecast built in, in the state a
    /// screenshot is for. Bedroom runs the app; Living room has fixed volume; Kitchen display a level.
    ///
    ///     cast-picker          an episode to play: Bedroom, then the Chromecast group, with the firewall line
    ///     cast-picker-allowed  the same once a cast has played from this PC: no firewall line
    ///     cast-none-found      no TV of either kind answers, past the wait
    ///     cast-menu            the title bar's cast menu: Chromecast, then the TVs on this network
    ///     cast-starting        Living room picked, while it opens the receiver: the mini starting
    ///     cast-counting        the same, the controller open
    ///     cast-playing-fixed   Living room playing, the controller open: fixed volume, so Mute only
    ///     cast-playing-level   Kitchen display playing, the controller open: a level, so a slider
    ///     cast-refused         Living room refuses every stream: the controller says why
    ///     cast-never-reads     Living room never reads the stream, after a minute
    ///     cast-lost            Living room drops off mid-episode
    ///     cast-taken-over      someone else takes Living room: the mini goes, the bar offers Play here
    ///     cast-bar             Living room playing with the controller closed: the mini and the filled button
    ///     cast-bar-menu        the same, with the button's menu open on what plays
    ///     cast-stopped         Stop casting from the controller
    ///     cast-up-next         Living room near the end of the episode: Up next counts down
    ///     cast-reconnecting    the connection closes and is made again for the engine's 20 s: the mini says so
    ///     cast-reconnecting-remote  the same with the controller open: only Stop casting takes input
    ///     cast-rejoined        the connection comes back after 4 s: the mini plays on in place
    ///     cast-rejoined-remote the same with the controller open
    ///     cast-gave-up         the 20 s run out: the mini goes and the notice offers Play here
    ///     cast-gave-up-remote  the same with the controller open: Casting stopped in place
    ///     cast-lost-remote     Living room drops off with the controller open
    ///     cast-stopped-on-tv   Living room's receiver is stopped on the TV: the notice with Play here
    ///     cast-stopped-on-tv-remote  the same with the controller open
    ///     cast-fallback        Kaeri's receiver doesn't launch on Living room: the TV's own player, and the notice
    ///     cast-fallback-remote the same with the controller open: the notice above it
    ///     cast-messages        the fallback and a stop on the TV, then Messages
    ///     cast-remote-more     Living room playing, the controller's More open
    ///     cast-idle-remote     Living room held with nothing on it, the controller open: Disconnect
    ///     cast-idle-remote-more  the same with More open
    ///     cast-menu-idle       the same in the title bar's menu: Disconnect
    /// Any cast scene ending in -reduced runs with animation effects off.
    /// </summary>
    async void RehearseCast(string shown)
    {
        var scene = CastSceneName;
        if (shown.EndsWith("-reduced", StringComparison.Ordinal))
            Motion.Override = false;
        await Task.Delay(1200);
        await Until(() => ctx.Known.Entry(900001) is not null, 5);
        var play = new TvPlay(900001, ctx.Known.Entry(900001)?.Title ?? "", "7", 0);
        if (scene == "cast-menu")
        {
            if (await Until(() => tv.CastRows.Length > 0 && tv.Rows.Length > 0))
                castButton?.Open();
            return;
        }
        var idle = scene is "cast-idle-remote" or "cast-idle-remote-more" or "cast-menu-idle";
        if (!idle)
            tv.Offer(play);
        if (scene is "cast-picker" or "cast-picker-allowed" or "cast-none-found")
            return;
        var name = scene == "cast-playing-level" ? "Kitchen display" : "Living room";
        if (!await Until(() => tv.CastRows.Any(r => r.Name == name)))
            return;
        await Task.Delay(600);
        tv.ConnectCast(tv.CastRows.First(r => r.Name == name));
        TvDialog.Showing?.Close();
        if (idle)
        {
            // Held with nothing on it.
            if (!await Until(() => tv.Session is { State: FfiCastSessionState.Connected }, 20))
                return;
            await Task.Delay(600);
            TvDialog.Showing?.Close();
            if (scene == "cast-menu-idle")
                castButton?.Open();
            else
            {
                castLayer?.Expand();
                if (scene == "cast-idle-remote-more")
                {
                    await Task.Delay(800);
                    castLayer?.ShowMore();
                }
            }
            return;
        }
        if (scene is "cast-starting")
            return;
        if (scene is "cast-counting" or "cast-refused" or "cast-never-reads")
        {
            await Until(() => tv.SessionLoaded, 20);
            castLayer?.Expand();
            return;
        }
        if (!await Until(() => tv.Session is { State: FfiCastSessionState.Playing, Item.Loading: false }, 20))
            return;
        var fake = fakeCasts.FirstOrDefault(f => f.Advert().Txt.Any(t => t.Key == "fn" && t.Value == name));
        switch (scene)
        {
            case "cast-lost":
                await Task.Delay(1500);
                fake?.GoSilent();
                break;
            case "cast-lost-remote":
                castLayer?.Expand();
                await Task.Delay(1500);
                fake?.GoSilent();
                break;
            case "cast-reconnecting" or "cast-gave-up":
                await Task.Delay(1500);
                fake?.LoseConnection(120_000);
                break;
            case "cast-reconnecting-remote" or "cast-gave-up-remote":
                castLayer?.Expand();
                await Task.Delay(1500);
                fake?.LoseConnection(120_000);
                break;
            case "cast-rejoined":
                await Task.Delay(1500);
                fake?.LoseConnection(4000);
                break;
            case "cast-rejoined-remote":
                castLayer?.Expand();
                await Task.Delay(1500);
                fake?.LoseConnection(4000);
                break;
            case "cast-stopped-on-tv":
                await Task.Delay(1500);
                fake?.StopApp();
                break;
            case "cast-stopped-on-tv-remote":
                castLayer?.Expand();
                await Task.Delay(1500);
                fake?.StopApp();
                break;
            case "cast-fallback":
                break;
            case "cast-fallback-remote":
                castLayer?.Expand();
                break;
            case "cast-messages":
                await Task.Delay(1500);
                fake?.StopApp();
                await Task.Delay(9500);
                ShowMessages();
                break;
            case "cast-remote-more":
                castLayer?.Expand();
                await Task.Delay(800);
                castLayer?.ShowMore();
                break;
            case "cast-taken-over":
                await Task.Delay(1500);
                fake?.TakeOver();
                break;
            case "cast-bar":
                break;
            case "cast-bar-menu":
                await Task.Delay(800);
                castButton?.Open();
                break;
            case "cast-stopped":
                castLayer?.Expand();
                await Task.Delay(1500);
                tv.DisconnectCast();
                break;
            case "cast-up-next":
                await Task.Delay(800);
                if (tv.Painter.Length is { } duration)
                    tv.SessionSeekTo(duration - 26);
                castLayer?.Expand();
                break;
            default:
                castLayer?.Expand();
                break;
        }
    }

    System.Net.Sockets.TcpListener? silentTv;
    readonly List<System.Net.Sockets.Socket> heldOpen = [];

    /// <summary>tv-slow's TV: a port on the loopback interface that takes connections and never says a word.</summary>
    ushort StartSilentTv()
    {
        var listener = silentTv = new System.Net.Sockets.TcpListener(System.Net.IPAddress.Loopback, 0);
        listener.Start();
        _ = Task.Run(async () =>
        {
            try
            {
                while (true)
                {
                    var socket = await listener.AcceptSocketAsync();
                    lock (heldOpen)
                        heldOpen.Add(socket);
                }
            }
            catch (Exception)
            {
                // Stopped.
            }
        });
        return (ushort)((System.Net.IPEndPoint)listener.LocalEndpoint).Port;
    }

    void StopSilentTv()
    {
        silentTv?.Stop();
        silentTv = null;
        lock (heldOpen)
        {
            foreach (var socket in heldOpen)
                socket.Dispose();
            heldOpen.Clear();
        }
    }

    /// <summary>
    /// The fake TVs running the app, found as DNS-SD would find them. <paramref name="apart"/>
    /// finds them at a documentation address rather than loopback, where the TVs with Chromecast
    /// built in are, as the picker leaves out a Chromecast at the address of a TV running the app.
    /// </summary>
    sealed class FixtureTvBrowser(FfiFakeTv[] tvs, FfiFakeTv? slow, ushort slowPort, bool silent, bool apart = false) : FfiTvBrowser
    {
        public FfiFakeTv[] Tvs => tvs;

        public void Start(FfiDiscoverySink sink)
        {
            if (silent)
                return;
            foreach (var fake in tvs)
            {
                var advert = fake.Advert();
                sink.Found(advert.Instance, [apart ? "192.0.2.10" : fake.Host()], fake.Port(), advert.Txt);
            }
            if (slow is not null)
            {
                var advert = slow.Advert();
                sink.Found(advert.Instance, ["127.0.0.1"], slowPort, advert.Txt);
            }
        }

        public void Stop()
        {
        }
    }

    static async Task<bool> Until(Func<bool> there, double seconds = 10)
    {
        var deadline = DateTimeOffset.UtcNow.AddSeconds(seconds);
        while (DateTimeOffset.UtcNow < deadline)
        {
            if (there())
                return true;
            await Task.Delay(50);
        }
        return false;
    }

    /// <summary>
    /// --show=tv-*: Play on TV with the fake TVs, in the state a screenshot is for.
    ///
    ///     tv-paired    Bedroom paired, Living room paired and driven, nothing open (Settings shows them)
    ///     tv-picker    Play on TV asked for an episode: the picker, Bedroom paired and Living room not
    ///     tv-pair      the picker opened to pair, with nothing to play
    ///     tv-code      pairing with Living room, its code to type
    ///     tv-wrong     a wrong code typed
    ///     tv-expired   wrong codes until the code ran out
    ///     tv-slow      pairing with a TV that does not answer yet
    ///     tv-remote    Living room playing the episode, its card open
    ///     tv-lost      the remote scene, then Living room gone from the network mid-episode
    ///     tv-unpaired  the remote scene, then Living room letting this PC go
    ///     tv-bar       the remote scene with its card closed: the title bar's button
    ///     tv-empty     the picker on a network where no TV answers, past its wait
    ///     tv-looking   the same, while it still looks
    /// </summary>
    async void RehearseTv(string scene)
    {
        await Task.Delay(1200);
        if (scene is "tv-empty" or "tv-looking")
        {
            // Nothing paired and nothing found.
            tv.OpenPicker();
            return;
        }
        if (tvSender is not { } sender || fixtureTvs is not { Tvs: [var living, var bedroom] })
            return;
        await Until(() => ctx.Known.Entry(900001) is not null, 5);
        var play = new TvPlay(900001, ctx.Known.Entry(900001)?.Title ?? "", "3", 312);
        try
        {
            var bedroomPairing = await sender.Pair(bedroom.PairLink(bedroom.OpenCode()));
            await bedroomPairing.Pair(null);
            tv.Refresh();
            switch (scene)
            {
                case "tv-picker":
                    tv.Offer(play);
                    return;
                case "tv-pair":
                    tv.OpenPicker();
                    return;
                case "tv-code" or "tv-wrong" or "tv-expired":
                {
                    tv.Offer(play);
                    if (!await Until(() => tv.Rows.Any(r => r.Name == "Living room" && r.Kind == FfiTvRowKind.New)))
                        return;
                    tv.Choose(tv.Rows.First(r => r.Name == "Living room" && r.Kind == FfiTvRowKind.New));
                    if (scene == "tv-code" || !await Until(() => tv.Pairing is { Typing: true, Working: false }))
                        return;
                    await Task.Delay(700);
                    for (var i = 0; i < 6; i++)
                    {
                        var wrong = living.ShownCode() == "111111" ? "222222" : "111111";
                        await tv.Submit(wrong);
                        await Until(() => tv.Pairing is { Working: false });
                        if (scene == "tv-wrong" || tv.Pairing is not { Expired: false })
                            return;
                        await Task.Delay(300);
                    }
                    return;
                }
                case "tv-slow":
                {
                    tv.Offer(play);
                    if (await Until(() => tv.Rows.Any(r => r.Name == "Kitchen")))
                        tv.Choose(tv.Rows.First(r => r.Name == "Kitchen"));
                    return;
                }
            }
            var livingPairing = await sender.Pair(living.PairLink(living.OpenCode()));
            var paired = await livingPairing.Pair(null);
            tv.Refresh();
            if (scene == "tv-paired")
            {
                tv.Connect(paired.Fingerprint);
                return;
            }
            tv.Offer(play);
            tv.Use(paired.Fingerprint);
            await Until(() => tv.Now is { Loading: false });
            switch (scene)
            {
                case "tv-bar":
                    TvDialog.Showing?.Close();
                    break;
                case "tv-lost":
                    await Task.Delay(1500);
                    living.Stop();
                    break;
                case "tv-unpaired":
                    await Task.Delay(1500);
                    living.RevokeAll();
                    break;
            }
        }
        catch (Exception e)
        {
            KaeriFfiMethods.LogLine(FfiLogLevel.Warn, "tv", $"the TV scene did not set up: {e.Message}");
        }
    }
#endif
}
