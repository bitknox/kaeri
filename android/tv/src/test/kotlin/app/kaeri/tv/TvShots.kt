package app.kaeri.tv

import app.kaeri.i18n.R
import android.content.ComponentName
import android.view.KeyEvent
import androidx.activity.ComponentActivity
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.performClick
import app.kaeri.design.ObiSilk
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalInspectionMode
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isFocused
import androidx.compose.ui.test.isRoot
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onLast
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.performSemanticsAction
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.pressKey
import androidx.compose.ui.test.requestFocus
import app.kaeri.testkit.captureRoboImage
import app.kaeri.testkit.captureScreenRoboImage
import com.github.takahirom.roborazzi.roborazziSystemPropertyOutputDirectory
import java.time.Clock
import java.time.Instant
import java.time.ZoneId
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.rules.ExternalResource
import org.junit.rules.RuleChain
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import app.kaeri.store.TorrentWords
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import app.kaeri.engine.core.AnilistId
import app.kaeri.engine.ffi.FfiLinkSent
import app.kaeri.tv.ui.ActionsHost
import app.kaeri.tv.ui.ActionsPanel
import app.kaeri.tv.ui.notInstalledPanel
import app.kaeri.engine.core.Translation
import app.kaeri.engine.ffi.playerStatsFixture
import app.kaeri.engine.model.ContentFor
import app.kaeri.engine.model.ContentList
import app.kaeri.engine.model.Ending
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.HiddenTitle
import app.kaeri.engine.model.MenuChoices
import app.kaeri.engine.model.MenuSource
import app.kaeri.engine.model.NextEpisode
import app.kaeri.engine.model.Notice
import app.kaeri.engine.model.NoticeKind
import app.kaeri.engine.model.PlayerOp
import app.kaeri.engine.model.PlayerUi
import app.kaeri.engine.model.SkipMarkFixture
import app.kaeri.engine.model.SkipOrigin
import app.kaeri.engine.model.SkipPrompt
import app.kaeri.engine.ffi.foundSkipScene
import app.kaeri.engine.ffi.skipMarkFixture
import app.kaeri.engine.model.FoundSkipFixture
import app.kaeri.engine.model.SkipFindPause
import app.kaeri.engine.model.ProxyStatus
import app.kaeri.engine.model.ProxyView
import app.kaeri.engine.core.SourceCandidate
import app.kaeri.engine.model.SettingKey
import app.kaeri.engine.model.TorrentView
import app.kaeri.engine.model.SourceNote
import app.kaeri.engine.model.SourceOption
import app.kaeri.engine.model.SkipKind
import app.kaeri.engine.model.StatsFixture
import app.kaeri.engine.ffi.playerStatsFixture
import app.kaeri.player.MpvCall
import app.kaeri.player.MpvEvent
import app.kaeri.player.MpvNative
import app.kaeri.player.MpvValue
import app.kaeri.player.PlayerModel
import app.kaeri.player.ScriptedMpv
import app.kaeri.store.AppClock
import app.kaeri.store.AppStore
import app.kaeri.store.HAS_TESTING_ENGINE
import app.kaeri.testkit.FixtureArt
import app.kaeri.testkit.Scene
import app.kaeri.testkit.PartyStates
import app.kaeri.tv.player.CHROME_HIDE
import app.kaeri.tv.player.TvPlayerScreen
import app.kaeri.tv.remote.PairPage
import app.kaeri.tv.remote.PairRequest
import app.kaeri.tv.remote.PairRequestDialog
import app.kaeri.tv.shell.Destination
import app.kaeri.tv.shell.TvNavigator
import app.kaeri.tv.shell.TvRoot
import app.kaeri.tv.signin.LocalPageHost
import app.kaeri.tv.signin.SignInPage
import app.kaeri.tv.ui.TV_ART_BUDGET
import app.kaeri.tv.ui.TvArtDecoder
import app.kaeri.tv.ui.TvTheme
import app.kaeri.engine.core.StyledSubtitles
import app.kaeri.engine.core.SubtitleBackground
import app.kaeri.engine.core.SubtitleColour
import app.kaeri.engine.core.SubtitleEdge
import app.kaeri.engine.core.SubtitleFont
import app.kaeri.engine.core.SubtitleThickness
import app.kaeri.engine.core.SettingValue
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.InvitePreview
import app.kaeri.engine.model.InvitedStore
import app.kaeri.engine.model.SubtitleStyle
import app.kaeri.engine.model.TimeRange
import app.kaeri.engine.model.TimelineMark

/**
 * The TV's screens, drawn on the JVM at 1920 × 1080 from a scene the testing engine recorded, and
 * the D-pad's focus rules walked through them: the first focus, the rail, Back to the opener.
 *
 * `./gradlew :tv:recordRoborazziFixturesDebug` writes them to android/.shots/roborazzi/.
 */
@OptIn(com.github.takahirom.roborazzi.ExperimentalRoborazziApi::class)
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = TvShots.TV)
class TvShots {
    private val context = RuntimeEnvironment.getApplication()
    val compose = createAndroidComposeRule<ComponentActivity>()

