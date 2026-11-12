// The fixtures flavour's launch: the testing engine on a scene, each launch in a directory of its
// own, steered by intent extras under the names of the Apple app's -Kaeri* arguments:
//
//   scene     watching (the default), sourceless (watching with no source set up), fresh or playing
//   media     what the playing scene streams: a path or URL on the device
//   section   the destination to open on: continue, calendar, seasonal, library or search
//   watched   episodes to mark watched once the engine is up: 900006:8, or 900001:1,2,3, with more
//             titles joined by / (900006:1,2/900002:1), oldest first
//   title     a title page to open, by id
//   query     a search to run, on Search
//   page      a page over the destination, by its route: settings, settings/playback
//   downloads any value: three downloads seeded, one finished, whose file is the fixtures' clip
//             in the app's download folder, one running and one waiting
//   notify    any value: two new-episode notifications posted for fixture titles, grouped, as the
//             periodic check posts them (the notification permission granted first)
//   check     seconds: the new-episode check run once that many seconds from now, as the periodic
//             work runs it, 0 for at once; kill the app first to have it start a quiet engine
//   widgets   which widgets to offer the launcher to pin, joined by commas: continue-small,
//             continue-wide, airing-small, airing-wide (the launcher asks before each); and the
//             widgets' record made at once, with its art
//   tv        a Play on TV scene, on the two fake TVs: paired, picker, code or remote (TvScene)
//   cast      a casting scene, on Bedroom and the made-up TVs with Chromecast built in: picker,
//             searching, none, blocked, starting, preparing, fixed, level, failed, never, network, codec,
//             deeplink, upnext, undo, audio, lost, taken or bar (CastScene)
//   discussion an episode number of the first fixture whose discussion opens in its sheet
//   sheet     a sheet to open: torrent, join, kitsu, update or show (from A2); match, match-fix,
//             match-found or match-empty open the match picker on the first fixture (unsure scene)
//   player    a player state to shoot (from A3)
//   found     flow or settings: the found skip times demo, the engine's part scripted, for recording motion
//   start     a made-up start (StartingFixture, as `slow` or `failed-no-episode`) said to the real
//             player screen on its own clock; `details` 1 opens Details, `startdelay` ms waits first
//   code      a tracker whose device-code sheet opens, with a made-up code that expires in 15 minutes
//   expires   seconds the `code` sheet's code has left, 0 for one already expired
//   day       days after today the calendar opens on, for a day with broadcasts: the fixtures'
//             own day has none
//   medium    read, watch or both: what Kaeri is for, chosen as Settings would once the engine is up
//             (the reading scenes); page=read/series/a%3A900301 opens a series' page over Read
//   series    a sheet or menu the series' page opens once it has loaded: chapter, read-from,
//             group, tracking, read-again, send, where-stop, choose, status or download
//   credits   start, end, both or parts: each layout of the chapter the reader opens comes with a
//             credit page skipped at its start, its end or both, or in two parts (CreditsStage)
//   creditshot what the reader shows once they have come: line, chrome, shown, shownchrome, kept,
//             parts or end (CreditShot)
//   groups    set: the series' source lists three groups, and its rows show a chapter in parts
//             and chapters from other groups (GroupsStage)
//   chapterwait ms a chapter opened from another waits before the engine hears of it, to see the
//             reader's pressed and loading states on chapters that open at once here
//   party     a watch party on the testing engine's fake server, for the chat board (PartyShot):
//             chatline, chatbusy, chatreact, chatfield, chatoff, chatfail, chatmute, chathidden,
//             chatshort, chatloop, lobby or bar; with media=<path> the player plays that file.
//             chatloop and bar wait for `am broadcast -a app.kaeri.fixtures.PARTY_GO` before
//             their timeline, so a recording can roll first

package app.kaeri.app

