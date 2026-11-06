// The pages that the engine's events used to draw again from scratch: Downloads, Sources,
// Settings and Plugin stores. Each check presses something, as a person does, and then asks UI
// Automation whether the same element is still there (its runtime id), where the focus is and
// what a box holds; the presses are also filmed by Feel. Fixture scene "watching" with the
// fixture's downloads, so it never touches the person's profile. Only when asked for by name.

using FlaUI.Core.AutomationElements;
using FlaUI.Core.Definitions;
using FlaUI.Core.Input;
using FlaUI.Core.WindowsAPI;

static class PagesScenario
{
    static string Id(AutomationElement? e) =>
        e is null ? "none" : string.Join(".", Run.Safe(() => e.Properties.RuntimeId.ValueOrDefault) ?? []);

    static AutomationElement? Focused(Run run) => Run.Safe(() => run.Window.Automation.FocusedElement());

    static void Check(Run run, string what, bool ok, string detail = "")
    {
        var line = $"{(ok ? "PASS" : "FAIL")} {what}{(detail.Length > 0 ? $" ({detail})" : "")}";
        Console.WriteLine($"   {line}");
        run.Note(line);
    }

    static AutomationElement[] OnPage(Run run, ControlType type) =>
        [.. run.Window.FindAllDescendants(cf => cf.ByControlType(type))
            .Where(e => Run.Safe(() => !e.IsOffscreen) && Run.Safe(() => e.BoundingRectangle) is { Width: > 0 } r && run.Inside(r))];

    static AutomationElement? Named(Run run, string name, ControlType type) =>
        OnPage(run, type).FirstOrDefault(e => Run.Safe(() => e.Name) == name);

    static string[] Ids(Run run, ControlType type) => [.. OnPage(run, type).Select(Id)];

    /// <summary>Whether the page's elements of a kind are the same ones after a wait, as a page drawn again would make new ones.</summary>
    static async Task Kept(Run run, string what, ControlType type, int ms)
    {
        var before = Ids(run, type);
        await Task.Delay(ms);
        var after = Ids(run, type);
        Check(run, what, before.Length > 0 && before.SequenceEqual(after), $"{before.Length} then {after.Length}, {before.Intersect(after).Count()} the same");
    }

    /// <summary>Downloads and Settings, on the watching scene with the fixture's downloads.</summary>
    public static async Task All(Run run)
    {
        await Downloads(run);
        await Settings(run);
    }

    /// <summary>Sources and Plugin stores, on a scene with a fixture source, the demo and a store.</summary>
    public static async Task Sourced(Run run)
    {
        await Sources(run);
        await Stores(run);
    }

    static async Task Downloads(Run run)
    {
        await FeelScenario.Go(run, "Downloads");
        await Task.Delay(1500);
        await Kept(run, "downloads: buttons kept over 3 s", ControlType.Button, 3000);
        var pause = Named(run, "Pause download", ControlType.Button);
        var id = Id(pause);
        await Feel.Click(run, "downloads: Pause", pause);
        await Task.Delay(1200);
        var resume = Named(run, "Resume download", ControlType.Button);
        Check(run, "downloads: Pause becomes Resume on the same button", resume is not null && Id(resume) == id, $"pause {id}, resume {Id(resume)}");
        Check(run, "downloads: the focus stays on it", Id(Focused(run)) == id, $"focused {Run.Safe(() => Focused(run)?.Name)}");
        // Resumed, the fixture's download waits for a slot: Pause goes, and the focus moves to the
        // row's next button rather than out of the page.
        await Feel.Click(run, "downloads: Resume", resume);
        await Task.Delay(1200);
        var focused = Run.Safe(() => Focused(run)?.Name);
        Check(run, "downloads: resumed, the focus is on the row's next button", focused is "Start now" or "Pause download", $"focused {focused}");
        await Kept(run, "downloads: buttons kept over 3 s after", ControlType.Button, 3000);
    }

    static async Task Sources(Run run)
    {
        await FeelScenario.Go(run, "Sources");
        // On arrival the page checks every source, which streams a note per source.
        await Task.Delay(400);
        await Kept(run, "sources: rows kept while the check streams in", ControlType.ListItem, 3000);
        var items = OnPage(run, ControlType.ListItem);
        Console.WriteLine($"   sources rows: {string.Join(" | ", items.Select(i => Run.Safe(() => i.Name)))}");
        // The first row that Alt+Down moves: a source in an ordered list. Rows named "title, line"
        // open a page of their own (media, servers, licensed services), so they are passed over.
        foreach (var name in items.Select(i => Run.Safe(() => i.Name) ?? "").Where(n => n.Length > 0 && !n.Contains(", ", StringComparison.Ordinal)).ToList())
        {
            await Feel.Click(run, $"sources: select {name}", Named(run, name, ControlType.ListItem));
            var before = Array.FindIndex(OnPage(run, ControlType.ListItem), i => Run.Safe(() => i.Name) == name);
            await Feel.Film(run, "sources: Alt+Down", () => { Keyboard.TypeSimultaneously(VirtualKeyShort.ALT, VirtualKeyShort.DOWN); return Task.CompletedTask; });
            await Task.Delay(800);
            var after = Array.FindIndex(OnPage(run, ControlType.ListItem), i => Run.Safe(() => i.Name) == name);
            if (after == before)
                continue;
            Check(run, "sources: Alt+Down moved it, focus kept", Run.Safe(() => Focused(run)?.Name) == name, $"{before} to {after}, focused {Run.Safe(() => Focused(run)?.Name)}");
            Keyboard.TypeSimultaneously(VirtualKeyShort.ALT, VirtualKeyShort.UP);
            await Task.Delay(800);
            var back = Array.FindIndex(OnPage(run, ControlType.ListItem), i => Run.Safe(() => i.Name) == name);
            Check(run, "sources: a second Alt+Up moves it back, focus kept", back == before && Run.Safe(() => Focused(run)?.Name) == name, $"at {back}, focused {Run.Safe(() => Focused(run)?.Name)}");
            return;
        }
        Check(run, "sources: a row to move", false, "none moved");
    }

