using Medium = Kaeri.Engine.Core.Medium;
using Kaeri.Engine.Model;

namespace Kaeri.App;

// Reading's places as the engine sends them: what Kaeri is for, Home, Updates, the lists, Search,
// a series and its chapters, the downloads and the book sources. The pages draw what is here and
// ask through these; nothing is decided on this side. The twin of the Apple app's ReadingModel.

/// <summary>What Kaeri is for on this device, and the question still to ask about it.</summary>
public sealed class MediumState
{
    internal Action<Command>? Send { get; set; }

    public MediumView? View { get; private set; }

    /// <summary>
    /// The question the dialog answers, held from the moment it shows until it is answered, so a
    /// view that arrives meanwhile does not take the dialog away.
    /// </summary>
    public MediumAsk? Asking { get; private set; }

    /// <summary>The view changed: the sidebar and Settings read it again.</summary>
    public event Action? Changed;

    /// <summary>The answer in force: Watch until the engine says otherwise.</summary>
    public KaeriIsFor Answer => View is { ReadingAvailable: true } view ? view.Answer : KaeriIsFor.Watch;

    public bool Reads => Answer is KaeriIsFor.Read or KaeriIsFor.Both;
    public bool Watches => Answer is KaeriIsFor.Watch or KaeriIsFor.Both;
    public bool Both => Answer == KaeriIsFor.Both;

    /// <summary>Whether the answer can be changed here: reading is in this build and the edition does not fix it.</summary>
    public bool Choosable => View is { ReadingAvailable: true, Fixed: false };

    public void Apply(MediumView view)
    {
        View = view;
        if (Asking is null && view.Ask is { } ask)
            Asking = ask;
        if (view.Ask is null)
            Asking = null;
        Changed?.Invoke();
    }

    public void Load() => Send?.Invoke(new Command.Medium(new MediumCommand.Load()));

    /// <summary>Settings' choice. Turning a medium off may delete its downloads.</summary>
    public void Choose(KaeriIsFor answer, bool deleteDownloads = false) =>
        Send?.Invoke(new Command.Medium(new MediumCommand.Choose(answer, deleteDownloads)));

    /// <summary>The first run's page, the dialog after the update, or the join dialog, answered.</summary>
    public void Answered(MediumAsk ask, KaeriIsFor answer)
    {
        Asking = null;
        Send?.Invoke(new Command.Medium(new MediumCommand.Answer(ask, answer)));
        Changed?.Invoke();
    }
}

/// <summary>A series' chapters as the engine last sent them.</summary>
public sealed record ReadChapters(BookEditionRow? Edition, VolumeGroup[] Volumes, ReadingFailure? Failure)
{
    public IEnumerable<ChapterRow> All => Volumes.SelectMany(v => v.Chapters);
}

/// <summary>Reading's search: the query on screen, its pages so far, and whether more can come.</summary>
public sealed class ReadSearchState
{
    public ReadSearchQuery Query { get; internal set; } = ReadingStore.EmptyQuery;
    public List<WorkTile> Tiles { get; } = [];
    public uint Page { get; internal set; }
    public bool HasMore { get; internal set; }
    public uint? OtherScope { get; internal set; }
    public uint? Total { get; internal set; }
    public bool Loading { get; internal set; }
    public ReadingFailure? Failure { get; internal set; }
    /// <summary>
    /// The sources' part of a search with words, the newest the engine sent for the query on
    /// screen. Null while browsing, and until the first send.
    /// </summary>
    public ReadSourceSearch? Sources { get; internal set; }
    /// <summary>The words AniList's results in <see cref="Tiles"/> were found for.</summary>
    public string TilesFor { get; internal set; } = "";

    /// <summary>How many results the first page brought: More from AniList's pages show under its button.</summary>
    public int FirstPage { get; internal set; }
}

