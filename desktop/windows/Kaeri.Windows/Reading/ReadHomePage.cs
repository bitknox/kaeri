using Medium = Kaeri.Engine.Core.Medium;
using Microsoft.UI.Xaml.Automation.Peers;
using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Dispatching;
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
/// Reading's Home from the engine's view: the spotlight with its amber action, Continue with the
/// silk track, This week by day with a link to Updates, Up next, then the engine's discovery rows
/// (Trending, Because you finished, New, More of a genre, Finished), then the closing line. A
/// section with nothing in it leaves with its heading. Each section is kept and changes in
/// place; the spotlight crossfades to a new pick, and on a cold load holds its room with a plate
/// after 600 ms. A row on its way holds placeholders at final size; a row that fails with nothing
/// kept fades out and its gap closes. Past 4 s an activity line says Home is loading; a failure
/// is one window notice. One 948-wide column; posters 132 by 198 at radius 4. Titled Home with
/// reading alone and Read with both.
/// </summary>
public sealed partial class ReadHomePage : Page
{
    /// <summary>The row whose View all opened Search, and how it had the focus: Back puts the focus there again.</summary>
    internal static (string Row, FocusState How)? Returning { get; set; }

    readonly Ctx ctx = Ctx.Current;
    readonly TextBlock title = new() { Style = Ui.Res<Style>("PageTitleStyle") };
    readonly TextBlock offline = new() { Style = Ui.Res<Style>("HazeBodyStyle"), Visibility = Visibility.Collapsed };
    readonly StateHost state = new();
    readonly ScrollViewer scroll = new() { VerticalScrollBarVisibility = ScrollBarVisibility.Auto, IsTabStop = false };
    readonly StackPanel column = new()
    {
        MaxWidth = ReadingParts.Column,
        HorizontalAlignment = HorizontalAlignment.Left,
        Padding = new Thickness(0, 8, 36, 48),
        XYFocusKeyboardNavigation = XYFocusKeyboardNavigationMode.Enabled,
    };
    // The person's own sections, each in its own place, the discovery rows by id, and what closes Home.
    readonly StackPanel own = new(), spotHost = new(), rows = new(), tail = new();
    readonly Border demoHost = new() { Visibility = Visibility.Collapsed }, continueHost = new(), weekHost = new(), waitingHost = new(), upNextHost = new();
    readonly TextBlock activityLine = new() { Style = Ui.Res<Style>("HazeCaptionStyle"), Margin = new Thickness(0, 16, 0, 0), Visibility = Visibility.Collapsed };
    readonly Dictionary<string, ReadHomeRowView> rowViews = [];
    readonly Microsoft.UI.Dispatching.DispatcherQueueTimer activity;
    ReadingHomeView? shown;
    CoverSlot? spot;
    string spotKey = "", continueKey = "", weekKey = "", upNextKey = "", tailKey = "";
    // When Home was last asked for: a row's placeholders come 600 ms after it.
    DateTimeOffset asked = DateTimeOffset.Now;
    bool demoOnly, hearing, complained;

