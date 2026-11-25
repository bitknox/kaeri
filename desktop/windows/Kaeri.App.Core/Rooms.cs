using Kaeri.App.RoomStand;

namespace Kaeri.App;

/// <summary>Where the room is with its episode: before play, counting down to it, or started.</summary>
public abstract record RoomStart
{
    public sealed record Before : RoomStart;

    public sealed record Counting(long Seconds) : RoomStart;

    public sealed record Started : RoomStart;
}

/// <summary>The room opened from an invite, before joining: who invited, and how many are in.</summary>
public record RoomInvited(string By, string Show, string Episode, int People);

/// <summary>
/// Everything the room's views read at one moment: the engine's room and members, the clock,
/// and what the app keeps itself (the start, an invite not yet answered, a move offer, who
/// carried on last, the room's place and its words for the episode).
/// </summary>
public record RoomScene(
    RoomView View,
    RoomMember[] Members,
    long NowMs,
    RoomStart Start,
    double Position,
    string EpisodeName,
    string? NextEpisodeName = null,
    RoomInvited? Invited = null,
    RoomMoveOffer? Offer = null,
    string? CarriedBy = null);

/// <summary>What the room's line asks of the viewer, if anything.</summary>
public enum RoomAsk
{
    None,
    CarryOn,
    Start,
    Join,
    JoinAnyway,
    GoAlong,
}

/// <summary>The room's line: two lines of words, and the button under them.</summary>
public record RoomLine(string Title, string Detail, RoomAsk Ask = RoomAsk.None, string? AskText = null, string? SecondText = null)
{
    /// <summary>What the line is about, without its counting seconds: a change of kind crossfades, a new second changes in place.</summary>
    public string Kind { get; init; } = "";
}

/// <summary>How a face's ring reads.</summary>
public enum RingKind
{
    /// <summary>Solid water: with the room.</summary>
    Water,
    /// <summary>Solid haze: paused.</summary>
    Haze,
    /// <summary>An arc filling toward the 20 s cap: the room waits for them.</summary>
    Progress,
    /// <summary>No ring, the face dimmed: not with the room.</summary>
    None,
}

/// <summary>A face as drawn: its ring, how full the arc is, how dim the initial, the badges.</summary>
public record RoomFaceLook(RingKind Ring, double Fraction, double Dim, bool PauseBadge, bool ErrorDot);

/// <summary>A person's row: the name, the word under it, the face.</summary>
public record RoomRow(string Id, string Name, string Word, RoomFaceLook Face, bool You, bool Watching, bool Muted, bool Left, string Spoken);

/// <summary>
/// The room as words and looks, from a <see cref="RoomScene"/>: the room's line, each person's
/// word and face, the strip's faces, and the move list. Built to board v5 (the room, 25 November
/// 2026); the engine decides every state, this only says it.
/// </summary>
public static class Rooms
{
    /// <summary>Carry on comes forward at 20 s; the arc fills toward it.</summary>
    public const double CapSeconds = 20;

    /// <summary>The full screen strip stays this long after any change.</summary>
    public static readonly TimeSpan StripAfterChange = TimeSpan.FromSeconds(8);

    /// <summary>A left row stays in place, dimmed, this long before the rows below close the gap.</summary>
    public static readonly TimeSpan LeftRowHold = TimeSpan.FromSeconds(4);

    /// <summary>Whole seconds since an epoch millisecond, floored, never below zero.</summary>
    public static long Seconds(long sinceMs, long nowMs) => Math.Max(0, (nowMs - sinceMs) / 1000);

    /// <summary>m:ss.</summary>
    public static string Clock(double seconds)
    {
        var s = (long)Math.Max(0, Math.Round(seconds));
        return $"{s / 60}:{s % 60:00}";
    }

    /// <summary>Names joined as the board says them: one, "A and B", or "3 people".</summary>
    public static string Names(IReadOnlyList<string> names) => names.Count switch
    {
        0 => "",
        1 => names[0],
        2 => RoomWords.Two(names[0], names[1]),
        _ => RoomWords.People(names.Count),
    };

