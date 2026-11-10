using Medium = Kaeri.Engine.Core.Medium;
using Microsoft.UI.Xaml.Automation.Peers;
using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Documents;
using Microsoft.UI.Xaml.Hosting;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Navigation;

namespace Kaeri.Windows.Pages;

using Pick = Kaeri.Engine.Model.Pick;

/// <summary>
/// Reading's Search, drawn as the shows' Search is and from the same parts: the title, the scope
/// and the row of filters in the shows' places and order (Genre, Format, Status, then Origin in
/// Season's place, Started in Year's and Your sources only in Available's once words are typed,
/// then the sort and Clear filters, with Filters and its panel at the end), and each set filter as
/// a chip that takes it off. With nothing typed, set or sorted it starts with Home's discovery
/// rows as rails, each with See all, which searches that row's query here and keeps its line. A
/// filter or a sort with nothing typed browses AniList in the shows' grid under its caption line,
/// more coming as the end nears. Typed words show AniList's covers in the grid (a result a source
/// has says so on its third line), then From your sources: one sideways row of covers per source,
/// each with its own placeholders, failure and Try again, and one line for the sources that found
/// nothing; More from AniList loads the next page under it. With both media on, a line under
/// AniList's grid counts the matches in Shows and switches to them. The start, the browse and the
/// words crossfade into each other; the title, the scope and the row stay where they are.
/// </summary>
public sealed partial class ReadSearchPage : Page
{
    /// <summary>A search another place asked before opening the page, such as a Home row's View all.</summary>
    internal static ReadSearchQuery? Opening { get; set; }

    /// <summary>
    /// The line a row's View all or See all gives its results, "Manga started in the last 12
    /// months", held while the search asks what the row asked.
    /// </summary>
    internal static (ReadSearchQuery Query, Msg Line)? Titled { get; set; }

    /// <summary>Ctrl+Shift+F from another page: the panel opens as Search comes up.</summary>
    internal static bool FiltersOnArrival;

    static ReadFilters? filters;
    static ReadFilters Filters => filters ??= KaeriFfiMethods.ReadFilters();

    /// <summary>The other filters while Your sources only is on: they hold, but AniList is not asked.</summary>
    const double Dimmed = 0.38;

    enum Mode { Start, Browse, Words }

    readonly Ctx ctx = Ctx.Current;
    // One line, cut short, so long words never push the filters and results down.
    readonly TextBlock title = new()
    {
        Text = T.NavSearch,
        Style = Ui.Res<Style>("PageTitleStyle"),
        MaxLines = 1,
        TextWrapping = TextWrapping.NoWrap,
        TextTrimming = TextTrimming.CharacterEllipsis,
    };
    readonly TextBlock demoLine = new() { Text = T.SearchDemoOnlyBooks, Style = Ui.Res<Style>("HazeBodyStyle"), TextWrapping = TextWrapping.Wrap, Visibility = Visibility.Collapsed };

    // The filter row and its menus, made again as each opens so they say what the query has now.
    readonly StackPanel genreList, formatList, originList;
    readonly MenuFlyout statusMenu = DropMenu(), startedMenu = DropMenu(), availableMenu = DropMenu(), sortMenu = DropMenu();
    readonly DropDownButton genres, format, status, origin, started, available, sort;
    readonly HyperlinkButton clear;
    readonly ToggleButton filtersButton;
    readonly InfoBadge filtersBadge;
    readonly Border panelHost = new() { Visibility = Visibility.Collapsed };
    readonly ReadFiltersPanel panel;
    bool panelOpen;
    // Each filter set, as a chip that takes it off.
    readonly Kaeri.Windows.Player.WrapRow chipRow = new() { Gap = 8, Visibility = Visibility.Collapsed };
    string[] chips = [];

    readonly ScrollViewer scroll = new() { VerticalScrollBarVisibility = ScrollBarVisibility.Auto, IsTabStop = false };
    // What the start, the browse and the words crossfade in.
    readonly StackPanel column = new() { Padding = new Thickness(0, 4, 0, 24), Spacing = 12 };

    // The start: Home's discovery rows as rails, with placeholders while Home loads.
    readonly StateHost startState = new() { Visibility = Visibility.Collapsed };
    readonly StackPanel startRows = new() { Spacing = 12 };
    readonly Dictionary<string, ReadStartRow> rowViews = [];
    DateTimeOffset startAsked = DateTimeOffset.Now;

    readonly StateHost state = new();
    // The results and the nothing anywhere message, one in place of the other.
    readonly StackPanel results = new(), body = new();
    readonly Border nothingHost = new() { Visibility = Visibility.Collapsed };
    // The caption line over AniList's grid: the filters while browsing, the count with words.
    readonly FadeLine line = new(() => new TextBlock
    {
        Style = Ui.Res<Style>("HazeCaptionStyle"),
        TextTrimming = TextTrimming.CharacterEllipsis,
        TextWrapping = TextWrapping.NoWrap,
    })
    { Margin = new Thickness(0, 0, 0, 12) };
    ReadGrid grid = Named(new ReadGrid(search: true));
    // AniList's later pages with words: under More from AniList.
    readonly ReadGrid later = Named(new ReadGrid(search: true) { Margin = new Thickness(0, SearchParts.LineGap, 0, 0), Visibility = Visibility.Collapsed });
    readonly CoverSlot anilistSlot;
    // AniList's part: the line, the grid in its slot, then the Shows line under it.
    readonly StackPanel anilist = new();
    readonly StackPanel sources = new() { Visibility = Visibility.Collapsed };
    readonly TextBlock sourcesHead = new() { Text = T.ReadSearchFromSources, Style = Ui.Res<Style>("SectionTitleStyle") };
    readonly StackPanel sourceRows = new() { XYFocusKeyboardNavigation = XYFocusKeyboardNavigationMode.Enabled };
    readonly Dictionary<string, SourceRowView> sourceViews = [];
    readonly FadeLine nothingWords = new(() => new TextBlock { Style = Ui.Res<Style>("HazeBodyStyle"), TextWrapping = TextWrapping.Wrap });
    readonly Border nothingLine = new() { Margin = new Thickness(0, 22, 0, 0), Padding = new Thickness(0, 12, 0, 0), BorderThickness = new Thickness(0, 1, 0, 0), Visibility = Visibility.Collapsed };
    readonly StackPanel moreRow = new() { Orientation = Orientation.Horizontal, Spacing = 12, Margin = new Thickness(0, 22, 0, 0), Visibility = Visibility.Collapsed };
    readonly Button moreAnilist = new() { Content = T.ReadSearchMoreAnilist, MinHeight = 32 };
    readonly ProgressRing moreRing = new() { Width = 16, Height = 16, MinWidth = 16, MinHeight = 16, IsActive = false, Visibility = Visibility.Collapsed, VerticalAlignment = VerticalAlignment.Center };
    readonly TextBlock shownWords = new() { Style = Ui.Res<Style>("HazeBodyStyle"), VerticalAlignment = VerticalAlignment.Center };
    // More to come while browsing: a row of plates in the cells' size under the grid, as the shows' Search has.
    readonly StackPanel morePlates = new() { Margin = new Thickness(0, SearchParts.LineGap, 0, 0), Visibility = Visibility.Collapsed };
    readonly Grid other = new() { MinHeight = 52, Margin = new Thickness(0, 22, 0, 0), BorderThickness = new Thickness(0, 1, 0, 0), Visibility = Visibility.Collapsed };
    readonly TextBlock otherWords = new() { Style = Ui.Res<Style>("HazeBodyStyle"), VerticalAlignment = VerticalAlignment.Center };
    readonly DispatcherTimer settle = new() { Interval = TimeSpan.FromMilliseconds(400) };
    AutoSuggestBox? box;
    string typed = "", anilistNote = "", announced = "", nothingKey = "";
    bool anilistShown = true, hearing;
    Mode? drawn;
    // When the words were last asked: a part's placeholders come 600 ms after it.
    DateTimeOffset ranAt = DateTimeOffset.Now;

