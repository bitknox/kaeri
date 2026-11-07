// How a page moved on the screen, frame by frame, whatever moved it: the app's UI thread or the
// compositor. While a wheel step runs, a thread waits for each frame Windows composes (DwmFlush)
// and copies a narrow strip of the page from the screen; afterwards each strip is matched against
// the one before to find how far the content moved. Per step:
//
//   stalls        composed frames that did not move while the scroll was under way: a still
//                 frame between two moving ones with no notch sent during the pause
//   largest jump  the most the page moved from one composed frame to the next, in px
//   first move    from sending a notch that found the page still to the first frame that moved
//   settle        from the last notch sent to the last frame that moved
//
// The strip is only the app's window when nothing covers it; the driver keeps the app in front.

using System.Diagnostics;
using System.Drawing;
using System.Globalization;
using System.Runtime.InteropServices;
using System.Text;

static partial class ScreenMotion
{
    sealed record Filmed(string Label, long[] At, int[] Shift, bool[] Sure, long[] Sent);

    static readonly List<Filmed> filmed = [];

    const int Width = 48, MaxShift = 160;

    /// <summary>Films the strip <paramref name="strip"/> while <paramref name="act"/> runs, given the notches' send times it fills.</summary>
    public static async Task Film(string label, Rectangle strip, Func<List<long>, Task> act)
    {
        var frames = new List<(long At, byte[] Rows)>();
        var sent = new List<long>();
        using var stop = new CancellationTokenSource();
        var thread = new Thread(() => Capture(strip, frames, stop.Token)) { IsBackground = true, Priority = ThreadPriority.Highest };
        thread.Start();
        await Task.Delay(100);
        await act(sent);
        stop.Cancel();
        thread.Join();
        var at = new long[frames.Count];
        var shift = new int[frames.Count];
        var sure = new bool[frames.Count];
        for (var i = 0; i < frames.Count; i++)
        {
            at[i] = frames[i].At;
            if (i > 0)
                (shift[i], sure[i]) = Match(frames[i - 1].Rows, frames[i].Rows, strip.Height);
        }
        filmed.Add(new(label, at, shift, sure, [.. sent]));
    }

    static void Capture(Rectangle strip, List<(long, byte[])> frames, CancellationToken stop)
    {
        var screen = GetDC(0);
        var memory = CreateCompatibleDC(screen);
        var bitmap = CreateCompatibleBitmap(screen, Width, strip.Height);
        var old = SelectObject(memory, bitmap);
        var info = new BitmapInfo { Size = 40, Width = Width, Height = -strip.Height, Planes = 1, BitCount = 32 };
        var pixels = new byte[Width * strip.Height * 4];
        try
        {
            while (!stop.IsCancellationRequested)
            {
                DwmFlush();
                var at = Stopwatch.GetTimestamp();
                BitBlt(memory, 0, 0, Width, strip.Height, screen, strip.Left, strip.Top, 0x00CC0020);
                GetDIBits(memory, bitmap, 0, (uint)strip.Height, pixels, ref info, 0);
                // Each row as three bands' brightness: enough to match, cheap to compare.
                var rows = new byte[strip.Height * 3];
                for (var y = 0; y < strip.Height; y++)
                    for (var band = 0; band < 3; band++)
                    {
                        var sum = 0;
                        for (var x = band * 16; x < band * 16 + 16; x++)
                        {
                            var p = (y * Width + x) * 4;
                            sum += pixels[p] + pixels[p + 1] * 2 + pixels[p + 2];
                        }
                        rows[y * 3 + band] = (byte)(sum / 64);
                    }
                frames.Add((at, rows));
            }
        }
        finally
        {
            SelectObject(memory, old);
            DeleteObject(bitmap);
            DeleteDC(memory);
            _ = ReleaseDC(0, screen);
        }
    }

    /// <summary>How far the content moved up from <paramref name="a"/> to <paramref name="b"/>, and whether the match is clear.</summary>
    static (int Shift, bool Sure) Match(byte[] a, byte[] b, int height)
    {
        var still = Cost(a, b, height, 0);
        if (still < 0.6)
            return (0, true);
        double best = double.MaxValue;
        var at = 0;
        for (var d = -MaxShift; d <= MaxShift; d++)
        {
            var c = Cost(a, b, height, d);
            if (c < best)
            {
                best = c;
                at = d;
            }
        }
        // A frame whose content changed otherwise (a cover arriving) matches nothing well.
        return (at, best < still * 0.5 && best < 6);
    }

    /// <summary>Mean difference of the rows where b's row y is a's row y + d.</summary>
    static double Cost(byte[] a, byte[] b, int height, int d)
    {
        long sum = 0;
        var n = 0;
        for (var y = Math.Max(0, -d); y < height && y + d < height; y++)
        {
            var ia = (y + d) * 3;
            var ib = y * 3;
            sum += Math.Abs(a[ia] - b[ib]) + Math.Abs(a[ia + 1] - b[ib + 1]) + Math.Abs(a[ia + 2] - b[ib + 2]);
            n += 3;
        }
        return n < height ? double.MaxValue : (double)sum / n;
    }

