#if KAERI_TESTING
using System.Diagnostics;
using System.Text;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml.Media;

namespace Kaeri.Windows;

/// <summary>
/// Testing build only: what desktop/windows/Kaeri.Drive reads to judge smoothness. With
/// KAERI_PROBE_DIR set, the app writes three files there, appended about twice a second so a hang
/// or a crash still leaves what came before it. Times are Stopwatch ticks (QPC), the same clock
/// the driver stamps its steps with, so the two line up.
///
/// frames.csv  one line per composition frame the UI thread rendered (CompositionTarget.Rendering)
/// stalls.csv  start and length in ms of every wait of 40 ms or more for the UI thread, from a
///             watchdog thread that asks it to run something every 10 ms
/// memory.csv  once a second: private bytes, working set, handles, threads
/// offsets.csv each page scroller's offsets on every frame they differ from the frame before,
///             stamped with that frame's line in frames.csv
/// marks.csv   moments to line up with, such as a page loading or a wheel notch arriving
/// </summary>
static class Probe
{
    static readonly object Gate = new();
    static readonly StringBuilder Frames = new(), Stalls = new(), Memory = new(), Marks = new(), Offsets = new();
    static string? dir;

    /// <summary>Whether this run records, known before Start: pages made with the window register their scrollers first.</summary>
    static readonly bool On = !string.IsNullOrEmpty(Environment.GetEnvironmentVariable("KAERI_PROBE_DIR"));

    /// <summary>The page scrollers watched, by the name offsets.csv gives them, how to read where they show, and the offsets last written.</summary>
    static readonly List<(WeakReference<Microsoft.UI.Xaml.Controls.ScrollViewer> Scroller, string Name, Func<(double Vertical, double Horizontal)> Shown, double[] Last)> scrollers = [];

    /// <summary>
    /// Writes where <paramref name="scroller"/> shows its content to offsets.csv on every frame it
    /// changed, as <paramref name="name"/>: its offsets, less what a glide on the compositor has still
    /// to go (<paramref name="shown"/>, as the glide computes it). The driver turns them into stalls,
    /// jumps and delays; the screen's own frames are in its screen motion table.
    /// </summary>
    public static void Watch(Microsoft.UI.Xaml.Controls.ScrollViewer scroller, string name, Func<(double Vertical, double Horizontal)> shown)
    {
        if (!On || scrollers.Any(s => s.Scroller.TryGetTarget(out var known) && known == scroller))
            return;
        scrollers.Add((new(scroller), name, shown, [double.NaN, double.NaN]));
    }

    /// <summary>Each wheel notch the window hears, handled or not, as a mark: "wheel" and its delta.</summary>
    public static void HearWheel(Microsoft.UI.Xaml.UIElement root)
    {
        if (dir is null)
            return;
        root.AddHandler(Microsoft.UI.Xaml.UIElement.PointerWheelChangedEvent, new Microsoft.UI.Xaml.Input.PointerEventHandler((_, e) =>
        {
            var point = e.GetCurrentPoint(null).Properties;
            Mark($"wheel {(point.IsHorizontalMouseWheel ? "sideways " : "")}{point.MouseWheelDelta}");
        }), true);
    }

    /// <summary>A moment the driver lines up with its steps, such as a navigation finishing: marks.csv.</summary>
    public static void Mark(string what)
    {
        if (dir is null)
            return;
        lock (Gate)
            Marks.Append(Stopwatch.GetTimestamp()).Append(',').Append(what.Replace(',', ' ')).Append('\n');
    }

