// What the player's Episodes and Sources panel draws from besides the engine's menus: the
// episode's releases across every source, a switch to one of them as the engine says it goes,
// and the words its rows use (player panel board). After the Apple and Android PanelState.

using System.Globalization;
using Kaeri.Engine.Core;
using Kaeri.Engine.Model;

namespace Kaeri.App;

/// <summary>The panel's two tabs. It opens on Episodes.</summary>
public enum PanelTab { Episodes, Sources }

/// <summary>What a switch of the playing episode goes to, by the ids the panel's rows carry.</summary>
/// <param name="ProviderId">The source, as <c>SourceCandidate.ProviderId</c> names it; empty for a stream of the source playing.</param>
/// <param name="Name">The name the source goes by.</param>
/// <param name="Release">The release, by its <c>SourceCandidate.Id</c>; null for a stream.</param>
/// <param name="Stream">The stream, by its <c>MenuSource.Id</c>; null for a release.</param>
public sealed record SwitchTarget(string ProviderId, string Name, string? Release = null, string? Stream = null)
{
    public static SwitchTarget Of(SwitchTo to) => new(to.ProviderId, to.Name, to.Release, to.Stream);

    public static SwitchTarget Of(SourceCandidate release) => new(release.ProviderId, ReleasePicker.SourceName(release), release.Id);

    /// <summary>The same row: a stream by its id, a release by its source and id.</summary>
    public bool Same(SwitchTarget other) =>
        Stream is not null ? Stream == other.Stream : other.Stream is null && Release == other.Release && ProviderId == other.ProviderId;
}

/// <summary>Where a switch picked in the panel stands.</summary>
public abstract record PanelPhase
{
    /// <summary>Sent, and the engine has said nothing yet.</summary>
    public sealed record Asked : PanelPhase;

    /// <summary>The release is being found; the episode plays on.</summary>
    public sealed record Finding : PanelPhase;

    /// <summary>Found and loading at the playhead; the picture is held.</summary>
    public sealed record Loading : PanelPhase;

    /// <summary>It did not start, and the episode plays on from what played before: the engine's line.</summary>
    public sealed record Failed(Msg Text) : PanelPhase;
}

/// <summary>
/// A switch picked in the panel, from the click until it lands or fails. The row it was picked on
/// shows it: nothing for 600 ms, then a spinner, then "Starting · 3 s"; a failure stays on the
/// row until the next pick, with Try again.
/// </summary>
/// <param name="Since">When it was picked, in milliseconds on the panel's clock: what the row's wait counts from.</param>
/// <param name="Candidate">The release it was picked from, for Try again.</param>
public sealed record PanelSwitch(SwitchTarget Target, PanelPhase Phase, long Since, SourceCandidate? Candidate = null)
{
    /// <summary>Waiting is silent this long: most switches land inside it.</summary>
    public const long Silent = 600;

    /// <summary>"Starting · 3 s" from here.</summary>
    public const long CountingFrom = 3_000;

    /// <summary>Whether <paramref name="target"/> is the row this switch was picked on.</summary>
    public bool On(SwitchTarget target) => Target.Same(target);

    public Msg? Failed => (Phase as PanelPhase.Failed)?.Text;

    /// <summary>Whether the spinner shows at <paramref name="now"/>: past the silent 600 ms, and not failed.</summary>
    public bool Spinning(long now) => Failed is null && now - Since >= Silent;

    /// <summary>The whole seconds waited at <paramref name="now"/>, once "Starting · 3 s" takes the detail line's place; else null.</summary>
    public long? Counting(long now) => Failed is null && now - Since >= CountingFrom ? (now - Since) / 1000 : null;
}

/// <summary>
/// A switch to the other version, subtitled or dubbed, from the pick until it lands, or its
/// failure until the next pick. The picked half of the switch shows it: nothing for 600 ms, then
/// a spinner, then "Finding episode 7 dubbed · 3 s" under the switch; the picture is held while
/// it loads.
/// </summary>
/// <param name="Since">When it was picked, in milliseconds on the panel's clock.</param>
public sealed record VersionSwitch(Translation To, PanelPhase Phase, long Since)
{
    public Msg? Failed => (Phase as PanelPhase.Failed)?.Text;

    /// <summary>Whether the half's spinner shows at <paramref name="now"/>: past the silent 600 ms, and not failed.</summary>
    public bool Spinning(long now) => Failed is null && now - Since >= PanelSwitch.Silent;