    public ReadHomePage()
    {
        Background = LiveBrush.Of("GroundBrush");
        AutomationProperties.SetHeadingLevel(title, AutomationHeadingLevel.Level1);
        var root = new Grid { Padding = new Thickness(36, 20, 0, 0), RowSpacing = 8 };
        root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        root.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });
        var head = new StackPanel { Margin = new Thickness(0, 0, 36, 0) };
        head.Children.Add(title);
        head.Children.Add(new OfflineLine(reading: true));
        head.Children.Add(offline);
        root.Children.Add(head);
        demoHost.Child = DemoLine();
        foreach (var part in new UIElement[] { spotHost, demoHost, continueHost, weekHost, waitingHost, upNextHost })
            own.Children.Add(part);
        waitingHost.Child = Waiting();
        waitingHost.Visibility = Visibility.Collapsed;
        AutomationProperties.SetLiveSetting(activityLine, AutomationLiveSetting.Polite);
        column.Children.Add(own);
        column.Children.Add(rows);
        column.Children.Add(activityLine);
        column.Children.Add(tail);
        scroll.Content = column;
        state.Children.Add(scroll);
        state.Placeholders = Placeholders;
        state.ActivityText = T.HomeLoading;
        Grid.SetRow(state, 1);
        root.Children.Add(state);
        Content = root;
        NavigationCacheMode = NavigationCacheMode.Enabled;
        activity = Timers.Weak(this, TimeSpan.FromMilliseconds(HomeWording.ActivityAfterMs), false, static page => page.ShowActivity());

        Refresh(VirtualKey.F5, VirtualKeyModifiers.None);
        Refresh(VirtualKey.R, VirtualKeyModifiers.Control);
        KeyboardAcceleratorPlacementMode = KeyboardAcceleratorPlacementMode.Hidden;
        Loaded += (_, _) =>
        {
            if (!hearing)
            {
                ctx.Known.Reading.Changed += Heard;
                ctx.Known.Medium.Changed += Titled;
            }
            hearing = true;
            Titled();
            // What came while the page was being built, before it heard.
            Show();
        };
        Unloaded += (_, _) =>
        {
            ctx.Known.Reading.Changed -= Heard;
            ctx.Known.Medium.Changed -= Titled;
            activity.Stop();
            hearing = false;
        };
    }

    void Refresh(VirtualKey key, VirtualKeyModifiers modifiers)
    {
        var accelerator = new KeyboardAccelerator { Key = key, Modifiers = modifiers };
        accelerator.Invoked += (_, e) =>
        {
            e.Handled = true;
            state.Wait(hasContent: shown is not null);
            Ask();
        };
        KeyboardAccelerators.Add(accelerator);
    }

    /// <summary>Home asked for again: the 600 ms and the 4 s count from now, and a new failure may be said.</summary>
    void Ask(bool refresh = true)
    {
        asked = DateTimeOffset.Now;
        complained = false;
        activity.Stop();
        activity.Start();
        ctx.Known.Reading.LoadHome(refresh: refresh);
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        // The demo's line follows the engine's demo state, which comes as its own event.
        ctx.Hear(this, batch =>
        {
            if (batch.Any(x => x is Event.Demo) && ctx.Known.Demo.OnlyBooks != demoOnly)
                Show(force: true);
        });
        Titled();
        if (ctx.Known.Reading.Home is null)
            state.Wait();
        else
            Show();
        Ask(refresh: false);
        // Back from a row's View all: the focus goes to that View all again.
        if (e.NavigationMode == NavigationMode.Back && Returning is { } back)
        {
            Returning = null;
            if (rowViews.TryGetValue(back.Row, out var view))
                DispatcherQueue.TryEnqueue(Microsoft.UI.Dispatching.DispatcherQueuePriority.Low, () => view.FocusViewAll(back.How));
        }
    }

    void Titled() => title.Text = ctx.Known.Medium.Both ? T.NavRead : T.HomeTitle;

    void Heard(ReadingPart part)
    {
        if (part == ReadingPart.Home)
            Show();
        else if (part == ReadingPart.Failure && ctx.Known.Reading.Failures.ContainsKey(ReadingAbout.Home) && ctx.Known.Reading.Home is null)
            state.Empty(ReadingParts.PlainEmpty(T.HomeFailed, ctx.Known.Reading.Failures[ReadingAbout.Home].Words(), T.ActionTryAgain, () =>
            {
                state.Wait();
                Ask();
            }, error: true));
    }

    /// <summary>The spotlight's plate and two rails, in the column's layout.</summary>
    UIElement Placeholders()
    {
        var plates = new StackPanel { Spacing = HomeWording.ShelfGap, MaxWidth = ReadingParts.Column, HorizontalAlignment = HorizontalAlignment.Left, Padding = new Thickness(0, 8, 36, 0) };
        plates.Children.Add(States.Plate(HomeWording.ArtMaxWidth * 0.8, HomeWording.ArtMaxWidth * 0.45, 16));
        plates.Children.Add(States.Covers(8, rail: true));
        plates.Children.Add(States.Covers(8, rail: true));
        return plates;
    }

    /// <summary>
    /// Draws the engine's view. Each of the person's own sections keeps its place and changes in
    /// place when its part of the view changed; the discovery rows are kept by id, so a row
    /// landing crossfades over its placeholders and nothing above it moves. The focus and the
    /// scroll stay where they were.
    /// </summary>
    void Show(bool force = false)
    {
        if (ctx.Known.Reading.Home is not { } home)
            return;
        if (!force && home == shown)
        {
            state.Done();
            return;
        }
        shown = home;
        demoOnly = ctx.Known.Demo.OnlyBooks;
        offline.Text = home.Offline && home.OfflineSince is { } since ? T.HomeOffline(ReadingParts.Clock(since)) : "";
        offline.Visibility = offline.Text.Length > 0 ? Visibility.Visible : Visibility.Collapsed;

        Spotlight(home);
        demoHost.Visibility = demoOnly && home.Spotlight is not null ? Visibility.Visible : Visibility.Collapsed;
        Section(continueHost, ref continueKey, home.ContinueReading, tiles => new ReadShelf(T.HomeReadContinue, Cards(tiles, track: true, continuing: true), artHeight: HomeWording.PosterHeight),
            (shelf, tiles) => shelf.Set(Cards(tiles, track: true, continuing: true)));
        var week = string.Join("\n", home.Week.Select(d => $"{d.Day}\t{d.Today}\t" + string.Join(",", d.Entries.Select(x => $"{x.Tile.Work}:{x.Tile.Title}:{x.Tile.Cover}:{x.OutAt}:{x.Usually}:{x.Chapter?.Text()}:{x.Tile.Next?.Text()}"))));
        if (week != weekKey)
        {
            weekKey = week;
            var mark = Ui.MarkFocus(weekHost);
            weekHost.Child = home.Week.Length > 0 ? Week(home.Week) : null;
            Ui.RestoreFocus(weekHost, mark);
        }
        waitingHost.Visibility = home.Pending.Contains(ReadHomePart.Yours) ? Visibility.Visible : Visibility.Collapsed;
        // View all opens Planning, so it shows only with Planning series in the row; a change in
        // that makes the shelf again with or without its link.
        var planning = ReadDiscovery.UpNextLinksPlanning(home.UpNext);
        ReadShelf UpNext(WorkTile[] tiles) => planning
            ? new ReadShelf(T.HomeUpNext, Cards(tiles), T.HomeViewAll, () => ReadingParts.OpenList(ctx, LibrarySegmentRead.Planning))
            : new ReadShelf(T.HomeUpNext, Cards(tiles));
        Section(upNextHost, ref upNextKey, home.UpNext, UpNext,
            (shelf, tiles) => { if (shelf.Linked == planning) shelf.Set(Cards(tiles)); else upNextHost.Child = UpNext(tiles); },
            planning ? "planning" : "");

        Rows(home.Rows ?? []);

        // A failure is one window notice, said once for each time Home is asked.
        if (home.Failed && !complained)
        {
            complained = true;
            ctx.Window.Say(T.HomeFailed, T.HomeTryAgain, () => Ask());
        }
        if (home.Pending.Length == 0)
        {
            activity.Stop();
            if (activityLine.Visibility == Visibility.Visible)
                Motion.Hide(activityLine);
        }

        var end = $"{home.Offline && (home.Rows ?? []).Length == 0}\n{home.End}";
        if (end != tailKey)
        {
            tailKey = end;
            var mark = Ui.MarkFocus(tail);
            tail.Children.Clear();
            if (home.Offline && (home.Rows ?? []).Length == 0)
                tail.Children.Add(OfflineRows());
            if (home.End is { } closing)
                tail.Children.Add(End(closing));
            Ui.RestoreFocus(tail, mark);
        }

        state.Done();
    }

    /// <summary>Continue's and Up next's cards: with the track on Continue, and Details alone in Up next's menu.</summary>
    IEnumerable<UIElement> Cards(WorkTile[] tiles, bool track = false, bool continuing = false) =>
        tiles.Select(t => (UIElement)ReadingParts.Card(ctx, t, track: track, continuing: continuing, lists: false));

    /// <summary>
    /// A shelf of the person's own in its place: made when it first has tiles, its cards changed
    /// in the same strip after, gone with its heading when it has none.
    /// </summary>
    static void Section(Border host, ref string key, WorkTile[] tiles, Func<WorkTile[], ReadShelf> make, Action<ReadShelf, WorkTile[]> update, string extra = "")
    {
        var now = extra + "\n" + string.Join("\n", tiles.Select(t => $"{t.Work}\t{t.Title}\t{t.Cover}\t{t.Progress}\t{t.NewChapter}\t{ReadingParts.Line(t)}"));
        if (now == key)
            return;
        key = now;
        if (tiles.Length == 0)
            host.Child = null;
        else if (host.Child is ReadShelf shelf)
            update(shelf, tiles);
        else
            host.Child = make(tiles);
    }

    /// <summary>
    /// The spotlight: a new pick crossfades over the old one. On a cold load, while the person's
    /// own part is on its way, a plate holds its room after 600 ms, so the rows never move down.
    /// </summary>
    void Spotlight(ReadingHomeView home)
    {
        if (home.Spotlight is { } tile && home.Spot is { } pick)
        {
            var key = string.Join("\n", tile.Work, tile.Title, tile.Cover, tile.Progress, pick.Case, pick.Line?.Text(), pick.Label.Text(), pick.Action, pick.Planned, pick.Banner, pick.Synopsis, pick.Facts.Joined());
            if (key == spotKey)
                return;
            spotKey = key;
            SpotSlot().Replace(new ReadSpotlightView(ctx, tile, pick));
            return;
        }
        spotKey = "";
        if (home.Pending.Contains(ReadHomePart.Yours))
        {
            SpotSlot().Ask(asked);
            return;
        }
        if (spot is { } gone)
        {
            spot = null;
            Leaving.Close(gone, () => spotHost.Children.Remove(gone));
        }
    }

    CoverSlot SpotSlot()
    {
        if (spot is not null)
            return spot;
        spot = new CoverSlot(SpotPlate(), new Grid());
        spotHost.Children.Add(spot);
        return spot;
    }

    /// <summary>The spotlight's plate: the words' bars beside the art's plate, at the spotlight's size.</summary>
    static FrameworkElement SpotPlate()
    {
        var plate = new Grid { ColumnSpacing = 40 };
        plate.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        plate.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var bars = new StackPanel { Spacing = 12, VerticalAlignment = VerticalAlignment.Center };
        bars.Children.Add(States.Plate(120, 14, 3));
        bars.Children.Add(States.Plate(320, 30, 4));
        bars.Children.Add(States.Plate(220, 12, 3));
        bars.Children.Add(States.Plate(360, 44, 3));
        bars.Children.Add(States.Plate(200, 32, 4));
        plate.Children.Add(bars);
        var art = States.Plate(HomeWording.ArtMaxWidth, Math.Round(HomeWording.ArtMaxWidth * 9 / 16), 16);
        Grid.SetColumn(art, 1);
        plate.Children.Add(art);
        AutomationProperties.SetAccessibilityView(plate, AccessibilityView.Raw);
        return plate;
    }

    /// <summary>Past 4 s with parts still on their way: "Loading suggestions", as Watch's Home says.</summary>
    void ShowActivity()
    {
        if (ctx.Known.Reading.Home is { Pending.Length: > 0 } || ctx.Known.Reading.Home is null)
        {
            activityLine.Text = T.HomeLoading;
            Motion.Show(activityLine);
        }
    }

    /// <summary>
    /// The discovery rows in the engine's order. A row that stays keeps its view and changes in
    /// place; a new one comes where it belongs; one that left fades out, then its gap closes.
    /// </summary>
    void Rows(ReadHomeRow[] now)
    {
        var ids = now.Select(r => r.Id).ToHashSet();
        foreach (var (id, view) in rowViews.ToList())
        {
            if (ids.Contains(id))
                continue;
            rowViews.Remove(id);
            view.Leave(() => rows.Children.Remove(view));
        }
        var at = 0;
        foreach (var row in now)
        {
            if (!rowViews.TryGetValue(row.Id, out var view))
            {
                view = new ReadHomeRowView(ctx, row);
                rowViews[row.Id] = view;
            }
            // Rows on their way out keep their place until their gap has closed.
            while (at < rows.Children.Count && rows.Children[at] is ReadHomeRowView leaving && !rowViews.ContainsValue(leaving))
                at++;
            var index = rows.Children.IndexOf(view);
            if (index != at)
            {
                if (index >= 0)
                    rows.Children.RemoveAt(index);
                rows.Children.Insert(Math.Min(at, rows.Children.Count), view);
            }
            at++;
            view.Update(row, asked);
        }
    }

    /// <summary>
    /// With the demo as the only source: a line that says so, and Add a source beside it, which
    /// goes under it when the window is narrow.
    /// </summary>
    UIElement DemoLine()
    {
        var line = new Kaeri.Windows.Player.WrapRow { Gap = 12, Margin = new Thickness(0, 20, 0, 0) };
        line.Children.Add(new TextBlock { Text = T.HomeDemoOnlyBooks, Style = Ui.Res<Style>("HazeCaptionStyle"), VerticalAlignment = VerticalAlignment.Center, TextWrapping = TextWrapping.Wrap });
        var add = Ui.Label(T.SourcesAddTitle, (_, _) => ReadingParts.AddSource(ctx, XamlRoot));
        add.VerticalAlignment = VerticalAlignment.Center;
        line.Children.Add(add);
        return line;
    }

    /// <summary>Offline on the first run, with no row kept: where the rows will be, a line that says when, 12 on 16.</summary>
    static UIElement OfflineRows() => new Border
    {
        Margin = new Thickness(0, 40, 0, 0),
        Padding = new Thickness(0, 16, 0, 0),
        BorderThickness = new Thickness(0, 1, 0, 0),
        BorderBrush = LiveBrush.Of("FaintRuleBrush"),
        Child = new TextBlock { Text = T.ReadHomeOfflineRows, Style = Ui.Res<Style>("HazeCaptionStyle"), FontSize = 12, LineHeight = 16, TextWrapping = TextWrapping.Wrap },
    };

    /// <summary>The person's own sections still on their way: a rail of plates where their shelf will be.</summary>
    static UIElement Waiting() => new Border { Margin = new Thickness(0, HomeWording.ShelfGap, 0, 0), Child = ReadPlates.Row(7) };

    // This week

    /// <summary>The week by day: each day's series coming out, three at most, the rest behind a link to Updates.</summary>
    UIElement Week(ReadWeekDay[] days)
    {
        var section = new StackPanel { Margin = new Thickness(0, HomeWording.ShelfGap, 0, 0) };
        section.Children.Add(ReadingParts.ShelfHeading(T.HomeThisWeek, T.NavUpdates, () => ctx.Window.Go("updates")));
        const double gap = 12, least = 120;
        var grid = new Grid { ColumnSpacing = gap, MinWidth = days.Length * least + (days.Length - 1) * gap };
        for (var i = 0; i < days.Length; i++)
        {
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            var day = Day(days[i]);
            Grid.SetColumn(day, i);
            grid.Children.Add(day);
        }
        var strip = new ScrollViewer
        {
            Content = grid,
            HorizontalScrollMode = ScrollMode.Auto,
            HorizontalScrollBarVisibility = ScrollBarVisibility.Hidden,
            VerticalScrollMode = ScrollMode.Disabled,
            VerticalScrollBarVisibility = ScrollBarVisibility.Disabled,
            IsTabStop = false,
        };
        HomeShelf.ForwardWheel(strip);
        section.Children.Add(strip);
        return section;
    }

    const int DayShown = 3;

    FrameworkElement Day(ReadWeekDay day)
    {
        var column = new StackPanel { Spacing = 12 };
        var label = new TextBlock
        {
            Text = day.Today ? T.HomeToday : ReadingParts.Weekday(day.Day),
            FontSize = 12,
            LineHeight = 16,
            FontWeight = FontWeights.SemiBold,
            Foreground = LiveBrush.Of(day.Today ? "SilkBrush" : "HazeBrush"),
        };
        AutomationProperties.SetHeadingLevel(label, AutomationHeadingLevel.Level3);
        column.Children.Add(label);
        if (day.Entries.Length == 0)
            column.Children.Add(new TextBlock { Text = T.WeekNothingUsually, Style = Ui.Res<Style>("HazeCaptionStyle"), TextWrapping = TextWrapping.Wrap });
        foreach (var entry in day.Entries.Take(DayShown))
            column.Children.Add(Cell(entry));
        if (day.Entries.Length > DayShown)
        {
            var more = new HyperlinkButton { Content = T.HomeDayMore(day.Entries.Length - DayShown), Style = Ui.Res<Style>("SilkLinkStyle"), FontSize = 12, Padding = new Thickness(0) };
            more.Click += (_, _) => ctx.Window.Go("updates");
            column.Children.Add(more);
        }
        return column;
    }

    /// <summary>A series in its day: the cover, the time it came out (or that it usually does), the title and the chapter.</summary>
    Button Cell(ReadWeekEntry entry)
    {
        var tile = entry.Tile;
        var row = new Grid { ColumnSpacing = 8 };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        var cover = ReadingParts.Cover(ctx, tile.Cover, 36);
        cover.VerticalAlignment = VerticalAlignment.Top;
        row.Children.Add(cover);
        var words = new StackPanel { Spacing = 2 };
        var when = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 4 };
        string time;
        if (entry.OutAt is { } at)
        {
            when.Children.Add(ReadingParts.WaterDot());
            time = ReadingParts.Clock(at);
            when.Children.Add(new TextBlock { Text = time, FontSize = 12, LineHeight = 16, FontWeight = FontWeights.SemiBold, Foreground = LiveBrush.Of("SilkBrush") });
        }
        else
        {
            time = T.WeekUsually;
            when.Children.Add(new TextBlock { Text = time, Style = Ui.Res<Style>("HazeCaptionStyle") });
        }
        words.Children.Add(when);
        words.Children.Add(new TextBlock
        {
            Text = tile.Title,
            Style = Ui.Res<Style>("ShowTitleStyle"),
            FontSize = 13,
            LineHeight = 17,
            MaxLines = 2,
            TextWrapping = TextWrapping.Wrap,
            TextTrimming = TextTrimming.CharacterEllipsis,
        });
        // The chapter that came out that day; for one only expected, the chapter the engine
        // expects, and nothing when it has no confident guess.
        var line = entry.Usually ? entry.Chapter?.Text() : entry.Chapter?.Text() ?? tile.Next?.Text();
        if (line is { Length: > 0 })
            words.Children.Add(new TextBlock { Text = line, Style = Ui.Res<Style>("HazeCaptionStyle"), TextWrapping = TextWrapping.NoWrap, TextTrimming = TextTrimming.CharacterEllipsis });
        Grid.SetColumn(words, 1);
        row.Children.Add(words);
        var button = new Button { Style = Ui.Res<Style>("HomeCardButtonStyle"), Content = row, HorizontalAlignment = HorizontalAlignment.Stretch };
        AutomationProperties.SetName(button, line is { Length: > 0 } ? T.ReadNameWeekTile(tile.Title, time, line) : T.ReadNameWeekTileTime(tile.Title, time));
        button.Click += (_, _) => ctx.Window.OpenSeries(tile.Work);
        button.ContextFlyout = ReadingParts.DetailsMenu(ctx, tile.Work);
        return button;
    }

    // The end

    /// <summary>
    /// The closing line, left over a faint rule, 12 on 16: that Home ends here with Updates and
    /// Search as links, or with no source yet, Add a source.
    /// </summary>
    UIElement End(ReadHomeEnd end)
    {
        var block = new StackPanel
        {
            Spacing = 8,
            Margin = new Thickness(0, 40, 0, 0),
            Padding = new Thickness(0, 16, 0, 0),
            BorderThickness = new Thickness(0, 1, 0, 0),
            BorderBrush = LiveBrush.Of("FaintRuleBrush"),
            HorizontalAlignment = HorizontalAlignment.Stretch,
        };
        block.Children.Add(new TextBlock
        {
            Text = end == ReadHomeEnd.NoSources ? T.HomeReadNoSources : T.HomeReadEnd,
            Style = Ui.Res<Style>("HazeCaptionStyle"),
            FontSize = 12,
            LineHeight = 16,
            TextWrapping = TextWrapping.Wrap,
            MaxWidth = 480,
            HorizontalAlignment = HorizontalAlignment.Left,
        });
        var links = new Kaeri.Windows.Player.WrapRow { Gap = 16 };
        if (end == ReadHomeEnd.NoSources)
            links.Children.Add(Link(T.SourcesAddTitle, () => ReadingParts.AddSource(ctx, XamlRoot)));
        else
        {
            links.Children.Add(Link(T.NavUpdates, () => ctx.Window.Go("updates")));
            links.Children.Add(Link(T.NavSearch, () => ctx.Window.Go("search")));
        }
        block.Children.Add(links);
        return block;
    }

    static HyperlinkButton Link(string words, Action act)
    {
        var link = new HyperlinkButton { Content = words, Style = Ui.Res<Style>("SilkLinkStyle"), FontSize = 12, Padding = new Thickness(0) };
        link.Click += (_, _) => act();
        return link;
    }
}