/// <summary>Which part of reading changed, so a page redraws only what it shows.</summary>
public enum ReadingPart
{
    Home, Updates, Library, Search, Series, Chapters, Versions, Tracking, Downloads, Estimate,
    Removals, Failure, ReaderDefaults, Send, Feed, Targets,
}

public sealed class ReadingStore
{
    internal Action<Command>? Send { get; set; }

    public static readonly ReadSearchQuery EmptyQuery = new("", null, [], [], null, [], [], null);

    public ReadingHomeView? Home { get; private set; }
    public bool HomeLoading { get; private set; }
    public UpdatesView? Updates { get; private set; }
    public Dictionary<LibrarySegmentRead, WorkTile[]> Lists { get; } = [];
    public ReadSearchState Search { get; } = new();
    public Dictionary<string, SeriesView> Series { get; } = [];
#if KAERI_TESTING
    /// <summary>Shots: the editions as a source with several groups lists them, since the fixture editions name none.</summary>
    public Func<BookEditionRow, BookEditionRow>? EditionShot { get; set; }
    /// <summary>Shots: the chapter rows that go with <see cref="EditionShot"/>.</summary>
    public Func<VolumeGroup[], VolumeGroup[]>? VolumesShot { get; set; }
#endif
    public Dictionary<string, ReadChapters> Chapters { get; } = [];
    public Dictionary<string, ChapterVersion[]> Versions { get; } = [];
    public Dictionary<string, TrackerReadRow[]> Tracking { get; } = [];
    public BookDownloadsView? Downloads { get; private set; }
    /// <summary>What the chapters being chosen will take to download, per series.</summary>
    public Dictionary<string, DownloadEstimate> Estimates { get; } = [];
    /// <summary>What Remove from lists came to, per series, until the next removal.</summary>
    public Dictionary<string, ListRemoval[]> Removals { get; } = [];
    public Dictionary<ReadingAbout, ReadingFailure> Failures { get; } = [];
    /// <summary>The reader's defaults per medium, for Settings' Reading pane.</summary>
    public Dictionary<Medium, ReaderSettingsView> ReaderDefaults { get; } = [];
    /// <summary>Send to e-reader and the feed.</summary>
    public SendState Sending { get; } = new();

    /// <summary>What changed, after it is kept.</summary>
    public event Action<ReadingPart>? Changed;

    /// <summary>How many chapters wait unread in Updates: the sidebar's count.</summary>
    public uint Unread => Updates?.Unread ?? 0;

    void Ask(ReadingCommand command) => Send?.Invoke(new Command.Reading(command));
    void AskReader(ReaderCommand command) => Send?.Invoke(new Command.Reader(command));

    // Asking

    public void LoadHome(bool refresh = false)
    {
        HomeLoading = true;
        Failures.Remove(ReadingAbout.Home);
        Ask(new ReadingCommand.LoadHome(refresh));
    }

    public void LoadUpdates() => Ask(new ReadingCommand.LoadUpdates());
    public void CheckUpdates() => Ask(new ReadingCommand.CheckUpdates());
    public void DownloadNew() => Ask(new ReadingCommand.DownloadNew());
    public void LoadList(LibrarySegmentRead segment) => Ask(new ReadingCommand.LoadLibrary(segment));

    public void RunSearch(ReadSearchQuery query)
    {
        // The sources' groups stay while the same words are asked again, as with Your sources
        // only turned on or off: each keeps its slot until its new answer.
        if (Search.Sources is { } sources && sources.Query.Text.Trim() != query.Text.Trim())
            Search.Sources = null;
        if (query.Text.Trim().Length == 0)
            Search.Sources = null;
        // Results of another search never show under this one's filters, as when View all
        // opens; turning Your sources only on or off keeps AniList's results for the same words.
        if (!SameQuery(Search.Query with { SourcesOnly = false }, query with { SourcesOnly = false }))
        {
            Search.Tiles.Clear();
            Search.FirstPage = 0;
            Search.TilesFor = "";
        }
        Search.Query = query;
        Search.Total = null;
        Search.Loading = true;
        Search.Failure = null;
        Ask(new ReadingCommand.Search(query));
    }

