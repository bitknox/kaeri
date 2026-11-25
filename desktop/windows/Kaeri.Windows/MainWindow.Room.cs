#if KAERI_TESTING
using Kaeri.App;
using Kaeri.App.RoomStand;
using Kaeri.Engine.Model;

namespace Kaeri.Windows;

/// <summary>
/// --show=room-*: the room's board frames over the fixture episode, from made-up rooms until the
/// engine's fake room and its fixture scenes land. The player, the column, the strip and the
/// popups are the app's own; only the room's state is played here, and the presses that change
/// it (Carry on, Play for everyone, Leave) are answered as the engine would.
/// </summary>
public sealed partial class MainWindow
{
    static readonly (string Kind, string From, string Text)[] RoomTalk =
    [
        ("c", "Mika", "everyone here?"),
        ("c", "Ren", "yes, go"),
        ("s", "", "Ren paused at 3:10"),
        ("c", "Ren", "back in two, kettle"),
        ("s", "", "Ren resumed"),
        ("r", "Ren", "11:52"),
        ("c", "Ada", "the relay handoff at the bridge"),
        ("c", "Mika", "every week this song"),
    ];

    static readonly RelayHealth[] FakeRelays =
    [
        new("relay-one.example", true, 40),
        new("relay-two.example", true, 40),
        new("relay-three.example", true, 40),
    ];

    long roomT0;
    RoomScene? fakeRoom;
    List<RoomChatLine> fakeChat = [];

    static List<RoomChatLine> Talk(string me, int skip = 0)
    {
        var lines = new List<RoomChatLine>();
        foreach (var (kind, from, text) in RoomTalk.Skip(skip))
            lines.Add(kind switch
            {
                "s" => new RoomChatLine.Note(text),
                "r" => new RoomChatLine.Reacted(from, Reaction.Laugh, text),
                _ => new RoomChatLine.Said(from, text, from == me),
            });
        return lines;
    }

    RoomMember Member(string name, MemberState state, string me, long ago = 0, float? behind = null, bool pausedRoom = false, string? episode = null) =>
        new(name.ToLowerInvariant(), name, name == me, state, roomT0 - ago, behind, episode is null ? null : new RoomEpisode(900001, episode), pausedRoom, false);

    RoomView FakeView(string me, bool playing = true, string? pausedBy = null, PausedFor[]? waiting = null, bool cap = false,
        RoomConnection? connection = null, RelayHealth[]? relays = null, RoomUpNext? upNext = null, bool dontWait = false, MoveCandidate[]? move = null) =>
        new("fixture-room", me, connection ?? new RoomConnection.Connected(), new RoomEpisode(900001, "7"), "kaeri.app/r#v1.Q2m4xR9dK7tWbH3nPzL8sYe",
            playing, pausedBy, waiting ?? [], cap, dontWait, upNext, relays ?? FakeRelays, move ?? []);

