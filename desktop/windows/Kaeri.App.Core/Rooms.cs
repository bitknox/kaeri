using Kaeri.Engine.Model;

namespace Kaeri.App;

/// <summary>Where the room is with its episode: before play, counting down to it, or started.</summary>
public abstract record RoomStart
{
    public sealed record Before : RoomStart;

    public sealed record Counting(long Seconds) : RoomStart;

    public sealed record Started : RoomStart;
}

/// <summary>
/// Everything the room's views read at one moment: the engine's room and members, the clock,
/// and what is still open to answer (an invite looked into, a move offer).
/// </summary>
public record RoomScene(
    RoomView View,
    RoomMember[] Members,
    long NowMs,
    RoomGlimpse? Invited = null,
    RoomMoveOffer? Offer = null)
{
    /// <summary>Before play, the count to it once everyone is ready, or started.</summary>
    public RoomStart Start => !View.BeforePlay ? new RoomStart.Started()
        : View.StartsAtMs is { } at ? new RoomStart.Counting(Math.Max(0, (at - NowMs + 999) / 1000))
        : new RoomStart.Before();

    /// <summary>The room's place now, in seconds: where it was, plus the time since while it plays.</summary>
    public double Position => (View.Position ?? 0) + (View.Playing && View.Position is not null ? Math.Max(0, NowMs - View.PositionAtMs) / 1000.0 : 0);

    /// <summary>The room's episode, as the engine names it ("9").</summary>
    public string Episode => View.RoomEpisode?.Episode ?? View.Episode?.Episode ?? "";

    /// <summary>The episode Up next plays.</summary>
    public string NextEpisode => View.UpNext?.Episode.Episode ?? View.NextEpisode?.Episode ?? "";
}

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
    public static long Seconds(long? sinceMs, long nowMs) => sinceMs is { } since ? Math.Max(0, (nowMs - since) / 1000) : 0;

    /// <summary>m:ss, or h:mm:ss past an hour.</summary>
    public static string Clock(double seconds)
    {
        var s = (long)Math.Max(0, Math.Floor(seconds));
        return s >= 3600 ? $"{s / 3600}:{s / 60 % 60:00}:{s % 60:00}" : $"{s / 60}:{s % 60:00}";
    }

    /// <summary>Names joined as the board says them: one, "A and B", or "3 people".</summary>
    public static string Names(IReadOnlyList<string> names) => names.Count switch
    {
        0 => "",
        1 => names[0],
        2 => RoomWords.Two(names[0], names[1]),
        _ => RoomWords.People(names.Count),
    };

    /// <summary>
    /// How the app answers the room starting, moving on, or this device coming in: it plays the
    /// room's episode from the start, never this device's own resume point, and the room moves the
    /// player to its place (the event's <c>At</c>) from there.
    /// </summary>
    public static Command Open(Event.RoomStarted started) => new Command.Play(started.Id, started.Episode, FromStart: true);

    /// <summary>An invite looked into, as a room not yet joined: its episode and who is there.</summary>
    public static RoomScene Invited(RoomGlimpse glimpse, long nowMs) =>
        new(new RoomView(glimpse.RoomId ?? "", "", new RoomConnection.Joining(), RoomEpisode: glimpse.RoomEpisode, Playing: glimpse.Playing),
            glimpse.Members ?? [], nowMs, Invited: glimpse);

    static RoomMember? Me(RoomScene s) => s.Members.FirstOrDefault(m => m.You);

    /// <summary>The room's line, highest first as the board orders it.</summary>
    public static RoomLine Line(RoomScene s)
    {
        var v = s.View;
        var me = Me(s);
        var at = Clock(s.Position);
        if (s.Offer is { } offer)
        {
            var others = (offer.Names ?? []).Where(n => n != me?.Name).ToList();
            return new RoomLine(RoomWords.MovedTheRoom(offer.By), RoomWords.GoAlongAsk(Names(others), others.Count, at),
                RoomAsk.GoAlong, RoomWords.GoAlong, RoomWords.StayHere) { Kind = "offer" };
        }
        if (s.Invited is { } inv)
        {
            var people = (inv.Members ?? []).Length;
            var by = inv.By ?? inv.Members?.FirstOrDefault()?.Name ?? "";
            var detail = inv.RoomEpisode is { } e ? RoomWords.InvitedTo(e.Title, e.Episode) : "";
            return new RoomLine(RoomWords.Invited(by), inv.Full ? RoomWords.Over(people) : detail,
                inv.Full ? RoomAsk.JoinAnyway : RoomAsk.Join, inv.Full ? RoomWords.JoinAnyway : RoomWords.JoinRoom, RoomWords.NotNow) { Kind = "invited" };
        }
        if (v.Connection is RoomConnection.Reconnecting)
            return new RoomLine(RoomWords.ReconnectingRoom, RoomWords.NoRelayAnswers) { Kind = "self-reconnecting" };
        if (me is { State: MemberState.Behind, BehindS: { } behind })
            return new RoomLine(RoomWords.Watching, RoomWords.YouBehind((long)Math.Round(behind))) { Kind = "self-behind" };

        var waits = v.Waiting ?? [];
        var waitSecs = waits.Length == 0 ? 0 : Seconds(waits[0].SinceMs, s.NowMs);
        if (waits.Any(w => w.You))
        {
            // Those waiting: not this device, not gone, and not waited for themselves (their rows say so).
            var waitedFor = waits.Select(w => w.Id).ToHashSet();
            var waiting = s.Members.Where(m => !m.You && m.State != MemberState.Left && !waitedFor.Contains(m.Id)).Select(m => m.Name).ToList();
            return new RoomLine(RoomWords.PausedForYou, RoomWords.Waiting(Names(waiting), waiting.Count, waitSecs)) { Kind = "self-wait" };
        }
        var names = waits.Select(w => w.Name).Distinct().ToList();
        var who = Names(names);

        if (s.Start is not RoomStart.Started)
        {
            if (s.Start is RoomStart.Counting { Seconds: var n })
                return new RoomLine(RoomWords.StartingIn(n), RoomWords.EveryoneReady) { Kind = "counting" };
            if (names.Count == 0)
                return new RoomLine(RoomWords.NotStarted, RoomWords.PlayStartsIt) { Kind = "before" };
            if (v.CanCarryOn)
                return new RoomLine(Still(waits, who, names.Count), RoomWords.WaitedCap, RoomAsk.Start,
                    names.Count > 1 ? RoomWords.StartWithoutThem : RoomWords.StartWithout(who)) { Kind = "before-cap" };
            return new RoomLine(RoomWords.StartingWhen(who, names.Count), RoomWords.Loading(waitSecs)) { Kind = "before-wait" };
        }
        if (v.UpNext is { EndsAtMs: { } ends, CancelledBy: null } && names.Count == 0)
            return new RoomLine(RoomWords.UpNextIn(Math.Max(0, (ends - s.NowMs + 999) / 1000)), RoomWords.PlaysForEveryone(s.NextEpisode)) { Kind = "up-next" };
        if (names.Count > 0 && v.CanCarryOn)
            return new RoomLine(Still(waits, who, names.Count), RoomWords.WaitedCap, RoomAsk.CarryOn,
                names.Count > 1 ? RoomWords.CarryOnWithoutThem : RoomWords.CarryOnWithout(who)) { Kind = "cap" };
        if (names.Count == 0 && v.CarriedOn is { } carried && (carried.Past ?? []).Length > 0)
        {
            var past = carried.Past!;
            var by = carried.ByYou ? RoomWords.YouAlone : carried.By;
            return new RoomLine(RoomWords.Watching, RoomWords.CarriedOn(by, Names(past), past.Length)) { Kind = "carried" };
        }
        if (names.Count > 0)
        {
            var secs = waits.Length != 1 ? RoomWords.Loading(waitSecs) : waits[0].Reason switch
            {
                WaitReason.Buffering => RoomWords.Buffering(waitSecs),
                WaitReason.Seeking => RoomWords.Seeking(waitSecs),
                _ => RoomWords.Loading(waitSecs),
            };
            return new RoomLine(RoomWords.PausedFor(who), secs) { Kind = "wait" };
        }
        if (!v.Playing)
        {
            var title = v.PausedByYou ? RoomWords.YouPaused : v.PausedBy is { } p ? RoomWords.Paused(p) : RoomWords.StatePaused;
            return new RoomLine(title, RoomWords.PausedAt(at)) { Kind = "paused" };
        }
        if (v.Moved is { } moved)
        {
            var went = moved.Went ?? [];
            return new RoomLine(RoomWords.MovedAway(moved.By), went.Length > 0 ? RoomWords.WentAlong(Names(went)) : RoomWords.WatchingAt(s.Episode, at)) { Kind = "moved" };
        }
        var relays = v.Relays ?? [];
        var down = relays.Count(r => !r.Healthy);
        if (down > 0 && down < relays.Length)
            return new RoomLine(RoomWords.Watching, RoomWords.RelaysDown(down, relays.Length)) { Kind = "relay-trouble" };
        return new RoomLine(RoomWords.Watching, RoomWords.WatchingAt(s.Episode, at)) { Kind = "watching" };
    }

    /// <summary>Whether a member counts as with the room: watching, or ready before play.</summary>
    public static bool IsWatching(RoomMember m) => m.State is MemberState.Watching or MemberState.Ready;

    /// <summary>A member's word, as the row and the strip's label say it.</summary>
    public static string Word(RoomScene s, RoomMember m)
    {
        var secs = Seconds(m.SinceMs, s.NowMs);
        var before = s.Start is not RoomStart.Started;
        return m.State switch
        {
            MemberState.Ready => RoomWords.StateReady,
            MemberState.Watching => before ? RoomWords.StateReady : RoomWords.StateWatching,
            MemberState.Paused when before => RoomWords.StateReady,
            MemberState.Paused => m.PausedRoom ? RoomWords.StatePausedIt : RoomWords.StatePaused,
            MemberState.Loading or MemberState.Buffering when Seeks(s, m) => RoomWords.StateSeeking(secs),
            MemberState.Loading => RoomWords.StateLoading(secs),
            MemberState.Buffering => RoomWords.StateBuffering(secs),
            MemberState.Behind => RoomWords.StateBehind((long)Math.Round(m.BehindS ?? 0)),
            MemberState.OtherEpisode => RoomWords.StateOther(m.Episode?.Episode),
            MemberState.Choosing => RoomWords.StateChoosing,
            MemberState.NoSource => RoomWords.StateNoSource(s.Episode),
            MemberState.PlayerClosed => RoomWords.StateClosed,
            MemberState.Reconnecting => RoomWords.StateReconnecting,
            MemberState.Left => RoomWords.StateLeft,
            _ => "",
        };
    }

    /// <summary>Whether the room waits for this member to catch up after a seek or Skip opening.</summary>
    static bool Seeks(RoomScene s, RoomMember m) =>
        (s.View.Waiting ?? []).Any(w => w.Id == m.Id && w.Reason == WaitReason.Seeking);

    /// <summary>A member's face: solid with the room, an arc while waited for, no ring and dimmed when not with it.</summary>
    public static RoomFaceLook Face(RoomScene s, RoomMember m)
    {
        var secs = Seconds(m.SinceMs, s.NowMs);
        var before = s.Start is not RoomStart.Started;
        return m.State switch
        {
            MemberState.Watching or MemberState.Ready or MemberState.Behind => new(RingKind.Water, 1, 1, false, false),
            MemberState.Paused when before => new(RingKind.Water, 1, 1, false, false),
            MemberState.Paused => new(RingKind.Haze, 1, 1, m.PausedRoom, false),
            MemberState.Loading or MemberState.Buffering when m.Waited || before =>
                new(RingKind.Progress, Math.Clamp(secs / CapSeconds, 0.04, 1), 1, false, false),
            MemberState.NoSource => new(RingKind.None, 0, 0.5, false, true),
            MemberState.Left => new(RingKind.None, 0, 0.35, false, false),
            _ => new(RingKind.None, 0, 0.5, false, false),
        };
    }

    /// <summary>The rows, this device's first, then join order; muted people say so.</summary>
    public static IReadOnlyList<RoomRow> Rows(RoomScene s)
    {
        var rows = new List<RoomRow>();
        foreach (var m in s.Members.OrderByDescending(m => m.You))
        {
            var word = Word(s, m);
            var muted = m.Muted && !m.You;
            if (muted)
                word = RoomWords.Muted(word);
            var name = m.You ? $"{m.Name} {RoomWords.YouMark}" : m.Name;
            rows.Add(new RoomRow(m.Id, m.Name, word, Face(s, m), m.You, IsWatching(m), muted, m.State == MemberState.Left, RoomWords.Spoken(name, word)));
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
        s.Start is not RoomStart.Started
        || s.Members.Any(m => m.State is not (MemberState.Watching or MemberState.Left))
        || (s.View.Waiting ?? []).Length > 0
        || s.NowMs - lastChangeMs < (long)StripAfterChange.TotalMilliseconds;

    /// <summary>The cap's line by why the room waits: seeking or buffering when everyone it waits for is, loading otherwise.</summary>
    static string Still(PausedFor[] waits, string who, int many) =>
        waits.All(w => w.Reason == WaitReason.Seeking) ? RoomWords.StillSeeking(who, many)
        : waits.All(w => w.Reason == WaitReason.Buffering) ? RoomWords.StillBuffering(who, many)
        : RoomWords.StillLoading(who, many);

    /// <summary>The strip's words: the line's first, with the counting seconds while the room waits.</summary>
    public static string StripWords(RoomScene s)
    {
        var line = Line(s);
        var waits = s.View.Waiting ?? [];
        if (line.Kind is "wait" or "before-wait" && waits.Length > 0)
            return RoomWords.WithSeconds(line.Title, Seconds(waits[0].SinceMs, s.NowMs));
        return line.Title;
    }

    /// <summary>What the room shows, as one string: a change keeps the strip up.</summary>
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

    /// <summary>What the video area shows for this device, from its own row and the room.</summary>
    public static RoomPanelKind PanelFor(RoomScene s)
    {
        if (s.Invited is not null)
            return RoomPanelKind.Invited;
        var me = Me(s);
        return me?.State switch
        {
            MemberState.Choosing => RoomPanelKind.Choosing,
            MemberState.NoSource => RoomPanelKind.NoSource,
            MemberState.Loading when s.View.CanDontWait => RoomPanelKind.SelfLoading,
            _ when s.View.BeforePlay => RoomPanelKind.Before,
            _ => RoomPanelKind.None,
        };
    }
}

/// <summary>What the video area shows when the player has nothing to show, or before play.</summary>
public enum RoomPanelKind
{
    None,
    /// <summary>Before play: the episode's banner, and Play for everyone until it is pressed.</summary>
    Before,
    /// <summary>Opened from an invite: the room is watching behind the banner.</summary>
    Invited,
    /// <summary>Choosing a source for the room's episode, in the app's own list.</summary>
    Choosing,
    /// <summary>None of your sources has the room's episode.</summary>
    NoSource,
    /// <summary>Your own episode opening while the room waits for it, with Don't wait for me.</summary>
    SelfLoading,
}

/// <summary>The video area's panel: its kind and what it names.</summary>
public record RoomPanel(RoomPanelKind Kind, string Show, string Episode, string? EpisodeName = null, long Minutes = 24,
    int SourceCount = 0, bool Pressed = false);
