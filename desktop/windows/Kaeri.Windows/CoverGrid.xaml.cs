using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Animation;

namespace Kaeri.Windows;

public sealed partial class CoverGrid : UserControl
{
    public CoverGrid()
    {
        InitializeComponent();
    }

    /// <summary>A single row that scrolls sideways, for a rail; the grid wraps.</summary>
    internal bool Rail
    {
        set
        {
            if (value)
                Items.Layout = new StackLayout { Orientation = Orientation.Horizontal, Spacing = 16 };
        }
    }

    /// <summary>The covers on screen, changed in place by <see cref="Show"/>.</summary>
    readonly System.Collections.ObjectModel.ObservableCollection<CoverItem> shown = [];

    /// <summary>
    /// Shows <paramref name="entries"/> with the fewest changes: a title that stays keeps its
    /// cell, its art and its focus, and takes newer words where they changed; a title that left
    /// goes; a new one is made where it belongs. A list that changes nothing moves nothing, and
    /// no cell flashes back to its placeholder colour while its art is bound again.
    /// </summary>
    internal void Show(IReadOnlyList<Entry> entries)
    {
        if (Items.ItemsSource != shown)
            Items.ItemsSource = shown;
        var was = new Dictionary<uint, CoverItem>();
        foreach (var item in shown)
            was.TryAdd(item.Id, item);
        var next = new List<CoverItem>(entries.Count);
        var seen = new HashSet<uint>();
        foreach (var entry in entries)
        {
            // A title listed twice is shown once, as a cell is one title.
            if (!seen.Add(entry.Id))
                continue;
            if (was.TryGetValue(entry.Id, out var old))
            {
                if (!old.Entry.Equals(entry))
                    old.Take(new CoverItem(entry));
                next.Add(old);
            }
            else
                next.Add(new CoverItem(entry).Load(Ctx.Current.Art));
        }
        Drawn.Arrange(shown, next);
#if KAERI_TESTING
        // --show=hover: the first cover's preview open, as if the pointer rested on it.
        if (Ctx.Current.Options.Show == "hover" && next.Count > 0 && !previewed)
        {
            previewed = true;
            _ = Task.Delay(3000).ContinueWith(_ => DispatcherQueue.TryEnqueue(() =>
            {
                if (Below(Items).OfType<ItemContainer>().FirstOrDefault(c => c.Tag is uint) is { } first)
                    HoverPreview.Show(first, (uint)first.Tag);
            }));
        }
#endif
    }

    /// <summary>More of the same list after what shows, such as Search's next page.</summary>
    internal void Append(IReadOnlyList<Entry> entries)
    {
        if (Items.ItemsSource != shown)
            Items.ItemsSource = shown;
        var have = shown.Select(c => c.Id).ToHashSet();
        foreach (var entry in entries)
            if (have.Add(entry.Id))
                shown.Add(new CoverItem(entry).Load(Ctx.Current.Art));
    }

    /// <summary>How many covers show.</summary>
    internal int Count => shown.Count;

    /// <summary>Whether the grid shows any covers now.</summary>
    internal bool HasCovers => shown.Count > 0;

#if KAERI_TESTING
    bool previewed;
#endif

    void OnInvoked(ItemsView sender, ItemsViewItemInvokedEventArgs e)
    {
        if (e.InvokedItem is not CoverItem item)
            return;
        // The cover flies to the title page's (a connected animation), when animations are on.
        if (Motion.Enabled && CoverOf(item) is { } cover)
            ConnectedAnimationService.GetForCurrentView().PrepareToAnimate(Pages.TitlePage.CoverAnimation, cover);
        Ctx.Current.OpenTitle(item.Id);
    }

    static IEnumerable<DependencyObject> Below(DependencyObject root)
    {
        var count = VisualTreeHelper.GetChildrenCount(root);
        for (var i = 0; i < count; i++)
        {
            var child = VisualTreeHelper.GetChild(root, i);
            yield return child;
            foreach (var deeper in Below(child))
                yield return deeper;
        }
    }

