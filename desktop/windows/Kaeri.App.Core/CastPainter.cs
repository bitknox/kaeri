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
    /// <summary>
    /// The connection to the TV closed and the engine makes it again, for up to 20 s: the clock
    /// holds and everything but Stop casting dims.
    /// </summary>
    Reconnecting,
    /// <summary>The TV went away, was stopped on the TV or was taken: Casting stopped, with the reason.</summary>
    Stopped,
    /// <summary>A TV held with nothing playing on it: Connected to Living room, Nothing playing yet.</summary>
    Idle,
}

/// <summary>
/// One frame of the mini and the expanded controller, from the session view as the engine gave
/// it. <see cref="Dim"/> while the TV loads (the seek line without its thumb and times); a
/// reconnect, an end and a TV held with nothing playing dim the controls in place by their
/// <see cref="Line"/>. A wait mid-episode holds the playhead and says so under the seek line after
/// 4 s, with nothing dimmed.
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
    string? Reason,
    Msg? Notice = null);

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
    /// <summary>What the TV last played, kept once the session ends, for the ended frame's title and seek line.</summary>
    FfiCastItem? lastItem;
    double? lastPlayhead;
    /// <summary>Where the clock holds while the connection is made again.</summary>
    double? heldAt;

    /// <summary>The session as the engine last said it.</summary>
    public FfiCastSessionView? View { get; private set; }

    /// <summary>The engine's change count of the last view taken.</summary>
    public ulong Rev => rev;

    /// <summary>The wait after which the line under the seek line says the TV waits for the stream.</summary>
    public static readonly TimeSpan WaitingAfter = TimeSpan.FromSeconds(4);

    public void Take(FfiCastSessionView next)
    {
        var now = clock();
        var was = View?.Item;
        var item = next.Item;
        // Where the playhead stood as the view came, run on to now: the clock holds there through a
        // reconnect, and an ended frame keeps it.
        var standing = Playhead(now);
        var reconnecting = next is { Reconnecting: true, State: FfiCastSessionState.Playing or FfiCastSessionState.Connected };
        heldAt = reconnecting ? Reconnecting ? heldAt : standing : null;
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
        else if (item is null ? next.State != FfiCastSessionState.Error : kept is not { } k || k.Id != item.Id || k.Episode != item.Episode)
            kept = null;
        if (item is not null && CastApi.Waiting(item) is { } seconds)
            waitingSince ??= now - TimeSpan.FromSeconds(seconds);
        else
            waitingSince = null;
        // The session ended: the frame keeps the episode and where the TV last was.
        if (item is null && next.State == FfiCastSessionState.Error && View?.Item is not null)
            lastPlayhead = standing;
        if (item is not null)
        {
            lastItem = item;
            lastPlayhead = null;
        }
        else if (next.State is FfiCastSessionState.Idle or FfiCastSessionState.Connecting)
        {
            lastItem = null;
            lastPlayhead = null;
        }
        View = next;
    }

    /// <summary>The connection to the TV is being made again: the clock holds.</summary>
    public bool Reconnecting => View is { Reconnecting: true, State: FfiCastSessionState.Playing or FfiCastSessionState.Connected };

    /// <summary>The engine's reason a session ended that is an end, not a failure: the TV went away, was stopped there or was taken.</summary>
    public static bool IsEnd(Msg? notice) => notice is Msg.NoticeCastLost or Msg.NoticeCastStoppedElsewhere or Msg.NoticeCastTakenOver;

    static bool Same(FfiCastItem a, FfiCastItem b) => a.Id == b.Id && a.Episode == b.Episode && a.Title == b.Title;

    /// <summary>A seek dragged or sent: the playhead shows it until a newer view comes.</summary>
    public void HoldSeek(double to) => seekHeld = (to, rev);

    /// <summary>Play or pause pressed: the glyph shows the press until a newer view comes.</summary>
    public void HoldPause(bool paused) => pauseHeld = (paused, rev);

    /// <summary>Whether the TV shows it paused, or the press says it will be.</summary>
    public bool Paused => pauseHeld?.Paused ?? View?.Item?.Paused ?? false;

    /// <summary>The length of what plays: as said, else as last said for the same episode.</summary>
    public double? Length => Shown is { } item ? item.Duration is { } d && d > 0 ? d : kept?.Length : null;

    /// <summary>What the frame shows: what the TV plays, or once the session ended, what it last played.</summary>
    FfiCastItem? Shown => View?.Item ?? (View?.State == FfiCastSessionState.Error ? lastItem : null);

    /// <summary>Where the TV's playhead is now, moved on from what it last said while it plays.</summary>
    public double? Playhead(DateTimeOffset? at = null)
    {
        if (seekHeld is { } seek)
            return seek.To;
        if (View?.Item is not { } item)
            return lastItem is null ? null : lastPlayhead ?? lastItem.Position;
        if (Reconnecting && heldAt is { } held)
            return held;
        var still = Paused || item.Loading || CastApi.Waiting(item) is not null || View.State != FfiCastSessionState.Playing || Reconnecting;
        var position = still ? item.Position : item.Position + Math.Max(0, ((at ?? clock()) - heardAt).TotalSeconds);
        return Length is { } d ? Math.Min(position, d) : position;
    }

    /// <summary>
    /// The frame for the TV called <paramref name="name"/>; null while nothing is loaded and
    /// nothing is joined again. With <paramref name="idle"/>, a TV held with nothing playing
    /// draws as well, for the controller that is open as the episode goes.
    /// </summary>
    public CastDrawn? Draw(string name, bool rejoining = false, DateTimeOffset? at = null, bool idle = false)
    {
        var view = View;
        var failed = view?.State == FfiCastSessionState.Error;
        var item = Shown;
        var nothing = idle && view?.State == FfiCastSessionState.Connected && view.Item is null && !rejoining;
        if (item is null && !rejoining && !failed && !nothing)
            return null;
        var line = rejoining && view?.State is not FfiCastSessionState.Playing ? CastLine.Rejoining
            : failed ? IsEnd(view?.Notice) ? CastLine.Stopped : CastLine.Failed
            : nothing ? CastLine.Idle
            : Reconnecting ? CastLine.Reconnecting
            : item is null || item.Loading || view?.State == FfiCastSessionState.Connecting ? CastLine.Starting
            : Paused ? CastLine.Paused
            : CastLine.Playing;
        if (line == CastLine.Idle)
            item = null;
        var status = line switch
        {
            CastLine.Rejoining or CastLine.Reconnecting => T.TvStatusReconnecting(name),
            CastLine.Failed => T.CastCouldntStart(name),
            CastLine.Stopped => T.CastStopped,
            CastLine.Idle => T.CastButtonConnected(name),
            CastLine.Starting => T.CastPhaseStarting(name),
            CastLine.Paused => T.CastStatusPaused(name),
            _ => T.TvStatusPlaying(name),
        };
        var dim = line is CastLine.Starting or CastLine.Rejoining;
        var when = at ?? clock();
        var position = line == CastLine.Idle ? 0 : Playhead(when) ?? 0;
        var length = line == CastLine.Idle ? null : Length;
        string? waiting = null;
        if (line is CastLine.Playing or CastLine.Paused && waitingSince is { } since && when - since >= WaitingAfter)
            waiting = T.CastPhaseWaiting(name, (long)Math.Floor((when - since).TotalSeconds));
        return new CastDrawn(
            line, status, dim, item?.Id, item?.Title ?? "", EpisodeLine(item), item?.Image,
            position, length,
            Elapsed: KaeriFfiMethods.PlayClock(position),
            Remaining: length is { } d ? "-" + KaeriFfiMethods.PlayClock(Math.Max(0, d - position)) : "",
            Total: length is { } t ? KaeriFfiMethods.PlayClock(t) : "",
            Paused, waiting,
            Reason: failed ? view?.Notice?.Text() : null,
            Notice: failed ? view?.Notice : null);
    }

    /// <summary>"Episode 7 · The made-up title", or the episode alone, or nothing for media from elsewhere.</summary>
    public static string EpisodeLine(FfiCastItem? item) => item switch
    {
        { Episode: { Length: > 0 } episode, EpisodeTitle: { Length: > 0 } title } => T.CastEpisodeTitled(episode, title),
        { Episode: { Length: > 0 } episode } => T.CastEpisode(episode),
        _ => "",
    };

    /// <summary>
    /// The cast button's name: Connecting, Connected, or while the TV comes back Reconnecting,
    /// as the mini's line says then.
    /// </summary>
    public static string ButtonName(CastButtonState state, string name, bool reconnecting) => state switch
    {
        CastButtonState.Connecting => T.CastButtonConnecting(name),
        CastButtonState.Connected when reconnecting => T.TvStatusReconnecting(name),
        CastButtonState.Connected => T.CastButtonConnected(name),
        _ => T.TvPlayOn,
    };

    /// <summary>
    /// The cast button from the session: the engine's state, waving while a left session is
    /// joined again, filled while a TV running Kaeri is driven, and there while any TV was found.
    /// </summary>
    public static CastButtonState Button(FfiCastSessionView? view, bool tvsFound, bool rejoining, bool driving)
    {
        if (view is not null && CastApi.Button(view) is { } said)
            return rejoining && said != CastButtonState.Connected ? CastButtonState.Connecting
                : driving && said is CastButtonState.Available or CastButtonState.Hidden ? CastButtonState.Connected
                // The engine knows only Cast TVs; a TV running Kaeri found here shows the button too.
                : said == CastButtonState.Hidden && tvsFound ? CastButtonState.Available
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

/// <summary>What a place in the remote's footer, or an item in its More menu, does.</summary>
public enum CastAction
{
    PlayHere,
    ChooseAnother,
    StopCasting,
    Disconnect,
    ChooseSource,
    TryAgain,
}

/// <summary>
/// The remote's footer and More menu for a frame: two places that keep their spot and change only
/// their words, the first in the accent when it is the way on, and More's items. While it
/// reconnects only Stop casting takes input: Play here waits for Casting stopped. A cast that
/// ended offers what the spec's table gives its reason; one with a single way on (an episode
/// that opens in another app) leaves the second place empty.
/// </summary>
public sealed record CastFooter(CastAction First, bool FirstAccent, bool FirstLive, CastAction? Second, CastAction[] More, bool MoreLive)
{
    public static CastFooter For(CastLine line, Msg? notice = null) => line switch
    {
        CastLine.Stopped or CastLine.Failed => Ended(line, notice),
        CastLine.Idle => new(CastAction.ChooseAnother, FirstAccent: false, FirstLive: true, CastAction.Disconnect, [CastAction.ChooseAnother, CastAction.Disconnect], MoreLive: true),
        CastLine.Reconnecting => new(CastAction.PlayHere, FirstAccent: false, FirstLive: false, CastAction.StopCasting, [CastAction.ChooseAnother, CastAction.StopCasting], MoreLive: false),
        _ => new(CastAction.PlayHere, FirstAccent: false, FirstLive: true, CastAction.StopCasting, [CastAction.ChooseAnother, CastAction.StopCasting], MoreLive: true),
    };

    /// <summary>The two actions of a cast that ended, by the engine's reason (native-ux's failure table).</summary>
    static CastFooter Ended(CastLine line, Msg? notice)
    {
        CastAction[] more = line == CastLine.Stopped ? [CastAction.ChooseAnother] : [];
        // A reason the table does not name: the way on the line has always offered.
        if (!TvWords.Tabled(notice))
            return line == CastLine.Stopped
                ? new(CastAction.PlayHere, FirstAccent: true, FirstLive: true, CastAction.ChooseAnother, more, MoreLive: true)
                : new(CastAction.ChooseSource, FirstAccent: true, FirstLive: true, CastAction.PlayHere, more, MoreLive: true);
        var offers = TvWords.OffersFor(notice).Select(Of).ToArray();
        return new(offers[0], FirstAccent: true, FirstLive: true, offers.Length > 1 ? offers[1] : null, more, MoreLive: true);
    }

    static CastAction Of(CastOffer offer) => offer switch
    {
        CastOffer.ChooseSource => CastAction.ChooseSource,
        CastOffer.TryAgain => CastAction.TryAgain,
        CastOffer.ChooseAnother => CastAction.ChooseAnother,
        _ => CastAction.PlayHere,
    };
        CastLine.Idle => new(CastAction.ChooseAnother, FirstAccent: false, FirstLive: true, CastAction.Disconnect, [CastAction.ChooseAnother, CastAction.Disconnect], MoreLive: true),
        CastLine.Reconnecting => new(CastAction.PlayHere, FirstAccent: false, FirstLive: false, CastAction.StopCasting, [CastAction.ChooseAnother, CastAction.StopCasting], MoreLive: false),
        _ => new(CastAction.PlayHere, FirstAccent: false, FirstLive: true, CastAction.StopCasting, [CastAction.ChooseAnother, CastAction.StopCasting], MoreLive: true),
    };

    /// <summary>The words of a footer place or a More item.</summary>
    public static string Words(CastAction action) => action switch
    {
        CastAction.PlayHere => T.CastPlayHere,
        CastAction.ChooseAnother => T.TvChooseAnother,
        CastAction.StopCasting => T.CastStop,
        CastAction.Disconnect => T.ActionDisconnect,
        CastAction.TryAgain => T.ActionTryAgain,
        _ => T.PlayerChooseSource,
    };
}
