using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;

namespace Kaeri.Windows;

/// <summary>
/// What every page reaches: the engine, what it has said, the art, and the window's ways to move
/// between pages and to play. One per window; pages find it as <see cref="Current"/>.
/// </summary>
/// <summary>A title opened by Surprise me: where it was picked from, and whether that pool has another.</summary>
sealed record Pick(uint Id, SurprisePool Pool, bool More);

sealed class Ctx
{
    public static Ctx Current { get; set; } = null!;

    public required EngineSession Session { get; init; }
    public required Art Art { get; init; }
    public required MainWindow Window { get; init; }
    public required Options Options { get; init; }
    public required Updater Updater { get; init; }
    /// <summary>Play on TV: the TVs, pairing, and the card of the one driven.</summary>
    public required TvRemote Tv { get; init; }

    public Known Known => Session.Known;

    /// <summary>Every batch of the engine's events, on the UI thread, after Known has them.</summary>
    public event Action<IReadOnlyList<Event>>? Batch;

    /// <summary>Proxies checked before they are saved: the proxy dialog's checks.</summary>
    public ProxyTrials Trials => trials ??= new(command => Send(command));
    ProxyTrials? trials;

    public void Heard(IReadOnlyList<Event> batch)
    {
        foreach (var e in batch)
            if (e is Event.ProxyChecked checkedNow)
                Trials.Heard(checkedNow.V1);
        // Each listener on its own: one page that throws leaves the rest of the batch to the others.
        if (Batch is { } listeners)
            foreach (var heard in listeners.GetInvocationList().Cast<Action<IReadOnlyList<Event>>>())
                Guarded(heard.Target?.GetType().Name ?? "a listener", () => heard(batch));
    }

    /// <summary>Runs <paramref name="act"/>, logging what it throws instead of ending the app.</summary>
    public static void Guarded(string who, Action act)
    {
        try
        {
            act();
        }
        catch (Exception error)
        {
            KaeriFfiMethods.LogLine(FfiLogLevel.Error, "windows", $"{who} failed on an engine batch: {error}");
        }
    }

    readonly Dictionary<object, Action> leaving = [];

    /// <summary>
    /// <paramref name="page"/> hears every batch from now until the frame leaves it; then
    /// <paramref name="leave"/> runs, once. The frame's leaving is the signal rather than
    /// Unloaded, which a page left before it loaded never raises: such a page would stay in
    /// memory as a listener for the rest of the run. A page the frame keeps for Back calls this
    /// on each visit; while it already hears, a second call changes nothing.
    /// </summary>
    public void Hear(Microsoft.UI.Xaml.FrameworkElement page, Action<IReadOnlyList<Event>> heard, Action? leave = null)
    {
        if (leaving.ContainsKey(page))
            return;
        Batch += heard;
        var gone = false;
        void Leave()
        {
            if (gone)
                return;
            gone = true;
            Batch -= heard;
            leaving.Remove(page);
            leave?.Invoke();
        }
        leaving[page] = Leave;
    }

    /// <summary>The frame has moved on from <paramref name="page"/>: it stops hearing.</summary>
    public void Left(object? page)
    {
        if (page is not null && leaving.TryGetValue(page, out var leave))
            leave();
    }

#if KAERI_TESTING
    bool hiddenShown;
    /// <summary>An event a shot makes up, taken in and heard as the engine's own.</summary>
    public void Inject(Event e)
    {
        Session.Known.Take(e);
        Heard([e]);
    }

    static readonly LibraryChoice[] ShowcaseLibraries =
    [
        new LibraryChoice("l1", "Shows", true, "shows"),
        new LibraryChoice("l2", "Films", true, "movies"),
        new LibraryChoice("l3", "Home videos", false, "other"),
    ];

    const string ShowcaseFolder = @"D:\Shows";

