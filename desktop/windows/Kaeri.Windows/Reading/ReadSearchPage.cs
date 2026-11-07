using Medium = Kaeri.Engine.Core.Medium;
using Microsoft.UI.Xaml.Automation.Peers;
using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Navigation;

namespace Kaeri.Windows.Pages;

/// <summary>
/// Reading's Search. With nothing typed it browses: the filters' line over a cover grid, and a
/// sort menu. Typed words show rows. The filters are format, origin, status and genres, each
/// genre included or left out. The last row counts the matches in the other scope and switches
/// to it; under the results, a line when the demo is the only book source.
/// </summary>
public sealed partial class ReadSearchPage : Page
{
    /// <summary>A search another place asked before opening the page, such as Home's genre shelf.</summary>
    internal static ReadSearchQuery? Opening { get; set; }

    static ReadFilters? filters;
    static ReadFilters Filters => filters ??= KaeriFfiMethods.ReadFilters();

    readonly Ctx ctx = Ctx.Current;
    readonly DropDownButton format = new() { MinHeight = 32, MinWidth = 96 };
    readonly DropDownButton origin = new() { MinHeight = 32, MinWidth = 96 };
    readonly DropDownButton status = new() { MinHeight = 32, MinWidth = 96 };
    readonly DropDownButton genres = new() { MinHeight = 32, MinWidth = 96 };
    readonly DropDownButton sort = new() { MinHeight = 32 };
    readonly ScrollViewer scroll = new() { VerticalScrollBarVisibility = ScrollBarVisibility.Auto, IsTabStop = false };
    readonly StateHost state = new();
    readonly StackPanel results = new() { Spacing = 0 };
    readonly TextBlock browseLine = new() { Style = Ui.Res<Style>("HazeBodyStyle"), Margin = new Thickness(0, 0, 0, 12) };
    readonly ReadGrid grid = new();
    readonly StackPanel rows = new() { TabFocusNavigation = KeyboardNavigationMode.Local, XYFocusKeyboardNavigation = XYFocusKeyboardNavigationMode.Enabled };
    readonly ProgressRing more = new() { IsActive = true, Width = 20, Height = 20, Margin = new Thickness(0, 16, 0, 0), HorizontalAlignment = HorizontalAlignment.Left, Visibility = Visibility.Collapsed };
    readonly StackPanel other = new() { Orientation = Orientation.Horizontal, Spacing = 8, Margin = new Thickness(0, 20, 0, 0), Visibility = Visibility.Collapsed };
    readonly TextBlock otherWords = new() { Style = Ui.Res<Style>("HazeBodyStyle"), VerticalAlignment = VerticalAlignment.Center };
    readonly TextBlock demoLine = new() { Text = T.SearchDemoOnlyBooks, Style = Ui.Res<Style>("HazeBodyStyle"), Margin = new Thickness(0, 16, 0, 0), TextWrapping = TextWrapping.Wrap, Visibility = Visibility.Collapsed };
    readonly DispatcherTimer settle = new() { Interval = TimeSpan.FromMilliseconds(400) };
    AutoSuggestBox? box;
    string typed = "";
    int drawnCount = -1;
    ReadSearchQuery? drawnQuery;
    bool hearing;

