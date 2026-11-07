using System.Diagnostics;
using System.Numerics;
using System.Runtime.CompilerServices;
using Kaeri.App;
using Microsoft.UI.Composition;
using Microsoft.UI.Input;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Hosting;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Windows.System;

namespace Kaeri.Windows;

/// <summary>
/// Smooth scrolling, the app's own setting on this device: on unless turned off, kept as a flag
/// file beside the app's other small choices.
/// </summary>
static class SmoothScrolling
{
    static string? path;

    public static bool On { get; private set; } = true;

    public static void Load(string configDir)
    {
        path = Path.Combine(configDir, "windows-wheel-steps");
        On = SmallFiles.Read(path) is null;
    }

    public static void Set(bool on)
    {
        if (on == On)
            return;
        On = on;
        if (path is null)
            return;
        if (on)
            SmallFiles.Delete(path);
        else
            SmallFiles.Write(path, "");
    }
}

/// <summary>
/// A mouse wheel that glides, as a browser's does: on a page's scroller, in XAML
/// <c>k:SmoothWheel.On="True"</c>. A notch from a mouse starts the page moving on the next frame
/// and eases it into place over about 130 ms (<see cref="WheelGlide"/>); notches that follow carry
/// on from the speed reached, so a fast spin is one glide. The glide runs on the compositor: the
/// scroller goes straight to the target, once a notch, and the content's Translation makes up the
/// difference and eases it away, so a busy UI thread (covers decoding, items realising) does not
/// hold the page still. A precision touchpad and touch scroll through DirectManipulation and are
/// left alone, as are the keyboard and the scrollbar, which stop a glide where it is. With Motion
/// off a notch moves at once; with Smooth scrolling off the scroller takes the wheel as Windows
/// does.
/// </summary>
public static class SmoothWheel
{
    public static readonly DependencyProperty OnProperty = DependencyProperty.RegisterAttached(
        "On", typeof(bool), typeof(SmoothWheel), new PropertyMetadata(false, (d, e) =>
        {
            if (d is ScrollViewer scroller && e.NewValue is true)
                Glider.Of(scroller).Hook();
        }));

    public static bool GetOn(DependencyObject d) => (bool)d.GetValue(OnProperty);

    public static void SetOn(DependencyObject d, bool value) => d.SetValue(OnProperty, value);

#if KAERI_TESTING
    /// <summary>KAERI_WHEEL=native in a testing run leaves the wheel to Windows, to compare against.</summary>
    static readonly bool Windows = Environment.GetEnvironmentVariable("KAERI_WHEEL") == "native";
#else
    const bool Windows = false;
#endif

    /// <summary>One frame of the main display, in seconds: 1/60 when Windows does not say.</summary>
    static readonly double FramePeriod = 1.0 / Math.Clamp(RefreshHz(), 30, 360);

    [System.Runtime.InteropServices.StructLayout(System.Runtime.InteropServices.LayoutKind.Sequential, CharSet = System.Runtime.InteropServices.CharSet.Unicode)]
    struct DevMode
    {
        [System.Runtime.InteropServices.MarshalAs(System.Runtime.InteropServices.UnmanagedType.ByValTStr, SizeConst = 32)] public string DeviceName;
        public short SpecVersion, DriverVersion, Size, DriverExtra;
        public int Fields, PositionX, PositionY, DisplayOrientation, DisplayFixedOutput;
        public short Color, Duplex, YResolution, TTOption, Collate;
        [System.Runtime.InteropServices.MarshalAs(System.Runtime.InteropServices.UnmanagedType.ByValTStr, SizeConst = 32)] public string FormName;
        public short LogPixels;
        public int BitsPerPel, PelsWidth, PelsHeight, DisplayFlags, DisplayFrequency;
        public int IcmMethod, IcmIntent, MediaType, DitherType, Reserved1, Reserved2, PanningWidth, PanningHeight;
    }

    [System.Runtime.InteropServices.DllImport("user32.dll", CharSet = System.Runtime.InteropServices.CharSet.Unicode)]
    static extern bool EnumDisplaySettingsW(string? device, int mode, ref DevMode devMode);

    static int RefreshHz()
    {
        var mode = new DevMode { Size = (short)System.Runtime.InteropServices.Marshal.SizeOf<DevMode>() };
        return EnumDisplaySettingsW(null, -1, ref mode) && mode.DisplayFrequency > 1 ? mode.DisplayFrequency : 60;
    }