    /// <summary>"Finding episode 7 dubbed · 3 s" at <paramref name="now"/>, from 3 s; else null.</summary>
    public string? Note(string episode, long now)
    {
        if (Failed is not null || now - Since < PanelSwitch.CountingFrom)
            return null;
        var seconds = (now - Since) / 1000;
        return To == Translation.Dub
            ? T.PlayerFindingVersionSeconds(episode, seconds)
            : T.PlayerFindingVersionSubbedSeconds(episode, seconds);
    }
}

/// <summary>
/// The episode's releases across every source, as <c>LoadSources</c> answered, in the order the
/// sources gave them; <paramref name="Rows"/> is each source's row as the engine last said it
/// while it asks, and after. <paramref name="Ended"/>: the listing ended before any source was
/// asked (none set up, the title unreadable, the episode not out), so no row is asked any more.
/// </summary>
public sealed record PanelReleases(uint Id, string Episode, SourceCandidate[]? Candidates, SourceListing[]? Rows = null, bool Ended = false)
{
    /// <summary>One source's releases, in the order it gave them.</summary>
    public SourceCandidate[] Of(string providerId) => [.. (Candidates ?? []).Where(c => c.ProviderId == providerId)];

    /// <summary>
    /// Release <paramref name="id"/> of <paramref name="providerId"/>, from the answer or, while
    /// the tab still loads, from the row that listed it.
    /// </summary>
    public SourceCandidate? Release(string providerId, string id) =>
        (Candidates ?? []).Concat((Rows ?? []).SelectMany(r => r.State is ListingState.Listed l ? l.Releases : []))
            .FirstOrDefault(c => c.ProviderId == providerId && c.Id == id);
}

/// <summary>A revealed row's buttons.</summary>
public enum EpisodeAction { Play, Resume, FromBeginning, FromBeginningPlaying, MarkWatched }

/// <summary>One row under Other sources.</summary>
public abstract record SourceGroup(string ProviderId, string Name, SourceOption? Option)
{
    /// <summary>Two releases or more: the source opens to list them.</summary>
    public sealed record Releases(string ProviderId, string Name, SourceOption? Option, SourceCandidate[] List) : SourceGroup(ProviderId, Name, Option);

    /// <summary>One release: the source's row plays it.</summary>
    public sealed record One(string ProviderId, string Name, SourceOption? Option, SourceCandidate Release) : SourceGroup(ProviderId, Name, Option);

    /// <summary>Resting, or nothing for this episode: said, dimmed, and nothing to pick.</summary>
    public sealed record Dim(string ProviderId, string Name, SourceOption? Option) : SourceGroup(ProviderId, Name, Option);

    /// <summary>Its releases have not come yet.</summary>
    public sealed record Waiting(string ProviderId, string Name, SourceOption? Option) : SourceGroup(ProviderId, Name, Option);

    /// <summary>Being asked: nothing to pick until it answers.</summary>
    public sealed record Asking(string ProviderId, string Name, SourceOption? Option) : SourceGroup(ProviderId, Name, Option);

    /// <summary>Answered with nothing to offer, or held back and not asked: said, dimmed, and nothing to pick.</summary>
    public sealed record Missed(string ProviderId, string Name, SourceOption? Option, ListingMiss Miss) : SourceGroup(ProviderId, Name, Option);

    /// <summary>Switched off, so never asked.</summary>
    public sealed record Off(string ProviderId, string Name, SourceOption? Option) : SourceGroup(ProviderId, Name, Option);
}

/// <summary>How a row's detail line stands while its source is asked.</summary>
public enum SourceLineKind
{
    /// <summary>The silent 600 ms after the tab asked: the line is held empty, one line high.</summary>
    Empty,

    /// <summary>"Asking", with a spinner.</summary>
    Asking,

    /// <summary>"Asking · 4 s", with a spinner, from 3 s.</summary>
    Counting,

    /// <summary>The source's answer: its coverage line, or why it has nothing.</summary>
    Said,
}

/// <summary>A row's detail line under Other sources: the words, whether the check slot spins, and whether the title goes to haze.</summary>
public readonly record struct SourceLine(SourceLineKind Kind, string Words, bool Spinner, bool Haze);

/// <summary>The words and choices the panel's rows share.</summary>
public static class PanelWords
{
    /// <summary>"E7", or the number as it is when it is not whole ("7.5", "OVA 2").</summary>
    public static string Number(string number) =>
        long.TryParse(number, NumberStyles.Integer, CultureInfo.InvariantCulture, out var n) ? T.EpisodeNumberShort(n) : number;

