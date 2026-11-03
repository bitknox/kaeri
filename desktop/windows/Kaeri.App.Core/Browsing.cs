using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;

namespace Kaeri.App;

/// <summary>
/// The browsing extras' rules that have no control of their own: the count line in a cell, the
/// Trending period's words, and what Go to episode makes of a number typed. The engine decides
/// each; this is how Windows asks and words it.
/// </summary>
public static class Browsing
{
    /// <summary>Under this many epx a cell takes the short count without measuring.</summary>
    public const double LongCountFrom = 120;

    /// <summary>
    /// Which form of a count line a cell shows: the long one when the cell has room and the line
    /// fits <paramref name="room"/> as <paramref name="width"/> measures it, else the short one,
    /// which keeps the dub part. A cell under 120 takes the short form unmeasured.
    /// </summary>
    public static string CountText(CountLine line, double room, Func<string, double> width) =>
        room >= LongCountFrom && width(line.Long) <= room ? line.Long : line.Short;

    /// <summary>
    /// A cell's second line: "TV · Plays here". The count has the first line, so the episode count
    /// is not said again, and the year stands in only when there is no count to say.
    /// </summary>
    public static string CellDetail(Entry entry)
    {
        if (entry.IsDemo)
            return T.SearchDemoPlays;
        var parts = new List<string>();
        if (entry.Format is { Length: > 0 } format)
            parts.Add(Wording.Format(format));
        if (entry.CountLine is null && entry.Year is { } year)
            parts.Add(year.ToString(System.Globalization.CultureInfo.InvariantCulture));
        if (entry.AvailableAt is { } at)
            parts.Add(Wording.Where(at));
        return string.Join(" \u00b7 ", parts);
    }

    /// <summary>Library's one line with the dub after it: "E7 of 24 · Dub 8".</summary>
    public static string WithDub(string line, Entry entry) =>
        entry.CountLine?.Dub is { Length: > 0 } dub ? line.Length == 0 ? dub : $"{line} \u00b7 {dub}" : line;

    /// <summary>The periods of the Trending choice, in order: today, this week, this month.</summary>
    public static TrendPeriod[] Periods => KaeriFfiMethods.TrendPeriods();

    /// <summary>"Trending today", "Trending this week": the rail's heading and the Sort menu's item.</summary>
    public static string Heading(TrendPeriod period) => KaeriFfiMethods.TrendPeriodLabel(period);

    /// <summary>"Today", "This week", "This month": a choice of the period menu.</summary>
    public static string Choice(TrendPeriod period) => KaeriFfiMethods.TrendPeriodChoice(period);

    /// <summary>What the number typed means for the table as of now.</summary>
    public static GoTo Go(EpisodeRow[] rows, string typed) =>
        KaeriFfiMethods.GoToEpisode(rows, typed, DateTimeOffset.UtcNow.ToUnixTimeSeconds());

    /// <summary>What Go to episode says of a number that is not a row, in the catalogue's words; null for a row or nothing typed.</summary>
    public static string? GoMessage(GoTo result) => KaeriFfiMethods.GoToMessage(result);

    /// <summary>The range after stepping by <paramref name="by"/>, held to the first and the last.</summary>
    public static int StepRange(int count, int current, int by) =>
        (int)KaeriFfiMethods.EpisodeRangeStep((uint)count, (uint)current, by);
}
