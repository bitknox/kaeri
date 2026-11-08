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
/// sort menu, with more coming as the end nears. Typed words show AniList's covers in the grid
/// (a result a source has says so on its third line), then From your sources: one sideways row
/// of covers per source, each with its own placeholders, failure and Try again, and one line for
/// the sources that found nothing; More from AniList loads the next page under it. The filters
/// are format, origin, status, genres and Started, then, after a rule, Your sources only, which
/// leaves AniList out. With both media on, a line under AniList's grid counts the matches in
/// Shows and switches to them. Only the parts wait for their answers, each on its own 600 ms;
/// what changes in place crossfades, and the line over the grid keeps its room so the grid
/// never moves.
/// </summary>
public sealed partial class ReadSearchPage : Page
{
    /// <summary>A search another place asked before opening the page, such as a Home row's View all.</summary>
    internal static ReadSearchQuery? Opening { get; set; }

    /// <summary>
    /// The line a row's View all gives its results, "Manga started in the last 12 months", held
    /// while the search asks what the row asked.
    /// </summary>
    internal static (ReadSearchQuery Query, Msg Line)? Titled { get; set; }

    static ReadFilters? filters;
    static ReadFilters Filters => filters ??= KaeriFfiMethods.ReadFilters();

    /// <summary>The other filters while Your sources only is on: they hold, but AniList is not asked.</summary>
    const double Dimmed = 0.38;

    readonly Ctx ctx = Ctx.Current;
    readonly TextBlock title = new() { Text = T.NavSearch, Style = Ui.Res<Style>("PageTitleStyle") };
    readonly DropDownButton format = Menu(), origin = Menu(), status = Menu(), genres = Menu(), started = Menu();
    readonly DropDownButton sort = new() { MinHeight = 32 };
    readonly StackPanel filterRow = new() { Orientation = Orientation.Horizontal, Spacing = 8 };
    readonly Border rule = new() { Width = 1, Height = 19, Margin = new Thickness(4, 0, 4, 0), VerticalAlignment = VerticalAlignment.Center, Visibility = Visibility.Collapsed };
    readonly ToggleButton sourcesOnly = new() { MinHeight = 32, CornerRadius = new CornerRadius(4), Visibility = Visibility.Collapsed };
    readonly FontIcon sourcesOnlyCheck = new() { Glyph = "", FontSize = 12, Visibility = Visibility.Collapsed };
    readonly ScrollViewer scroll = new() { VerticalScrollBarVisibility = ScrollBarVisibility.Auto, IsTabStop = false };
    readonly StateHost state = new();
    // The results and the nothing anywhere message, one in place of the other.
    readonly StackPanel results = new(), body = new();
    readonly Border nothingHost = new() { Visibility = Visibility.Collapsed };
    // The line over AniList's grid: the filters while browsing, the count with words.
    readonly FadeLine line = new(() => new TextBlock
    {
        FontSize = 14,
        LineHeight = 19,
        Foreground = Ui.Res<Brush>("HazeBrush"),
        TextTrimming = TextTrimming.CharacterEllipsis,
        TextWrapping = TextWrapping.NoWrap,
    })
    { Margin = new Thickness(0, 0, 0, 12) };
    ReadGrid grid = Named(new ReadGrid());
    // AniList's later pages with words: under More from AniList.
    readonly ReadGrid later = Named(new ReadGrid { Margin = new Thickness(0, 22, 0, 0), Visibility = Visibility.Collapsed });
    readonly CoverSlot anilistSlot;
    // AniList's part: the line, the grid in its slot, then the Shows line under it.
    readonly StackPanel anilist = new();
    readonly StackPanel sources = new() { Visibility = Visibility.Collapsed };
    readonly TextBlock sourcesHead = new() { Text = T.ReadSearchFromSources, FontSize = 20, LineHeight = 26, FontWeight = Microsoft.UI.Text.FontWeights.SemiBold };
    readonly StackPanel sourceRows = new() { XYFocusKeyboardNavigation = XYFocusKeyboardNavigationMode.Enabled };
    readonly Dictionary<string, SourceRowView> sourceViews = [];
    readonly FadeLine nothingWords = new(() => new TextBlock { FontSize = 14, LineHeight = 19, TextWrapping = TextWrapping.Wrap, Foreground = Ui.Res<Brush>("HazeBrush") });
    readonly Border nothingLine = new() { Margin = new Thickness(0, 22, 0, 0), Padding = new Thickness(0, 12, 0, 0), BorderThickness = new Thickness(0, 1, 0, 0), Visibility = Visibility.Collapsed };
    readonly StackPanel moreRow = new() { Orientation = Orientation.Horizontal, Spacing = 12, Margin = new Thickness(0, 22, 0, 0), Visibility = Visibility.Collapsed };
    readonly Button moreAnilist = new() { Content = T.ReadSearchMoreAnilist, MinHeight = 32 };
    readonly ProgressRing moreRing = new() { Width = 16, Height = 16, MinWidth = 16, MinHeight = 16, IsActive = false, Visibility = Visibility.Collapsed, VerticalAlignment = VerticalAlignment.Center };
    readonly TextBlock shownWords = new() { FontSize = 14, LineHeight = 19, VerticalAlignment = VerticalAlignment.Center };
    readonly ProgressRing more = new() { IsActive = true, Width = 20, Height = 20, Margin = new Thickness(0, 16, 0, 0), HorizontalAlignment = HorizontalAlignment.Left, Visibility = Visibility.Collapsed };
    readonly Grid other = new() { MinHeight = 52, Margin = new Thickness(0, 22, 0, 0), BorderThickness = new Thickness(0, 1, 0, 0), Visibility = Visibility.Collapsed };
    readonly TextBlock otherWords = new() { Style = Ui.Res<Style>("HazeBodyStyle"), VerticalAlignment = VerticalAlignment.Center };
    readonly TextBlock demoLine = new() { Text = T.SearchDemoOnlyBooks, Style = Ui.Res<Style>("HazeBodyStyle"), Margin = new Thickness(0, 16, 0, 0), TextWrapping = TextWrapping.Wrap, Visibility = Visibility.Collapsed };
    readonly DispatcherTimer settle = new() { Interval = TimeSpan.FromMilliseconds(400) };
    AutoSuggestBox? box;
    string typed = "", anilistNote = "", announced = "", nothingKey = "";
    bool anilistShown = true, hearing;
    bool? browsed;
    // When the words were last asked: a part's placeholders come 600 ms after it.
    DateTimeOffset ranAt = DateTimeOffset.Now;

