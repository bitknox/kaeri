using Kaeri.App;
using Kaeri.Engine.Model;
using Xunit;

namespace Kaeri.App.Tests;

public class StartScreenTests
{
    static TimeSpan Ms(int n) => TimeSpan.FromMilliseconds(n);

    static StartScreen Start(double? resume = 606) => new(900_001, "7", resume);

    static StartStep Ask(string name, uint place, uint total, bool pinned = false) => new StartStep.AskingSource(name, place, total, pinned);

    static void Asking(StartScreen start, string name, uint place, uint total, TimeSpan at, bool pinned = false) =>
        start.Step(Ask(name, place, total, pinned), new Msg.StartAskingSource(name), at);

    static StartFailure Failure(NoSourceWhy why) => new(why, new Msg.StartFailed("nothing"));

    [Fact]
    public void TheStepLineIsThereFromZeroAndASecondTextWaitsForTheHold()
    {
        var start = Start();
        Assert.Equal(T.StartLookingUpTitle, start.Frame(TimeSpan.Zero).Step);
        Asking(start, "Alpha", 1, 3, Ms(200));
        Assert.Equal(T.StartLookingUpTitle, start.Frame(Ms(300)).Step);
        Assert.False(start.Tick(Ms(500)));
        Assert.True(start.Tick(Ms(600)));
        Assert.Equal(T.StartAskingSource("Alpha"), start.Frame(Ms(600)).Step);
    }

    [Fact]
    public void ANewerTextReplacesTheOneWaiting()
    {
        var start = Start();
        Asking(start, "Alpha", 1, 3, Ms(100));
        Asking(start, "Brook", 2, 3, Ms(300));
        start.Tick(Ms(700));
        Assert.Equal(T.StartAskingSource("Brook"), start.Frame(Ms(700)).Step);
    }

    [Fact]
    public void ThePercentageChangesInPlaceWithNoHold()
    {
        var start = Start();
        start.Step(new StartStep.Buffering(10), null, Ms(700));
        var first = start.Frame(Ms(700));
        start.Step(new StartStep.Buffering(11), null, Ms(800));
        var second = start.Frame(Ms(800));
        Assert.Equal("buffering", second.Key);
        Assert.NotEqual(first.Step, second.Step);
        Assert.Equal(11u, second.Percent);
    }

    [Fact]
    public void TheDetailLineComesFromOneSecondAndTheSecondsFromThree()
    {
        var start = Start();
        Assert.Equal("", start.Frame(Ms(900)).Detail);
        Assert.Equal(T.PlayerStartResuming("10:06"), start.Frame(Ms(1000)).Detail);
        Assert.EndsWith(T.WindowsStartSeconds(3), start.Frame(TimeSpan.FromSeconds(3)).Detail);
    }

    [Fact]
    public void ButtonsArriveByTime()
    {
        var start = Start();
        Assert.Equal([StartAction.Cancel], start.Frame(Ms(500)).Buttons.Select(b => b.Action));
        Assert.Equal([StartAction.Cancel, StartAction.FromBeginning], start.Frame(Ms(1000)).Buttons.Select(b => b.Action));
        Assert.Equal(
            [StartAction.Cancel, StartAction.FromBeginning, StartAction.ChooseSource, StartAction.Details],
            start.Frame(TimeSpan.FromSeconds(3)).Buttons.Select(b => b.Action));
    }

    [Fact]
    public void SkipComesAfterEightSecondsOnASourceWithAnotherToGoTo()
    {
        var start = Start();
        Asking(start, "Alpha", 1, 3, TimeSpan.Zero);
        start.Tick(Ms(600));
        Assert.DoesNotContain(start.Frame(TimeSpan.FromSeconds(7)).Buttons, b => b.Action == StartAction.Skip);
        var skip = Assert.Single(start.Frame(TimeSpan.FromSeconds(8)).Buttons, b => b.Action == StartAction.Skip);
        Assert.Equal(T.PlayerStartSkipSource("Alpha"), skip.Label);

        var last = Start();
        Asking(last, "Alpha", 1, 1, TimeSpan.Zero);
        last.Tick(Ms(600));
        Assert.DoesNotContain(last.Frame(TimeSpan.FromSeconds(9)).Buttons, b => b.Action == StartAction.Skip);
    }

