using System.Diagnostics;
using System.Globalization;
using System.Runtime.InteropServices;
using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Kaeri.Windows.Discussion;
using Kaeri.Windows.Pages;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using VirtualKey = global::Windows.System.VirtualKey;

namespace Kaeri.Windows.Player;

/// <summary>
/// The player, over the page it came from. mpv draws through d3d11 composition into a swapchain
/// of its own; this view hands that swapchain to the SwapChainPanel (polled, since mpv says
/// nothing when it makes a new one) and tells mpv the panel's size in pixels. The engine drives
/// mpv through kaeri-mpv-host (FfiPlayer); the controls here only send the engine's player
/// commands and draw what the engine and the player say.
/// </summary>
public sealed partial class PlayerView : UserControl
{
    static readonly TimeSpan IdleHide = TimeSpan.FromSeconds(2);

    Ctx? ctx;
    FfiPlayer? player;
    readonly DispatcherQueueTimer tick;
    readonly DispatcherQueueTimer idle;
    nint bound;
    bool paused, fromPlayer, overBar, menuOpen, keepControls;
    double position, duration, volume = 100;
    Coming? upNext;
    PlayerUi.Menus? menus;
    /// <summary>The file's audio and subtitle tracks, as mpv last listed them.</summary>
    FfiTrack[] tracks = [];
    string tracksSeen = "";
#if KAERI_TESTING
    bool tracksHeld;
#endif
    readonly StillWatching still = new();
    bool pictureSeen, pointerHidden, mini, miniTipShown;
    SeekPreview? preview;
    PreviewSource? stream, thumbs, previewSource;

    /// <summary>Where the player keeps its compiled shaders, for the seek preview to share.</summary>
    internal string? ShaderCache { get; set; }
    string? thumbnails;
    readonly object loads = new();
    PreviewSource? lastLoad;

    // Motion and the corner: what is shown, so a fade in progress does not read as a state.
    readonly DispatcherQueueTimer click;
    readonly DispatcherQueueTimer flash;
    bool chromeShown = true, startingShown;
    UIElement? cornerCard;
    bool skipShown, stallShown, upNextShown, skipHover;
    string skipDone = "";
    readonly Stopwatch skipSince = new(), stallSince = new();
    long startingFrom;
    bool startingHeld;
    ulong stallBase;
    Undoable? undo;
    string startingEpisode = "", playingEpisode = "";
    double speed = 1, mutedFrom, wheelSeek;
    string timelineStatus = "";

    static readonly double[] Speeds = [0.5, 0.75, 1, 1.25, 1.5, 1.75, 2];

    /// <summary>An automatic skip that can be undone: back to <c>To</c>, for 6 s of playback or 15 s paused, twice that with a screen reader.</summary>
    sealed class Undoable(double to, string line)
    {
        public double To { get; } = to;
        public string Line { get; } = line;
        public double Played { get; set; }
        public double Waited { get; set; }
    }

    public PlayerView()
    {
        InitializeComponent();
        Timeline.Spoken = () => T.PlayerPositionSpoken(SpokenClock(position), SpokenClock(duration));
        VolumeSlider.Spoken = () => T.PlayerVolumeSpoken((long)Math.Round(VolumeSlider.Value));
        ToolTips();
        click = DispatcherQueue.GetForCurrentThread().CreateTimer();
        click.Interval = TimeSpan.FromMilliseconds(GetDoubleClickTime());
        click.IsRepeating = false;
        click.Tick += (_, _) => TogglePause();
        flash = DispatcherQueue.GetForCurrentThread().CreateTimer();
        flash.Interval = TimeSpan.FromMilliseconds(1200);
        flash.IsRepeating = false;
        flash.Tick += (_, _) => Motion.Hide(Flash);
        Motion.QuickOpacity(SkipCard);
        AutomationProperties.SetLiveSetting(SkipCard, AutomationLiveSetting.Polite);
        // ] and [ have no VirtualKey names.
        foreach (var (key, step) in new[] { ((VirtualKey)221, 1), ((VirtualKey)219, -1) })
        {
            var accelerator = new KeyboardAccelerator { Key = key };
            accelerator.Invoked += (_, e) => Key(e, () => SpeedStep(step));
            KeyboardAccelerators.Add(accelerator);
        }
        // The window closing takes the starting screen's clock and its looping line down first.
        Unloaded += (_, _) =>
        {
            startTick?.Stop();
            EndStart();
        };
        tick = DispatcherQueue.GetForCurrentThread().CreateTimer();
        tick.Interval = TimeSpan.FromMilliseconds(250);
        tick.Tick += (_, _) => Tick();
        idle = DispatcherQueue.GetForCurrentThread().CreateTimer();
        idle.Interval = IdleHide;
        idle.IsRepeating = false;
        idle.Tick += (_, _) => HideControlsIfIdle();
        // A slider takes its pointer events: the preview listens even to the handled ones.
        Timeline.AddHandler(PointerMovedEvent, new PointerEventHandler(OnTimelinePointerMoved), true);
        Root.AddHandler(PointerMovedEvent, new PointerEventHandler(OnPointerMoved), true);
        Timeline.AddHandler(PointerPressedEvent, new PointerEventHandler(OnTimelinePressed), true);
        Timeline.AddHandler(PointerReleasedEvent, new PointerEventHandler(OnTimelineReleased), true);
        Timeline.AddHandler(PointerCaptureLostEvent, new PointerEventHandler(OnTimelineReleased), true);
        VolumeSlider.AddHandler(PointerPressedEvent, new PointerEventHandler((_, _) => volumeDragging = true), true);
        VolumeSlider.AddHandler(PointerReleasedEvent, new PointerEventHandler((_, _) => volumeDragging = false), true);
        VolumeSlider.AddHandler(PointerCaptureLostEvent, new PointerEventHandler((_, _) => volumeDragging = false), true);
        // The party's reaction moments stand on the corner's card, whatever its height.
        Corner.SizeChanged += (_, _) => LiftCorner();
        AddHandler(PreviewKeyDownEvent, new KeyEventHandler(OnPreviewKey), true);
        LosingFocus += OnLosingFocus;
    }

    /// <summary>
    /// While the player is up the keyboard stays in it: a click on a control that takes no focus
    /// would otherwise hand it to the page underneath, whose list then scrolls on Space and the
    /// arrows instead of the player hearing them. A menu, a flyout or a dialog, in their popups,
    /// still take it.
    /// </summary>
    void OnLosingFocus(UIElement sender, LosingFocusEventArgs e)
    {
        // Only a click's: a flyout, a dialog or the window moving focus on purpose is let be,
        // as cancelling theirs fails inside XAML (E_INVALIDARG, with no stack).
        if (!Showing || e.FocusState != FocusState.Pointer || e.NewFocusedElement is not DependencyObject next
            || Within(next, this) || InPopup(next))
            return;
        e.TryCancel();
    }

    bool InPopup(DependencyObject element)
    {
        for (var at = element; at is not null; at = VisualTreeHelper.GetParent(at))
            if (at is Popup)
                return true;
        if (XamlRoot is null)
            return false;
        foreach (var popup in VisualTreeHelper.GetOpenPopupsForXamlRoot(XamlRoot))
            if (popup.Child is { } child && Within(element, child))
                return true;
        return false;
    }

    /// <summary>
    /// Space is play and pause, also with the keyboard on a bar button or the seek line, which
    /// would otherwise press the button or step the line. Enter still presses a focused button.
    /// </summary>
    void OnPreviewKey(object sender, KeyRoutedEventArgs e)
    {
        if (e.Key != VirtualKey.Space || e.Handled || !Showing || Typing() || still.Asking)
            return;
        if (XamlRoot is null || FocusManager.GetFocusedElement(XamlRoot) is not Control focused || focused is not (ButtonBase or Slider))
            return;
        if (!Within(focused, Chrome) && !Within(focused, Corner))
            return;
        e.Handled = true;
        if (e.KeyStatus.WasKeyDown)
            return;
        ShowControls();
        TogglePause();
    }

    /// <summary>True while the player is on screen.</summary>
    public bool Showing => Visibility == Visibility.Visible;

    /// <summary>A file plays on screen, past its starting screen: in a party, its seat hears the room's notices.</summary>
    internal bool Playing => Showing && pictureSeen;

    /// <summary>Fullscreen asked for: by F, the button or the engine.</summary>
    internal event Action<bool?>? FullscreenRequested;

    /// <summary>The player shown or hidden, for the window to give its title bar to it.</summary>
    internal event Action<bool>? ShowingChanged;

    /// <summary>The controls shown or hidden, for the window's caption buttons to follow.</summary>
    internal event Action<bool>? ControlsChanged;

    internal void Attach(Ctx ctx, FfiPlayer? player)
    {
        this.ctx = ctx;
        this.player = player;
        keepControls = ctx.Options.KeepControls;
        SubtitleStyling.SystemChanged += () => DispatcherQueue.TryEnqueue(() =>
        {
            styled = null;
            ApplySubtitleStyle(onlyIfChanged: true);
        });
    }

    /// <summary>A player command that comes from the viewer: it also says somebody is there.</summary>
    void Send(PlayerCommand command)
    {
        still.Input();
        ctx?.Send(new Command.Player(command));
    }

    /// <summary>A player command the player sends itself, such as playing on when the countdown ends.</summary>
    void SendQuietly(PlayerCommand command) => ctx?.Send(new Command.Player(command));

    // The window's side: show, hide, and what the player and the engine say.

    internal void Open()
    {
        if (Showing)
            return;
        // Where focus was, for the way back when the player closes.
        returnFocus = XamlRoot is null ? null : FocusManager.GetFocusedElement(XamlRoot) as Control;
        Visibility = Visibility.Visible;
        still.Reset();
        ShowControls();
        LiftSubtitles(animated: false);
        tick.Start();
        ShowStats();
        ChatReflect();
        // Just made visible, the view has no layout yet and WinUI refuses it the focus, which
        // then stays on what opened it under the player: the player's keys would go there (Space
        // pressing a Home card again). Asked again once it is laid out.
        if (!Focus(FocusState.Programmatic))
            DispatcherQueue.TryEnqueue(DispatcherQueuePriority.Low, () =>
            {
                if (Showing)
                    Focus(FocusState.Programmatic);
            });
        ShowingChanged?.Invoke(true);
#if KAERI_TESTING
        Probe.Mark("loaded player");
#endif
    }

    Control? returnFocus;

    /// <summary>After the player closed: focus goes back to the control that opened it, when it is still there.</summary>
    internal bool RestoreFocus()
    {
        var back = returnFocus;
        returnFocus = null;
        return back is { XamlRoot: not null, IsLoaded: true } && back.Focus(FocusState.Programmatic);
    }

    internal void Hide()
    {
        if (!Showing)
            return;
        closedByHost = false;
        verdictWait?.Stop();
        scrubbing = null;
        seekTarget = null;
        pauseAsked = null;
        upNextStill = null;
        discussion?.Close();
        marking = null;
        skipPrompt = null;
        DrawMarking();
        tick.Stop();
        idle.Stop();
        ShowPointer();
        ClosePreview();
        if (mini)
            MiniRequested?.Invoke(false);
        SetMini(false);
        still.Reset();
        click.Stop();
        Visibility = Visibility.Collapsed;
        bufferingSince.Reset();
        bufferingShown = false;
        foreach (var layer in new UIElement[] { BufferingRing, StillLayer, Starting, SkipSlot, UndoCard, UpNextCard, StallCard, AlertCard, Flash, StatsLayer, ChatFieldSlot, TickLabel })
            Motion.Set(layer, false);
        ChatClear();
        // Kept up for the next playback, which waits for its own figures.
        stats.Restart();
        statsUp = false;
        startingShown = skipShown = stallShown = upNextShown = false;
        EndStart();
        start = null;
        cornerCard = null;
        undo = null;
        upNext = null;
        thumbnails = null;
        thumbs = null;
        timelineMarks = [];
        buffered = [];
        DrawTimeline();
        DropSubtitleLift();
        ShowingChanged?.Invoke(false);
    }

    internal void Heard(FfiPlayerEvent e)
    {
        switch (e)
        {
            case FfiPlayerEvent.Opened o:
                closedByHost = false;
                muted = false;
                Open();
                if (o.FullscreenValue)
                    FullscreenRequested?.Invoke(true);
                break;
            case FfiPlayerEvent.Closed:
                HostClosed();
                break;
            case FfiPlayerEvent.Fullscreen f:
                FullscreenRequested?.Invoke(f.On);
                break;
            case FfiPlayerEvent.State s:
                ShowState(s.StateValue);
                break;
        }
    }

