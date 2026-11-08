// The Windows app's own rules, and the testing engine's Watching scene read the way the app
// reads it: through EngineSession into Known, then the model's own queue rules.

using Kaeri.App;
using Kaeri.Engine.Core;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Xunit;

namespace Kaeri.App.Tests;

static class HomeGuard
{
    public static void Check()
    {
        var guarded = Environment.GetEnvironmentVariable("KAERI_TEST_HOME");
        var home = Environment.GetEnvironmentVariable(OperatingSystem.IsWindows() ? "USERPROFILE" : "HOME");
        Assert.False(string.IsNullOrEmpty(guarded), "run the tests with desktop/windows/scripts/test.sh, which gives them a temporary HOME");
        Assert.Equal(guarded, home);
    }
}

class QuietHost : FfiHost
{
    public bool OpenUrl(string url) => false;
    public void Notify(string title, string body) { }
    public string? SecretGet(string id) => null;
    public bool SecretSet(string id, string value) => false;
    public bool SecretDelete(string id) => true;
}

public class KnownTests
{
    static Entry Title(uint id, string title) => new(Id: id, Title: title);

    [Fact]
    public void AStatusSetHereShowsAtOnceAndMovesTheTitleBetweenLists()
    {
        var known = new Known();
        known.Take(new Event.Content(new ContentFor.Library(LibrarySegment.Watching), new ContentList.Entries([Title(1, "Lantern Street"), Title(2, "Moon Laundry")])));
        known.Take(new Event.Content(new ContentFor.Library(LibrarySegment.Planning), new ContentList.Empty()));
        known.SetStatus(1, LibrarySegment.Planning);
        Assert.Equal(LibrarySegment.Planning, known.Entry(1)?.ListStatus);
        Assert.Equal([2u], known.ListEntries(new ContentFor.Library(LibrarySegment.Watching)).Select(e => e.Id));
        Assert.Equal([1u], known.ListEntries(new ContentFor.Library(LibrarySegment.Planning)).Select(e => e.Id));
    }

    static HomeView Home(params uint[] continuing) => new(
        Sections: [new HomeSection(Kind: SectionKind.Continue, Title: new Msg.HomeThisWeek(), Total: (uint)continuing.Length,
            Items: [.. continuing.Select(id => new HomeItem(Entry: Title(id, "Lantern Street"), Action: HomeAction.Resume, Playable: Playable.Unknown))])],
        Pending: []);

    [Fact]
    public void ATitleRemovedOnHomeStaysOutUntilAHomeComesWithoutIt()
    {
        var known = new Known();
        known.Take(new Event.Home(Home(1, 2)));
        known.HiddenFromContinue.Add(1);
        known.DismissedHere.Add(2);
        // A Home that crossed the removal still holds both.
        known.Take(new Event.Home(Home(1, 2)));
        Assert.Contains(1u, known.HiddenFromContinue);
        Assert.Contains(2u, known.DismissedHere);
        known.Take(new Event.Home(Home(3)));
        Assert.Empty(known.HiddenFromContinue);
        Assert.Empty(known.DismissedHere);
    }

    [Fact]
    public void ProgressMarksTheEpisodesItCarriesInTheTable()
    {
        var known = new Known();
        known.Take(new Event.Episodes(7, [new EpisodeRow("1", Title: "Lanterns"), new EpisodeRow("2")]));
        known.Take(new Event.Progress(7, null, null, [new EpisodeProgress("1", Watched: 1, Completed: true)]));
        Assert.True(known.Episodes[7][0].Completed);
        Assert.Equal("Lanterns", known.Episodes[7][0].Title);
        Assert.False(known.Episodes[7][1].Completed);
    }

    [Fact]
    public void OfflineFollowsTheEnginesConnectivity()
    {
        var known = new Known();
        Assert.False(known.Offline);
        known.Take(new Event.Connectivity(true));
        Assert.True(known.Offline);
        known.Take(new Event.Connectivity(false));
        Assert.False(known.Offline);
    }

    [Fact]
    public void TheDemoStandsAsTheEngineLastSaidAndSortsLast()
    {
        var known = new Known();
        Assert.False(known.Demo.On);
        known.Take(new Event.Demo(new DemoState(On: true, Offer: false, Only: true, Played: false)));
        Assert.True(known.Demo.On);
        Assert.True(known.Demo.Only);
        known.Take(new Event.Providers(
        [
            new ProviderRow(Id: "demo", Kind: "demo", State: "ready", Usable: true, Name: "Demo", Origin: new ProviderOrigin.BuiltIn(), IsDemo: true),
            new ProviderRow(Id: "alpha", Kind: "plugin", State: "ready", Usable: true, Name: "Alpha", Origin: new ProviderOrigin.BuiltIn()),
        ]));
        Assert.Equal(["alpha", "demo"], known.OrderedProviders.Select(r => r.Id));
        known.Take(new Event.Demo(new DemoState(On: true, Only: false, Played: true)));
        Assert.True(known.Demo.Played);
        Assert.False(known.Demo.Only);
    }

    [Fact]
    public void ThePlayersStartingLineForTheDemoSaysPart()
    {
        Assert.Equal(T.PlayerStartingPart("1"), Lines.StartingPart("1"));
        Assert.Equal(Lines.Starting("2.5"), Lines.StartingPart("2.5"));
    }

    [Fact]
    public void ATitleHasADubOnlyWhenItsSourceSaidOne()
    {
        var known = new Known();
        Assert.False(known.Dubbed.ContainsKey(7));
        known.Take(new Event.TitleDub(7, new DubExtent(Through: 6, Complete: false)));
        known.Take(new Event.TitleDub(8, null));
        Assert.True(known.Dubbed[7]);
        Assert.False(known.Dubbed[8]);
    }

    [Fact]
    public void ProgressMovesTheTitleWhereverItWasSeen()
    {
        var known = new Known();
        known.Take(new Event.Content(new ContentFor.Continue(), new ContentList.Entries([Title(7, "Stardust Wayfarer")])));
        known.Take(new Event.Progress(7, new TitleProgress(Done: 3, Next: 4), new ResumePoint(Position: 60), []));
        var entry = known.ListEntries(new ContentFor.Continue()).Single();
        Assert.Equal(3u, entry.Progress?.Done);
        Assert.Equal(60.0, entry.Resume?.Position);
    }

    [Fact]
    public void AListRowKeepsWhatTheTitlePageKnew()
    {
        var known = new Known();
        known.Take(new Event.Detail(Title(7, "Stardust Wayfarer") with { Synopsis = "A courier crosses the sky.", BannerUrl = "http://127.0.0.1/b.jpg" }));
        known.Take(new Event.Content(new ContentFor.Seasonal(), new ContentList.Entries([Title(7, "Stardust Wayfarer")])));
        var entry = known.Entry(7)!;
        Assert.Equal("A courier crosses the sky.", entry.Synopsis);
        Assert.Equal("http://127.0.0.1/b.jpg", entry.BannerUrl);
    }

    [Fact]
    public void CalendarRowsKeepTheirOwnBroadcasts()
    {
        var known = new Known();
        var rows = new[]
        {
            Title(7, "Kumo no Ue no Toshokan") with { AiringEpisode = 5, AiringIn = -420_000 },
            Title(7, "Kumo no Ue no Toshokan") with { AiringEpisode = 6, AiringIn = 184_500 },
        };
        known.Take(new Event.Content(new ContentFor.Calendar(), new ContentList.Entries(rows)));
        Assert.Equal([5u, 6u], known.ListRows(new ContentFor.Calendar()).Select(r => r.AiringEpisode!.Value));
    }

    [Fact]
    public void ClosingCountsWhatDownloadsAndWhatSeeds()
    {
        static DownloadRow Row(DownloadState state) => new(
            Id: 1, AnilistId: 1, Title: "Atelier Hotaru", Episode: "1", State: state, Fraction: 0,
            Downloaded: 0, Total: 0);
        var known = new Known();
        known.Take(new Event.Downloads([Row(DownloadState.Active), Row(DownloadState.Queued), Row(DownloadState.Done), Row(DownloadState.Paused)]));
        Assert.Equal((2, 0), known.Running());
    }

    [Fact]
    public void ANewTitleLanguageAsksAgainForWhatWasShown()
    {
        var known = new Known();
        Assert.Empty(known.AskAgain());
        var list = new ContentList.Entries([Title(1, "Atelier Hotaru")]);
        known.Take(new Event.Content(new ContentFor.Continue(), list));
        known.Take(new Event.Content(new ContentFor.Library(LibrarySegment.Watching), list));
        known.Take(new Event.Content(new ContentFor.Search(Searches.Text("lamp")), new ContentList.Empty()));
        known.Take(new Event.Content(new ContentFor.Search(Searches.Text("relay")), list));
        var again = known.AskAgain().ToList();
        Assert.Equal(3, again.Count);
        Assert.IsType<Command.LoadContinue>(again[0]);
        Assert.Equal(LibrarySegment.Watching, Assert.IsType<Command.LoadLibrary>(again[1]).V1);
        // Only the search asked last, not every one asked before it.
        Assert.Equal("relay", Assert.IsType<Command.Search>(again[2]).Query.Text);
    }
}

public class CalendarTests
{
    static readonly TimeZoneInfo Utc = TimeZoneInfo.Utc;

    [Fact]
    public void TheWeekStartsOnMonday()
    {
        System.Globalization.CultureInfo.CurrentCulture = System.Globalization.CultureInfo.GetCultureInfo("en-GB");
        // Tuesday 20 October 2026, 19:30 UTC.
        var tuesday = new DateTimeOffset(2026, 10, 20, 19, 30, 0, TimeSpan.Zero).ToUnixTimeSeconds();
        Assert.Equal(new DateOnly(2026, 10, 19), Calendar.WeekStart(tuesday, 0, Utc));
        Assert.Equal(new DateOnly(2026, 10, 26), Calendar.WeekStart(tuesday, 1, Utc));
        Assert.Equal("19 to 25 October 2026", Calendar.WeekTitle(new DateOnly(2026, 10, 19)));
        Assert.Equal("28 September to 4 October 2026", Calendar.WeekTitle(new DateOnly(2026, 9, 28)));
        // The system's clock: a 12-hour locale reads its own way.
        var us = System.Globalization.CultureInfo.GetCultureInfo("en-US");
        Assert.Matches(@"^7:30\sPM$", Calendar.Time(tuesday, Utc, us));
    }

    [Fact]
    public void BroadcastsFallOnTheirDayAndSayWhenTheyMoved()
    {
        System.Globalization.CultureInfo.CurrentCulture = System.Globalization.CultureInfo.GetCultureInfo("en-GB");
        var anchor = new DateTimeOffset(2026, 10, 20, 12, 0, 0, TimeSpan.Zero).ToUnixTimeSeconds();
        var rows = new[]
        {
            new Entry(Id: 1, Title: "Atelier Hotaru", AiringIn: 4 * 3600 + 1800, AiringEpisode: 10, DelayedFrom: anchor + 11 * 3600),
            new Entry(Id: 2, Title: "Natsukaze Relay", AiringIn: -86_400, AiringEpisode: 8),
            new Entry(Id: 3, Title: "No schedule"),
        };
        var broadcasts = Calendar.Broadcasts(rows, anchor, new Known(), mine: false);
        Assert.Equal([2u, 1u], broadcasts.Select(b => b.Entry.Id));
        // The time comes first, the move beside it, then what is out or coming.
        Assert.Equal("16:30", broadcasts[1].Clock(Utc));
        Assert.Equal("Moved from 23:00", broadcasts[1].DelayNote(Utc));
        Assert.Null(broadcasts[0].DelayNote(Utc));
        Assert.Equal("E10 in 4 h 30 min", broadcasts[1].Status(anchor));
        Assert.Equal("E8 out 1 d ago", broadcasts[0].Status(anchor));
        Assert.Equal("Atelier Hotaru, E10, 16:30, Moved from 23:00, in 4 h 30 min", broadcasts[1].Spoken(anchor, Utc));
        Assert.Equal("Natsukaze Relay, E8, 12:00, Out 1 d ago", broadcasts[0].Spoken(anchor, Utc));
        var week = Calendar.Week(new DateOnly(2026, 10, 19), broadcasts, Utc);
        Assert.Equal(2u, week[0].Broadcasts.Single().Entry.Id);
        Assert.Equal(1u, week[1].Broadcasts.Single().Entry.Id);
        Assert.True(broadcasts[0].IsNew(anchor));
        Assert.Empty(Calendar.Broadcasts(rows, anchor, new Known(), mine: true));
    }

