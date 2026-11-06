using Kaeri.App;
using Kaeri.Engine.Core;
using Kaeri.Engine.Model;
using Xunit;

namespace Kaeri.App.Tests;

[Collection("Strings")]
public class StartScreenTests
{
    public StartScreenTests() => T.Resolver = null;

    static TimeSpan Ms(int n) => TimeSpan.FromMilliseconds(n);

    static StartScreen Start(double? resume = 606) => new(900_001, "7", resume);

    static void Asking(StartScreen start, string name, uint place, uint total, TimeSpan at, bool pinned = false) =>
        start.Step(new StartStep.AskingSource(name, place, total, pinned), new Msg.StartAskingSource(name), at);

    static void Found(StartScreen start, string? source, TimeSpan at) =>
        start.Step(new StartStep.Found(3, source, null, Translation.Sub), new Msg.StartOpeningStream(), at);

    static IEnumerable<StartAction> Actions(StartFrame frame) => frame.Buttons.Select(b => b.Action);

    [Fact]
    public void ANewerStepWaitsOutTheHoldAndOnlyTheLatestShows()
    {
        var start = Start();
        Assert.Equal(T.StartLookingUpTitle, start.Frame(TimeSpan.Zero).Step);
        Asking(start, "Alpha", 1, 3, Ms(100));
        Assert.Equal(StartPhase.Lookup, start.Frame(Ms(100)).Phase);
        Found(start, null, Ms(300));
        Assert.Equal(StartPhase.Lookup, start.Frame(Ms(300)).Phase);
        Assert.False(start.Tick(Ms(500)));
        Assert.True(start.Tick(Ms(600)));
        Assert.Equal(StartPhase.Opening, start.Frame(Ms(600)).Phase);
    }

    [Fact]
    public void APassedOverPinIsOnTheDetailLineOnceTheStreamsAreFound()
    {
        var start = Start(null);
        Asking(start, "Alpha", 1, 3, Ms(700), pinned: true);
        start.Step(new StartStep.PinMissed("Alpha"), null, Ms(2000));
        Found(start, "Gamma", Ms(2000));
        Assert.Contains("Alpha doesn't have episode 7 yet. Playing from the next source.", start.Frame(Ms(3000)).Detail);
    }

    [Fact]
    public void AStepAfterTheHoldShowsAtOnce()
    {
        var start = Start();
        Asking(start, "Alpha", 1, 3, Ms(700));
        var frame = start.Frame(Ms(700));
        Assert.Equal(StartPhase.Asking, frame.Phase);
        Assert.Equal("Asking Alpha", frame.Step);
    }

    [Fact]
    public void TheToolsComeByTimeInArrivalOrder()
    {
        var start = Start();
        Asking(start, "Alpha", 1, 3, Ms(700));
        Assert.Equal([StartAction.Cancel], Actions(start.Frame(Ms(900))));
        Assert.Equal([StartAction.Cancel, StartAction.FromBeginning], Actions(start.Frame(Ms(1000))));
        Assert.Equal(
            [StartAction.Cancel, StartAction.FromBeginning, StartAction.ChooseSource, StartAction.Details],
            Actions(start.Frame(Ms(3000))));
        Assert.Equal(StartAction.Skip, Actions(start.Frame(Ms(8800))).Last());
    }

    [Fact]
    public void SkipNeedsEightSecondsOnOneSourceAndAnotherToGoTo()
    {
        var start = Start();
        Asking(start, "Alpha", 1, 3, Ms(700));
        Assert.DoesNotContain(StartAction.Skip, Actions(start.Frame(Ms(8600))));
        var skip = Assert.Single(start.Frame(Ms(8700)).Buttons, b => b.Action == StartAction.Skip);
        Assert.Equal("Skip Alpha", skip.Label);
        Assert.Equal("Skip Alpha", start.Frame(Ms(8700)).Rows.Last().Skip);
        // The last source has nothing to skip to.
        Asking(start, "Harbor", 3, 3, Ms(9000));
        Assert.DoesNotContain(StartAction.Skip, Actions(start.Frame(Ms(20_000))));
    }

    [Fact]
    public void TheDetailComesAtOneSecondAndTheSecondsAtThree()
    {
        var start = Start();
        Asking(start, "Alpha", 1, 3, Ms(700));
        Assert.Equal("", start.Frame(Ms(900)).Detail);
        Assert.Equal("Resuming at 10:06 · 1 of 3 sources", start.Frame(Ms(1000)).Detail);
        Assert.EndsWith("10 s", start.Frame(Ms(10_000)).Detail);
    }

    [Fact]
    public void TheResumeFactStaysAndFlipsWhenPlayingFromTheBeginning()
    {
        var start = Start();
        Asking(start, "Alpha", 1, 3, Ms(700));
        start.Step(new StartStep.SourceMissed("Alpha", SourceMiss.NoEpisode), null, Ms(1500));
        Asking(start, "Brook", 2, 3, Ms(1500));
        start.Tick(Ms(1500));
        Assert.Equal("Resuming at 10:06 · Alpha doesn't have episode 7 · 2 of 3 sources", start.Frame(Ms(2000)).Detail);
        start.BeginFromStart();
        var frame = start.Frame(Ms(2000));
        Assert.StartsWith("Starting from the beginning · ", frame.Detail);
        Assert.Contains(frame.Buttons, b => b.Action == StartAction.FromBeginning && !b.Shown);
    }

