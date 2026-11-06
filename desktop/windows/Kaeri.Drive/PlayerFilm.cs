// The player filmed while nobody touches it: every frame the screen shows, at half size, so what
// changes on its own can be looked at frame by frame. Only the clock and the progress should move
// over the video; anything else that changes (a row reflowing, a card blanking, a panel built
// again) is a redraw to fix. Fixture scenes only:
//
//   player-film: Play on Home and the handover to the first frame, 15 s with the controls hidden,
//     15 s with the pointer on the top bar (controls up), then a seek into the ending and the
//     Up next card until the next episode starts.
//   party-film: the scene's party rehearsal, named by KAERI_DRIVE_SHOW (party-lobby, party-panel,
//     party-chat-moment...), filmed for 20 s from 6 s after launch.
//
// Each film is a folder of NNNN-<ms>.png frames beside the report.

using System.Diagnostics;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using FlaUI.Core.Input;
using FlaUI.Core.WindowsAPI;

static partial class PlayerFilm
{
    public static async Task All(Run run)
    {
        var w = run.Window.BoundingRectangle;
        var s = GetDpiForWindow(run.Hwnd) / 96.0;
        var video = new Point(w.Left + w.Width / 2, w.Top + w.Height / 3);
        var title = new Point((int)(w.Left + 160 * s), (int)(w.Top + 28 * s));
        var barY = (int)(w.Bottom - 32 * s);
        var lineFrom = (int)(w.Left + (12 + 3 * 40 + 3 * 6 + 70) * s);
        var lineTo = (int)(w.Right - (12 + 2 * 40 + 3 * 6 + 70) * s);

        // Home can still be filling in when Play is first pressed: again until the player loaded.
        var marks = Path.Combine(run.Options.Out, run.Name, "marks.csv");
        for (var attempt = 0; attempt < 3 && !Loaded(marks); attempt++)
            await run.Step("film: Play and the first frame", async () =>
            {
                FlaUI.Core.AutomationElements.AutomationElement? found = null;
                for (var i = 0; i < 20 && found is null; i++)
                {
                    found = run.NameStarts("Resume") ?? run.NameStarts("Play episode");
                    if (found is null)
                        await Task.Delay(500);
                }
                if (found is null)
                {
                    run.Note("no Play on Home");
                    return;
                }
                // Pressed through UI Automation: the film is of what follows, not of the pointer.
                await Record(run, "1-handover", 7000, () =>
                {
                    if (found.Patterns.Invoke.PatternOrDefault is { } invoke)
                        invoke.Invoke();
                    else
                        found.Click();
                    return Task.CompletedTask;
                });
            });
        if (!Loaded(marks))
        {
            run.Note("the player never loaded");
            return;
        }
        await run.Step("film: playing, controls hidden", async () =>
        {
            PlayerChecks.Move(video);
            await Task.Delay(200);
            await Record(run, "2-idle", 15000);
        });
        await run.Step("film: playing, pointer on the top bar", async () =>
        {
            PlayerChecks.Move(new Point(title.X + 4, title.Y));
            await Task.Delay(100);
            PlayerChecks.Move(title);
            await Task.Delay(400);
            await Record(run, "3-bar", 15000);
        });
        await run.Step("film: a seek into the ending, then Up next", async () =>
        {
            // By keys, which the player hears wherever the pointer is: three Shift+Right (30 s
            // each) and one L (10 s) from about 0:50 land near 2:30 of the 3:00 clip, where Up
            // next shows.
            run.Front();
            PlayerChecks.Move(video);
            await Task.Delay(300);
            await Record(run, "4-ending", 26000, async () =>
            {
                for (var i = 0; i < 3; i++)
                {
                    Keyboard.TypeSimultaneously(VirtualKeyShort.SHIFT, VirtualKeyShort.RIGHT);
                    await Task.Delay(150);
                }
                Keyboard.Type(VirtualKeyShort.KEY_L);
            });
        });
    }

    public static async Task Party(Run run)
    {
        var w = run.Window.BoundingRectangle;
        Mouse.MoveTo(new Point(w.Left + w.Width / 2, w.Top + w.Height / 3));
        await Task.Delay(2000);
        await run.Step("film: the rehearsal, untouched", async () => await Record(run, "party", 20000));
    }

    static bool Loaded(string marks)
    {
        try
        {
            using var stream = new FileStream(marks, FileMode.Open, FileAccess.Read, FileShare.ReadWrite);
            return new StreamReader(stream).ReadToEnd().Contains("loaded player", StringComparison.Ordinal);
        }
        catch (IOException)
        {
            return false;
        }
    }

    /// <summary>Films the window for <paramref name="ms"/>, <paramref name="act"/> at its start.</summary>
    static async Task Record(Run run, string label, int ms, Func<Task>? act = null)
    {
        var dir = Path.Combine(run.Options.Out, run.Name, label);
        if (Directory.Exists(dir))
            Directory.Delete(dir, true);
        Directory.CreateDirectory(dir);
        var area = run.Window.BoundingRectangle;
        var saves = new List<Task>();
        var clock = Stopwatch.StartNew();
        var acting = act?.Invoke() ?? Task.CompletedTask;
        var n = 0;
        while (clock.ElapsedMilliseconds < ms)
        {
            var at = clock.ElapsedMilliseconds;
            var half = new Bitmap(area.Width / 2, area.Height / 2, PixelFormat.Format24bppRgb);
            using (var full = new Bitmap(area.Width, area.Height, PixelFormat.Format32bppRgb))
            {
                using (var g = Graphics.FromImage(full))
                    g.CopyFromScreen(area.Left, area.Top, 0, 0, area.Size, CopyPixelOperation.SourceCopy);
                using var h = Graphics.FromImage(half);
                h.InterpolationMode = InterpolationMode.Bilinear;
                h.DrawImage(full, 0, 0, half.Width, half.Height);
            }
            var name = Path.Combine(dir, $"{n++:D4}-{at:D5}.png");
            saves.Add(Task.Run(() =>
            {
                half.Save(name, ImageFormat.Png);
                half.Dispose();
            }));
            await Task.Yield();
        }
        await acting;
        await Task.WhenAll(saves);
        Console.WriteLine($"   {label}: {n} frames in {ms} ms");
    }

    [LibraryImport("user32.dll")]
    private static partial uint GetDpiForWindow(nint hwnd);
}