    /// <summary>A frame's room, by its board name.</summary>
    (RoomScene Scene, List<RoomChatLine> Chat, RoomPanel? Panel, string[] Muted) FakeRoomFor(string state)
    {
        const string show = "Stardust Wayfarer";
        const string ep = "Episode 7";
        var me = "Ada";
        RoomMember[] Three(MemberState ren = MemberState.Watching, long ago = 0, MemberState mika = MemberState.Watching, MemberState ada = MemberState.Watching, bool mikaPaused = false) =>
            [Member("Ada", ada, me), Member("Mika", mika, me, pausedRoom: mikaPaused), Member("Ren", ren, me, ago)];
        PausedFor[] WaitRen(long ago, PauseReason why = PauseReason.Loading) => [new("Ren", why, roomT0 - ago)];
        var chat = Talk(me);
        RoomScene S(RoomView v, RoomMember[] m, RoomStart? start = null, RoomInvited? invited = null, RoomMoveOffer? offer = null, string? carried = null) =>
            new(v, m, roomT0, start ?? new RoomStart.Started(), 724, ep, "Episode 8 · The Anchor Leg", invited, offer, carried);
        var started = new List<RoomChatLine> { new RoomChatLine.Note($"Mika started the room · {ep}") };
        RoomPanel Panel(RoomPanelKind kind) => new(kind, show, $"{ep} · The Long Leg", "7", 24,
            [new("Local files", "1080p · 1.2 GB", true), new("Night Index", "720p · 640 MB", false)], 3);
        switch (state)
        {
            case "room-paused":
                return (S(FakeView(me, playing: false, pausedBy: "Mika"), Three(MemberState.Paused, mika: MemberState.Paused, ada: MemberState.Paused, mikaPaused: true)),
                    [.. chat, new RoomChatLine.Note("Mika paused at 12:04")], null, []);
            case "room-wait":
            case "room-full-wait":
                return (S(FakeView(me, playing: false, waiting: WaitRen(6_000)), Three(MemberState.Loading, 6_000, MemberState.Paused, MemberState.Paused)),
                    [.. chat, new RoomChatLine.Note(RoomWords.PausedForLine("Ren"))], null, []);
            case "room-cap":
            case "room-full-cap":
                return (S(FakeView(me, playing: false, waiting: WaitRen(20_000), cap: true), Three(MemberState.Loading, 20_000, MemberState.Paused, MemberState.Paused)),
                    [.. chat, new RoomChatLine.Note(RoomWords.PausedForLine("Ren"))], null, []);
            case "room-carried":
                return (S(FakeView(me), Three(MemberState.Loading, 24_000), carried: "Mika"),
                    [.. chat, new RoomChatLine.Note(RoomWords.PausedForLine("Ren")), new RoomChatLine.Note(RoomWords.CarriedOnLine("Mika", "Ren"))], null, []);
            case "room-self-wait":
                me = "Ren";
                return (S(FakeView(me, playing: false, waiting: WaitRen(6_000), dontWait: true),
                        [Member("Ren", MemberState.Loading, me, 6_000), Member("Mika", MemberState.Paused, me), Member("Ada", MemberState.Paused, me)]),
                    [.. Talk(me), new RoomChatLine.Note(RoomWords.PausedForYouLine)], Panel(RoomPanelKind.SelfLoading), []);
            case "room-buffering":
                return (S(FakeView(me, playing: false, waiting: WaitRen(3_000, PauseReason.Buffering)), Three(MemberState.Buffering, 3_000, MemberState.Paused, MemberState.Paused)),
                    [.. chat, new RoomChatLine.Note(RoomWords.PausedForLine("Ren"))], null, []);
            case "room-six":
                return (S(FakeView(me), [.. Three(MemberState.Buffering, 3_000), Member("Sora", MemberState.Watching, me), Member("Kenji", MemberState.Watching, me), Member("Noa", MemberState.Watching, me)]),
                    chat, null, []);
            case "room-choosing" or "room-to-full":
                return (S(FakeView(me), Three(MemberState.Choosing)), chat, null, []);
            case "room-nosource":
                return (S(FakeView(me), Three(MemberState.NoSource)), chat, null, []);
            case "room-closed":
                return (S(FakeView(me), Three(MemberState.PlayerClosed)), chat, null, []);
            case "room-reconnecting":
                return (S(FakeView(me), Three(MemberState.Reconnecting)), chat, null, []);
            case "room-behind":
                return (S(FakeView(me), [Member("Ada", MemberState.Watching, me), Member("Mika", MemberState.Watching, me), Member("Ren", MemberState.Behind, me, behind: 4)]), chat, null, []);
            case "room-other":
                return (S(FakeView(me), [Member("Ada", MemberState.Watching, me), Member("Mika", MemberState.Watching, me), Member("Ren", MemberState.OtherEpisode, me, episode: "6")]), chat, null, []);
            case "room-own-choosing":
                return (S(FakeView(me), Three(ada: MemberState.Choosing)), chat, Panel(RoomPanelKind.Choosing), []);
            case "room-own-nosource":
                return (S(FakeView(me), Three(ada: MemberState.NoSource)), chat, Panel(RoomPanelKind.NoSource), []);
            case "room-self-behind":
                return (S(FakeView(me), [Member("Ada", MemberState.Behind, me, behind: 4), Member("Mika", MemberState.Watching, me), Member("Ren", MemberState.Watching, me)]), chat, null, []);
            case "room-self-reconnecting":
                return (S(FakeView(me, connection: new RoomConnection.Reconnecting(0), relays: [.. FakeRelays.Select(r => r with { Healthy = false, RttMs = null })]),
                    [Member("Ada", MemberState.Reconnecting, me), Member("Mika", MemberState.Watching, me), Member("Ren", MemberState.Watching, me)]), chat, null, []);
            case "room-relay-trouble" or "room-details-trouble":
                return (S(FakeView(me, relays: [FakeRelays[0], FakeRelays[1] with { Healthy = false, RttMs = null }, FakeRelays[2]]), Three()), chat, null, []);
            case "room-upnext":
                return (S(FakeView(me, upNext: new RoomUpNext(new RoomEpisode(900001, "8"), roomT0 + 7_400, [], null)), Three()),
                    [.. chat, new RoomChatLine.Note("Up next: episode 8 for everyone")], null, []);
            case "room-skip":
                return (S(FakeView(me), Three()), [.. chat, new RoomChatLine.Note(RoomWords.SkippedLine("Mika"))], null, []);
            case "room-before":
                return (S(FakeView(me, playing: false), Three(MemberState.Loading, 4_000), new RoomStart.Before()), [.. started, .. LobbyTalk(me)], Panel(RoomPanelKind.Before), []);
            case "room-before-wait":
                return (S(FakeView(me, playing: false, waiting: WaitRen(6_000)), Three(MemberState.Loading, 6_000), new RoomStart.Before()), [.. started, .. LobbyTalk(me)], Panel(RoomPanelKind.Before) with { Pressed = true }, []);
            case "room-before-cap":
                return (S(FakeView(me, playing: false, waiting: WaitRen(20_000), cap: true), Three(MemberState.Loading, 20_000), new RoomStart.Before()), [.. started, .. LobbyTalk(me)], Panel(RoomPanelKind.Before) with { Pressed = true }, []);
            case "room-before-count":
                return (S(FakeView(me, playing: false), Three(), new RoomStart.Counting(3)), [.. started, .. LobbyTalk(me)], Panel(RoomPanelKind.Before) with { Pressed = true }, []);
            case "room-invited" or "room-invited-over":
            {
                var over = state == "room-invited-over";
                RoomMember[] list = over
                    ? [.. new[] { "Mika", "Ren", "Sora", "Kenji", "Noa", "Yui", "Taro", "Hana", "Kai", "Rio" }.Select(n => Member(n, MemberState.Watching, me))]
                    : [Member("Mika", MemberState.Watching, me), Member("Ren", MemberState.Watching, me)];
                return (S(FakeView(me), list, invited: new RoomInvited("Mika", show, ep, list.Length)), [new RoomChatLine.Note(RoomWords.ChatOpensOnJoin)], Panel(RoomPanelKind.Invited), []);
            }
            case "room-move-start" or "room-move-pick":
                me = "Mika";
                return (S(FakeView(me, move: [new("ada", "Ada", false, true), new("ren", "Ren", false, true), new("taro", "Taro", true, false)]),
                        [Member("Ada", MemberState.Watching, me), Member("Mika", MemberState.Watching, me), Member("Ren", MemberState.Watching, me), Member("Taro", MemberState.Watching, me)]),
                    Talk(me), null, ["Taro"]);
            case "room-move-offer":
                return (S(FakeView(me), [Member("Ada", MemberState.Watching, me), Member("Ren", MemberState.Watching, me), Member("Taro", MemberState.Watching, me), Member("Mika", MemberState.Left, me)],
                        offer: new RoomMoveOffer("offer-1", "Mika", ["Ada", "Ren"], show, "7", roomT0 + 600_000)),
                    [.. Talk(me).Take(4), new RoomChatLine.Note("Mika moved to a new room")], null, []);
            case "room-move-after":
                me = "Taro";
                return (S(FakeView(me), [Member("Taro", MemberState.Watching, me), Member("Ada", MemberState.Left, me), Member("Ren", MemberState.Left, me)]),
                    [new RoomChatLine.Note($"Mika started the room · {ep}"), new RoomChatLine.Note("Mika left"), new RoomChatLine.Note("Ada left"), new RoomChatLine.Note("Ren left")], null, []);
            default:
                return (S(FakeView(me), Three()), chat, null, []);
        }
    }

