// Reading's places as the engine sends them: what Kaeri is for, Home, Updates, the lists, Search,
// a series and its chapters, the downloads, the book sources and Send to e-reader. The screens draw
// what is here and ask through these models; nothing is decided on this side.

package app.kaeri.store

import app.kaeri.engine.core.ChapterId
import app.kaeri.engine.core.WorkId
import app.kaeri.engine.ffi.readFilters
import app.kaeri.engine.model.BookDownloadsView
import app.kaeri.engine.model.BookEditionRow
import app.kaeri.engine.model.BookServerFailure
import app.kaeri.engine.model.BookServerKind
import app.kaeri.engine.model.BookServerRow
import app.kaeri.engine.model.BookSourceCommand
import app.kaeri.engine.model.BookSourceEvent
import app.kaeri.engine.model.BookSourcesView
import app.kaeri.engine.model.ChapterVersion
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.DownloadEstimate
import app.kaeri.engine.model.DownloadPick
import app.kaeri.engine.model.EditionMiss
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.ExportFile
import app.kaeri.engine.model.ExportFormat
import app.kaeri.engine.model.ExportTarget
import app.kaeri.engine.model.FolderRef
import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.engine.model.LibrarySegmentRead
import app.kaeri.engine.model.ListRemoval
import app.kaeri.engine.model.MatchChoice
import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.MediumAsk
import app.kaeri.engine.model.MediumCommand
import app.kaeri.engine.model.MediumView
import app.kaeri.engine.model.Msg
import app.kaeri.engine.model.NewChapter
import app.kaeri.engine.model.ReadFilters
import app.kaeri.engine.model.ReadSearchQuery
import app.kaeri.engine.model.ReaderCommand
import app.kaeri.engine.model.ReaderEvent
import app.kaeri.engine.model.ReaderSettingKey
import app.kaeri.engine.model.ReaderSettingsView
import app.kaeri.engine.model.ReaderValue
import app.kaeri.engine.model.ReadingAbout
import app.kaeri.engine.model.ReadingCommand
import app.kaeri.engine.model.ReadingEvent
import app.kaeri.engine.model.ReadingFailure
import app.kaeri.engine.model.ReadingHomeView
import app.kaeri.engine.model.SeriesView
import app.kaeri.engine.model.ServerStep
import app.kaeri.engine.model.SettingScope
import app.kaeri.engine.model.SourceSeriesChoice
import app.kaeri.engine.model.TrackerReadRow
import app.kaeri.engine.model.UpdatesView
import app.kaeri.engine.model.VolumeGroup
import app.kaeri.engine.model.WorkTile
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update

/** Watch, read or both: the device's answer, the build's edition and the question waiting. */
class MediumModel(private val send: (Command) -> Unit) {
    private val _view = MutableStateFlow<MediumView?>(null)

    /** The engine's view, from the first `Medium` event on. */
    val view: StateFlow<MediumView?> = _view.asStateFlow()

    private val _asking = MutableStateFlow<MediumAsk?>(null)

    /** The question on screen: held from the engine's ask until it is answered. */
    val asking: StateFlow<MediumAsk?> = _asking.asStateFlow()

    /** What Kaeri is for on this device: Watch until the engine says. */
    val answer: KaeriIsFor get() = _view.value?.answer ?: KaeriIsFor.WATCH

    /** Whether the person can choose: reading is in the build and the edition does not fix it. */
    val choosable: Boolean get() = _view.value?.let { it.readingAvailable && !it.fixed } ?: false

    /** A choice a Fixtures launch makes as Settings would, once the first view is there. */
    internal var launchChoice: KaeriIsFor? = null

    internal fun apply(view: MediumView) {
        _view.value = view
        val ask = view.ask
        if (ask == null) _asking.value = null else if (_asking.value == null) _asking.value = ask
        launchChoice?.let { wanted ->
            launchChoice = null
            if (choosable && view.answer != wanted) choose(wanted)
        }
    }

    fun load() = send(Command.Medium(MediumCommand.Load))

