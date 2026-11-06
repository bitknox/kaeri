// How the app feels: each action filmed from just before the click until the window has been still
// for a while, from the screen, as fast as it can be read. Per action, feel.md says how long the
// first visible answer took, when the window settled, and how many separate bursts of change it
// went through. More than one burst is the window changing again after it answered: content
// arriving and moving what was there, a placeholder replaced by something of another size, a list
// rebuilt. The frames that start each burst are kept beside the report to look at.

using System.Diagnostics;
using System.Drawing;
using System.Drawing.Imaging;
using System.Globalization;
using System.Runtime.InteropServices;
using System.Text;
using FlaUI.Core.AutomationElements;
using FlaUI.Core.Definitions;
using FlaUI.Core.Input;
using FlaUI.Core.WindowsAPI;

sealed record Filmed(string Label, double? FirstMs, double SettledMs, int Bursts, double LaterArea, int Frames, string[] Shots, string? Note);

static class Feel
{
    /// <summary>A frame counts as changed when this share of the window differs from the one before.</summary>
    const double Changed = 0.0015;
    /// <summary>Stillness that separates two bursts, and that ends the filming.</summary>
    const double QuietMs = 350;
    const int Small = 4;

    static readonly List<Filmed> filmed = [];
    static string dir = "";
    static int shot;

    /// <summary>
    /// Films <paramref name="act"/>: the pointer is moved by <paramref name="aim"/> first and left to
    /// settle, so a hover is not counted, then the moment <paramref name="act"/> starts is time zero.
    /// </summary>
    public static async Task Film(Run run, string label, Func<Task> act, Func<Task>? aim = null, int longestMs = 4000, string? note = null, bool wheel = false)
    {
        dir = Path.Combine(run.Options.Out, run.Name);
        Directory.CreateDirectory(dir);
        if (aim is not null)
            await aim();
        await Task.Delay(500);
        var area = run.Window.BoundingRectangle;
        using var before = Grab(area);
        var previous = Shrink(before);
        var first = Shrink(before);
        var timeline = new List<(double Ms, double Change, double FromStart, Bitmap Full)>();
        var clock = Stopwatch.StartNew();
        await act();
        double lastChange = -1;
        while (clock.ElapsedMilliseconds < longestMs)
        {
            var ms = clock.Elapsed.TotalMilliseconds;
            var full = Grab(area);
            var small = Shrink(full);
            var change = Differ(previous, small);
            var fromStart = Differ(first, small);
            previous.Dispose();
            previous = small;
            if (change >= Changed)
            {
                lastChange = ms;
                timeline.Add((ms, change, fromStart, full));
            }
            else
            {
                timeline.Add((ms, change, fromStart, null!));
                full.Dispose();
                if (lastChange >= 0 && ms - lastChange > QuietMs + 400)
                    break;
                if (lastChange < 0 && ms > 2500)
                    break;
            }
        }
        previous.Dispose();
        first.Dispose();

        // Bursts: runs of changed frames with less than QuietMs of stillness between them.
        var bursts = new List<List<(double Ms, double Change, double FromStart, Bitmap Full)>>();
        double? lastAt = null;
        foreach (var frame in timeline.Where(f => f.Change >= Changed))
        {
            if (lastAt is null || frame.Ms - lastAt > QuietMs)
                bursts.Add([]);
            bursts[^1].Add(frame);
            lastAt = frame.Ms;
        }
        var shots = new List<string>();
        void Keep(Bitmap? image, string what)
        {
            if (image is null)
                return;
            var name = $"feel-{++shot:D3}-{what}.png";
            image.Save(Path.Combine(dir, name), ImageFormat.Png);
            shots.Add(name);
        }
        Keep(before, "before");
        foreach (var burst in bursts)
            Keep(burst[0].Full, $"burst-{burst[0].Ms:0}ms");
        if (bursts.Count > 0)
            Keep(bursts[^1][^1].Full, "settled");
        var later = bursts.Skip(1).Sum(b => b.Max(f => f.Change));
        // A scroll that moves in steps changes few frames, each by a lot; a smooth one changes
        // many frames a little at a time.
        var moved = timeline.Where(f => f.Change >= Changed).ToList();
        if (wheel && moved.Count > 0)
            note = string.Create(CultureInfo.InvariantCulture,
                $"{moved.Count} frames moved, largest step {moved.Max(f => f.Change):P0}, median step {moved.Select(f => f.Change).Order().ElementAt(moved.Count / 2):P0}, frames {string.Join(" ", moved.Select(f => $"{f.Ms:0}:{f.Change:P0}"))}");
        filmed.Add(new(label, bursts.FirstOrDefault()?[0].Ms, lastChange < 0 ? 0 : lastChange, bursts.Count, later, timeline.Count, [.. shots], note));
        foreach (var frame in timeline)
            frame.Full?.Dispose();
        Console.WriteLine($"   {label}: first {(bursts.Count > 0 ? $"{bursts[0][0].Ms:0} ms" : "none")}, settled {lastChange:0} ms, {bursts.Count} burst(s), {timeline.Count} frames");
    }

