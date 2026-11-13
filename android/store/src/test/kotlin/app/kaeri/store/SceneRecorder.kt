package app.kaeri.store

import app.kaeri.engine.model.TrendPeriod
import java.io.File
import java.nio.file.Files
import java.time.Instant
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.newSingleThreadContext
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeout
import kotlinx.coroutines.withTimeoutOrNull
import org.junit.After
import org.junit.Assume.assumeTrue
import org.junit.Before
import org.junit.Test
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.ContentFor
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.LibrarySegment
import app.kaeri.engine.model.SearchQuery
import app.kaeri.testkit.Scene

/**
 * Records the testing engine's scenes for the screenshot tests: every event the engine sends while
 * the store asks for what each screen shows, written as JSON into the test kit, which the tests
 * replay into a store of their own with no engine.
 *
 * Runs only when asked, by android/scripts/scenes.sh, which sets `kaeri.scenes.out`.
 */
@OptIn(ExperimentalCoroutinesApi::class, kotlinx.coroutines.DelicateCoroutinesApi::class)
class SceneRecorder {
    private val main = newSingleThreadContext("main")
    private val scope = CoroutineScope(SupervisorJob() + main)
    private val root: File = Files.createTempDirectory("kaeri-scenes").toFile()
    private val out = System.getProperty("kaeri.scenes.out")?.let(::File)

    /** The moment the engine's clock is stopped at, which the replays read as now. */
    private val at = System.getenv(SCENES_AT_VAR)?.toLongOrNull()?.let(Instant::ofEpochSecond)

    @Before
    fun setUp() {
        assumeTrue("android/scripts/scenes.sh records the scenes", out != null)
        assumeTrue("the app flavour's engine has no scenes", HAS_TESTING_ENGINE)
        checkNotNull(at) { "$SCENES_AT_VAR stops the engine's clock, so every run records the same scenes" }
        val home = System.getenv("HOME").orEmpty()
        check(home.isNotEmpty() && home != System.getProperty("user.home")) { "HOME is the user's own: $home" }
        Dispatchers.setMain(main)
    }

    @After
    fun tearDown() {
        scope.cancel()
        Dispatchers.resetMain()
        main.close()
        root.deleteRecursively()
    }

    @Test
    fun record_the_scenes() {
        // Home's world changes what the notices and the widgets' record hold, so its scenes are
        // recorded apart, under their own names, in a run with the variable set.
        val world = System.getenv(HOME_WORLD_VAR) == "1"
        val scenes = if (world) HOME_SCENES else RECORDED
        for (scene in scenes) {
            val recorded = record(scene, if (world) "home-" else "")
            Scene.write(recorded, out!!)
            withoutPort(File(out, "${recorded.name}.json"))
        }
    }

    /**
     * [file] with the fake AniList's address as one name: the fake listens on whatever port is
     * free, and the errors the engine passes on name it.
     */
    private fun withoutPort(file: File) {
        check(file.isFile) { "the scene was written somewhere else than $file" }
        file.writeText(file.readText().replace(FAKE_ADDRESS, FAKE_NAME))
    }

    private fun record(scene: FixtureScene, prefix: String): Scene = runBlocking {
        val events = mutableListOf<Event>()
        var batches = 0
        // Where each question's events end, for [settledNotices].
        val ends = mutableListOf<Int>()
        val store = withContext(main) { AppStore(scope) }
        val home = File(root, scene.name.lowercase())
        val recordedAt = at!!
        withContext(main) {
            store.onApplied = { batch ->
                batches++
                events += batch.filter(::kept)
            }
            store.start(EngineLaunch(EngineLaunch.Engine.Fixtures(scene), EngineHome.override(home)), AppHost(MemorySecrets()))
        }
        withTimeout(60_000) { store.phase.first { it == AppStore.Phase.Running } }
        settled(store, scene, "start")
        ends += withContext(main) { events.size }
        // What every screen asks for, one question at a time, each answered before the next: asked
        // together, they are answered in whatever order the engine's work finishes, and two runs
        // would record different scenes.
        for ((what, ask) in asks(store)) {
            val before = withContext(main) {
                ask()
                batches
            }
            // The store counts its quiet from the last event, which the question before ended
            // long ago: the quiet only counts once this question's first answer is in.
            withTimeoutOrNull(ANSWER_MS) { while (withContext(main) { batches } == before) delay(10) }
            settled(store, scene, what)
            ends += withContext(main) { events.size }
        }
        // A browse by genre, once the search before it is answered: a newer search takes an
        // older one's place.
        val browse = SearchQuery(genres = listOf("Fantasy"))
        withContext(main) { store.search(browse) }
        withTimeout(60_000) {
            // The list on screen stays as it was until the answer, so settling alone would not wait.
            while (withContext(main) { events.none { it is Event.Content && it.view == ContentFor.Search(browse) } }) delay(50)
        }
        settled(store, scene, "the browse")
        withContext(main) { store.shutdown() }
        withTimeout(10_000) { store.phase.first { it == AppStore.Phase.Stopped } }
        // Home's scenes are named as one word after their prefix (home-oneleft); the rest keep the
        // enum's underscores (link_opened), as their shots ask for them.
        val name = scene.name.lowercase()
        val answered = jobsEndLast(settledNotices(events.toList(), ends), ends)
        Scene(if (prefix.isEmpty()) name else prefix + name.replace("_", ""), recordedAt, collapsed(answered))
    }

