using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Kaeri.Windows.Pages;

/// <summary>
/// The five states of one place on a page, after the spec: nothing for the first 600 ms, then
/// static placeholders in the real layout, the job's activity line under them past 4 s, one
/// crossfade to the content, and in place of content an empty state, "You're offline" or a
/// failure with Try again. The first child (from XAML) is the content.
/// </summary>
public sealed partial class StateHost : Grid
{
    static readonly TimeSpan Silence = TimeSpan.FromMilliseconds(600), Activity = TimeSpan.FromSeconds(4), Least = TimeSpan.FromMilliseconds(300);

    readonly DispatcherQueueTimer silence, activity;
    readonly StackPanel waiting = new() { Spacing = 12, Visibility = Visibility.Collapsed };
    readonly Grid message = new() { Visibility = Visibility.Collapsed };
    readonly TextBlock activityLine;
    DateTimeOffset placeholdersSince;
    bool loading, placeholdersShown, layered;

    public StateHost()
    {
        silence = Timers.Weak(this, Silence, false, static host => host.ShowWaiting());
        activity = Timers.Weak(this, Activity, false, static host => Motion.Show(host.activityLine));
        activityLine = Ui.Text("", "HazeCaptionStyle");
        activityLine.Visibility = Visibility.Collapsed;
        AutomationProperties.SetLiveSetting(activityLine, AutomationLiveSetting.Polite);
    }

    /// <summary>Builds the placeholders in the content's layout.</summary>
    public Func<UIElement>? Placeholders { get; set; }

    /// <summary>The job's activity line past 4 s: "Loading this season".</summary>
    public string ActivityText { get; set; } = T.StateLoading;

    /// <summary>
    /// Whether the place loads over the network. A place whose answer is on this device, such as
    /// the downloads or the saved sources, keeps its placeholders while offline instead of saying
    /// "You're offline".
    /// </summary>
    public bool NeedsConnection { get; set; } = true;

    UIElement? Content => Children.Count > 0 && Children[0] != waiting && Children[0] != message ? Children[0] : null;

    void Layer()
    {
        if (layered)
            return;
        layered = true;
        Children.Add(waiting);
        Children.Add(message);
    }

    /// <summary>
    /// A load has started. With content already showing, it stays until the answer; on a first
    /// visit the place stays empty for 600 ms, then shows placeholders.
    /// </summary>
    public void Wait(bool hasContent = false)
    {
        Layer();
        // Already waiting: a second ask does not restart the 600 ms.
        if (loading && !hasContent && (silence.IsRunning || placeholdersShown))
            return;
        loading = true;
        if (hasContent)
            return;
        Motion.Set(message, false);
        if (Content is { } content && !placeholdersShown)
            content.Visibility = Visibility.Collapsed;
        silence.Stop();
        silence.Start();
    }

    /// <summary>
    /// A new load in place of what shows, such as another season or other filters: what shows
    /// stays for the first 600 ms, then gives way to the placeholders. With nothing showing, the
    /// placeholders come at once, in the content's own size, so the place is never blank.
    /// </summary>
    public void WaitKeeping(bool showing)
    {
        if (!showing)
        {
            if (!(loading && placeholdersShown))
                WaitNow();
            return;
        }
        Layer();
        if (loading && (silence.IsRunning || placeholdersShown))
            return;
        loading = true;
        Motion.Set(message, false);
        if (Content is { } content)
            content.Visibility = Visibility.Visible;
        silence.Stop();
        silence.Start();
    }

    void ShowWaiting()
    {
        if (!loading)
            return;
        if (Content is { } shown)
            shown.Visibility = Visibility.Collapsed;
        if (NeedsConnection && Ctx.Current.Known.Offline)
        {
            Offline();
            return;
        }
        waiting.Children.Clear();
        if (Placeholders?.Invoke() is { } placeholders)
            waiting.Children.Add(placeholders);
        activityLine.Text = ActivityText;
        activityLine.Visibility = Visibility.Collapsed;
        waiting.Children.Add(activityLine);
        placeholdersShown = true;
        placeholdersSince = DateTimeOffset.UtcNow;
        Motion.Set(waiting, true);
        activity.Stop();
        activity.Start();
    }

    /// <summary>
    /// Placeholders now, with no fade of their own: for a place whose caller crossfades it with
    /// both sides drawn, as Trending's period switch does once the old covers have stood 600 ms.
    /// </summary>
    public void WaitNow()
    {
        Layer();
        loading = true;
        silence.Stop();
        Motion.Set(message, false);
        if (Content is { } content)
            content.Visibility = Visibility.Collapsed;
        waiting.Children.Clear();
        if (Placeholders?.Invoke() is { } placeholders)
            waiting.Children.Add(placeholders);
        activityLine.Text = ActivityText;
        activityLine.Visibility = Visibility.Collapsed;
        waiting.Children.Add(activityLine);
        placeholdersShown = true;
        placeholdersSince = DateTimeOffset.UtcNow;
        Motion.Set(waiting, true);
        activity.Stop();
        activity.Start();
    }

