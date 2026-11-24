// The player's Episodes and Sources panel: its rows' words and buttons, the other sources' groups,
// and a switch as the engine says it goes.

using System.Globalization;
using Kaeri.App;
using Kaeri.Engine.Core;
using Kaeri.Engine.Model;
using Xunit;

namespace Kaeri.App.Tests;

public class PlayerPanelTests
{
    const long Now = 1_800_000_000;

    static SourceCandidate Release(string provider, string id, bool auto = false, bool dubbed = false, bool dual = false) =>
        new(id, provider, ProviderName: provider == "harbor" ? "Harbor Mirror" : "Tidewater", Title: $"Stardust Wayfarer - {id}",
            Quality: 1080, Label: "lantern-archive", Size: "1.4 GiB", DualAudio: dual, Dubbed: dubbed, AutoPick: auto);

    static SourceOption Option(string id, string name, SourceNote? note = null) => new(id, name, 9, 12, false, note, [], null);

    [Fact]
    public void ARowSaysWhatTheTableKnowsOfIt()
    {
        var row = new EpisodeRow("5", Title: "The Lantern Fair", DurationSecs: 1440, Source: "Harbor Mirror");
        Assert.Equal("E5 The Lantern Fair", PanelWords.Heading(row));
        Assert.Equal("Episode 5", PanelWords.Heading(row with { Title = " " }));
        Assert.Equal("24 min · From Harbor Mirror", PanelWords.Detail(row, false, false, 0, null, Now));
        // The source is not named where no source lists the show.
        Assert.Equal("24 min", PanelWords.Detail(row, false, true, 0, null, Now));
        Assert.Equal($"{T.TimeMinutesLeft(12)} · From Harbor Mirror", PanelWords.Detail(row with { Watched = 0.5 }, false, false, 0, null, Now));
        Assert.Equal($"{T.EpisodeWatched} · From Harbor Mirror", PanelWords.Detail(row with { Completed = true }, false, false, 0, null, Now));
        Assert.Equal(T.EpisodeRowPlaying(T.TimeMinutesLeft(11)), PanelWords.Detail(row, true, false, 1440 - 660, 1440, Now));
        Assert.Equal($"{T.EpisodeFiller} · {T.EpisodeUnsourced}", PanelWords.Detail(row with { Kind = EpisodeKind.Filler, Unsourced = true }, false, false, 0, null, Now));
        var later = row with { AirsAt = Now + 3600, DelayedFrom = Now - 3600 };
        Assert.Equal($"{T.EpisodeRowAirs("soon")} · {T.EpisodeRowDelayed("soon")}", PanelWords.Detail(later, false, false, 0, null, Now, _ => "soon"));
    }

    [Fact]
    public void ABroadcastSaysTodayTomorrowItsDayOrItsDate()
    {
        var gb = CultureInfo.GetCultureInfo("en-GB");
        var utc = TimeZoneInfo.Utc;
        // Thursday 14 January 2027, 10:00 UTC.
        var now = new DateTimeOffset(2027, 1, 14, 10, 0, 0, TimeSpan.Zero).ToUnixTimeSeconds();
        long At(int days, int hour) => new DateTimeOffset(2027, 1, 14 + days, hour, 30, 0, TimeSpan.Zero).ToUnixTimeSeconds();
        Assert.Equal(T.CauseAirsToday("16:30"), PanelWords.AirsWords(At(0, 16), now, utc, gb));
        Assert.Equal(T.CauseAirsTomorrow("16:30"), PanelWords.AirsWords(At(1, 16), now, utc, gb));
        Assert.Equal(T.CauseAirsOnDate("Monday", "16:30"), PanelWords.AirsWords(At(4, 16), now, utc, gb));
        Assert.Equal(T.CauseAirsOnDate("24 Jan", "16:30"), PanelWords.AirsWords(At(10, 16), now, utc, gb));
    }

    [Fact]
    public void TheStillShowsThePlayheadOrTheHistory()
    {
        var row = new EpisodeRow("3", DurationSecs: 1000, Watched: 0.25);
        Assert.Equal(0.5, PanelWords.Progress(row, true, false, 500, 1000));
        Assert.Equal(0.25, PanelWords.Progress(row, false, false, 0, null));
        Assert.Null(PanelWords.Progress(row with { Completed = true }, false, false, 0, null));
        Assert.Null(PanelWords.Progress(row, false, true, 0, null));
    }

