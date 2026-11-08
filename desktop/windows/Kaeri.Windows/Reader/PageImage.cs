using Medium = Kaeri.Engine.Core.Medium;
using System.Runtime.InteropServices.WindowsRuntime;
using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using Microsoft.UI.Xaml.Shapes;

namespace Kaeri.Windows.Pages;

/// <summary>
/// Page images from the engine's files: decoded by the framework off the UI thread at the size
/// they show, and the last few kept so turning back draws at once. A raw file (grey or RGBA
/// pixels) is converted on a worker thread.
/// </summary>
static class PageImages
{
    sealed record Entry(ImageSource Source, bool Ready);

    static readonly LinkedList<(string Key, ImageSource Source)> recent = new();
    static readonly Dictionary<string, LinkedListNode<(string Key, ImageSource Source)>> byKey = [];
    static readonly HashSet<ImageSource> opened = [];
    const int Keep = 18;

    /// <summary>The width a file decodes to for a box <paramref name="pixels"/> wide, in steps so a resize reuses it.</summary>
    static int DecodeWidth(PageFile file, double pixels)
    {
        var full = (int)Math.Max(file.Width, 1);
        var step = (int)Math.Ceiling(Math.Max(pixels, 64) / 256) * 256;
        return Math.Min(full, step);
    }

    /// <summary>
    /// The image for <paramref name="file"/> about <paramref name="pixels"/> wide (the whole file,
    /// before any crop). <paramref name="ready"/> says it is decoded already and draws with no fade.
    /// </summary>
    public static ImageSource? Get(PageFile file, double pixels, out bool ready)
    {
        ready = false;
        var width = DecodeWidth(file, pixels);
        var key = file.Path + "|" + width;
        if (byKey.TryGetValue(key, out var node))
        {
            recent.Remove(node);
            recent.AddFirst(node);
            ready = opened.Contains(node.Value.Source);
            return node.Value.Source;
        }
        ImageSource? source = file.Encoding switch
        {
            PageEncoding.Raw raw => Raw(file, raw),
            _ => Original(file, width),
        };
        if (source is null)
            return null;
        byKey[key] = recent.AddFirst((key, source));
        while (recent.Count > Keep && recent.Last is { } last)
        {
            recent.RemoveLast();
            byKey.Remove(last.Value.Key);
            opened.Remove(last.Value.Source);
        }
        return source;
    }

    static BitmapImage? Original(PageFile file, int width)
    {
        if (!Uri.TryCreate(file.Path, UriKind.Absolute, out var uri))
            return null;
        var bitmap = new BitmapImage { DecodePixelType = DecodePixelType.Physical };
        if (width < file.Width)
            bitmap.DecodePixelWidth = width;
        bitmap.ImageOpened += (_, _) => opened.Add(bitmap);
        bitmap.UriSource = uri;
        return bitmap;
    }