    [Fact]
    public void APressedPlayFromTheBeginningKeepsItsSlotSoNothingMoves()
    {
        var start = Start();
        start.Tick(Ms(6000));
        var before = start.Frame(Ms(6000));
        Assert.Equal([StartAction.Cancel, StartAction.FromBeginning, StartAction.ChooseSource, StartAction.Details], Actions(before));
        Assert.All(before.Buttons, b => Assert.True(b.Shown));
        start.BeginFromStart();
        var after = start.Frame(Ms(6000));
        // The same slots in the same order, the pressed one hidden in its place.
        Assert.Equal(Actions(before), Actions(after));
        Assert.Equal(
            [(StartAction.Cancel, true), (StartAction.FromBeginning, false), (StartAction.ChooseSource, true), (StartAction.Details, true)],
            after.Buttons.Select(b => (b.Action, b.Shown)));
    }

    [Fact]
    public void NoResumeNoPlayFromTheBeginning()
    {
        var start = Start(null);
        Asking(start, "Alpha", 1, 1, Ms(700));
        Assert.False(start.Resumes);
        Assert.Equal([StartAction.Cancel], Actions(start.Frame(Ms(1500))));
        Assert.Equal("", start.Frame(Ms(1500)).Detail);
    }

    [Fact]
    public void SlowWordingInEveryPhaseFromTwentySecondsAndNoFalseFailure()
    {
        var asking = Start();
        Asking(asking, "Brook", 2, 3, Ms(1500));
        var frame = asking.Frame(Ms(24_000));
        Assert.Equal("Still waiting for Brook", frame.Step);
        Assert.Null(frame.Failure);

        var peers = Start();
        Asking(peers, "Harbor", 1, 2, Ms(700));
        peers.Step(new StartStep.FindingPeers("Harbor"), new Msg.StartFindingPeers(), Ms(900));
        peers.Tick(Ms(1300));
        Assert.Equal(StartPhase.Peers, peers.Frame(Ms(19_000)).Phase);
        Assert.Equal("Still waiting for Harbor", peers.Frame(Ms(21_000)).Step);

        var opening = Start();
        Asking(opening, "Brook", 1, 3, Ms(700));
        Found(opening, "Brook", Ms(2300));
        Assert.Equal("Still waiting for Brook", opening.Frame(Ms(21_000)).Step);
    }

    [Fact]
    public void AFailureSaysWhyInPlainWordsAndWhoWasAsked()
    {
        var start = Start();
        start.Fail(new StartFailure(
            NoSourceWhy.NotFound,
            new Msg.NoticeReason(new Cause.NoSourceHasEpisode("7", "alpha: not found, brook: not found")),
            ["Alpha", "Brook"]));
        var frame = start.Frame(Ms(3000));
        Assert.Equal("Couldn't play episode 7", frame.Failure?.Title);
        Assert.Equal("No source has episode 7. Alpha and Brook were asked.", frame.Failure?.Reason);
        Assert.Equal([StartAction.ChooseAnother, StartAction.TryAgain, StartAction.Details, StartAction.Close], Actions(frame));
    }

    [Fact]
    public void AFailureOnAHalfEpisodeNamesItAndNoSourcesOffersNoTryAgain()
    {
        var start = new StartScreen(1, "7.5", null);
        start.Fail(new StartFailure(NoSourceWhy.NoSources, new Msg.NoticeReason(new Cause.NoSources()), []));
        var frame = start.Frame(Ms(500));
        Assert.Equal("Couldn't play episode 7.5", frame.Failure?.Title);
        Assert.DoesNotContain("Add one", frame.Failure?.Reason);
        Assert.Equal([StartAction.AddStore, StartAction.Close], Actions(frame));
    }

    [Fact]
    public void ATitleWithNoMatchLeadsWithFixMatch()
    {
        var start = Start();
        start.Fail(new StartFailure(
            NoSourceWhy.NotFound,
            new Msg.NoticeReason(new Cause.TitleNoMatch("alpha: no match")),
            ["Alpha"]));
        var frame = start.Frame(Ms(2000));
        Assert.Equal(StartAction.FixMatch, frame.Buttons[0].Action);
        Assert.Equal(StartLook.Primary, frame.Buttons[0].Look);
        Assert.DoesNotContain("alpha: no match", frame.Failure?.Reason);
        Assert.EndsWith("Alpha were asked.", frame.Failure?.Reason);
    }

    [Fact]
    public void OnlyAFailureEventMakesAFailure()
    {
        var start = Start();
        Assert.Null(start.Frame(Ms(60_000)).Failure);
        start.Fail(new StartFailure(
            NoSourceWhy.NotAired,
            new Msg.NoticeNotAired(7, new Cause.AirsTomorrow("18:30")),
            []));
        var frame = start.Frame(Ms(700));
        Assert.Equal("Episode 7 airs tomorrow at 18:30.", frame.Failure?.Reason);
        Assert.Equal([StartAction.Close], Actions(frame));
    }

