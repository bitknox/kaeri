// The browsing screens left alone: each page opened, then filmed for a while without a touch
// while the engine sends what it sends (sync, progress, art; on the profile copy, the network).
// Nothing should change on screen that is not an intended animation, and no element should be
// made again. Per page the report says how many frames changed, where, and how many of the
// page's elements UI Automation finds again by their runtime id afterwards. Only when asked for
// by name: "idle" on the fixture scene, "idle-real" on the profile copy.

using System.Diagnostics;
using System.Drawing;
using System.Drawing.Imaging;
using System.Globalization;
using System.Runtime.InteropServices;
using System.Text;
using FlaUI.Core.AutomationElements;
using FlaUI.Core.Definitions;
using FlaUI.Core.Input;

static class IdleScenario
{
    static readonly StringBuilder report = new();

    static int Seconds => int.TryParse(Environment.GetEnvironmentVariable("KAERI_IDLE_SECONDS"), out var s) ? s : 15;

    public static async Task All(Run run)
    {
        report.Clear();
        report.AppendLine("# Left alone");
        report.AppendLine();
        report.AppendLine("| Page | Frames changed | Largest change | Where (x,y,w,h of the union) | Elements kept | Remade | Shots |");
        report.AppendLine("|---|---:|---:|---|---:|---:|---|");
        var only = Environment.GetEnvironmentVariable("KAERI_IDLE_PAGES") is { Length: > 0 } list ? list.Split(',') : null;
        bool Wanted(string page) => only is null || only.Contains(page);
        if (Wanted("Home"))
        {
            await FeelScenario.Go(run, "Home");
            await Watch(run, "home");
        }
        foreach (var page in new[] { "Library", "Seasonal", "Calendar", "Search" })
        {
            if (!Wanted(page))
                continue;
            await FeelScenario.Go(run, page);
            await Watch(run, page.ToLowerInvariant());
        }
        if (Wanted("Title"))
        {
            await FeelScenario.Go(run, "Seasonal");
            await Task.Delay(1500);
            var cover = run.Covers().FirstOrDefault(c => Run.Safe(() => c.BoundingRectangle) is { } r && r.Height > r.Width * 1.2);
            if (cover is not null)
            {
                run.Click(cover);
                await Watch(run, "title");
            }
        }
        File.WriteAllText(Path.Combine(run.Options.Out, run.Name, "idle.md"), report.ToString());
        Console.WriteLine(report.ToString());
    }

    /// <summary>The pointer resting on a cover in Seasonal, filmed: the card should grow from the cover, and leave when the pointer does.</summary>
    public static async Task HoverCard(Run run)
    {
        await FeelScenario.Go(run, "Seasonal");
        await Task.Delay(1500);
        // The card opens only in the window in front: a click on the page's title makes sure.
        var front = run.Window.BoundingRectangle;
        Native.SetForegroundWindow(run.Hwnd);
        Mouse.MoveTo(new Point(front.Left + 320, front.Top + 92));
        Mouse.Click();
        await Task.Delay(500);
        var covers = run.Covers().Where(c => Run.Safe(() => c.BoundingRectangle) is { } r && r.Height > r.Width * 1.2).Take(3).ToArray();
        foreach (var (cover, i) in covers.Select((c, i) => (c, i)))
        {
            var r = cover.BoundingRectangle;
            await Feel.Film(run, $"hover card: rest on cover {i + 1}", () => Task.Delay(1200),
                async () =>
                {
                    // A real pointer arrives in steps: the cover hears it enter, then move, then rest.
                    var at = new Point(r.Left + r.Width / 2, r.Top + r.Height / 3);
                    Mouse.MoveTo(new Point(at.X - 30, at.Y));
                    for (var step = 0; step < 6; step++)
                    {
                        Native.Nudge(5, 0);
                        await Task.Delay(16);
                    }
                }, 3000);
        }
        var w = run.Window.BoundingRectangle;
        await Feel.Film(run, "hover card: pointer leaves", async () =>
        {
            for (var step = 0; step < 10; step++)
            {
                Native.Nudge(0, 60);
                await Task.Delay(16);
            }
        }, null, 2000);
        Feel.Write(Path.Combine(run.Options.Out));
    }

    static string Id(AutomationElement e) => string.Join(".", Run.Safe(() => e.Properties.RuntimeId.ValueOrDefault) ?? []);

    static Dictionary<string, string> Elements(Run run)
    {
        var found = new Dictionary<string, string>();
        foreach (var e in run.Window.FindAllDescendants(cf => cf.ByControlType(ControlType.Button)
                     .Or(cf.ByControlType(ControlType.ListItem))
                     .Or(cf.ByControlType(ControlType.Text))
                     .Or(cf.ByControlType(ControlType.Hyperlink))
                     .Or(cf.ByControlType(ControlType.Image))))
        {
            if (Run.Safe(() => e.BoundingRectangle) is not { Width: > 0 } r || !run.Inside(r))
                continue;
            var id = Id(e);
            if (id.Length > 0)
                found[id] = $"{Run.Safe(() => e.ControlType)} '{Run.Safe(() => e.Name)}'";
        }
        return found;
    }