    /** A Fixtures launch's choice, made as Settings would: now, or once the first view is there. */
    fun chooseAtLaunch(answer: KaeriIsFor) {
        val view = _view.value
        if (view == null) {
            launchChoice = answer
        } else if (choosable && view.answer != answer) {
            choose(answer)
        }
    }

    fun choose(answer: KaeriIsFor, deleteDownloads: Boolean = false) =
        send(Command.Medium(MediumCommand.Choose(answer, deleteDownloads)))

    fun answer(ask: MediumAsk, answer: KaeriIsFor) {
        _asking.value = null
        send(Command.Medium(MediumCommand.Answer(ask, answer)))
    }
}

/** Whether the answer reads. */
val KaeriIsFor.reads: Boolean get() = this == KaeriIsFor.READ || this == KaeriIsFor.BOTH

/** Whether the answer watches. */
val KaeriIsFor.watches: Boolean get() = this == KaeriIsFor.WATCH || this == KaeriIsFor.BOTH

/** A series' chapters for the edition in use, or why they did not load. */
data class ReadChapters(
    val edition: BookEditionRow?,
    val volumes: List<VolumeGroup>,
    val failure: ReadingFailure?,
)

/** Reading's Search: the query asked, the pages answered so far and how many there are. */
data class ReadSearchState(
    val query: ReadSearchQuery = ReadingModel.EMPTY_QUERY,
    val tiles: List<WorkTile> = emptyList(),
    val page: UInt = 0u,
    val hasMore: Boolean = false,
    val otherScope: UInt? = null,
    val total: UInt? = null,
    val loading: Boolean = false,
    val failure: ReadingFailure? = null,
)

/** A finished download batch, said as a notification while Downloads finished is on. */
data class DownloadsDone(val work: WorkId?, val title: Msg, val body: Msg)

/** Reading's places: Home, Updates, the lists, Search, series, chapters, tracking and downloads. */
class ReadingModel(private val send: (Command) -> Unit) {
    private val _home = MutableStateFlow<ReadingHomeView?>(null)
    val home: StateFlow<ReadingHomeView?> = _home.asStateFlow()

    private val _homeLoading = MutableStateFlow(false)
    val homeLoading: StateFlow<Boolean> = _homeLoading.asStateFlow()

    private val _updates = MutableStateFlow<UpdatesView?>(null)
    val updates: StateFlow<UpdatesView?> = _updates.asStateFlow()

    private val _lists = MutableStateFlow<Map<LibrarySegmentRead, List<WorkTile>>>(emptyMap())

    /** Each list's tiles, once loaded: a list missing from the map has not answered yet. */
    val lists: StateFlow<Map<LibrarySegmentRead, List<WorkTile>>> = _lists.asStateFlow()

    private val _search = MutableStateFlow(ReadSearchState())
    val search: StateFlow<ReadSearchState> = _search.asStateFlow()

    private val _series = MutableStateFlow<Map<WorkId, SeriesView>>(emptyMap())
    val series: StateFlow<Map<WorkId, SeriesView>> = _series.asStateFlow()

    private val _chapters = MutableStateFlow<Map<WorkId, ReadChapters>>(emptyMap())
    val chapters: StateFlow<Map<WorkId, ReadChapters>> = _chapters.asStateFlow()

    private val _versions = MutableStateFlow<Map<ChapterId, List<ChapterVersion>>>(emptyMap())
    val versions: StateFlow<Map<ChapterId, List<ChapterVersion>>> = _versions.asStateFlow()

    private val _tracking = MutableStateFlow<Map<WorkId, List<TrackerReadRow>>>(emptyMap())
    val tracking: StateFlow<Map<WorkId, List<TrackerReadRow>>> = _tracking.asStateFlow()

    private val _downloads = MutableStateFlow<BookDownloadsView?>(null)
    val downloads: StateFlow<BookDownloadsView?> = _downloads.asStateFlow()

    private val _estimates = MutableStateFlow<Map<WorkId, DownloadEstimate>>(emptyMap())

