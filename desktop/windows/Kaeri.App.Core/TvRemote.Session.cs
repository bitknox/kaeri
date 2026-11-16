using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;

namespace Kaeri.App;

/// <summary>
/// The cast session: a TV with Chromecast built in held from the title bar's cast button, which
/// every play goes to until it is let go. The engine's watch gives the view the button, the mini
/// and the controller draw from (<see cref="Painter"/>); the controller's tracks and Up next come
/// from the session's card (<see cref="ControllerCard"/>). Rejoining a cast left running at the
/// last launch is the engine's; this only says so while it happens.
/// </summary>
public sealed partial class TvRemote
{
    CastPainter? painter;
    FfiCastSessionWatch? sessionWatch;
    FfiCastSession? controllerSource;
    /// <summary>What to play once the TV is connected, when the engine did not carry it over itself.</summary>
    TvPlay? pendingPlay;
    /// <summary>What the player here played as the TV took it: the player closes once the TV has it.</summary>
    TvPlay? handingOff;
    /// <summary>The last of Kaeri's episodes the TV played, for Play here, Choose source and the notice.</summary>
    TvPlay? lastPlay;
    string? leftOn;
    DateTimeOffset rejoinUntil;

    /// <summary>How long the button waves for a cast left running before it gives up on the TV answering.</summary>
    public static readonly TimeSpan RejoinFor = TimeSpan.FromSeconds(30);

    /// <summary>The session as drawn: the engine's view with the held seek and the waiting clock.</summary>
    public CastPainter Painter => painter ??= new CastPainter(() => this.clock());

    /// <summary>The session view as the engine last said it.</summary>
    public FfiCastSessionView? Session => Painter.View;

    /// <summary>The controller's card: tracks, Up next and skip, while the TV holds a play.</summary>
    public FfiCastCard? ControllerCard { get; private set; }

    /// <summary>The window plays an episode: while a TV is held, the engine sends it there.</summary>
    public Action<TvPlay>? PlayOnTv { get; set; }

    /// <summary>What the player here plays now, with its second, so the cast button can carry it over.</summary>
    public Func<TvPlay?>? PlayingHere { get; set; }

    /// <summary>A cast left running at the last launch is being joined again, until the TV answers.</summary>
    public bool Rejoining => leftOn is not null && clock() < rejoinUntil
        && Session?.State is not (FfiCastSessionState.Playing or FfiCastSessionState.Connected or FfiCastSessionState.Error);

    /// <summary>The title bar's cast button. A Kaeri TV driven over its link counts as connected.</summary>
    public CastButtonState Button => CastPainter.Button(Session, Rows.Length > 0 || CastRows.Length > 0, Rejoining, Connected is not null && Live);

    /// <summary>A TV is held, connecting or connected: plays go to it.</summary>
    public bool SessionHeld => Session?.State is FfiCastSessionState.Connecting or FfiCastSessionState.Connected or FfiCastSessionState.Playing;

    /// <summary>Something of the session's is on the TV, or ended there with a reason: the mini shows.</summary>
    public bool SessionLoaded => Rejoining || Session is { Item: not null, State: FfiCastSessionState.Playing or FfiCastSessionState.Error or FfiCastSessionState.Connecting };

    /// <summary>The held TV's name, or the name of the TV a cast is being joined on.</summary>
    public string SessionName => Session?.DeviceName is { Length: > 0 } name ? name
        : CastRows.FirstOrDefault(r => r.Key == (Session?.DeviceKey ?? leftOn))?.Name ?? "";

    /// <summary>The frame of the mini and the controller now; with <paramref name="idle"/>, also for a TV held with nothing playing.</summary>
    public CastDrawn? Drawn(DateTimeOffset? at = null, bool idle = false) => Painter.Draw(SessionName, Rejoining, at, idle);

    /// <summary>The last episode of Kaeri's the TV played.</summary>
    public TvPlay? SessionPlay => lastPlay;

    void AttachSession(FfiTvSender tvSender)
    {
        CastApi.SetDevice(tvSender, "computer");
        try
        {
            LeftRunning(tvSender.CastLeftOn);
            if (tvSender.CastWatch() is not { } watch)
                return;
            sessionWatch = watch;
            TakeSession(watch.State());
            WatchSession(watch);
        }
        catch (Exception e)
        {
            KaeriFfiMethods.LogLine(FfiLogLevel.Warn, "cast", $"no cast session: {e.Message}");
        }
    }

    Func<string?>? readLeftOn;

