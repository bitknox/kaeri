using Kaeri.Engine.Model;

namespace Kaeri.App;

/// <summary>Search queries as the apps ask them. The bindings leave a record's other fields unset, and the ones that are records of their own must be filled in.</summary>
public static class Searches
{
    /// <summary>A search for these words, with no filter.</summary>
    public static SearchQuery Text(string text) => Of(text);

    /// <summary>One season of one year, most popular first.</summary>
    public static SearchQuery InSeason(Season season, uint year) =>
        Of("", SearchSort.Popularity, season, new Bounds(Min: year, Max: year));

    /// <summary>Nothing set: no text, no sort and no filter, each list filled in for the bindings.</summary>
    public static SearchQuery Nothing => Of("");

    /// <summary>
    /// What is typed with the Search page's filters, which are a query of their own with no
    /// words. With nothing typed it is a browse, which the engine answers popular first unless a
    /// sort is picked. The period counts only for Trending.
    /// </summary>
    public static SearchQuery With(string text, SearchQuery filters) => Filled(filters) with
    {
        Text = text,
        Period = filters.Sort == SearchSort.Trending && filters.Period is TrendPeriod.Week or TrendPeriod.Month ? filters.Period : null,
    };

    /// <summary>The filters and sort of a query with its words left out: what Search keeps of a browse it opens.</summary>
    public static SearchQuery FiltersOf(SearchQuery query) => Filled(query) with { Text = "" };

    /// <summary>A query with every list and range filled in, as the bindings need to send it.</summary>
    static SearchQuery Filled(SearchQuery q) => q with
    {
        Text = q.Text ?? "",
        Genres = q.Genres ?? [],
        GenresExcluded = q.GenresExcluded ?? [],
        Tags = q.Tags ?? [],
        TagsExcluded = q.TagsExcluded ?? [],
        Formats = q.Formats ?? [],
        Statuses = q.Statuses ?? [],
        Years = q.Years ?? Open,
        Episodes = q.Episodes ?? Open,
        Minutes = q.Minutes ?? Open,
        Sources = q.Sources ?? [],
        Countries = q.Countries ?? [],
        StreamingOn = q.StreamingOn ?? [],
    };

    /// <summary>A range with neither end set.</summary>
    public static Bounds Open => new(Min: null, Max: null);

    /// <summary>
    /// Whether two queries ask the same thing. A query read back from the engine carries new
    /// arrays, which a record's own equality compares by reference.
    /// </summary>
    public static bool Same(SearchQuery a, SearchQuery b) =>
        a.Text == b.Text && a.Sort == b.Sort && a.Period == b.Period && a.Season == b.Season && a.MinTagRank == b.MinTagRank
        && a.MinScore == b.MinScore && a.MinPopularity == b.MinPopularity && a.LicensedOnly == b.LicensedOnly && a.Available == b.Available
        && a.OnList == b.OnList
        && Same(a.Years, b.Years) && Same(a.Episodes, b.Episodes) && Same(a.Minutes, b.Minutes)
        && Same(a.Genres, b.Genres) && Same(a.GenresExcluded, b.GenresExcluded)
        && Same(a.Tags, b.Tags) && Same(a.TagsExcluded, b.TagsExcluded)
        && Same(a.Formats, b.Formats) && Same(a.Statuses, b.Statuses) && Same(a.Sources, b.Sources)
        && Same(a.Countries, b.Countries) && Same(a.StreamingOn, b.StreamingOn);

    /// <summary>Whether two lists' views are the same, searches by what they ask.</summary>
    public static bool Same(ContentFor a, ContentFor b) =>
        a is ContentFor.Search x && b is ContentFor.Search y ? Same(x.Query, y.Query) : a == b;

    static bool Same<T>(T[]? a, T[]? b) => (a ?? []).SequenceEqual(b ?? []);

    static bool Same(Bounds? a, Bounds? b) => (a?.Min, a?.Max) == (b?.Min, b?.Max);

    /// <summary>AniList's genres, for the Genre filter until the engine's own list arrives.</summary>
    public static readonly string[] Genres =
    [
        "Action", "Adventure", "Comedy", "Drama", "Ecchi", "Fantasy", "Horror", "Mahou Shoujo", "Mecha", "Music",
        "Mystery", "Psychological", "Romance", "Sci-Fi", "Slice of Life", "Sports", "Supernatural", "Thriller",
    ];