    static RoomMember? Me(RoomScene s) => s.Members.FirstOrDefault(m => m.You);

    /// <summary>The room's line, highest first as the board orders it.</summary>
    public static RoomLine Line(RoomScene s)
    {
        var v = s.View;
        var me = Me(s);
        var at = Clock(s.Position);
        if (s.Offer is { } offer)
        {
            var others = offer.Names.Where(n => n != me?.Name).ToList();
            return new RoomLine(RoomWords.MovedTheRoom(offer.By), RoomWords.GoAlongAsk(Names(others), others.Count, at),
                RoomAsk.GoAlong, RoomWords.GoAlong, RoomWords.StayHere) { Kind = "offer" };
        }
        if (s.Invited is { } inv)
        {
            var over = inv.People >= 10;
            return new RoomLine(RoomWords.Invited(inv.By), over ? RoomWords.Over(inv.People) : $"{inv.Show} · {inv.Episode}",
                over ? RoomAsk.JoinAnyway : RoomAsk.Join, over ? RoomWords.JoinAnyway : RoomWords.JoinRoom, RoomWords.NotNow) { Kind = "invited" };
        }
        if (v.Connection is RoomConnection.Reconnecting)
            return new RoomLine(RoomWords.ReconnectingRoom, RoomWords.NoRelayAnswers) { Kind = "self-reconnecting" };
        if (me is { State: MemberState.Behind, BehindS: { } behind })
            return new RoomLine(RoomWords.Watching, RoomWords.YouBehind((long)Math.Round(behind))) { Kind = "self-behind" };

        var waits = v.Waiting;
        var waitSecs = waits.Length == 0 ? 0 : Seconds(waits.Min(w => w.SinceMs), s.NowMs);
        if (me is not null && waits.Any(w => w.Name == me.Name))
        {
            var waiting = s.Members.Where(m => !m.You && m.State != MemberState.Left).Select(m => m.Name).ToList();
            return new RoomLine(RoomWords.PausedForYou, RoomWords.Waiting(Names(waiting), waiting.Count, waitSecs)) { Kind = "self-wait" };
        }
        var names = waits.Select(w => w.Name).Distinct().ToList();
        var who = Names(names);
        var them = names.Count > 1 ? RoomWords.Them : who;

        if (s.Start is not RoomStart.Started)
        {
            if (s.Start is RoomStart.Counting { Seconds: var n })
                return new RoomLine(RoomWords.StartingIn(n), RoomWords.EveryoneReady) { Kind = "counting" };
            if (names.Count == 0)
                return new RoomLine(RoomWords.NotStarted, RoomWords.PlayStartsIt) { Kind = "before" };
            if (v.CanCarryOn)
                return new RoomLine(RoomWords.StillLoading(who, names.Count), RoomWords.WaitedCap, RoomAsk.Start, RoomWords.StartWithout(them)) { Kind = "before-cap" };
            return new RoomLine(RoomWords.StartingWhen(who, names.Count), RoomWords.Why(RoomWords.WhyLoading, waitSecs)) { Kind = "before-wait" };
        }
        if (v.UpNext is { EndsAtMs: { } ends, CancelledBy: null } && names.Count == 0)
            return new RoomLine(RoomWords.UpNextIn(Math.Max(0, (ends - s.NowMs + 999) / 1000)), RoomWords.PlaysForEveryone(s.NextEpisodeName ?? "")) { Kind = "up-next" };
        if (names.Count > 0 && v.CanCarryOn)
            return new RoomLine(RoomWords.StillLoading(who, names.Count), RoomWords.WaitedCap, RoomAsk.CarryOn, RoomWords.CarryOnWithout(them)) { Kind = "cap" };
        if (names.Count == 0 && s.CarriedBy is { } by)
        {
            var still = s.Members.Where(m => m.State == MemberState.Loading).Select(m => m.Name).ToList();
            if (still.Count > 0)
                return new RoomLine(RoomWords.Watching, RoomWords.CarriedOn(by, Names(still), still.Count)) { Kind = "carried" };
        }
        if (names.Count > 0)
        {
            var why = waits.Length == 1 && waits[0].Reason == PauseReason.Buffering ? RoomWords.WhyBuffering : RoomWords.WhyLoading;
            return new RoomLine(RoomWords.PausedFor(who), RoomWords.Why(why, waitSecs)) { Kind = "wait" };
        }
        if (!v.Playing)
        {
            var pauser = v.PausedBy is { } p && p == me?.Name ? RoomWords.YouPaused : RoomWords.Paused(v.PausedBy ?? "");
            return new RoomLine(v.PausedBy is null ? RoomWords.StatePaused : pauser, RoomWords.PausedAt(at)) { Kind = "paused" };
        }
        var down = v.Relays.Count(r => !r.Healthy);
        if (down > 0 && down < v.Relays.Length)
            return new RoomLine(RoomWords.Watching, RoomWords.RelaysDown(down, v.Relays.Length)) { Kind = "relay-trouble" };
        return new RoomLine(RoomWords.Watching, RoomWords.WatchingAt(s.EpisodeName, at)) { Kind = "watching" };
    }