    [Fact]
    public void ARevealedRowOffersWhatTheBoardGivesIt()
    {
        var row = new EpisodeRow("4", DurationSecs: 1440);
        Assert.Equal([EpisodeAction.Play, EpisodeAction.MarkWatched], PanelWords.Actions(row, false, false, false));
        Assert.Equal([EpisodeAction.FromBeginningPlaying], PanelWords.Actions(row, true, false, false));
        Assert.Equal([EpisodeAction.Resume, EpisodeAction.FromBeginning], PanelWords.Actions(row with { Watched = 0.4 }, false, false, false));
        Assert.Equal([EpisodeAction.Play], PanelWords.Actions(row with { Completed = true }, false, false, false));
        Assert.Empty(PanelWords.Actions(row, false, false, true));
        Assert.Equal([EpisodeAction.MarkWatched], PanelWords.Actions(row with { Unsourced = true }, false, false, false));
        Assert.Equal([EpisodeAction.MarkWatched], PanelWords.Actions(row, false, true, false));
        Assert.Empty(PanelWords.Actions(row with { Completed = true }, false, true, false));
        Assert.True(PanelWords.Primary(EpisodeAction.Resume));
        Assert.False(PanelWords.Primary(EpisodeAction.MarkWatched));
    }

    [Fact]
    public void AReleaseIsWordedFromItsOwnFields()
    {
        Assert.Equal("1080p · 1.4 GiB · lantern-archive · Dubbed · Automatic picks this",
            PanelWords.Release(Release("harbor", "05", auto: true, dubbed: true)));
        Assert.EndsWith(T.PlayerPanelDualAudio.Replace(' ', ' '), PanelWords.Release(Release("harbor", "05", dual: true)));
    }

    [Fact]
    public void OtherSourcesKeepTheirOrderAndTheirReleasesTheSources()
    {
        SourceOption[] options =
        [
            Option("harbor", "Harbor Mirror"), Option("tidewater", "Tidewater"), Option("north", "North Shelf"),
            Option("quiet", "Quiet Reel", new SourceNote.Resting()),
        ];
        var releases = new PanelReleases(7, "5", null);
        // Before the releases come, each source waits but the resting one.
        var waiting = PanelWords.Others(options, releases, "harbor");
        Assert.Equal(["tidewater", "north", "quiet"], waiting.Select(g => g.ProviderId));
        Assert.IsType<SourceGroup.Waiting>(waiting[0]);
        Assert.IsType<SourceGroup.Dim>(waiting[2]);
        releases = releases with { Candidates = [Release("tidewater", "b"), Release("harbor", "a"), Release("tidewater", "a")] };
        var groups = PanelWords.Others(options, releases, null);
        var two = Assert.IsType<SourceGroup.Releases>(groups[1]);
        // The source's own order, not ranked.
        Assert.Equal(["b", "a"], two.List.Select(r => r.Id));
        Assert.IsType<SourceGroup.One>(groups[0]);
        Assert.IsType<SourceGroup.Dim>(groups[2]);
        Assert.IsType<SourceGroup.Dim>(groups[3]);
        // With no listing of sources, the releases' own sources stand in.
        Assert.Equal(["harbor"], PanelWords.Others(null, releases, "tidewater").Select(g => g.ProviderId));
    }

    [Fact]
    public void DubbedWaitsForTheDubToReachTheEpisode()
    {
        Assert.True(PanelWords.PastDub(new DubExtent(8, false), "9"));
        Assert.False(PanelWords.PastDub(new DubExtent(8, false), "8"));
        Assert.False(PanelWords.PastDub(new DubExtent(8, true), "9"));
        Assert.False(PanelWords.PastDub(null, "9"));
    }

    [Fact]
    public void APickWaitsSilentlyThenSpinsThenCounts()
    {
        var pick = new PanelSwitch(new SwitchTarget("tidewater", "Tidewater", "b"), new PanelPhase.Asked(), 1000);
        Assert.False(pick.Spinning(1599));
        Assert.True(pick.Spinning(1600));
        Assert.Null(pick.Counting(3999));
        Assert.Equal(3, pick.Counting(4000));
        Assert.Equal(5, pick.Counting(6500));
        var failed = pick with { Phase = new PanelPhase.Failed(new Msg.PlayerPanelSwitchFailed("Tidewater", "Harbor Mirror")) };
        Assert.False(failed.Spinning(9000));
        Assert.Null(failed.Counting(9000));
    }

