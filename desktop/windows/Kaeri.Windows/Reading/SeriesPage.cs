using Medium = Kaeri.Engine.Core.Medium;
using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Navigation;

namespace Kaeri.Windows.Pages;

/// <summary>
/// A series to read: the cover, the title, the facts in small capitals and the synopsis, then the
/// one amber action naming the chapter and page, with Status and Download beside it. To the side,
/// the source in use with Change, the trackers' progress and the preferred group. The chapters
/// below, grouped by volume, read volumes folded. The navigation parameter is the work id. The
/// twin of the Apple app's SeriesPage.
/// </summary>
public sealed partial class SeriesPage : Page
{
    const double Wide = 900;

    readonly Ctx ctx = Ctx.Current;
    string work = "";

    readonly StateHost state = new();
    readonly ScrollViewer scroll = new() { HorizontalScrollMode = ScrollMode.Disabled, VerticalScrollBarVisibility = ScrollBarVisibility.Auto };
    readonly Grid body = new() { MaxWidth = 1180, Margin = new Thickness(32, 24, 32, 40), ColumnSpacing = 40, RowSpacing = 24 };
    readonly Grid head = new() { ColumnSpacing = 28 };
    readonly StackPanel side = new() { Spacing = 0 };
    readonly StackPanel chapters = new() { Spacing = 0 };
    readonly SendStatusRows sends;
    readonly Grid chooseBar = new() { Padding = new Thickness(32, 10, 32, 12), ColumnSpacing = 8, Visibility = Visibility.Collapsed };
    readonly TextBlock estimate = Ui.Text("", "HazeCaptionStyle", wrap: false);
    readonly Button chooseMark, chooseSend, chooseDownload;
    readonly DispatcherQueueTimer estimateClock;

    bool newestFirst = true, wide = true;
    /// <summary>The chapters ticked while choosing; null when not choosing.</summary>
    HashSet<string>? choosing;
    /// <summary>Volumes opened or folded by hand, which a redraw keeps.</summary>
    readonly Dictionary<string, bool> folded = [];
    /// <summary>Remove from lists was asked: its answer opens the result.</summary>
    bool removing;
    /// <summary>A chapter's Versions submenu while it is open, filled when the engine answers.</summary>
    (string Chapter, MenuFlyoutSubItem Item)? versionsOpen;
    string? drawnHead, drawnSide, drawnChapters;
    /// <summary>The header's words as last drawn, which hold the send rows.</summary>
    StackPanel? drawnWords;

    public SeriesPage()
    {
        NavigationCacheMode = NavigationCacheMode.Disabled;
        body.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        body.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(300) });
        for (var i = 0; i < 3; i++)
            body.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        body.Children.Add(head);
        body.Children.Add(side);
        body.Children.Add(chapters);
        scroll.Content = body;
        state.Children.Add(scroll);
        state.Placeholders = Placeholders;
        state.ActivityText = T.StateLoading;

        sends = new SendStatusRows(ctx, () => work, () => ctx.Known.Reading.Series.GetValueOrDefault(work));

        // Choose chapters' bar: Cancel, Mark read, Send, and the amber Download with the count.
        chooseBar.Background = Ui.Res<Brush>("RaisedBrush");
        chooseBar.BorderBrush = Ui.Res<Brush>("FaintRuleBrush");
        chooseBar.BorderThickness = new Thickness(0, 1, 0, 0);
        chooseBar.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        for (var i = 0; i < 4; i++)
            chooseBar.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        estimate.VerticalAlignment = VerticalAlignment.Center;
        AutomationProperties.SetLiveSetting(estimate, AutomationLiveSetting.Polite);
        chooseBar.Children.Add(estimate);
        var cancel = Ui.Label(T.ActionCancel, (_, _) => StopChoosing());
        chooseMark = Ui.Label(T.SeriesMarkRead, (_, _) =>
        {
            if (choosing is { Count: > 0 } chosen)
                ctx.Known.Reading.MarkRead(work, chosen, true);
            StopChoosing();
        });
        chooseSend = Ui.Label(T.SendTitle, async (_, _) =>
        {
            if (choosing is not { Count: > 0 } chosen || Series is not { } series)
                return;
            var ids = Ordered(chosen);
            StopChoosing();
            await SendDialog.Show(ctx, XamlRoot, series, ids);
        });
        chooseDownload = Ui.Label(T.DownloadChapters(0), (_, _) =>
        {
            if (choosing is { Count: > 0 } chosen)
                ctx.Known.Reading.Download(work, new DownloadPick.Chapters([.. Ordered(chosen)]));
            StopChoosing();
        }, accent: true);
        var at = 1;
        foreach (var button in new[] { cancel, chooseMark, chooseSend, chooseDownload })
        {
            Grid.SetColumn(button, at++);
            chooseBar.Children.Add(button);
        }
        // Escape leaves choosing, as Cancel does.
        var escape = new Microsoft.UI.Xaml.Input.KeyboardAccelerator { Key = global::Windows.System.VirtualKey.Escape };
        escape.Invoked += (_, e) =>
        {
            if (choosing is null)
                return;
            e.Handled = true;
            StopChoosing();
        };
        KeyboardAccelerators.Add(escape);
        estimateClock = Timers.Weak(this, TimeSpan.FromMilliseconds(150), false, static page => page.AskEstimate());

        var root = new Grid();
        root.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });
        root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        root.Children.Add(state);
        Grid.SetRow(chooseBar, 1);
        root.Children.Add(chooseBar);
        Content = root;
        SizeChanged += (_, e) => Layout(e.NewSize.Width);
    }

    SeriesView? Series => ctx.Known.Reading.Series.GetValueOrDefault(work);

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        work = e.Parameter as string ?? "";
        ctx.Known.Reading.Changed += OnReading;
        ctx.Known.Books.Changed += OnBooks;
        ctx.Known.Medium.Changed += OnMedium;
        ctx.Hear(this, _ => { }, () =>
        {
            ctx.Known.Reading.Changed -= OnReading;
            ctx.Known.Books.Changed -= OnBooks;
            ctx.Known.Medium.Changed -= OnMedium;
            estimateClock.Stop();
        });
        var known = Series is not null;
        state.Wait(hasContent: known);
        if (known)
            Draw();
        ctx.Known.Reading.LoadSeries(work);
        ctx.Known.Books.LoadForWork(work);
        ctx.Known.Reading.Sending.LoadTargets();