    internal void Heard(IReadOnlyList<Event> batch)
    {
        foreach (var e in batch)
        {
            switch (e)
            {
                case Event.PlayerUi ui:
                    Heard(ui.Ui);
                    break;
                case Event.PlaybackStaged s when !Showing:
                    HeardStart(e);
                    Staged(s);
                    break;
                case Event.PlaybackStaged:
                    // A newer play is being found behind the one on screen (N, an episode
                    // picked, a roll-on): closing the player must stop it too.
                    stagedAhead = true;
                    HeardStart(e);
                    break;
                case Event.PlaybackEnded { Failure: null, RollingOn: false } when closedByHost && Showing:
                    Hide();
                    break;
                case Event.PlaybackEnded { Failure: null } when findingFor is not null:
                    // Over before the player opened, with no failure for the starting screen to
                    // show: the engine's notice says why, and the screen goes.
                    findingFor = null;
                    Hide();
                    break;
                case Event.PlaybackStaged or Event.StartingStep or Event.PlaybackEnded:
                    HeardStart(e);
                    break;
                case Event.PlayerSkipMark or Event.PlayerSkipPrompt or Event.PlayerSkipNotice:
                    HeardSkip(e);
                    break;
                case Event.PartyJoined or Event.PartyLeft:
                    ShowPartyButton();
                    ChatHeard(e);
                    break;
                case Event.PartyChat or Event.PartyChatState or Event.PartyReaction or Event.PartyChatRefused:
                    ChatHeard(e);
                    break;
                case Event.TitleDub td when td.Id == playingId:
                    BuildTracks();
                    break;
                case Event.ShowSettings st when st.V1.Id == playingId:
                    ownSpoilers = st.V1.Rows.FirstOrDefault(r => r.Key == ShowSettingKey.SpoilerProtection) is { } own ? own.Value == "on" : null;
                    break;
                case Event.Settings:
                    stylePanel?.Heard(batch);
                    ApplySubtitleStyle(onlyIfChanged: true);
                    break;
                case Event.PlayerTimeline t:
                    timelineMarks = t.Marks ?? [];
                    DrawTimeline();
                    break;
                case Event.PlayerBuffered b:
                    buffered = b.Ranges ?? [];
                    DrawTimeline();
                    break;
                case Event.PlayerStats ps:
                    TookStats(ps.Stats);
                    break;
                case Event.Clipboard clip:
                    TookClipboard(clip.Text);
                    break;
                case Event.PlaybackVolume v:
                    TookVolume(v.V1);
                    break;
            }
        }
    }

    /// <summary>mpv's volume: from the player's state, which has it from the start of a session,
    /// and from the engine when the viewer changed it. The note shows once the change is in.</summary>
    void TookVolume(double level)
    {
        // Reached, or mpv settled somewhere else (a step it clamped): the asked level goes.
        if (volumeAsked is { } asked && (Math.Abs(asked - level) < 0.01 || volumeSince.Elapsed > TimeSpan.FromSeconds(1)))
            volumeAsked = null;
        mpvVolume = level;
        ShowVolume();
    }

    /// <summary>The volume asked for last while mpv has not reported it yet, as the Mac's volumeAsked.</summary>
    double? volumeAsked;
    readonly Stopwatch volumeSince = new();
    double mpvVolume = 100;
    bool volumeDragging;

    /// <summary>
    /// Step the volume by <paramref name="by"/> from where it shows, held to 0 and 100 (or above
    /// that when mpv already is). The line shows the step at once, mpv's report after.
    /// </summary>
    void StepVolume(double by, bool flash = true)
    {
        still.Input();
        if (muted)
            Mute();
        var from = volumeAsked ?? mpvVolume;
        var to = Math.Round(Math.Clamp(from + by, 0, Math.Max(100, from)));
        if (to == from)
            return;
        volumeAsked = to;
        volumeSince.Restart();
        Send(new PlayerCommand.Volume(to - from));
        ShowVolume();
        if (flash)
            ShowFlash(to <= 0 ? T.PlayerMuted : T.PlayerVolumeLevel(Wording.Percent(to)));
    }

    void ShowVolume()
    {
        var level = volumeAsked ?? mpvVolume;
        if (level == volume)
            return;
        volume = level;
        // A drag keeps the thumb under the pointer.
        if (!volumeDragging)
        {
            fromPlayer = true;
            VolumeSlider.Value = Math.Clamp(volume, 0, 100);
            fromPlayer = false;
        }
        ShowMuted();
    }

    /// <summary>Play or pause, shown at once: the glyph does not wait for mpv's answer.</summary>
    void TogglePause()
    {
        var to = !paused;
        Send(new PlayerCommand.TogglePause());
        if (!pictureSeen)
            return;
        pauseAsked = to;
        pauseSince.Restart();
        ShowPaused(to);
        if (to)
            ShowControls();
    }

    bool? pauseAsked;
    readonly Stopwatch pauseSince = new();
    bool pausedShown;

    /// <summary>The play/pause button's glyph, name and tooltip, touched only when it changes, so its open tooltip stays.</summary>
    void ShowPaused(bool now)
    {
        paused = now;
        if (now == pausedShown && PlayPauseIcon.Glyph.Length > 0)
            return;
        pausedShown = now;
        PlayPauseIcon.Glyph = now ? "" : "";
        AutomationProperties.SetName(PlayPause, now ? T.ActionPlay : T.ActionPause);
        ToolTipService.SetToolTip(PlayPause, T.TooltipWithKey(now ? T.ActionPlay : T.ActionPause, "Space"));
    }

    // Scrubbing: the line follows the pointer, mpv follows with keyframe seeks, and the release
    // lands with an exact one (native-ux.md, scrubbing).

    /// <summary>The drag's time while the seek line is held.</summary>
    double? scrubbing;
    double? scrubSent;
    /// <summary>Where a click, a key or a release sent the playhead, until mpv has landed there.</summary>
    double? seekTarget;
    readonly Stopwatch seekSince = new();
    bool timelineSettling;

    void OnTimelinePressed(object sender, PointerRoutedEventArgs e)
    {
        if (duration <= 0 || !e.GetCurrentPoint(Timeline).Properties.IsLeftButtonPressed && e.Pointer.PointerDeviceType == Microsoft.UI.Input.PointerDeviceType.Mouse)
            return;
        // Pressed on the thumb where it is: nothing to seek yet.
        var at = Timeline.Value;
        scrubbing = at;
        scrubSent = Math.Abs(at - position) < 1 ? at : null;
        scrubSeeked = false;
        scrubByMouse = e.Pointer.PointerDeviceType == Microsoft.UI.Input.PointerDeviceType.Mouse;
        ShowControls();
        Scrub(at);
    }

    bool scrubSeeked, scrubByMouse;

    void OnTimelineReleased(object sender, PointerRoutedEventArgs e) => EndScrub();

    /// <summary>
    /// A mouse drag whose release never reached the line (a quick click can end in the slider's
    /// thumb without a release here): ended once the button is up, so the line is never held.
    /// </summary>
    void EndScrubIfLetGo()
    {
        if (scrubbing is not null && scrubByMouse && (GetAsyncKeyState(0x01) & 0x8000) == 0)
            EndScrub();
    }

    [LibraryImport("user32.dll")]
    private static partial short GetAsyncKeyState(int key);

    void EndScrub()
    {
        if (scrubbing is not { } at)
            return;
        scrubbing = null;
        scrubSent = null;
        if (scrubSeeked || Math.Abs(at - position) >= 1)
            SeekTo(at);
        ShowControls();
    }

    /// <summary>A keyframe seek for each half second the drag moves, straight to mpv: it drops what it can't keep up with. Not in a party, where each would reach the room as a seek.</summary>
    void Scrub(double seconds)
    {
        scrubbing = seconds;
        ShowClocks();
        ShowPlayed();
        if (InParty || player is not { } p || duration <= 0)
            return;
        if (scrubSent is { } sent && Math.Abs(sent - seconds) < 0.5)
            return;
        ulong now;
        lock (loads)
            now = playback;
        if (now == 0)
            return;
        scrubSent = seconds;
        scrubSeeked = true;
        p.Apply(now, [new PlayerOp.Set("percent-pos", new MpvValue.Double(Math.Clamp(seconds / duration * 100, 0, 100)))]);
    }

    /// <summary>An exact seek the viewer asked for: the line holds there until mpv lands.</summary>
    void SeekTo(double seconds)
    {
        if (duration <= 0)
            return;
        seekTarget = Math.Clamp(seconds, 0, duration);
        seekSince.Restart();
        if (upNext is not null)
            upNext.Sought = true;
        // Restarted here as well as when the engine's seek reaches mpv, so mpv's first report
        // from the new place never finds the countdown spent.
        Sought(seekTarget.Value);
        SendControl(new PlayerCommand.SeekTo(seekTarget.Value));
        HoldLine();
    }

    /// <summary>The thumb and the clocks at the time asked for, before mpv has answered.</summary>
    void HoldLine()
    {
        ShowClocks();
        fromPlayer = true;
        Timeline.Value = Math.Min(Shown, Timeline.Maximum);
        fromPlayer = false;
    }

    /// <summary>A command for the playback on screen: once the next episode has taken over, the engine lets it go rather than carrying it there.</summary>
    void SendControl(PlayerCommand command, bool quiet = false)
    {
        ulong now;
        lock (loads)
            now = playback;
        if (!quiet)
            still.Input();
        ctx?.Send(now == 0 ? new Command.Player(command) : new Command.Control(now, command));
    }

    /// <summary>The title staged and waiting on a source, before the player itself has said anything.</summary>
    uint? findingFor;

    /// <summary>
    /// Play was pressed: the starting screen at once, the show's banner behind it, while the
    /// engine finds a source, which can take seconds (native-ux: from PlaybackStaged to the first
    /// frame). The player's own Starting carries on from it.
    /// </summary>
    void Staged(Event.PlaybackStaged s)
    {
        closedWhileFinding = false;
        stagedAhead = false;
        Open();
        findingFor = s.Id;
        pictureSeen = false;
        playingId = s.Id;
        ShowTitle.Text = s.Title;
        EpisodeTitle.Text = Lines.EpisodeNamed(s.Episode, s.EpisodeTitle);
        playingEpisode = s.Episode;
        var entry = ctx?.Known.Entry(s.Id);
        BeginStart(new PlayerUi.Starting(s.Id, s.Title, s.Episode, s.EpisodeTitle, entry?.CoverUrl, entry?.BannerUrl));
    }

    void Heard(PlayerUi ui)
    {
        switch (ui)
        {
            case PlayerUi.Starting s:
                if (closedWhileFinding)
                {
                    // Found after the viewer closed the starting screen: told so, not shown.
                    player?.ViewerClosed();
                    break;
                }
                // Up since Play was pressed (Staged): the same screen goes on, its clock too.
                var fromStaged = findingFor == s.Id;
                findingFor = null;
                stagedAhead = false;
                closedByHost = false;
                Open();
                still.Started();
                pictureSeen = false;
                ClosePreview();
                Motion.Set(StillLayer, false);
                still.Answered();
                // Whether the show has a dub decides whether Tracks offers Sub and Dub; asked once
                // a session for a show whose page has not already asked.
                playingId = s.Id;
                ownSpoilers = null;
                if (ctx is { } c && !c.Known.Dubbed.ContainsKey(s.Id))
                    c.Send(new Command.LoadDub(s.Id));
                // The table's synopses and the show's own protection, for the Episodes panel.
                if (ctx is { } asking && s.Id != 0)
                {
                    asking.Send(new Command.LoadEpisodes(s.Id));
                    asking.Send(new Command.LoadShowSettings(s.Id));
                }
                ShowTitle.Text = s.Title;
                EpisodeTitle.Text = Lines.EpisodeNamed(s.EpisodeValue, s.EpisodeTitle);
                playingEpisode = s.EpisodeValue;
                ShowTvButton();
                if (fromStaged && start is { } open && open.Id == s.Id && open.Episode == s.EpisodeValue)
                {
                    // Up since Play was pressed: the same start goes on, its steps and clock too,
                    // given the art now if the title's entry had none.
                    if (StartingArt.Source is null && (s.BannerUrl ?? s.CoverUrl) is { Length: > 0 } art)
                    {
                        ctx?.Art.Bind(art, image =>
                        {
                            StartingArt.Source = image;
                            startArtLoaded = true;
                            RevealStartArt();
                        });
                        ctx?.Art.Bytes(art, bytes => _ = MeasureGround(bytes));
                    }
                }
                else
                {
                    BeginStart(s);
                }
                undo = null;
                UpdateCorner();
                // The last file's marks, buffered stretches and length are not this one's.
                timelineMarks = [];
                buffered = [];
                seekTarget = null;
                duration = 0;
                position = 0;
                DrawTimeline();
                break;
            case PlayerUi.Episode ep:
                playingEpisode = ep.Number;
                ShowTitle.Text = ep.Show;
                EpisodeTitle.Text = Lines.Episode(ep.Label, ep.Title, ep.Translation);
                break;
            case PlayerUi.Menus m:
                menus = m;
                BuildMenus();
                break;
            case PlayerUi.Skip skip:
                SkipCard.Content = skip.Kind switch { SkipKind.Ending => T.PlayerSkipEnding, SkipKind.Recap => T.PlayerSkipRecap, _ => T.PlayerSkipOpening };
                skipDone = Skipped(skip.Kind);
                skipShown = true;
                UpdateCorner();
                break;
            case PlayerUi.SkipCleared:
                skipShown = false;
                UpdateCorner();
                break;
            case PlayerUi.AutoSkipped auto:
                undo = new Undoable(auto.UndoTo, Skipped(auto.Kind));
                UndoLine.Text = undo.Line;
                UpdateCorner();
                break;
            case PlayerUi.UpNext when HoldUpNext:
                break;
            case PlayerUi.UpNext next:
                UpNextTitle.Text = Lines.EpisodeNamed(next.Next.Number, next.Next.Title);
                var card = new Coming(next.Next.Number, next.Auto, next.EndingAt, next.Countdown);
                // Said again for the same episode (once its still is fetched): what the viewer did
                // with the card stays, and so does the still.
                if (upNext is { } before && before.Number == card.Number)
                {
                    card.Cancelled = before.Cancelled;
                    card.Played = before.Played;
                    card.RestartedAt = before.RestartedAt;
                    card.Sought = before.Sought;
                }
                upNext = card;
                UpNextStillFrame.Visibility = next.Next.Thumbnail is null ? Visibility.Collapsed : Visibility.Visible;
                if (next.Next.Thumbnail != upNextStill)
                {
                    upNextStill = next.Next.Thumbnail;
                    UpNextStill.Source = null;
                    ctx?.Art.Bind(next.Next.Thumbnail, image => UpNextStill.Source = image);
                }
                still.After = (int)next.StillWatchingAfter;
                ShowUpNext();
                break;
            case PlayerUi.UpNextCleared:
                upNext = null;
                ShowUpNext();
                break;
            case PlayerUi.Stall stall:
                stallBase = stall.Seconds;
                stallSince.Restart();
                StallLine.Text = T.PlayerStallTitle((long)stall.Seconds);
                stallShown = true;
                UpdateCorner();
                break;
            case PlayerUi.StallCleared:
                stallShown = false;
                UpdateCorner();
                break;
            case PlayerUi.Alert alert:
                // A chat line has its own place on the video, in the column.
                if (!ChatEcho(alert.Text))
                    Alert(alert.Text);
                break;
            case PlayerUi.Thumbnails thumb:
                if (thumb.Url != thumbnails)
                    ClosePreview();
                thumbnails = thumb.Url;
                thumbs = null;
                break;
        }
    }

