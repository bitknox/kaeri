using Kaeri.App;
using Kaeri.Engine.Model;
using Kaeri.Windows.Pages;
using Microsoft.UI.Composition;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Hosting;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Animation;

namespace Kaeri.Windows.Casting;

/// <summary>What the window does for the cast layer: the show's page, playing here, the sources.</summary>
sealed class CastHost
{
    public required Action<uint> OpenShow { get; init; }
    public required Action<TvPlay> PlayHere { get; init; }
    public required Action<TvPlay> ChooseSource { get; init; }
    /// <summary>The mini came or went: the toast moves above it.</summary>
    public required Action<bool> MiniShown { get; init; }
    /// <summary>The show a cast left running was on at the last launch, for the mini while it joins again.</summary>
    public required Func<(uint Id, string Title)?> Remembered { get; init; }
}

/// <summary>
/// The cast on the page pane: the mini at its foot on every page while something is on the TV,
/// the controller grown from it over the pane, and the bar at the top when someone else took the
/// TV. The controller and the mini morph into each other: the container grows from the mini's
/// rect on the standard spring with the art flying across, the mini's row fading out in the first
/// 100 ms and the controls in over the next 200; closing runs the other way. A click mid-morph
/// turns it round from where it is. With animation effects off the two crossfade in place.
/// </summary>
sealed class CastLayer
{
    const string ArtFlight = "kaeri-cast-art";
    readonly Ctx ctx;
    readonly TvRemote tv;
    readonly CastHost host;
    readonly Grid pane;
    readonly CastMini mini;
    readonly CastController controller;
    readonly InfoBar takenOver = new()
    {
        Severity = InfoBarSeverity.Informational,
        IsClosable = true,
        VerticalAlignment = VerticalAlignment.Top,
        Margin = new Thickness(36, 16, 36, 0),
        MaxWidth = 720,
    };
    readonly Microsoft.UI.Dispatching.DispatcherQueueTimer ticker;
    TvPlay? takenPlay;
    bool expanded, miniShown;

    public CastLayer(Ctx ctx, TvRemote tv, Grid pane, CastHost host)
    {
        this.ctx = ctx;
        this.tv = tv;
        this.pane = pane;
        this.host = host;
        mini = new CastMini(ctx, tv, Expand) { Visibility = Visibility.Collapsed };
        controller = new CastController(ctx, tv, new CastActions
        {
            Collapse = Collapse,
            OpenShow = OpenShow,
            PlayHere = PlayHere,
            Stop = tv.DisconnectCast,
            ChooseAnother = () =>
            {
                Collapse();
                tv.ChooseAnotherTv();
            },
            ChooseSource = () =>
            {
                Collapse();
                if (tv.CastSources() is { } play)
                    host.ChooseSource(play);
            },
        })
        { Visibility = Visibility.Collapsed };
        Grid.SetRow(mini, pane.RowDefinitions.Count - 1);
        pane.Children.Add(mini);
        Grid.SetRowSpan(takenOver, pane.RowDefinitions.Count);
        pane.Children.Add(takenOver);
        Grid.SetRowSpan(controller, pane.RowDefinitions.Count);
        pane.Children.Add(controller);
        var playHere = new Button { Content = T.CastPlayHere };
        playHere.Click += (_, _) =>
        {
            takenOver.IsOpen = false;
            if (takenPlay is { } play)
                host.PlayHere(play);
        };
        takenOver.ActionButton = playHere;
        takenOver.IsOpen = false;

        ticker = pane.DispatcherQueue.CreateTimer();
        ticker.Interval = TimeSpan.FromMilliseconds(250);
        ticker.Tick += (_, _) => Refresh();
        controller.PointerPressed += TurnRound;
    }

    /// <summary>The controller is open.</summary>
    public bool Expanded => expanded;

    /// <summary>Someone else took the TV: the mini goes, and the bar offers to carry on here.</summary>
    public void TakenOver(TvPlay play, Msg notice)
    {
        takenPlay = play;
        takenOver.Message = notice.Text();
        takenOver.IsOpen = true;
        if (expanded)
            Close(at: true);
        Refresh();
    }

    /// <summary>Follows the session: called on every change, and four times a second while the mini shows.</summary>
    public void Refresh()
    {
        var drawn = Frame();
        if (drawn is null)
        {
            if (expanded)
                Close(at: true);
            ShowMini(false);
            ticker.Stop();
            return;
        }
        if (takenOver.IsOpen)
            takenOver.IsOpen = false;
        mini.Show(drawn);
        if (controller.Visibility == Visibility.Visible)
        {
            controller.Heard(tv.Session);
            controller.Show(drawn, tv.Session, tv.ControllerCard);
        }
        ShowMini(true);
        if (!ticker.IsRunning)
            ticker.Start();
    }

