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

    /// <summary>
    /// The covers, replaced whole. The same titles with the same words leave the grid as it is,
    /// so an answer that changes nothing (a sync, a reload on a visit) moves nothing, and focus,
    /// an open menu and the art already shown stay.
    /// </summary>
    internal IReadOnlyList<CoverItem> Covers
    {
        set
        {
            if (Items.ItemsSource is IReadOnlyList<CoverItem> shown && Alike(shown, value))
                return;
            Items.ItemsSource = value;
#if KAERI_TESTING
            // --show=hover: the first cover's preview open, as if the pointer rested on it.
            if (Ctx.Current.Options.Show == "hover" && value.Count > 0 && !previewed)
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
    }

    static bool Alike(IReadOnlyList<CoverItem> a, IReadOnlyList<CoverItem> b)
    {
        if (a.Count != b.Count)
            return false;
        for (var i = 0; i < a.Count; i++)
        {
            if (!a[i].Entry.Equals(b[i].Entry) || a[i].Caption != b[i].Caption || a[i].Count != b[i].Count)
                return false;
        }
        return true;
    }

    /// <summary>Whether the grid shows any covers now.</summary>
    internal bool HasCovers => Items.ItemsSource is IReadOnlyList<CoverItem> { Count: > 0 };

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

    /// <summary>A container is up: the pointer resting on it opens the preview of the title it holds now, which a recycled container changes.</summary>
    void OnCoverLoaded(object sender, RoutedEventArgs e)
    {
        if (sender is FrameworkElement { Tag: uint })
            HoverPreview.Attach((FrameworkElement)sender, () => (uint)((FrameworkElement)sender).Tag);
    }

    void OnOpen(object sender, RoutedEventArgs e)
    {
        if ((sender as FrameworkElement)?.Tag is CoverItem item)
            Ctx.Current.OpenTitle(item.Id);
    }

    void OnPlan(object sender, RoutedEventArgs e)
    {
        if ((sender as FrameworkElement)?.Tag is not CoverItem item)
            return;
        // The engine only queues the change for the tracker: the status is set here at once, and said.
        var ctx = Ctx.Current;
        ctx.Send(new Command.SetStatus(item.Id, LibrarySegment.Planning));
        ctx.Known.SetStatus(item.Id, LibrarySegment.Planning);
        ctx.Window.Say(T.LibraryAddedPlanning);
    }
}