    static MenuFlyout DropMenu() => new() { Placement = FlyoutPlacementMode.BottomEdgeAlignedLeft };

    /// <summary>AniList's grid as screen readers meet it: "Series from AniList", each cover's place in it.</summary>
    static ReadGrid Named(ReadGrid grid)
    {
        AutomationProperties.SetName(grid, T.ReadSearchTotalUnknown);
        return grid;
    }

    public ReadSearchPage()
    {
        Background = LiveBrush.Of("GroundBrush");
        NavigationCacheMode = NavigationCacheMode.Enabled;

        // The filter row, as the shows' Search has it.
        (var genreFlyout, genreList) = SearchParts.Boxes(200, scrolls: true);
        genres = SearchParts.Menu(T.FilterGenre, genreFlyout);
        genreFlyout.Opening += (_, _) => BuildGenres();
        (var formatFlyout, formatList) = SearchParts.Boxes(180);
        format = SearchParts.Menu(T.FilterFormat, formatFlyout);
        formatFlyout.Opening += (_, _) => BuildFormats();
        status = SearchParts.Menu(T.FilterStatus, statusMenu);
        statusMenu.Opening += (_, _) => BuildStatuses();
        (var originFlyout, originList) = SearchParts.Boxes(180);
        origin = SearchParts.Menu(T.FilterOrigin, originFlyout);
        originFlyout.Opening += (_, _) => BuildOrigins();
        started = SearchParts.Menu(T.ReadSearchStarted, startedMenu, minWidth: 88);
        startedMenu.Opening += (_, _) => BuildStarted();
        available = SearchParts.Menu(T.SearchAvailableName, availableMenu);
        available.Visibility = Visibility.Collapsed;
        availableMenu.Opening += (_, _) => BuildAvailable();
        sort = SearchParts.Menu(T.SearchSortName, sortMenu, minWidth: 88);
        sortMenu.Opening += (_, _) => BuildSorts();
        clear = SearchParts.Clear(ClearFilters);
        (filtersButton, filtersBadge) = SearchParts.FiltersToggle();
        filtersButton.Click += (_, _) => ShowPanel(!panelOpen);
        panel = new ReadFiltersPanel(Filters, () => Query, Run, () => ShowPanel(false));
        panelHost.Child = panel;
        ElementCompositionPreview.SetIsTranslationEnabled(panelHost, true);

        var root = new Grid();
        root.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        // Every filter, in a panel at the right edge that Filters and Ctrl+Shift+F open and Esc closes.
        root.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var page = new Grid { Padding = new Thickness(36, 20, 36, 0), RowSpacing = 12 };
        page.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        page.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        page.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });

        // The title, the offline line and the scope where the shows' Search has them, so switching
        // the scope moves none of them.
        var head = new StackPanel();
        AutomationProperties.SetHeadingLevel(title, AutomationHeadingLevel.Level1);
        head.Children.Add(title);
        head.Children.Add(new OfflineLine(reading: true));
        head.Children.Add(new ScopeSwitch { Place = "search", Reading = true, Margin = new Thickness(0, 4, 0, 0) });
        head.Children.Add(demoLine);
        page.Children.Add(head);

        // The filters scroll sideways where they don't fit; Filters stays at the end, outside the
        // scroll, so the button that closes the panel is never scrolled away.
        var bar = new Grid { ColumnSpacing = 8 };
        bar.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        bar.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        foreach (var part in new FrameworkElement[] { genres, format, status, origin, started, available, sort, clear })
            row.Children.Add(part);
        bar.Children.Add(new ScrollViewer
        {
            Content = row,
            HorizontalScrollMode = ScrollMode.Auto,
            HorizontalScrollBarVisibility = ScrollBarVisibility.Hidden,
            VerticalScrollMode = ScrollMode.Disabled,
            VerticalScrollBarVisibility = ScrollBarVisibility.Disabled,
        });
        Grid.SetColumn(filtersButton, 1);
        bar.Children.Add(filtersButton);
        Grid.SetRow(bar, 1);
        page.Children.Add(bar);

        // The start: Home's rows, with plates for two rails while Home loads.
        startState.Placeholders = StartPlates;
        startState.Children.Add(startRows);

        // AniList's part: the line, the grid, and the Shows line under them.
        line.MinHeight = 16;
        anilist.Children.Add(line);
        anilistSlot = new CoverSlot(ReadingParts.ListText ? ReadPlates.Grid() : (FrameworkElement)States.Covers(), grid);
        anilist.Children.Add(anilistSlot);
        other.BorderBrush = LiveBrush.Of("FaintRuleBrush");
        other.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        other.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var otherLink = new HyperlinkButton
        {
            Style = Ui.Res<Style>("SilkLinkStyle"),
            VerticalAlignment = VerticalAlignment.Center,
            Content = new StackPanel
            {
                Orientation = Orientation.Horizontal,
                Spacing = 4,
                Children = { new TextBlock { Text = T.SearchShowThem, FontWeight = FontWeights.SemiBold }, new FontIcon { Glyph = "", FontSize = 10 } },
            },
        };
        otherLink.Click += (_, _) => ctx.Window.SetScope("search", false);
        other.Children.Add(otherWords);
        Grid.SetColumn(otherLink, 1);
        other.Children.Add(otherLink);
        anilist.Children.Add(other);

        // From your sources: a row per source, then the line for those that found nothing.
        AutomationProperties.SetHeadingLevel(sourcesHead, AutomationHeadingLevel.Level2);
        sources.Children.Add(sourcesHead);
        sources.Children.Add(sourceRows);
        nothingLine.BorderBrush = LiveBrush.Of("FaintRuleBrush");
        nothingLine.Child = nothingWords;
        sources.Children.Add(nothingLine);

        // More from AniList: while words are typed, more comes when asked, not on the scroll. The
        // button keeps the focus and stays; a small ring beside it while the page comes.
        AutomationProperties.SetAccessibilityView(moreRing, AccessibilityView.Raw);
        moreAnilist.Click += (_, _) =>
        {
            if (ctx.Known.Reading.Search.Loading)
                return;
            ctx.Known.Reading.SearchMore();
            Show();
        };
        moreRow.Children.Add(moreAnilist);
        moreRow.Children.Add(moreRing);
        moreRow.Children.Add(shownWords);

        morePlates.Children.Add(new ScrollViewer
        {
            Content = States.Covers(8, rail: true),
            HorizontalScrollMode = ScrollMode.Disabled,
            HorizontalScrollBarVisibility = ScrollBarVisibility.Hidden,
            VerticalScrollMode = ScrollMode.Disabled,
            IsTabStop = false,
        });
        AutomationProperties.SetName(morePlates, T.SearchLoadingMore);

        body.Children.Add(anilist);
        body.Children.Add(sources);
        body.Children.Add(moreRow);
        body.Children.Add(later);
        body.Children.Add(morePlates);
        results.Children.Add(body);
        results.Children.Add(nothingHost);
        state.Children.Add(results);
        column.Children.Add(chipRow);
        column.Children.Add(startState);
        column.Children.Add(state);
        // The column sits in a grid of its own, so a crossfade's picture lies over it alone.
        scroll.Content = new Grid { Children = { column } };
        SmoothWheel.SetOn(scroll, true);
        scroll.ViewChanged += (_, _) => Near();
        Grid.SetRow(scroll, 2);
        page.Children.Add(scroll);
        root.Children.Add(page);
        Grid.SetColumn(panelHost, 1);
        root.Children.Add(panelHost);
        Content = root;

        // Esc closes the open panel; with it shut, Esc is left to whoever had it.
        var escape = new KeyboardAccelerator { Key = global::Windows.System.VirtualKey.Escape };
        escape.Invoked += (_, args) =>
        {
            if (!panelOpen)
                return;
            args.Handled = true;
            ShowPanel(false);
        };
        KeyboardAccelerators.Add(escape);
        KeyboardAcceleratorPlacementMode = KeyboardAcceleratorPlacementMode.Hidden;

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
            // What came while the page was being built, before it heard.
            Show();
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

    /// <summary>Nothing typed, set or sorted: the start, with Home's rows.</summary>
    static bool IsStart(ReadSearchQuery q) => q.Text.Trim().Length == 0 && !AnyFilter(q) && q.Sort is null;

    Mode Now => !Browsing ? Mode.Words : IsStart(Query) ? Mode.Start : Mode.Browse;

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
            // Asked already by the place that opened the page, a row's View all.
            Opening = null;
            typed = asked.Text;
            ranAt = DateTimeOffset.Now;
            Fresh();
        }
        else if (e.Parameter is string text && text.Trim() != search.Query.Text)
        {
            typed = text.Trim();
            Fresh();
            Run(search.Query with { Text = typed });
        }
        else if (!IsStart(search.Query) && search.Tiles.Count == 0 && !search.Loading && search.Sources is null)
        {
            typed = search.Query.Text;
            Run(search.Query);
        }
        else
            typed = search.Query.Text;
        startAsked = DateTimeOffset.Now;
        Show();
        if (FiltersOnArrival)
        {
            FiltersOnArrival = false;
            ShowPanel(true);
        }
    }

    /// <summary>
    /// A new search as the page comes in: what an earlier search left is gone at once, under the
    /// page's own crossfade, and the parts wait for the new answers.
    /// </summary>
    void Fresh()
    {
        drawn = null;
        nothingKey = "";
        anilistNote = "";
        nothingHost.Child = null;
        nothingHost.Visibility = Visibility.Collapsed;
        body.Visibility = Visibility.Visible;
        Empty();
    }

    /// <summary>The results' parts empty at once, their room kept: nothing an earlier search left can show.</summary>
    void Empty()
    {
        line.Clear();
        anilistSlot.Clear();
        later.Visibility = Visibility.Collapsed;
        moreRow.Visibility = Visibility.Collapsed;
        sources.Visibility = Visibility.Collapsed;
        sourceRows.Children.Clear();
        sourceViews.Clear();
    }

    /// <summary>AniList's later pages go while new words are asked.</summary>
    void HideLater()
    {
        if (later.Visibility == Visibility.Collapsed)
            return;
        Motion.FadeOutThen(later, () =>
        {
            if (later.Opacity > 0.01)
                return;
            later.Visibility = Visibility.Collapsed;
            later.Opacity = 1;
        });
    }

    /// <summary>Enter in the window's box while the page is up: the words search here, with no new step for Back.</summary>
    internal void Take(string words)
    {
        settle.Stop();
        typed = words.Trim();
        if (typed != Query.Text.Trim())
            Run(Query with { Text = typed });
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
        ctx.Known.Reading.RunSearch(query);
        ranAt = DateTimeOffset.Now;
        Show();
    }

    void Heard(ReadingPart part)
    {
        if (part is ReadingPart.Search or ReadingPart.Failure)
            Show();
        else if (part is ReadingPart.Home && drawn == Mode.Start)
            ShowStart();
    }

    /// <summary>
    /// Draws the search as it stands, the title and the filter row with it. Between the start, the
    /// browse and the words the content crossfades; the title, the scope and the row stay put.
    /// </summary>
    void Show()
    {
        Other();
        Demo();
        var mode = Now;
        title.Text = mode switch
        {
            Mode.Start => T.NavSearch,
            Mode.Browse => T.SearchBrowse,
            _ => T.SearchResultsFor(Query.Text.Trim()),
        };
        ShowFilters();
        if (drawn is { } before && before != mode)
        {
            drawn = mode;
            Crossfade.Swap(column, () =>
            {
                // Out of the start, the results come empty, never with what an earlier search left.
                if (before == Mode.Start)
                    Fresh();
                Draw();
            });
            return;
        }
        Draw();
    }

    void Draw()
    {
        var mode = Now;
        drawn = mode;
        var start = mode == Mode.Start;
        startState.Visibility = start ? Visibility.Visible : Visibility.Collapsed;
        state.Visibility = start ? Visibility.Collapsed : Visibility.Visible;
        var search = ctx.Known.Reading.Search;
        if (start)
            ShowStart();
        else if (mode == Mode.Browse)
            ShowBrowse(search);
        else
            ShowWords(search);
    }

    // The start

    /// <summary>
    /// Home's discovery rows as the shows' rails: each heading in the section style with See all at
    /// its end, the covers in the shows' cells. Placeholders while Home loads, and the shows'
    /// failure with Try again when it could not.
    /// </summary>
    void ShowStart()
    {
        var reading = ctx.Known.Reading;
        if (reading.Home is null && !reading.HomeLoading && !reading.Failures.ContainsKey(ReadingAbout.Home))
            reading.LoadHome();
        ReadHomeRow[] rows = [.. (reading.Home?.Rows ?? []).Where(r => (r.Tiles?.Length ?? 0) > 0 || r.Pending)];
        StartRows(rows);
        if (rows.Length > 0)
            startState.Done();
        else if (reading.Failures.TryGetValue(ReadingAbout.Home, out var failure))
            startState.Failed(failure.Words(), () => reading.LoadHome(refresh: true));
        else if (reading.Home is null || reading.HomeLoading)
            startState.Wait();
        else
            startState.Done();
    }

    /// <summary>The rows in Home's order; a row that stays keeps its view and changes in place.</summary>
    void StartRows(ReadHomeRow[] rows)
    {
        var ids = rows.Select(r => r.Id).ToList();
        if (!ids.SequenceEqual(startRows.Children.OfType<ReadStartRow>().Select(v => v.Id)))
        {
            startRows.Children.Clear();
            foreach (var row in rows)
            {
                if (!rowViews.TryGetValue(row.Id, out var view))
                    rowViews[row.Id] = view = new ReadStartRow(ctx, row, SeeAll);
                startRows.Children.Add(view);
            }
            foreach (var gone in rowViews.Keys.Except(ids).ToList())
                rowViews.Remove(gone);
        }
        foreach (var row in rows)
            rowViews[row.Id].Update(row, startAsked);
    }

    /// <summary>See all: the row's own query searched here, its line over the results.</summary>
    void SeeAll(ReadHomeRow row)
    {
        if (row.ViewAll is not { } query)
            return;
        Titled = row.ViewAllTitle is { } words ? (query, words) : null;
        typed = query.Text;
        Run(query);
    }

    /// <summary>Plates for two rails, where Home's rows will be.</summary>
    static UIElement StartPlates()
    {
        var plates = new StackPanel { Spacing = 12 };
        for (var i = 0; i < 2; i++)
        {
            var heading = States.Plate(160, 14, 3);
            heading.HorizontalAlignment = HorizontalAlignment.Left;
            heading.Margin = new Thickness(0, 7, 0, 7);
            plates.Children.Add(heading);
            plates.Children.Add(States.Covers(8, rail: true));
        }
        AutomationProperties.SetAccessibilityView(plates, AccessibilityView.Raw);
        return plates;
    }

    // The results

    /// <summary>A filter or a sort with nothing typed: AniList's covers by the filters, more as the end nears.</summary>
    void ShowBrowse(ReadSearchState search)
    {
        Nothing(null);
        sources.Visibility = Visibility.Collapsed;
        sourceRows.Children.Clear();
        sourceViews.Clear();
        moreRow.Visibility = Visibility.Collapsed;
        later.Visibility = Visibility.Collapsed;
        AnilistShown(true, animate: false);
        morePlates.Visibility = search.HasMore && search.Tiles.Count > 0 ? Visibility.Visible : Visibility.Collapsed;
        if (search.Failure is { } failure && search.Tiles.Count == 0)
        {
            state.Empty(States.Unavailable(T.ReadSearchFailed, failure.Words(), T.ActionTryAgain, () => Run(search.Query), error: true));
            return;
        }
        if (search.Tiles.Count == 0 && (search.Loading || search.Page == 0 && search.HasMore))
        {
            // What shows stays 600 ms, then the grid's placeholders; the line keeps its words.
            state.Done();
            anilistNote = "";
            anilistSlot.Ask(ranAt);
            return;
        }
        if (search.Tiles.Count == 0)
        {
            var any = AnyFilter(search.Query);
            state.Empty(States.Unavailable(
                T.ReadSearchNothingBrowse,
                any ? T.ReadSearchNothingFiltersLine : T.ReadSearchNothingLine,
                any ? T.SearchClearFilters : null,
                any ? ClearFilters : null));
            return;
        }
        line.Text = BrowseLine(search.Query, search.Total);
        Grids(search, null);
        state.Done();
        // Laid out first, so a short page that leaves the end in sight asks for more at once.
        DispatcherQueue.TryEnqueue(Microsoft.UI.Dispatching.DispatcherQueuePriority.Low, Near);
    }

    /// <summary>
    /// Words typed: AniList's part and the sources' rows, each with its own placeholders and
    /// failure, so one slow source holds up nothing else. Nothing found anywhere takes the
    /// results' place in one crossfade, and gives it back the same way.
    /// </summary>
    void ShowWords(ReadSearchState search)
    {
        var q = search.Query;
        var words = q.Text.Trim();
        var found = search.Sources;
        morePlates.Visibility = Visibility.Collapsed;
        state.Done();
        var mine = search.TilesFor == words && search.Tiles.Count > 0;
        var failed = search.Failure is not null && !mine || found?.Anilist == ReadAnilistState.Failed;
        var asking = !failed && !mine && search.Loading;
        var nothing = !failed && !asking && search.Tiles.Count == 0;
        var groups = found?.Groups ?? [];
        var sourcesDone = found is { Done: true };
        var key = $"{words}\n{q.SourcesOnly}\n{AnyFilter(q)}";

        if (!q.SourcesOnly && nothing && (groups.Length == 0 || sourcesDone && ReadDiscovery.SourcesFoundNothing(found!)))
        {
            Nothing("anywhere\n" + key + groups.Length, () => NothingAnywhere(q, groups.Length > 0));
            Announce(search);
            return;
        }
        if (q.SourcesOnly && found is not null && (groups.Length == 0 || sourcesDone && ReadDiscovery.SourcesFoundNothing(found)))
        {
            Nothing("sources\n" + key, SourcesOnlyNothing);
            return;
        }
        Nothing(null);

        AnilistShown(!q.SourcesOnly, animate: true);
        if (!q.SourcesOnly)
        {
            if (!mine)
                HideLater();
            if (failed)
            {
                line.Text = "";
                AnilistNote("failed", () => CoverSlot.FailureBlock(T.ReadSearchAnilistFailed, T.ReadSearchAnilistFailedLine, () => Run(Query)));
            }
            else if (asking)
            {
                anilistNote = "";
                anilistSlot.Ask(ranAt);
            }
            else if (nothing)
            {
                line.Text = "";
                AnilistNote("nothing", () => AnilistNothing(q));
            }
            else
            {
                anilistNote = "";
                line.Text = ReadDiscovery.Total(search.Total);
                Grids(search, found);
            }
        }

        Sources(search, found, groups, first: q.SourcesOnly);
        var wantMore = !q.SourcesOnly && mine && (search.HasMore || search.Loading);
        moreRow.Visibility = wantMore ? Visibility.Visible : Visibility.Collapsed;
        var loadingMore = mine && search.Loading;
        moreRing.IsActive = loadingMore;
        moreRing.Visibility = loadingMore ? Visibility.Visible : Visibility.Collapsed;
        moreAnilist.Visibility = search.HasMore || loadingMore ? Visibility.Visible : Visibility.Collapsed;
        shownWords.Text = ReadDiscovery.Shown(search.Tiles.Count, search.Total);
        Announce(search);
    }

    /// <summary>
    /// AniList's grids: the first page in the slot, and with words the later pages under More
    /// from AniList. Results that only grow are added in place; other results come as a fresh
    /// grid over the old one, with the focus kept on the same cover.
    /// </summary>
    void Grids(ReadSearchState search, ReadSourceSearch? found)
    {
        var browsing = Browsing;
        var tiles = search.Tiles;
        var first = browsing || search.FirstPage <= 0 ? tiles.Count : Math.Min(search.FirstPage, tiles.Count);
        var joins = string.Join(",", (found?.Joins ?? []).Select(j => j.Work + ":" + (j.SourceNames ?? []).Length));
        var look = $"{browsing}\n{search.Query.Text.Trim()}\n{joins}";
        Func<string, string?>? placed = found is null ? null : work => ReadDiscovery.Placed(found, work);
        // With words, AniList's results say their format and origin; browsing keeps their facts.
        Func<WorkTile, string?>? caption = browsing ? null : (Func<WorkTile, string?>)ReadDiscovery.Caption;
        var head = tiles.Take(first).ToList();
        if (anilistSlot.Showing && !grid.Holds(head, look))
        {
            var fresh = Named(new ReadGrid(search: true));
            fresh.Show(ctx, head, placed, caption, look: look);
            grid = fresh;
            anilistSlot.Replace(fresh);
        }
        else
        {
            grid.Show(ctx, head, placed, caption, look: look);
            anilistSlot.Content();
        }
        var rest = tiles.Skip(first).ToList();
        later.Show(ctx, rest, placed, caption, look: look);
        later.Visibility = rest.Count > 0 ? Visibility.Visible : Visibility.Collapsed;
        if (rest.Count > 0)
            later.Opacity = 1;
    }

    /// <summary>The results, or a message in their place, in one crossfade; one message for another changes in place.</summary>
    void Nothing(string? key, Func<UIElement>? make = null)
    {
        var want = key ?? "";
        if (want == nothingKey)
            return;
        var was = nothingKey;
        nothingKey = want;
        void Apply()
        {
            nothingHost.Child = want.Length > 0 ? make!() : null;
            nothingHost.Visibility = want.Length > 0 ? Visibility.Visible : Visibility.Collapsed;
            body.Visibility = want.Length > 0 ? Visibility.Collapsed : Visibility.Visible;
        }
        if (was.Length > 0 && want.Length > 0)
            Apply();
        else
        {
            // Back from a message: the results come back empty, as their new answers are on
            // their way, never with what the words before the message found.
            if (want.Length == 0)
                Empty();
            Crossfade.Swap(results, Apply);
        }
    }

    /// <summary>A note in AniList's slot: its failure with Try again, or that it found nothing.</summary>
    void AnilistNote(string which, Func<FrameworkElement> make)
    {
        if (anilistNote != which + Query.Text)
        {
            anilistNote = which + Query.Text;
            anilistSlot.Fail(make());
        }
    }

    /// <summary>"AniList found nothing for these words." with the line to try other words, while the sources still answer.</summary>
    StackPanel AnilistNothing(ReadSearchQuery q)
    {
        var note = new StackPanel { Spacing = 2, Margin = new Thickness(0, 0, 0, 4) };
        note.Children.Add(new TextBlock { Text = T.ReadSearchAnilistNothing, FontSize = 14, LineHeight = 19, FontWeight = FontWeights.SemiBold, TextWrapping = TextWrapping.Wrap });
        note.Children.Add(new TextBlock { Text = AnyFilter(q) ? T.ReadSearchNothingFiltersLine : T.ReadSearchNothingLine, Style = Ui.Res<Style>("HazeCaptionStyle"), TextWrapping = TextWrapping.Wrap });
        AutomationProperties.SetLiveSetting(note, AutomationLiveSetting.Polite);
        return note;
    }

    /// <summary>Nothing from AniList and nothing from any source: the shows' empty state with the words, the line to try others, and the sources' line.</summary>
    UIElement NothingAnywhere(ReadSearchQuery q, bool sourcesAsked)
    {
        var any = AnyFilter(q);
        var lead = any ? T.ReadSearchNothingFiltersLine : T.ReadSearchNothingLine;
        var words = sourcesAsked ? $"{lead} {T.ReadSearchSourcesNothing}" : lead;
        return States.Unavailable(T.ReadSearchNothing(q.Text.Trim()), words, any ? T.SearchClearFilters : null, any ? ClearFilters : null);
    }

    /// <summary>Your sources only, and none found anything: "Nothing found in your sources" and the line to try other words.</summary>
    UIElement SourcesOnlyNothing() => States.Unavailable(T.ReadSearchSourcesOnlyNothing, T.ReadSearchNothingLine);

    /// <summary>Clear filters: every filter and the sort off, the words kept, as the shows' Clear filters does.</summary>
    void ClearFilters()
    {
        Titled = null;
        Run(ReadingStore.EmptyQuery with { Text = Query.Text });
    }

    /// <summary>Counts the sources' answers, so a section fading out stays when a later answer has rows.</summary>
    int sourcesTurn;
    bool headShown, sectionLeaving;

    /// <summary>
    /// From your sources: a row per source asked, answered or failed, in Your sources order, each
    /// keeping its place as its state changes; the sources that found nothing share one line. New
    /// words keep the rows that show until their answers come. A row that leaves fades out, then
    /// its gap closes. The heading fades in with the first row.
    /// </summary>
    void Sources(ReadSearchState search, ReadSourceSearch? found, ReadSourceGroup[] groups, bool first)
    {
        if (found is null && search.Loading && sourceViews.Count > 0)
        {
            foreach (var view in sourceViews.Values)
                view.Ask(ranAt);
            return;
        }
        if (found is null || groups.Length == 0)
        {
            sourcesTurn++;
            sectionLeaving = false;
            Motion.FadeTo(sources, 1, TimeSpan.Zero);
            sources.Visibility = Visibility.Collapsed;
            sourceRows.Children.Clear();
            sourceViews.Clear();
            return;
        }
        // The shows' gap over a section: 34 under AniList's part, 4 at the top.
        var gap = first ? 4 : 34;
        if (sources.Visibility == Visibility.Collapsed)
        {
            // A fade's held value outlasts a plain set, so the heading is hidden through Motion too.
            Motion.FadeTo(sourcesHead, 0, TimeSpan.Zero);
            headShown = false;
            sourcesHead.Margin = new Thickness(0, gap, 0, 0);
            sources.Visibility = Visibility.Visible;
        }
        else if (Math.Abs(sourcesHead.Margin.Top - gap) > 0.5)
        {
            // AniList going or coming with Your sources only: the heading's gap eases with it.
            Motion.Ease(sourcesHead, sourcesHead.Margin.Top, gap, v => sourcesHead.Margin = new Thickness(0, v, 0, 0));
        }
        var slotted = groups.Where(ReadDiscovery.Slotted).ToList();
        var ids = slotted.Select(g => g.SourceId).ToHashSet();
        foreach (var (id, view) in sourceViews.ToList())
        {
            if (ids.Contains(id))
                continue;
            sourceViews.Remove(id);
            view.Leave(() => sourceRows.Children.Remove(view));
        }
        var at = 0;
        for (var i = 0; i < slotted.Count; i++)
        {
            var group = slotted[i];
            if (!sourceViews.TryGetValue(group.SourceId, out var view))
            {
                view = new SourceRowView(ctx, group.SourceId);
                view.Changed += HeadIn;
                sourceViews[group.SourceId] = view;
            }
            // Rows on their way out keep their place until their gap has closed.
            while (at < sourceRows.Children.Count && sourceRows.Children[at] is SourceRowView { IsLeaving: true })
                at++;
            var index = sourceRows.Children.IndexOf(view);
            if (index != at)
            {
                if (index >= 0)
                    sourceRows.Children.RemoveAt(index);
                sourceRows.Children.Insert(Math.Min(at, sourceRows.Children.Count), view);
            }
            at++;
            view.First = i == 0;
            view.Update(group, ranAt);
        }
        var nothing = ReadDiscovery.NothingLine(found);
        nothingLine.Margin = new Thickness(0, slotted.Count == 0 ? 12 : 22, 0, 0);
        if (nothing is not null)
        {
            nothingWords.Text = nothing;
            if (nothingLine.Visibility == Visibility.Collapsed)
            {
                nothingLine.Opacity = 0;
                nothingLine.Visibility = Visibility.Visible;
                Motion.FadeTo(nothingLine, 1, Motion.FadeInTime);
            }
        }
        else if (nothingLine.Visibility == Visibility.Visible)
            Motion.FadeOutThen(nothingLine, () =>
            {
                if (ReadDiscovery.NothingLine(ctx.Known.Reading.Search.Sources ?? found) is null)
                {
                    nothingLine.Visibility = Visibility.Collapsed;
                    nothingWords.Text = "";
                }
                nothingLine.Opacity = 1;
            });
        if (slotted.Count == 0 && nothing is null)
        {
            // Every hit joined AniList's results: the section has nothing left to show, so it
            // fades out as one, heading and all, rather than leaving its heading alone.
            if (headShown && !sectionLeaving)
            {
                var turn = ++sourcesTurn;
                sectionLeaving = true;
                Motion.FadeOutThen(sources, () =>
                {
                    if (!sectionLeaving || turn != sourcesTurn)
                        return;
                    sectionLeaving = false;
                    sources.Visibility = Visibility.Collapsed;
                    sourceRows.Children.Clear();
                    sourceViews.Clear();
                    nothingLine.Visibility = Visibility.Collapsed;
                    Motion.FadeTo(sources, 1, TimeSpan.Zero);
                });
            }
            return;
        }
        sourcesTurn++;
        if (sectionLeaving)
        {
            // Rows again before the section had gone: it comes back.
            sectionLeaving = false;
            Motion.FadeTo(sources, 1, Motion.FadeInTime);
        }
        HeadIn();
    }

    /// <summary>From your sources fades in with its first row, or with the line for the sources that found nothing.</summary>
    void HeadIn()
    {
        if (headShown || sources.Visibility != Visibility.Visible)
            return;
        if (sourceViews.Values.Any(v => v.Seen) || nothingLine.Visibility == Visibility.Visible)
        {
            headShown = true;
            Motion.FadeTo(sourcesHead, 1, Motion.FadeInTime);
        }
    }

    /// <summary>
    /// AniList's part going or coming with Your sources only: its grid fades out in 200 ms while
    /// the sources' rows rise into its place on the standard spring, and back again. With
    /// reduced motion the page crossfades from one to the other.
    /// </summary>
    void AnilistShown(bool show, bool animate)
    {
        if (show == anilistShown)
            return;
        anilistShown = show;
        if (!animate)
        {
            anilist.Visibility = show ? Visibility.Visible : Visibility.Collapsed;
            anilist.Opacity = 1;
            anilist.Height = double.NaN;
            return;
        }
        if (!Motion.Enabled)
        {
            Crossfade.Swap(results, () =>
            {
                anilist.Visibility = show ? Visibility.Visible : Visibility.Collapsed;
                anilist.Opacity = 1;
                anilist.Height = double.NaN;
            });
            return;
        }
        if (show)
        {
            anilist.Height = 0;
            anilist.Opacity = 0;
            anilist.Visibility = Visibility.Visible;
            anilist.Measure(new global::Windows.Foundation.Size(results.ActualWidth > 0 ? results.ActualWidth : double.PositiveInfinity, double.PositiveInfinity));
            var to = anilist.DesiredSize.Height;
            Motion.Ease(anilist, 0, to, v =>
            {
                anilist.Height = v;
                if (Math.Abs(v - to) < 0.5)
                    anilist.Height = double.NaN;
            });
            Motion.FadeTo(anilist, 1, Motion.FadeInTime);
            return;
        }
        var from = anilist.ActualHeight;
        Motion.FadeTo(anilist, 0, Motion.FadeInTime);
        Motion.Ease(anilist, from, 0, v =>
        {
            anilist.Height = v;
            if (v <= 0.5 && !anilistShown)
            {
                anilist.Visibility = Visibility.Collapsed;
                anilist.Height = double.NaN;
                anilist.Opacity = 1;
            }
        });
    }

    /// <summary>
    /// Once the whole search has settled, one polite announcement of what came: "4 from AniList,
    /// 2 from your sources." Turning Your sources only on says once that AniList's results are hidden.
    /// </summary>
    void Announce(ReadSearchState search)
    {
        if (search.Sources is not { } found || ReadDiscovery.Finished(found, search.Tiles.Count) is not { } said)
            return;
        var q = search.Query;
        var key = string.Join("\n", q.Text.Trim(), q.SourcesOnly, q.Started, q.Status, q.Sort,
            string.Join(",", q.Formats ?? []), string.Join(",", q.Origins ?? []), string.Join(",", q.Genres ?? []), string.Join(",", q.ExcludeGenres ?? []));
        if (key == announced)
            return;
        announced = key;
        Tell(said);
    }

    /// <summary>
    /// Says <paramref name="text"/> to Narrator from the page's heading, a text element that always
    /// has its peer, replacing what Search said before that has not been read yet.
    /// </summary>
    void Tell(string text)
    {
        var peer = FrameworkElementAutomationPeer.FromElement(title) ?? FrameworkElementAutomationPeer.CreatePeerForElement(title);
        peer?.RaiseNotificationEvent(AutomationNotificationKind.ActionCompleted, AutomationNotificationProcessing.MostRecent, text, "kaeri-read-search");
    }

    /// <summary>The next page while browsing, when the end of the results is within a screen of sight.</summary>
    void Near()
    {
        var search = ctx.Known.Reading.Search;
        if (Now != Mode.Browse || !search.HasMore || search.Loading || search.Tiles.Count == 0)
            return;
        if (scroll.VerticalOffset + scroll.ViewportHeight < scroll.ExtentHeight - morePlates.ActualHeight - scroll.ViewportHeight)
            return;
        ctx.Known.Reading.SearchMore();
    }

    /// <summary>"1 match in Shows / Show them": the matches in the other scope under AniList's grid, with both media on.</summary>
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

    // The filters

    /// <summary>Whether a query sets any filter, the words, the sort and Your sources only aside.</summary>
    internal static bool AnyFilter(ReadSearchQuery q) =>
        (q.Formats?.Length ?? 0) > 0 || (q.Origins?.Length ?? 0) > 0 || q.Status is not null
        || (q.Genres?.Length ?? 0) > 0 || (q.ExcludeGenres?.Length ?? 0) > 0 || ReadDiscovery.StartedSet(q);

    static string[] Labels(string[]? keys, ReadFilterChoice[] choices) =>
        [.. (keys ?? []).Select(k => choices.FirstOrDefault(c => c.Key == k)?.Label.Text() ?? k)];

    /// <summary>"Manga, Japan, Popular · 1,240 titles": what the browse shows, in the order it shows it.</summary>
    static string BrowseLine(ReadSearchQuery q, uint? total)
    {
        var f = Filters;
        if (Titled is { } titled)
        {
            if (ReadDiscovery.SameAsk(titled.Query, q, f.Sorts.FirstOrDefault()))
                return total is { } count ? T.ReadSearchLineTotal(titled.Line.Text(), T.ReadSearchTotal(count)) : T.ReadSearchBrowseLine(titled.Line.Text());
            // Asked something else since: the row's line is gone for good.
            Titled = null;
        }
        var parts = Labels(ReadDiscovery.FoldedFormats(q.Formats), f.Formats).Concat(Labels(q.Origins, f.Origins)).ToList();
        if (q.Status is { } s)
            parts.AddRange(Labels([s], f.Statuses));
        parts.AddRange((q.Genres ?? []).Select(Searches.GenreName));
        parts.AddRange((q.ExcludeGenres ?? []).Select(g => T.SearchGenreLeftOut(Searches.GenreName(g))));
        if (ReadDiscovery.StartedSet(q))
            parts.AddRange(Labels([q.Started!], f.Started ?? []));
        parts.Add(SortWords(q.Sort ?? f.Sorts.FirstOrDefault()));
        var what = parts.Count == 0 ? "" : parts.Skip(1).Aggregate(parts[0], T.ReadSearchBrowseParts);
        return total is { } n ? T.ReadSearchLineTotal(what, T.ReadSearchTotal(n)) : T.ReadSearchBrowseLine(what);
    }

    static string SortWords(string? key) => key switch
    {
        "TRENDING_DESC" => T.ReadSortTrending,
        "SCORE_DESC" => T.ReadSortScore,
        "START_DATE_DESC" => T.ReadSortNewest,
        "TITLE_ROMAJI" => T.LibrarySortTitle,
        _ => T.SortPopularity,
    };

    /// <summary>
    /// Each drop-down says its name, or in amber what it has, as the shows' row does; Your sources
    /// only comes with the first word; then Clear filters, the chips, the badge and the panel.
    /// </summary>
    void ShowFilters()
    {
        var f = Filters;
        var q = Query;
        var words = !Browsing;
        string[] genreWords = [.. (q.Genres ?? []).Select(Searches.GenreName), .. (q.ExcludeGenres ?? []).Select(g => T.SearchGenreLeftOut(Searches.GenreName(g)))];
        SearchParts.Say(genres, T.FilterGenre, SearchFilters.Say(T.FilterGenre, genreWords), genreWords.Length > 0);
        var formatWords = Labels(ReadDiscovery.FoldedFormats(q.Formats), f.Formats);
        SearchParts.Say(format, T.FilterFormat, SearchFilters.Say(T.FilterFormat, formatWords), formatWords.Length > 0);
        var statusWord = q.Status is { } s ? Labels([s], f.Statuses)[0] : null;
        SearchParts.Say(status, T.FilterStatus, statusWord ?? T.FilterStatus, statusWord is not null);
        var originWords = Labels(q.Origins, f.Origins);
        SearchParts.Say(origin, T.FilterOrigin, SearchFilters.Say(T.FilterOrigin, originWords), originWords.Length > 0);
        var startedWord = ReadDiscovery.StartedSet(q) ? Labels([q.Started!], f.Started ?? [])[0] : null;
        SearchParts.Say(started, T.ReadSearchStarted, startedWord ?? T.ReadSearchStarted, startedWord is not null);
        SearchParts.Say(available, T.SearchAvailableName, q.SourcesOnly ? T.ReadSearchSourcesOnly : T.SearchAvailableName, q.SourcesOnly);
        SearchParts.Say(sort, T.SearchSortName, q.Sort is { } o ? SortWords(o) : T.SearchSortName, q.Sort is not null);

        // Your sources only asks the sources alone, so it comes with the first word and goes with the last.
        if (words && available.Visibility == Visibility.Collapsed)
        {
            available.Opacity = 0;
            available.Visibility = Visibility.Visible;
            Motion.FadeTo(available, 1, Motion.FadeInTime);
        }
        else if (!words && available.Visibility == Visibility.Visible)
            Motion.FadeOutThen(available, () =>
            {
                if (!Browsing)
                    return;
                available.Visibility = Visibility.Collapsed;
                available.Opacity = 1;
            });

        // While it is on, AniList is not asked: the filters and the sort for AniList dim, and Narrator hears why.
        var dimmed = words && q.SourcesOnly;
        var dim = dimmed ? Dimmed : 1;
        foreach (var button in new[] { genres, format, status, origin, started, sort })
        {
            if (Math.Abs(button.Opacity - dim) > 0.01)
                Motion.FadeTo(button, dim, Motion.FadeInTime);
            AutomationProperties.SetHelpText(button, dimmed ? T.WindowsReadSearchFiltersDimmed : "");
        }

        if (AnyFilter(q) || q.Sort is not null || words && q.SourcesOnly)
            Motion.Show(clear);
        else
            Motion.Hide(clear);
        // Every reading filter is in the row, so the panel holds none of its own to count.
        SearchParts.Badge(filtersButton, filtersBadge, 0);
        ShowChips();
        if (panelOpen)
            panel.Show(q, words);
    }

    void BuildGenres()
    {
        var included = Query.Genres ?? [];
        genreList.Children.Clear();
        foreach (var genre in Filters.Genres)
            genreList.Children.Add(SearchParts.Box(Searches.GenreName(genre), included.Contains(genre), on =>
                Run(ReadFiltersPanel.Picking(Query, genre, on ? Pick.Include : Pick.Off))));
    }

    /// <summary>One Manga, which takes one-shots in.</summary>
    void BuildFormats()
    {
        var f = Filters;
        var choices = ReadDiscovery.FormatChoices(f.Formats);
        var apart = choices.Length != f.Formats.Length;
        var folded = ReadDiscovery.FoldedFormats(Query.Formats);
        formatList.Children.Clear();
        foreach (var choice in choices)
        {
            var key = choice.Key;
            formatList.Children.Add(SearchParts.Box(choice.Label.Text(), folded.Contains(key), _ =>
                Run(Query with { Formats = ReadDiscovery.ToggleFormat(Query.Formats, key, apart) })));
        }
    }

    void BuildOrigins()
    {
        var chosen = Query.Origins ?? [];
        originList.Children.Clear();
        foreach (var choice in Filters.Origins)
        {
            var key = choice.Key;
            originList.Children.Add(SearchParts.Box(choice.Label.Text(), chosen.Contains(key), on =>
            {
                var list = (Query.Origins ?? []).Where(k => k != key).ToList();
                if (on)
                    list.Add(key);
                Run(Query with { Origins = [.. list] });
            }));
        }
    }

    void BuildStatuses()
    {
        var current = Query.Status;
        statusMenu.Items.Clear();
        statusMenu.Items.Add(SearchParts.Radio(T.ReadFilterAny, "status", current is null, () => Run(Query with { Status = null })));
        foreach (var choice in Filters.Statuses)
        {
            var key = choice.Key;
            statusMenu.Items.Add(SearchParts.Radio(choice.Label.Text(), "status", current == key, () => Run(Query with { Status = key })));
        }
    }

    /// <summary>When the series started: any time, the last 12 months or the last 3 years.</summary>
    void BuildStarted()
    {
        var q = Query;
        var startedKey = ReadDiscovery.StartedSet(q) ? q.Started : null;
        startedMenu.Items.Clear();
        foreach (var choice in Filters.Started ?? [])
        {
            var key = choice.Key;
            var anyTime = key == "any"; // i18n-skip: the engine's key
            startedMenu.Items.Add(SearchParts.Radio(choice.Label.Text(), "started", anyTime ? startedKey is null : startedKey == key, () =>
                Run(Query with { Started = anyTime ? null : key })));
        }
    }

    /// <summary>All, or Your sources only, which leaves AniList out.</summary>
    void BuildAvailable()
    {
        var on = Query.SourcesOnly;
        availableMenu.Items.Clear();
        availableMenu.Items.Add(SearchParts.Radio(T.SearchAvailableAll, "available", !on, () => SourcesOnly(false)));
        availableMenu.Items.Add(SearchParts.Radio(T.ReadSearchSourcesOnly, "available", on, () => SourcesOnly(true)));
    }

    void SourcesOnly(bool on)
    {
        if (on == Query.SourcesOnly)
            return;
        Run(Query with { SourcesOnly = on });
        if (on)
            Tell(T.WindowsReadSearchAnilistHidden);
    }

    /// <summary>
    /// The sort: with words, Relevance first, as AniList orders by the match when no sort is
    /// chosen; with none, the engine's sorts, Popular standing for no choice.
    /// </summary>
    void BuildSorts()
    {
        var f = Filters;
        var words = !Browsing;
        var current = Query.Sort ?? (words ? null : f.Sorts.FirstOrDefault());
        sortMenu.Items.Clear();
        if (words)
            sortMenu.Items.Add(SearchParts.Radio(T.SortRelevance, "sort", current is null, () => Run(Query with { Sort = null })));
        foreach (var key in f.Sorts)
            sortMenu.Items.Add(SearchParts.Radio(SortWords(key), "sort", key == current, () => Run(Query with { Sort = key })));
    }

    // The chips

    /// <summary>
    /// Each filter set as a chip, in the row's order, that takes it off. Made again only when
    /// they change; a chip that held the focus hands it to the one now in its place.
    /// </summary>
    void ShowChips()
    {
        var next = Chips(Query, !Browsing);
        var labels = next.Select(c => c.Label).ToArray();
        if (labels.SequenceEqual(chips))
            return;
        var focused = XamlRoot is { } root ? FocusManager.GetFocusedElement(root) as UIElement : null;
        var at = focused is null ? -1 : chipRow.Children.IndexOf(focused);
        chips = labels;
        chipRow.Children.Clear();
        foreach (var (label, without) in next)
            chipRow.Children.Add(SearchParts.Chip(label, () => Run(without(Query))));
        chipRow.Visibility = next.Length > 0 ? Visibility.Visible : Visibility.Collapsed;
        if (at < 0)
            return;
        if (chipRow.Children.Count > 0)
            ((Control)chipRow.Children[Math.Min(at, chipRow.Children.Count - 1)]).Focus(FocusState.Keyboard);
        else
            filtersButton.Focus(FocusState.Keyboard);
    }

    static (string Label, Func<ReadSearchQuery, ReadSearchQuery> Without)[] Chips(ReadSearchQuery q, bool words)
    {
        var f = Filters;
        var list = new List<(string, Func<ReadSearchQuery, ReadSearchQuery>)>();
        foreach (var genre in q.Genres ?? [])
            list.Add((Searches.GenreName(genre), x => ReadFiltersPanel.Picking(x, genre, Pick.Off)));
        foreach (var genre in q.ExcludeGenres ?? [])
            list.Add((T.SearchGenreLeftOut(Searches.GenreName(genre)), x => ReadFiltersPanel.Picking(x, genre, Pick.Off)));
        var formatWords = Labels(ReadDiscovery.FoldedFormats(q.Formats), f.Formats);
        if (formatWords.Length > 0)
            list.Add((string.Join(", ", formatWords), x => x with { Formats = [] }));
        if (q.Status is { } status)
            list.Add((Labels([status], f.Statuses)[0], x => x with { Status = null }));
        var originWords = Labels(q.Origins, f.Origins);
        if (originWords.Length > 0)
            list.Add((string.Join(", ", originWords), x => x with { Origins = [] }));
        if (ReadDiscovery.StartedSet(q))
            list.Add((Labels([q.Started!], f.Started ?? [])[0], x => x with { Started = null }));
        if (words && q.SourcesOnly)
            list.Add((T.ReadSearchSourcesOnly, x => x with { SourcesOnly = false }));
        return [.. list];
    }

    // The panel

    /// <summary>Ctrl+Shift+F on this page: the panel opens, or closes when it is open.</summary>
    internal void ToggleFilters() => ShowPanel(!panelOpen, fromKeys: true);

    /// <summary>
    /// The panel slides in from the right edge on the standard spring and fades out as it goes,
    /// as the shows' does, the results taking their new width at once. Opened from the keyboard,
    /// it takes the focus; closed with the focus in it, the focus goes back to Filters.
    /// </summary>
    void ShowPanel(bool open, bool fromKeys = false)
    {
        panelOpen = open;
        filtersButton.IsChecked = open;
        var visual = ElementCompositionPreview.GetElementVisual(panelHost);
        if (open)
        {
            panel.Show(Query, !Browsing);
            Motion.ShowStandard(panelHost);
            Motion.Spring(visual, "Translation", System.Numerics.Vector3.Zero, new System.Numerics.Vector3(48, 0, 0));
            if (fromKeys)
                DispatcherQueue.TryEnqueue(Microsoft.UI.Dispatching.DispatcherQueuePriority.Low, panel.FocusStart);
            return;
        }
        var inside = XamlRoot is { } root && FocusManager.GetFocusedElement(root) is DependencyObject focused && Within(focused, panelHost);
        Motion.Spring(visual, "Translation", new System.Numerics.Vector3(48, 0, 0));
        Motion.HideStandard(panelHost);
        if (inside)
            filtersButton.Focus(FocusState.Keyboard);
    }

    static bool Within(DependencyObject element, DependencyObject ancestor)
    {
        for (var at = element; at is not null; at = VisualTreeHelper.GetParent(at))
            if (at == ancestor)
                return true;
        return false;
    }
}