    /// <summary>The frame, with the show remembered from the last launch while a cast is joined again.</summary>
    CastDrawn? Frame()
    {
        if (!tv.SessionLoaded || tv.Session?.Notice is Msg.NoticeCastTakenOver)
            return null;
        var drawn = tv.Drawn();
        if (drawn is { Line: CastLine.Rejoining, Title.Length: 0 } && host.Remembered() is { } kept)
            drawn = drawn with { Id = kept.Id, Title = kept.Title };
        return drawn;
    }

    void ShowMini(bool shown)
    {
        if (miniShown == shown)
            return;
        miniShown = shown;
        if (shown)
        {
            mini.Opacity = expanded ? 0 : 1;
            Motion.FadeIn(mini);
        }
        else
            Motion.Hide(mini);
        host.MiniShown(shown);
    }

    // Opening and closing.

    public void Expand()
    {
        if (!miniShown || expanded)
            return;
        expanded = true;
        if (controller.Visibility == Visibility.Collapsed)
        {
            controller.Visibility = Visibility.Visible;
            foreach (var faded in controller.Faded)
                faded.Opacity = 0;
            controller.Art.Root.Opacity = 1;
            controller.Opacity = Motion.Enabled ? 1 : 0;
            if (Frame() is { } drawn)
                controller.Show(drawn, tv.Session, tv.ControllerCard);
            controller.UpdateLayout();
        }
        Run(open: true);
        controller.Play.Focus(FocusState.Programmatic);
    }

    public void Collapse() => Close(at: false);

    /// <summary>Closes into the mini; <paramref name="at"/> at once, when the mini itself is going.</summary>
    void Close(bool at)
    {
        if (!expanded && running is null)
            return;
        expanded = false;
        if (at)
        {
            Stop();
            Settle(open: false);
            return;
        }
        Run(open: false);
        if (miniShown)
            mini.Opener.Focus(FocusState.Programmatic);
    }

    /// <summary>The title or the art: the controller closes into the mini over the show's page.</summary>
    void OpenShow()
    {
        if (Frame()?.Id is { } id)
            host.OpenShow(id);
        Collapse();
    }

    void PlayHere()
    {
        Close(at: true);
        if (tv.CastPlayHere() is { } play)
            host.PlayHere(play);
    }

    // The morph.

    EventHandler<object>? running;
    ConnectedAnimation? flight;
    RectangleClip? clip;

    /// <summary>
    /// One run of the morph towards open or closed, from wherever the last one stood. The
    /// container's edge and the fades are worked out per frame; the art flies on a connected
    /// animation, cancelled if the run turns round.
    /// </summary>
    void Run(bool open)
    {
        Stop();
        if (!Motion.Enabled)
        {
            Crossfade(open);
            return;
        }
        var visual = ElementCompositionPreview.GetElementVisual(controller);
        var compositor = visual.Compositor;
        clip ??= compositor.CreateRectangleClip();
        visual.Clip = clip;
        var height = pane.ActualHeight;
        var width = pane.ActualWidth;
        var miniTop = Math.Max(0, height - Math.Max(mini.ActualHeight, 76));
        // From where the edge stands now, as a share of the way from the mini to the full pane.
        var from = edge;
        var to = open ? 1.0 : 0.0;
        var miniFrom = mini.Opacity;
        var controlsFrom = controller.Faded.First().Opacity;
        clip.Left = 0;
        clip.Right = (float)width;
        clip.Bottom = (float)height;
        clip.Top = (float)(miniTop * (1 - from));

        Fly(open);
        var clock = System.Diagnostics.Stopwatch.StartNew();
        EventHandler<object>? step = null;
        step = (_, _) =>
        {
            var t = clock.Elapsed.TotalSeconds;
            var k = SpringRaw(t);
            var g = edge = from + (to - from) * k;
            clip.Top = (float)(miniTop * (1 - g));
            clip.Right = (float)pane.ActualWidth;
            clip.Bottom = (float)pane.ActualHeight;
            // Opening: the mini out in 100 ms, the controls in from 100 to 300 ms. Closing: the
            // controls out in 100 ms, the mini back from 100 to 300 ms.
            var miniNow = open ? Ramp(miniFrom, 0, t, 0, 0.1) : Ramp(miniFrom, 1, t, 0.1, 0.2);
            var controlsNow = open ? Ramp(controlsFrom, 1, t, 0.1, 0.2) : Ramp(controlsFrom, 0, t, 0, 0.1);
            mini.Opacity = miniNow;
            foreach (var faded in controller.Faded)
                faded.Opacity = controlsNow;
            if (t >= SettleTime)
            {
                Stop();
                Settle(open);
            }
        };
        running = step;
        CompositionTarget.Rendering += step;
        Touchable(false);
    }