    /// <summary>Grey or RGBA pixels the engine left undecoded, made into BGRA on a worker thread.</summary>
    static WriteableBitmap? Raw(PageFile file, PageEncoding.Raw raw)
    {
        int width = (int)file.Width, height = (int)file.Height;
        if (width <= 0 || height <= 0)
            return null;
        var bitmap = new WriteableBitmap(width, height);
        var queue = DispatcherQueue.GetForCurrentThread();
        var path = file.Path;
        var stride = (int)raw.Stride;
        var grey = raw.Format == PixelFormat.Gray8;
        _ = Task.Run(() =>
        {
            byte[] bgra;
            try
            {
                var bytes = File.ReadAllBytes(path);
                bgra = new byte[width * height * 4];
                for (var y = 0; y < height; y++)
                {
                    var row = y * stride;
                    for (var x = 0; x < width; x++)
                    {
                        var o = (y * width + x) * 4;
                        if (grey)
                        {
                            var v = row + x < bytes.Length ? bytes[row + x] : (byte)0;
                            bgra[o] = v;
                            bgra[o + 1] = v;
                            bgra[o + 2] = v;
                            bgra[o + 3] = 255;
                        }
                        else
                        {
                            var i = row + x * 4;
                            if (i + 3 >= bytes.Length)
                                continue;
                            // Premultiplied, as the bitmap wants it.
                            var a = bytes[i + 3];
                            bgra[o] = (byte)(bytes[i + 2] * a / 255);
                            bgra[o + 1] = (byte)(bytes[i + 1] * a / 255);
                            bgra[o + 2] = (byte)(bytes[i] * a / 255);
                            bgra[o + 3] = a;
                        }
                    }
                }
            }
            catch (Exception e) when (e is IOException or UnauthorizedAccessException)
            {
                return;
            }
            queue.TryEnqueue(() =>
            {
                using (var stream = bitmap.PixelBuffer.AsStream())
                    stream.Write(bgra, 0, bgra.Length);
                bitmap.Invalidate();
                opened.Add(bitmap);
                Painted?.Invoke(bitmap);
            });
        });
        return bitmap;
    }

    /// <summary>A raw page finished converting.</summary>
    public static event Action<ImageSource>? Painted;

    public static bool IsOpened(ImageSource source) => opened.Contains(source);

    /// <summary>The engine re-made the files (a retry): nothing kept is trusted.</summary>
    public static void Forget()
    {
        recent.Clear();
        byKey.Clear();
        opened.Clear();
    }
}

/// <summary>
/// One page's image fitted in a box, cropped to the rectangle the engine found when the file
/// says so, aligned to the spine when it is half of two. The image fades in when it decodes,
/// with reduced motion too (a fade is not movement); one that was ready draws on the first
/// frame, and Paper mode cuts.
/// </summary>
sealed class PageImageView : Canvas
{
    readonly Image image = new() { Stretch = Stretch.Fill };
    readonly Rectangle dim = new() { Fill = new SolidColorBrush(Microsoft.UI.Colors.Black), Opacity = 0.07, IsHitTestVisible = false, Visibility = Visibility.Collapsed };
    readonly Canvas frame = new();
    PageFile? file;
    ImageSource? source;
    double scale = 1;

    /// <summary>How the page meets its box: centred, or against the spine of a spread.</summary>
    public HorizontalAlignment Align { get; set; } = HorizontalAlignment.Center;

    /// <summary>Fill the box's width and let the height run (Fit width).</summary>
    public bool FitWidth { get; set; }

    /// <summary>Fill the box's height and let the width run (Fit height).</summary>
    public bool FitHeight { get; set; }

    /// <summary>The image is on screen, its fade under way.</summary>
    public event Action? Painted;

    /// <summary>The image is fully on screen: its fade, if it had one, is over.</summary>
    public event Action? Settled;

    public bool IsPainted { get; private set; }

    /// <summary>Paper mode: the image appears at once, with no fade.</summary>
    public bool Cut { get; set; }

    public PageImageView()
    {
        frame.Children.Add(image);
        frame.Children.Add(dim);
        Children.Add(frame);
        AutomationProperties.SetAccessibilityView(image, AccessibilityView.Raw);
        image.ImageOpened += (_, _) => Shown(fade: true);
        Loaded += (_, _) => PageImages.Painted += OnRawPainted;
        Unloaded += (_, _) => PageImages.Painted -= OnRawPainted;
        SizeChanged += (_, _) => Place();
    }

    void OnRawPainted(ImageSource painted)
    {
        if (painted == source)
            Shown(fade: true);
    }

    /// <summary>Paper mode dims the page's white a little instead of tinting it.</summary>
    public bool Dim
    {
        set => dim.Visibility = value ? Visibility.Visible : Visibility.Collapsed;
    }

