using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;

namespace Kaeri.Windows.Pages;

/// <summary>
/// Search, opened from the title bar's box: the results as a poster grid under a row of filters.
/// With nothing typed and no filter set it shows this season's titles; a filter with nothing typed
/// browses, as the engine's query does with no text. The filters stay set for the session, so a
/// new search from the title bar keeps them. While the page is up, the results follow the box as
/// it is typed in, and the next page comes as the end of the results comes into sight.
/// </summary>
public sealed partial class SearchPage : Page
{
    static SearchFilters remembered = SearchFilters.None;

    readonly Ctx ctx = Ctx.Current;
    /// <summary>
    /// After typing stops for as long as the engine waits before it searches: only then does the
    /// page start waiting for the answer, so the results that show stay while the words change.
    /// </summary>
    readonly DispatcherTimer settle = new() { Interval = TimeSpan.FromMilliseconds(400) };
    /// <summary>The title bar's search box, found once the page is in the window.</summary>
    AutoSuggestBox? box;
    /// <summary>The pages of the search on screen past the first: whether there are more, how many came, and whether one is on its way.</summary>
    bool hasNext, askingMore;
    uint pages = 1;
    readonly List<Entry> more = [];
    string query = "";
    SearchFilters filters = remembered;
    /// <summary>What was last asked, or null while the page shows this season.</summary>
    SearchQuery? asked;
    Entry[] entries = [];
    readonly SurpriseButton surprise;
    TrendingRail? rail;

    public SearchPage()
    {
        InitializeComponent();
        surprise = new SurpriseButton(() => new SurprisePool.Filters(Searches.With(query, filters)), T.SurpriseHintFilters);
        SurpriseHost.Content = surprise;
        State.Placeholders = () => States.Covers();
        State.ActivityText = T.StateLoading;
        settle.Tick += (_, _) =>
        {
            settle.Stop();
            Wait();
        };
        Loaded += (_, _) =>
        {
            box ??= (XamlRoot?.Content as FrameworkElement)?.FindName("SearchBox") as AutoSuggestBox;
            if (box is not null)
            {
                box.TextChanged -= OnBoxText;
                box.TextChanged += OnBoxText;
            }
        };
        Unloaded += (_, _) =>
        {
            if (box is not null)
                box.TextChanged -= OnBoxText;
        };
        MoreRow.Children.Add(new ScrollViewer
        {
            Content = States.Covers(8, rail: true),
            HorizontalScrollMode = ScrollMode.Disabled,
            HorizontalScrollBarVisibility = ScrollBarVisibility.Hidden,
            VerticalScrollMode = ScrollMode.Disabled,
            IsTabStop = false,
        });
        // The frame keeps the page for Back and the next visit: the results, the filters and the scroll stay.
        NavigationCacheMode = NavigationCacheMode.Enabled;
    }

    /// <summary>
    /// The box typed in while the page is up: the results follow it, as on the Mac. The engine
    /// waits until the typing stops and searches once; Enter still searches at once.
    /// </summary>
    void OnBoxText(AutoSuggestBox sender, AutoSuggestBoxTextChangedEventArgs args)
    {
        if (args.Reason != AutoSuggestionBoxTextChangeReason.UserInput || Frame?.Content != this)
            return;
        var text = sender.Text.Trim();
        if (text == query)
            return;
        query = text;
        Run(live: true, typing: true);
    }

    /// <summary>Enter in the box while this page is up: searches at once, on this page.</summary>
    internal void Submit(string text)
    {
        query = text;
        openedWith = text;
        Run();
    }

