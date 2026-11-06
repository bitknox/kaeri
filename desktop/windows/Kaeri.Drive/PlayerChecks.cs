// The player's checks: what a click on the bar leaves behind, Space, a drag along the seek line
// and the volume line, and a seek into the ending, each with a capture of the screen at the
// moment that tells. Fixture scene "playing", so it never touches the person's profile. The
// player hides its controls from UI Automation while video plays, so the bar is found by its
// layout (Kaeri.Windows/Player/PlayerView.xaml): 12 epx in from the window's edges, 40 epx buttons.

using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using FlaUI.Core.Input;
using FlaUI.Core.WindowsAPI;

static partial class PlayerChecks
{
    public static async Task All(Run run)
    {
        // Home can still be filling in when the first Play is pressed: again until the app's
        // probe says the player loaded.
        var marks = Path.Combine(run.Options.Out, run.Name, "marks.csv");
        for (var attempt = 0; attempt < 3 && !PlayerLoaded(marks); attempt++)
            await run.Step("play from Home", async () => await Scenarios.PlayFromHome(run, 9000));
        await Task.Delay(4000);
        if (!PlayerLoaded(marks))
        {
            run.Note("the player never loaded");
            return;
        }
        run.Front();
        var w = run.Window.BoundingRectangle;
        var s = GetDpiForWindow(run.Hwnd) / 96.0;
        int X(double epx) => (int)(w.Left + epx * s);
        int FromRight(double epx) => (int)(w.Right - epx * s);
        var barY = (int)(w.Bottom - 32 * s);
        var playPause = new Point(X(32), barY);
        var volume = new Point(FromRight(12 + 40 + 6 + 20), barY);
        // The line runs between the times: after three buttons and the elapsed time, before the
        // remaining time and two buttons. Inset so the ends are on the line.
        var lineFrom = X(12 + 3 * 40 + 3 * 6 + 70);
        var lineTo = FromRight(12 + 2 * 40 + 3 * 6 + 70);
        Point Line(double fraction) => new((int)(lineFrom + (lineTo - lineFrom) * fraction), barY);
        var video = new Point(w.Left + w.Width / 2, w.Top + w.Height / 3);

        // A person's clicks and keys, as Kaeri.Drive's other scenarios give them.
        await run.Step("click pause then play on the bar, then leave the pointer still", async () =>
        {
            Front(run);
            Mouse.MoveTo(video);
            await Task.Delay(400);
            Mouse.MoveTo(playPause);
            await Task.Delay(300);
            Mouse.Click();
            await Task.Delay(700);
            Shot(run, "01-clicked-pause");
            Mouse.Click();
            await Task.Delay(300);
            Mouse.MoveTo(video);
            at = video;
            await Task.Delay(3800);
            Shot(run, "02-still-after-click");
        });
        await run.Step("Space after a click on the bar", async () =>
        {
            Front(run);
            Keyboard.Type(VirtualKeyShort.SPACE);
            await Task.Delay(800);
            Shot(run, "03-space-pauses");
            Keyboard.Type(VirtualKeyShort.SPACE);
            await Task.Delay(3800);
            Shot(run, "04-space-plays-and-hides");
        });
        await run.Step("drag along the seek line", async () =>
        {
            Front(run);
            Move(video);
            await Task.Delay(300);
            Move(Line(0.15));
            await Task.Delay(300);
            Button(down: true);
            for (var i = 0; i <= 30; i++)
            {
                Move(Line(0.15 + 0.5 * i / 30));
                await Task.Delay(60);
                if (i == 20)
                    Shot(run, "05-scrub-mid");
            }
            await Task.Delay(400);
            Shot(run, "06-scrub-held");
            Button(down: false);
            await Task.Delay(150);
            Shot(run, "07-scrub-released");
            await Task.Delay(2500);
            Shot(run, "08-scrub-landed");
        });
        await run.Step("drag the volume line", async () =>
        {
            Front(run);
            Move(volume);
            await Task.Delay(200);
            Button(down: true);
            Button(down: false);
            await Task.Delay(900);
            // The flyout's 160 epx line sits centred above the button; at 100 its thumb is at the right end.
            var thumb = new Point((int)(volume.X + 48 * s), (int)(volume.Y - 64 * s));
            Shot(run, "09-volume-open");
            Move(thumb);
            await Task.Delay(200);
            Button(down: true);
            for (var i = 0; i <= 20; i++)
            {
                Move(new Point(thumb.X - (int)(i * 4 * s), thumb.Y));
                await Task.Delay(50);
            }
            await Task.Delay(500);
            Shot(run, "10-volume-held");
            Button(down: false);
            await Task.Delay(800);
            Shot(run, "11-volume-after");
            // The button again closes the flyout; Esc would reach the player if it had gone.
            Move(volume);
            Button(down: true);
            Button(down: false);
            await Task.Delay(600);
        });
        await run.Step("click into the ending", async () =>
        {
            Front(run);
            Move(video);
            await Task.Delay(300);
            Move(Line(0.93));
            await Task.Delay(300);
            Button(down: true);
            Button(down: false);
            await Task.Delay(1200);
            Shot(run, "12-ending-1s");
            Move(video);
            await Task.Delay(3000);
            Shot(run, "13-ending-4s");
            await Task.Delay(12000);
            Shot(run, "14-ending-16s");
        });
    }

