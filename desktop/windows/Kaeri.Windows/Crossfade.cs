using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media.Imaging;

namespace Kaeri.Windows;

/// <summary>
/// A swap of what an area shows, as a crossfade with both sides drawn: the area as it stood is
/// kept as a picture over it, the change is made, and the picture fades away on the standard
/// token, so the area is never empty between the two. A fade, so it stays with animation effects
/// off.
/// </summary>
static class Crossfade
{
    /// <summary>Makes <paramref name="change"/> to <paramref name="area"/>, fading from how it looked.</summary>
    public static async void Swap(FrameworkElement area, Action change)
    {
        Grid? host = null;
        for (var up = Microsoft.UI.Xaml.Media.VisualTreeHelper.GetParent(area); up is not null; up = Microsoft.UI.Xaml.Media.VisualTreeHelper.GetParent(up))
            if (up is Grid grid)
            {
                host = grid;
                break;
            }
        RenderTargetBitmap? before = null;
        if (host is not null && area.ActualWidth > 0 && area.ActualHeight > 0)
        {
            try
            {
                before = new RenderTargetBitmap();
                await before.RenderAsync(area);
            }
            catch (Exception)
            {
                // Nothing to fade from: the change just shows.
                before = null;
            }
        }
        change();
        if (before is null || host is null)
            return;
        var offset = area.TransformToVisual(host).TransformPoint(new global::Windows.Foundation.Point(0, 0));
        var ghost = new Image
        {
            Source = before,
            Width = area.ActualWidth,
            Height = area.ActualHeight,
            Stretch = Microsoft.UI.Xaml.Media.Stretch.Fill,
            HorizontalAlignment = HorizontalAlignment.Left,
            VerticalAlignment = VerticalAlignment.Top,
            Margin = new Thickness(offset.X - host.Padding.Left, offset.Y - host.Padding.Top, 0, 0),
            IsHitTestVisible = false,
        };
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetAccessibilityView(ghost, Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
        // Over the whole grid, as its place is measured from the grid's corner, inside its padding:
        // in the area's own row it would stand lower by the rows above, such as the party bar over
        // the pages.
        Grid.SetRowSpan(ghost, Math.Max(1, host.RowDefinitions.Count));
        Grid.SetColumnSpan(ghost, Math.Max(1, host.ColumnDefinitions.Count));
        host.Children.Add(ghost);
        // The fade waits two frames, so what the change settles a frame late (a scroll to a row
        // just built) happens under the picture and never shows half done.
        var frames = 0;
        EventHandler<object>? drawn = null;
        drawn = (_, _) =>
        {
            if (++frames < 2)
                return;
            Microsoft.UI.Xaml.Media.CompositionTarget.Rendering -= drawn;
            Motion.HideStandard(ghost);
            var timer = ghost.DispatcherQueue.CreateTimer();
            timer.Interval = Motion.Standard + TimeSpan.FromMilliseconds(80);
            timer.IsRepeating = false;
            timer.Tick += (_, _) => host.Children.Remove(ghost);
            timer.Start();
        };
        Microsoft.UI.Xaml.Media.CompositionTarget.Rendering += drawn;
    }
}