/// <summary>
/// Reading's spotlight: the label, the title in the show font, the facts, the synopsis, a line by
/// case (what is new with the water dot, where you stopped with the track, or that sources are
/// asked on opening), then the amber main action, Details or On Planning, and More. The art sits
/// beside the words while the column is wide, above them when it is not.
/// </summary>
sealed partial class ReadSpotlightView : Grid
{
    readonly Ctx ctx;
    readonly WorkTile tile;
    readonly ReadSpotlightWords words;
    readonly Button art;
    readonly Border frame;
    Border? cover;
    bool side = true;

    public ReadSpotlightView(Ctx ctx, WorkTile tile, Kaeri.Engine.Model.ReadSpotlight spot)
    {
        this.ctx = ctx;
        this.tile = tile;
        words = new ReadSpotlightWords(ctx, tile, spot);
        ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(40) });
        ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });

        frame = new Border { CornerRadius = new CornerRadius(16), Background = LiveBrush.Of("RaisedBrush") };
        if (spot.Banner is { } banner)
        {
            var image = new Image { Stretch = Stretch.UniformToFill };
            AutomationProperties.SetAccessibilityView(image, AccessibilityView.Raw);
            ArtFade.Watch(image);
            ctx.Art.Bind(banner, source => image.Source = source);
            frame.Child = image;
        }
        art = new Button { Style = Ui.Res<Style>("HomeCardButtonStyle"), Content = frame, CornerRadius = new CornerRadius(16) };
        AutomationProperties.SetName(art, T.ReadNameSpotlightArt(tile.Title));
        art.Click += (_, _) => ctx.Window.OpenSeries(tile.Work);
        Children.Add(words);
        Children.Add(art);
        Place(true);
        SizeChanged += (_, e) =>
        {
            var wide = e.NewSize.Width >= 720;
            if (wide != side)
                Place(wide);
            Size(e.NewSize.Width);
        };
    }

    void Place(bool wide)
    {
        side = wide;
        ColumnDefinitions[1].Width = new GridLength(wide ? 40 : 0);
        ColumnDefinitions[2].Width = wide ? GridLength.Auto : new GridLength(0);
        SetColumn(words, 0);
        SetRow(words, wide ? 0 : 1);
        SetColumn(art, wide ? 2 : 0);
        SetColumnSpan(art, wide ? 1 : 3);
        SetRow(art, 0);
        art.HorizontalAlignment = HorizontalAlignment.Left;
        words.VerticalAlignment = VerticalAlignment.Center;
        words.Margin = new Thickness(0, wide ? 0 : 16, 0, 0);
    }

    void Size(double width)
    {
        var w = side ? Math.Min(HomeWording.ArtMaxWidth, Math.Round(width * HomeWording.ArtShare)) : Math.Min(HomeWording.ArtMaxWidth, width);
        if (w <= 0)
            return;
        frame.Width = w;
        frame.Height = Math.Round(w * 9 / 16);
        // No banner: the book's cover, centred on the raised ground.
        if (frame.Child is null)
        {
            cover = ReadingParts.Cover(ctx, tile.Cover, 100);
            cover.HorizontalAlignment = HorizontalAlignment.Center;
            cover.VerticalAlignment = VerticalAlignment.Center;
            frame.Child = cover;
        }
        if (cover is not null)
        {
            cover.Height = Math.Round(frame.Height * 0.86);
            cover.Width = Math.Round(cover.Height / 1.5);
        }
    }
}

