using System.Globalization;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;

namespace Kaeri.App;

/// <summary>What one range of the episode list shows: rows <c>Start</c> to <c>End</c> of the table, the first <c>Fold</c> of them as one line.</summary>
public readonly record struct SynopsisPlan(int Start, int End, int Fold);

/// <summary>One item of the range picker: the range's words and what it says of itself, "Watched" or "Not out yet".</summary>
public readonly record struct RangeItem(string Label, string? State)
{
    /// <summary>What Narrator reads for the item.</summary>
    public string Spoken => State is null ? Label : T.EpisodeRowSpoken(Label, State);
}

/// <summary>
/// What the episode list's rows decide about synopses, apart from any control: how tall a row is,
/// which rows spoiler protection holds back and where its line goes, what a row's facts say, and
/// how a long series splits into ranges with its watched episodes folded. The title page draws
/// what these say.
/// </summary>
public static class Synopsis
{
    /// <summary>A still beside the text on a wide row; a narrower window stacks the text under a smaller still.</summary>
    public const double StillWide = 192, StillNarrow = 144;

    /// <summary>
    /// A row's fixed height: 128 beside the text, and on a narrow window 164 with the text under
    /// the still or 100 without. Computed from the size, never from the text.
    /// </summary>
    public const double RowWide = 128, RowNarrowTall = 164, RowNarrowShort = 100;

    /// <summary>The room three lines of body text take.</summary>
    public const double TextHeight = 60;

    /// <summary>Under this many epx the list uses the narrow row.</summary>
    public const double NarrowBelow = 640;

    /// <summary>How many leading watched rows are worth a line of their own.</summary>
    const int FoldLeast = 2;

    /// <summary>
    /// The row's height. A wide row keeps its height whether or not it has text, since the text
    /// sits beside the still.
    /// </summary>
    public static double RowHeight(bool narrow, bool room) =>
        !narrow ? RowWide : room ? RowNarrowTall : RowNarrowShort;

    /// <summary>Whether a row keeps room for text: text is there or on its way, and protection is not holding this row back.</summary>
    public static bool HasRoom(EpisodeText text, bool held) => text != EpisodeText.None && !held;

    /// <summary>The row's synopsis, when it has one.</summary>
    public static string? TextOf(EpisodeRow row) =>
        row.Description?.Trim() is { Length: > 0 } text ? text : null;

    /// <summary>Whether protection could hold this row back: it is ahead and unwatched, whether or not the viewer asked to see it.</summary>
    public static bool Protectable(EpisodeRow row, bool on, long next) =>
        on && row.PastProgress && !row.Completed && Number(row.Number) is { } n && n > next;

    /// <summary>Whether protection holds the row back now: protectable, and the viewer has not asked to see it.</summary>
    public static bool Held(EpisodeRow row, bool on, long next, bool shown) => Protectable(row, on, next) && !shown;

    /// <summary>
    /// Where protection starts in <paramref name="rows"/>: the index of the first row ahead and
    /// the number of the last episode before it, which the line names.
    /// </summary>
    public static (int At, long Before)? ProtectionStart(IReadOnlyList<EpisodeRow> rows, bool on, long next)
    {
        if (!on)
            return null;
        for (var at = 0; at < rows.Count; at++)
        {
            if (!Protectable(rows[at], on, next))
                continue;
            var before = at > 0 ? Number(rows[at - 1].Number) : Number(rows[at].Number) is { } n ? n - 1 : null;
            return (at, Math.Max(0, before ?? 0));
        }
        return null;
    }

    /// <summary>An episode number as a whole number, a half episode taking the one before it.</summary>
    public static long? Number(string text) =>
        long.TryParse(text, NumberStyles.Integer, CultureInfo.InvariantCulture, out var n) ? n
        : double.TryParse(text, NumberStyles.Float, CultureInfo.InvariantCulture, out var f) ? (long)Math.Floor(f) : null;

    /// <summary>
    /// A row's facts line: its length, or what is left of it, and when it aired; a filler episode
    /// says so, an unreleased one says when it airs. <paramref name="now"/> is Unix seconds.
    /// </summary>
    public static string Facts(EpisodeRow row, long now, Func<long, string> date)
    {
        static long Minutes(double secs) => (long)Math.Max(1, Math.Round(secs / 60));
        long? aired = row.AirsAt is { } at && at <= now ? at : null;
        var line = row.Unsourced ? T.EpisodeUnsourced
            : row.DurationSecs is { } secs
                ? row.Watched > 0 && !row.Completed ? T.EpisodeFactsLeft(Minutes(secs * (1 - row.Watched)))
                    : aired is { } on ? T.EpisodeFacts(Minutes(secs), date(on)) : T.EpisodeFactsLength(Minutes(secs))
            : row.AirsAt is { } later && later > now ? T.EpisodeRowAirs(date(later))
            : row.MetadataOnly ? T.EpisodeRowNotOut
            : aired is { } was ? date(was) : "";
        return row.Kind == EpisodeKind.Filler && line.Length > 0 ? $"{line} · {T.EpisodeFiller}" : line;
    }

    /// <summary>The date of a Unix time as the row's facts word it: "21 May 2014".</summary>
    public static string Date(long unix, CultureInfo? culture = null) =>
        DateTimeOffset.FromUnixTimeSeconds(unix).ToLocalTime().ToString("d MMM yyyy", culture ?? CultureInfo.CurrentCulture);

    /// <summary>One item per range, labelled by the episode numbers it holds.</summary>
    public static RangeItem[] RangeItems(IReadOnlyList<EpisodeRow> rows, IReadOnlyList<EpisodeRange> ranges) =>
    [
        .. ranges.Select(range =>
        {
            var from = Number(rows[(int)range.Start].Number) ?? range.Start + 1;
            var to = Number(rows[(int)range.End - 1].Number) ?? range.End;
            return new RangeItem(T.EpisodeRange(from, to), range.Watched ? T.EpisodeWatched : range.NotOut ? T.EpisodeRowNotOut : null);
        }),
    ];

    /// <summary>
    /// The plan for <paramref name="range"/> of <paramref name="rows"/>, or for the whole table when
    /// it has no ranges. <paramref name="fold"/> is the engine's count of leading watched rows; a
    /// run shorter than two stays as rows, and nothing folds once the viewer opened the line.
    /// </summary>
    public static SynopsisPlan Plan(int count, EpisodeRange? range, uint fold, bool opened)
    {
        var (start, end) = range is { } r ? ((int)r.Start, (int)r.End) : (0, count);
        return new(start, end, opened || fold < FoldLeast ? 0 : (int)fold);
    }

    /// <summary>The ranges, the one to open and its plan for a table, asking the engine.</summary>
    public static (EpisodeRange[] Ranges, int Selected, SynopsisPlan Plan) Layout(EpisodeRow[] rows, int? chosen, bool opened, long now)
    {
        var ranges = KaeriFfiMethods.EpisodeRanges(rows, now);
        var at = ranges.Length == 0 ? 0 : Math.Clamp(chosen ?? (int)KaeriFfiMethods.EpisodeRangeToOpen(rows), 0, ranges.Length - 1);
        var range = ranges.Length == 0 ? new EpisodeRange(0, (uint)rows.Length, false, false) : ranges[at];
        return (ranges, at, Plan(rows.Length, range, KaeriFfiMethods.EpisodeFold(rows, range), opened));
    }
}
