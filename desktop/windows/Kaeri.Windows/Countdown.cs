using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;

namespace Kaeri.Windows;

/// <summary>A once-a-second redraw for a page while something on it counts down, such as the wait for AniList's limit.</summary>
static class Countdown
{
    public static DispatcherQueueTimer Start(FrameworkElement page, Func<bool> counting, Action redraw)
    {
        var timer = page.DispatcherQueue.CreateTimer();
        timer.Interval = TimeSpan.FromSeconds(1);
        timer.Tick += (_, _) =>
        {
            if (counting())
                redraw();
        };
        page.Loaded += (_, _) => timer.Start();
        page.Unloaded += (_, _) => timer.Stop();
        timer.Start();
        return timer;
    }
}
