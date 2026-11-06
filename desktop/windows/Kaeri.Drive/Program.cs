// Kaeri.Drive: plays the testing build of the Windows app as a person would, then says how it went.
//
//   dotnet run --project desktop/windows/Kaeri.Drive -- --app <Kaeri.exe> --media <dir> --out <dir>
//       [--only nav,home,hover,title,resize,player,player-4k] [--rounds 3]
//
// Each scenario starts the app fresh on a fixture scene (never the profile), with KAERI_PROBE_DIR
// set so the app records its frames, UI-thread stalls and memory (Kaeri.Windows/Probe.cs). The
// driver moves the real pointer and presses real keys through UI Automation, stamping each step
// with the same clock. Afterwards it writes <out>/report.md: per step, the frame times while
// something moved, the frames that missed the display's refresh, the longest wait for the UI
// thread, and per scenario memory, the app's warnings and errors, crashes and hangs.
//
// It takes the pointer and keyboard while it runs: leave the machine alone until it says done.

using System.Diagnostics;
using System.Drawing;
using System.Globalization;
using System.Runtime.InteropServices;
using System.Text;
using FlaUI.Core;
using FlaUI.Core.AutomationElements;
using FlaUI.Core.Definitions;
using FlaUI.Core.Input;
using FlaUI.Core.WindowsAPI;
using FlaUI.UIA3;

var options = Args.Parse(args);
var all = new (string Name, Func<Run, Task> Body, string Scene, bool Player)[]
{
    ("nav", Scenarios.Nav, "watching", false),
    ("home", Scenarios.Home, "watching", false),
    ("hover", Scenarios.Hover, "watching", false),
    ("title", Scenarios.Title, "watching", false),
    ("resize", Scenarios.Resize, "watching", false),
    ("player", Scenarios.Player, "playing", true),
    ("player-4k", Scenarios.Player4K, "playing", true),
    // Not a test: prints what UI Automation sees on Home, Library and the player, for writing scenarios.
    ("dump", Scenarios.Dump, "playing", true),
    // The person's own profile, sources and network, with no scene: only when asked for by name,
    // and only with their say-so, since it plays from their real sources and writes their history.
    ("real", Scenarios.Real, "", false),
    // The same profile, only moving around: the sidebar's pages, and titles opened from Home.
    ("real-nav", Scenarios.RealNav, "", false),
    // How it feels: every action filmed, for jumps and slow answers (Feel.cs). Only when asked for by name.
    ("feel", FeelScenario.All, "watching", false),
    // Scrolling every page that scrolls (ScrollScenario.cs); scroll-real on the person's own profile, scrolling only.
    ("scroll", ScrollScenario.Fixture, "watching", false),
    // The feel scenario on a copy of the person's profile (KAERI_PROFILE_ROOT): real sources and network.
    ("feel-real", FeelScenario.All, "", false),
    ("scroll-real", ScrollScenario.Real, "", false),
    // Downloads, Sources, Settings and Plugin stores kept in place under the engine's events (PagesScenario.cs). Only when asked for by name.
    ("pages", PagesScenario.All, "watching", true),
    ("pages-sources", PagesScenario.Sourced, "demo-and-store", false),
    // The wheel over Home's shelves and the grids against the wheel over what is not one, filmed frame by frame.
    ("wheel", FeelScenario.Wheels, "watching", false),
    // The player's checks: clicks on the bar, Space, dragging the seek and volume lines, the ending. By name only.
    ("player-checks", PlayerChecks.All, "playing", true),
    // The player and the party rehearsals filmed untouched, frame by frame (PlayerFilm.cs). By name only.
    ("player-film", PlayerFilm.All, "playing", true),
    ("party-film", PlayerFilm.Party, "watching", false),
};
if (options.Only is null)
    all = [.. all.Where(s => s.Name is not ("dump" or "real" or "real-nav" or "feel" or "feel-real" or "scroll" or "scroll-real" or "pages" or "pages-sources" or "wheel" or "player-checks" or "player-film" or "party-film"))];
Directory.CreateDirectory(options.Out);
var results = new List<Result>();
Console.WriteLine($"Kaeri.Drive: display refresh {Native.RefreshHz()} Hz; the pointer and keyboard are in use until this says done.");
foreach (var (name, body, scene, playerScene) in all.Where(s => options.Only is null || options.Only.Contains(s.Name)))
{
    var media = name == "player-4k" ? Path.Combine(options.Media, "hevc-2160p60.mkv") : Path.Combine(options.Media, "sdr-1080p24.mkv");
    Console.WriteLine($"-- {name}");
    var run = new Run(name, options, scene, playerScene ? media : null);
    try
    {
        await run.Start();
        await body(run);
    }
    catch (Exception e)
    {
        run.Note($"stopped: {e.GetType().Name}: {e.Message} at {e.StackTrace?.Split('\n').FirstOrDefault(l => l.Contains("Program.cs", StringComparison.Ordinal))?.Trim()}");
    }
    results.Add(await run.Finish());
}
Feel.Write(options.Out);
var report = Report.Write(results, Native.RefreshHz());
File.WriteAllText(Path.Combine(options.Out, "report.md"), report);
Console.WriteLine(report);
Console.WriteLine($"Kaeri.Drive: done. {Path.Combine(options.Out, "report.md")}");