    /** The size line of a pick, per series, while chapters are being chosen. */
    val estimates: StateFlow<Map<WorkId, DownloadEstimate>> = _estimates.asStateFlow()

    private val _removals = MutableStateFlow<Map<WorkId, List<ListRemoval>>>(emptyMap())

    /** What Remove from lists did on each tracker, per series, once it answered. */
    val removals: StateFlow<Map<WorkId, List<ListRemoval>>> = _removals.asStateFlow()

    private val _failures = MutableStateFlow<Map<ReadingAbout, ReadingFailure>>(emptyMap())
    val failures: StateFlow<Map<ReadingAbout, ReadingFailure>> = _failures.asStateFlow()

    private val _readerDefaults = MutableStateFlow<Map<Medium, ReaderSettingsView>>(emptyMap())

    /** The reader's defaults per medium, for Settings' Reading pane. */
    val readerDefaults: StateFlow<Map<Medium, ReaderSettingsView>> = _readerDefaults.asStateFlow()

    private val _newChapters = MutableSharedFlow<List<NewChapter>>(extraBufferCapacity = 4)

    /** New chapters a check found, for the app's notifications. */
    val newChapters: SharedFlow<List<NewChapter>> = _newChapters.asSharedFlow()

    private val _downloadsDone = MutableSharedFlow<DownloadsDone>(extraBufferCapacity = 4)

    /** A finished download batch, for the app's Downloads finished notification. */
    val downloadsDone: SharedFlow<DownloadsDone> = _downloadsDone.asSharedFlow()

    /** What Search's filters choose from, fixed so they show before anything loads. */
    val filters: ReadFilters by lazy { readFilters() }

    /** Unread new chapters, for the Updates badge. */
    val unread: Int get() = _updates.value?.unread?.toInt() ?: 0

    // Asking

    fun loadHome(refresh: Boolean = false) {
        _homeLoading.value = true
        _failures.update { it - ReadingAbout.HOME }
        reading(ReadingCommand.LoadHome(refresh))
    }

    fun loadUpdates() = reading(ReadingCommand.LoadUpdates)
    fun checkUpdates() = reading(ReadingCommand.CheckUpdates)
    fun downloadNew() = reading(ReadingCommand.DownloadNew)
    fun loadList(segment: LibrarySegmentRead) = reading(ReadingCommand.LoadLibrary(segment))

    fun runSearch(query: ReadSearchQuery) {
        _search.value = ReadSearchState(query = query, loading = true)
        reading(ReadingCommand.Search(query))
    }

    fun searchMore() {
        val now = _search.value
        if (!now.hasMore || now.loading) return
        _search.value = now.copy(loading = true)
        reading(ReadingCommand.SearchMore(now.query, now.page + 1u))
    }

    /** A series page: its view and its chapters. */
    fun loadSeries(work: WorkId) {
        _failures.update { it - ReadingAbout.SERIES }
        reading(ReadingCommand.LoadSeries(work))
        reading(ReadingCommand.LoadChapters(work, null))
    }

    /** A series' view alone, when it is not held yet. */
    fun loadSeriesInfo(work: WorkId) {
        if (_series.value[work] == null) reading(ReadingCommand.LoadSeries(work))
    }

    fun loadChapters(work: WorkId, edition: String? = null) = reading(ReadingCommand.LoadChapters(work, edition))
    fun loadVersions(work: WorkId, chapter: ChapterId) = reading(ReadingCommand.LoadVersions(work, chapter))

    fun chooseEdition(work: WorkId, edition: BookEditionRow) {
        reading(ReadingCommand.ChooseEdition(work, edition.sourceId, edition.versionLabel, edition.language))
        refreshSeries(work)
    }

    fun markRead(work: WorkId, chapters: List<ChapterId>, read: Boolean) {
        reading(ReadingCommand.MarkRead(work, chapters, read))
        refreshSeries(work)
    }

    fun markReadUpTo(work: WorkId, chapter: ChapterId) {
        reading(ReadingCommand.MarkReadUpTo(work, chapter))
        refreshSeries(work)
    }