    /// <summary>Try again on one source's group: only that group asks again.</summary>
    public void RetrySource(string sourceId) =>
        Ask(new ReadingCommand.RetrySourceSearch(Search.Query, sourceId));

    public void SearchMore()
    {
        if (!Search.HasMore || Search.Loading)
            return;
        Search.Loading = true;
        Ask(new ReadingCommand.SearchMore(Search.Query, Search.Page + 1));
    }

    public void LoadSeries(string work)
    {
        Failures.Remove(ReadingAbout.Series);
        Ask(new ReadingCommand.LoadSeries(work));
        Ask(new ReadingCommand.LoadChapters(work, null));
    }

    /// <summary>A series' details without its chapters, for a cover or title shown outside its page.</summary>
    public void LoadSeriesInfo(string work)
    {
        if (!Series.ContainsKey(work))
            Ask(new ReadingCommand.LoadSeries(work));
    }

    public void LoadChapters(string work, string? edition = null) => Ask(new ReadingCommand.LoadChapters(work, edition));
    public void LoadVersions(string work, string chapter) => Ask(new ReadingCommand.LoadVersions(work, chapter));

    public void ChooseEdition(string work, BookEditionRow edition) =>
        Ask(new ReadingCommand.ChooseEdition(work, edition.SourceId, edition.VersionLabel, edition.Language));

    /// <summary>The group chapters come from in the source in use, or null for the one with most chapters.</summary>
    public void ChooseGroup(string work, string? group) => Ask(new ReadingCommand.ChooseGroup(work, group));

    string? InUse(string work) => Chapters.TryGetValue(work, out var c) ? c.Edition?.SourceId : null;

    /// <summary>Marks, then the series and its chapters again, as the engine sends nothing for a mark.</summary>
    public void MarkRead(string work, IEnumerable<string> chapters, bool read)
    {
        Ask(new ReadingCommand.MarkRead(work, [.. chapters], read));
        RefreshSeries(work);
    }

    public void MarkReadUpTo(string work, string chapter)
    {
        Ask(new ReadingCommand.MarkReadUpTo(work, chapter));
        RefreshSeries(work);
    }

    public void ReadAgain(string work)
    {
        Ask(new ReadingCommand.ReadAgain(work));
        RefreshSeries(work);
    }

    public void UndoReadAgain(string work)
    {
        Ask(new ReadingCommand.UndoReadAgain(work));
        RefreshSeries(work);
    }

    public void Bookmark(string work, string chapter, bool on)
    {
        Ask(new ReadingCommand.Bookmark(work, chapter, on));
        Ask(new ReadingCommand.LoadChapters(work, InUse(work)));
    }

    public void HideFromContinue(string work) => Ask(new ReadingCommand.HideFromContinue(work));
    public void SetStatus(string work, LibrarySegmentRead status) => Ask(new ReadingCommand.SetStatus(work, status));

    /// <summary>Takes the series off every signed-in tracker's reading list; Kaeri keeps the place.</summary>
    public void RemoveFromLists(string work)
    {
        Removals.Remove(work);
        Ask(new ReadingCommand.RemoveFromLists(work));
    }

    public void SetScore(string work, float score) => Ask(new ReadingCommand.SetScore(work, score));
    public void LoadTracking(string work) => Ask(new ReadingCommand.LoadTracking(work));
    public void ChooseMapping(string work, string tracker, string? id) => Ask(new ReadingCommand.ChooseMapping(work, tracker, id));
    public void ResolveConflict(string work, string tracker, bool keepLocal) => Ask(new ReadingCommand.ResolveConflict(work, tracker, keepLocal));
    public void ConnectMangaUpdates(string username, string password) => Ask(new ReadingCommand.ConnectMangaUpdates(username, password));

