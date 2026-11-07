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
/// Reading's Home from the engine's view: the spotlight with its amber action, Continue with the
/// silk track, This week by day with a link to Updates, Up next, Releasing now, Because you
/// finished, one genre shelf, then the closing line. A section with nothing in it leaves with its
/// heading. One 948-wide column; posters 132 by 198 at radius 4. Titled Home with reading alone
/// and Read with both.
/// </summary>
public sealed partial class ReadHomePage : Page
{
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
    ReadingHomeView? shown;
    bool demoOnly, hearing;

    public ReadHomePage()
    {
        Background = Ui.Res<Brush>("GroundBrush");
        AutomationProperties.SetHeadingLevel(title, AutomationHeadingLevel.Level1);
        var root = new Grid { Padding = new Thickness(36, 20, 0, 0), RowSpacing = 8 };
        root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        root.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });
        var head = new StackPanel { Margin = new Thickness(0, 0, 36, 0) };
        head.Children.Add(title);
        head.Children.Add(new OfflineLine());
        head.Children.Add(offline);
        root.Children.Add(head);
        scroll.Content = column;
        state.Children.Add(scroll);
        state.Placeholders = Placeholders;
        state.ActivityText = T.StateLoading;
        Grid.SetRow(state, 1);
        root.Children.Add(state);
        Content = root;
        NavigationCacheMode = NavigationCacheMode.Enabled;

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
        };
        Unloaded += (_, _) =>
        {
            ctx.Known.Reading.Changed -= Heard;
            ctx.Known.Medium.Changed -= Titled;
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
            ctx.Known.Reading.LoadHome(refresh: true);
        };
        KeyboardAccelerators.Add(accelerator);
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
        ctx.Known.Reading.LoadHome();
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
                ctx.Known.Reading.LoadHome(refresh: true);
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
    /// Draws the engine's view. The column is made again only when the view changed; the focus
    /// and the scroll stay where they were.
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
        var mark = Ui.MarkFocus(column);
        column.Children.Clear();

        if (home.Spotlight is { } tile && home.Spot is { } spot)
        {
            column.Children.Add(new ReadSpotlightView(ctx, tile, spot));
            if (demoOnly)
                column.Children.Add(new TextBlock { Text = T.HomeDemoOnlyBooks, Style = Ui.Res<Style>("HazeBodyStyle"), Margin = new Thickness(0, 16, 0, 0), TextWrapping = TextWrapping.Wrap });
        }
        if (home.ContinueReading.Length > 0)
            column.Children.Add(new ReadShelf(T.NavContinue, home.ContinueReading.Select(t => (UIElement)ReadingParts.Card(ctx, t, track: true, continuing: true)),
                artHeight: HomeWording.PosterHeight));
        if (home.Week.Length > 0)
            column.Children.Add(Week(home.Week));
        if (home.Pending.Contains(ReadHomePart.Yours))
            column.Children.Add(Waiting());
        if (home.UpNext.Length > 0)
            column.Children.Add(Shelf(T.HomeUpNext, home.UpNext, T.HomeViewAll, () => ReadingParts.OpenList(ctx, LibrarySegmentRead.Planning)));
        if (home.Releasing.Length > 0)
            column.Children.Add(Shelf(T.HomeReadReleasing, home.Releasing, T.HomeSearch, () => ReadingParts.OpenSearch(ctx)));
        if (home.BecauseTitle is { } because && home.Because.Length > 0)
            column.Children.Add(Shelf(T.HomeBecause(because), home.Because));
        if (home.Genre is { } genre && home.GenreShelf.Length > 0)
            column.Children.Add(Shelf(T.HomeMoreGenre(genre), home.GenreShelf, T.HomeSearch, () => ReadingParts.OpenSearch(ctx, genre)));
        if (home.Pending.Contains(ReadHomePart.Discovery) || home.Pending.Contains(ReadHomePart.Because))
            column.Children.Add(Waiting());
        if (home.Failed)
            column.Children.Add(ReadingParts.PlainEmpty(T.HomeFailed,
                ctx.Known.Reading.Failures.TryGetValue(ReadingAbout.Home, out var failure) ? failure.Words() : null,
                T.ActionTryAgain, () => ctx.Known.Reading.LoadHome(refresh: true), error: true));
        if (home.End is { } end)
            column.Children.Add(End(end));

        state.Done();
        Ui.RestoreFocus(column, mark);
    }

    ReadShelf Shelf(string heading, WorkTile[] tiles, string? link = null, Action? act = null) =>
        new(heading, tiles.Select(t => (UIElement)ReadingParts.Card(ctx, t)), link, act);

    /// <summary>A part of Home still on its way: a rail of plates where its shelf will be.</summary>
    static UIElement Waiting() => new Border { Margin = new Thickness(0, HomeWording.ShelfGap, 0, 0), Child = States.Covers(8, rail: true) };

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
            Foreground = Ui.Res<Brush>(day.Today ? "SilkBrush" : "HazeBrush"),
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
            when.Children.Add(new TextBlock { Text = time, FontSize = 12, LineHeight = 16, FontWeight = FontWeights.SemiBold, Foreground = Ui.Res<Brush>("SilkBrush") });
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
        AutomationProperties.SetName(button, ReadingParts.Joined(tile.Title, time, line));
        button.Click += (_, _) => ctx.Window.OpenSeries(tile.Work);
        button.ContextFlyout = ReadingParts.TileMenu(ctx, tile.Work);
        return button;
    }

    // The end

    UIElement End(ReadHomeEnd end)
    {
        var line = new StackPanel { Spacing = 12, Margin = new Thickness(0, 48, 0, 0), HorizontalAlignment = HorizontalAlignment.Center };
        var words = new TextBlock
        {
            Text = end == ReadHomeEnd.NoSources ? T.HomeReadNoSources : T.HomeReadEnd,
            Style = Ui.Res<Style>("HazeBodyStyle"),
            TextAlignment = TextAlignment.Center,
            TextWrapping = TextWrapping.Wrap,
            MaxWidth = 480,
        };
        line.Children.Add(words);
        if (end == ReadHomeEnd.NoSources)
        {
            var add = Ui.Label(T.SourcesAddTitle, (_, _) => ReadingParts.AddSource(ctx, XamlRoot));
            add.HorizontalAlignment = HorizontalAlignment.Center;
            line.Children.Add(add);
        }
        return line;
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

        frame = new Border { CornerRadius = new CornerRadius(16), Background = Ui.Res<Brush>("RaisedBrush") };
        if (spot.Banner is { } banner)
        {
            var image = new Image { Stretch = Stretch.UniformToFill };
            AutomationProperties.SetAccessibilityView(image, AccessibilityView.Raw);
            ArtFade.Watch(image);
            ctx.Art.Bind(banner, source => image.Source = source);
            frame.Child = image;
        }
        art = new Button { Style = Ui.Res<Style>("HomeCardButtonStyle"), Content = frame, CornerRadius = new CornerRadius(16) };
        AutomationProperties.SetName(art, ReadingParts.Joined(tile.Title, T.HomeDetails));
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
                row.Children.Add(new TextBlock { Text = text, Style = Ui.Res<Style>("HazeCaptionStyle"), Foreground = Ui.Res<Brush>("SilkBrush"), TextTrimming = TextTrimming.CharacterEllipsis });
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
