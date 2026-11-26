using Kaeri.Engine.Model;

namespace Kaeri.App;

/// <summary>
/// The party's conversation, in words: a reaction line, a tick's names, how long a line holds
/// on the video, and which lines are new. The conversation itself is the engine's, in memory for
/// the party only, and arrives whole after every change.
/// </summary>
public static class Chat
{
    /// <summary>The most lines on the video at once; the rest fold into "n more in chat".</summary>
    public const int Column = 3;

    /// <summary>Characters left at which the field's count shows.</summary>
    public const int CountFrom = 20;

    /// <summary>The four reactions, in the order the buttons show them.</summary>
    public static readonly Reaction[] All = [Reaction.Laugh, Reaction.Gasp, Reaction.Clap, Reaction.Cry];

    /// <summary>The button's name for a reaction: "Laugh".</summary>
    public static string Label(Reaction reaction) => reaction switch
    {
        Reaction.Laugh => T.RoomReactLaugh,
        Reaction.Gasp => T.RoomReactGasp,
        Reaction.Clap => T.RoomReactClap,
        _ => T.RoomReactCry,
    };

    /// <summary>Names with what they did, and when: "Ren and Mika laughed at 12:04".</summary>
    public static string Reacted(string names, Reaction reaction, string at) => reaction switch
    {
        Reaction.Laugh => T.RoomReactedLaugh(names, at),
        Reaction.Gasp => T.RoomReactedGasp(names, at),
        Reaction.Clap => T.RoomReactedClap(names, at),
        _ => T.RoomReactedCry(names, at),
    };

    /// <summary>A moment in the episode: <c>m:ss</c>, or <c>h:mm:ss</c> from the first hour.</summary>
    public static string Time(double at)
    {
        var total = (long)Math.Round(Math.Max(0, at), MidpointRounding.AwayFromZero);
        var (hours, minutes, seconds) = (total / 3600, total / 60 % 60, total % 60);
        return hours > 0 ? $"{hours}:{minutes:00}:{seconds:00}" : $"{minutes}:{seconds:00}";
    }

    /// <summary>Members named together: "Ren", "Ren and Mika", "Ren, Mika and Sol", "Ren, Mika and 2 others".</summary>
    public static string Names(string first, IReadOnlyList<string> with) => with.Count switch
    {
        0 => first,
        1 => T.RoomNamesTwo(first, with[0]),
        2 => T.RoomNamesThree(first, with[0], with[1]),
        _ => T.RoomNamesOthers(first, with[0], with.Count - 1),
    };

    /// <summary>Who a line is from, as it reads: "You" for this device's own.</summary>
    public static string Who(RoomChatLine line) => line.You ? T.RoomYou : line.From;

    /// <summary>A reaction line as the conversation says it: "Ren and Mika laughed at 12:04".</summary>
    public static string? ReactionLine(RoomChatLine line) =>
        line.Body is RoomChatBody.Reacted r ? Reacted(Names(Who(line), r.With), r.Reaction, Time(r.At)) : null;

    /// <summary>How long a line holds on the video: 4 s, and 1 s for every 10 words past 12, as notices do.</summary>
    public static TimeSpan Hold(string text)
    {
        var words = text.Split((char[]?)null, StringSplitOptions.RemoveEmptyEntries).Length;
        return TimeSpan.FromSeconds(4 + Math.Max(0, words - 12) / 10.0);
    }

    /// <summary>One tick on the seek bar: where it is and who reacted there.</summary>
    public sealed record Tick(double At, IReadOnlyList<string> Who)
    {
        /// <summary>The seek bar's label near it: "12:04 · Ren and Mika".</summary>
        public string Label() => T.RoomTickLabel(Time(At), Who.Count == 0 ? "" : Names(Who[0], Who.Skip(1).ToList()));
    }

    /// <summary>
    /// The reactions' marks as ticks on a line <paramref name="width"/> wide for an episode
    /// <paramref name="duration"/> long: marks within <paramref name="near"/> of the tick before
    /// them share it.
    /// </summary>
    public static IReadOnlyList<Tick> Ticks(IEnumerable<RoomReactionMark> marks, double duration, double width, double near)
    {
        var ticks = new List<(double At, List<string> Who)>();
        if (duration <= 0 || width <= 0)
            return [];
        foreach (var mark in marks.OrderBy(m => m.At))
        {
            if (ticks.Count > 0 && (mark.At - ticks[^1].At) / duration * width <= near)
            {
                if (!ticks[^1].Who.Contains(mark.From))
                    ticks[^1].Who.Add(mark.From);
            }
            else
                ticks.Add((mark.At, [mark.From]));
        }
        return ticks.Select(t => new Tick(t.At, t.Who)).ToList();
    }

    /// <summary>The typed, unmuted lines new since <paramref name="seen"/>, and the newest id.</summary>
    public static (IReadOnlyList<RoomChatLine> Fresh, ulong Newest) Fresh(IReadOnlyCollection<RoomChatLine> lines, ulong seen)
    {
        var newest = Math.Max(seen, lines.Count == 0 ? seen : lines.Max(l => l.Id));
        var fresh = lines.Where(l => l.Id > seen && !l.Muted && l.Body is RoomChatBody.Said).ToList();
        return (fresh, newest);
    }

    /// <summary>
    /// What the field's end says: a wait while Send waits, the characters left from
    /// <see cref="CountFrom"/>, or nothing.
    /// </summary>
    public static string? Tail(string text, uint maxLength, uint waitSeconds)
    {
        if (waitSeconds > 0)
            return T.RoomChatWait(waitSeconds);
        var left = Math.Max(0, (int)maxLength - text.Length);
        return left <= CountFrom && text.Trim().Length > 0 ? left.ToString(System.Globalization.CultureInfo.CurrentCulture) : null;
    }
}
