using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Kaeri.Windows.Pages;

/// <summary>
/// A button whose work runs as one of the engine's jobs, such as installing or updating a plugin:
/// once the work is sent the button is held, with a ring beside it, until the job ends, so a
/// click is seen at once. What runs is kept by key, so a button drawn again while its job runs
/// comes back held; the row it sits in usually changes when the job ends.
/// </summary>
static class JobButton
{
    static readonly Dictionary<string, ulong> running = [];
    static bool listening;

    /// <param name="key">What the work is about, such as the plugin's id.</param>
    /// <param name="send">Asks first where it must, then sends the work and gives its job; null when nothing was sent.</param>
    public static FrameworkElement Make(Ctx ctx, string key, string text, Func<Task<ulong?>> send, bool accent = false)
    {
        Listen(ctx);
        var ring = new ProgressRing { Width = 16, Height = 16, VerticalAlignment = VerticalAlignment.Center };
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        Button button = null!;
        button = Ui.Label(text, async (_, _) =>
        {
            if (running.ContainsKey(key))
                return;
            button.IsEnabled = false;
            if (await send() is { } job)
                running[key] = job;
            Show();
        }, accent);
        row.Children.Add(ring);
        row.Children.Add(button);
        void Show()
        {
            var busy = running.ContainsKey(key);
            button.IsEnabled = !busy;
            ring.IsActive = busy;
            ring.Visibility = busy ? Visibility.Visible : Visibility.Collapsed;
        }
        void Heard(IReadOnlyList<Event> batch)
        {
            if (batch.Any(e => e is Event.JobDone))
                Show();
        }
        row.Loaded += (_, _) =>
        {
            ctx.Batch += Heard;
            Show();
        };
        row.Unloaded += (_, _) => ctx.Batch -= Heard;
        Show();
        return row;
    }

    /// <summary>Jobs that end are let go here first, whether or not their button is on screen.</summary>
    static void Listen(Ctx ctx)
    {
        if (listening)
            return;
        listening = true;
        ctx.Batch += batch =>
        {
            foreach (var e in batch)
                if (e is Event.JobDone done)
                    foreach (var key in running.Where(p => p.Value == done.V1).Select(p => p.Key).ToList())
                        running.Remove(key);
        };
    }
}