    /// <summary>What the page was last opened with: a typed query, a browse from Home, or nothing.</summary>
    object? openedWith;
    bool opened;

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        // Heard from each visit's start, before anything is asked, until the frame moves on.
        ctx.Hear(this, Heard, settle.Stop);
        // Back to the search this page still shows: it stays as it was, and only takes an answer
        // that came while another page was up.
        if (opened && e.NavigationMode == NavigationMode.Back && SameOpening(e.Parameter, openedWith))
        {
            if (!ctx.Known.ListEntries(View).Select(x => x.Id).SequenceEqual(entries.Select(x => x.Id)))
            {
                entries = ctx.Known.ListEntries(View);
                Show();
            }
            rail?.Visit();
            return;
        }
        opened = true;
        openedWith = e.Parameter;
        query = e.Parameter as string ?? (e.Parameter as SearchQuery)?.Text ?? "";
        if (e.Parameter is SearchQuery browse)
            filters = SearchFilters.None with { Genres = browse.Genres ?? [], Sort = browse.Sort };
#if KAERI_TESTING
        // --show=browse: a genre and a sort set with nothing typed, for a screenshot.
        if (ctx.Options.Show == "browse")
            filters = SearchFilters.None with { Genres = ["Adventure"], Sort = SearchSort.Popularity };
        else if (ctx.Options.Show == "available")
            filters = SearchFilters.None with { Available = Available.PlaysHere };
        else if (ctx.Options.Show == "browse-none")
            filters = SearchFilters.None with { Genres = ["Horror"], Year = 1971 };
#endif
        if (ctx.Known.Facets is null)
            ctx.Send(new Command.LoadSearchFacets());
        BuildFilters();
        Run();
        rail?.Visit();
    }

#if KAERI_TESTING
    /// <summary>
    /// --show=browse-period-menu opens the heading's menu; browse-trending-week switches to This
    /// week; motion-period does both as a person would, the menu first, for the period clip.
    /// </summary>
    void StageRail(TrendingRail rail)
    {
        void After(double seconds, Action act)
        {
            var timer = new DispatcherTimer { Interval = TimeSpan.FromSeconds(seconds) };
            timer.Tick += (_, _) =>
            {
                timer.Stop();
                act();
            };
            timer.Start();
        }
        switch (ctx.Options.Show)
        {
            case "browse-period-menu":
                After(2.5, rail.OpenMenu);
                break;
            case "browse-trending-week":
                After(2.5, () => rail.Pick(TrendPeriod.Week));
                break;
            case "motion-period":
                After(3.0, rail.OpenMenu);
                After(4.4, () => rail.Pick(TrendPeriod.Week));
                break;
        }
    }