    public void Download(string work, DownloadPick pick)
    {
        Ask(new ReadingCommand.Download(work, pick));
        Ask(new ReadingCommand.LoadChapters(work, InUse(work)));
    }

    public void CancelDownloads(string work, IEnumerable<string> chapters) => Ask(new ReadingCommand.DownloadCancel(work, [.. chapters]));

    public void DeleteDownloads(string work, IEnumerable<string> chapters)
    {
        Ask(new ReadingCommand.DownloadDelete(work, [.. chapters]));
        Ask(new ReadingCommand.LoadChapters(work, InUse(work)));
    }

    /// <summary>Delete read: the downloads of the series' read chapters, bookmarked ones kept.</summary>
    public void DeleteRead(string work)
    {
        Ask(new ReadingCommand.DownloadDeleteRead(work));
        Ask(new ReadingCommand.LoadDownloads());
    }

    /// <summary>Asks what downloading <paramref name="pick"/> will take; an empty pick clears the line.</summary>
    public void EstimateDownload(string work, DownloadPick pick)
    {
        if (pick is DownloadPick.Chapters { Ids.Length: 0 })
        {
            Estimates.Remove(work);
            Changed?.Invoke(ReadingPart.Estimate);
            return;
        }
        Ask(new ReadingCommand.EstimateDownload(work, pick));
    }

    public void LoadDownloads() => Ask(new ReadingCommand.LoadDownloads());

    void RefreshSeries(string work)
    {
        if (Series.ContainsKey(work))
        {
            Ask(new ReadingCommand.LoadSeries(work));
            Ask(new ReadingCommand.LoadChapters(work, InUse(work)));
        }
        // Updates' rows turn haze once read.
        if (Updates is not null)
            Ask(new ReadingCommand.LoadUpdates());
    }

    /// <summary>Asks for the manga and novel defaults, for Settings.</summary>
    public void LoadReaderDefaults()
    {
        AskReader(new ReaderCommand.LoadSettings(Medium.Manga, null));
        AskReader(new ReaderCommand.LoadSettings(Medium.Novel, null));
    }

    /// <summary>Changes a medium's default, as Settings does: kept where the key belongs, and synced when it is a choice.</summary>
    public void SetReaderDefault(Medium medium, ReaderSettingKey key, ReaderValue value) =>
        AskReader(new ReaderCommand.SetSetting(null, medium, null, key, ReaderSession.HomeScope(key), value));

    // Hearing

    public void Take(Event e)
    {
        switch (e)
        {
            case Event.Reader { V1: ReaderEvent.Settings settings } when settings.V1.Work is null:
                ReaderDefaults[settings.V1.Medium] = settings.V1;
                Changed?.Invoke(ReadingPart.ReaderDefaults);
                break;
            case Event.Reading reading:
                if (Sending.Apply(reading.V1) is { } sendPart)
                    Changed?.Invoke(sendPart);
                Apply(reading.V1);
                break;
        }
    }