    [Fact]
    public void AnEstimatedTimeHasATildeAndADubRowSaysSo()
    {
        System.Globalization.CultureInfo.CurrentCulture = System.Globalization.CultureInfo.GetCultureInfo("en-GB");
        var anchor = new DateTimeOffset(2026, 10, 20, 12, 0, 0, TimeSpan.Zero).ToUnixTimeSeconds();
        var rows = new[]
        {
            new Entry(Id: 1, Title: "Atelier Hotaru", AiringIn: 4 * 3600 + 1800, AiringEpisode: 10, AiringApproximate: true, AiringTranslation: Translation.Dub),
            new Entry(Id: 2, Title: "Natsukaze Relay", AiringIn: 3600, AiringEpisode: 8),
            new Entry(Id: 3, Title: "Kiriha Station", AiringIn: 7200, AiringEpisode: 3, AiringTranslation: Translation.Sub),
        };
        var broadcasts = Calendar.Broadcasts(rows, anchor, new Known(), mine: false);
        Func<uint, Broadcast> broadcast = id => broadcasts.Single(b => b.Entry.Id == id);
        // The dub's mark ends the line, held to the words before it; a sub row, or one not known, has none.
        Assert.Equal("~16:30", broadcast(1).Clock(Utc));
        Assert.Equal("E10 in 4 h 30 min\u00A0·\u00A0Dubbed", broadcast(1).Status(anchor));
        Assert.Equal("E10 out 2 h ago\u00A0·\u00A0Dubbed", broadcast(1).Status(anchor + 6 * 3600 + 1800));
        Assert.Equal("Atelier Hotaru, E10 dubbed, about 16:30, in 4 h 30 min", broadcast(1).Spoken(anchor, Utc));
        Assert.Equal("E10 dubbed", broadcast(1).EpisodeText);
        Assert.True(broadcast(1).Dubbed);
        Assert.False(broadcast(2).Dubbed);
        Assert.False(broadcast(3).Dubbed);
        Assert.Equal("E8", broadcast(2).EpisodeText);
        Assert.Equal("14:00", broadcast(3).Clock(Utc));
        Assert.Equal("E3 in 2 h", broadcast(3).Status(anchor));
    }
}

public class SharedWordsTests
{
    [Fact]
    public void TheQueueReadsAsTheOtherAppsReadIt()
    {
        var entry = new Entry(
            Id: 900_001, Title: "Stardust Wayfarer", Episodes: 12,
            Progress: new TitleProgress(Done: 6, Next: 7),
            Resume: new ResumePoint(Position: 606, Fraction: 606.0 / 1440));
        var queue = Wording.Said(KaeriFfiMethods.ContinueQueue([entry], [], Casing.Sentence));
        Assert.Equal("Resume episode 7 · 10:06", queue.InProgress[0].Button);
        Assert.Equal("Resume E7 · 10:06", queue.InProgress[0].Action);
        Assert.Equal("14 min left", queue.InProgress[0].Left);
        var finished = entry with { Progress = new TitleProgress(Done: 12, Next: 1), Resume = null };
        Assert.Equal("Watch again", Wording.Said(KaeriFfiMethods.QueueItem(finished, Casing.Sentence)).Button);
        var almost = entry with { Progress = new TitleProgress(Done: 11, Next: 12), Resume = null };
        Assert.Equal("Play episode 12", Wording.Said(KaeriFfiMethods.QueueItem(almost, Casing.Sentence)).Button);
        Assert.Equal("4:05", KaeriFfiMethods.PlayClock(245.4));
        Assert.Equal("auto-play next", KaeriFfiMethods.SettingLabel(SettingKey.AutoNext));
        Assert.Equal("Expires in 15 min", KaeriFfiMethods.DeviceCodeExpiry(899));
        Assert.Equal("Expires in 45 s", KaeriFfiMethods.DeviceCodeExpiry(45));
        Assert.Equal("Code expired", KaeriFfiMethods.DeviceCodeExpiry(0));
    }
}

public class LinesTests
{
    [Fact]
    public void ADubSaysSoAfterTheEpisodeAndASubSaysNothing()
    {
        Assert.Equal("Episode 10 · Dubbed", Lines.Episode("Episode 10", null, Translation.Dub));
        Assert.Equal("Episode 10 · Dubbed · The Return", Lines.Episode("Episode 10", "The Return", Translation.Dub));
        Assert.Equal("Episode 10 · The Return", Lines.Episode("Episode 10", "The Return", Translation.Sub));
        Assert.Equal("Episode 10", Lines.Episode("Episode 10", "", Translation.Sub));
        // Continue's preview, for a show whose translation is not known yet.
        Assert.Equal("Episode 10 · The Return", Lines.Episode("Episode 10", "The Return", null));
    }

    [Fact]
    public void ASavedSettingGoesUnsaidAndARefusalStillShows()
    {
        Assert.True(Lines.SettingSaved(new Notice(NoticeKind.Info, "Saved auto-play next.", null, new Msg.NoticeSaved("auto-play next"))));
        Assert.False(Lines.SettingSaved(new Notice(NoticeKind.Alert, "not saved: the file is read-only", null)));
        Assert.False(Lines.SettingSaved(new Notice(NoticeKind.Info, "Episode 3 downloaded", null)));
    }
}

public class TorrentsTests
{
    static SettingsView Settings(TorrentView torrent) => new(
        Translation: Translation.Sub, Quality: 1080, Theme: ThemeMode.Adaptive,
        Sources: new ConfiguredSources(), PresenceClient: false, Torrent: torrent);

    [Fact]
    public void AProxyNeedsItsHostAndAPort()
    {
        var draft = TorrentDraft.From(null);
        Assert.False(draft.Complete);
        draft = draft with { Host = " proxy.example ", Port = "0" };
        Assert.False(draft.Complete, "port 0 is no port");
        draft = draft with { Port = "1080" };
        Assert.True(draft.Complete);
        Assert.Equal(("proxy.example", (ushort)1080), (draft.Proxy!.Host, draft.Proxy.Port));
        Assert.Null(draft.Proxy.User);
        Assert.Null(draft.Proxy.Password);
        Assert.False((draft with { Host = "" }).Complete);
    }

    [Fact]
    public void TheProxyIsSavedOrRemovedWithOneCommand()
    {
        var proxy = new TorrentProxy("proxy.example", 1080, "kai", null);
        Assert.Same(proxy, Assert.IsType<Command.SetTorrentProxy>(Torrents.SetProxy(proxy)).Proxy);
        Assert.Null(Assert.IsType<Command.SetTorrentProxy>(Torrents.SetProxy(null)).Proxy);
        Assert.IsType<Command.CheckProxy>(Torrents.CheckNow());
    }

    [Fact]
    public void AProxyIsCheckedOneAtATimeAndAVerdictGoesOnlyToWhatItChecked()
    {
        var sent = new List<Command.CheckTorrentProxy>();
        var trials = new ProxyTrials(c => sent.Add(Assert.IsType<Command.CheckTorrentProxy>(c)));
        var first = new TrialAsk(new TorrentProxy("proxy.example", 1080, "kai", null));
        var second = new TrialAsk(new TorrentProxy("relay.example", 1081, "kai", null));
        var said = new List<(TrialAsk, ProxyStatus)>();

        trials.Ask(first, (ask, status) => said.Add((ask, status)));
        Assert.True(trials.Checking);
        Assert.Equal("proxy.example", Assert.Single(sent).Proxy.Host);

        // Asked again before the first answered: the second waits, and the first's answer is
        // nobody's, since it was not made on what is asked now.
        trials.Ask(second, (ask, status) => said.Add((ask, status)));
        Assert.Single(sent);
        trials.Heard(ProxyStatus.Passed);
        Assert.Empty(said);
        Assert.Equal("relay.example", sent[^1].Proxy.Host);
        trials.Heard(ProxyStatus.Failed);
        var (asked, verdict) = Assert.Single(said);
        Assert.Equal(second, asked);
        Assert.Equal(ProxyStatus.Failed, verdict);
        Assert.False(trials.Checking);

        // A dialog closed while its check runs hears nothing of it, and the next check waits for it.
        trials.Ask(first, (ask, status) => said.Add((ask, status)));
        trials.Drop();
        Assert.False(trials.Checking);
        trials.Ask(second, (ask, status) => said.Add((ask, status)));
        Assert.Equal(3, sent.Count);
        trials.Heard(ProxyStatus.Passed);
        Assert.Single(said);
        Assert.Equal(4, sent.Count);
        trials.Heard(ProxyStatus.Passed);
        Assert.Equal(2, said.Count);
        Assert.Equal(second, said[1].Item1);
    }

    [Fact]
    public void EditProxyOpensOnTheSavedProxyWithTheKeptPassword()
    {
        Assert.Equal("Not saved. The proxy check failed, so the saved proxy stays.", TorrentWords.NotSaved());

        // Edit proxy opens on the saved proxy with the password empty, which keeps the saved one.
        var settings = Settings(new TorrentView(new ProxyView("proxy.example", 1080, "kai")));
        var draft = TorrentDraft.From(settings);
        Assert.Equal(("proxy.example", "1080", "kai", ""), (draft.Host, draft.Port, draft.User, draft.Password));
        Assert.Null(draft.Proxy!.Password);
        Assert.Equal("Through a SOCKS5 proxy at proxy.example:1080.", TorrentWords.OnNote(settings.Torrent));
        Assert.Equal("Connecting directly, without a proxy.", TorrentWords.OnNote(new TorrentView(null)));
    }

    [Fact]
    public void TheCheckSaysWhatItFoundAndWhen()
    {
        Assert.Equal(("just now", "Passed", false), TorrentWords.Check(ProxyStatus.Passed, 5));
        Assert.Equal("Failed", TorrentWords.Check(ProxyStatus.Failed, null).Word);
        Assert.True(TorrentWords.Check(ProxyStatus.Failed, null).Failed);
        Assert.Equal("Not checked yet", TorrentWords.Check(null, null).Note);
        var known = new Known();
        known.Take(new Event.TorrentProxy(ProxyStatus.Checking));
        Assert.Equal(ProxyStatus.Checking, known.ProxyCheck!.Value.Status);
        known.Take(new Event.TorrentProxy(null));
        Assert.Null(known.ProxyCheck);
    }
}

public class SettingsCatalogTests
{
    [Fact]
    public void RichPresenceWaitsOnPresenceAndTheTitle()
    {
        Assert.True(SettingsCatalog.IsSwitch(SettingKey.PresenceRich));
        Assert.Equal([SettingKey.Presence, SettingKey.PresenceTitle], SettingsCatalog.Needs(SettingKey.PresenceRich));
        Assert.Equal([SettingKey.Syncplay], SettingsCatalog.Needs(SettingKey.SyncplayRoom));
        Assert.Empty(SettingsCatalog.Needs(SettingKey.Presence));
    }

    static SettingRow Row(SettingKey key, string value, SettingChange? change = null) =>
        new(key, value, change ?? new SettingChange.Steps(true, true));

    [Fact]
    public void ARowsControlFollowsItsKeyNotItsValue()
    {
        // A switch is a switch whatever words the row says, and a ladder is a menu even when one
        // of its values reads "off".
        Assert.Equal(SettingControl.Switch, SettingsCatalog.ControlFor(Row(SettingKey.AutoNext, "on")));
        Assert.Equal(SettingControl.Switch, SettingsCatalog.ControlFor(Row(SettingKey.Torrents, "off")));
        Assert.Equal(SettingControl.Choice, SettingsCatalog.ControlFor(Row(SettingKey.UpNextCountdown, "off")));
        Assert.Equal(SettingControl.Choice, SettingsCatalog.ControlFor(Row(SettingKey.Upscaling, "off")));
        Assert.Equal(SettingControl.Choice, SettingsCatalog.ControlFor(Row(SettingKey.Quality, "1080p")));
        Assert.Equal(SettingControl.Text, SettingsCatalog.ControlFor(Row(SettingKey.DownloadDir, "D:/Anime", new SettingChange.Typed("D:/Anime"))));
        Assert.Equal(SettingControl.ReadOnly, SettingsCatalog.ControlFor(Row(SettingKey.TorrentProxy, "external", new SettingChange.Fixed())));
    }