#if KAERI_TESTING
        Shots();
#endif
    }

    void OnReading(ReadingPart part)
    {
        switch (part)
        {
            case ReadingPart.Series:
            case ReadingPart.Tracking:
                if (Series is not null)
                    Draw();
                break;
            case ReadingPart.Chapters:
                DrawChapters();
                break;
            case ReadingPart.Versions:
                FillVersions();
                break;
            case ReadingPart.Send:
            case ReadingPart.Targets:
                sends.Draw();
                DrawSide();
                break;
            case ReadingPart.Estimate:
                DrawEstimate();
                break;
            case ReadingPart.Removals:
                if (removing && ctx.Known.Reading.Removals.TryGetValue(work, out var results))
                {
                    removing = false;
                    _ = SeriesDialogs.Removed(XamlRoot, results);
                }
                break;
            case ReadingPart.Failure:
                if (Series is null && ctx.Known.Reading.Failures.TryGetValue(ReadingAbout.Series, out var failure))
                    state.Failed(failure.Words(), () => ctx.Known.Reading.LoadSeries(work));
                break;
        }
    }

    void OnBooks()
    {
        if (Series is not null)
            DrawSide();
    }

    void OnMedium()
    {
        if (Series is not null)
            DrawSide();
    }

    // Drawing

    void Draw()
    {
        if (Series is not { } series)
            return;
        DrawHead(series);
        DrawSide();
        DrawChapters();
        state.Done();
    }

    void Layout(double width)
    {
        var now = width >= Wide;
        if (now == wide && body.Children.Count > 0 && Grid.GetRow(chapters) != 0)
            return;
        wide = now;
        Grid.SetRow(head, 0);
        Grid.SetColumnSpan(head, wide ? 1 : 2);
        // Wide: the panel beside the header and the chapters; narrow, under the header.
        Grid.SetRow(side, wide ? 0 : 1);
        Grid.SetColumn(side, wide ? 1 : 0);
        Grid.SetColumnSpan(side, wide ? 1 : 2);
        Grid.SetRowSpan(side, wide ? 3 : 1);
        Grid.SetRow(chapters, wide ? 1 : 2);
        Grid.SetColumnSpan(chapters, wide ? 1 : 2);
        body.ColumnDefinitions[1].Width = wide ? new GridLength(300) : new GridLength(0);
        body.Margin = wide ? new Thickness(32, 24, 32, 40) : new Thickness(16, 16, 16, 32);
        // The cover's size follows the width.
        drawnHead = null;
        if (Series is { } series)
            DrawHead(series);
    }

    void DrawHead(SeriesView series)
    {
        var print = Drawn.Fingerprint((wide, series.Titles, series.Cover, series.Facts, series.Synopsis, series.Action, series.List, series.Score, Volumes().Select(v => (v.Volume, v.Chapters.Count(c => c.Download is null))).ToList(), Downloaded().Count));
        if (print == drawnHead)
            return;
        drawnHead = print;
        var focus = Ui.MarkFocus(head);
        head.Children.Clear();

        var cover = SeriesWords.Cover(ctx, series.Cover, wide ? 200 : 120, wide ? 300 : 180);
        cover.VerticalAlignment = VerticalAlignment.Top;
        head.Children.Add(cover);

        var words = new StackPanel { Spacing = 8 };
        Grid.SetColumn(words, 1);
        var title = Ui.Text(series.Titles.Display, wide ? "ShowTitleDisplayStyle" : "ShowTitleMediumStyle");
        AutomationProperties.SetHeadingLevel(title, AutomationHeadingLevel.Level1);
        title.IsTextSelectionEnabled = true;
        words.Children.Add(title);
        if (new[] { series.Titles.Romaji, series.Titles.Native, series.Titles.English }.FirstOrDefault(t => t is { Length: > 0 } && t != series.Titles.Display) is { } other)
            words.Children.Add(Ui.Haze(other));
        if (series.Facts.Joined() is { Length: > 0 } facts)
            words.Children.Add(SeriesWords.SmallCaps(facts));
        if (series.Synopsis is { Length: > 0 } synopsis)
        {
            // Six lines, and all of it on a click; a screen reader reads it whole either way.
            var text = Ui.Text(synopsis, "BodyTextBlockStyle");
            text.MaxLines = 6;
            text.TextTrimming = TextTrimming.WordEllipsis;
            text.Margin = new Thickness(0, 4, 0, 0);
            text.Tapped += (_, _) => text.MaxLines = text.MaxLines == 0 ? 6 : 0;
            words.Children.Add(text);
        }
        words.Children.Add(Actions(series));
        // The send rows are kept across redraws: they leave the old words for the new.
        // Parent can read null while they still sit in the old words, so the old words are kept.
        drawnWords?.Children.Remove(sends);
        words.Children.Add(sends);
        drawnWords = words;
        head.Children.Add(words);
        Ui.RestoreFocus(head, focus);
    }

    /// <summary>The amber action naming the chapter and page, then Status, Download and More.</summary>
    StackPanel Actions(SeriesView series)
    {
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, Margin = new Thickness(0, 12, 0, 0) };
        if (SeriesWords.Action(series.Action) is { } words)
        {
            var main = new Button { Content = words, Style = Ui.Res<Style>("PrimaryActionStyle") };
            main.Click += (_, _) => Perform(series);
            row.Children.Add(main);
        }
        else
        {
            // No source has it: the way on is another source.
            var another = new Button { Content = T.SeriesReadFromAnother, MinHeight = 40 };
            another.Click += async (_, _) => await SeriesDialogs.ReadFrom(ctx, XamlRoot, work);
            row.Children.Add(another);
        }
        row.Children.Add(Menu(series.List is { } list ? SeriesWords.List(list) : T.SeriesAddToList, StatusMenu(series)));
        row.Children.Add(Menu(T.SeriesDownload, DownloadMenu()));
        var more = new DropDownButton { Content = new FontIcon { Glyph = "", FontSize = 14 }, MinHeight = 40, Flyout = MoreMenu(series) };
        AutomationProperties.SetName(more, T.ActionMore);
        ToolTipService.SetToolTip(more, T.ActionMore);
        row.Children.Add(more);
        return row;
    }

    static DropDownButton Menu(string words, MenuFlyout flyout) => new() { Content = words, MinHeight = 40, Flyout = flyout };

    void Perform(SeriesView series)
    {
        switch (series.Action)
        {
            case SeriesAction.ReadAgain:
                _ = SeriesDialogs.ReadAgain(ctx, XamlRoot, series);
                break;
            case SeriesAction.NoSource:
                _ = SeriesDialogs.ReadFrom(ctx, XamlRoot, work);
                break;
            default:
                ctx.Window.OpenReader(work, SeriesWords.Chapter(series.Action));
                break;
        }
    }

    /// <summary>The lists, a score from 10 to 1, and Remove from lists.</summary>
    MenuFlyout StatusMenu(SeriesView series)
    {
        var menu = new MenuFlyout();
        foreach (var status in SeriesWords.Statuses)
            menu.Items.Add(SeriesWords.Item(SeriesWords.List(status), () => ctx.Known.Reading.SetStatus(work, status), on: series.List == status));
        menu.Items.Add(new MenuFlyoutSeparator());
        var score = new MenuFlyoutSubItem { Text = T.SeriesScore };
        for (var n = 10; n >= 1; n--)
        {
            var value = n;
            score.Items.Add(SeriesWords.Item(value.ToString(System.Globalization.CultureInfo.CurrentCulture),
                () => ctx.Known.Reading.SetScore(work, value), on: series.Score is { } now && (int)Math.Round(now) == value));
        }
        menu.Items.Add(score);
        if (series.List is not null)
        {
            menu.Items.Add(new MenuFlyoutSeparator());
            menu.Items.Add(SeriesWords.Item(T.SeriesRemoveFromLists, async () =>
            {
                if (await SeriesDialogs.AskRemove(XamlRoot, series))
                {
                    removing = true;
                    ctx.Known.Reading.RemoveFromLists(work);
                }
            }));
        }
        return menu;
    }

    IReadOnlyList<VolumeGroup> Volumes() => ctx.Known.Reading.Chapters.TryGetValue(work, out var c) ? c.Volumes : [];

    List<ChapterRow> Downloaded() => [.. Volumes().SelectMany(v => v.Chapters).Where(c => c.Download is not null)];

    /// <summary>The next 5 or 10 unread, every unread chapter, a volume, Choose chapters, and delete.</summary>
    MenuFlyout DownloadMenu()
    {
        var menu = new MenuFlyout();
        var volumes = Volumes();
        var unread = volumes.SelectMany(v => v.Chapters).Any(c => c.State is not ChapterReadState.Read && c.Download is null);
        menu.Items.Add(SeriesWords.Item(T.DownloadNext(5), () => ctx.Known.Reading.Download(work, new DownloadPick.NextUnread(5))));
        menu.Items.Add(SeriesWords.Item(T.DownloadNext(10), () => ctx.Known.Reading.Download(work, new DownloadPick.NextUnread(10))));
        menu.Items.Add(SeriesWords.Item(T.DownloadUnread, () => ctx.Known.Reading.Download(work, new DownloadPick.Unread()), enabled: unread));
        var toFetch = volumes.Where(g => g.Chapters.Any(c => c.Download is null)).ToList();
        if (toFetch.Count > 0)
            menu.Items.Add(new MenuFlyoutSeparator());
        var novel = Series?.Medium == Medium.Novel;
        foreach (var group in toFetch)
        {
            var missing = group.Chapters.Where(c => c.Download is null).Select(c => c.Id).ToList();
            menu.Items.Add(SeriesWords.Item(T.DownloadVolumeCount(SeriesWords.Volume(group, novel).Name, missing.Count), () =>
            {
                if (group.Volume is { Length: > 0 } volume)
                    ctx.Known.Reading.Download(work, new DownloadPick.Volume(volume));
                else
                    ctx.Known.Reading.Download(work, new DownloadPick.Chapters([.. missing]));
            }));
        }
        menu.Items.Add(new MenuFlyoutSeparator());
        menu.Items.Add(SeriesWords.Item(T.DownloadChoose, StartChoosing));
        var downloaded = Downloaded();
        if (downloaded.Count > 0)
        {
            menu.Items.Add(new MenuFlyoutSeparator());
            menu.Items.Add(SeriesWords.Destructive(T.DownloadDeleteSeries, () => ctx.Known.Reading.DeleteDownloads(work, downloaded.Select(c => c.Id))));
        }
        return menu;
    }

    /// <summary>More: the trackers, Read from, and Read again from the start.</summary>
    MenuFlyout MoreMenu(SeriesView series)
    {
        var menu = new MenuFlyout();
        menu.Items.Add(SeriesWords.Item(T.SeriesTracking, () => _ = SeriesDialogs.Tracking(ctx, XamlRoot, work)));
        menu.Items.Add(SeriesWords.Item(T.SeriesReadFromMenu, () => _ = SeriesDialogs.ReadFrom(ctx, XamlRoot, work)));
        menu.Items.Add(new MenuFlyoutSeparator());
        menu.Items.Add(SeriesWords.Item(T.SeriesReadAgainMenu, () => _ = SeriesDialogs.ReadAgain(ctx, XamlRoot, series)));
        return menu;
    }

    // The side panel

    /// <summary>Reading from, the trackers' progress, the preferred group, where you stopped, and the other medium's version.</summary>
    void DrawSide()
    {
        if (Series is not { } series)
            return;
        var tracking = ctx.Known.Reading.Tracking.GetValueOrDefault(work) ?? series.Tracking;
        var editions = ctx.Known.Books.ForWork.GetValueOrDefault(work)?.Editions ?? [];
        var both = ctx.Known.Medium.Both;
        var print = Drawn.Fingerprint((series.Edition, tracking, editions.Length, series.AskWhereStopped, both ? series.OtherMedium : null, ctx.Known.Reading.Sending.Targets.Select(t => (t.Id, t.Name)).ToList()));
        if (print == drawnSide)
            return;
        drawnSide = print;
        var focus = Ui.MarkFocus(side);
        side.Children.Clear();

        if (series.Edition is { } edition)
            AddSide(T.SeriesReadingFrom, SeriesWords.Source(edition, Math.Max(0, editions.Length - 1)), T.SeriesChange,
                () => _ = SeriesDialogs.ReadFrom(ctx, XamlRoot, work));
        var followed = tracking.Where(t => t.Mapping is TrackerMapping.Mapped).ToList();
        if (followed.Count > 0)
            AddSide(SeriesWords.Names([.. followed.Select(t => SeriesWords.Tracker(t.Tracker))]), SeriesWords.Progress(followed[0], series), T.SeriesTracking,
                () => _ = SeriesDialogs.Tracking(ctx, XamlRoot, work));
        else if (tracking.Length > 0)
            AddSide(SeriesWords.Names([.. tracking.Select(t => SeriesWords.Tracker(t.Tracker))]), T.SeriesTrackerAsk, T.SeriesChange,
                () => _ = SeriesDialogs.Tracking(ctx, XamlRoot, work));
        if (series.Edition?.VersionLabel is { Length: > 0 } label)
            AddSide(label, T.SeriesPreferredGroup, T.SeriesChange, () => _ = SeriesDialogs.ReadFrom(ctx, XamlRoot, work));
        if (series.AskWhereStopped is { } sent)
            AddSide(T.SendTitle, T.StopNote(SeriesWords.Device(ctx, sent.Target)), T.StopNoteAction,
                () => _ = StoppedDialog.Show(ctx, XamlRoot, series, sent));
        if (both && series.OtherMedium is { } otherMedium)
        {
            side.Children.Add(SeriesWords.Rule());
            var open = new Button
            {
                HorizontalAlignment = HorizontalAlignment.Stretch,
                HorizontalContentAlignment = HorizontalAlignment.Stretch,
                Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
                BorderThickness = new Thickness(0),
                Padding = new Thickness(0, 8, 0, 8),
            };
            var line = new Grid { ColumnSpacing = 8 };
            line.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            line.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            line.Children.Add(SeriesWords.Pair(otherMedium.Relation.Text(), otherMedium.Title));
            var chevron = new FontIcon { Glyph = "", FontSize = 12, Foreground = Ui.Res<Brush>("TextFillColorSecondaryBrush") };
            Grid.SetColumn(chevron, 1);
            line.Children.Add(chevron);
            open.Content = line;
            AutomationProperties.SetName(open, $"{otherMedium.Relation.Text()}, {otherMedium.Title}");
            open.Click += (_, _) =>
            {
                if (otherMedium.Medium == Medium.Anime && uint.TryParse(otherMedium.Work.Split(':').Last(), out var id))
                    ctx.OpenTitle(id);
                else
                    ctx.Window.OpenSeries(otherMedium.Work);
            };
            side.Children.Add(open);
        }
        Ui.RestoreFocus(side, focus);
    }

    void AddSide(string label, string value, string? action, Action act)
    {
        side.Children.Add(SeriesWords.Rule());
        var row = new Grid { ColumnSpacing = 12, Padding = new Thickness(0, 10, 0, 10) };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.Children.Add(SeriesWords.Pair(label, value));
        if (action is not null)
        {
            var button = new HyperlinkButton { Content = action, Style = Ui.Res<Style>("SilkLinkStyle"), VerticalAlignment = VerticalAlignment.Center };
            AutomationProperties.SetName(button, $"{action}, {label}");
            button.Click += (_, _) => act();
            Grid.SetColumn(button, 1);
            row.Children.Add(button);
        }
        side.Children.Add(row);
    }

    // The chapters

    void DrawChapters()
    {
        if (Series is not { } series)
            return;
        ctx.Known.Reading.Chapters.TryGetValue(work, out var read);
        var print = Drawn.Fingerprint((read, newestFirst, choosing?.Count, choosing is null ? null : string.Join(",", choosing.Order())));
        if (print == drawnChapters)
            return;
        drawnChapters = print;
        // The head's Download menu counts what is left to fetch.
        DrawHead(series);
        var focus = Ui.MarkFocus(chapters);
        chapters.Children.Clear();

        var volumes = read?.Volumes ?? [];
        var byVolume = volumes.Any(v => v.Volume is { Length: > 0 }) && volumes.Length > 1;
        var top = new Grid { Margin = new Thickness(0, 8, 0, 8) };
        top.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        top.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var heading = Ui.Text(byVolume ? T.SeriesVolumes : T.SeriesChapters, "SectionTitleStyle");
        AutomationProperties.SetHeadingLevel(heading, AutomationHeadingLevel.Level2);
        top.Children.Add(heading);
        top.Children.Add(OrderButton());
        chapters.Children.Add(top);

        if (read is null)
        {
            chapters.Children.Add(States.Rows(6, 0, 0, 46, 0));
            return;
        }
        if (read.Failure is ReadingFailure.NoSource)
        {
            // Nothing was ever read from a source, so nothing failed: an empty state that leads to Read from.
            chapters.Children.Add(States.Unavailable(T.SeriesNoSourceTitle, T.SeriesNoSourceLine, T.SeriesNoSourceSearch,
                () => _ = SeriesDialogs.ReadFrom(ctx, XamlRoot, work)));
            return;
        }
        if (read.Failure is { } failure)
        {
            var source = read.Edition?.SourceName ?? series.Edition?.SourceName ?? "";
            var block = new StackPanel { Spacing = 6, Margin = new Thickness(0, 12, 0, 12) };
            var line = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
            line.Children.Add(States.ErrorGlyph());
            line.Children.Add(Ui.Text(T.SeriesChaptersFailed(source), "BodyStrongTextBlockStyle"));
            block.Children.Add(line);
            block.Children.Add(Ui.Haze(failure.Words()));
            var retry = Ui.Label(T.ReaderTryAgain, (_, _) => ctx.Known.Reading.LoadChapters(work, read.Edition?.SourceId));
            retry.Margin = new Thickness(0, 6, 0, 0);
            block.Children.Add(retry);
            chapters.Children.Add(block);
            return;
        }
        if (!read.All.Any())
        {
            chapters.Children.Add(States.Unavailable(T.SeriesNoChaptersTitle, T.SeriesNoChaptersLine, T.SeriesReadFromAnother,
                () => _ = SeriesDialogs.ReadFrom(ctx, XamlRoot, work)));
            return;
        }
        var preferred = read.Edition?.VersionLabel ?? series.Edition?.VersionLabel;
        var novel = series.Medium == Medium.Novel;
        IEnumerable<VolumeGroup> groups = newestFirst ? Enumerable.Reverse(volumes) : volumes;
        if (!byVolume)
        {
            foreach (var row in Ordered(volumes.SelectMany(v => v.Chapters)))
                chapters.Children.Add(Row(series, row, preferred));
        }
        else
        {
            foreach (var group in groups)
                chapters.Children.Add(Volume(series, group, preferred, novel));
        }
        Ui.RestoreFocus(chapters, focus);
    }

    IEnumerable<ChapterRow> Ordered(IEnumerable<ChapterRow> rows) => newestFirst ? Enumerable.Reverse(rows) : rows;

    /// <summary>The chapters chosen, in reading order.</summary>
    List<string> Ordered(HashSet<string> chosen) => [.. Volumes().SelectMany(v => v.Chapters).Select(c => c.Id).Where(chosen.Contains)];

    DropDownButton OrderButton()
    {
        var menu = new MenuFlyout();
        foreach (var (words, newest) in new[] { (T.SeriesNewestFirst, true), (T.SeriesOldestFirst, false) })
        {
            var item = new RadioMenuFlyoutItem { Text = words, GroupName = "order", IsChecked = newestFirst == newest };
            item.Click += (_, _) =>
            {
                newestFirst = newest;
                DrawChapters();
            };
            menu.Items.Add(item);
        }
        var button = new DropDownButton { Content = newestFirst ? T.SeriesNewestFirst : T.SeriesOldestFirst, Flyout = menu, VerticalAlignment = VerticalAlignment.Center };
        AutomationProperties.SetName(button, $"{T.SeriesOrder}, {(newestFirst ? T.SeriesNewestFirst : T.SeriesOldestFirst)}");
        Grid.SetColumn(button, 1);
        return button;
    }

    /// <summary>
    /// A volume: its heading with the chapters' range and "read", and its chapters under it.
    /// Read volumes start folded; one opened or folded by hand stays so. Rows are made when the
    /// volume first opens, so a long series draws quickly.
    /// </summary>
    Expander Volume(SeriesView series, VolumeGroup group, string? preferred, bool novel)
    {
        var key = group.Volume ?? "";
        var open = folded.TryGetValue(key, out var byHand) ? byHand : !group.AllRead;
        var (name, rest) = SeriesWords.Volume(group, novel);
        var header = new StackPanel { Spacing = 2 };
        header.Children.Add(Ui.Text(name, "BodyStrongTextBlockStyle", wrap: false));
        if (rest.Length > 0)
            header.Children.Add(Ui.Text(rest, "HazeCaptionStyle", wrap: false));
        var expander = new Expander
        {
            Header = header,
            IsExpanded = open,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Stretch,
            Margin = new Thickness(0, 0, 0, 4),
        };
        AutomationProperties.SetName(expander, rest.Length > 0 ? $"{name}, {rest}" : name);
        var rows = new StackPanel();
        expander.Content = rows;
        void Fill()
        {
            if (rows.Children.Count > 0)
                return;
            foreach (var row in Ordered(group.Chapters))
                rows.Children.Add(Row(series, row, preferred));
        }
        if (open)
            Fill();
        expander.Expanding += (_, _) =>
        {
            folded[key] = true;
            Fill();
        };
        expander.Collapsed += (_, _) => folded[key] = false;
        return expander;
    }

    /// <summary>
    /// A chapter: its name and title, a bookmark, the second line, and a ring while it downloads.
    /// A click reads it, or ticks it while choosing; its context menu has the rest.
    /// </summary>
    Button Row(SeriesView series, ChapterRow row, string? preferred)
    {
        var chosen = choosing?.Contains(row.Id) ?? false;
        var read = row.State is ChapterReadState.Read;
        var unreadNew = row.NewChapter && !read;
        var grid = new Grid { ColumnSpacing = 12, MinHeight = 34 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        if (choosing is not null)
        {
            var tick = new FontIcon { Glyph = chosen ? "" : "", FontSize = 16, VerticalAlignment = VerticalAlignment.Center };
            AutomationProperties.SetAccessibilityView(tick, AccessibilityView.Raw);
            grid.Children.Add(tick);
        }
        var words = new StackPanel { Spacing = 2, VerticalAlignment = VerticalAlignment.Center };
        Grid.SetColumn(words, 1);
        var first = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
        var name = SeriesWords.ChapterName(row);
        var nameText = Ui.Text(name, null, wrap: false);
        if (read)
            nameText.Foreground = Ui.Res<Brush>("TextFillColorSecondaryBrush");
        first.Children.Add(nameText);
        // A file named for its chapter has no title beyond the name.
        var title = row.Title is { Length: > 0 } t && t != name && t != row.Label ? t : null;
        if (title is not null)
            first.Children.Add(Ui.Text(title, "HazeBodyStyle", wrap: false));
        if (row.Bookmarked)
        {
            var mark = new FontIcon { Glyph = "", FontSize = 10, Foreground = Ui.Res<Brush>("TextFillColorSecondaryBrush") };
            AutomationProperties.SetAccessibilityView(mark, AccessibilityView.Raw);
            first.Children.Add(mark);
        }
        words.Children.Add(first);
        var line = SeriesWords.ChapterLine(row, preferred);
        if (line.Length > 0)
        {
            var second = Ui.Text(line, "HazeCaptionStyle", wrap: false);
            if (unreadNew)
                second.Foreground = Ui.Res<Brush>("TextFillColorPrimaryBrush");
            words.Children.Add(second);
        }
        grid.Children.Add(words);

        if (row.Download is ChapterDownloadState.Downloading { Total: > 0 } d)
        {
            var ring = new ProgressRing { IsIndeterminate = false, Value = 100.0 * d.DoneValue / d.Total, Width = 20, Height = 20 };
            // The stop square in the ring, as on the Mac and Android.
            var square = new Microsoft.UI.Xaml.Shapes.Rectangle
            {
                Width = 6,
                Height = 6,
                RadiusX = 1,
                RadiusY = 1,
                Fill = Ui.Res<Brush>("SilkBrush"),
                HorizontalAlignment = HorizontalAlignment.Center,
                VerticalAlignment = VerticalAlignment.Center,
            };
            var mark = new Grid { Width = 20, Height = 20 };
            mark.Children.Add(ring);
            mark.Children.Add(square);
            var stop = new Button { Content = mark, Padding = new Thickness(6), Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent), BorderThickness = new Thickness(0) };
            AutomationProperties.SetName(stop, T.ChapterDownloadStop);
            ToolTipService.SetToolTip(stop, T.ChapterDownloadStop);
            stop.Click += (_, _) => ctx.Known.Reading.CancelDownloads(work, [row.Id]);
            Grid.SetColumn(stop, 2);
            grid.Children.Add(stop);
        }
        else if (row.Download is ChapterDownloadState.Failed)
        {
            var retry = Ui.Label(T.ChapterRetry, (_, _) => ctx.Known.Reading.Download(work, new DownloadPick.Chapters([row.Id])));
            retry.VerticalAlignment = VerticalAlignment.Center;
            AutomationProperties.SetName(retry, $"{T.ChapterRetry}, {name}");
            Grid.SetColumn(retry, 2);
            grid.Children.Add(retry);
        }

        var button = new Button
        {
            Content = grid,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Stretch,
            Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
            BorderThickness = new Thickness(0),
            Padding = new Thickness(12, 6, 12, 6),
            MinHeight = 46,
        };
        if (chosen)
            button.Background = Ui.Res<Brush>("RaisedBrush");
        AutomationProperties.SetName(button, string.Join(", ", new[] { name, title, row.Bookmarked ? T.SeriesBookmarked : null, line }.Where(s => s is { Length: > 0 })));
        if (choosing is not null)
            AutomationProperties.SetItemStatus(button, chosen ? T.SeriesChosen(1) : "");
        button.Click += (_, _) =>
        {
            if (choosing is { } set)
            {
                if (!set.Remove(row.Id))
                    set.Add(row.Id);
                ChoosingChanged();
            }
            else
            {
                ctx.Window.OpenReader(work, row.Id);
            }
        };
        var menu = new MenuFlyout();
        menu.Opening += (_, _) => FillMenu(menu, series, row);
        button.ContextFlyout = menu;
        return button;
    }

    /// <summary>A chapter's menu: read, the marks, its download, Send to e-reader, the bookmark and its versions.</summary>
    void FillMenu(MenuFlyout menu, SeriesView series, ChapterRow row)
    {
        var reading = ctx.Known.Reading;
        var read = row.State is ChapterReadState.Read;
        menu.Items.Clear();
        menu.Items.Add(SeriesWords.Item(T.SeriesReadChapterMenu, () => ctx.Window.OpenReader(work, row.Id)));
        menu.Items.Add(new MenuFlyoutSeparator());
        menu.Items.Add(SeriesWords.Item(read ? T.SeriesMarkUnread : T.SeriesMarkRead, () => reading.MarkRead(work, [row.Id], !read)));
        menu.Items.Add(SeriesWords.Item(T.SeriesMarkReadUpTo, () => reading.MarkReadUpTo(work, row.Id)));
        menu.Items.Add(new MenuFlyoutSeparator());
        switch (row.Download)
        {
            case null:
                menu.Items.Add(SeriesWords.Item(T.SeriesDownload, () => reading.Download(work, new DownloadPick.Chapters([row.Id]))));
                break;
            case ChapterDownloadState.Done:
                menu.Items.Add(SeriesWords.Destructive(T.ChapterDownloadDelete, () => reading.DeleteDownloads(work, [row.Id])));
                break;
            case ChapterDownloadState.Failed:
                menu.Items.Add(SeriesWords.Item(T.ChapterRetry, () => reading.Download(work, new DownloadPick.Chapters([row.Id]))));
                break;
            default:
                menu.Items.Add(SeriesWords.Item(T.ChapterDownloadStop, () => reading.CancelDownloads(work, [row.Id])));
                break;
        }
        menu.Items.Add(SeriesWords.Item(T.SendChapterAction, () => _ = SendDialog.Show(ctx, XamlRoot, series, [row.Id])));
        menu.Items.Add(SeriesWords.Item(row.Bookmarked ? T.SeriesRemoveBookmark : T.SeriesBookmark, () => reading.Bookmark(work, row.Id, !row.Bookmarked)));
        if (row.Versions > 1)
        {
            menu.Items.Add(new MenuFlyoutSeparator());
            var versions = new MenuFlyoutSubItem { Text = T.SeriesVersions(row.Versions) };
            versionsOpen = (row.Id, versions);
            menu.Closed += (_, _) =>
            {
                if (versionsOpen?.Item == versions)
                    versionsOpen = null;
            };
            FillVersions();
            reading.LoadVersions(work, row.Id);
            menu.Items.Add(versions);
        }
    }

    /// <summary>The open Versions submenu: the edition in use ticked, each other one to prefer.</summary>
    void FillVersions()
    {
        if (versionsOpen is not { } open)
            return;
        var list = ctx.Known.Reading.Versions.GetValueOrDefault(open.Chapter) ?? [];
        open.Item.Items.Clear();
        if (list.Length == 0)
        {
            open.Item.Items.Add(new MenuFlyoutItem { Text = T.SeriesVersionsLoading, IsEnabled = false });
            return;
        }
        foreach (var version in list)
        {
            var name = version.Edition.VersionLabel is { Length: > 0 } label ? label : version.Edition.SourceName;
            if (version.Edition.Preferred)
                open.Item.Items.Add(SeriesWords.Item(T.SeriesVersionInUse(name), () => { }, on: true));
            else
                open.Item.Items.Add(SeriesWords.Item(T.SeriesPreferVersion(name), () => ctx.Known.Reading.ChooseEdition(work, version.Edition)));
        }
    }

    // Choosing chapters

    void StartChoosing()
    {
        choosing = [];
        // Every volume opens while choosing, so a whole volume can be ticked.
        foreach (var group in Volumes())
            folded[group.Volume ?? ""] = true;
        ChoosingChanged();
        Motion.Show(chooseBar);
    }

    void StopChoosing()
    {
        choosing = null;
        estimateClock.Stop();
        ctx.Known.Reading.EstimateDownload(work, new DownloadPick.Chapters([]));
        Motion.Hide(chooseBar);
        DrawChapters();
    }

    void ChoosingChanged()
    {
        var count = choosing?.Count ?? 0;
        chooseDownload.Content = T.DownloadChapters(count);
        chooseMark.IsEnabled = chooseSend.IsEnabled = chooseDownload.IsEnabled = count > 0;
        chooseBar.Visibility = Visibility.Visible;
        DrawChapters();
        // A pause while ticking goes on, then one ask.
        estimateClock.Stop();
        estimateClock.Start();
    }

    void AskEstimate()
    {
        if (choosing is { } set)
            ctx.Known.Reading.EstimateDownload(work, new DownloadPick.Chapters([.. Ordered(set)]));
    }

    void DrawEstimate()
    {
        var line = choosing is { Count: > 0 } && ctx.Known.Reading.Estimates.TryGetValue(work, out var e) ? e.Line.Joined() : "";
        if (line != estimate.Text)
            estimate.Text = line;
    }

    // Placeholders

    UIElement Placeholders()
    {
        var grid = new Grid { MaxWidth = 1180, Margin = new Thickness(32, 24, 32, 40), ColumnSpacing = 28 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.Children.Add(States.Plate(200, 300, 8));
        var lines = new StackPanel { Spacing = 10 };
        Grid.SetColumn(lines, 1);
        lines.Children.Add(States.Title(320));
        lines.Children.Add(States.Plate(220, 12, 3));
        for (var i = 0; i < 4; i++)
            lines.Children.Add(States.Plate(480, 12, 3));
        lines.Children.Add(States.Plate(220, 40, 6));
        grid.Children.Add(lines);
        return grid;
    }

#if KAERI_TESTING
    /// <summary>Shots: a send in a state (--send), or one of the series dialogs (--show).</summary>
    async void Shots()
    {
        var sendState = ctx.Options.Send;
        var sheet = ctx.Options.Show;
        if (sendState is null && sheet is not ("read-from" or "tracking" or "remove-lists" or "send" or "stopped"))
            return;
        for (var i = 0; i < 50 && (Series is null || !ctx.Known.Reading.Chapters.ContainsKey(work) || ctx.Known.Reading.Sending.Targets.Length == 0); i++)
            await Task.Delay(100);
        if (Series is not { } series)
            return;
        var ids = Volumes().SelectMany(v => v.Chapters).Take(3).Select(c => c.Id).ToList();
        var target = ctx.Known.Reading.Sending.DefaultTarget;
        if (sendState is not null && target is not null)
        {
            var id = ctx.Known.Reading.Sending.Stage(sendState, work, ids, target.Id);
            sends.Draw();
            await SendDialog.Show(ctx, XamlRoot, series, ids, id);
            return;
        }
        switch (sheet)
        {
            case "read-from":
                await SeriesDialogs.ReadFrom(ctx, XamlRoot, work);
                break;
            case "tracking":
                await SeriesDialogs.Tracking(ctx, XamlRoot, work);
                break;
            case "remove-lists":
                await SeriesDialogs.AskRemove(XamlRoot, series);
                break;
            case "send":
                await SendDialog.Show(ctx, XamlRoot, series, ids);
                break;
            case "stopped" when series.AskWhereStopped is { } sent:
                await StoppedDialog.Show(ctx, XamlRoot, series, sent);
                break;
        }
    }
#endif
}
