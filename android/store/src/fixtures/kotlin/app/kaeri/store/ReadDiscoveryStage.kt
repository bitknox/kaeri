// The discovery board's Search states, staged for shots on the fixtures engine: the testing engine
// has no sources that answer a search slowly, fail or time out, and no AniList that fails on cue.
// Each state runs a real search first and waits for the engine's own answer, then says the board's
// answer over it with the engine's own events, so the store and the screens take it as they would
// from the engine. Covers are the fixtures' own; titles are made up.

package app.kaeri.store

import app.kaeri.engine.model.Event
import app.kaeri.engine.model.Msg
import app.kaeri.engine.model.ReadAnilistState
import app.kaeri.engine.model.ReadSearchQuery
import app.kaeri.engine.model.ReadSourceGroup
import app.kaeri.engine.model.ReadSourceJoin
import app.kaeri.engine.model.ReadSourceKind
import app.kaeri.engine.model.ReadSourceSearch
import app.kaeri.engine.model.ReadSourceState
import app.kaeri.engine.model.ReadingAbout
import app.kaeri.engine.model.ReadingEvent
import app.kaeri.engine.model.ReadingFailure
import app.kaeri.engine.model.ReadingMsg
import app.kaeri.engine.model.WorkTile
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull

object ReadDiscoveryStage {
    val STATES: List<String> = listOf("rd-search-mid", "rd-search-unreachable", "rd-search-sources-only", "rd-search-nothing", "rd-search-done")

    /**
     * States for recordings, which play out over a few seconds: `rd-typing` answers the words,
     * then new words twice, the first slower than 600 ms and the second at once; `rd-rows-landing`
     * holds Home's rows on their way for [ROWS_HELD] ms, then lands them.
     */
    val MOTION: List<String> = listOf("rd-typing", "rd-rows-landing")

    /** How long `rd-rows-landing` holds the rows on their way: long enough to scroll a phone down to them. */
    private const val ROWS_HELD = 5_000L