    /// <summary>Whether a member counts as with the room: watching, or ready before play.</summary>
    public static bool IsWatching(RoomMember m) => m.State is MemberState.Watching;

    /// <summary>A member's word, as the row and the strip's label say it.</summary>
    public static string Word(RoomScene s, RoomMember m)
    {
        var secs = Seconds(m.SinceMs, s.NowMs);
        var before = s.Start is not RoomStart.Started;
        return m.State switch
        {
            MemberState.Watching => before ? RoomWords.StateReady : RoomWords.StateWatching,
            MemberState.Paused when before => RoomWords.StateReady,
            MemberState.Paused => m.PausedRoom ? RoomWords.StatePausedIt : RoomWords.StatePaused,
            MemberState.Loading => RoomWords.StateLoading(secs),
            MemberState.Buffering => RoomWords.StateBuffering(secs),
            MemberState.Behind => RoomWords.StateBehind((long)Math.Round(m.BehindS ?? 0)),
            MemberState.OtherEpisode => m.Episode is { } e ? RoomWords.StateOther(e.Episode) : RoomWords.StateOtherUnknown,
            MemberState.Choosing => RoomWords.StateChoosing,
            MemberState.NoSource => RoomWords.StateNoSource(s.View.Episode?.Episode ?? ""),
            MemberState.PlayerClosed => RoomWords.StateClosed,
            MemberState.Reconnecting => RoomWords.StateReconnecting,
            MemberState.Left => RoomWords.StateLeft,
            _ => "",
        };
    }

    /// <summary>A member's face: solid with the room, an arc while waited for, no ring and dimmed when not with it.</summary>
    public static RoomFaceLook Face(RoomScene s, RoomMember m)
    {
        var secs = Seconds(m.SinceMs, s.NowMs);
        var before = s.Start is not RoomStart.Started;
        return m.State switch
        {
            MemberState.Watching or MemberState.Behind => new(RingKind.Water, 1, 1, false, false),
            MemberState.Paused when before => new(RingKind.Water, 1, 1, false, false),
            MemberState.Paused => new(RingKind.Haze, 1, 1, m.PausedRoom, false),
            MemberState.Loading or MemberState.Buffering => new(RingKind.Progress, Math.Clamp(secs / CapSeconds, 0.04, 1), 1, false, false),
            MemberState.NoSource => new(RingKind.None, 0, 0.5, false, true),
            MemberState.Left => new(RingKind.None, 0, 0.35, false, false),
            _ => new(RingKind.None, 0, 0.5, false, false),
        };
    }

