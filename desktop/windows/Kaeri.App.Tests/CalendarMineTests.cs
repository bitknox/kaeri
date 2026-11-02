using Kaeri.App;
using Kaeri.Engine.Model;
using Xunit;

namespace Kaeri.App.Tests;

public class CalendarMineTests
{
    static Entry Title(LibrarySegment? status, TitleProgress? progress) =>
        new(Id: 1, Title: "Lantern Harbour", Progress: progress, ListStatus: status);

    static readonly TitleProgress Started = new(Done: 3, Next: 4);

    [Fact]
    public void ADroppedTitleIsNotMineWhateverItsProgress()
    {
        Assert.False(Calendar.IsMine(Title(LibrarySegment.Dropped, Started)));
        Assert.False(Calendar.IsMine(Title(LibrarySegment.Dropped, null)));
    }

    [Fact]
    public void WatchingAndPlanningAreMine()
    {
        Assert.True(Calendar.IsMine(Title(LibrarySegment.Watching, null)));
        Assert.True(Calendar.IsMine(Title(LibrarySegment.Planning, null)));
    }

    [Fact]
    public void ProgressWithNoStatusIsMine()
    {
        Assert.True(Calendar.IsMine(Title(null, Started)));
        Assert.False(Calendar.IsMine(Title(null, null)));
    }
}
