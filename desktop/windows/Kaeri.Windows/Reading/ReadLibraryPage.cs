using Medium = Kaeri.Engine.Core.Medium;
using Microsoft.UI.Xaml.Automation.Peers;
using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Navigation;

namespace Kaeri.Windows.Pages;

/// <summary>
/// Reading's Library: one cover grid per list, AniList's six then Your files, picked from the
/// list menu, in the order of Last read or Title. A cover opens the series; its menu has Details
/// and Add to list.
/// </summary>
public sealed partial class ReadLibraryPage : Page
{
    /// <summary>The list on show, kept from one visit to the next and set by Home's View all.</summary>
    internal static LibrarySegmentRead Showing { get; set; } = LibrarySegmentRead.Reading;

    /// <summary>Title order instead of last read, kept from one visit to the next.</summary>
    static bool byTitle;

    readonly Ctx ctx = Ctx.Current;
    readonly DropDownButton listButton = new() { MinHeight = 32 };
    readonly MenuFlyout listMenu = new() { Placement = FlyoutPlacementMode.BottomEdgeAlignedLeft };
    readonly DropDownButton orderButton = new() { MinHeight = 32 };
    readonly StateHost state = new() { Padding = new Thickness(0, 4, 36, 48) };
    readonly ReadGrid grid = new();
    LibrarySegmentRead? drawn;
    WorkTile[]? drawnTiles;
    bool drawnByTitle, hearing;

    public ReadLibraryPage()
    {
        Background = Ui.Res<Brush>("GroundBrush");
        NavigationCacheMode = NavigationCacheMode.Enabled;
        var root = new Grid { Padding = new Thickness(36, 20, 0, 0), RowSpacing = 12 };
        root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        root.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });

        var head = new StackPanel { Margin = new Thickness(0, 0, 36, 0) };
        var title = new TextBlock { Text = T.NavLibrary, Style = Ui.Res<Style>("PageTitleStyle") };
        AutomationProperties.SetHeadingLevel(title, AutomationHeadingLevel.Level1);
        head.Children.Add(title);
        head.Children.Add(new OfflineLine());
        root.Children.Add(head);

        var bar = new Grid { ColumnSpacing = 12, Margin = new Thickness(0, 0, 36, 0) };
        bar.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        bar.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        bar.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var scope = new ScopeSwitch { Place = "library", Reading = true, Margin = new Thickness(0, 0, 12, 0) };
        bar.Children.Add(scope);
        listButton.Flyout = listMenu;
        listMenu.Opening += (_, _) => FillLists();
        Grid.SetColumn(listButton, 1);
        listButton.HorizontalAlignment = HorizontalAlignment.Left;
        bar.Children.Add(listButton);
        var orders = new MenuFlyout { Placement = FlyoutPlacementMode.BottomEdgeAlignedRight };
        foreach (var (words, title_) in new[] { (T.ReadSortLastRead, false), (T.LibrarySortTitle, true) })
        {
            var item = new RadioMenuFlyoutItem { Text = words, GroupName = "order", IsChecked = byTitle == title_ };
            item.Click += (_, _) =>
            {
                byTitle = title_;
                Order();
                Show();
            };
            orders.Items.Add(item);
        }
        orderButton.Flyout = orders;
        Grid.SetColumn(orderButton, 2);
        bar.Children.Add(orderButton);
        Grid.SetRow(bar, 1);
        root.Children.Add(bar);

        state.Children.Add(grid);
        state.Placeholders = () => States.Covers(12);
        var scroll = new ScrollViewer { VerticalScrollBarVisibility = ScrollBarVisibility.Auto, IsTabStop = false, Content = state };
        Grid.SetRow(scroll, 2);
        root.Children.Add(scroll);
        Content = root;

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
        if (e.Parameter is LibrarySegmentRead list)
            Showing = list;
        Order();
        // Every list, so the list menu has its counts; the one on show first.
        ctx.Known.Reading.LoadList(Showing);
        foreach (var other in ReadingParts.Lists.Where(l => l != Showing))
            ctx.Known.Reading.LoadList(other);
        drawn = null;
        Show();
    }

    void Heard(ReadingPart part)
    {
        if (part is ReadingPart.Library or ReadingPart.Failure)
            Show();
    }

    int Count(LibrarySegmentRead list) => ctx.Known.Reading.Lists.TryGetValue(list, out var tiles) ? tiles.Length : 0;

    /// <summary>The list menu, each list with its count, the one on show checked.</summary>
    void FillLists()
    {
        listMenu.Items.Clear();
        foreach (var list in ReadingParts.Lists)
        {
            if (list == LibrarySegmentRead.YourFiles)
                listMenu.Items.Add(new MenuFlyoutSeparator());
            var item = new RadioMenuFlyoutItem { Text = T.ReadLibraryListValue(ReadingParts.List(list), Count(list)), GroupName = "list", IsChecked = list == Showing };
            item.Click += (_, _) =>
            {
                if (Showing == list)
                    return;
                Showing = list;
                ctx.Known.Reading.LoadList(list);
                Show();
            };
            listMenu.Items.Add(item);
        }
    }

    void Order()
    {
        var words = byTitle ? T.LibrarySortTitle : T.ReadSortLastRead;
        orderButton.Content = words;
        AutomationProperties.SetName(orderButton, T.LibrarySortButton(words));
        foreach (var item in ((MenuFlyout)orderButton.Flyout).Items.OfType<RadioMenuFlyoutItem>())
            item.IsChecked = item.Text == words;
    }

    void Show()
    {
        var list = Showing;
        var name = ReadingParts.List(list);
        listButton.Content = name;
        AutomationProperties.SetName(listButton, T.ReadLibraryListValue(name, Count(list)));
        var reading = ctx.Known.Reading;
        if (!reading.Lists.TryGetValue(list, out var tiles))
        {
            if (reading.Failures.TryGetValue(ReadingAbout.Library, out var failure))
                state.Empty(ReadingParts.PlainEmpty(T.LibraryListLoadFailed(name), failure.Words(), T.ActionTryAgain, () =>
                {
                    state.Wait();
                    reading.LoadList(list);
                }, error: true));
            else
                state.Wait();
            drawn = null;
            return;
        }
        if (drawn == list && drawnTiles == tiles && drawnByTitle == byTitle)
            return;
        var changedList = drawn != list;
        drawn = list;
        drawnTiles = tiles;
        drawnByTitle = byTitle;
        if (tiles.Length == 0)
        {
            state.Empty(list == LibrarySegmentRead.YourFiles
                ? ReadingParts.PlainEmpty(T.ReadLibraryFilesEmptyTitle, T.ReadLibraryFilesEmptyLine, T.ReadLibraryFilesAdd, () => ReadingParts.AddSource(ctx, XamlRoot))
                : ReadingParts.PlainEmpty(T.ReadLibraryEmpty(name), T.ReadLibraryEmptyLine, T.HomeSearch, () => ReadingParts.OpenSearch(ctx)));
            return;
        }
        var sorted = byTitle
            ? tiles.OrderBy(t => t.Title, StringComparer.Create(System.Globalization.CultureInfo.CurrentCulture, System.Globalization.CompareOptions.IgnoreCase)).ToList()
            : tiles.ToList();
        var mark = changedList ? null : Ui.MarkFocus(grid);
        grid.Show(ctx, sorted);
        AutomationProperties.SetName(grid, T.ReadLibraryListValue(name, tiles.Length));
        state.Done();
        if (changedList)
            Motion.FadeIn(grid);
        else
            Ui.RestoreFocus(grid, mark);
    }
}