    /** Waits for [store] to be quiet after [what], with nothing loading and no job running. */
    private suspend fun settled(store: AppStore, scene: FixtureScene, what: String) {
        withTimeout(60_000) {
            check(store.settle(quietMillis = QUIET_MS, timeoutMillis = 55_000)) { "the ${scene.name} scene did not settle after $what" }
        }
    }

    /**
     * What every screen asks for, named for a failure: the lists, the library's segments, Search's
     * start and the searches the shots run, and each fixture title's page, episodes and settings
     * sheet.
     */
    private fun asks(store: AppStore): List<Pair<String, () -> Unit>> = buildList {
        add("Home" to { store.loadHome() })
        add("Sync" to { store.loadSync() })
        // The lawful sources' pages: the Sources rows, the servers and the licensed services.
        add("the servers" to { store.loadServers() })
        add("the providers" to { store.checkProviders() })
        add("the licensed services" to { store.loadLicensed() })
        add("Continue watching" to { store.loadContinue() })
        add("This season" to { store.loadSeasonal() })
        add("the calendar" to { store.loadCalendar() })
        add("Downloads" to { store.loadDownloads() })
        // Search's start, one part at a time: asked together, they arrive in any order.
        add("the browse rails" to { store.loadBrowse() })
        add("the filters' choices" to { store.loadSearchFacets() })
        add("the recent searches" to { store.send(Command.LoadRecentSearches) })
        // Trending's other periods, which the rail asks for when its menu is used.
        for (period in listOf(TrendPeriod.WEEK, TrendPeriod.MONTH, TrendPeriod.TODAY)) add("Trending $period" to { store.loadTrending(period) })
        // Watch together's place: the rooms kept and Start a party's servers.
        add("the party rooms" to { store.loadPartyRooms() })
        add("the party form" to { store.loadPartyForm() })
        add("a search" to { store.search(SearchQuery(text = "hoshi")) })
        for (segment in LibraryModel.order) add("the library's $segment" to { store.loadLibrary(segment) })
        for (id in FIXTURE_IDS) {
            add("$id's page" to { store.loadDetail(id) })
            add("$id's episodes" to { store.loadEpisodes(id) })
            add("$id's settings" to { store.loadShowSettings(id) })
            add("$id's availability" to { store.loadAvailability(id, false) })
        }
        // Fix this match: a search by the first fixture's name, on the chain.
        add("a match search" to { store.searchMatch(FIXTURE_IDS.first(), "Stardust", null) })
        // The long franchise's pages the shots open, and some of their neighbours': the rest
        // are seasons whose title has not arrived.
        for (id in LONG_FRANCHISE_IDS) {
            add("$id's page" to { store.loadDetail(id) })
            add("$id's episodes" to { store.loadEpisodes(id) })
        }
    }

