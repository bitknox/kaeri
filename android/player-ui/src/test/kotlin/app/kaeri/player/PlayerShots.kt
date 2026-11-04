package app.kaeri.player

import androidx.activity.ComponentActivity
import androidx.compose.foundation.Image
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalInspectionMode
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onRoot
import app.kaeri.testkit.captureRoboImage
import com.github.takahirom.roborazzi.roborazziSystemPropertyOutputDirectory
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import org.junit.After
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import app.kaeri.design.KaeriTheme
import app.kaeri.engine.core.Translation
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToNode
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipeUp
import app.kaeri.engine.model.CommentBlock
import app.kaeri.engine.model.CommentSpan
import app.kaeri.engine.model.CommentView
import app.kaeri.engine.model.DiscussionState
import app.kaeri.engine.model.DiscussionView
import app.kaeri.engine.model.Entry
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.Msg
import app.kaeri.engine.model.PostState
import app.kaeri.ui.Destination
import app.kaeri.ui.DiscussionSheet
import app.kaeri.ui.LocalNavigator
import app.kaeri.ui.Navigator
import app.kaeri.engine.model.NextEpisode
import app.kaeri.engine.model.PlayerOp
import app.kaeri.engine.model.PlayerUi
import app.kaeri.engine.model.SkipKind
import app.kaeri.engine.model.SkipMarkFixture
import app.kaeri.engine.model.SkipNotice
import app.kaeri.engine.model.SkipOrigin
import app.kaeri.engine.model.SkipPrompt
import app.kaeri.engine.ffi.foundSkipScene
import app.kaeri.engine.ffi.skipMarkFixture
import app.kaeri.engine.model.FoundSkipFixture
import app.kaeri.engine.model.SkipFindPause
import app.kaeri.engine.model.TimeRange
import app.kaeri.engine.model.TimelineMark
import app.kaeri.player.ui.PlayerScreen
import app.kaeri.store.AppStore
import app.kaeri.testkit.FixtureArt
import app.kaeri.ui.LocalAppStore

/**
 * The player's boards, drawn on the JVM at the board's size, 915 × 412 dp in landscape: the model
 * on a scripted mpv, put where the board has it, and the title's wide art standing in for mpv's
 * picture, which only a device draws. mpv draws the subtitles too, so the shots have none.
 *
 * `./gradlew :player:recordRoborazziFixturesDebug` writes them to android/.shots/roborazzi/.
 */
