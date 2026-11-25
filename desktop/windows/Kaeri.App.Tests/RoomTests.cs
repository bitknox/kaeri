using Kaeri.App;
using Kaeri.App.RoomStand;
using Xunit;

namespace Kaeri.App.Tests;

/// <summary>The room's words and looks, against board v5's frames.</summary>
public class RoomTests
{
    const long Now = 1_800_000_000_000;

    static RoomMember M(string name, MemberState state, bool you = false, long ago = 0, float? behind = null, bool pausedRoom = false, string? episode = null) =>
        new(name.ToLowerInvariant(), name, you, state, Now - ago, behind, episode is null ? null : new RoomEpisode(1, episode), pausedRoom, false);

    static RoomView View(bool playing = true, string? pausedBy = null, PausedFor[]? waiting = null, bool cap = false,
        RoomConnection? connection = null, RelayHealth[]? relays = null, RoomUpNext? upNext = null) =>
        new("room", "Ada", connection ?? new RoomConnection.Connected(), new RoomEpisode(1, "9"), "kaeri.app/r#v1.x", playing, pausedBy,
            waiting ?? [], cap, false, upNext, relays ?? [new("relay-one.example", true, 40), new("relay-two.example", true, 40), new("relay-three.example", true, 40)], []);

    static RoomScene Scene(RoomView view, RoomMember[] members, RoomStart? start = null, RoomInvited? invited = null, RoomMoveOffer? offer = null, string? carried = null) =>
        new(view, members, Now, start ?? new RoomStart.Started(), 724, "Episode 9", "Episode 10", invited, offer, carried);

    static RoomMember[] Three(MemberState ren = MemberState.Watching, long ago = 0) =>
        [M("Ada", MemberState.Watching, you: true), M("Mika", MemberState.Watching), M("Ren", ren, ago: ago)];

    [Fact]
    public void WatchingTogetherSaysTheEpisodeAndThePlace()
    {
        var line = Rooms.Line(Scene(View(), Three()));
        Assert.Equal(("Watching together", "Episode 9 · 12:04"), (line.Title, line.Detail));
        Assert.Equal(RoomAsk.None, line.Ask);
    }

    [Fact]
    public void AWaitNamesWhoAndCountsTheSeconds()
    {
        var waiting = new[] { new PausedFor("Ren", PauseReason.Loading, Now - 6_400) };
        var line = Rooms.Line(Scene(View(playing: false, waiting: waiting), Three(MemberState.Loading, 6_400)));
        Assert.Equal(("Paused for Ren", "Loading · 6 s"), (line.Title, line.Detail));
        Assert.Equal("wait", line.Kind);
    }

    [Fact]
    public void ABufferingWaitSaysBuffering()
    {
        var waiting = new[] { new PausedFor("Ren", PauseReason.Buffering, Now - 3_000) };
        var line = Rooms.Line(Scene(View(playing: false, waiting: waiting), Three(MemberState.Buffering, 3_000)));
        Assert.Equal(("Paused for Ren", "Buffering · 3 s"), (line.Title, line.Detail));
    }

    [Fact]
    public void AtTwentySecondsCarryOnComesForward()
    {
        var waiting = new[] { new PausedFor("Ren", PauseReason.Loading, Now - 20_000) };
        var line = Rooms.Line(Scene(View(playing: false, waiting: waiting, cap: true), Three(MemberState.Loading, 20_000)));
        Assert.Equal(("Ren is still loading", "The room has waited 20 s"), (line.Title, line.Detail));
        Assert.Equal((RoomAsk.CarryOn, "Carry on without Ren"), (line.Ask, line.AskText));
    }

    [Fact]
    public void TwoWaitedForAreNamedAndCarryOnSaysThem()
    {
        var waiting = new[] { new PausedFor("Ren", PauseReason.Loading, Now - 21_000), new PausedFor("Mika", PauseReason.Loading, Now - 20_000) };
        var line = Rooms.Line(Scene(View(playing: false, waiting: waiting, cap: true), Three(MemberState.Loading)));
        Assert.Equal("Ren and Mika are still loading", line.Title);
        Assert.Equal("Carry on without them", line.AskText);
    }

    [Fact]
    public void AfterCarryOnTheLineSaysWhoAndWhoJoinsLater()
    {
        var line = Rooms.Line(Scene(View(), Three(MemberState.Loading, 24_000), carried: "Mika"));
        Assert.Equal(("Watching together", "Mika carried on · Ren joins when ready"), (line.Title, line.Detail));
    }

    [Fact]
    public void YourOwnWaitSaysWhoIsWaiting()
    {
        var members = new[] { M("Ren", MemberState.Loading, you: true, ago: 6_000), M("Mika", MemberState.Paused), M("Ada", MemberState.Paused) };
        var waiting = new[] { new PausedFor("Ren", PauseReason.Loading, Now - 6_000) };
        var line = Rooms.Line(Scene(View(playing: false, waiting: waiting), members));
        Assert.Equal(("Paused for you", "Mika and Ada are waiting · 6 s"), (line.Title, line.Detail));
    }

