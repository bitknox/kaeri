using Kaeri.Engine.Model;

namespace Kaeri.App;

/// <summary>
/// The line under a title or series page's main action while what plays or is read there is
/// private: the head in the text colour, then the rest after " · ". "Private · Not saved or
/// synced", "Private · Your place is kept until Kaeri quits", "Always private · Not saved or synced".
/// </summary>
public sealed record PrivateLine(string Head, string Tail)
{
    /// <summary>What a screen reader hears of it: the two halves as one sentence.</summary>
    public string Spoken => $"{Head}, {Tail}";
}

/// <summary>
/// Private watching and reading as the engine last said it: whether the mode is on (never saved,
/// so every launch starts with it off), the shows and series kept always private on this device,
/// and which titles have a place held in memory. The engine holds every rule; this only keeps
/// what the window, the title and series pages and the media controls draw from.
/// </summary>
public sealed class Privately
{
    readonly HashSet<uint> alwaysShows = [];
    readonly HashSet<string> alwaysSeries = [];
    readonly Dictionary<uint, bool> keptShows = [];

    /// <summary>Process-wide: private mode is on now, for the app's own log lines.</summary>
    public static bool Now { get; private set; }

    /// <summary>Private mode is on.</summary>
    public bool On { get; private set; }

    /// <summary>The mode turned on or off, or a title's own flag changed.</summary>
    public event Action? Changed;

    /// <summary>Folds one of the engine's events in. Returns whether the mode or a flag changed.</summary>
    public bool Take(Event e)
    {
        var changed = false;
        switch (e)
        {
            case Event.Private p:
                changed = On != p.V1;
                On = p.V1;
                Now = p.V1;
                break;
            case Event.AlwaysPrivate a:
                changed = Flag(a.Title, a.On);
                break;
            case Event.Progress p:
                // The title's own flag comes first in what the engine says, so Always is the flag.
                changed = Flag(new PrivateTitle.Show(p.Id), p.PrivateValue == PrivateMode.Always);
                keptShows[p.Id] = p.Kept;
                break;
            case Event.ShowSettings sheet:
                changed = Flag(new PrivateTitle.Show(sheet.V1.Id), sheet.V1.AlwaysPrivate);
                break;
            case Event.Reading { V1: ReadingEvent.Series series }:
                changed = Flag(new PrivateTitle.Series(series.V1.Work), series.V1.Private == PrivateMode.Always);
                break;
        }
        if (changed)
            Changed?.Invoke();
        return changed;
    }

    bool Flag(PrivateTitle title, bool on) => title switch
    {
        PrivateTitle.Show show => on ? alwaysShows.Add(show.Id) : alwaysShows.Remove(show.Id),
        PrivateTitle.Series series => on ? alwaysSeries.Add(series.Work) : alwaysSeries.Remove(series.Work),
        _ => false,
    };

    /// <summary>Show <paramref name="id"/> is always private on this device.</summary>
    public bool ShowAlways(uint id) => alwaysShows.Contains(id);

    /// <summary>Series <paramref name="work"/> is always private on this device.</summary>
    public bool SeriesAlways(string work) => alwaysSeries.Contains(work);

    /// <summary>The line under show <paramref name="id"/>'s main action, or null when none shows.</summary>
    public PrivateLine? ShowLine(uint id) =>
        Line(ShowAlways(id) ? PrivateMode.Always : null, On, keptShows.GetValueOrDefault(id));

    /// <summary>The line under a series page's main action, or null when none shows.</summary>
    public PrivateLine? SeriesLine(SeriesView series) =>
        Line(SeriesAlways(series.Work) ? PrivateMode.Always : null, On, series.PlaceKept);

    /// <summary>
    /// What a page shows, from what its view said and whether the mode is on now: the title's own
    /// flag first, then the mode. The engine's <c>PrivateMode::shown</c>.
    /// </summary>
    public static PrivateMode? Shown(PrivateMode? view, bool on) => view switch
    {
        PrivateMode.Always => PrivateMode.Always,
        _ when on => PrivateMode.Session,
        _ => null,
    };

    /// <summary>The line's words for <paramref name="view"/> with the mode as it is now.</summary>
    public static PrivateLine? Line(PrivateMode? view, bool on, bool kept) => Shown(view, on) switch
    {
        PrivateMode.Always => new(T.TitleAlwaysPrivate, T.PrivateLineNotSaved),
        PrivateMode.Session => new(T.PrivateBadge, kept ? T.PrivateLineKept : T.PrivateLineNotSaved),
        _ => null,
    };

    /// <summary>
    /// The system's media controls say only "Playing in Kaeri", with no show, episode or picture:
    /// while the mode is on, or for a show always private.
    /// </summary>
    public bool Neutral(bool alwaysPrivate) => On || alwaysPrivate;

    /// <summary>What the media overlay's title and subtitle say: the episode and the show, or the neutral words alone.</summary>
    public static (string Title, string Subtitle, bool Picture) NowPlaying(string show, string episode, bool neutral) =>
        neutral ? (T.NowPlayingPrivate, "", false) : (episode, show, true);

    /// <summary>What screen readers hear once when the mode turns on or off.</summary>
    public static string Announcement(bool on) => on ? T.PrivateModeOnAnnouncement : T.PrivateModeOffAnnouncement;

    /// <summary>The line under the settings card, as the board has it: what it does not do, then the shortcut.</summary>
    public static string Under => $"{T.PrivateModeLimits} {T.PrivateModeShortcutHint(Keys)}";

    /// <summary>
    /// A title's id, a trailer's video id or a host as the app's own log lines write it: left out
    /// while private mode is on, since the engine's log filter knows addresses but not these.
    /// </summary>
    public static string Logged(object value) => Now ? "(private)" : value.ToString() ?? "";

    /// <summary>The shortcut's keys on Windows, as the hint under the card names them.</summary>
    public const string Keys = "Ctrl+Shift+N";
}