import android.appwidget.AppWidgetManager
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.os.Environment
import android.util.Log
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.WorkManager
import java.util.UUID
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
import app.kaeri.engine.model.DeviceCodePrompt
import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.store.reads
import app.kaeri.engine.model.DiscussionState
import app.kaeri.engine.model.Event
import app.kaeri.store.EngineEpisodeCheck
import app.kaeri.store.covers
import app.kaeri.store.AppStore
import app.kaeri.store.MatchSearch
import app.kaeri.store.EngineHome
import app.kaeri.store.NewEpisode
import app.kaeri.store.EngineLaunch
import app.kaeri.store.enginePlatform
import app.kaeri.engine.ffi.FfiCastBrowser
import app.kaeri.engine.ffi.FfiException
import app.kaeri.engine.ffi.FfiTvBrowser
import app.kaeri.store.CastScene
import app.kaeri.store.FixtureCasts
import app.kaeri.store.FixtureTvBrowser
import app.kaeri.store.FixtureScene
import app.kaeri.store.FixtureParty
import app.kaeri.store.PartyShot
import app.kaeri.store.PartyStage
import app.kaeri.store.FixtureTvs
import app.kaeri.store.TvPlayRequest
import app.kaeri.store.TvScene
import app.kaeri.store.fixtureDownloadClip
import app.kaeri.store.fixtureMedia
import app.kaeri.store.fixtures
import app.kaeri.widgets.AiringSmallReceiver
import app.kaeri.widgets.AiringWideReceiver
import app.kaeri.widgets.ContinueSmallReceiver
import app.kaeri.widgets.ContinueWideReceiver

internal object Flavour {
    const val FIXTURES = true

    /** The fixtures flavour never installs anything: it has no permission to. */
    const val UPDATES = false

    private var marked = false

    /** The application, for what the casting scenes' made-up TVs say. */
    private var appContext: Context? = null

    /**
     * What the e-ink detection is told of a wide colour gamut in place of the display's answer.
     * An emulator has none, so the colour question's scene says it has one, as an unknown colour
     * panel would.
     */
    var wideColourGamut: Boolean? = null
        private set

    /** What the e-ink detection is told of the system's e-ink flag: the colour question's scene says yes. */
    var einkSupport: String? = null
        private set

    /** The process's launch: the engine starts once, so a rebuilt activity keeps the files it runs on. */
    private var launched: EngineLaunch? = null

    /** The `chapterwait` extra, in ms. */
    private var chapterWait = 0L

    /** A reader command on its way to the engine: a chapter opened from another held back [chapterWait]. */
    fun readerSend(command: app.kaeri.engine.model.Command, send: (app.kaeri.engine.model.Command) -> Unit) {
        val open = (command as? app.kaeri.engine.model.Command.Reader)?.v1 is app.kaeri.engine.model.ReaderCommand.OpenChapter
        if (open && chapterWait > 0) {
            android.os.Handler(android.os.Looper.getMainLooper()).postDelayed({ send(command) }, chapterWait)
        } else {
            send(command)
        }
    }

    fun launch(context: Context, intent: Intent): EngineLaunch {
        launched?.let { return it }
        appContext = context.applicationContext
        val named = intent.getStringExtra("scene")
        val scene = named?.let(FixtureScene::named) ?: FixtureScene.WATCHING
        // shell=eink: any scene on a grey e-ink panel, answered at start, for the e-ink shell's shots.
        val einkShell = intent.getStringExtra("shell") == "eink"
        wideColourGamut = if (scene == FixtureScene.EINK_KALEIDO_UNSURE) true else if (einkShell) false else null
        einkSupport = if (scene == FixtureScene.EINK_KALEIDO_UNSURE || einkShell) "1" else null
        if (named != null && FixtureScene.named(named) == null) Log.w(KaeriApp.TAG, "no scene named $named")
        // Home's world: invented titles dated by the scene's clock, for Home's shots.
        if (intent.getStringExtra("world") == "1") android.system.Os.setenv("KAERI_FIXTURES_HOME_WORLD", "1", true)
        // The fuller catalogue, which Search's filters=set route needs for titles that match.
        if (intent.getStringExtra("catalogue") == "1" || intent.getStringExtra(MainActivity.FILTERS) != null) {
            android.system.Os.setenv("KAERI_FIXTURES_CATALOGUE", "1", true)
        }
        intent.getStringExtra("media")?.let { fixtureMedia = it }
        chapterWait = intent.getStringExtra("chapterwait")?.toLongOrNull()?.coerceAtLeast(0) ?: 0L
        FixtureCasts.scene = CastScene.named(intent.getStringExtra("cast"))
        if (intent.getStringExtra("downloads") != null) fixtureDownloadClip = downloadClip(context)
        return EngineLaunch.fixtures(scene, context.cacheDir)
            .copy(platform = enginePlatform(context), profile = intent.getStringExtra("profile") ?: ProfileChoice.saved(context))
            .also { launched = it }
    }