sealed record Args(string App, string Media, string Out, HashSet<string>? Only, int Rounds)
{
    public static Args Parse(string[] args)
    {
        string? app = null, media = null, @out = null;
        HashSet<string>? only = null;
        var rounds = 3;
        for (var i = 0; i + 1 < args.Length; i += 2)
            switch (args[i])
            {
                case "--app": app = args[i + 1]; break;
                case "--media": media = args[i + 1]; break;
                case "--out": @out = args[i + 1]; break;
                case "--only": only = [.. args[i + 1].Split(',')]; break;
                case "--rounds": rounds = int.Parse(args[i + 1], CultureInfo.InvariantCulture); break;
            }
        if (app is null || media is null || @out is null)
            throw new ArgumentException("usage: --app <Kaeri.exe> --media <dir> --out <dir> [--only a,b] [--rounds n]");
        return new(Path.GetFullPath(app), Path.GetFullPath(media), Path.GetFullPath(@out), only, rounds);
    }
}

/// <summary>One step's span on the probe's clock.</summary>
/// <summary>One step's span on the probe's clock, and the moment of its click when it clicked.</summary>
sealed record Step(string Label, long Start, long End, long? Click = null);

sealed record Result(string Name, List<Step> Steps, List<string> Notes, string Dir, string? LogDir, DateTime Started);

/// <summary>One scenario's app: started, driven, closed.</summary>
sealed class Run(string name, Args options, string scene, string? media)
{
    public readonly string Name = name;
    public readonly Args Options = options;
    readonly List<Step> steps = [];
    readonly List<string> notes = [];
    readonly string dir = Path.Combine(options.Out, name);
    readonly UIA3Automation automation = new();
    Process? process;
    DateTime started;
    public Window Window { get; private set; } = null!;
    public nint Hwnd => Window.Properties.NativeWindowHandle.Value;

    public async Task Start()
    {
        if (Directory.Exists(dir))
            Directory.Delete(dir, true);
        Directory.CreateDirectory(dir);
        var info = new ProcessStartInfo(Options.App) { UseShellExecute = false };
        if (scene.Length > 0)
            info.ArgumentList.Add($"--scene={scene}");
        // A given size also keeps the run from saving it as the person's window place.
        info.ArgumentList.Add("--size=2560x1392");
        // A party rehearsal, for party-film.
        if (Environment.GetEnvironmentVariable("KAERI_DRIVE_SHOW") is { Length: > 0 } show)
            info.ArgumentList.Add($"--show={show}");
        if (media is not null)
        {
            info.ArgumentList.Add($"--media={media}");
            // No --play: the scenario clicks Play on Home, as a person does, which also gives the
            // window the keyboard; a launched window may not have it.
            // The pages scenario wants the fixture's three downloads, the finished one at the clip.
            if (Name == "pages")
                info.ArgumentList.Add("--downloads");
        }
        info.Environment["KAERI_PROBE_DIR"] = dir;
        if (scene.Length > 0)
        {
            info.Environment["KAERI_FIXTURES_HOME_WORLD"] = "1";
            info.Environment["KAERI_FIXTURES_CATALOGUE"] = "1";
        }
        // One copy runs at a time: a launch while the last scenario's copy is still closing would
        // hand over to it and exit.
        var name = Path.GetFileNameWithoutExtension(Options.App);
        for (var i = 0; i < 40 && Process.GetProcessesByName(name).Any(p => Run.Safe(() => p.MainModule?.FileName) == Options.App); i++)
            await Task.Delay(250);
        // Windows keeps the last copy's single-instance key for a moment after it exits, and a
        // launch then hands over to it and exits at once: try again until one stays.
        for (var attempt = 1; ; attempt++)
        {
            started = DateTime.Now;
            process = Process.Start(info)!;
            if (!process.WaitForExit(3000))
                break;
            if (attempt == 6)
                throw new InvalidOperationException("the app exited at once six times");
            await Task.Delay(1500);
        }
        // By id: FlaUI closes a Process object it is handed, and this one must outlive it.
        var app = Application.Attach(process.Id);
        Window = app.GetMainWindow(automation, TimeSpan.FromSeconds(30)) ?? throw new InvalidOperationException("no window");
        await Task.Delay(4000);
        Front();
    }

    public void Front()
    {
        Native.SetForegroundWindow(Hwnd);
        // Not Window.Focus(): UI Automation would move the keyboard focus to the window itself,
        // off the control that has it, and the player's keys would not reach the player.
    }

    long? clicked;

    /// <summary>
    /// The pointer glides to <paramref name="target"/>, then clicks, and the click's moment is the
    /// step's: what the report times from, so the glide is not counted against the app.
    /// </summary>
    public void Click(AutomationElement target)
    {
        var r = target.BoundingRectangle;
        Mouse.MoveTo(new Point(r.Left + r.Width / 2, r.Top + r.Height / 2));
        Thread.Sleep(80);
        clicked = Stopwatch.GetTimestamp();
        Mouse.Click();
    }

