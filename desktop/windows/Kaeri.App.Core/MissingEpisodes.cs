using System.Globalization;
using Kaeri.Engine.Model;

namespace Kaeri.App;

/// <summary>
/// The status under a title's buttons while the next episode has aired and no source lists it:
/// a head in silk ("Episode 6 aired 25 min ago.") and a tail in haze that says where the looking
/// stands. <see cref="Spoken"/> is the two together, as Narrator reads the line.
/// </summary>
/// <param name="Head">The first sentence, or empty when the schedule does not date the episode.</param>
/// <param name="Tail">The rest.</param>
/// <param name="LongAgo">A week or more after airing: the dot turns haze, as water marks a new episode.</param>
/// <param name="CheckAgain">Whether Check again is offered: until a week after airing.</param>
/// <param name="Help">Check again's tooltip and hint, during the hours Kaeri looks by itself only.</param>
public sealed record MissingStatus(string Head, string Tail, bool LongAgo, bool CheckAgain, string? Help)
{
    public string Spoken => Head.Length == 0 ? Tail : Tail.Length == 0 ? Head : $"{Head} {Tail}";
}

/// <summary>
/// The words for an aired episode that no source lists, from what the engine works out
/// (<c>missing_episode</c>, <c>aired_ago</c>, <c>unsourced_yet</c>, <c>coverage_line</c>). The
/// engine says which; this writes the sentences.
/// </summary>
public static class MissingWords
{
    /// <summary>
    /// The episode the title's page is missing, when it is: the next one to watch has aired, no
    /// source lists it and none lists a later one. Null while the table has not come.
    /// </summary>
    public static MissingEpisode? Of(Entry entry, EpisodeRow[]? rows, long now) =>
        rows is null ? null : KaeriModelMethods.MissingEpisode(rows, entry.Progress?.Next ?? 1, entry.Resume is not null, now);

    /// <summary>"25 min ago", "2 h ago", or null from a day after airing on, when the date is said instead.</summary>
    public static string? Ago(long airedAt, long now) => KaeriModelMethods.AiredAgo(airedAt, now) switch
    {
        AiredAgo.Minutes m => T.TimeAgo(T.TimeMinutes(m.MinutesValue)),
        AiredAgo.Hours h => T.TimeAgo(T.TimeHours(h.HoursValue)),
        _ => null,
    };

    /// <summary>
    /// The status for <paramref name="missing"/> at <paramref name="now"/>. <paramref name="asked"/>
    /// is what the last check the viewer asked for came to, if one did: an empty answer turns "yet"
    /// into "still", and quiet rechecks change nothing. <paramref name="date"/> words a Unix time
    /// as the page's rows do.
    /// </summary>
    public static MissingStatus Status(MissingEpisode missing, long now, CheckOutcome? asked, bool offline, Func<long, string> date)
    {
        var help = missing.Phase == MissingPhase.Quiet ? T.EpisodeCheckAgainHelp(missing.EveryMinutes, missing.ForHours) : null;
        if (missing.Phase == MissingPhase.Undated || missing.AiredAt is not { } aired)
            return new("", T.EpisodeUnsourced, false, true, help);
        var head = Ago(aired, now) is { } ago
            ? T.EpisodeMissingAired(missing.Episode, ago)
            : T.EpisodeMissingAiredOn(missing.Episode, date(aired));
        var answer = asked switch
        {
            CheckOutcome.Missing => T.EpisodeMissingStill,
            CheckOutcome.NoAnswer => T.EpisodeMissingNoAnswer,
            _ => null,
        };
        return missing.Phase switch
        {
            MissingPhase.Quiet => new(head, $"{answer ?? T.EpisodeMissingNotYet} {(offline ? T.EpisodeMissingOffline : T.EpisodeMissingLooksAgain(missing.EveryMinutes))}", false, true, help),
            MissingPhase.LongAgo => new(head, T.EpisodeMissingNone, true, false, null),
            // Past the quiet window nothing looks by itself, offline or not: "still" stands alone.
            _ => new(head, answer ?? T.EpisodeMissingStill, false, true, null),
        };
    }

    /// <summary>What Narrator hears once a check the viewer asked for comes back, and the episode is still missing.</summary>
    public static string? Answered(CheckOutcome outcome) => outcome switch
    {
        CheckOutcome.Missing => T.EpisodeMissingStill,
        CheckOutcome.NoAnswer => T.EpisodeMissingNoAnswer,
        CheckOutcome.Offline => T.StateOfflineLine,
        _ => null,
    };

