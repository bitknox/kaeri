
namespace Kaeri.App;

/// <summary>
/// The room's words, from the catalogue (i18n/en/ui-room.ftl), named as the board draws them.
/// Episodes stay the engine's text ("9").
/// </summary>
public static class RoomWords
{
    /// <summary>An episode for the catalogue, as the engine names it ("9", "12.5").</summary>
    public static string N(string? episode) => episode ?? "";

    // The column's header
    public static string Title => T.RoomSettingsGroup;
    public static string Invite => T.RoomInvite;
    public static string Details => T.RoomDetails;
    public static string Leave => T.ActionLeave;
    public static string HideRoom => T.RoomHide;
    public static string ShowRoom => T.RoomShow;

    // The room's line
    public static string Watching => T.RoomLineWatching;
    public static string WatchingAt(string episode, string at) => T.RoomLineWatchingDetail(N(episode), at);
    public static string PausedFor(string names) => T.RoomLinePausedFor(names);
    public static string PausedForYou => T.RoomLinePausedForYou;
    public static string Waiting(string names, int many, long seconds) => T.RoomLineWaitingForYou(names, many, seconds);
    public static string Loading(long seconds) => T.RoomStateLoading(seconds);
    public static string Buffering(long seconds) => T.RoomStateBuffering(seconds);
    public static string Seeking(long seconds) => T.RoomStateSeeking(seconds);
    public static string StillLoading(string names, int many) => T.RoomLineStillLoading(names, many);
    public static string WaitedCap => T.RoomLineWaited((long)Rooms.CapSeconds);
    public static string CarryOnWithout(string name) => T.RoomCarryOnWithout(name);
    public static string CarryOnWithoutThem => T.RoomCarryOnWithoutThem;
    public static string StartWithout(string name) => T.RoomStartWithout(name);
    public static string StartWithoutThem => T.RoomStartWithoutThem;
    public static string CarriedOn(string by, string names, int many) =>
        many > 1 ? T.RoomLineCarriedOnMany(by) : T.RoomLineCarriedOnOne(by, names);
    public static string Paused(string by) => T.RoomLinePausedBy(by);
    public static string YouPaused => T.RoomLineYouPaused;
    public static string PausedAt(string at) => T.RoomLinePausedAt(at);
    public static string NotStarted => T.RoomLineNotStarted;
    public static string PlayStartsIt => T.RoomLinePlayStarts;
    public static string StartingIn(long n) => T.RoomLineStartingIn(n);
    public static string EveryoneReady => T.RoomLineEveryoneReady;
    public static string StartingWhen(string names, int many) => T.RoomLineStartingWhen(names, many);
    public static string UpNextIn(long seconds) => T.RoomLineUpNext(seconds);
    public static string PlaysForEveryone(string episode) => T.RoomLineUpNextDetail(N(episode));
    public static string ReconnectingRoom => T.RoomLineReconnecting;
    public static string NoRelayAnswers => T.RoomLineReconnectingDetail;
    public static string RelaysDown(int down, int of) => T.RoomLineRelaysDown(down, of);
    public static string YouBehind(long seconds) => T.RoomLineYouBehind(seconds);
    public static string Invited(string by) => T.RoomInvitedBy(by);
    public static string InvitedTo(string title, string episode) => T.RoomInvitedDetail(title, N(episode));
    public static string Over(int people) => T.RoomInvitedOver(people);
    public static string JoinRoom => T.RoomJoin;
    public static string JoinAnyway => T.RoomJoinAnyway;
    public static string NotNow => T.ActionNotNow;
    public static string MovedTheRoom(string by) => T.RoomLineMoved(by);
    public static string GoAlongAsk(string others, int many, string at) =>
        many == 0 ? T.RoomLineMovedDetailAlone(at) : T.RoomLineMovedDetail(others, many, at);
    public static string GoAlong => T.RoomGoAlong;
    public static string StayHere => T.RoomStay;
    public static string MovedAway(string by) => T.RoomLineMovedAway(by);
    public static string WentAlong(string names) => T.RoomLineWentAlong(names);
    public static string WithSeconds(string line, long seconds) => T.RoomLineWithSeconds(line, seconds);

