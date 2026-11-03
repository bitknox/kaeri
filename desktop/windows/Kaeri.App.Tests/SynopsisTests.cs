// The episode list's rows: their height, what spoiler protection holds back, their facts and the ranges of a long series.

using System.Globalization;
using Kaeri.App;
using Kaeri.Engine.Model;
using Xunit;

namespace Kaeri.App.Tests;

public class SynopsisTests
{
    static readonly CultureInfo Gb = CultureInfo.GetCultureInfo("en-GB");

    static EpisodeRow Row(int n, bool completed = false, bool ahead = false, ulong? secs = 1440) =>
        new(n.ToString(), DurationSecs: secs, Completed: completed, PastProgress: ahead, SpoilerAhead: ahead);

    /// <summary>A table whose next episode is <paramref name="next"/>: the ones before it watched, the ones after it ahead.</summary>
    static EpisodeRow[] Table(int count, int next) =>
        [.. Enumerable.Range(1, count).Select(n => Row(n, completed: n < next, ahead: n > next))];

    static string OnThe21st(long _) => "21 May 2014";

    [Fact]
    public void AWideRowKeepsItsHeightAndANarrowOneFollowsTheText()
    {
        Assert.Equal(128, Synopsis.RowHeight(false, true));
        Assert.Equal(128, Synopsis.RowHeight(false, false));
        Assert.Equal(164, Synopsis.RowHeight(true, true));
        Assert.Equal(100, Synopsis.RowHeight(true, false));
    }

    [Fact]
    public void RoomIsKeptOnlyWhereTextCanLand()
    {
        Assert.True(Synopsis.HasRoom(EpisodeText.Present, false));
        Assert.True(Synopsis.HasRoom(EpisodeText.Expected, false));
        Assert.False(Synopsis.HasRoom(EpisodeText.None, false));
        Assert.False(Synopsis.HasRoom(EpisodeText.Present, true));
    }

    [Fact]
    public void ARowIsHeldBackOnlyWhileProtectionIsOnAndNotRevealed()
    {
        var rows = Table(4, 2);
        Assert.False(Synopsis.Held(rows[1], true, false));
        Assert.True(Synopsis.Held(rows[2], true, false));
        Assert.False(Synopsis.Held(rows[2], false, false));
        Assert.False(Synopsis.Held(rows[2], true, true));
        Assert.True(Synopsis.Protectable(rows[2], true));
    }

    [Fact]
    public void TheLineGoesBeforeTheFirstRowAheadAndNamesTheEpisodeBeforeIt()
    {
        Assert.Equal((2, 2L), Synopsis.ProtectionStart(Table(5, 2), true));
        Assert.Null(Synopsis.ProtectionStart(Table(5, 2), false));
        Assert.Null(Synopsis.ProtectionStart(Table(3, 9), true));
        Assert.Equal((0, 0L), Synopsis.ProtectionStart([Row(1, ahead: true), Row(2, ahead: true)], true));
    }

    [Fact]
    public void FactsSayTheLengthTheDateAndWhatIsLeft()
    {
        var row = Row(1);
        Assert.Equal("24 min", Synopsis.Facts(row, 100, OnThe21st));
        row = row with { AirsAt = 50 };
        Assert.Equal("24 min · Aired 21 May 2014", Synopsis.Facts(row, 100, OnThe21st));
        Assert.Equal("12 min left · Aired 21 May 2014", Synopsis.Facts(row with { Watched = 0.5 }, 100, OnThe21st));
        Assert.Equal("24 min · Aired 21 May 2014 · Filler", Synopsis.Facts(row with { Kind = EpisodeKind.Filler }, 100, OnThe21st));
        Assert.Equal("Airs 21 May 2014", Synopsis.Facts(Row(2, secs: null) with { AirsAt = 500 }, 100, OnThe21st));
        Assert.Equal("", Synopsis.Facts(Row(3, secs: null), 100, OnThe21st));
    }

    [Fact]
    public void NarrowFactsLeadWithTheNumberAndPutTheDateOnItsOwnLine()
    {
        var row = Row(8) with { AirsAt = 50 };
        Assert.Equal("E8 · 24 min\nAired 21 May 2014", Synopsis.Facts(row, 100, OnThe21st, 8));
        Assert.Equal("E8 · 12 min left\nAired 21 May 2014", Synopsis.Facts(row with { Watched = 0.5 }, 100, OnThe21st, 8));
        Assert.Equal("E8 · 24 min · Filler\nAired 21 May 2014", Synopsis.Facts(row with { Kind = EpisodeKind.Filler }, 100, OnThe21st, 8));
        Assert.Equal("E8 · 24 min", Synopsis.Facts(row with { AirsAt = null }, 100, OnThe21st, 8));
    }

    [Fact]
    public void ADateIsWordedWithTheDayFirst()
    {
        Assert.Equal("21 May 2014", Synopsis.Date(new DateTimeOffset(2014, 5, 21, 12, 0, 0, TimeSpan.Zero).ToUnixTimeSeconds(), Gb));
    }

    [Fact]
    public void RangeItemsAreWordedByTheNumbersTheyHoldAndSayWhatIsDone()
    {
        var rows = Table(112, 101).Select((r, i) => i >= 100 ? r with { AirsAt = 900 } : r).ToArray();
        var ranges = Kaeri.Engine.Ffi.KaeriFfiMethods.EpisodeRanges(rows, 100);
        var items = Synopsis.RangeItems(rows, ranges);
        Assert.Equal(2, items.Length);
        Assert.Equal(new RangeItem("Episodes 1 to 100", "Watched"), items[0]);
        Assert.Equal(new RangeItem("Episodes 101 to 112", "Not out yet"), items[1]);
        Assert.Equal("Episodes 1 to 100, Watched", items[0].Spoken);
    }

    [Fact]
    public void ALongTableOpensOnTheRangeHoldingTheNextEpisodeWithTheWatchedRowsFolded()
    {
        var rows = Table(250, 230);
        var (ranges, at, plan) = Synopsis.Layout(rows, null, false, 0);
        Assert.Equal(3, ranges.Length);
        Assert.Equal(2, at);
        Assert.Equal(new SynopsisPlan(200, 250, 29), plan);
        Assert.Equal(0, Synopsis.Layout(rows, null, true, 0).Plan.Fold);
        Assert.Equal(new SynopsisPlan(0, 100, 0), Synopsis.Layout(rows, 0, false, 0).Plan with { Fold = 0 });
    }

    [Fact]
    public void AShortTableHasNoRangesAndARunOfOneWatchedRowIsNotFolded()
    {
        var (ranges, _, plan) = Synopsis.Layout(Table(10, 4), null, false, 0);
        Assert.Empty(ranges);
        Assert.Equal(new SynopsisPlan(0, 10, 3), plan);
        Assert.Equal(0, Synopsis.Layout(Table(10, 2), null, false, 0).Plan.Fold);
    }

    [Fact]
    public void TheOutlookReadsPresentExpectedOrNone()
    {
        var rows = Table(3, 1);
        Assert.Equal(EpisodeText.None, Kaeri.Engine.Ffi.KaeriFfiMethods.EpisodeText(rows));
        rows[1] = rows[1] with { SynopsisPending = true };
        Assert.Equal(EpisodeText.Expected, Kaeri.Engine.Ffi.KaeriFfiMethods.EpisodeText(rows));
        rows[2] = rows[2] with { Description = "A storm closes the pass." };
        Assert.Equal(EpisodeText.Present, Kaeri.Engine.Ffi.KaeriFfiMethods.EpisodeText(rows));
    }
}
