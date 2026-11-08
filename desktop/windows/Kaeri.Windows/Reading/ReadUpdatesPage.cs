using Medium = Kaeri.Engine.Core.Medium;
using Microsoft.UI.Xaml.Automation.Peers;
using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Navigation;
using Windows.System;

namespace Kaeri.Windows.Pages;

/// <summary>
/// Updates: new chapters by day, newest first, from the series you follow. Unread rows are silk
/// and bold with the water dot, read rows haze. A row opens the reader at its chapter; its cover
/// opens the series. Download new is the one tool, secondary: nothing here is amber.
/// </summary>
public sealed partial class ReadUpdatesPage : Page
{
    const double ListWidth = 772, CoverWidth = 32;

    readonly Ctx ctx = Ctx.Current;
    readonly TextBlock checkedLine = new() { Style = Ui.Res<Style>("HazeBodyStyle") };
    readonly Button downloadNew = new() { MinHeight = 32 };
    readonly StateHost state = new();
    readonly StackPanel list = new() { MaxWidth = ListWidth, HorizontalAlignment = HorizontalAlignment.Left, Padding = new Thickness(0, 4, 36, 48) };
    UpdatesView? shown;
    bool hearing;

    public ReadUpdatesPage()
    {
        Background = Ui.Res<Brush>("GroundBrush");
        NavigationCacheMode = NavigationCacheMode.Enabled;
        var root = new Grid { Padding = new Thickness(36, 20, 0, 0), RowSpacing = 12 };
        root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        root.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });

        var head = new Grid { ColumnSpacing = 12, Margin = new Thickness(0, 0, 36, 0) };
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var words = new StackPanel { Spacing = 2 };
        var title = new TextBlock { Text = T.NavUpdates, Style = Ui.Res<Style>("PageTitleStyle") };
        AutomationProperties.SetHeadingLevel(title, AutomationHeadingLevel.Level1);
        words.Children.Add(title);
        words.Children.Add(new OfflineLine());
        AutomationProperties.SetLiveSetting(checkedLine, Microsoft.UI.Xaml.Automation.Peers.AutomationLiveSetting.Polite);
        words.Children.Add(checkedLine);
        head.Children.Add(words);
        downloadNew.Content = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 8,
            Children = { new FontIcon { Glyph = "", FontSize = 14 }, new TextBlock { Text = T.UpdatesDownloadNew } },
        };
        AutomationProperties.SetName(downloadNew, T.UpdatesDownloadNew);
        downloadNew.VerticalAlignment = VerticalAlignment.Center;
        downloadNew.Click += (_, _) => ctx.Known.Reading.DownloadNew();
        Grid.SetColumn(downloadNew, 1);
        head.Children.Add(downloadNew);
        root.Children.Add(head);

        var scroll = new ScrollViewer { VerticalScrollBarVisibility = ScrollBarVisibility.Auto, IsTabStop = false, Content = list };
        state.Children.Add(scroll);
        state.Placeholders = () => new StackPanel
        {
            MaxWidth = ListWidth,
            HorizontalAlignment = HorizontalAlignment.Left,
            Spacing = 12,
            Children = { States.Heading(), States.Rows(6, CoverWidth, CoverWidth * 1.5, rowHeight: 64, spacing: 0) },
        };
        Grid.SetRow(state, 1);
        root.Children.Add(state);
        Content = root;

        // F5 and Ctrl+R ask every followed series now.
        foreach (var (key, modifiers) in new[] { (VirtualKey.F5, VirtualKeyModifiers.None), (VirtualKey.R, VirtualKeyModifiers.Control) })
        {
            var accelerator = new KeyboardAccelerator { Key = key, Modifiers = modifiers };
            accelerator.Invoked += (_, e) =>
            {
                e.Handled = true;
                ctx.Known.Reading.CheckUpdates();
            };
            KeyboardAccelerators.Add(accelerator);
        }
        KeyboardAcceleratorPlacementMode = KeyboardAcceleratorPlacementMode.Hidden;

        Loaded += (_, _) =>
        {
            if (!hearing)
                ctx.Known.Reading.Changed += Heard;
            hearing = true;
            // What came while the page was being built, before it heard.
            Show();
        };
        Unloaded += (_, _) =>
        {
            ctx.Known.Reading.Changed -= Heard;
            hearing = false;
        };
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        if (ctx.Known.Reading.Updates is null)
            state.Wait();
        else
            Show();
        ctx.Known.Reading.LoadUpdates();
    }

    void Heard(ReadingPart part)
    {
        if (part == ReadingPart.Updates)
            Show();
        else if (part == ReadingPart.Failure && ctx.Known.Reading.Updates is null
            && ctx.Known.Reading.Failures.TryGetValue(ReadingAbout.Updates, out var failure))
            state.Failed(failure.Words(), () => ctx.Known.Reading.LoadUpdates());
    }

    void Show()
    {
        if (ctx.Known.Reading.Updates is not { } updates)
            return;
        downloadNew.IsEnabled = updates.Unread > 0;
        checkedLine.Text = updates.Checking ? T.ChapterUpdatesChecking
            : updates.CheckedAt is { } at ? T.ChapterUpdatesCheckedCaughtUp(ReadingParts.Ago(at))
            : "";
        checkedLine.Visibility = checkedLine.Text.Length > 0 ? Visibility.Visible : Visibility.Collapsed;
        if (updates == shown)
        {
            state.Done();
            return;
        }
        shown = updates;
        var mark = Ui.MarkFocus(list);
        list.Children.Clear();
        foreach (var failure in updates.Failures)
            list.Children.Add(FailureRow(failure));
        if (updates.Days.Length == 0)
            list.Children.Add(ReadingParts.PlainEmpty(T.UpdatesEmptyTitle, T.ChapterUpdatesEmptyLine));
        foreach (var day in updates.Days)
        {
            var heading = new TextBlock
            {
                Text = ReadingParts.Day(day.Day),
                FontSize = 11,
                FontWeight = FontWeights.Bold,
                Foreground = Ui.Res<Brush>("SilkBrush"),
                Margin = new Thickness(0, 20, 0, 8),
            };
            AutomationProperties.SetHeadingLevel(heading, AutomationHeadingLevel.Level2);
            list.Children.Add(heading);
            var rows = new StackPanel
            {
                TabFocusNavigation = KeyboardNavigationMode.Local,
                XYFocusKeyboardNavigation = XYFocusKeyboardNavigationMode.Enabled,
            };
            foreach (var row in day.Rows)
            {
                rows.Children.Add(new Border { Height = 1, Background = Ui.Res<Brush>("FaintRuleBrush") });
                rows.Children.Add(Row(row));
            }
            list.Children.Add(rows);
        }
        state.Done();
        Ui.RestoreFocus(list, mark);
    }

    /// <summary>"Chapter 88 · Group name", "Chapter 43 · 2 versions": the label, then the versions.</summary>
    static string Line(ChapterRow row)
    {
        if (row.Versions > 1)
            return ReadingParts.Joined(row.Label, T.ChapterUpdatesVersions(row.Versions));
        return ReadingParts.Joined(row.Label, row.VersionLabel);
    }

    /// <summary>A new chapter: the cover (which opens the series), the title, the chapter and the time (which open the reader).</summary>
    UIElement Row(UpdateRow row)
    {
        var unread = row.Chapter.State is not ChapterReadState.Read;
        var work = row.Tile.Work;
        var grid = new Grid { ColumnSpacing = 12, Padding = new Thickness(0, 8, 0, 8) };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });

        var cover = new Button { Style = Ui.Res<Style>("HomeCardButtonStyle"), Content = ReadingParts.Cover(ctx, row.Tile.Cover, CoverWidth), VerticalAlignment = VerticalAlignment.Center };
        AutomationProperties.SetName(cover, T.ChapterUpdatesOpenSeries(row.Tile.Title));
        ToolTipService.SetToolTip(cover, T.ChapterUpdatesOpenSeries(row.Tile.Title));
        cover.Click += (_, _) => ctx.Window.OpenSeries(work);
        grid.Children.Add(cover);

        var inner = new Grid { ColumnSpacing = 12 };
        inner.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        inner.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var words = new StackPanel { Spacing = 2, VerticalAlignment = VerticalAlignment.Center };
        var silk = Ui.Res<Brush>("SilkBrush");
        var haze = Ui.Res<Brush>("HazeBrush");
        words.Children.Add(new TextBlock
        {
            Text = row.Tile.Title,
            FontSize = 13,
            FontWeight = unread ? FontWeights.SemiBold : FontWeights.Normal,
            Foreground = unread ? silk : haze,
            TextTrimming = TextTrimming.CharacterEllipsis,
            TextWrapping = TextWrapping.NoWrap,
        });
        var chapter = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
        if (unread)
            chapter.Children.Add(ReadingParts.WaterDot());
        var line = Line(row.Chapter);
        chapter.Children.Add(new TextBlock { Text = line, FontSize = 12, Foreground = unread ? silk : haze, TextTrimming = TextTrimming.CharacterEllipsis });
        words.Children.Add(chapter);
        var doing = row.Chapter.Download is not null ? ReadingParts.ChapterState(row.Chapter) : null;
        if (doing is not null)
            words.Children.Add(new TextBlock { Text = doing, FontSize = 11, Foreground = haze });
        inner.Children.Add(words);
        string? time = null;
        if (row.Chapter.Published is { } at)
        {
            time = ReadingParts.Clock(at);
            var clock = new TextBlock { Text = time, FontSize = 12, Foreground = haze, VerticalAlignment = VerticalAlignment.Center };
            Grid.SetColumn(clock, 1);
            inner.Children.Add(clock);
        }
        var open = new Button
        {
            Style = Ui.Res<Style>("HomeCardButtonStyle"),
            Content = inner,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Stretch,
        };
        AutomationProperties.SetName(open, ReadingParts.Joined(row.Tile.Title, line, doing, time));
        AutomationProperties.SetHelpText(open, T.ChapterUpdatesOpenChapterHint);
        open.Click += (_, _) => ctx.Window.OpenReader(work, row.Chapter.Id);
        Grid.SetColumn(open, 1);
        grid.Children.Add(open);

        // One menu for the row, on the cover and the words, by right-click, Shift+F10 or the menu key.
        var menu = new MenuFlyout();
        if (unread)
            menu.Items.Add(ReadingParts.Item(T.SeriesMarkRead, () => ctx.Known.Reading.MarkRead(work, [row.Chapter.Id], true)));
        else
            menu.Items.Add(ReadingParts.Item(T.SeriesMarkUnread, () => ctx.Known.Reading.MarkRead(work, [row.Chapter.Id], false)));
        if (row.Chapter.Download is null)
            menu.Items.Add(ReadingParts.Item(T.SeriesDownload, () => ctx.Known.Reading.Download(work, new DownloadPick.Chapters([row.Chapter.Id]))));
        menu.Items.Add(ReadingParts.Item(T.ChapterUpdatesOpenSeriesMenu, () => ctx.Window.OpenSeries(work)));
        open.ContextFlyout = menu;
        cover.ContextFlyout = menu;
        return grid;
    }

    /// <summary>A series the check could not ask: its title, the reason, and Try again.</summary>
    UIElement FailureRow(UpdateFailure failure)
    {
        var grid = new Grid { ColumnSpacing = 12, Padding = new Thickness(0, 10, 0, 10) };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var glyph = States.ErrorGlyph(14);
        glyph.VerticalAlignment = VerticalAlignment.Center;
        grid.Children.Add(glyph);
        var words = new StackPanel { Spacing = 2 };
        words.Children.Add(new TextBlock { Text = failure.Title, FontSize = 13, Foreground = Ui.Res<Brush>("SilkBrush"), TextTrimming = TextTrimming.CharacterEllipsis });
        words.Children.Add(new TextBlock { Text = ReadingParts.Joined(failure.SourceName, failure.Failure.Words()), Style = Ui.Res<Style>("HazeCaptionStyle"), TextWrapping = TextWrapping.Wrap });
        Grid.SetColumn(words, 1);
        grid.Children.Add(words);
        var retry = Ui.Label(T.ActionTryAgain, (_, _) => ctx.Known.Reading.CheckUpdates());
        retry.VerticalAlignment = VerticalAlignment.Center;
        AutomationProperties.SetName(retry, ReadingParts.Joined(T.ActionTryAgain, failure.Title));
        Grid.SetColumn(retry, 2);
        grid.Children.Add(retry);
        return Ui.Card(grid, 0);
    }
}