    /// <summary>
    /// A screenshot run's: no Up next card, for a fixture clip short enough to be near its end
    /// from the start.
    /// </summary>
    internal bool HoldUpNext { get; set; }

    /// <summary>A line for the viewer over the video, where the window's toasts can't be seen.</summary>
    internal void Say(string text) => Alert(text);

    /// <summary>Whether the alert is up: it counts as one of the chat column's three lines.</summary>
    bool alertUp;

    async void Alert(string text)
    {
        AlertText.Text = text;
        Motion.Show(AlertCard);
        alertUp = true;
        MakeChatRoom();
        Announce(AlertText);
        await Task.Delay(TimeSpan.FromSeconds(4 * NoticeTime.Scale(DeviceWatch.ScreenReaderRunning())));
        if (AlertText.Text == text)
        {
            Motion.Hide(AlertCard);
            alertUp = false;
            DrainChat();
        }
    }

    /// <summary>
    /// The corner's one card, by rank: the stall prompt, the Undo note, Up next, the Skip button.
    /// A change fades the old card out and the new one in (standard) and announces the new one once.
    /// </summary>
    void UpdateCorner()
    {
        UIElement? want = stallShown ? StallCard : skipPrompt is not null && promptCard is not null ? promptCard : undo is not null ? UndoCard : upNextShown ? UpNextCard : skipShown ? SkipSlot : null;
        if (want == cornerCard)
            return;
        if (cornerCard is not null)
            Motion.HideStandard(cornerCard);
        cornerCard = want;
        if (want is null)
            return;
        if (want == SkipSlot)
        {
            skipSince.Restart();
            SkipCard.Opacity = 1;
        }
        Motion.ShowStandard(want);
        if (want == UpNextCard && DeviceWatch.ScreenReaderRunning())
            DispatcherQueue.TryEnqueue(() => UpNextCancel.Focus(FocusState.Programmatic));
        Announce(want == promptCard ? promptCard! : want == StallCard ? StallLine : want == UndoCard ? UndoLine : want == UpNextCard ? UpNextTitle : SkipCard);
    }

    /// <summary>Has a screen reader say what an element now says, once.</summary>
    static void Announce(UIElement element) =>
        FrameworkElementAutomationPeer.CreatePeerForElement(element)?.RaiseAutomationEvent(AutomationEvents.LiveRegionChanged);

    /// <summary>What a key did, while the controls stay hidden: 1.2 s in the middle of the video.</summary>
    void ShowFlash(string text)
    {
        FlashText.Text = text;
        Motion.Show(Flash);
        Announce(FlashText);
        flash.Stop();
        flash.Start();
    }

    // The starting screen is in PlayerView.Starting.cs.

    /// <summary>The corner's clocks, four times a second: the stall count, the Undo window, Skip's dim.</summary>
    void CornerClock()
    {
        if (stallShown)
            StallLine.Text = T.PlayerStallTitle((long)(stallBase + (ulong)stallSince.Elapsed.TotalSeconds));
        if (undo is { } u)
        {
            if (paused)
                u.Waited += 0.25;
            else
                u.Played += 0.25;
            var scale = NoticeTime.Scale(DeviceWatch.ScreenReaderRunning());
            if (u.Played >= 6 * scale || u.Waited >= 15 * scale)
            {
                undo = null;
                UpdateCorner();
            }
        }
        if (cornerCard == SkipSlot)
            SkipCard.Opacity = skipHover || skipSince.Elapsed < TimeSpan.FromSeconds(8) ? 1 : 0.7;
    }

    void OnSkipEntered(object sender, PointerRoutedEventArgs e) => skipHover = true;

    void OnSkipExited(object sender, PointerRoutedEventArgs e)
    {
        skipHover = false;
        skipSince.Restart();
    }

    void OnUndo(object sender, RoutedEventArgs e) => Undo();

    void Undo()
    {
        if (undo is not { } u)
            return;
        undo = null;
        SendControl(new PlayerCommand.UndoSkip(u.To));
        UpdateCorner();
    }

    // The seek bar's track: silk at a quarter for the length, at under half for what is
    // buffered, amber for what has played, and water on top for the opening and ending, so the
    // marks still read once played past (native-ux.md, the timeline).

    TimelineMark[] timelineMarks = [];
    uint playingId;
    TimeRange[] buffered = [];

    void OnTimelineSized(object sender, SizeChangedEventArgs e)
    {
        DrawTimeline();
        // The thumb ends a drag that began on it, whose release the line may not hear.
        if (!thumbHooked && Find(Timeline, "HorizontalThumb") is Thumb thumb)
        {
            thumbHooked = true;
            thumb.DragCompleted += (_, _) => EndScrub();
        }
    }

    bool thumbHooked;

    void DrawTimeline()
    {
        TimelineUnder.Children.Clear();
        tickPlaces.Clear();
        if (Find(Timeline, "HorizontalTrackRect") is not FrameworkElement track || track.ActualWidth <= 0)
            return;
        var thumb = Find(Timeline, "HorizontalThumb") as FrameworkElement;
        var at = track.TransformToVisual(TimelineUnder).TransformPoint(default);
        var width = track.ActualWidth;
        var height = Math.Max(2, track.ActualHeight);
        // The slider's value runs between the thumb's centre at either end.
        var half = (thumb?.ActualWidth ?? 0) / 2;
        var usable = Math.Max(1, width - 2 * half);
        double X(double seconds) => at.X + half + Math.Clamp(duration > 0 ? seconds / duration : 0, 0, 1) * usable;
        var silk = (Brush)Application.Current.Resources["SilkBrush"];
        var water = (Brush)Application.Current.Resources["WaterBrush"];
        void Bar(double from, double to, Brush brush, double opacity)
        {
            if (to - from < 1)
                return;
            var bar = new Microsoft.UI.Xaml.Shapes.Rectangle
            {
                Width = to - from, Height = height, Fill = brush, Opacity = opacity,
                RadiusX = height / 2, RadiusY = height / 2,
            };
            Canvas.SetLeft(bar, from);
            Canvas.SetTop(bar, at.Y);
            TimelineUnder.Children.Add(bar);
        }
        Bar(at.X, at.X + width, silk, 0.25);
        playedBar = null;
        if (duration <= 0)
            return;
        foreach (var range in buffered)
            Bar(X(range.Start), X(range.End), silk, 0.45);
        // What has played, from the track's start to the thumb, kept to move with the value.
        playedBar = new Microsoft.UI.Xaml.Shapes.Rectangle
        {
            Height = height, Fill = (Brush)Application.Current.Resources["AmberBrush"],
            RadiusX = height / 2, RadiusY = height / 2,
        };
        Canvas.SetLeft(playedBar, at.X);
        Canvas.SetTop(playedBar, at.Y);
        TimelineUnder.Children.Add(playedBar);
        playedFrom = at.X;
        playedTo = X;
        ShowPlayed();
        // The opening and ending go on top, so they read wherever the viewer is.
        foreach (var mark in timelineMarks)
            Bar(X(mark.Start), X(mark.End), water, 1);
        // The stretch being marked grows at half water until it is saved.
        if (ProvisionalMark() is { } mine)
            Bar(X(mine.From), X(mine.To), water, 0.5);
        // The party's reactions, as ticks above the line.
        DrawTicks(X, at.Y, usable);
    }

    Microsoft.UI.Xaml.Shapes.Rectangle? playedBar;
    double playedFrom;
    Func<double, double> playedTo = _ => 0;

    void ShowPlayed()
    {
        if (playedBar is not null)
            playedBar.Width = Math.Max(0, playedTo(Timeline.Value) - playedFrom);
    }

    static DependencyObject? Find(DependencyObject parent, string name)
    {
        for (var i = 0; i < VisualTreeHelper.GetChildrenCount(parent); i++)
        {
            var child = VisualTreeHelper.GetChild(parent, i);
            if (child is FrameworkElement { Name: var n } && n == name)
                return child;
            if (Find(child, name) is { } found)
                return found;
        }
        return null;
    }

    void ShowState(FfiPlayerState state)
    {
        if (state.HasPicture && startingShown && !startingHeld)
        {
            // The first frame fades in over the starting screen.
            startingShown = false;
            EndStart();
            Motion.Hide(Starting);
        }
        if (state.HasPicture && !pictureSeen)
        {
            pictureSeen = true;
            FirstPicture();
        }
        // A press shows at once (TogglePause); mpv's word settles it once it agrees, or after 1 s.
        if (pauseAsked is { } asked && (asked == state.Paused || pauseSince.Elapsed > TimeSpan.FromSeconds(1)))
            pauseAsked = null;
        ShowPaused(pauseAsked ?? state.Paused);
        if (paused)
            ShowControls();
        else if (chromeShown && !idle.IsRunning)
            idle.Start();
        if (state.Position is { } p)
            position = p;
        if (state.Volume is { } level)
            TookVolume(level);
        if (state.Duration is { } d && d > 0 && d != duration)
        {
            duration = d;
            DrawTimeline();
        }
        if (state.Buffering && pictureSeen && !startingShown)
        {
            if (!bufferingSince.IsRunning)
                bufferingSince.Restart();
        }
        else
            bufferingSince.Reset();
        ShowBuffering();
        // A seek asked for holds the line where it was sent until mpv is there, or for 3 s.
        if (seekTarget is { } target && (seekSince.Elapsed > TimeSpan.FromSeconds(3) || !state.Seeking && Math.Abs(position - target) < 1.5))
            seekTarget = null;
        ShowClocks();
        ShowTracks(state.Tracks);
        ShowUpNext();
        fromPlayer = true;
        Timeline.Maximum = Math.Max(1, duration);
        // While the keyboard is on the slider the thumb is left to the person, so the screen
        // reader does not hear it change under them. A jump of more than a few seconds still lands.
        // While dragged, the thumb is the pointer's.
        var shown = Shown;
        if (scrubbing is null && (Timeline.FocusState != FocusState.Keyboard || Math.Abs(Timeline.Value - shown) > 5))
            Timeline.Value = Math.Min(shown, Timeline.Maximum);
        fromPlayer = false;
    }

    readonly Stopwatch bufferingSince = new();
    bool bufferingShown;

    /// <summary>The ring for a wait on data, half a second in, while no stall card says more.</summary>
    void ShowBuffering()
    {
        var on = bufferingSince.IsRunning && bufferingSince.Elapsed > TimeSpan.FromMilliseconds(500) && !stallShown && Showing;
        if (on == bufferingShown)
            return;
        bufferingShown = on;
        BufferingRing.IsActive = on;
        if (on)
            Motion.Show(BufferingRing);
        else
            Motion.Hide(BufferingRing);
    }