    companion object {
        /** The environment variable that stops the engine's clock, which the build sets for a recording. */
        const val SCENES_AT_VAR = "KAERI_SCENES_AT"

        /** The fake AniList's address, on the port it was given. */
        private val FAKE_ADDRESS = Regex("""http://127\.0\.0\.1:\d+""")

        /** What a scene says instead of [FAKE_ADDRESS]. */
        private const val FAKE_NAME = "http://anilist.example"

        /** How long the store stays quiet before a question counts as answered. */
        const val QUIET_MS = 300L

        /** How long a question may go without a first answer: some change nothing and have none. */
        const val ANSWER_MS = 2_000L

        /** The environment variable that puts Home's world in a scene. */
        const val HOME_WORLD_VAR = "KAERI_FIXTURES_HOME_WORLD"

        /** The scenes Home's shots replay, recorded with the world on. */
        val HOME_SCENES = listOf(
            FixtureScene.WATCHING, FixtureScene.SUGGESTED, FixtureScene.ONE_LEFT, FixtureScene.CONTINUE_FULL,
            FixtureScene.NOT_FOUND, FixtureScene.SLOW, FixtureScene.FAILED, FixtureScene.OFFLINE,
        )

        /** The scenes the shots replay: the first two, the link and lawful source states, and first play's demo, subtitle and crash states. */
        val RECORDED = listOf(
            FixtureScene.FRESH, FixtureScene.WATCHING, FixtureScene.LINK_OPENED, FixtureScene.LICENSED, FixtureScene.SERVERS,
            FixtureScene.SOURCES_BUSY, FixtureScene.AVAILABLE, FixtureScene.DEMO_ONLY, FixtureScene.DEMO_PLAYED, FixtureScene.DEMO_AND_STORE,
            FixtureScene.DEMO_REMOVED, FixtureScene.SUBTITLE_STYLED, FixtureScene.CRASH_OFF, FixtureScene.CRASH_SAVED, FixtureScene.CRASH_HELD,
            FixtureScene.TOGETHER, FixtureScene.SYNC_ON, FixtureScene.SYNC_NEARBY, FixtureScene.SYNC_CLOUD, FixtureScene.SYNC_TV_PHONE_SIGN_IN,
            FixtureScene.PROFILES, FixtureScene.BROWSING,
        )

        /** The test kit's titles, 900001 to 900006. */
        val FIXTURE_IDS = (900001u..900006u).toList()

        /**
         * Seasons 1, 2, 8, 9, 10 and 12 of the test kit's long franchise, Cloudcutter Airship, 920001
         * to 920012, which every scene's fake AniList has beside the six.
         */
        val LONG_FRANCHISE_IDS = listOf(920001u, 920002u, 920008u, 920009u, 920010u, 920012u)

        /**
         * [events] with each metadata notice at the end of the question it came in, [ends] being
         * where each question's events end. The engine says a notice, the status rows and their
         * toast, ahead of whichever event passes it next once the metadata ladder moved, and
         * which one that is depends on how the engine's tasks were scheduled.
         */
        private fun settledNotices(events: List<Event>, ends: List<Int>): List<Event> {
            val out = ArrayList<Event>(events.size)
            var start = 0
            for (end in ends + events.size) {
                val part = events.subList(start, maxOf(start, end))
                val notices = part.indices.filter { i ->
                    (part[i] is Event.MetaStatus && part.getOrNull(i + 1) is Event.Toast) ||
                        (part[i] is Event.Toast && part.getOrNull(i - 1) is Event.MetaStatus)
                }.toSet()
                out += part.filterIndexed { i, _ -> i !in notices }
                out += part.filterIndexed { i, _ -> i in notices }
                start = maxOf(start, end)
            }
            return out
        }

        /**
         * [events] with each job's end said after the rest of its question's events, up to the
         * next job's start or note, [ends] being where each question's events end. A job can leave
         * work running when it ends, such as the episode names looked up after a listing or a
         * franchise's cards after a title's page, and that work's events land before or after
         * the job's end by how the engine's tasks were scheduled. A replay applies the same
         * events either way: the end only takes the job's activity line away.
         */
        private fun jobsEndLast(events: List<Event>, ends: List<Int>): List<Event> {
            val out = ArrayList<Event>(events.size)
            var start = 0
            for (end in ends + events.size) {
                val done = ArrayList<Event>()
                for (event in events.subList(start, maxOf(start, end))) {
                    when (event) {
                        is Event.JobDone -> done += event
                        is Event.JobStarted, is Event.JobNote -> {
                            out += done
                            done.clear()
                            out += event
                        }
                        else -> out += event
                    }
                }
                out += done
                start = maxOf(start, end)
            }
            return out
        }

        /**
         * [events] without a Home that the next event replaces at once. Home's sections are
         * answered side by side, so the order they come in differs from run to run, while the
         * Home they end on does not. A failed Home stays, as the store counts each failure.
         */
        private fun collapsed(events: List<Event>): List<Event> = events.filterIndexed { i, event ->
            !(event is Event.Home && !event.v1.failed && events.getOrNull(i + 1) is Event.Home)
        }

        /**
         * What a replay keeps: everything but art, which the test kit's files answer, and the
         * player's, which no screenshot of these screens shows.
         */
        private fun kept(event: Event): Boolean = when (event) {
            is Event.ArtworkReady, is Event.ImageFailed -> false
            is Event.PlayerOps, is Event.PlayerUi, is Event.PlaybackStaged, is Event.PlaybackEnded, Event.PlaybackKept,
            is Event.PlayerStats, is Event.Clipboard -> false
            else -> true
        }
    }
}