    [Fact]
    public void TheLaddersAreTheEnginesInTheEnginesWords()
    {
        Assert.Equal(["480p", "720p", "1080p", "2160p"], SettingsCatalog.Options(SettingKey.Quality).Select(o => o.Label));
        Assert.Equal(["70%", "80%", "85%", "90%", "95%"], SettingsCatalog.Options(SettingKey.CommitThreshold).Select(o => o.Label));
        Assert.Equal(["off", "natural · ArtCNN", "sharp · Anime4K"], SettingsCatalog.Options(SettingKey.Upscaling).Select(o => o.Label));
        Assert.Equal(["off", "5 s", "10 s", "15 s", "20 s", "30 s"], SettingsCatalog.Options(SettingKey.UpNextCountdown).Select(o => o.Label));
        Assert.Equal(["off", "until ratio 0.5", "until ratio 1.0", "until ratio 1.5", "until ratio 2.0"], SettingsCatalog.Options(SettingKey.SeedRatio).Select(o => o.Label));
        Assert.Equal(["off", "1 episode", "2 episodes", "3 episodes", "5 episodes", "10 episodes"], SettingsCatalog.Options(SettingKey.KeepNextEpisodes).Select(o => o.Label));
        Assert.Equal("unlimited", SettingsCatalog.Options(SettingKey.UploadLimit)[0].Label);
        Assert.Equal("262 kB/s", SettingsCatalog.Options(SettingKey.UploadLimit)[1].Label);
        Assert.Equal(new SettingValue[] { new SettingValue.Bool(false), new SettingValue.Bool(true) }, SettingsCatalog.Options(SettingKey.Syncplay).Select(o => o.Value));
        Assert.Empty(SettingsCatalog.Options(SettingKey.DownloadDir));
        Assert.Empty(SettingsCatalog.Options(SettingKey.MpvOptions));
    }

    [Fact]
    public void ARowStandsOnTheOptionItsWordsSay()
    {
        Assert.Equal(new SettingValue.Int(1080), SettingsCatalog.Current(Row(SettingKey.Quality, "1080p"))?.Value);
        Assert.Null(SettingsCatalog.Current(Row(SettingKey.Quality, "1440p")));
        // The session's choice is beside the saved one, and the row stands on the saved one.
        Assert.Equal(new SettingValue.Str("sub"), SettingsCatalog.Current(Row(SettingKey.Translation, "sub · dub this session"))?.Value);
        Assert.True(SettingsCatalog.IsOn(Row(SettingKey.Fullscreen, "on")));
        Assert.False(SettingsCatalog.IsOn(Row(SettingKey.Fullscreen, "off")));
        Assert.False(SettingsCatalog.Editable(Row(SettingKey.TorrentProxy, "external", new SettingChange.Fixed())));
        Assert.Equal("D:/Anime", SettingsCatalog.TypedText(Row(SettingKey.DownloadDir, "D:/Anime", new SettingChange.Typed("D:/Anime"))));
    }
}

public class SourcesTests
{
    static ProviderRow Plugin(string id, bool usable = true, string state = "ready") =>
        new(Id: id, Kind: "plugin", State: state, Usable: usable, Name: id, Origin: new ProviderOrigin.ByHand());

    static ProviderRow Torrent() =>
        new(Id: "torrent", Kind: "torrent", State: "ready", Usable: true, Name: "Torrents", Origin: new ProviderOrigin.BuiltIn());

    [Fact]
    public void AMoveStepsOverWhatHasNoRowAndKeepsIt()
    {
        var rows = new[] { Torrent(), Plugin("a-archive"), Plugin("b-library") };
        // "plugins" has no row of its own: it stays after the plugins it stands for.
        var order = new[] { "torrent", "plugins", "remote" };
        var down = SourceOrder.Moved(order, rows, "torrent", 1);
        Assert.Equal(["a-archive", "torrent", "b-library", "plugins", "remote"], down);
        Assert.Null(SourceOrder.Moved(order, rows, "torrent", -1));
        var up = SourceOrder.Moved(["a-archive", "b-library", "torrent", "plugins"], rows, "b-library", -1);
        Assert.Equal(["b-library", "a-archive", "torrent", "plugins"], up);
        Assert.Null(SourceOrder.Moved(order, rows, "missing", 1));
    }

    [Fact]
    public void TheRowsAreSortedByTheOrder()
    {
        var rows = new[] { Plugin("a-archive"), Torrent(), Plugin("b-library") };
        var sorted = SourceOrder.Sorted(rows, ["torrent", "plugins"]);
        Assert.Equal(["torrent", "a-archive", "b-library"], sorted.Select(r => r.Id));
        Assert.Equal(["b-library", "torrent", "a-archive"], SourceOrder.Sorted(rows, ["b-library", "torrent"]).Select(r => r.Id));
    }

    [Fact]
    public void AProviderStandsAsItsRowSaysItDoes()
    {
        Assert.Equal(ProviderStanding.Working, Plugin("a").Standing());
        Assert.Equal(ProviderStanding.Failing, Plugin("a", usable: false).Standing());
        Assert.Equal(ProviderStanding.Failing, Plugin("a", state: "down").Standing());
        Assert.Equal(ProviderStanding.Slow, Plugin("a", state: "degraded").Standing());
        Assert.Equal(ProviderStanding.Off, (Plugin("a") with { Disabled = true }).Standing());
        Assert.Equal(ProviderStanding.HeldBack, (Plugin("a") with { HeldBack = true }).Standing());
        Assert.Null(ProviderStanding.Working.StandingLine());
        Assert.Equal("Not answering", ProviderStanding.Failing.StandingLine());
    }
}

public class StoresTests
{
    static StorePluginRow Listed(string id, StorePluginStatus status, string version = "1.0.0", params string[] hosts) =>
        new(Id: id, Name: id, Version: version, Status: status, Hosts: hosts);

    static StoreRow Store(string id, params StorePluginRow[] plugins) =>
        new(Id: id, Name: id + " plugins", Url: $"https://{id}.example", Host: $"{id}.example", Fingerprint: "9f3c 1a2b",
            Member: "Mika", State: new StoreState.Ready(), Plugins: plugins);

    [Fact]
    public void APluginComesFromTheStoreThatKeepsItUpdated()
    {
        var stores = new[]
        {
            Store("one", Listed("films", new StorePluginStatus.OtherStore("two plugins")), Listed("hand", new StorePluginStatus.AddedByHand())),
            Store("two", Listed("films", new StorePluginStatus.Installed()), Listed("hand", new StorePluginStatus.Available())),
        };
        Assert.Equal("two", Stores.OriginOf(stores, "films")?.Store.Id);
        Assert.Equal("one", Stores.OriginOf(stores, "hand")?.Store.Id);
        Assert.Null(Stores.OriginOf(stores, "nothing"));
    }

    [Fact]
    public void UpdatesAndContactsAreCounted()
    {
        var store = Store("one",
            Listed("a", new StorePluginStatus.Update("1.0.0", ["new.example"]), "1.1.0", "x.example", "new.example"),
            Listed("b", new StorePluginStatus.Installed()));
        Assert.Equal(1, Stores.Updates(store));
        Assert.Equal("Contacts x.example and one more", Stores.ContactsLine(store.Plugins[0]));
        Assert.Equal("Lists no hosts to contact", Stores.ContactsLine(store.Plugins[1]));
        Assert.Equal("1.0.0", Stores.InstalledVersion(store.Plugins[0]));
        Assert.Equal("Version 1.0.0 installed, 1.1.0 available", Stores.Standing(store.Plugins[0]));
        Assert.Equal("Run by Ren, Aki · one.example", Stores.RunBy(["Ren", "Aki"], "one.example"));
        Assert.Equal("one.example", Stores.RunBy([], "one.example"));
        Assert.Equal("Joined without a name", Stores.JoinedAs(store with { Anonymous = true }));
    }

    [Fact]
    public void AnInviteIsFoundInWhatWasPasted()
    {
        Assert.Equal("https://plugins.example/join/abc#key", Stores.InviteIn("  https://plugins.example/join/abc#key \n"));
        Assert.Equal("https://plugins.example/join/abc#key",
            Stores.InviteIn("kaeri://join?link=https%3A%2F%2Fplugins.example%2Fjoin%2Fabc%23key"));
        Assert.Equal("https://plugins.example/join/abc#key", Stores.InviteIn("kaeri://plugins.example/join/abc#key"));
        Assert.Null(Stores.InviteIn("https://plugins.example/about"));
        Assert.Null(Stores.InviteIn("not a link"));
    }

    [Fact]
    public void TheEventsFillWhatThePagesRead()
    {
        var known = new Known();
        known.Take(new Event.SourcesChanged());
        Assert.True(known.Rebuilding);
        known.Take(new Event.Providers([new ProviderRow(Id: "torrent", Kind: "torrent", State: "ready", Usable: true, Name: "Torrents", Origin: new ProviderOrigin.BuiltIn())]));
        Assert.False(known.Rebuilding);
        Assert.True(known.ProvidersLoaded);
        known.Take(new Event.Stores([Store("one")]));
        known.Take(new Event.StorePreview(new InvitePreview.Failed("The link is not an invite.", null)));
        Assert.IsType<InvitePreview.Failed>(known.Preview);
        known.Take(new Event.StoreJoined("one"));
        Assert.Equal("one", known.Joined);
        Assert.Null(known.Preview);
    }

    [Fact]
    public void AListThatCameBackEmptyCountsAsLoaded()
    {
        var known = new Known();
        Assert.False(known.ServersLoaded);
        Assert.False(known.HiddenLoaded);
        Assert.False(known.PartyRecentLoaded);
        Assert.False(known.Queue.Loaded);
        known.Take(new Event.Server(new ServerEvent.Servers([])));
        known.Take(new Event.HiddenTitles([]));
        known.Take(new Event.PartyRecent([]));
        known.Take(new Event.Downloads([]));
        Assert.True(known.ServersLoaded);
        Assert.True(known.HiddenLoaded);
        Assert.True(known.PartyRecentLoaded);
        Assert.True(known.Queue.Loaded);
    }
}

public class DownloadsTests
{
    static DownloadRow Row(long id, uint title, DownloadState state, ulong downloaded, ulong total) =>
        new(Id: id, AnilistId: title, Title: $"Show {title}", Episode: id.ToString(), State: state,
            Fraction: total == 0 ? 0 : (double)downloaded / total, Downloaded: downloaded, Total: total);

    [Fact]
    public void TheQueueGroupsByTitleWhereTheFirstRowIs()
    {
        var queue = new DownloadQueue();
        var now = DateTimeOffset.UnixEpoch;
        queue.Apply([
            Row(1, 7, DownloadState.Done, 1000, 1000), Row(2, 9, DownloadState.Queued, 0, 0),
            Row(3, 7, DownloadState.Active, 300, 1000), Row(4, 9, DownloadState.Paused, 50, 100)], now);
        var groups = queue.Groups();
        Assert.Equal([7u, 9u], groups.Select(g => g.Id));
        Assert.Equal([1L, 3L], groups[0].Rows.Select(r => r.Id));
        // A finished download counts its size, the others what they have so far.
        Assert.Equal(1300UL, groups[0].BytesOnDisk);
        Assert.Equal(1350UL, queue.BytesOnDisk);
    }

    [Fact]
    public void ASpeedIsReadFromOneEventToTheNextAndSmoothed()
    {
        var queue = new DownloadQueue();
        var t = DateTimeOffset.UnixEpoch;
        queue.Apply([Row(1, 7, DownloadState.Active, 0, 10_000)], t);
        Assert.Null(queue.SecondsLeft(queue.Rows[0]));
        queue.Apply([Row(1, 7, DownloadState.Active, 1_000, 10_000)], t.AddSeconds(1));
        Assert.Equal(1_000, queue.Rates[1], 3);
        Assert.Equal(9, queue.SecondsLeft(queue.Rows[0]));
        queue.Apply([Row(1, 7, DownloadState.Active, 3_000, 10_000)], t.AddSeconds(2));
        Assert.Equal(1_500, queue.Rates[1], 3);
        // A paused download has no speed.
        queue.Apply([Row(1, 7, DownloadState.Paused, 3_000, 10_000)], t.AddSeconds(3));
        Assert.False(queue.Rates.ContainsKey(1));
    }

