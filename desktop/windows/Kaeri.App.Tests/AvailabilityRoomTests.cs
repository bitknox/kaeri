// The title page's availability line: a placeholder keeps its room until the answer, which keeps
// the room for a line and closes it for none, and the wait ends after 2 s with no answer.

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
}
