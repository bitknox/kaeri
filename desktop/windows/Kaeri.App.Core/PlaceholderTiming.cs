using Kaeri.Engine.Model;

namespace Kaeri.App;

/// <summary>
/// When a place that loads shows its placeholders and how long they stay, after the spec's
/// States, Loading. Most places show nothing for the first 600 ms, then placeholders that stay at
/// least 0.3 s before the content replaces them. The title page keeps the room of what it has yet
/// to load from its first frame: its placeholders come at once, and the content replaces them the
/// moment it lands, with no hold. Times count from the start of the load.
/// </summary>
public sealed record PlaceholderTiming(TimeSpan After, TimeSpan Hold)
{
    /// <summary>Every screen but the title page: nothing for 600 ms, then placeholders held 0.3 s.</summary>
    public static readonly PlaceholderTiming Elsewhere = new(TimeSpan.FromMilliseconds(600), TimeSpan.FromMilliseconds(300));

    /// <summary>The title page: placeholders in the content's room from the first frame, replaced as it lands.</summary>
    public static readonly PlaceholderTiming TitlePage = new(TimeSpan.Zero, TimeSpan.Zero);

    /// <summary>Whether the placeholders come with the first frame, with no wait before them.</summary>
    public bool AtOnce => After <= TimeSpan.Zero;

    /// <summary>Whether the placeholders show once the load has run <paramref name="waited"/> with nothing yet.</summary>
    public bool Shown(TimeSpan waited) => waited >= After;

    /// <summary>
    /// How much longer placeholders that have stood <paramref name="shownFor"/> stay once the
    /// content is here. Zero: the content replaces them now.
    /// </summary>
    public TimeSpan Left(TimeSpan shownFor) => shownFor >= Hold ? TimeSpan.Zero : Hold - shownFor;

    /// <summary>
    /// What the place shows for content that lands <paramref name="landed"/> after the load
    /// started: whether placeholders stood before it, and when the content shows.
    /// </summary>
    public (bool Placeholders, TimeSpan ContentAt) Landing(TimeSpan landed)
    {
        if (!Shown(landed))
            return (false, landed);
        return (true, landed + Left(landed - After));
    }

    /// <summary>
    /// Whether the title page still waits for the parts only its own fetch brings: the facts, More
    /// about, Related and the people followed. A list's row and a partial page (a season's kept
    /// header) leave them waiting; the whole page, the demo, or a fetch that ended without the
    /// whole page (the list's row then stands) end the wait.
    /// </summary>
    public static bool TitleWaits(Entry? entry, bool fetchEnded) =>
        !fetchEnded && entry is not ({ IsDemo: true } or { Detail.Partial: false });
}