    /**
     * The fixtures' clip, copied out of the APK into the app's download folder, where a finished
     * download is kept: what the seeded finished episode plays, offline.
     */
    private fun downloadClip(context: Context): String? = runCatching {
        val folder = (context.getExternalFilesDir(Environment.DIRECTORY_MOVIES) ?: context.filesDir.resolve("downloads"))
            .resolve("fixtures").apply { mkdirs() }
        val file = folder.resolve("episode-06.mkv")
        context.assets.open("fixture-download.mkv").use { input -> file.outputStream().use { input.copyTo(it) } }
        file.path
    }.onFailure { Log.w(KaeriApp.TAG, "the fixture clip was not copied: ${it.message}") }.getOrNull()

    /**
     * The engine background work starts while the app is not running: the watching scene, in a
     * directory of its own, never the one a launch is using.
     */
    fun background(context: Context): EngineLaunch {
        val parent = context.cacheDir.resolve("fixtures-background")
        parent.listFiles()?.forEach { it.deleteRecursively() }
        return EngineLaunch(
            engine = EngineLaunch.Engine.Fixtures(FixtureScene.WATCHING),
            home = EngineHome.override(parent.resolve(UUID.randomUUID().toString())),
        )
    }

    /**
     * Two new-episode notifications for fixture titles, as a check posts them, with the covers the
     * engine serves: what the `notify` extra asks for.
     */
    private fun notifyFixtures(store: AppStore, notifier: EpisodeNotifier) {
        MainScope().launch {
            val ids = listOf(900001u, 900006u)
            val entries = withTimeoutOrNull(60_000) { store.titles.entries.first { titles -> ids.all { it in titles } } } ?: return@launch
            val covers = store.covers(ids.mapNotNull { entries[it]?.coverUrl }, EngineEpisodeCheck.COVER_SIZE, 10_000)
            val now = System.currentTimeMillis() / 1000
            notifier.post(
                listOf(900001u to 8u to 3_600L, 900006u to 3u to 600L).map { (show, ago) ->
                    val (id, episode) = show
                    val entry = entries.getValue(id)
                    NewEpisode(id, entry.title, episode, now - ago, entry.coverUrl, entry.coverUrl?.let(covers::get))
                },
            )
        }
    }

    /**
     * Offer the launcher the widgets [names] names, for a look at them on a home screen, and make
     * their record now rather than at the next check.
     */
    private fun pinWidgets(context: Context, names: String) {
        val receivers = mapOf(
            "continue-small" to ContinueSmallReceiver::class.java,
            "continue-wide" to ContinueWideReceiver::class.java,
            "airing-small" to AiringSmallReceiver::class.java,
            "airing-wide" to AiringWideReceiver::class.java,
        )
        val manager = context.getSystemService(AppWidgetManager::class.java)
        for (name in names.split(',').map(String::trim).filter(String::isNotEmpty)) {
            val receiver = receivers[name] ?: run {
                Log.w(KaeriApp.TAG, "no widget named $name: ${receivers.keys.joinToString()}")
                null
            } ?: continue
            if (manager?.isRequestPinAppWidgetSupported != true) {
                Log.w(KaeriApp.TAG, "this launcher pins no widgets")
                break
            }
            manager.requestPinAppWidget(ComponentName(context, receiver), null, null)
        }
        val app = context.applicationContext as? KaeriApp ?: return
        MainScope().launch {
            runCatching { app.makeWidgetRecord() }.onFailure { Log.w(KaeriApp.TAG, "no widgets' record: ${it.message}") }
        }
    }

    /**
     * The fake TVs, Living room and Bedroom, on the loopback interface: never the real network. A
     * casting scene has Bedroom alone, as Living room is a TV with Chromecast built in there, and
     * none at all when nothing is to answer.
     */
    fun tvBrowser(context: Context, store: AppStore): FfiTvBrowser? {
        val engine = store.engine ?: return null
        val scene = FixtureCasts.scene ?: return FixtureTvs.start(engine)
        if (scene.empty) return FixtureTvBrowser(emptyList())
        return try {
            FixtureTvBrowser(listOf(engine.startFakeTv("Bedroom")))
        } catch (e: FfiException) {
            Log.w(KaeriApp.TAG, "the fake TV did not start: ${e.message}")
            null
        }
    }

    /** The made-up TVs with Chromecast built in, which no network is asked for. */
    fun castBrowser(context: Context): FfiCastBrowser = FixtureCasts.browser()