    [Fact]
    public void ARowIsMatchedByItsReleaseOrItsStream()
    {
        var release = new SwitchTarget("tidewater", "Tidewater", "b");
        Assert.True(release.Same(new SwitchTarget("tidewater", "Another name", "b")));
        Assert.False(release.Same(new SwitchTarget("harbor", "Harbor Mirror", "b")));
        Assert.False(release.Same(new SwitchTarget("", "Tidewater", Stream: "b")));
        var stream = new SwitchTarget("", "Harbor Mirror", Stream: "s2");
        Assert.True(stream.Same(new SwitchTarget("harbor", "Harbor Mirror", Stream: "s2")));
    }

    [Fact]
    public void TheReleasesAreAskedForOncePerEpisode()
    {
        var panel = new PanelState();
        Assert.Null(panel.AskReleases());
        panel.Playing(7, "5");
        Assert.Equal((7u, "5"), panel.AskReleases());
        Assert.Null(panel.AskReleases());
        Assert.False(panel.Listed(7, "6", []));
        Assert.True(panel.Listed(7, "5", [Release("harbor", "a")]));
        Assert.Equal((7u, "5"), panel.AskReleases(again: true));
        panel.Playing(7, "6");
        Assert.Null(panel.Releases);
        Assert.Equal((7u, "6"), panel.AskReleases());
    }

    [Fact]
    public void ASwitchHoldsWhileItLoadsAndLandsOnItsRelease()
    {
        var panel = new PanelState { Open = true };
        panel.Playing(7, "5");
        panel.AskReleases();
        var picked = Release("tidewater", "b");
        panel.Listed(7, "5", [Release("harbor", "a"), picked]);
        panel.Picked(SwitchTarget.Of(picked), 100, picked);
        var to = new SwitchTo("tidewater", "Tidewater", "b", null);
        Assert.Equal(PanelState.Effect.None, panel.Switched(new SourceSwitch.Finding(to), 250));
        Assert.IsType<PanelPhase.Finding>(panel.Switch?.Phase);
        // The row keeps its wait from the pick.
        Assert.Equal(100, panel.Switch?.Since);
        Assert.Equal(PanelState.Effect.Hold, panel.Switched(new SourceSwitch.Loading(to, 312.5), 900));
        Assert.True(panel.Holding);
        Assert.Equal(312.5, panel.HoldAt);
        Assert.Equal(PanelState.Effect.Release, panel.Switched(new SourceSwitch.Landed(to, 313), 1500));
        Assert.False(panel.Holding);
        Assert.Null(panel.Switch);
        Assert.Equal("Tidewater", panel.PlayingSource);
        Assert.Equal(picked, panel.PlayingRelease);
        Assert.Equal(1, panel.Landings);
    }

    [Fact]
    public void AnotherSourceFoundLetsGoOfTheLandedRelease()
    {
        var panel = new PanelState { Open = true };
        panel.Playing(7, "5");
        panel.AskReleases();
        var picked = Release("tidewater", "b");
        panel.Listed(7, "5", [Release("harbor", "a"), picked]);
        panel.Picked(SwitchTarget.Of(picked), 100, picked);
        var to = new SwitchTo("tidewater", "Tidewater", "b", null);
        panel.Switched(new SourceSwitch.Landed(to, 313), 1500);
        // The same source said again keeps the release.
        panel.PlayingSource = "Tidewater";
        Assert.Equal(picked, panel.PlayingRelease);
        // The episode started again elsewhere, as for a dub.
        panel.PlayingSource = "Harbor Mirror";
        Assert.Equal("Harbor Mirror", panel.PlayingSource);
        Assert.Null(panel.PlayingRelease);
    }

    [Fact]
    public void ALandingWithThePanelClosedIsSaidInANotice()
    {
        var panel = new PanelState();
        panel.Playing(7, "5");
        var to = new SwitchTo("tidewater", "Tidewater", "b", null);
        Assert.Equal(PanelState.Effect.ReleaseAndNotice, panel.Switched(new SourceSwitch.Landed(to, 10), 0));
        // A stream from the menu lands quietly.
        Assert.Equal(PanelState.Effect.Release, panel.Switched(new SourceSwitch.Landed(new SwitchTo("", "Harbor Mirror", null, "s2"), 10), 0));
    }

