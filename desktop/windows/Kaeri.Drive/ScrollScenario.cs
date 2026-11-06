// Scrolling: the wheel on each page that scrolls, a notch at a time and then fast, as steps the
// report times (frame times while it moved, missed frames, the longest UI-thread stall). "scroll"
// runs on the fixture scene; "scroll-real" on the person's own profile, scrolling and nothing else.

using System.Drawing;
using FlaUI.Core.AutomationElements;
using FlaUI.Core.Definitions;
using FlaUI.Core.Input;
using FlaUI.Core.WindowsAPI;

static class ScrollScenario
{
    static AutomationElement? Nav(Run run, string page) =>
        run.Window.FindAllDescendants(cf => cf.ByName(page).And(cf.ByControlType(ControlType.ListItem))).FirstOrDefault();

    static async Task Wheel(Run run, string label, Point at, int notches, int gapMs)
    {
        Mouse.MoveTo(at);
        await Task.Delay(400);
        await run.Step(label, async () =>
        {
            for (var i = 0; i < Math.Abs(notches); i++)
            {
                Mouse.Scroll(notches < 0 ? -1 : 1);
                await Task.Delay(gapMs);
            }
            await Task.Delay(700);
        });
    }

    /// <summary>Down a notch at a time, as reading; then back up fast, as flicking.</summary>
    static async Task Page(Run run, string page, Func<Rectangle, Point> where)
    {
        if (Nav(run, page) is { } nav)
        {
            run.Click(nav);
            await Task.Delay(2500);
        }
        var at = where(run.Window.BoundingRectangle);
        await Wheel(run, $"{page}: down a notch at a time", at, -12, 140);
        await Wheel(run, $"{page}: up fast", at, 12, 25);
    }

    static Point At(Rectangle w, double x, double y) => new(w.Left + (int)(w.Width * x), w.Top + (int)(w.Height * y));

    public static async Task Fixture(Run run) => await Pages(run, open: true);

    public static async Task Real(Run run) => await Pages(run, open: false);

    static async Task Pages(Run run, bool open)
    {
        await Task.Delay(3000);
        await Page(run, "Home", w => At(w, 0.45, 0.55));
        // Over the gaps between shelves and over a shelf's cards are handled differently.
        await Wheel(run, "Home: over a shelf's cards, down", At(run.Window.BoundingRectangle, 0.40, 0.70), -8, 140);
        await Page(run, "Seasonal", w => At(w, 0.55, 0.60));
        await Page(run, "Library", w => At(w, 0.55, 0.60));
        await Page(run, "Search", w => At(w, 0.55, 0.60));
        await Page(run, "Calendar", w => At(w, 0.55, 0.60));
        await Page(run, "Settings", w => At(w, 0.60, 0.60));
        if (!open)
            return;
        // A title page's episode list: the first poster on Seasonal.
        if (Nav(run, "Seasonal") is { } seasonal)
        {
            run.Click(seasonal);
            await Task.Delay(2500);
        }
        if (run.Covers().FirstOrDefault() is { } poster)
        {
            run.Click(poster);
            await Task.Delay(3000);
            var at = At(run.Window.BoundingRectangle, 0.45, 0.70);
            await Wheel(run, "Title page: down a notch at a time", at, -12, 140);
            await Wheel(run, "Title page: up fast", at, 12, 25);
        }
    }
}
