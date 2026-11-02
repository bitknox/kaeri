// Every title the app has been told of, once, so every list and page shows a title as it now
// stands: a mark or a watch moves it everywhere at once.

package app.kaeri.store

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import app.kaeri.engine.core.AnilistId
import app.kaeri.engine.model.DubExtent
import app.kaeri.engine.model.Entry
import app.kaeri.engine.model.EpisodeProgress
import app.kaeri.engine.model.EpisodeRow
import app.kaeri.engine.model.LibrarySegment
import app.kaeri.engine.model.Msg
import app.kaeri.engine.model.ResumePoint
import app.kaeri.engine.model.TitleAired
import app.kaeri.engine.model.TitleProgress

/**
 * The titles by id, merged from every list, page and progress the engine sends. Lists hold ids
 * and read their rows from here.
 */
class TitleCache {
    private val _entries = MutableStateFlow<Map<AnilistId, Entry>>(emptyMap())
    val entries: StateFlow<Map<AnilistId, Entry>> = _entries.asStateFlow()

    private val _episodes = MutableStateFlow<Map<AnilistId, List<EpisodeRow>>>(emptyMap())

    /** Each title's episode table, as last loaded. */
    val episodes: StateFlow<Map<AnilistId, List<EpisodeRow>>> = _episodes.asStateFlow()

    private val _dubLines = MutableStateFlow<Map<AnilistId, DubExtent>>(emptyMap())

    /** How far each title's dub goes, where its source could say; [line] words it. */
    val dubLines: StateFlow<Map<AnilistId, DubExtent>> = _dubLines.asStateFlow()

    /** The titles whose page has been loaded, so their detail-only fields are known. */
    var detailed: Set<AnilistId> = emptySet()
        private set

    /**
     * The titles whose page's head came ahead of the page (a partial page): their title, art,
     * facts and synopsis are here, and the rest of the page is still on its way. A title is in
     * this or in [detailed], never both.
     */
    var headed: Set<AnilistId> = emptySet()
        private set

    /** Whether [id]'s head is here to show: its page, whole or only its head. */
    fun hasHead(id: AnilistId): Boolean = id in detailed || id in headed

    private val _failed = MutableStateFlow<Set<AnilistId>>(emptySet())

    /** The titles whose page was asked for and did not come: what "AniList did not answer" says. */
    val failed: StateFlow<Set<AnilistId>> = _failed.asStateFlow()

    /** The titles whose whole page was asked for and has not come yet. */
    private val pending = mutableSetOf<AnilistId>()

    internal fun detailAsked(id: AnilistId) {
        pending += id
        if (id in _failed.value) _failed.value = _failed.value - id
    }

    /** A page's fetch ended: without the whole page, AniList did not answer, whatever head came first. */
    internal fun detailEnded(id: AnilistId) {
        if (pending.remove(id)) _failed.value = _failed.value + id
    }

    operator fun get(id: AnilistId): Entry? = _entries.value[id]

    private fun update(change: (MutableMap<AnilistId, Entry>) -> Unit) {
        _entries.value = _entries.value.toMutableMap().also(change)
    }

    /**
     * A list's rows. A list carries no detail-only fields, so a page's stay where one was loaded,
     * and so does a known list status the list does not know.
     */
    internal fun merge(list: List<Entry>, local: Boolean = false) = update { entries ->
        for (entry in list) {
            val kept = entries[entry.id]
            if (kept == null) {
                entries[entry.id] = entry
                continue
            }
            var merged = entry.copy(
                listStatus = entry.listStatus ?: kept.listStatus,
                // A list arrives ahead of the broadcast line its own follow-up brings.
                lastAired = entry.lastAired ?: kept.lastAired,
            )
            // Only Continue's list speaks for what this device watched. A tracker's list (the
            // Library after sign-in) may say 12 of 12 or none, and must not turn E7 at 10:06 into E1.
            if (!local) merged = merged.copy(progress = kept.progress ?: entry.progress, resume = kept.resume ?: entry.resume, watchedIn = kept.watchedIn ?: entry.watchedIn)
            if (entry.id in detailed || entry.id in headed) {
                merged = merged.copy(
                    related = kept.related,
                    recommended = kept.recommended,
                    trailerUrl = kept.trailerUrl,
                    availableOn = kept.availableOn,
                    // The page's own fields, its seasons among them: a list refreshed under an open
                    // page would otherwise take its picker and related titles away.
                    detail = kept.detail,
                )
            }
            entries[entry.id] = merged
        }
    }