    public void Show(PageFile next, double rasterScale)
    {
        scale = rasterScale;
        if (file is { } was && was.Path == next.Path && was.Crop == next.Crop && source is not null)
        {
            Place();
            return;
        }
        file = next;
        IsPainted = false;
        image.Opacity = 0;
        Place();
    }

    /// <summary>The size the page takes in a box of <paramref name="box"/>, cropped and fitted.</summary>
    public static global::Windows.Foundation.Size Fitted(PageFile file, global::Windows.Foundation.Size box, bool fitWidth, bool fitHeight)
    {
        double w = file.Crop?.Width ?? file.Width, h = file.Crop?.Height ?? file.Height;
        if (w <= 0 || h <= 0 || box.Width <= 0 || box.Height <= 0)
            return new(0, 0);
        var s = fitWidth ? box.Width / w : fitHeight ? box.Height / h : Math.Min(box.Width / w, box.Height / h);
        return new(w * s, h * s);
    }

    void Place()
    {
        if (file is not { } f || ActualWidth <= 0 || ActualHeight <= 0)
            return;
        var shown = Fitted(f, new(ActualWidth, ActualHeight), FitWidth, FitHeight);
        if (shown.Width <= 0)
            return;
        double cw = f.Crop?.Width ?? f.Width, ch = f.Crop?.Height ?? f.Height;
        var s = shown.Width / cw;
        // The whole file at this scale, moved so the crop's corner sits at the frame's.
        image.Width = f.Width * s;
        image.Height = f.Height * s;
        SetLeft(image, -(f.Crop?.X ?? 0) * s);
        SetTop(image, -(f.Crop?.Y ?? 0) * s);
        frame.Width = shown.Width;
        frame.Height = shown.Height;
        dim.Width = shown.Width;
        dim.Height = shown.Height;
        frame.Clip = new RectangleGeometry { Rect = new(0, 0, shown.Width, shown.Height) };
        var left = Align switch
        {
            HorizontalAlignment.Left => 0,
            HorizontalAlignment.Right => ActualWidth - shown.Width,
            _ => (ActualWidth - shown.Width) / 2,
        };
        SetLeft(frame, FitWidth ? 0 : left);
        SetTop(frame, FitHeight || FitWidth ? Math.Max(0, (ActualHeight - shown.Height) / 2) : (ActualHeight - shown.Height) / 2);
        var next = PageImages.Get(f, f.Width * s * scale, out var ready);
        if (next != source)
        {
            source = next;
            image.Source = next;
            if (ready)
                Shown(fade: false);
        }
    }

    void Shown(bool fade)
    {
        if (IsPainted)
            return;
        IsPainted = true;
        Painted?.Invoke();
        if (fade && !Cut)
            Motion.FadeInThen(image, () => Settled?.Invoke());
        else
        {
            image.Opacity = 1;
            Settled?.Invoke();
        }
    }
}

/// <summary>
/// The Raised plate that holds a page's place with its number. Nothing shows for 600 ms, and a
/// line says it is loading from 4 s. A plate that takes over from one already on screen (the
/// opening plate) shows at once, so nothing blinks between them.
/// </summary>
sealed class PagePlate : Grid
{
    readonly TextBlock number;
    readonly TextBlock line;
    readonly DispatcherQueueTimer first, slow;

    public PagePlate(int page, ReaderPalette palette, bool atOnce = false)
        : this(page.ToString(System.Globalization.CultureInfo.CurrentCulture), T.ReaderPageLoading(page), palette, atOnce)
    {
    }