    /// <summary>When the placeholders came, while they show.</summary>
    public DateTimeOffset? PlaceholdersSince => placeholdersShown ? placeholdersSince : null;

    /// <summary>The content now, with no fade of its own: the caller crossfades the place.</summary>
    public void DoneNow()
    {
        Layer();
        loading = false;
        silence.Stop();
        activity.Stop();
        Motion.Set(message, false);
        placeholdersShown = false;
        Motion.Set(waiting, false);
        if (Content is { } content)
            content.Visibility = Visibility.Visible;
    }

    /// <summary>The answer: the content, crossfaded over the placeholders if they showed.</summary>
    public void Done() => Settle(null);

    /// <summary>An answer with nothing in it: the empty state in the content's place.</summary>
    public void Empty(UIElement state) => Settle(state);

    /// <summary>Failed to load: a plain line and Try again. The engine's reason goes to the log.</summary>
    public void Failed(string reason, Action retry)
    {
        if (NeedsConnection && Ctx.Current.Known.Offline)
        {
            Settle(OfflineState());
            return;
        }
        KaeriFfiMethods.LogLine(FfiLogLevel.Warn, "windows", $"a page didn't load: {reason}");
        Settle(States.Unavailable(T.StateLoadFailed, T.StateCheckConnection, T.ActionTryAgain, () =>
        {
            Wait();
            retry();
        }, error: true));
    }

    /// <summary>
    /// Ends a load with the content (<paramref name="state"/> null) or a state in its place. When
    /// the placeholders showed, they stay their 0.3 s and then give way in one crossfade on the
    /// standard token; otherwise the answer simply shows. A state already showing is replaced in
    /// place, without fading out and in again. The last answer wins, also inside the 0.3 s.
    /// </summary>
    async void Settle(UIElement? state)
    {
        Layer();
        loading = false;
        silence.Stop();
        activity.Stop();
        var turn = ++settles;
        if (state is null)
            Motion.Set(message, false);
        if (placeholdersShown)
        {
            var left = Least - (DateTimeOffset.UtcNow - placeholdersSince);
            if (left > TimeSpan.Zero)
                await Task.Delay(left);
            // A later load or a later answer has this in hand.
            if (loading || turn != settles || !placeholdersShown)
                return;
            placeholdersShown = false;
            Motion.HideStandard(waiting);
            if (state is null)
            {
                if (Content is { } content)
                    Motion.FadeIn(content, Motion.Standard);
            }
            else
            {
                if (Content is { } content)
                    content.Visibility = Visibility.Collapsed;
                Say(state, Motion.Standard);
            }
        }
        else
        {
            Motion.Set(waiting, false);
            if (Content is { } content)
                content.Visibility = state is null ? Visibility.Visible : Visibility.Collapsed;
            if (state is not null)
                Say(state, Motion.FadeInTime);
        }
    }

    int settles;

    /// <summary>Offline with nothing cached: "You're offline" with Show downloads.</summary>
    void Offline()
    {
        loading = false;
        silence.Stop();
        activity.Stop();
        Motion.Set(waiting, false);
        placeholdersShown = false;
        settles++;
        Say(OfflineState(), Motion.FadeInTime);
    }

    static StackPanel OfflineState() =>
        States.Unavailable(T.StateOfflineTitle, T.StateOfflineDetail, T.StateOfflineShowDownloads, () => Ctx.Current.Window.Go("downloads"));

    void Say(UIElement state, TimeSpan fade)
    {
        Layer();
        var showing = message.Visibility == Visibility.Visible && message.Children.Count > 0;
        message.Children.Clear();
        message.Children.Add(state);
        FitMessage();
        if (showing)
            Motion.Set(message, true);
        else
            Motion.FadeIn(message, fade);
    }

    ScrollViewer? scroller;