    static bool PlayerLoaded(string marks)
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

    /// <summary>The window to the front, asked again while another window has it.</summary>
    static void Front(Run run)
    {
        for (var i = 0; i < 5 && GetForegroundWindow() != run.Hwnd; i++)
        {
            run.Front();
            Thread.Sleep(400);
        }
        if (GetForegroundWindow() != run.Hwnd)
            run.Note("the window would not come to the front");
    }

    [DllImport("user32.dll")]
    static extern nint GetForegroundWindow();

    static int shots;

    /// <summary>The screen under the window, as the eye sees it: mpv's picture with the controls over it.</summary>
    public static void Shot(Run run, string name)
    {
        try
        {
            var r = run.Window.BoundingRectangle;
            using var bitmap = new Bitmap(r.Width, r.Height, PixelFormat.Format32bppRgb);
            using (var g = Graphics.FromImage(bitmap))
                g.CopyFromScreen(r.Left, r.Top, 0, 0, r.Size, CopyPixelOperation.SourceCopy);
            var dir = Path.Combine(run.Options.Out, run.Name);
            Directory.CreateDirectory(dir);
            bitmap.Save(Path.Combine(dir, $"{name}.png"), ImageFormat.Png);
            shots++;
        }
        catch (Exception e)
        {
            run.Note($"no capture for {name}: {e.Message}");
        }
    }

    [LibraryImport("user32.dll")]
    private static partial uint GetDpiForWindow(nint hwnd);

    // The pointer through SendInput, as a mouse moves it: FlaUI's moves set the cursor, which
    // WinUI's pointer input does not see while a slider holds the pointer, so a drag never moved.

    [StructLayout(LayoutKind.Sequential)]
    struct MouseInput
    {
        public int Type;
        public int Pad;
        public int X, Y, Data, Flags, Time;
        public nint Extra;
    }

    [DllImport("user32.dll")]
    static extern uint SendInput(uint count, MouseInput[] inputs, int size);

    [DllImport("user32.dll")]
    static extern int GetSystemMetrics(int index);

    const int MoveFlag = 0x0001, LeftDown = 0x0002, LeftUp = 0x0004, Absolute = 0x8000, VirtualDesk = 0x4000;

    static void Send(int flags, Point at)
    {
        int left = GetSystemMetrics(76), top = GetSystemMetrics(77), width = GetSystemMetrics(78), height = GetSystemMetrics(79);
        var input = new MouseInput
        {
            Type = 0,
            X = (int)Math.Round((at.X - left) * 65535.0 / Math.Max(1, width - 1)),
            Y = (int)Math.Round((at.Y - top) * 65535.0 / Math.Max(1, height - 1)),
            Flags = flags | Absolute | VirtualDesk,
        };
        SendInput(1, [input], Marshal.SizeOf<MouseInput>());
    }

    static Point at;

    static void Move(Point to)
    {
        // A few steps on the way, as a hand moves.
        var from = at.IsEmpty ? to : at;
        for (var i = 1; i <= 4; i++)
            Send(MoveFlag, new Point(from.X + (to.X - from.X) * i / 4, from.Y + (to.Y - from.Y) * i / 4));
        at = to;
    }

    static void Button(bool down) => Send(down ? LeftDown : LeftUp, at);

    [StructLayout(LayoutKind.Sequential)]
    struct KeyInput
    {
        public int Type;
        public int Pad;
        public short Vk, Scan;
        public int Flags, Time;
        public nint Extra;
        public long Fill;
    }

    [DllImport("user32.dll", EntryPoint = "SendInput")]
    static extern uint SendKeys(uint count, KeyInput[] inputs, int size);

    [DllImport("user32.dll")]
    static extern uint MapVirtualKeyW(uint code, uint type);

    /// <summary>A key pressed and let go, with its scan code, as a keyboard sends it.</summary>
    static void Key(short vk)
    {
        var scan = (short)MapVirtualKeyW((uint)vk, 0);
        var size = Marshal.SizeOf<MouseInput>();
        SendKeys(1, [new KeyInput { Type = 1, Vk = vk, Scan = scan }], size);
        Thread.Sleep(40);
        SendKeys(1, [new KeyInput { Type = 1, Vk = vk, Scan = scan, Flags = 0x0002 }], size);
    }
}