    /**
     * The app's manifest has no plain activity for the rule to start, as a library's test manifest
     * would: it is declared to Robolectric's package manager first.
     */
    @get:Rule
    val rules: RuleChain = RuleChain.outerRule(
        object : ExternalResource() {
            override fun before() {
                val app = RuntimeEnvironment.getApplication()
                shadowOf(app.packageManager).addActivityIfNotPresent(ComponentName(app, ComponentActivity::class.java))
            }
        },
    ).around(compose)

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)

    @Before
    fun setUp() = assumeTrue(HAS_TESTING_ENGINE)

    @After
    fun tearDown() {
        scope.cancel()
        AppClock.clock = Clock.systemDefaultZone()
    }

    @Test
    fun continue_home() = shoot("tv-home", scene = HOME_WATCHING) {
        assertTrue("the hero's play button takes the first focus", focusedText().startsWith("Resume") || focusedText().startsWith("Play"))
    }

    /** Home's second case, a recommendation, and the third's not-found. */
    @Test
    fun home_suggested() = shoot("tv-home-suggested", scene = "home-suggested")

    @Test
    fun home_not_found() = shoot("tv-home-not-found", scene = "home-notfound")

    @Test
    fun rail_opens_on_left() = shoot("tv-rail") {
        press(Key.DirectionLeft)
        assertEquals("Home", focusedText())
    }

    @Test
    fun rail_right_returns_to_the_content() = shoot(null) {
        val first = focusedText()
        press(Key.DirectionLeft)
        press(Key.DirectionRight)
        assertEquals(first, focusedText())
    }

    @Test
    fun search() = shoot("tv-search", query = "hoshi")

    @Test
    fun title() = shoot("tv-title", title = 900001u)

    /**
     * Source for this title from the title page's More, by the D-pad alone: the panel lists Automatic
     * and each source with its coverage or note, focus on the chosen one, long names cut short.
     */
    @Test
    fun title_source_picker() = shoot("tv-source-picker", title = 900001u) { openSourcePicker() }

    /** The same panel at 720p, where the rows must still fit the panel's 400 dp column. */
    @Test
    @Config(qualifiers = "w640dp-h360dp-land-television-xhdpi")
    fun title_source_picker_720p() = shoot("tv-source-picker-720p", title = 900001u) { openSourcePicker() }

    /** Nothing pinned: Automatic is the chosen row and holds the focus, the long name cut short. */
    @Test
    fun title_source_picker_automatic() = shoot("tv-source-picker-automatic", title = 900001u) { openSourcePicker(pinned = false) }

    @Test
    @Config(qualifiers = "w640dp-h360dp-land-television-xhdpi")
    fun title_source_picker_automatic_720p() = shoot("tv-source-picker-automatic-720p", title = 900001u) { openSourcePicker(pinned = false) }

    /** Center on a source pins it, closes the panel and sends the choice. */
    @Test
    fun source_picker_picks_with_center() = shoot(null, title = 900001u) {
        openSourcePicker()
        // Up from the chosen release, past its source, to the one above.
        repeat(5) { if (!focusedText().startsWith("Mukashi Archive")) press(Key.DirectionUp) }
        assertTrue("focus reaches the long name: ${focusedText()}", focusedText().startsWith("Mukashi Archive"))
        press(Key.DirectionCenter)
        assertTrue(
            "the choice is sent",
            shot.waiting.contains(Command.SetProviderPreference(900001u, "mukashi-archive")),
        )
        assertTrue("the panel closed", compose.onAllNodesWithText("Automatic").fetchSemanticsNodes().isEmpty())
    }

    private fun pickerOptions(pinned: Boolean = true) = listOf(
        SourceOption("hotaru-open-archive", "Hotaru Open Archive", 13u, 13u, false, null, emptyList(), null),
        SourceOption("mukashi-archive", "Mukashi Archive With A Very Long Name That Cannot Possibly Fit", 6u, 13u, false, null, emptyList(), null),
        SourceOption(
            "torrents", "Torrent playback", 12u, 13u, pinned, null,
            listOf(
                SourceCandidate("batch-1080", "torrents", "Stardust Wayfarer complete batch", 1080u, "From a trusted uploader", "14.2 GiB", true, false, true),
                SourceCandidate("single-1080", "torrents", "Stardust Wayfarer 01", 1080u, null, "1.1 GiB", false, false, false),
                SourceCandidate("single-720", "torrents", "Stardust Wayfarer 01", 720u, null, "480 MiB", false, false, false),
            ),
            "single-1080".takeIf { pinned },
        ),
        SourceOption("fumi-archive", "Fumi Archive", 0u, 13u, false, SourceNote.Resting, emptyList(), null),
        SourceOption("old-reel", "Old Reel", 0u, 13u, false, SourceNote.Failed("Timed out after 20 seconds"), emptyList(), null),
    )

    /** Focus More, open it, and choose Source for this title, as the remote would. */
    private fun openSourcePicker(pinned: Boolean = true) {
        compose.onNodeWithContentDescription("More").requestFocus()
        compose.waitForIdle()
        press(Key.DirectionCenter)
        downTo("Source for this title")
        press(Key.DirectionCenter)
        shot.apply(Event.SourceOptions(900001u, pickerOptions(pinned)))
        settle()
        val first = if (pinned) "Stardust Wayfarer 01" else "Automatic"
        assertTrue("the chosen row holds the focus: ${focusedText()}", focusedText().startsWith(first))
    }

    @Test
    fun calendar() = shoot("tv-calendar", start = Destination.Calendar)

    @Test
    fun seasonal() = shoot("tv-seasonal", start = Destination.Seasonal)

    @Test
    fun library() = shoot("tv-library", start = Destination.Library)

    @Test
    fun settings() = shoot("tv-settings", start = Destination.Settings)

    /** Back from a title page puts the focus back on the card that opened it. */
    @Test
    fun back_returns_to_the_opener() = shoot(null) {
        press(Key.DirectionDown)
        val opener = focusedText()
        press(Key.DirectionCenter)
        settle()
        assertTrue("a title page is open", opener != focusedText())
        compose.runOnUiThread { compose.activity.onBackPressedDispatcher.onBackPressed() }
        settle()
        assertEquals(opener, focusedText())
    }

    /** The player in the opening at 2:31 of 24:02, the chrome up as after a key. */
    @Test
    fun player() = playerShot("tv-player") {}

    /** The seek bar with the engine's opening and ending marks and what is buffered ahead of the played part. */
    @Test
    fun player_buffered() = playerShot("tv-player-buffered") { model ->
        compose.runOnUiThread {
            model.apply(Event.PlayerTimeline(1u, listOf(TimelineMark(SkipKind.OPENING, 130.0, 220.0), TimelineMark(SkipKind.ENDING, 1291.0, 1381.0))))
            model.apply(Event.PlayerBuffered(1u, listOf(TimeRange(0.0, 480.0), TimeRange(700.0, 760.0))))
        }
    }

    /** The subtitles stand above the chrome while it is up, and go back down once it hides. */
    @Test
    fun subtitles_rise_above_the_controls() {
        fun subPos() = (playerMpv!!.calls.filterIsInstance<MpvCall.Set>().lastOrNull { it.property == "sub-pos" }?.value as? MpvValue.Text)?.value?.toDoubleOrNull()
        var up: Double? = null
        playerShot(null) {
            up = subPos()
            time = CHROME_HIDE + 1_000
        }
        // The model's ticker reads its clock on the main looper, every 250 ms.
        org.robolectric.Shadows.shadowOf(android.os.Looper.getMainLooper()).idleFor(java.time.Duration.ofSeconds(1))
        compose.mainClock.advanceTimeBy(2_000)
        compose.waitForIdle()
        assertTrue("lifted while the chrome is up: $up", up != null && up!! < 80.0)
        assertEquals(100.0, subPos()!!, 0.5)
    }

    /** The chrome gone again, the skip offer holding Center in the bottom right. */
    @Test
    fun player_skip() = playerShot("tv-player-skip") { time = CHROME_HIDE + 1_000 }

    /** A service's page this TV cannot open: the QR code, and Send to phone while a phone is paired. */
    @Test
    fun not_installed() = notInstalledShot("tv-not-installed", null)

    @Test
    fun send_to_phone_sent() = notInstalledShot("tv-send-sent", FfiLinkSent.Sent(1u))

    @Test
    fun send_to_phone_held() = notInstalledShot("tv-send-held", FfiLinkSent.Held)

    @Test
    fun send_to_phone_refused() = notInstalledShot("tv-send-refused", FfiLinkSent.Refused)

    private fun notInstalledShot(name: String, outcome: FfiLinkSent?) {
        val host = ActionsHost()
        val context = androidx.test.core.app.ApplicationProvider.getApplicationContext<android.content.Context>()
        host.open(notInstalledPanel(context, "Hikari Library", "https://watch.example/title/900001", outcome?.let { { _: String -> it } }))
        compose.setContent { TvTheme { Box(Modifier.fillMaxSize().background(ObiSilk.Ground)) { ActionsPanel(host) } } }
        compose.waitForIdle()
        if (outcome != null) {
            compose.onNodeWithText("Send to phone").performSemanticsAction(SemanticsActions.OnClick)
            compose.waitForIdle()
        }
        compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/$name.png")
    }

    // Skip intro: the marking panel at each step, the prompts, the sharing question and the list.

    private fun tvMarking(name: String, state: SkipMarkFixture, at: Double = 151.0) = playerShot(name) { model ->
        compose.runOnUiThread {
            playerMpv!!.emit(MpvEvent.property("time-pos", MpvValue.Real(at)))
            model.apply(Event.PlayerSkipMark(1u, skipMarkFixture(state)))
        }
        compose.mainClock.advanceTimeBy(1_000)
        compose.waitForIdle()
    }

    @Test
    fun skip_start() = tvMarking("tv-skip-start", SkipMarkFixture.START, 90.0)

    @Test
    fun skip_end() = tvMarking("tv-skip-end", SkipMarkFixture.END, 150.0)

    @Test
    fun skip_review() = tvMarking("tv-skip-review", SkipMarkFixture.REVIEW)

    @Test
    fun skip_too_short() = tvMarking("tv-skip-too-short", SkipMarkFixture.TOO_SHORT)

    @Test
    fun skip_shares() = tvMarking("tv-skip-shares", SkipMarkFixture.SHARES)

    /** "Wrong opening times?" in the corner, Mark focused beside Report. */
    @Test
    fun skip_wrong() = playerShot("tv-skip-wrong") { model ->
        time = CHROME_HIDE + 1_000
        compose.runOnUiThread { model.apply(Event.PlayerSkipPrompt(1u, SkipPrompt.WrongTimes(SkipKind.OPENING))) }
        compose.mainClock.advanceTimeBy(1_000)
        compose.waitForIdle()
        assertEquals("Mark", focusedText())
    }

    /** "Was that the opening?" with Mark it focused. */
    @Test
    fun skip_was_opening() = playerShot("tv-skip-was-opening") { model ->
        time = CHROME_HIDE + 1_000
        compose.runOnUiThread { model.apply(Event.PlayerSkipPrompt(1u, SkipPrompt.WasOpening(130.0, 220.0))) }
        compose.mainClock.advanceTimeBy(1_000)
        compose.waitForIdle()
        assertEquals("Mark it", focusedText())
    }

    /** The question the first Save asks. */
    @Test
    fun skip_share() = playerShot("tv-skip-share") { model ->
        compose.runOnUiThread { model.apply(Event.PlayerSkipPrompt(1u, SkipPrompt.Share)) }
        compose.mainClock.advanceTimeBy(1_000)
        compose.waitForIdle()
    }

    /** The Skip times panel from the button row. */
    @Test
    fun skip_times() = playerShot("tv-skip-times") { model ->
        compose.runOnUiThread {
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
        repeat(8) { if (focusedText() != "Skip times") press(Key.DirectionRight) }
        assertEquals("Skip times", focusedText())
        press(Key.DirectionCenter)
    }

    // Found skip times: the engine's fixture scenes on the TV, as the board draws them.

    private fun tvFound(name: String, state: FoundSkipFixture, focus: String? = null, hide: Boolean = true) = playerShot(name) { model ->
        val scene = foundSkipScene(state)
        if (hide) time = CHROME_HIDE + 1_000
        compose.runOnUiThread {
            playerMpv!!.emit(MpvEvent.property("time-pos", MpvValue.Real(scene.position)))
            for (ui in scene.ui) model.apply(Event.PlayerUi(1u, ui))
            model.apply(Event.PlayerTimeline(1u, scene.marks))
            scene.prompt?.let { model.apply(Event.PlayerSkipPrompt(1u, it)) }
            scene.mark?.let { model.apply(Event.PlayerSkipMark(1u, it)) }
        }
        compose.mainClock.advanceTimeBy(1_000)
        compose.waitForIdle()
        focus?.let { assertEquals(it, focusedText()) }
    }

    @Test
    fun found_skip() = tvFound("tv-found-skip", FoundSkipFixture.SKIP)

    /** The card, focus on Yes. */
    @Test
    fun found_prompt() = tvFound("tv-found-prompt", FoundSkipFixture.PROMPT, focus = "Yes")

    /** Right walks the card's buttons from Yes, and the card keeps the focus. */
    @Test
    fun found_prompt_focus_walks() {
        tvFound("tv-found-prompt-walk", FoundSkipFixture.PROMPT, focus = "Yes")
        press(Key.DirectionRight)
        assertEquals("Adjust", focusedText())
        press(Key.DirectionRight)
        assertEquals("Don't ask again", focusedText())
    }

    @Test
    fun found_saved() = tvFound("tv-found-saved", FoundSkipFixture.SAVED)

    @Test
    fun found_never() = tvFound("tv-found-never", FoundSkipFixture.NEVER)

    /** Adjust's review, focus on Watch the edges. */
    @Test
    fun found_review() = tvFound("tv-found-review", FoundSkipFixture.REVIEW, focus = "Watch the edges", hide = false)

    @Test
    fun found_undo() = tvFound("tv-found-undo", FoundSkipFixture.UNDO)

    /** Settings, Playback, with finding paused by low storage. */
    @Test
    fun found_settings_paused() = shoot("tv-found-settings-paused", section = "Playback", then = {
        val view = shot.settings.view.value!!
        shot.apply(listOf(Event.Settings(view.copy(skipFindPaused = SkipFindPause.STORAGE))))
        settle()
        // The row is below the first screenful: the way down is how the list scrolls.
        press(Key.DirectionRight)
        var presses = 0
        while (compose.onAllNodesWithText("Paused until", substring = true).fetchSemanticsNodes().isEmpty() && presses++ < 14) press(Key.DirectionDown)
        compose.onNodeWithText("Find skip times from the audio").assertExists()
        compose.onNodeWithText("Paused until", substring = true).assertExists()
    })

    /** Back closes the card unanswered. */
    @Test
    fun found_prompt_back_dismisses() = tvFound("tv-found-prompt-back", FoundSkipFixture.PROMPT).also {
        compose.runOnUiThread {
            listOf(KeyEvent.ACTION_DOWN, KeyEvent.ACTION_UP).forEach { compose.activity.dispatchKeyEvent(KeyEvent(it, KeyEvent.KEYCODE_BACK)) }
        }
        compose.mainClock.advanceTimeBy(600)
        compose.waitForIdle()
        assertTrue("Back dismissed the card", sentCommands.any { it.toString().contains("DismissSkipPrompt") })
    }

    /** Back steps the marking panel back, then closes it. */
    @Test
    fun back_closes_the_marking_panel() = playerShot(null) { model ->
        compose.runOnUiThread { model.apply(Event.PlayerSkipMark(1u, skipMarkFixture(SkipMarkFixture.REVIEW))) }
        compose.mainClock.advanceTimeBy(1_000)
        compose.waitForIdle()
        compose.runOnUiThread {
            listOf(KeyEvent.ACTION_DOWN, KeyEvent.ACTION_UP).forEach {
                compose.activity.dispatchKeyEvent(KeyEvent(it, KeyEvent.KEYCODE_BACK))
            }
        }
        compose.mainClock.advanceTimeBy(600)
        compose.waitForIdle()
        assertTrue("Back was sent to the engine", sentCommands.any { it.toString().contains("CancelSkipMark") })
    }

    /** Settings › Remotes › Pair a phone: the QR code, the code beside it, and Done. */
    @Test
    fun pair() {
        compose.setContent {
            TvTheme {
                PairPage(name = "Den", code = "042917", link = PAIR_LINK, address = "192.168.1.20:47811", onClose = {})
            }
        }
        compose.waitForIdle()
        compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/tv-pair.png")
    }

    /** A phone asked to pair: its code over the browse UI, Cancel focused. */
    @Test
    fun pair_request() {
        compose.setContent {
            TvTheme { PairRequestDialog(PairRequest("042917", "Mika's phone"), onCancel = {}) }
        }
        compose.waitForIdle()
        captureScreenRoboImage("${roborazziSystemPropertyOutputDirectory()}/tv-pair-request.png")
    }

    /** Up Next as the ending starts: its card bottom right, Play now focused. */
    @Test
    fun player_up_next() = playerShot("tv-player-up-next") { model ->
        time = CHROME_HIDE + 1_000
        compose.runOnUiThread {
            model.apply(Event.PlayerUi(1u, PlayerUi.UpNext(NextEpisode("8", "A Town Without Night", null), true, 148.0, 10u, 3u)))
        }
        compose.mainClock.advanceTimeBy(1_000)
        compose.waitForIdle()
        assertEquals("Play now", focusedText())
    }

    /** The Info panel: the episode, then how it plays. */
    @Test
    fun player_info() = playerShot("tv-player-info") { model ->
        // The chrome is up with focus on Play: Right along the buttons to Info.
        repeat(6) { if (focusedText() != "Info") press(Key.DirectionRight) }
        assertEquals("Info", focusedText())
        // The engine's summary, as it says it from the file playing, a stream here.
        compose.runOnUiThread { model.apply(Event.PlayerStats(1u, playerStatsFixture(StatsFixture.PLAYING))) }
        press(Key.DirectionCenter)
    }

    /** The remote's Back closes an open panel, and the player stays. */
    @Test
    fun back_closes_a_player_panel() = playerShot(null) { model ->
        repeat(6) { if (focusedText() != "Info") press(Key.DirectionRight) }
        press(Key.DirectionCenter)
        // The panel's title alone: the chrome under it is hidden from a screen reader while it is open.
        assertEquals("the panel is open", 1, compose.onAllNodesWithText("Info").fetchSemanticsNodes().size)
        assertEquals("the panel is titled", 1, compose.onAllNodes(hasText("Info") and SemanticsMatcher.keyIsDefined(SemanticsProperties.Heading)).fetchSemanticsNodes().size)
        // Through the activity, as a remote's key comes, not the dispatcher.
        compose.runOnUiThread {
            listOf(KeyEvent.ACTION_DOWN, KeyEvent.ACTION_UP).forEach {
                compose.activity.dispatchKeyEvent(KeyEvent(it, KeyEvent.KEYCODE_BACK))
            }
        }
        compose.mainClock.advanceTimeBy(600)
        compose.waitForIdle()
        assertEquals("the panel is closed", 1, compose.onAllNodesWithText("Info").fetchSemanticsNodes().size)
        assertEquals("its title is gone", 0, compose.onAllNodes(hasText("Info") and SemanticsMatcher.keyIsDefined(SemanticsProperties.Heading)).fetchSemanticsNodes().size)
        assertTrue("the player is still up", model.isPresented)
    }

    /**
     * The page's address shows once its server has bound on an IO thread, in real time, which the
     * test's clock does not wait for: a loaded machine reached the assertion first.
     */
    private fun awaitText(text: String) {
        compose.waitUntil(timeoutMillis = 10_000) { compose.onAllNodesWithText(text, substring = true).fetchSemanticsNodes().isNotEmpty() }
        compose.onNodeWithText(text, substring = true).assertExists()
    }

    /** First launch's Welcome: Nagare, what Kaeri is, Continue focused. */
    @Test
    fun first_launch() = shoot("tv-first-launch", firstLaunch = true, scene = "fresh") {
        assertEquals("Continue", focusedText())
    }

    /** Settings, Sources with the demo: its row, from the demo_only scene. */
    @Test
    fun first_play_demo_source() = shoot("tv-first-play-demo-source", section = "Sources", scene = "demo_only") {
        press(Key.DirectionRight)
    }

    /** Settings, About with saving crash reports on and two reports kept. */
    @Test
    fun first_play_crash_saved() = shoot("tv-first-play-crash-saved", section = "About", scene = "crash_saved") {
        press(Key.DirectionRight)
        downTo("Report a bug")
    }

    /** Report a bug, full screen on the TV. */
    @Test
    fun first_play_report() = shoot("tv-first-play-report", section = "About", scene = "crash_saved") {
        press(Key.DirectionRight)
        downTo("Report a bug")
        press(Key.DirectionCenter)
        settle()
    }

    /** Home offline: the quiet line under the spotlight says from when it was kept. */
    @Test
    fun home_offline() = shoot("tv-home-offline", scene = HOME_WATCHING, offline = true) {
        val view = checkNotNull(shot.home.view.value)
        shot.apply(listOf(Event.Home(view.copy(offline = true, offlineSince = 1_761_900_000L))))
        compose.mainClock.advanceTimeBy(1_000)
        compose.waitForIdle()
        assertTrue(compose.onAllNodesWithText("Offline. Showing Home from", substring = true).fetchSemanticsNodes().isNotEmpty())
    }

    /** Settings › Add a plugin store, from Settings: the QR code for a phone, and Type the link. */
    /** Joining with an invite that works more than once: the store, then Your name (optional) above Join. */
    @Test
    fun join_group() = shoot("tv-join-group", start = Destination.Settings, pageHost = "192.0.2.10") {
        compose.runOnUiThread { nav.openJoinStore() }
        compose.mainClock.advanceTimeBy(1_000)
        val invited = InvitedStore(
            "http://127.0.0.1:8787/join/club#key", "Hilltop Plugins", "http://127.0.0.1:8787", "127.0.0.1", "9f3c1a2b77de0c41",
            alreadyJoined = false, owners = listOf("Ren"), plugins = emptyList(), deviceLabel = "TV", namesJoiner = true,
        )
        compose.runOnUiThread { shot.apply(listOf(Event.StorePreview(InvitePreview.Store(invited)))) }
        compose.mainClock.advanceTimeBy(1_000)
        compose.waitForIdle()
    }

    @Test
    fun join_store() = shoot("tv-join-store", start = Destination.Settings, pageHost = "192.0.2.10") {
        compose.runOnUiThread { nav.openJoinStore() }
        compose.mainClock.advanceTimeBy(3_000)
        compose.waitForIdle()
        assertEquals("Type the link", focusedText())
    }

    /** Kitsu's sign-in: the QR code for its page, and the whole address with the page's key. */
    @Test
    fun sign_in_kitsu() = shoot("tv-sign-in-kitsu", start = Destination.Settings, pageHost = "192.0.2.10") {
        compose.runOnUiThread { nav.openSignIn("kitsu") }
        compose.mainClock.advanceTimeBy(3_000)
        // The page's server starts on an IO thread, in real time the test clock does not hold.
        compose.waitUntil(timeoutMillis = 15_000) { compose.onAllNodesWithText("#k=", substring = true).fetchSemanticsNodes().isNotEmpty() }
        compose.waitForIdle()
        awaitText("#k=")
    }

    /** Settings › General › Language: the sheet of languages, a flag beside each name. */
    @Test
    fun language_sheet() = shoot("tv-language", section = "General") {
        press(Key.DirectionRight)
        downTo("Language")
        press(Key.DirectionCenter)
        settle()
    }

    /** Settings › General › Hidden titles: the sheet, each title with when it was hidden and Restore. */
    @Test
    fun hidden_titles() = shoot("tv-hidden-titles", section = "General") {
        press(Key.DirectionRight)
        settle()
        val now = System.currentTimeMillis() / 1000
        val hidden = listOf(900002u, 900003u, 900004u).mapNotNull { shot.titles[it] }.mapIndexed { i, entry ->
            HiddenTitle(entry = entry, untilAirs = i == 0, hiddenAt = now - (i + 1) * 3 * 86_400L)
        }
        shot.apply(listOf(Event.HiddenTitles(hidden)))
        downTo("Hidden titles")
        press(Key.DirectionCenter)
        settle()
    }

    /** Settings › Sources, torrents off: the switch, and that torrents connect directly unless a proxy is set. */
    @Test
    fun torrents_off() = shoot("tv-torrents-off", section = "Sources") {
        press(Key.DirectionRight)
        downTo("Torrent playback")
    }

    /** Torrents on with no proxy: they connect directly, and the proxy toggle is off with no check row. */
    @Test
    fun torrents_direct() = shoot("tv-torrents-direct", section = "Sources", torrents = ProxyStatus.DIRECT) {
        press(Key.DirectionRight)
        downTo("Route torrent traffic through a proxy")
    }

    /** The switch turns torrents on at once, with nothing asked. */
    @Test
    fun torrents_switch_turns_on_at_once() = shoot(null, section = "Sources") {
        press(Key.DirectionRight)
        downTo("Torrent playback")
        press(Key.DirectionCenter)
        settle()
        assertTrue(shot.waiting.contains(Command.SetSetting(SettingKey.TORRENTS, SettingValue.Bool(true))))
    }

    /**
     * The proxy toggle turned on: the form, the page for the details (here on a TV no phone can
     * reach, as the JVM has no network), then Type it for the fields as pills and the password as dots.
     */
    @Test
    fun torrents_proxy_form() = shoot(null, section = "Sources", torrents = ProxyStatus.DIRECT) {
        press(Key.DirectionRight)
        downTo("Route torrent traffic through a proxy")
        press(Key.DirectionCenter)
        settle()
        assertEquals("Type it", focusedText())
        compose.onNodeWithText("Route torrent traffic through a proxy").assertExists()
        compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/tv-torrents-proxy-form.png")
        compose.onNodeWithText("Type it").performSemanticsAction(SemanticsActions.OnClick)
        settle()
        // The proxy's host and port, its user name, its password.
        val fields = compose.onAllNodes(hasSetTextAction())
        fields[0].performTextInput("proxy.example")
        fields[1].performTextInput("1080")
        fields[3].performTextInput("not-shown")
        fields[0].requestFocus()
        settle()
        // Drawn as dots, and marked a password, so TalkBack does not read it out.
        fields[3].assert(SemanticsMatcher.keyIsDefined(SemanticsProperties.Password))
        compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/tv-torrents-type-it.png")
        // Done: the details, whole, as rows, and focus back on Type it.
        compose.onNodeWithText("Done").performSemanticsAction(SemanticsActions.OnClick)
        settle()
        assertEquals("Type it", focusedText())
        compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/tv-torrents-ready.png")

        // Save checks the new proxy first, the other buttons locked and focus kept on it.
        repeat(4) { if (focusedText() != "Save") press(Key.DirectionLeft) }
        assertEquals("Save", focusedText())
        press(Key.DirectionCenter)
        compose.mainClock.advanceTimeBy(1_000)
        settle()
        assertEquals("Save", focusedText())
        compose.onNodeWithText(context.getString(TorrentWords.CHECKING_PROXY)).assertExists()
        compose.onNodeWithText("Type it").assertIsNotEnabled()
        compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/tv-torrents-proxy-form-checking.png")

        // Failed: nothing saved, the form stays open, and focus where it was.
        compose.runOnUiThread { shot.apply(listOf(Event.ProxyChecked(ProxyStatus.FAILED))) }
        settle()
        assertEquals("Save", focusedText())
        compose.onNodeWithText("Not saved", substring = true).assertExists()
        compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/tv-torrents-proxy-form-failed.png")
    }

    /** The page behind a real QR code with its address; after 10 minutes the code expires. */
    @Test
    fun torrents_proxy_page() = shoot(null, section = "Sources", torrents = ProxyStatus.DIRECT, pageHost = "192.0.2.10") {
        press(Key.DirectionRight)
        downTo("Route torrent traffic through a proxy")
        press(Key.DirectionCenter)
        settle()
        awaitText("http://192.0.2.10:")
        compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/tv-torrents-change.png")
        compose.mainClock.advanceTimeBy(SignInPage.LIFETIME_MILLIS + 1_000)
        settle()
        compose.onNodeWithText("Code expired").assertExists()
        compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/tv-torrents-code-expired.png")
    }

    /** Turning the toggle off removes the proxy at once. */
    @Test
    fun torrents_proxy_toggle_off() = shoot(null, section = "Sources", torrents = ProxyStatus.PASSED, proxy = true) {
        press(Key.DirectionRight)
        downTo("Route torrent traffic through a proxy")
        press(Key.DirectionCenter)
        settle()
        assertTrue(shot.waiting.contains(Command.SetTorrentProxy(null)))
    }

    /** Torrents on behind a proxy, the check passed: its water dot and Check now. */
    @Test
    fun torrents_on() = shoot("tv-torrents-on", section = "Sources", torrents = ProxyStatus.PASSED, proxy = true) {
        press(Key.DirectionRight)
        downTo("Proxy check")
    }

    /** The check under way. */
    @Test
    fun torrents_checking() = shoot("tv-torrents-checking", section = "Sources", torrents = ProxyStatus.CHECKING, proxy = true) {
        press(Key.DirectionRight)
        downTo("Proxy check")
    }

    /** A failed check: its vermilion dot, and that torrents start once it passes. */
    @Test
    fun torrents_check_failed() = shoot("tv-torrents-check-failed", section = "Sources", torrents = ProxyStatus.FAILED, proxy = true) {
        press(Key.DirectionRight)
        downTo("Proxy check")
    }

    /** A saved proxy: Edit proxy… beside the proxy toggle, where it is and who it lets in. */
    @Test
    fun torrents_edit_proxy_row() = shoot("tv-torrents-edit-proxy-row", section = "Sources", torrents = ProxyStatus.PASSED, proxy = true) {
        press(Key.DirectionRight)
        downTo("Edit proxy…")
    }

    /**
     * Edit proxy: the saved proxy filled in, Save focused. Save checks first: Checking the proxy,
     * then a failed check's reason with the saved proxy kept, then a passed one saves and closes.
     */
    @Test
    fun torrents_edit_proxy() = shoot(null, section = "Sources", torrents = ProxyStatus.PASSED, proxy = true) {
        press(Key.DirectionRight)
        downTo("Edit proxy…")
        press(Key.DirectionCenter)
        settle()
        assertEquals("Save", focusedText())
        compose.onNodeWithText("proxy.example:1080 · as mika").assertExists()
        compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/tv-torrents-edit-proxy.png")

        press(Key.DirectionCenter)
        compose.mainClock.advanceTimeBy(1_000)
        settle()
        assertEquals("Save", focusedText())
        compose.onNodeWithText(context.getString(TorrentWords.CHECKING_PROXY)).assertExists()
        compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/tv-torrents-edit-proxy-checking.png")

        compose.runOnUiThread { shot.apply(listOf(Event.ProxyChecked(ProxyStatus.FAILED))) }
        settle()
        assertEquals("Save", focusedText())
        compose.onNodeWithText("Not saved", substring = true).assertExists()
        compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/tv-torrents-edit-proxy-failed.png")

        // Tried again and passed: saved, and back on the row it came from.
        press(Key.DirectionCenter)
        settle()
        compose.runOnUiThread { shot.apply(listOf(Event.ProxyChecked(ProxyStatus.PASSED))) }
        settle()
        assertTrue(focusedText().startsWith("Edit proxy…"))
        compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/tv-torrents-edit-proxy-saved.png")
    }

    /** Closing a torrents page puts focus back on the row that opened it, not on the section's first row. */
    @Test
    fun torrents_page_returns_to_its_row() = shoot(null, section = "Sources", torrents = ProxyStatus.PASSED, proxy = true) {
        press(Key.DirectionRight)
        downTo("Edit proxy…")
        press(Key.DirectionCenter)
        settle()
        compose.onNodeWithText("Cancel").performSemanticsAction(SemanticsActions.OnClick)
        settle()
        assertTrue(focusedText().startsWith("Edit proxy…"))
    }

    /** Edit proxy's Type it: the proxy's fields, and focus back on Type it once they are done. */
    @Test
    fun torrents_edit_proxy_type_it() = shoot(null, section = "Sources", torrents = ProxyStatus.PASSED, proxy = true) {
        press(Key.DirectionRight)
        downTo("Edit proxy…")
        press(Key.DirectionCenter)
        settle()
        compose.onNodeWithText("Type it").performSemanticsAction(SemanticsActions.OnClick)
        settle()
        // Named whatever they hold, the filled ones as the empty.
        compose.onNodeWithContentDescription("Proxy host").assertExists()
        compose.onNodeWithContentDescription("Password, if asked").assertExists()
        compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/tv-torrents-edit-proxy-type-it.png")
        compose.onNodeWithText("Done").performSemanticsAction(SemanticsActions.OnClick)
        settle()
        assertEquals("Type it", focusedText())
    }

    /** Offline, Edit proxy's check cannot reach the proxy: it fails, and the saved proxy is kept. */
    @Test
    fun torrents_edit_proxy_offline() = shoot(null, section = "Sources", torrents = ProxyStatus.PASSED, proxy = true, offline = true) {
        press(Key.DirectionRight)
        downTo("Edit proxy…")
        press(Key.DirectionCenter)
        settle()
        press(Key.DirectionCenter)
        // A slow network: half a minute in, still checking.
        compose.mainClock.advanceTimeBy(30_000)
        settle()
        compose.onNodeWithText(context.getString(TorrentWords.CHECKING_PROXY)).assertExists()
        compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/tv-torrents-edit-proxy-slow.png")
        compose.runOnUiThread { shot.apply(listOf(Event.ProxyChecked(ProxyStatus.FAILED))) }
        settle()
        assertEquals("Save", focusedText())
        compose.onNodeWithText("Not saved", substring = true).assertExists()
        compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/tv-torrents-edit-proxy-failed-offline.png")
    }

    /** Edit proxy's Use a phone: the page for the proxy alone, here on a TV no phone can reach, focus on Type it. */
    @Test
    fun torrents_edit_proxy_page() = shoot(null, section = "Sources", torrents = ProxyStatus.PASSED, proxy = true) {
        press(Key.DirectionRight)
        downTo("Edit proxy…")
        press(Key.DirectionCenter)
        settle()
        press(Key.DirectionRight)
        assertEquals("Use a phone", focusedText())
        press(Key.DirectionCenter)
        settle()
        assertEquals("Type it", focusedText())
        compose.onNodeWithText("Scan with your phone, then change the proxy on the page that opens.").assertExists()
        compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/tv-torrents-edit-proxy-page.png")
    }

    /** Seeding, under Torrents while they are on, with its best-effort line. */
    @Test
    fun torrents_seeding() = shoot("tv-torrents-seeding", section = "Sources", torrents = ProxyStatus.DIRECT) {
        press(Key.DirectionRight)
        downTo("Upload limit")
    }

    /** A torrent starting: 23 s in, what the wait is on, and another source offered. */
    @Test
    fun player_torrent_starting() = startingShot("tv-player-torrent-starting", waited = 23_000)

    /** The first seconds of a start, before the wait says what it is on. */
    @Test
    fun player_starting() = startingShot("tv-player-starting", waited = 1_000)

    /** A torrent that stalled mid-episode: the stall card, Switch source focused. */
    @Test
    fun player_torrent_stalled() {
        playerShot(null) { model ->
            time = CHROME_HIDE + 1_000
            compose.runOnUiThread { model.apply(Event.PlayerUi(1u, PlayerUi.Stall(12uL))) }
            compose.mainClock.advanceTimeBy(1_000)
            compose.waitForIdle()
            assertEquals("Switch source", focusedText())
        }
        // The chrome goes and the card keeps focus, drawn so.
        // A second at a time, so the chrome's fade runs its frames.
        // The model's ticker reads its clock on the main looper, so the looper runs too.
        repeat(3) {
            org.robolectric.Shadows.shadowOf(android.os.Looper.getMainLooper()).idleFor(java.time.Duration.ofSeconds(1))
            compose.mainClock.advanceTimeBy(1_000)
            compose.waitForIdle()
        }
        compose.onNodeWithText("Episodes").assertDoesNotExist()
        assertEquals("Switch source", focusedText())
        compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/tv-player-torrent-stalled.png")
    }

    /** The Source panel with torrents among the sources, the one playing marked. */
    @Test
    fun player_torrent_sources() = playerShot("tv-player-torrent-sources") { model ->
        compose.runOnUiThread {
            model.apply(
                Event.PlayerUi(
                    1u,
                    PlayerUi.Menus(
                        episodes = emptyList(),
                        sources = listOf(
                            MenuSource("0", "torrent", "1080p · torrent · 1", current = true),
                            MenuSource("1", "torrent", "1080p · torrent · 2"),
                            MenuSource("2", "alpha", "720p · HLS", health = "failed the last 3 times · 12 min ago"),
                        ),
                        translation = Translation.SUB,
                        upscaling = MenuChoices("off"),
                        toneMapping = null,
                        nightMode = false,
                    ),
                ),
            )
        }
        repeat(6) { if (focusedText() != "Source") press(Key.DirectionRight) }
        assertEquals("Source", focusedText())
        press(Key.DirectionCenter)
        settle()
    }

    /** A torrent that never started: the engine's reason, and another source focused. */
    @Test
    fun player_torrent_failed() {
        AppClock.clock = Clock.fixed(Instant.parse("2026-10-20T19:02:00Z"), ZONE)
        val store = AppStore(scope, decoder = TvArtDecoder, artworkBudget = TV_ART_BUDGET, clock = { 0L }, artworkWithoutEngine = FixtureArt::bytes)
        val model = PlayerModel(send = {}, backend = { ScriptedMpv() }, scope = scope, now = { time }, hwdec = "no")
        compose.runOnUiThread {
            model.apply(Event.PlaybackStaged(900_001u, TITLE, "7", EPISODE, 1.0))
            model.apply(Event.Toast(Notice(NoticeKind.ALERT, "no playable stream: waiting for metadata: deadline has elapsed", null)))
            model.apply(Event.PlaybackEnded(false, Ending.Failed, false))
        }
        compose.setContent {
            TvTheme { TvRoot(store, model, TvNavigator(), play = { _, _ -> }) }
        }
        settle()
        assertEquals("Choose another source", focusedText())
        compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/tv-player-torrent-failed.png")
    }

    /** A playback that never played: why, as the engine said it, and another source focused. */
    @Test
    fun player_failed() {
        AppClock.clock = Clock.fixed(Instant.parse("2026-10-20T19:02:00Z"), ZONE)
        val store = AppStore(scope, decoder = TvArtDecoder, artworkBudget = TV_ART_BUDGET, clock = { 0L }, artworkWithoutEngine = FixtureArt::bytes)
        val model = PlayerModel(send = {}, backend = { ScriptedMpv() }, scope = scope, now = { time }, hwdec = "no")
        compose.runOnUiThread {
            model.apply(Event.PlaybackStaged(900_001u, TITLE, "7", EPISODE, 1.0))
            model.apply(Event.Toast(Notice(NoticeKind.ALERT, "no playable stream: every release failed", null)))
            model.apply(Event.PlaybackEnded(false, Ending.Failed, false))
        }
        compose.setContent {
            TvTheme { TvRoot(store, model, TvNavigator(), play = { _, _ -> }) }
        }
        settle()
        assertEquals("Choose another source", focusedText())
        compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/tv-player-failed.png")
    }

    /** The starting screen [waited] ms after the engine said it is starting, before any frame. */
    private fun startingShot(name: String, waited: Long) {
        AppClock.clock = Clock.fixed(Instant.parse("2026-10-20T19:02:00Z"), ZONE)
        val store = AppStore(scope, decoder = TvArtDecoder, artworkBudget = TV_ART_BUDGET, clock = { 0L }, artworkWithoutEngine = FixtureArt::bytes)
        val model = PlayerModel(send = {}, backend = { ScriptedMpv() }, scope = scope, now = { time }, hwdec = "no")
        compose.runOnUiThread {
            model.apply(Event.PlaybackStaged(900_001u, TITLE, "7", EPISODE, 1.0))
            model.apply(
                Event.PlayerUi(
                    1u,
                    PlayerUi.Starting(900_001u, TITLE, "7", EPISODE, "${FixtureArt.HOST}cover/900001-xl.jpg", "${FixtureArt.HOST}banner/900001.jpg"),
                ),
            )
        }
        compose.setContent {
            TvTheme {
                CompositionLocalProvider(LocalInspectionMode provides true) {
                    TvRoot(store, model, TvNavigator(), play = { _, _ -> })
                }
            }
        }
        compose.waitForIdle()
        time = waited
        compose.runOnUiThread { model.poke() }
        settle()
        awaitArt(store)
        settle()
        compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/$name.png")
    }

    /** Down through the rows until the one starting with [label] has focus. */
    private fun downTo(label: String) {
        repeat(40) {
            if (focusedTexts().any { it.startsWith(label) }) return
            press(Key.DirectionDown)
        }
        assertTrue("focus reached $label, at ${focusedTexts()}", focusedTexts().any { it.startsWith(label) })
    }

    /** The model's clock, held where a shot puts it. */
    private var time = 0L

    /** What the last [playerShot]'s model sent to the engine. */
    private val sentCommands = mutableListOf<Command>()

    /** The mpv the last [playerShot] played on. */
    private var playerMpv: ScriptedMpv? = null

    /** The navigator of the root [shoot] drew. */
    private var nav = TvNavigator()

    private fun playerShot(name: String?, then: (PlayerModel) -> Unit) {
        AppClock.clock = Clock.fixed(Instant.parse("2026-10-20T19:02:00Z"), ZONE)
        val store = AppStore(scope, decoder = TvArtDecoder, artworkBudget = TV_ART_BUDGET, clock = { 0L }, artworkWithoutEngine = FixtureArt::bytes)
        var mpv: ScriptedMpv? = null
        val model = PlayerModel(send = { sentCommands += it }, backend = { ScriptedMpv().also { mpv = it; playerMpv = it } }, scope = scope, now = { time }, hwdec = "no")
        compose.runOnUiThread {
            model.subtitleStyle = SubtitleStyle(
                100u, SubtitleFont.DEFAULT, SubtitleColour.WHITE,
                SubtitleEdge.OUTLINE, SubtitleThickness.MEDIUM,
                SubtitleBackground.NO_BACKGROUND, 75u, 0u, StyledSubtitles.KEEP, false,
            )
            model.apply(Event.PlaybackStaged(900_001u, TITLE, "7", EPISODE, 1.0))
            model.apply(Event.PlayerOps(1u, listOf(PlayerOp.Open(true, null, null), PlayerOp.Load("/media/episode.mkv", emptyList()))))
            model.apply(Event.PlayerUi(1u, PlayerUi.Episode("7", "Episode 7", EPISODE, TITLE, Translation.SUB)))
        }
        compose.waitForIdle()
        val art = FixtureArt.bytes("${FixtureArt.HOST}banner/900001.jpg")!!
        val still = android.graphics.BitmapFactory.decodeByteArray(art, 0, art.size).asImageBitmap()
        compose.setContent {
            TvTheme {
                // As a preview draws it: the spinner, which turns for ever, as a still.
                CompositionLocalProvider(LocalInspectionMode provides true) {
                    TvRoot(store, model, TvNavigator(), play = { _, _ -> }, playerScreen = {
                        TvPlayerScreen(it, video = { modifier ->
                            Image(still, contentDescription = null, contentScale = ContentScale.Crop, modifier = modifier)
                        })
                    })
                }
            }
        }
        compose.waitForIdle()
        compose.runOnUiThread {
            val m = mpv!!
            m.emit(MpvEvent.of(MpvNative.Event.START_FILE))
            m.emit(MpvEvent.of(MpvNative.Event.FILE_LOADED))
            m.emit(MpvEvent.property("duration", MpvValue.Real(1442.0)))
            m.emit(MpvEvent.property("pause", MpvValue.Flag(false)))
            m.emit(MpvEvent.property("video-params", MpvValue.parse("""{"w":1920,"h":1080,"dw":1920,"dh":1080}""")!!))
            m.emit(MpvEvent.property("chapter-list", MpvValue.parse(CHAPTERS)!!))
            m.emit(MpvEvent.of(MpvNative.Event.PLAYBACK_RESTART))
            m.emit(MpvEvent.property("time-pos", MpvValue.Real(151.0)))
            model.apply(Event.PlayerUi(1u, PlayerUi.Skip(SkipKind.OPENING, 220.0)))
            model.poke()
        }
        compose.mainClock.advanceTimeBy(1_000)
        compose.waitForIdle()
        then(model)
        compose.mainClock.advanceTimeBy(1_000)
        compose.waitForIdle()
        if (name == null) return
        awaitArt(store)
        compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/$name.png")
    }

    /**
     * The app's root on [start] (or a title page, or Search with [query]), fed the scene's
     * events, then [then], then the capture when [name] is given.
     */
    // Watch together: the place after Library, the lobby with its QR code, and what a party starts on.

    @Test
    fun together() = shoot("tv-together", start = Destination.Together, scene = TOGETHER) {
        assertTrue("Start a party takes the first focus", focusedText().contains("Start a party"))
    }

    @Test
    fun together_in_party() = shoot("tv-together-in-party", start = Destination.Together, scene = TOGETHER) {
        compose.runOnUiThread { shot.apply(PartyStates.inRoom()) }
        settle()
        nav.pop()
    }

    @Test
    fun party_lobby() = shoot("tv-party-lobby", start = Destination.Together, scene = TOGETHER) {
        compose.runOnUiThread { shot.apply(PartyStates.inRoom()) }
    }

    @Test
    fun party_lobby_ready() = shoot("tv-party-lobby-ready", start = Destination.Together, scene = TOGETHER) {
        compose.runOnUiThread { shot.apply(PartyStates.inRoom(PartyStates.view(ready = true), PartyStates.members(ready = 1, youReady = true))) }
    }

    @Test
    fun party_lobby_starting() = shoot("tv-party-lobby-starting", start = Destination.Together, scene = TOGETHER) {
        compose.runOnUiThread { shot.apply(PartyStates.inRoom(PartyStates.view(ready = true, startingIn = 3u), PartyStates.members(ready = 2, youReady = true))) }
    }

    @Test
    fun party_pick() = shoot("tv-party-pick", start = Destination.Together, scene = TOGETHER) {
        press(Key.DirectionCenter)
    }

    private fun shoot(
        name: String?,
        start: Destination = Destination.Home,
        title: AnilistId? = null,
        query: String? = null,
        firstLaunch: Boolean = false,
        offline: Boolean = false,
        /** Whether every show on Continue was last watched dubbed. */
        dub: Boolean = false,
        scene: String = WATCHING,
        /** The Settings section to open at, by its name. */
        section: String? = null,
        /** Torrents on, with this the proxy check's last word (Direct: no proxy set). */
        torrents: ProxyStatus? = null,
        /** With torrents on, behind a saved SOCKS5 proxy rather than connecting directly. */
        proxy: Boolean = false,
        /** The TV's address on the network, so a QR page has one: none, as the JVM has no network, when null. */
        pageHost: String? = null,
        then: (() -> Unit)? = null,
    ) {
        val recorded = Scene.named(scene)
        AppClock.clock = Clock.fixed(recorded.recordedAt, ZONE)
        val store = AppStore(scope, decoder = TvArtDecoder, artworkBudget = TV_ART_BUDGET, clock = { 0L }, artworkWithoutEngine = FixtureArt::bytes)
        shot = store
        val events = recorded.events.filter(::replayed).map { if (dub) dubbed(it) else it }.map { if (torrents != null) torrentsOn(it, proxy) else it } +
            listOfNotNull(Event.Connectivity(true).takeIf { offline })
        // The proxy check's last word, said once.
        torrents?.let { store.apply(listOf(Event.TorrentProxy(it))) }
        store.apply(events)
        val player = PlayerModel(send = {}, backend = { ScriptedMpv() }, scope = scope, now = { 0L }, hwdec = "no")
        nav = TvNavigator(if (query != null) Destination.Search else start).apply {
            query?.let { typeSearch(it) }
            title?.let(::openTitle)
            section?.let(::goSettings)
        }
        val nav = nav
        compose.setContent {
            TvTheme {
                if (pageHost == null) {
                    TvRoot(store, player, nav, play = { _, _ -> }, firstLaunch = firstLaunch)
                } else {
                    CompositionLocalProvider(LocalPageHost provides { _ -> pageHost }) {
                        TvRoot(store, player, nav, play = { _, _ -> }, firstLaunch = firstLaunch)
                    }
                }
            }
        }
        compose.waitForIdle()
        store.apply(events)
        settle()
        // Once more for what a screen asks for after a wait, such as Search after its pause.
        store.apply(events)
        awaitArt(store)
        settle()
        then?.let {
            it()
            settle()
        }
        if (name != null) compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/$name.png")
    }


    /** Removing a media server keeps its page: the removal says so, Undo is under focus and brings it back. */
    @Test
    fun server_removal_offers_undo() = shoot("tv-server-removed", scene = "servers", then = {
        val id = shot.servers.rows.value.first().id
        nav.openServer(id)
        settle()
        compose.onNodeWithText("Remove", substring = true).performSemanticsAction(SemanticsActions.OnClick)
        settle()
        assertTrue(shot.servers.rows.value.none { it.id == id } || focusedText().startsWith("Undo"))
        assertEquals("Undo", focusedText())
    })

    /** A media server that did not answer says so in its row, with when it last did. */
    @Test
    fun server_not_answering() = shoot("tv-server-not-answering", scene = "sources_busy", section = "Sources") {
        compose.onNodeWithText("Not answering", substring = true).assertExists()
    }

    /** Search has an Available chip that narrows by where a title plays. */
    @Test
    fun search_available_chip() = shoot("tv-search-available", query = "hoshi", then = {
        compose.onNodeWithText("Available").assertExists()
    })

    private fun settle() {
        compose.mainClock.advanceTimeBy(3_000)
        compose.waitForIdle()
    }

    private fun press(key: Key) {
        // To the newest window: a panel or a dialog over the screen, when one is up.
        compose.onAllNodes(isRoot()).onLast().performKeyInput { pressKey(key) }
        compose.mainClock.advanceTimeBy(600)
        compose.waitForIdle()
    }

    /** The focused controls' texts, one a window: the screen's, and a dialog's over it. */
    private fun focusedTexts(): List<String> = compose.onAllNodes(isFocused()).fetchSemanticsNodes().map { node ->
        node.config.getOrNull(SemanticsProperties.Text).orEmpty().joinToString(" ") { it.text }
    }

    /** The focused control's text, its descendants' merged. */
    /** The store [shoot] made, for a test that has the engine answer partway. */
    private lateinit var shot: AppStore

    private fun focusedText(): String {
        val node: SemanticsNodeInteraction = compose.onNode(isFocused())
        val text = node.fetchSemanticsNode().config.getOrNull(SemanticsProperties.Text).orEmpty()
        return text.joinToString(" ") { it.text }
    }

    /** Wait until the art has stopped landing: decoded off the main thread, then kept on it. */
    private fun awaitArt(store: AppStore) {
        val deadline = System.currentTimeMillis() + 20_000
        var landed = -1
        var still = System.currentTimeMillis()
        while (System.currentTimeMillis() < deadline) {
            compose.waitForIdle()
            val now = store.artwork.landed.value
            if (now != landed) {
                landed = now
                still = System.currentTimeMillis()
            } else if (System.currentTimeMillis() - still > 500) {
                return
            }
            Thread.sleep(20)
        }
    }

    companion object {
        /** A 1080p TV: 960 × 540 dp at 320 dpi. */
        const val TV = "w960dp-h540dp-land-television-xhdpi"

        const val WATCHING = "watching"
        const val HOME_WATCHING = "home-watching"
        const val TOGETHER = "together"

        /** A pairing link as the engine writes one, with a made-up key. */
        const val PAIR_LINK = "kaeri-remote://pair?addr=192.168.1.20:47811&fp=" +
            "3f9a0c4e7b1d28e5a6c3f0b9d4e7a1c2b5f8e3d6a9c0b7e4f1a2d5c8b3e6f9a0&code=042917"

        val ZONE: ZoneId = ZoneId.of("Europe/Copenhagen")

        const val TITLE = "Hoshikuzu no Tabibito"
        const val EPISODE = "The Lantern Road"

        /** The opening 2:10 to 3:40, the ending 21:31 to 23:01. */
        const val CHAPTERS = """[{"title":"Prologue","time":0},{"title":"Opening","time":130},{"title":"Part A","time":220},""" +
            """{"title":"Ending","time":1291},{"title":"Preview","time":1381}]"""

        fun replayed(event: Event): Boolean = when (event) {
            is Event.JobStarted, is Event.JobNote, is Event.JobDone, is Event.Toast -> false
            else -> true
        }
    }
}

