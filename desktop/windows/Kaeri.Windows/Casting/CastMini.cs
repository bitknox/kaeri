using Kaeri.App;
using Kaeri.Windows.Pages;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;

namespace Kaeri.Windows.Casting;

/// <summary>
/// The mini across the foot of the page pane while something is on the TV, on every page: the
/// show's art and title with the TV's line (or a ring and the starting or joining line), the
/// transport in the middle over the seek line with its times, and the chevron up. A click
/// anywhere but a control opens the controller. Widgets change in place, for what the view
/// changed only.
/// </summary>
sealed class CastMini : Grid
{
    readonly Ctx ctx;
    readonly TvRemote tv;
    readonly Action expand;
    public readonly CastParts.Art Art = new(72, 40, 4);
    readonly TextBlock title = new() { Style = Ui.Res<Style>("ShowTitleStyle"), FontSize = 14, LineHeight = 20 };
    readonly CastGlyph lead = new(13, Ui.Res<Brush>("TextFillColorSecondaryBrush"));
    readonly ProgressRing ring = new() { Width = 11, Height = 11, MinWidth = 11, MinHeight = 11, IsActive = false, Visibility = Visibility.Collapsed };
    readonly TextBlock line = new() { FontSize = 12, LineHeight = 16, Foreground = Ui.Res<Brush>("TextFillColorSecondaryBrush"), TextTrimming = TextTrimming.CharacterEllipsis };
    readonly StackPanel transport = new() { Orientation = Orientation.Horizontal, Spacing = 6, HorizontalAlignment = HorizontalAlignment.Center };
    readonly FontIcon playGlyph = CastParts.Glyph("", 16);
    readonly Button play;
    readonly SeekLine seek = new(compact: true);
    public readonly Button Chevron;
    /// <summary>The art and the lines: a button of their own, so the keyboard can open the controller too.</summary>
    public readonly Button Opener;