    [Fact]
    public void APauseSaysWhoAndWhere()
    {
        Assert.Equal(("Mika paused", "at 12:04"), Pair(Rooms.Line(Scene(View(playing: false, pausedBy: "Mika"), Three()))));
        Assert.Equal(("You paused", "at 12:04"), Pair(Rooms.Line(Scene(View(playing: false, pausedBy: "Ada"), Three()))));
    }

    [Fact]
    public void BeforePlayTheLineSaysHowItStarts()
    {
        Assert.Equal(("Not started yet", "Play starts it for everyone"), Pair(Rooms.Line(Scene(View(playing: false), Three(MemberState.Loading, 4_000), new RoomStart.Before()))));
        Assert.Equal(("Starting in 3", "Everyone is ready"), Pair(Rooms.Line(Scene(View(playing: false), Three(), new RoomStart.Counting(3)))));
        var waiting = new[] { new PausedFor("Ren", PauseReason.Loading, Now - 6_000) };
        Assert.Equal(("Starting when Ren is ready", "Loading · 6 s"), Pair(Rooms.Line(Scene(View(playing: false, waiting: waiting), Three(MemberState.Loading, 6_000), new RoomStart.Before()))));
        var cap = Rooms.Line(Scene(View(playing: false, waiting: waiting, cap: true), Three(MemberState.Loading, 20_000), new RoomStart.Before()));
        Assert.Equal((RoomAsk.Start, "Start without Ren"), (cap.Ask, cap.AskText));
    }

    [Fact]
    public void ReadyIsAutomaticBeforePlay()
    {
        var scene = Scene(View(playing: false), Three(MemberState.Loading, 4_000), new RoomStart.Before());
        var rows = Rooms.Rows(scene);
        Assert.Equal(["Ready", "Ready", "Loading · 4 s"], rows.Select(r => r.Word));
        Assert.Equal("Ada (you), Ready", rows[0].Spoken);
    }

    [Fact]
    public void AnInviteAsksToJoinAndPastTenWarnsFirst()
    {
        var line = Rooms.Line(Scene(View(), Three(), invited: new RoomInvited("Mika", "Natsukaze Relay", "Episode 9", 3)));
        Assert.Equal(("Mika invited you", "Natsukaze Relay · Episode 9"), Pair(line));
        Assert.Equal(("Join the room", "Not now"), (line.AskText, line.SecondText));
        var over = Rooms.Line(Scene(View(), Three(), invited: new RoomInvited("Mika", "Natsukaze Relay", "Episode 9", 10)));
        Assert.Equal("10 people are here already · rooms work best with 10 or fewer", over.Detail);
        Assert.Equal(RoomAsk.JoinAnyway, over.Ask);
    }

    [Fact]
    public void AMoveOfferAsksGoAlongInTheLine()
    {
        var offer = new RoomMoveOffer("o1", "Mika", ["Ada", "Ren"], "Natsukaze Relay", "9", Now + 600_000);
        var line = Rooms.Line(Scene(View(), Three(), offer: offer));
        Assert.Equal(("Mika moved the room", "Go along? Ren is asked too. The episode carries on from 12:04."), Pair(line));
        Assert.Equal(("Go along", "Stay here"), (line.AskText, line.SecondText));
    }

    [Fact]
    public void YourConnectionAndARelayDownSayWhatGoesOn()
    {
        Assert.Equal(("Reconnecting to the room", "No relay is answering · your episode keeps playing"),
            Pair(Rooms.Line(Scene(View(connection: new RoomConnection.Reconnecting(0)), Three()))));
        var relays = new RelayHealth[] { new("a.example", true, 40), new("b.example", false, null), new("c.example", true, 40) };
        Assert.Equal(("Watching together", "1 of 3 relays isn't answering · the room carries on"), Pair(Rooms.Line(Scene(View(relays: relays), Three()))));
    }

    [Fact]
    public void BehindYourselfSaysTheGap()
    {
        var members = new[] { M("Ada", MemberState.Behind, you: true, behind: 4.2f), M("Mika", MemberState.Watching), M("Ren", MemberState.Watching) };
        Assert.Equal(("Watching together", "You're 4 s behind · catching up"), Pair(Rooms.Line(Scene(View(), members))));
    }

    [Fact]
    public void UpNextCountsDownForEveryone()
    {
        var up = new RoomUpNext(new RoomEpisode(1, "10"), Now + 7_200, [], null);
        Assert.Equal(("Up next in 8 s", "Episode 10 plays for everyone"), Pair(Rooms.Line(Scene(View(upNext: up), Three()))));
    }