/** [event], when it is the settings, with torrents on, connecting directly or behind a [proxy]. */
private fun torrentsOn(event: Event, proxy: Boolean = false): Event = when (event) {
    is Event.Settings -> Event.Settings(
        event.v1.copy(
            rows = event.v1.rows.map { if (it.key == SettingKey.TORRENTS) it.copy(value = "on") else it },
            sources = event.v1.sources.copy(torrent = true),
            torrent = TorrentView(if (proxy) ProxyView("proxy.example", 1080u, "mika") else null),
        ),
    )
    else -> event
}

/** [event], when it is a list of titles, with every one last watched dubbed, as the engine would send them. */
private fun dubbed(event: Event): Event = when (event) {
    is Event.Detail -> Event.Detail(event.v1.copy(watchedIn = Translation.DUB))
    is Event.Browse -> Event.Browse(event.v1.map { rail -> rail.copy(entries = rail.entries.map { it.copy(watchedIn = Translation.DUB) }) })
    is Event.CalendarBackfill -> event.copy(entries = event.entries.map { it.copy(watchedIn = Translation.DUB) })
    is Event.Content -> (event.content as? ContentList.Entries)
        ?.let { event.copy(content = it.copy(v1 = it.v1.map { entry -> entry.copy(watchedIn = Translation.DUB) })) }
        ?: event
    else -> event
}
