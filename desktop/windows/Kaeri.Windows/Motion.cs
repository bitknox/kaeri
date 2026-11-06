using Microsoft.UI.Composition;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Hosting;
using Microsoft.UI.Xaml.Media.Animation;
using Windows.UI.ViewManagement;

namespace Kaeri.Windows;

/// <summary>
/// The motion tokens on Windows' animation APIs. Springs are critically damped (quick: 200 ms
/// period, standard: 300 ms); fade-in is 200 ms decelerating, fade-out 300 ms accelerating.
/// With "Animation effects" off (<see cref="UISettings.AnimationsEnabled"/>), or Motion off in
/// Settings, the spatial motions (drill-in, the connected cover) become crossfades; fades stay,
/// as the brief says.
/// </summary>
static class Motion
{
    static readonly UISettings settings = new();

    /// <summary>A screenshot run's: reduced motion on or off whatever the system says.</summary>
    internal static bool? Override { get; set; }

    /// <summary>
    /// Settings › Playback › Motion, as the engine last said it: off reduces motion as the
    /// system's Animation effects off does; on leaves it to the system.
    /// </summary>
    internal static bool Chosen { get; set; } = true;

    public static bool Enabled => Override ?? (Chosen && settings.AnimationsEnabled);

    public static readonly TimeSpan Quick = TimeSpan.FromMilliseconds(200);
    public static readonly TimeSpan Standard = TimeSpan.FromMilliseconds(300);
    public static readonly TimeSpan FadeInTime = TimeSpan.FromMilliseconds(200);
    public static readonly TimeSpan FadeOutTime = TimeSpan.FromMilliseconds(300);

    static readonly Dictionary<UIElement, Storyboard> running = [];

    /// <summary>Shows a collapsed element by fading it in (fade-in).</summary>
    public static void Show(UIElement element) => Show(element, FadeInTime);

    /// <summary>Shows an element with the standard token, for cards appearing inside a screen.</summary>
    public static void ShowStandard(UIElement element) => Show(element, Standard);

    static void Show(UIElement element, TimeSpan time)
    {
        if (element.Visibility == Visibility.Visible && !running.ContainsKey(element) && element.Opacity >= 1)
            return;
        var from = element.Visibility == Visibility.Visible ? element.Opacity : 0;
        element.Visibility = Visibility.Visible;
        Fade(element, from, 1, time, EasingMode.EaseOut, null);
    }

    /// <summary>Fades an element out (fade-out), then collapses it so the video scans out alone.</summary>
    public static void Hide(UIElement element) => Hide(element, FadeOutTime);

    public static void HideStandard(UIElement element) => Hide(element, Standard);

    public static void HideQuick(UIElement element) => Hide(element, Quick);

    static void Hide(UIElement element, TimeSpan time)
    {
        if (element.Visibility == Visibility.Collapsed)
            return;
        Fade(element, element.Opacity, 0, time, EasingMode.EaseIn, () =>
        {
            element.Visibility = Visibility.Collapsed;
            element.Opacity = 1;
        });
    }

    /// <summary>Fades an element that keeps its place to the opacity <paramref name="to"/>: in on the fade-in token, away on the fade-out token.</summary>
    public static void Fade(UIElement element, double to)
    {
        if (!Enabled)
        {
            element.Opacity = to;
            return;
        }
        Fade(element, element.Opacity, to, to > element.Opacity ? FadeInTime : FadeOutTime, to > element.Opacity ? EasingMode.EaseOut : EasingMode.EaseIn, null);
    }

    /// <summary>
    /// Text changing in place: the new half fades in over the old, which fades away, on the
    /// standard token, whether or not animation effects are on (a fade is what the brief keeps).
    /// </summary>
    public static void Crossfade(UIElement from, UIElement to)
    {
        Fade(to, to.Opacity, 1, Standard, EasingMode.EaseOut, null);
        Fade(from, from.Opacity, 0, Standard, EasingMode.EaseIn, null);
    }

    /// <summary>Fades an element that keeps its place to <paramref name="to"/>, whether or not animation effects are on.</summary>
    public static void FadeTo(UIElement element, double to, TimeSpan time) =>
        Fade(element, element.Opacity, to, time, to > element.Opacity ? EasingMode.EaseOut : EasingMode.EaseIn, null);

    /// <summary>Fades freshly arrived content in from nothing: art, a page, a crossfade's new half.</summary>
    public static void FadeIn(UIElement element, TimeSpan? time = null)
    {
        element.Visibility = Visibility.Visible;
        Fade(element, 0, 1, time ?? FadeInTime, EasingMode.EaseOut, null);
    }