    /// <summary>
    /// The TV a cast was left running on at the last launch, read now and while nothing is held:
    /// the engine stops naming it once a cast, with a session or without, connects or plays.
    /// </summary>
    public void LeftRunning(Func<string?> read)
    {
        readLeftOn = read;
        leftOn = read();
        rejoinUntil = clock() + RejoinFor;
    }

    /// <summary>Reads again whether a cast is still left running, while one is waited for.</summary>
    void ReadLeftOn()
    {
        if (leftOn is null || readLeftOn is not { } read)
            return;
        try
        {
            if (read() is null)
                leftOn = null;
        }
        catch (Exception e)
        {
            KaeriFfiMethods.LogLine(FfiLogLevel.Warn, "cast", $"the cast left running wasn't read: {e.Message}");
        }
    }

    async void WatchSession(FfiCastSessionWatch watch)
    {
        try
        {
            while (sessionWatch == watch && await watch.Next() is { } view)
            {
                if (sessionWatch != watch)
                    return;
                TakeSession(view);
            }
        }
        catch (Exception e)
        {
            KaeriFfiMethods.LogLine(FfiLogLevel.Warn, "cast", $"the cast session's watch stopped: {e.Message}");
        }
    }

    /// <summary>
    /// A view from the engine. A play waiting on the connection goes now; the player that asked
    /// closes once the TV has its episode; someone else taking the TV is said once.
    /// </summary>
    public void TakeSession(FfiCastSessionView view)
    {
        var was = Session;
        Painter.Take(view);
        if (view.State is FfiCastSessionState.Playing or FfiCastSessionState.Connected or FfiCastSessionState.Error)
            leftOn = null;
        else if (view.State == FfiCastSessionState.Idle)
            ReadLeftOn();
        if (view.Item is { Id: { } id, Episode: { Length: > 0 } episode } item)
            lastPlay = new TvPlay(id, item.Title, episode);
        if (view.State is FfiCastSessionState.Connected or FfiCastSessionState.Playing && pendingPlay is { } play)
        {
            pendingPlay = null;
            PlayOnTv?.Invoke(play);
        }
        if (handingOff is { } handed && view.Item?.Id == handed.Id)
        {
            handingOff = null;
            HandedOff?.Invoke();
        }
        if (view.State is FfiCastSessionState.Idle or FfiCastSessionState.Error)
        {
            pendingPlay = null;
            handingOff = null;
        }
        Controller(view);
        Say();
        if (view.State == FfiCastSessionState.Error && was?.State != FfiCastSessionState.Error
            && CastPainter.IsEnd(view.Notice) && view.Notice is { } why && lastPlay is { } last)
            SessionEnded?.Invoke(last with { From = Painter.Playhead() is { } at && at >= 5 ? at : null }, why);
    }

    /// <summary>
    /// The held TV went away, was stopped on the TV or was taken by another sender: the episode,
    /// at the second the TV was last at, and the engine's reason, for the notice with Play here.
    /// </summary>
    public Action<TvPlay, Msg>? SessionEnded { get; set; }

    /// <summary>
    /// Something plays on the held TV, or is starting there: the way to let go is Stop casting.
    /// With nothing playing it is Disconnect. The two never show together.
    /// </summary>
    public bool SessionPlaying => Session is { State: FfiCastSessionState.Playing } or { State: FfiCastSessionState.Connecting, Item: not null };

    /// <summary>The words for letting go of the held TV: Stop casting while something plays, else Disconnect.</summary>
    public string LetGoWord => SessionPlaying ? T.CastStop : T.ActionDisconnect;

    /// <summary>The controller's card follows the play on the TV, and goes when the TV is let go.</summary>
    void Controller(FfiCastSessionView view)
    {
        if (view.State is not (FfiCastSessionState.Playing or FfiCastSessionState.Connected))
        {
            controllerSource = null;
            ControllerCard = null;
            return;
        }
        if (controllerSource is not null || sender is null)
            return;
        try
        {
            if (sender.CastSession() is not { } source)
                return;
            controllerSource = source;
            ControllerCard = source.Card();
            ReadController(source);
        }
        catch (Exception e)
        {
            KaeriFfiMethods.LogLine(FfiLogLevel.Warn, "cast", $"no card for the cast: {e.Message}");
        }
    }

    async void ReadController(FfiCastSession source)
    {
        try
        {
            while (await source.NextCard() is { } card)
            {
                if (controllerSource != source)
                    return;
                ControllerCard = card;
                Say();
            }
        }
        catch (Exception e)
        {
            KaeriFfiMethods.LogLine(FfiLogLevel.Warn, "cast", $"the cast's card stopped: {e.Message}");
        }
        // That play's card ended: the next view finds the next one's.
        if (controllerSource == source)
            controllerSource = null;
    }

