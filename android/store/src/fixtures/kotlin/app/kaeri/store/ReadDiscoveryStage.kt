// The discovery board's Search states, staged for shots on the fixtures engine: the testing engine
// has no sources that answer a search slowly, fail or time out, and no AniList that fails on cue.
// Each state runs a real search first and waits for the engine's own answer, then says the board's
// answer over it with the engine's own events, so the store and the screens take it as they would
// from the engine. Covers are the fixtures' own; titles are made up.

package app.kaeri.store

import app.kaeri.engine.model.Event
import app.kaeri.engine.model.ReadAnilistState
import app.kaeri.engine.model.ReadSearchQuery
import app.kaeri.engine.model.ReadSourceGroup
import app.kaeri.engine.model.ReadSourceJoin
import app.kaeri.engine.model.ReadSourceSearch
import app.kaeri.engine.model.ReadSourceState
import app.kaeri.engine.model.ReadingAbout
import app.kaeri.engine.model.ReadingEvent
import app.kaeri.engine.model.ReadingFailure
import app.kaeri.engine.model.WorkTile
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull

object ReadDiscoveryStage {
    val STATES: List<String> = listOf("rd-search-mid", "rd-search-unreachable", "rd-search-sources-only", "rd-search-nothing", "rd-search-done")

    /** The words every state searches for. */
    const val WORDS = "lantern" // i18n-skip

    /** Made-up sources, as Your sources would list them. */
    private const val HOME_LIBRARY = "Home library" // i18n-skip
    private const val COMICS = "Comics folder" // i18n-skip
    private const val NORTHWIND = "Northwind" // i18n-skip
    private const val DEMO = "Demo" // i18n-skip