    /// <summary>
    /// A way out and back, such as a title opened and Back: the window before <paramref name="trip"/>
    /// and after it, compared. A place kept differs by little more than the pointer's hover.
    /// </summary>
    public static async Task Kept(Run run, string label, Func<Task> trip)
    {
        dir = Path.Combine(run.Options.Out, run.Name);
        Directory.CreateDirectory(dir);
        var aside = new Point(run.Window.BoundingRectangle.Left + 8, run.Window.BoundingRectangle.Bottom - 8);
        Mouse.MoveTo(aside);
        await Task.Delay(800);
        var area = run.Window.BoundingRectangle;
        using var before = Grab(area);
        await trip();
        Mouse.MoveTo(aside);
        await Task.Delay(1500);
        using var after = Grab(area);
        using var a = Shrink(before);
        using var b = Shrink(after);
        var differ = Differ(a, b);
        var shots = new List<string>();
        foreach (var (image, what) in new[] { (before, "kept-before"), (after, "kept-after") })
        {
            var name = $"feel-{++shot:D3}-{what}.png";
            image.Save(Path.Combine(dir, name), ImageFormat.Png);
            shots.Add(name);
        }
        var note = string.Create(CultureInfo.InvariantCulture, $"after the trip, {differ:P1} of the window differs from before it");
        filmed.Add(new(label, null, 0, 0, differ, 0, [.. shots], note));
        Console.WriteLine($"   {label}: {note}");
    }

    /// <summary>A click on <paramref name="target"/>, filmed.</summary>
    public static Task Click(Run run, string label, AutomationElement? target, int longestMs = 4000)
    {
        if (target is null)
        {
            filmed.Add(new(label, null, 0, 0, 0, 0, [], "not found"));
            Console.WriteLine($"   {label}: not found");
            return Task.CompletedTask;
        }
        var r = target.BoundingRectangle;
        var at = new Point(r.Left + r.Width / 2, r.Top + r.Height / 2);
        return Film(run, label, () => { Mouse.Click(); return Task.CompletedTask; }, () => { Mouse.MoveTo(at); return Task.CompletedTask; }, longestMs);
    }

    public static void Write(string outDir)
    {
        var text = new StringBuilder();
        text.AppendLine("# How it feels");
        text.AppendLine();
        text.AppendLine("Per action: the first visible change after the click, when the window was last seen changing, and how many separate bursts of change it took. More than one burst is the window changing again after it answered. Later change is the share of the window the later bursts changed, at most.");
        text.AppendLine();
        text.AppendLine("| Action | First change | Settled | Bursts | Later change | Frames | Shots | Note |");
        text.AppendLine("|---|---:|---:|---:|---:|---:|---|---|");
        foreach (var f in filmed)
            text.AppendLine(string.Create(CultureInfo.InvariantCulture,
                $"| {f.Label} | {(f.FirstMs is { } ms ? $"{ms:0} ms" : "none")} | {f.SettledMs:0} ms | {f.Bursts} | {f.LaterArea:P1} | {f.Frames} | {string.Join(" ", f.Shots)} | {f.Note} |"));
        File.WriteAllText(Path.Combine(outDir, "feel.md"), text.ToString());
    }

    static Bitmap Grab(Rectangle area)
    {
        var bitmap = new Bitmap(area.Width, area.Height, PixelFormat.Format32bppRgb);
        using var g = Graphics.FromImage(bitmap);
        g.CopyFromScreen(area.Left, area.Top, 0, 0, area.Size, CopyPixelOperation.SourceCopy);
        return bitmap;
    }

    static Bitmap Shrink(Bitmap full)
    {
        var small = new Bitmap(full.Width / Small, full.Height / Small, PixelFormat.Format32bppRgb);
        using var g = Graphics.FromImage(small);
        g.InterpolationMode = System.Drawing.Drawing2D.InterpolationMode.NearestNeighbor;
        g.DrawImage(full, 0, 0, small.Width, small.Height);
        return small;
    }

    /// <summary>The share of pixels that differ by more than a little between two frames of one size.</summary>
    static double Differ(Bitmap a, Bitmap b)
    {
        var rect = new Rectangle(0, 0, Math.Min(a.Width, b.Width), Math.Min(a.Height, b.Height));
        var da = a.LockBits(rect, ImageLockMode.ReadOnly, PixelFormat.Format32bppRgb);
        var db = b.LockBits(rect, ImageLockMode.ReadOnly, PixelFormat.Format32bppRgb);
        try
        {
            var bytes = da.Stride * rect.Height;
            var pa = new byte[bytes];
            var pb = new byte[bytes];
            Marshal.Copy(da.Scan0, pa, 0, bytes);
            Marshal.Copy(db.Scan0, pb, 0, bytes);
            var differ = 0;
            for (var i = 0; i < bytes; i += 4)
            {
                var d = Math.Abs(pa[i] - pb[i]) + Math.Abs(pa[i + 1] - pb[i + 1]) + Math.Abs(pa[i + 2] - pb[i + 2]);
                if (d > 36)
                    differ++;
            }
            return differ / (double)(rect.Width * rect.Height);
        }
        finally
        {
            a.UnlockBits(da);
            b.UnlockBits(db);
        }
    }
}