    public PagePlate(string label, string loading, ReaderPalette palette, bool atOnce)
    {
        Background = new SolidColorBrush(palette.Raised);
        Opacity = atOnce ? 1 : 0;
        IsHitTestVisible = false;
        number = new TextBlock { Text = label, FontSize = 15, Foreground = new SolidColorBrush(palette.Haze), HorizontalAlignment = HorizontalAlignment.Center, TextWrapping = TextWrapping.WrapWholeWords, TextAlignment = TextAlignment.Center };
        line = new TextBlock { Text = loading, FontSize = 13, Foreground = new SolidColorBrush(palette.Haze), HorizontalAlignment = HorizontalAlignment.Center, Visibility = Visibility.Collapsed, TextWrapping = TextWrapping.WrapWholeWords, TextAlignment = TextAlignment.Center };
        var stack = new StackPanel { Spacing = 6, VerticalAlignment = VerticalAlignment.Center, HorizontalAlignment = HorizontalAlignment.Center, Margin = new Thickness(16) };
        stack.Children.Add(number);
        stack.Children.Add(line);
        Children.Add(stack);
        first = Timers.Weak(this, TimeSpan.FromMilliseconds(600), false, p => Motion.FadeIn(p));
        slow = Timers.Weak(this, TimeSpan.FromMilliseconds(4000), false, p => Motion.Show(p.line));
        Loaded += (_, _) =>
        {
            if (Opacity < 1)
                first.Start();
            slow.Start();
        };
        Unloaded += (_, _) =>
        {
            first.Stop();
            slow.Stop();
        };
    }

    /// <summary>The plate is on screen, or on its way.</summary>
    public bool Shown => Opacity > 0;

    /// <summary>The loading line says something else from now on (how long a source search has taken).</summary>
    public void Loading(string text) => line.Text = text;
}

/// <summary>
/// A page that failed: left-aligned in the page's place, vermilion only on the glyph, Try again
/// as the one action, and turning past it works.
/// </summary>
sealed class PageFailedView : Grid
{
    public PageFailedView(int page, string? words, ReaderPalette palette, Action retry)
    {
        Background = new SolidColorBrush(palette.Raised);
        var stack = new StackPanel
        {
            Spacing = 10,
            MaxWidth = 320,
            HorizontalAlignment = HorizontalAlignment.Left,
            VerticalAlignment = VerticalAlignment.Top,
            Margin = new Thickness(32, 120, 32, 32),
        };
        var glyph = new FontIcon { Glyph = "", FontSize = 22, Foreground = new SolidColorBrush(palette.Vermilion), HorizontalAlignment = HorizontalAlignment.Left };
        AutomationProperties.SetAccessibilityView(glyph, AccessibilityView.Raw);
        stack.Children.Add(glyph);
        stack.Children.Add(new TextBlock { Text = T.ReaderPageFailed(page), FontSize = 17, FontWeight = Microsoft.UI.Text.FontWeights.SemiBold, Foreground = new SolidColorBrush(palette.Silk), TextWrapping = TextWrapping.WrapWholeWords });
        if (!string.IsNullOrEmpty(words))
            stack.Children.Add(new TextBlock { Text = words, FontSize = 15, Foreground = new SolidColorBrush(palette.Haze), TextWrapping = TextWrapping.WrapWholeWords });
        var again = ReaderButtons.Pill(T.ReaderTryAgain, palette, (_, _) => retry());
        again.Margin = new Thickness(0, 4, 0, 0);
        stack.Children.Add(again);
        stack.Children.Add(new TextBlock { Text = T.WindowsReaderPageFailedHint, FontSize = 13, Foreground = new SolidColorBrush(palette.Haze), TextWrapping = TextWrapping.WrapWholeWords });
        Children.Add(stack);
    }
}

/// <summary>
/// One part of a leaf: the plate until the image is on screen, the image over it, or the error
/// in its place.
/// </summary>
sealed class PartView : Grid
{
    readonly ReaderSession reader;
    readonly LeafPart part;
    readonly string chapter;
    readonly ReaderPalette palette;
    readonly PageImageView image = new();
    readonly bool atOnce;
    readonly Func<LeafPart, PageFile?>? kept;
    PagePlate? plate;
    PageFailedView? failed;