    static async Task Watch(Run run, string page)
    {
        var dir = Path.Combine(run.Options.Out, run.Name);
        Directory.CreateDirectory(dir);
        // Out of the way, so no hover counts.
        var w = run.Window.BoundingRectangle;
        Mouse.MoveTo(new Point(w.Left + 8, w.Bottom - 8));
        await Task.Delay(3000);
        var before = Elements(run);
        // Where each element of the page stands, to read layout from (Home's shelves and gaps).
        File.WriteAllLines(Path.Combine(dir, $"idle-{page}-rects.txt"), run.Window.FindAllDescendants()
            .Select(e => (Type: Run.Safe(() => e.ControlType.ToString()), Name: Run.Safe(() => e.Name), Rect: Run.Safe(() => e.BoundingRectangle)))
            .Where(e => e.Rect.Width > 0 && run.Inside(e.Rect))
            .Select(e => $"{e.Rect.Top - w.Top},{e.Rect.Bottom - w.Top},{e.Rect.Left - w.Left},{e.Rect.Width} {e.Type} {e.Name}"));
        var area = run.Window.BoundingRectangle;
        var previous = Grab(area);
        var changed = 0;
        var largest = 0.0;
        Rectangle? union = null;
        var shots = new List<string>();
        var clock = Stopwatch.StartNew();
        while (clock.Elapsed.TotalSeconds < Seconds)
        {
            await Task.Delay(60);
            var now = Grab(area);
            var (share, box) = Differ(previous, now);
            if (share > 0.0002)
            {
                changed++;
                largest = Math.Max(largest, share);
                union = union is { } u ? Rectangle.Union(u, box) : box;
                if (shots.Count < 6)
                {
                    var name = $"idle-{page}-{clock.ElapsedMilliseconds:00000}ms.png";
                    using (var g = Graphics.FromImage(now))
                    using (var pen = new Pen(Color.Magenta, 3))
                        g.DrawRectangle(pen, box);
                    now.Save(Path.Combine(dir, name), ImageFormat.Png);
                    // The saved frame has the box drawn on it: compare the next with a clean one.
                    now.Dispose();
                    now = Grab(area);
                    shots.Add(name);
                }
            }
            previous.Dispose();
            previous = now;
        }
        previous.Dispose();
        var after = Elements(run);
        var kept = before.Keys.Count(after.ContainsKey);
        var remade = before.Where(p => !after.ContainsKey(p.Key)).Select(p => p.Value).Take(12).ToList();
        report.AppendLine(string.Create(CultureInfo.InvariantCulture,
            $"| {page} | {changed} | {largest:P2} | {(union is { } b ? $"{b.X},{b.Y},{b.Width},{b.Height}" : "")} | {kept}/{before.Count} | {before.Count - kept}: {string.Join("; ", remade).Replace("|", "/", StringComparison.Ordinal)} | {string.Join(" ", shots)} |"));
        Console.WriteLine($"   idle {page}: {changed} frames changed, {kept}/{before.Count} elements kept");
    }

    static Bitmap Grab(Rectangle area)
    {
        var bitmap = new Bitmap(area.Width, area.Height, PixelFormat.Format32bppRgb);
        using var g = Graphics.FromImage(bitmap);
        g.CopyFromScreen(area.Left, area.Top, 0, 0, area.Size, CopyPixelOperation.SourceCopy);
        return bitmap;
    }

    /// <summary>The share of pixels that differ, at full size, and the box around them.</summary>
    static (double Share, Rectangle Box) Differ(Bitmap a, Bitmap b)
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
            int differ = 0, left = int.MaxValue, top = int.MaxValue, right = -1, bottom = -1;
            for (var i = 0; i < bytes; i += 4)
            {
                var d = Math.Abs(pa[i] - pb[i]) + Math.Abs(pa[i + 1] - pb[i + 1]) + Math.Abs(pa[i + 2] - pb[i + 2]);
                if (d <= 24)
                    continue;
                differ++;
                var p = i / 4;
                var x = p % (da.Stride / 4);
                var y = p / (da.Stride / 4);
                left = Math.Min(left, x);
                right = Math.Max(right, x);
                top = Math.Min(top, y);
                bottom = Math.Max(bottom, y);
            }
            var box = right < 0 ? Rectangle.Empty : new Rectangle(left, top, right - left + 1, bottom - top + 1);
            return (differ / (double)(rect.Width * rect.Height), box);
        }
        finally
        {
            a.UnlockBits(da);
            b.UnlockBits(db);
        }
    }
}