    /// <summary>The time the line and the clocks show: the drag's, a seek on its way, or mpv's.</summary>
    double Shown => scrubbing ?? seekTarget ?? position;

    void ShowClocks()
    {
        var shown = Shown;
        Elapsed.Text = KaeriFfiMethods.PlayClock(shown);
        Remaining.Text = "-" + KaeriFfiMethods.PlayClock(Math.Max(0, duration - shown));
        var total = KaeriFfiMethods.PlayClock(duration);
        var status = paused ? T.PlayerTimelinePaused(Elapsed.Text, total) : T.PlayerTimelinePlaying(Elapsed.Text, total);
        if (status != timelineStatus)
        {
            timelineStatus = status;
            AutomationProperties.SetItemStatus(Timeline, status);
        }
    }

    // The swapchain and the panel's size.

    void Tick()
    {
        EndScrubIfLetGo();
        CornerClock();
        ShowBuffering();
        if (marking is { Step: SkipMarkStep.End })
            DrawTimeline();
        // The panel's timed states: the dashes, the waiting line, "Copied" going.
        if (stats.Shown)
            ShowStats();
        if (player is not { } asked)
            return;
        swapchainAsk ??= new SwapchainAsk(DispatcherQueue);
        swapchainAsk.Ask(asked, swapchain =>
        {
            // The answer for a player since closed or replaced is not this one's.
            if (player != asked || swapchain == bound)
                return;
            try
            {
                Dxgi.SetSwapChain(VideoPanel, swapchain);
                var scale = VideoPanel.CompositionScaleX;
                if (Math.Abs(scale - 1) > 0.001)
                    Dxgi.SetInverseScale(swapchain, VideoPanel.CompositionScaleX, VideoPanel.CompositionScaleY);
                bound = swapchain;
            }
            catch (COMException e)
            {
                KaeriFfiMethods.LogLine(FfiLogLevel.Warn, "windows", $"binding mpv's swapchain failed: {e.Message}");
            }
        });
    }

    SwapchainAsk? swapchainAsk;

    (uint W, uint H, double Scale)? panelAsked, panelSent;
    Microsoft.UI.Dispatching.DispatcherQueueTimer? panelPace;

    /// <summary>
    /// A drag of the window's edge sends a size for every step of it, and each one makes mpv
    /// rebuild its swapchain and draw again. The first size goes at once, then at most one
    /// every 50 ms, and the last always; between them the panel stretches the last frame.
    /// </summary>
    void OnPanelSizeChanged(object sender, SizeChangedEventArgs e)
    {
        if (player is null || e.NewSize.Width < 1 || e.NewSize.Height < 1)
            return;
        var scale = XamlRoot?.RasterizationScale ?? 1.0;
        panelAsked = ((uint)Math.Max(1, Math.Round(e.NewSize.Width * scale)), (uint)Math.Max(1, Math.Round(e.NewSize.Height * scale)), scale);
        if (panelPace is null)
        {
            panelPace = DispatcherQueue.CreateTimer();
            panelPace.Interval = TimeSpan.FromMilliseconds(50);
            panelPace.IsRepeating = false;
            panelPace.Tick += (_, _) => SendPanelSize();
        }
        if (panelPace.IsRunning)
            return;
        SendPanelSize();
    }

    void SendPanelSize()
    {
        if (player is null || panelAsked is not { } size || size == panelSent)
            return;
        panelSent = size;
        player.CompositionSize(size.W, size.H);
        player.Display(size.W, size.H, size.Scale, null);
        LiftSubtitles(animated: false);
        panelPace?.Start();
    }

    // Showing and hiding the controls.

    void ShowControls()
    {
        ShowPointer();
        if (still.Asking)
            return;
        if (!chromeShown)
        {
            chromeShown = true;
            Motion.Show(Chrome);
            LiftSubtitles();
            ControlsChanged?.Invoke(true);
        }
        idle.Stop();
        if (!paused)
            idle.Start();
    }

    /// <summary>The controls fade out (fade-out), then collapse so the video scans out alone.</summary>
    void HideChrome()
    {
        if (!chromeShown)
            return;
        chromeShown = false;
        Motion.Hide(Chrome);
        // An empty field goes with the controls.
        if (chatField is { } field && field.Box.Text.Length == 0)
            CloseChatField();
        TickLabel.Visibility = Visibility.Collapsed;
        LiftSubtitles();
        PreviewBox.Visibility = Visibility.Collapsed;
        ControlsChanged?.Invoke(false);
    }

    void HideControlsIfIdle()
    {
        if (keepControls || paused || overBar || menuOpen || Marking)
            return;
        if (ChatFieldHolds())
        {
            idle.Start();
            return;
        }
        if (ControlsInUse())
        {
            // A screen reader is running, or a control has focus by any means: hold them.
            idle.Start();
            return;
        }
        HideChrome();
        HidePointer();
    }

    /// <summary>
    /// Whether the controls must stay: a screen reader runs (its cursor moves without focus, and
    /// controls that collapse leave the tree), the seek line is being dragged, or the keyboard is
    /// on a control. A control that only has focus from a click does not hold them.
    /// </summary>
    bool ControlsInUse()
    {
        if (DeviceWatch.ScreenReaderRunning() || scrubbing is not null || volumeDragging)
            return true;
        if (XamlRoot is null || FocusManager.GetFocusedElement(XamlRoot) is not Control { FocusState: FocusState.Keyboard } focused)
            return false;
        for (DependencyObject? at = focused; at is not null; at = VisualTreeHelper.GetParent(at))
        {
            if (at == Chrome)
                return true;
        }
        return false;
    }

    [LibraryImport("user32.dll")]
    private static partial uint GetDoubleClickTime();

    [LibraryImport("user32.dll")]
    private static partial int ShowCursor([MarshalAs(UnmanagedType.Bool)] bool show);

    /// <summary>The pointer goes with the controls when nobody is using it.</summary>
    void HidePointer()
    {
        // Not over the starting screen or a failure, where there are buttons to reach.
        if (pointerHidden || !Showing || startingShown)
            return;
        pointerHidden = true;
        while (ShowCursor(false) >= 0)
        {
            // The count falls below zero and the pointer is gone.
        }
    }

    void ShowPointer()
    {
        if (!pointerHidden)
            return;
        pointerHidden = false;
        while (ShowCursor(true) < 0)
        {
            // Back to zero or above: the pointer is shown again.
        }
    }

    /// <summary>
    /// Any movement shows the controls; over a bar they stay. Where the pointer is comes from its
    /// position on every move, handled ones included: a bar's PointerExited does not come after a
    /// click on one of its buttons, which left the controls up for good.
    /// </summary>
    void OnPointerMoved(object sender, PointerRoutedEventArgs e)
    {
        overBar = chromeShown && (Under(TopBar, e) || Under(BottomBar, e));
        ShowControls();
    }

    static bool Under(FrameworkElement bar, PointerRoutedEventArgs e)
    {
        var at = e.GetCurrentPoint(bar).Position;
        return at.X >= 0 && at.Y >= 0 && at.X < bar.ActualWidth && at.Y < bar.ActualHeight;
    }

    void OnRootExited(object sender, PointerRoutedEventArgs e) => overBar = false;
    void OnMenuOpened(object? sender, object e) => menuOpen = true;

    // Watch party.

    PartyPanel? partyPanel;

    void OnParty(object sender, RoutedEventArgs e) => ShowParty();

    /// <summary>The party's panel when in one; the form that starts one when not.</summary>
    internal void ShowParty()
    {
        if (ctx is null)
            return;
        if (ctx.Known.Party is null)
        {
            ctx.Window.ShowParty(new PartyDialog.For.Playing());
            return;
        }
        if (partyPanel is null)
        {
            partyPanel = new PartyPanel(ctx);
            partyPanel.Flyout.Opened += OnMenuOpened;
            partyPanel.Flyout.Closed += OnMenuClosed;
            partyPanel.Flyout.Opened += (_, _) => ChatPopover(true);
            partyPanel.Flyout.Closed += (_, _) => ChatPopover(false);
        }
        ShowControls();
        partyPanel.Flyout.ShowAt(PartyButton);
    }

    /// <summary>Play on TV, in the big player only, once the engine gives a sender.</summary>
    void ShowTvButton() =>
        TvButton.Visibility = ctx?.Tv.Available == true && !mini ? Visibility.Visible : Visibility.Collapsed;

    /// <summary>
    /// Play on TV from here: paused here, then the TV to choose, or straight to the one driven
    /// already. Once the TV plays it the window closes the player.
    /// </summary>
    void OnPlayOnTv(object sender, RoutedEventArgs e)
    {
        if (ctx is not { } c || playingId == 0 || playingEpisode.Length == 0)
            return;
        if (!paused)
            Send(new PlayerCommand.TogglePause());
        c.Tv.Offer(new TvPlay(playingId, ShowTitle.Text, playingEpisode, position > 5 ? position : null, FromPlayer: true));
    }

    static string Skipped(SkipKind kind) => kind switch { SkipKind.Ending => T.PlayerSkippedEnding, SkipKind.Recap => T.PlayerSkippedRecap, _ => T.PlayerSkippedOpening };

    /// <summary>The buttons' tooltips, each with its key: "Episodes (E)".</summary>
    void ToolTips()
    {
        ToolTipService.SetToolTip(BackButton, T.TooltipWithKey(T.ActionBack, "Alt+Left"));
        ToolTipService.SetToolTip(TracksButton, T.TooltipWithKey(T.PlayerSubtitlesAndAudio, "T"));
        ToolTipService.SetToolTip(EpisodesButton, T.TooltipWithKey(T.PlayerEpisodes, "E"));
        ToolTipService.SetToolTip(MiniButton, T.TooltipWithKey(T.PlayerMiniPlayer, "P"));
        ToolTipService.SetToolTip(FullscreenButton, T.TooltipWithKey(T.PlayerFullscreen, "F"));
        ToolTipService.SetToolTip(PlayPause, T.TooltipWithKey(T.ActionPause, "Space"));
        ToolTipService.SetToolTip(Back10Button, T.TooltipWithKey(T.PlayerBack10, "J"));
        ToolTipService.SetToolTip(Forward10Button, T.TooltipWithKey(T.PlayerForward10, "L"));
        ToolTipService.SetToolTip(StatsClose, T.TooltipWithKey(T.ActionClose, "I"));
        ToolTipService.SetToolTip(UndoButton, T.TooltipWithKey(T.ActionUndo, "U"));
        MiniTip.Subtitle = T.PlayerMiniTip("P");
    }

    void ShowPartyButton()
    {
        var inParty = ctx?.Known.Party is not null;
        PartyDot.Opacity = inParty ? 1 : 0;
        ToolTipService.SetToolTip(PartyButton, T.PartyTitle);
        AutomationProperties.SetName(PartyButton, T.PartyTitle);
    }

    void OnMenuClosed(object? sender, object e)
    {
        menuOpen = false;
        ShowControls();
    }

    void OnVideoTapped(object sender, TappedRoutedEventArgs e)
    {
        if (e.OriginalSource != VideoPanel && e.OriginalSource != Chrome && e.OriginalSource != Starting)
            return;
        if (e.PointerDeviceType == Microsoft.UI.Input.PointerDeviceType.Touch)
        {
            // Touch: a tap toggles the controls rather than pausing.
            if (chromeShown)
                HideChrome();
            else
                ShowControls();
            return;
        }
        // A click pauses after the double-click interval, so a double-click is fullscreen only.
        click.Stop();
        click.Start();
    }

    void OnVideoDoubleTapped(object sender, DoubleTappedRoutedEventArgs e)
    {
        click.Stop();
        if (e.OriginalSource == VideoPanel || e.OriginalSource == Chrome)
            FullscreenRequested?.Invoke(null);
    }

    void OnWheel(object sender, PointerRoutedEventArgs e)
    {
        // A scroll over a panel or a card scrolls it, or nothing: never the volume or the position.
        if (e.OriginalSource is DependencyObject source && Over(source))
            return;
        // Volume 2% a notch; a horizontal scroll seeks 0.25 s a step, gathered into whole steps.
        var point = e.GetCurrentPoint(this);
        var notches = point.Properties.MouseWheelDelta / 120.0;
        if (point.Properties.IsHorizontalMouseWheel)
        {
            wheelSeek += notches * 2.5;
            var by = Math.Truncate(wheelSeek / 0.25) * 0.25;
            if (by != 0)
            {
                wheelSeek -= by;
                SeekBy(by);
            }
        }
        else
        {
            StepVolume(notches * 2);
        }
        e.Handled = true;
    }

    /// <summary>Whether <paramref name="source"/> is in one of the panels or cards over the video.</summary>
    bool Over(DependencyObject source)
    {
        for (var at = source; at is not null; at = VisualTreeHelper.GetParent(at))
        {
            if (at == StatsLayer || at == Starting || at == StillLayer || at == Corner || at == ChatColumn || at == ChatFieldSlot
                || discussion is not null && at == discussion)
                return true;
            if (at == VideoPanel || at == this)
                return false;
        }
        return false;
    }