    public static void Start(DispatcherQueue ui)
    {
        dir = Environment.GetEnvironmentVariable("KAERI_PROBE_DIR");
        if (string.IsNullOrEmpty(dir))
            return;
        Directory.CreateDirectory(dir);
        File.WriteAllText(Path.Combine(dir, "frames.csv"), $"# frequency {Stopwatch.Frequency}\n");
        File.WriteAllText(Path.Combine(dir, "stalls.csv"), "start,ms\n");
        File.WriteAllText(Path.Combine(dir, "memory.csv"), "at,private_mb,working_mb,handles,threads\n");
        File.WriteAllText(Path.Combine(dir, "marks.csv"), "at,what\n");

        File.WriteAllText(Path.Combine(dir, "offsets.csv"), "at,scroller,vertical,horizontal\n");

        CompositionTarget.Rendering += (_, _) =>
        {
            var now = Stopwatch.GetTimestamp();
            lock (Gate)
                Frames.Append(now).Append('\n');
            for (var i = scrollers.Count - 1; i >= 0; i--)
            {
                var (scroller, name, shown, last) = scrollers[i];
                if (!scroller.TryGetTarget(out var s))
                {
                    scrollers.RemoveAt(i);
                    continue;
                }
                if (s.XamlRoot is null)
                    continue;
                var (v, h) = shown();
                if (v == last[0] && h == last[1])
                    continue;
                last[0] = v;
                last[1] = h;
                lock (Gate)
                    Offsets.Append(now).Append(',').Append(name).Append(',')
                        .Append(v.ToString("F2", System.Globalization.CultureInfo.InvariantCulture)).Append(',')
                        .Append(h.ToString("F2", System.Globalization.CultureInfo.InvariantCulture)).Append('\n');
            }
        };

        var watchdog = new Thread(() => Watch(ui)) { IsBackground = true, Name = "probe watchdog", Priority = ThreadPriority.AboveNormal };
        watchdog.Start();
        var writer = new Thread(Write) { IsBackground = true, Name = "probe writer" };
        writer.Start();
    }

    /// <summary>Asks the UI thread to run an empty item every 10 ms and times how long it waits.</summary>
    static void Watch(DispatcherQueue ui)
    {
        var ran = new ManualResetEventSlim();
        while (true)
        {
            ran.Reset();
            var asked = Stopwatch.GetTimestamp();
            if (!ui.TryEnqueue(DispatcherQueuePriority.High, ran.Set))
                return;
            ran.Wait();
            var waited = Stopwatch.GetElapsedTime(asked).TotalMilliseconds;
            if (waited >= 40)
                lock (Gate)
                    Stalls.Append(asked).Append(',').Append(waited.ToString("F1", System.Globalization.CultureInfo.InvariantCulture)).Append('\n');
            Thread.Sleep(10);
        }
    }

    static void Write()
    {
        var self = Process.GetCurrentProcess();
        var lastMemory = 0L;
        while (true)
        {
            Thread.Sleep(500);
            var now = Stopwatch.GetTimestamp();
            if (Stopwatch.GetElapsedTime(lastMemory, now).TotalSeconds >= 1)
            {
                lastMemory = now;
                self.Refresh();
                lock (Gate)
                    Memory.Append(now).Append(',').Append(self.PrivateMemorySize64 >> 20).Append(',').Append(self.WorkingSet64 >> 20)
                        .Append(',').Append(self.HandleCount).Append(',').Append(self.Threads.Count).Append('\n');
            }
            Flush("frames.csv", Frames);
            Flush("stalls.csv", Stalls);
            Flush("memory.csv", Memory);
            Flush("marks.csv", Marks);
            Flush("offsets.csv", Offsets);
        }
    }

    /// <summary>Appends what <paramref name="buffer"/> holds, and drops it only once it is written.</summary>
    static void Flush(string name, StringBuilder buffer)
    {
        string text;
        lock (Gate)
            text = buffer.ToString();
        if (text.Length == 0)
            return;
        try
        {
            File.AppendAllText(Path.Combine(dir!, name), text);
            lock (Gate)
                buffer.Remove(0, text.Length);
        }
        catch (IOException)
        {
            // The driver reading at the same moment: the next round writes it.
        }
    }
}
#endif
