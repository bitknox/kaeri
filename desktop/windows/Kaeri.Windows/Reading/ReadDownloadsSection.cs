using Medium = Kaeri.Engine.Core.Medium;
using Microsoft.UI.Xaml.Automation.Peers;
using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Navigation;

namespace Kaeri.Windows.Pages;

/// <summary>
/// Downloads' Reading section: one row per series with what its chapters are doing, a failure's
/// cause with Retry, and Delete read for chapters already read (the engine spares bookmarked
/// ones). A series' menu offers Send to e-reader with its downloaded chapters, and every send
/// under way, ready or failed keeps a status row above the series. Headed Reading with its
/// chapters and size, first on Downloads with both media on; alone, under the page's own title,
/// with reading alone.
/// </summary>
public sealed partial class ReadDownloadsSection : UserControl
{
    readonly Ctx ctx = Ctx.Current;
    readonly TextBlock heading = new() { Text = T.SettingsGroupReading, Style = Ui.Res<Style>("SectionTitleStyle") };
    readonly TextBlock total = new() { Style = Ui.Res<Style>("HazeBodyStyle"), VerticalAlignment = VerticalAlignment.Bottom };
    readonly StackPanel head = new() { Orientation = Orientation.Horizontal, Spacing = 12, Margin = new Thickness(0, 0, 0, 8) };
    readonly StackPanel rows = new() { TabFocusNavigation = KeyboardNavigationMode.Local, XYFocusKeyboardNavigation = XYFocusKeyboardNavigationMode.Enabled };
    readonly SendStatusRows sends;
    BookDownloadsView? shown;
    bool hearing, headed = true;

    /// <summary>Headed Reading, as with both media on. Without, only the total shows above the rows.</summary>
    public bool Headed
    {
        get => headed;
        set
        {
            headed = value;
            heading.Visibility = value ? Visibility.Visible : Visibility.Collapsed;
        }
    }

    /// <summary>Whether the engine has said what is downloaded, and whether any series is.</summary>
    public bool HasAnswer => ctx.Known.Reading.Downloads is not null;
    public bool Any => ctx.Known.Reading.Downloads is { Series.Length: > 0 } || ctx.Known.Reading.Sending.Sends.Count > 0;

    /// <summary>The section's content changed: the page may show or hide it, or its empty state.</summary>
    public event Action? Changed;

    public ReadDownloadsSection()
    {
        AutomationProperties.SetHeadingLevel(heading, AutomationHeadingLevel.Level2);
        head.Children.Add(heading);
        head.Children.Add(total);
        sends = new SendStatusRows(ctx) { Margin = new Thickness(0, 0, 0, 8) };
        var stack = new StackPanel();
        stack.Children.Add(head);
        stack.Children.Add(sends);
        stack.Children.Add(rows);
        Content = stack;
        Loaded += (_, _) =>
        {
            if (!hearing)
                ctx.Known.Reading.Changed += Heard;
            hearing = true;
            ctx.Known.Reading.LoadDownloads();
            // The status rows name each send's e-reader.
            if (ctx.Known.Reading.Sending.Targets.Length == 0)
                ctx.Known.Reading.Sending.LoadTargets();
            sends.Draw();
            Show();
        };
        Unloaded += (_, _) =>
        {
            ctx.Known.Reading.Changed -= Heard;
            hearing = false;
        };
    }

    void Heard(ReadingPart part)
    {
        if (part is ReadingPart.Send or ReadingPart.Targets or ReadingPart.Series or ReadingPart.Downloads)
            sends.Draw();
        // A send starting or ending can show or hide the section.
        if (part is ReadingPart.Send)
            Changed?.Invoke();
        // A cover can come with a series' details later than its downloads.
        if (part is ReadingPart.Downloads or ReadingPart.Series)
            Show(part == ReadingPart.Series);
    }

    /// <summary>
    /// Send to e-reader with the series' downloaded chapters, or the send it has under way.
    /// </summary>
    async void Send(BookDownloadSeries series, string[] kept)
    {
        var running = ctx.Known.Reading.Sending.Running(series.Work);
        await SendDialog.Show(ctx, XamlRoot, series.Work, series.Title, running?.Chapters ?? kept, running?.Id);
        sends.Draw();
    }

