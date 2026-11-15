using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;

namespace Kaeri.App;

/// <summary>How the cast's status line reads, with its dot or spinner.</summary>
public enum CastLine
{
    Playing,
    Paused,
    /// <summary>The TV loads the episode: the only dimmed state.</summary>
    Starting,
    /// <summary>The session ended with the engine's reason.</summary>
    Failed,
    /// <summary>Joining a cast left running at the last launch, until the TV answers.</summary>
    Rejoining,
}

/// <summary>
/// One frame of the mini and the expanded controller, from the session view as the engine gave
/// it. <see cref="Dim"/> only while the TV loads; a wait mid-episode holds the playhead and says
/// so under the seek line after 4 s, with nothing dimmed.
/// </summary>
public sealed record CastDrawn(
    CastLine Line,
    string Status,
    bool Dim,
    uint? Id,
    string Title,
    string EpisodeLine,
    string? Image,
    double Position,
    double? Duration,
    string Elapsed,
    string Remaining,
    string Total,
    bool Paused,
    string? Waiting,
    string? Reason);

/// <summary>
/// Draws the cast from the engine's session view without amplifying it: widgets change only for
/// what the view changed. An echo of a pause, a status without the media's length, or the TV
/// waiting for the stream leaves the times, the episode line and the dim as they were. A dragged
/// seek and a pressed play or pause hold what was asked until a view newer than the press comes.
/// </summary>
public sealed class CastPainter(Func<DateTimeOffset>? clock = null)
{
    readonly Func<DateTimeOffset> clock = clock ?? (() => DateTimeOffset.UtcNow);
    DateTimeOffset heardAt;
    ulong rev;
    (uint? Id, string? Episode, double Length)? kept;
    DateTimeOffset? waitingSince;
    (double To, ulong Rev)? seekHeld;
    (bool Paused, ulong Rev)? pauseHeld;

    /// <summary>The session as the engine last said it.</summary>
    public FfiCastSessionView? View { get; private set; }

    /// <summary>The change count: the engine's once it says one, else the views taken.</summary>
    public ulong Rev => rev;

    /// <summary>The wait after which the line under the seek line says the TV waits for the stream.</summary>
    public static readonly TimeSpan WaitingAfter = TimeSpan.FromSeconds(4);

    public void Take(FfiCastSessionView next)
    {
        var now = clock();
        var was = View?.Item;
        var item = next.Item;
        rev = CastApi.Rev(next) ?? rev + 1;
        if (seekHeld is { } seek && rev > seek.Rev)
            seekHeld = null;
        if (pauseHeld is { } pause && rev > pause.Rev)
            pauseHeld = null;
        // The playhead runs on from where the TV last said, unless it said something new.
        if (item is null || was is null || !Same(was, item) || was.Position != item.Position || was.Paused != item.Paused
            || was.Loading != item.Loading || (CastApi.Waiting(was) is null) != (CastApi.Waiting(item) is null))
            heardAt = now;
        if (item is { Duration: { } length } && length > 0)
            kept = (item.Id, item.Episode, length);
        else if (item is null || kept is not { } k || k.Id != item.Id || k.Episode != item.Episode)
            kept = null;
        if (item is not null && CastApi.Waiting(item) is { } seconds)
            waitingSince ??= now - TimeSpan.FromSeconds(seconds);
        else
            waitingSince = null;
        View = next;
    }

    static bool Same(FfiCastItem a, FfiCastItem b) => a.Id == b.Id && a.Episode == b.Episode && a.Title == b.Title;

    /// <summary>A seek dragged or sent: the playhead shows it until a newer view comes.</summary>
    public void HoldSeek(double to) => seekHeld = (to, rev);

    /// <summary>Play or pause pressed: the glyph shows the press until a newer view comes.</summary>
    public void HoldPause(bool paused) => pauseHeld = (paused, rev);

    /// <summary>Whether the TV shows it paused, or the press says it will be.</summary>
    public bool Paused => pauseHeld?.Paused ?? View?.Item?.Paused ?? false;

