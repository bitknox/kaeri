using Kaeri.App;
using Kaeri.Engine.Model;
using Xunit;

namespace Kaeri.App.Tests;

public class TapEdgesTests
{
    static TapZoneSettings Zones(ZoneLayout paged, ZoneSize size = ZoneSize.Standard, ZoneLayout scrolling = ZoneLayout.Off) =>
        new(Paged: paged, Scrolling: scrolling, Size: size);

    [Fact]
    public void ANovelTurnsOnTheEdgesTheDevicesTapZonesGive()
    {
        Assert.Equal(1 / 3.0, TapEdges.Novel(Zones(ZoneLayout.Sides), scrolled: false), 3);
        Assert.Equal(0.25, TapEdges.Novel(Zones(ZoneLayout.Sides, ZoneSize.Small), scrolled: false), 3);
        // A saved Off is kept: taps only show and hide the controls.
        Assert.Equal(0, TapEdges.Novel(Zones(ZoneLayout.Off), scrolled: false));
        Assert.Equal(0, TapEdges.Novel(Zones(ZoneLayout.Sides), scrolled: true));
        Assert.Equal(1 / 3.0, TapEdges.Novel(null, scrolled: false), 3);
    }

    [Fact]
    public void AClickOnTheEndPagesEdgeTurnsAndItsMiddleShowsTheControls()
    {
        Assert.True(TapEdges.EndPage(0.9, 1 / 3.0, rtl: false));
        Assert.False(TapEdges.EndPage(0.1, 1 / 3.0, rtl: false));
        Assert.Null(TapEdges.EndPage(0.5, 1 / 3.0, rtl: false));
        Assert.True(TapEdges.EndPage(0.1, 1 / 3.0, rtl: true));
        Assert.Null(TapEdges.EndPage(0.9, 0, rtl: false));
    }
}