    /// <summary>Runs <paramref name="act"/> as a named step, then checks the app is still there and answering.</summary>
    public async Task Step(string label, Func<Task> act)
    {
        Alive();
        clicked = null;
        var start = Stopwatch.GetTimestamp();
        await act();
        steps.Add(new(label, start, Stopwatch.GetTimestamp(), clicked));
        Alive();
    }

    public Task Step(string label, Action act) => Step(label, () => { act(); return Task.CompletedTask; });

    void Alive()
    {
        if (process!.HasExited)
            throw new InvalidOperationException($"the app exited with code {process.ExitCode:X}");
        if (!Native.Answers(Hwnd, 5000))
        {
            Note("the window did not answer for 5 s");
            if (!Native.Answers(Hwnd, 20000))
                throw new InvalidOperationException("the window stopped answering");
        }
    }

    /// <summary>Records <paramref name="text"/>, with a capture of the app's window beside the report.</summary>
    public void Note(string text)
    {
        var shot = Capture();
        notes.Add(shot is null ? text : $"{text} ({shot})");
        Console.WriteLine($"   {text}");
    }

    /// <summary>The app's window alone, whatever is in front of it, as note-N.png in the scenario's folder.</summary>
    string? Capture()
    {
        try
        {
            if (process is not { HasExited: false } || Window is null)
                return null;
            var r = Window.BoundingRectangle;
            using var bitmap = new Bitmap(r.Width, r.Height);
            using (var g = Graphics.FromImage(bitmap))
            {
                var dc = g.GetHdc();
                Native.PrintWindow(Hwnd, dc, 2);
                g.ReleaseHdc(dc);
            }
            var name = $"note-{notes.Count + 1}.png";
            bitmap.Save(Path.Combine(dir, name), System.Drawing.Imaging.ImageFormat.Png);
            return name;
        }
        catch (Exception)
        {
            return null;
        }
    }

    public AutomationElement? Named(string name) =>
        Window.FindFirstDescendant(cf => cf.ByName(name));

    /// <summary>The title bar's back button: the first button in the window's top 48 px.</summary>
    public void TitleBack()
    {
        var top = Window.BoundingRectangle.Top;
        var back = Window.FindAllDescendants(cf => cf.ByControlType(ControlType.Button))
            .FirstOrDefault(b => Safe(() => b.BoundingRectangle) is { Height: > 0 } r && r.Top < top + 48 && r.Left < Window.BoundingRectangle.Left + 60);
        if (back is null)
            Note("no back button in the title bar");
        else
            Click(back);
    }

    public AutomationElement? NameStarts(string start) =>
        Window.FindAllDescendants().FirstOrDefault(e => Safe(() => e.Name)?.StartsWith(start, StringComparison.Ordinal) == true);

    public AutomationElement[] Covers() =>
        [.. Window.FindAllDescendants(cf => cf.ByControlType(ControlType.ListItem).Or(cf.ByControlType(ControlType.Button)))
            // Grid items are list items; Home's cards and posters are buttons. Either way, cover sized.
            .Where(e => Safe(() => e.BoundingRectangle) is { Width: > 100, Height: > 150 } r && Inside(r))];

    public bool Inside(Rectangle r)
    {
        var w = Window.BoundingRectangle;
        return r.Top > w.Top + 60 && r.Bottom < w.Bottom - 10 && r.Left > w.Left + 200 && r.Right < w.Right - 10;
    }

    public static T? Safe<T>(Func<T> read)
    {
        try { return read(); }
        catch (Exception) { return default; }
    }

    public Point Centre => new(Window.BoundingRectangle.Left + Window.BoundingRectangle.Width / 2, Window.BoundingRectangle.Top + Window.BoundingRectangle.Height / 2);

    public async Task<Result> Finish()
    {
        try
        {
            if (process is { HasExited: false })
            {
                Window.Close();
                if (!process.WaitForExit(8000))
                {
                    Note("the app did not close within 8 s of Close");
                    process.Kill();
                }
            }
        }
        catch (Exception e)
        {
            Note($"closing: {e.Message}");
            if (process is { HasExited: false })
                process.Kill();
        }
        await Task.Delay(800);
        var log = scene.Length == 0
            ? Environment.GetEnvironmentVariable("KAERI_PROFILE_ROOT") is { Length: > 0 } copy ? copy : Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "kaeri")
            : Directory.GetDirectories(Path.GetTempPath(), "kaeri-app-*")
                .Select(d => new DirectoryInfo(d))
                .Where(d => d.CreationTime >= started.AddSeconds(-2))
                .OrderBy(d => d.CreationTime)
                .FirstOrDefault()?.FullName;
        automation.Dispose();
        return new(Name, steps, notes, dir, log, started);
    }
}

static class Scenarios
{
    static readonly string[] Pages = ["Home", "Calendar", "Seasonal", "Search", "Library", "Downloads", "Watch together", "Sources", "Plugin stores"];

    public static async Task Nav(Run run)
    {
        for (var round = 0; round < run.Options.Rounds; round++)
            foreach (var page in Pages)
                await run.Step($"open {page}", async () =>
                {
                    run.Click(run.Named(page) ?? throw new InvalidOperationException($"no {page} item"));
                    await Task.Delay(1200);
                });
    }