    /// <summary>The length of what plays: as said, else as last said for the same episode.</summary>
    public double? Length => View?.Item is { } item ? item.Duration is { } d && d > 0 ? d : kept?.Length : null;

    /// <summary>Where the TV's playhead is now, moved on from what it last said while it plays.</summary>
    public double? Playhead(DateTimeOffset? at = null)
    {
        if (seekHeld is { } seek)
            return seek.To;
        if (View?.Item is not { } item)
            return null;
        var still = Paused || item.Loading || CastApi.Waiting(item) is not null || View.State != FfiCastSessionState.Playing;
        var position = still ? item.Position : item.Position + Math.Max(0, ((at ?? clock()) - heardAt).TotalSeconds);
        return Length is { } d ? Math.Min(position, d) : position;
    }

    /// <summary>The frame for the TV called <paramref name="name"/>; null while nothing is loaded and nothing is joined again.</summary>
    public CastDrawn? Draw(string name, bool rejoining = false, DateTimeOffset? at = null)
    {
        var view = View;
        var item = view?.Item;
        var failed = view?.State == FfiCastSessionState.Error;
        if (item is null && !rejoining && !failed)
            return null;
        var line = rejoining && view?.State is not FfiCastSessionState.Playing ? CastLine.Rejoining
            : failed ? CastLine.Failed
            : item is null || item.Loading || view?.State == FfiCastSessionState.Connecting ? CastLine.Starting
            : Paused ? CastLine.Paused
            : CastLine.Playing;
        var status = line switch
        {
            CastLine.Rejoining => T.TvStatusReconnecting(name),
            CastLine.Failed => T.CastCouldntStart(name),
            CastLine.Starting => T.CastPhaseStarting(name),
            CastLine.Paused => T.CastStatusPaused(name),
            _ => T.TvStatusPlaying(name),
        };
        var dim = line is CastLine.Starting or CastLine.Rejoining;
        var when = at ?? clock();
        var position = Playhead(when) ?? 0;
        var length = Length;
        string? waiting = null;
        if (!dim && waitingSince is { } since && when - since >= WaitingAfter)
            waiting = T.CastPhaseWaiting(name, (long)Math.Floor((when - since).TotalSeconds));
        return new CastDrawn(
            line, status, dim, item?.Id, item?.Title ?? "", EpisodeLine(item), item?.Image,
            position, length,
            Elapsed: KaeriFfiMethods.PlayClock(position),
            Remaining: length is { } d ? "-" + KaeriFfiMethods.PlayClock(Math.Max(0, d - position)) : "",
            Total: length is { } t ? KaeriFfiMethods.PlayClock(t) : "",
            Paused, waiting,
            Reason: failed ? view?.Notice?.Text() : null);
    }

    /// <summary>"Episode 7 · The made-up title", or the episode alone, or nothing for media from elsewhere.</summary>
    public static string EpisodeLine(FfiCastItem? item) => item switch
    {
        { Episode: { Length: > 0 } episode, EpisodeTitle: { Length: > 0 } title } => T.CastEpisodeTitled(episode, title),
        { Episode: { Length: > 0 } episode } => T.CastEpisode(episode),
        _ => "",
    };

    /// <summary>
    /// The cast button from the session: the engine's state once it says one; until then,
    /// waving while it connects or joins again, filled while a TV is held, and there at all
    /// only while TVs were found.
    /// </summary>
    public static CastButtonState Button(FfiCastSessionView? view, bool tvsFound, bool rejoining, bool driving)
    {
        if (view is not null && CastApi.Button(view) is { } said)
            return rejoining && said != CastButtonState.Connected ? CastButtonState.Connecting
                : driving && said is CastButtonState.Available or CastButtonState.Hidden ? CastButtonState.Connected
                : said;
        return view?.State switch
        {
            FfiCastSessionState.Connecting => CastButtonState.Connecting,
            FfiCastSessionState.Connected or FfiCastSessionState.Playing => CastButtonState.Connected,
            _ when rejoining => CastButtonState.Connecting,
            _ when driving => CastButtonState.Connected,
            _ when tvsFound => CastButtonState.Available,
            _ => CastButtonState.Hidden,
        };
    }
}