    /// <summary>"Episode 11", for a row with no title of its own.</summary>
    public static string Plain(string number) =>
        long.TryParse(number, NumberStyles.Integer, CultureInfo.InvariantCulture, out var n) ? T.EpisodeNumber(n) : T.EpisodeNumberText(number);

    /// <summary>The row's own title, when it has one.</summary>
    public static string? Title(EpisodeRow row) => row.Title is { } t && !string.IsNullOrWhiteSpace(t) ? t : null;

    /// <summary>The row's whole title line: "E7 The Lantern Fair", or "Episode 7".</summary>
    public static string Heading(EpisodeRow row) => Title(row) is { } t ? $"{Number(row.Number)} {t}" : Plain(row.Number);

    /// <summary>Whether the episode has not aired yet at <paramref name="now"/>, in Unix seconds.</summary>
    public static bool NotOut(EpisodeRow row, long now) => row.AirsAt is { } at && at > now;

    /// <summary>Whether the table is one only AniList lists: no source has the show.</summary>
    public static bool MetadataOnly(IReadOnlyList<EpisodeRow> rows) => rows.Any(r => r.MetadataOnly);

    static long Minutes(double secs) => (long)Math.Max(1, Math.Round(secs / 60));

    /// <summary>
    /// The row's one detail line, from the table's own fields: "Playing · 11 min left" on the
    /// playing row, else the kind, then airs, unsourced, watched, time left or length, and the
    /// source that fills it in. <paramref name="now"/> is Unix seconds.
    /// </summary>
    public static string Detail(EpisodeRow row, bool current, bool metadataOnly, double position, double? duration, long now, Func<long, string>? airs = null)
    {
        if (current)
        {
            var length = duration is { } d && d > 0 ? d : row.DurationSecs ?? 0;
            return T.EpisodeRowPlaying(T.TimeMinutesLeft(Minutes(length - position)));
        }
        airs ??= at => AirsWords(at, now);
        var parts = new List<string>();
        if (row.Kind == EpisodeKind.Filler)
            parts.Add(T.EpisodeFiller);
        else if (row.Kind == EpisodeKind.Mixed)
            parts.Add(T.EpisodeRowMixed);
        if (row.AirsAt is { } at && at > now)
        {
            parts.Add(T.EpisodeRowAirs(airs(at)));
            if (row.DelayedFrom is { } was)
                parts.Add(T.EpisodeRowDelayed(airs(was)));
            return string.Join(" · ", parts);
        }
        if (row.Unsourced && !metadataOnly)
        {
            parts.Add(T.EpisodeUnsourced);
            return string.Join(" · ", parts);
        }
        if (row.Completed)
            parts.Add(T.EpisodeWatched);
        else if (row.Watched > 0 && !metadataOnly && row.DurationSecs is { } secs)
            parts.Add(T.TimeMinutesLeft(Minutes(secs * (1 - row.Watched))));
        else if (row.DurationSecs is { } all && all > 0)
            parts.Add(T.EpisodeRowLength(Minutes(all)));
        if (!metadataOnly && row.Source is { Length: > 0 } source)
            parts.Add(T.EpisodeRowFrom(source));
        return string.Join(" · ", parts);
    }

    /// <summary>The silk line on the still: the playhead on the playing row, the history's on a started one.</summary>
    public static double? Progress(EpisodeRow row, bool current, bool metadataOnly, double position, double? duration)
    {
        if (metadataOnly)
            return null;
        if (current)
        {
            double? length = duration is { } d && d > 0 ? d : row.DurationSecs;
            return length is { } l && l > 0 ? Math.Clamp(position / l, 0, 1) : null;
        }
        return !row.Completed && row.Watched > 0 ? Math.Clamp(row.Watched, 0, 1) : null;
    }

    /// <summary>
    /// "1080p · 1.4 GiB · label · Dubbed · Automatic picks this", from the release's own fields
    /// only. Each part keeps its words on one line.
    /// </summary>
    public static string Release(SourceCandidate release)
    {
        var parts = new List<string>();
        if (release.Quality is { } q)
            parts.Add($"{q}p");
        if (release.Size is { Length: > 0 } size)
            parts.Add(size);
        if (release.Label is { } label && !string.IsNullOrWhiteSpace(label))
            parts.Add(label);
        if (release.Dubbed)
            parts.Add(T.AudioDubbed);
        else if (release.DualAudio)
            parts.Add(T.PlayerPanelDualAudio);
        if (release.AutoPick)
            parts.Add(T.PlayerReleaseAutomatic);
        return string.Join(" · ", parts.Select(p => p.Replace(' ', ' ')));
    }

