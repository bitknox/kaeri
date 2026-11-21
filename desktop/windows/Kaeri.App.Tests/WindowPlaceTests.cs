// Where the window opens and what is kept of it: a window left maximised opens maximised again,
// and Restore down takes it back to the frame it had before, on the display it is on.

using Kaeri.App;
using Xunit;

namespace Kaeri.App.Tests;

public class WindowPlaceTests
{
    // A 1920 by 1080 display with the taskbar along its bottom.
    static readonly WindowFrame Display = new(0, 0, 1920, 1080);
    static readonly WindowFrame Work = new(0, 0, 1920, 1032);

    /// <summary>The window maximised on a work area: past it on every side by the resize border.</summary>
    static WindowFrame MaximisedOn(WindowFrame work, int border) =>
        new(work.X - border, work.Y - border, work.Width + 2 * border, work.Height + 2 * border);

    [Fact]
    public void AFirstMaximiseKeepsTheFrameTheWindowOpenedWith()
    {
        // Nothing kept yet: the first launch's frame, then Maximise.
        var opened = WindowPlace.First(Work, 1.0);
        var kept = WindowPlace.Keep(maximised: true, current: MaximisedOn(Work, 8), normal: opened, kept: null);
        Assert.Equal(opened, kept);
    }

    [Fact]
    public void AWindowMovedMaximisedToAnotherDisplayKeepsThatDisplaysFrame()
    {
        // Kept on the first display, then Win+Shift+Right to a 125% display on its right, where
        // Windows moved the frame Restore down goes back to.
        var onFirst = new WindowFrame(192, 103, 1536, 825);
        var secondWork = new WindowFrame(1920, 0, 2560, 1392);
        var onSecond = new WindowFrame(2240, 196, 1920, 1000);
        var kept = WindowPlace.Keep(maximised: true, current: MaximisedOn(secondWork, 10), normal: onSecond, kept: onFirst);
        Assert.Equal(onSecond, kept);
    }

    [Fact]
    public void ARestoredWindowKeepsItsOwnFrame()
    {
        var current = new WindowFrame(300, 200, 1200, 800);
        Assert.Equal(current, WindowPlace.Keep(maximised: false, current, normal: current, kept: new WindowFrame(0, 0, 1280, 800)));
    }

    [Fact]
    public void WhenWindowsDoesNotSayTheMaximisedRectangleIsStillNotKept()
    {
        var before = new WindowFrame(300, 200, 1200, 800);
        Assert.Equal(before, WindowPlace.Keep(maximised: true, MaximisedOn(Work, 8), normal: null, kept: before));
        Assert.Null(WindowPlace.Keep(maximised: true, MaximisedOn(Work, 8), normal: null, kept: null));
    }

    [Fact]
    public void TheNormalFrameIsMovedOutOfWorkspaceCoordinates()
    {
        var normal = new WindowFrame(100, 50, 800, 600);
        // Taskbar along the bottom: the work area starts at the display's corner.
        Assert.Equal(normal, WindowPlace.FromWorkspace(normal, Display, Work));
        // Taskbar on the left.
        Assert.Equal(new WindowFrame(148, 50, 800, 600), WindowPlace.FromWorkspace(normal, Display, new WindowFrame(48, 0, 1872, 1080)));
        // A second display with its taskbar at the top.
        Assert.Equal(
            new WindowFrame(2100, 148, 1200, 800),
            WindowPlace.FromWorkspace(new WindowFrame(2100, 100, 1200, 800), new WindowFrame(1920, 0, 2560, 1440), new WindowFrame(1920, 48, 2560, 1392)));
    }

    [Theory]
    [InlineData(8, 1.0)]
    [InlineData(10, 1.25)]
    [InlineData(16, 2.0)]
    public void APlaceKeptAsTheMaximisedRectangleOpensMaximisedOverTheFirstFrame(int border, double scale)
    {
        var opening = WindowPlace.Opening(MaximisedOn(Work, border), maximised: true, Work, scale);
        Assert.Equal(WindowPlace.First(Work, scale), opening);
    }

    [Fact]
    public void AKeptFrameOpensWhereItWasLeft()
    {
        var frame = new WindowFrame(300, 200, 1200, 800);
        Assert.Equal(frame, WindowPlace.Opening(frame, maximised: true, Work, 1.0));
        Assert.Equal(frame, WindowPlace.Opening(frame, maximised: false, Work, 1.0));
    }

    [Fact]
    public void AFrameWhoseTitleStripCannotBeGrabbedOpensAtTheFirstFrame()
    {
        Assert.Null(WindowPlace.Opening(new WindowFrame(300, 200, 1200, 800), maximised: false, workArea: null, 1.0));
        // Its top within 40 pixels of the work area's bottom.
        Assert.Null(WindowPlace.Opening(new WindowFrame(300, 1000, 1200, 800), maximised: false, Work, 1.0));
        // Only 100 pixels of it on the display.
        Assert.Null(WindowPlace.Opening(new WindowFrame(1820, 200, 1200, 800), maximised: true, Work, 1.0));
    }

    [Fact]
    public void TheFirstFrameIs80PercentOfTheWorkAreaCentredAndAtLeast1280By800()
    {
        Assert.Equal(new WindowFrame(192, 103, 1536, 825), WindowPlace.First(Work, 1.0));
        Assert.Equal(new WindowFrame(43, 0, 1280, 728), WindowPlace.First(new WindowFrame(0, 0, 1366, 728), 1.0));
        Assert.Equal(new WindowFrame(2176, 96, 2048, 1200), WindowPlace.First(new WindowFrame(1920, 0, 2560, 1392), 1.5));
    }
}
