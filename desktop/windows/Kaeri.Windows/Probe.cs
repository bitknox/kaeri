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
/// </summary>
static class Probe
{
    static readonly object Gate = new();
    static readonly StringBuilder Frames = new(), Stalls = new(), Memory = new(), Marks = new();
    static string? dir;

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

        CompositionTarget.Rendering += (_, _) =>
        {
            lock (Gate)
                Frames.Append(Stopwatch.GetTimestamp()).Append('\n');
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