    public static async Task Home(Run run)
    {
        Mouse.MoveTo(run.Centre);
        for (var round = 0; round < run.Options.Rounds; round++)
        {
            await run.Step("scroll Home down, a notch at a time", async () =>
            {
                for (var i = 0; i < 14; i++)
                {
                    Native.Wheel(-1);
                    await Task.Delay(120);
                }
                await Task.Delay(600);
            });
            await run.Step("scroll Home up, fast", async () =>
            {
                for (var i = 0; i < 14; i++)
                {
                    Native.Wheel(1);
                    await Task.Delay(25);
                }
                await Task.Delay(800);
            });
        }
        await HoverRow(run, "hover Home covers");
    }

    public static async Task Hover(Run run)
    {
        run.Named("Seasonal")?.Click(moveMouse: true);
        await Task.Delay(2500);
        for (var round = 0; round < run.Options.Rounds; round++)
            await HoverRow(run, "hover Seasonal covers, resting on each");
        await run.Step("sweep across covers without resting", async () =>
        {
            Mouse.MovePixelsPerMillisecond = 2;
            foreach (var cover in run.Covers().Take(10))
                Mouse.MoveTo(Mid(cover.BoundingRectangle));
            Mouse.MovePixelsPerMillisecond = 0.5;
            await Task.Delay(1500);
        });
    }

    static async Task HoverRow(Run run, string label)
    {
        var covers = run.Covers().Take(8).ToArray();
        if (covers.Length == 0)
        {
            run.Note($"{label}: no covers on screen");
            return;
        }
        await run.Step(label, async () =>
        {
            Mouse.MovePixelsPerMillisecond = 0.6;
            foreach (var cover in covers)
            {
                Mouse.MoveTo(Mid(cover.BoundingRectangle));
                await Task.Delay(1400);
            }
            Mouse.MoveTo(new Point(run.Window.BoundingRectangle.Left + 100, run.Window.BoundingRectangle.Bottom - 200));
            await Task.Delay(800);
        });
    }

    public static async Task Title(Run run)
    {
        run.Named("Seasonal")?.Click(moveMouse: true);
        await Task.Delay(2000);
        for (var round = 0; round < run.Options.Rounds * 2; round++)
        {
            var covers = run.Covers();
            if (covers.Length == 0)
            {
                run.Note("no covers in Seasonal");
                return;
            }
            var cover = covers[round % covers.Length];
            await run.Step("open a title from Seasonal", async () =>
            {
                run.Click(cover);
                await Task.Delay(2500);
            });
            await run.Step("scroll the title page down and up", async () =>
            {
                Mouse.MoveTo(run.Centre);
                for (var i = 0; i < 10; i++)
                {
                    Native.Wheel(-1);
                    await Task.Delay(90);
                }
                for (var i = 0; i < 10; i++)
                {
                    Native.Wheel(1);
                    await Task.Delay(90);
                }
                await Task.Delay(700);
            });
            await run.Step("back to Seasonal", async () =>
            {
                run.TitleBack();
                await Task.Delay(1500);
            });
        }
    }

    public static async Task Resize(Run run)
    {
        var sizes = new[] { (2560, 1392), (1280, 800), (1920, 1080), (900, 700), (2560, 1392), (1600, 1000) };
        for (var round = 0; round < run.Options.Rounds; round++)
            foreach (var (w, h) in sizes)
                await run.Step($"resize to {w}x{h}", async () =>
                {
                    Native.Size(run.Hwnd, w, h);
                    await Task.Delay(1200);
                });
        await run.Step("drag the edge from 2560 to 1100 and back", async () => await Drag(run));
        run.Named("Library")?.Click(moveMouse: true);
        await Task.Delay(2000);
        await run.Step("drag the edge on Library", async () => await Drag(run));
    }

    static async Task Drag(Run run)
    {
        for (var w = 2560; w >= 1100; w -= 24)
        {
            Native.Size(run.Hwnd, w, 1392);
            await Task.Delay(16);
        }
        for (var w = 1100; w <= 2560; w += 24)
        {
            Native.Size(run.Hwnd, w, 1392);
            await Task.Delay(16);
        }
        await Task.Delay(800);
    }

    public static async Task Player(Run run)
    {
        await run.Step("click Play, first playback starts (cold shader cache)", async () => await PlayFromHome(run, 10000));
        await PlayerAbuse(run);
        for (var i = 0; i < run.Options.Rounds + 2; i++)
        {
            await run.Step("close the player", async () =>
            {
                run.Front();
                Keyboard.Type(VirtualKeyShort.ESCAPE);
                await Task.Delay(2000);
            });
            await run.Step("play again from Home", async () => await PlayFromHome(run, 5000));
        }
        await run.Step("play untouched for 20 s", async () =>
        {
            Mouse.MoveTo(new Point(run.Window.BoundingRectangle.Right - 300, run.Window.BoundingRectangle.Top + 300));
            await Task.Delay(20000);
        });
        await Stats(run, "stats after the run");
    }

