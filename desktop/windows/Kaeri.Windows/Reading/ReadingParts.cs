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

    /// <summary>Library's and Search's grid: the cell, as wide as a row's covers, and the gap between cells.</summary>
    public const double GridCell = HomeWording.PosterWidth, GridGap = HomeWording.Gap;

    /// <summary>The text size the person set in Windows, 1 to 2.25.</summary>
    public static double TextScale
    {
        get
        {
            try
            {
                return (uiSettings ??= new global::Windows.UI.ViewManagement.UISettings()).TextScaleFactor;
            }
            catch (Exception)
            {
                return 1;
            }
        }
    }

    static global::Windows.UI.ViewManagement.UISettings? uiSettings;

    /// <summary>
    /// Very large text: Search's and View all's covers become a one-column list, the cover beside
    /// the title and its line, as on the phones.
    /// </summary>
    public static bool ListText => TextScale >= 2;

    // Covers

    /// <summary>
    /// A book's cover at 2:3 from its URL, rounded and edged, in the raised colour until the art
    /// arrives, which then fades in. Art the app already has shows at once. A series with no
    /// cover at all and a <paramref name="title"/> gets a plate set with its title and the book
    /// glyph, never a plate that looks like it is loading.
    /// </summary>
    public static Border Cover(Ctx ctx, string? url, double width, double radius = 4, string? title = null)
    {
        if (url is null && title is { Length: > 0 })
            return Plate(title, width, radius);
        var image = new Image { Stretch = Stretch.UniformToFill };
        AutomationProperties.SetAccessibilityView(image, AccessibilityView.Raw);
        ArtFade.Watch(image);
        var frame = new Border
        {
            Width = width,
            Height = Math.Round(width * 1.5),
            CornerRadius = new CornerRadius(radius),
            Background = LiveBrush.Of("RaisedBrush"),
            BorderBrush = LiveBrush.Of("EdgeBrush"),
            BorderThickness = new Thickness(1),
            Child = image,
        };
        ctx.Art.Bind(url, art => image.Source = art);
        return frame;
    }

    /// <summary>
    /// The plate of a series with no cover: its title in the show font from the top, the book
    /// glyph at the foot, on the raised colour. Hidden from screen readers, as the caption under it
    /// says the title once.
    /// </summary>
    static Border Plate(string title, double width, double radius)
    {
        var size = Math.Max(12, Math.Round(width / 9));
        var inside = new Grid { Padding = new Thickness(Math.Round(width / 11)) };
        inside.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });
        inside.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        inside.Children.Add(new TextBlock
        {
            Text = title,
            FontFamily = Ui.Res<FontFamily>("ShowFont"),
            FontWeight = FontWeights.Bold,
            FontSize = size,
            LineHeight = Math.Round(size * 1.3),
            LineStackingStrategy = LineStackingStrategy.BlockLineHeight,
            Foreground = LiveBrush.Of("SilkBrush"),
            TextWrapping = TextWrapping.WrapWholeWords,
            TextTrimming = TextTrimming.CharacterEllipsis,
            MaxLines = 5,
            IsTextScaleFactorEnabled = false,
        });
        var glyph = new FontIcon { Glyph = "\uE736", FontSize = 16, Foreground = LiveBrush.Of("HazeBrush"), HorizontalAlignment = HorizontalAlignment.Left };
        Grid.SetRow(glyph, 1);
        inside.Children.Add(glyph);
        AutomationProperties.SetAccessibilityView(inside, AccessibilityView.Raw);
        return new Border
        {
            Width = width,
            Height = Math.Round(width * 1.5),
            CornerRadius = new CornerRadius(radius),
            Background = LiveBrush.Of("RaisedBrush"),
            BorderBrush = LiveBrush.Of("EdgeBrush"),
            BorderThickness = new Thickness(1),
            Child = inside,
        };
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
            Background = LiveBrush.Of("SilkBrush"),
            Width = width * Math.Clamp(progress ?? 0, 0, 1),
        };
        return new Border
        {
            Width = width,
            Height = height,
            CornerRadius = new CornerRadius(height / 2),
            Background = progress is null ? null : LiveBrush.Of("RuleBrush"),
            Child = fill,
        };
    }

    // The cover card

    /// <summary>The height of a card's caption lines: the title's, then the line's.</summary>
    public const double TitleLine = 20, CaptionLine = 16;

    /// <summary>
    /// A series to open: its cover, the title and one line under it, both kept their height so
    /// covers in a row line up. A discovery tile whose series no source has says so as its line,
    /// over two lines when it needs them. With <paramref name="track"/> the silk track sits under
    /// the cover, as Continue has it. <paramref name="placed"/> is a third line in silk, "In Home
    /// library", which wraps rather than cut; <paramref name="placedRoom"/> keeps two lines for it
    /// on a card without one, so a grid's cells are one height. Its menu, on right-click,
    /// Shift+F10 and the menu key, is Details with Add to list, or with
    /// <paramref name="continuing"/> Details with Remove from Continue.
    /// </summary>
    public static Button Card(Ctx ctx, WorkTile tile, double width = HomeWording.PosterWidth, bool track = false, bool continuing = false, string? placed = null, bool placedRoom = false, string? caption = null, bool lists = true)
    {
        var stack = new StackPanel { Width = width };
        stack.Children.Add(Cover(ctx, tile.Cover, width, title: tile.Title));
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
        var line = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6, Margin = new Thickness(0, 2, 0, 0), MinHeight = ReadPlates.Lines(CaptionLine) };
        if (tile.NewChapter)
            line.Children.Add(WaterDot());
        // Search words its own lines: an AniList result's format and origin, a source's hit how much it holds.
        var said = caption ?? Line(tile);
        var words = new TextBlock
        {
            Text = said,
            Style = Ui.Res<Style>("HazeCaptionStyle"),
            TextWrapping = tile.NotInSources ? TextWrapping.WrapWholeWords : TextWrapping.NoWrap,
            MaxLines = tile.NotInSources ? 2 : 1,
            TextTrimming = TextTrimming.CharacterEllipsis,
            MaxWidth = width - (tile.NewChapter ? 12 : 0),
        };
        if (tile.NewChapter)
            words.Foreground = LiveBrush.Of("SilkBrush");
        line.Children.Add(words);
        stack.Children.Add(line);
        if (placed is not null || placedRoom)
        {
            var where = new TextBlock
            {
                Text = placed ?? "",
                Style = Ui.Res<Style>("HazeCaptionStyle"),
                Foreground = LiveBrush.Of("SilkBrush"),
                FontWeight = FontWeights.SemiBold,
                TextWrapping = TextWrapping.WrapWholeWords,
                MaxLines = 2,
                Margin = new Thickness(0, 2, 0, 0),
                MinHeight = placedRoom ? 2 * ReadPlates.Lines(CaptionLine) : 0,
            };
            stack.Children.Add(where);
        }
        var button = new Button
        {
            Style = Ui.Res<Style>("HomeCardButtonStyle"),
            Content = stack,
            // The hover fill reaches past the art by the outset, which the style's margin takes back.
            Width = width + 2 * HomeWording.HoverOutset,
            Tag = tile.Work,
        };
        AutomationProperties.SetName(button, TileName(tile.Title, said, placed));
        button.Click += (_, _) => ctx.Window.OpenSeries(tile.Work);
        button.ContextFlyout = continuing ? ContinueMenu(ctx, tile.Work) : lists ? TileMenu(ctx, tile.Work) : DetailsMenu(ctx, tile.Work);
        return button;
    }

    /// <summary>
    /// A series in Search, in the shows' Search cell: the cover 150 by 225 at the art's radius,
    /// the title kept two lines high so every cell is one height, then the caption in haze.
    /// <paramref name="placed"/> is a third line in silk, "In Home library"; with
    /// <paramref name="placedRoom"/> a card without one keeps its room. The same menu and name as
    /// <see cref="Card"/>.
    /// </summary>
    public static Button SearchCard(Ctx ctx, WorkTile tile, string? caption = null, string? placed = null, bool placedRoom = false, bool lists = true)
    {
        var width = SearchParts.CoverWidth;
        var stack = new StackPanel { Width = width, Spacing = 2 };
        var cover = Cover(ctx, tile.Cover, width, Art.Radius(width), title: tile.Title);
        cover.Margin = new Thickness(0, 0, 0, 6);
        stack.Children.Add(cover);
        stack.Children.Add(new TextBlock
        {
            Text = tile.Title,
            Style = Ui.Res<Style>("ShowTitleStyle"),
            MaxLines = 2,
            TextWrapping = TextWrapping.WrapWholeWords,
            Height = States.CoverTitleHeight,
        });
        var said = caption ?? Line(tile);
        var words = new TextBlock
        {
            Text = said,
            Style = Ui.Res<Style>("HazeCaptionStyle"),
            TextWrapping = tile.NotInSources ? TextWrapping.WrapWholeWords : TextWrapping.NoWrap,
            MaxLines = tile.NotInSources ? 2 : 1,
            TextTrimming = TextTrimming.CharacterEllipsis,
        };
        if (tile.NewChapter)
            words.Foreground = LiveBrush.Of("SilkBrush");
        stack.Children.Add(words);
        if (placed is not null || placedRoom)
            stack.Children.Add(new TextBlock
            {
                Text = placed ?? "",
                Style = Ui.Res<Style>("HazeCaptionStyle"),
                Foreground = LiveBrush.Of("SilkBrush"),
                FontWeight = FontWeights.SemiBold,
                TextWrapping = TextWrapping.WrapWholeWords,
                MaxLines = 2,
                MinHeight = placedRoom ? 2 * ReadPlates.Lines(CaptionLine) : 0,
            });
        var button = new Button
        {
            Style = Ui.Res<Style>("HomeCardButtonStyle"),
            Content = stack,
            // The hover fill reaches past the art by the outset, which the style's margin takes back.
            Width = width + 2 * HomeWording.HoverOutset,
            Tag = tile.Work,
        };
        AutomationProperties.SetName(button, TileName(tile.Title, said, placed));
        button.Click += (_, _) => ctx.Window.OpenSeries(tile.Work);
        button.ContextFlyout = lists ? TileMenu(ctx, tile.Work) : DetailsMenu(ctx, tile.Work);
        return button;
    }

    /// <summary>
    /// A result as a row for very large text: the cover beside the title, its line and where a
    /// source has it, all wrapping. The same menu and name as its card.
    /// </summary>
    public static Button RowCard(Ctx ctx, WorkTile tile, string? placed = null, string? caption = null, bool lists = true)
    {
        var grid = new Grid { ColumnSpacing = 12 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        var cover = Cover(ctx, tile.Cover, 56, title: tile.Title);
        cover.VerticalAlignment = VerticalAlignment.Top;
        grid.Children.Add(cover);
        var words = new StackPanel { Spacing = 2, VerticalAlignment = VerticalAlignment.Center };
        words.Children.Add(new TextBlock { Text = tile.Title, Style = Ui.Res<Style>("ShowTitleStyle"), FontSize = 14, TextWrapping = TextWrapping.WrapWholeWords });
        var said = caption ?? Line(tile);
        if (said.Length > 0)
            words.Children.Add(new TextBlock { Text = said, Style = Ui.Res<Style>("HazeCaptionStyle"), TextWrapping = TextWrapping.WrapWholeWords });
        if (placed is not null)
            words.Children.Add(new TextBlock { Text = placed, Style = Ui.Res<Style>("HazeCaptionStyle"), Foreground = LiveBrush.Of("SilkBrush"), FontWeight = FontWeights.SemiBold, TextWrapping = TextWrapping.WrapWholeWords });
        Grid.SetColumn(words, 1);
        grid.Children.Add(words);
        var button = new Button
        {
            Style = Ui.Res<Style>("HomeCardButtonStyle"),
            Content = grid,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Stretch,
            Tag = tile.Work,
        };
        AutomationProperties.SetName(button, TileName(tile.Title, said, placed));
        button.Click += (_, _) => ctx.Window.OpenSeries(tile.Work);
        button.ContextFlyout = lists ? TileMenu(ctx, tile.Work) : DetailsMenu(ctx, tile.Work);
        return button;
    }

    /// <summary>A tile with no list choices here, as a source's hit, Up next and This week: Details alone.</summary>
    public static MenuFlyout DetailsMenu(Ctx ctx, string work)
    {
        var menu = new MenuFlyout();
        menu.Items.Add(Item(T.HomeDetails, () => ctx.Window.OpenSeries(work)));
        return menu;
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

    /// <summary>A tile's Narrator name: its title, then its caption and the source that has it when there are.</summary>
    public static string TileName(string title, string? line, string? placed) => (line is { Length: > 0 }, placed is { Length: > 0 }) switch
    {
        (true, true) => T.ReadNameTilePlaced(title, line!, placed!),
        (true, false) => T.ReadNameTile(title, line!),
        (false, true) => T.ReadNameTileOnlyPlaced(title, placed!),
        _ => title,
    };

    public static string Joined(params string?[] parts) => string.Join(" · ", parts.Where(p => p is { Length: > 0 }));

    /// <summary>
    /// The line under a cover: Not found in your sources once a series' check found no source,
    /// else what comes next, else its first two facts.
    /// </summary>
    public static string Line(WorkTile tile)
    {
        if (tile.NotInSources)
            return T.HomeNotFound;
        if (tile.Next is { } next)
            return next.Text();
        return ReadDiscovery.ShelfFacts(tile);
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
        OpenSearch(ctx, query);
    }

    /// <summary>
    /// Reading's Search on <paramref name="query"/>, a row's View all: its filters and sort set, and
    /// its first results the row's titles in the same order.
    /// </summary>
    public static void OpenSearch(Ctx ctx, ReadSearchQuery query, Msg? line = null)
    {
        ctx.Known.Reading.RunSearch(query);
        ReadSearchPage.Opening = query;
        ReadSearchPage.Titled = line is null ? null : (query, line);
        ctx.Window.OpenReadSearch(query.Text);
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
/// A shelf of the person's own: the heading with its link, and the cards in a <see cref="CoverStrip"/>
/// that scrolls sideways by whole cards. The wheel moves the page; Shift and a sideways wheel
/// move the strip. The arrows show while the pointer is over the shelf. Tab enters the strip
/// once, the arrow keys move along it.
/// </summary>
sealed partial class ReadShelf : StackPanel
{
    readonly CoverStrip strip;

    public ReadShelf(string title, IEnumerable<UIElement> cards, string? link = null, Action? act = null, double cardWidth = HomeWording.PosterWidth, double artHeight = HomeWording.PosterHeight)
    {
        Margin = new Thickness(0, HomeWording.ShelfGap, 0, 0);
        Children.Add(ReadingParts.ShelfHeading(title, link, act));
        strip = new CoverStrip(title, cardWidth, artHeight);
        strip.Set([.. cards.OfType<FrameworkElement>()]);
        Children.Add(strip);
    }

    /// <summary>New cards in the same strip, which keeps its sideways place and the focus on the card it was on.</summary>
    public void Set(IEnumerable<UIElement> cards) => strip.Set([.. cards.OfType<FrameworkElement>()]);
}

/// <summary>
/// A grid of covers that fills its width with as many cells as fit, 16 apart: Library's cells
/// are as wide as a row's covers, Search's are the shows' Search cells. Tab enters it once; the
/// arrow keys move between covers.
/// </summary>
sealed partial class ReadGrid : ItemsRepeater
{
    readonly bool search;

    public ReadGrid(bool search = false)
    {
        this.search = search;
        Layout = MakeLayout(search);
        TabFocusNavigation = KeyboardNavigationMode.Once;
        XYFocusKeyboardNavigation = XYFocusKeyboardNavigationMode.Enabled;
    }

    /// <summary>
    /// The grid's layout: cells 132 wide that never stretch, so its covers match a row's, 16
    /// apart and 24 between lines; in Search the shows' grid, cells 150 wide, 16 apart and 26
    /// between lines; with very large text, one result per line.
    /// </summary>
    public static Layout MakeLayout(bool search = false) => ReadingParts.ListText
        ? new StackLayout { Spacing = 8 }
        : new UniformGridLayout
        {
            MinItemWidth = search ? SearchParts.CoverWidth : ReadingParts.GridCell,
            MinColumnSpacing = search ? SearchParts.CoverGap : ReadingParts.GridGap,
            MinRowSpacing = search ? SearchParts.LineGap : 24,
            ItemsStretch = UniformGridLayoutItemsStretch.None,
            ItemsJustification = UniformGridLayoutItemsJustification.Start,
        };

    readonly System.Collections.ObjectModel.ObservableCollection<WorkTile> items = [];
    string shownLook = "";
    bool shownList, shownLists;

    /// <summary>
    /// Whether <paramref name="tiles"/> under <paramref name="look"/> are what the grid shows, or
    /// what it shows with more after: then <see cref="Show"/> adds them in place.
    /// </summary>
    public bool Holds(IReadOnlyList<WorkTile> tiles, string look)
    {
        if (ItemTemplate is null || look.Length == 0 || look != shownLook || ReadingParts.ListText != shownList)
            return false;
        if (tiles.Count < items.Count)
            return false;
        for (var i = 0; i < items.Count; i++)
            if (Same(items[i]) != Same(tiles[i]))
                return false;
        return true;
    }

    static string Same(WorkTile tile) => $"{tile.Work}\t{tile.Title}\t{tile.Cover}\t{tile.NotInSources}";

    /// <summary>
    /// Shows <paramref name="tiles"/>, each a card the cell's width. <paramref name="placed"/>
    /// gives a result its third line, "In Home library"; when any has one, every cell keeps room
    /// for it, as the grid's cells are one height. <paramref name="caption"/> words a result's
    /// line; <paramref name="lists"/> gives its menu the lists. <paramref name="look"/> names
    /// what the lines depend on: under the same one, results that only grow are added in place.
    /// </summary>
    public void Show(Ctx ctx, IReadOnlyList<WorkTile> tiles, Func<string, string?>? placed = null, Func<WorkTile, string?>? caption = null, bool lists = true, string look = "")
    {
        var list = ReadingParts.ListText;
        if (lists == shownLists && Holds(tiles, look))
        {
            for (var i = items.Count; i < tiles.Count; i++)
                items.Add(tiles[i]);
            Sizes();
            return;
        }
        shownLook = look;
        shownList = list;
        shownLists = lists;
        var room = placed is not null && tiles.Any(t => placed(t.Work) is not null);
        if (list != Layout is StackLayout)
            Layout = MakeLayout(search);
        ItemTemplate = new TileFactory(ctx, placed, room, caption, lists, list, search, items);
        items.Clear();
        foreach (var tile in tiles)
            items.Add(tile);
        if (ItemsSource != items)
            ItemsSource = items;
    }

    /// <summary>Each cover drawn says how many the grid holds now, "4 of 60".</summary>
    void Sizes()
    {
        for (var i = 0; i < items.Count; i++)
            if (TryGetElement(i) is { } element)
                AutomationProperties.SetSizeOfSet(element, items.Count);
    }

    sealed partial class TileFactory(Ctx ctx, Func<string, string?>? placed, bool room, Func<WorkTile, string?>? caption, bool lists, bool list, bool search, IList<WorkTile> items) : IElementFactory
    {
        public UIElement GetElement(ElementFactoryGetArgs args)
        {
            if (args.Data is not WorkTile tile)
                return new Grid();
            FrameworkElement card = list
                ? ReadingParts.RowCard(ctx, tile, placed?.Invoke(tile.Work), caption?.Invoke(tile), lists)
                : search ? ReadingParts.SearchCard(ctx, tile, caption?.Invoke(tile), placed?.Invoke(tile.Work), room, lists)
                : ReadingParts.Card(ctx, tile, ReadingParts.GridCell, placed: placed?.Invoke(tile.Work), placedRoom: room, caption: caption?.Invoke(tile), lists: lists);
            var at = items.IndexOf(tile);
            if (at >= 0)
            {
                AutomationProperties.SetPositionInSet(card, at + 1);
                AutomationProperties.SetSizeOfSet(card, items.Count);
            }
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