    fun readAgain(work: WorkId) {
        reading(ReadingCommand.ReadAgain(work))
        refreshSeries(work)
    }

    fun undoReadAgain(work: WorkId) {
        reading(ReadingCommand.UndoReadAgain(work))
        refreshSeries(work)
    }

    fun bookmark(work: WorkId, chapter: ChapterId, on: Boolean) {
        reading(ReadingCommand.Bookmark(work, chapter, on))
        reloadChapters(work)
    }

    fun hideFromContinue(work: WorkId) = reading(ReadingCommand.HideFromContinue(work))

    fun setStatus(work: WorkId, status: LibrarySegmentRead) {
        reading(ReadingCommand.SetStatus(work, status))
        refreshSeries(work)
    }

    fun removeFromLists(work: WorkId) {
        _removals.update { it - work }
        reading(ReadingCommand.RemoveFromLists(work))
    }

    /** The removal's results have been seen. */
    fun removalSeen(work: WorkId) = _removals.update { it - work }

    fun setScore(work: WorkId, score: Float) = reading(ReadingCommand.SetScore(work, score))
    fun loadTracking(work: WorkId) = reading(ReadingCommand.LoadTracking(work))
    fun chooseMapping(work: WorkId, tracker: String, id: String?) = reading(ReadingCommand.ChooseMapping(work, tracker, id))
    fun resolveConflict(work: WorkId, tracker: String, keepLocal: Boolean) =
        reading(ReadingCommand.ResolveConflict(work, tracker, keepLocal))

    fun connectMangaUpdates(username: String, password: String) =
        reading(ReadingCommand.ConnectMangaUpdates(username, password))

    fun download(work: WorkId, pick: DownloadPick) {
        reading(ReadingCommand.Download(work, pick))
        reloadChapters(work)
    }

    fun cancelDownloads(work: WorkId, chapters: List<ChapterId>) {
        reading(ReadingCommand.DownloadCancel(work, chapters))
        reloadChapters(work)
    }

    fun deleteDownloads(work: WorkId, chapters: List<ChapterId>) {
        reading(ReadingCommand.DownloadDelete(work, chapters))
        reloadChapters(work)
        if (_downloads.value != null) reading(ReadingCommand.LoadDownloads)
    }

    fun deleteRead(work: WorkId) {
        reading(ReadingCommand.DownloadDeleteRead(work))
        reading(ReadingCommand.LoadDownloads)
    }

    /** The size line of [pick]; an empty pick clears it. */
    fun estimateDownload(work: WorkId, pick: DownloadPick) {
        if (pick is DownloadPick.Chapters && pick.ids.isEmpty()) {
            _estimates.update { it - work }
            return
        }
        reading(ReadingCommand.EstimateDownload(work, pick))
    }

    fun clearEstimate(work: WorkId) = _estimates.update { it - work }

    fun loadDownloads() = reading(ReadingCommand.LoadDownloads)

    /** Where the person stopped on an e-reader the chapters were sent to; `null` chapter for "Not yet". */
    fun stoppedAt(work: WorkId, chapter: ChapterId?, page: UInt?) {
        reading(ReadingCommand.StoppedAt(work, chapter, page))
        refreshSeries(work)
    }

    fun loadReaderDefaults() {
        send(Command.Reader(ReaderCommand.LoadSettings(Medium.MANGA, null)))
        send(Command.Reader(ReaderCommand.LoadSettings(Medium.NOVEL, null)))
    }

    fun setReaderDefault(medium: Medium, key: ReaderSettingKey, value: ReaderValue) =
        send(Command.Reader(ReaderCommand.SetSetting(null, medium, null, key, SettingScope.MEDIUM, value)))

    private fun reading(command: ReadingCommand) = send(Command.Reading(command))

    private fun reloadChapters(work: WorkId) = reading(ReadingCommand.LoadChapters(work, _chapters.value[work]?.edition?.sourceId))

    private fun refreshSeries(work: WorkId) {
        if (_series.value[work] != null) {
            reading(ReadingCommand.LoadSeries(work))
            reloadChapters(work)
        }
        // Updates' rows turn haze once read; Home's Continue moves.
        if (_updates.value != null) reading(ReadingCommand.LoadUpdates)
    }