    /// <summary>A genre as the filter shows it, in the app's language. AniList's name is what a search sends; a genre the catalogue has no word for shows as it came.</summary>
    public static string GenreName(string genre) => genre switch
    {
        "Action" => T.GenreAction,
        "Adventure" => T.GenreAdventure,
        "Comedy" => T.GenreComedy,
        "Drama" => T.GenreDrama,
        "Ecchi" => T.GenreEcchi,
        "Fantasy" => T.GenreFantasy,
        "Horror" => T.GenreHorror,
        "Mahou Shoujo" => T.GenreMahouShoujo,
        "Mecha" => T.GenreMecha,
        "Music" => T.GenreMusic,
        "Mystery" => T.GenreMystery,
        "Psychological" => T.GenrePsychological,
        "Romance" => T.GenreRomance,
        "Sci-Fi" => T.GenreSciFi,
        "Slice of Life" => T.GenreSliceOfLife,
        "Sports" => T.GenreSports,
        "Supernatural" => T.GenreSupernatural,
        "Thriller" => T.GenreThriller,
        _ => genre,
    };

    /// <summary>A format as the Format filter names it: "TV", "Movie", "OVA".</summary>
    public static string FormatName(MediaFormat format) => format switch
    {
        MediaFormat.Tv => T.FormatTv,
        MediaFormat.TvShort => T.FormatTvShort,
        MediaFormat.Movie => T.FormatMovie,
        MediaFormat.Special => T.FormatSpecial,
        MediaFormat.Ova => T.FormatOva,
        MediaFormat.Ona => T.FormatOna,
        _ => T.FormatMusic,
    };

    /// <summary>A sort as the Sort filter names it.</summary>
    public static string SortName(SearchSort sort) => sort switch
    {
        SearchSort.Relevance => T.SortRelevance,
        SearchSort.Trending => T.SearchSortTrending,
        SearchSort.Popularity => T.SortPopularity,
        SearchSort.Score => T.SortScore,
        SearchSort.Newest => T.SearchSortNewest,
        SearchSort.Oldest => T.SearchSortOldest,
        SearchSort.Title => T.SearchSortTitle,
        _ => T.SearchSortFavourites,
    };

    /// <summary>A sort as the Sort button says it: Trending with its period ("Trending this week").</summary>
    public static string SortName(SearchSort sort, TrendPeriod? period) =>
        sort == SearchSort.Trending ? Browsing.Heading(period ?? TrendPeriod.Today) : SortName(sort);

    /// <summary>The formats the Format filter offers, by the engine's names for them.</summary>
    public static readonly MediaFormat[] Formats =
        [MediaFormat.Tv, MediaFormat.Movie, MediaFormat.Ona, MediaFormat.Ova, MediaFormat.Special, MediaFormat.TvShort];

    /// <summary>The sorts the Sort filter offers beside the default (best match, or popular when browsing).</summary>
    public static readonly SearchSort[] Sorts =
        [SearchSort.Popularity, SearchSort.Trending, SearchSort.Score, SearchSort.Newest, SearchSort.Title];

    /// <summary>The years the Year filter offers: next year back to 1970, newest first.</summary>
    public static uint[] Years(int thisYear) => [.. Enumerable.Range(1970, thisYear - 1970 + 2).Reverse().Select(y => (uint)y)];

    /// <summary>What the Available drop-down says for a choice: "Plays here", "On your services".</summary>
    public static string AvailableName(Available available) => available switch
    {
        Available.PlaysHere => T.SearchAvailablePlaysHere,
        _ => T.SearchAvailableServices,
    };

    /// <summary>The statuses the Status filter offers, in the engine's order.</summary>
    public static AiringStatus[] Statuses => KaeriModelMethods.SearchStatuses();

    /// <summary>A status as the Status filter names it: "Airing", "Aired", "Not yet released".</summary>
    public static string StatusName(AiringStatus status) => status switch
    {
        AiringStatus.Releasing => T.MediaStatusAiring,
        AiringStatus.Finished => T.MediaStatusAired,
        AiringStatus.NotYetReleased => T.MediaStatusUpcoming,
        AiringStatus.Cancelled => T.MediaStatusCancelled,
        _ => T.MediaStatusHiatus,
    };

    /// <summary>The source materials the panel offers, the common ones first, in the engine's order.</summary>
    public static SourceMaterial[] Sources => KaeriModelMethods.SearchSources();

    /// <summary>A source material in the app's language: "Original story", "Light novel".</summary>
    public static string SourceName(SourceMaterial source) => source switch
    {
        SourceMaterial.Original => T.MaterialOriginal,
        SourceMaterial.Manga => T.MaterialManga,
        SourceMaterial.LightNovel => T.MaterialLightNovel,
        SourceMaterial.VisualNovel => T.MaterialVisualNovel,
        SourceMaterial.VideoGame => T.MaterialVideoGame,
        SourceMaterial.Novel => T.MaterialNovel,
        SourceMaterial.Doujinshi => T.MaterialDoujinshi,
        SourceMaterial.Anime => T.MaterialAnime,
        SourceMaterial.WebNovel => T.MaterialWebNovel,
        SourceMaterial.LiveAction => T.MaterialLiveAction,
        SourceMaterial.Game => T.MaterialGame,
        SourceMaterial.Comic => T.MaterialComic,
        SourceMaterial.MultimediaProject => T.MaterialMultimedia,
        SourceMaterial.PictureBook => T.MaterialPictureBook,
        _ => T.RelationOther,
    };

