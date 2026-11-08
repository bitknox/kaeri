using Medium = Kaeri.Engine.Core.Medium;
using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;

namespace Kaeri.Windows.Pages;

/// <summary>
/// What reading's places share: a cover from a URL with the art fade, a series' cover card, a
/// shelf with its heading and link, the plain empty state with no picture, and the words for
/// lists and chapter states.
/// </summary>
static class ReadingParts
{
    /// <summary>Home's column, as the board draws it.</summary>
    public const double Column = 948;

    /// <summary>Library's and Search's grid: the smallest cell and the gap between cells.</summary>
    public const double GridCell = 128, GridGap = 16;

    // Covers

    /// <summary>
    /// A book's cover at 2:3 from its URL, rounded and edged, in the raised colour until the art
    /// arrives, which then fades in. Art the app already has shows at once.
    /// </summary>
    public static Border Cover(Ctx ctx, string? url, double width, double radius = 4)
    {
        var image = new Image { Stretch = Stretch.UniformToFill };
        AutomationProperties.SetAccessibilityView(image, AccessibilityView.Raw);
        ArtFade.Watch(image);
        var frame = new Border
        {
            Width = width,
            Height = Math.Round(width * 1.5),
            CornerRadius = new CornerRadius(radius),
            Background = Ui.Res<Brush>("RaisedBrush"),
            BorderBrush = Ui.Res<Brush>("EdgeBrush"),
            BorderThickness = new Thickness(1),
            Child = image,
        };
        ctx.Art.Bind(url, art => image.Source = art);
        return frame;
    }

    /// <summary>The water dot beside new things.</summary>
    public static Ellipse WaterDot(double size = 6)
    {
        var dot = Ui.Dot(Colours.Water);
        dot.Width = dot.Height = size;
        AutomationProperties.SetAccessibilityView(dot, AccessibilityView.Raw);
        return dot;
    }

    /// <summary>A silk track on the rule, <paramref name="progress"/> of the way along.</summary>
    public static Border Track(double width, double? progress, double height = 3)
    {
        var fill = new Border
        {
            Height = height,
            HorizontalAlignment = HorizontalAlignment.Left,
            CornerRadius = new CornerRadius(height / 2),
            Background = Ui.Res<Brush>("SilkBrush"),
            Width = width * Math.Clamp(progress ?? 0, 0, 1),
        };
        return new Border
        {
            Width = width,
            Height = height,
            CornerRadius = new CornerRadius(height / 2),
            Background = progress is null ? null : Ui.Res<Brush>("RuleBrush"),
            Child = fill,
        };
    }

    // The cover card