    public CastMini(Ctx ctx, TvRemote tv, Action expand)
    {
        this.ctx = ctx;
        this.tv = tv;
        this.expand = expand;
        Height = 76;
        Padding = new Thickness(16, 0, 16, 0);
        ColumnSpacing = 14;
        Background = Ui.Res<Brush>("SidebarBrush");
        BorderBrush = Ui.Res<Brush>("DividerStrokeColorDefaultBrush");
        BorderThickness = new Thickness(0, 1, 0, 0);
        AutomationProperties.SetLandmarkType(this, AutomationLandmarkType.Custom);
        AutomationProperties.SetLocalizedLandmarkType(this, T.TvPlayOn);
        ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });

        // The art and the two lines, 260 wide for the words.
        var words = new StackPanel { Width = 260, Spacing = 0, VerticalAlignment = VerticalAlignment.Center };
        words.Children.Add(title);
        var second = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
        lead.VerticalAlignment = VerticalAlignment.Center;
        ring.VerticalAlignment = VerticalAlignment.Center;
        second.Children.Add(lead);
        second.Children.Add(ring);
        second.Children.Add(line);
        words.Children.Add(second);
        var opener = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 14 };
        opener.Children.Add(Art.Root);
        opener.Children.Add(words);
        Opener = new Button
        {
            Content = opener,
            Padding = new Thickness(0),
            Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
            BorderThickness = new Thickness(0),
            VerticalAlignment = VerticalAlignment.Center,
        };
        Opener.Click += (_, _) => expand();
        Children.Add(Opener);

        // Back 10, play or pause in its round, forward 10, over the seek line.
        var back = CastParts.Icon(CastParts.Ten(back: true, 18), T.PlayerBack10);
        back.Click += (_, _) => tv.SessionSeekBy(-10);
        play = CastParts.Round(playGlyph, 36);
        CastParts.Name(play, T.ActionPause);
        play.Click += (_, _) => tv.SessionTogglePause();
        var forward = CastParts.Icon(CastParts.Ten(back: false, 18), T.PlayerForward10);
        forward.Click += (_, _) => tv.SessionSeekBy(10);
        transport.Children.Add(back);
        transport.Children.Add(play);
        transport.Children.Add(forward);
        Motion.QuickOpacity(transport);
        var middle = new StackPanel { Spacing = 0, VerticalAlignment = VerticalAlignment.Center, HorizontalAlignment = HorizontalAlignment.Stretch };
        middle.Children.Add(transport);
        seek.Root.MaxWidth = 440;
        seek.Root.HorizontalAlignment = HorizontalAlignment.Stretch;
        seek.Seek = tv.SessionSeekTo;
        middle.Children.Add(seek.Root);
        SetColumn(middle, 1);
        Children.Add(middle);

        Chevron = CastParts.Icon(CastParts.Glyph("", 14), T.TvShowRemote(""));
        Chevron.Click += (_, _) => expand();
        SetColumn(Chevron, 2);
        Children.Add(Chevron);

        // Anywhere but a control opens the controller.
        Tapped += (_, e) =>
        {
            if (Within<ButtonBase>(e.OriginalSource as DependencyObject) || Within<Slider>(e.OriginalSource as DependencyObject))
                return;
            e.Handled = true;
            expand();
        };
    }

    bool Within<TElement>(DependencyObject? at) where TElement : DependencyObject
    {
        for (; at is not null && at != this; at = VisualTreeHelper.GetParent(at))
            if (at is TElement)
                return true;
        return false;
    }

    /// <summary>Draws the frame; what did not change is not touched.</summary>
    public void Show(CastDrawn drawn)
    {
        Art.Show(ctx, drawn.Id, drawn.Image);
        var name = tv.SessionName;
        if (title.Text != drawn.Title)
            title.Text = drawn.Title;
        var busy = drawn.Line is CastLine.Starting or CastLine.Rejoining;
        if (ring.IsActive != busy)
        {
            ring.IsActive = busy;
            ring.Visibility = busy ? Visibility.Visible : Visibility.Collapsed;
            lead.Visibility = busy ? Visibility.Collapsed : Visibility.Visible;
        }
        lead.Show(CastButtonState.Connected);
        var words = busy || drawn.Line == CastLine.Failed ? drawn.Status : name;
        if (line.Text != words)
            line.Text = words;
        CastParts.PlayGlyph(play, playGlyph, drawn.Paused);
        var opacity = drawn.Dim ? 0.38 : 1;
        if (transport.Opacity != opacity)
            transport.Opacity = opacity;
        foreach (var child in transport.Children.OfType<Control>())
            child.IsEnabled = !drawn.Dim && drawn.Line != CastLine.Failed;
        seek.Show(drawn);
        var tip = T.TvShowRemote(name);
        if (ToolTipService.GetToolTip(Chevron) as string != tip)
            CastParts.Name(Chevron, tip);
        AutomationProperties.SetName(Opener, string.IsNullOrEmpty(drawn.EpisodeLine) ? drawn.Title : $"{drawn.Title}, {drawn.EpisodeLine}");
    }

    /// <summary>The playhead moved on, on this PC's clock.</summary>
    public void Tick(CastDrawn drawn) => seek.Show(drawn);
}

/// <summary>
/// The seek line with its times: elapsed and total in the mini, elapsed and minus remaining in
/// the controller, the waiting line between them in the controller. While the TV starts the line
/// has no thumb and no times. A drag holds its value until the drag rests and the TV answers.
/// </summary>
sealed class SeekLine
{
    readonly bool compact;
    readonly Slider slider = new() { Minimum = 0, Maximum = 1, StepFrequency = 1, SmallChange = 10, LargeChange = 60, IsThumbToolTipEnabled = false, VerticalAlignment = VerticalAlignment.Center };
    readonly Border track = new() { Height = 4, CornerRadius = new CornerRadius(2), VerticalAlignment = VerticalAlignment.Center, Opacity = 0.38, Visibility = Visibility.Collapsed };
    readonly TextBlock elapsed = new() { VerticalAlignment = VerticalAlignment.Center };
    readonly TextBlock end = new() { VerticalAlignment = VerticalAlignment.Center };
    readonly StackPanel waiting = new() { Orientation = Orientation.Horizontal, Spacing = 6, HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center, Visibility = Visibility.Collapsed };
    readonly TextBlock waitingText = new();
    readonly Microsoft.UI.Dispatching.DispatcherQueueTimer rest;
    bool quiet, dragging;

    /// <summary>A seek the drag settled on.</summary>
    public Action<double>? Seek { get; set; }

