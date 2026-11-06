// The feel scenario: every page from the sidebar, then on each the actions that change what it
// shows, filmed by Feel. Fixture scene "watching", so it never touches the person's profile.

using System.Drawing;
using FlaUI.Core.AutomationElements;
using FlaUI.Core.Definitions;
using FlaUI.Core.Input;
using FlaUI.Core.WindowsAPI;

static class FeelScenario
{
    static readonly string[] Pages = ["Calendar", "Seasonal", "Search", "Library", "Downloads", "Watch together", "Sources", "Plugin stores", "Settings", "Home"];

    static AutomationElement? Nav(Run run, string page) =>
        run.Window.FindAllDescendants(cf => cf.ByName(page).And(cf.ByControlType(ControlType.ListItem))).FirstOrDefault();

    /// <summary>The first element named <paramref name="name"/> that is on the page or in the title bar.</summary>
    static AutomationElement? Find(Run run, string name, ControlType type) =>
        run.Window.FindAllDescendants(cf => cf.ByName(name).And(cf.ByControlType(type)))
            .FirstOrDefault(e => Run.Safe(() => e.BoundingRectangle) is { Width: > 0 } r
                && (run.Inside(r) || r.Top < run.Window.BoundingRectangle.Top + 60));

    static AutomationElement? Starts(Run run, string start, ControlType type) =>
        run.Window.FindAllDescendants(cf => cf.ByControlType(type))
            .FirstOrDefault(e => Run.Safe(() => e.Name)?.StartsWith(start, StringComparison.Ordinal) == true
                && Run.Safe(() => e.BoundingRectangle) is { Width: > 0 } r && run.Inside(r));

    static AutomationElement? Poster(Run run) =>
        run.Covers().FirstOrDefault(c => Run.Safe(() => c.BoundingRectangle) is { } r && r.Height > r.Width * 1.2);

    internal static async Task Go(Run run, string page)
    {
        // A step that opened the player (a card under a hidden button, say) hides the sidebar:
        // Escape closes it first.
        for (var i = 0; i < 3 && Nav(run, page) is null; i++)
        {
            Keyboard.Type(VirtualKeyShort.ESCAPE);
            await Task.Delay(1500);
        }
        await Feel.Click(run, $"sidebar: {page}", Nav(run, page));
        await Task.Delay(600);
    }

    /// <summary>A button that is on screen and wide enough to be meant for a click, not one a row hides.</summary>
    static AutomationElement? Shown(Run run, string name) =>
        run.Window.FindAllDescendants(cf => cf.ByName(name).And(cf.ByControlType(ControlType.Button)))
            .FirstOrDefault(e => Run.Safe(() => !e.IsOffscreen) && Run.Safe(() => e.BoundingRectangle) is { Width: >= 30 } r && run.Inside(r));

    static Task Key(Run run, string label, VirtualKeyShort key) =>
        Feel.Film(run, label, () => { Keyboard.Type(key); return Task.CompletedTask; });

    static Task Wheel(Run run, int notches, Point? at = null) => Feel.Film(run, $"wheel {notches}",
        async () =>
        {
            for (var i = 0; i < Math.Abs(notches); i++)
            {
                Native.Wheel(notches < 0 ? -1 : 1);
                await Task.Delay(40);
            }
        },
        () => { Mouse.MoveTo(at ?? run.Centre); return Task.CompletedTask; });

    /// <summary>A slow wheel, a notch at a time, filmed frame by frame to see whether each notch is a jump or a glide.</summary>
    static Task SlowWheel(Run run, string label, Func<Point?> at, int notches) => Feel.Film(run, label,
        () =>
        {
            // The notches go on while the filming runs, so each one's frames are seen.
            _ = Task.Run(async () =>
            {
                for (var i = 0; i < Math.Abs(notches); i++)
                {
                    Native.Wheel(notches < 0 ? -1 : 1);
                    await Task.Delay(300);
                }
            });
            return Task.CompletedTask;
        },
        () => { Mouse.MoveTo(at() ?? run.Centre); return Task.CompletedTask; }, 6000, wheel: true);