    // The controls.

    void OnClose(object sender, RoutedEventArgs e) => Close();

    internal void Close()
    {
        if (findingFor is not null)
        {
            // Nothing opened yet: what is being found stops, and nothing the engine says brings
            // the player back until Play is pressed again (as Apple's viewerClosed).
            findingFor = null;
            closedWhileFinding = true;
            Send(new PlayerCommand.Stop());
            Hide();
            return;
        }
        if (!closedByHost)
            player?.ViewerClosed();
        if (stagedAhead)
        {
            // The file on screen is closed; the one staged since is still being found, and
            // would open the player again once found (as Apple's viewerClosed).
            stagedAhead = false;
            closedWhileFinding = true;
            Send(new PlayerCommand.Stop());
        }
        Hide();
    }

    /// <summary>The viewer closed the starting screen before a source was found.</summary>
    bool closedWhileFinding;

    /// <summary>A newer playback has been staged since the one on screen and has not spoken yet.</summary>
    bool stagedAhead;

    /// <summary>
    /// mpv's window was closed by the engine before a first frame: the start failed, and its
    /// failure, which comes just after, keeps the starting screen up to say so.
    /// </summary>
    bool closedByHost;
    DispatcherQueueTimer? verdictWait;

    /// <summary>
    /// The player closed under a start that never showed a picture: the screen stays a moment
    /// for the engine's verdict, a failure it then shows, instead of hiding and losing it.
    /// </summary>
    void HostClosed()
    {
        if (!Showing || start is null || pictureSeen || start.Failed)
        {
            Hide();
            return;
        }
        closedByHost = true;
        if (verdictWait is null)
        {
            verdictWait = DispatcherQueue.CreateTimer();
            verdictWait.Interval = TimeSpan.FromSeconds(2);
            verdictWait.IsRepeating = false;
            verdictWait.Tick += (_, _) =>
            {
                // No failure came, nor another start: nothing to say over.
                if (closedByHost && start is not { Failed: true })
                    Hide();
            };
        }
        verdictWait.Stop();
        verdictWait.Start();
    }

    void OnPlayPause(object sender, RoutedEventArgs e) => TogglePause();
    void OnBack10(object sender, RoutedEventArgs e) => SeekBy(-10, flash: false);
    void OnForward10(object sender, RoutedEventArgs e) => SeekBy(10, flash: false);
    void OnFullscreen(object sender, RoutedEventArgs e) => FullscreenRequested?.Invoke(null);
    void OnSkip(object sender, RoutedEventArgs e) => SendControl(new PlayerCommand.TakeSkip());
    void OnPlayNext(object sender, RoutedEventArgs e) => PlayNext(auto: false);
    void OnCancelNext(object sender, RoutedEventArgs e) => CancelNext();

    /// <summary>
    /// Up next as the engine hands it over: the card shows from where the ending starts (or near
    /// the end of the file when that is not known), and its countdown runs on the playhead from
    /// there; at zero this sends PlayNext, once. A seek that lands in or past the countdown runs
    /// it again from its full length there, rather than playing the next episode at once.
    /// </summary>
    sealed record Coming(string Number, bool Auto, double? EndingAt, uint Countdown)
    {
        public bool Cancelled { get; set; }
        public bool Played { get; set; }
        public double? RestartedAt { get; set; }
        /// <summary>The viewer sought since the card came: a roll-on is then theirs, not one nobody watched.</summary>
        public bool Sought { get; set; }

        public double? ShowsAt(double duration)
        {
            if (EndingAt is { } at)
                return at;
            if (duration <= 0)
                return null;
            return duration > 120 ? duration - 30 : Math.Max(0, duration - (Countdown > 0 ? Countdown : 10));
        }

        public double? Remaining(double position, double duration)
        {
            if (!Auto || Cancelled || Countdown == 0 || ShowsAt(duration) is not { } shows)
                return null;
            var start = Math.Max(shows, RestartedAt ?? shows);
            return position < start ? null : Math.Max(0, Countdown - (position - start));
        }
    }

    /// <summary>The thumbnail Up next shows, so the same one said again does not blank it.</summary>
    string? upNextStill;

    bool InParty => ctx?.Known.Party is not null;

    /// <summary>In a party whose room moves on at the end: everyone goes on together, so nobody's player plays the next episode itself.</summary>
    bool RoomLeads => InParty && ctx?.Known.Party?.MovesOn == true;

    /// <summary>A seek reached mpv, the viewer's or the engine's: a countdown it lands in or past starts again there.</summary>
    void Sought(double target)
    {
        if (upNext is not { Played: false } card || card.ShowsAt(duration) is not { } start)
            return;
        card.RestartedAt = target >= start ? target : null;
    }

    void ShowUpNext()
    {
        var card = upNext;
        var leads = RoomLeads;
        UpNextButtons.Visibility = leads ? Visibility.Collapsed : Visibility.Visible;
        var shown = card is { Cancelled: false } && card.ShowsAt(duration) is { } at && Shown >= at;
        upNextShown = shown;
        UpdateCorner();
        if (!shown || card is null)
            return;
        if (leads)
        {
            // The room moves on when the file ends, for everyone at once: the card says so and
            // counts the last seconds of the file down.
            var span = (double)Math.Max(10, card.Countdown);
            var toEnd = duration - Shown;
            var counting = duration > 0 && toEnd <= span;
            UpNextBar.Visibility = counting ? Visibility.Visible : Visibility.Collapsed;
            UpNextLine.Visibility = Visibility.Visible;
            UpNextLine.Text = counting ? T.PartyNextLine((long)Math.Ceiling(Math.Max(0, toEnd))) : T.PartyNextOff;
            if (counting)
                UpNextFill.ScaleX = Math.Clamp(toEnd / span, 0, 1);
            return;
        }
        // With a screen reader running the card waits for a choice instead of counting down, and
        // so does it in a party the room doesn't move on: Play now picks the episode for the room.
        // On the time shown, so a seek on its way holds the line full rather than hiding it.
        var left = DeviceWatch.ScreenReaderRunning() || InParty ? null : card.Remaining(Shown, duration);
        UpNextBar.Visibility = left is null ? Visibility.Collapsed : Visibility.Visible;
        UpNextLine.Visibility = UpNextBar.Visibility;
        if (left is { } l)
        {
            UpNextFill.ScaleX = Math.Clamp(l / card.Countdown, 0, 1);
            UpNextLine.Text = T.PlayerPlaysIn((long)Math.Ceiling(l));
            // Not while the viewer drags the line, whose keyframe seeks move the playhead, nor
            // while a seek is on its way.
            if (l <= 0 && scrubbing is null && seekTarget is null)
                PlayNext(auto: true);
        }
    }

    void PlayNext(bool auto)
    {
        if (upNext is { Played: true } || RoomLeads)
            return;
        if (upNext is not null)
            upNext.Played = true;
        if (InParty)
        {
            // The room's pick, for everyone: the lobby has it.
            if (upNext is { } card)
                PlayInRoom(card.Number);
            return;
        }
        if (auto && upNext is not { Sought: true })
        {
            movingOn = true;
            still.RollingOn();
            SendControl(new PlayerCommand.PlayNext(), quiet: true);
        }
        else
        {
            movingOn = auto;
            SendControl(new PlayerCommand.PlayNext());
        }
    }

    /// <summary>
    /// Another episode, asked for in the player while in a party: the room's pick, as Play is
    /// everywhere in a party, never a play of this device's own. The player gives way to the
    /// lobby, where the room starts it once everyone is ready. The room's own episode is already
    /// playing here.
    /// </summary>
    void PlayInRoom(string episode)
    {
        if (ctx is not { } c || playingId == 0 || episode == playingEpisode)
            return;
        still.Input();
        c.Send(new Command.Play(playingId, episode, false));
        Close();
        c.Window.Go("together");
    }

    /// <summary>N and Shift+N, the media keys' Next and Previous: the next or previous episode in the player's list.</summary>
    void Step(int by)
    {
        if (!InParty)
        {
            SendControl(new PlayerCommand.Step(by));
            return;
        }
        var list = menus?.Episodes ?? [];
        var at = Array.FindIndex(list, e => e.Current);
        if (at < 0)
            at = Array.FindIndex(list, e => e.Number == playingEpisode);
        if (at >= 0 && at + by >= 0 && at + by < list.Length)
            PlayInRoom(list[at + by].Number);
    }

    /// <summary>An episode picked in the Episodes panel.</summary>
    void ChooseEpisode(string number)
    {
        if (InParty)
            PlayInRoom(number);
        else
            SendControl(new PlayerCommand.Choose(new PlayerChoice.Episode(number)));
    }

    /// <summary>What the media keys and the taskbar's buttons ask, for the playback on screen.</summary>
    internal void MediaCommand(PlayerCommand command)
    {
        if (command is PlayerCommand.Step step)
            Step((int)step.V1);
        else if (command is PlayerCommand.TogglePause && Showing)
            TogglePause();
        else
            SendControl(command);
    }

    void CancelNext()
    {
        if (upNext is not null)
            upNext.Cancelled = true;
        ShowUpNext();
        SendControl(new PlayerCommand.CancelNext());
    }
    void OnStallWait(object sender, RoutedEventArgs e) => StallAnswer(new PlayerCommand.StallWait());
    void OnStallSwitch(object sender, RoutedEventArgs e) => StallAnswer(new PlayerCommand.StallSwitch());

    /// <summary>The stall card answered: it goes at once, as on the Mac; the engine says it again while the stall holds.</summary>
    void StallAnswer(PlayerCommand answer)
    {
        stallShown = false;
        UpdateCorner();
        SendControl(answer);
    }

    internal void ShowFullscreen(bool on)
    {
        FullscreenIcon.Glyph = on ? "" : "";
        var name = on ? T.PlayerExitFullscreen : T.PlayerFullscreen;
        AutomationProperties.SetName(FullscreenButton, name);
        ToolTipService.SetToolTip(FullscreenButton, T.TooltipWithKey(name, "F"));
    }

    /// <summary>A time said in words for a screen reader: "12 minutes 4 seconds".</summary>
    static string SpokenClock(double seconds)
    {
        var total = (long)Math.Max(0, Math.Floor(seconds));
        var parts = new List<string>(3);
        if (total / 3600 > 0)
            parts.Add(T.PlayerSpokenHours(total / 3600));
        if (total % 3600 / 60 > 0)
            parts.Add(T.PlayerSpokenMinutes(total % 3600 / 60));
        if (total % 60 > 0 || parts.Count == 0)
            parts.Add(T.PlayerSpokenSeconds(total % 60));
        return string.Join(" ", parts);
    }

    void OnTimelineChanged(object sender, RangeBaseValueChangedEventArgs e)
    {
        ShowPlayed();
        if (fromPlayer)
            return;
        if (scrubbing is not null)
        {
            Scrub(e.NewValue);
            return;
        }
        // A press on the line moves the value before the press reaches this view: whether it
        // starts a drag is known once the press has been handled. A key or a screen reader's
        // change seeks exactly.
        if (timelineSettling)
            return;
        timelineSettling = true;
        DispatcherQueue.TryEnqueue(DispatcherQueuePriority.Low, () =>
        {
            timelineSettling = false;
            if (scrubbing is null && Showing && Math.Abs(Timeline.Value - Shown) > 1)
                SeekTo(Timeline.Value);
        });
    }

    void OnVolumeChanged(object sender, RangeBaseValueChangedEventArgs e)
    {
        if (fromPlayer)
            return;
        var to = Math.Round(e.NewValue);
        if (Math.Abs(to - volume) >= 1)
            StepVolume(to - volume, flash: false);
    }

    /// <summary>mpv listed the file's tracks again: rebuild the menu when what it offers changed.</summary>
    void ShowTracks(FfiTrack[] now)
    {
#if KAERI_TESTING
        if (tracksHeld)
            return;
#endif
        var seen = string.Join(";", now.Select(t => $"{t.Kind}{t.Id}{(t.Selected ? "*" : "")}{t.Language}{t.Title}"));
        if (seen == tracksSeen)
            return;
        tracksSeen = seen;
        tracks = now;
        BuildTracks();
    }