    /// <summary>
    /// A series to open: its cover, the title and one line under it. With <paramref name="track"/>
    /// the silk track sits under the cover, as Continue has it. Its menu, on right-click, Shift+F10
    /// and the menu key, is Details with Add to list, or with <paramref name="continuing"/> Details
    /// with Remove from Continue.
    /// </summary>
    public static Button Card(Ctx ctx, WorkTile tile, double width = HomeWording.PosterWidth, bool track = false, bool continuing = false)
    {
        var stack = new StackPanel { Width = width };
        stack.Children.Add(Cover(ctx, tile.Cover, width));
        if (track)
            stack.Children.Add(new Border
            {
                Margin = new Thickness(0, 6, 0, 0),
                Child = Track(width, tile.Progress is { } p && !tile.NewChapter ? p : null),
            });
        stack.Children.Add(new TextBlock
        {
            Text = tile.Title,
            Style = Ui.Res<Style>("ShowTitleStyle"),
            FontSize = 14,
            MaxLines = 1,
            TextWrapping = TextWrapping.NoWrap,
            TextTrimming = TextTrimming.CharacterEllipsis,
            Margin = new Thickness(0, track ? 6 : 8, 0, 0),
        });
        var line = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6, Margin = new Thickness(0, 2, 0, 0) };
        if (tile.NewChapter)
            line.Children.Add(WaterDot());
        var words = new TextBlock
        {
            Text = Line(tile),
            Style = Ui.Res<Style>("HazeCaptionStyle"),
            TextWrapping = TextWrapping.NoWrap,
            TextTrimming = TextTrimming.CharacterEllipsis,
            MaxWidth = width - (tile.NewChapter ? 12 : 0),
        };
        if (tile.NewChapter)
            words.Foreground = Ui.Res<Brush>("SilkBrush");
        line.Children.Add(words);
        stack.Children.Add(line);
        var button = new Button
        {
            Style = Ui.Res<Style>("HomeCardButtonStyle"),
            Content = stack,
            Width = width,
            Tag = tile.Work,
        };
        AutomationProperties.SetName(button, Joined(tile.Title, Line(tile)));
        button.Click += (_, _) => ctx.Window.OpenSeries(tile.Work);
        button.ContextFlyout = continuing ? ContinueMenu(ctx, tile.Work) : TileMenu(ctx, tile.Work);
        return button;
    }

    /// <summary>A tile's menu: Details, and Add to list with the lists a series can be put on.</summary>
    public static MenuFlyout TileMenu(Ctx ctx, string work)
    {
        var menu = new MenuFlyout();
        menu.Items.Add(Item(T.HomeDetails, () => ctx.Window.OpenSeries(work)));
        var lists = new MenuFlyoutSubItem { Text = T.SeriesAddToList };
        foreach (var list in Statuses)
            lists.Items.Add(Item(List(list), () => ctx.Known.Reading.SetStatus(work, list)));
        menu.Items.Add(lists);
        return menu;
    }

    /// <summary>A Continue tile's menu: Details, and Remove from Continue.</summary>
    public static MenuFlyout ContinueMenu(Ctx ctx, string work)
    {
        var menu = new MenuFlyout();
        menu.Items.Add(Item(T.HomeDetails, () => ctx.Window.OpenSeries(work)));
        menu.Items.Add(Item(T.HomeRemoveContinue, () => ctx.Known.Reading.HideFromContinue(work)));
        return menu;
    }

    public static MenuFlyoutItem Item(string text, Action act)
    {
        var item = new MenuFlyoutItem { Text = text };
        item.Click += (_, _) => act();
        return item;
    }

    // Headings and empty states

    /// <summary>A shelf's heading with its link at the end, as Home's shelves have it.</summary>
    public static Grid ShelfHeading(string title, string? link = null, Action? act = null)
    {
        var head = new Grid { Margin = new Thickness(0, 0, 0, 12) };
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var heading = new TextBlock
        {
            Text = title,
            FontSize = 20,
            LineHeight = 28,
            FontWeight = FontWeights.SemiBold,
            VerticalAlignment = VerticalAlignment.Bottom,
            TextWrapping = TextWrapping.NoWrap,
            TextTrimming = TextTrimming.CharacterEllipsis,
            Margin = new Thickness(0, 0, 12, 0),
        };
        AutomationProperties.SetHeadingLevel(heading, AutomationHeadingLevel.Level2);
        head.Children.Add(heading);
        if (link is not null && act is not null)
        {
            var button = new HyperlinkButton { Content = link, Style = Ui.Res<Style>("SilkLinkStyle"), VerticalAlignment = VerticalAlignment.Bottom };
            button.Click += (_, _) => act();
            Grid.SetColumn(button, 1);
            head.Children.Add(button);
        }
        return head;
    }

    /// <summary>
    /// An empty place with no picture: a title, a line and at most one button where the content
    /// would be, from the left, as the reading board has it. A failure has the error glyph.
    /// </summary>
    public static StackPanel PlainEmpty(string title, string? line, string? button = null, Action? act = null, bool error = false)
    {
        var panel = new StackPanel { Spacing = 4, MaxWidth = 480, HorizontalAlignment = HorizontalAlignment.Left, Margin = new Thickness(0, 24, 0, 24) };
        var heading = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        if (error)
            heading.Children.Add(States.ErrorGlyph());
        var text = new TextBlock { Text = title, Style = Ui.Res<Style>("SubtitleTextBlockStyle"), TextWrapping = TextWrapping.WrapWholeWords };
        AutomationProperties.SetHeadingLevel(text, AutomationHeadingLevel.Level2);
        heading.Children.Add(text);
        panel.Children.Add(heading);
        if (line is { Length: > 0 })
            panel.Children.Add(Ui.Haze(line));
        if (button is not null && act is not null)
        {
            var b = new Button { Content = button, MinHeight = 32, Margin = new Thickness(0, 12, 0, 0) };
            b.Click += (_, _) => act();
            panel.Children.Add(b);
        }
        return panel;
    }

    // Words

    public static string Joined(params string?[] parts) => string.Join(" · ", parts.Where(p => p is { Length: > 0 }));

    /// <summary>The line under a cover: what comes next, else its facts joined.</summary>
    public static string Line(WorkTile tile)
    {
        if (tile.Next is { } next)
            return next.Text();
        if (tile.FormatLabel is { } label && (tile.Facts?.Length ?? 0) == 0)
            return label;
        return tile.Facts.Joined();
    }

    public static string List(LibrarySegmentRead list) => list switch
    {
        LibrarySegmentRead.Reading => T.ReadListReading,
        LibrarySegmentRead.Planning => T.ReadListPlanning,
        LibrarySegmentRead.Completed => T.ReadListCompleted,
        LibrarySegmentRead.Paused => T.ReadListPaused,
        LibrarySegmentRead.Dropped => T.ReadListDropped,
        LibrarySegmentRead.Rereading => T.ReadListRereading,
        _ => T.ReadListYourFiles,
    };

    /// <summary>AniList's six lists, then Your files.</summary>
    public static readonly LibrarySegmentRead[] Lists =
    [
        LibrarySegmentRead.Reading, LibrarySegmentRead.Planning, LibrarySegmentRead.Completed, LibrarySegmentRead.Paused,
        LibrarySegmentRead.Dropped, LibrarySegmentRead.Rereading, LibrarySegmentRead.YourFiles,
    ];

    /// <summary>The lists a series can be put on.</summary>
    public static readonly LibrarySegmentRead[] Statuses = Lists[..6];

    /// <summary>A chapter's state in words, or null for an unread one with nothing to say.</summary>
    public static string? ChapterState(ChapterRow row)
    {
        switch (row.Download)
        {
            case ChapterDownloadState.Queued:
                return T.ChapterStateQueued;
            case ChapterDownloadState.WaitsForWifi:
                return T.ChapterStateWifi;
            case ChapterDownloadState.Downloading d:
                return T.ChapterStateDownloading(d.DoneValue, d.Total);
            case ChapterDownloadState.Failed f:
                return T.ChapterStateFailed(f.Failure.Words());
        }
        return row.State switch
        {
            ChapterReadState.InProgress { Of: { } of } p => T.ChapterStatePageOf(p.Page, of),
            ChapterReadState.InProgress p => T.ChapterStatePage(p.Page),
            ChapterReadState.Read => T.ChapterStateRead,
            _ => row.NewChapter ? T.ChapterStateNew : null,
        };
    }

    static DateTimeOffset Local(long unix) => DateTimeOffset.FromUnixTimeSeconds(unix).ToLocalTime();

    /// <summary>"Today", "Yesterday", else "Monday 3 November".</summary>
    public static string Day(long unix)
    {
        var day = Local(unix).Date;
        var today = DateTime.Today;
        if (day == today)
            return T.ChapterUpdatesToday;
        if (day == today.AddDays(-1))
            return T.ChapterUpdatesYesterday;
        return day.ToString("dddd d MMMM", System.Globalization.CultureInfo.CurrentCulture);
    }

    /// <summary>A weekday's name, for This week's columns.</summary>
    public static string Weekday(long unix) =>
        Local(unix).ToString("dddd", System.Globalization.CultureInfo.CurrentCulture);

    /// <summary>The time of day in the viewer's clock.</summary>
    public static string Clock(long unix) =>
        Local(unix).ToString("t", System.Globalization.CultureInfo.CurrentCulture);

    /// <summary>"12 min ago".</summary>
    public static string Ago(long unix) =>
        Wording.Ago(Math.Max(0, DateTimeOffset.UtcNow.ToUnixTimeSeconds() - unix));

    /// <summary>The main action's words, or null where there is nothing to read.</summary>
    public static string? Action(SeriesAction? action) => action switch
    {
        SeriesAction.Read r => T.ActionReadChapter(r.Label),
        SeriesAction.Continue c => T.ActionContinueChapter(c.Label, c.Page),
        SeriesAction.ContinuePart { Percent: { } percent } p => T.ActionContinuePart(p.Label, percent),
        SeriesAction.ContinuePart p => T.ActionContinuePartPlain(p.Label),
        SeriesAction.ReadPart r => T.ActionReadPart(r.Label),
        SeriesAction.ReadAgain => T.ActionReadAgain,
        _ => null,
    };

    /// <summary>
    /// Does the main action: the reader at its chapter. Read again and no source are asked on the
    /// series' page, so those open it.
    /// </summary>
    public static void Perform(Ctx ctx, SeriesAction? action, string work)
    {
        switch (action)
        {
            case SeriesAction.Read r:
                ctx.Window.OpenReader(work, r.Chapter);
                break;
            case SeriesAction.Continue c:
                ctx.Window.OpenReader(work, c.Chapter);
                break;
            case SeriesAction.ContinuePart p:
                ctx.Window.OpenReader(work, p.Chapter);
                break;
            case SeriesAction.ReadPart r:
                ctx.Window.OpenReader(work, r.Chapter);
                break;
            default:
                ctx.Window.OpenSeries(work);
                break;
        }
    }

    // Moving between places

    /// <summary>
    /// Reading's Search, with <paramref name="genre"/> set when given: the search is asked first,
    /// so the page opens on it.
    /// </summary>
    public static void OpenSearch(Ctx ctx, string? genre = null)
    {
        var query = ReadingStore.EmptyQuery;
        if (genre is not null)
            query = query with { Genres = [genre] };
        ctx.Known.Reading.RunSearch(query);
        ReadSearchPage.Opening = query;
        Open(ctx, "search", ctx.Window.SearchReading);
    }

    /// <summary>Reading's Library at <paramref name="list"/>.</summary>
    public static void OpenList(Ctx ctx, LibrarySegmentRead list)
    {
        ReadLibraryPage.Showing = list;
        Open(ctx, "library", ctx.Window.LibraryReading);
    }

    /// <summary>A place that follows the medium: as it is with reading alone, else on its Reading scope.</summary>
    static void Open(Ctx ctx, string tag, bool onReading)
    {
        if (!ctx.Known.Medium.Both || onReading)
            ctx.Window.Go(tag);
        else
            ctx.Window.SetScope(tag, true);
    }

    /// <summary>Add a source, the choice of a folder, a server or a plugin store.</summary>
    public static void AddSource(Ctx ctx, XamlRoot root) => _ = new AddBookSourceDialog(ctx, root).ShowAsync();
}