    /// <summary>Play on TV asked while a TV is held: it plays there, and the player that asked closes once it does.</summary>
    bool OfferToSession(TvPlay request)
    {
        if (!SessionHeld)
            return false;
        if (request.FromPlayer)
            handingOff = request;
        if (Session?.State == FfiCastSessionState.Connecting)
            pendingPlay = request;
        else
            PlayOnTv?.Invoke(request);
        Say();
        return true;
    }

    /// <summary>
    /// A Chromecast row chosen: the TV is held from now on. What Play on TV asked for, or what
    /// the player here plays, moves to it; the player closes once the TV has it.
    /// </summary>
    public void ConnectCast(FfiCastRow row)
    {
        if (sender is null)
            return;
        var request = Request ?? PlayingHere?.Invoke();
        Problem = null;
        try
        {
            var from = request is { FromPlayer: true } here ? new CastFrom(here.Id, here.Episode, here.From ?? 0) : null;
            var carried = CastApi.Connect(sender, row.Key, from);
            pendingPlay = carried ? null : request;
            handingOff = request is { FromPlayer: true } ? request : null;
            Request = null;
            if (Sheet == TvSheet.Picker)
                Sheet = null;
        }
        catch (Exception e)
        {
            Problem = TvWords.Of(e);
        }
        Say();
    }

    /// <summary>
    /// A TV running Kaeri chosen from the title bar's menu: what the player here plays goes to
    /// it, as Play on TV would send it; a TV not paired yet starts pairing.
    /// </summary>
    public void ChooseTv(FfiTvRow row)
    {
        Request ??= PlayingHere?.Invoke();
        Problem = null;
        Choose(row);
    }

    /// <summary>Stop casting or Disconnect: what plays on the TV stops, and the TV is let go.</summary>
    public void DisconnectCast()
    {
        pendingPlay = null;
        handingOff = null;
        sender?.CastDisconnect();
        Say();
    }

    /// <summary>
    /// Play here: the TV is let go, and the episode comes back with its second for the player
    /// here. Null when nothing of Kaeri's played on the TV.
    /// </summary>
    public TvPlay? CastPlayHere()
    {
        if (sender is null)
            return null;
        var last = lastPlay;
        var at = Painter.Playhead();
        pendingPlay = null;
        handingOff = null;
        var from = CastApi.PlayHere(sender);
        if (from is null)
            sender.CastDisconnect();
        Say();
        if (from is { } back)
            return new TvPlay(back.Id, last?.Title ?? "", back.Episode, back.Position);
        return last is { } play ? play with { From = at is { } s && s >= 5 ? s : null } : null;
    }

    /// <summary>Choose another TV: this one is let go, and the chooser plays the same episode on the next.</summary>
    public void ChooseAnotherTv()
    {
        var play = lastPlay is { } last ? last with { From = Painter.Playhead() } : null;
        DisconnectCast();
        Request = play;
        Say();
    }

    /// <summary>The episode the TV couldn't play, for its sources here: the TV is let go first.</summary>
    public TvPlay? CastSources()
    {
        var play = lastPlay;
        DisconnectCast();
        return play;
    }

    /// <summary>A control of the mini's or the controller's, sent to the held TV.</summary>
    public void SessionCommand(FfiTvCommand command) => sender?.CastCommand(command);

    /// <summary>Play or pause: the glyph swaps at once and stays.</summary>
    public void SessionTogglePause()
    {
        Painter.HoldPause(!Painter.Paused);
        SessionCommand(new FfiTvCommand.TogglePause());
        Say();
    }

    /// <summary>A seek to <paramref name="to"/>, held on the seek line until the TV answers.</summary>
    public void SessionSeekTo(double to)
    {
        Painter.HoldSeek(to);
        SessionCommand(new FfiTvCommand.SeekTo(to));
        Say();
    }

    /// <summary>Back or forward by <paramref name="by"/> seconds, held as a seek is.</summary>
    public void SessionSeekBy(double by)
    {
        if (Painter.Playhead() is { } at)
        {
            var to = Math.Max(0, at + by);
            if (Painter.Length is { } length)
                to = Math.Min(to, length);
            Painter.HoldSeek(to);
        }
        SessionCommand(new FfiTvCommand.Seek(by));
        Say();
    }

    /// <summary>Up next's Play now and Cancel, through the play's card.</summary>
    public void SessionPlayNext() => controllerSource?.PlayNext();

    public void SessionCancelNext() => controllerSource?.CancelNext();
}