    /// <summary>
    /// The Tracks menu: Sub or Dub at the top when the show has both, then the file's own
    /// subtitles (Off first) and audio, each by its language and title. A pick goes to the
    /// engine, which applies it and keeps it for the series.
    /// </summary>
    void BuildTracks()
    {
        TracksMenu.Items.Clear();
        // Both, when the source says the show has a dub, or the dub is what plays.
        if (menus is { } m && (m.Translation == Engine.Core.Translation.Dub || ctx?.Known.Dubbed.GetValueOrDefault(playingId) == true))
            foreach (var (translation, label) in new[] { (Engine.Core.Translation.Sub, T.AudioSubbed), (Engine.Core.Translation.Dub, T.AudioDubbed) })
            {
                var item = new RadioMenuFlyoutItem { Text = label, GroupName = "translation", IsChecked = m.Translation == translation };
                item.Click += (_, _) => Send(new PlayerCommand.Choose(new PlayerChoice.Translation(translation)));
                TracksMenu.Items.Add(item);
            }
        var subtitles = tracks.Where(t => t.Kind == TrackKind.Subtitle).ToArray();
        var audio = tracks.Where(t => t.Kind == TrackKind.Audio).ToArray();
        if (subtitles.Length == 0 && audio.Length == 0)
            return;
        if (TracksMenu.Items.Count > 0)
            TracksMenu.Items.Add(new MenuFlyoutSeparator());

        var subs = new MenuFlyoutSubItem { Text = T.PlayerSubtitles };
        var off = new RadioMenuFlyoutItem { Text = T.ValueOff, GroupName = "subtitles", IsChecked = !subtitles.Any(t => t.Selected) };
        off.Click += (_, _) => Send(new PlayerCommand.ChooseTrack(TrackKind.Subtitle, null));
        subs.Items.Add(off);
        foreach (var track in subtitles)
            subs.Items.Add(TrackItem(track, "subtitles"));
        subs.Items.Add(new MenuFlyoutSeparator());
        subs.Items.Add(LoadItem(TrackKind.Subtitle));
        subs.Items.Add(new MenuFlyoutSeparator());
        var styleItem = new MenuFlyoutItem { Text = T.PlayerSubsStyle };
        styleItem.Click += (_, _) => ShowStylePanel();
        subs.Items.Add(styleItem);
        TracksMenu.Items.Add(subs);

        var sound = new MenuFlyoutSubItem { Text = T.PlayerAudio };
        foreach (var track in audio)
            sound.Items.Add(TrackItem(track, "audio"));
        if (audio.Length > 0)
            sound.Items.Add(new MenuFlyoutSeparator());
        sound.Items.Add(LoadItem(TrackKind.Audio));
        TracksMenu.Items.Add(sound);
    }

    SubtitleStylePanel? stylePanel;

    /// <summary>The quick subtitle style, from the subtitles button.</summary>
    void ShowStylePanel()
    {
        if (ctx is null)
            return;
        if (stylePanel is null)
        {
            stylePanel = new SubtitleStylePanel(ctx);
            stylePanel.Flyout.Opened += OnMenuOpened;
            stylePanel.Flyout.Closed += OnMenuClosed;
        }
        ShowControls();
        stylePanel.Flyout.ShowAt(TracksButton);
    }

    MenuFlyoutItem LoadItem(TrackKind kind)
    {
        var item = new MenuFlyoutItem { Text = kind == TrackKind.Subtitle ? T.PlayerLoadSubtitleFile : T.PlayerLoadAudioFile };
        item.Click += (_, _) => LoadTrack(kind);
        return item;
    }

    static readonly string[] SubtitleFiles = [".srt", ".ass", ".ssa", ".vtt", ".sub", ".sup"];
    static readonly string[] AudioFiles = [".mka", ".m4a", ".aac", ".ac3", ".eac3", ".flac", ".mp3", ".ogg", ".opus", ".wav"];

    /// <summary>
    /// Load subtitle file… or Load audio file…: the system's file picker, then mpv adds the file
    /// as a track and selects it. mpv's alone, so the next episode does not carry it. Cancelling
    /// the picker says nothing.
    /// </summary>
    async void LoadTrack(TrackKind kind)
    {
        if (ctx is null || player is not { } p)
            return;
        var subtitle = kind == TrackKind.Subtitle;
        string? path;
        try
        {
            var picker = new Microsoft.Windows.Storage.Pickers.FileOpenPicker(ctx.Window.AppWindow.Id)
            {
                CommitButtonText = subtitle ? T.PlayerLoadSubtitlesCommit : T.PlayerLoadAudioCommit,
                SuggestedStartLocation = Microsoft.Windows.Storage.Pickers.PickerLocationId.Downloads,
            };
            foreach (var extension in subtitle ? SubtitleFiles : AudioFiles)
                picker.FileTypeFilter.Add(extension);
            path = (await picker.PickSingleFileAsync())?.Path;
        }
        catch (Exception e)
        {
            KaeriFfiMethods.LogLine(FfiLogLevel.Warn, "windows", $"the file picker failed: {e.Message}");
            Alert(T.PlayerFilePickerFailed);
            return;
        }
        if (path is not { Length: > 0 })
            return;
        try
        {
            // mpv reads the file before it answers: off the UI thread.
            await Task.Run(() => p.LoadTrack(kind, path));
        }
        catch (FfiException e)
        {
            KaeriFfiMethods.LogLine(FfiLogLevel.Warn, "windows", $"mpv did not load the track: {e.Message}");
            Alert(subtitle ? T.PlayerSubtitleFileFailed : T.PlayerAudioFileFailed);
        }
        ShowControls();
    }

    RadioMenuFlyoutItem TrackItem(FfiTrack track, string group)
    {
        var item = new RadioMenuFlyoutItem
        {
            Text = TrackLabel(track),
            GroupName = group,
            IsChecked = track.Selected,
        };
        var (kind, id) = (track.Kind, track.Id);
        item.Click += (_, _) => Send(new PlayerCommand.ChooseTrack(kind, id));
        return item;
    }

    /// <summary>"English · Signs &amp; Songs": the language, then the track's own words when it has any.</summary>
    static string TrackLabel(FfiTrack track)
    {
        var name = Wording.TrackName(track.Language, track.Title ?? "");
        return track.External ? T.PlayerTrackAdded(name) : name;
    }

    EpisodesPanel? episodesPanel;
    bool? ownSpoilers;

    /// <summary>The Episodes panel: compact rows and a footer, from the engine's menu entries and the title's table.</summary>
    void EnsureEpisodesPanel()
    {
        if (episodesPanel is not null)
            return;
        episodesPanel = new EpisodesPanel(
            () => (PanelEpisodes(), ctx?.Known.Episodes.GetValueOrDefault(playingId) ?? [],
                ownSpoilers ?? SettingsCatalog.IsOn(ctx?.Known.Setting(SettingKey.SpoilerProtection))),
            ChooseEpisode);
        episodesPanel.Flyout.Opened += OnMenuOpened;
        episodesPanel.Flyout.Closed += OnMenuClosed;
        EpisodesButton.Flyout = episodesPanel.Flyout;
    }

    void BuildMenus()
    {
        if (menus is null)
            return;
        EnsureEpisodesPanel();
        EpisodesButton.IsEnabled = menus.Episodes.Length > 0;

        BuildTracks();

        SettingsMenu.Items.Clear();
        if (menus.Sources.Length > 0)
        {
            var sources = new MenuFlyoutSubItem { Text = T.PlayerSources };
            foreach (var source in menus.Sources)
            {
                var item = new RadioMenuFlyoutItem { Text = source.Detail is { Length: > 0 } d ? $"{source.Label} · {d}" : source.Label, GroupName = "source", IsChecked = source.Current };
                var id = source.Id;
                item.Click += (_, _) => Send(new PlayerCommand.Choose(new PlayerChoice.Source(id)));
                sources.Items.Add(item);
            }
            SettingsMenu.Items.Add(sources);
        }
        var upscaling = new MenuFlyoutSubItem { Text = T.PlayerUpscaling };
        foreach (var choice in menus.Upscaling.Options ?? [])
        {
            var item = new RadioMenuFlyoutItem { Text = choice.Label, GroupName = "upscaling", IsChecked = choice.Id == menus.Upscaling.Current };
            var id = choice.Id;
            item.Click += (_, _) => Send(new PlayerCommand.Upscaling(id));
            upscaling.Items.Add(item);
        }
        SettingsMenu.Items.Add(upscaling);
        var speed = new MenuFlyoutSubItem { Text = T.PlayerSpeed };
        foreach (var s in new[] { 0.75, 1.0, 1.25, 1.5, 2.0 })
        {
            var item = new MenuFlyoutItem { Text = s == 1 ? T.PlayerSpeedNormal : T.PlayerSpeedValue(Wording.Number(s)) };
            item.Click += (_, _) =>
            {
                this.speed = s;
                Send(new PlayerCommand.SpeedTo(s));
            };
            speed.Items.Add(item);
        }
        SettingsMenu.Items.Add(speed);
        var night = new ToggleMenuFlyoutItem { Text = T.PlayerNightMode, IsChecked = menus.NightMode };
        night.Click += (_, _) => Send(new PlayerCommand.NightMode(night.IsChecked));
        SettingsMenu.Items.Add(night);
        SettingsMenu.Items.Add(new MenuFlyoutSeparator());
        SettingsMenu.Items.Add(StatsItem());
    }

    // The episode's discussion: Discussion in the More menu from 85% of the episode, once the engine found a thread.

    DiscussionPane? discussion;

    static bool Within(DependencyObject element, DependencyObject ancestor)
    {
        for (var at = element; at is not null; at = VisualTreeHelper.GetParent(at))
            if (at == ancestor)
                return true;
        return false;
    }

    void OnSettingsMenuOpening(object? sender, object e)
    {
        AddSkipMenu(SettingsMenu);
        // The item and its separator, both, so no separator is left behind each time.
        foreach (var old in SettingsMenu.Items.Where(i => i.Tag as string == "discussion").ToList())
            SettingsMenu.Items.Remove(old);
        if (ctx is not { } c || !uint.TryParse(playingEpisode, out var episode) || !Discussions.Counted(position, duration)
            || !c.Known.Discussions.ContainsKey((playingId, episode)))
            return;
        var item = new MenuFlyoutItem { Text = T.DiscussionOpen, Tag = "discussion" };
        item.Click += (_, _) => OpenDiscussion(episode);
        SettingsMenu.Items.Insert(0, item);
        if (SettingsMenu.Items.Count > 1 && SettingsMenu.Items[1] is not MenuFlyoutSeparator)
            SettingsMenu.Items.Insert(1, new MenuFlyoutSeparator { Tag = "discussion" });
    }

    /// <summary>Opens the discussion beside the video. Playback pauses and stays paused: the viewer chooses.</summary>
    internal void OpenDiscussion(uint episode)
    {
        if (ctx is not { } c || playingId == 0)
            return;
        discussion?.Close();
        if (!paused)
            Send(new PlayerCommand.TogglePause());
        // The window's caption buttons float over the player; the pane starts below them.
        var pane = new DiscussionPane(c, playingId, episode) { Padding = new Thickness(0, 40, 0, 0) };
        discussion = pane;
        pane.Closed += () =>
        {
            if (discussion != pane)
                return;
            discussion = null;
            Root.Children.Remove(pane);
            if (discussionScrim is not null)
                Root.Children.Remove(discussionScrim);
            discussionScrim = null;
            Root.ColumnDefinitions[1].Width = GridLength.Auto;
            Focus(FocusState.Programmatic);
        };
        Root.Children.Add(pane);
        PlaceDiscussion();
        pane.Start();
    }

    Microsoft.UI.Xaml.Shapes.Rectangle? discussionScrim;

    /// <summary>Beside the video from 1000 epx of window; over it, with a light dismiss, below.</summary>
    void PlaceDiscussion()
    {
        if (discussion is not { } pane)
            return;
        var window = XamlRoot?.Size.Width ?? Root.ActualWidth;
        var over = window > 0 && window < DiscussionPane.OverlayBelow;
        if (over)
        {
            Grid.SetColumn(pane, 0);
            Grid.SetColumnSpan(pane, 2);
            if (discussionScrim is null)
            {
                discussionScrim = new Microsoft.UI.Xaml.Shapes.Rectangle { Fill = (Brush)Application.Current.Resources["ScrimBrush"] };
                Grid.SetColumnSpan(discussionScrim, 2);
                discussionScrim.Tapped += (_, e) =>
                {
                    e.Handled = true;
                    discussion?.Close();
                };
                Root.Children.Insert(Root.Children.IndexOf(pane), discussionScrim);
            }
        }
        else
        {
            if (discussionScrim is not null)
                Root.Children.Remove(discussionScrim);
            discussionScrim = null;
            Grid.SetColumn(pane, 1);
            Grid.SetColumnSpan(pane, 1);
        }
    }

    // Keys: arrows and S act without showing the controls; every other key shows them.

    void Key(KeyboardAcceleratorInvokedEventArgs e, Action act, bool show = true)
    {
        if (!Showing)
            return;
        // A dialog over the player takes the keys: Escape closes it, not the player under it.
        if (Dialogs.Open)
            return;
        // Typing in the discussion's field, or pressing a key on one of its buttons, is not a player key.
        if (discussion is { } pane && XamlRoot is { } root && FocusManager.GetFocusedElement(root) is DependencyObject focus && Within(focus, pane))
            return;
        // Nor is typing in the chat's field, over the video or in the people flyout.
        if (Typing())
            return;
        e.Handled = true;
        if (show)
            ShowControls();
        act();
    }