    /**
     * The TV scene the `tv` extra names, set up once Play on TV is, and true once it is on
     * screen: what announceReady waits on besides the store. True with no scene asked for.
     */
    suspend fun reachedTv(intent: Intent, store: AppStore): Boolean {
        if (!reachedReading(intent, store)) return false
        PartyShot.named(intent.getStringExtra("party"))?.let { shot ->
            val app = appContext as? KaeriApp ?: return false
            FixtureParty.listen(app)
            return FixtureParty.stage(shot, store, PlayerStage(app))
        }
        intent.getStringExtra("sheet")?.takeIf { it.startsWith("match") }?.let { stageMatch(store, it) }
        intent.getStringExtra("discussion")?.let { value ->
            // "row" waits for the thread to be found without opening it, for the episode row's quiet line.
            if (value == "row") {
                foundFixtureThread(store, 7u)
                delay(500)
            } else {
                value.toUIntOrNull()?.let { reachedDiscussion(store, it) }
            }
        }
        CastScene.named(intent.getStringExtra("cast"))?.let { return reachedCast(intent, store, it) }
        intent.getStringExtra("prelaunch")?.let { state ->
            if (!app.kaeri.store.Prelaunch.stage(store, state)) Log.w(KaeriApp.TAG, "no prelaunch state named $state: ${app.kaeri.store.Prelaunch.STATES.joinToString()}")
        }
        intent.getStringExtra("discovery")?.let { state ->
            if (!app.kaeri.store.ReadDiscoveryStage.stage(store, state)) Log.w(KaeriApp.TAG, "no discovery state named $state: ${app.kaeri.store.ReadDiscoveryStage.STATES.joinToString()}")
        }
        // groups=set: the series' source lists three groups, and its rows show parts and other groups.
        if (intent.getStringExtra("groups") == "set") app.kaeri.store.GroupsStage.stage(store)
        intent.getStringExtra("sync")?.let { state ->
            if (!app.kaeri.store.FixtureSync.stage(store, state)) Log.w(KaeriApp.TAG, "no sync state named $state: ${app.kaeri.store.FixtureSync.STATES.joinToString()}")
        }
        val scene = TvScene.named(intent.getStringExtra("tv")) ?: return true
        val browser = withTimeoutOrNull(60_000) {
            while (store.tv.attachedBrowser == null) delay(50)
            store.tv.attachedBrowser
        } ?: return false
        val id = 900001u
        val title = withTimeoutOrNull(30_000) { store.titles.entries.first { id in it }[id]?.title }.orEmpty()
        FixtureTvs.stage(store.tv, scene, browser, TvPlayRequest(id, title, "3"))
        return withTimeoutOrNull(10_000) {
            while (!store.tv.reached(scene)) delay(50)
            // The sheet's own animation in.
            delay(700)
            true
        } ?: false
    }

    /**
     * The reading extras done: the medium asked for chosen, a series' page asked for loaded, and
     * reading's Home or Updates there when they are on screen. True with none asked for.
     */
    private suspend fun reachedReading(intent: Intent, store: AppStore): Boolean {
        val medium = intent.getStringExtra("medium")?.let { named -> KaeriIsFor.entries.firstOrNull { it.name.equals(named, ignoreCase = true) } }
        val work = intent.getStringExtra(MainActivity.PAGE)?.substringAfter("/series/", "")?.takeIf { it.isNotEmpty() }?.let(android.net.Uri::decode)
        val reading = intent.getStringExtra("scene")?.let(FixtureScene::named)?.name?.startsWith("READING") == true
        if (medium == null && work == null && !reading) return true
        return withTimeoutOrNull(60_000) {
            if (medium != null) store.medium.view.first { it?.answer == medium }
            if (work != null) {
                store.reading.series.first { it[work] != null }
                store.reading.chapters.first { it[work] != null }
            } else if (store.medium.answer.reads) {
                // What the reading screen on show loads first: Home, Updates or the library.
                // Search and Settings load nothing of reading's until asked.
                withTimeoutOrNull(8_000) {
                    kotlinx.coroutines.flow.combine(store.reading.home, store.reading.updates, store.reading.lists) { home, updates, lists ->
                        home != null || updates != null || lists.isNotEmpty()
                    }.first { it }
                }
            }
            // Reading's Search asked for: its answer in, the sources' with it.
            if (intent.getStringExtra("scope") == "reading") {
                withTimeoutOrNull(15_000) { store.reading.search.first { !it.loading && (it.sources == null || it.sources?.done == true) } }
            }
            // The sheet a shot asked for, opening.
            delay(if (intent.getStringExtra("series") != null) 1_200 else 300)
            true
        } ?: false
    }