    [Fact]
    public void AFailureStaysOnItsRowAndLetsTheHoldGo()
    {
        var panel = new PanelState { Open = true };
        panel.Playing(7, "5");
        var picked = Release("tidewater", "b");
        panel.Picked(SwitchTarget.Of(picked), 0, picked);
        var to = new SwitchTo("tidewater", "Tidewater", "b", null);
        panel.Switched(new SourceSwitch.Loading(to, 40), 100);
        var text = new Msg.PlayerPanelSwitchFailed("Tidewater", "Harbor Mirror");
        Assert.Equal(PanelState.Effect.Release, panel.Switched(new SourceSwitch.Failed(to, 40, text, null), 5000));
        Assert.False(panel.Holding);
        Assert.Equal(text, panel.Switch?.Failed);
        Assert.Equal(picked, panel.Switch?.Candidate);
        Assert.Equal(0, panel.Landings);
    }

    [Fact]
    public void ASwitchThePanelDidNotPickGetsARowOfItsOwn()
    {
        var panel = new PanelState();
        panel.Playing(7, "5");
        panel.Picked(new SwitchTarget("tidewater", "Tidewater", "b"), 0);
        panel.Switched(new SourceSwitch.Loading(new SwitchTo("", "Harbor Mirror", null, "s2"), 5), 700);
        Assert.Equal("s2", panel.Switch?.Target.Stream);
        Assert.Equal(700, panel.Switch?.Since);
    }

    [Fact]
    public void TheOtherVersionFollowsFindingLoadingAndLanded()
    {
        var panel = new PanelState { Open = true };
        panel.Playing(7, "5");
        panel.AskReleases();
        var picked = Release("tidewater", "b");
        panel.Listed(7, "5", [Release("harbor", "a"), picked]);
        panel.Picked(SwitchTarget.Of(picked), 0, picked);
        panel.Switched(new SourceSwitch.Landed(new SwitchTo("tidewater", "Tidewater", "b", null), 5), 0);

        Assert.True(panel.PickedVersion(Translation.Dub, Translation.Sub, 100));
        Assert.IsType<PanelPhase.Asked>(panel.Version?.Phase);
        Assert.False(panel.PickedVersion(Translation.Dub, Translation.Sub, 150), "one at a time");
        var dub = new SwitchTo("", "", null, null, Translation.Dub);
        Assert.Equal(PanelState.Effect.None, panel.Switched(new SourceSwitch.Finding(dub), 200));
        Assert.IsType<PanelPhase.Finding>(panel.Version?.Phase);
        Assert.Equal(100, panel.Version?.Since);
        Assert.Null(panel.Switch);
        var found = new SwitchTo("harbor", "Harbor Mirror", null, null, Translation.Dub);
        Assert.Equal(PanelState.Effect.Hold, panel.Switched(new SourceSwitch.Loading(found, 40), 900));
        Assert.True(panel.Holding);
        Assert.Equal(PanelState.Effect.ReleaseAndRelist, panel.Switched(new SourceSwitch.Landed(found, 40), 1500));
        Assert.Null(panel.Version);
        Assert.False(panel.Holding);
        Assert.Equal("Harbor Mirror", panel.PlayingSource);
        Assert.Null(panel.PlayingRelease);
    }

    [Fact]
    public void TheOtherVersionWaitsInSilenceThenSpinsThenCounts()
    {
        var version = new VersionSwitch(Translation.Dub, new PanelPhase.Finding(), 0);
        Assert.False(version.Spinning(599));
        Assert.True(version.Spinning(600));
        Assert.Null(version.Note("5", 2_999));
        Assert.Equal(T.PlayerFindingVersionSeconds("5", 4), version.Note("5", 4_200));
        var sub = version with { To = Translation.Sub };
        Assert.Equal(T.PlayerFindingVersionSubbedSeconds("5", 3), sub.Note("5", 3_000));
    }