    // Hearing

    internal fun observe(event: Event) {
        if (event is Event.Reader) {
            val reader = event.v1
            if (reader is ReaderEvent.Settings && reader.v1.work == null) {
                _readerDefaults.update { it + (reader.v1.medium to reader.v1) }
            }
            return
        }
        if (event is Event.Reading) apply(event.v1)
    }

    internal fun apply(event: ReadingEvent) {
        when (event) {
            is ReadingEvent.Home -> {
                _home.value = stage?.invoke(event.v1) ?: event.v1
                _homeLoading.value = false
            }
            is ReadingEvent.Updates -> _updates.value = event.v1
            is ReadingEvent.Library -> _lists.update { it + (event.segment to event.tiles) }
            is ReadingEvent.Search -> {
                val now = _search.value
                if (event.query != now.query) return
                val first = event.page == 0u
                _search.value = now.copy(
                    tiles = if (first) event.tiles else now.tiles + event.tiles,
                    page = event.page,
                    hasMore = event.hasMore,
                    otherScope = event.otherScope,
                    total = event.total ?: if (first) null else now.total,
                    loading = false,
                    failure = null,
                )
            }
            is ReadingEvent.Series -> _series.update { it + (event.v1.work to event.v1) }
            is ReadingEvent.Chapters -> _chapters.update { it + (event.work to ReadChapters(event.edition, event.volumes, event.failure)) }
            is ReadingEvent.Versions -> _versions.update { it + (event.chapter to event.versions) }
            is ReadingEvent.Tracking -> _tracking.update { it + (event.work to event.rows) }
            is ReadingEvent.Downloads -> _downloads.value = event.v1
            is ReadingEvent.DownloadEstimate -> _estimates.update { it + (event.v1.work to event.v1) }
            is ReadingEvent.RemovedFromLists -> {
                _removals.update { it + (event.work to event.results) }
                refreshSeries(event.work)
            }
            is ReadingEvent.NewChapters -> _newChapters.tryEmit(event.v1)
            is ReadingEvent.DownloadsDone -> _downloadsDone.tryEmit(DownloadsDone(event.work, event.title, event.body))
            is ReadingEvent.Failed -> {
                _failures.update { it + (event.about to event.failure) }
                if (event.about == ReadingAbout.HOME) _homeLoading.value = false
                if (event.about == ReadingAbout.SEARCH) _search.update { it.copy(loading = false, failure = event.failure) }
            }
            else -> {}
        }
    }

    /** A change a Fixtures shot makes to every Home the engine sends: the new-chapter spotlight. */
    internal var stage: ((ReadingHomeView) -> ReadingHomeView)? = null
        set(value) {
            field = value
            val home = _home.value
            if (value != null && home != null) _home.value = value(home)
        }

    companion object {
        val EMPTY_QUERY = ReadSearchQuery(
            text = "",
            medium = null,
            formats = emptyList(),
            origins = emptyList(),
            status = null,
            genres = emptyList(),
            excludeGenres = emptyList(),
            sort = null,
        )
    }
}

/** A series' editions across the sources, while Read from asks them. */
data class WorkEditions(
    val editions: List<BookEditionRow>,
    val asking: Boolean,
    val missing: List<EditionMiss> = emptyList(),
)

/** Read from's Search there: one source searched for a series it did not match. */
data class SourceSeriesSearch(
    val sourceId: String,
    val query: String,
    val choices: List<SourceSeriesChoice> = emptyList(),
    val failure: ReadingFailure? = null,
    val searching: Boolean = true,
)

/** The Add a server sheet's answer: the server it found, the one it added or why not. */
sealed interface ServerAnswer {
    data class Checked(val address: String, val kind: BookServerKind, val name: String, val version: String) : ServerAnswer
    data class Added(val id: String, val kind: BookServerKind, val series: UInt?) : ServerAnswer
    data class Failed(val step: ServerStep, val failure: BookServerFailure) : ServerAnswer
}