    public ReadSearchPage()
    {
        Background = Ui.Res<Brush>("GroundBrush");
        NavigationCacheMode = NavigationCacheMode.Enabled;
        var root = new Grid { Padding = new Thickness(36, 20, 0, 0), RowSpacing = 12 };
        root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        root.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });

        var head = new StackPanel { Margin = new Thickness(0, 0, 36, 0) };
        var title = new TextBlock { Text = T.NavSearch, Style = Ui.Res<Style>("PageTitleStyle") };
        AutomationProperties.SetHeadingLevel(title, AutomationHeadingLevel.Level1);
        head.Children.Add(title);
        head.Children.Add(new OfflineLine());
        root.Children.Add(head);

        // The scope first, then the filters; the sort at the end while browsing.
        var bar = new Grid { ColumnSpacing = 8, Margin = new Thickness(0, 0, 36, 0) };
        bar.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        bar.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        bar.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        bar.Children.Add(new ScopeSwitch { Place = "search", Reading = true, Margin = new Thickness(0, 0, 12, 0) });
        var filterRow = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        foreach (var button in new[] { format, origin, status, genres })
            filterRow.Children.Add(button);
        var filterScroll = new ScrollViewer
        {
            Content = filterRow,
            HorizontalScrollMode = ScrollMode.Auto,
            HorizontalScrollBarVisibility = ScrollBarVisibility.Hidden,
            VerticalScrollMode = ScrollMode.Disabled,
            VerticalScrollBarVisibility = ScrollBarVisibility.Disabled,
            IsTabStop = false,
        };
        Grid.SetColumn(filterScroll, 1);
        bar.Children.Add(filterScroll);
        Grid.SetColumn(sort, 2);
        bar.Children.Add(sort);
        Grid.SetRow(bar, 1);
        root.Children.Add(bar);
        Menus();

        var otherLink = new HyperlinkButton { Content = T.SearchShowThem, Style = Ui.Res<Style>("SilkLinkStyle"), Padding = new Thickness(0), VerticalAlignment = VerticalAlignment.Center };
        otherLink.Click += (_, _) => ctx.Window.SetScope("search", false);
        other.Children.Add(otherWords);
        other.Children.Add(otherLink);
        results.Children.Add(browseLine);
        results.Children.Add(grid);
        results.Children.Add(rows);
        results.Children.Add(more);
        state.Children.Add(results);
        state.Placeholders = () => Browsing ? States.Covers(12) : States.Rows(6, 40, 60, rowHeight: 76, spacing: 0);
        var column = new StackPanel { Padding = new Thickness(0, 4, 36, 48) };
        column.Children.Add(state);
        column.Children.Add(other);
        column.Children.Add(demoLine);
        scroll.Content = column;
        scroll.ViewChanged += (_, _) => Near();
        Grid.SetRow(scroll, 2);
        root.Children.Add(scroll);
        Content = root;

        settle.Tick += (_, _) =>
        {
            settle.Stop();
            Run(Query with { Text = typed });
        };
        Loaded += (_, _) =>
        {
            box ??= (XamlRoot?.Content as FrameworkElement)?.FindName("SearchBox") as AutoSuggestBox;
            if (box is not null)
            {
                box.TextChanged -= OnBoxText;
                box.TextChanged += OnBoxText;
            }
            if (!hearing)
            {
                ctx.Known.Reading.Changed += Heard;
                ctx.Known.Medium.Changed += Other;
            }
            hearing = true;
        };
        Unloaded += (_, _) =>
        {
            settle.Stop();
            if (box is not null)
                box.TextChanged -= OnBoxText;
            ctx.Known.Reading.Changed -= Heard;
            ctx.Known.Medium.Changed -= Other;
            hearing = false;
        };
    }

    ReadSearchQuery Query => ctx.Known.Reading.Search.Query;

    bool Browsing => Query.Text.Trim().Length == 0;

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        ctx.Hear(this, batch =>
        {
            if (batch.Any(x => x is Event.Demo))
                Demo();
        });
        var search = ctx.Known.Reading.Search;
        if (Opening is { } asked)
        {
            // Asked already by the place that opened the page.
            Opening = null;
            typed = asked.Text;
        }
        else if (e.Parameter is string text && text.Trim() != search.Query.Text)
        {
            typed = text.Trim();
            Run(search.Query with { Text = typed });
        }
        else if (search.Tiles.Count == 0 && !search.Loading)
        {
            typed = search.Query.Text;
            Run(search.Query);
        }
        else
            typed = search.Query.Text;
        drawnCount = -1;
        Menus();
        Show();
    }

    /// <summary>The window's box typed in while the page is up: the results follow it once the typing stops.</summary>
    void OnBoxText(AutoSuggestBox sender, AutoSuggestBoxTextChangedEventArgs args)
    {
        if (args.Reason != AutoSuggestionBoxTextChangeReason.UserInput || Frame?.Content != this)
            return;
        var text = sender.Text.Trim();
        if (text == typed)
            return;
        typed = text;
        settle.Stop();
        settle.Start();
    }

    void Run(ReadSearchQuery query)
    {
        var showing = ctx.Known.Reading.Search.Tiles.Count > 0;
        ctx.Known.Reading.RunSearch(query);
        drawnCount = -1;
        state.WaitKeeping(showing);
        Menus();
    }

    void Heard(ReadingPart part)
    {
        if (part is ReadingPart.Search or ReadingPart.Failure)
            Show();
    }

    void Show()
    {
        var search = ctx.Known.Reading.Search;
        Other();
        Demo();
        more.Visibility = search.HasMore && search.Tiles.Count > 0 ? Visibility.Visible : Visibility.Collapsed;
        if (search.Failure is { } failure && search.Tiles.Count == 0)
        {
            state.Empty(ReadingParts.PlainEmpty(T.ReadSearchFailed, failure.Words(), T.ActionTryAgain, () => Run(search.Query), error: true));
            return;
        }
        if (search.Tiles.Count == 0 && (search.Loading || search.Page == 0 && search.HasMore))
        {
            state.Wait();
            return;
        }
        if (search.Tiles.Count == 0)
        {
            var any = AnyFilter(search.Query);
            state.Empty(ReadingParts.PlainEmpty(
                Browsing ? T.ReadSearchNothingBrowse : T.ReadSearchNothing(search.Query.Text),
                any ? T.ReadSearchNothingFiltersLine : T.ReadSearchNothingLine,
                any ? T.ReadSearchClearFilters : null,
                any ? () => Run(ReadingStore.EmptyQuery with { Text = search.Query.Text }) : null));
            drawnCount = 0;
            return;
        }
        if (drawnCount == search.Tiles.Count && drawnQuery is { } q && ReadingStore.SameQuery(q, search.Query))
        {
            state.Done();
            return;
        }
        var fresh = drawnQuery is null || !ReadingStore.SameQuery(drawnQuery, search.Query) || drawnCount < 0;
        drawnCount = search.Tiles.Count;
        drawnQuery = search.Query;
        var mark = fresh ? null : Ui.MarkFocus(results);
        if (Browsing)
        {
            browseLine.Text = BrowseLine(search.Query, search.Total);
            browseLine.Visibility = Visibility.Visible;
            rows.Visibility = Visibility.Collapsed;
            rows.Children.Clear();
            grid.Visibility = Visibility.Visible;
            grid.Show(ctx, [.. search.Tiles]);
        }
        else
        {
            browseLine.Visibility = Visibility.Collapsed;
            grid.Visibility = Visibility.Collapsed;
            rows.Visibility = Visibility.Visible;
            rows.Children.Clear();
            foreach (var tile in search.Tiles)
            {
                rows.Children.Add(Row(tile));
                rows.Children.Add(new Border { Height = 1, Background = Ui.Res<Brush>("FaintRuleBrush") });
            }
        }
        state.Done();
        if (!fresh)
            Ui.RestoreFocus(results, mark);
        // Laid out first, so a short page that leaves the end in sight asks for more at once.
        DispatcherQueue.TryEnqueue(Microsoft.UI.Dispatching.DispatcherQueuePriority.Low, Near);
    }

    /// <summary>The next page, when the end of the results is within a screen of sight.</summary>
    void Near()
    {
        var search = ctx.Known.Reading.Search;
        if (!search.HasMore || search.Loading || search.Tiles.Count == 0)
            return;
        if (scroll.VerticalOffset + scroll.ViewportHeight < scroll.ExtentHeight - scroll.ViewportHeight)
            return;
        ctx.Known.Reading.SearchMore();
    }

    /// <summary>"12 in Shows": the matches in the other scope, and the switch to them, with both media on.</summary>
    void Other()
    {
        if (ctx.Known.Medium.Both && ctx.Known.Reading.Search.OtherScope is { } n && n > 0)
        {
            otherWords.Text = T.SearchOtherScope(n, T.LibraryScopeShows);
            other.Visibility = Visibility.Visible;
        }
        else
            other.Visibility = Visibility.Collapsed;
    }

    void Demo() => demoLine.Visibility = ctx.Known.Demo.OnlyBooks ? Visibility.Visible : Visibility.Collapsed;

    /// <summary>A result row: the cover, the title, and what comes next or its facts.</summary>
    Button Row(WorkTile tile)
    {
        var grid = new Grid { ColumnSpacing = 12, Padding = new Thickness(0, 8, 0, 8) };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.Children.Add(ReadingParts.Cover(ctx, tile.Cover, 40));
        var words = new StackPanel { Spacing = 2, VerticalAlignment = VerticalAlignment.Center };
        words.Children.Add(new TextBlock { Text = tile.Title, Style = Ui.Res<Style>("ShowTitleStyle"), FontSize = 15, TextTrimming = TextTrimming.CharacterEllipsis, TextWrapping = TextWrapping.NoWrap });
        var line = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
        if (tile.NewChapter)
            line.Children.Add(ReadingParts.WaterDot());
        line.Children.Add(new TextBlock { Text = ReadingParts.Line(tile), Style = Ui.Res<Style>("HazeCaptionStyle"), TextTrimming = TextTrimming.CharacterEllipsis });
        words.Children.Add(line);
        if (tile.FormatLabel is { } label && tile.Next is not null)
            words.Children.Add(new TextBlock { Text = label, Style = Ui.Res<Style>("HazeCaptionStyle") });
        Grid.SetColumn(words, 1);
        grid.Children.Add(words);
        var button = new Button
        {
            Style = Ui.Res<Style>("HomeCardButtonStyle"),
            Content = grid,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Stretch,
            MaxWidth = 772,
        };
        AutomationProperties.SetName(button, ReadingParts.Joined(tile.Title, ReadingParts.Line(tile)));
        button.Click += (_, _) => ctx.Window.OpenSeries(tile.Work);
        button.ContextFlyout = ReadingParts.TileMenu(ctx, tile.Work);
        return button;
    }

    // Filters

    static bool AnyFilter(ReadSearchQuery q) =>
        (q.Formats?.Length ?? 0) > 0 || (q.Origins?.Length ?? 0) > 0 || q.Status is not null
        || (q.Genres?.Length ?? 0) > 0 || (q.ExcludeGenres?.Length ?? 0) > 0;

    static string[] Labels(string[]? keys, ReadFilterChoice[] choices) =>
        [.. (keys ?? []).Select(k => choices.FirstOrDefault(c => c.Key == k)?.Label.Text() ?? k)];

    /// <summary>The first chosen, or "Manga +2".</summary>
    static string? Value(string[]? keys, ReadFilterChoice[] choices)
    {
        var words = Labels(keys, choices);
        return words.Length switch
        {
            0 => null,
            1 => words[0],
            _ => T.ReadFilterMany(words[0], words.Length - 1),
        };
    }

    /// <summary>"Manga · Japan · Releasing · 1,240 titles": what the browse shows.</summary>
    static string BrowseLine(ReadSearchQuery q, uint? total)
    {
        var f = Filters;
        var parts = Labels(q.Formats, f.Formats).Concat(Labels(q.Origins, f.Origins)).ToList();
        if (q.Status is { } s)
            parts.AddRange(Labels([s], f.Statuses));
        parts.AddRange(q.Genres ?? []);
        parts.AddRange((q.ExcludeGenres ?? []).Select(T.ReadFilterNotGenre));
        var what = parts.Count == 0 ? T.ReadFilterAny : string.Join(", ", parts);
        return total is { } n ? ReadingParts.Joined(what, T.ReadSearchTotal(n)) : T.ReadSearchBrowseLine(what);
    }

    static string SortWords(string? key) => key switch
    {
        "TRENDING_DESC" => T.ReadSortTrending,
        "SCORE_DESC" => T.ReadSortScore,
        "START_DATE_DESC" => T.ReadSortNewest,
        "TITLE_ROMAJI" => T.LibrarySortTitle,
        _ => T.ReadSortPopular,
    };

    /// <summary>Each filter's menu, built from the query in force: its name until set, then what it has, in amber.</summary>
    void Menus()
    {
        var f = Filters;
        var q = Query;
        format.Flyout = Choices(f.Formats, q.Formats ?? [], key => Toggle(q, key, formats: true));
        Say(format, T.ReadFilterFormat, Value(q.Formats, f.Formats));
        origin.Flyout = Choices(f.Origins, q.Origins ?? [], key => Toggle(q, key, formats: false));
        Say(origin, T.ReadFilterOrigin, Value(q.Origins, f.Origins));

        var statuses = new MenuFlyout { Placement = FlyoutPlacementMode.BottomEdgeAlignedLeft };
        var any = new RadioMenuFlyoutItem { Text = T.ReadFilterAny, GroupName = "status", IsChecked = q.Status is null };
        any.Click += (_, _) => Run(Query with { Status = null });
        statuses.Items.Add(any);
        foreach (var choice in f.Statuses)
        {
            var item = new RadioMenuFlyoutItem { Text = choice.Label.Text(), GroupName = "status", IsChecked = q.Status == choice.Key };
            item.Click += (_, _) => Run(Query with { Status = choice.Key });
            statuses.Items.Add(item);
        }
        status.Flyout = statuses;
        Say(status, T.ReadFilterStatus, q.Status is { } s ? Value([s], f.Statuses) : null);

        // Each genre included or left out, never both.
        var genreMenu = new MenuFlyout { Placement = FlyoutPlacementMode.BottomEdgeAlignedLeft };
        foreach (var genre in f.Genres)
        {
            var included = (q.Genres ?? []).Contains(genre);
            var leftOut = (q.ExcludeGenres ?? []).Contains(genre);
            var sub = new MenuFlyoutSubItem { Text = leftOut ? T.ReadFilterNotGenre(genre) : genre };
            if (included || leftOut)
                sub.Icon = new FontIcon { Glyph = included ? "" : "" };
            var include = new ToggleMenuFlyoutItem { Text = T.ReadFilterIncluded, IsChecked = included };
            include.Click += (_, _) => Genre(genre, include: !included, exclude: false);
            var exclude = new ToggleMenuFlyoutItem { Text = T.ReadFilterLeftOut, IsChecked = leftOut };
            exclude.Click += (_, _) => Genre(genre, include: false, exclude: !leftOut);
            sub.Items.Add(include);
            sub.Items.Add(exclude);
            genreMenu.Items.Add(sub);
        }
        genres.Flyout = genreMenu;
        var count = (q.Genres?.Length ?? 0) + (q.ExcludeGenres?.Length ?? 0);
        Say(genres, T.ReadFilterGenres, count switch
        {
            0 => null,
            1 => q.Genres is [var only] ? only : T.ReadFilterNotGenre(q.ExcludeGenres![0]),
            _ => T.ReadFilterGenresCount(count),
        });

        // The sort, while browsing: typed words come in the order that matches them best.
        var sorts = new MenuFlyout { Placement = FlyoutPlacementMode.BottomEdgeAlignedRight };
        var current = q.Sort ?? f.Sorts.FirstOrDefault();
        foreach (var key in f.Sorts)
        {
            var item = new RadioMenuFlyoutItem { Text = SortWords(key), GroupName = "sort", IsChecked = key == current };
            item.Click += (_, _) => Run(Query with { Sort = key });
            sorts.Items.Add(item);
        }
        sort.Flyout = sorts;
        sort.Content = SortWords(current);
        AutomationProperties.SetName(sort, T.LibrarySortButton(SortWords(current)));
        sort.Visibility = Browsing ? Visibility.Visible : Visibility.Collapsed;
    }

    static MenuFlyout Choices(ReadFilterChoice[] choices, string[] chosen, Action<string> toggle)
    {
        var menu = new MenuFlyout { Placement = FlyoutPlacementMode.BottomEdgeAlignedLeft };
        foreach (var choice in choices)
        {
            var item = new ToggleMenuFlyoutItem { Text = choice.Label.Text(), IsChecked = chosen.Contains(choice.Key) };
            item.Click += (_, _) => toggle(choice.Key);
            menu.Items.Add(item);
        }
        return menu;
    }

    void Toggle(ReadSearchQuery q, string key, bool formats)
    {
        var list = (formats ? q.Formats : q.Origins) ?? [];
        string[] next = list.Contains(key) ? [.. list.Where(k => k != key)] : [.. list, key];
        Run(formats ? Query with { Formats = next } : Query with { Origins = next });
    }

    void Genre(string genre, bool include, bool exclude)
    {
        var q = Query;
        var with = (q.Genres ?? []).Where(g => g != genre).ToList();
        var without = (q.ExcludeGenres ?? []).Where(g => g != genre).ToList();
        if (include)
            with.Add(genre);
        if (exclude)
            without.Add(genre);
        Run(q with { Genres = [.. with], ExcludeGenres = [.. without] });
    }

    static void Say(DropDownButton button, string name, string? value)
    {
        var text = value ?? name;
        if (button.Content as string != text)
            button.Content = text;
        AutomationProperties.SetName(button, value is null ? name : $"{name}, {value}");
        if (value is not null)
            button.Foreground = Ui.Res<Brush>("AccentTextFillColorTertiaryBrush");
        else
            button.ClearValue(Control.ForegroundProperty);
    }
}