    /// <summary>A filter's menu as the bar draws it: 32 high, the words in weight 500, no fill until the pointer is over it.</summary>
    static DropDownButton Menu() => new()
    {
        MinHeight = 32,
        MinWidth = 96,
        FontWeight = Microsoft.UI.Text.FontWeights.Medium,
        Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
        BorderBrush = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
        Foreground = Ui.Res<Brush>("HazeBrush"),
    };

    /// <summary>AniList's grid as screen readers meet it: "Series from AniList", each cover's place in it.</summary>
    static ReadGrid Named(ReadGrid grid)
    {
        AutomationProperties.SetName(grid, T.ReadSearchTotalUnknown);
        return grid;
    }

    public ReadSearchPage()
    {
        Background = Ui.Res<Brush>("GroundBrush");
        NavigationCacheMode = NavigationCacheMode.Enabled;
        var root = new Grid { Padding = new Thickness(36, 20, 0, 0), RowSpacing = 12 };
        root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        root.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });

        var head = new StackPanel { Margin = new Thickness(0, 0, 36, 0) };
        AutomationProperties.SetHeadingLevel(title, AutomationHeadingLevel.Level1);
        head.Children.Add(title);
        head.Children.Add(new OfflineLine(reading: true));
        root.Children.Add(head);

        // The scope first, then the filters, which scroll sideways in a narrow window, then Your
        // sources only after a rule, which stays in view; the sort at the end while browsing.
        var bar = new Grid { ColumnSpacing = 8, Margin = new Thickness(0, 0, 36, 0) };
        bar.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        bar.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        bar.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        bar.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        bar.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        bar.Children.Add(new ScopeSwitch { Place = "search", Reading = true, Margin = new Thickness(0, 0, 12, 0) });
        foreach (var button in new[] { format, origin, status, genres, started })
            filterRow.Children.Add(button);
        rule.Background = Ui.Res<Brush>("RuleBrush");
        AutomationProperties.SetAccessibilityView(rule, AccessibilityView.Raw);
        SourcesOnlyLook();
        var filterScroll = new ScrollViewer
        {
            Content = filterRow,
            HorizontalScrollMode = ScrollMode.Auto,
            HorizontalScrollBarVisibility = ScrollBarVisibility.Hidden,
            VerticalScrollMode = ScrollMode.Disabled,
            VerticalScrollBarVisibility = ScrollBarVisibility.Disabled,
            IsTabStop = false,
        };
        HomeShelf.ForwardWheel(filterScroll);
        Grid.SetColumn(filterScroll, 1);
        bar.Children.Add(filterScroll);
        Grid.SetColumn(rule, 2);
        bar.Children.Add(rule);
        Grid.SetColumn(sourcesOnly, 3);
        bar.Children.Add(sourcesOnly);
        Grid.SetColumn(sort, 4);
        bar.Children.Add(sort);
        Grid.SetRow(bar, 1);
        root.Children.Add(bar);
        Menus();

        // AniList's part: the line, the grid, and the Shows line under them.
        line.MinHeight = 19;
        anilist.Children.Add(line);
        anilistSlot = new CoverSlot(ReadPlates.Grid(), grid);
        anilist.Children.Add(anilistSlot);
        other.BorderBrush = Ui.Res<Brush>("FaintRuleBrush");
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
                Children = { new TextBlock { Text = T.SearchShowThem, FontWeight = Microsoft.UI.Text.FontWeights.SemiBold }, new FontIcon { Glyph = "", FontSize = 10 } },
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
        nothingLine.BorderBrush = Ui.Res<Brush>("FaintRuleBrush");
        nothingLine.Child = nothingWords;
        sources.Children.Add(nothingLine);

        // More from AniList: while words are typed, more comes when asked, not on the scroll. The
        // button keeps the focus and stays; a small ring beside it while the page comes.
        shownWords.Foreground = Ui.Res<Brush>("HazeBrush");
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

        body.Children.Add(anilist);
        body.Children.Add(sources);
        body.Children.Add(moreRow);
        body.Children.Add(later);
        body.Children.Add(more);
        results.Children.Add(body);
        results.Children.Add(nothingHost);
        state.Children.Add(results);
        var column = new StackPanel { Padding = new Thickness(0, 4, 36, 48), MaxWidth = ReadingParts.Column + 36, HorizontalAlignment = HorizontalAlignment.Left };
        column.Children.Add(state);
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
        else if (search.Tiles.Count == 0 && !search.Loading && search.Sources is null)
        {
            typed = search.Query.Text;
            Run(search.Query);
        }
        else
            typed = search.Query.Text;
        Menus();
        Show();
    }

    /// <summary>
    /// A new search as the page comes in: what an earlier search left is gone at once, under the
    /// page's own crossfade, and the parts wait for the new answers.
    /// </summary>
    void Fresh()
    {
        browsed = null;
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
        Menus();
        Show();
    }

    void Heard(ReadingPart part)
    {
        if (part is ReadingPart.Search or ReadingPart.Failure)
            Show();
    }

    /// <summary>
    /// Draws the search as it stands. Between browsing and words the results crossfade, as the
    /// grid's line and the sources' part change together.
    /// </summary>
    void Show()
    {
        Other();
        Demo();
        var browsing = Browsing;
        if (browsed is { } before && before != browsing)
        {
            browsed = browsing;
            Crossfade.Swap(results, Draw);
            return;
        }
        browsed = browsing;
        Draw();
    }

    void Draw()
    {
        var search = ctx.Known.Reading.Search;
        if (Browsing)
            ShowBrowse(search);
        else
            ShowWords(search);
    }

    /// <summary>Nothing typed: AniList's covers by the filters, more as the end nears.</summary>
    void ShowBrowse(ReadSearchState search)
    {
        Nothing(null);
        sources.Visibility = Visibility.Collapsed;
        sourceRows.Children.Clear();
        sourceViews.Clear();
        moreRow.Visibility = Visibility.Collapsed;
        later.Visibility = Visibility.Collapsed;
        AnilistShown(true, animate: false);
        more.Visibility = search.HasMore && search.Tiles.Count > 0 ? Visibility.Visible : Visibility.Collapsed;
        if (search.Failure is { } failure && search.Tiles.Count == 0)
        {
            state.Empty(ReadingParts.PlainEmpty(T.ReadSearchFailed, failure.Words(), T.ActionTryAgain, () => Run(search.Query), error: true));
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
            state.Empty(ReadingParts.PlainEmpty(
                T.ReadSearchNothingBrowse,
                any ? T.ReadSearchNothingFiltersLine : T.ReadSearchNothingLine,
                any ? T.ReadSearchClearFilters : null,
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
        more.Visibility = Visibility.Collapsed;
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
            var fresh = Named(new ReadGrid());
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
        note.Children.Add(new TextBlock { Text = T.ReadSearchAnilistNothing, FontSize = 14, LineHeight = 19, FontWeight = Microsoft.UI.Text.FontWeights.SemiBold, TextWrapping = TextWrapping.Wrap });
        note.Children.Add(new TextBlock { Text = AnyFilter(q) ? T.ReadSearchNothingFiltersLine : T.ReadSearchNothingLine, Style = Ui.Res<Style>("HazeCaptionStyle"), TextWrapping = TextWrapping.Wrap });
        AutomationProperties.SetLiveSetting(note, AutomationLiveSetting.Polite);
        return note;
    }

    /// <summary>A message in the results' place: its heading, then its lines.</summary>
    static StackPanel Message(string heading, params string?[] lines)
    {
        var block = new StackPanel { Margin = new Thickness(0, 6, 0, 0), HorizontalAlignment = HorizontalAlignment.Left, MaxWidth = 480 };
        var words = new TextBlock { Text = heading, FontSize = 20, LineHeight = 26, FontWeight = Microsoft.UI.Text.FontWeights.SemiBold, TextWrapping = TextWrapping.Wrap };
        AutomationProperties.SetHeadingLevel(words, AutomationHeadingLevel.Level2);
        block.Children.Add(words);
        foreach (var line in lines.Where(l => l is { Length: > 0 }))
            block.Children.Add(new TextBlock { Text = line, Style = Ui.Res<Style>("HazeBodyStyle"), TextWrapping = TextWrapping.Wrap, Margin = new Thickness(0, 6, 0, 0) });
        return block;
    }

    /// <summary>Nothing from AniList and nothing from any source: the words, the line to try others, and the sources' line.</summary>
    UIElement NothingAnywhere(ReadSearchQuery q, bool sourcesAsked)
    {
        var any = AnyFilter(q);
        var block = Message(T.ReadSearchNothing(q.Text.Trim()), any ? T.ReadSearchNothingFiltersLine : T.ReadSearchNothingLine, sourcesAsked ? T.ReadSearchSourcesNothing : null);
        if (any)
        {
            var clear = Ui.Label(T.ReadSearchClearFilters, (_, _) => ClearFilters());
            clear.Margin = new Thickness(0, 16, 0, 0);
            block.Children.Add(clear);
        }
        return block;
    }

    /// <summary>Your sources only, and none found anything: "Nothing found in your sources" and the line to try other words.</summary>
    UIElement SourcesOnlyNothing() => Message(T.ReadSearchSourcesOnlyNothing, T.ReadSearchNothingLine);

    void ClearFilters() => Run(ReadingStore.EmptyQuery with { Text = Query.Text, SourcesOnly = Query.SourcesOnly });

    /// <summary>
    /// From your sources: a row per source asked, answered or failed, in Your sources order, each
    /// keeping its place as its state changes; the sources that found nothing share one line. New
    /// words keep the rows that show until their answers come. A row that leaves fades out, then
    /// its gap closes. The heading fades in with the first row.
    /// </summary>
    /// <summary>Counts the sources' answers, so a section fading out stays when a later answer has rows.</summary>
    int sourcesTurn;
    bool headShown, sectionLeaving;

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
        if (sources.Visibility == Visibility.Collapsed)
        {
            // A fade's held value outlasts a plain set, so the heading is hidden through Motion too.
            Motion.FadeTo(sourcesHead, 0, TimeSpan.Zero);
            headShown = false;
            sourcesHead.Margin = new Thickness(0, first ? 4 : 34, 0, 0);
            sources.Visibility = Visibility.Visible;
        }
        else if (Math.Abs(sourcesHead.Margin.Top - (first ? 4 : 34)) > 0.5)
        {
            // AniList going or coming with Your sources only: the heading's gap eases with it.
            Motion.Ease(sourcesHead, sourcesHead.Margin.Top, first ? 4 : 34, v => sourcesHead.Margin = new Thickness(0, v, 0, 0));
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
        if (!Browsing || !search.HasMore || search.Loading || search.Tiles.Count == 0)
            return;
        if (scroll.VerticalOffset + scroll.ViewportHeight < scroll.ExtentHeight - scroll.ViewportHeight)
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

    /// <summary>
    /// Your sources only: an outline in the edge colour with haze words while off; on, a check and
    /// the words in the secondary fill, never amber.
    /// </summary>
    void SourcesOnlyLook()
    {
        var words = new TextBlock { Text = T.ReadSearchSourcesOnly, VerticalAlignment = VerticalAlignment.Center };
        sourcesOnly.Content = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6, Children = { sourcesOnlyCheck, words } };
        AutomationProperties.SetName(sourcesOnly, T.ReadSearchSourcesOnly);
        var haze = Ui.Res<Brush>("HazeBrush");
        var edge = Ui.Res<Brush>("EdgeBrush");
        var fill = Ui.Res<Brush>("ControlFillColorSecondaryBrush");
        var fillOver = Ui.Res<Brush>("ControlFillColorTertiaryBrush");
        var text = Ui.Res<Brush>("TextFillColorPrimaryBrush");
        var clear = new SolidColorBrush(Microsoft.UI.Colors.Transparent);
        // i18n-skip: lightweight styling keys of the toggle button
        foreach (var (key, value) in new (string, object)[]
        {
            ("ToggleButtonBackground", clear), ("ToggleButtonBackgroundPointerOver", fillOver), ("ToggleButtonBackgroundPressed", fill),
            ("ToggleButtonForeground", haze), ("ToggleButtonForegroundPointerOver", text), ("ToggleButtonForegroundPressed", text),
            ("ToggleButtonBorderBrush", edge), ("ToggleButtonBorderBrushPointerOver", edge), ("ToggleButtonBorderBrushPressed", edge),
            ("ToggleButtonBackgroundChecked", fill), ("ToggleButtonBackgroundCheckedPointerOver", fillOver), ("ToggleButtonBackgroundCheckedPressed", fill),
            ("ToggleButtonForegroundChecked", text), ("ToggleButtonForegroundCheckedPointerOver", text), ("ToggleButtonForegroundCheckedPressed", text),
            ("ToggleButtonBorderBrushChecked", clear), ("ToggleButtonBorderBrushCheckedPointerOver", clear), ("ToggleButtonBorderBrushCheckedPressed", clear),
        })
            sourcesOnly.Resources[key] = value;
        sourcesOnly.BorderThickness = new Thickness(1);
        sourcesOnly.FontWeight = Microsoft.UI.Text.FontWeights.Medium;
        sourcesOnly.Click += (_, _) =>
        {
            var on = sourcesOnly.IsChecked == true;
            Run(Query with { SourcesOnly = on });
            if (on)
                Tell(T.WindowsReadSearchAnilistHidden);
        };
    }

    // Filters

    static bool AnyFilter(ReadSearchQuery q) =>
        (q.Formats?.Length ?? 0) > 0 || (q.Origins?.Length ?? 0) > 0 || q.Status is not null
        || (q.Genres?.Length ?? 0) > 0 || (q.ExcludeGenres?.Length ?? 0) > 0 || ReadDiscovery.StartedSet(q);

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

    /// <summary>"Manga, Japan, Popular · 1,240 titles": what the browse shows, in the order it shows it.</summary>
    static string BrowseLine(ReadSearchQuery q, uint? total)
    {
        var f = Filters;
        if (Titled is { } titled)
        {
            if (ReadDiscovery.SameAsk(titled.Query, q, f.Sorts.FirstOrDefault()))
                return total is { } count ? ReadingParts.Joined(titled.Line.Text(), T.ReadSearchTotal(count)) : T.ReadSearchBrowseLine(titled.Line.Text());
            // Asked something else since: the row's line is gone for good.
            Titled = null;
        }
        var parts = Labels(ReadDiscovery.FoldedFormats(q.Formats), f.Formats).Concat(Labels(q.Origins, f.Origins)).ToList();
        if (q.Status is { } s)
            parts.AddRange(Labels([s], f.Statuses));
        parts.AddRange(q.Genres ?? []);
        parts.AddRange((q.ExcludeGenres ?? []).Select(T.ReadFilterNotGenre));
        if (ReadDiscovery.StartedSet(q))
            parts.AddRange(Labels([q.Started!], f.Started ?? []));
        parts.Add(SortWords(q.Sort ?? f.Sorts.FirstOrDefault()));
        var what = string.Join(", ", parts);
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

    /// <summary>Each filter's menu, built from the query in force: its name until set, then "Format: Manga" on the secondary fill.</summary>
    void Menus()
    {
        var f = Filters;
        var q = Query;
        // One Manga, which takes one-shots in.
        format.Flyout = Choices(ReadDiscovery.FormatChoices(f.Formats), q.Formats ?? [], key =>
            Run(Query with { Formats = ReadDiscovery.ToggleFormat(Query.Formats, key, ReadDiscovery.FormatChoices(f.Formats).Length != f.Formats.Length) }));
        Say(format, T.ReadFilterFormat, Value(ReadDiscovery.FoldedFormats(q.Formats), f.Formats));
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

        // When the series started: any time, the last 12 months or the last 3 years.
        var startedMenu = new MenuFlyout { Placement = FlyoutPlacementMode.BottomEdgeAlignedLeft };
        var startedKey = ReadDiscovery.StartedSet(q) ? q.Started : null;
        foreach (var choice in f.Started ?? [])
        {
            var anyTime = choice.Key == "any"; // i18n-skip: the engine's key
            var item = new RadioMenuFlyoutItem { Text = choice.Label.Text(), GroupName = "started", IsChecked = anyTime ? startedKey is null : startedKey == choice.Key };
            item.Click += (_, _) => Run(Query with { Started = anyTime ? null : choice.Key });
            startedMenu.Items.Add(item);
        }
        started.Flyout = startedMenu;
        Say(started, T.ReadSearchStarted, startedKey is null ? null : Value([startedKey], f.Started ?? []));

        // Your sources only, with words typed: the rule and the toggle fade in with the first
        // word, and the other filters dim while it is on, as AniList is not asked; Narrator hears why.
        var words = !Browsing;
        foreach (var part in new FrameworkElement[] { rule, sourcesOnly })
        {
            if (words && part.Visibility == Visibility.Collapsed)
            {
                part.Opacity = 0;
                part.Visibility = Visibility.Visible;
                Motion.FadeTo(part, 1, Motion.FadeInTime);
            }
            else if (!words && part.Visibility == Visibility.Visible)
                Motion.FadeOutThen(part, () =>
                {
                    if (!Browsing)
                        return;
                    part.Visibility = Visibility.Collapsed;
                    part.Opacity = 1;
                });
        }
        sourcesOnly.IsChecked = q.SourcesOnly;
        sourcesOnlyCheck.Visibility = q.SourcesOnly ? Visibility.Visible : Visibility.Collapsed;
        sourcesOnly.FontWeight = q.SourcesOnly ? Microsoft.UI.Text.FontWeights.SemiBold : Microsoft.UI.Text.FontWeights.Medium;
        var dimmed = words && q.SourcesOnly;
        var dim = dimmed ? Dimmed : 1;
        if (Math.Abs(filterRow.Opacity - dim) > 0.01)
            Motion.FadeTo(filterRow, dim, Motion.FadeInTime);
        foreach (var button in new[] { format, origin, status, genres, started })
            AutomationProperties.SetHelpText(button, dimmed ? T.WindowsReadSearchFiltersDimmed : "");

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

    /// <summary>A filter's button: its name in haze until set, then "Format: Manga" in the text colour on the secondary fill.</summary>
    static void Say(DropDownButton button, string name, string? value)
    {
        var text = value is null ? name : $"{T.ReadFilterSet(name)} {value}";
        if (button.Content as string != text)
            button.Content = text;
        AutomationProperties.SetName(button, value is null ? name : $"{name}, {value}");
        button.Foreground = Ui.Res<Brush>(value is null ? "HazeBrush" : "TextFillColorPrimaryBrush");
        button.Background = value is null ? new SolidColorBrush(Microsoft.UI.Colors.Transparent) : Ui.Res<Brush>("ControlFillColorSecondaryBrush");
    }
}