    [Fact]
    public void TheOtherVersionThatFailsSaysSoAndPlaysOn()
    {
        var panel = new PanelState { Open = true };
        panel.Playing(7, "5");
        panel.PickedVersion(Translation.Dub, Translation.Sub, 0);
        var dub = new SwitchTo("harbor", "Harbor Mirror", null, null, Translation.Dub);
        panel.Switched(new SourceSwitch.Loading(dub, 40), 100);
        var text = new Msg.PlayerPanelDubFailed("5");
        Assert.Equal(PanelState.Effect.Release, panel.Switched(new SourceSwitch.Failed(dub, 40, text, null), 5000));
        Assert.False(panel.Holding);
        Assert.Equal(text, panel.Version?.Failed);
        Assert.False(panel.Version!.Spinning(9000));
        Assert.Null(panel.Version.Note("5", 9000));
        // Picked again from the switch.
        Assert.True(panel.PickedVersion(Translation.Dub, Translation.Sub, 9000));
        // With the panel closed, the player says it.
        panel.Open = false;
        Assert.Equal(PanelState.Effect.ReleaseAndAlert, panel.Switched(new SourceSwitch.Failed(dub, 40, text, null), 9500));
    }

    [Fact]
    public void AnotherSourceTheEngineMovedToLetsGoOfTheRelease()
    {
        var panel = new PanelState { Open = true };
        panel.Playing(7, "5");
        panel.AskReleases();
        var picked = Release("tidewater", "b");
        panel.Listed(7, "5", [picked]);
        panel.Picked(SwitchTarget.Of(picked), 0, picked);
        panel.Switched(new SourceSwitch.Landed(new SwitchTo("tidewater", "Tidewater", "b", null), 5), 0);
        // Another stream of the release keeps it.
        panel.Switched(new SourceSwitch.Landed(new SwitchTo("tidewater", "Tidewater", null, "2"), 60), 0);
        Assert.Equal(picked, panel.PlayingRelease);
        // Its stream broke off and the engine moved to another source.
        panel.Switched(new SourceSwitch.Landed(new SwitchTo("harbor", "Harbor Mirror", null, "3"), 90), 0);
        Assert.Equal("Harbor Mirror", panel.PlayingSource);
        Assert.Null(panel.PlayingRelease);
    }

    [Fact]
    public void AStartAgainForgetsTheVersionOnItsWay()
    {
        var panel = new PanelState();
        panel.Playing(7, "5");
        panel.PickedVersion(Translation.Dub, Translation.Sub, 0);
        panel.Restarted();
        Assert.Null(panel.Version);
        Assert.False(panel.PickedVersion(Translation.Sub, Translation.Sub, 10), "the version playing is no switch");
    }

    [Fact]
    public void AnotherEpisodeDropsTheReleasesAndThePicks()
    {
        var panel = new PanelState();
        panel.Playing(7, "5");
        panel.AskReleases();
        var picked = Release("tidewater", "b");
        panel.Picked(SwitchTarget.Of(picked), 0, picked);
        panel.Switched(new SourceSwitch.Landed(new SwitchTo("tidewater", "Tidewater", "b", null), 5), 0);
        panel.Playing(7, "6");
        Assert.Null(panel.Releases);
        Assert.Null(panel.Switch);
        Assert.Null(panel.PlayingRelease);
    }

    static SourceListing Listing(string id, string name, ListingState state) => new(id, name, state);

    static SourceListing[] Rows(params (string Id, ListingState State)[] rows) =>
        [.. rows.Select(r => Listing(r.Id, r.Id switch
        {
            "fixture" => "Fixture Source",
            "home" => "Home server",
            "folder" => "Media folder",
            "index" => "Example index",
            "second" => "Second example",
            "archive" => "Archive example",
            _ => "Old example",
        }, r.State))];

    static ListingState Asking => new ListingState.Asking();

    static ListingState Listed(params SourceCandidate[] releases) => new ListingState.Listed(releases);

    static ListingState Missed(ListingMiss miss) => new ListingState.Missed(miss);

    /// <summary>What each row under Other sources says at <paramref name="now"/>: name, words, spinner, haze, pickable.</summary>
    static (string Name, string Words, bool Spinner, bool Haze, bool Pick)[] Said(PanelState panel, long now, SourceOption[]? options = null) =>
        [.. PanelWords.Others(options, panel.Releases, "fixture").Select(g =>
        {
            var line = panel.Line(g, now);
            return (g.Name, line.Words, line.Spinner, line.Haze, g is SourceGroup.One or SourceGroup.Releases);
        })];