    /// <summary>
    /// Shift or a sideways wheel along a shelf: glides it by <paramref name="by"/> when this is
    /// a mouse's notch and Smooth scrolling is on. False leaves the wheel to the caller.
    /// </summary>
    public static bool Sideways(ScrollViewer scroller, PointerRoutedEventArgs e, double by)
    {
        if (!Notch(e, scroller, out _))
            return false;
        Glider.Of(scroller).Push(across: true, by);
        return true;
    }

    /// <summary>Whether <paramref name="e"/> is a mouse wheel's whole notches, for the app to take.</summary>
    static bool Notch(PointerRoutedEventArgs e, UIElement over, out int delta)
    {
        delta = 0;
        if (!SmoothScrolling.On || Windows || e.Pointer.PointerDeviceType != PointerDeviceType.Mouse)
            return false;
        if (e.KeyModifiers.HasFlag(VirtualKeyModifiers.Control))
            return false;
        delta = e.GetCurrentPoint(over).Properties.MouseWheelDelta;
        return delta != 0 && delta % 120 == 0;
    }

    /// <summary>One scroller's glides, down and across.</summary>
    sealed class Glider
    {
        static readonly ConditionalWeakTable<ScrollViewer, Glider> all = new();

        public static Glider Of(ScrollViewer scroller) => all.GetValue(scroller, s => new Glider(s));

        /// <summary>How far a notch moves a page: about what Windows' own scroller moves it.</summary>
        const double NotchPixels = 108;

        readonly ScrollViewer scroller;
        readonly Axis down, across;
        readonly Stopwatch clock = Stopwatch.StartNew();
        UIElement? hooked;
        /// <summary>The content the glides move, and its composition visual.</summary>
        UIElement? content;
        Visual? visual;

        Glider(ScrollViewer scroller)
        {
            this.scroller = scroller;
            down = new(false);
            across = new(true);
            scroller.Unloaded += (_, _) => Stop();
            // The keyboard, the scrollbar, a click and a touchpad take over from a glide.
            scroller.AddHandler(UIElement.KeyDownEvent, new KeyEventHandler((_, _) => Stop()), true);
            scroller.AddHandler(UIElement.PointerPressedEvent, new PointerEventHandler((_, _) => Stop()), true);
            scroller.DirectManipulationStarted += (_, _) => Stop();
        }

        /// <summary>
        /// Hears the wheel inside the scroller before the scroller does: on its template's root,
        /// which every notch over the content or the scrollbar passes on the way up.
        /// </summary>
        public void Hook()
        {
            scroller.Loaded += (_, _) => Attach();
            if (scroller.IsLoaded)
                Attach();
        }

        void Attach()
        {
#if KAERI_TESTING
            Probe.Watch(scroller, Named(), () => (down.Shown(scroller.VerticalOffset, Now), across.Shown(scroller.HorizontalOffset, Now)));
#endif
            // Ready before the first notch, so that one starts on the next frame too.
            Prepare();
            var root = VisualTreeHelper.GetChildrenCount(scroller) > 0 ? VisualTreeHelper.GetChild(scroller, 0) as UIElement : null;
            root ??= scroller.Content as UIElement;
            if (root is null || root == hooked)
                return;
            hooked?.RemoveHandler(UIElement.PointerWheelChangedEvent, (PointerEventHandler)OnWheel);
            hooked = root;
            root.AddHandler(UIElement.PointerWheelChangedEvent, (PointerEventHandler)OnWheel, false);
        }

#if KAERI_TESTING
        /// <summary>The page and the scroller's name, for the probe.</summary>
        string Named()
        {
            DependencyObject? at = scroller;
            while (at is not null and not Page)
                at = VisualTreeHelper.GetParent(at);
            var page = at?.GetType().Name ?? "window";
            return string.IsNullOrEmpty(scroller.Name) ? page : $"{page}.{scroller.Name}";
        }
#endif

        double Now => clock.Elapsed.TotalSeconds;

        /// <summary>The content's visual with Translation turned on; false when there is no content to move.</summary>
        bool Prepare()
        {
            if (scroller.Content is not UIElement now)
                return false;
            if (now != content)
            {
                Stop();
                content = now;
                ElementCompositionPreview.SetIsTranslationEnabled(now, true);
                visual = ElementCompositionPreview.GetElementVisual(now);
            }
            return visual is not null;
        }

        void OnWheel(object sender, PointerRoutedEventArgs e)
        {
            if (e.Handled || !Notch(e, scroller, out var delta))
                return;
            var point = e.GetCurrentPoint(scroller).Properties;
            // Shift and a sideways wheel stay Windows' own on a page; a shelf calls Sideways.
            if (point.IsHorizontalMouseWheel || e.KeyModifiers.HasFlag(VirtualKeyModifiers.Shift))
                return;
            if (scroller.VerticalScrollMode == ScrollMode.Disabled || scroller.ScrollableHeight <= 0)
                return;
            e.Handled = true;
            Push(across: false, -delta / 120.0 * NotchPixels);
        }