    /// <summary>"38 chapters · 1.1 GB", as the engine words the Reading total; empty with nothing kept.</summary>
    static string Total(BookDownloadsView view)
    {
        if (view.Total.Length > 0)
            return view.Total.Joined();
        return view.Meter.Chapters > 0 ? T.ReadDownloadsTotal(view.Meter.Chapters, Ui.Bytes(view.Meter.Used)) : "";
    }

    void Show(bool force = false)
    {
        if (ctx.Known.Reading.Downloads is not { } view)
            return;
        if (view == shown && !force)
            return;
        shown = view;
        total.Text = Total(view);
        head.Visibility = Headed || total.Text.Length > 0 ? Visibility.Visible : Visibility.Collapsed;
        var mark = Ui.MarkFocus(rows);
        rows.Children.Clear();
        for (var i = 0; i < view.Series.Length; i++)
        {
            if (i > 0)
                rows.Children.Add(new Border { Height = 1, Margin = new Thickness(50, 0, 0, 0), Background = Ui.Res<Brush>("FaintRuleBrush") });
            rows.Children.Add(Row(view, view.Series[i]));
        }
        Ui.RestoreFocus(rows, mark);
        Changed?.Invoke();
    }

    /// <summary>The cover from the row, else the series' details, else a list holding it; asked for when none has it.</summary>
    string? Cover(BookDownloadSeries series)
    {
        if (series.Cover is { } cover)
            return cover;
        var reading = ctx.Known.Reading;
        if (reading.Series.TryGetValue(series.Work, out var details) && details.Cover is { } known)
            return known;
        foreach (var tiles in reading.Lists.Values)
            if (tiles.FirstOrDefault(t => t.Work == series.Work) is { Cover: { } listed })
                return listed;
        reading.LoadSeriesInfo(series.Work);
        return null;
    }

    /// <summary>One series: cover, title, its line, the ring that stops it while bytes move, and Retry or Delete read.</summary>
    UIElement Row(BookDownloadsView view, BookDownloadSeries series)
    {
        var failed = series.Failed.Length > 0;
        // The chapters kept here: what Send to e-reader works from.
        var kept = view.Rows
            .Where(r => r.Work == series.Work && r.State is ChapterDownloadState.Done)
            .Select(r => r.Chapter.Id)
            .ToArray();
        var grid = new Grid { ColumnSpacing = 14, Padding = new Thickness(0, 10, 0, 10) };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.Children.Add(ReadingParts.Cover(ctx, Cover(series), 36));
        var words = new StackPanel { Spacing = 2, VerticalAlignment = VerticalAlignment.Center };
        words.Children.Add(new TextBlock { Text = series.Title, Style = Ui.Res<Style>("ShowTitleStyle"), FontSize = 14, TextTrimming = TextTrimming.CharacterEllipsis, TextWrapping = TextWrapping.NoWrap });
        var line = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
        if (failed)
            line.Children.Add(States.ErrorGlyph(12));
        var lineWords = series.Line.Joined();
        line.Children.Add(new TextBlock { Text = lineWords, Style = Ui.Res<Style>("HazeCaptionStyle"), TextWrapping = TextWrapping.Wrap });
        words.Children.Add(line);
        var open = new Button
        {
            Style = Ui.Res<Style>("HomeCardButtonStyle"),
            Content = words,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Stretch,
        };
        AutomationProperties.SetName(open, ReadingParts.Joined(series.Title, lineWords));
        open.Click += (_, _) => ctx.Window.OpenSeries(series.Work);
        Grid.SetColumn(open, 1);
        grid.Children.Add(open);

        var tools = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, VerticalAlignment = VerticalAlignment.Center };
        if (series.Progress is { } progress)
        {
            var moving = view.Rows
                .Where(r => r.Work == series.Work && r.State is ChapterDownloadState.Downloading or ChapterDownloadState.Queued or ChapterDownloadState.WaitsForWifi)
                .Select(r => r.Chapter.Id)
                .ToArray();
            var ring = new Button { Content = Ring(progress), Padding = new Thickness(6), MinWidth = 32, MinHeight = 32, Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent), BorderThickness = new Thickness(0) };
            AutomationProperties.SetName(ring, ReadingParts.Joined(T.ChapterDownloadStop, series.Title));
            ToolTipService.SetToolTip(ring, T.ChapterDownloadStop);
            ring.Click += (_, _) => ctx.Known.Reading.CancelDownloads(series.Work, moving);
            tools.Children.Add(ring);
        }
        if (failed)
        {
            var retry = Ui.Label(T.ChapterRetry, (_, _) => ctx.Known.Reading.Download(series.Work, new DownloadPick.Chapters(series.Failed)));
            AutomationProperties.SetName(retry, ReadingParts.Joined(T.ChapterRetry, series.Title));
            tools.Children.Add(retry);
        }
        else if (series.Read > 0)
        {
            var delete = Ui.Label(T.ReadDownloadsDeleteRead, (_, _) => ctx.Known.Reading.DeleteRead(series.Work));
            AutomationProperties.SetName(delete, ReadingParts.Joined(T.ReadDownloadsDeleteRead, series.Title));
            tools.Children.Add(delete);
        }
        Grid.SetColumn(tools, 2);
        grid.Children.Add(tools);

        // The row's menu: what the tools do, by right-click, Shift+F10 or the menu key.
        var menu = new MenuFlyout();
        menu.Items.Add(ReadingParts.Item(T.HomeDetails, () => ctx.Window.OpenSeries(series.Work)));
        if (kept.Length > 0)
            menu.Items.Add(ReadingParts.Item(T.SendTitle, () => Send(series, kept)));
        if (failed)
            menu.Items.Add(ReadingParts.Item(T.ChapterRetry, () => ctx.Known.Reading.Download(series.Work, new DownloadPick.Chapters(series.Failed))));
        if (series.Read > 0)
            menu.Items.Add(ReadingParts.Item(T.ReadDownloadsDeleteRead, () => ctx.Known.Reading.DeleteRead(series.Work)));
        open.ContextFlyout = menu;
        return grid;
    }

    /// <summary>The ring while bytes move: a silk arc on the rule with a stop square inside.</summary>
    static UIElement Ring(float fraction)
    {
        const double size = 16;
        var ring = new ProgressRing
        {
            IsIndeterminate = false,
            Minimum = 0,
            Maximum = 1,
            Value = Math.Max(0.02, Math.Min(fraction, 1)),
            Width = size,
            Height = size,
            Foreground = Ui.Res<Brush>("SilkBrush"),
            Background = Ui.Res<Brush>("RuleBrush"),
        };
        AutomationProperties.SetAccessibilityView(ring, AccessibilityView.Raw);
        var grid = new Grid { Width = size, Height = size };
        grid.Children.Add(ring);
        grid.Children.Add(new Border { Width = 6, Height = 6, CornerRadius = new CornerRadius(1), Background = Ui.Res<Brush>("SilkBrush"), HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center });
        return grid;
    }
}

