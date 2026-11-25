namespace Kaeri.App.RoomStand;

// Stand-ins for the engine's room FFI (spec room-nostr.md, section 6), shaped as uniffi-bindgen-cs
// writes records and enums, so the swap to the generated types is deleting this file and the
// `using Kaeri.App.RoomStand;` lines. Names follow the spec's Party-to-Room renames.

/// <summary>A member's state, as the engine works it out (section 5's table).</summary>
public enum MemberState
{
    Watching,
    Paused,
    Loading,
    Buffering,
    Behind,
    OtherEpisode,
    Choosing,
    NoSource,
    PlayerClosed,
    Reconnecting,
    Left,
}

/// <summary>Why the room waits for someone.</summary>
public enum PauseReason
{
    Loading,
    Seeking,
    Buffering,
}

/// <summary>Someone the room waits for, and since when (epoch ms on this device's clock).</summary>
public record PausedFor(string Name, PauseReason Reason, long SinceMs);

/// <summary>One of the room's relays and how it answers.</summary>
public record RelayHealth(string Url, bool Healthy, uint? RttMs);

/// <summary>An episode of a title, as the room names it.</summary>
public record RoomEpisode(uint Id, string Episode);

/// <summary>Up next for the room: <c>EndsAtMs</c> is null while the card waits for a longer file.</summary>
public record RoomUpNext(RoomEpisode Episode, long? EndsAtMs, string[] WaitingFor, string? CancelledBy);

/// <summary>A row of the mover's list: everyone present, ticked unless muted.</summary>
public record MoveCandidate(string Id, string Name, bool Muted, bool Ticked);

/// <summary>Someone moved the room and ticked this member.</summary>
public record RoomMoveOffer(string Id, string By, string[] Names, string Title, string Episode, long ExpiresAtMs);

public abstract record RoomConnection
{
    public sealed record Connected : RoomConnection;

    public sealed record Reconnecting(uint RelaysUp) : RoomConnection;
}

/// <summary>The room as it stands.</summary>
public record RoomView(
    string RoomId,
    string Name,
    RoomConnection Connection,
    RoomEpisode? Episode,
    string? Link,
    bool Playing,
    string? PausedBy,
    PausedFor[] Waiting,
    bool CanCarryOn,
    bool CanDontWait,
    RoomUpNext? UpNext,
    RelayHealth[] Relays,
    MoveCandidate[] MoveCandidates);

/// <summary>A member of the room. <c>SinceMs</c> is when the state began (epoch ms).</summary>
public record RoomMember(
    string Id,
    string Name,
    bool You,
    MemberState State,
    long SinceMs,
    float? BehindS,
    RoomEpisode? Episode,
    bool PausedRoom,
    bool Background);

/// <summary>The new message kinds the room's chat and line say.</summary>
public abstract record RoomMessageKind
{
    public sealed record AlreadyPaused(string By) : RoomMessageKind;

    public sealed record CarriedOn(string By, string[] Past) : RoomMessageKind;

    public sealed record SkippedOpening(string By) : RoomMessageKind;

    public sealed record NewInvite(string By) : RoomMessageKind;

    public sealed record UpNextCancelled(string By) : RoomMessageKind;
}

/// <summary>The room's commands the views send (section 6's table), nested as <c>Command.Room</c> will be.</summary>
public abstract record RoomCommand
{
    public sealed record CarryOn : RoomCommand;

    public sealed record DontWait : RoomCommand;

    public sealed record SkipOpening(double To) : RoomCommand;

    public sealed record CancelUpNext : RoomCommand;

    public sealed record PlayNextNow : RoomCommand;

    public sealed record ChoosingSource(bool Open) : RoomCommand;

    public sealed record ClosePlayer : RoomCommand;

    public sealed record NewInvite : RoomCommand;

    public sealed record MoveRoom(string[] Take) : RoomCommand;

    public sealed record AnswerMove(string Offer, bool Go) : RoomCommand;

    public sealed record SetRelays(string[] Relays) : RoomCommand;

    public sealed record Say(string Text) : RoomCommand;

    public sealed record Mute(string Name, bool On) : RoomCommand;

    /// <summary>Play for everyone, before play; mid-episode a play or pause is the player's own command.</summary>
    public sealed record Play : RoomCommand;

    /// <summary>Join from an invite (today's <c>PlayParty</c> with the invite), or Not now.</summary>
    public sealed record Join(bool Go) : RoomCommand;

    public sealed record Leave : RoomCommand;
}