    /**
     * A title's page: whole, or only its head ahead of it (`detail.partial`). A head is kept until
     * the whole page replaces it, and is no use once the whole page is here; the whole page is
     * still awaited after it.
     */
    internal fun merge(detail: Entry) {
        val partial = detail.detail?.partial == true
        if (partial && detail.id in detailed) return
        update { entries ->
            val kept = entries[detail.id]
            entries[detail.id] = if (kept == null) detail else detail.copy(
                listStatus = detail.listStatus ?: kept.listStatus,
                lastAired = detail.lastAired ?: kept.lastAired,
            )
        }
        if (partial) {
            headed = headed + detail.id
            return
        }
        headed = headed - detail.id
        detailed = detailed + detail.id
        pending -= detail.id
        if (detail.id in _failed.value) _failed.value = _failed.value - detail.id
    }

    /** Where a title stands after something moved it. */
    internal fun progress(
        id: AnilistId,
        progress: TitleProgress?,
        resume: ResumePoint?,
        rows: List<EpisodeProgress>,
    ) {
        update { entries -> entries[id]?.let { entries[id] = it.copy(progress = progress, resume = resume) } }
        if (rows.isEmpty()) return
        val table = _episodes.value[id] ?: return
        val marked = rows.associateBy { it.number }
        _episodes.value = _episodes.value + (id to table.map { row ->
            marked[row.number]?.let { row.copy(watched = it.watched, completed = it.completed) } ?: row
        })
    }

    /** The latest broadcast of each airing title, said after the list it annotates. */
    internal fun lastAired(rows: List<TitleAired>) = update { entries ->
        for (row in rows) {
            entries[row.id]?.let { entries[row.id] = it.copy(lastAired = row.aired) }
        }
    }

    internal fun episodes(id: AnilistId, rows: List<EpisodeRow>, failed: Msg? = null) {
        _episodes.value = _episodes.value + (id to rows)
        _episodesFailed.value = if (failed == null) _episodesFailed.value - id else _episodesFailed.value + (id to failed)
    }

    private val _episodesFailed = MutableStateFlow<Map<AnilistId, Msg>>(emptyMap())

    /** Why a title's episodes could not be loaded at all, for its page to say instead of "no episodes". */
    val episodesFailed: StateFlow<Map<AnilistId, Msg>> = _episodesFailed.asStateFlow()

    /** How far [id]'s dub goes, worded for the page; nothing when there is none to say. */
    internal fun dub(id: AnilistId, extent: DubExtent?) {
        _dubLines.value = if (extent == null) _dubLines.value - id else _dubLines.value + (id to extent)
    }

    /**
     * Whether [id]'s episodes, as last loaded, are AniList's alone: no source set up has it, so it
     * can be marked watched but there is nothing to play or download it from.
     */
    fun unlisted(id: AnilistId): Boolean = _episodes.value[id]?.firstOrNull()?.metadataOnly == true

    /** The titles whose episodes, as last loaded, are AniList's alone. */
    internal fun unlistedTitles(): List<AnilistId> =
        _episodes.value.filterValues { it.firstOrNull()?.metadataOnly == true }.keys.toList()

    /** A title put on a list here, ahead of the tracker's word. */
    fun setStatus(id: AnilistId, segment: LibrarySegment) = update { entries ->
        entries[id]?.let { entries[id] = it.copy(listStatus = segment) }
    }
}

/** The title page's line on its dub: "Dubbed to episode 6", or "Dubbed" when every aired episode is. */
fun DubExtent.line(context: android.content.Context): String =
    if (complete) app.kaeri.i18n.Strings.audioDubbed(context) else app.kaeri.i18n.Strings.titleDubbedTo(context, through.toInt())