    /** The kinds beside the made-up sources' names; the demo is built in and has none. */
    private val KINDS = mapOf("home-library" to ReadSourceKind.SERVER, "folders" to ReadSourceKind.FOLDER, "northwind" to ReadSourceKind.PLUGIN) // i18n-skip

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
        when (state) {
            "rd-typing" -> return typing(store)
            "rd-rows-landing" -> return rowsLanding(store)
        }
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
        // A source's hit says how much the source holds: a server its volumes, a folder its files,
        // a plugin nothing yet.
        fun hit(i: Int, title: String, amount: Msg?) = tile(i, title).copy(origin = null, amount = amount)
        val library = listOf(hit(4, "Lantern Street Notes, Vol. 2", Msg.Reading(ReadingMsg.ReadingVolumes(2L))), hit(5, "Paper Lantern Recipes", Msg.Reading(ReadingMsg.ReadingVolumes(3L)))) // i18n-skip
        val folder = listOf(hit(6, "Lantern Festival Sketchbook", Msg.Reading(ReadingMsg.ReadHitFilesFormat(1L, "CBZ")))) // i18n-skip
        // Done: the folder holds eight, so its row shows five and Show 3 more.
        val fullFolder = folder + listOf(
            "Lantern Festival Sketchbook 2", "Lantern Festival Sketchbook 3", "Lantern Light Studies", // i18n-skip
            "Lanterns at Low Tide", "Lantern Market Doodles", "Lantern Glass", "Lanternfish Notes", // i18n-skip
        ).mapIndexed { i, title -> hit(10 + i, title, Msg.Reading(ReadingMsg.ReadHitFilesFormat(1L, "CBZ"))) } // i18n-skip
        val northwind = listOf(hit(7, "Lantern Ferry Timetables", null).copy(cover = null), hit(8, "Lantern Hill", null), hit(9, "Lantern Moths", null)) // i18n-skip
        val libraryJoined = listOf(hit(0, "Lantern Street Notes", Msg.Reading(ReadingMsg.ReadingVolumes(4L))), hit(1, "Lantern Street Notes, Side Stories", Msg.Reading(ReadingMsg.ReadingVolumes(2L)))) // i18n-skip
        val join = ReadSourceJoin(anilist[1].work, listOf("home-library"), listOf(HOME_LIBRARY)) // i18n-skip
        fun group(id: String, name: String, state: ReadSourceState, tiles: List<WorkTile> = emptyList()) =
            ReadSourceGroup(id, name, KINDS[id], state, tiles, 0u)
        val apply = { event: ReadingEvent -> store.apply(Event.Reading(event)) }
        when (state) {
            "rd-search-mid", "rd-search-done" -> {
                val done = state == "rd-search-done"
                // As the engine sends them: the sources and their joins first, then AniList's page.
                apply(
                    ReadingEvent.SourceSearch(
                        ReadSourceSearch(
                            query,
                            ReadAnilistState.ANSWERED,
                            listOf(
                                group("home-library", HOME_LIBRARY, ReadSourceState.Hits, library), // i18n-skip
                                group("folders", COMICS, ReadSourceState.Hits, if (done) fullFolder else folder), // i18n-skip
                                if (done) group("northwind", NORTHWIND, ReadSourceState.TimedOut(15u)) else group("northwind", NORTHWIND, ReadSourceState.Asking), // i18n-skip
                                group("demo", DEMO, ReadSourceState.Nothing), // i18n-skip
                            ),
                            listOf(join),
                            done,
                        ),
                    ),
                )
                apply(ReadingEvent.Search(query, 0u, anilist, false, null, anilist.size.toUInt()))
            }
            "rd-search-unreachable" -> {
                apply(ReadingEvent.Failed(ReadingAbout.SEARCH, ReadingFailure.Offline))
                apply(
                    ReadingEvent.SourceSearch(
                        ReadSourceSearch(
                            query,
                            ReadAnilistState.FAILED,
                            listOf(
                                group("home-library", HOME_LIBRARY, ReadSourceState.Hits, libraryJoined + library.drop(1)), // i18n-skip
                                group("folders", COMICS, ReadSourceState.Hits, folder), // i18n-skip
                                group("northwind", NORTHWIND, ReadSourceState.Failed(ReadingFailure.SourceDown(NORTHWIND))), // i18n-skip
                            ),
                            emptyList(),
                            true,
                        ),
                    ),
                )
            }
            "rd-search-sources-only" -> {
                // An empty answer first, shown for a moment, so the list is at its top when the
                // staged groups come in, as for a first answer, rather than kept on the row of the
                // engine's own answer it was showing.
                apply(ReadingEvent.SourceSearch(ReadSourceSearch(query, ReadAnilistState.SKIPPED, emptyList(), emptyList(), false)))
                withContext(Dispatchers.Default) { delay(300) }
                apply(
                    ReadingEvent.SourceSearch(
                        ReadSourceSearch(
                            query,
                            ReadAnilistState.SKIPPED,
                            listOf(
                                group("home-library", HOME_LIBRARY, ReadSourceState.Hits, libraryJoined + library.drop(1)), // i18n-skip
                                group("folders", COMICS, ReadSourceState.Hits, folder), // i18n-skip
                                group("northwind", NORTHWIND, ReadSourceState.Hits, northwind), // i18n-skip
                            ),
                            emptyList(),
                            true,
                        ),
                    ),
                )
            }
            "rd-search-nothing" -> {
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
                apply(ReadingEvent.Search(query, 0u, emptyList(), false, null, 0u))
            }
        }
        return true
    }

    /**
     * New words over an answer: "lantern" answered as the board's done state, then "lantern hill"
     * asked with the old answer kept, its sources and AniList answering past 600 ms, then
     * "lantern moth" answered within 200 ms. Nothing goes to the engine for the new words.
     */
    private suspend fun typing(store: AppStore): Boolean {
        if (!stage(store, "rd-search-done")) return false
        val art = art(store)
        fun tile(i: Int, title: String) = art[i % art.size].copy(title = title, notInSources = false)
        fun hit(i: Int, title: String, amount: Msg?) = tile(i, title).copy(origin = null, amount = amount)
        fun group(id: String, name: String, state: ReadSourceState, tiles: List<WorkTile> = emptyList()) =
            ReadSourceGroup(id, name, KINDS[id], state, tiles, 0u)
        val apply = { event: ReadingEvent -> store.apply(Event.Reading(event)) }
        suspend fun wait(ms: Long) = withContext(Dispatchers.Default) { delay(ms) }

        wait(2_500)
        val hill = ReadingModel.EMPTY_QUERY.copy(text = "lantern hill") // i18n-skip
        store.reading.askedForStage(hill)
        wait(900)
        apply(
            ReadingEvent.SourceSearch(
                ReadSourceSearch(
                    hill, ReadAnilistState.ASKING,
                    listOf(
                        group("home-library", HOME_LIBRARY, ReadSourceState.Hits, listOf(hit(5, "Lantern Hill, Vol. 1", Msg.Reading(ReadingMsg.ReadingVolumes(1L))))), // i18n-skip
                        group("folders", COMICS, ReadSourceState.Asking), // i18n-skip
                        group("northwind", NORTHWIND, ReadSourceState.Asking), // i18n-skip
                        group("demo", DEMO, ReadSourceState.Nothing), // i18n-skip
                    ),
                    emptyList(), false,
                ),
            ),
        )
        wait(500)
        val hillTiles = listOf(tile(8, "Lantern Hill"), tile(2, "The Lantern Keeper")) // i18n-skip
        apply(ReadingEvent.Search(hill, 0u, hillTiles, false, null, hillTiles.size.toUInt()))
        wait(600)
        apply(
            ReadingEvent.SourceSearch(
                ReadSourceSearch(
                    hill, ReadAnilistState.ANSWERED,
                    listOf(
                        group("home-library", HOME_LIBRARY, ReadSourceState.Hits, listOf(hit(5, "Lantern Hill, Vol. 1", Msg.Reading(ReadingMsg.ReadingVolumes(1L))))), // i18n-skip
                        group("folders", COMICS, ReadSourceState.Nothing), // i18n-skip
                        group("northwind", NORTHWIND, ReadSourceState.Hits, listOf(hit(8, "Lantern Hill", null))), // i18n-skip
                        group("demo", DEMO, ReadSourceState.Nothing), // i18n-skip
                    ),
                    emptyList(), true,
                ),
            ),
        )

        wait(2_500)
        val moth = ReadingModel.EMPTY_QUERY.copy(text = "lantern moth") // i18n-skip
        store.reading.askedForStage(moth)
        wait(200)
        val mothTiles = listOf(tile(9, "Lantern Moths"), tile(3, "Lanterns Upriver"), tile(0, "Lantern Street Notes")) // i18n-skip
        apply(ReadingEvent.Search(moth, 0u, mothTiles, false, null, mothTiles.size.toUInt()))
        apply(
            ReadingEvent.SourceSearch(
                ReadSourceSearch(
                    moth, ReadAnilistState.ANSWERED,
                    listOf(
                        group("home-library", HOME_LIBRARY, ReadSourceState.Nothing), // i18n-skip
                        group("folders", COMICS, ReadSourceState.Nothing), // i18n-skip
                        group("northwind", NORTHWIND, ReadSourceState.Hits, listOf(hit(9, "Lantern Moths", null))), // i18n-skip
                        group("demo", DEMO, ReadSourceState.Nothing), // i18n-skip
                    ),
                    emptyList(), true,
                ),
            ),
        )
        return true
    }

    /** Home's rows on their way with nothing kept for [ROWS_HELD] ms, then landing as the engine sent them. */
    private suspend fun rowsLanding(store: AppStore): Boolean {
        if (store.reading.home.value == null) store.reading.loadHome()
        waitFor { store.reading.home.value?.rows?.any { it.tiles.isNotEmpty() } == true }
        val home = store.reading.home.value ?: return false
        withContext(Dispatchers.Main) {
            store.reading.stage = { it.copy(rows = it.rows.map { row -> row.copy(tiles = emptyList(), pending = true) }) }
        }
        withContext(Dispatchers.Default) { delay(ROWS_HELD) }
        withContext(Dispatchers.Main) {
            store.reading.stage = null
            store.apply(Event.Reading(ReadingEvent.Home(home)))
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