    /// <summary>The countries of origin the panel offers, as ISO codes, in the engine's order.</summary>
    public static string[] Countries => KaeriModelMethods.SearchCountries();

    /// <summary>A country of origin by name in the app's language, or the engine's word for one it has no name for.</summary>
    public static string CountryName(string code) => code.ToUpperInvariant() switch
    {
        "JP" => T.CountryJapan,
        "CN" => T.CountryChina,
        "KR" => T.CountrySouthKorea,
        "TW" => T.CountryTaiwan,
        _ => KaeriModelMethods.SearchCountryLabel(code),
    };

    static SearchQuery Of(string text, SearchSort? sort = null, Season? season = null, Bounds? years = null) => new(
        Text: text,
        Sort: sort,
        Genres: [],
        GenresExcluded: [],
        Tags: [],
        TagsExcluded: [],
        Formats: [],
        Statuses: [],
        Season: season,
        Years: years ?? new Bounds(Min: null, Max: null),
        Episodes: new Bounds(Min: null, Max: null),
        Minutes: new Bounds(Min: null, Max: null),
        Sources: [],
        Countries: [],
        StreamingOn: []);
}

/// <summary>
/// The Search page's filters: a query with no words, read and changed through the engine's own
/// rules (a genre picked, a format toggled, a chip's words, what Clear takes off), as the Mac and
/// Android use them. What is here is what Windows words or decides for itself.
/// </summary>
public static class SearchFilters
{
    /// <summary>"Hide obscure titles": at least this many people have the title on a list. The engine's <c>OBSCURE_BELOW</c>.</summary>
    public const uint ObscureBelow = 1_000;

    /// <summary>How strongly a tag must apply when nothing else is set. The engine's <c>DEFAULT_TAG_RANK</c>.</summary>
    public const byte DefaultTagRank = 60;

    /// <summary>The episode counts the panel offers with one click: one, up to 12, 13 to 26, 50 or more. The Mac's.</summary>
    public static readonly Bounds[] EpisodePresets = [new(1, 1), new(null, 12), new(13, 26), new(50, null)];

    /// <summary>The episode lengths the panel offers with one click: up to 15 minutes, 20 to 30, an hour or more. The Mac's.</summary>
    public static readonly Bounds[] MinutePresets = [new(null, 15), new(20, 30), new(60, null)];

    /// <summary>A filter narrows the search, so an empty box no longer shows the before-typing page.</summary>
    public static bool Narrows(this SearchQuery filters) => filters.HasFilters();

    /// <summary>Nothing set, the sort included: Clear filters has nothing to take off.</summary>
    public static bool IsEmpty(this SearchQuery filters) => filters.Sort is null && !filters.HasFilters();

    /// <summary>The year, when the years are a single one.</summary>
    public static uint? SingleYear(this SearchQuery filters) =>
        filters.Years is { Min: { } min, Max: { } max } && min == max ? min : null;

    /// <summary>
    /// The years as one year, or none, from the Year drop-down. Unlike the engine's <c>in_year</c>
    /// the season stays: on Windows a season alone stands for that season of every year.
    /// </summary>
    public static SearchQuery WithYear(this SearchQuery filters, uint? year) => filters with { Years = new Bounds(year, year) };

    /// <summary>The years as a range, from the panel. A season keeps only while the years are open or a single one.</summary>
    public static SearchQuery InYears(this SearchQuery filters, Bounds years)
    {
        var range = years.Min is not null || years.Max is not null;
        var single = years.Min is not null && years.Min == years.Max;
        return filters with { Years = years, Season = range && !single ? null : filters.Season };
    }

    /// <summary>What a click on a genre or tag makes it: off, then in, then out, then off again.</summary>
    public static Pick Next(this Pick pick) => pick switch
    {
        Pick.Off => Pick.Include,
        Pick.Include => Pick.Exclude,
        _ => Pick.Off,
    };

    /// <summary>A pick's word, for a tag's row and the screen reader: "Included", "Left out", or null when off.</summary>
    public static string? Word(this Pick pick) => pick switch
    {
        Pick.Include => T.FilterIncluded,
        Pick.Exclude => T.FilterLeftOut,
        _ => null,
    };

    /// <summary>The genres as the Genre drop-down says them: those wanted, then those left out ("Not Romance").</summary>
    public static string[] GenreWords(this SearchQuery filters) =>
        [.. filters.Genres.Select(Searches.GenreName), .. filters.GenresExcluded.Select(g => T.SearchGenreLeftOut(Searches.GenreName(g)))];