    /// <summary>
    /// An empty, offline or failed state sits in the middle of what can be seen below the place's
    /// top, as on the Mac. In a scroller the place has no height of its own to centre in, so the
    /// message is given the viewport's.
    /// </summary>
    void FitMessage()
    {
        if (scroller is null)
        {
            for (var at = VisualTreeHelper.GetParent(this); at is not null; at = VisualTreeHelper.GetParent(at))
                if (at is ScrollViewer found)
                {
                    scroller = found;
                    scroller.SizeChanged += (_, _) => FitMessage();
                    break;
                }
            if (scroller is null)
                return;
        }
        if (scroller.Content is not UIElement inner || message.Children.Count == 0)
            return;
        try
        {
            var top = TransformToVisual(inner).TransformPoint(default).Y + Padding.Top;
            // The page's own bottom padding stays below it, so the message never makes the page scroll.
            message.MinHeight = Math.Max(0, scroller.ViewportHeight - top - Padding.Bottom - 24);
        }
        catch (ArgumentException)
        {
            // Not in the scroller's tree yet: the next size change places it.
        }
    }

    /// <summary>Clears any empty, offline or failed state; the content shows.</summary>
    public void Clear()
    {
        Done();
    }
}

/// <summary>The state pieces pages share: empty states, the offline line, placeholders.</summary>
static partial class States
{
    /// <summary>
    /// An empty state: Nagare (cell 5) above a title, one line and at most one button, centred in
    /// the place it fills.
    /// </summary>
    public static StackPanel Empty(string title, string line, string? button = null, Action? act = null, int cell = 5)
    {
        var panel = new StackPanel { Spacing = 8, HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center, MaxWidth = 420, Margin = new Thickness(0, 48, 0, 48) };
        var nagare = new Nagare(cell) { HorizontalAlignment = HorizontalAlignment.Center, Margin = new Thickness(0, 0, 0, 12) };
        panel.Children.Add(nagare);
        Words(panel, title, line, button, act, error: false);
        return panel;
    }

    /// <summary>
    /// The small unavailable state, with no Nagare: offline, failed, a filtered list. A failure
    /// has the vermilion glyph beside its title, and nothing else in vermilion.
    /// </summary>
    public static StackPanel Unavailable(string title, string? line, string? button = null, Action? act = null, bool error = false)
    {
        var panel = new StackPanel { Spacing = 8, HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center, MaxWidth = 420, Margin = new Thickness(0, 48, 0, 48) };
        Words(panel, title, line, button, act, error);
        return panel;
    }

