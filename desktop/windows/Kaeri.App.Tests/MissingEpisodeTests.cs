// An aired episode no source lists: the status under the title's buttons in each state, the
// row's line, Source for this title's ranges, the waiting rule for Check again, and the scene
// whose first check comes back empty and whose second finds the episode.

using Kaeri.App;
using Kaeri.Engine.Core;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Xunit;

namespace Kaeri.App.Tests;

[Collection("Strings")]
public class MissingEpisodeTests
{
    const long Now = 1_800_000_000;
    const long Minute = 60, Hour = 3_600, Day = 24 * Hour;

    public MissingEpisodeTests()
    {
        T.Resolver = null;
        T.Language = "en";
    }

    static string OnThe7th(long _) => "7 Nov 2026";

    static MissingEpisode Missing(MissingPhase phase, long? ago = 25 * Minute, uint? previous = 5) =>
        new(Episode: 6, AiredAt: ago is { } a ? Now - a : null, Phase: phase, Previous: previous, EveryMinutes: 15, ForHours: 6);

    static MissingStatus Status(MissingEpisode missing, CheckOutcome? asked = null, bool offline = false) =>
        MissingWords.Status(missing, Now, asked, offline, OnThe7th);

    [Fact]
    public void JustAiredItSaysWhenAndThatKaeriLooksAgain()
    {
        var status = Status(Missing(MissingPhase.Quiet));
        Assert.Equal("Episode 6 aired 25 min ago.", status.Head);
        Assert.Equal("Your sources don't have it yet. Kaeri looks again every 15 minutes.", status.Tail);
        Assert.Equal($"{status.Head} {status.Tail}", status.Spoken);
        Assert.True(status.CheckAgain);
        Assert.False(status.LongAgo);
        Assert.Equal(T.EpisodeCheckAgainHelp(15, 6), status.Help);
    }

    [Fact]
    public void AnEmptyAnswerTheViewerAskedForSaysStill()
    {
        Assert.Equal($"{T.EpisodeMissingStill} {T.EpisodeMissingLooksAgain(15)}", Status(Missing(MissingPhase.Quiet), CheckOutcome.Missing).Tail);
        Assert.Equal($"{T.EpisodeMissingNoAnswer} {T.EpisodeMissingLooksAgain(15)}", Status(Missing(MissingPhase.Quiet), CheckOutcome.NoAnswer).Tail);
        // Found and offline say nothing about the sources.
        Assert.Equal(Status(Missing(MissingPhase.Quiet)).Tail, Status(Missing(MissingPhase.Quiet), CheckOutcome.Offline).Tail);
    }

    [Fact]
    public void OfflineInTheQuietWindowSaysKaeriLooksWhenBackOnline()
    {
        Assert.Equal($"{T.EpisodeMissingNotYet} {T.EpisodeMissingOffline}", Status(Missing(MissingPhase.Quiet), offline: true).Tail);
        Assert.Equal($"{T.EpisodeMissingStill} {T.EpisodeMissingOffline}", Status(Missing(MissingPhase.Quiet), CheckOutcome.Missing, offline: true).Tail);
    }

    [Fact]
    public void HoursAfterAiringTheHeadSaysHoursAndPastTheWindowStillStandsAlone()
    {
        var quiet = Status(Missing(MissingPhase.Quiet, 2 * Hour));
        Assert.Equal("Episode 6 aired 2 h ago.", quiet.Head);
        var after = Status(Missing(MissingPhase.After, 8 * Hour));
        Assert.Equal("Episode 6 aired 8 h ago.", after.Head);
        Assert.Equal(T.EpisodeMissingStill, after.Tail);
        Assert.Null(after.Help);
        Assert.True(after.CheckAgain);
        // Offline past the window keeps "still": nothing looks by itself there.
        Assert.Equal(T.EpisodeMissingStill, Status(Missing(MissingPhase.After, 8 * Hour), offline: true).Tail);
        Assert.Equal(T.EpisodeMissingNoAnswer, Status(Missing(MissingPhase.After, 8 * Hour), CheckOutcome.NoAnswer).Tail);
    }

    [Fact]
    public void FromADayOnTheHeadSaysTheDate()
    {
        Assert.Equal("Episode 6 aired on 7 Nov 2026.", Status(Missing(MissingPhase.After, 2 * Day)).Head);
    }

    [Fact]
    public void AWeekOnTheWordsChangeAndCheckAgainGoes()
    {
        var status = Status(Missing(MissingPhase.LongAgo, 8 * Day));
        Assert.Equal(T.EpisodeMissingNone, status.Tail);
        Assert.False(status.CheckAgain);
        Assert.True(status.LongAgo);
        Assert.Null(status.Help);
    }