    /// <summary>A source's line: why it is resting, or how much of the title it has.</summary>
    public static string Source(SourceOption option) => option.Note switch
    {
        null => Coverage(option),
        _ => SourcePicker.Detail(option),
    };

    /// <summary>How much of the title a source has, whatever its note: the line of a source that listed releases now.</summary>
    public static string Coverage(SourceOption option) => T.SourcePickerCoverage(option.Covered, option.Of);

    /// <summary>
    /// When a broadcast airs: "today at 16:30", "tomorrow at 16:30", "on Thursday at 16:30" within
    /// six days, and "on 29 Oct at 16:30" later. Both are Unix seconds.
    /// </summary>
    public static string AirsWords(long at, long now, TimeZoneInfo? zone = null, CultureInfo? culture = null)
    {
        zone ??= TimeZoneInfo.Local;
        culture ??= CultureInfo.CurrentCulture;
        var when = TimeZoneInfo.ConvertTime(DateTimeOffset.FromUnixTimeSeconds(at), zone);
        var today = TimeZoneInfo.ConvertTime(DateTimeOffset.FromUnixTimeSeconds(now), zone);
        var time = when.ToString("t", culture);
        var days = (when.Date - today.Date).Days;
        return days switch
        {
            0 => T.CauseAirsToday(time),
            1 => T.CauseAirsTomorrow(time),
            >= 2 and <= 6 => T.CauseAirsOnDate(culture.DateTimeFormat.GetDayName(when.DayOfWeek), time),
            _ => T.CauseAirsOnDate(when.ToString("d MMM", culture), time),
        };
    }

    /// <summary>What a revealed row can do, as the board has it, in order; the first is the amber one when it plays.</summary>
    public static EpisodeAction[] Actions(EpisodeRow row, bool current, bool metadataOnly, bool notOut) => true switch
    {
        _ when metadataOnly => row.Completed ? [] : [EpisodeAction.MarkWatched],
        _ when notOut => [],
        _ when row.Unsourced => row.Completed ? [] : [EpisodeAction.MarkWatched],
        _ when current => [EpisodeAction.FromBeginningPlaying],
        _ when row.Watched > 0 && !row.Completed => [EpisodeAction.Resume, EpisodeAction.FromBeginning],
        _ when row.Completed => [EpisodeAction.Play],
        _ => [EpisodeAction.Play, EpisodeAction.MarkWatched],
    };

    /// <summary>Whether a button is the amber one.</summary>
    public static bool Primary(EpisodeAction action) => action is EpisodeAction.Play or EpisodeAction.Resume;

    public static string Title(EpisodeAction action) => action switch
    {
        EpisodeAction.Play => T.ActionPlay,
        EpisodeAction.Resume => T.ActionResume,
        EpisodeAction.MarkWatched => T.EpisodeMarkWatched,
        _ => T.EpisodePlayFromBeginning,
    };

    /// <summary>
    /// Every other source, in the order set in Sources; before their lines come, the releases' own
    /// sources. <paramref name="skip"/> is the one playing, which has its own place above.
    /// </summary>
    public static SourceGroup[] Others(SourceOption[]? options, PanelReleases? releases, string? skip)
    {
        if (releases?.Rows is { } rows)
            return [.. rows.Where(r => r.ProviderId != skip).Select(r => Row(r, options?.FirstOrDefault(o => o.ProviderId == r.ProviderId), releases.Ended))];
        var known = releases?.Candidates is not null;
        IEnumerable<(string Provider, string Name, SourceOption? Option)> groups = options is not null
            ? options.Where(o => o.ProviderId != skip).Select(o => (o.ProviderId, o.Name, (SourceOption?)o))
            : (releases?.Candidates ?? []).Where(c => c.ProviderId != skip).DistinctBy(c => c.ProviderId)
                .Select(c => (c.ProviderId, ReleasePicker.SourceName(c), (SourceOption?)null));
        return
        [
            .. groups.Select(g =>
            {
                var list = releases?.Of(g.Provider) ?? [];
                var resting = g.Option?.Note is not null;
                return (SourceGroup)(true switch
                {
                    _ when list.Length > 1 && !resting => new SourceGroup.Releases(g.Provider, g.Name, g.Option, list),
                    _ when list.Length == 1 && !resting => new SourceGroup.One(g.Provider, g.Name, g.Option, list[0]),
                    _ when resting || known => new SourceGroup.Dim(g.Provider, g.Name, g.Option),
                    _ => new SourceGroup.Waiting(g.Provider, g.Name, g.Option),
                });
            }),
        ];
    }