    public SeekLine(bool compact)
    {
        this.compact = compact;
        var size = compact ? 12 : 10;
        foreach (var text in new[] { elapsed, end, waitingText })
        {
            text.FontSize = size;
            text.Foreground = Ui.Res<Brush>("TextFillColorSecondaryBrush");
            Microsoft.UI.Xaml.Documents.Typography.SetNumeralAlignment(text, Microsoft.UI.Xaml.FontNumeralAlignment.Tabular);
        }
        track.Background = Ui.Res<Brush>("TextFillColorSecondaryBrush");
        AutomationProperties.SetName(slider, T.PlayerPosition);
        rest = slider.DispatcherQueue.CreateTimer();
        rest.Interval = TimeSpan.FromMilliseconds(350);
        rest.IsRepeating = false;
        rest.Tick += (_, _) =>
        {
            dragging = false;
            Seek?.Invoke(slider.Value);
        };
        slider.ValueChanged += (_, e) =>
        {
            if (quiet)
                return;
            dragging = true;
            Times(e.NewValue, slider.Maximum > 1 ? slider.Maximum : null);
            rest.Stop();
            rest.Start();
        };
        if (compact)
        {
            // Elapsed, the line, the total, in one row.
            Root.ColumnSpacing = 10;
            Root.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            Root.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            Root.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            Root.Children.Add(elapsed);
            Grid.SetColumn(slider, 1);
            Grid.SetColumn(track, 1);
            Root.Children.Add(slider);
            Root.Children.Add(track);
            Grid.SetColumn(end, 2);
            Root.Children.Add(end);
            Root.Margin = new Thickness(0, -6, 0, -6);
        }
        else
        {
            // The line, then elapsed and minus remaining under it, the waiting line between them.
            Root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
            Root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
            Root.Children.Add(slider);
            Root.Children.Add(track);
            var times = new Grid { Margin = new Thickness(0, -4, 0, 0), MinHeight = 14 };
            end.HorizontalAlignment = HorizontalAlignment.Right;
            times.Children.Add(elapsed);
            times.Children.Add(waiting);
            times.Children.Add(end);
            waiting.Children.Add(new ProgressRing { Width = 10, Height = 10, MinWidth = 10, MinHeight = 10, IsActive = true });
            waiting.Children.Add(waitingText);
            AutomationProperties.SetLiveSetting(waitingText, AutomationLiveSetting.Polite);
            Grid.SetRow(times, 1);
            Root.Children.Add(times);
        }
    }

    public Grid Root { get; } = new();

    public Slider Slider => slider;

    public void Show(CastDrawn drawn)
    {
        var starting = drawn.Dim;
        slider.Visibility = starting ? Visibility.Collapsed : Visibility.Visible;
        track.Visibility = starting ? Visibility.Visible : Visibility.Collapsed;
        slider.IsEnabled = drawn.Duration is not null && drawn.Line != CastLine.Failed;
        if (!dragging)
        {
            quiet = true;
            var length = Math.Max(drawn.Duration ?? 0, 1);
            if (slider.Maximum != length)
                slider.Maximum = length;
            var at = Math.Min(drawn.Position, length);
            if (Math.Abs(slider.Value - at) >= 0.5)
                slider.Value = at;
            quiet = false;
            Times(drawn.Position, drawn.Duration, drawn);
        }
        var wait = !compact && drawn.Waiting is not null;
        if (wait)
            waitingText.Text = drawn.Waiting!;
        if ((waiting.Visibility == Visibility.Visible) != wait)
            waiting.Visibility = wait ? Visibility.Visible : Visibility.Collapsed;
        elapsed.Opacity = end.Opacity = starting ? 0 : 1;
    }

    void Times(double position, double? length, CastDrawn? drawn = null)
    {
        var first = drawn?.Elapsed ?? Kaeri.Engine.Ffi.KaeriFfiMethods.PlayClock(position);
        var last = drawn is not null ? compact ? drawn.Total : drawn.Remaining
            : length is { } d ? compact ? Kaeri.Engine.Ffi.KaeriFfiMethods.PlayClock(d) : "-" + Kaeri.Engine.Ffi.KaeriFfiMethods.PlayClock(Math.Max(0, d - position)) : "";
        if (elapsed.Text != first)
            elapsed.Text = first;
        if (end.Text != last)
            end.Text = last;
        AutomationProperties.SetItemStatus(slider, first);
    }
}