    public void Apply(ReadingEvent e)
    {
        switch (e)
        {
            case ReadingEvent.Home home:
                Home = home.V1;
                HomeLoading = false;
                Changed?.Invoke(ReadingPart.Home);
                break;
            case ReadingEvent.Updates updates:
                Updates = updates.V1;
                Changed?.Invoke(ReadingPart.Updates);
                break;
            case ReadingEvent.Library library:
                Lists[library.Segment] = library.Tiles;
                Changed?.Invoke(ReadingPart.Library);
                break;
            case ReadingEvent.Search search:
                if (!SameQuery(search.Query, Search.Query))
                    return;
                if (search.Page == 0)
                {
                    Search.Total = null;
                    Search.Tiles.Clear();
                }
                if (search.Total is { } total)
                    Search.Total = total;
                Search.Tiles.AddRange(search.Tiles);
                if (search.Page == 0)
                    Search.FirstPage = Search.Tiles.Count;
                Search.TilesFor = search.Query.Text.Trim();
                Search.Page = search.Page;
                Search.HasMore = search.HasMore;
                Search.OtherScope = search.OtherScope;
                Search.Loading = false;
                // Results for the query on screen: a failure that came before them (an older
                // query's, which names none) no longer stands.
                Search.Failure = null;
                Failures.Remove(ReadingAbout.Search);
                Changed?.Invoke(ReadingPart.Search);
                break;
            case ReadingEvent.SourceSearch found:
                if (!SameQuery(found.V1.Query, Search.Query))
                    return;
                Search.Sources = found.V1;
                // Your sources only leaves AniList out: no page of its own comes, so the search
                // is answered here. AniList's last results stay for turning it off again, while
                // they are for the same words.
                if (found.V1.Anilist == ReadAnilistState.Skipped)
                {
                    Search.Loading = false;
                    Search.HasMore = false;
                    if (Search.TilesFor != found.V1.Query.Text.Trim())
                    {
                        Search.Tiles.Clear();
                        Search.Total = null;
                    }
                }
                Changed?.Invoke(ReadingPart.Search);
                break;
            case ReadingEvent.Series series:
                var seriesView = series.V1;
#if KAERI_TESTING
                if (EditionShot is { } staged && seriesView.Edition is { } shown)
                    seriesView = seriesView with { Edition = staged(shown) };
#endif
                Series[seriesView.Work] = seriesView;
                Changed?.Invoke(ReadingPart.Series);
                break;
            case ReadingEvent.Chapters chapters:
                var listed = new ReadChapters(chapters.Edition, chapters.Volumes, chapters.Failure);
#if KAERI_TESTING
                if (EditionShot is { } stagedEdition && listed.Edition is { } edition)
                    listed = listed with { Edition = stagedEdition(edition) };
                if (VolumesShot is { } stagedVolumes)
                    listed = listed with { Volumes = stagedVolumes(listed.Volumes) };
#endif
                Chapters[chapters.Work] = listed;
                Changed?.Invoke(ReadingPart.Chapters);
                break;
            case ReadingEvent.Versions versions:
                Versions[versions.Chapter] = versions.VersionsValue;
                Changed?.Invoke(ReadingPart.Versions);
                break;
            case ReadingEvent.Tracking tracking:
                Tracking[tracking.Work] = tracking.Rows;
                Changed?.Invoke(ReadingPart.Tracking);
                break;
            case ReadingEvent.Downloads downloads:
                Downloads = downloads.V1;
                Changed?.Invoke(ReadingPart.Downloads);
                break;
            case ReadingEvent.DownloadEstimate estimate:
                Estimates[estimate.V1.Work] = estimate.V1;
                Changed?.Invoke(ReadingPart.Estimate);
                break;
            case ReadingEvent.RemovedFromLists removed:
                Removals[removed.Work] = removed.Results;
                Changed?.Invoke(ReadingPart.Removals);
                RefreshSeries(removed.Work);
                break;
            case ReadingEvent.Failed failed:
                Failures[failed.About] = failed.Failure;
                if (failed.About == ReadingAbout.Home)
                    HomeLoading = false;
                if (failed.About == ReadingAbout.Search)
                {
                    Search.Loading = false;
                    Search.Failure = failed.Failure;
                }
                Changed?.Invoke(ReadingPart.Failure);
                break;
        }
    }

    /// <summary>Two queries are the same search: lists compare by their items.</summary>
    public static bool SameQuery(ReadSearchQuery a, ReadSearchQuery b) =>
        a.Text == b.Text && a.Medium == b.Medium && a.Status == b.Status && a.Sort == b.Sort
        && Started(a.Started) == Started(b.Started) && a.SourcesOnly == b.SourcesOnly
        && Same(a.Formats, b.Formats) && Same(a.Origins, b.Origins)
        && Same(a.Genres, b.Genres) && Same(a.ExcludeGenres, b.ExcludeGenres)
        && (a.SourceScope is null ? b.SourceScope is null : b.SourceScope is not null && Same(a.SourceScope, b.SourceScope));

