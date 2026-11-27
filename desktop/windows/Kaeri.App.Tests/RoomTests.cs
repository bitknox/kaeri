using Kaeri.App;
using Kaeri.Engine.Model;
using Xunit;

namespace Kaeri.App.Tests;

/// <summary>The room's words and looks, against board v5's frames.</summary>
public class RoomTests
{
    const long Now = 1_800_000_000_000;

    static RoomMember M(string name, MemberState state, bool you = false, long ago = 0, float? behind = null, bool pausedRoom = false,
        string? episode = null, bool waited = true, bool muted = false) =>
        new(name.ToLowerInvariant(), name, you, state, Now - ago, behind, episode is null ? null : new RoomEpisode(1, episode), pausedRoom, waited, false, muted);

    static PausedFor Wait(string name, long ago, WaitReason why = WaitReason.Loading, bool you = false) =>
        new(name.ToLowerInvariant(), name, why, Now - ago, you);

    static RoomView View(bool playing = true, string? pausedBy = null, bool pausedByYou = false, PausedFor[]? waiting = null, bool cap = false,
        RoomConnection? connection = null, RelayHealth[]? relays = null, RoomUpNext? upNext = null, bool before = false, long? startsAt = null,
        RoomCarriedOn? carried = null, RoomMoved? moved = null) =>
        new("room", "Ada", connection ?? new RoomConnection.Connected(), new RoomEpisode(1, "9"),
            new RoomEpisodeInfo(1, "9", "Natsukaze Relay", "The Long Leg"), new RoomEpisodeInfo(1, "10", "Natsukaze Relay"),
            "https://kaeri.app/room#v=3&k=x", playing, before, startsAt, pausedBy, pausedByYou, 724, Now, waiting ?? [], null, null, cap, false,
            carried, upNext, null, relays ?? [new("wss://relay-one.example", true, 40), new("wss://relay-two.example", true, 40), new("wss://relay-three.example", true, 40)],
            false, [], moved, null);