/// <summary>
/// A shelf: the heading with its link, and the cards in a strip that scrolls sideways by whole
/// cards. The wheel moves the page; Shift and a sideways wheel move the strip. The arrows show
/// while the pointer is over the shelf. Tab enters the strip once, the arrow keys move along it.
/// </summary>
sealed partial class ReadShelf : Grid
{
    readonly ScrollViewer scroll = new()
    {
        HorizontalScrollBarVisibility = ScrollBarVisibility.Hidden,
        HorizontalScrollMode = ScrollMode.Enabled,
        IsHorizontalRailEnabled = true,
        VerticalScrollMode = ScrollMode.Disabled,
        VerticalScrollBarVisibility = ScrollBarVisibility.Disabled,
        IsTabStop = false,
    };
    readonly StackPanel strip = new()
    {
        Orientation = Orientation.Horizontal,
        Spacing = HomeWording.Gap,
        TabFocusNavigation = KeyboardNavigationMode.Once,
        XYFocusKeyboardNavigation = XYFocusKeyboardNavigationMode.Enabled,
    };
    readonly Button previous, next;
    readonly double cardWidth;
    bool inside;

    public ReadShelf(string title, IEnumerable<UIElement> cards, string? link = null, Action? act = null, double cardWidth = HomeWording.PosterWidth, double artHeight = HomeWording.PosterHeight)
    {
        this.cardWidth = cardWidth;
        Margin = new Thickness(0, HomeWording.ShelfGap, 0, 0);
        RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        Children.Add(ReadingParts.ShelfHeading(title, link, act));
        foreach (var card in cards)
            strip.Children.Add(card);
        AutomationProperties.SetName(strip, title);
        scroll.Content = strip;
        HomeShelf.ForwardWheel(scroll);
        scroll.ViewChanged += (_, _) => Arrows();
        scroll.SizeChanged += (_, _) => Arrows();
        SetRow(scroll, 1);
        Children.Add(scroll);
        previous = Arrow("", T.HomePrevious, false, artHeight);
        next = Arrow("", T.HomeNext, true, artHeight);
        Children.Add(previous);
        Children.Add(next);
        Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent);
        PointerEntered += (_, _) => { inside = true; Arrows(); };
        PointerExited += (_, _) => { inside = false; Arrows(); };
    }

    Button Arrow(string glyph, string name, bool forward, double artHeight)
    {
        var button = new Button
        {
            Width = 36,
            Height = 36,
            Padding = new Thickness(0),
            CornerRadius = new CornerRadius(18),
            HorizontalAlignment = forward ? HorizontalAlignment.Right : HorizontalAlignment.Left,
            VerticalAlignment = VerticalAlignment.Top,
            Margin = new Thickness(forward ? 0 : -18, artHeight / 2 - 18, forward ? -18 : 0, 0),
            Background = Ui.Res<Brush>("RaisedBrush"),
            Content = new FontIcon { Glyph = glyph, FontSize = 14 },
            Opacity = 0,
            IsHitTestVisible = false,
            IsTabStop = false,
        };
        Canvas.SetZIndex(button, 5);
        AutomationProperties.SetName(button, name);
        SetRow(button, 1);
        button.Click += (_, _) =>
        {
            var to = ShelfPaging.Target(scroll.HorizontalOffset, scroll.ViewportWidth, scroll.ExtentWidth, cardWidth, strip.Children.Count, forward);
            Motion.Ease(scroll, scroll.HorizontalOffset, to, v => scroll.ChangeView(v, null, null, true));
        };
        return button;
    }

    void Arrows()
    {
        Set(previous, inside && ShelfPaging.Can(scroll.HorizontalOffset, scroll.ViewportWidth, scroll.ExtentWidth, false));
        Set(next, inside && ShelfPaging.Can(scroll.HorizontalOffset, scroll.ViewportWidth, scroll.ExtentWidth, true));
    }

    static void Set(Button arrow, bool shown)
    {
        arrow.IsHitTestVisible = shown;
        if (shown == arrow.Opacity > 0.5)
            return;
        Motion.Fade(arrow, shown ? 1 : 0);
    }
}

