#if KAERI_TESTING
using System.Globalization;
using Kaeri.App;
using Kaeri.Engine.Core;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Kaeri.Windows.Player;

/// <summary>
/// The testing build's player panel demo (<c>--show=player-panel...</c>): the testing engine has one
/// source with one stream, so the engine's part in the panel's Sources tab is played here. It lists
/// made-up sources and releases beside the fixture's own, and answers a switch as the engine would:
/// Harbor Mirror lands at the playhead, on the fixture's clip loaded again, and Tidewater fails after
/// a wait. The episodes get made-up synopses, one long enough for More, and the dub goes through
/// episode 8. The player, the panel and its rows are the app's own.
/// </summary>
static class PanelDemoData
{
    /// <summary>The made-up source whose presence says the listing has them already.</summary>
    public const string Marker = "harbor";

    public static readonly SourceOption[] Sources =
    [
        new("harbor", "Harbor Mirror", 12, 12),
        new("tidewater", "Tidewater", 9, 12),
        new("northshelf", "North Shelf", 4, 12),
        new("quietreel", "Quiet Reel", 0, 12, Note: new SourceNote.Resting()),
    ];

    /// <summary>Each made-up source's releases for any episode: Harbor's land, Tidewater's fails, North Shelf has none.</summary>
    public static SourceCandidate[] Releases(string episode)
    {
        var title = $"Stardust Wayfarer - {episode.PadLeft(2, '0')}";
        return
        [
            new("h-1080", "harbor", $"{title} (1080p)", "Harbor Mirror", 1080, "paper-lantern", "1.4 GiB", AutoPick: true),
            new("h-720-dub", "harbor", $"{title} (720p, dubbed)", "Harbor Mirror", 720, "paper-lantern", "620 MiB", Dubbed: true),
            new("h-1080-dual", "harbor", $"{title} (1080p, dual audio, extended credits and signs)", "Harbor Mirror", 1080,
                "a very long release label that runs past one line", "2.1 GiB", DualAudio: true),
            new("t-1080", "tidewater", $"{title} [1080p]", "Tidewater", 1080, "tidewater-encodes", "1.2 GiB"),
        ];
    }

    /// <summary>Made-up synopses by episode: the fifth's runs past three lines, the sixth's fits.</summary>
    public static readonly Dictionary<string, string> Synopses = new()
    {
        ["5"] = "The beacon keepers of the northern ridge remember every traveller who ever passed under their lights, "
            + "and one of them remembers the lamplighter too, from a winter she cannot recall. Their ledger holds a page "
            + "in her hand, signed with a name she has never used, and a map of a road that no longer exists. "
            + "She spends the night copying it by lantern light while the keepers argue over whether to let her leave.",
        ["6"] = "A mapmaker's daughter offers to trade a route for a story.",
        ["9"] = "The last beacon on the old road goes dark, and the lamplighter has to choose which light to save.",
    };
}

public sealed partial class PlayerView
{
    /// <summary>The fixture's table with the demo's synopses.</summary>
    internal Event.Episodes PanelDemoTable(Event.Episodes table) =>
        table with { Rows = [.. table.Rows.Select(row => PanelDemoData.Synopses.TryGetValue(row.Number, out var text) ? row with { Description = text } : row)] };

    /// <summary>A state of the panel for a shot, once the picture is up.</summary>
    async Task PanelDemo(string shot)
    {
        if (ctx is not { } c)
            return;
        // The dub reaches episode 8.
        var dub = new Event.TitleDub(playingId, new DubExtent(8, false));
        c.Known.Take(dub);
        Heard([dub]);
        switch (shot)
        {
            case "player-panel":
                OpenPanel(PanelTab.Episodes, keyboard: false);
                break;
            case "player-panel-reveal":
                OpenPanel(PanelTab.Episodes, keyboard: false);
                await Task.Delay(600);
                RevealEpisode("5", toggle: false);
                break;
            case "player-panel-sources":
                OpenPanel(PanelTab.Sources, keyboard: false);
                break;
            case "player-panel-releases":
                OpenPanel(PanelTab.Sources, keyboard: false);
                await Task.Delay(1200);
                ToggleReleases("harbor");
                break;
        }
    }

    /// <summary>The panel's commands the demo answers in the engine's place.</summary>
    internal void PanelDemoSend(Command command)
    {
        switch (command)
        {
            case Command.LoadSources load:
                DispatcherQueue.TryEnqueue(async () =>
                {
                    await Task.Delay(700);
                    var listed = new Event.Sources(load.Id, load.Episode, PanelDemoData.Releases(load.Episode), []);
                    Heard([listed]);
                });
                break;
            case Command.SwitchSource pick:
                DispatcherQueue.TryEnqueue(() => _ = PanelDemoSwitch(pick));
                break;
        }
    }

    async Task PanelDemoSwitch(Command.SwitchSource pick)
    {
        var release = PanelDemoData.Releases(pick.Episode).FirstOrDefault(r => r.ProviderId == pick.ProviderId && r.Id == pick.SourceId);
        if (release is null)
            return;
        var now = CurrentPlayback();
        var to = new SwitchTo(release.ProviderId, release.ProviderName, release.Id);
        void Say(SourceSwitch state) => Heard([new Event.PlayerSwitch(now, state)]);
        await Task.Delay(150);
        Say(new SourceSwitch.Finding(to));
        if (release.ProviderId == "tidewater")
        {
            // Long enough for the row's spinner and its "Starting · 3 s" to be seen.
            await Task.Delay(5_000);
            Say(new SourceSwitch.Failed(to, position, new Msg.PlayerPanelSwitchFailed(release.ProviderName, panel.PlayingSource ?? "Fixture Source")));
            return;
        }
        await Task.Delay(900);
        var at = position;
        if (player?.Snapshot() is { } frame)
            TookSnapshot(frame);
        Say(new SourceSwitch.Loading(to, at));
        PreviewSource? load;
        lock (loads)
            load = lastLoad;
        if (player is { } p && load is not null)
        {
            PlayerOp[] ops = [new PlayerOp.Load(load.Url, [.. load.Options, new MpvOption("start", at.ToString("0.0", CultureInfo.InvariantCulture))])];
            NoteLoad(now, ops);
            p.Apply(now, ops);
        }
        // The old picture goes, and the new one comes.
        for (var waited = 0; pictureNow && waited < 15_000; waited += 20)
            await Task.Delay(20);
        for (var waited = 0; !pictureNow && waited < 15_000; waited += 50)
            await Task.Delay(50);
        Say(new SourceSwitch.Landed(to, at));
    }
}
#endif