/** The book sources: folders, servers and plugins, the Add a server sheet, and Read from's editions. */
class BookSourcesModel(private val send: (Command) -> Unit) {
    /** Told the servers each time the view changes, for their progress lines. */
    internal var onServers: (List<BookServerRow>) -> Unit = {}

    private val _view = MutableStateFlow<BookSourcesView?>(null)
    val view: StateFlow<BookSourcesView?> = _view.asStateFlow()

    private val _server = MutableStateFlow<ServerAnswer?>(null)

    /** The Add a server sheet's last answer. */
    val server: StateFlow<ServerAnswer?> = _server.asStateFlow()

    private val _checking = MutableStateFlow(false)
    val checking: StateFlow<Boolean> = _checking.asStateFlow()

    private val _matches = MutableStateFlow<Map<String, List<MatchChoice>>>(emptyMap())
    val matches: StateFlow<Map<String, List<MatchChoice>>> = _matches.asStateFlow()

    private val _forWork = MutableStateFlow<Map<WorkId, WorkEditions>>(emptyMap())
    val forWork: StateFlow<Map<WorkId, WorkEditions>> = _forWork.asStateFlow()

    private val _sourceSeries = MutableStateFlow<Map<WorkId, SourceSeriesSearch>>(emptyMap())
    val sourceSeries: StateFlow<Map<WorkId, SourceSeriesSearch>> = _sourceSeries.asStateFlow()

    /** Whether there are no book sources at all. */
    val isEmpty: Boolean get() = _view.value?.let { it.folders.isEmpty() && it.servers.isEmpty() && it.plugins.isEmpty() } ?: true

    fun load() = books(BookSourceCommand.Load)
    fun addFolder(folder: FolderRef) = books(BookSourceCommand.AddFolder(folder))
    fun removeFolder(id: String) = books(BookSourceCommand.RemoveFolder(id))
    fun restoreFolder(id: String) = books(BookSourceCommand.RestoreFolder(id))
    fun scan() = books(BookSourceCommand.Scan)
    fun removeServer(id: String) = books(BookSourceCommand.RemoveServer(id))
    fun setOff(id: String, off: Boolean) = books(BookSourceCommand.SetOff(id, off))
    fun retryProgress(id: String) = books(BookSourceCommand.RetryProgress(id))
    fun setReportProgress(id: String, on: Boolean) = books(BookSourceCommand.SetReportProgress(id, on))
    fun setOrder(ids: List<String>) = books(BookSourceCommand.SetOrder(ids))
    fun findMatch(group: String, query: String) = books(BookSourceCommand.FindMatch(group, query))
    fun chooseMatch(group: String, anilist: UInt) = books(BookSourceCommand.ChooseMatch(group, anilist))
    fun keep(group: String) = books(BookSourceCommand.Keep(group))

    fun loadForWork(work: WorkId) {
        _forWork.update {
            val held = it[work]
            it + (work to WorkEditions(held?.editions ?: emptyList(), asking = true, missing = held?.missing ?: emptyList()))
        }
        books(BookSourceCommand.LoadForWork(work))
    }

    fun searchSource(work: WorkId, sourceId: String, query: String) {
        _sourceSeries.update { it + (work to SourceSeriesSearch(sourceId, query)) }
        books(BookSourceCommand.SearchSourceForWork(work, sourceId, query))
    }

    fun chooseSourceSeries(work: WorkId, sourceId: String, seriesRef: String) {
        _sourceSeries.update { it - work }
        _forWork.update { map -> map[work]?.let { map + (work to it.copy(asking = true)) } ?: map }
        books(BookSourceCommand.ChooseSourceSeries(work, sourceId, seriesRef))
    }

    fun clearSourceSearch(work: WorkId) = _sourceSeries.update { it - work }

    fun checkServer(address: String) {
        _server.value = null
        _checking.value = true
        books(BookSourceCommand.CheckServer(address))
    }

    fun signIn(address: String, user: String, password: String) {
        _checking.value = true
        books(BookSourceCommand.SignIn(address, user, password))
    }