    /// <summary>
    /// An unsourced row's line: "Aired 25 min ago · Not in your sources yet" within a day of
    /// airing, the date from then on, and "No source has this episode" a week on or undated.
    /// </summary>
    public static string RowLine(EpisodeRow row, long now, Func<long, string> date)
    {
        if (row.AirsAt is not { } aired || aired > now)
            return T.EpisodeUnsourced;
        var yet = KaeriModelMethods.UnsourcedYet(aired, now);
        var when = yet && Ago(aired, now) is { } ago ? T.EpisodeRowAiredAgo(ago) : T.MediaAiredOn(date(aired));
        return T.EpisodeFactsJoin(when, yet ? T.EpisodeUnsourcedYet : T.EpisodeUnsourced);
    }

    /// <summary>
    /// How long until the status or a row's line next reads differently: the next whole minute
    /// since airing, when "25 min ago" turns 26 or the phase moves on.
    /// </summary>
    public static TimeSpan NextTick(long airedAt, long now)
    {
        var into = ((now - airedAt) % 60 + 60) % 60;
        return TimeSpan.FromSeconds(60 - into);
    }

    /// <summary>A source's line in Source for this title, from what the engine says it has: "Episodes 1 to 5. Not 6 yet".</summary>
    public static string Coverage(SourceOption option, long now) => KaeriModelMethods.CoverageLine(option, now) switch
    {
        CoverageLine.Episodes e => T.SourcePickerEpisodes(e.CountValue, e.First, e.Last),
        CoverageLine.NotYet n => T.SourcePickerEpisodesNotYet(n.First, n.Last, Lacks(n.Lacking)),
        CoverageLine.Not n => T.SourcePickerEpisodesNot(n.First, n.Last, Lacks(n.Lacking)),
        CoverageLine.Gap g => T.SourcePickerCoverageMissing(g.Covered, g.Of, Lacks(g.Lacking)),
        CoverageLine.Count c => T.SourcePickerCoverage(c.Covered, c.Of),
        _ => T.SourcePickerCoverage(option.Covered, option.Of),
    };

    /// <summary>The episodes a source lacks, inside its line: "6", "5 or 6", "4 to 6".</summary>
    static string Lacks(Lacking lacking) => lacking switch
    {
        Lacking.One one => one.Episode.ToString(CultureInfo.CurrentCulture),
        Lacking.Pair pair => T.SourcePickerMissingPair(pair.First, pair.Second),
        Lacking.Range range => T.SourcePickerMissingRange(range.First, range.Last),
        _ => "",
    };
}

/// <summary>
/// What the checks the viewer asked for came to, per title and episode, for as long as the app
/// runs: the page that opens again still says "still". Only the UI thread touches it.
/// </summary>
public sealed class MissingChecks
{
    readonly Dictionary<(uint, uint), CheckOutcome> asked = [];

    /// <summary>The last answer to a Check again of this episode, other than offline, if one came.</summary>
    public CheckOutcome? Asked(uint id, uint episode) => asked.TryGetValue((id, episode), out var outcome) ? outcome : null;

    public void Take(Event e)
    {
        // Offline asks nothing, so it says nothing about the sources.
        if (e is Event.EpisodeChecked { V1: { Asked: true } check } && check.Outcome is CheckOutcome.Missing or CheckOutcome.NoAnswer)
            asked[(check.Id, check.Episode)] = check.Outcome;
    }
}

/// <summary>
/// The waiting rule for Check again: nothing for 600 ms after the press, then Checking in the
/// button, held at least 0.3 s once it shows, so it never flickers.
/// </summary>
public static class CheckWait
{
    public static readonly TimeSpan Quiet = TimeSpan.FromMilliseconds(600);
    public static readonly TimeSpan Held = TimeSpan.FromMilliseconds(300);

    /// <summary>How long an answer that came at <paramref name="now"/> waits before it shows: until Checking has stood its 0.3 s.</summary>
    public static TimeSpan Before(DateTimeOffset? checkingShown, DateTimeOffset now) =>
        checkingShown is { } shown && shown + Held > now ? shown + Held - now : TimeSpan.Zero;
}