    static Point? Middle(AutomationElement? e) =>
        e is not null && Run.Safe(() => e.BoundingRectangle) is { Width: > 0 } r ? new Point(r.Left + r.Width / 2, r.Top + r.Height / 2) : null;

    /// <summary>The wheel over a shelf against the wheel over what is not one, in a window small enough for Home to scroll.</summary>
    public static async Task Wheels(Run run)
    {
        Native.Size(run.Hwnd, 1600, 900);
        await Task.Delay(2000);
        var w = run.Window.BoundingRectangle;
        // The window in front, by a click on the page's title, so the wheel reaches it.
        Native.SetForegroundWindow(run.Hwnd);
        Mouse.MoveTo(new Point(w.Left + 320, w.Top + 92));
        Mouse.Click();
        await Task.Delay(500);
        // The spotlight's text, top left of the page: no shelf under the pointer.
        await SlowWheel(run, "wheel slowly over the spotlight (no shelf)", () => new Point(w.Left + 420, w.Top + 260), -3);
        await SlowWheel(run, "wheel slowly back up over the spotlight", () => new Point(w.Left + 420, w.Top + 260), 3);
        await SlowWheel(run, "wheel slowly over a shelf's poster", () => Middle(Poster(run)), -3);
        await SlowWheel(run, "wheel slowly back up over a shelf's poster", () => Middle(Poster(run)), 3);
        await Go(run, "Seasonal");
        await SlowWheel(run, "seasonal: wheel slowly over a cover", () => Middle(Poster(run)), -3);
        await SlowWheel(run, "seasonal: wheel slowly back up over a cover", () => Middle(Poster(run)), 3);
        await Go(run, "Home");
        Native.Size(run.Hwnd, 2560, 1392);
        await Task.Delay(1500);
    }