    [Fact]
    public void ThePlayFromTheBeginningGoesWhenItIsChosen()
    {
        var start = Start();
        start.BeginFromStart();
        Assert.DoesNotContain(start.Frame(TimeSpan.FromSeconds(4)).Buttons, b => b.Action == StartAction.FromBeginning);
        Assert.DoesNotContain(T.PlayerStartResuming("10:06"), start.Frame(TimeSpan.FromSeconds(4)).Detail);
    }

    [Fact]
    public void ASlowStartSaysItIsWaitingFromTwentySeconds()
    {
        var start = Start();
        Asking(start, "Alpha", 2, 3, Ms(100));
        start.Tick(Ms(800));
        Assert.Equal(T.StartAskingSource("Alpha"), start.Frame(TimeSpan.FromSeconds(19)).Step);
        var slow = start.Frame(TimeSpan.FromSeconds(20));
        Assert.Equal(T.PlayerStartStillWaiting("Alpha"), slow.Step);
        Assert.Equal("slow", slow.Key);
    }

    [Fact]
    public void AMissLeadsTheDetailLineAndTheListKeepsEverySource()
    {
        var start = Start(null);
        Asking(start, "Alpha", 1, 3, Ms(100));
        start.Step(new StartStep.SourceMissed("Alpha", SourceMiss.NoEpisode), null, Ms(2400));
        Asking(start, "Brook", 2, 3, Ms(2500));
        start.Tick(Ms(3200));
        var frame = start.Frame(Ms(4000));
        Assert.StartsWith(T.PlayerStartMissNoEpisode("Alpha", "7"), frame.Detail);
        Assert.Contains(frame.Rows, r => r.Name == "Alpha" && r.Mark == StartMark.Empty);
        Assert.Contains(frame.Rows, r => r.Name == "Brook" && r.Mark == StartMark.Now);
    }

    [Fact]
    public void OfflineWaitsWithShowDownloadsAndComesBackToTheStep()
    {
        var start = Start();
        Asking(start, "Alpha", 1, 3, Ms(100));
        start.Tick(Ms(800));
        start.Connectivity(true, Ms(1000));
        var offline = start.Frame(Ms(1500));
        Assert.Equal(T.PlayerStartOffline, offline.Step);
        Assert.Equal(T.PlayerStartOfflineDetail("7"), offline.Detail);
        Assert.Equal([StartAction.Cancel, StartAction.ShowDownloads], offline.Buttons.Select(b => b.Action));
        start.Connectivity(false, Ms(2000));
        Assert.Equal(T.StartAskingSource("Alpha"), start.Frame(Ms(2100)).Step);
    }

    [Fact]
    public void FailuresOfferWhatFitsTheReason()
    {
        var aired = Start();
        aired.Fail(Failure(NoSourceWhy.NotAired));
        Assert.Equal([StartAction.Close], aired.Frame(Ms(100)).Buttons.Select(b => b.Action));

        var none = Start();
        none.Fail(Failure(NoSourceWhy.NoSources));
        Assert.Equal(StartAction.AddStore, none.Frame(Ms(100)).Buttons[0].Action);

        var nomatch = Start();
        Asking(nomatch, "Alpha", 1, 2, Ms(100));
        nomatch.Step(new StartStep.SourceMissed("Alpha", SourceMiss.NoMatch), null, Ms(900));
        nomatch.Fail(Failure(NoSourceWhy.NotFound));
        var frame = nomatch.Frame(Ms(1000));
        Assert.Equal(StartAction.FixMatch, frame.Buttons[0].Action);
        Assert.Equal(StartLook.Primary, frame.Buttons[0].Look);
        Assert.Equal(T.PlayerFailedTitle(7), frame.Failure?.Title);

        var noEpisode = Start();
        Asking(noEpisode, "Alpha", 1, 2, Ms(100));
        noEpisode.Step(new StartStep.SourceMissed("Alpha", SourceMiss.NoEpisode), null, Ms(900));
        noEpisode.Fail(Failure(NoSourceWhy.NotFound));
        Assert.Equal(StartAction.ChooseAnother, noEpisode.Frame(Ms(1000)).Buttons[0].Action);
    }

    [Fact]
    public void ASlowStartIsAnnouncedOnce()
    {
        var start = Start();
        Assert.Null(start.SlowAnnouncement(TimeSpan.FromSeconds(7)));
        Assert.Equal(T.WindowsStartStillStarting, start.SlowAnnouncement(TimeSpan.FromSeconds(8)));
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