    [Fact]
    public void OfflineWaitsWithShowDownloadsAndStartsOnTheNextStep()
    {
        var start = Start();
        start.Step(new StartStep.WaitingForConnection(), null, Ms(0));
        var offline = start.Frame(Ms(1000));
        Assert.Equal("Waiting for a connection", offline.Step);
        Assert.Contains("You're offline", offline.Detail);
        Assert.Equal([StartAction.Cancel, StartAction.ShowDownloads], Actions(offline));
        // Long offline waits are not slow starts.
        Assert.Null(start.SlowAnnouncement(Ms(9000)));
        Asking(start, "Alpha", 1, 2, Ms(8700));
        var back = start.Frame(Ms(8700));
        Assert.Equal("Asking Alpha", back.Step);
        Assert.Equal([StartAction.Cancel, StartAction.FromBeginning], Actions(back).Take(2));
        // The slow wording counts from the end of the wait.
        Assert.Equal("Asking Alpha", start.Frame(Ms(26_000)).Step);
        Assert.Equal("Still waiting for Alpha", start.Frame(Ms(29_000)).Step);
    }

    [Fact]
    public void OfflineTooLongFailsWithTryAgainFirst()
    {
        var start = Start();
        start.Step(new StartStep.WaitingForConnection(), null, Ms(0));
        start.Fail(new StartFailure(NoSourceWhy.NoConnection, new Msg.NoticeReason(new Cause.NoConnection()), []));
        var frame = start.Frame(Ms(120_000));
        Assert.Equal("No connection.", frame.Failure?.Reason);
        Assert.Equal([StartAction.TryAgain, StartAction.ShowDownloads, StartAction.Close], Actions(frame));
    }

    [Fact]
    public void BufferingPercentIsANumberInTheStepAndChangesInPlace()
    {
        var start = Start();
        start.Step(new StartStep.Buffering(10), new Msg.StartBuffering(), Ms(5000));
        Assert.Equal(10u, start.Frame(Ms(5000)).Percent);
        start.Step(new StartStep.Buffering(25), new Msg.StartBuffering(), Ms(5100));
        var frame = start.Frame(Ms(5100));
        Assert.Equal(25u, frame.Percent);
        Assert.Contains("25", frame.Step);
    }

    [Fact]
    public void ADirectStreamBuffersWithNoPercent()
    {
        var start = Start();
        Asking(start, "Brook", 1, 3, Ms(700));
        Found(start, "Brook", Ms(2300));
        start.Step(new StartStep.OpeningStream(1, 3), null, Ms(2350));
        start.Step(new StartStep.Speed(1_800_000), null, Ms(3400));
        var frame = start.Frame(Ms(3400));
        Assert.Equal(StartPhase.Buffering, frame.Phase);
        Assert.Equal("Buffering", frame.Step);
        Assert.Null(frame.Percent);
        Assert.Equal("Resuming at 10:06 · From Brook · 3 s", frame.Detail);
    }

    [Fact]
    public void SpeedHasOneDecimal()
    {
        var start = Start();
        start.Step(new StartStep.Swarm(12, 3_400_000), null, Ms(6000));
        start.Step(new StartStep.Buffering(40), new Msg.StartBuffering(), Ms(8000));
        Assert.Equal("Resuming at 10:06 · 12 peers · 3.4 MB/s · 15 s", start.Frame(Ms(15_000)).Detail);
        Assert.Equal("14.0 MB", StartScreen.Speed(14_000_000));
        Assert.Equal("800 kB", StartScreen.Speed(800_000));
    }

    [Fact]
    public void NamesAreListedAsASentence()
    {
        Assert.Null(StartScreen.Names([]));
        Assert.Equal("Alpha", StartScreen.Names(["Alpha"]));
        Assert.Equal("Alpha and Brook", StartScreen.Names(["Alpha", "Brook"]));
        Assert.Equal("Alpha, Brook and Harbor", StartScreen.Names(["Alpha", "Brook", "Harbor"]));
    }

    [Fact]
    public void ASlowStartIsAnnouncedOnce()
    {
        var start = Start();
        Assert.Null(start.SlowAnnouncement(TimeSpan.FromSeconds(7)));
        Assert.Equal(T.PlayerStartStillStarting, start.SlowAnnouncement(TimeSpan.FromSeconds(8)));
        Assert.Null(start.SlowAnnouncement(TimeSpan.FromSeconds(9)));
    }

    [Fact]
    public void BrightArtGetsTheDeeperGround()
    {
        byte[] Field(byte value) => Enumerable.Repeat(value, 64 * 64 * 4).ToArray();
        Assert.Equal(StartGround.Deep, StartGround.For(StartGround.Luminance(Field(240), 64, 64)));
        Assert.Equal(StartGround.Normal, StartGround.For(StartGround.Luminance(Field(20), 64, 64)));
    }
}