    /// <summary>One source's row as the engine said it while it asks, and after.</summary>
    static SourceGroup Row(SourceListing row, SourceOption? option, bool ended) => row.State switch
    {
        ListingState.Asking when ended => new SourceGroup.Dim(row.ProviderId, row.Name, option),
        ListingState.Listed { Releases.Length: > 1 } listed => new SourceGroup.Releases(row.ProviderId, row.Name, option, listed.Releases),
        ListingState.Listed { Releases: [var one] } => new SourceGroup.One(row.ProviderId, row.Name, option, one),
        ListingState.Listed => new SourceGroup.Dim(row.ProviderId, row.Name, option),
        ListingState.Missed missed => new SourceGroup.Missed(row.ProviderId, row.Name, option, missed.Miss),
        ListingState.Off => new SourceGroup.Off(row.ProviderId, row.Name, option),
        _ => new SourceGroup.Asking(row.ProviderId, row.Name, option),
    };

    /// <summary>
    /// Why a source has nothing for the episode, as the starting screen's rows say it: "Didn't
    /// answer", "Took too long", "Doesn't have episode 7", "Couldn't match this title", or "Not
    /// asked" with the reason.
    /// </summary>
    public static string Miss(ListingMiss miss, string episode) => miss switch
    {
        ListingMiss.NoMatch => T.PlayerStartRowNoMatch,
        ListingMiss.Ambiguous => T.SourceNoteAmbiguous,
        ListingMiss.NoEpisode => T.PlayerStartRowNoEpisode(episode),
        ListingMiss.TimedOut => T.PlayerStartRowTimedOut,
        ListingMiss.HeldBack held => StartScreen.HeldNote(held.Why),
        _ => T.PlayerStartRowFailed,
    };

    /// <summary>Whether the dub does not reach this episode yet, so Dubbed cannot be picked.</summary>
    public static bool PastDub(DubExtent? dub, string? episode) =>
        dub is { Complete: false } d && episode is not null
        && double.TryParse(episode, NumberStyles.Float, CultureInfo.InvariantCulture, out var n) && n > d.Through;
}

/// <summary>
/// The panel's state beside the engine's menus: open or not, its tab, the episode's releases,
/// the switch picked and where it stands, and what plays now. Only the UI thread touches it.
/// </summary>
public sealed class PanelState
{
    /// <summary>How long the held picture stays over the new one once a switch lands: the crossfade's length.</summary>
    public const int HeldFade = 400;

    /// <summary>
    /// How long a switch's load waits for the picture to be copied before it goes ahead without one.
    /// </summary>
    public const int HoldWait = 600;

    public bool Open { get; set; }
    public PanelTab Tab { get; set; } = PanelTab.Episodes;

    /// <summary>The episode playing, as the panel lists its releases for.</summary>
    public (uint Id, string Episode)? Episode { get; private set; }

    /// <summary>The episode's releases across every source, as the panel last asked for them.</summary>
    public PanelReleases? Releases { get; private set; }

    /// <summary>When the releases were last asked for, in milliseconds on the panel's clock: the silent 600 ms count from here.</summary>
    public long AskedAt { get; private set; }

    /// <summary>When each source's row first said it is asked, by provider id: its seconds count from here.</summary>
    readonly Dictionary<string, long> askingSince = [];

    /// <summary>The switch picked in the panel, from the pick until it lands, or its failure until the next pick.</summary>
    public PanelSwitch? Switch { get; private set; }

    /// <summary>The switch to the other version, from the pick until it lands, or its failure until the next pick.</summary>
    public VersionSwitch? Version { get; private set; }

    /// <summary>
    /// The source the episode plays from: what the start found, then what each switch landed on.
    /// Another source found, as when the episode starts again for a dub, lets go of the release
    /// picked on the last one, so Playing now never names one source with another's release.
    /// </summary>
    public string? PlayingSource
    {
        get => playingSource;
        set
        {
            if (value != playingSource)
                PlayingRelease = null;
            playingSource = value;
        }
    }