    /// <summary>The folder and server pages' shots: invented folders, matches and a signed-in server in place of the empty fixtures.</summary>
    Event[]? LawfulPages(Command command)
    {
        var show = Options.Show;
        if (command is Command.Licensed { V1: LicensedCommand.LoadAvailability ask } && show?.StartsWith("lawful-title-", StringComparison.Ordinal) == true)
        {
            var main = new ServiceLink("Lumen Plus", "https://lumen.example/watch/1", null);
            var other = new ServiceLink("Harbor Library", "https://harbor.example/watch/1", null);
            var found = show switch
            {
                "lawful-title-a" => new TitleAvailability(Id: ask.Id, Case: AvailabilityCase.PlaysAlsoOn, Main: null, AlsoOn: [main, other], Yours: [], Others: [], Episodes: [], ChooseServices: false),
                "lawful-title-b" => new TitleAvailability(Id: ask.Id, Case: AvailabilityCase.OnService, Main: main, AlsoOn: [other], Yours: [], Others: [], Episodes: [], ChooseServices: false),
                _ => new TitleAvailability(Id: ask.Id, Case: AvailabilityCase.NotInSources, Main: null, AlsoOn: [], Yours: [], Others: ["Lumen Plus", "Harbor Library"], Episodes: [], ChooseServices: true),
            };
            return [new Event.Availability(found)];
        }
        return null;
    }

    Event[]? LawfulServer(ServerCommand command)
    {
        const string address = "http://media.example:8096";
        var show = Options.Show;
        switch (command)
        {
            case ServerCommand.Discover:
                return show == "lawful-server-none" ? [new Event.Server(new ServerEvent.DiscoveryDone())]
                    : [new Event.Server(new ServerEvent.Found(new FoundServer("s1", "Hotaru media", address))), new Event.Server(new ServerEvent.DiscoveryDone())];
            case ServerCommand.Check when show == "lawful-server-cert":
                return [new Event.Server(new ServerEvent.Failed(ServerStep.Address, new ServerFailure.Certificate("media.example"))), new Event.Server(new ServerEvent.Certificate(KaeriFfiMethods.TestingCertificateCheck()))];
            case ServerCommand.Check:
                return show == "lawful-server-unreachable" ? [new Event.Server(new ServerEvent.Failed(ServerStep.Address, new ServerFailure.Unreachable(address)))]
                    : [new Event.Server(new ServerEvent.Checked(new CheckedServer(Id: "s1", Name: "Hotaru media", Version: "10.10.3", Address: address, QuickConnect: true)))];
            case ServerCommand.SignIn:
                return show == "lawful-server-credentials" ? [new Event.Server(new ServerEvent.Failed(ServerStep.SignIn, new ServerFailure.Credentials()))]
                    : [new Event.Server(new ServerEvent.SignedIn("s1")), new Event.Server(new ServerEvent.Libraries("s1", ShowcaseLibraries))];
            case ServerCommand.StartQuickConnect:
                return [new Event.Server(new ServerEvent.QuickConnectCode("482913"))];
            case ServerCommand.LoadLibraries:
                return [new Event.Server(new ServerEvent.Libraries("s1", ShowcaseLibraries))];
        }
        return null;
    }
#endif