    void OnKeyPause(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, TogglePause);
    void OnKeyBack10(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => SeekBy(-10, flash: false));
    void OnKeyForward10(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => SeekBy(10, flash: false));
    void OnKeyBack5(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => { if (!MarkingNudge(-1)) SeekBy(-5); }, show: false);
    void OnKeyForward5(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => { if (!MarkingNudge(1)) SeekBy(5); }, show: false);
    void OnKeyBack30(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => { if (!MarkingNudge(-5)) SeekBy(-30); }, show: false);
    void OnKeyForward30(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => { if (!MarkingNudge(5)) SeekBy(30); }, show: false);
    void OnKeyVolumeUp(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => StepVolume(5), show: false);
    void OnKeyVolumeDown(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => StepVolume(-5), show: false);
    void OnKeyMute(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, Mute, show: false);
    void OnKeyUndo(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, Undo, show: false);
    void OnKeySpeedNormal(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => SpeedTo(1));
    void OnKeyEpisodes(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => OpenMenu(EpisodesButton));
    void OnKeyTracks(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => OpenMenu(TracksButton));

    /// <summary>A seek by key or scroll, with the time it lands on in the flash.</summary>
    void SeekBy(double seconds, bool flash = true)
    {
        // Nothing to seek in before the file has loaded: the engine would let it go.
        if (duration <= 0 || startingShown)
            return;
        // Named for the playback on screen, so the engine lets a seek go once the next episode
        // has taken over instead of carrying it there.
        var to = Math.Clamp(Shown + seconds, 0, duration);
        SendControl(new PlayerCommand.Seek(seconds));
        if (upNext is not null)
            upNext.Sought = true;
        seekTarget = to;
        seekSince.Restart();
        Sought(to);
        HoldLine();
        if (flash)
            ShowFlash(KaeriFfiMethods.PlayClock(to));
    }

    /// <summary>
    /// M: silence and back, as mpv's own mute (as the Mac's), so the volume kept for the next
    /// session is never 0. With nothing to mute through, the volume goes to 0 and back.
    /// </summary>
    void Mute()
    {
        ulong now;
        lock (loads)
            now = playback;
        if (player is { } p && now != 0)
        {
            still.Input();
            muted = !muted;
            p.Apply(now, [new PlayerOp.Set("mute", new MpvValue.Flag(muted))]);
            ShowMuted();
            ShowFlash(muted ? T.PlayerMuted : T.PlayerVolumeLevel(Wording.Percent(volume)));
            return;
        }
        if (volume > 0)
        {
            mutedFrom = volume;
            StepVolume(-volume);
        }
        else
            StepVolume(mutedFrom > 0 ? mutedFrom : 100);
    }

    /// <summary>mpv's mute, which this player sets; a new mpv starts unmuted.</summary>
    bool muted;

    void ShowMuted()
    {
        var silent = muted || volume <= 0;
        VolumeIcon.Glyph = silent ? "" : "";
        AutomationProperties.SetName(VolumeButton, silent ? T.PlayerVolumeMuted : T.PlayerVolume);
    }

    /// <summary>] and [: a step along the speeds the Speed menu lists.</summary>
    void SpeedStep(int step)
    {
        var at = Array.FindIndex(Speeds, x => x >= speed - 0.01);
        at = at < 0 ? Speeds.Length - 1 : at;
        if (step < 0 && Speeds[at] > speed + 0.01)
            at--;
        SpeedTo(Speeds[Math.Clamp(at + step, 0, Speeds.Length - 1)]);
    }

    void SpeedTo(double to)
    {
        speed = to;
        Send(new PlayerCommand.SpeedTo(to));
        ShowFlash(to == 1 ? T.PlayerSpeedNormal : T.PlayerSpeedValue(Wording.Number(to)));
    }

    void OpenMenu(Button button)
    {
        if (button.Visibility == Visibility.Visible && button.IsEnabled)
            button.Flyout?.ShowAt(button);
    }
    void OnKeyFullscreen(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => FullscreenRequested?.Invoke(null));
    void OnKeySkip(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () =>
    {
        if (skipShown)
            ShowFlash(skipDone);
        Send(new PlayerCommand.TakeSkip());
    }, show: false);
    void OnKeyNext(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => Step(1));
    void OnKeyPrevious(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => Step(-1));
    void OnKeyMini(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => ToggleMini());
    void OnKeyClose(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, Close, show: false);

    void OnKeyEnter(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () =>
    {
        if (still.Asking)
            ContinueWatching();
        else if (MarkingEnter())
            return;
        else if (cornerCard == StallCard)
            StallAnswer(new PlayerCommand.StallSwitch());
        else if (cornerCard == UndoCard)
            Undo();
        else if (cornerCard == UpNextCard && !RoomLeads)
            PlayNext(auto: false);
        else if (cornerCard == SkipSlot)
            SendControl(new PlayerCommand.TakeSkip());
    });

    /// <summary>Esc: cancel Up next or the stall prompt, then leave fullscreen, then close.</summary>
    void OnKeyEscape(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () =>
    {
        if (still.Asking)
            StillStopNow();
        else if (Marking)
            Send(new PlayerCommand.CancelSkipMark());
        else if (discussion is not null)
            discussion.Close();
        else if (mini)
            ToggleMini();
        else if (skipPrompt is not null && cornerCard == promptCard)
            DismissPrompt();
        else if (cornerCard == UpNextCard && !RoomLeads)
            CancelNext();
        else if (cornerCard == StallCard)
            StallAnswer(new PlayerCommand.StallWait());
        else if (FullscreenOn?.Invoke() == true)
            FullscreenRequested?.Invoke(false);
        else
            Close();
    }, show: false);

    // Still watching?

    /// <summary>The new episode has its first picture: pause and ask when nobody has been here for a run of them.</summary>
    void FirstPicture()
    {
        if (still.FirstPicture())
            AskStillWatching();
#if KAERI_TESTING
        else if (ctx?.Options.Show is { } show)
            Demo(show);
#endif
    }

    void AskStillWatching()
    {
        StillLine.Text = T.PlayerStillWatchingDetail(still.After);
        Motion.Show(StillLayer);
        HideChrome();
        ShowPointer();
        if (!paused)
            SendQuietly(new PlayerCommand.TogglePause());
        StillContinue.Focus(FocusState.Programmatic);
    }

    void ContinueWatching()
    {
        still.Input();
        still.Answered();
        Motion.Hide(StillLayer);
        if (paused)
            SendQuietly(new PlayerCommand.TogglePause());
        ShowControls();
    }

    void StillStopNow()
    {
        still.Answered();
        Motion.Set(StillLayer, false);
        Close();
    }

    void OnStillContinue(object sender, RoutedEventArgs e) => ContinueWatching();
    void OnStillStop(object sender, RoutedEventArgs e) => StillStopNow();

    /// <summary>The panel's rows: the player's menu, or on a ranged shot one per row of the table.</summary>
    MenuEpisode[] PanelEpisodes()
    {
#if KAERI_TESTING
        if (ctx?.Options.Show is { } show && show.StartsWith("player-episodes-ranges", StringComparison.Ordinal)
            && ctx.Known.Episodes.GetValueOrDefault(playingId) is { Length: > 100 } table)
            return table.Select(r => new MenuEpisode(r.Number, T.EpisodeNumber(long.Parse(r.Number, System.Globalization.CultureInfo.InvariantCulture)), r.Title, r.Watched >= 0.9, false, r.Number == "230")).ToArray();
#endif
        return menus?.Episodes ?? [];
    }

#if KAERI_TESTING
    /// <summary>
    /// A state to bring up for a screenshot, on the testing build:
    /// <c>--show=still|mini|preview|undo|stall|upnext|skip|starting|flash|tracks|timeline|settings|subtitles|subtitles-mini</c>, and
    /// Stats for nerds as <c>stats</c>, <c>stats-waiting</c>, <c>stats-buffering</c>, <c>stats-hdr</c>,
    /// <c>stats-copied</c> or <c>stats-mini</c>; a <c>.srt</c> beside the clip is loaded for those and for
    /// <c>subtitles</c> and <c>subtitles-mini</c>.
    /// </summary>
    /// <summary>A found skip times state from the engine's fixtures, as <c>--show=found-skip</c> and the rest: its marks, buttons, card, bar and panel over the playing clip, its times scaled to the clip's length.</summary>
    async Task StageFound(string what)
    {
        var state = what switch
        {
            "found-skip" => FoundSkipFixture.Skip,
            "found-controls" => FoundSkipFixture.Controls,
            "found-prompt" => FoundSkipFixture.Prompt,
            "found-saved" => FoundSkipFixture.Saved,
            "found-never" => FoundSkipFixture.Never,
            "found-review" => FoundSkipFixture.Review,
            "found-nudged" => FoundSkipFixture.Nudged,
            "found-undo" => FoundSkipFixture.Undo,
            "found-panel" => FoundSkipFixture.Panel,
            "found-panel-paused" => FoundSkipFixture.PanelPaused,
            var motion when motion.StartsWith("found-motion-", StringComparison.Ordinal) => FoundSkipFixture.Prompt,
            _ => FoundSkipFixture.SettingsOn,
        };
        var motionName = what.StartsWith("found-motion-", StringComparison.Ordinal) ? what["found-motion-".Length..] : null;
        var scene = KaeriFfiMethods.FoundSkipScene(state);
        if (scene.Settings)
            return;
        HoldUpNext = true;
        upNext = null;
        ShowUpNext();
        // The fixture's times, 91 to 180 s and 1290 to 1379 s of a 1420 s episode, sit in the
        // shot's 60 s clip at 10 to 34 s and 44 to 58 s.
        (double At, double In)[] anchors = [(0, 0), (91, 10), (180, 34), (1290, 44), (1379, 58)];
        double Into(double at)
        {
            if (at >= anchors[^1].At)
                return Math.Min(60, anchors[^1].In + (at - anchors[^1].At) * 2 / 41);
            var n = Array.FindLastIndex(anchors, a => a.At <= at);
            var (a0, b0) = (anchors[n], anchors[n + 1]);
            return a0.In + (at - a0.At) * (b0.In - a0.In) / (b0.At - a0.At);
        }
        var nothingYet = scene.Paused is not null;
        timelineMarks = nothingYet ? [] : [.. scene.Marks.Select(m => new TimelineMark(m.Kind, Into(m.Start), Into(m.End), m.Origin))];
        DrawTimeline();
        foreach (var ui in scene.Ui)
            Heard(ui switch
            {
                PlayerUi.Skip skip => new PlayerUi.Skip(skip.Kind, Into(skip.To)),
                PlayerUi.AutoSkipped auto => new PlayerUi.AutoSkipped(auto.Kind, Into(auto.UndoTo)),
                _ => ui,
            });
        // The Undo note stays up for the shot, as --show=undo's does.
        if (undo is not null)
        {
            undo.Played = -600;
            undo.Waited = -600;
        }
        if (scene.Mark is { } view)
        {
            marking = view with { Start = view.Start is { } vs ? Into(vs) : null, End = view.End is { } ve ? Into(ve) : null };
            if (marking.Start is { } ms && marking.End is { } me)
                marking = marking with { Length = me - ms };
            DrawMarking();
        }
        if (scene.Prompt is { } card && motionName is null)
        {
            promptHeld = true;
            ShowPrompt(card);
        }
        if (scene.Controls || scene.Mark is not null)
            ShowControls();
        if (scene.Panel)
        {
            await Task.Delay(300);
            ShowSkipTimes();
        }
        if (motionName is not null && scene.Prompt is { } question)
            await FoundMotion(motionName, question, Into);
    }

    /// <summary>
    /// The found card's motion for a recording: the card comes in, then the engine's answer
    /// replaces its question in place (<c>yes</c>, <c>never</c>), or Adjust closes it for the
    /// review (<c>adjust</c>). It starts when the recording's script drops the go file.
    /// </summary>
    async Task FoundMotion(string name, SkipPrompt question, Func<double, double> into)
    {
        var go = Path.Combine(Path.GetTempPath(), "kaeri-motion-go");
        while (!File.Exists(go))
            await Task.Delay(40);
        await Task.Delay(600);
        promptHeld = false;
        ShowPrompt(question);
        await Task.Delay(2600);
        switch (name)
        {
            case "yes":
                ShowPrompt(KaeriFfiMethods.FoundSkipScene(FoundSkipFixture.Saved).Prompt);
                break;
            case "never":
                ShowPrompt(KaeriFfiMethods.FoundSkipScene(FoundSkipFixture.Never).Prompt);
                break;
            case "adjust":
                ShowPrompt(null);
                if (KaeriFfiMethods.FoundSkipScene(FoundSkipFixture.Review).Mark is { } view)
                {
                    marking = view with { Start = view.Start is { } vs ? into(vs) : null, End = view.End is { } ve ? into(ve) : null };
                    marking = marking with { Length = (marking.End ?? 0) - (marking.Start ?? 0) };
                }
                DrawMarking();
                ShowControls();
                break;
        }
    }