    /// <summary>Clicks Home's Resume or Play, then waits <paramref name="ms"/> for the player.</summary>
    internal static async Task PlayFromHome(Run run, int ms)
    {
        AutomationElement? button = null;
        for (var i = 0; i < 20 && button is null; i++)
        {
            button = run.NameStarts("Resume") ?? run.NameStarts("Play episode") ?? ContinueCard(run);
            if (button is null)
                await Task.Delay(500);
        }
        if (button is null)
        {
            run.Note("no Resume or Play button on Home");
            return;
        }
        run.Click(button);
        await Task.Delay(ms);
    }

    /// <summary>The first Continue card on Home: its name is "Title, Episode N next".</summary>
    static AutomationElement? ContinueCard(Run run) =>
        run.Window.FindAllDescendants(cf => cf.ByControlType(ControlType.Button))
            .FirstOrDefault(b => Run.Safe(() => b.Name)?.EndsWith(" next", StringComparison.Ordinal) == true);

    /// <summary>
    /// The person's own setup: the first thing Home offers to play, from their real sources, then
    /// the same handling as the fixture player gets. A real source takes a while to answer, so
    /// each start waits longer, and a capture is kept of what showed.
    /// </summary>
    public static async Task Real(Run run)
    {
        await Task.Delay(4000);
        await run.Step("play the first Continue title from a real source", async () => await PlayFromHome(run, 30000));
        run.Note("30 s after Play");
        await PlayerAbuse(run);
        for (var i = 0; i < 2; i++)
        {
            await run.Step("close the player", async () =>
            {
                run.Front();
                Keyboard.Type(VirtualKeyShort.ESCAPE);
                await Task.Delay(2500);
            });
            await run.Step("play again from Home", async () => await PlayFromHome(run, 15000));
        }
        await run.Step("play untouched for 30 s", async () =>
        {
            Mouse.MoveTo(new Point(run.Window.BoundingRectangle.Right - 300, run.Window.BoundingRectangle.Top + 300));
            await Task.Delay(30000);
        });
        await Stats(run, "stats after the run");
        run.Note("at the end");
    }

    /// <summary>
    /// The person's own profile, moving around: the sidebar's pages, titles opened from Home, and
    /// Play pressed on Continue twice, for how soon the starting screen shows.
    /// </summary>
    public static async Task RealNav(Run run)
    {
        await Task.Delay(5000);
        await Nav(run);
        for (var i = 0; i < 2; i++)
        {
            run.Named("Home")?.Click(moveMouse: true);
            await Task.Delay(3000);
            await run.Step("press Play on Continue: the starting screen", async () => await PlayFromHome(run, 12000));
            await run.Step("close the player", async () =>
            {
                run.Front();
                Keyboard.Type(VirtualKeyShort.ESCAPE);
                await Task.Delay(2500);
            });
        }
        run.Named("Home")?.Click(moveMouse: true);
        await Task.Delay(3000);
        for (var i = 0; i < 4; i++)
        {
            // Posters only: a Continue card ("…, Episode N next") would play.
            var covers = run.Covers().Where(c => Run.Safe(() => c.Name)?.EndsWith(" next", StringComparison.Ordinal) != true).ToArray();
            if (covers.Length == 0)
            {
                run.Note("no posters on Home");
                return;
            }
            var cover = covers[i * 3 % covers.Length];
            await run.Step("open a title from Home", async () =>
            {
                run.Click(cover);
                await Task.Delay(3000);
            });
            await run.Step("back to Home", async () =>
            {
                run.TitleBack();
                await Task.Delay(2500);
            });
        }
    }

    public static async Task Dump(Run run)
    {
        await Task.Delay(8000);
        Print(run, "player");
        run.Front();
        Keyboard.Type(VirtualKeyShort.KEY_I);
        await Task.Delay(2500);
        Print(run, "player with stats");
        Keyboard.Type(VirtualKeyShort.ESCAPE);
        await Task.Delay(3000);
        Print(run, "home");
        run.Named("Library")?.Click(moveMouse: true);
        await Task.Delay(3000);
        Print(run, "library");
    }

    static void Print(Run run, string label)
    {
        var lines = run.Window.FindAllDescendants()
            .Select(e => (Type: Run.Safe(() => e.ControlType.ToString()), Class: Run.Safe(() => e.ClassName), Name: Run.Safe(() => e.Name) ?? "", Box: Run.Safe(() => e.BoundingRectangle)))
            .Where(e => e.Box.Width > 0)
            .Select(e => $"{e.Type} | {e.Class} | {(e.Name.Length > 70 ? e.Name[..70] : e.Name)} | {e.Box.Width}x{e.Box.Height}")
            .Distinct()
            .ToArray();
        File.WriteAllLines(Path.Combine(run.Options.Out, $"dump-{label.Replace(' ', '-')}.txt"), lines);
        run.Note($"{label}: {lines.Length} elements written");
    }

    public static async Task Player4K(Run run)
    {
        await run.Step("click Play, 4K 60 fps HEVC starts", async () => await PlayFromHome(run, 8000));
        await run.Step("play 4K untouched for 20 s", async () => await Task.Delay(20000));
        await Stats(run, "stats while 4K plays");
        await run.Step("seek through 4K", async () =>
        {
            run.Front();
            for (var i = 0; i < 12; i++)
            {
                Keyboard.Type(VirtualKeyShort.RIGHT);
                await Task.Delay(250);
            }
            await Task.Delay(3000);
        });
        await run.Step("fullscreen 4K and back", async () =>
        {
            Keyboard.Type(VirtualKeyShort.KEY_F);
            await Task.Delay(4000);
            Keyboard.Type(VirtualKeyShort.KEY_F);
            await Task.Delay(3000);
        });
    }