    /// <summary>
    /// A trailer's video fading in over the banner (fade-in) or away again (fade-out), from where
    /// it stands. <paramref name="done"/> runs when the fade ends, unless another took over.
    /// </summary>
    public static void Video(UIElement element, bool shown, Action? done = null)
    {
        if (!shown && element.Opacity <= 0)
        {
            done?.Invoke();
            return;
        }
        Fade(element, element.Opacity, shown ? 1 : 0, shown ? FadeInTime : FadeOutTime, shown ? EasingMode.EaseOut : EasingMode.EaseIn, done);
    }

    /// <summary>
    /// A trailer's art over the video (<paramref name="covered"/>) or away from it. The art goes
    /// when the first frame plays, on the fade-in token (200 ms), and comes back over the
    /// video on the fade-out token (300 ms).
    /// </summary>
    public static void Cover(UIElement art, bool covered, Action? done = null)
    {
        if (!Enabled)
        {
            Set(art, true);
            art.Opacity = covered ? 1 : 0;
            done?.Invoke();
            return;
        }
        Fade(art, art.Opacity, covered ? 1 : 0, covered ? FadeOutTime : FadeInTime, covered ? EasingMode.EaseIn : EasingMode.EaseOut, done);
    }

    /// <summary>
    /// Eases a value from one number to another on the standard token, calling <paramref name="set"/>
    /// with each step: what a property no animation can reach (a padding) follows. With animation
    /// effects off it takes the end value at once. Starting another for the same <paramref name="owner"/>
    /// replaces the first. It runs on Fluent's point-to-point curve, a step each frame drawn rather
    /// than on a timer, whose 15.6 ms ticks made the move uneven.
    /// </summary>
    public static void Ease(object owner, double from, double to, Action<double> set)
    {
        if (eases.Remove(owner, out var was))
            Microsoft.UI.Xaml.Media.CompositionTarget.Rendering -= was;
        if (!Enabled || Math.Abs(from - to) < 0.5)
        {
            set(to);
            return;
        }
        var clock = System.Diagnostics.Stopwatch.StartNew();
        EventHandler<object>? step = null;
        step = (_, _) =>
        {
            var t = Math.Min(1, clock.Elapsed / Standard);
            set(from + (to - from) * PointToPoint(t));
            if (t >= 1)
            {
                Microsoft.UI.Xaml.Media.CompositionTarget.Rendering -= step;
                if (eases.TryGetValue(owner, out var current) && current == step)
                    eases.Remove(owner);
            }
        };
        eases[owner] = step;
        Microsoft.UI.Xaml.Media.CompositionTarget.Rendering += step;
    }

    static readonly Dictionary<object, EventHandler<object>> eases = [];

    /// <summary>
    /// Fluent's point-to-point curve, cubic-bezier(0.55, 0.55, 0, 1), for something on screen
    /// moving or resizing in place: how far it has come at <paramref name="t"/> of its time.
    /// </summary>
    internal static double PointToPoint(double t)
    {
        if (t <= 0)
            return 0;
        if (t >= 1)
            return 1;
        static double Bezier(double a, double b, double u) => 3 * (1 - u) * (1 - u) * u * a + 3 * (1 - u) * u * u * b + u * u * u;
        double lo = 0, hi = 1;
        for (var i = 0; i < 30; i++)
        {
            var mid = (lo + hi) / 2;
            if (Bezier(0.55, 0, mid) < t)
                lo = mid;
            else
                hi = mid;
        }
        return Bezier(0.55, 1, (lo + hi) / 2);
    }

    /// <summary>Fluent's point-to-point curve for a keyframe animation.</summary>
    static KeySpline PointToPointSpline() => new()
    {
        ControlPoint1 = new global::Windows.Foundation.Point(0.55, 0.55),
        ControlPoint2 = new global::Windows.Foundation.Point(0, 1),
    };

    /// <summary>Fluent's decelerate curve, cubic-bezier(0, 0, 0, 1), for something coming in on the composition layer.</summary>
    public static CubicBezierEasingFunction Decelerate(Compositor compositor) =>
        compositor.CreateCubicBezierEasingFunction(new(0, 0), new(0, 1));

    /// <summary>Fluent's accelerate curve, cubic-bezier(1, 0, 1, 1), for something leaving on the composition layer.</summary>
    public static CubicBezierEasingFunction Accelerate(Compositor compositor) =>
        compositor.CreateCubicBezierEasingFunction(new(1, 0), new(1, 1));