/// <summary>The spotlight's words and actions.</summary>
sealed partial class ReadSpotlightWords : StackPanel
{
    public ReadSpotlightWords(Ctx ctx, WorkTile tile, Kaeri.Engine.Model.ReadSpotlight spot)
    {
        MaxWidth = 440;
        HorizontalAlignment = HorizontalAlignment.Left;
        Children.Add(new TextBlock { Text = spot.Label.Text(), Style = Ui.Res<Style>("HazeBodyStyle"), FontSize = 14, LineHeight = 19, Margin = new Thickness(0, 0, 0, 8) });
        var title = new TextBlock
        {
            Text = tile.Title,
            FontFamily = Ui.Res<FontFamily>("ShowFont"),
            FontWeight = FontWeights.Bold,
            FontSize = 30,
            LineHeight = 38,
            MaxLines = 2,
            TextWrapping = TextWrapping.Wrap,
            TextTrimming = TextTrimming.CharacterEllipsis,
        };
        AutomationProperties.SetHeadingLevel(title, AutomationHeadingLevel.Level2);
        Children.Add(title);
        if (spot.Facts.Length > 0)
            Children.Add(new TextBlock
            {
                Text = spot.Facts.Joined().ToUpper(System.Globalization.CultureInfo.CurrentCulture),
                Style = Ui.Res<Style>("HazeCaptionStyle"),
                CharacterSpacing = 80,
                FontWeight = FontWeights.Medium,
                Margin = new Thickness(0, 10, 0, 0),
                TextTrimming = TextTrimming.CharacterEllipsis,
            });
        if (spot.Synopsis is { Length: > 0 } synopsis)
            Children.Add(new TextBlock
            {
                Text = System.Text.RegularExpressions.Regex.Replace(synopsis, @"\s*\n+\s*", " "),
                Style = Ui.Res<Style>("HazeBodyStyle"),
                FontSize = 14,
                LineHeight = 20,
                MaxLines = 3,
                TextWrapping = TextWrapping.Wrap,
                TextTrimming = TextTrimming.CharacterEllipsis,
                Margin = new Thickness(0, 12, 0, 0),
            });
        if (Line(tile, spot) is { } line)
            Children.Add(line);
        Children.Add(Actions(ctx, tile, spot));
    }

