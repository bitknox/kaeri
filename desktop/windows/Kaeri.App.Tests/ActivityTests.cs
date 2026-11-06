using Kaeri.App;
using Kaeri.Engine.Model;
using Xunit;

namespace Kaeri.App.Tests;

public class ActivityTests
{
    static readonly DateTimeOffset Start = DateTimeOffset.FromUnixTimeSeconds(1_800_000_000);

    [Fact]
    public void AJobShowsOnlyOnceItHasRunSixHundredMilliseconds()
    {
        var jobs = new Jobs();
        jobs.Take(new Event.JobStarted(1, "checking sources"), Start);
        Assert.Empty(jobs.Visible(Start.AddMilliseconds(599)));
        Assert.Single(jobs.Visible(Start.AddMilliseconds(600)));
        Assert.Equal("Checking sources", jobs.Visible(Start.AddSeconds(1))[0].Words);
    }

    [Fact]
    public void TheNewestJobComesFirstAndADoneJobGoes()
    {
        var jobs = new Jobs();
        jobs.Take(new Event.JobStarted(1, "checking sources"), Start);
        jobs.Take(new Event.JobStarted(2, "loading the calendar"), Start.AddMilliseconds(100));
        Assert.Equal([2ul, 1ul], jobs.Visible(Start.AddSeconds(2)).Select(j => j.Id));
        Assert.True(jobs.Take(new Event.JobDone(2), Start.AddSeconds(2)));
        Assert.Equal([1ul], jobs.Visible(Start.AddSeconds(2)).Select(j => j.Id));
        Assert.False(jobs.Take(new Event.JobDone(9), Start.AddSeconds(2)));
    }

    [Fact]
    public void ANoteTakesTheJobsPlaceInTheWordsWithItsWait()
    {
        var jobs = new Jobs();
        jobs.Take(new Event.JobStarted(1, "syncing"), Start);
        jobs.Take(new Event.JobNote(1, new Msg.StatusWaitingForRedirect("AniList"), TimeSpan.FromSeconds(12)), Start.AddSeconds(1));
        var job = jobs.Running[0];
        Assert.Equal(job.Note, job.Words);
        Assert.Equal(Start.AddSeconds(13), job.WaitUntil);
    }

    [Fact]
    public void MessagesKeepTheLastTwoHundredNewestFirst()
    {
        var log = new NoticeLog();
        for (var i = 0; i < 205; i++)
            log.Add($"notice {i}", i % 2 == 0, Start.AddSeconds(i));
        log.Add("", false, Start);
        Assert.Equal(NoticeLog.Kept, log.All.Count);
        Assert.Equal("notice 204", log.All[0].Text);
        Assert.True(log.All[0].Alert);
        Assert.Equal("notice 5", log.All[^1].Text);
    }

    [Fact]
    public void TheStatusSaysWhereAniListSyncStands()
    {
        Dictionary<string, SyncState> trackers = [];
        Assert.Equal(SyncDot.SignedOut, Summary.Of(trackers, false).Dot);
        trackers["anilist"] = new SyncState("anilist", true, 0, 0, User: "mika", Last: "pulled");
        Assert.Equal(SyncDot.Synced, Summary.Of(trackers, false).Dot);
        trackers["anilist"] = new SyncState("anilist", true, 3, 0, User: "mika");
        Assert.Equal(SyncDot.Waiting, Summary.Of(trackers, false).Dot);
        trackers["anilist"] = new SyncState("anilist", true, 0, 0, User: "mika", PullError: new PullError("the server did not answer", 0));
        Assert.Equal(SyncDot.Problem, Summary.Of(trackers, false).Dot);
        trackers["anilist"] = new SyncState("anilist", true, 0, 0, User: "mika", NeedsReauth: true);
        Assert.Equal(SyncDot.Problem, Summary.Of(trackers, false).Dot);
        Assert.Equal(SyncDot.Waiting, Summary.Of(trackers, true).Dot);
    }

    [Fact]
    public void SettlingADifferenceTakesItsRowAwayAtOnce()
    {
        var known = new Known();
        var said = 0;
        known.ConflictsChanged += () => said++;
        known.Take(new Event.Conflicts([new ConflictRow(5, "Stardust Wayfarer", "progress", "7", "5"), new ConflictRow(6, "The Steel Gardener", "score", "8", "6")]));
        var command = known.Resolve(5, keepLocal: true);
        Assert.Equal(new Command.ResolveConflict(5, true), command);
        Assert.Equal([6u], known.Conflicts.Select(c => c.AnilistId));
        Assert.Equal(2, said);
    }

    [Fact]
    public void ElapsedTimesReadInSecondsThenMinutes()
    {
        Assert.Equal(T.DurationSeconds(4), Summary.Elapsed(TimeSpan.FromSeconds(4.6)));
        Assert.Equal(T.DurationMinutesSeconds(1, "12"), Summary.Elapsed(TimeSpan.FromSeconds(72)));
    }
}
