using Kaeri.App;
using Kaeri.Engine.Ffi;
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
/// and the controller grown from it over the pane. When the cast ends from outside the mini goes
/// (the window's notice takes its place), and an open controller says why in place, with Play
/// here. The controller and the mini morph into each other: the container grows from the mini's
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
    readonly Microsoft.UI.Dispatching.DispatcherQueueTimer ticker;
    bool expanded, miniShown, footShown;
    /// <summary>The frame of a cast that ended, kept while the controller stays open on it.</summary>
    CastDrawn? ended;

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
        Grid.SetRowSpan(controller, pane.RowDefinitions.Count);
        pane.Children.Add(controller);

        ticker = pane.DispatcherQueue.CreateTimer();
        ticker.Interval = TimeSpan.FromMilliseconds(250);
        ticker.Tick += (_, _) => Refresh();
        controller.PointerPressed += TurnRound;
    }

    /// <summary>The controller is open.</summary>
    public bool Expanded => expanded;

    /// <summary>Follows the session: called on every change, and four times a second while the mini or the controller shows.</summary>
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
        // The mini is for a cast that goes on: an end, a failure or a TV with nothing on it has none.
        var onMini = drawn.Line is not (CastLine.Stopped or CastLine.Failed or CastLine.Idle);
        if (onMini)
            mini.Show(drawn);
        if (controller.Visibility == Visibility.Visible)
        {
            controller.Heard(tv.Session);
            controller.Show(drawn, tv.Session, tv.ControllerCard);
        }
        ShowMini(onMini);
        if (onMini || expanded)
        {
            if (!ticker.IsRunning)
                ticker.Start();
        }
        else
            ticker.Stop();
    }

    /// <summary>
    /// The frame, with the show remembered from the last launch while a cast is joined again.
    /// Open, the controller also draws a TV held with nothing on it, and keeps an ended frame
    /// until it closes.
    /// </summary>
    CastDrawn? Frame()
    {
        var session = tv.Session;
        var drawn = tv.SessionLoaded || tv.SessionHeld || session?.State == FfiCastSessionState.Error ? tv.Drawn(idle: expanded) : null;
        if (drawn is { Line: CastLine.Rejoining, Title.Length: 0 } && host.Remembered() is { } kept)
            drawn = drawn with { Id = kept.Id, Title = kept.Title };
        if (!expanded)
            return drawn;
        if (drawn is { Line: CastLine.Stopped or CastLine.Failed })
            ended = drawn;
        else if (drawn is null && ended is { } last)
            return last;
        else
            ended = null;
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
        Foot();
    }

    /// <summary>The window's notice stands over the mini while that shows, else at the foot.</summary>
    void Foot()
    {
        var shown = miniShown && !expanded;
        if (footShown == shown)
            return;
        footShown = shown;
        host.MiniShown(shown);
    }

    // Opening and closing.

    /// <summary>
    /// Opens the controller from the mini, or from the cast button while a TV is held with nothing
    /// on it.
    /// </summary>
    public void Expand()
    {
        if (expanded)
            return;
        expanded = true;
        if (Frame() is not { } drawn)
        {
            expanded = false;
            return;
        }
        if (controller.Visibility == Visibility.Collapsed)
        {
            controller.Visibility = Visibility.Visible;
            foreach (var faded in controller.Faded)
                faded.Opacity = 0;
            controller.Art.Root.Opacity = 1;
            controller.Opacity = Motion.Enabled ? 1 : 0;
            controller.Show(drawn, tv.Session, tv.ControllerCard);
            controller.UpdateLayout();
        }
        Run(open: true);
        Foot();
        if (!ticker.IsRunning)
            ticker.Start();
        // Into the controller: play or pause, else the way on, else the chevron down.
        if (!controller.Play.Focus(FocusState.Programmatic) && !controller.FocusFooter())
            controller.Collapse.Focus(FocusState.Programmatic);
    }

    public void Collapse() => Close(at: false);

    /// <summary>The controller's More menu, opened, for a fixture's frame of it.</summary>
    public void ShowMore()
    {
        if (expanded)
            controller.ShowMore();
    }

    /// <summary>Closes into the mini; <paramref name="at"/> at once, when the mini itself is going.</summary>
    void Close(bool at)
    {
        if (!expanded && running is null)
            return;
        expanded = false;
        ended = null;
        Foot();
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
        var edges = clip ??= compositor.CreateRectangleClip();
        visual.Clip = edges;
        var height = pane.ActualHeight;
        var width = pane.ActualWidth;
        var miniTop = Math.Max(0, height - Math.Max(mini.ActualHeight, 76));
        // From where the edge stands now, as a share of the way from the mini to the full pane.
        var from = edge;
        var to = open ? 1.0 : 0.0;
        var miniFrom = mini.Opacity;
        var controlsFrom = controller.Faded.First().Opacity;
        edges.Left = 0;
        edges.Right = (float)width;
        edges.Bottom = (float)height;
        edges.Top = (float)(miniTop * (1 - from));
        // The mini's row fades over the container as it grows from it, and comes back over it as
        // it shrinks; clicks meanwhile go to the container, to turn the morph round.
        Canvas.SetZIndex(mini, 2);
        Canvas.SetZIndex(controller, 1);
        mini.IsHitTestVisible = false;

        Fly(open);
        var clock = System.Diagnostics.Stopwatch.StartNew();
        EventHandler<object>? step = null;
        step = (_, _) =>
        {
            var t = clock.Elapsed.TotalSeconds;
            var k = SpringRaw(t);
            var g = edge = from + (to - from) * k;
            edges.Top = (float)(miniTop * (1 - g));
            edges.Right = (float)pane.ActualWidth;
            edges.Bottom = (float)pane.ActualHeight;
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
        edge = open ? 1 : 0;
        Canvas.SetZIndex(mini, 0);
        Canvas.SetZIndex(controller, 1);
        mini.IsHitTestVisible = !open;
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
        // Open, the controller covers the pane and the mini under it takes no clicks.
        Canvas.SetZIndex(mini, open ? 0 : 2);
        mini.IsHitTestVisible = !open;
        if (!open)
            controller.Visibility = Visibility.Collapsed;
    }
}
