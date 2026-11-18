// The people followed on AniList, as the title page words them: lines, scores, marks and what
// a screen reader hears.

using Kaeri.App;
using Kaeri.Engine.Model;
using Xunit;

namespace Kaeri.App.Tests;

public class FollowingTests
{
    static FollowedEntry Person(string name, LibrarySegment? status, uint progress = 0, uint? score = null) =>
        new(name, null, status, score, progress);

    [Fact]
    public void ALineSaysHowFarSomeoneUnderwayIsAndTheStatusAloneOtherwise()
    {
        Assert.Equal("Watching · Episode 8", Following.Line(Person("Lumi", LibrarySegment.Watching, 8)));
        Assert.Equal("Paused · Episode 5", Following.Line(Person("TeoVale", LibrarySegment.Paused, 5)));
        Assert.Equal("Dropped · Episode 3", Following.Line(Person("Yunami", LibrarySegment.Dropped, 3)));
        Assert.Equal("Completed", Following.Line(Person("Rowanpaper", LibrarySegment.Completed, 12)));
        Assert.Equal("Planning", Following.Line(Person("Kofi", LibrarySegment.Planning)));
        // Watching with nothing watched yet has no episode to name.
        Assert.Equal("Watching", Following.Line(Person("Ines", LibrarySegment.Watching)));
        Assert.Equal("", Following.Line(Person("Halden", null)));
    }

    [Fact]
    public void AScoreIsOutOfTenRoundedAndNothingWithoutOne()
    {
        Assert.Equal("8/10", Following.ScoreText(Person("Ines", LibrarySegment.Watching, 8, 80)));
        Assert.Equal("9/10", Following.ScoreText(Person("Ines", LibrarySegment.Watching, 8, 85)));
        Assert.Equal("", Following.ScoreText(Person("SannaQ", LibrarySegment.Watching, 10)));
        Assert.Equal("", Following.ScoreText(Person("SannaQ", LibrarySegment.Watching, 10, 0)));
        Assert.Equal("", Following.ScoreText(Person("SannaQ", LibrarySegment.Watching, 10, 4)));
    }

    [Fact]
    public void APersonReadsAsOneElementWithTheScoreSpokenInFull()
    {
        Assert.Equal("Lumi, Watching · Episode 8, Score 7 out of 10", Following.Spoken(Person("Lumi", LibrarySegment.Watching, 8, 70)));
        Assert.Equal("Kofi, Planning", Following.Spoken(Person("Kofi", LibrarySegment.Planning)));
    }

    [Fact]
    public void AMarkShowsThreePicturesThenHowManyMore()
    {
        FollowedMark[] four = [new("Lumi", null), new("Dario", null), new("Oskar", null), new("Halden", null)];
        var (pictures, more) = Following.Stack(four);
        Assert.Equal(["Lumi", "Dario", "Oskar"], pictures.Select(m => m.Name));
        Assert.Equal(1, more);
        Assert.Equal("+1", Following.More(more));
        var (one, none) = Following.Stack([new FollowedMark("SannaQ", null)]);
        Assert.Single(one);
        Assert.Equal(0, none);
    }

    [Fact]
    public void AMarkSaysEveryNameAndNoScore()
    {
        Assert.Equal("TeoVale watched up to here.", Following.MarksSpoken([new FollowedMark("TeoVale", null)]));
        Assert.Equal("", Following.MarksSpoken([]));
        FollowedMark[] three = [new("Lumi", null), new("Dario", null), new("Oskar", null)];
        var said = Following.MarksSpoken(three);
        Assert.Equal(Names.Platform ? "Lumi, Dario, and Oskar watched up to here." : "Lumi, Dario and Oskar watched up to here.", said);
    }

    [Fact]
    public void WindowsListsNamesWithItsOwnListFormat()
    {
        if (OperatingSystem.IsWindows())
            Assert.True(Names.Platform);
        Assert.Equal("Ines and Lumi", Names.Fallback(["Ines", "Lumi"]));
        Assert.Equal("Ines, Lumi and Dario", Names.Fallback(["Ines", "Lumi", "Dario"]));
        Assert.Equal("Ines", Names.And(["Ines"]));
    }

    [Fact]
    public void AFoldCarriesTheMarksOfTheRowsInsideItInOrder()
    {
        EpisodeRow[] rows =
        [
            new("3", Followed: [new FollowedMark("Yunami", null)]),
            new("4"),
            new("5", Followed: [new FollowedMark("TeoVale", null), new FollowedMark("Yunami", null)]),
        ];
        Assert.Equal(["Yunami", "TeoVale"], Following.Union(rows).Select(m => m.Name));
        Assert.Empty(Following.Union([new EpisodeRow("1")]));
    }

    [Fact]
    public void APersonWithoutAPictureShowsTheirFirstLetter()
    {
        Assert.Equal("S", Following.Initial("SannaQ"));
        Assert.Equal("Ö", Following.Initial("ödla"));
        Assert.Equal("", Following.Initial("  "));
    }
}