    [Fact]
    public void ASeedingFilesSizeIsWhatWentUpOverTheRatio()
    {
        Assert.Equal(400UL, new SeedRow(Name: "x", UploadedBytes: 100, Ratio: 0.25, Goal: 1, LeftSecs: 60).Size());
        Assert.Null(new SeedRow(Name: "x", UploadedBytes: 0, Ratio: 0, Goal: 1, LeftSecs: 60).Size());
    }
}

public class PlayerRulesTests
{
    [Fact]
    public void ItAsksAfterTheEpisodesThatRolledOnWithNobodyThere()
    {
        var still = new StillWatching { After = 3 };
        for (var i = 0; i < 2; i++)
        {
            still.RollingOn();
            still.Started();
            Assert.False(still.FirstPicture());
        }
        still.RollingOn();
        still.Started();
        Assert.True(still.FirstPicture());
        Assert.True(still.Asking);
        // Asked once until it is answered.
        Assert.False(still.FirstPicture());
        still.Answered();
        still.Input();
        Assert.Equal(0, still.RolledOn);
    }

    [Fact]
    public void AnythingTheViewerDoesStartsTheCountOver()
    {
        var still = new StillWatching { After = 2 };
        still.RollingOn();
        still.Started();
        still.Input();
        still.RollingOn();
        still.Started();
        Assert.False(still.FirstPicture());
        // An episode the viewer started is not a roll.
        still.Started();
        Assert.Equal(1, still.RolledOn);
    }

    [Fact]
    public void ZeroNeverAsks()
    {
        var still = new StillWatching { After = 0 };
        for (var i = 0; i < 5; i++)
        {
            still.RollingOn();
            still.Started();
        }
        Assert.False(still.FirstPicture());
    }

    [Fact]
    public void APreviewReadsTheStreamWithOnlyTheOptionsThatReachIt()
    {
        MpvOption[] options =
        [
            new("http-header-fields", "Referer: https://cdn.example/"), new("user-agent", "x"), new("start", "612"),
            new("sid", "no"), new("force-media-title", "Show"), new("tls-verify", "yes"),
        ];
        Assert.Equal(["http-header-fields", "user-agent", "tls-verify"], PreviewSource.Reach(options).Select(o => o.Name));
        Assert.Equal(300, PreviewSource.TimeAt(50, 100, 600), 6);
        Assert.Equal(600, PreviewSource.TimeAt(500, 100, 600), 6);
        Assert.Equal(0, PreviewSource.TimeAt(-4, 100, 600), 6);
    }
}

public class SceneTests
{
    /// <summary>
    /// A proxy checked before it is saved: the engine answers on ProxyChecked, through the
    /// trials the dialog asks with, and nothing is saved. The fixture engine's exit is made up,
    /// so the check sends nothing anywhere.
    /// </summary>
    [Fact(Timeout = 60_000)]
    public async Task AProxyCheckedFirstAnswersAndSavesNothing()
    {
        HomeGuard.Check();
        var root = AppDirs.NewScratchRoot("cs-trial");
        var gate = new object();
        var engine = FfiEngine.StartScene(AppDirs.Under(root), new QuietHost(), new FfiScene.Fresh(), EngineSession.AppStart());
        using (var session = new EngineSession(engine, a => { lock (gate) a(); }))
        {
            var trials = new ProxyTrials(c => session.Send(c));
            session.Batch += batch =>
            {
                foreach (var e in batch)
                    if (e is Event.ProxyChecked checkedNow)
                        trials.Heard(checkedNow.V1);
            };
            session.Start();
            var deadline = DateTime.UtcNow.AddSeconds(20);
            SettingsView? before = null;
            while (before is null && DateTime.UtcNow < deadline)
            {
                await Task.Delay(50, TestContext.Current.CancellationToken);
                lock (gate) before = session.Known.Settings;
            }
            Assert.NotNull(before);

            ProxyStatus? said = null;
            var ask = new TrialAsk(new TorrentProxy("proxy.example", 1080, null, null));
            lock (gate) trials.Ask(ask, (_, view) => said = view);
            while (said is null && DateTime.UtcNow < deadline)
                await Task.Delay(50, TestContext.Current.CancellationToken);
            Assert.Equal(ProxyStatus.Passed, said);
            lock (gate)
            {
                Assert.False(trials.Checking);
                Assert.Equal(before.Torrent, session.Known.Settings!.Torrent);
            }
        }
        DeleteRoot(root);
    }

    [Fact(Timeout = 90_000)]
    public async Task TheWatchingSceneFillsContinue()
    {
        HomeGuard.Check();
        var root = AppDirs.NewScratchRoot("cs-app");
        var gate = new object();
        var engine = FfiEngine.StartScene(AppDirs.Under(root), new QuietHost(), new FfiScene.Watching(), EngineSession.AppStart());
        using (var session = new EngineSession(engine, a => { lock (gate) a(); }))
        {
            session.Start();
            session.Send(new Command.LoadContinue());
            Entry[] entries = [];
            var deadline = DateTime.UtcNow.AddSeconds(60);
            while (entries.Length < 4 && DateTime.UtcNow < deadline)
            {
                await Task.Delay(100, TestContext.Current.CancellationToken);
                lock (gate) entries = session.Known.ListEntries(new ContentFor.Continue());
            }
            Assert.Equal(4, entries.Length);
            var queue = Wording.Said(KaeriFfiMethods.ContinueQueue([.. entries], [], Casing.Sentence));
            var first = queue.InProgress[0];
            Assert.Equal(900_001u, first.Entry.Id);
            Assert.Equal("Resume episode 7 · 10:06", first.Button);
        }
        DeleteRoot(root);
    }

    /// <summary>
    /// The catalog's words are the engine's: choosing each option of a stepped setting makes the
    /// engine's row say exactly the option's label, so a menu built from the catalog never
    /// drifts from what the rows read.
    /// </summary>
    [Fact(Timeout = 120_000)]
    public async Task EveryOptionInTheCatalogIsWhatTheEngineThenSays()
    {
        HomeGuard.Check();
        var root = AppDirs.NewScratchRoot("cs-catalog");
        var gate = new object();
        var engine = FfiEngine.StartScene(AppDirs.Under(root), new QuietHost(), new FfiScene.Fresh(), EngineSession.AppStart());
        using (var session = new EngineSession(engine, a => { lock (gate) a(); }))
        {
            session.Start();
            async Task<SettingRow?> Row(SettingKey key, Func<SettingRow, bool> until)
            {
                var deadline = DateTime.UtcNow.AddSeconds(15);
                SettingRow? row = null;
                while (DateTime.UtcNow < deadline)
                {
                    lock (gate) row = session.Known.Setting(key);
                    if (row is not null && until(row))
                        return row;
                    await Task.Delay(50, TestContext.Current.CancellationToken);
                }
                return row;
            }

            SettingKey[] keys =
            [
                SettingKey.Quality, SettingKey.CommitThreshold, SettingKey.Upscaling, SettingKey.UpNextCountdown,
                SettingKey.DownloadQuality, SettingKey.KeepNextEpisodes, SettingKey.Titles, SettingKey.AutoNext,
                SettingKey.NightMode, SettingKey.Trailers, SettingKey.SeedHours, SettingKey.UploadLimit, SettingKey.SeedRatio,
            ];
            foreach (var key in keys)
            {
                Assert.NotNull(await Row(key, _ => true));
                foreach (var option in SettingsCatalog.Options(key))
                {
                    session.Send(new Command.SetSetting(key, option.Value));
                    var row = await Row(key, r => r.Value == option.Label);
                    Assert.True(row?.Value == option.Label, $"{key} set to {option.Label}: the engine's row says \"{row?.Value}\"");
                    Assert.Equal(option, SettingsCatalog.Current(row));
                }
            }
            lock (gate)
            {
                Assert.Equal(SettingControl.Choice, SettingsCatalog.ControlFor(session.Known.Setting(SettingKey.Quality)!));
                Assert.Equal(SettingControl.Switch, SettingsCatalog.ControlFor(session.Known.Setting(SettingKey.AutoNext)!));
                Assert.Equal(SettingControl.Text, SettingsCatalog.ControlFor(session.Known.Setting(SettingKey.DownloadDir)!));
            }
        }
        DeleteRoot(root);
    }

    /// <summary>The app's own Simkl and Trakt clients turn those trackers on, so Accounts can sign in to them.</summary>
    [Fact(Timeout = 60_000)]
    public async Task TheAppsClientsTurnOnSimklAndTrakt()
    {
        HomeGuard.Check();
        var root = AppDirs.NewScratchRoot("cs-trackers");
        var gate = new object();
        var engine = FfiEngine.StartScene(AppDirs.Under(root), new QuietHost(), new FfiScene.Fresh(), EngineSession.AppStart());
        using (var session = new EngineSession(engine, a => { lock (gate) a(); }))
        {
            session.Start();
            string[] trackers = [];
            var deadline = DateTime.UtcNow.AddSeconds(20);
            while (!(trackers.Contains("simkl") && trackers.Contains("trakt")) && DateTime.UtcNow < deadline)
            {
                await Task.Delay(100, TestContext.Current.CancellationToken);
                lock (gate) trackers = [.. session.Known.Sync.Keys];
            }
            Assert.Contains("simkl", trackers);
            Assert.Contains("trakt", trackers);
        }
        DeleteRoot(root);
    }

    internal static void DeleteRoot(string root)
    {
        var deadline = DateTime.UtcNow.AddSeconds(10);
        while (true)
        {
            try
            {
                Directory.Delete(root, recursive: true);
                return;
            }
            catch (DirectoryNotFoundException)
            {
                return;
            }
            catch (Exception) when (DateTime.UtcNow < deadline)
            {
                Thread.Sleep(200);
            }
        }
    }
}

public class ShowSettingsTests
{
    [Fact]
    public void EveryRowTheEngineHasOffersWhatItsSettingTakes()
    {
        foreach (var key in Enum.GetValues<ShowSettingKey>())
            Assert.NotEmpty(ShowSettingsCatalog.Choices(key));
        Assert.Equal(["Subbed", "Dubbed"], ShowSettingsCatalog.Choices(ShowSettingKey.Translation).Select(c => c.Label));
        Assert.Equal(["480p", "720p", "1080p", "2160p"], ShowSettingsCatalog.Choices(ShowSettingKey.Quality).Select(c => c.Label));
        Assert.Equal(["Standard", "Higher"], ShowSettingsCatalog.Choices(ShowSettingKey.DownloadQuality).Select(c => c.Label));
        Assert.Equal("Off", ShowSettingsCatalog.Choices(ShowSettingKey.KeepNext)[0].Label);
    }

    [Fact]
    public void AShowsOwnValueStandsOnItsChoice()
    {
        Assert.Equal(-1, ShowSettingsCatalog.IndexOf(ShowSettingKey.Quality, null));
        Assert.Equal(2, ShowSettingsCatalog.IndexOf(ShowSettingKey.Quality, new SettingValue.Int(1080)));
        Assert.Equal(1, ShowSettingsCatalog.IndexOf(ShowSettingKey.SkipFiller, new SettingValue.Bool(false)));
        Assert.Equal(1, ShowSettingsCatalog.IndexOf(ShowSettingKey.Translation, new SettingValue.Str("dub")));
        Assert.Equal(-1, ShowSettingsCatalog.IndexOf(ShowSettingKey.Quality, new SettingValue.Int(360)));
    }

    [Fact]
    public void ARowSaysItsValueInTheSheetsWords()
    {
        Assert.Equal("Dubbed", ShowSettingsCatalog.Shown(ShowSettingKey.Translation, "dub"));
        Assert.Equal("On", ShowSettingsCatalog.Shown(ShowSettingKey.SkipOpening, "on"));
        Assert.Equal("720p", ShowSettingsCatalog.Shown(ShowSettingKey.Quality, "720p"));
        Assert.Equal("Standard", ShowSettingsCatalog.Shown(ShowSettingKey.DownloadQuality, "standard"));
        Assert.Equal(SettingKey.KeepNextEpisodes, ShowSettingsCatalog.Setting(ShowSettingKey.KeepNext));
    }
}

