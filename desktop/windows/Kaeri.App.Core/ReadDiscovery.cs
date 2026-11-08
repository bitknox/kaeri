using Kaeri.Engine.Model;

namespace Kaeri.App;

/// <summary>
/// The words and choices of reading's discovery: Home's rows, and Search's From your sources
/// section, as the discovery board draws them. Kept apart from the controls so they can be tested.
/// </summary>
public static class ReadDiscovery
{
    /// <summary>"Server", "Folder" or "Plugin" beside a source's name; nothing for one built in, such as the demo.</summary>
    public static string? KindWords(ReadSourceKind? kind) => kind switch
    {
        ReadSourceKind.Folder => T.ReadSearchSourceKindFolder,
        ReadSourceKind.Server => T.ReadSearchSourceKindServer,
        ReadSourceKind.Plugin => T.ReadSearchSourceKindPlugin,
        _ => null,
    };

    /// <summary>
    /// An AniList result's line in Search: its format and origin, "Light novel · Japan". The
    /// format is the catalogue's words the engine put first in the facts, never its key.
    /// </summary>
    public static string? Caption(WorkTile tile)
    {
        if (tile.NotInSources)
            return T.HomeNotFound;
        return Two(Format(tile), tile.Origin?.Text());
    }

    /// <summary>
    /// A Home row tile's line, two facts at most, as the board draws each row: Trending
    /// "Japan · Releasing", New "Japan · since March" (its status until the engine says when it
    /// started), Finished "Japan · 112 chapters", Because and More of a genre, mixing formats,
    /// "Manga · Finished". Not found in your sources once a series' check found none.
    /// </summary>
    public static string RowCaption(ReadRowKind kind, WorkTile tile)
    {
        if (tile.NotInSources)
            return T.HomeNotFound;
        var origin = tile.Origin?.Text();
        var lead = kind is ReadRowKind.Because or ReadRowKind.MoreGenre ? Format(tile) : origin ?? Format(tile);
        var second = kind switch
        {
            ReadRowKind.Finished => Count(tile) ?? Status(tile),
            _ => Status(tile),
        };
        return Two(lead, second) ?? "";
    }

    /// <summary>The format's words, "Light novel": the first of the engine's facts.</summary>
    public static string? Format(WorkTile tile) => tile.Facts is { Length: > 0 } facts ? facts[0].Text() : null;

    /// <summary>The status among the facts, "Releasing".</summary>
    public static string? Status(WorkTile tile) =>
        (tile.Facts ?? []).FirstOrDefault(IsStatus)?.Text();

    /// <summary>The count among the facts, "112 chapters" or "9 volumes".</summary>
    public static string? Count(WorkTile tile) =>
        (tile.Facts ?? []).FirstOrDefault(f => f is Msg.Reading { V1: ReadingMsg.ReadingChapters or ReadingMsg.ReadingVolumes })?.Text();

    static bool IsStatus(Msg fact) => fact is Msg.Reading
    {
        V1: ReadingMsg.ReadingStatusReleasing or ReadingMsg.ReadingStatusFinished or ReadingMsg.ReadingStatusHiatus
            or ReadingMsg.ReadingStatusCancelled or ReadingMsg.ReadingStatusNotYet,
    };

    static string? Two(string? first, string? second)
    {
        var parts = new[] { first, second }.Where(p => !string.IsNullOrEmpty(p)).Cast<string>().ToArray();
        return parts.Length == 0 ? null : string.Join(" · ", parts);
    }

    /// <summary>
    /// A filter's chosen formats as the bar says them: one-shots fold into Manga, so "Manga" for
    /// manga with one-shots, else the first and how many more.
    /// </summary>
    public static string[] FoldedFormats(string[]? keys) =>
        (keys ?? []) is var all && all.Contains("manga") ? [.. all.Where(k => k != "one-shot")] : all; // i18n-skip: the engine's format keys

    /// <summary>The format menu's choices: one Manga, which takes one-shots in, so no one-shot choice of its own.</summary>
    public static ReadFilterChoice[] FormatChoices(ReadFilterChoice[] choices) =>
        [.. choices.Where(c => c.Key != OneShot)];

    /// <summary>
    /// The formats after Manga or another format is chosen or let go in the menu. While the
    /// engine still lists one-shots apart, Manga takes them with it, so Manga always includes
    /// one-shots.
    /// </summary>
    public static string[] ToggleFormat(string[]? chosen, string key, bool oneShotsApart)
    {
        var list = (chosen ?? []).ToList();
        var on = !list.Contains(key);
        list.RemoveAll(k => k == key || key == MangaKey && k == OneShot);
        if (on)
        {
            list.Add(key);
            if (key == MangaKey && oneShotsApart)
                list.Add(OneShot);
        }
        return [.. list];
    }

    // i18n-skip: the engine's format keys
    const string MangaKey = "manga", OneShot = "one-shot";

    /// <summary>A source's hit's line: how much of the series the source holds, "2 volumes"; empty when the source does not say.</summary>
    public static string Amount(WorkTile tile) => tile.Amount?.Text() ?? "";

