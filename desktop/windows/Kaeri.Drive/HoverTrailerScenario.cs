// How long a hover card's trailer takes to start, on a copy of the person's profile
// (KAERI_PROFILE_ROOT), with its real network: the pointer rests on one cover after another on
// Seasonal, waits for the trailer, and leaves. Nothing is clicked but the sidebar's Seasonal.
// The numbers come from the app's log (run with KAERI_LOG=info,app=debug): "hover card <id>:
// video shown <n> ms after the pointer rested", which this scenario gathers into the report.

using System.Drawing;
using System.Globalization;
using System.Text.RegularExpressions;
using FlaUI.Core.Input;

static partial class HoverTrailerScenario
{
    /// <summary>How many covers it rests on, one after another.</summary>
    const int Covers = 8;

    /// <summary>The wait on Seasonal before the first hover: the app's warm-up comes at a still moment after the covers show.</summary>
    static readonly TimeSpan Settle = TimeSpan.FromSeconds(10);

    public static async Task All(Run run)
    {
        if (Environment.GetEnvironmentVariable("KAERI_PROFILE_ROOT") is not { Length: > 0 })
        {
            run.Note("hover-trailer: KAERI_PROFILE_ROOT is not set; it runs on a profile copy only");
            return;
        }
        var from = DateTime.UtcNow;
        // The profile's Home takes a while to settle at launch, and a click then can be lost.
        await Task.Delay(6000);
        Rectangle[] covers = [];
        for (var attempt = 0; attempt < 3 && covers.Length < Covers; attempt++)
        {
            await FeelScenario.Go(run, "Seasonal");
            await Task.Delay(Settle);
            covers = [.. run.Covers()
                .Where(c => Run.Safe(() => c.BoundingRectangle) is { } r && r.Height > r.Width * 1.2)
                .Take(Covers)
                .Select(c => c.BoundingRectangle)];
        }
        run.Note($"hover-trailer: {covers.Length} covers to rest on");
        if (covers.Length == 0)
            return;
        foreach (var (r, i) in covers.Select((r, i) => (r, i)))
        {
            await Onto(run, r);
            await Task.Delay(3000);
            Band(run, r, i + 1);
            await Task.Delay(3600);
            await Away(run);
            run.Note($"hover-trailer: rested on cover {i + 1}");
        }
        // Then from an open card straight onto the next cover in its row, as a person reading a
        // row does: that cover's trailer is cued while the card it left is still closing.
        var pairs = covers.Zip(covers.Skip(1)).Where(p => Math.Abs(p.First.Top - p.Second.Top) < 4 && p.Second.Left > p.First.Left).Take(3).ToList();
        foreach (var (a, b) in pairs)
        {
            await Onto(run, a);
            await Task.Delay(3000);
            run.Guard();
            for (var step = 0; step < 60 && Mouse.Position.X < b.Left + 40; step++)
            {
                Check(run);
                Native.Nudge(Math.Min(12, b.Left + 40 - Mouse.Position.X), 0);
                await Task.Delay(16);
            }
            await Task.Delay(3000);
            await Away(run);
            run.Note("hover-trailer: from a card onto the next cover");
        }
        await Task.Delay(1000);
        Gather(run, from);
    }

    /// <summary>
    /// From just left of the cover, onto it in steps as a real pointer comes, then still. The
    /// pointer jumps there: a glide would cross other covers, and a card one of them opened would
    /// make this cover's wait the shorter one for scanning a row.
    /// </summary>
    static async Task Onto(Run run, Rectangle r)
    {
        run.Guard();
        Mouse.Position = new Point(r.Left - 24, r.Top + r.Height / 3);
        await Task.Delay(50);
        for (var step = 0; step < 8; step++)
        {
            Check(run);
            Native.Nudge(8, 0);
            await Task.Delay(16);
        }
    }

    /// <summary>
    /// Away over the sidebar, where nothing opens a card, in steps so the card hears the pointer
    /// leave it (a jump is not always heard, and the card would stay), and long enough for the next
    /// cover's full dwell rather than the scanning one.
    /// </summary>
    static async Task Away(Run run)
    {
        run.Guard();
        var w = run.Window.BoundingRectangle;
        for (var step = 0; step < 40 && Mouse.Position.X > w.Left + 120; step++)
        {
            Check(run);
            Native.Nudge(-60, 0);
            await Task.Delay(16);
        }
        Mouse.Position = new Point(w.Left + 40, w.Top + w.Height - 60);
        await Task.Delay(1800);
    }