public class SearchTests
{
    [Fact]
    public void AFilterWithNothingTypedBrowses()
    {
        var filters = Searches.Nothing.WithYear(2019) with { Genres = ["Drama"], Formats = [MediaFormat.Movie] };
        var query = Searches.With("", filters);
        Assert.Equal("", query.Text);
        Assert.Equal(["Drama"], query.Genres);
        Assert.Equal([MediaFormat.Movie], query.Formats);
        Assert.Equal(new Bounds(2019, 2019), query.Years);
        Assert.Equal(2019u, filters.SingleYear());
        Assert.True(filters.Narrows());
        Assert.False(Searches.Nothing.Narrows());
        Assert.True(Searches.Nothing.IsEmpty());
        var sorted = Searches.Nothing with { Sort = SearchSort.Score };
        Assert.False(sorted.Narrows());
        Assert.False(sorted.IsEmpty());
    }

    [Fact]
    public void ABrowseOpenedKeepsItsFiltersAndLeavesItsWords()
    {
        var browse = new SearchQuery(Text: "lamp", Genres: ["Mecha"], MinScore: 70, Years: new Bounds(), Episodes: new Bounds(), Minutes: new Bounds());
        var filters = Searches.FiltersOf(browse);
        Assert.Equal("", filters.Text);
        Assert.Equal(["Mecha"], filters.Genres);
        Assert.Equal((byte)70, filters.MinScore);
        // Every list is filled in, so the engine's own rules can be called on it.
        Assert.Empty(filters.TagsExcluded);
        Assert.True(filters.HasFilters());
        Assert.Equal("lamp", Searches.With("lamp", filters).Text);
    }

    [Fact]
    public void AQueryIsTheSameQueryWhateverArraysCarryIt()
    {
        var filters = Searches.Nothing with { Genres = ["Drama", "Mystery"] };
        var a = Searches.With("lamp", filters);
        var b = Searches.With("lamp", filters with { Genres = ["Drama", "Mystery"] });
        Assert.NotEqual(a, b);
        Assert.True(Searches.Same(a, b));
        Assert.False(Searches.Same(a, Searches.With("lamp", Searches.Nothing)));
        Assert.True(Searches.Same(Searches.Text("x"), new SearchQuery(Text: "x", Years: new Bounds(), Episodes: new Bounds(), Minutes: new Bounds())));

        var known = new Known();
        known.Take(new Event.Content(new ContentFor.Search(a), new ContentList.Empty()));
        Assert.IsType<ContentList.Empty>(known.List(new ContentFor.Search(b)));
        known.Take(new Event.Content(new ContentFor.Search(b), new ContentList.Failed("down")));
        Assert.Single(known.Lists);
        Assert.IsType<ContentList.Failed>(known.List(new ContentFor.Search(a)));
    }

    [Fact]
    public void AGenreClickedGoesInThenOutThenOffAndADropDownSaysWhatItHas()
    {
        Assert.Equal(Pick.Include, Pick.Off.Next());
        Assert.Equal(Pick.Exclude, Pick.Include.Next());
        Assert.Equal(Pick.Off, Pick.Exclude.Next());
        Assert.Equal(T.FilterIncluded, Pick.Include.Word());
        Assert.Equal(T.FilterLeftOut, Pick.Exclude.Word());
        Assert.Null(Pick.Off.Word());

        var filters = Searches.Nothing.PickingGenre("Mystery", Pick.Include).PickingGenre("Action", Pick.Include);
        Assert.Equal(["Mystery", "Action"], filters.Genres);
        filters = filters.PickingGenre("Mystery", Pick.Include.Next());
        Assert.Equal(["Action"], filters.Genres);
        Assert.Equal(["Mystery"], filters.GenresExcluded);
        Assert.Equal(["Action", "Not Mystery"], filters.GenreWords());
        Assert.Equal([MediaFormat.Movie, MediaFormat.Tv], Searches.Nothing.TogglingFormat(MediaFormat.Movie).TogglingFormat(MediaFormat.Tv).Formats);
        Assert.Equal("Genre", SearchFilters.Say("Genre", []));
        Assert.Equal("Action, Mystery", SearchFilters.Say("Genre", ["Action", "Mystery"]));
        Assert.Equal("Action +2", SearchFilters.Say("Genre", ["Action", "Drama", "Mystery"]));
        Assert.Equal(2027u, Searches.Years(2026)[0]);
        Assert.Equal(1970u, Searches.Years(2026)[^1]);
    }

    [Fact]
    public void EveryFilterSetIsAChipThatTakesItOff()
    {
        var filters = Searches.Nothing.InYears(new Bounds(2010, 2015)) with
        {
            Sort = SearchSort.Score,
            Genres = ["Mecha"],
            GenresExcluded = ["Romance"],
            Tags = ["Trains"],
            TagsExcluded = ["Gardening"],
            MinTagRank = 80,
            Formats = [MediaFormat.Tv, MediaFormat.Movie],
            Statuses = [AiringStatus.Releasing],
            Episodes = new Bounds(null, 12),
            Minutes = new Bounds(20, 30),
            MinScore = 70,
            MinPopularity = SearchFilters.ObscureBelow,
            Sources = [SourceMaterial.LightNovel],
            Countries = ["KR"],
            StreamingOn = ["Lantern Library"],
            LicensedOnly = true,
            OnList = OnList.Hide,
            Available = Available.PlaysHere,
        };
        var chips = filters.Chips();
        Assert.Equal(
            [
                "Mecha", "Not Romance", "Trains", "Not Gardening", "Tag rank 80% or higher", "2010 to 2015",
                "TV, Movie", "Airing", "Up to 12 episodes", "20 to 30 min", "Score 70% or higher", "Hide obscure titles",
                "Light novel", "South Korea", "On Lantern Library", "Licensed only", "Not on my list", "Plays here",
            ],
            chips.Select(c => c.Label));
        // The panel's badge counts its own: each tag, the rank, length, score, obscure, origin, streaming, licence, list.
        Assert.Equal(12u, filters.AdvancedCount());

        // Each chip takes off its own filter and no other; all of them leave the sort.
        var left = filters;
        foreach (var (key, _) in chips)
        {
            var before = left.Chips().Length;
            left = left.Without(key);
            Assert.Equal(before - 1, left.Chips().Length);
        }
        Assert.False(left.Narrows());
        Assert.Equal(SearchSort.Score, left.Sort);
        Assert.False(filters.Cleared().Narrows());
        Assert.Equal(SearchSort.Score, filters.Cleared().Sort);
    }

    [Fact]
    public void AYearKeepsTheSeasonButARangeOfYearsDoesNot()
    {
        var spring = Searches.Nothing with { Season = Season.Spring };
        Assert.Equal(Season.Spring, spring.WithYear(2024).Season);
        Assert.Equal(Season.Spring, spring.WithYear(null).Season);
        Assert.Equal(Season.Spring, spring.InYears(new Bounds(2024, 2024)).Season);
        Assert.Null(spring.InYears(new Bounds(2020, 2024)).Season);
        Assert.Null(spring.InYears(new Bounds(2020, null)).Season);
        Assert.Equal("Since 2020", spring.InYears(new Bounds(2020, null)).Chips().Single().Label);
    }

    [Fact]
    public void ThePanelsPresetsAndFieldsSayAndAskWhatTheEngineDoes()
    {
        Assert.Equal(["1 episode", "Up to 12 episodes", "13 to 26 episodes", "50 episodes or more"], SearchFilters.EpisodePresets.Select(SearchFilters.EpisodesWords));
        Assert.Equal(["Up to 15 min", "20 to 30 min", "60 min or more"], SearchFilters.MinutePresets.Select(SearchFilters.MinutesWords));
        Assert.Equal(new Bounds(null, 12), SearchFilters.Range(double.NaN, 12));
        Assert.Equal(new Bounds(1990, null), SearchFilters.Range(1990.4, double.NaN));
        Assert.Equal(Searches.Open, SearchFilters.Range(double.NaN, double.NaN));
        Assert.Null(SearchFilters.Score(0));
        Assert.Equal((byte)70, SearchFilters.Score(70));
        Assert.Null(SearchFilters.TagRank(SearchFilters.DefaultTagRank));
        Assert.Equal((byte)80, SearchFilters.TagRank(80));
        Assert.Equal([T.MediaStatusAiring, T.MediaStatusAired], Searches.Statuses.Take(2).Select(Searches.StatusName));
        Assert.Equal(T.MaterialOriginal, Searches.SourceName(Searches.Sources[0]));
        Assert.Equal([T.CountryJapan, T.CountryChina, T.CountrySouthKorea, T.CountryTaiwan], Searches.Countries.Select(Searches.CountryName));
    }

    [Fact]
    public void TagsAreFoundByNameAndTheBoxOffersGenresAndTagsAsFilters()
    {
        var facets = new SearchFacets(
            Genres: ["Mecha", "Mystery", "Music"],
            TagGroups:
            [
                new TagGroup("Setting-Scene", [new FacetTag("Trains", "Railways."), new FacetTag("Mountain Huts", "High up.")]),
                new TagGroup("Theme-Drama", [new FacetTag("The Last Lantern", "A twist.", Spoiler: true)]),
            ],
            StreamingSites: ["Lantern Library"]);
        Assert.Equal(["Trains"], SearchFilters.TagsFound(facets, "rain", spoilers: false).Select(t => t.Name));
        Assert.Empty(SearchFilters.TagsFound(facets, "lantern", spoilers: false));
        Assert.Equal(["The Last Lantern"], SearchFilters.TagsFound(facets, "lantern", spoilers: true).Select(t => t.Name));
        Assert.Empty(SearchFilters.TagsFound(facets, "  ", spoilers: true));

        Assert.Empty(SearchFilters.Suggestions(facets, Searches.Nothing, "m"));
        Assert.Equal([(false, "Mecha")], SearchFilters.Suggestions(facets, Searches.Nothing, "mec"));
        Assert.Equal([(true, "Mountain Huts")], SearchFilters.Suggestions(facets, Searches.Nothing, "mou"));
        // What is already wanted is not offered again.
        Assert.Empty(SearchFilters.Suggestions(facets, Searches.Nothing with { Genres = ["Mecha"] }, "mec"));
        Assert.Empty(SearchFilters.Suggestions(facets, Searches.Nothing with { Tags = ["Mountain Huts"] }, "mou"));
        Assert.Empty(SearchFilters.Suggestions(null, Searches.Nothing, "mec"));
    }
}

public class TitleSeasonsTests
{
    static LinkedTitle Linked(uint id, Season? season, ushort? year) => new(Id: id, Title: $"Lantern Road {id}", Season: season, Year: year);

    [Fact]
    public void ThePickerNamesEachSeasonInWatchOrderWithThisOnePicked()
    {
        var detail = new TitleDetail(Seasons: [Linked(910001, Season.Spring, 2021), Linked(910002, null, 2022), Linked(910003, null, null)]);
        var entry = new Entry(Id: 910002, Title: "Lantern Road", Detail: detail,
            Related: [new RelatedTitle(910001, "Lantern Road", "prequel", "TV"), new RelatedTitle(910006, "Lantern Road: the film", "side_story", "MOVIE")]);
        var seasons = TitleSeasons.Of(entry);
        Assert.Equal(["Season 1 · Spring 2021", "Season 2 · 2022", "Season 3"], seasons.Select(s => s.Label));
        Assert.Equal([false, true, false], seasons.Select(s => s.IsThis));
        Assert.Equal([910006u], TitleSeasons.Related(entry).Select(r => r.Id));
    }

    [Fact]
    public void OneSeasonOrAWalkUnderWayIsNoPicker()
    {
        Assert.Empty(TitleSeasons.Of(new Entry(Id: 1, Title: "A")));
        Assert.Empty(TitleSeasons.Of(new Entry(Id: 1, Title: "A", Detail: new TitleDetail(Seasons: [Linked(1, null, 2020)]))));
    }
}