    [Fact]
    public void UndatedItKeepsTodaysWordsWithCheckAgain()
    {
        var status = Status(Missing(MissingPhase.Undated, null));
        Assert.Equal("", status.Head);
        Assert.Equal(T.EpisodeUnsourced, status.Tail);
        Assert.Equal(T.EpisodeUnsourced, status.Spoken);
        Assert.True(status.CheckAgain);
    }

    [Fact]
    public void TheRowSaysWhenItAiredAndThatTheSourcesDoNotHaveItYet()
    {
        var row = new EpisodeRow("6", AirsAt: Now - 25 * Minute, Unsourced: true);
        Assert.Equal("Aired 25 min ago · Not in your sources yet", MissingWords.RowLine(row, Now, OnThe7th));
        Assert.Equal("Aired 7 Nov 2026 · Not in your sources yet", MissingWords.RowLine(row with { AirsAt = Now - 2 * Day }, Now, OnThe7th));
        Assert.Equal("Aired 7 Nov 2026 · No source has this episode", MissingWords.RowLine(row with { AirsAt = Now - 8 * Day }, Now, OnThe7th));
        Assert.Equal(T.EpisodeUnsourced, MissingWords.RowLine(row with { AirsAt = null }, Now, OnThe7th));
        // The facts line of an unsourced row is this line.
        Assert.Equal("Aired 25 min ago · Not in your sources yet", Synopsis.Facts(row, Now, OnThe7th));
    }

    [Fact]
    public void TheTimeReadsAgainOnTheMinuteSinceAiring()
    {
        Assert.Equal(TimeSpan.FromSeconds(60), MissingWords.NextTick(Now - 25 * Minute, Now));
        Assert.Equal(TimeSpan.FromSeconds(20), MissingWords.NextTick(Now - 25 * Minute - 40, Now));
    }

    static SourceOption Option(uint covered, uint? first, uint? last, uint[] lacking, long? since = Now - 25 * Minute) =>
        new("s", "Source", covered, 6, First: first, Last: last, Lacking: lacking, LackingSince: since);

    [Fact]
    public void ASourceSaysWhichEpisodesItHasAndWhichItDoesNot()
    {
        Assert.Equal("Episodes 1 to 6", SourcePicker.Detail(Option(6, 1, 6, []), Now));
        Assert.Equal("Episodes 1 to 5. Not 6 yet", SourcePicker.Detail(Option(5, 1, 5, [6]), Now));
        Assert.Equal("Episodes 1 to 4. Not 5 or 6 yet", SourcePicker.Detail(Option(4, 1, 4, [5, 6]), Now));
        Assert.Equal("Episodes 1 to 3. Not 4 to 6 yet", SourcePicker.Detail(Option(3, 1, 3, [4, 5, 6]), Now));
        Assert.Equal("Episodes 1 to 5. Not 6", SourcePicker.Detail(Option(5, 1, 5, [6], Now - 8 * Day), Now));
        Assert.Equal("5 of 6 episodes. Not 3", SourcePicker.Detail(Option(5, 1, 6, [3]), Now));
        // Without ranges, the count it always was.
        Assert.Equal("5 of 6 episodes", SourcePicker.Detail(Option(5, null, null, []), Now));
        // A note keeps its words.
        Assert.Equal(T.SourceNoteResting, SourcePicker.Detail(new SourceOption("s", "Source", 0, 6, Note: new SourceNote.Resting(), First: 1, Last: 5), Now));
    }

    [Fact]
    public void CheckingStandsItsThreeTenthsOfASecond()
    {
        var shown = DateTimeOffset.FromUnixTimeSeconds(Now);
        Assert.Equal(TimeSpan.Zero, CheckWait.Before(null, shown));
        Assert.Equal(TimeSpan.FromMilliseconds(200), CheckWait.Before(shown, shown.AddMilliseconds(100)));
        Assert.Equal(TimeSpan.Zero, CheckWait.Before(shown, shown.AddMilliseconds(400)));
    }

    [Fact]
    public void OnlyAnAnswerTheViewerAskedForIsKept()
    {
        var checks = new MissingChecks();
        checks.Take(new Event.EpisodeChecked(new EpisodeCheck(900002, 6, CheckOutcome.Missing, false)));
        Assert.Null(checks.Asked(900002, 6));
        checks.Take(new Event.EpisodeChecked(new EpisodeCheck(900002, 6, CheckOutcome.Offline, true)));
        Assert.Null(checks.Asked(900002, 6));
        checks.Take(new Event.EpisodeChecked(new EpisodeCheck(900002, 6, CheckOutcome.Missing, true)));
        Assert.Equal(CheckOutcome.Missing, checks.Asked(900002, 6));
        Assert.Null(checks.Asked(900002, 7));
    }