    /// <summary>The rows, this device's first, then join order; muted people say so.</summary>
    public static IReadOnlyList<RoomRow> Rows(RoomScene s, IReadOnlyCollection<string>? muted = null)
    {
        var rows = new List<RoomRow>();
        foreach (var m in s.Members.OrderByDescending(m => m.You))
        {
            var word = Word(s, m);
            var isMuted = muted is not null && muted.Contains(m.Name) && !m.You;
            if (isMuted)
                word = RoomWords.Muted(word);
            var name = m.You ? RoomWords.You(m.Name) : m.Name;
            rows.Add(new RoomRow(m.Id, m.Name, word, Face(s, m), m.You, IsWatching(m), isMuted, m.State == MemberState.Left, $"{name}, {word}"));
        }
        return rows;
    }

    /// <summary>Up to four faces in order; past four, three and a "+N" that stands for the rest.</summary>
    public static (IReadOnlyList<RoomRow> Shown, int More, int MoreNotWatching) Capped(IReadOnlyList<RoomRow> rows)
    {
        if (rows.Count <= 4)
            return (rows, 0, 0);
        var rest = rows.Skip(3).ToList();
        return (rows.Take(3).ToList(), rest.Count, rest.Count(r => !r.Watching));
    }

    /// <summary>
    /// Whether the full screen strip shows: while anyone isn't watching, and for 8 s after any
    /// change. Before play nobody is watching yet, so it shows.
    /// </summary>
    public static bool StripShows(RoomScene s, long lastChangeMs) =>
        s.Members.Any(m => m.State is not (MemberState.Watching or MemberState.Left))
        || s.View.Waiting.Length > 0
        || s.NowMs - lastChangeMs < (long)StripAfterChange.TotalMilliseconds;

    /// <summary>The strip's words: the line's first, with the counting seconds while the room waits.</summary>
    public static string StripWords(RoomScene s)
    {
        var line = Line(s);
        if (line.Kind is "wait" or "before-wait" && s.View.Waiting.Length > 0)
            return $"{line.Title} · {Seconds(s.View.Waiting.Min(w => w.SinceMs), s.NowMs)} s";
        return line.Title;
    }

    /// <summary>A member's state for the line that keeps the strip up: the fingerprint of what the room shows.</summary>
    public static string Fingerprint(RoomScene s) =>
        string.Join("|", s.Members.Select(m => $"{m.Id}:{m.State}")) + "#" + Line(s).Kind;

    /// <summary>The move list's button, counting the others ticked.</summary>
    public static string MoveButton(int othersTicked) => RoomWords.MoveWith(othersTicked);

    /// <summary>A face's initial: the name's first character as a person reads it, upper case.</summary>
    public static string Initial(string name)
    {
        var trimmed = name.Trim();
        return trimmed.Length == 0 ? "?" : System.Globalization.StringInfo.GetNextTextElement(trimmed).ToUpperInvariant();
    }
}

/// <summary>A line of the room's chat: what the room did, what someone said, or a reaction.</summary>
public abstract record RoomChatLine
{
    public sealed record Note(string Text) : RoomChatLine;

    public sealed record Said(string From, string Text, bool You) : RoomChatLine;

    public sealed record Reacted(string From, Kaeri.Engine.Model.Reaction Reaction, string At) : RoomChatLine;
}

/// <summary>What the video area shows when the player has nothing to show, or before play.</summary>
public enum RoomPanelKind
{
    None,
    /// <summary>Before play: the episode's banner, and Play for everyone until it is pressed.</summary>
    Before,
    /// <summary>Opened from an invite: the room is watching behind the banner.</summary>
    Invited,
    /// <summary>Your sources for the room's episode, to play in the room.</summary>
    Choosing,
    /// <summary>None of your sources has the room's episode.</summary>
    NoSource,
    /// <summary>Your own episode opening while the room waits for it, with Don't wait for me.</summary>
    SelfLoading,
}

/// <summary>A source in the Choosing panel.</summary>
public record RoomSourceChoice(string Name, string Detail, bool Picked);

/// <summary>The video area's panel: its kind and what it names.</summary>
public record RoomPanel(RoomPanelKind Kind, string Show, string EpisodeLine, string Episode, long Minutes = 24,
    RoomSourceChoice[]? Sources = null, int SourceCount = 0, bool Pressed = false);