public class NoticesTests
{
    [Fact]
    public void AnEpisodeOutSaysWhereTheViewerIs()
    {
        var said = new[]
        {
            new NewEpisode(900002, "Lantern Road", 9, 0, CoverUrl: "cover", BannerUrl: "banner"),
            new NewEpisode(900003, "Hotaru Atelier", 10, 0, NextForYou: 8),
            new NewEpisode(900004, "Kiriya Pass", 4, 0, Translation: Translation.Dub),
            new NewEpisode(900002, "Lantern Road", 9, 0),
        };
        var notices = Notices.Aired(said);
        Assert.Equal(3, notices.Length);
        Assert.Equal(new SystemNotice("aired-900002-9", "Lantern Road", "Episode 9 is out.", 900002, "9", "banner"), notices[0]);
        Assert.Equal(("Episode 10 is out. Next for you: episode 8.", "8"), (notices[1].Body, notices[1].Play));
        Assert.Equal("Episode 4 is out, dubbed.", notices[2].Body);
        Assert.EndsWith("-dub", notices[2].Tag);
    }

    static DownloadRow Row(long id, uint show, string episode, DownloadState state) =>
        new(id, show, $"Show {show}", episode, state, state == DownloadState.Done ? 1 : 0.5, 0, 0);

    [Fact]
    public void AFinishedDownloadIsSaidOncePerShow()
    {
        DownloadRow[] before = [Row(1, 7, "3", DownloadState.Active), Row(2, 8, "1", DownloadState.Active), Row(3, 8, "2", DownloadState.Queued), Row(4, 9, "1", DownloadState.Done)];
        DownloadRow[] now = [Row(1, 7, "3", DownloadState.Done), Row(2, 8, "1", DownloadState.Done), Row(3, 8, "2", DownloadState.Done), Row(4, 9, "1", DownloadState.Done)];
        var notices = Notices.Downloaded(before, now);
        Assert.Equal(["Episode 3 downloaded.", "2 episodes downloaded."], notices.Select(n => n.Body));
        Assert.Equal("1", notices[1].Play);
        Assert.Empty(Notices.Downloaded(null, now));
        Assert.Empty(Notices.Downloaded(now, now));
    }

    [Fact]
    public void AButtonsArgumentsSayWhatToDo()
    {
        Assert.Equal(("play", 900002u, "9"), Notices.Read(new Dictionary<string, string> { ["action"] = "play", ["show"] = "900002", ["episode"] = "9" }));
        Assert.Null(Notices.Read(new Dictionary<string, string> { ["action"] = "open" }));
    }
}

public class EpisodeDownloadTests
{
    static DownloadRow Row(long id, uint show, string episode, DownloadState state, double fraction = 0.5, ulong total = 100) =>
        new(id, show, $"Show {show}", episode, state, fraction, (ulong)(fraction * total), total);

    [Fact]
    public void AnEpisodeWithNoRowOffersADownload()
    {
        var cell = EpisodeDownload.Of([Row(1, 7, "3", DownloadState.Done)], 7, "4");
        Assert.Equal(EpisodeDownloadKind.None, cell.Kind);
        Assert.True(cell.Asks);
        Assert.Equal("Download episode 4", cell.Say);
    }

    [Fact]
    public void ARunningDownloadSaysHowFar()
    {
        var going = EpisodeDownload.Of([Row(1, 7, "3", DownloadState.Active, 0.456)], 7, "3");
        Assert.Equal("Downloading · 45%", going.Say);
        Assert.False(going.Asks);
        Assert.Equal(EpisodeDownloadKind.Queued, EpisodeDownload.Of([Row(1, 7, "3", DownloadState.Active, 0, 0)], 7, "3").Kind);
        Assert.Equal("Download paused at 50%", EpisodeDownload.Of([Row(1, 7, "3", DownloadState.Paused)], 7, "3").Say);
    }

    [Fact]
    public void AFinishedFileWinsOverAFailedRetry()
    {
        DownloadRow[] rows = [Row(1, 7, "3", DownloadState.Done, 1), Row(2, 7, "3", DownloadState.Failed), Row(3, 8, "3", DownloadState.Active)];
        Assert.Equal(EpisodeDownloadKind.Done, EpisodeDownload.Of(rows, 7, "3").Kind);
        var failed = EpisodeDownload.Of([Row(1, 7, "3", DownloadState.Failed), Row(2, 7, "3", DownloadState.Queued)], 7, "3");
        Assert.Equal(EpisodeDownloadKind.Queued, failed.Kind);
        Assert.True(EpisodeDownload.Of([Row(1, 7, "3", DownloadState.Failed)], 7, "3").Asks);
    }
}

public class LinkTests
{
    const string Https = "https://plugins.example/join/abc#key";

    [Fact]
    public void AnInviteIsReadFromTheStoresAddressOrTheAppsOwn()
    {
        Assert.Equal(new Link.Invite(Https), Link.Parse(Https));
        Assert.Equal(new Link.Invite(Https), Link.Parse("kaeri://plugins.example/join/abc#key"));
        Assert.Equal(new Link.Invite(Https), Link.Parse("kaeri://join?link=" + Uri.EscapeDataString(Https)));
        Assert.Equal(new Link.Invite(Https), Link.Parse($" \"{Https}\"\n"));
        Assert.Equal(new Link.Auth("anilist", "kaeri://auth/anilist#token=x"), Link.Parse("kaeri://auth/anilist#token=x"));
        Assert.Equal("mal", (Link.Parse("kaeri://auth/mal?code=x&state=y") as Link.Auth)?.Tracker);
        Assert.Null(Link.Parse("kaeri://auth/"));
        Assert.DoesNotContain("token", Link.Parse("kaeri://auth/anilist#token=x")!.ToString());
        Assert.Null(Link.Parse("https://plugins.example/about"));
        Assert.Null(Link.Parse("kaeri://join?link=" + Uri.EscapeDataString("http://plugins.example/join/abc#key")));
    }

    [Fact]
    public void AStoresOpenLinkIsReadWholeWithItsMark()
    {
        const string open = "https://plugins.example/join/abc?open#key";
        Assert.Equal(new Link.Invite(open), Link.Parse("kaeri://plugins.example/join/abc?open#key"));
        Assert.Equal(new Link.Invite(open), Link.Parse("kaeri://join?link=" + Uri.EscapeDataString(open)));
    }

    [Fact]
    public void TheWidgetsDestinationsAreReadAsTheMacReadsThem()
    {
        Assert.Equal(new Link.Title(900001), Link.Parse("kaeri://title/900001"));
        Assert.Equal(new Link.Play(900001, 7), Link.Parse("kaeri://play/900001/7"));
        Assert.Equal(new Link.Calendar(), Link.Parse("kaeri://calendar"));
        Assert.Equal(new Link.Accounts(), Link.Parse("KAERI://settings/accounts"));
        Assert.Equal(new Link.Download(42), Link.Parse("kaeri://download/42"));
        Assert.Null(Link.Parse("kaeri://download/x"));
        Assert.Null(Link.Parse("kaeri://title/0"));
        Assert.Null(Link.Parse("kaeri://play/900001"));
        Assert.Null(Link.Parse("kaeri://settings/playback"));
        Assert.Null(Link.Parse("kaeri://"));
        Assert.Null(Link.Parse("--scene=watching"));
    }

    [Fact]
    public void APartyInviteIsHandedOnWholeForTheEngineToRead()
    {
        const string invite = "kaeri://party?server=party.example%3A8999&room=quiet-harbor-k7m2xq9ad4tnwe3p&title=900001&episode=7";
        Assert.Equal(new Link.Party(invite), Link.Parse(invite));
        Assert.Equal(new Link.Party("KAERI://party?room=x"), Link.Parse("\"KAERI://party?room=x\""));
        Assert.Equal(invite, Link.InCommandLine($"\"C:\\kaeri.exe\" \"{invite}\""));
    }

    [Fact]
    public void ALaunchCarriesItsLinkAsAnArgumentOrInItsCommandLine()
    {
        Assert.Equal("kaeri://calendar", Link.InArguments(["--scene=watching", "kaeri://calendar"]));
        Assert.Null(Link.InArguments(["--scene=watching"]));
        Assert.Equal("kaeri://plugins.example/join/abc#key",
            Link.InCommandLine("\"C:\\Program Files\\kaeri\\kaeri.exe\" \"kaeri://plugins.example/join/abc#key\""));
        Assert.Equal("kaeri://calendar", Link.InCommandLine("kaeri.exe kaeri://calendar --x"));
        Assert.Null(Link.InCommandLine("\"C:\\kaeri\\kaeri.exe\""));
    }
}

public class UpdateTests
{
    static readonly byte[] Body = System.Text.Encoding.ASCII.GetBytes(string.Concat(Enumerable.Repeat("fixture installer ", 4000)));
    static string Sha(byte[] bytes) => Convert.ToHexStringLower(System.Security.Cryptography.SHA256.HashData(bytes));

    static ReleaseAsset Asset(string arch, ulong size, string? sha = null) => new(
        Name: $"kaeri-0.11.0-windows-{arch}-setup.exe",
        Url: $"https://releases.example/kaeri-0.11.0-windows-{arch}-setup.exe",
        Sha256: sha ?? Sha(Body),
        Size: size);

    static Release Release(ReleaseAsset? x64, ReleaseAsset? arm64) => new(
        Version: "0.11.0", PublishedAt: 0, PageUrl: "https://releases.example/v0.11.0", Notes: "",
        Macos: null, Ios: null, Android: null, WindowsX64: x64, WindowsArm64: arm64, LinuxX8664: null, LinuxAarch64: null, Beta: false);

