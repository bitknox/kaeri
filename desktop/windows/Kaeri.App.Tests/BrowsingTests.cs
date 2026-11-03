// The browsing extras' Windows side: the count line's two forms, a cell's second line, Trending's periods, and Go to episode.

using Kaeri.App;
using Kaeri.Engine.Model;
using Xunit;

namespace Kaeri.App.Tests;

public class BrowsingTests
{
    static EpisodeRow Row(int n) => new(n.ToString());

    static CountLine Line() => new("402 of 412 · Dub 64", "402 of 412", "402 of 412 episodes, dub up to 64", "Dub 64");

    [Fact]
    public void ACellTakesTheLongCountOnlyWhereItIsWideAndTheLineFits()
    {
        double Wide(string s) => s.Length * 7;
        Assert.Equal("402 of 412", Browsing.CountText(Line(), 130, Wide));
        Assert.Equal("402 of 412 · Dub 64", Browsing.CountText(Line(), 150, Wide));
        Assert.Equal("402 of 412", Browsing.CountText(Line(), 110, _ => 1));
    }

    [Fact]
    public void ALibraryLineKeepsTheDub()
    {
        var entry = new Entry(Id: 1, Title: "Lantern Road", CountLine: Line());
        Assert.Equal("E7 of 24 · Dub 64", Browsing.WithDub("E7 of 24", entry));
        Assert.Equal("Dub 64", Browsing.WithDub("", entry));
    }

    [Fact]
    public void TrendingHasThreePeriodsSaidInTheCataloguesWords()
    {
        Assert.Equal([TrendPeriod.Today, TrendPeriod.Week, TrendPeriod.Month], Browsing.Periods);
        Assert.Equal(T.TrendingWeek, Browsing.Heading(TrendPeriod.Week));
        Assert.Equal(T.TrendingChoiceMonth, Browsing.Choice(TrendPeriod.Month));
    }

    [Fact]
    public void ASortCarriesItsPeriodOnlyForTrendingWeekAndMonth()
    {
        var week = Searches.With("", SearchFilters.None with { Sort = SearchSort.Trending, Period = TrendPeriod.Week });
        Assert.Equal(TrendPeriod.Week, week.Period);
        var score = Searches.With("", SearchFilters.None with { Sort = SearchSort.Score, Period = TrendPeriod.Week });
        Assert.Null(score.Period);
        Assert.Equal(T.TrendingMonth, Searches.SortName(SearchSort.Trending, TrendPeriod.Month));
    }

    [Fact]
    public void GoingToANumberFindsItsRowOrSaysWhy()
    {
        var rows = Enumerable.Range(1, 12).Select(Row).ToArray();
        Assert.IsType<GoTo.Row>(Browsing.Go(rows, "7"));
        var none = Browsing.Go(rows, "40");
        Assert.IsType<GoTo.NoSuch>(none);
        Assert.Equal(T.EpisodeGoToNone(40, 12), Browsing.GoMessage(none));
        Assert.Null(Browsing.GoMessage(Browsing.Go(rows, "")));
    }

    [Fact]
    public void SteppingARangeStopsAtTheFirstAndTheLast()
    {
        Assert.Equal(0, Browsing.StepRange(3, 0, -1));
        Assert.Equal(2, Browsing.StepRange(3, 2, 1));
        Assert.Equal(1, Browsing.StepRange(3, 0, 1));
    }
}