    /// <summary>The fade-in token's curve, cubic-bezier(0, 0, 0.58, 1), on the composition layer.</summary>
    public static CubicBezierEasingFunction FadeInEasing(Compositor compositor) =>
        compositor.CreateCubicBezierEasingFunction(new(0, 0), new(0.58f, 1));

    /// <summary>The fade-out token's curve, cubic-bezier(0.42, 0, 1, 1), on the composition layer.</summary>
    public static CubicBezierEasingFunction FadeOutEasing(Compositor compositor) =>
        compositor.CreateCubicBezierEasingFunction(new(0.42f, 0), new(1, 1));

    /// <summary>Stops any fade and sets the element where it would have ended.</summary>
    public static void Set(UIElement element, bool shown)
    {
        if (running.Remove(element, out var board))
            board.Stop();
        element.Opacity = 1;
        element.Visibility = shown ? Visibility.Visible : Visibility.Collapsed;
    }

    /// <summary>The canvas's fade tokens as curves: fade-in cubic-bezier(0, 0, 0.58, 1), fade-out cubic-bezier(0.42, 0, 1, 1).</summary>
    static KeySpline Spline(EasingMode mode) => mode == EasingMode.EaseOut
        ? new KeySpline { ControlPoint1 = new global::Windows.Foundation.Point(0, 0), ControlPoint2 = new global::Windows.Foundation.Point(0.58, 1) }
        : new KeySpline { ControlPoint1 = new global::Windows.Foundation.Point(0.42, 0), ControlPoint2 = new global::Windows.Foundation.Point(1, 1) };

    static void Fade(UIElement element, double from, double to, TimeSpan time, EasingMode mode, Action? done)
    {
        if (running.Remove(element, out var was))
            was.Stop();
        var animation = new DoubleAnimationUsingKeyFrames();
        animation.KeyFrames.Add(new DiscreteDoubleKeyFrame { KeyTime = TimeSpan.Zero, Value = from });
        animation.KeyFrames.Add(new SplineDoubleKeyFrame { KeyTime = time, Value = to, KeySpline = Spline(mode) });
        Storyboard.SetTarget(animation, element);
        Storyboard.SetTargetProperty(animation, "Opacity");
        var board = new Storyboard();
        board.Children.Add(animation);
        board.Completed += (_, _) =>
        {
            if (running.TryGetValue(element, out var current) && current == board)
            {
                running.Remove(element);
                element.Opacity = to;
                done?.Invoke();
            }
        };
        running[element] = board;
        board.Begin();
    }

    /// <summary>
    /// An element's height eased from one value to another on the standard token and Fluent's
    /// point-to-point curve, then left to its content again: a dialog moving to a taller or shorter step. With animation effects off it
    /// simply takes the new height.
    /// </summary>
    public static void Height(FrameworkElement element, double from, double to)
    {
        if (running.Remove(element, out var was))
            was.Stop();
        element.Height = double.NaN;
        if (!Enabled || Math.Abs(from - to) < 1)
            return;
        var animation = new DoubleAnimationUsingKeyFrames { EnableDependentAnimation = true };
        animation.KeyFrames.Add(new DiscreteDoubleKeyFrame { KeyTime = TimeSpan.Zero, Value = from });
        animation.KeyFrames.Add(new SplineDoubleKeyFrame { KeyTime = Standard, Value = to, KeySpline = PointToPointSpline() });
        Storyboard.SetTarget(animation, element);
        Storyboard.SetTargetProperty(animation, "Height");
        var board = new Storyboard();
        board.Children.Add(animation);
        board.Completed += (_, _) =>
        {
            if (running.TryGetValue(element, out var current) && current == board)
            {
                running.Remove(element);
                board.Stop();
                element.Height = double.NaN;
            }
        };
        running[element] = board;
        board.Begin();
    }

    /// <summary>An opacity that eases to each new value on the quick token: the row dim after marking watched.</summary>
    public static void QuickOpacity(UIElement element) =>
        element.OpacityTransition = new ScalarTransition { Duration = Quick };

    /// <summary>
    /// A critically damped spring on a composition property ("Scale.X", "Offset.Y"...) of the
    /// element's visual: quick or standard.
    /// </summary>
    public static void Spring(UIElement element, string property, float to, bool quick = false)
    {
        var visual = ElementCompositionPreview.GetElementVisual(element);
        var spring = visual.Compositor.CreateSpringScalarAnimation();
        spring.DampingRatio = 1f;
        spring.Period = quick ? Quick : Standard;
        spring.FinalValue = to;
        if (!Enabled)
        {
            spring.Period = TimeSpan.FromMilliseconds(1);
        }
        visual.StartAnimation(property, spring);
    }
}
