using Kaeri.Engine.Model;

namespace Kaeri.App;

/// <summary>
/// What the room's place (Watch together) says failed, and why, said when it failed: the link
/// Join tried, or the recent room a Rejoin tried. Never an older room's reason, never another
/// look's answer, and never the room this device was in ending.
/// </summary>
public sealed class RoomPlace(bool inRoom)
{
    bool wasInRoom = inRoom;

    /// <summary>The recent room a Rejoin tried, while its answer is awaited or after it failed.</summary>
    public string? Rejoining { get; private set; }

    /// <summary>Why the Rejoin of <see cref="Rejoining"/> failed.</summary>
    public (string Lead, string? Then)? RejoinFailed { get; private set; }

    /// <summary>The link that failed, as tried or as opened; the field takes it.</summary>
    public string? FailedLink { get; private set; }

    /// <summary>Why <see cref="FailedLink"/> failed.</summary>
    public (string Lead, string? Then) FailedWhy { get; private set; }

    /// <summary>Counts each failure of a link, so the field takes the link once per failure.</summary>
    public int Failures { get; private set; }

    /// <summary>Rejoin a recent room: its answer shows on its row.</summary>
    public void Rejoin(string roomId)
    {
        Rejoining = roomId;
        RejoinFailed = null;
    }

    /// <summary>A link tried in the field: its own try, so an earlier failure or Rejoin is done with.</summary>
    public void Try()
    {
        Rejoining = null;
        RejoinFailed = null;
        FailedLink = null;
    }

    /// <summary>A link that never reached the engine: not an invite, or an older version's.</summary>
    public void Refused(string link, string why)
    {
        FailedLink = link;
        FailedWhy = (why, null);
        Failures++;
    }

    /// <summary>
    /// The engine's batch, after <paramref name="known"/> took it. <paramref name="looking"/> is
    /// the invite looked into, <paramref name="typed"/> what the field holds. Returns whether the
    /// place should redraw, and whether a failed join's look-in goes.
    /// </summary>
    public (bool Touched, bool ForgetGlimpse) Take(IReadOnlyList<Event> batch, Known known, string? looking, string typed)
    {
        var inRoom = wasInRoom;
        var touched = false;
        var forget = false;
        foreach (var e in batch)
        {
            switch (e)
            {
                case Event.RoomJoined:
                    Rejoining = null;
                    RejoinFailed = null;
                    inRoom = true;
                    touched = true;
                    break;
                case Event.RoomLeft { Why: RoomLeftWhy.Left or RoomLeftWhy.Moved }:
                    inRoom = false;
                    touched = true;
                    break;
                // Only a join that failed counts against a recent room or a link. The room this
                // device was in ending leaves a look under way alone.
                case Event.RoomLeft left:
                    if (!inRoom && Rejoining is not null)
                        RejoinFailed = (T.RoomErrorNobodyAnsweredLead, T.RoomErrorNobodyAnsweredLine);
                    else if (!inRoom && (looking ?? typed.Trim()) is { Length: > 0 } link)
                    {
                        // The invite it was for keeps its link, and its look-in goes.
                        FailedLink = link;
                        FailedWhy = RoomWords.LeftLine(left.Why);
                        Failures++;
                        forget = looking is not null;
                    }
                    inRoom = false;
                    touched = true;
                    break;
                // A Rejoin of a room no longer kept here fails as a look into it.
                case Event.RoomLookFailed { Invite: null } f when Rejoining is not null && f.RoomId == Rejoining:
                    RejoinFailed = (f.Reason.Text(), null);
                    touched = true;
                    break;
                // The look waited on found nothing; another link's answer never counts.
                case Event.RoomLookFailed f when f.Invite is { } invite && invite == looking
                    && known.RoomLookFailure is { } look:
                    FailedLink = invite;
                    FailedWhy = RoomWords.LookLine(look);
                    Failures++;
                    touched = true;
                    break;
                case Event.RoomState or Event.RoomMembers or Event.RoomRecent or Event.RoomGlimpse or Event.RoomLookFailed:
                    touched = true;
                    break;
            }
        }
        wasInRoom = known.Room is not null;
        return (touched, forget);
    }
}