    /// <summary>
    /// <paramref name="atOnce"/>: the plate shows from the first frame, as it takes over from the
    /// opening plate. <paramref name="kept"/> names the file a part shows when the session no
    /// longer has it (a chapter a strip ran on from).
    /// </summary>
    public PartView(ReaderSession reader, LeafPart part, string chapter, ReaderPalette palette, HorizontalAlignment align, bool fitWidth, bool fitHeight, string label, bool atOnce = false, Func<LeafPart, PageFile?>? kept = null)
    {
        this.reader = reader;
        this.part = part;
        this.chapter = chapter;
        this.palette = palette;
        this.atOnce = atOnce;
        this.kept = kept;
        image.Align = align;
        image.FitWidth = fitWidth;
        image.FitHeight = fitHeight;
        image.Dim = palette.Paper;
        image.Cut = palette.Paper;
        // The plate stays under the image until the image's fade is over: the page never fades
        // in over bare ground.
        image.Settled += () =>
        {
            if (plate is not null)
            {
                Children.Remove(plate);
                plate = null;
            }
        };
        AutomationProperties.SetName(this, label);
        Children.Add(image);
        if (part.Rotate)
        {
            image.RenderTransformOrigin = new global::Windows.Foundation.Point(0.5, 0.5);
            image.RenderTransform = new RotateTransform { Angle = 90 };
        }
        SizeChanged += (_, _) => Fit();
        Refresh(1);
    }

    /// <summary>A rotated spread lies on its side: the image's box is the part's box turned.</summary>
    void Fit()
    {
        if (!part.Rotate)
            return;
        image.Width = ActualHeight;
        image.Height = ActualWidth;
        image.HorizontalAlignment = HorizontalAlignment.Center;
        image.VerticalAlignment = VerticalAlignment.Center;
        image.Margin = new Thickness((ActualWidth - ActualHeight) / 2, (ActualHeight - ActualWidth) / 2, (ActualWidth - ActualHeight) / 2, (ActualHeight - ActualWidth) / 2);
    }

    public bool Painted => image.IsPainted;

    /// <summary>The engine's files or failures changed: show what there is now.</summary>
    public void Refresh(double rasterScale)
    {
        var file = reader.FileOf(part, chapter) ?? kept?.Invoke(part);
        // A chapter the strip ran on from has no failures of its own any more; the open one does.
        var failure = file is null && (kept is null || reader.View?.Chapter.Id == chapter) ? reader.FailureOf(part, chapter) : null;
        if (failure is not null)
        {
            if (failed is null)
            {
                failed = new PageFailedView((int)part.Page + 1, failure.Words(), palette, () => reader.Retry(part.Page));
                Children.Add(failed);
            }
            if (plate is not null)
            {
                Children.Remove(plate);
                plate = null;
            }
            return;
        }
        if (failed is not null)
        {
            Children.Remove(failed);
            failed = null;
        }
        if (file is not null)
            image.Show(file, rasterScale);
        if (!image.IsPainted && plate is null)
        {
            plate = new PagePlate((int)part.Page + 1, palette, atOnce);
            Children.Insert(0, plate);
        }
    }
}

/// <summary>The reader's two kinds of button: the quiet pill and the one amber action.</summary>
static class ReaderButtons
{
    public static Button Pill(string text, ReaderPalette palette, RoutedEventHandler click, double height = 40)
    {
        var button = new Button
        {
            Content = text,
            MinHeight = height,
            Padding = new Thickness(18, 0, 18, 0),
            CornerRadius = new CornerRadius(height / 2),
            Background = new SolidColorBrush(palette.High),
            Foreground = new SolidColorBrush(palette.Silk),
            BorderThickness = new Thickness(0),
            FontSize = 15,
            FontWeight = Microsoft.UI.Text.FontWeights.SemiBold,
        };
        Brushes(button, palette.High, palette.Silk);
        button.Click += click;
        return button;
    }