#endif

    /// <summary>
    /// Asks the engine for what the box and the filters now say, or this season when they say
    /// nothing. <paramref name="live"/> searches as the words or filters change, which the engine
    /// waits on and does not keep among the recent searches; <paramref name="typing"/> leaves the
    /// results showing until the typing stops.
    /// </summary>
    void Run(bool live = false, bool typing = false)
    {
        settle.Stop();
        remembered = filters;
        ShowFilters();
        // Surprise me picks from the filters, so it goes when words are typed; the rail shows with nothing asked.
        SurpriseHost.Visibility = query.Length == 0 ? Visibility.Visible : Visibility.Collapsed;
        var browsing = query.Length == 0 && !filters.Narrows && filters.Sort is null;
        if (browsing)
        {
            rail ??= new TrendingRail(period => Change(filters with { Sort = SearchSort.Trending, Period = period is TrendPeriod.Today ? null : period }, now: true));
            if (RailHost.Content is null)
            {
                RailHost.Content = rail;
                rail.Visit();
#if KAERI_TESTING
                StageRail(rail);
#endif
            }
            RailHost.Visibility = Visibility.Visible;
        }
        else
            RailHost.Visibility = Visibility.Collapsed;
        if (browsing)
        {
            asked = null;
            more.Clear();
            hasNext = askingMore = false;
            CaptionLine.Visibility = Visibility.Collapsed;
            Heading.Text = T.NavSearch;
            Section.Text = T.ContinueThisSeason;
            Section.Visibility = Visibility.Visible;
            entries = ctx.Known.ListEntries(new ContentFor.Seasonal());
            Wait();
            if (ctx.Known.List(new ContentFor.Seasonal()) is null or ContentList.Failed)
                ctx.Send(new Command.LoadSeasonal());
            Show();
            return;
        }
        var next = Searches.With(query, filters);
        if (asked is not { } was || !Searches.Same(was, next))
        {
            // Another search: its pages start again, and what the last said of itself goes.
            more.Clear();
            hasNext = askingMore = false;
            pages = 1;
            CaptionLine.Visibility = Visibility.Collapsed;
        }
        asked = next;
        Heading.Text = query.Length > 0 ? T.SearchResultsFor(query) : T.SearchBrowse;
        Section.Visibility = Visibility.Collapsed;
        entries = ctx.Known.ListEntries(new ContentFor.Search(next));
        if (typing && entries.Length == 0)
            settle.Start();
        else
            Wait();
        ctx.Send(live ? new Command.SearchLive(next) : new Command.Search(next));
        Show();
    }

    /// <summary>
    /// The new ask's results are on their way: known ones show now; otherwise the results that
    /// show stay 600 ms before plates take their place, or plates come at once over nothing.
    /// </summary>
    void Wait()
    {
        if (entries.Length > 0)
            State.Wait(hasContent: true);
        else if (ctx.Known.List(View) is not (ContentList.Entries or ContentList.Empty or ContentList.Failed))
            State.WaitKeeping(Grid.HasCovers);
    }

    ContentFor View => asked is { } q ? new ContentFor.Search(q) : new ContentFor.Seasonal();

    static bool SameOpening(object? a, object? b) => (a, b) switch
    {
        (null, null) => true,
        (string x, string y) => x == y,
        (SearchQuery x, SearchQuery y) => ReferenceEquals(x, y) || Searches.Same(x, y),
        _ => false,
    };

    void Heard(IReadOnlyList<Event> batch)
    {
        surprise.Heard(batch);
        rail?.Heard(batch);
        foreach (var e in batch)
        {
            if (e is Event.Facets)
                BuildGenres();
            if (e is Event.Demo or Event.Providers)
                DemoLine.Visibility = ctx.Known.Demo is { On: true, Only: true } ? Visibility.Visible : Visibility.Collapsed;
            if (e is Event.LiveSearch live)
                PausedLine.Visibility = live.Paused && query.Length > 0 ? Visibility.Visible : Visibility.Collapsed;
            if (e is Event.Caption caption && Searches.Same(caption.View, View))
            {
                CaptionLine.Text = caption.Text;
                CaptionLine.Visibility = caption.Text.Length > 0 ? Visibility.Visible : Visibility.Collapsed;
            }
            if (e is Event.SearchPaging paging && asked is { } a && Searches.Same(paging.Query, a))
            {
                hasNext = paging.HasNext;
                pages = 1;
                more.Clear();
                askingMore = false;
            }
            if (e is Event.SearchMore page && asked is { } b && Searches.Same(page.Query, b))
            {
                askingMore = false;
                // An empty page failed: it is asked for again as the end comes into sight.
                if (page.Entries.Length > 0)
                {
                    pages = page.Page;
                    hasNext = page.HasNext;
                    var have = entries.Concat(more).Select(x => x.Id).ToHashSet();
                    var added = page.Entries.Where(x => have.Add(x.Id)).ToArray();
                    more.AddRange(added);
                    Grid.Append(added);
                }
                ShowMore();
            }
            if (e is not Event.Content c || !Searches.Same(c.View, View) || c.ContentValue is ContentList.Loading)
                continue;
            entries = ctx.Known.ListEntries(View);
            Show();
        }
    }

    void Show()
    {
        // One haze line while the demo is the only source: it goes when a store source is installed.
        DemoLine.Visibility = ctx.Known.Demo is { On: true, Only: true } ? Visibility.Visible : Visibility.Collapsed;
        var list = ctx.Known.List(View);
        // Not answered yet: what shows stays until the answer.
        if (entries.Length == 0 && list is not (ContentList.Entries or ContentList.Empty or ContentList.Failed))
            return;
        Grid.Show([.. entries, .. more]);
        ShowMore();
        if (entries.Length > 0)
            State.Done();
        else if (list is ContentList.Failed f)
            State.Failed(f.V1, () => Run());
        else if (list is ContentList.Entries or ContentList.Empty)
            State.Empty(Nothing());
    }

    /// <summary>The plates after the results while there is a next page; asks for it once they are near.</summary>
    void ShowMore()
    {
        var shown = asked is not null && hasNext && Grid.HasCovers;
        MoreRow.Visibility = shown ? Visibility.Visible : Visibility.Collapsed;
        if (shown)
            // Laid out first, so a short page that leaves the plates in sight asks at once.
            DispatcherQueue.TryEnqueue(Microsoft.UI.Dispatching.DispatcherQueuePriority.Low, Near);
    }

    void OnScrolled(object? sender, ScrollViewerViewChangedEventArgs e) => Near();

    /// <summary>The next page, when the plates after the results are within a screen of sight.</summary>
    void Near()
    {
        if (asked is not { } q || !hasNext || askingMore || MoreRow.Visibility != Visibility.Visible)
            return;
        if (Scroll.VerticalOffset + Scroll.ViewportHeight < Scroll.ExtentHeight - MoreRow.ActualHeight - Scroll.ViewportHeight)
            return;
        askingMore = true;
        ctx.Send(new Command.SearchMore(q, pages + 1));
    }

    /// <summary>No results: the query repeated, and a way out when the filters narrowed it.</summary>
    UIElement Nothing()
    {
        if (asked is null)
            return States.Unavailable(T.SeasonalEmptyTitle(Wording.SeasonWithYear(KaeriFfiMethods.SeasonAt(DateTimeOffset.UtcNow.ToUnixTimeSeconds()))), T.SeasonalEmptyDetail);
        if (!filters.Narrows)
            return States.Unavailable(T.SearchNothingFound(query), T.SearchNothingFoundDetail);
        var title = query.Length > 0 ? T.SearchNothingFoundWithFilters(query) : T.SearchNothingWithFilters;
        return States.Unavailable(title, T.SearchTryFewerFilters, T.SearchClearFilters, Clear);
    }

    // The filter row.

    void BuildFilters()
    {
        BuildGenres();
        FormatList.Children.Clear();
        foreach (var format in Searches.Formats)
        {
            var box = new CheckBox { Content = Searches.FormatName(format), Tag = format, IsChecked = filters.Formats.Contains(format) };
            box.Click += (_, _) => Change(filters.Toggle(format));
            FormatList.Children.Add(box);
        }

        SeasonMenu.Items.Clear();
        SeasonMenu.Items.Add(Radio(T.FilterAnySeason, "season", filters.Season is null, () => filters with { Season = null }));
        foreach (var season in Enum.GetValues<Season>())
            SeasonMenu.Items.Add(Radio(Wording.SeasonName(season), "season", filters.Season == season, () => filters with { Season = season }));

        YearMenu.Items.Clear();
        YearMenu.Items.Add(Radio(T.FilterAnyYear, "year", filters.Year is null, () => filters with { Year = null }));
        foreach (var year in Searches.Years(DateTime.Now.Year))
            YearMenu.Items.Add(Radio(year.ToString(System.Globalization.CultureInfo.InvariantCulture), "year", filters.Year == year, () => filters with { Year = year }));

        AvailableMenu.Items.Clear();
        AvailableMenu.Items.Add(Radio(T.SearchAvailableAll, "available", filters.Available is null, () => filters with { Available = null }));
        foreach (var available in new[] { Available.PlaysHere, Available.OnYourServices })
            AvailableMenu.Items.Add(Radio(Searches.AvailableName(available), "available", filters.Available == available, () => filters with { Available = available }));

        SortMenu.Items.Clear();
        SortMenu.Items.Add(Radio(T.SortRelevance, "sort", filters.Sort is null, () => filters with { Sort = null }));
        foreach (var sort in Searches.Sorts)
        {
            if (sort != SearchSort.Trending)
            {
                SortMenu.Items.Add(Radio(Searches.SortName(sort), "sort", filters.Sort == sort, () => filters with { Sort = sort, Period = null }));
                continue;
            }
            // Trending is three items, one per period: the sort and its period set together.
            foreach (var p in Browsing.Periods)
            {
                var period = p;
                var on = filters.Sort == sort && (filters.Period ?? TrendPeriod.Today) == period;
                SortMenu.Items.Add(Radio(Browsing.Heading(period), "sort", on, () => filters with { Sort = sort, Period = period is TrendPeriod.Today ? null : period }));
            }
        }
    }

    /// <summary>The genres: the engine's list once it has one, AniList's usual ones until then.</summary>
    void BuildGenres()
    {
        var genres = ctx.Known.Facets?.Genres is { Length: > 0 } g ? g : Searches.Genres;
        GenreList.Children.Clear();
        foreach (var genre in genres)
        {
            var box = new CheckBox { Content = Searches.GenreName(genre), IsChecked = filters.Genres.Contains(genre) };
            box.Click += (_, _) => Change(filters.Toggle(genre, genres));
            GenreList.Children.Add(box);
        }
    }

    RadioMenuFlyoutItem Radio(string text, string group, bool on, Func<SearchFilters> then)
    {
        var item = new RadioMenuFlyoutItem { Text = text, GroupName = group, IsChecked = on };
        item.Click += (_, _) => Change(then());
        return item;
    }

    /// <summary>A filter changed: show it at once, search once the clicks stop.</summary>
    void Change(SearchFilters to, bool now = false)
    {
        filters = to;
        if (now)
            BuildFilters();
        ShowFilters();
        // The engine waits until the clicks stop (250 ms) and searches once.
        Run(live: !now);
    }

    /// <summary>Each drop-down says its name, or in amber what it has.</summary>
    void ShowFilters()
    {
        Say(GenreButton, T.FilterGenre, SearchFilters.Say(T.FilterGenre, [.. filters.Genres.Select(Searches.GenreName)]), filters.Genres.Length > 0);
        Say(FormatButton, T.FilterFormat, SearchFilters.Say(T.FilterFormat, [.. filters.Formats.Select(Searches.FormatName)]), filters.Formats.Length > 0);
        Say(SeasonButton, T.FilterSeason, filters.Season is { } s ? Wording.SeasonName(s) : T.FilterSeason, filters.Season is not null);
        Say(YearButton, T.FilterYear, filters.Year?.ToString(System.Globalization.CultureInfo.InvariantCulture) ?? T.FilterYear, filters.Year is not null);
        Say(AvailableButton, T.SearchAvailableName, filters.Available is { } a ? Searches.AvailableName(a) : T.SearchAvailableName, filters.Available is not null);
        Say(SortButton, T.SearchSortName, filters.Sort is { } o ? Searches.SortName(o, filters.Period) : T.SearchSortName, filters.Sort is not null);
        if (filters.IsEmpty)
            Motion.Hide(ClearButton);
        else
            Motion.Show(ClearButton);
    }

    /// <summary>A filter's words, set only when they change, and its spoken name: the filter, then what it has.</summary>
    static void Say(DropDownButton button, string name, string text, bool set)
    {
        if (button.Content as string != text)
            button.Content = text;
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(button, set ? $"{name}, {text}" : name);
        if (set)
            button.Foreground = (Microsoft.UI.Xaml.Media.Brush)Application.Current.Resources["AccentTextFillColorTertiaryBrush"];
        else
            button.ClearValue(Control.ForegroundProperty);
    }

    void Clear()
    {
        filters = SearchFilters.None;
        BuildFilters();
        Run();
    }

    void OnClear(object sender, RoutedEventArgs e) => Clear();
}