    string? playingSource;

    /// <summary>The release playing, when one was picked in the panel and landed.</summary>
    public SourceCandidate? PlayingRelease { get; private set; }

    /// <summary>Bumped as a switch lands: the Sources tab scrolls back to Playing now.</summary>
    public int Landings { get; private set; }

    /// <summary>A switch is loading, from its Loading until Landed or Failed: the seek line stays at <see cref="HoldAt"/>.</summary>
    public bool Holding { get; private set; }

    public double? HoldAt { get; private set; }

    /// <summary>Another episode plays: its releases and picks are its own.</summary>
    public void Playing(uint id, string episode)
    {
        if (Episode is { } e && e.Id == id && e.Episode == episode)
            return;
        Episode = (id, episode);
        Releases = null;
        Switch = null;
        Version = null;
        PlayingRelease = null;
    }

    /// <summary>
    /// The episode started again, as it does in the other version when nothing played yet: no
    /// switch of it is on its way.
    /// </summary>
    public void Restarted() => Version = null;

    /// <summary>
    /// The other version picked: false while one is still on its way, as the engine finds a single
    /// version at a time, and for the version playing.
    /// </summary>
    public bool PickedVersion(Translation to, Translation playing, long now)
    {
        if (Version is { Failed: null })
            return false;
        if (to == playing)
        {
            Version = null;
            return false;
        }
        Version = new VersionSwitch(to, new PanelPhase.Asked(), now);
        return true;
    }

    /// <summary>
    /// The releases to ask for, or null when the ones asked for are this episode's already, unless
    /// <paramref name="again"/>.
    /// </summary>
    public (uint Id, string Episode)? AskReleases(bool again = false, long now = 0)
    {
        if (Episode is not { } e)
            return null;
        // A listing that ended before asking any source is asked again on the next look.
        if (!again && Releases is { } r && r.Id == e.Id && r.Episode == e.Episode && !r.Ended)
            return null;
        Releases = new PanelReleases(e.Id, e.Episode, null);
        AskedAt = now;
        askingSince.Clear();
        return e;
    }

    /// <summary>
    /// The engine's rows while it asks: kept, whole, when they are for the episode asked for. A row
    /// that says it is asked for the first time starts its seconds at <paramref name="now"/>.
    /// </summary>
    public bool Progressed(uint id, string episode, SourceListing[] rows, long now)
    {
        if (Releases is not { } r || r.Id != id || r.Episode != episode)
            return false;
        foreach (var row in rows)
        {
            if (row.State is ListingState.Asking)
                askingSince.TryAdd(row.ProviderId, now);
        }
        Releases = r with { Rows = rows };
        return true;
    }

    /// <summary>
    /// What a row under Other sources says at <paramref name="now"/>: nothing for the 600 ms after
    /// the tab asked, then "Asking" and a spinner, then "Asking · 4 s" from 3 s after the row first
    /// said it is asked; once it answered, its coverage line or why it has nothing.
    /// </summary>
    public SourceLine Line(SourceGroup group, long now)
    {
        // A source that listed releases now says how much it has, never an older resting or failed note.
        var coverage = group.Option is { } o
            ? group is SourceGroup.One or SourceGroup.Releases ? PanelWords.Coverage(o) : PanelWords.Source(o)
            : null;
        switch (group)
        {
            case SourceGroup.Asking or SourceGroup.Waiting:
            {
                if (now - AskedAt < PanelSwitch.Silent)
                    return new SourceLine(SourceLineKind.Empty, "", false, false);
                var since = askingSince.TryGetValue(group.ProviderId, out var at) ? at : AskedAt;
                return now - since >= PanelSwitch.CountingFrom
                    ? new SourceLine(SourceLineKind.Counting, T.PlayerPanelAsking((now - since) / 1000), true, false)
                    : new SourceLine(SourceLineKind.Asking, T.PlayerStartRowAsking, true, false);
            }
            case SourceGroup.Missed missed:
                return new SourceLine(SourceLineKind.Said, PanelWords.Miss(missed.Miss, Episode?.Episode ?? Releases?.Episode ?? ""), false, true);
            case SourceGroup.Off:
                return new SourceLine(SourceLineKind.Said, T.SourceNoteOff, false, true);
            case SourceGroup.Dim:
                return new SourceLine(SourceLineKind.Said, coverage ?? "", false, true);
            case SourceGroup.One one:
                return new SourceLine(SourceLineKind.Said, coverage ?? PanelWords.Release(one.Release), false, false);
            default:
                return new SourceLine(SourceLineKind.Said, coverage ?? "", false, false);
        }
    }