    public static async Task All(Run run)
    {
        await Wheels(run);

        // Every page from the sidebar, and back Home.
        foreach (var page in Pages)
            await Go(run, page);

        // Home: the spotlight's Details, back, and a row's arrows.
        await Feel.Click(run, "home: spotlight Details", Find(run, "Details", ControlType.Button), 5000);
        await Feel.Click(run, "title: back to Home", Find(run, "Back", ControlType.Button));
        // Not on the person's own profile: a click that misses an arrow plays a Continue card,
        // and a play there reaches their tracker.
        if (run.Name != "feel-real")
        {
            await Feel.Click(run, "home: row Next", Shown(run, "Next"));
            await Feel.Click(run, "home: row Previous", Shown(run, "Previous"));
            await Go(run, "Home");
        }

        // Scroll kept: down Home, a title, back.
        await Wheel(run, -6);
        await Feel.Click(run, "home (scrolled): open a poster", Poster(run), 5000);
        await Feel.Click(run, "title: back to scrolled Home (should look as before)", Find(run, "Back", ControlType.Button));
        await Wheel(run, 12);
        await Go(run, "Home");

        // The same in a window small enough for Home to scroll: the frame after Back is the one before.
        Native.Size(run.Hwnd, 1600, 900);
        await Task.Delay(1500);
        await Wheel(run, -5);
        await Feel.Kept(run, "home 1600x900 (scrolled): a poster and Back, kept", async () =>
        {
            await Feel.Click(run, "home 1600x900 (scrolled): open a poster", Poster(run), 5000);
            await Feel.Click(run, "title: back to scrolled Home at 1600x900", Find(run, "Back", ControlType.Button));
        });
        await Go(run, "Library");
        await Feel.Click(run, "library: Completed (for the trip)", Starts(run, "Completed", ControlType.ListItem));
        await Wheel(run, -3);
        await Feel.Kept(run, "library 1600x900 (Completed, scrolled): a cover and Back, kept", async () =>
        {
            await Feel.Click(run, "library (scrolled): open a cover", Poster(run), 5000);
            await Feel.Click(run, "title: back to Library", Find(run, "Back", ControlType.Button));
        });
        await Feel.Click(run, "library: Watching (after the trip)", Starts(run, "Watching", ControlType.ListItem));
        Native.Size(run.Hwnd, 2560, 1392);
        await Task.Delay(1500);
        await Go(run, "Home");

        // Calendar.
        await Go(run, "Calendar");
        await Feel.Click(run, "calendar: next week", Find(run, "Next week", ControlType.Button));
        await Feel.Click(run, "calendar: previous week", Find(run, "Previous week", ControlType.Button));
        await Feel.Click(run, "calendar: My list", Find(run, "My list", ControlType.ListItem));
        await Feel.Click(run, "calendar: All airing", Find(run, "All airing", ControlType.ListItem));
        await Feel.Click(run, "calendar: Today", Find(run, "Today", ControlType.Button));

        // Seasonal.
        await Go(run, "Seasonal");
        await Feel.Click(run, "seasonal: TV", Find(run, "TV", ControlType.ListItem));
        await Feel.Click(run, "seasonal: Movie", Find(run, "Movie", ControlType.ListItem));
        await Feel.Click(run, "seasonal: All formats", Find(run, "All formats", ControlType.ListItem));
        await Feel.Click(run, "seasonal: previous season", Find(run, "Previous season", ControlType.Button), 5000);
        await Feel.Click(run, "seasonal: next season", Find(run, "Next season", ControlType.Button), 5000);
        await Wheel(run, -5);
        await Feel.Click(run, "seasonal (scrolled): open a poster", Poster(run), 5000);
        await Feel.Click(run, "title: back to scrolled Seasonal (should look as before)", Find(run, "Back", ControlType.Button));

        // Search: typing, and a filter.
        await Go(run, "Search");
        var box = Find(run, "Search titles", ControlType.Edit);
        await Feel.Film(run, "search: type a query", () => { Keyboard.Type("sky"); return Task.CompletedTask; },
            () =>
            {
                if (box is not null)
                {
                    var r = box.BoundingRectangle;
                    Mouse.MoveTo(new Point(r.Left + r.Width / 2, r.Top + r.Height / 2));
                    Mouse.Click();
                }
                return Task.CompletedTask;
            }, 5000);
        await Key(run, "search: Enter", VirtualKeyShort.ENTER);
        await Feel.Film(run, "search: clear the query", () =>
        {
            Keyboard.TypeSimultaneously(VirtualKeyShort.CONTROL, VirtualKeyShort.KEY_A);
            Keyboard.Type(VirtualKeyShort.BACK);
            return Task.CompletedTask;
        });
        await Go(run, "Search");
        await Feel.Click(run, "search: Format filter", Find(run, "Format", ControlType.Button));
        await Key(run, "search: close the filter", VirtualKeyShort.ESCAPE);

        // Library.
        await Go(run, "Library");
        await Feel.Click(run, "library: Planning", Starts(run, "Planning", ControlType.ListItem));
        await Feel.Click(run, "library: Completed", Starts(run, "Completed", ControlType.ListItem));
        await Feel.Click(run, "library: Watching", Starts(run, "Watching", ControlType.ListItem));
        await Feel.Click(run, "library: Sort", Starts(run, "Sort by", ControlType.Button));
        await Key(run, "library: close Sort", VirtualKeyShort.ESCAPE);

        // Settings: a switch both ways, each pane, and a scroll.
        await Go(run, "Settings");
        await Feel.Click(run, "settings: Auto-play next off", Find(run, "Auto-play next", ControlType.Button));
        await Feel.Click(run, "settings: Auto-play next on", Find(run, "Auto-play next", ControlType.Button));
        var left = run.Window.BoundingRectangle.Left;
        var panes = run.Window.FindAllDescendants(cf => cf.ByControlType(ControlType.ListItem))
            .Where(e => Run.Safe(() => e.BoundingRectangle) is { Width: > 0 } r && r.Left > left + 240 && r.Left < left + 520)
            .ToArray();
        for (var i = 0; i < Math.Min(panes.Length, 9); i++)
            await Feel.Click(run, $"settings: pane {i + 1} {Run.Safe(() => panes[i].Name)}", panes[i]);
        await Wheel(run, -10, new Point(left + 1100, run.Window.BoundingRectangle.Top + 600));
    }
}
