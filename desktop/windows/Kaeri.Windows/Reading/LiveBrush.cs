using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media;
using Windows.UI.ViewManagement;

namespace Kaeri.Windows.Pages;

/// <summary>
/// Theme brushes for reading's Home and Search that follow a high contrast switch while the app
/// runs. Each key hands out one brush, shared by every element that asks for it, and its colour is
/// looked up again when high contrast turns on or off, so the screens repaint without a restart.
/// </summary>
static class LiveBrush
{
    static readonly Dictionary<string, SolidColorBrush> Made = [];
    static UISettings? settings;

    /// <summary>The brush for <paramref name="key"/>; a brush that is not a solid colour is handed out as it is.</summary>
    public static Brush Of(string key)
    {
        if (Made.TryGetValue(key, out var made))
            return made;
        var found = Ui.Res<Brush>(key);
        if (found is not SolidColorBrush solid)
            return found;
        Watch();
        made = new SolidColorBrush(solid.Color) { Opacity = solid.Opacity };
        Made[key] = made;
        return made;
    }

    static void Watch()
    {
        if (settings is not null)
            return;
        var queue = DispatcherQueue.GetForCurrentThread();
        // High contrast turning on or off changes the system's colours, which UISettings tells of
        // on a thread of its own. WinUI swaps its theme dictionary on the same change, so the
        // colours are read once it has run, and once more a moment later in case they were still
        // settling.
        var again = queue.CreateTimer();
        again.Interval = TimeSpan.FromMilliseconds(400);
        again.IsRepeating = false;
        again.Tick += (_, _) => Refresh();
        settings = new UISettings();
        try
        {
            settings.ColorValuesChanged += (_, _) => queue.TryEnqueue(DispatcherQueuePriority.Low, () =>
            {
                Refresh();
                again.Start();
            });
        }
        catch (System.Runtime.InteropServices.COMException)
        {
            // Without the signal the brushes are as they were at start, as before.
        }
    }

    static void Refresh()
    {
        foreach (var (key, brush) in Made)
        {
            if (Ui.Res<Brush>(key) is not SolidColorBrush now)
                continue;
            if (brush.Color != now.Color)
                brush.Color = now.Color;
            if (brush.Opacity != now.Opacity)
                brush.Opacity = now.Opacity;
        }
    }
}