    static IEnumerable<RoomChatLine> LobbyTalk(string me) =>
    [
        new RoomChatLine.Said("Mika", "everyone here?", me == "Mika"),
        new RoomChatLine.Said("Ren", "two minutes, making tea", me == "Ren"),
        new RoomChatLine.Said("Ada", "ready when you are", me == "Ada"),
    ];

    /// <summary>Waits until the process has run this long, so a run's shots land on a moment.</summary>
    static async Task AtUptime(double seconds)
    {
        var started = System.Diagnostics.Process.GetCurrentProcess().StartTime;
        var wait = started.AddSeconds(seconds) - DateTime.Now;
        if (wait > TimeSpan.Zero)
            await Task.Delay(wait);
    }

    /// <summary>--show=room-*: the fixture episode playing, then the room in the frame's state.</summary>
    async void RehearseRoom(string state)
    {
        // A -rm suffix is the same frame with Reduce Motion on.
        if (state.EndsWith("-rm", StringComparison.Ordinal))
        {
            Motion.Override = false;
            state = state[..^3];
        }
        await Task.Delay(1500);
        ctx.Play(900001, "7", fromStart: true);
        for (var i = 0; i < 100 && !Player.Playing; i++)
            await Task.Delay(200);
        if (state is "room-morph" or "room-wait-story")
            await AtUptime(14);
        else
            await Task.Delay(600);
        roomT0 = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
        var (scene, chat, panel, muted) = FakeRoomFor(state);
        fakeRoom = scene;
        fakeChat = chat;
        Player.RoomSent -= FakeRoomHeard;
        Player.RoomSent += FakeRoomHeard;
        // Still frames hold their clock, so a shot reads the seconds the board draws; the wait's
        // own story (room-wait-story) runs on the wall clock.
        Func<RoomScene, RoomScene> clock = state is "room-wait-story" or "room-morph" or "room-to-full"
            ? s => s with { NowMs = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds() }
            : s => s;
        if (!scene.View.Playing)
            Player.ShotPaused(true);
        Player.ShowRoom(scene, chat, muted, morph: state == "room-morph", now: clock);
        Player.ShowRoomPanel(panel);
        switch (state)
        {
            case "room-full-wait" or "room-full-cap":
                await Task.Delay(400);
                Fullscreen(true);
                await Task.Delay(900);
                Player.ShotHideControls();
                break;
            case "room-to-full":
                await AtUptime(14);
                Fullscreen(true);
                await AtUptime(18);
                Fullscreen(false);
                break;
            case "room-invite":
                await Task.Delay(500);
                Player.ShotRoomInvite(false);
                break;
            case "room-move-start":
                await Task.Delay(500);
                Player.ShotRoomInvite(true);
                break;
            case "room-move-pick":
                await Task.Delay(500);
                Player.ShotRoomMove();
                break;
            case "room-details" or "room-details-trouble":
                await Task.Delay(500);
                Player.ShotRoomDetails(false);
                break;
            case "room-details-edit":
                await Task.Delay(500);
                Player.ShotRoomDetails(true);
                break;
            case "room-leave":
                await Task.Delay(500);
                Player.ShotRoomLeave();
                break;
            case "room-close":
                await Task.Delay(500);
                Player.ShotRoomClose();
                break;
            case "room-own-choosing" or "room-own-nosource" or "room-before" or "room-before-wait" or "room-before-cap"
                or "room-before-count" or "room-invited" or "room-invited-over" or "room-self-wait":
                await Task.Delay(300);
                Player.ShotHideControls();
                break;
            case "room-wait-story":
                await RoomWaitStory();
                break;
        }
    }