/// <summary>
/// A grid of covers that fills its width with as many cells of at least 128 as fit, 16 apart,
/// as Library and Search draw them. Tab enters it once; the arrow keys move between covers.
/// </summary>
sealed partial class ReadGrid : ItemsRepeater
{
    public ReadGrid()
    {
        Layout = new UniformGridLayout
        {
            MinItemWidth = ReadingParts.GridCell,
            MinColumnSpacing = ReadingParts.GridGap,
            MinRowSpacing = 24,
            ItemsStretch = UniformGridLayoutItemsStretch.Fill,
        };
        TabFocusNavigation = KeyboardNavigationMode.Once;
        XYFocusKeyboardNavigation = XYFocusKeyboardNavigationMode.Enabled;
    }

    /// <summary>Shows <paramref name="tiles"/>, each a card the cell's width.</summary>
    public void Show(Ctx ctx, IReadOnlyList<WorkTile> tiles)
    {
        ItemTemplate = new TileFactory(ctx);
        ItemsSource = tiles.ToList();
    }

    sealed partial class TileFactory(Ctx ctx) : IElementFactory
    {
        public UIElement GetElement(ElementFactoryGetArgs args)
        {
            if (args.Data is not WorkTile tile)
                return new Grid();
            var card = ReadingParts.Card(ctx, tile, ReadingParts.GridCell);
            // The cell's width: the cover keeps 2:3 at whatever the layout gives it.
            card.Width = double.NaN;
            card.HorizontalAlignment = HorizontalAlignment.Stretch;
            card.SizeChanged += (_, e) =>
            {
                if (card.Content is StackPanel { Children: [Border cover, ..] } stack && e.NewSize.Width > 0)
                {
                    stack.Width = e.NewSize.Width;
                    cover.Width = e.NewSize.Width;
                    cover.Height = Math.Round(e.NewSize.Width * 1.5);
                }
            };
            return card;
        }

