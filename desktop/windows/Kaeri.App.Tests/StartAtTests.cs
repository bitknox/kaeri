using Kaeri.App;
using Kaeri.Engine.Model;
using Xunit;

namespace Kaeri.App.Tests;

public class StartAtTests
{
    static Event Staged(uint id, string episode) => new Event.PlaybackStaged(id, "Lantern Street", episode, null, 1.0, null, false);

    [Fact]
    public void PlayHereStartsWhereTheTvWasOnlyPastItsFirstSeconds()
    {
        Assert.Equal(new StartAt(7, "3", 642.5), StartAt.From(7, "3", 642.5));
        Assert.Null(StartAt.From(7, "3", 4.9));
        Assert.Null(StartAt.From(7, "3", null));
    }

    [Fact]
    public void AnotherPlayStagedFirstGivesThePlaceUp()
    {
        var known = new Known { StartAt = new StartAt(7, "3", 642.5) };
        known.Take(Staged(7, "3"));
        Assert.NotNull(known.StartAt);
        known.Take(Staged(7, "4"));
        Assert.Null(known.StartAt);
    }
}
