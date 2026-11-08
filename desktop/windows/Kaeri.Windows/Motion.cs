using Microsoft.UI.Composition;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Hosting;
using Microsoft.UI.Xaml.Media.Animation;
using Windows.UI.ViewManagement;

namespace Kaeri.Windows;

/// <summary>
/// The motion tokens on Windows' animation APIs. Springs are critically damped (quick: 200 ms
/// period, standard: 300 ms), as the Mac's <c>spring(duration:, bounce: 0)</c>; where a keyframe
/// animation stands in for one it runs on <see cref="SpringSpline"/>, the same curve. Fade-in is
/// 200 ms decelerating, fade-out 300 ms accelerating.
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
    public static void Show(UIElement element) => Show(element, FadeInTime, Curve.FadeIn);

    /// <summary>Shows an element with the standard token, for cards appearing inside a screen.</summary>
    public static void ShowStandard(UIElement element) => Show(element, Standard, Curve.Spring);

    static void Show(UIElement element, TimeSpan time, Curve curve)
    {
        if (element.Visibility == Visibility.Visible && !running.ContainsKey(element) && element.Opacity >= 1)
            return;
        var from = element.Visibility == Visibility.Visible ? element.Opacity : 0;
        element.Visibility = Visibility.Visible;
        Fade(element, from, 1, time, curve, null);
    }

    /// <summary>Fades an element out (fade-out), then collapses it so the video scans out alone.</summary>
    public static void Hide(UIElement element) => Hide(element, FadeOutTime, Curve.FadeOut);

    public static void HideStandard(UIElement element) => Hide(element, Standard, Curve.Spring);

    public static void HideQuick(UIElement element) => Hide(element, Quick, Curve.Spring);

    static void Hide(UIElement element, TimeSpan time, Curve curve)
    {
        if (element.Visibility == Visibility.Collapsed)
            return;
        Fade(element, element.Opacity, 0, time, curve, () =>
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
        Fade(element, element.Opacity, to, to > element.Opacity ? FadeInTime : FadeOutTime, to > element.Opacity ? Curve.FadeIn : Curve.FadeOut, null);
    }

    /// <summary>
    /// Text changing in place: the new half fades in over the old, which fades away, on the
    /// standard token, whether or not animation effects are on (a fade is what the brief keeps).
    /// </summary>
    public static void Crossfade(UIElement from, UIElement to)
    {
        Fade(to, to.Opacity, 1, Standard, Curve.Spring, null);
        Fade(from, from.Opacity, 0, Standard, Curve.Spring, null);
    }

    /// <summary>Fades an element that keeps its place to <paramref name="to"/>, whether or not animation effects are on.</summary>
    public static void FadeTo(UIElement element, double to, TimeSpan time) =>
        Fade(element, element.Opacity, to, time, to > element.Opacity ? Curve.FadeIn : Curve.FadeOut, null);

    /// <summary>Fades freshly arrived content in from nothing: art, a page, a crossfade's new half.</summary>
    public static void FadeIn(UIElement element, TimeSpan? time = null)
    {
        element.Visibility = Visibility.Visible;
        Fade(element, 0, 1, time ?? FadeInTime, Curve.FadeIn, null);
    }

    /// <summary>
    /// <see cref="FadeIn"/>, then <paramref name="done"/> once the element is fully on screen
    /// (unless another fade took over), whether or not animation effects are on.
    /// </summary>
    public static void FadeInThen(UIElement element, Action done)
    {
        element.Visibility = Visibility.Visible;
        Fade(element, 0, 1, FadeInTime, Curve.FadeIn, done);
    }

    /// <summary>
    /// A fade out on the fade-out token, then <paramref name="done"/>, whether or not animation
    /// effects are on: a state leaving over what is under it.
    /// </summary>
    public static void FadeOutThen(UIElement element, Action done) =>
        Fade(element, element.Opacity, 0, FadeOutTime, Curve.FadeOut, done);

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
        Fade(element, element.Opacity, shown ? 1 : 0, shown ? FadeInTime : FadeOutTime, shown ? Curve.FadeIn : Curve.FadeOut, done);
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
        Fade(art, art.Opacity, covered ? 1 : 0, covered ? FadeOutTime : FadeInTime, covered ? Curve.FadeOut : Curve.FadeIn, done);
    }

    /// <summary>
    /// Eases a value from one number to another on the standard token, calling <paramref name="set"/>
    /// with each step: what a property no animation can reach (a padding) follows. With animation
    /// effects off it takes the end value at once. Starting another for the same <paramref name="owner"/>
    /// replaces the first. Each step comes with a frame, not a timer, so the value moves once per
    /// frame drawn, along the standard spring.
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
            set(from + (to - from) * SpringAt(t));
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
    /// How far a zero-bounce spring has come at <paramref name="t"/> of its period, from rest:
    /// 1 − (1 + ωt)e^(−ωt) with ω = 2π, scaled to land on 1 at the period's end, where the spring
    /// itself is within 1.4% of it.
    /// </summary>
    internal static double SpringAt(double t)
    {
        const double w = 2 * Math.PI;
        static double Raw(double x) => 1 - (1 + w * x) * Math.Exp(-w * x);
        return t >= 1 ? 1 : Raw(t) / Raw(1);
    }

    /// <summary>
    /// <see cref="SpringAt"/> as a cubic-bezier, for keyframe animations: (0.21, 0.11, 0.21, 1)
    /// stays within 0.6% of it over the token's time.
    /// </summary>
    static readonly global::Windows.Foundation.Point SpringFrom = new(0.21, 0.11), SpringTo = new(0.21, 1);

    /// <summary>The quick and standard springs as a keyframe curve, run over the token's time.</summary>
    public static KeySpline SpringSpline() => new() { ControlPoint1 = SpringFrom, ControlPoint2 = SpringTo };

    /// <summary>The quick and standard springs as a composition easing, run over the token's time.</summary>
    public static CubicBezierEasingFunction SpringEasing(Compositor compositor) =>
        compositor.CreateCubicBezierEasingFunction(new((float)SpringFrom.X, (float)SpringFrom.Y), new((float)SpringTo.X, (float)SpringTo.Y));

    /// <summary>The fade-in token as a composition easing: cubic-bezier(0, 0, 0.58, 1).</summary>
    public static CubicBezierEasingFunction FadeInEasing(Compositor compositor) =>
        compositor.CreateCubicBezierEasingFunction(new(0, 0), new(0.58f, 1));

    /// <summary>Stops any fade and sets the element where it would have ended.</summary>
    public static void Set(UIElement element, bool shown)
    {
        if (running.Remove(element, out var board))
            board.Stop();
        element.Opacity = 1;
        element.Visibility = shown ? Visibility.Visible : Visibility.Collapsed;
    }

    /// <summary>The curve a fade runs on: the fade-in or fade-out token's, or the spring's, for a fade on quick or standard.</summary>
    enum Curve
    {
        FadeIn,
        FadeOut,
        Spring,
    }

    /// <summary>The canvas's fade tokens as curves: fade-in cubic-bezier(0, 0, 0.58, 1), fade-out cubic-bezier(0.42, 0, 1, 1), and the spring's.</summary>
    static KeySpline Spline(Curve curve) => curve switch
    {
        Curve.FadeIn => new KeySpline { ControlPoint1 = new global::Windows.Foundation.Point(0, 0), ControlPoint2 = new global::Windows.Foundation.Point(0.58, 1) },
        Curve.FadeOut => new KeySpline { ControlPoint1 = new global::Windows.Foundation.Point(0.42, 0), ControlPoint2 = new global::Windows.Foundation.Point(1, 1) },
        _ => SpringSpline(),
    };

    static void Fade(UIElement element, double from, double to, TimeSpan time, Curve curve, Action? done)
    {
        if (running.Remove(element, out var was))
            was.Stop();
        var animation = new DoubleAnimationUsingKeyFrames();
        animation.KeyFrames.Add(new DiscreteDoubleKeyFrame { KeyTime = TimeSpan.Zero, Value = from });
        animation.KeyFrames.Add(new SplineDoubleKeyFrame { KeyTime = time, Value = to, KeySpline = Spline(curve) });
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
    /// An element's height eased from one value to another on the standard token, then left to its
    /// content again: a dialog moving to a taller or shorter step. With animation effects off it
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
        animation.KeyFrames.Add(new SplineDoubleKeyFrame { KeyTime = Standard, Value = to, KeySpline = SpringSpline() });
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
    /// A critically damped spring on a composition property ("Scale", "Offset"...) of
    /// <paramref name="visual"/>, quick or standard, from <paramref name="from"/> when given and
    /// otherwise from where it stands, so a spring that takes over keeps going from there. With
    /// animation effects off the property takes its end value at once.
    /// </summary>
    public static void Spring(Visual visual, string property, System.Numerics.Vector3 to, System.Numerics.Vector3? from = null, bool quick = false)
    {
        if (!Enabled)
        {
            visual.StopAnimation(property);
            if (property == "Scale")
                visual.Scale = to;
            else if (property == "Offset")
                visual.Offset = to;
            else if (property == "Translation")
                visual.Properties.InsertVector3("Translation", to);
            return;
        }
        var spring = visual.Compositor.CreateSpringVector3Animation();
        spring.DampingRatio = 1f;
        // Composition's period is the spring's 1/ω, not a full turn: a critically damped spring
        // settles in about 2π periods. The Mac's spring(duration:) settles in its duration, so
        // the token is divided by 2π; taken as it was, the hover card grew for over 1.5 s.
        spring.Period = TimeSpan.FromTicks((long)((quick ? Quick : Standard).Ticks / (2 * Math.PI)));
        spring.FinalValue = to;
        if (from is { } start)
            spring.InitialValue = start;
        visual.StartAnimation(property, spring);
    }
}