    /** The casting scene [scene], set up once Play on TV is, and true once it is on screen. */
    private suspend fun reachedCast(intent: Intent, store: AppStore, scene: CastScene): Boolean {
        withTimeoutOrNull(60_000) { while (!store.cast.available) delay(50) } ?: return false
        val id = 900001u
        val title = withTimeoutOrNull(30_000) { store.titles.entries.first { id in it }[id]?.title }.orEmpty()
        store.loadEpisodes(id)
        withTimeoutOrNull(10_000) { store.titles.episodes.first { id in it } }
        FixtureCasts.stage(appContext ?: return false, store, MainScope(), scene, title)
        return withTimeoutOrNull(20_000) {
            while (!FixtureCasts.reached(store, scene)) delay(50)
            // The sheet's own animation in; the none scene waits out the picker's 10 s.
            delay(if (scene == CastScene.NONE) 10_500 else 700)
            true
        } ?: false
    }

    /**
     * Marking episodes watched here does not make the engine look their threads up, as playback
     * does: say the fixture thread found, as the engine would, with its six comments.
     */
    private fun foundFixtureThread(store: AppStore, episode: UInt) {
        store.apply(listOf(Event.DiscussionFound(900001u, episode, 6u)))
    }

    /**
     * The discussion of [episode] of the first fixture, open in its sheet once the engine has
     * found its thread and read the first page: what the `discussion` extra asks for.
     */
    private suspend fun reachedDiscussion(store: AppStore, episode: UInt) {
        val id = 900001u
        foundFixtureThread(store, episode)
        store.discussion.show(id, episode)
        withTimeoutOrNull(30_000) { store.discussion.open.first { it?.state is DiscussionState.Ready } }
        // The sheet's own animation in, and the avatars.
        delay(1_200)
    }

    /** The reading scenes open their book in the reader, as the Read switch will. Reading-dark is the build with reading off: no reader to open. */
    private fun openReaderScene(intent: Intent) {
        // Any work in any scene, such as the demo's story: read=<work>, eink=1 for the e-ink reader.
        intent.getStringExtra("read")?.let { work ->
            app.kaeri.reader.phone.ReaderLauncher.open(
                app.kaeri.reader.phone.ReaderRequest(work, eink = intent.getStringExtra("eink") == "1"),
            )
            return
        }
        val scene = intent.getStringExtra("scene")?.let(FixtureScene::named) ?: return
        val (work, eink) = when (scene) {
            FixtureScene.READER_MANGA_RTL, FixtureScene.READER_MOVED, FixtureScene.READER_LONG -> "a:900301" to false
            FixtureScene.READER_STRIP -> "a:900303" to false
            FixtureScene.READER_NOVEL -> "a:900302" to false
            FixtureScene.EINK_GREY, FixtureScene.EINK_KALEIDO_UNSURE, FixtureScene.EINK_LONG -> "a:900301" to true
            else -> return
        }
        // Left to the app's own screens when the launch asked for one.
        if (intent.getStringExtra("reader") == "off") return
        app.kaeri.reader.phone.ReaderLauncher.open(
            app.kaeri.reader.phone.ReaderRequest(work, eink = eink),
        )
    }