    static bool Same(string[]? a, string[]? b) => (a ?? []).SequenceEqual(b ?? []);

    /// <summary>"any" and no choice are the same: any time.</summary>
    static string? Started(string? key) => key is null or "any" ? null : key;
}

/// <summary>The sources that have a work, for Read from.</summary>
public sealed record WorkEditions(BookEditionRow[] Editions, bool Asking, EditionMiss[] Missing);

/// <summary>What "Search there" found in one source for a work.</summary>
public sealed record SourceSeriesSearch(string SourceId, string Query, SourceSeriesChoice[] Choices, ReadingFailure? Failure, bool Searching);

/// <summary>What Add a server found at the address typed, before signing in.</summary>
public sealed record ServerFound(string Address, BookServerKind Kind, string Name, string Version);

/// <summary>Folders, servers and plugins that books come from.</summary>
public sealed class BookSourcesState
{
    internal Action<Command>? Send { get; set; }

    public BookSourcesView? View { get; private set; }
    public ServerFound? Checked { get; private set; }
    public (string Id, BookServerKind Kind, uint? Series)? Added { get; private set; }
    public (ServerStep Step, BookServerFailure Failure)? Failed { get; private set; }
    public bool Checking { get; private set; }
    public Dictionary<string, MatchChoice[]> Matches { get; } = [];
    /// <summary>Which of your sources have a work, as each answers: the Read from dialog.</summary>
    public Dictionary<string, WorkEditions> ForWork { get; } = [];
    /// <summary>"Search there" for a work, one source at a time.</summary>
    public Dictionary<string, SourceSeriesSearch> SourceSeries { get; } = [];

    public event Action? Changed;

    public bool Empty => View is not { } v || (v.Folders.Length == 0 && v.Servers.Length == 0 && v.Plugins.Length == 0);

    void Ask(BookSourceCommand command) => Send?.Invoke(new Command.BookSources(command));

    public void Load() => Ask(new BookSourceCommand.Load());
    public void AddFolder(string path, string name) => Ask(new BookSourceCommand.AddFolder(new FolderRef(path, null, null, name)));
    public void RemoveFolder(string id) => Ask(new BookSourceCommand.RemoveFolder(id));
    public void RestoreFolder(string id) => Ask(new BookSourceCommand.RestoreFolder(id));
    public void Scan() => Ask(new BookSourceCommand.Scan());
    public void RemoveServer(string id) => Ask(new BookSourceCommand.RemoveServer(id));
    public void SetOff(string id, bool off) => Ask(new BookSourceCommand.SetOff(id, off));
    public void SetReportProgress(string id, bool on) => Ask(new BookSourceCommand.SetReportProgress(id, on));
    public void RetryProgress(string id) => Ask(new BookSourceCommand.RetryProgress(id));
    public void FindMatch(string group, string query) => Ask(new BookSourceCommand.FindMatch(group, query));
    public void ChooseMatch(string group, uint anilist) => Ask(new BookSourceCommand.ChooseMatch(group, anilist));
    public void Keep(string group) => Ask(new BookSourceCommand.Keep(group));
    public void SetOrder(IEnumerable<string> ids) => Ask(new BookSourceCommand.SetOrder([.. ids]));

    public void LoadForWork(string work)
    {
        ForWork[work] = ForWork.TryGetValue(work, out var was) ? was with { Asking = true } : new WorkEditions([], true, []);
        Ask(new BookSourceCommand.LoadForWork(work));
        Changed?.Invoke();
    }