    static RoomScene Scene(RoomView view, RoomMember[] members, RoomGlimpse? invited = null, RoomMoveOffer? offer = null) =>
        new(view, members, Now, invited, offer);

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
        var waiting = new[] { Wait("Ren", 6_400, WaitReason.Loading) };
        var line = Rooms.Line(Scene(View(playing: false, waiting: waiting), Three(MemberState.Loading, 6_400)));
        Assert.Equal(("Paused for Ren", "Loading · 6\u00A0s"), (line.Title, line.Detail));
        Assert.Equal("wait", line.Kind);
    }

    [Fact]
    public void ABufferingWaitSaysBuffering()
    {
        var waiting = new[] { Wait("Ren", 3_000, WaitReason.Buffering) };
        var line = Rooms.Line(Scene(View(playing: false, waiting: waiting), Three(MemberState.Buffering, 3_000)));
        Assert.Equal(("Paused for Ren", "Buffering · 3\u00A0s"), (line.Title, line.Detail));
    }

    [Fact]
    public void ASeekingWaitSaysSeekingOnTheLineAndTheRow()
    {
        var waiting = new[] { Wait("Ren", 2_000, WaitReason.Seeking) };
        var scene = Scene(View(playing: false, waiting: waiting), Three(MemberState.Loading, 2_000));
        var line = Rooms.Line(scene);
        Assert.Equal(("Paused for Ren", "Seeking · 2\u00A0s"), (line.Title, line.Detail));
        Assert.Equal("Seeking · 2\u00A0s", Rooms.Word(scene, scene.Members[2]));
    }

    [Fact]
    public void ThePlaceSaysWhyARoomEndedWithABoldStartAndItsHint()
    {
        Assert.Equal(("Couldn't reach this room's relays.", "Ask for a new invite."), RoomWords.LeftLine(new RoomLeftWhy.Unreachable()));
        Assert.Null(RoomWords.LeftLine(new RoomLeftWhy.Full()).Then);
    }

    [Fact]
    public void AtTwentySecondsCarryOnComesForward()
    {
        var waiting = new[] { Wait("Ren", 20_000, WaitReason.Loading) };
        var line = Rooms.Line(Scene(View(playing: false, waiting: waiting, cap: true), Three(MemberState.Loading, 20_000)));
        Assert.Equal(("Ren is still loading", "The room has waited 20\u00A0s"), (line.Title, line.Detail));
        Assert.Equal((RoomAsk.CarryOn, "Carry on without Ren"), (line.Ask, line.AskText));
    }

    [Fact]
    public void TheRoomsPlaceTruncatesItsSecondsAsTheEngineDoes()
    {
        // 12.9 s is 0:12 everywhere: rounding it to 0:13 put the line a second off the other apps'.
        Assert.Equal("0:12", Rooms.Clock(12.9));
        Assert.Equal("0:00", Rooms.Clock(-1));
        Assert.Equal("1:00:05", Rooms.Clock(3605.99));
    }

    [Fact]
    public void TheCapSaysWhyItWaitsWhenEveryoneWaitedForWaitsTheSameWay()
    {
        string Title(params PausedFor[] waiting) =>
            Rooms.Line(Scene(View(playing: false, waiting: waiting, cap: true), Three(MemberState.Loading, 20_000))).Title;
        Assert.Equal("Ren is still seeking", Title(Wait("Ren", 20_000, WaitReason.Seeking)));
        Assert.Equal("Ren is still buffering", Title(Wait("Ren", 20_000, WaitReason.Buffering)));
        Assert.Equal("Ren and Mika are still loading", Title(Wait("Ren", 20_000, WaitReason.Seeking), Wait("Mika", 20_000, WaitReason.Buffering)));
    }

    [Fact]
    public void TwoWaitedForAreNamedAndCarryOnSaysThem()
    {
        var waiting = new[] { Wait("Ren", 21_000, WaitReason.Loading), Wait("Mika", 20_000, WaitReason.Loading) };
        var line = Rooms.Line(Scene(View(playing: false, waiting: waiting, cap: true), Three(MemberState.Loading)));
        Assert.Equal("Ren and Mika are still loading", line.Title);
        Assert.Equal("Carry on without them", line.AskText);
    }

    [Fact]
    public void AfterCarryOnTheLineSaysWhoAndWhoJoinsLater()
    {
        var line = Rooms.Line(Scene(View(carried: new RoomCarriedOn("Mika", false, ["Ren"])), Three(MemberState.Loading, 24_000)));
        Assert.Equal(("Watching together", "Mika carried on · Ren joins when ready"), (line.Title, line.Detail));
    }

    [Fact]
    public void YourOwnWaitSaysWhoIsWaiting()
    {
        var members = new[] { M("Ren", MemberState.Loading, you: true, ago: 6_000), M("Mika", MemberState.Paused), M("Ada", MemberState.Paused) };
        var waiting = new[] { Wait("Ren", 6_000, WaitReason.Loading, you: true) };
        var line = Rooms.Line(Scene(View(playing: false, waiting: waiting), members));
        Assert.Equal(("Paused for you", "Mika and Ada are waiting · 6\u00A0s"), (line.Title, line.Detail));
    }

    [Fact]
    public void YourOwnWaitLeavesOutThoseWaitedForToo()
    {
        var members = new[] { M("Ada", MemberState.Loading, you: true, ago: 6_000), M("Mika", MemberState.Paused), M("Ren", MemberState.Loading, ago: 6_000) };
        var waiting = new[] { Wait("Ada", 6_000, WaitReason.Seeking, you: true), Wait("Ren", 6_000, WaitReason.Seeking) };
        var line = Rooms.Line(Scene(View(playing: false, waiting: waiting), members));
        Assert.Equal(("Paused for you", "Mika is waiting · 6\u00A0s"), (line.Title, line.Detail));
    }

    [Fact]
    public void APauseSaysWhoAndWhere()
    {
        Assert.Equal(("Mika paused", "at 12:04"), Pair(Rooms.Line(Scene(View(playing: false, pausedBy: "Mika"), Three()))));
        Assert.Equal(("You paused", "at 12:04"), Pair(Rooms.Line(Scene(View(playing: false, pausedBy: "Ada", pausedByYou: true), Three()))));
    }

    [Fact]
    public void BeforePlayTheLineSaysHowItStarts()
    {
        Assert.Equal(("Not started yet", "Play starts it for everyone"), Pair(Rooms.Line(Scene(View(playing: false, before: true), Three(MemberState.Loading, 4_000)))));
        Assert.Equal(("Starting in 3", "Everyone is ready"), Pair(Rooms.Line(Scene(View(playing: false, before: true, startsAt: Now + 3_000), Three()))));
        var waiting = new[] { Wait("Ren", 6_000, WaitReason.Loading) };
        Assert.Equal(("Starting when Ren is ready", "Loading · 6\u00A0s"), Pair(Rooms.Line(Scene(View(playing: false, waiting: waiting, before: true), Three(MemberState.Loading, 6_000)))));
        var cap = Rooms.Line(Scene(View(playing: false, waiting: waiting, cap: true, before: true), Three(MemberState.Loading, 20_000)));
        Assert.Equal((RoomAsk.Start, "Start without Ren"), (cap.Ask, cap.AskText));
    }

    [Fact]
    public void ReadyIsAutomaticBeforePlay()
    {
        var scene = Scene(View(playing: false, before: true), Three(MemberState.Loading, 4_000));
        var rows = Rooms.Rows(scene);
        Assert.Equal(["Ready", "Ready", "Loading · 4\u00A0s"], rows.Select(r => r.Word));
        Assert.Equal("Ada (you), Ready", rows[0].Spoken);
    }

    [Fact]
    public void AnInviteAsksToJoinAndPastTenWarnsFirst()
    {
        var line = Rooms.Line(Invited(3));
        Assert.Equal(("Mika invited you", "Natsukaze Relay · Episode 9"), Pair(line));
        Assert.Equal(("Join the room", "Not now"), (line.AskText, line.SecondText));
        var over = Rooms.Line(Invited(10));
        Assert.Equal("10 people are here already · rooms work best with 10 or fewer", over.Detail);
        Assert.Equal(RoomAsk.JoinAnyway, over.Ask);
    }

    [Fact]
    public void AMoveOfferAsksGoAlongInTheLine()
    {
        var offer = new RoomMoveOffer("o1", "Mika", "Natsukaze Relay", "9", Now + 600_000, ["Ada", "Ren"]);
        var line = Rooms.Line(Scene(View(), Three(), offer: offer));
        Assert.Equal(("Mika moved the room", "Go along? Ren is asked too. The episode carries on from 12:04."), Pair(line));
        Assert.Equal(("Go along", "Stay"), (line.AskText, line.SecondText));
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
        Assert.Equal(("Watching together", "You're 4\u00A0s behind · catching up"), Pair(Rooms.Line(Scene(View(), members))));
    }

    [Fact]
    public void UpNextCountsDownForEveryone()
    {
        var up = new RoomUpNext(new RoomEpisodeInfo(1, "10", "Natsukaze Relay"), Now + 7_200, [], null);
        Assert.Equal(("Up next in 8\u00A0s", "Episode 10 plays for everyone"), Pair(Rooms.Line(Scene(View(upNext: up), Three()))));
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
        Assert.Equal(("Behind · 4\u00A0s", RingKind.Water, 1), Of(M("Ren", MemberState.Behind, behind: 4)));
        Assert.Equal(("Loading · 6\u00A0s", RingKind.Progress, 1), Of(M("Ren", MemberState.Loading, ago: 6_900)));
        Assert.Equal(0.3, Rooms.Face(s, M("Ren", MemberState.Loading, ago: 6_000)).Fraction, 3);
        Assert.Equal(("Buffering · 3\u00A0s", RingKind.Progress, 1), Of(M("Ren", MemberState.Buffering, ago: 3_000)));
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
        var members = new[] { M("Mika", MemberState.Watching), M("Ada", MemberState.Watching, you: true), M("Taro", MemberState.Watching, muted: true) };
        var rows = Rooms.Rows(Scene(View(), members));
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
        var waiting = new[] { Wait("Ren", 6_000, WaitReason.Loading) };
        Assert.Equal("Paused for Ren · 6\u00A0s", Rooms.StripWords(Scene(View(playing: false, waiting: waiting), Three(MemberState.Loading, 6_000))));
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

    /// <summary>An invite looked into, with this many people there (ten or more is full).</summary>
    static RoomScene Invited(int people)
    {
        var members = Enumerable.Range(0, people).Select(i => M(i == 0 ? "Mika" : $"P{i}", MemberState.Watching)).ToArray();
        return Rooms.Invited(new RoomGlimpse(null, "Mika", members, new RoomEpisodeInfo(1, "9", "Natsukaze Relay"), true, people >= 10), Now);
    }

    [Fact]
    public void TheOldRoomSaysWhoMovedAndWhoWentAlong()
    {
        var line = Rooms.Line(Scene(View(moved: new RoomMoved("Mika", ["Ren"])), [M("Ada", MemberState.Watching, you: true), M("Taro", MemberState.Watching)]));
        Assert.Equal(("Mika moved to a new room", "Ren went along · this room carries on"), Pair(line));
    }

    [Fact]
    public void YouCarryingOnSaysYou()
    {
        var line = Rooms.Line(Scene(View(carried: new RoomCarriedOn("Ada", true, ["Ren", "Mika"])), Three(MemberState.Loading)));
        Assert.Equal("You carried on · they join when ready", line.Detail);
    }

    static (string, string) Pair(RoomLine line) => (line.Title, line.Detail);

    static RoomMoveOffer Offer() => new("offer-1", "Mika", "Natsukaze Relay", "9", Now + 60_000);

    [Fact]
    public void AMoveOfferNeverOutlivesItsRoom()
    {
        var left = new Known();
        left.Take(new Event.RoomJoined(View()));
        left.Take(new Event.RoomMoveOffer(Offer()));
        left.Take(new Event.RoomLeft(new RoomLeftWhy.Left()));
        Assert.Null(left.RoomMoveOffer);

        // Joining another room without leaving first (a Rejoin from the place) drops it too.
        var other = new Known();
        other.Take(new Event.RoomJoined(View()));
        other.Take(new Event.RoomMoveOffer(Offer()));
        other.Take(new Event.RoomJoined(View() with { RoomId = "another" }));
        Assert.Null(other.RoomMoveOffer);

        // The same room said again keeps the question open.
        var same = new Known();
        same.Take(new Event.RoomJoined(View()));
        same.Take(new Event.RoomMoveOffer(Offer()));
        same.Take(new Event.RoomJoined(View()));
        Assert.NotNull(same.RoomMoveOffer);
    }

    [Fact]
    public void ALookInIsForgottenOnNotNowAndWhenAnotherLinkIsTried()
    {
        var known = new Known();
        known.LookInto(InviteA);
        known.Take(Glimpse(InviteA));
        Assert.NotNull(known.RoomGlimpse);
        known.LookInto(null);
        Assert.Null(known.RoomGlimpse);

        // Another link tried: the first look's glimpse goes, and its late answer never comes back.
        known.LookInto(InviteA);
        known.Take(Glimpse(InviteA));
        known.LookInto(InviteB);
        Assert.Null(known.RoomGlimpse);
        known.Take(Glimpse(InviteA));
        Assert.Null(known.RoomGlimpse);
    }

    const string InviteA = "https://kaeri.app/room#v=3&k=first";
    const string InviteB = "https://kaeri.app/room#v=3&k=second";

    static Event Glimpse(string? invite) => new Event.RoomGlimpse(new RoomGlimpse(By: "Mika", Members: [], Invite: invite));

    static Event LookFailed(string? invite, RoomLeftWhy? why) =>
        new Event.RoomLookFailed(RoomId: "room-1", Reason: new Msg.RoomLookUnreachable(), Invite: invite, Why: why);

    [Fact]
    public void OnlyTheLookWaitedOnShowsItsAnswer()
    {
        var known = new Known();
        // Nothing asked: a glimpse or a failure is nobody's.
        known.Take(Glimpse(InviteA));
        known.Take(LookFailed(InviteA, new RoomLeftWhy.Unreachable()));
        Assert.Null(known.RoomGlimpse);
        Assert.Null(known.RoomLookFailure);

        known.LookInto(InviteB);
        known.Take(Glimpse(InviteA));
        known.Take(LookFailed(InviteA, new RoomLeftWhy.Unreachable()));
        known.Take(Glimpse(null));
        Assert.Null(known.RoomGlimpse);
        Assert.Null(known.RoomLookFailure);

        known.Take(Glimpse(InviteB));
        Assert.NotNull(known.RoomGlimpse);
        known.Take(LookFailed(InviteB, new RoomLeftWhy.Unreachable()));
        Assert.Null(known.RoomGlimpse);
        Assert.Equal(("Couldn't reach this room's relays.", "Ask for a new invite."), RoomWords.LookLine(known.RoomLookFailure!.Value));

        // Looking again starts clean.
        known.LookInto(InviteB);
        Assert.Null(known.RoomLookFailure);
    }

    [Fact]
    public void AFailedLookLeavesTheRoomYouAreIn()
    {
        var known = new Known();
        known.Take(new Event.RoomJoined(View()));
        known.LookInto(InviteA);
        // The room said again while the look is out: it still waits for its answer.
        known.Take(new Event.RoomJoined(View()));
        known.Take(LookFailed(InviteA, new RoomLeftWhy.Unreachable()));
        Assert.NotNull(known.Room);
        Assert.NotNull(known.RoomLookFailure);
        Assert.Null(known.RoomLeftWhy);

        // In another room: the look that led there is done.
        known.LookInto(InviteB);
        known.Take(new Event.RoomJoined(View() with { RoomId = "another" }));
        Assert.Null(known.RoomLooking);
    }

    [Fact]
    public void LeavingARoomNeverEndsALook()
    {
        var known = new Known();
        known.Take(new Event.RoomJoined(View()));
        known.LookInto(InviteA);
        known.Take(Glimpse(InviteA));
        known.Take(new Event.RoomLeft(new RoomLeftWhy.Unreachable()));
        Assert.Null(known.Room);
        Assert.Equal(InviteA, known.RoomLooking);
        Assert.NotNull(known.RoomGlimpse);
    }

    [Fact]
    public void ALookFailureWithoutAJoinReasonSaysTheEnginesWords()
    {
        Assert.Equal(("That isn't a room's invite.", null), RoomWords.LookLine(("That isn't a room's invite.", null)));
        Assert.Equal("Couldn't reach this room's relays. Ask for a new invite.", RoomWords.Look(("x", new RoomLeftWhy.Unreachable())));
    }

    // The place: what failed and why, said when it failed.

    static (RoomPlace Place, Known Known) Place(bool inRoom = false)
    {
        var known = new Known();
        if (inRoom)
            known.Take(new Event.RoomJoined(View()));
        return (new RoomPlace(inRoom), known);
    }

    static (bool, bool) Hear(RoomPlace place, Known known, string? looking, string typed, params Event[] batch)
    {
        foreach (var e in batch)
            known.Take(e);
        return place.Take(batch, known, looking, typed);
    }

    [Fact]
    public void AJoinThatFailsKeepsTheInvitesLinkEvenWhenTheFieldIsEmpty()
    {
        // Opened from outside the app (or written as kaeri://): the field holds nothing like it.
        var (place, known) = Place();
        known.LookInto(InviteA);
        var (_, forget) = Hear(place, known, InviteA, "", new Event.RoomLeft(new RoomLeftWhy.Full()));
        Assert.Equal(InviteA, place.FailedLink);
        Assert.Equal(RoomWords.LeftLine(new RoomLeftWhy.Full()), place.FailedWhy);
        Assert.Equal(1, place.Failures);
        Assert.True(forget);
    }

    [Fact]
    public void AnOldInviteSaysSoAfterAnEarlierRoomFailed()
    {
        var (place, known) = Place();
        known.LookInto(InviteA);
        Hear(place, known, InviteA, InviteA, new Event.RoomLeft(new RoomLeftWhy.Unreachable()));
        place.Try();
        place.Refused("kaeri://party?room=x", "This invite is from an older version.");
        Assert.Equal(("This invite is from an older version.", null), place.FailedWhy);
    }

    [Fact]
    public void AnotherLooksLateFailureNeverCountsAgainstTheField()
    {
        var (place, known) = Place();
        known.LookInto(InviteB);
        Hear(place, known, InviteB, InviteB, LookFailed(InviteB, new RoomLeftWhy.Unreachable()));
        Assert.Equal(1, place.Failures);
        Hear(place, known, InviteB, "typing another", LookFailed(InviteA, new RoomLeftWhy.Unreachable()));
        Assert.Equal(1, place.Failures);
        Assert.Equal(InviteB, place.FailedLink);
    }

    [Fact]
    public void YourRoomEndingLeavesALookAlone()
    {
        var (place, known) = Place(inRoom: true);
        known.LookInto(InviteB);
        Hear(place, known, InviteB, InviteB, Glimpse(InviteB));
        var (_, forget) = Hear(place, known, InviteB, InviteB, new Event.RoomLeft(new RoomLeftWhy.Unreachable()));
        Assert.False(forget);
        Assert.Null(place.FailedLink);
        Assert.Equal(0, place.Failures);
        Assert.NotNull(known.RoomGlimpse);

        // Out of the room now: a join of that invite failing is the invite's.
        Hear(place, known, InviteB, InviteB, new Event.RoomLeft(new RoomLeftWhy.Full()));
        Assert.Equal(InviteB, place.FailedLink);
    }

    [Fact]
    public void ARejoinOfARoomNoLongerKeptSaysSoOnItsRow()
    {
        var (place, known) = Place();
        place.Rejoin("room-1");
        Hear(place, known, null, "", new Event.RoomLookFailed(RoomId: "room-1", Reason: new Msg.RoomLookNotKept(), Invite: null, Why: null));
        Assert.Equal(("That room isn't kept any more.", null), place.RejoinFailed);

        // Another room's answer is not this row's.
        place.Rejoin("room-2");
        Hear(place, known, null, "", new Event.RoomLookFailed(RoomId: "room-1", Reason: new Msg.RoomLookNotKept(), Invite: null, Why: null));
        Assert.Null(place.RejoinFailed);

        // A Rejoin whose room never answers fails as a join.
        Hear(place, known, null, "", new Event.RoomLeft(new RoomLeftWhy.Unreachable()));
        Assert.Equal((T.RoomErrorNobodyAnsweredLead, T.RoomErrorNobodyAnsweredLine), place.RejoinFailed);
        Assert.Null(place.FailedLink);
    }

    [Fact]
    public void TheRoomsEpisodeOpensFromItsStartNeverTheResumePoint()
    {
        foreach (var (kind, at) in new (RoomStartKind, double?)[] { (RoomStartKind.Start, null), (RoomStartKind.MoveOn, null), (RoomStartKind.Rejoin, 12.0) })
        {
            var play = Assert.IsType<Command.Play>(Rooms.Open(new Event.RoomStarted(900001u, "9", kind, at)));
            Assert.Equal("9", play.Episode);
            Assert.True(play.FromStart, $"{kind} at {at}");
        }
    }
}