    static UIElement? Line(WorkTile tile, Kaeri.Engine.Model.ReadSpotlight spot)
    {
        var row = new StackPanel { Orientation = Orientation.Horizontal, Margin = new Thickness(0, 16, 0, 0) };
        var text = spot.Line?.Text();
        switch (spot.Case)
        {
            case ReadSpotlightCase.NewChapter when text is not null:
                row.Spacing = 8;
                row.Children.Add(ReadingParts.WaterDot());
                row.Children.Add(new TextBlock { Text = text, Style = Ui.Res<Style>("HazeCaptionStyle"), Foreground = LiveBrush.Of("SilkBrush"), TextTrimming = TextTrimming.CharacterEllipsis });
                return row;
            case ReadSpotlightCase.Continuing when text is not null:
                row.Spacing = 10;
                if (tile.Progress is { } progress)
                {
                    var track = ReadingParts.Track(200, progress, 4);
                    track.VerticalAlignment = VerticalAlignment.Center;
                    AutomationProperties.SetName(track, text);
                    row.Children.Add(track);
                }
                row.Children.Add(new TextBlock { Text = text, Style = Ui.Res<Style>("HazeCaptionStyle"), TextTrimming = TextTrimming.CharacterEllipsis, VerticalAlignment = VerticalAlignment.Center });
                return row;
            case ReadSpotlightCase.Popular:
                row.Children.Add(new TextBlock { Text = text ?? T.HomeReadCheckedOnOpen, Style = Ui.Res<Style>("HazeCaptionStyle"), TextTrimming = TextTrimming.CharacterEllipsis });
                return row;
            default:
                return null;
        }
    }

