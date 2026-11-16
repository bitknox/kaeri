using Kaeri.Engine.Model;

namespace Kaeri.App;

/// <summary>What stands in the room of the title page's availability line.</summary>
public enum AvailabilityRoom
{
    /// <summary>Where the show is listed is not known yet: a placeholder line keeps the room.</summary>
    Held,

    /// <summary>The answer has a line: Also on, Listed on, or Not found in your sources.</summary>
    Line,

    /// <summary>The answer has no line, or none came in time: the room closes.</summary>
    Closed,
}

/// <summary>
/// The title page's availability line under the actions. Until the sources say where the show is
/// listed, a placeholder line keeps its room from the page's first frame, and the answer replaces
/// it the moment it lands, with no hold. The engine says nothing when its fetch fails, as when
/// offline, so after <see cref="WaitFor"/> the page stops waiting and the room closes.
/// </summary>
public static class AvailabilityLine
{
    /// <summary>How long the room waits for the answer, from the page opening or another season picked there.</summary>
    public static readonly TimeSpan WaitFor = TimeSpan.FromSeconds(2);

    /// <summary>Whether an answer draws a line: services it is also on, or others it is listed on.</summary>
    public static bool Has(TitleAvailability? answer) => answer switch
    {
        { Case: AvailabilityCase.PlaysAlsoOn or AvailabilityCase.OnService, AlsoOn.Length: > 0 } => true,
        { Case: AvailabilityCase.NotInSources, Others.Length: > 0 } => true,
        _ => false,
    };

    /// <summary>
    /// Whether the episodes say no source plays the title: their first row is AniList's alone.
    /// Null rows, not yet known, say nothing.
    /// </summary>
    public static bool Unlisted(EpisodeRow[]? rows) => rows is [{ MetadataOnly: true }, ..];

    /// <summary>Whether an answer is for a title some source plays here.</summary>
    public static bool PlaysHere(TitleAvailability answer) =>
        answer.Case is AvailabilityCase.PlaysHere or AvailabilityCase.PlaysAlsoOn;

    /// <summary>
    /// The answer that counts: one for what the page knows of its sources, <paramref name="playsHere"/>
    /// (null while the episodes are not known), or after <see cref="WaitFor"/> whatever answer there
    /// is. An answer for what the page no longer holds true, kept from before, does not count.
    /// </summary>
    public static TitleAvailability? Counted(TitleAvailability? answer, bool? playsHere, TimeSpan waited) => answer switch
    {
        null => null,
        _ when waited >= WaitFor => answer,
        _ when playsHere is { } here && PlaysHere(answer) == here => answer,
        _ => null,
    };

    /// <summary>
    /// What the room shows <paramref name="waited"/> after the wait started, with the answer that
    /// counts (<see cref="Counted"/>), if any.
    /// </summary>
    public static AvailabilityRoom Room(TitleAvailability? counted, TimeSpan waited) => counted switch
    {
        null when waited < WaitFor => AvailabilityRoom.Held,
        null => AvailabilityRoom.Closed,
        _ when Has(counted) => AvailabilityRoom.Line,
        _ => AvailabilityRoom.Closed,
    };

    /// <summary>
    /// The placeholder's rows: two while the episodes say no source plays the title, as its answer
    /// is then the listed-on sentence with the Choose your services link under it, and one
    /// otherwise, for Also on.
    /// </summary>
    public static int HeldRows(EpisodeRow[]? rows) => Unlisted(rows) ? 2 : 1;
}