    /// <summary>The engine's listing: kept when it is for the episode asked for.</summary>
    public bool Listed(uint id, string episode, SourceCandidate[] candidates)
    {
        if (Releases is not { } r || r.Id != id || r.Episode != episode)
            return false;
        Releases = r with { Candidates = candidates };
        return true;
    }

    /// <summary>
    /// The engine ended the listing before asking any source: kept as ended, with no releases,
    /// when it is for the episode asked for, so no row says it is asked any more.
    /// </summary>
    public bool Failed(uint id, string episode)
    {
        if (Releases is not { } r || r.Id != id || r.Episode != episode)
            return false;
        Releases = r with { Candidates = r.Candidates ?? [], Ended = true };
        return true;
    }

    /// <summary>A release or stream picked: the row says so from now.</summary>
    public void Picked(SwitchTarget target, long now, SourceCandidate? candidate = null) =>
        Switch = new PanelSwitch(target, new PanelPhase.Asked(), now, candidate);

    /// <summary>
    /// What the engine's word on a switch asks of the player. <c>ReleaseAndRelist</c> lands the
    /// other version, whose releases are the ones to list now; <c>ReleaseAndAlert</c> lets go of
    /// a version that failed with the panel closed, which says so in its place.
    /// </summary>
    public enum Effect { None, Hold, Release, ReleaseAndNotice, ReleaseAndRelist, ReleaseAndAlert }

    /// <summary>What the engine says of a switch.</summary>
    public Effect Switched(SourceSwitch state, long now)
    {
        switch (state)
        {
            case SourceSwitch.Finding f:
                Track(f.To, new PanelPhase.Finding(), now);
                return Effect.None;
            case SourceSwitch.Loading l:
                Track(l.To, new PanelPhase.Loading(), now);
                Holding = true;
                HoldAt = l.At;
                return Effect.Hold;
            case SourceSwitch.Landed landed:
            {
                var target = SwitchTarget.Of(landed.To);
                PlayingSource = landed.To.Name;
                if (landed.To.Release is { } release)
                    // While the tab loads the row that listed it has it; else the pick, if it is this one.
                    PlayingRelease = Releases?.Release(landed.To.ProviderId, release)
                        ?? (Switch?.Candidate is { } picked && picked.ProviderId == landed.To.ProviderId && picked.Id == release ? picked : null);
                else if (landed.To.Stream is null || landed.To.ProviderId != PlayingRelease?.ProviderId)
                    // The other version, or a stream of another source the engine moved to: the
                    // release picked before is not what plays.
                    PlayingRelease = null;
                Landings++;
                Holding = false;
                HoldAt = null;
                if (landed.To.Translation is not null)
                {
                    Version = null;
                    return Releases is not null ? Effect.ReleaseAndRelist : Effect.Release;
                }
                if (Switch?.On(target) != false)
                    Switch = null;
                return !Open && landed.To.Release is not null ? Effect.ReleaseAndNotice : Effect.Release;
            }
            case SourceSwitch.Failed failed:
                Track(failed.To, new PanelPhase.Failed(failed.Text), now);
                Holding = false;
                HoldAt = null;
                return failed.To.Translation is not null && !Open ? Effect.ReleaseAndAlert : Effect.Release;
        }
        return Effect.None;
    }

    /// <summary>The hold ends without a word on it: the player closed, or another episode started.</summary>
    public void LetGo()
    {
        Holding = false;
        HoldAt = null;
    }

    /// <summary>
    /// The row's switch moves on to <paramref name="phase"/>; a switch the panel did not pick (a
    /// stream from the old menu, a pick from a remote) gets a row of its own.
    /// </summary>
    void Track(SwitchTo to, PanelPhase phase, long now)
    {
        if (to.Translation is { } version)
        {
            // The other version: one picked elsewhere (a remote) is tracked from now.
            Version = Version is { } pending && pending.To == version ? pending with { Phase = phase } : new VersionSwitch(version, phase, now);
            return;
        }
        var target = SwitchTarget.Of(to);
        Switch = Switch is { } known && known.On(target) ? known with { Phase = phase } : new PanelSwitch(target, phase, now);
    }
}
