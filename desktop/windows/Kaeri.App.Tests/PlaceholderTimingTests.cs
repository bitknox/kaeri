// When placeholders show and how long they stay: at once and with no hold on the title page,
// after 600 ms and for at least 0.3 s everywhere else.

using Kaeri.App;
using Kaeri.Engine.Model;
using Xunit;

namespace Kaeri.App.Tests;

public class PlaceholderTimingTests
{
    static TimeSpan Ms(int ms) => TimeSpan.FromMilliseconds(ms);

    [Fact]
    public void TheTitlePageShowsItsPlaceholdersFromTheFirstFrame()
    {
        var title = PlaceholderTiming.TitlePage;
        Assert.True(title.AtOnce);
        Assert.True(title.Shown(TimeSpan.Zero));
    }

    [Fact]
    public void OnTheTitlePageContentReplacesThePlaceholdersTheMomentItLands()
    {
        var title = PlaceholderTiming.TitlePage;
        // Placeholders that have stood no time at all give way at once: no 0.3 s hold.
        Assert.Equal(TimeSpan.Zero, title.Left(TimeSpan.Zero));
        Assert.Equal((true, Ms(0)), title.Landing(Ms(0)));
        Assert.Equal((true, Ms(120)), title.Landing(Ms(120)));
        Assert.Equal((true, Ms(700)), title.Landing(Ms(700)));
    }

    [Fact]
    public void OtherScreensWait600MsAndHoldTheirPlaceholders300Ms()
    {
        var elsewhere = PlaceholderTiming.Elsewhere;
        Assert.False(elsewhere.AtOnce);
        Assert.False(elsewhere.Shown(Ms(599)));
        Assert.True(elsewhere.Shown(Ms(600)));
        Assert.Equal(Ms(300), elsewhere.Left(TimeSpan.Zero));
        Assert.Equal(Ms(200), elsewhere.Left(Ms(100)));
        Assert.Equal(TimeSpan.Zero, elsewhere.Left(Ms(300)));
        // A fast load shows no placeholders; one that lands just after 600 ms waits out the hold.
        Assert.Equal((false, Ms(450)), elsewhere.Landing(Ms(450)));
        Assert.Equal((true, Ms(900)), elsewhere.Landing(Ms(700)));
        Assert.Equal((true, Ms(1200)), elsewhere.Landing(Ms(1200)));
    }

    [Fact]
    public void TheTitlePageWaitsForItsWholePage()
    {
        var row = new Entry(Id: 1, Title: "Lantern Road", Synopsis: "A lamplighter walks the coast.");
        Assert.True(PlaceholderTiming.TitleWaits(null, fetchEnded: false));
        Assert.True(PlaceholderTiming.TitleWaits(row, fetchEnded: false));
        // A season's kept header comes first and still leaves the rest waiting.
        Assert.True(PlaceholderTiming.TitleWaits(row with { Detail = new TitleDetail(Partial: true) }, fetchEnded: false));
        Assert.False(PlaceholderTiming.TitleWaits(row with { Detail = new TitleDetail(Partial: false) }, fetchEnded: false));
        Assert.False(PlaceholderTiming.TitleWaits(row with { IsDemo = true }, fetchEnded: false));
        // The fetch ended without the whole page: the list's row stands, no placeholders for good.
        Assert.False(PlaceholderTiming.TitleWaits(row, fetchEnded: true));
    }
}