/// <summary>
/// One of Home's discovery rows on Search's start, as the shows' rails are: its heading in the
/// section style, with the row's side words in haze after it and See all at its end, then its
/// covers in the shows' cells in a strip that pages by whole covers. While the row is on its way
/// it holds the shows' rail placeholders after 600 ms.
/// </summary>
sealed partial class ReadStartRow : StackPanel
{
    readonly Ctx ctx;
    readonly TextBlock heading = new()
    {
        Style = Ui.Res<Style>("SectionTitleStyle"),
        MaxLines = 1,
        TextWrapping = TextWrapping.NoWrap,
        TextTrimming = TextTrimming.CharacterEllipsis,
        VerticalAlignment = VerticalAlignment.Center,
    };
    readonly Run words = new(), side = new();
    readonly HyperlinkButton seeAll;
    readonly CoverSlot slot;
    ReadHomeRow row;
    string drawn = "";

    public ReadStartRow(Ctx ctx, ReadHomeRow row, Action<ReadHomeRow> seeAll)
    {
        this.ctx = ctx;
        this.row = row;
        // 12 under the heading, as under the shows' section headings.
        Spacing = 12;
        side.FontSize = 12;
        side.FontWeight = FontWeights.Normal;
        side.Foreground = LiveBrush.Of("HazeBrush");
        heading.Inlines.Add(words);
        heading.Inlines.Add(side);
        AutomationProperties.SetHeadingLevel(heading, AutomationHeadingLevel.Level2);
        this.seeAll = SearchParts.SeeAll(() => seeAll(this.row));
        Children.Add(SearchParts.RailHead(heading, this.seeAll));
        var name = ReadDiscovery.Heading(row.Title).ToString();
        slot = new CoverSlot((FrameworkElement)States.Covers(12, rail: true), new CoverStrip(name, SearchParts.CoverWidth, SearchParts.CoverHeight)) { Label = name };
        Children.Add(slot);
    }