    /** The Add a server sheet closed or started over. */
    fun resetServerSheet() {
        _server.value = null
        _checking.value = false
    }

    /** Misses a Fixtures shot shows under Read from when the engine has none: Not found in. */
    internal var stagedMisses: Map<WorkId, List<EditionMiss>> = emptyMap()

    internal fun apply(event: BookSourceEvent) {
        when (event) {
            is BookSourceEvent.View -> {
                _view.value = event.v1
                onServers(event.v1.servers)
            }
            is BookSourceEvent.Matches -> _matches.update { it + (event.group to event.choices) }
            is BookSourceEvent.ServerChecked -> {
                _server.value = ServerAnswer.Checked(event.address, event.kind, event.name, event.version)
                _checking.value = false
            }
            is BookSourceEvent.ServerAdded -> {
                _server.value = ServerAnswer.Added(event.id, event.kind, event.series)
                _checking.value = false
            }
            is BookSourceEvent.ServerFailed -> {
                _server.value = ServerAnswer.Failed(event.step, event.failure)
                _checking.value = false
            }
            is BookSourceEvent.ForWork -> {
                val missing = event.missing.ifEmpty { stagedMisses[event.work].orEmpty() }
                _forWork.update { it + (event.work to WorkEditions(event.editions, event.asking, missing)) }
            }
            is BookSourceEvent.SourceSeries -> _sourceSeries.update { map ->
                val now = map[event.work]
                if (now == null || now.sourceId != event.sourceId || now.query != event.query) map
                else map + (event.work to SourceSeriesSearch(event.sourceId, event.query, event.choices, event.failure, searching = false))
            }
        }
    }

    internal fun showServer(answer: ServerAnswer) {
        _server.value = answer
    }

    private fun books(command: BookSourceCommand) = send(Command.BookSources(command))
}

/**
 * One Send to e-reader: preparing, waiting for Wi-Fi, ready to share, or failed. A send outlives
 * the sheet that asked for it: closing the sheet leaves it going, and the series page and
 * Downloads show it until it is shared or put away.
 */
sealed interface ExportJob {
    val job: ULong
    val work: WorkId

    /** The e-reader it is for, by id. */
    val target: String

    /** [done] of [total] steps, and [chaptersDone] of [chapters] gathered; zero [chapters] while the engine has not said. */
    data class Preparing(
        override val job: ULong,
        override val work: WorkId,
        override val target: String,
        val done: UInt,
        val total: UInt,
        val chaptersDone: UInt = 0u,
        val chapters: UInt = 0u,
    ) : ExportJob {
        /** How far, from 0 to 1, or `null` before the engine says. */
        val fraction: Float? get() = if (total > 0u) (done.toFloat() / total.toFloat()).coerceIn(0f, 1f) else null
    }

    /** Held for Wi-Fi: goes on by itself once the network allows; [line] says why. */
    data class Waiting(override val job: ULong, override val work: WorkId, override val target: String, val line: Msg) : ExportJob
    data class Ready(override val job: ULong, override val work: WorkId, override val target: String, val chapters: List<ChapterId>, val files: List<ExportFile>) : ExportJob

    /** [chapter] is the one it could not prepare, when it was one. */
    data class Failed(
        override val job: ULong,
        override val work: WorkId,
        override val target: String,
        val failure: ReadingFailure,
        val chapter: String? = null,
    ) : ExportJob

    /** Still going: preparing or held for Wi-Fi. */
    val running: Boolean get() = this is Preparing || this is Waiting
}

/** Send to e-reader: the devices to prepare for, and each send under way or ready to share. */
class ExportModel(private val send: (Command) -> ULong?) {
    private val _targets = MutableStateFlow<List<ExportTarget>>(emptyList())
    val targets: StateFlow<List<ExportTarget>> = _targets.asStateFlow()

    private val _chosen = MutableStateFlow<String?>(null)

    /** The e-reader the sheet starts on: the one last sent to on this device, as the engine keeps it. */
    val chosen: StateFlow<String?> = _chosen.asStateFlow()