    static async Task PlayerAbuse(Run run)
    {
        run.Front();
        await Stats(run, "stats after starting");
        await run.Step("seek forward 20 times fast", async () =>
        {
            for (var i = 0; i < 20; i++)
            {
                Keyboard.Type(VirtualKeyShort.RIGHT);
                await Task.Delay(80);
            }
            await Task.Delay(2500);
        });
        await run.Step("seek back 20 times fast", async () =>
        {
            for (var i = 0; i < 20; i++)
            {
                Keyboard.Type(VirtualKeyShort.LEFT);
                await Task.Delay(80);
            }
            await Task.Delay(2500);
        });
        await run.Step("jump 30 s five times", async () =>
        {
            for (var i = 0; i < 5; i++)
            {
                Keyboard.TypeSimultaneously(VirtualKeyShort.SHIFT, VirtualKeyShort.RIGHT);
                await Task.Delay(600);
            }
            await Task.Delay(2000);
        });
        await run.Step("pause and play 10 times", async () =>
        {
            for (var i = 0; i < 20; i++)
            {
                Keyboard.Type(VirtualKeyShort.SPACE);
                await Task.Delay(400);
            }
            await Task.Delay(1500);
        });
        await run.Step("volume up and down", async () =>
        {
            for (var i = 0; i < 8; i++)
            {
                Keyboard.Type(VirtualKeyShort.DOWN);
                await Task.Delay(120);
            }
            for (var i = 0; i < 8; i++)
            {
                Keyboard.Type(VirtualKeyShort.UP);
                await Task.Delay(120);
            }
            await Task.Delay(1500);
        });
        for (var i = 0; i < 3; i++)
            await run.Step("fullscreen and back", async () =>
            {
                Keyboard.Type(VirtualKeyShort.KEY_F);
                await Task.Delay(2500);
                Keyboard.Type(VirtualKeyShort.KEY_F);
                await Task.Delay(2500);
            });
        await run.Step("controls: show by moving, then let them hide", async () =>
        {
            var w = run.Window.BoundingRectangle;
            Mouse.MovePixelsPerMillisecond = 1;
            Mouse.MoveTo(new Point(w.Left + w.Width / 2, w.Top + w.Height / 2));
            Mouse.MoveTo(new Point(w.Left + w.Width / 3, w.Top + w.Height / 3));
            await Task.Delay(3500);
        });
        await run.Step("sweep the seek preview along the timeline", async () =>
        {
            var w = run.Window.BoundingRectangle;
            var y = w.Bottom - 70;
            Mouse.MoveTo(new Point(w.Left + 300, y));
            await Task.Delay(800);
            var line = run.Window.FindFirstDescendant(cf => cf.ByControlType(ControlType.Slider));
            if (line is not null && Run.Safe(() => line.BoundingRectangle) is { Width: > 100 } r)
                y = r.Top + r.Height / 2;
            Mouse.MovePixelsPerMillisecond = 0.4;
            for (var x = w.Left + 300; x < w.Right - 300; x += 120)
            {
                Mouse.MoveTo(new Point(x, y));
                await Task.Delay(150);
            }
            Mouse.MovePixelsPerMillisecond = 0.5;
            Mouse.MoveTo(run.Centre);
            await Task.Delay(1500);
        });
        await run.Step("resize the window while playing", async () =>
        {
            foreach (var (w, h) in new[] { (1280, 720), (1920, 1080), (1000, 600), (2560, 1392) })
            {
                Native.Size(run.Hwnd, w, h);
                await Task.Delay(1500);
            }
        });
        await run.Step("drag the edge while playing", async () =>
        {
            for (var w = 2560; w >= 1200; w -= 30)
            {
                Native.Size(run.Hwnd, w, 1392);
                await Task.Delay(16);
            }
            for (var w = 1200; w <= 2560; w += 30)
            {
                Native.Size(run.Hwnd, w, 1392);
                await Task.Delay(16);
            }
            await Task.Delay(1500);
        });
        await run.Step("mini player and back", async () =>
        {
            run.Front();
            Keyboard.Type(VirtualKeyShort.KEY_P);
            await Task.Delay(3000);
            Keyboard.Type(VirtualKeyShort.KEY_P);
            await Task.Delay(3000);
        });
    }

    /// <summary>Stats for nerds up, its rows read, and away again.</summary>
    static async Task Stats(Run run, string label)
    {
        await run.Step(label, async () =>
        {
            run.Front();
            Keyboard.Type(VirtualKeyShort.KEY_I);
            await Task.Delay(2500);
            var rows = run.Window.FindAllDescendants()
                .Select(e => Run.Safe(() => e.Name) ?? "")
                .Where(n => n.Contains(':') && (n.Contains("rame", StringComparison.Ordinal) || n.Contains("ropped", StringComparison.Ordinal)
                    || n.Contains("elayed", StringComparison.Ordinal) || n.Contains("ync", StringComparison.Ordinal) || n.Contains("ecoder", StringComparison.Ordinal)))
                .Distinct()
                .ToArray();
            run.Note($"{label}: " + (rows.Length == 0 ? "no stats rows found" : string.Join(" | ", rows)));
            Keyboard.Type(VirtualKeyShort.KEY_I);
            await Task.Delay(800);
        });
    }