    async void Demo(string what)
    {
        await Task.Delay(1200);
        switch (what)
        {
            case "still":
                still.After = 3;
                AskStillWatching();
                break;
            case "mini":
                ToggleMini();
                break;
            case "player-episodes-ranges":
            case "player-episodes-ranges-menu":
                // 250 episodes in ranges, 230 playing; with -menu, the range menu open on Go to episode.
                ShowControls();
                OpenMenu(EpisodesButton);
                if (what == "player-episodes-ranges-menu")
                {
                    await Task.Delay(900);
                    episodesPanel?.OpenRanges();
                }
                break;
            case "player-episodes":
            case "player-episodes-focus":
                // The Episodes panel open; with -focus, the keyboard then on its second row.
                ShowControls();
                OpenMenu(EpisodesButton);
                if (what == "player-episodes-focus")
                {
                    await Task.Delay(800);
                    episodesPanel?.FocusRow(1);
                }
                break;
            case "discussion-player":
                if (ctx is { } talk && uint.TryParse(playingEpisode, out var talked))
                {
                    talk.Known.Take(new Event.DiscussionFound(playingId, talked, 214));
                    ShowControls();
                    OpenDiscussion(talked);
                }
                break;
            case "preview":
                ShowControls();
                PreviewAtFraction(0.4);
                break;
            case "undo":
                undo = new Undoable(0, T.PlayerSkippedOpening) { Played = -600, Waited = -600 };
                UndoLine.Text = undo.Line;
                UpdateCorner();
                break;
            case "stall":
                stallBase = 9;
                stallSince.Restart();
                stallShown = true;
                UpdateCorner();
                break;
            case "upnext":
                UpNextTitle.Text = T.EpisodeNumber(2);
                UpNextStillFrame.Visibility = Visibility.Visible;
                upNext = new Coming("2", false, position, 0);
                ShowUpNext();
                break;
            case "skip":
                SkipCard.Content = T.PlayerSkipOpening;
                skipShown = true;
                UpdateCorner();
                break;
            case "starting":
                await StartDemo("starting-slow-24");
                break;
            case var m when m.StartsWith("starting-", StringComparison.Ordinal):
                await StartDemo(m);
                break;
            case "tracks":
            case "tracks-sub":
                tracksHeld = true;
                // The fixture's source has a dub of the show; tracks-sub answers as one without.
                if (what == "tracks-sub" && ctx is { } known)
                    known.Known.Dubbed[playingId] = false;
                tracks =
                [
                    new FfiTrack(TrackKind.Audio, 1, "jpn", null, true, false),
                    new FfiTrack(TrackKind.Audio, 2, "eng", null, false, false),
                    new FfiTrack(TrackKind.Subtitle, 1, "eng", "Signs & Songs", false, false),
                    new FfiTrack(TrackKind.Subtitle, 2, "eng", "Full", true, false),
                    new FfiTrack(TrackKind.Subtitle, 3, "spa", null, false, false),
                ];
                BuildTracks();
                ShowControls();
                TracksMenu.ShowAt(TracksButton);
                await Task.Delay(400);
                // Open Subtitles, as a viewer's arrow key would.
                if (TracksMenu.Items.OfType<MenuFlyoutSubItem>().FirstOrDefault() is { } sub
                    && FrameworkElementAutomationPeer.CreatePeerForElement(sub)?.GetPattern(PatternInterface.ExpandCollapse)
                        is Microsoft.UI.Xaml.Automation.Provider.IExpandCollapseProvider peer)
                    peer.Expand();
                break;
            case var f when f.StartsWith("found-"):
                await StageFound(f);
                break;
            case var m when m.StartsWith("mark-"):
                var fixture = m switch
                {
                    "mark-end" => SkipMarkFixture.End,
                    "mark-review" => SkipMarkFixture.Review,
                    "mark-short" => SkipMarkFixture.TooShort,
                    "mark-long" => SkipMarkFixture.TooLong,
                    "mark-replaces" => SkipMarkFixture.Replaces,
                    "mark-shares" => SkipMarkFixture.Shares,
                    "mark-noentry" => SkipMarkFixture.NoEntry,
                    "mark-editing" => SkipMarkFixture.Editing,
                    "mark-previewing" => SkipMarkFixture.Previewing,
                    _ => SkipMarkFixture.Start,
                };
                HoldUpNext = true;
                upNext = null;
                ShowUpNext();
                marking = KaeriFfiMethods.SkipMarkFixture(fixture);
                DrawMarking();
                ShowControls();
                break;
            case "prompt-opening":
                promptHeld = true;
                ShowPrompt(new SkipPrompt.WasOpening(91, 181));
                break;
            case "prompt-wrong":
                promptHeld = true;
                ShowPrompt(new SkipPrompt.WrongTimes(SkipKind.Opening));
                break;
            case "prompt-share":
                ShowPrompt(new SkipPrompt.Share());
                break;
            case "skip-times":
                timelineMarks = [new TimelineMark(SkipKind.Opening, 91, 181, SkipOrigin.Yours), new TimelineMark(SkipKind.Ending, 1290, 1380, SkipOrigin.AniSkip)];
                ShowSkipTimes();
                break;
            case "timeline":
                // An opening, an ending and what is buffered, as fractions of the fixture clip, the
                // opening early so the played fill runs under it.
                var d = Math.Max(duration, 1);
                timelineMarks =
                [
                    new TimelineMark(SkipKind.Opening, d * 0.03, d * 0.2),
                    new TimelineMark(SkipKind.Ending, d * 0.85, d * 0.97),
                ];
                buffered = [new TimeRange(0, d * 0.7)];
                DrawTimeline();
                ShowControls();
                break;
            case "flash":
                flash.Interval = TimeSpan.FromSeconds(600);
                ShowFlash("12:04");
                break;
            case "settings":
                // The player's Settings menu open, with Stats for nerds at its foot.
                ShowControls();
                SettingsMenu.ShowAt(SettingsButton);
                break;
            case "stats":
            case "stats-waiting":
            case "stats-buffering":
            case "stats-hdr":
            case "stats-copied":
            case "stats-mini":
            case "subtitles":
            case "subtitles-mini":
                // The short clip is near its end from the start: no Up next card over the shot.
                HoldUpNext = true;
                upNext = null;
                ShowUpNext();
                // Subtitles from a file beside the clip, so the shot shows the panel clear of them.
                if (player is { } p && Path.ChangeExtension(ctx?.Options.Media ?? @"C:\setup\sdr-testsrc.mp4", ".srt") is { } srt
                    && File.Exists(srt))
                    await Task.Run(() => p.LoadTrack(TrackKind.Subtitle, srt));
                // The live panel reads mpv itself; the others hold the engine's made-up states.
                switch (what)
                {
                    case "stats-waiting":
                        // Opened before the file has: over the starting screen, a while on.
                        await StartDemo("starting-asking-first-4");
                        StageStats(StatsFixture.Waiting, TimeSpan.FromSeconds(5), copied: false);
                        break;
                    case "stats-buffering":
                        StageStats(StatsFixture.Buffering, null, copied: false);
                        break;
                    case "stats-hdr":
                        StageStats(StatsFixture.Hdr, null, copied: false);
                        break;
                    case "stats-copied":
                        StageStats(StatsFixture.Playing, null, copied: true);
                        break;
                    case "stats-mini":
                        StageStats(null, null, copied: false);
                        await Task.Delay(2500);
                        ToggleMini();
                        break;
                    case "subtitles":
                        ShowControls();
                        break;
                    case "subtitles-mini":
                        // In the mini window, its controls up: the subtitles clear of them.
                        ToggleMini();
                        await Task.Delay(800);
                        ShowControls();
                        break;
                    default:
                        StageStats(null, null, copied: false);
                        break;
                }
                break;
        }
    }
#endif

    // The mini player: the video in a small window that stays on top.

    /// <summary>Asked to go into the mini window (true), or back (false).</summary>
    internal event Action<bool>? MiniRequested;

    void OnMini(object sender, RoutedEventArgs e) => ToggleMini();

    /// <summary>Back from the mini window, if in it: a dialog is about to open, and needs the room.</summary>
    internal void LeaveMini()
    {
        if (mini)
            ToggleMini();
    }

    void ToggleMini()
    {
        var on = !mini;
        SetMini(on);
        MiniRequested?.Invoke(on);
        if (on && !miniTipShown)
        {
            miniTipShown = true;
            MiniTip.IsOpen = true;
            _ = Task.Delay(TimeSpan.FromSeconds(6)).ContinueWith(_ => DispatcherQueue.TryEnqueue(() => MiniTip.IsOpen = false));
        }
        ShowControls();
    }

    /// <summary>The controls of the small window: play, skip, the timeline, and the way back.</summary>
    void SetMini(bool on)
    {
        mini = on;
        var big = on ? Visibility.Collapsed : Visibility.Visible;
        BackButton.Visibility = TracksButton.Visibility = EpisodesButton.Visibility = FullscreenButton.Visibility = big;
        ShowTvButton();
        Elapsed.Visibility = Remaining.Visibility = VolumeButton.Visibility = SettingsButton.Visibility = big;
        Corner.Visibility = big;
        ShowTitle.FontSize = on ? 14 : 18;
        ShowTitle.LineHeight = on ? 20 : 24;
        EpisodeTitle.Visibility = big;
        TopBar.Padding = on ? new Thickness(12, 6, 140, 6) : new Thickness(8, 8, 148, 8);
        MiniIcon.Glyph = on ? "\uE73F" : "\uE8A7";
        ToolTipService.SetToolTip(MiniButton, T.TooltipWithKey(on ? T.PlayerMiniBack : T.PlayerMiniPlayer, "P"));
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(MiniButton, on ? T.PlayerMiniBack : T.PlayerMiniPlayer);
        if (on)
            ClosePreview();
        stats.Mini = on;
        ShowStats();
        // The small window has no room for the column or the reactions.
        if (on)
            ChatClear();
        ChatReflect();
    }

    // Previews on the timeline.

    internal void NoteLoad(ulong playback, PlayerOp[] ops)
    {
        lock (loads)
            this.playback = playback;
        // A new file opening: Stats for nerds, if up, asks the engine's new player again.
        if (ops.Any(op => op is PlayerOp.Open))
            DispatcherQueue.TryEnqueue(StatsReopen);
        foreach (var op in ops)
        {
            if (op is PlayerOp.Load load)
            {
                lock (loads)
                    lastLoad = new PreviewSource(load.Url, PreviewSource.Reach(load.Options));
            }
            else if (op is PlayerOp.Seek seek)
            {
                // Before mpv has it, so the countdown never sees the landing as time played.
                DispatcherQueue.TryEnqueue(() => Sought(seek.Absolute ? seek.Seconds : position + seek.Seconds));
            }
        }
    }

    PreviewSource? Source()
    {
        PreviewSource? now;
        lock (loads)
            now = lastLoad;
        if (thumbnails is { Length: > 0 } url)
            return thumbs ??= new PreviewSource(url, now?.Options ?? []);
        return stream is not null && now is not null && stream.Url == now.Url ? stream : stream = now;
    }

    void OnTimelinePointerMoved(object sender, PointerRoutedEventArgs e)
    {
        if (mini || duration <= 0)
            return;
        var point = e.GetCurrentPoint(Timeline).Position;
        PreviewAtFraction(Math.Clamp((point.X - 10) / Math.Max(1, Timeline.ActualWidth - 20), 0, 1));
        ShowTickLabel(e.GetCurrentPoint(TimelineUnder).Position.X);
    }

    void PreviewAtFraction(double fraction)
    {
        if (duration <= 0 || ctx is null || Source() is not { } from)
            return;
        var seconds = PreviewSource.TimeAt(fraction * (Timeline.ActualWidth - 20), Timeline.ActualWidth - 20, duration);
        preview ??= new SeekPreview(ctx.Session.Engine, PreviewPanel, DispatcherQueue, ShaderCache);
        preview.Show(seconds, from, XamlRoot?.RasterizationScale ?? 1.0);
        PreviewTime.Text = KaeriFfiMethods.PlayClock(seconds);
        var left = Timeline.TransformToVisual(Chrome).TransformPoint(new global::Windows.Foundation.Point(10 + fraction * (Timeline.ActualWidth - 20), 0)).X;
        PreviewBox.Margin = new Thickness(Math.Clamp(left - 84, 8, Math.Max(8, Chrome.ActualWidth - 176)), 0, 0, 8);
        PreviewBox.Visibility = Visibility.Visible;
    }

    void OnTimelineExited(object sender, PointerRoutedEventArgs e)
    {
        PreviewBox.Visibility = Visibility.Collapsed;
        TickLabel.Visibility = Visibility.Collapsed;
    }

    void ClosePreview()
    {
        PreviewBox.Visibility = Visibility.Collapsed;
        preview?.Close();
    }

    /// <summary>The top bar's title: what drags the window while the player is up.</summary>
    internal UIElement DragArea => TitleArea;

    /// <summary>Whether the window is full screen now.</summary>
    internal Func<bool>? FullscreenOn { get; set; }
}
