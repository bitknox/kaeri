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
        await FeelScenario.Go(run, "Seasonal");
        await Task.Delay(Settle);
        var covers = run.Covers()
            .Where(c => Run.Safe(() => c.BoundingRectangle) is { } r && r.Height > r.Width * 1.2)
            .Take(Covers)
            .Select(c => c.BoundingRectangle)
            .ToArray();
        if (covers.Length == 0)
        {
            run.Note("hover-trailer: no covers on Seasonal");
            return;
        }
        var w = run.Window.BoundingRectangle;
        foreach (var (r, i) in covers.Select((r, i) => (r, i)))
        {
            run.Guard();
            var y = r.Top + r.Height / 3;
            // From just left of the cover, onto it in steps as a real pointer comes, then still.
            Mouse.MoveTo(new Point(r.Left - 24, y));
            await Task.Delay(50);
            for (var step = 0; step < 8; step++)
            {
                if (!run.InFront)
                    throw new InvalidOperationException("another window came in front; stopped before sending input");
                Native.Nudge(8, 0);
                await Task.Delay(16);
            }
            await Task.Delay(7000);
            run.Guard();
            // Away over the sidebar, where nothing opens a card, long enough for the next cover's
            // full dwell rather than the scanning one.
            Mouse.MoveTo(new Point(w.Left + 40, w.Top + w.Height - 60));
            await Task.Delay(1800);
            run.Note($"hover-trailer: rested on cover {i + 1}");
        }
        await Task.Delay(1000);
        Gather(run, from);
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