    /// <summary>"Search there": look for the work in one source by the words typed.</summary>
    public void SearchSource(string work, string sourceId, string query)
    {
        SourceSeries[work] = new SourceSeriesSearch(sourceId, query, [], null, true);
        Ask(new BookSourceCommand.SearchSourceForWork(work, sourceId, query));
        Changed?.Invoke();
    }

    /// <summary>The series picked in that source is the work; the editions come again.</summary>
    public void ChooseSourceSeries(string work, string sourceId, string seriesRef)
    {
        SourceSeries.Remove(work);
        if (ForWork.TryGetValue(work, out var was))
            ForWork[work] = was with { Asking = true };
        Ask(new BookSourceCommand.ChooseSourceSeries(work, sourceId, seriesRef));
        Changed?.Invoke();
    }

    public void ClearSourceSearch(string work) => SourceSeries.Remove(work);

    public void CheckServer(string address)
    {
        Checked = null;
        Failed = null;
        Added = null;
        Checking = true;
        Ask(new BookSourceCommand.CheckServer(address));
        Changed?.Invoke();
    }

    public void SignIn(string address, string user, string password)
    {
        Failed = null;
        Checking = true;
        Ask(new BookSourceCommand.SignIn(address, user, password));
        Changed?.Invoke();
    }

    /// <summary>
    /// Looks at a server before anything is added: an address with a user and password, or a
    /// Kavita OPDS link alone. Answered with <c>Event.CameIn</c>: Found, or Empty with why not.
    /// </summary>
    public void Probe(string address, string? user, string? password) =>
        Ask(new BookSourceCommand.Probe(address, user, password));

    /// <summary>Adds a server Probe found, asking only <paramref name="libraries"/> (empty asks every one).</summary>
    public void AddServer(string pending, IEnumerable<string> libraries, bool reportProgress) =>
        Ask(new BookSourceCommand.AddServer(pending, reportProgress, [.. libraries]));

    /// <summary>Looks again at servers that showed no series, now.</summary>
    public void Recheck() => Ask(new BookSourceCommand.Recheck());

    public void ResetServerDialog()
    {
        Checked = null;
        Added = null;
        Failed = null;
        Checking = false;
    }

    public void Take(Event e)
    {
        if (e is not Event.BookSources { V1: var source })
            return;
        switch (source)
        {
            case BookSourceEvent.View view:
                View = view.V1;
                break;
            case BookSourceEvent.Matches matches:
                Matches[matches.Group] = matches.Choices;
                break;
            case BookSourceEvent.ServerChecked found:
                Checked = new ServerFound(found.Address, found.Kind, found.Name, found.Version);
                Checking = false;
                break;
            case BookSourceEvent.ServerAdded added:
                Added = (added.Id, added.Kind, added.Series);
                Checking = false;
                break;
            case BookSourceEvent.ServerFailed failed:
                Failed = (failed.Step, failed.Failure);
                Checking = false;
                break;
            case BookSourceEvent.ForWork forWork:
                ForWork[forWork.Work] = new WorkEditions(forWork.Editions, forWork.Asking, forWork.Missing);
                break;
            case BookSourceEvent.SourceSeries found:
                if (!SourceSeries.TryGetValue(found.Work, out var current) || current.SourceId != found.SourceId || current.Query != found.Query)
                    return;
                SourceSeries[found.Work] = new SourceSeriesSearch(found.SourceId, found.Query, found.Choices, found.Failure, false);
                break;
            default:
                return;
        }
        Changed?.Invoke();
    }
}

public static class ReadingWords
{
    /// <summary>A failure in plain words, from the engine's own messages.</summary>
    public static string Words(this ReadingFailure failure) => KaeriModelMethods.ReadingFailureMsg(failure).Text();

    /// <summary>The engine's parts of a line, joined with a middle dot.</summary>
    public static string Joined(this IEnumerable<Msg>? parts) => string.Join(" · ", (parts ?? []).Select(p => p.Text()).Where(t => t.Length > 0));
}
