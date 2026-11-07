// How a scroller moved under the wheel, from the probe's offsets.csv (each page scroller's offset
// on every frame it changed) and its "wheel" marks (each notch as the window heard it). Per step:
//
//   stalls        frames that did not move while the scroll was under way: a still frame between
//                 two moving ones with no notch arriving in that pause
//   late          frames that took more than 1.5 refreshes while the scroll was under way
//   largest jump  the most it moved from one frame to the next, in px
//   first move    from a notch that found the page at rest to the first frame that moved, the
//                 step's first notch and the slowest of them
//   settle        from the last notch to the last frame that moved
//
// The offset is read at the start of each frame, so a move made in a frame is seen in the next
// one: every time here carries that one frame.

using System.Diagnostics;
using System.Globalization;
using System.Text;

static class ScrollMotion
{
    sealed record Offset(long At, string Name, double Vertical, double Horizontal);

    public static string Table(string dir, IEnumerable<Step> steps, double period)
    {
        var offsets = Read(Path.Combine(dir, "offsets.csv"));
        if (offsets.Count == 0)
            return "";
        var frames = File.Exists(Path.Combine(dir, "frames.csv"))
            ? File.ReadLines(Path.Combine(dir, "frames.csv")).Where(l => l.Length > 0 && l[0] != '#').Select(l => long.Parse(l, CultureInfo.InvariantCulture)).ToArray()
            : [];
        var wheels = File.Exists(Path.Combine(dir, "marks.csv"))
            ? File.ReadLines(Path.Combine(dir, "marks.csv")).Skip(1).Select(l => l.Split(',', 2)).Where(p => p.Length == 2 && p[1].StartsWith("wheel", StringComparison.Ordinal)).Select(p => long.Parse(p[0], CultureInfo.InvariantCulture)).ToArray()
            : [];
        var ms = 1000.0 / Stopwatch.Frequency;
        var md = new StringBuilder();
        md.AppendLine("Scroll motion (offsets.csv): stalls are still frames mid-scroll with no notch in the pause; late frames took over 1.5 refreshes mid-scroll.");
        md.AppendLine();
        md.AppendLine("| Step | Scroller | Notches | px a notch | Stalls | Late | Largest jump px | First move ms | Slowest first move ms | Settle ms |");
        md.AppendLine("|---|---|---:|---:|---:|---:|---:|---:|---:|---:|");
        foreach (var s in steps)
        {
            var notches = wheels.Where(w => w >= s.Start && w <= s.End).ToArray();
            if (notches.Length == 0)
                continue;
            // The scroller that moved most in the step, on the axis it moved most along.
            var moved = offsets.Where(o => o.At >= s.Start && o.At <= s.End).GroupBy(o => o.Name)
                .Select(g =>
                {
                    var before = offsets.LastOrDefault(o => o.Name == g.Key && o.At < s.Start) ?? g.First();
                    double v = 0, h = 0, pv = before.Vertical, ph = before.Horizontal;
                    foreach (var o in g)
                    {
                        v += Math.Abs(o.Vertical - pv);
                        h += Math.Abs(o.Horizontal - ph);
                        (pv, ph) = (o.Vertical, o.Horizontal);
                    }
                    return (Name: g.Key, Across: h > v, Total: Math.Max(v, h), Before: before);
                })
                .OrderByDescending(x => x.Total).FirstOrDefault();
            if (moved.Name is null || moved.Total < 1)
            {
                md.AppendLine($"| {s.Label} | - | {notches.Length} | 0 | - | - | - | - | - | - |");
                continue;
            }
            double Pick(Offset o) => moved.Across ? o.Horizontal : o.Vertical;
            var mine = offsets.Where(o => o.Name == moved.Name && o.At <= s.End).ToArray();
            var at = frames.Where(f => f >= s.Start && f <= s.End).ToArray();
            var value = new double[at.Length];
            var j = 0;
            var current = Pick(moved.Before);
            for (var i = 0; i < at.Length; i++)
            {
                while (j < mine.Length && mine[j].At <= at[i])
                    current = Pick(mine[j++]);
                value[i] = current;
            }
            var moving = new bool[at.Length];
            for (var i = 1; i < at.Length; i++)
                moving[i] = Math.Abs(value[i] - value[i - 1]) > 0.01;
            var first = Array.IndexOf(moving, true);
            var last = Array.LastIndexOf(moving, true);
            if (first < 0)
                continue;
            int stalls = 0, late = 0;
            double jump = 0;
            for (var i = first; i <= last; i++)
            {
                if (i > first && (at[i] - at[i - 1]) * ms > period * 1.5)
                    late++;
                if (moving[i])
                {
                    jump = Math.Max(jump, Math.Abs(value[i] - value[i - 1]));
                    continue;
                }
                // A pause: a stall unless a notch came during it.
                var end = i;
                while (end <= last && !moving[end])
                    end++;
                if (!notches.Any(w => w > at[i - 1] && w <= at[end]))
                    stalls += end - i;
                i = end - 1;
            }
            // First moves: from each notch that found the scroller still for the two frames before it.
            var firstMoves = new List<double>();
            foreach (var w in notches)
            {
                var k = Array.FindIndex(at, f => f >= w);
                if (k < 0)
                    continue;
                if (k >= 2 && (moving[k - 1] || moving[k - 2]))
                    continue;
                var m = Array.FindIndex(moving, k, x => x);
                if (m >= 0)
                    firstMoves.Add((at[m] - w) * ms);
            }
            var settle = (at[last] - notches[^1]) * ms;
            var distance = Math.Abs(value[last] - value[Math.Max(0, first - 1)]);
            md.AppendLine(string.Create(CultureInfo.InvariantCulture,
                $"| {s.Label} | {moved.Name}{(moved.Across ? " (across)" : "")} | {notches.Length} | {distance / notches.Length:F0} | {stalls} of {last - first + 1} | {late} | {jump:F0} | {(firstMoves.Count > 0 ? firstMoves[0].ToString("F0", CultureInfo.InvariantCulture) : "-")} | {(firstMoves.Count > 0 ? firstMoves.Max().ToString("F0", CultureInfo.InvariantCulture) : "-")} | {settle:F0} |"));
        }
        md.AppendLine();
        // Only steps with notches make rows: a scenario without the wheel gets no table.
        return steps.Any(s => wheels.Any(w => w >= s.Start && w <= s.End)) ? md.ToString() : "";
    }

    static List<Offset> Read(string path) =>
        File.Exists(path)
            ? [.. File.ReadLines(path).Skip(1).Select(l => l.Split(',')).Where(p => p.Length == 4)
                .Select(p => new Offset(long.Parse(p[0], CultureInfo.InvariantCulture), p[1], double.Parse(p[2], CultureInfo.InvariantCulture), double.Parse(p[3], CultureInfo.InvariantCulture)))]
            : [];
}