    /// <summary>The board's wait story at 1×: Ren opens at 1 s, the cap at 20 s, Mika carries on at 23 s, Ren lands at 27 s.</summary>
    async Task RoomWaitStory()
    {
        var me = "Ada";
        var t0 = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
        RoomMember[] M(MemberState ren, long since, MemberState others) =>
            [new("ada", "Ada", true, others, t0, null, null, false, false), new("mika", "Mika", false, others, t0, null, null, false, false), new("ren", "Ren", false, ren, since, null, null, false, false)];
        void Show(RoomView v, RoomMember[] m, string? carried, params RoomChatLine[] more)
        {
            fakeChat = [.. Talk(me), .. more];
            fakeRoom = fakeRoom! with { View = v, Members = m, CarriedBy = carried, NowMs = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds() };
            Player.ShowRoom(fakeRoom, fakeChat);
        }
        await Task.Delay(1000);
        var opened = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
        Player.ShotPaused(true);
        Show(FakeView(me, playing: false, waiting: [new("Ren", PauseReason.Loading, opened)]), M(MemberState.Loading, opened, MemberState.Paused), null, new RoomChatLine.Note(RoomWords.PausedForLine("Ren")));
        await Task.Delay(19_000);
        Show(FakeView(me, playing: false, waiting: [new("Ren", PauseReason.Loading, opened)], cap: true), M(MemberState.Loading, opened, MemberState.Paused), null, new RoomChatLine.Note(RoomWords.PausedForLine("Ren")));
        await Task.Delay(3_000);
        Player.ShotPaused(false);
        Show(FakeView(me), M(MemberState.Loading, opened, MemberState.Watching), "Mika", new RoomChatLine.Note(RoomWords.PausedForLine("Ren")), new RoomChatLine.Note(RoomWords.CarriedOnLine("Mika", "Ren")));
        await Task.Delay(4_000);
        Show(FakeView(me), M(MemberState.Watching, opened, MemberState.Watching), null, new RoomChatLine.Note(RoomWords.PausedForLine("Ren")), new RoomChatLine.Note(RoomWords.CarriedOnLine("Mika", "Ren")), new RoomChatLine.Note(RoomWords.JoinedAt("Ren", "12:31")));
    }