    /** What the extras ask of the engine once it is up, once per process. */
    fun started(context: Context, store: AppStore, intent: Intent, notifier: EpisodeNotifier, player: app.kaeri.player.PlayerModel) {
        StartReplay.from(context, store, player, intent)
        if (marked) return
        marked = true
        intent.getStringExtra("credits")?.let { what ->
            val scene = intent.getStringExtra("scene")?.let(FixtureScene::named)
            val grey = scene == FixtureScene.EINK_GREY || scene == FixtureScene.EINK_LONG || intent.getStringExtra("eink") == "1"
            if (!app.kaeri.store.CreditsStage.stage(context, store, what, grey)) Log.w(KaeriApp.TAG, "credits takes ${app.kaeri.store.CreditsStage.STATES.joinToString()}, not $what")
        }
        app.kaeri.reader.CreditShot.step = intent.getStringExtra("creditshot")
        // plate=hold: every page waits on its plate, for the plate's shots.
        app.kaeri.reader.PlateShot.hold = intent.getStringExtra("plate") == "hold"
        openReaderScene(intent)
        if (intent.getStringExtra("shell") == "eink") {
            // The panel's question answered as a person would: e-ink, in greys.
            val eink = (context.applicationContext as KaeriApp).eink
            MainScope().launch {
                eink.state.first { it.profile != null }
                eink.answerColour(false)
            }
        }
        intent.getStringExtra("medium")?.let { named ->
            KaeriIsFor.entries.firstOrNull { it.name.equals(named, ignoreCase = true) }?.let(store.medium::chooseAtLaunch)
                ?: Log.w(KaeriApp.TAG, "medium takes read, watch or both, not $named")
        }
        intent.getStringExtra("series")?.let { app.kaeri.ui.reading.SeriesShots.open = it }
        // scope=reading: Search on reading's side, with the query's words and readfilters=set.
        if (intent.getStringExtra("scope") == "reading") {
            app.kaeri.ui.reading.showReadingSearch(store, intent.getStringExtra(MainActivity.QUERY), intent.getStringExtra("readfilters") == "set")
        }
        if (intent.getStringExtra("notify") != null) notifyFixtures(store, notifier)
        intent.getStringExtra("widgets")?.let { pinWidgets(context, it) }
        intent.getStringExtra("check")?.toLongOrNull()?.let { delay ->
            val once = OneTimeWorkRequestBuilder<NewEpisodesWorker>().setInitialDelay(delay, TimeUnit.SECONDS).build()
            WorkManager.getInstance(context).enqueue(once)
        }
        intent.getStringExtra("watched")?.let { watched ->
            val marks = watched.split('/').mapNotNull { mark ->
                val parts = mark.split(':', limit = 2)
                val id = parts.firstOrNull()?.toUIntOrNull()
                if (parts.size != 2 || id == null) {
                    Log.w(KaeriApp.TAG, "watched takes <id>:<episodes>, not $mark")
                    null
                } else {
                    id to parts[1].split(',')
                }
            }
            for ((id, episodes) in marks) store.setWatched(id, episodes)
            // Asked again after the marks, which the engine takes in order.
            store.loadContinue(force = true)
            for ((id, _) in marks) store.loadDetail(id)
        }
        intent.getStringExtra("day")?.toIntOrNull()?.let { store.calendar.opensOn = it }
        intent.getStringExtra("code")?.let { tracker ->
            val expiresAt = System.currentTimeMillis() / 1000 + codeSecondsLeft(intent)
            store.apply(Event.DeviceCode(DeviceCodePrompt(tracker, "4F2K9QX1", "https://activate.example/pin", expiresAt)))
        }
        intent.getStringExtra("player")?.let { Log.w(KaeriApp.TAG, "player=$it is not in this build yet") }
        intent.getStringExtra("found")?.let { FoundDemo.run(context.applicationContext as app.kaeri.player.PlayerHost, store, MainScope(), it) }
    }

    /**
     * The match picker for the first fixture, once its page has asked (the `unsure` scene): `match`
     * opens it as the page's button does, `match-fix` from the More menu, `match-found` after a
     * search that finds the show, `match-empty` after one that finds nothing.
     */
    private suspend fun stageMatch(store: AppStore, sheet: String) {
        val id = 900001u
        store.loadDetail(id)
        withTimeoutOrNull(30_000) { store.match.state.first { it.asks(id) } }
        store.match.open(id, fromMenu = sheet != "match")
        val source = store.match.state.value.source(id)?.first
        when (sheet) {
            "match-found" -> store.searchMatch(id, "Stardust Wayfarer", source)
            "match-empty" -> store.searchMatch(id, "Zzyzx Nothing", source)
            else -> return
        }
        withTimeoutOrNull(10_000) { store.match.state.first { it.searches[id].let { s -> s != null && s !is MatchSearch.Searching } } }
        // The sheet's own animation in.
        delay(800)
    }

    /** How long a `code` extra's prompt has left: `expires=<seconds>`, 0 for an expired code, or 899. */
    private fun codeSecondsLeft(intent: Intent): Long = intent.getStringExtra("expires")?.toLongOrNull()?.coerceAtLeast(0) ?: 899
}

/** The phone's player, as the party scenes steer it. */
private class PlayerStage(private val app: KaeriApp) : PartyStage {
    private val player get() = app.player

    override val playing: Boolean get() = player.isPresented && player.hasPicture && player.duration != null

    override fun holdControls(on: Boolean) = player.hold("shot", on) // i18n-skip

    override fun hideControls() = player.hideControls()

    override fun seekTo(seconds: Double) = player.seekTo(seconds)

    override fun setPlaying(playing: Boolean) = player.setPlaying(playing)

    override fun hideChat(on: Boolean) = app.store.party.setHideChat(app, on)
}