    public ulong? Send(Command command)
    {
#if KAERI_TESTING
        // The match picker's shots: a search by hand that fails or never comes back, and a pick that is never answered.
        if (command is Command.ManualSearch failing && Options.Show == "match-failed")
        {
            Window.DispatcherQueue.TryEnqueue(async () =>
            {
                await Task.Delay(300);
                Inject(new Event.MatchSearchFailed(failing.Id, failing.Query, new Msg.MatchSearchTimedOut()));
            });
            return null;
        }
        if ((command is Command.ManualSearch && Options.Show == "match-menu") || (command is Command.FixMatch && Options.Show == "match-picking"))
            return null;
        // The source picker's shots: the listing is the one the showcase invented, or none at all.
        if (command is Command.LoadSourceOptions listing && Options.Show?.StartsWith("source-picker", StringComparison.Ordinal) == true)
        {
            if (Options.Show != "source-picker-loading")
                Window.DispatcherQueue.TryEnqueue(() =>
                {
                    var events = new Event[] { new Event.SourceOptions(listing.V1, Showcase.PickerOptions(Options.Show)) };
                    Session.Known.Take(events[0]);
                    Heard(events);
                });
            return null;
        }
        // --show=settings-hidden: Hidden titles with two entries the fixtures hid, one three days ago.
        if (Options.Show == "settings-hidden" && command is Command.LoadHiddenTitles)
        {
            if (hiddenShown)
                return null;
            hiddenShown = true;
            Window.DispatcherQueue.TryEnqueue(() =>
            {
                var now = DateTimeOffset.UtcNow.ToUnixTimeSeconds();
                var rows = new[] { (900001u, false, now - 3 * 86400), (900002u, true, now - 2 * 3600) }
                    .Select(h => new HiddenTitle(new Entry(Id: h.Item1, Title: h.Item1 == 900001u ? "Lantern Street" : "Moon Laundry"), h.Item2, h.Item3))
                    .ToArray();
                var events = new Event[] { new Event.HiddenTitles(rows) };
                Session.Known.Take(events[0]);
                Heard(events);
            });
            return null;
        }
        if (LawfulPages(command) is { } pages)
        {
            Window.DispatcherQueue.TryEnqueue(() =>
            {
                foreach (var e in pages)
                    Session.Known.Take(e);
                Heard(pages);
            });
            return null;
        }
        // The media server flow's shots: the fixture engine reaches no server, so the steps are answered with an invented one.
        if (Options.Show?.StartsWith("lawful-", StringComparison.Ordinal) == true && command is Command.Server server && LawfulServer(server.V1) is { } answer)
        {
            _ = Task.Delay(350).ContinueWith(_ => Window.DispatcherQueue.TryEnqueue(() =>
            {
                foreach (var e in answer)
                    Session.Known.Take(e);
                Heard(answer);
            }));
            return null;
        }
        // A discussion's shots: the fixture engine has no AniList forum, so the pane is answered with the invented thread.
        if (command is Command.OpenDiscussion opening && Discussion.DiscussionShowcase.Is(Options.Show))
        {
            Window.DispatcherQueue.TryEnqueue(() =>
            {
                var events = new Event[] { new Event.Discussion(Discussion.DiscussionShowcase.View(Options.Show!, opening.Media, opening.Episode)) };
                Session.Known.Take(events[0]);
                Heard(events);
            });
            return null;
        }
        // A showcase has no store to ask: the invite preview is the one it invented.
        if (Options.Showcase && command is Command.PreviewStore preview)
        {
            Window.DispatcherQueue.TryEnqueue(() =>
            {
                var events = new[] { Showcase.Preview(preview.Link) };
                Session.Known.Take(events[0]);
                Heard(events);
            });
            return null;
        }
        if (Options.Showcase && command is Command.JoinStore)
        {
            Window.DispatcherQueue.TryEnqueue(() =>
            {
                var events = new Event[] { new Event.StoreJoined("hotaru") };
                Session.Known.Take(events[0]);
                Heard(events);
            });
            return null;
        }
        // A proxy dialog's check held while it checks, or answered as failed, for a screenshot:
        // the fixture's own check always passes.
        if (command is Command.CheckTorrentProxy && Options.Show is "torrent-edit-checking" or "torrent-edit-failed")
        {
            if (Options.Show == "torrent-edit-failed")
                _ = Task.Delay(1200).ContinueWith(_ => Window.DispatcherQueue.TryEnqueue(() => Heard([new Event.ProxyChecked(ProxyStatus.Failed)])));
            return null;
        }
#endif
        return Session.Send(command);
    }

    public void OpenTitle(uint id) => Window.OpenTitle(id);

    /// <summary>The title Surprise me opened last, while its page is up: it gives the page its Another.</summary>
    public Pick? Pick { get; set; }

    /// <summary>One of the window's own pages, by its tag: "downloads".</summary>
    public void Go(string tag) => Window.Go(tag);

    /// <summary>Back to Your sources from the page of a source that was just removed.</summary>
    public void LeaveRemoved() => Window.LeaveRemoved();

    /// <summary>
    /// Plays an episode from where it was left, or from its start with <paramref name="fromStart"/>.
    /// In a party the engine picks it for the room instead, and the lobby shows the pick.
    /// </summary>
    public void Play(uint id, string episode, bool fromStart = false)
    {
        Send(new Command.Play(id, episode, fromStart));
        if (Known.Party is not null && !Window.PlayerUp)
            Window.Go("together");
    }

    public void Search(string query) => Window.ShowSearch(query);

    /// <summary>Pauses every download that runs or waits; each resumes from its own row.</summary>
    public void PauseAllDownloads()
    {
        foreach (var row in Known.Downloads.Where(r => r.State is DownloadState.Active or DownloadState.Queued))
            Send(new Command.DownloadPause(row.Id));
    }
}