    /// <summary>
    /// The chips under the filter row: each filter set, in the order the filters are offered, with
    /// the key that takes it off. The engine words each one, as on the Mac and Android; the
    /// genres, formats, statuses, season, sources, countries and Available take the app's own
    /// words, as the drop-downs and the panel say them.
    /// </summary>
    public static (FilterKey Key, string Label)[] Chips(this SearchQuery filters) =>
        [.. filters.Active().Select(f => (f.Key, Label(filters, f)))];

    static string Label(SearchQuery q, ActiveFilter filter) => filter.Key switch
    {
        FilterKey.Genre g => Searches.GenreName(g.Name),
        FilterKey.NotGenre g => T.SearchGenreLeftOut(Searches.GenreName(g.Name)),
        FilterKey.Premiered when q.Season is { } season => Wording.SeasonName(season),
        FilterKey.Formats => Joined(q.Formats.Select(Searches.FormatName)),
        FilterKey.Statuses => Joined(q.Statuses.Select(Searches.StatusName)),
        FilterKey.Sources => Joined(q.Sources.Select(Searches.SourceName)),
        FilterKey.Countries => Joined(q.Countries.Select(Searches.CountryName)),
        FilterKey.Obscure => T.FilterHideObscure,
        FilterKey.Licensed => T.FilterLicensedOnly,
        FilterKey.Available when q.Available is { } available => Searches.AvailableName(available),
        _ => filter.Label,
    };

    /// <summary>Names in a row, as the engine joins a chip's: "TV, Movie".</summary>
    static string Joined(IEnumerable<string> names) => string.Join(", ", names);

    /// <summary>A preset's words, the engine's as its chip says them: "1 episode", "Up to 12 episodes".</summary>
    public static string EpisodesWords(Bounds episodes) =>
        (Searches.Nothing with { Episodes = episodes }).Active().FirstOrDefault()?.Label ?? "";

    /// <summary>A preset's words, the engine's as its chip says them: "Up to 15 min", "20 to 30 min".</summary>
    public static string MinutesWords(Bounds minutes) =>
        (Searches.Nothing with { Minutes = minutes }).Active().FirstOrDefault()?.Label ?? "";

    /// <summary>A range from two number fields, either of them empty. An empty field reads as NaN.</summary>
    public static Bounds Range(double from, double to) => new(Whole(from), Whole(to));

    static uint? Whole(double value) => double.IsNaN(value) || value < 0 ? null : (uint)Math.Round(Math.Min(value, uint.MaxValue));

    /// <summary>The least score the panel's slider asks for: none at its start.</summary>
    public static byte? Score(double value) => value < 1 ? null : (byte)Math.Min(100, Math.Round(value));

    /// <summary>The tag rank the panel's slider asks for: none at the default, so the query says no more than it must.</summary>
    public static byte? TagRank(double value)
    {
        var rank = (byte)Math.Clamp(Math.Round(value), 0, 100);
        return rank == DefaultTagRank ? null : rank;
    }

    /// <summary>The tags whose names hold what is typed in Find a tag, any case, spoilers only while shown, at most 40.</summary>
    public static FacetTag[] TagsFound(SearchFacets? facets, string typed, bool spoilers)
    {
        var words = typed.Trim();
        if (facets is null || words.Length == 0)
            return [];
        return [.. (facets.TagGroups ?? []).SelectMany(g => g.Tags ?? [])
            .Where(t => (spoilers || !t.Spoiler) && t.Name.Contains(words, StringComparison.CurrentCultureIgnoreCase))
            .Take(40)];
    }

    /// <summary>
    /// What the title bar's box offers as filters for what is typed: the genres, then up to six
    /// tags, whose names start a word with it, none already wanted, at most eight, from two letters.
    /// </summary>
    public static (bool Tag, string Name)[] Suggestions(SearchFacets? facets, SearchQuery filters, string typed)
    {
        var words = typed.Trim();
        if (facets is null || words.Length < 2)
            return [];
        var genres = facets.GenresMatching(words).Where(g => !filters.Genres.Contains(g)).Select(g => (Tag: false, Name: g));
        var tags = facets.TagsMatching(words, 6).Where(t => !filters.Tags.Contains(t)).Select(t => (Tag: true, Name: t));
        return [.. genres.Concat(tags).Take(8)];
    }

    /// <summary>What a drop-down says: its name when unset, else what it has, "Action, Drama" or "Action +2".</summary>
    public static string Say(string name, IReadOnlyList<string> picked) => picked.Count switch
    {
        0 => name,
        1 => picked[0],
        2 => T.FilterPickedTwo(picked[0], picked[1]),
        _ => T.FilterPickedMore(picked[0], picked.Count - 1),
    };
}