    // Names joined
    public static string Two(string a, string b) => T.RoomNamesAnd(a, b);
    public static string People(int n) => T.RoomNamesPeople(n);
    public static string YouAlone => T.RoomYou;
    public static string YouMark => T.RoomYouMarker;

    // The people list
    public static string PeopleList => T.RoomPeople;

    // A row's word
    public static string StateWatching => T.RoomStateWatching;
    public static string StateReady => T.RoomStateReady;
    public static string StatePausedIt => T.RoomStatePausedIt;
    public static string StatePaused => T.RoomStatePaused;
    public static string StateBehind(long s) => T.RoomStateBehind(s);
    public static string StateLoading(long s) => T.RoomStateLoading(s);
    public static string StateBuffering(long s) => T.RoomStateBuffering(s);
    public static string StateSeeking(long s) => T.RoomStateSeeking(s);
    public static string StateOther(string? episode) => T.RoomStateOtherEpisode(N(episode));
    public static string StateChoosing => T.RoomStateChoosing;
    public static string StateNoSource(string episode) => T.RoomStateNoSource(N(episode));
    public static string StateClosed => T.RoomStatePlayerClosed;
    public static string StateReconnecting => T.RoomStateReconnecting;
    public static string StateLeft => T.RoomStateLeft;
    public static string Muted(string word) => T.RoomStateAndMuted(word);
    public static string Spoken(string name, string word) => T.RoomFaceLabel(name, word);
    public static string More(int n) => T.RoomMoreFaces(n);
    public static string MoreSpoken(int n) => T.RoomMoreFacesLabel(n);
    public static string NotWatching(int n) => T.RoomMoreNotWatching(n);
    public static string AllWatching => T.RoomMoreAllWatching;

    // The chat
    public static string SaySomething => T.RoomChatField;
    public static string Send => T.RoomChatSend;
    public static string ChatOpensOnJoin => T.RoomChatOpens;
    public static string NotSent => T.RoomChatNotSent;
    public static string TryAgain => T.ActionTryAgain;

    // Before play, over the video
    public static string UpInTheRoom => T.RoomUpInRoom;
    public static string RoomIsWatching => T.RoomRoomIsWatching;
    /// <summary>"Episode 9 · The Long Leg · 24 min"; without a name, the plain episode and its length.</summary>
    public static string EpisodeLength(string episode, string? title, long minutes) => title is { Length: > 0 }
        ? T.RoomBannerEpisode(N(episode), title, T.RoomMinutes(minutes))
        : T.RoomBannerEpisodePlain(N(episode), T.RoomMinutes(minutes));
    public static string PlaysFromYourSources => T.RoomPlaysFromSources;
    public static string PlayForEveryone => T.RoomPlayForEveryone;
    public static string AnyoneCanStart => T.RoomAnyoneCanStart;
    public static string ChooseSourceFor(string episode) => T.RoomChooseSource(N(episode));
    public static string RoomKeepsWatching => T.RoomChooseSourceLine;
    public static string PlayInRoom => T.RoomPlayInRoom;
    public static string NoneHas(string episode) => T.RoomNoSourceTitle(N(episode));
    public static string LookedIn(int sources) => T.RoomNoSourceLine(sources);
    public static string OpenSources => T.RoomOpenSources;
    public static string SearchTitles => T.RoomSearchTitles;
    public static string RoomPausedUntil(long seconds) => T.RoomPausedUntilStart(seconds);
    public static string DontWait => T.RoomDontWait;
    public static string StartingEpisode(string episode) => T.RoomStartingEpisode(N(episode));

    // Up next and Skip
    public static string UpNextForRoom => T.RoomUpNext;
    public static string PlaysInSeconds(long seconds) => T.RoomUpNextCount(seconds);
    public static string UpNextWaiting(string name) => T.RoomUpNextWaiting(name);
    public static string PlayNextNow => T.RoomPlayNextNow;
    public static string SkipOpening => T.RoomSkipOpening;