    /// <summary>
    /// The scene as the page drives it: episode 6 aired 25 minutes ago with episode 5 to play
    /// meanwhile, the first Check again comes back empty, the second finds it, and Source for
    /// this title says what each source has.
    /// </summary>
    [Fact(Timeout = 90_000)]
    public async Task TheSceneSaysEpisodeSixIsMissingAndTheSecondCheckFindsIt()
    {
        HomeGuard.Check();
        const uint id = 900002;
        var root = AppDirs.NewScratchRoot("cs-missing");
        var gate = new object();
        var engine = FfiEngine.StartScene(AppDirs.Under(root), new QuietHost(), new FfiScene.MissingEpisode(), EngineSession.AppStart(signIn: false));
        using (var session = new EngineSession(engine, a => { lock (gate) a(); }))
        {
            var answers = new List<EpisodeCheck>();
            session.Batch += batch =>
            {
                foreach (var e in batch)
                    if (e is Event.EpisodeChecked c)
                        answers.Add(c.V1);
            };
            session.Start();
            session.Send(new Command.LoadHome(false));
            session.Send(new Command.LoadEpisodes(id));
            var deadline = DateTime.UtcNow.AddSeconds(60);
            async Task Until(Func<bool> done)
            {
                while (DateTime.UtcNow < deadline)
                {
                    lock (gate)
                        if (done())
                            return;
                    await Task.Delay(50, TestContext.Current.CancellationToken);
                }
                Assert.Fail("the engine did not say it in time");
            }

            await Until(() => session.Known.Episodes.TryGetValue(id, out var rows) && rows.Length >= 6);
            MissingEpisode missing;
            lock (gate)
            {
                var now = DateTimeOffset.UtcNow.ToUnixTimeSeconds();
                missing = KaeriModelMethods.MissingEpisode(session.Known.Episodes[id], 6, false, now)!;
                Assert.NotNull(missing);
                Assert.Equal((6u, MissingPhase.Quiet, (uint?)5), (missing.Episode, missing.Phase, missing.Previous));
                var status = MissingWords.Status(missing, now, session.Known.MissingChecks.Asked(id, 6), session.Known.Offline, at => Synopsis.Date(at));
                Assert.StartsWith("Episode 6 aired 2", status.Head);
                Assert.Equal($"{T.EpisodeMissingNotYet} {T.EpisodeMissingLooksAgain(15)}", status.Tail);
                var row = session.Known.Episodes[id].First(r => r.Number == "6");
                Assert.EndsWith(T.EpisodeUnsourcedYet, Synopsis.Facts(row, now, at => Synopsis.Date(at)));
            }

            session.Send(new Command.Missing(new MissingCommand.CheckAgain(id, 6)));
            await Until(() => answers.Count == 1);
            lock (gate)
            {
                Assert.Equal(new EpisodeCheck(id, 6, CheckOutcome.Missing, true), answers[0]);
                Assert.Equal(CheckOutcome.Missing, session.Known.MissingChecks.Asked(id, 6));
                var status = MissingWords.Status(missing, DateTimeOffset.UtcNow.ToUnixTimeSeconds(), session.Known.MissingChecks.Asked(id, 6), false, at => Synopsis.Date(at));
                Assert.StartsWith(T.EpisodeMissingStill, status.Tail);
            }

            session.Send(new Command.Missing(new MissingCommand.CheckAgain(id, 6)));
            await Until(() => answers.Count == 2);
            lock (gate)
            {
                Assert.Equal(CheckOutcome.Found, answers[1].Outcome);
                // The table came ahead of the answer: the page plays episode 6 now.
                Assert.Null(KaeriModelMethods.MissingEpisode(session.Known.Episodes[id], 6, false, DateTimeOffset.UtcNow.ToUnixTimeSeconds()));
                Assert.False(session.Known.Episodes[id].First(r => r.Number == "6").Unsourced);
            }

            session.Send(new Command.LoadSourceOptions(id));
            await Until(() => session.Known.SourceChoices.For(id) is { Length: >= 3 });
            lock (gate)
            {
                var options = session.Known.SourceChoices.For(id)!;
                string Line(string provider) => SourcePicker.Detail(options.First(o => o.ProviderId == provider));
                Assert.Equal("Episodes 1 to 6", Line("fixture"));
                Assert.Equal("Episodes 1 to 4. Not 5 or 6 yet", Line("fixture-yamabiko"));
                Assert.NotNull(options.First(o => o.ProviderId == "fixture-minato").Note);
            }
        }
        SceneTests.DeleteRoot(root);
    }
}