    sealed class Serve(byte[] body, System.Net.HttpStatusCode code = System.Net.HttpStatusCode.OK) : HttpMessageHandler
    {
        public int Asked;
        protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancel)
        {
            Asked++;
            return Task.FromResult(new HttpResponseMessage(code) { Content = new ByteArrayContent(body) });
        }
    }

    /// <summary>A server that answers with <paramref name="first"/> bytes of the body, then sends nothing more and never closes; with <paramref name="silent"/> it never answers at all.</summary>
    sealed class Hang(byte[] body, int first, bool silent = false) : HttpMessageHandler
    {
        protected override async Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancel)
        {
            if (silent)
                await Task.Delay(Timeout.Infinite, cancel);
            return new HttpResponseMessage(System.Net.HttpStatusCode.OK) { Content = new StreamContent(new Stalls(body[..first])) };
        }
    }

    sealed class Stalls(byte[] head) : Stream
    {
        int at;
        public override async ValueTask<int> ReadAsync(Memory<byte> into, CancellationToken cancel = default)
        {
            if (at < head.Length)
            {
                var n = Math.Min(into.Length, head.Length - at);
                head.AsMemory(at, n).CopyTo(into);
                at += n;
                return n;
            }
            await Task.Delay(Timeout.Infinite, cancel);
            return 0;
        }
        public override int Read(byte[] buffer, int offset, int count) => throw new NotSupportedException();
        public override bool CanRead => true;
        public override bool CanSeek => false;
        public override bool CanWrite => false;
        public override long Length => throw new NotSupportedException();
        public override long Position { get => at; set => throw new NotSupportedException(); }
        public override void Flush() { }
        public override long Seek(long offset, SeekOrigin origin) => throw new NotSupportedException();
        public override void SetLength(long value) => throw new NotSupportedException();
        public override void Write(byte[] buffer, int offset, int count) => throw new NotSupportedException();
    }

    static string Dir() => Path.Combine(Path.GetTempPath(), "kaeri-update-test-" + Guid.NewGuid().ToString("N"));

    [Fact]
    public async Task ADownloadThatStopsSendingFailsAsALostConnection()
    {
        HomeGuard.Check();
        var dir = Dir();
        try
        {
            var asset = Asset("x64", (ulong)Body.Length);
            var stall = TimeSpan.FromMilliseconds(300);
            var midway = await Updates.Fetch(new HttpClient(new Hang(Body, 1000)), asset, "0.11.0", dir, _ => { }, default, stall);
            Assert.Equal(UpdateStep.Failed, midway.Step);
            Assert.Equal("The download stopped: the connection was lost. Try again.", midway.Reason);
            Assert.Empty(Directory.GetFiles(dir));
            var never = await Updates.Fetch(new HttpClient(new Hang(Body, 0, silent: true)) { Timeout = Timeout.InfiniteTimeSpan }, asset, "0.11.0", dir, _ => { }, default, stall);
            Assert.Equal("The download stopped: the connection was lost. Try again.", never.Reason);
            // Cancelled by the person: no failure to show.
            using var cancel = new CancellationTokenSource(TimeSpan.FromMilliseconds(100));
            Assert.Equal(UpdateStep.Idle, (await Updates.Fetch(new HttpClient(new Hang(Body, 1000)), asset, "0.11.0", dir, _ => { }, cancel.Token, TimeSpan.FromSeconds(30))).Step);
        }
        finally
        {
            if (Directory.Exists(dir))
                Directory.Delete(dir, recursive: true);
        }
    }

    [Fact]
    public void ThisMachinesInstallerIsArm64sOnArmAndX64sOtherwise()
    {
        var (x64, arm64) = (Asset("x64", 1), Asset("arm64", 2));
        Assert.Equal(arm64, Updates.InstallerFor(Release(x64, arm64), System.Runtime.InteropServices.Architecture.Arm64));
        Assert.Equal(x64, Updates.InstallerFor(Release(x64, null), System.Runtime.InteropServices.Architecture.Arm64));
        Assert.Equal(x64, Updates.InstallerFor(Release(x64, arm64), System.Runtime.InteropServices.Architecture.X64));
        Assert.Null(Updates.InstallerFor(Release(null, arm64), System.Runtime.InteropServices.Architecture.X64));
    }

    [Fact]
    public async Task ADownloadIsReadyOnlyWhenItsSizeAndSha256Match()
    {
        HomeGuard.Check();
        var dir = Dir();
        try
        {
            var steps = new List<UpdateStep>();
            var serve = new Serve(Body);
            var asset = Asset("x64", (ulong)Body.Length);
            var ready = await Updates.Fetch(new HttpClient(serve), asset, "0.11.0", dir, s => steps.Add(s.Step), default);
            Assert.Equal(UpdateStep.Ready, ready.Step);
            Assert.Equal(Path.Combine(dir, asset.Name), ready.Path);
            Assert.Contains(UpdateStep.Verifying, steps);
            // Already there and whole: kept, not fetched again.
            Assert.Equal(UpdateStep.Ready, (await Updates.Fetch(new HttpClient(serve), asset, "0.11.0", dir, _ => { }, default)).Step);
            Assert.Equal(1, serve.Asked);

            var wrong = await Updates.Fetch(new HttpClient(new Serve(Body)), Asset("arm64", (ulong)Body.Length, sha: new string('0', 64)), "0.11.0", dir, _ => { }, default);
            Assert.Equal(UpdateStep.Failed, wrong.Step);
            Assert.Equal("The download doesn't match the checksum listed for this version.", wrong.Reason);
            Assert.False(File.Exists(Path.Combine(dir, Asset("arm64", 1).Name)), "a file that does not match is not kept");

            var shorter = await Updates.Fetch(new HttpClient(new Serve(Body[..1000])), Asset("x64", (ulong)Body.Length) with { Name = "short.exe" }, "0.11.0", dir, _ => { }, default);
            Assert.Equal(UpdateStep.Failed, shorter.Step);
            Assert.StartsWith("The download is 1.0 kB, not the", shorter.Reason);

            var refused = await Updates.Fetch(new HttpClient(new Serve([], System.Net.HttpStatusCode.NotFound)), Asset("x64", 5) with { Name = "gone.exe" }, "0.11.0", dir, _ => { }, default);
            Assert.Equal("The download stopped: the server refused it (404). Try again later.", refused.Reason);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    [Fact]
    public void TheInstallerRunsSilentlyAndTheAppOpensAgainAfterIt()
    {
        var command = Updates.RelaunchCommand(@"C:\Temp\kaeri-update\setup.exe", @"C:\Temp\kaeri-update\install.log", @"C:\Apps\kaeri\kaeri.exe");
        Assert.Equal(
            "/d /c ping -n 3 127.0.0.1 >nul & start \"\" /wait \"C:\\Temp\\kaeri-update\\setup.exe\" /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /SP- /CLOSEAPPLICATIONS " +
            "/LOG=\"C:\\Temp\\kaeri-update\\install.log\" & start \"\" \"C:\\Apps\\kaeri\\kaeri.exe\"",
            command);
        var again = Updates.RelaunchCommand("setup.exe", "install.log", "kaeri.exe",
            Updates.RelaunchArguments(["--scene=watching", "--show=update-install", "--update-from=http://127.0.0.1/setup.exe,ab,1,0.11.0", "kaeri://store?url=https://store.example", "--size=1100x700", "a b"]));
        Assert.EndsWith("& start \"\" \"kaeri.exe\" --scene=watching --size=1100x700 \"a b\"", again);
    }

    [Fact]
    public void VersionsCompareNumberByNumber()
    {
        Assert.True(Updates.Older("0.10.9", "0.10.10"));
        Assert.False(Updates.Older("0.11.0", "0.11.0"));
        Assert.False(Updates.Older("0.11.1", "0.11.0"));
        Assert.True(Updates.Older("0.10", "0.10.1"));
    }

    [Fact]
    public void TheLineSaysWhereTheCheckAndTheUpdateStand()
    {
        var available = new UpdateStatus.Available(Release(Asset("x64", 1), null), false);
        Assert.Equal("Not checked yet", Updates.Line(null, UpdateState.Idle, false, false));
        Assert.Equal("Checks once you're back online", Updates.Line(null, UpdateState.Idle, true, false));
        Assert.Equal("Up to date", Updates.Line(new UpdateStatus.UpToDate(null, false), UpdateState.Idle, false, false));
        Assert.Equal("You have 0.13.4, a beta. Updates resume when a newer stable version is out.",
            Updates.Line(new UpdateStatus.UpToDate("0.13.4", true), UpdateState.Idle, false, false));
        Assert.Equal("Not checked. Check your connection and try again.", Updates.Line(new UpdateStatus.Failed("gitHub could not be reached"), UpdateState.Idle, false, false));
        Assert.Equal("Version 0.11.0 is out", Updates.Line(available, UpdateState.Idle, false, true));
        Assert.Equal("Version 0.11.0 is out. Its page has the installer.", Updates.Line(available, UpdateState.Idle, false, false));
        Assert.Equal("Downloading version 0.11.0 · 34 MB of 71 MB",
            Updates.Line(available, new UpdateState(UpdateStep.Downloading, "0.11.0", 34_200_000, 71_400_000), false, true));
        Assert.Equal("Version 0.11.0 is ready to install", Updates.Line(available, new UpdateState(UpdateStep.Ready, "0.11.0"), false, true));
        Assert.Equal("The update to version 0.11.0 stopped. The download stopped: the connection was lost.",
            Updates.Line(available, new UpdateState(UpdateStep.Failed, "0.11.0", Reason: "The download stopped: the connection was lost."), false, true));
        Assert.Equal(0.5, new UpdateState(UpdateStep.Downloading, "0.11.0", 50, 100).Fraction);
        Assert.Null(new UpdateState(UpdateStep.Failed, "0.11.0").Fraction);
    }
}

public class PartyTests
{
    static PartyMember Member(string name, bool? ready, bool you = false, string? file = "Stardust Wayfarer · Episode 7", bool? matches = true) =>
        new(Name: name, Ready: ready, File: file, Duration: 30, FileMatches: matches, You: you);

    [Fact]
    public void TheRoomWaitsForWhoeverIsNotReady()
    {
        var you = Member("Ada", false, you: true);
        Assert.Equal("Nobody else is here yet. Send the invite link.", Parties.ReadyLine([you]));
        Assert.Equal("Play starts once you're ready", Parties.ReadyLine([you, Member("Mika", true)]));
        Assert.Equal("Play starts once Ren is ready", Parties.ReadyLine([you with { Ready = true }, Member("Ren", false)]));
        Assert.Equal("Play starts once you and Ren are ready", Parties.ReadyLine([you, Member("Mika", true), Member("Ren", false)]));
        Assert.Equal("Everyone is ready", Parties.ReadyLine([you with { Ready = true }, Member("Mika", true), Member("Ren", null)]));
        Assert.False(Parties.AllReady([you with { Ready = true }]));
    }

    [Fact]
    public void TheLobbyLineNamesWhoIsNotReadyEvenWhileTheRoomPlays()
    {
        var you = Member("Ada", true, you: true);
        Assert.Equal("Play starts once Ren is ready", Parties.StartLine(null, [you, Member("Ren", false)]));
        Assert.Equal("Everyone is ready", Parties.StartLine(null, [you, Member("Ren", true)]));
        Assert.Equal("Starts at 0:00 when everyone is ready", Parties.StartLine(null, [you with { Ready = false }, Member("Ren", false)]));
        Assert.Equal("Nobody else is here yet. Send the invite link.", Parties.StartLine(null, [you]));
        Assert.StartsWith("Starting", Parties.StartLine(3, [you, Member("Ren", false)]));
    }

    [Fact]
    public void AMemberSaysTheirStateAndAFileThatDiffers()
    {
        Assert.Equal("Ada (you)", Parties.MemberName(Member("Ada", true, you: true)));
        Assert.Equal("Ready", Parties.MemberState(Member("Mika", true)));
        Assert.Equal("No ready state", Parties.MemberState(Member("Ren", null)));
        Assert.Equal("Playing a different file", Parties.MemberFile(Member("Ren", false, matches: false)));
        Assert.Equal("Nothing open yet", Parties.MemberFile(Member("Ren", false, file: null, matches: null)));
        Assert.Null(Parties.MemberFile(Member("Mika", true)));
    }

    [Fact]
    public void TheConnectionLineSaysEncryptionTheRoundTripAndRetries()
    {
        var view = new PartyView(Server: "party.example:8999", Room: "r", Name: "Ada", Connection: new PartyConnection.Connected(),
            Encrypted: true, Ready: false, Episode: null, Invite: null, Motd: null, LatencyMs: 32);
        Assert.Equal("Connected, encrypted · 32 ms", Parties.ConnectionLine(view));
        Assert.Equal("Connection lost. Trying again, 2 of 6", Parties.ConnectionLine(view with { Connection = new PartyConnection.Reconnecting(2, 6) }));
    }

    [Fact]
    public void TheSuggestedServerSaysWhoRunsIt()
    {
        var form = new PartyForm(RecentServers: [], SuggestedServer: "syncplay.pl:8995", SuggestedRunBy: "the Syncplay project", Room: "r", Name: "Ada");
        Assert.Equal("No server of your own? syncplay.pl:8995 is a public one, run by the Syncplay project.", Parties.ServerHint("", form));
        Assert.Equal("A public server run by the Syncplay project, not by Kaeri.", Parties.ServerHint(" syncplay.pl:8995 ", form));
    }

    [Fact]
    public void AServerReadsAsAHostAndAPort()
    {
        foreach (var ok in new[] { "", "party.example", "party.example:8999", "[::1]:8999", "127.0.0.1:8999" })
            Assert.Null(Parties.ServerProblem(ok));
        foreach (var bad in new[] { "party.example:", "party.example:0", "party.example:99999", "https://party.example", "a:b:c", ":8999", "[::1" })
            Assert.NotNull(Parties.ServerProblem(bad));
    }
}

public class StatsPanelTests
{
    sealed class Clock
    {
        public TimeSpan Now = TimeSpan.FromMinutes(5);
        public void Pass(double seconds) => Now += TimeSpan.FromSeconds(seconds);
    }

    static PlayerStats Stats(bool waiting = false, params (string Label, string Value, bool Warn)[] rows) =>
        new([.. rows.Select(r => new DetailRow(r.Label, r.Value, r.Warn))], [], waiting);

    [Fact]
    public void ToggledItComesUpThenGoesAwayAndTellsThePlayerEachTime()
    {
        // I, the close button and the Settings check all toggle; the key itself is a XAML
        // accelerator, which these tests without WinUI can't press.
        var clock = new Clock();
        var panel = new StatsPanel(() => clock.Now);
        Assert.Equal(new PlayerCommand.Stats(true), panel.Toggle());
        Assert.True(panel.Shown);
        clock.Pass(0.7);
        Assert.NotNull(panel.View());
        Assert.Equal(new PlayerCommand.Stats(false), panel.Toggle());
        Assert.False(panel.Shown);
        Assert.Null(panel.View());
        Assert.Equal(new PlayerCommand.Stats(true), panel.Toggle());
        Assert.True(panel.Shown);
    }

    [Fact]
    public void OpenedBeforeTheFileItShowsNothingThenDashesThenTheWaitingLine()
    {
        var clock = new Clock();
        var panel = new StatsPanel(() => clock.Now);
        panel.Show(true);
        Assert.Null(panel.View());
        clock.Pass(0.7);
        var dashes = panel.View();
        Assert.NotNull(dashes);
        Assert.NotEmpty(dashes.Rows);
        Assert.All(dashes.Rows, row => Assert.False(row.Warn));
        Assert.False(dashes.WaitingLine);
        Assert.False(dashes.CanCopy);
        clock.Pass(3.5);
        Assert.True(panel.View()!.WaitingLine);
    }

    [Fact]
    public void TheEnginesRowsReplaceTheDashesAndCopyStatsWakes()
    {
        var clock = new Clock();
        var panel = new StatsPanel(() => clock.Now);
        panel.Show(true);
        Assert.True(panel.Took(Stats(false, ("Video", "1280 × 720 · H.264 · 24 fps", false), ("Buffer", "0 s ahead, waiting", true))));
        var view = panel.View()!;
        Assert.Equal(["Video", "Buffer"], view.Rows.Select(r => r.Label));
        Assert.True(view.Rows[1].Warn);
        Assert.True(view.CanCopy);
        Assert.False(view.WaitingLine);
    }

    [Fact]
    public void ShutItSaysNothingAndPassesOverTheEngine()
    {
        var clock = new Clock();
        var panel = new StatsPanel(() => clock.Now);
        Assert.False(panel.Took(Stats(false, ("Video", "720p", false))));
        Assert.Null(panel.View());
        Assert.False(panel.Copied());
        panel.Show(true);
        // The summary alone, with no rows, as the engine says it when the panel shuts.
        Assert.False(panel.Took(Stats()));
    }

    [Fact]
    public void CopiedStandsInForTheButtonForAMoment()
    {
        var clock = new Clock();
        var panel = new StatsPanel(() => clock.Now);
        panel.Show(true);
        panel.Took(Stats(false, ("Video", "720p", false)));
        Assert.True(panel.Copied());
        Assert.True(panel.View()!.Copied);
        clock.Pass(1.6);
        Assert.False(panel.View()!.Copied);
    }

    [Fact]
    public void ANewPlaybackWaitsAgainAndTheMiniPlayerHidesIt()
    {
        var clock = new Clock();
        var panel = new StatsPanel(() => clock.Now);
        panel.Show(true);
        panel.Took(Stats(false, ("Video", "720p", false)));
        panel.Mini = true;
        Assert.Null(panel.View());
        panel.Mini = false;
        Assert.NotNull(panel.View());
        panel.Restart();
        Assert.Null(panel.View());
        clock.Pass(1);
        Assert.False(panel.View()!.CanCopy);
        Assert.True(panel.Shown);
    }

    [Fact]
    public void AStagedShotHoldsItsRows()
    {
        var clock = new Clock();
        var panel = new StatsPanel(() => clock.Now);
        panel.Stage(KaeriFfiMethods.PlayerStatsFixture(StatsFixture.Buffering), null, copied: true);
        Assert.False(panel.Took(Stats(false, ("Video", "720p", false))));
        var view = panel.View()!;
        Assert.Single(view.Rows, r => r.Warn);
        Assert.Equal("Buffer", view.Rows.Single(r => r.Warn).Label);
        clock.Pass(30);
        Assert.True(panel.View()!.Copied);
    }

    [Fact]
    public void CopyStatsNamesTheSystemAndTheModelNotTheMachine()
    {
        Assert.Equal("Windows 11 24H2 (build 26100.2894)", StatsAbouts.System(26100, 2894, "24H2"));
        Assert.Equal("Windows 10 (build 19045)", StatsAbouts.System(19045, 0, null));
        Assert.Equal("21HMCTO1WW (ThinkPad X1 Carbon Gen 11)", StatsAbouts.Device("21HMCTO1WW", "ThinkPad X1 Carbon Gen 11"));
        Assert.Equal("Surface Laptop, 7th Edition", StatsAbouts.Device("Surface Laptop, 7th Edition", "124I:00036T:000M:0300000D:11B:01F"));
        Assert.Equal("Surface Laptop, 7th Edition", StatsAbouts.Device("Surface Laptop, 7th Edition", "1.0"));
        Assert.Equal("", StatsAbouts.Device("To be filled by O.E.M.", "System Version"));
        Assert.Equal("Virtual Machine", StatsAbouts.Device(" Virtual Machine ", null));
    }
}

public class SubtitleLiftTests
{
    sealed class Clock
    {
        public TimeSpan Now = TimeSpan.FromMinutes(5);
        public void Pass(double ms) => Now += TimeSpan.FromMilliseconds(ms);
    }

    [Fact]
    public void TheyClearTheBarButNeverGoPastHalfWay()
    {
        Assert.Equal(87.5, SubtitleLift.Position(90, 720));
        Assert.Equal(91.7, SubtitleLift.Position(90, 1080));
        Assert.Equal(50, SubtitleLift.Position(300, 400));
        Assert.Equal(SubtitleLift.Bottom, SubtitleLift.Position(0, 720));
        Assert.Equal(SubtitleLift.Bottom, SubtitleLift.Position(90, 0));
    }

    [Fact]
    public void TheyRiseWithTheFadeInAndSettleWithTheFadeOut()
    {
        var clock = new Clock();
        var lift = new SubtitleLift(() => clock.Now);
        Assert.Equal(SubtitleLift.Bottom, lift.Next());
        Assert.Null(lift.Next());

        lift.Head(88, TimeSpan.FromMilliseconds(200));
        Assert.True(lift.Moving);
        clock.Pass(100);
        // Decelerating: most of the way up by half the time.
        var halfUp = lift.Next();
        Assert.NotNull(halfUp);
        Assert.InRange(halfUp.Value, 88, 92);
        clock.Pass(150);
        Assert.False(lift.Moving);
        Assert.Equal(88, lift.Next());
        Assert.Null(lift.Next());

        lift.Head(SubtitleLift.Bottom, TimeSpan.FromMilliseconds(300));
        clock.Pass(150);
        // Accelerating: still near the top by half the time.
        var halfDown = lift.Next();
        Assert.NotNull(halfDown);
        Assert.InRange(halfDown.Value, 88, 91);
        clock.Pass(200);
        Assert.Equal(SubtitleLift.Bottom, lift.Next());
    }

    [Fact]
    public void ABarComingBackMidwayTurnsThemFromWhereTheyAre()
    {
        var clock = new Clock();
        var lift = new SubtitleLift(() => clock.Now);
        lift.Head(88, TimeSpan.Zero);
        Assert.Equal(88, lift.Next());
        lift.Head(SubtitleLift.Bottom, TimeSpan.FromMilliseconds(300));
        clock.Pass(200);
        var turned = lift.At();
        Assert.Equal(Math.Round(turned, 1), lift.Next());
        lift.Head(88, TimeSpan.FromMilliseconds(200));
        Assert.Equal(turned, lift.At(), 6);
        clock.Pass(200);
        Assert.Equal(88, lift.Next());
    }

    [Fact]
    public void ANewPlaybackIsToldAgainAtOnce()
    {
        var clock = new Clock();
        var lift = new SubtitleLift(() => clock.Now);
        lift.Head(88, TimeSpan.Zero);
        Assert.Equal(88, lift.Next());
        lift.Forget();
        // mpv keeps sub-pos from the last file: the bottom is said again though it is the default.
        Assert.Equal(SubtitleLift.Bottom, lift.Next());
        lift.Head(88, TimeSpan.Zero);
        Assert.False(lift.Moving);
        Assert.Equal(88, lift.Next());
    }
}

public class LawfulLinesTests
{
    static readonly DateTimeOffset Now = new(2026, 10, 31, 12, 0, 0, TimeSpan.Zero);

    static ServerRow Server(MatchProgress? matching = null, bool signedOutHere = false, bool notAnswering = false, long? lastSeen = null) =>
        new(Id: "s1", Name: "Mika's Jellyfin", Address: "http://media.example:8096", Version: "10.10.3", User: "mika",
            Libraries: [], Series: 84, Matched: 12, Report: true, SignedOut: signedOutHere, SignedOutHere: signedOutHere, Matching: matching, NotAnswering: notAnswering, LastSeen: lastSeen);

    static FolderRow Folder(FolderScan? scan = null, uint series = 18, uint episodes = 214, uint needsLook = 3) =>
        new(Path: @"D:\Media", Files: 300, Available: true, Scan: scan, Series: series, Episodes: episodes, NeedsLook: needsLook);

    [Fact]
    public void AServerThatDoesNotAnswerSaysWhenItLastDid()
    {
        var seen = Now.AddHours(-2).ToUnixTimeSeconds();
        var row = Server(notAnswering: true, lastSeen: seen);
        Assert.Equal(T.ServersRowNotAnswering(Lawful.SeenAt(seen, Now)), Lawful.ServerLine(row, new Known(), Now));
        Assert.Equal(T.ServersRowSignedOutHere, Lawful.ServerLine(Server(signedOutHere: true, notAnswering: true, lastSeen: seen), new Known(), Now));
    }

    [Fact]
    public void AServerMatchingSaysHowFarWithoutAWait()
    {
        var line = Lawful.ServerLine(Server(new MatchProgress(Done: 12, Total: 84, WaitSeconds: null)), new Known(), Now);
        Assert.Equal(T.ServersRowMatching(12, 84), line);
    }

    [Fact]
    public void AServerSignedOutHereSaysSoRatherThanBlamingTheServer()
    {
        Assert.Equal(T.ServersRowSignedOutHere, Lawful.ServerLine(Server(signedOutHere: true), new Known(), Now));
    }

    [Fact]
    public void AFolderCountsFilesThenSumsUpWhatItHolds()
    {
        var known = new Known();
        Assert.Equal(T.LibraryRowScanning(1204), Lawful.FolderLine(Folder(new FolderScan.Scanning(1204)), known, Now));
        var done = Lawful.FolderLine(Folder(), known, Now);
        Assert.Equal($"{T.LibraryRowSummary(T.LibraryEpisodes(214), T.LibrarySeries(18))} · {T.LibraryRowNeedsLook(3)}", done);
    }

    [Fact]
    public void TheWaitForTheLimitCountsDownFromWhenItWasSaid()
    {
        var known = new Known();
        var matching = new MatchProgress(Done: 31, Total: 58, WaitSeconds: 12);
        known.Take(new Event.Local(new LocalEvent.View(new LocalView(Folders: [Folder(new FolderScan.Matching(matching))], Groups: [], Scanning: true))));
        var start = DateTimeOffset.UtcNow;
        Assert.InRange(known.WaitingSeconds(@"f:D:\Media", start.AddSeconds(2)) ?? -1, 9, 10);
        Assert.Null(known.WaitingSeconds(@"f:D:\Media", start.AddSeconds(13)));
        Assert.True(known.AnyWaiting(start));
        Assert.False(known.AnyWaiting(start.AddSeconds(13)));
    }

    [Fact]
    public void TheAvailableFilterGoesToTheEngineAndNarrowsTheRow()
    {
        var filters = Searches.Nothing with { Available = Available.PlaysHere };
        Assert.True(filters.Narrows());
        Assert.Equal(Available.PlaysHere, Searches.With("lantern", filters).Available);
        Assert.False(Searches.Same(Searches.With("lantern", filters), Searches.With("lantern", Searches.Nothing)));
    }

    [Fact]
    public void AResultsLineEndsWhereItPlays()
    {
        Assert.Equal(T.SearchResultInLibrary, Wording.Where(new AvailableAt.InLibrary()));
        Assert.Equal(T.SearchResultOnServer("Mika's Jellyfin"), Wording.Where(new AvailableAt.OnServer("Mika's Jellyfin")));
        Assert.Equal(T.SearchResultOnService("Lantern Library"), Wording.Where(new AvailableAt.OnService("Lantern Library")));
        Assert.Equal(T.SearchResultPlaysHere, Wording.Where(new AvailableAt.PlaysHere()));
    }
}