    /// <summary>The cover art of the shown container for <paramref name="item"/>, if it is realised.</summary>
    UIElement? CoverOf(CoverItem item)
    {
        var container = Below(Items).OfType<ItemContainer>().FirstOrDefault(c => c.DataContext == item);
        return container is null ? null : Below(container).OfType<FrameworkElement>().FirstOrDefault(f => f.Tag as string == "cover");
    }

    /// <summary>
    /// A container is up: the pointer resting on it opens the preview of the title it holds now,
    /// which a recycled container changes, and its menu is made as it opens, for that title.
    /// </summary>
    void OnCoverLoaded(object sender, RoutedEventArgs e)
    {
        if (sender is not FrameworkElement { Tag: uint } container)
            return;
        HoverPreview.Attach(container, () => (uint)container.Tag);
        if (container.ContextFlyout is null)
        {
            var menu = new MenuFlyout();
            menu.Opening += (_, _) =>
            {
                menu.Items.Clear();
                if (container.DataContext is CoverItem item)
                    Fill(menu, item);
            };
            container.ContextFlyout = menu;
        }
    }

    static readonly (LibrarySegment Segment, string Name)[] Statuses =
    [
        (LibrarySegment.Watching, T.LibraryStatusWatching), (LibrarySegment.Planning, T.LibraryStatusPlanning),
        (LibrarySegment.Completed, T.LibraryStatusCompleted), (LibrarySegment.Paused, T.LibraryStatusPaused),
        (LibrarySegment.Dropped, T.LibraryStatusDropped),
    ];

    /// <summary>
    /// The title menu, as on the Mac: Open, the list status with the current one checked (Add to
    /// Planning while the title is on no list), and Open on AniList.
    /// </summary>
    static void Fill(MenuFlyout menu, CoverItem item)
    {
        var ctx = Ctx.Current;
        var id = item.Id;
        var open = new MenuFlyoutItem { Text = T.TitleOpen, Icon = new FontIcon { Glyph = "" } };
        open.Click += (_, _) => ctx.OpenTitle(id);
        menu.Items.Add(open);
        var status = ctx.Known.Entry(id)?.ListStatus ?? item.Entry.ListStatus;
        if (status is null)
        {
            var plan = new MenuFlyoutItem { Text = T.LibraryAddPlanning, Icon = new FontIcon { Glyph = "" } };
            plan.Click += (_, _) => SetStatus(id, LibrarySegment.Planning);
            menu.Items.Add(plan);
        }
        else
        {
            var list = new MenuFlyoutSubItem { Text = T.TitleListStatus, Icon = new FontIcon { Glyph = "" } };
            foreach (var (segment, name) in Statuses)
            {
                var radio = new RadioMenuFlyoutItem { Text = name, GroupName = "status", IsChecked = segment == status };
                radio.Click += (_, _) => SetStatus(id, segment);
                list.Items.Add(radio);
            }
            menu.Items.Add(list);
        }
        menu.Items.Add(new MenuFlyoutSeparator());
        var anilist = new MenuFlyoutItem { Text = T.TitleOpenAnilist, Icon = new FontIcon { Glyph = "" } };
        anilist.Click += (_, _) => ctx.Send(new Command.OpenExternal($"https://anilist.co/anime/{id}"));
        menu.Items.Add(anilist);
    }

    /// <summary>The engine only queues the change for the tracker: the status is set here at once, and said.</summary>
    static void SetStatus(uint id, LibrarySegment segment)
    {
        var ctx = Ctx.Current;
        if ((ctx.Known.Entry(id)?.ListStatus) == segment)
            return;
        ctx.Send(new Command.SetStatus(id, segment));
        ctx.Known.SetStatus(id, segment);
        if (segment == LibrarySegment.Planning)
            ctx.Window.Say(T.LibraryAddedPlanning);
    }
}