    /// <summary>
    /// The line over AniList's results: "212 results" once the count is exact, else "Series from
    /// AniList", as AniList's own count for a longer search is a stand-in.
    /// </summary>
    public static string Total(uint? total) => total is { } n ? T.ReadSearchTotal(n) : T.ReadSearchTotalUnknown;

    /// <summary>
    /// The third line of an AniList result that sources have: "In Home library", or "In 2 of
    /// your sources". Null when no source joined it.
    /// </summary>
    public static string? Placed(ReadSourceSearch? sources, string work)
    {
        if (sources?.Joins?.FirstOrDefault(j => j.Work == work) is not { } join)
            return null;
        var names = join.SourceNames ?? [];
        return names.Length switch
        {
            0 => null,
            1 => T.ReadSearchInSource(names[0]),
            _ => T.ReadSearchInSources(names.Length),
        };
    }

    /// <summary>Whether any result shown has a third line, so every cell of the grid keeps room for one.</summary>
    public static bool AnyPlaced(ReadSourceSearch? sources, IEnumerable<WorkTile> tiles) =>
        sources?.Joins is { Length: > 0 } joins && tiles.Any(t => joins.Any(j => j.Work == t.Work));

    /// <summary>
    /// Whether a group takes a slot of its own: asking, with hits left after the joins, or failed.
    /// Groups that found nothing share the line at the end, and one whose every hit joined an
    /// AniList result is left out.
    /// </summary>
    public static bool Slotted(ReadSourceGroup group) => group.State switch
    {
        ReadSourceState.Asking => true,
        ReadSourceState.Hits => (group.Tiles?.Length ?? 0) > 0,
        ReadSourceState.Failed or ReadSourceState.TimedOut => true,
        _ => false,
    };

    /// <summary>The names of the groups that found nothing, in Your sources order.</summary>
    public static IReadOnlyList<string> NothingFrom(ReadSourceSearch sources) =>
        [.. (sources.Groups ?? []).Where(g => g.State is ReadSourceState.Nothing).Select(g => g.SourceName)];

    /// <summary>"Nothing found in Comics folder and Demo.", or null when every source found something or is still asked.</summary>
    public static string? NothingLine(ReadSourceSearch sources) =>
        StartScreen.Names(NothingFrom(sources)) is { } names ? T.ReadSearchSourceNothing(names) : null;

    /// <summary>Whether every source has answered and found nothing: with AniList's nothing, nothing anywhere.</summary>
    public static bool SourcesFoundNothing(ReadSourceSearch sources) =>
        (sources.Groups ?? []).All(g => g.State is ReadSourceState.Nothing);

    /// <summary>The hits the sources found: those in their rows and those that joined an AniList result.</summary>
    public static int Hits(ReadSourceSearch sources) =>
        (sources.Groups ?? []).Sum(g => (g.Tiles?.Length ?? 0) + (int)g.Joined);

    /// <summary>
    /// What Narrator says once when the whole search has settled: "4 from AniList, 2 from your
    /// sources. Northwind didn't answer." Null until it has.
    /// </summary>
    public static string? Finished(ReadSourceSearch sources, int anilist)
    {
        if (!sources.Done)
            return null;
        var said = sources.Anilist == ReadAnilistState.Skipped
            ? T.WindowsReadSearchFinishedSources(Hits(sources))
            : T.WindowsReadSearchFinished(anilist, Hits(sources));
        if (sources.Anilist == ReadAnilistState.Failed)
            said += " " + T.ReadSearchAnilistFailed;
        foreach (var group in sources.Groups ?? [])
            if (group.State is ReadSourceState.Failed or ReadSourceState.TimedOut)
                said += " " + T.ReadSearchSourceFailed(group.SourceName);
        return said;
    }

    /// <summary>A failed group's reason: the engine's for a failure, "No answer in 15 s." for the time running out.</summary>
    public static string? Reason(ReadSourceState state) => state switch
    {
        ReadSourceState.Failed failed => failed.Failure.Words(),
        ReadSourceState.TimedOut timedOut => T.ReadSearchSourceTimeout(timedOut.Seconds),
        _ => null,
    };

    /// <summary>"30 of 212 shown" beside More from AniList, or "30 shown" while the count is not known.</summary>
    public static string Shown(int shown, uint? total) =>
        total is { } all ? T.ReadSearchShown(shown, all) : T.ReadSearchShownSome(shown);

    /// <summary>
    /// A row's heading as runs: the words before the title, the title in the show font, and the
    /// words after it. Only "Because you finished X" has a title.
    /// </summary>
    public static HomeHeading Heading(Msg title)
    {
        var whole = title.Text();
        if (title is Msg.HomeBecause because && because.Title.Length > 0)
        {
            var at = whole.LastIndexOf(because.Title, StringComparison.Ordinal);
            if (at >= 0)
                return new(whole[..at], because.Title, whole[(at + because.Title.Length)..]);
        }
        return new(whole);
    }

    /// <summary>Whether a query sets the Started filter.</summary>
    public static bool StartedSet(ReadSearchQuery query) => query.Started is { Length: > 0 } s && s != "any";
}