/// <summary>Downloads with reading alone: the Reading section under the page's title, or the empty state.</summary>
public sealed partial class ReadDownloadsPage : Page
{
    readonly Ctx ctx = Ctx.Current;
    readonly ReadDownloadsSection section = new() { Headed = false };
    readonly StateHost state = new() { Padding = new Thickness(0, 4, 36, 48), MaxWidth = 900, HorizontalAlignment = HorizontalAlignment.Left, NeedsConnection = false };

    public ReadDownloadsPage()
    {
        Background = Ui.Res<Brush>("GroundBrush");
        var root = new Grid { Padding = new Thickness(36, 20, 0, 0), RowSpacing = 12 };
        root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        root.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });
        var title = new TextBlock { Text = T.DownloadsTitle, Style = Ui.Res<Style>("PageTitleStyle"), Margin = new Thickness(0, 0, 36, 0) };
        AutomationProperties.SetHeadingLevel(title, AutomationHeadingLevel.Level1);
        root.Children.Add(title);
        state.Children.Add(section);
        state.Placeholders = () => States.Rows(3, 36, 54, rowHeight: 74, spacing: 0);
        var scroll = new ScrollViewer { VerticalScrollBarVisibility = ScrollBarVisibility.Auto, IsTabStop = false, Content = state };
        Grid.SetRow(scroll, 1);
        root.Children.Add(scroll);
        Content = root;
        section.Changed += Settle;
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        if (ctx.Known.Reading.Downloads is null)
            state.Wait();
        else
            Settle();
    }

    void Settle()
    {
        if (!section.HasAnswer)
            return;
        if (section.Any)
            state.Done();
        else
            state.Empty(States.Empty(T.DownloadsEmptyTitle, T.WindowsReadDownloadsEmptyDetail));
    }
}
