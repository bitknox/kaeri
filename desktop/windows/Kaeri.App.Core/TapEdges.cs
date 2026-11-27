using Kaeri.Engine.Model;

namespace Kaeri.App;

/// <summary>
/// The edge a novel's page turns itself on, from the device's tap zone settings, until the
/// Click zones page and the engine's hit test come to the novel too: the paginator knows only a
/// fraction of the width at each side. Off turns nothing; any other layout turns on the outer
/// third, or the outer quarter at Small, as Sides does.
/// </summary>
public static class TapEdges
{
    public static double Novel(TapZoneSettings? zones, bool scrolled)
    {
        // No settings yet: Sides, the desktop's own start, for pages; Off for a scrolled novel.
        if (zones is null)
            return scrolled ? 0 : 1 / 3.0;
        var layout = scrolled ? zones.Scrolling : zones.Paged;
        if (layout == ZoneLayout.Off)
            return 0;
        return zones.Size == ZoneSize.Small ? 0.25 : 1 / 3.0;
    }

    /// <summary>Which way a click on a novel's end page goes: forward, back, or neither (the controls), by the same edge.</summary>
    public static bool? EndPage(double x, double edge, bool rtl)
    {
        if (edge <= 0 || x >= edge && x <= 1 - edge)
            return null;
        var right = x > 1 - edge;
        return right != rtl;
    }
}