    static async Task Settings(Run run)
    {
        await FeelScenario.Go(run, "Settings");
        await Task.Delay(800);
        // The switch is on Playback; Settings opens on the pane it was left on, General at first.
        Named(run, "Playback", ControlType.ListItem)?.Click();
        await Task.Delay(800);
        var toggle = Named(run, "Auto-play next", ControlType.Button);
        Check(run, "settings: the Auto-play next switch is there", toggle is not null);
        var id = Id(toggle);
        var cards = Ids(run, ControlType.Group).Concat(Ids(run, ControlType.Button)).ToArray();
        await Feel.Click(run, "settings: Auto-play next off", toggle);
        await Task.Delay(800);
        Check(run, "settings: the switch is the same after the engine's answer", Id(Named(run, "Auto-play next", ControlType.Button)) == id);
        await Feel.Click(run, "settings: Auto-play next on", toggle);
        await Task.Delay(800);
        var again = Ids(run, ControlType.Group).Concat(Ids(run, ControlType.Button)).ToArray();
        Check(run, "settings: no card made again", cards.SequenceEqual(again), $"{cards.Length} then {again.Length}, {cards.Intersect(again).Count()} the same");

        // A pick in a drop-down, and the box is the same after.
        if (Named(run, "Quality", ControlType.ComboBox) is { } quality)
        {
            var boxId = Id(quality);
            quality.AsComboBox().Expand();
            await Task.Delay(500);
            Keyboard.Type(VirtualKeyShort.DOWN);
            Keyboard.Type(VirtualKeyShort.ENTER);
            await Task.Delay(1000);
            Check(run, "settings: Quality box kept after a pick", Id(Named(run, "Quality", ControlType.ComboBox)) == boxId);
            quality.AsComboBox().Expand();
            await Task.Delay(500);
            Keyboard.Type(VirtualKeyShort.UP);
            Keyboard.Type(VirtualKeyShort.ENTER);
            await Task.Delay(800);
        }

        // Devices: a name being typed stays while the pane hears from the engine.
        var panes = run.Window.FindAllDescendants(cf => cf.ByControlType(ControlType.ListItem))
            .Where(e => Run.Safe(() => e.BoundingRectangle) is { Width: > 0 } r && r.Left > run.Window.BoundingRectangle.Left + 240 && r.Left < run.Window.BoundingRectangle.Left + 520)
            .ToArray();
        var devices = panes.FirstOrDefault(p => Run.Safe(() => p.Name) == "Devices" || Run.Safe(() => p.FindFirstDescendant(cf => cf.ByName("Devices"))) is not null);
        await Feel.Click(run, "settings: Devices pane", devices);
        await Task.Delay(800);
        if (Named(run, "Name", ControlType.Edit) is { } box)
        {
            var was = box.AsTextBox().Text;
            box.Click();
            Keyboard.TypeSimultaneously(VirtualKeyShort.CONTROL, VirtualKeyShort.KEY_A);
            Keyboard.Type("Half a na");
            // Something the pane hears while the box is typed in: a setting changed elsewhere.
            await Task.Delay(2000);
            Check(run, "settings: a typed device name stays", box.AsTextBox().Text == "Half a na" && Id(Focused(run)) == Id(box), $"holds {box.AsTextBox().Text}");
            Keyboard.TypeSimultaneously(VirtualKeyShort.CONTROL, VirtualKeyShort.KEY_A);
            Keyboard.Type(was);
            Keyboard.Type(VirtualKeyShort.ENTER);
            await Task.Delay(500);
        }
        else
            Check(run, "settings: the device name box", false, "not found");

        // Settings opens on the pane it was last on.
        await FeelScenario.Go(run, "Home");
        await FeelScenario.Go(run, "Settings");
        await Task.Delay(800);
        var selected = run.Window.FindAllDescendants(cf => cf.ByControlType(ControlType.ListItem))
            .Where(e => Run.Safe(() => e.BoundingRectangle) is { Width: > 0 } r && r.Left > run.Window.BoundingRectangle.Left + 240 && r.Left < run.Window.BoundingRectangle.Left + 520)
            .FirstOrDefault(e => Run.Safe(() => e.Patterns.SelectionItem.Pattern.IsSelected.Value) == true);
        Check(run, "settings: opens on the last pane", Named(run, "Name", ControlType.Edit) is not null, $"selected {Run.Safe(() => selected?.Name)}");
    }

    static async Task Stores(Run run)
    {
        await FeelScenario.Go(run, "Plugin stores");
        await Task.Delay(1500);
        var rows = OnPage(run, ControlType.ListItem).Length;
        if (rows == 0)
        {
            Console.WriteLine("   stores: no stores in this scene");
            return;
        }
        await Kept(run, "stores: rows kept", ControlType.ListItem, 2500);
    }
}