    /// <summary>Where the container's edge stands: 0 at the mini's rect, 1 over the whole pane.</summary>
    double edge;

    /// <summary>
    /// Mid-morph a click anywhere on the controller turns it round, so its controls take no
    /// clicks until it rests.
    /// </summary>
    void Touchable(bool touchable)
    {
        foreach (var faded in controller.Faded)
            faded.IsHitTestVisible = touchable;
        controller.Art.Root.IsHitTestVisible = touchable;
    }

    void TurnRound(object sender, Microsoft.UI.Xaml.Input.PointerRoutedEventArgs e)
    {
        if (running is null)
            return;
        e.Handled = true;
        if (expanded)
            Collapse();
        else
            Expand();
    }

    /// <summary>How long a run lasts: the standard spring (a 0.3 s response) comes to rest by then.</summary>
    const double SettleTime = 0.7;

    /// <summary>A critically damped spring with the standard 0.3 s response, from rest: 1 − (1 + ωt)e^(−ωt).</summary>
    static double SpringRaw(double t)
    {
        const double w = 2 * Math.PI / 0.3;
        return t >= SettleTime ? 1 : 1 - (1 + w * t) * Math.Exp(-w * t);
    }

    static double Ramp(double from, double to, double t, double delay, double length) =>
        from + (to - from) * Math.Clamp((t - delay) / length, 0, 1);

    /// <summary>The art flies between the mini and the controller, on the standard spring.</summary>
    void Fly(bool open)
    {
        var service = ConnectedAnimationService.GetForCurrentView();
        var (source, target) = open ? (mini.Art.Root, controller.Art.Root) : (controller.Art.Root, mini.Art.Root);
        if (source.ActualWidth <= 0 || target.ActualWidth <= 0)
        {
            controller.Art.Root.Opacity = open ? 1 : 0;
            return;
        }
        service.DefaultDuration = Motion.Standard;
        service.DefaultEasingFunction = Motion.SpringEasing(ElementCompositionPreview.GetElementVisual(source).Compositor);
        flight = service.PrepareToAnimate(ArtFlight, source);
        flight.Configuration = new BasicConnectedAnimationConfiguration();
        if (!open)
        {
            // The controller's art leaves with the flight rather than being clipped away under it.
            controller.Art.Root.Opacity = 0;
            mini.Art.Root.Opacity = 1;
        }
        else
            controller.Art.Root.Opacity = 1;
        if (!flight.TryStart(target))
        {
            flight = null;
            controller.Art.Root.Opacity = open ? 1 : 0;
        }
        else
            flight.Completed += (_, _) => flight = null;
    }

    /// <summary>With animation effects off: the controller and the mini crossfade in place, no art in flight.</summary>
    void Crossfade(bool open)
    {
        controller.Art.Root.Opacity = 1;
        foreach (var faded in controller.Faded)
            faded.Opacity = 1;
        ElementCompositionPreview.GetElementVisual(controller).Clip = null;
        if (open)
        {
            Motion.FadeTo(controller, 1, Motion.Standard);
            Motion.FadeTo(mini, 0, Motion.Standard);
        }
        else
        {
            Motion.FadeTo(mini, 1, Motion.Standard);
            Motion.FadeOutThen(controller, () =>
            {
                if (!expanded)
                    Settle(open: false);
            });
        }
    }

    /// <summary>A run turned round or ended: the frame callback and the flight stop where they are.</summary>
    void Stop()
    {
        if (running is not null)
        {
            CompositionTarget.Rendering -= running;
            running = null;
        }
        if (flight is not null)
        {
            flight.Cancel();
            flight = null;
        }
    }

    /// <summary>At rest: open over the pane, or gone with the mini whole again.</summary>
    void Settle(bool open)
    {
        var visual = ElementCompositionPreview.GetElementVisual(controller);
        visual.Clip = null;
        clip = null;
        controller.Opacity = 1;
        foreach (var faded in controller.Faded)
            faded.Opacity = open ? 1 : 0;
        controller.Art.Root.Opacity = 1;
        mini.Opacity = open ? 0 : 1;
        edge = open ? 1 : 0;
        Touchable(true);
        if (!open)
            controller.Visibility = Visibility.Collapsed;
    }
}