    /// <summary>The amber main action, Details or On Planning, and More with Remove from Continue.</summary>
    static UIElement Actions(Ctx ctx, WorkTile tile, Kaeri.Engine.Model.ReadSpotlight spot)
    {
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 10, Margin = new Thickness(0, 18, 0, 0) };
        if (ReadingParts.Action(spot.Action) is { } words)
        {
            var main = new Button { Style = Ui.Res<Style>("AccentButtonStyle"), MinHeight = 32, Content = Glyphed("", words) };
            AutomationProperties.SetName(main, words);
            main.Click += (_, _) => ReadingParts.Perform(ctx, spot.Action, tile.Work);
            row.Children.Add(main);
        }
        else if (!spot.Planned)
        {
            var add = new Button { Style = Ui.Res<Style>("AccentButtonStyle"), MinHeight = 32, Content = Glyphed("", T.LibraryAddPlanning) };
            AutomationProperties.SetName(add, T.LibraryAddPlanning);
            add.Click += (_, _) =>
            {
                add.IsEnabled = false;
                ctx.Known.Reading.SetStatus(tile.Work, LibrarySegmentRead.Planning);
            };
            row.Children.Add(add);
        }
        var second = new Button { MinHeight = 32, Content = spot.Planned ? Glyphed("", T.ReadingOnPlanning) : T.HomeDetails };
        AutomationProperties.SetName(second, spot.Planned ? T.ReadingOnPlanning : T.HomeDetails);
        second.Click += (_, _) => ctx.Window.OpenSeries(tile.Work);
        row.Children.Add(second);
        if (spot.Case is ReadSpotlightCase.NewChapter or ReadSpotlightCase.Continuing)
        {
            var more = new Button { MinHeight = 32, MinWidth = 32, Padding = new Thickness(0), Content = new FontIcon { Glyph = "", FontSize = 16 } };
            AutomationProperties.SetName(more, T.HomeMore);
            ToolTipService.SetToolTip(more, T.HomeMore);
            var menu = new MenuFlyout();
            menu.Items.Add(ReadingParts.Item(T.HomeRemoveContinue, () => ctx.Known.Reading.HideFromContinue(tile.Work)));
            more.Flyout = menu;
            row.Children.Add(more);
        }
        return row;
    }

    static StackPanel Glyphed(string glyph, string words) => new()
    {
        Orientation = Orientation.Horizontal,
        Spacing = 8,
        Children = { new FontIcon { Glyph = glyph, FontSize = 12 }, new TextBlock { Text = words } },
    };
}