        public void RecycleElement(ElementFactoryRecycleArgs args) { }
    }
}

/// <summary>
/// Shows or Reading, with both media on: first in Library's and Search's content. With
/// <see cref="Place"/> set it swaps the window's page for the other scope's; without, it tells
/// its own page through <see cref="Switched"/>. Hidden unless both media are on.
/// </summary>
public sealed partial class ScopeSwitch : UserControl
{
    readonly Ctx ctx = Ctx.Current;
    readonly SelectorBar bar = new() { Margin = new Thickness(-8, 0, 0, 0) };
    readonly SelectorBarItem shows = new() { Text = T.LibraryScopeShows };
    readonly SelectorBarItem reading = new() { Text = T.LibraryScopeReading };
    bool quiet, hearing;

    /// <summary>"library" or "search": the window place it switches. Empty for a page's own switch.</summary>
    public string Place { get; set; } = "";

    /// <summary>Which side the page it sits on shows.</summary>
    public bool Reading { get; set; }

    /// <summary>The page's own switch moved: true for Reading.</summary>
    public event Action<bool>? Switched;

    public ScopeSwitch()
    {
        bar.Items.Add(shows);
        bar.Items.Add(reading);
        AutomationProperties.SetName(bar, T.LibraryScopeLabel);
        Content = bar;
        Visibility = Visibility.Collapsed;
        bar.SelectionChanged += OnChanged;
        Loaded += (_, _) =>
        {
            if (!hearing)
                ctx.Known.Medium.Changed += Apply;
            hearing = true;
            Apply();
        };
        Unloaded += (_, _) =>
        {
            ctx.Known.Medium.Changed -= Apply;
            hearing = false;
        };
    }

    void Apply()
    {
        quiet = true;
        (Reading ? reading : shows).IsSelected = true;
        quiet = false;
        Visibility = ctx.Known.Medium.Both ? Visibility.Visible : Visibility.Collapsed;
    }

    void OnChanged(SelectorBar sender, SelectorBarSelectionChangedEventArgs args)
    {
        if (quiet)
            return;
        var now = bar.SelectedItem == reading;
        if (now == Reading)
            return;
        if (Place.Length > 0)
        {
            ctx.Window.SetScope(Place, now);
            // This page stays the other side's: back to its own, in case the window kept it.
            quiet = true;
            (Reading ? reading : shows).IsSelected = true;
            quiet = false;
            return;
        }
        Reading = now;
        Switched?.Invoke(now);
    }
}