    static void Words(StackPanel panel, string title, string? line, string? button, Action? act, bool error)
    {
        var heading = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, HorizontalAlignment = HorizontalAlignment.Center };
        if (error)
            heading.Children.Add(ErrorGlyph());
        var text = new TextBlock { Text = title, Style = Ui.Res<Style>("SubtitleTextBlockStyle"), TextWrapping = TextWrapping.WrapWholeWords, TextAlignment = TextAlignment.Center };
        AutomationProperties.SetHeadingLevel(text, AutomationHeadingLevel.Level2);
        heading.Children.Add(text);
        panel.Children.Add(heading);
        if (line is { Length: > 0 })
        {
            var words = Ui.Haze(line);
            words.TextAlignment = TextAlignment.Center;
            words.HorizontalAlignment = HorizontalAlignment.Center;
            panel.Children.Add(words);
        }
        if (button is not null && act is not null)
        {
            var b = new Button { Content = button, HorizontalAlignment = HorizontalAlignment.Center, Margin = new Thickness(0, 8, 0, 0), MinHeight = 32 };
            b.Click += (_, _) => act();
            panel.Children.Add(b);
        }
    }

    /// <summary>The error glyph, the one vermilion thing on an error.</summary>
    public static FontIcon ErrorGlyph(double size = 16) => new()
    {
        Glyph = "",
        FontSize = size,
        Foreground = new SolidColorBrush(Ui.Vermilion),
        VerticalAlignment = VerticalAlignment.Center,
    };

    /// <summary>Raised plates in the cover grid's layout: covers 150 by 225 and two caption bars.</summary>
    /// <summary>A cover cell's title is kept two lines high, so every cell is one height: this, in px.</summary>
    public const double CoverTitleHeight = 40;

    public static UIElement Covers(int count = 12, bool rail = false)
    {
        var grid = new ItemsRepeater
        {
            // A rail's plates are one row, as the rail is: a grid of them would collapse to a row when it lands.
            Layout = rail ? new StackLayout { Orientation = Orientation.Horizontal, Spacing = 16 }
                : new UniformGridLayout { MinItemWidth = 150, MinColumnSpacing = 16, MinRowSpacing = 20, ItemsJustification = UniformGridLayoutItemsJustification.Start },
            ItemsSource = Enumerable.Range(0, count).ToList(),
            ItemTemplate = new CoverPlaceholderFactory(),
        };
        AutomationProperties.SetAccessibilityView(grid, AccessibilityView.Raw);
        return grid;
    }

    sealed partial class CoverPlaceholderFactory : IElementFactory
    {
        public UIElement GetElement(ElementFactoryGetArgs args)
        {
            // The cover grid's cell, line for line: the cover, the title's two lines, the caption.
            var panel = new StackPanel { Width = 150, Spacing = 2, Padding = new Thickness(0, 0, 0, 6) };
            var cover = Plate(150, 225, Art.Radius(150));
            cover.Margin = new Thickness(0, 0, 0, 6);
            panel.Children.Add(cover);
            var title = Plate(110, 12, 3);
            title.VerticalAlignment = VerticalAlignment.Top;
            title.Margin = new Thickness(0, 4, 0, 0);
            panel.Children.Add(new Border { Height = CoverTitleHeight, Child = title });
            var caption = Plate(70, 10, 3);
            caption.VerticalAlignment = VerticalAlignment.Center;
            panel.Children.Add(new Border { Height = 16, Child = caption });
            return panel;
        }

        public void RecycleElement(ElementFactoryRecycleArgs args) { }
    }

    /// <summary>
    /// Placeholder rows: a thumbnail plate and two bars, in a list's layout. A list whose rows
    /// have a set height passes it, and the space between them, so the rows that replace the
    /// plates take the room they kept.
    /// </summary>
    public static UIElement Rows(int count, double thumbWidth, double thumbHeight, double rowHeight = 0, double spacing = 12)
    {
        var panel = new StackPanel { Spacing = spacing };
        for (var i = 0; i < count; i++)
        {
            var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 14 };
            if (rowHeight > 0)
                row.Height = rowHeight;
            if (thumbWidth > 0)
                row.Children.Add(Plate(thumbWidth, thumbHeight, 6));
            var bars = new StackPanel { Spacing = 8, VerticalAlignment = VerticalAlignment.Center };
            bars.Children.Add(Plate(220, 14, 3));
            bars.Children.Add(Plate(140, 10, 3));
            row.Children.Add(bars);
            panel.Children.Add(row);
        }
        AutomationProperties.SetAccessibilityView(panel, AccessibilityView.Raw);
        return panel;
    }

    /// <summary>A section heading's plate, where <see cref="Ui.Heading"/> would sit.</summary>
    public static Border Heading()
    {
        var plate = Plate(140, 14, 3);
        plate.Margin = new Thickness(0, 19, 0, 7);
        return plate;
    }

    /// <summary>
    /// A page heading's plates, where a page title and the line under it would sit, for a page
    /// whose title comes with what it loads.
    /// </summary>
    public static StackPanel Title(double width = 260)
    {
        var panel = new StackPanel { Spacing = 12 };
        panel.Children.Add(Plate(width, 28, 4));
        panel.Children.Add(Plate(width * 1.4, 14, 3));
        return panel;
    }

    public static Border Plate(double width, double height, double radius) => new()
    {
        Width = width,
        Height = height,
        CornerRadius = new CornerRadius(radius),
        HorizontalAlignment = HorizontalAlignment.Left,
        Background = Ui.Res<Brush>("RaisedBrush"),
    };
}

/// <summary>
/// "Offline. Downloaded episodes still play." under a page's title in haze beside the glyph,
/// while the engine says there is no connection. Not a coloured banner.
/// </summary>
public sealed partial class OfflineLine : StackPanel
{
    public OfflineLine() : this(false) { }

    /// <param name="reading">On a reading page: downloaded chapters, not episodes.</param>
    public OfflineLine(bool reading)
    {
        Orientation = Orientation.Horizontal;
        Spacing = 8;
        Margin = new Thickness(0, 4, 0, 0);
        Visibility = Visibility.Collapsed;
        Children.Add(new FontIcon { Glyph = "", FontSize = 14, Foreground = Ui.Res<Brush>("HazeBrush"), VerticalAlignment = VerticalAlignment.Center });
        Children.Add(Ui.Text(reading ? T.ReadStateOfflineLine : T.StateOfflineLine, "HazeBodyStyle", wrap: false));
        AutomationProperties.SetLiveSetting(this, AutomationLiveSetting.Polite);
        Loaded += (_, _) =>
        {
            Visibility = Ctx.Current.Known.Offline ? Visibility.Visible : Visibility.Collapsed;
            Ctx.Current.Batch += Heard;
        };
        Unloaded += (_, _) => Ctx.Current.Batch -= Heard;
    }

    void Heard(IReadOnlyList<Event> batch)
    {
        if (!batch.Any(e => e is Event.Connectivity))
            return;
        if (Ctx.Current.Known.Offline)
            Motion.Show(this);
        else
            Motion.Hide(this);
    }
}