@OptIn(com.github.takahirom.roborazzi.ExperimentalRoborazziApi::class)
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = PlayerShots.PHONE_LANDSCAPE)
class PlayerShots {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)

    /** The model's clock, held where a shot puts it. */
    private var time = 0L

    @After
    fun tearDown() = scope.cancel()

    /** AndroidPlayer: the controls up in the opening, at 2:31 of 24:02. */
    @Test
    fun phone_player() = shoot("phone-player") { model, mpv ->
        mpv.emit(MpvEvent.property("time-pos", MpvValue.Real(151.0)))
        model.apply(Event.PlayerUi(1u, PlayerUi.Skip(SkipKind.OPENING, 220.0)))
    }

    /** AndroidPlayerUpNext: the ending under way at 21:33, Up Next two seconds into its countdown, the controls hidden. */
    @Test
    fun phone_player_up_next() = shoot("phone-player-up-next") { model, mpv ->
        model.apply(
            Event.PlayerUi(
                1u,
                PlayerUi.UpNext(NextEpisode("8", "A Town Without Night", "${FixtureArt.HOST}ep/900001/8.jpg"), true, 1291.0, 10u, 3u),
            ),
        )
        mpv.emit(MpvEvent.property("time-pos", MpvValue.Real(1293.0)))
        model.hideControls()
    }

    /** Waiting for data mid-episode, the controls hidden: the buffering line along the video's top edge. */
    @Test
    fun phone_player_buffering() = shoot("phone-player-buffering") { model, mpv ->
        mpv.emit(MpvEvent.property("time-pos", MpvValue.Real(612.0)))
        mpv.emit(MpvEvent.property("paused-for-cache", MpvValue.Flag(true)))
        time = 1_500
        model.hideControls()
        check(model.showsBuffering) { "the shot is of a buffering player" }
    }

    /** The seek line with the engine's opening and ending marks and what is buffered ahead of the played part. */
    @Test
    fun phone_player_buffered() = shoot("phone-player-buffered") { model, mpv ->
        mpv.emit(MpvEvent.property("time-pos", MpvValue.Real(151.0)))
        model.apply(Event.PlayerTimeline(1u, listOf(TimelineMark(SkipKind.OPENING, 130.0, 220.0), TimelineMark(SkipKind.ENDING, 1291.0, 1381.0))))
        model.apply(Event.PlayerBuffered(1u, listOf(TimeRange(0.0, 480.0), TimeRange(700.0, 760.0))))
    }

    /** The starting screen (T2): the banner art under the dim, the indicator and its line, before the first frame. */
    @Test
    fun phone_player_starting() = shoot("phone-player-starting", picture = false) { model, _ ->
        model.apply(
            Event.PlayerUi(
                1u,
                PlayerUi.Starting(900_001u, TITLE, "7", EPISODE, "${FixtureArt.HOST}cover/900001-xl.jpg", "${FixtureArt.HOST}banner/900001.jpg"),
            ),
        )
    }

    /** The store of the shot being taken, for the steps that act on the screen. */
    private lateinit var store: AppStore
    private lateinit var model: PlayerModel
    private var playing: ScriptedMpv? = null
    private val sent = mutableListOf<app.kaeri.engine.model.Command>()

    /**
     * AndroidPlayerMore: past 85 % of the episode, the More button in the top bar, opened onto the
     * discussion with its count and that opening it pauses.
     */
    @Test
    fun phone_player_more_menu() = shoot("phone-player-more-menu", discussionSheet = true, after = {
        compose.onNodeWithContentDescription("More").performClick()
    }) { _, mpv ->
        mpv.emit(MpvEvent.property("time-pos", MpvValue.Real(1250.0)))
    }

    @Test
    fun phone_player_more_menu_waits_for_the_watched_point_to_offer_the_discussion() {
        shoot("phone-player-before-85", discussionSheet = true) { _, mpv -> mpv.emit(MpvEvent.property("time-pos", MpvValue.Real(151.0))) }
        compose.onNodeWithContentDescription("More").performClick()
        compose.waitForIdle()
        compose.onNodeWithText("Discussion").assertDoesNotExist()
    }

    /**
     * The quick panel under Subtitles and audio on a phone held sideways: the board's 360 dp side
     * sheet on the end edge, over the playing picture, with the look picker and its rows.
     */
    @Test
    fun phone_player_subtitle_panel() = shoot("phone-player-subtitle-panel", after = { openSubtitlePanel() }) { _, mpv ->
        mpv.emit(MpvEvent.property("time-pos", MpvValue.Real(1250.0)))
    }.also {
        val bounds = compose.onNodeWithTag("side-sheet").fetchSemanticsNode().boundsInRoot
        val screen = compose.onAllNodes(androidx.compose.ui.test.isRoot())[0].fetchSemanticsNode().boundsInRoot
        val density = compose.activity.resources.displayMetrics.density
        check(kotlin.math.abs(bounds.width / density - 360f) < 1f) { "the side sheet is 360 dp wide: ${bounds.width / density}" }
        check(bounds.right >= screen.right - 1f) { "the side sheet sits on the end edge" }
        check(bounds.height >= screen.height - 1f) { "the side sheet is as tall as the screen" }
    }

    /** The same panel held upright stays the bottom sheet. */
    @Test
    @Config(sdk = [36], qualifiers = PHONE_PORTRAIT)
    fun phone_player_subtitle_panel_portrait() = shoot("phone-player-subtitle-panel-portrait", after = { openSubtitlePanel() }) { _, mpv ->
        mpv.emit(MpvEvent.property("time-pos", MpvValue.Real(1250.0)))
    }.also {
        compose.onNodeWithTag("side-sheet").assertDoesNotExist()
    }

    private fun openSubtitlePanel() {
        store.apply(app.kaeri.testkit.Scene.named("subtitle_styled").events.filter { it is Event.Settings })
        compose.onNodeWithContentDescription("Subtitles, audio and picture").performClick()
        compose.waitForIdle()
        compose.mainClock.advanceTimeBy(600)
        compose.waitForIdle()
        // The list holds the tracks first, so the style rows are scrolled into view as a thumb would.
        compose.onNodeWithText("Subtitles, audio and picture").performTouchInput { swipeUp() }
        compose.waitForIdle()
        compose.onNode(androidx.compose.ui.test.hasScrollAction()).performScrollToNode(androidx.compose.ui.test.hasText("Style"))
        compose.waitForIdle()
    }

    /**
     * The Episodes sheet over a show whose table is cached: compact rows, and under them a footer
     * with the playing episode's name and three lines of its synopsis.
     */
    @Test
    @Config(sdk = [36], qualifiers = PHONE_PORTRAIT)
    fun phone_player_episodes() = shoot("phone-player-episodes", after = {
        compose.onNodeWithContentDescription("Episodes").performClick()
        compose.waitForIdle()
        compose.mainClock.advanceTimeBy(600)
        compose.waitForIdle()
    }) { model, mpv ->
        mpv.emit(MpvEvent.property("time-pos", MpvValue.Real(151.0)))
        val rows = app.kaeri.testkit.SynopsisFixtures.table(900_001u)
        store.apply(listOf(Event.Episodes(900_001u, rows)))
        model.apply(
            Event.PlayerUi(
                1u,
                PlayerUi.Menus(
                    episodes = app.kaeri.testkit.SynopsisFixtures.menu(rows),
                    sources = emptyList(),
                    translation = Translation.SUB,
                    upscaling = app.kaeri.engine.model.MenuChoices("off"),
                    toneMapping = null,
                    nightMode = false,
                ),
            ),
        )
    }

    /** The discussion sheet opened from the player: the video pauses where it is. */
    @Test
    fun phone_player_discussion_sheet() = shoot("phone-player-discussion", discussionSheet = true, after = {
        compose.onNodeWithContentDescription("More").performClick()
        compose.waitForIdle()
        compose.onNodeWithText("Discussion").performClick()
        compose.waitForIdle()
        // The engine is not there to answer, so the pause shows as the command it was sent.
        check(sent.any { it.toString().contains("TogglePause") }) { "opening the discussion pauses the video: $sent" }
        playing!!.emit(MpvEvent.property("pause", MpvValue.Flag(true)))
    }) { _, mpv ->
        mpv.emit(MpvEvent.property("time-pos", MpvValue.Real(1250.0)))
    }

    // Skip intro: the marking panel at each step, the prompts, the sharing question and the list.

    private fun marking(name: String, state: SkipMarkFixture, at: Double = 151.0) = shoot(name) { model, mpv ->
        mpv.emit(MpvEvent.property("time-pos", MpvValue.Real(at)))
        model.apply(Event.PlayerSkipMark(1u, skipMarkFixture(state)))
    }

    @Test
    fun phone_skip_start() = marking("phone-skip-start", SkipMarkFixture.START, 90.0)

    @Test
    fun phone_skip_end() = marking("phone-skip-end", SkipMarkFixture.END, 150.0)

    @Test
    fun phone_skip_review() = marking("phone-skip-review", SkipMarkFixture.REVIEW)

    @Test
    fun phone_skip_too_short() = marking("phone-skip-too-short", SkipMarkFixture.TOO_SHORT)

    @Test
    fun phone_skip_too_long() = marking("phone-skip-too-long", SkipMarkFixture.TOO_LONG)

    @Test
    fun phone_skip_replaces() = marking("phone-skip-replaces", SkipMarkFixture.REPLACES)

    @Test
    fun phone_skip_shares() = marking("phone-skip-shares", SkipMarkFixture.SHARES)

    @Test
    fun phone_skip_no_entry() = marking("phone-skip-no-entry", SkipMarkFixture.NO_ENTRY)

    @Test
    fun phone_skip_editing() = marking("phone-skip-editing", SkipMarkFixture.EDITING)

    @Test
    fun phone_skip_previewing() = marking("phone-skip-previewing", SkipMarkFixture.PREVIEWING)

    /** "Was that the opening?" in the corner, the controls hidden. */
    @Test
    fun phone_skip_was_opening() = shoot("phone-skip-was-opening") { model, mpv ->
        mpv.emit(MpvEvent.property("time-pos", MpvValue.Real(260.0)))
        model.apply(Event.PlayerSkipPrompt(1u, SkipPrompt.WasOpening(130.0, 220.0)))
        time = 1_000
        model.hideControls()
    }

    /** "Wrong opening times?" after Undo on AniSkip's times. */
    @Test
    fun phone_skip_wrong() = shoot("phone-skip-wrong") { model, mpv ->
        mpv.emit(MpvEvent.property("time-pos", MpvValue.Real(135.0)))
        model.apply(Event.PlayerSkipPrompt(1u, SkipPrompt.WrongTimes(SkipKind.OPENING)))
        time = 1_000
        model.hideControls()
    }

    /** The question the first Save asks. */
    @Test
    fun phone_skip_share() = shoot("phone-skip-share") { model, mpv ->
        mpv.emit(MpvEvent.property("time-pos", MpvValue.Real(151.0)))
        model.apply(Event.PlayerSkipPrompt(1u, SkipPrompt.Share))
    }

    /** The notice after a Save. */
    @Test
    fun phone_skip_saved() = shoot("phone-skip-saved") { model, mpv ->
        mpv.emit(MpvEvent.property("time-pos", MpvValue.Real(225.0)))
        model.apply(Event.PlayerTimeline(1u, listOf(TimelineMark(SkipKind.OPENING, 130.0, 220.0, SkipOrigin.YOURS))))
        model.apply(Event.PlayerSkipNotice(1u, SkipNotice.Saved(SkipKind.OPENING)))
    }

    /** More > Skip times: an opening from AniSkip, an ending of the viewer's, no recap. */
    @Test
    fun phone_skip_times() = shoot("phone-skip-times", after = {
        compose.onNodeWithContentDescription("More").performClick()
        compose.waitForIdle()
        compose.onNodeWithText("Skip times").performClick()
        compose.waitForIdle()
    }) { model, mpv ->
        mpv.emit(MpvEvent.property("time-pos", MpvValue.Real(151.0)))
        model.apply(
            Event.PlayerTimeline(
                1u,
                listOf(
                    TimelineMark(SkipKind.OPENING, 130.0, 220.0, SkipOrigin.ANI_SKIP),
                    TimelineMark(SkipKind.ENDING, 1291.0, 1381.0, SkipOrigin.YOURS),
                ),
            ),
        )
    }

    // Found skip times: each of the board's states, from the engine's fixture scenes.

    /**
     * A found-times fixture state: the player where the scene has it, its skip button or undo, the
     * marks, the card and the review as the engine says them, with the controls up or hidden.
     */
    private fun found(name: String, state: FoundSkipFixture, paused: Boolean = false, after: () -> Unit = {}) {
        val scene = foundSkipScene(state)
        shoot(name, episode = scene.episode.toString(), length = scene.length, after = {
            if (scene.panel) {
                compose.onNodeWithContentDescription("More").performClick()
                compose.waitForIdle()
                compose.onNodeWithText("Skip times").performClick()
                compose.waitForIdle()
            }
            after()
        }) { model, mpv ->
            mpv.emit(MpvEvent.property("time-pos", MpvValue.Real(scene.position)))
            for (ui in scene.ui) model.apply(Event.PlayerUi(1u, ui))
            model.apply(Event.PlayerTimeline(1u, scene.marks))
            scene.prompt?.let { model.apply(Event.PlayerSkipPrompt(1u, it)) }
            scene.mark?.let { model.apply(Event.PlayerSkipMark(1u, it)) }
            if (paused) {
                val view = store.settings.view.value
                if (view != null) store.apply(listOf(Event.Settings(view.copy(skipFindPaused = scene.paused ?: SkipFindPause.LOW_POWER))))
            }
            if (!scene.controls && !scene.panel) {
                time = 1_000
                model.hideControls()
            }
        }
    }

    @Test
    fun phone_found_skip() = found("phone-found-skip", FoundSkipFixture.SKIP)

    @Test
    fun phone_found_controls() = found("phone-found-controls", FoundSkipFixture.CONTROLS)

    @Test
    fun phone_found_prompt() = found("phone-found-prompt", FoundSkipFixture.PROMPT)

    @Test
    fun phone_found_saved() = found("phone-found-saved", FoundSkipFixture.SAVED)

    @Test
    fun phone_found_never() = found("phone-found-never", FoundSkipFixture.NEVER)

    @Test
    fun phone_found_review() = found("phone-found-review", FoundSkipFixture.REVIEW)

    @Test
    fun phone_found_nudged() = found("phone-found-nudged", FoundSkipFixture.NUDGED)

    @Test
    fun phone_found_undo() = found("phone-found-undo", FoundSkipFixture.UNDO)

    @Test
    fun phone_found_panel() = found("phone-found-panel", FoundSkipFixture.PANEL)

    @Test
    fun phone_found_panel_paused() = found("phone-found-panel-paused", FoundSkipFixture.PANEL_PAUSED, paused = true)

    /** The More menu with the two skip entries. */
    @Test
    fun phone_skip_more_menu() = shoot("phone-skip-more", after = {
        compose.onNodeWithContentDescription("More").performClick()
    }) { _, mpv ->
        mpv.emit(MpvEvent.property("time-pos", MpvValue.Real(151.0)))
    }

    private fun shoot(
        name: String,
        picture: Boolean = true,
        discussionSheet: Boolean = false,
        after: () -> Unit = {},
        episode: String = "7",
        length: Double = 1442.0,
        at: (PlayerModel, ScriptedMpv) -> Unit,
    ) {
        store = AppStore(scope, clock = { 0L }, artworkWithoutEngine = FixtureArt::bytes)
        if (discussionSheet) {
            store.apply(Event.DiscussionFound(900_001u, 7u, 214u))
            store.apply(Event.Discussion(THREAD))
        }
        var mpv: ScriptedMpv? = null
        // The clock held, so the controls stay up for the shot as after a tap.
        model = PlayerModel(send = { sent += it }, backend = { ScriptedMpv().also { mpv = it; playing = it } }, scope = scope, now = { time }, hwdec = "no")
        compose.runOnUiThread {
            model.apply(Event.PlaybackStaged(900_001u, TITLE, episode, EPISODE, 1.0))
            model.apply(Event.PlayerOps(1u, listOf(PlayerOp.Open(true, null, null), PlayerOp.Load("/media/episode.mkv", emptyList()))))
            model.apply(Event.PlayerUi(1u, PlayerUi.Episode(episode, "Episode $episode", EPISODE, TITLE, Translation.SUB)))
        }
        compose.waitForIdle()
        val art = FixtureArt.bytes("${FixtureArt.HOST}banner/900001.jpg")!!
        val still = android.graphics.BitmapFactory.decodeByteArray(art, 0, art.size).asImageBitmap()
        compose.setContent {
            KaeriTheme {
                // As a preview draws it: what moves for ever, such as the buffering line, as a still.
                val nav = androidx.navigation.compose.rememberNavController()
                CompositionLocalProvider(
                    LocalAppStore provides store,
                    LocalNavigator provides androidx.compose.runtime.remember(nav) { Navigator(nav, Destination.Home) },
                    LocalInspectionMode provides true,
                ) {
                    PlayerScreen(model, video = { _, modifier, _ ->
                        Image(still, contentDescription = null, contentScale = ContentScale.Crop, modifier = modifier)
                    })
                    val target by store.discussion.target.collectAsState()
                    target?.let { DiscussionSheet(ENTRY, it.episode) { store.discussion.dismiss() } }
                }
            }
        }
        compose.waitForIdle()
        compose.runOnUiThread {
            val m = mpv!!
            m.emit(MpvEvent.of(MpvNative.Event.START_FILE))
            m.emit(MpvEvent.of(MpvNative.Event.FILE_LOADED))
            m.emit(MpvEvent.property("duration", MpvValue.Real(length)))
            m.emit(MpvEvent.property("pause", MpvValue.Flag(false)))
            m.emit(MpvEvent.property("video-params", MpvValue.parse("""{"w":1920,"h":1080,"dw":1920,"dh":1080}""")!!))
            m.emit(MpvEvent.property("chapter-list", MpvValue.parse(CHAPTERS)!!))
            if (picture) m.emit(MpvEvent.of(MpvNative.Event.PLAYBACK_RESTART))
            model.poke()
            at(model, m)
        }
        compose.mainClock.advanceTimeBy(1_000)
        compose.waitForIdle()
        awaitArt(store)
        after()
        compose.mainClock.advanceTimeBy(600)
        compose.waitForIdle()
        compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/$name.png")
    }

    /** Wait until the art the screen asked for has landed. */
    private fun awaitArt(store: AppStore) {
        val deadline = System.currentTimeMillis() + 10_000
        while (store.artwork.hasPending && System.currentTimeMillis() < deadline) {
            compose.waitForIdle()
            Thread.sleep(20)
        }
        compose.mainClock.advanceTimeBy(500)
        compose.waitForIdle()
    }

    companion object {
        /** The player boards: 915 × 412 dp, landscape, at 420 dpi, as the emulator's phone turned. */
        const val PHONE_LANDSCAPE = "w915dp-h412dp-land-420dpi"

        /** The same phone upright. */
        const val PHONE_PORTRAIT = "w412dp-h915dp-port-420dpi"

        private val ENTRY = Entry(
            id = 900_001u, title = "Hoshikuzu no Tabibito", secondary = null, format = "TV", episodes = 13u, year = 2026u, score = null,
            studio = null, synopsis = "", coverUrl = "${FixtureArt.HOST}cover/900001-xl.jpg", bannerUrl = null,
            genres = emptyList(), availableOn = emptyList(), progress = null, airingIn = null,
            airingEpisode = null, nextEpisode = null, nextAiringIn = null, lastAired = null, resume = null,
            related = emptyList(), recommended = emptyList(), trailerUrl = null, listStatus = null,
            titleRomaji = "Hoshikuzu no Tabibito", titleEnglish = null, titleNative = null, season = null, startDate = null,
            endDate = null, source = null, adult = false, detail = null,
            status = null, synopsisSpans = emptyList(),
        )

        private fun comment(id: UInt, who: String, text: String, hours: Long, likes: UInt, replies: List<CommentView> = emptyList()) = CommentView(
            id = id, author = who, avatar = null, createdAt = 0L, age = Msg.DiscussionAgeHours(hours),
            body = listOf(CommentBlock(spans = listOf(CommentSpan(text = text)))), likeCount = likes,
            pending = PostState.POSTED, replies = replies,
        )

        private val THREAD = DiscussionView(
            media = 900_001u, episode = 7u, title = "Episode 7 Discussion", commentCount = 214u,
            url = "https://anilist.example/forum/thread/1",
            comments = listOf(
                comment(11u, "lampwick", "Nobody spoke for almost a minute, and it was the best part of the episode.", 2, 41u, listOf(comment(12u, "tamsin", "And the music came back on the exact frame her lantern caught.", 1, 13u))),
                comment(14u, "ferrylog", "The lantern light on the faces is drawn frame by frame, not a filter.", 1, 8u),
            ),
            state = DiscussionState.Ready(more = false), canPost = false, signedIn = false, loadImages = false,
        )

        const val TITLE = "Hoshikuzu no Tabibito"
        const val EPISODE = "The Lantern Road"

        /** The board's episode: the opening 2:10 to 3:40, the ending 21:31 to 23:01. */
        const val CHAPTERS = """[{"title":"Prologue","time":0},{"title":"Opening","time":130},{"title":"Part A","time":220},""" +
            """{"title":"Ending","time":1291},{"title":"Preview","time":1381}]"""
    }
}