    public string Id => row.Id;

    /// <summary>Takes the row as the engine now has it; <paramref name="since"/> is when Search began waiting for Home.</summary>
    public void Update(ReadHomeRow now, DateTimeOffset since)
    {
        row = now;
        var name = ReadDiscovery.Heading(now.Title).ToString();
        if (words.Text != name)
            words.Text = name;
        var aside = now.Side?.Text() is { Length: > 0 } a ? a : null;
        // An em space keeps the side words apart from the heading.
        var spaced = aside is null ? "" : " " + aside;
        if (side.Text != spaced)
            side.Text = spaced;
        AutomationProperties.SetName(heading, aside is null ? name : T.ReadNameHeading(name, aside));
        seeAll.Visibility = now.ViewAll is null ? Visibility.Collapsed : Visibility.Visible;
        slot.Label = name;
        var tiles = now.Tiles ?? [];
        if (tiles.Length == 0)
        {
            if (now.Pending)
                slot.Ask(since);
            return;
        }
        var key = string.Join("\n", tiles.Select(t => $"{t.Work}\t{t.Title}\t{t.Cover}\t{ReadDiscovery.RowCaption(now.Kind, t)}"));
        if (key == drawn)
            return;
        drawn = key;
        slot.Covers([.. tiles.Select(t => (FrameworkElement)ReadingParts.SearchCard(ctx, t, caption: ReadDiscovery.RowCaption(now.Kind, t)))]);
    }
}