    [Fact]
    public void EachStateHasItsWordAndRing()
    {
        var s = Scene(View(), Three());
        (string, RingKind, double) Of(RoomMember m)
        {
            var f = Rooms.Face(s, m);
            return (Rooms.Word(s, m), f.Ring, f.Dim);
        }
        Assert.Equal(("Watching", RingKind.Water, 1), Of(M("Ren", MemberState.Watching)));
        Assert.Equal(("Paused it", RingKind.Haze, 1), Of(M("Ren", MemberState.Paused, pausedRoom: true)));
        Assert.True(Rooms.Face(s, M("Ren", MemberState.Paused, pausedRoom: true)).PauseBadge);
        Assert.Equal(("Paused", RingKind.Haze, 1), Of(M("Ren", MemberState.Paused)));
        Assert.Equal(("Behind · 4 s", RingKind.Water, 1), Of(M("Ren", MemberState.Behind, behind: 4)));
        Assert.Equal(("Loading · 6 s", RingKind.Progress, 1), Of(M("Ren", MemberState.Loading, ago: 6_900)));
        Assert.Equal(0.3, Rooms.Face(s, M("Ren", MemberState.Loading, ago: 6_000)).Fraction, 3);
        Assert.Equal(("Buffering · 3 s", RingKind.Progress, 1), Of(M("Ren", MemberState.Buffering, ago: 3_000)));
        Assert.Equal(("On episode 8", RingKind.None, 0.5), Of(M("Ren", MemberState.OtherEpisode, episode: "8")));
        Assert.Equal(("Choosing a source", RingKind.None, 0.5), Of(M("Ren", MemberState.Choosing)));
        Assert.Equal(("No source for episode 9", RingKind.None, 0.5), Of(M("Ren", MemberState.NoSource)));
        Assert.True(Rooms.Face(s, M("Ren", MemberState.NoSource)).ErrorDot);
        Assert.Equal(("Player closed", RingKind.None, 0.5), Of(M("Ren", MemberState.PlayerClosed)));
        Assert.Equal(("Reconnecting", RingKind.None, 0.5), Of(M("Ren", MemberState.Reconnecting)));
        Assert.Equal(("Left", RingKind.None, 0.35), Of(M("Ren", MemberState.Left)));
    }

    [Fact]
    public void YourRowComesFirstAndMutedPeopleSaySo()
    {
        var members = new[] { M("Mika", MemberState.Watching), M("Ada", MemberState.Watching, you: true), M("Taro", MemberState.Watching) };
        var rows = Rooms.Rows(Scene(View(), members), ["Taro"]);
        Assert.Equal(["Ada", "Mika", "Taro"], rows.Select(r => r.Name));
        Assert.Equal("Watching · Muted", rows[2].Word);
    }

    [Fact]
    public void PastFourFacesTheRestFoldIntoMore()
    {
        var six = new[] { "Ada", "Mika", "Ren", "Sora", "Kenji", "Noa" }.Select((n, i) => M(n, i == 3 ? MemberState.NoSource : MemberState.Watching, you: i == 0)).ToArray();
        var (shown, more, notWatching) = Rooms.Capped(Rooms.Rows(Scene(View(), six)));
        Assert.Equal(3, shown.Count);
        Assert.Equal((3, 1), (more, notWatching));
        Assert.Equal(4, Rooms.Capped(Rooms.Rows(Scene(View(), Three().Append(M("Sora", MemberState.Watching)).ToArray()))).Shown.Count);
    }

    [Fact]
    public void TheStripShowsWhileAnyoneIsNotWatchingAndForEightSecondsAfterAChange()
    {
        var calm = Scene(View(), Three());
        Assert.True(Rooms.StripShows(calm, Now - 7_900));
        Assert.False(Rooms.StripShows(calm, Now - 8_000));
        Assert.True(Rooms.StripShows(Scene(View(), Three(MemberState.Choosing)), Now - 60_000));
        var waiting = new[] { new PausedFor("Ren", PauseReason.Loading, Now - 6_000) };
        Assert.Equal("Paused for Ren · 6 s", Rooms.StripWords(Scene(View(playing: false, waiting: waiting), Three(MemberState.Loading, 6_000))));
    }

    [Fact]
    public void TheMoveButtonCountsTheOthers()
    {
        Assert.Equal("Move with 2 others", Rooms.MoveButton(2));
        Assert.Equal("Move with 1 other", Rooms.MoveButton(1));
    }

    [Fact]
    public void FacesTakeTheFirstCharacterAsRead()
    {
        Assert.Equal("A", Rooms.Initial("ada"));
        Assert.Equal("?", Rooms.Initial("  "));
        Assert.Equal("É", Rooms.Initial("élodie"));
    }

    static (string, string) Pair(RoomLine line) => (line.Title, line.Detail);
}
