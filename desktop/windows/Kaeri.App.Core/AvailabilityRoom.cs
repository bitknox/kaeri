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
    /// What the room shows <paramref name="waited"/> after the wait started, with the answer the
    /// page has for the title, if any. An answer on screen stays while the title is asked again.
    /// </summary>
    public static AvailabilityRoom Room(TitleAvailability? answer, TimeSpan waited) => answer switch
    {
        null when waited < WaitFor => AvailabilityRoom.Held,
        null => AvailabilityRoom.Closed,
        _ when Has(answer) => AvailabilityRoom.Line,
        _ => AvailabilityRoom.Closed,
    };
}