    private val _jobs = MutableStateFlow<Map<ULong, ExportJob>>(emptyMap())

    /** The sends this session knows of, by job: those it started and any held across a restart. */
    val jobs: StateFlow<Map<ULong, ExportJob>> = _jobs.asStateFlow()

    /** What each job was asked to send, for its Ready and the Shared it reports. */
    private val asked = mutableMapOf<ULong, Triple<WorkId, String, List<ChapterId>>>()

    fun loadTargets() {
        send(Command.Reading(ReadingCommand.LoadExportTargets))
    }

    /** Keep [target] as the e-reader the sheet starts on next time. */
    fun choose(target: String) {
        if (_chosen.value == target) return
        _chosen.value = target
        send(Command.Reading(ReadingCommand.SetSendTarget(target)))
    }

    /** Prepare [chapters] of [work] for [target] in [format]; the job, or `null` without an engine. */
    fun prepare(work: WorkId, chapters: List<ChapterId>, target: String, format: ExportFormat): ULong? {
        choose(target)
        val job = send(Command.Reading(ReadingCommand.Prepare(work, DownloadPick.Chapters(chapters), target, format))) ?: return null
        asked[job] = Triple(work, target, chapters)
        _jobs.update { it + (job to ExportJob.Preparing(job, work, target, 0u, chapters.size.toUInt())) }
        return job
    }

    /** Stop a send that is going, held ones too, which the engine then forgets for the next launch. */
    fun cancel(job: ULong) {
        send(Command.Reading(ReadingCommand.CancelPrepare(job)))
        forget(job)
    }

    /** Jobs whose files went to the share sheet, told to the engine once each. */
    private val told = mutableSetOf<ULong>()

    /**
     * The files went to the share sheet: the engine keeps where they went for "Where did you
     * stop?", once. The send stays, so Share again can open the sheet again, until it is put away.
     */
    fun shared(job: ULong) {
        val ready = _jobs.value[job] as? ExportJob.Ready ?: return
        if (!told.add(job)) return
        send(Command.Reading(ReadingCommand.Shared(ready.work, ready.chapters, ready.target)))
    }

    /** Whether [job]'s files went to the share sheet already. */
    fun wasShared(job: ULong): Boolean = job in told

    /** Put away a send that has ended, ready or failed. */
    fun forget(job: ULong) {
        asked -= job
        told -= job
        _jobs.update { it - job }
    }

    /** A job ended in the engine without its own answer: it failed, or was stopped. */
    internal fun ended(job: ULong, failure: ReadingFailure?) {
        val held = _jobs.value[job] ?: return
        if (held is ExportJob.Ready || held is ExportJob.Failed) return
        if (failure == null) forget(job) else _jobs.update { it + (job to ExportJob.Failed(job, held.work, held.target, failure)) }
    }

    internal fun apply(event: ReadingEvent) {
        when (event) {
            is ReadingEvent.ExportTargets -> {
                _targets.value = event.v1
                event.v1.firstOrNull { it.chosen }?.let { _chosen.value = it.id }
            }
            is ReadingEvent.ExportProgress -> {
                val (work, target) = asked[event.job] ?: return
                _jobs.update {
                    it + (event.job to ExportJob.Preparing(event.job, work, target, event.done, event.total, event.chaptersDone, event.chapters))
                }
            }
            is ReadingEvent.ExportWaiting -> {
                if (event.job !in asked) asked[event.job] = Triple(event.work, event.target, emptyList())
                _jobs.update { it + (event.job to ExportJob.Waiting(event.job, event.work, event.target, event.line)) }
            }
            is ReadingEvent.ExportReady -> {
                val (work, target, chapters) = asked[event.job] ?: return
                _jobs.update { it + (event.job to ExportJob.Ready(event.job, work, target, chapters, event.files)) }
            }
            is ReadingEvent.ExportFailed -> {
                val (work, target) = asked[event.job] ?: return
                _jobs.update { it + (event.job to ExportJob.Failed(event.job, work, target, event.failure, event.chapter)) }
            }
            else -> {}
        }
    }
}
