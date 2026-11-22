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

/// <summary>The episode's releases across every source, as <c>LoadSources</c> answered, in the order the sources gave them.</summary>
public sealed record PanelReleases(uint Id, string Episode, SourceCandidate[]? Candidates)
{
    /// <summary>One source's releases, in the order it gave them.</summary>
    public SourceCandidate[] Of(string providerId) => [.. (Candidates ?? []).Where(c => c.ProviderId == providerId)];
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
}

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
        null => T.SourcePickerCoverage(option.Covered, option.Of),
        _ => SourcePicker.Detail(option),
    };

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

    /// <summary>The switch picked in the panel, from the pick until it lands, or its failure until the next pick.</summary>
    public PanelSwitch? Switch { get; private set; }

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
        PlayingRelease = null;
    }

    /// <summary>
    /// The releases to ask for, or null when the ones asked for are this episode's already, unless
    /// <paramref name="again"/>.
    /// </summary>
    public (uint Id, string Episode)? AskReleases(bool again = false)
    {
        if (Episode is not { } e)
            return null;
        if (!again && Releases is { } r && r.Id == e.Id && r.Episode == e.Episode)
            return null;
        Releases = new PanelReleases(e.Id, e.Episode, null);
        return e;
    }

    /// <summary>The engine's listing: kept when it is for the episode asked for.</summary>
    public bool Listed(uint id, string episode, SourceCandidate[] candidates)
    {
        if (Releases is not { } r || r.Id != id || r.Episode != episode)
            return false;
        Releases = r with { Candidates = candidates };
        return true;
    }

    /// <summary>A release or stream picked: the row says so from now.</summary>
    public void Picked(SwitchTarget target, long now, SourceCandidate? candidate = null) =>
        Switch = new PanelSwitch(target, new PanelPhase.Asked(), now, candidate);

    /// <summary>What the engine's word on a switch asks of the player.</summary>
    public enum Effect { None, Hold, Release, ReleaseAndNotice }

    /// <summary>What the engine says of a switch.</summary>
    public Effect Switched(SourceSwitch state, long now)
    {
        switch (state)
        {
            case SourceSwitch.Finding f:
                Track(SwitchTarget.Of(f.To), new PanelPhase.Finding(), now);
                return Effect.None;
            case SourceSwitch.Loading l:
                Track(SwitchTarget.Of(l.To), new PanelPhase.Loading(), now);
                Holding = true;
                HoldAt = l.At;
                return Effect.Hold;
            case SourceSwitch.Landed landed:
            {
                var target = SwitchTarget.Of(landed.To);
                PlayingSource = landed.To.Name;
                if (landed.To.Release is { } release)
                    PlayingRelease = Releases?.Candidates?.FirstOrDefault(c => c.ProviderId == landed.To.ProviderId && c.Id == release)
                        ?? Switch?.Candidate;
                if (Switch?.On(target) != false)
                    Switch = null;
                Landings++;
                Holding = false;
                HoldAt = null;
                return !Open && landed.To.Release is not null ? Effect.ReleaseAndNotice : Effect.Release;
            }
            case SourceSwitch.Failed failed:
                Track(SwitchTarget.Of(failed.To), new PanelPhase.Failed(failed.Text), now);
                Holding = false;
                HoldAt = null;
                return Effect.Release;
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
    void Track(SwitchTarget target, PanelPhase phase, long now) =>
        Switch = Switch is { } known && known.On(target) ? known with { Phase = phase } : new PanelSwitch(target, phase, now);
}