    /// <summary>The fake room's answers to the room's presses.</summary>
    void FakeRoomHeard(RoomCommand command)
    {
        if (fakeRoom is not { } room)
            return;
        var me = room.Members.FirstOrDefault(m => m.You)?.Name ?? "Ada";
        switch (command)
        {
            case RoomCommand.CarryOn:
                var past = room.View.Waiting.Select(w => w.Name).ToArray();
                fakeChat = [.. fakeChat, new RoomChatLine.Note(RoomWords.CarriedOnLine(RoomWords.YouAlone, Rooms.Names(past)))];
                fakeRoom = room with { View = room.View with { Waiting = [], CanCarryOn = false, Playing = true }, Start = new RoomStart.Started(), CarriedBy = me };
                Player.ShotPaused(false);
                Player.ShowRoomPanel(null);
                break;
            case RoomCommand.Play:
                fakeRoom = room with { View = room.View with { Waiting = [new("Ren", PauseReason.Loading, roomT0)] } };
                Player.ShowRoomPanel(new RoomPanel(RoomPanelKind.Before, "Stardust Wayfarer", "Episode 7 · The Long Leg", "7", Pressed: true));
                break;
            case RoomCommand.Say say:
                fakeChat = [.. fakeChat, new RoomChatLine.Said(me, say.Text, true)];
                break;
            case RoomCommand.Leave:
                Player.EndRoom();
                fakeRoom = null;
                return;
            default:
                return;
        }
        if (fakeRoom is { } now)
            Player.ShowRoom(now, fakeChat);
    }
}
#endif