    static Point Mid(Rectangle r) => new(r.Left + r.Width / 2, r.Top + r.Height / 2);
}

static class Report
{
    public static string Write(List<Result> results, int hz)
    {
        var period = 1000.0 / hz;
        var md = new StringBuilder();
        md.AppendLine($"# Kaeri.Drive report, {DateTime.Now:yyyy-MM-dd HH:mm}");
        md.AppendLine();
        md.AppendLine($"Display {hz} Hz ({period:F1} ms a frame). Frame times are the UI thread's composition frames while");
        md.AppendLine("something moved (gaps over 500 ms are idle, not counted). Missed: frames that took more than 1.5 refreshes.");
        md.AppendLine("Stall: the longest the UI thread kept a 10 ms watchdog waiting (40 ms or more are recorded).");
        md.AppendLine();
        foreach (var r in results)
        {
            var frequency = Stopwatch.Frequency;
            var frames = ReadFrames(Path.Combine(r.Dir, "frames.csv"));
            var stalls = ReadPairs(Path.Combine(r.Dir, "stalls.csv"));
            var memory = ReadMemory(Path.Combine(r.Dir, "memory.csv"));
            var marks = ReadMarks(Path.Combine(r.Dir, "marks.csv"));
            md.AppendLine($"## {r.Name}");
            md.AppendLine();
            md.AppendLine("| Step | Page shown ms | Frames | p50 ms | p95 ms | p99 ms | Worst ms | Missed | Longest stall ms |");
            md.AppendLine("|---|---:|---:|---:|---:|---:|---:|---:|---:|");
            foreach (var s in r.Steps)
            {
                var times = new List<double>();
                long? last = null;
                foreach (var f in frames.Where(f => f >= s.Start && f <= s.End))
                {
                    if (last is { } l)
                    {
                        var ms = (f - l) * 1000.0 / frequency;
                        if (ms <= 500)
                            times.Add(ms);
                    }
                    last = f;
                }
                times.Sort();
                var stall = stalls.Where(p => p.At >= s.Start && p.At <= s.End).Select(p => p.Ms).DefaultIfEmpty(0).Max();
                var missed = times.Count(t => t > period * 1.5);
                var shown = marks.Where(m => m.At >= s.Start && m.At <= s.End && m.What.StartsWith("loaded ", StringComparison.Ordinal)).Select(m => (m.At - (s.Click ?? s.Start)) * 1000.0 / frequency).DefaultIfEmpty(-1).First();
                md.AppendLine($"| {s.Label} | {(shown >= 0 ? shown.ToString("F0", CultureInfo.InvariantCulture) : "-")} | {times.Count} | {Pct(times, 50)} | {Pct(times, 95)} | {Pct(times, 99)} | {(times.Count > 0 ? times[^1].ToString("F0", CultureInfo.InvariantCulture) : "-")} | {(times.Count > 0 ? $"{missed} ({100.0 * missed / times.Count:F0}%)" : "-")} | {(stall > 0 ? stall.ToString("F0", CultureInfo.InvariantCulture) : "-")} |");
            }
            md.AppendLine();
            var big = stalls.Where(p => p.Ms >= 250).ToArray();
            if (big.Length > 0)
                md.AppendLine($"UI thread stalls of 250 ms or more: {big.Length}, the longest {big.Max(p => p.Ms):F0} ms.");
            if (memory.Count > 1)
                md.AppendLine($"Memory: private {memory[0].Private} MB at start, {memory[^1].Private} MB at the end, {memory.Max(m => m.Private)} MB at most; handles {memory[0].Handles} to {memory[^1].Handles}.");
            foreach (var note in r.Notes)
                md.AppendLine($"- {note}");
            md.AppendLine();
            md.AppendLine(LogLines(r.LogDir, r.Started));
        }
        return md.ToString();
    }

    static string Pct(List<double> sorted, int p) =>
        sorted.Count == 0 ? "-" : sorted[Math.Min(sorted.Count - 1, (int)Math.Ceiling(p / 100.0 * sorted.Count) - 1)].ToString("F1", CultureInfo.InvariantCulture);

    static long[] ReadFrames(string path) =>
        File.Exists(path)
            ? [.. File.ReadLines(path).Where(l => l.Length > 0 && l[0] != '#').Select(l => long.Parse(l, CultureInfo.InvariantCulture))]
            : [];