        public void Push(bool across, double by)
        {
            var axis = across ? this.across : down;
            var offset = across ? scroller.HorizontalOffset : scroller.VerticalOffset;
            var end = Math.Max(0, across ? scroller.ScrollableWidth : scroller.ScrollableHeight);
            if (!Motion.Enabled || !Prepare())
            {
                Stop();
                Go(across, Math.Clamp(offset + by, 0, end));
                return;
            }
            if (axis.Glide(by, Now, offset, end, visual!) is { } to)
                Go(across, to);
        }

        void Go(bool across, double to)
        {
            if (across)
                scroller.ChangeView(to, null, null, true);
            else
                scroller.ChangeView(null, to, null, true);
        }

        /// <summary>Glides stop where the scroller is: the keyboard, the scrollbar or the page moved it.</summary>
        void Stop()
        {
            if (visual is null || !(down.Gliding || across.Gliding))
                return;
            down.Stop(visual);
            across.Stop(visual);
            visual.Properties.InsertVector3("Translation", Vector3.Zero);
        }
    }

    /// <summary>The glide along one axis, and the Translation channel that shows it.</summary>
    sealed class Axis(bool across)
    {
        readonly WheelGlide glide = new();
        readonly string channel = across ? "Translation.X" : "Translation.Y";
        /// <summary>Where the last notch sent the scroller, while its glide is on the way.</summary>
        double? sent;

        public bool Gliding => sent is not null;

        /// <summary>
        /// Moves the glide's target by <paramref name="by"/>, and starts the content easing there
        /// from wherever it shows now: the offset to send the scroller to, or null when it stays.
        /// </summary>
        public double? Glide(double by, double now, double offset, double end, Visual visual)
        {
            // A glide that has arrived, or whose scroller something else moved, is over: the next
            // starts from the scroller as it stands.
            if (!glide.Running(now) || sent is not { } was || Math.Abs(was - offset) > 1)
            {
                sent = null;
                glide.Stop();
            }
            var from = sent ?? offset;
            glide.Push(by, now, offset, 0, end);
            var to = glide.Target;
            if (Math.Abs(to - from) < 0.01)
                return null;
            sent = to;
            var (start, speed, length) = glide.Segment;
            var distance = to - start;
            // The glide's cubic as a curve from 0 to 1: with its control points a third and two
            // thirds along, time runs straight and the value follows the cubic exactly.
            var lead = Math.Abs(distance) < 0.01 ? 0 : Math.Clamp(speed * length / distance, 0, 3) / 3;
            // An animation shows its first frame where it starts, so the glide's first frame
            // would not move: it starts a frame in instead, the curve cut there (de Casteljau).
            var skip = Math.Min(FramePeriod / length, 0.5);
            double Lerp(double a, double b) => a + (b - a) * skip;
            double p01 = Lerp(0, lead), p12 = Lerp(lead, 1), p23 = 1;
            double p012 = Lerp(p01, p12), p123 = Lerp(p12, p23);
            var done = Lerp(p012, p123);
            var rest = 1 - done;
            var compositor = visual.Compositor;
            var animation = compositor.CreateScalarKeyFrameAnimation();
            // The content shows where it was (its Translation now, plus the scroller's jump), a
            // frame along the glide.
            animation.InsertExpressionKeyFrame(0, string.Create(System.Globalization.CultureInfo.InvariantCulture, $"this.StartingValue + {to - from - distance * done}"));
            animation.InsertKeyFrame(1, 0, compositor.CreateCubicBezierEasingFunction(
                new(1 / 3f, (float)(rest > 1e-6 ? (p123 - done) / rest : 1)), new(2 / 3f, (float)(rest > 1e-6 ? (p23 - done) / rest : 1))));
            animation.Duration = TimeSpan.FromSeconds(Math.Max(length * (1 - skip), 0.001));
            visual.StartAnimation(channel, animation);
            return to;
        }

        /// <summary>Where the page shows along this axis: the scroller's offset less what the glide has still to go.</summary>
        public double Shown(double offset, double now) =>
            sent is { } to && glide.Running(now) ? offset - (to - glide.Sample(now).Position) : offset;

        public void Stop(Visual visual)
        {
            if (sent is null)
                return;
            sent = null;
            glide.Stop();
            visual.StopAnimation(channel);
        }
    }
}
