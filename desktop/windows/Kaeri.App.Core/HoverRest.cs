namespace Kaeri.App;

/// <summary>What the pointer resting on a cover asks for now.</summary>
public enum HoverRestStep
{
    None,
    /// <summary>Cue the title's trailer, unseen.</summary>
    Cue,
    /// <summary>Open the cover's card.</summary>
    Open,
}

/// <summary>
/// When the cover under the pointer cues its trailer and opens its card, as the Mac's hover
/// intent decides. The wait counts from the pointer arriving on the cover, and moves inside the
/// cover do not restart it; the trailer is cued at 150 ms and the card opens at the dwell, each
/// only while the pointer has slowed (under 12 px of travel in the last 100 ms). A scroll starts
/// the wait again from the moment the page stops, so the cover that comes to rest under a still
/// pointer opens like one pointed at. A key press holds everything until the pointer moves.
/// Times are milliseconds on any one clock.
/// </summary>
public sealed class HoverRest
{
    public static readonly TimeSpan Dwell = TimeSpan.FromMilliseconds(450);
    /// <summary>The dwell for the next cover when a card closed less than <see cref="Scanning"/> ago.</summary>
    public static readonly TimeSpan NextDwell = TimeSpan.FromMilliseconds(150);
    public static readonly TimeSpan Scanning = TimeSpan.FromSeconds(1);
    /// <summary>The pointer has slowed when it travelled less than this in the last <see cref="Window"/>.</summary>
    public const double SlowTravel = 12;
    const long Window = 100;

    readonly List<(long At, double X, double Y)> trail = [];
    bool on, cued, opened, held;
    long from;
    TimeSpan dwell = Dwell;

    /// <summary>When the wait began: the pointer arriving, or the page stopping after a scroll.</summary>
    public long From => from;

    /// <summary>Whether the pointer is on a cover whose card has not opened yet.</summary>
    public bool Waiting => on && !opened;

    /// <summary>The dwell for a card that closed at <paramref name="closedAt"/>, at <paramref name="now"/>.</summary>
    public static TimeSpan DwellAfter(long closedAt, long now) =>
        now - closedAt < Scanning.TotalMilliseconds ? NextDwell : Dwell;

    /// <summary>The pointer is on a new cover at (<paramref name="x"/>, <paramref name="y"/>); its wait starts at <paramref name="start"/>, now unless given.</summary>
    public void Enter(double x, double y, long now, TimeSpan dwell, long? start = null)
    {
        on = true;
        cued = false;
        opened = false;
        from = start ?? now;
        this.dwell = dwell;
        trail.Clear();
        trail.Add((now, x, y));
    }

    /// <summary>The pointer moved on the cover.</summary>
    public void Move(double x, double y, long now)
    {
        if (trail.Count > 0)
        {
            var last = trail[^1];
            if (Math.Abs(x - last.X) >= 1 || Math.Abs(y - last.Y) >= 1)
                held = false;
        }
        trail.Add((now, x, y));
        Prune(now);
    }

    /// <summary>The pointer left the cover.</summary>
    public void Leave()
    {
        on = false;
        trail.Clear();
    }

    /// <summary>A key press or a click: nothing opens until the pointer moves.</summary>
    public void Hold() => held = true;

    /// <summary>The page scrolled at <paramref name="now"/>: any card has closed, and the wait starts again from here.</summary>
    public void Scrolled(long now)
    {
        from = now;
        cued = false;
        opened = false;
        held = false;
        dwell = Dwell;
    }

    /// <summary>Where things stand at <paramref name="now"/>.</summary>
    public HoverRestStep Step(long now)
    {
        if (!on || opened || held || !Slowed(now))
            return HoverRestStep.None;
        var waited = now - from;
        if (waited >= dwell.TotalMilliseconds)
        {
            opened = true;
            return HoverRestStep.Open;
        }
        if (!cued && HoverCue.After(dwell) is { } cue && waited >= cue.TotalMilliseconds)
        {
            cued = true;
            return HoverRestStep.Cue;
        }
        return HoverRestStep.None;
    }

    bool Slowed(long now)
    {
        Prune(now);
        double travel = 0;
        for (var i = 1; i < trail.Count; i++)
            travel += Math.Sqrt(Math.Pow(trail[i].X - trail[i - 1].X, 2) + Math.Pow(trail[i].Y - trail[i - 1].Y, 2));
        return travel < SlowTravel;
    }

    void Prune(long now)
    {
        while (trail.Count > 1 && trail[0].At < now - Window)
            trail.RemoveAt(0);
        // A lone sample older than the window is a pointer at rest.
        if (trail.Count == 1 && trail[0].At < now - Window)
            trail[0] = (now, trail[0].X, trail[0].Y);
    }
}