    static List<(long At, double Ms)> ReadPairs(string path) =>
        File.Exists(path)
            ? [.. File.ReadLines(path).Skip(1).Where(l => l.Contains(',')).Select(l => l.Split(',')).Select(p => (long.Parse(p[0], CultureInfo.InvariantCulture), double.Parse(p[1], CultureInfo.InvariantCulture)))]
            : [];

static List<(long At, string What)> ReadMarks(string path) =>        File.Exists(path)            ? [.. File.ReadLines(path).Skip(1).Where(l => l.Contains(',')).Select(l => l.Split(',', 2)).Select(p => (long.Parse(p[0], CultureInfo.InvariantCulture), p[1]))]            : [];
    static List<(long At, long Private, long Handles)> ReadMemory(string path) =>
        File.Exists(path)
            ? [.. File.ReadLines(path).Skip(1).Where(l => l.Contains(',')).Select(l => l.Split(',')).Select(p => (long.Parse(p[0], CultureInfo.InvariantCulture), long.Parse(p[1], CultureInfo.InvariantCulture), long.Parse(p[3], CultureInfo.InvariantCulture)))]
            : [];

    /// <summary>The app's own warnings and errors from the fixture run's log, counted by message.</summary>
    /// <summary>A log's lines, read while the app that writes it may still have it open.</summary>
    static IEnumerable<string> Shared(string path)
    {
        using var stream = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete);
        using var reader = new StreamReader(stream);
        while (reader.ReadLine() is { } line)
            yield return line;
    }

    static string LogLines(string? root, DateTime since)
    {
        var logs = root is null || !Directory.Exists(Path.Combine(root, "cache", "logs")) ? [] : Directory.GetFiles(Path.Combine(root, "cache", "logs"));
        if (logs.Length == 0)
            return "No app log found.\n";
        // The log is UTC; a profile's log holds earlier runs too, so only this run's lines count.
        var from = since.ToUniversalTime().AddSeconds(-2).ToString("yyyy-MM-ddTHH:mm:ss", CultureInfo.InvariantCulture);
        var counts = new Dictionary<string, int>();
        foreach (var line in logs.SelectMany(Shared))
        {
            if (line.Length < 19 || string.CompareOrdinal(line, 0, from, 0, 19) < 0)
                continue;
            var at = line.IndexOf(" WARN ", StringComparison.Ordinal);
            if (at < 0)
                at = line.IndexOf(" ERROR ", StringComparison.Ordinal);
            if (at < 0)
                continue;
            var text = line[(at + 1)..];
            text = text.Length > 180 ? text[..180] : text;
            counts[text] = counts.GetValueOrDefault(text) + 1;
        }
        if (counts.Count == 0)
            return "The app logged no warnings or errors.\n";
        var md = new StringBuilder("App log warnings and errors:\n\n");
        foreach (var (text, n) in counts.OrderByDescending(c => c.Value).Take(15))
            md.AppendLine($"- {n}x `{text.Replace('`', '\'')}`");
        return md.ToString();
    }
}

static partial class Native
{
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    struct DevMode
    {
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string DeviceName;
        public short SpecVersion, DriverVersion, Size, DriverExtra;
        public int Fields, PositionX, PositionY, DisplayOrientation, DisplayFixedOutput;
        public short Color, Duplex, YResolution, TTOption, Collate;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string FormName;
        public short LogPixels;
        public int BitsPerPel, PelsWidth, PelsHeight, DisplayFlags, DisplayFrequency;
        public int IcmMethod, IcmIntent, MediaType, DitherType, Reserved1, Reserved2, PanningWidth, PanningHeight;
    }

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    static extern bool EnumDisplaySettingsW(string? device, int mode, ref DevMode devMode);

    public static int RefreshHz()
    {
        var mode = new DevMode { Size = (short)Marshal.SizeOf<DevMode>() };
        return EnumDisplaySettingsW(null, -1, ref mode) && mode.DisplayFrequency > 1 ? mode.DisplayFrequency : 60;
    }

    [LibraryImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static partial bool SetForegroundWindow(nint hwnd);

    [LibraryImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static partial bool SetWindowPos(nint hwnd, nint after, int x, int y, int w, int h, uint flags);

    [LibraryImport("user32.dll")]
    private static partial void mouse_event(uint flags, int dx, int dy, uint data, nuint extra);

    /// <summary>
    /// The wheel by <paramref name="notches"/>, down when negative. FlaUI's Mouse.Scroll turns a
    /// negative amount into an unsigned one by a saturating cast, so a scroll down sends nothing.
    /// </summary>
    public static void Wheel(int notches, bool sideways = false) =>
        mouse_event(sideways ? 0x01000u : 0x0800u, 0, 0, unchecked((uint)(notches * 120)), 0);

    /// <summary>The window's size, its top left kept.</summary>
    public static void Size(nint hwnd, int w, int h) => SetWindowPos(hwnd, 0, 0, 0, w, h, 0x0002 | 0x0004 | 0x0010);

    [LibraryImport("user32.dll", EntryPoint = "SendMessageTimeoutW")]
    private static partial nint SendMessageTimeout(nint hwnd, uint msg, nint w, nint l, uint flags, uint timeout, out nint result);

    /// <summary>Whether the window's thread handles a message within <paramref name="ms"/>.</summary>
    public static bool Answers(nint hwnd, uint ms) => SendMessageTimeout(hwnd, 0, 0, 0, 0x0002, ms, out _) != 0;

    /// <summary>Draws the window into <paramref name="dc"/>; flag 2 takes DirectComposition content too.</summary>
    [LibraryImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static partial bool PrintWindow(nint hwnd, nint dc, uint flags);
}