    /// <summary>The table for every step filmed, for report.md; empty when none was.</summary>
    public static string Table(double period)
    {
        if (filmed.Count == 0)
            return "";
        var ms = 1000.0 / Stopwatch.Frequency;
        var md = new StringBuilder();
        md.AppendLine("## Screen motion");
        md.AppendLine();
        md.AppendLine("Each composed frame of a strip of the page, matched against the one before. Stalls are still frames mid-scroll with no notch sent in the pause; unsure frames could not be matched (content changing in place).");
        md.AppendLine();
        md.AppendLine("| Step | Frames | Moved px | Stalls | Unsure | Largest jump px | Frame gap over 1.5 refreshes | First move ms | Slowest first move ms | Settle ms |");
        md.AppendLine("|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|");
        foreach (var f in filmed)
        {
            var moving = f.Shift.Select(s => s != 0).ToArray();
            var first = Array.IndexOf(moving, true);
            var last = Array.LastIndexOf(moving, true);
            if (first < 0 || f.Sent.Length == 0)
            {
                md.AppendLine($"| {f.Label} | {f.At.Length} | 0 | - | - | - | - | - | - | - |");
                continue;
            }
            int stalls = 0, unsure = 0, gaps = 0, jump = 0;
            for (var i = first; i <= last; i++)
            {
                if (i > first && (f.At[i] - f.At[i - 1]) * ms > period * 1.5)
                    gaps++;
                if (!f.Sure[i])
                    unsure++;
                if (moving[i])
                {
                    jump = Math.Max(jump, Math.Abs(f.Shift[i]));
                    continue;
                }
                var end = i;
                while (end <= last && !moving[end])
                    end++;
                if (!f.Sent.Any(w => w > f.At[i - 1] && w <= f.At[end]))
                    stalls += end - i;
                i = end - 1;
            }
            var firstMoves = new List<double>();
            foreach (var w in f.Sent)
            {
                var k = Array.FindIndex(f.At, a => a >= w);
                if (k < 2 || moving[k - 1] || moving[k - 2])
                    continue;
                var m = Array.FindIndex(moving, k, x => x);
                if (m >= 0)
                    firstMoves.Add((f.At[m] - w) * ms);
            }
            var moved = f.Shift.Sum(s => Math.Abs(s));
            md.AppendLine(string.Create(CultureInfo.InvariantCulture,
                $"| {f.Label} | {last - first + 1} | {moved} | {stalls} | {unsure} | {jump} | {gaps} | {(firstMoves.Count > 0 ? firstMoves[0].ToString("F0", CultureInfo.InvariantCulture) : "-")} | {(firstMoves.Count > 0 ? firstMoves.Max().ToString("F0", CultureInfo.InvariantCulture) : "-")} | {(f.At[last] - f.Sent[^1]) * ms:F0} |"));
        }
        md.AppendLine();
        return md.ToString();
    }

    /// <summary>Each step's shifts frame by frame, for a closer look: motion.csv beside the report.</summary>
    public static void Write(string dir)
    {
        if (filmed.Count == 0)
            return;
        var csv = new StringBuilder("step,at_ms,shift,sure,notch\n");
        foreach (var f in filmed)
        {
            var origin = f.At.Length > 0 ? f.At[0] : 0;
            for (var i = 0; i < f.At.Length; i++)
            {
                var notch = f.Sent.Count(w => w > (i > 0 ? f.At[i - 1] : long.MinValue) && w <= f.At[i]);
                csv.Append(f.Label.Replace(',', ' ')).Append(',')
                    .Append(((f.At[i] - origin) * 1000.0 / Stopwatch.Frequency).ToString("F2", CultureInfo.InvariantCulture)).Append(',')
                    .Append(f.Shift[i]).Append(',').Append(f.Sure[i] ? 1 : 0).Append(',').Append(notch).Append('\n');
            }
        }
        File.WriteAllText(Path.Combine(dir, "motion.csv"), csv.ToString());
    }

    [StructLayout(LayoutKind.Sequential)]
    struct BitmapInfo
    {
        public int Size, Width, Height;
        public short Planes, BitCount;
        public int Compression, SizeImage, XPelsPerMeter, YPelsPerMeter, ClrUsed, ClrImportant;
        public int Colors;
    }

    [LibraryImport("dwmapi.dll")]
    private static partial int DwmFlush();

    [LibraryImport("user32.dll")]
    private static partial nint GetDC(nint hwnd);

    [LibraryImport("user32.dll")]
    private static partial int ReleaseDC(nint hwnd, nint dc);

    [LibraryImport("gdi32.dll")]
    private static partial nint CreateCompatibleDC(nint dc);

    [LibraryImport("gdi32.dll")]
    private static partial nint CreateCompatibleBitmap(nint dc, int w, int h);

    [LibraryImport("gdi32.dll")]
    private static partial nint SelectObject(nint dc, nint obj);

    [LibraryImport("gdi32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static partial bool DeleteObject(nint obj);

    [LibraryImport("gdi32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static partial bool DeleteDC(nint dc);

    [LibraryImport("gdi32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static partial bool BitBlt(nint dest, int x, int y, int w, int h, nint src, int sx, int sy, uint rop);

    [LibraryImport("gdi32.dll")]
    private static partial int GetDIBits(nint dc, nint bitmap, uint start, uint lines, [Out] byte[] bits, ref BitmapInfo info, uint usage);
}