    static void Check(Run run)
    {
        if (!run.InFront)
            throw new InvalidOperationException("another window came in front; stopped before sending input");
    }

    /// <summary>
    /// The open card's video band, from the screen, twice 400 ms apart: a playing video changes
    /// between them, the art does not. The first is kept as band-N.png. Only while the app is in front.
    /// </summary>
    static void Band(Run run, Rectangle cover, int n)
    {
        if (!run.InFront)
            return;
        var w = run.Window.BoundingRectangle;
        var left = Math.Clamp(cover.Left + cover.Width / 2 - 192, w.Left, w.Right - 384);
        var top = Math.Clamp(cover.Top - 24 + 1, w.Top, w.Bottom - 216);
        var area = new Rectangle(left + 8, top + 8, 384 - 16, 216 - 16);
        using var first = Grab(area);
        Thread.Sleep(400);
        if (!run.InFront)
            return;
        using var second = Grab(area);
        long sum = 0;
        for (var y = 0; y < area.Height; y += 4)
            for (var x = 0; x < area.Width; x += 4)
            {
                var a = first.GetPixel(x, y);
                var b = second.GetPixel(x, y);
                sum += Math.Abs(a.R - b.R) + Math.Abs(a.G - b.G) + Math.Abs(a.B - b.B);
            }
        var change = sum / (double)(area.Width / 4 * (area.Height / 4) * 3);
        first.Save(Path.Combine(run.Options.Out, run.Name, $"band-{n}.png"));
        Console.WriteLine($"   hover-trailer: cover {n} band changed {change:0.0} a channel in 400 ms");
        File.AppendAllText(Path.Combine(run.Options.Out, run.Name, "bands.txt"), $"{n} {change:0.0}\n");
    }

    static Bitmap Grab(Rectangle area)
    {
        var bitmap = new Bitmap(area.Width, area.Height);
        using var g = Graphics.FromImage(bitmap);
        g.CopyFromScreen(area.Left, area.Top, 0, 0, area.Size, CopyPixelOperation.SourceCopy);
        return bitmap;
    }

    /// <summary>The log's hover lines since <paramref name="from"/>, in order, as notes.</summary>
    static void Gather(Run run, DateTime from)
    {
        var root = Environment.GetEnvironmentVariable("KAERI_PROFILE_ROOT")!;
        var logs = Path.Combine(root, "cache", "logs");
        if (!Directory.Exists(logs))
        {
            run.Note("hover-trailer: no log directory");
            return;
        }
        var lines = new List<string>();
        foreach (var file in Directory.GetFiles(logs, "app.log.*").Where(f => File.GetLastWriteTimeUtc(f) >= from))
        {
            using var stream = new FileStream(file, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete);
            using var reader = new StreamReader(stream);
            while (reader.ReadLine() is { } line)
            {
                if (!line.Contains("source=trailer", StringComparison.Ordinal))
                    continue;
                var stamp = Stamp().Match(line);
                if (stamp.Success && DateTime.TryParse(stamp.Value, CultureInfo.InvariantCulture, DateTimeStyles.AdjustToUniversal | DateTimeStyles.AssumeUniversal, out var at) && at < from)
                    continue;
                lines.Add(line);
            }
        }
        var shown = lines.Select(l => Shown().Match(l)).Where(m => m.Success).Select(m => int.Parse(m.Groups[1].Value, CultureInfo.InvariantCulture)).ToList();
        run.Note($"hover-trailer: {shown.Count} trailer(s) shown; ms after the pointer rested, in order: {string.Join(", ", shown)}");
        File.WriteAllLines(Path.Combine(run.Options.Out, run.Name, "trailer-log.txt"), lines);
    }

    [GeneratedRegex(@"^\S+")]
    private static partial Regex Stamp();

    [GeneratedRegex(@"video shown (\d+) ms after the pointer rested")]
    private static partial Regex Shown();
}