    /// <summary>
    /// The SourcesLoading scene's sources as the engine says them, from the tab's ask at 1000 ms on
    /// the panel's clock: Home server lists one release at 0.45 s, Media folder can't match the
    /// title at 1.3 s, Example index lists five at 2.4 s, Archive example fails at 6.2 s and Second
    /// example takes too long at 20 s; Old example is switched off. The rows at the board's five
    /// frames: 0.3 s, 1 s, 4 s, 12 s and 21 s after the ask.
    /// </summary>
    [Fact]
    public void SourcesLoadingSaysEachSourcesRowAsItAnswers()
    {
        var panel = new PanelState { Open = true };
        panel.Playing(900001, "7");
        const long asked = 1000;
        Assert.Equal((900001u, "7"), panel.AskReleases(now: asked));
        var one = Release("home", "h-1");
        SourceCandidate[] five = [.. Enumerable.Range(1, 5).Select(i => Release("index", $"i-{i}"))];
        SourceOption[] options = [Option("home", "Home server"), Option("folder", "Media folder"), Option("index", "Example index"), Option("second", "Second example"), Option("archive", "Archive example"), Option("old", "Old example")];
        var coverage = T.SourcePickerCoverage(9, 12);
        var off = new ListingState.Off();
        (string, ListingState) Fixture() => ("fixture", Missed(new ListingMiss.NoEpisode()));

        // Every row at once, in the order set in Sources; the one playing has its own place above.
        Assert.True(panel.Progressed(900001, "7", Rows(Fixture(), ("home", Asking), ("folder", Asking), ("index", Asking), ("second", Asking), ("archive", Asking), ("old", off)), asked));
        // 0.45 s: inside the silent 600 ms.
        panel.Progressed(900001, "7", Rows(Fixture(), ("home", Listed(one)), ("folder", Asking), ("index", Asking), ("second", Asking), ("archive", Asking), ("old", off)), asked + 450);
        Assert.Equal(
            [
                ("Home server", coverage, false, false, true),
                ("Media folder", "", false, false, false),
                ("Example index", "", false, false, false),
                ("Second example", "", false, false, false),
                ("Archive example", "", false, false, false),
                ("Old example", "Switched off", false, true, false),
            ],
            Said(panel, asked + 500, options));
        // 0.3 s, as the board's first frame: nothing asked shows yet.
        Assert.All(Said(panel, asked + 300).Skip(1).SkipLast(1), row => Assert.Equal(("", false), (row.Words, row.Spinner)));

        // 1 s: spinners and Asking; Home server never spun.
        var at1 = Said(panel, asked + 1000);
        Assert.Equal(("Home server", false), (at1[0].Name, at1[0].Spinner));
        Assert.All(at1[1..5], row => Assert.Equal(("Asking", true, false), (row.Words, row.Spinner, row.Pick)));

        // 4 s: Media folder couldn't match, Example index listed five; the slow two count.
        panel.Progressed(900001, "7", Rows(Fixture(), ("home", Listed(one)), ("folder", Missed(new ListingMiss.Ambiguous())), ("index", Asking), ("second", Asking), ("archive", Asking), ("old", off)), asked + 1300);
        panel.Progressed(900001, "7", Rows(Fixture(), ("home", Listed(one)), ("folder", Missed(new ListingMiss.Ambiguous())), ("index", Listed(five)), ("second", Asking), ("archive", Asking), ("old", off)), asked + 2400);
        Assert.Equal(
            [
                ("Home server", coverage, false, false, true),
                ("Media folder", "Couldn't match this title", false, true, false),
                ("Example index", coverage, false, false, true),
                ("Second example", "Asking · 4 s", true, false, false),
                ("Archive example", "Asking · 4 s", true, false, false),
                ("Old example", "Switched off", false, true, false),
            ],
            Said(panel, asked + 4200, options));
        var index = Assert.IsType<SourceGroup.Releases>(PanelWords.Others(null, panel.Releases, "fixture")[2]);
        Assert.Equal(5, index.List.Length);
        // From 3 s exactly, and Asking before.
        Assert.Equal("Asking", Said(panel, asked + 2999)[3].Words);
        Assert.Equal("Asking · 3 s", Said(panel, asked + 3005)[3].Words);

        // 12 s: Archive example didn't answer; Second example still counts.
        panel.Progressed(900001, "7", Rows(Fixture(), ("home", Listed(one)), ("folder", Missed(new ListingMiss.Ambiguous())), ("index", Listed(five)), ("second", Asking), ("archive", Missed(new ListingMiss.Failed())), ("old", off)), asked + 6200);
        var at12 = Said(panel, asked + 12_000);
        Assert.Equal(("Asking · 12 s", true), (at12[3].Words, at12[3].Spinner));
        Assert.Equal(("Didn't answer", false, true), (at12[4].Words, at12[4].Spinner, at12[4].Haze));

        // 21 s: Second example took too long, and nothing is asked any more.
        panel.Progressed(900001, "7", Rows(Fixture(), ("home", Listed(one)), ("folder", Missed(new ListingMiss.Ambiguous())), ("index", Listed(five)), ("second", Missed(new ListingMiss.TimedOut())), ("archive", Missed(new ListingMiss.Failed())), ("old", off)), asked + 20_000);
        var at21 = Said(panel, asked + 21_000);
        Assert.Equal(("Took too long", true), (at21[3].Words, at21[3].Haze));
        Assert.DoesNotContain(at21, row => row.Spinner);
        // The final word keeps the rows as they were said.
        Assert.True(panel.Listed(900001, "7", [one, .. five]));
        Assert.Equal(at21, Said(panel, asked + 21_000));
    }