    public static Button Amber(string text, ReaderPalette palette, RoutedEventHandler click, double height = 48)
    {
        var button = new Button
        {
            Content = text,
            MinHeight = height,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            CornerRadius = new CornerRadius(height / 2),
            Background = new SolidColorBrush(palette.Amber),
            Foreground = new SolidColorBrush(palette.OnAmber),
            BorderThickness = new Thickness(0),
            FontSize = 17,
            FontWeight = Microsoft.UI.Text.FontWeights.SemiBold,
        };
        Brushes(button, palette.Amber, palette.OnAmber);
        button.Click += click;
        return button;
    }

    /// <summary>A quiet text button: no fill until the pointer is over it.</summary>
    public static Button Plain(string text, ReaderPalette palette, RoutedEventHandler click, double height = 44)
    {
        var button = new Button
        {
            Content = text,
            MinHeight = height,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            Padding = new Thickness(12, 0, 12, 0),
            CornerRadius = new CornerRadius(height / 2),
            Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
            Foreground = new SolidColorBrush(palette.Silk),
            BorderThickness = new Thickness(0),
            FontSize = 15,
            FontWeight = Microsoft.UI.Text.FontWeights.SemiBold,
        };
        Brushes(button, Microsoft.UI.Colors.Transparent, palette.Silk);
        button.Resources["ButtonBackgroundPointerOver"] = new SolidColorBrush(ReaderPalette.WithAlpha(palette.Silk, 0.08));
        button.Resources["ButtonBackgroundPressed"] = new SolidColorBrush(ReaderPalette.WithAlpha(palette.Silk, 0.05));
        button.Click += click;
        return button;
    }

    /// <summary>The button keeps its colours on hover and press, a little lighter or darker.</summary>
    static void Brushes(Button button, global::Windows.UI.Color fill, global::Windows.UI.Color text)
    {
        var resources = button.Resources;
        resources["ButtonBackground"] = new SolidColorBrush(fill);
        resources["ButtonBackgroundPointerOver"] = new SolidColorBrush(ReaderPalette.WithAlpha(fill, 0.9));
        resources["ButtonBackgroundPressed"] = new SolidColorBrush(ReaderPalette.WithAlpha(fill, 0.8));
        resources["ButtonForeground"] = new SolidColorBrush(text);
        resources["ButtonForegroundPointerOver"] = new SolidColorBrush(text);
        resources["ButtonForegroundPressed"] = new SolidColorBrush(text);
        resources["ButtonBorderBrush"] = new SolidColorBrush(Microsoft.UI.Colors.Transparent);
        resources["ButtonBorderBrushPointerOver"] = new SolidColorBrush(Microsoft.UI.Colors.Transparent);
        resources["ButtonBorderBrushPressed"] = new SolidColorBrush(Microsoft.UI.Colors.Transparent);
    }
}

/// <summary>What the reader says to Narrator: a page turn as a notification, a line as a live region.</summary>
static class ReaderSpeech
{
    /// <summary>
    /// Says <paramref name="text"/> once, replacing anything the reader said before that has not
    /// been read yet, so a run of turns ends on the page reached.
    /// </summary>
    public static void Say(UIElement from, string text, string activity = "kaeri-reader-place")
    {
        if (string.IsNullOrEmpty(text))
            return;
        var peer = FrameworkElementAutomationPeer.FromElement(from) ?? FrameworkElementAutomationPeer.CreatePeerForElement(from);
        peer?.RaiseNotificationEvent(AutomationNotificationKind.ActionCompleted, AutomationNotificationProcessing.ImportantMostRecent, text, activity);
    }

    /// <summary>The element's text changed: a live region says it.</summary>
    public static void Raise(UIElement element)
    {
        var peer = FrameworkElementAutomationPeer.FromElement(element) ?? FrameworkElementAutomationPeer.CreatePeerForElement(element);
        peer?.RaiseAutomationEvent(AutomationEvents.LiveRegionChanged);
    }
}