    // Invite
    public static string InviteAnyone => T.RoomInviteAnyone;
    public static string InviteKey => T.RoomInviteKey;
    public static string CopyLink => T.RoomCopyLink;
    public static string LinkCopied => T.RoomLinkCopied;
    public static string Share => T.RoomShare;
    public static string NewInvite => T.RoomNewInvite;
    public static string NewInviteBody => T.RoomNewInviteLine;
    public static string MoveRoom => T.RoomMoveOpen;
    public static string MoveRoomBody => T.RoomMoveOpenLine;

    // Details
    public static string Encryption => T.RoomDetailsEncryption;
    public static string EndToEnd => T.RoomDetailsEndToEnd;
    public static string Relays => T.RoomRelays;
    public static string RelaysAnswering(int up, int of) => up == of ? T.RoomRelaysAnswering(up) : T.RoomRelaysAnsweringOf(up, of);
    public static string Rtt(uint ms) => T.RoomRelayLatency(ms);
    public static string NotAnswering => T.RoomRelayNotAnswering;
    public static string RelaysBody => T.RoomDetailsRelaysLine;
    public static string EditRelays => T.RoomEditRelays;
    public static string EditRelaysTitle => T.RoomEditRelaysTitle;
    public static string EditRelaysBody => T.RoomEditRelaysLine;
    public static string Remove(string what) => T.RoomRelayRemove(what);
    /// <summary>The relay field's placeholder: a scheme, not words.</summary>
    public const string RelayScheme = "wss://";
    public static string Add => T.RoomRelayAdd;
    public static string RelaysInSettings => T.RoomEditRelaysSettings;
    public static string Done => T.ActionDone;

    // Leave
    public static string LeaveTitle => T.RoomLeaveTitle;
    public static string LeaveBody => T.RoomLeaveLine;
    public static string LeaveCloseBody => T.RoomLeaveLineKeep;
    public static string KeepRoom => T.RoomKeep;
    public static string Cancel => T.ActionCancel;

    // Move to a new room
    public static string MoveTitle => T.RoomMoveTitle;
    public static string MoveBody => T.RoomMoveBody;
    public static string MoveFoot(string at) => T.RoomMoveFoot(at);
    public static string AlwaysGoes => T.RoomMoveAlwaysGoes;
    public static string MutedNotTicked => T.RoomMoveMuted;
    public static string MoveWith(int others) => T.RoomMoveWith(others);

    // Why a room ended, when it ended on its own
    public static string Left(Kaeri.Engine.Model.RoomLeftWhy why) => why switch
    {
        Kaeri.Engine.Model.RoomLeftWhy.Unreachable => T.RoomErrorUnreachable,
        Kaeri.Engine.Model.RoomLeftWhy.Full => T.RoomErrorFull,
        Kaeri.Engine.Model.RoomLeftWhy.OldInvite => T.RoomErrorOldInvite,
        Kaeri.Engine.Model.RoomLeftWhy.Failed f => f.Reason.Text(),
        _ => T.RoomErrorNobodyAnswered,
    };

    /// <summary>The room's place says why in two parts: a bold start, then the hint when there is one.</summary>
    public static (string Lead, string? Then) LeftLine(Kaeri.Engine.Model.RoomLeftWhy why) => why switch
    {
        Kaeri.Engine.Model.RoomLeftWhy.Unreachable => (T.RoomErrorUnreachableLead, T.RoomErrorAskNewInvite),
        Kaeri.Engine.Model.RoomLeftWhy.Full or Kaeri.Engine.Model.RoomLeftWhy.OldInvite or Kaeri.Engine.Model.RoomLeftWhy.Failed => (Left(why), null),
        _ => (T.RoomErrorNobodyAnsweredLead, T.RoomErrorNobodyAnsweredLine),
    };
}