    [Fact]
    public void ARowSaysWhyItsSourceHasNothingInTheStartingScreensWords()
    {
        Assert.Equal("Didn't answer", PanelWords.Miss(new ListingMiss.Failed(), "7"));
        Assert.Equal("Took too long", PanelWords.Miss(new ListingMiss.TimedOut(), "7"));
        Assert.Equal("Doesn't have episode 7", PanelWords.Miss(new ListingMiss.NoEpisode(), "7"));
        Assert.Equal("Doesn't have this show", PanelWords.Miss(new ListingMiss.NoMatch(), "7"));
        Assert.Equal("Couldn't match this title", PanelWords.Miss(new ListingMiss.Ambiguous(), "7"));
        Assert.Equal(StartScreen.HeldNote(SourceHold.Cellular), PanelWords.Miss(new ListingMiss.HeldBack(SourceHold.Cellular), "7"));
        Assert.StartsWith("Not asked", PanelWords.Miss(new ListingMiss.HeldBack(SourceHold.TorrentsOff), "7"));
        // The picker's lines name the source.
        Assert.Equal("Alpha couldn't match this title", ReleasePicker.Missed(new MissedListing("Alpha", new ListingMiss.Ambiguous()), "7"));
    }

    [Fact]
    public void AHeldBackSourceSaysSoFromTheFirstWordWithNoSpinner()
    {
        var panel = new PanelState { Open = true };
        panel.Playing(7, "5");
        panel.AskReleases(now: 0);
        panel.Progressed(7, "5", [Listing("index", "Example index", Missed(new ListingMiss.HeldBack(SourceHold.Cellular))), Listing("home", "Home server", Asking)], 1);
        var rows = Said(panel, 4000);
        Assert.Equal((StartScreen.HeldNote(SourceHold.Cellular), false, true, false), (rows[0].Words, rows[0].Spinner, rows[0].Haze, rows[0].Pick));
        Assert.Equal(("Asking · 3 s", true), (rows[1].Words, rows[1].Spinner));
        Assert.Equal((StartScreen.HeldNote(SourceHold.Cellular), false), (Said(panel, 1)[0].Words, Said(panel, 1)[0].Spinner));
    }

    [Fact]
    public void RowsForAnotherAskAreLeftAloneAndANewAskStartsOver()
    {
        var panel = new PanelState { Open = true };
        panel.Playing(7, "5");
        panel.AskReleases(now: 0);
        Assert.False(panel.Progressed(7, "6", [Listing("home", "Home server", Asking)], 10), "another episode's rows");
        Assert.True(panel.Progressed(7, "5", [Listing("home", "Home server", Asking)], 10));
        Assert.Equal(SourceLineKind.Counting, panel.Line(PanelWords.Others(null, panel.Releases, null)[0], 3100).Kind);
        // The other version landed: its releases are asked anew, and the seconds count from the new ask.
        panel.AskReleases(again: true, now: 5000);
        Assert.Null(panel.Releases?.Rows);
        panel.Progressed(7, "5", [Listing("home", "Home server", Asking)], 5010);
        Assert.Equal(SourceLineKind.Empty, panel.Line(PanelWords.Others(null, panel.Releases, null)[0], 5300).Kind);
        Assert.Equal(SourceLineKind.Asking, panel.Line(PanelWords.Others(null, panel.Releases, null)[0], 5700).Kind);
    }
}