    /**
     * Stage [state]: Search runs the words and, once the engine has answered, gets the board's
     * answer. Covers come from [art], series the engine already has (Home's), as many as needed.
     */
    suspend fun stage(store: AppStore, state: String): Boolean {
        if (state !in STATES) return false
        val query = ReadingModel.EMPTY_QUERY.copy(text = WORDS, sourcesOnly = state == "rd-search-sources-only")
        store.reading.runSearch(query)
        // The engine's own answer first, so the staged one is the last word.
        waitFor { store.reading.search.value.let { !it.loading && (it.sources == null || it.sources.done) } }
        withContext(Dispatchers.Default) { delay(300) }
        val art = art(store)
        if (art.isEmpty()) return false
        fun tile(i: Int, title: String) = art[i % art.size].copy(title = title, notInSources = false)
        val anilist = listOf(
            tile(0, "Lantern Street Notes"), // i18n-skip
            tile(1, "Lantern Street Notes, Side Stories"), // i18n-skip
            tile(2, "The Lantern Keeper"), // i18n-skip
            tile(3, "Lanterns Upriver"), // i18n-skip
        )
        val library = listOf(tile(4, "Lantern Street Notes, Vol. 2"), tile(5, "Paper Lantern Recipes")) // i18n-skip
        val folder = listOf(tile(6, "Lantern Festival Sketchbook")) // i18n-skip
        val northwind = listOf(tile(7, "Lantern Ferry Timetables").copy(cover = null), tile(8, "Lantern Hill"), tile(9, "Lantern Moths")) // i18n-skip
        val join = ReadSourceJoin(anilist[1].work, listOf("home-library"), listOf(HOME_LIBRARY)) // i18n-skip
        fun group(id: String, name: String, state: ReadSourceState, tiles: List<WorkTile> = emptyList()) = ReadSourceGroup(id, name, state, tiles, 0u)
        val apply = { event: ReadingEvent -> store.apply(Event.Reading(event)) }
        when (state) {
            "rd-search-mid", "rd-search-done" -> {
                apply(ReadingEvent.Search(query, 0u, anilist, false, null, anilist.size.toUInt()))
                val done = state == "rd-search-done"
                apply(
                    ReadingEvent.SourceSearch(
                        ReadSourceSearch(
                            query,
                            ReadAnilistState.ANSWERED,
                            listOf(
                                group("home-library", HOME_LIBRARY, ReadSourceState.Hits, library), // i18n-skip
                                group("folders", COMICS, ReadSourceState.Hits, folder), // i18n-skip
                                if (done) group("northwind", NORTHWIND, ReadSourceState.TimedOut(15u)) else group("northwind", NORTHWIND, ReadSourceState.Asking), // i18n-skip
                                group("demo", DEMO, ReadSourceState.Nothing), // i18n-skip
                            ),
                            listOf(join),
                            done,
                        ),
                    ),
                )
            }
            "rd-search-unreachable" -> {
                apply(ReadingEvent.Failed(ReadingAbout.SEARCH, ReadingFailure.Offline))
                apply(
                    ReadingEvent.SourceSearch(
                        ReadSourceSearch(
                            query,
                            ReadAnilistState.FAILED,
                            listOf(
                                group("home-library", HOME_LIBRARY, ReadSourceState.Hits, listOf(anilist[0], anilist[1]) + library.drop(1)), // i18n-skip
                                group("folders", COMICS, ReadSourceState.Hits, folder), // i18n-skip
                                group("northwind", NORTHWIND, ReadSourceState.Failed(ReadingFailure.SourceDown(NORTHWIND))), // i18n-skip
                            ),
                            emptyList(),
                            true,
                        ),
                    ),
                )
            }
            "rd-search-sources-only" -> apply(
                ReadingEvent.SourceSearch(
                    ReadSourceSearch(
                        query,
                        ReadAnilistState.SKIPPED,
                        listOf(
                            group("home-library", HOME_LIBRARY, ReadSourceState.Hits, listOf(anilist[0], anilist[1]) + library.drop(1)), // i18n-skip
                            group("folders", COMICS, ReadSourceState.Hits, folder), // i18n-skip
                            group("northwind", NORTHWIND, ReadSourceState.Hits, northwind), // i18n-skip
                        ),
                        emptyList(),
                        true,
                    ),
                ),
            )
            "rd-search-nothing" -> {
                apply(ReadingEvent.Search(query, 0u, emptyList(), false, null, 0u))
                apply(
                    ReadingEvent.SourceSearch(
                        ReadSourceSearch(
                            query,
                            ReadAnilistState.ANSWERED,
                            listOf(
                                group("home-library", HOME_LIBRARY, ReadSourceState.Nothing), // i18n-skip
                                group("folders", COMICS, ReadSourceState.Nothing), // i18n-skip
                                group("demo", DEMO, ReadSourceState.Nothing), // i18n-skip
                            ),
                            emptyList(),
                            true,
                        ),
                    ),
                )
            }
        }
        return true
    }

    /** Series the engine already has, for their covers: Home's rows and lists, else Search's. */
    private suspend fun art(store: AppStore): List<WorkTile> {
        if (store.reading.home.value == null) store.reading.loadHome()
        waitFor { store.reading.home.value != null }
        val home = store.reading.home.value
        val tiles = buildList {
            home?.rows?.forEach { addAll(it.tiles) }
            home?.continueReading?.let(::addAll)
            home?.upNext?.let(::addAll)
        }.filter { it.cover != null }.distinctBy { it.work }
        return tiles.ifEmpty { store.reading.search.value.tiles }
    }

    /**
     * Wait up to ten seconds for [done], on real time: under Robolectric the main looper's clock
     * stands still, so a delay on the main dispatcher would never end.
     */
    private suspend fun waitFor(done: () -> Boolean) = withContext(Dispatchers.Default) {
        withTimeoutOrNull(10_000) { while (!done()) delay(50) }
    }

    /** The query the states search, for tests that wait on it. */
    fun query(state: String): ReadSearchQuery = ReadingModel.EMPTY_QUERY.copy(text = WORDS, sourcesOnly = state == "rd-search-sources-only")
}