/// <summary>
/// A Fluent button whose hover and press tint comes and goes on the quick token, as the Mac's
/// buttons over the video do, rather than on Fluent's 83 ms. In XAML, on a button or in a style:
/// <c>local:QuickTint.On="True"</c>.
/// </summary>
public static class QuickTint
{
    public static readonly DependencyProperty OnProperty = DependencyProperty.RegisterAttached(
        "On", typeof(bool), typeof(QuickTint), new PropertyMetadata(false, (d, e) =>
        {
            if (d is Microsoft.UI.Xaml.Controls.Control control && e.NewValue is true)
            {
                control.Loaded += (_, _) => Soften(control);
                if (control.IsLoaded)
                    Soften(control);
            }
        }));

    public static bool GetOn(DependencyObject d) => (bool)d.GetValue(OnProperty);

    public static void SetOn(DependencyObject d, bool value) => d.SetValue(OnProperty, value);

    /// <summary>The template's own fill, Fluent's ContentPresenter, takes the quick token's time.</summary>
    static void Soften(Microsoft.UI.Xaml.Controls.Control control)
    {
        if (Microsoft.UI.Xaml.Media.VisualTreeHelper.GetChildrenCount(control) > 0
            && Microsoft.UI.Xaml.Media.VisualTreeHelper.GetChild(control, 0) is Microsoft.UI.Xaml.Controls.ContentPresenter presenter
            && presenter.BackgroundTransition?.Duration != Motion.Quick)
            presenter.BackgroundTransition = new BrushTransition { Duration = Motion.Quick };
    }
}
