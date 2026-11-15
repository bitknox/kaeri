// The title page's availability line: a placeholder keeps its room until the answer, which keeps
// the room for a line and closes it for none, and the wait ends after 2 s with no answer. An
// answer counts once it is for what the episodes say of the sources, and while they say none
// plays the title the placeholder holds two rows.

using Kaeri.App;
using Kaeri.Engine.Model;
using Xunit;

namespace Kaeri.App.Tests;

public class AvailabilityRoomTests
{
    static TimeSpan Ms(int ms) => TimeSpan.FromMilliseconds(ms);

    static readonly ServiceLink Lumen = new("Lumen Plus", "https://lumen.example/watch/1", null);
    static readonly ServiceLink Harbor = new("Harbor Library", "https://harbor.example/watch/1", null);

    static TitleAvailability Answer(AvailabilityCase kind, ServiceLink? main = null, ServiceLink[]? alsoOn = null, string[]? others = null) =>
        new(Id: 7, Case: kind, Main: main, AlsoOn: alsoOn ?? [], Yours: [], Others: others ?? [], Episodes: [], ChooseServices: false);

    [Fact]
    public void BeforeTheAnswerAPlaceholderKeepsTheRoom()
    {
        Assert.Equal(AvailabilityRoom.Held, AvailabilityLine.Room(null, TimeSpan.Zero));
        Assert.Equal(AvailabilityRoom.Held, AvailabilityLine.Room(null, Ms(1999)));
    }

    [Fact]
    public void AnAnswerWithALineKeepsTheRoomForIt()
    {
        Assert.Equal(AvailabilityRoom.Line, AvailabilityLine.Room(Answer(AvailabilityCase.PlaysAlsoOn, alsoOn: [Lumen, Harbor]), Ms(400)));
        Assert.Equal(AvailabilityRoom.Line, AvailabilityLine.Room(Answer(AvailabilityCase.OnService, main: Lumen, alsoOn: [Harbor]), Ms(400)));
        Assert.Equal(AvailabilityRoom.Line, AvailabilityLine.Room(Answer(AvailabilityCase.NotInSources, others: ["Lumen Plus"]), Ms(400)));
        // An answer that came late still draws its line.
        Assert.Equal(AvailabilityRoom.Line, AvailabilityLine.Room(Answer(AvailabilityCase.PlaysAlsoOn, alsoOn: [Lumen]), Ms(5000)));
    }

    [Fact]
    public void AnAnswerWithoutALineClosesTheRoom()
    {
        Assert.Equal(AvailabilityRoom.Closed, AvailabilityLine.Room(Answer(AvailabilityCase.PlaysAlsoOn), Ms(400)));
        // Watch on Lumen Plus is the main action, and no other service has it.
        Assert.Equal(AvailabilityRoom.Closed, AvailabilityLine.Room(Answer(AvailabilityCase.OnService, main: Lumen), Ms(400)));
        Assert.Equal(AvailabilityRoom.Closed, AvailabilityLine.Room(Answer(AvailabilityCase.NotInSources), Ms(400)));
    }

    [Fact]
    public void WithNoAnswerAfter2SecondsTheRoomCloses()
    {
        Assert.Equal(AvailabilityLine.WaitFor, TimeSpan.FromSeconds(2));
        Assert.Equal(AvailabilityRoom.Closed, AvailabilityLine.Room(null, Ms(2000)));
        Assert.Equal(AvailabilityRoom.Closed, AvailabilityLine.Room(null, Ms(9000)));
    }

    [Fact]
    public void AnAnswerCountsOnlyForWhatTheEpisodesSayOfTheSources()
    {
        var plays = Answer(AvailabilityCase.PlaysAlsoOn, alsoOn: [Lumen]);
        var listed = Answer(AvailabilityCase.NotInSources, others: ["Lantern Library"]);
        // Before the episodes, no answer counts: the placeholder stands.
        Assert.Null(AvailabilityLine.Counted(plays, null, Ms(300)));
        Assert.Equal(AvailabilityRoom.Held, AvailabilityLine.Room(AvailabilityLine.Counted(plays, null, Ms(300)), Ms(300)));
        // Once they are known, the answer for what they say counts, and one kept from before does not.
        Assert.Same(plays, AvailabilityLine.Counted(plays, true, Ms(300)));
        Assert.Null(AvailabilityLine.Counted(plays, false, Ms(300)));
        Assert.Same(listed, AvailabilityLine.Counted(listed, false, Ms(300)));
        Assert.Null(AvailabilityLine.Counted(listed, true, Ms(300)));
        Assert.True(AvailabilityLine.PlaysHere(Answer(AvailabilityCase.PlaysHere)));
        Assert.False(AvailabilityLine.PlaysHere(Answer(AvailabilityCase.OnService, main: Lumen)));
        // After the 2 s cap, whatever answer there is counts.
        Assert.Same(plays, AvailabilityLine.Counted(plays, null, Ms(2000)));
        Assert.Same(plays, AvailabilityLine.Counted(plays, false, Ms(2000)));
        Assert.Null(AvailabilityLine.Counted(null, true, Ms(2000)));
    }

    [Fact]
    public void ThePlaceholderHoldsTwoRowsWhileNoSourcePlaysTheTitle()
    {
        EpisodeRow[] unlisted = [new EpisodeRow("1", MetadataOnly: true), new EpisodeRow("2", MetadataOnly: true)];
        EpisodeRow[] listed = [new EpisodeRow("1"), new EpisodeRow("2", Unsourced: true)];
        Assert.True(AvailabilityLine.Unlisted(unlisted));
        Assert.Equal(2, AvailabilityLine.HeldRows(unlisted));
        Assert.False(AvailabilityLine.Unlisted(listed));
        Assert.Equal(1, AvailabilityLine.HeldRows(listed));
        // Episodes not yet known, or none at all: the one row of Also on.
        Assert.Equal(1, AvailabilityLine.HeldRows(null));
        Assert.Equal(1, AvailabilityLine.HeldRows([]));
    }
}
