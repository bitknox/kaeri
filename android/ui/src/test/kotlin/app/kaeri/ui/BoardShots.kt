package app.kaeri.ui

import app.kaeri.i18n.R
import android.view.ViewGroup
import org.robolectric.RuntimeEnvironment
import androidx.activity.ComponentActivity
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.getAllSemanticsNodes
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.hasAnyDescendant
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasScrollToKeyAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onFirst
import androidx.compose.ui.test.onLast
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performSemanticsAction
import androidx.compose.ui.test.performScrollToKey
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipeUp
import androidx.compose.ui.unit.dp
import androidx.core.graphics.Insets
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import app.kaeri.testkit.captureRoboImage
import app.kaeri.testkit.captureScreenRoboImage
import androidx.compose.ui.test.longClick
import app.kaeri.engine.model.ContentFor
import app.kaeri.engine.model.ContentList
import com.github.takahirom.roborazzi.roborazziSystemPropertyOutputDirectory
import java.time.Clock
import java.time.ZoneId
import kotlin.math.roundToInt
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assume.assumeTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import app.kaeri.design.KaeriTheme
import app.kaeri.engine.core.AnilistId
import app.kaeri.engine.core.Translation
import app.kaeri.engine.core.SourceCandidate
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.EpisodeRow
import app.kaeri.engine.model.SectionKind
import app.kaeri.engine.model.SpotlightCase
import app.kaeri.engine.model.SourceNote
import app.kaeri.engine.model.SourceOption
import app.kaeri.engine.model.SearchQuery
import app.kaeri.store.AppClock
import app.kaeri.store.AppStore
import app.kaeri.store.HAS_TESTING_ENGINE
import app.kaeri.testkit.FixtureArt
import app.kaeri.testkit.Scene
import app.kaeri.testkit.SynopsisFixtures
import app.kaeri.testkit.PartyStates
import app.kaeri.engine.model.PartyFound
import app.kaeri.engine.model.PartySources
import app.kaeri.engine.model.PartyConnection

/**
 * Each board's screen, drawn on the JVM from a scene the testing engine recorded, at the board's
 * size: no emulator. The store is the app's own, fed the scene's events with no engine behind it,
 * and the clock held at the moment the scene was recorded, so every run draws the same pixels.
 *
 * `./gradlew :ui:recordRoborazziFixturesDebug` writes them to android/.shots/roborazzi/, and
 * android/scripts/pairs.sh sets them beside the boards.
 */
@OptIn(com.github.takahirom.roborazzi.ExperimentalRoborazziApi::class)
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36])
class BoardShots {
    private val context = RuntimeEnvironment.getApplication()
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)

    @Before
    fun setUp() {
        // The fixtures flavour's: the scenes are the testing engine's, and so are its titles.
        assumeTrue(HAS_TESTING_ENGINE)
    }

    @After
    fun tearDown() {
        scope.cancel()
        AppClock.clock = Clock.systemDefaultZone()
    }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_home() = shoot("phone-home", HOME_WATCHING)

    // Home's other cases and states, on the scenes recorded with Home's invented world.

    @Test
    @Config(qualifiers = PHONE)
    fun phone_home_suggested() = shoot("phone-home-suggested", "home-suggested")

    @Test
    @Config(qualifiers = PHONE)
    fun phone_home_oneleft() = shoot("phone-home-oneleft", "home-oneleft")

    @Test
    @Config(qualifiers = PHONE)
    fun phone_home_continue_device() = shoot("phone-home-continue-device", WATCHING, device = true)

    @Test
    @Config(qualifiers = PHONE)
    fun phone_home_continue_device_cards() = shoot("phone-home-continue-device-cards", WATCHING, device = true, screen = true) {
        compose.onRoot().performTouchInput { swipeUp(startY = bottom * 0.8f, endY = top + height * 0.2f, durationMillis = 300) }
        compose.waitForIdle()
    }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_first_launch_join() = shoot("phone-first-launch-join", FRESH, firstLaunch = true) {
        open("Set up from another device")
        kotlinx.coroutines.runBlocking { app.kaeri.store.FixtureSync.stage(it, "first-ok-join") }
    }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_join_failed_offline() = shoot("phone-join-failed-offline", WATCHING) { kotlinx.coroutines.runBlocking { app.kaeri.store.FixtureSync.stage(it, "join-failed-offline") } }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_join_failed_not_allowed() = shoot("phone-join-failed-not-allowed", WATCHING) { kotlinx.coroutines.runBlocking { app.kaeri.store.FixtureSync.stage(it, "join-failed-not-allowed") } }

    /** One frame of the sign-in wait board: Devices on [scene], with [state] staged as the Fixtures app stages it. */
    private fun signInWait(name: String, scene: String, state: String?) =
        shoot(name, scene, page = "settings/devices", screen = true) {
            if (state != null) kotlinx.coroutines.runBlocking { app.kaeri.store.FixtureSync.stage(it, state) }
        }

    @Test @Config(qualifiers = PHONE)
    fun phone_wait_off() = signInWait("phone-wait-off", WATCHING, null)

    @Test @Config(qualifiers = PHONE)
    fun phone_wait_picker() = signInWait("phone-wait-picker", "sync_nearby", "picker")

    @Test @Config(qualifiers = PHONE)
    fun phone_wait_waiting() = signInWait("phone-wait-waiting", "sync_nearby", "waiting")

    @Test @Config(qualifiers = PHONE)
    fun phone_wait_connecting() = signInWait("phone-wait-connecting", "sync_nearby", "connecting")

    @Test @Config(qualifiers = PHONE)
    fun phone_wait_setting_up() = signInWait("phone-wait-setting-up", "sync_nearby", "setting-up")

    @Test @Config(qualifiers = PHONE)
    fun phone_wait_key_sheet() = signInWait("phone-wait-key-sheet", "sync_nearby", "key-sheet")

    @Test @Config(qualifiers = PHONE)
    fun phone_wait_timed_out() = signInWait("phone-wait-timed-out", "sync_nearby", "timed-out")

    @Test @Config(qualifiers = PHONE)
    fun phone_wait_denied() = signInWait("phone-wait-denied", "sync_nearby", "denied")

    @Test @Config(qualifiers = PHONE)
    fun phone_wait_setup_failed() = signInWait("phone-wait-setup-failed", "sync_nearby", "setup-failed")

    @Test @Config(qualifiers = PHONE)
    fun phone_wait_cloud() = signInWait("phone-wait-cloud", "sync_cloud", null)

    @Test @Config(qualifiers = PHONE)
    fun phone_wait_switch_service() = signInWait("phone-wait-switch-service", "sync_cloud", "switch-service")

    @Test @Config(qualifiers = PHONE)
    fun phone_wait_remove_ask() = signInWait("phone-wait-remove-ask", "sync_cloud", "remove-ask")

    @Test @Config(qualifiers = PHONE)
    fun phone_wait_signed_out() = signInWait("phone-wait-signed-out", "sync_cloud", "signed-out")

    @Test
    @Config(qualifiers = PHONE)
    fun phone_home_continue_full() = shoot("phone-home-continue-full", "home-continuefull")

    @Test
    @Config(qualifiers = PHONE)
    fun phone_home_not_found() = shoot("phone-home-not-found", "home-notfound")

    /** One part of Home failed to load: one notice, with Try again. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_home_failed() = shoot("phone-home-failed", "home-failed")

    /** The kept Home while offline: the line under the title says from when. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_home_offline() = shoot("phone-home-offline", HOME_WATCHING) {
        compose.runOnUiThread { offline(it, since = 1_761_900_000L) }
    }

    /** Offline with nothing kept: the state, and the way to what plays. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_home_offline_empty() = shoot("phone-home-offline-empty", "home-offline")

    /** Signed out with nothing watched: Home still has a spotlight and shelves, and no empty state. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_home_fresh() = shoot("phone-home-fresh", FRESH)

    @Test
    @Config(qualifiers = TABLET)
    fun tablet_home() = shoot("tablet-home", HOME_WATCHING)

    @Test
    @Config(qualifiers = TABLET)
    fun tablet_home_offline() = shoot("tablet-home-offline", HOME_WATCHING) {
        compose.runOnUiThread { offline(it, since = 1_761_900_000L) }
    }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_calendar_failed() = shoot("phone-calendar-failed", WATCHING, start = Destination.Calendar, exclude = ::calendarList) {
        compose.runOnUiThread { it.apply(listOf(Event.Content(ContentFor.Calendar, ContentList.Failed(FAILURE)))) }
    }

    /** The calendar with an estimated time ("~") and rows marked Dub (sub rows plain) beside their times. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_calendar_marked() = shoot("phone-calendar-marked", WATCHING, start = Destination.Calendar) { store ->
        compose.runOnUiThread {
            val rows = store.calendar.window.value.rows.mapIndexed { index, row ->
                when (index % 3) {
                    0 -> row.copy(airingApproximate = true)
                    1 -> row.copy(airingDubbed = true, airingTranslation = app.kaeri.engine.core.Translation.DUB)
                    else -> row.copy(airingTranslation = app.kaeri.engine.core.Translation.SUB)
                }
            }
            store.apply(listOf(Event.Content(ContentFor.Calendar, ContentList.Entries(rows))))
        }
    }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_seasonal_failed() = shoot("phone-seasonal-failed", WATCHING, start = Destination.Seasonal, exclude = ::seasonalList) {
        compose.runOnUiThread { it.apply(listOf(Event.Content(ContentFor.Seasonal, ContentList.Failed(FAILURE)))) }
    }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_library_signed_out() = shoot("phone-library-signed-out", FRESH, start = Destination.Library)

    @Test
    @Config(qualifiers = PHONE)
    fun phone_continue_menu() = shoot("phone-continue-menu", HOME_WATCHING, screen = true) { store ->
        val lead = checkNotNull(store.continuing.queue(store.titles.entries.value).lead)
        // The spotlight shows the same show first; the card is the last of its title on screen.
        compose.onAllNodesWithText(lead.entry.showTitle).onLast().performTouchInput { longClick() }
    }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_first_launch() = shoot("phone-first-launch", FRESH, firstLaunch = true)

    @Test
    @Config(qualifiers = PHONE)
    fun phone_first_launch_anilist() = shoot("phone-first-launch-anilist", FRESH, firstLaunch = true) { open("Continue") }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_first_launch_sources() = shoot("phone-first-launch-sources", FRESH, firstLaunch = true) {
        open("Continue")
        compose.mainClock.advanceTimeBy(1_000)
        open("Not now")
    }

    @Test
    @Config(qualifiers = TABLET)
    fun tablet_first_launch() = shoot("tablet-first-launch", FRESH, firstLaunch = true)

    @Test
    @Config(qualifiers = PHONE)
    fun phone_title() = shoot("phone-title", WATCHING, title = 900001u)

    @Test
    @Config(qualifiers = PHONE)
    fun phone_calendar() = shoot("phone-calendar", WATCHING, start = Destination.Calendar)

    @Test
    @Config(qualifiers = PHONE)
    fun phone_search() = shoot("phone-search", WATCHING, start = Destination.Search, search = SearchQuery(text = "hoshi"))

    @Test
    @Config(qualifiers = PHONE)
    fun phone_search_start() = shoot("phone-search-start", WATCHING, start = Destination.Search)

    @Test
    @Config(qualifiers = PHONE)
    fun phone_search_browse() = shoot("phone-search-browse", WATCHING, start = Destination.Search, search = SearchQuery(genres = listOf("Fantasy")))

    @Test
    @Config(qualifiers = PHONE)
    fun phone_search_filters() =
        shoot("phone-search-filters", WATCHING, start = Destination.Search, search = SearchQuery(genres = listOf("Fantasy")), sheet = "filters")

    @Test
    @Config(qualifiers = PHONE)
    fun phone_library() = shoot("phone-library", WATCHING, start = Destination.Library)

    @Test
    @Config(qualifiers = PHONE)
    fun phone_more_about() = shoot("phone-more-about", WATCHING, title = 900001u) { open("More about this show") }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_show_settings() = shoot("phone-show-settings", WATCHING, title = 900001u) {
        compose.onNodeWithContentDescription("More").performClick()
        compose.waitForIdle()
        open("Settings for this show")
    }

    /** Source for this title, from the title page's More: Automatic, each source, the chosen release checked. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_source_picker() = shoot("phone-source-picker", WATCHING, title = 900001u, screen = true) { store -> openPicker(store, pinned = true) }

    /** The same with nothing pinned: Automatic checked, a note where a source has no count. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_source_picker_automatic() = shoot("phone-source-picker-automatic", WATCHING, title = 900001u, screen = true) { store ->
        openPicker(store, pinned = false)
    }

    @Test
    @Config(qualifiers = TABLET)
    fun tablet_source_picker() = shoot("tablet-source-picker", WATCHING, title = 900001u, screen = true) { store -> openPicker(store, pinned = true) }

    /** Picking a source sends it, closes the sheet and leaves the page as it was. */
    @Test
    @Config(qualifiers = PHONE)
    fun picking_a_source_sends_it_and_closes_the_sheet() = shoot("phone-source-picked", WATCHING, title = 900001u, screen = true) { store ->
        openPicker(store, pinned = false)
        compose.onNodeWithText("Mukashi Archive").performClick()
        compose.waitForIdle()
        assertEquals(
            app.kaeri.engine.model.Command.SetProviderPreference(900001u, "mukashi-archive"),
            store.waiting.last(),
        )
        compose.onAllNodesWithText("Each episode from the first source that has it").assertCountEquals(0)
    }

    /** Episodes no source lists are dimmed, say so, and offer no play. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_title_unsourced() = shoot("phone-title-unsourced", WATCHING, title = 900001u) { store ->
        val rows = checkNotNull(store.titles.episodes.value[900001u])
        compose.runOnUiThread {
            store.apply(listOf(Event.Episodes(900001u, rows.map { if (it.number.toInt() >= 8) it.copy(unsourced = true) else it })))
        }
        compose.waitForIdle()
        toEpisodes()
        compose.onAllNodesWithText("E8 \u00b7 No source has this episode", substring = true).assertCountEquals(1)
    }

    private fun openPicker(store: AppStore, pinned: Boolean) {
        compose.onNodeWithContentDescription("More").performClick()
        compose.waitForIdle()
        open("Source for this title")
        compose.runOnUiThread { store.apply(listOf(Event.SourceOptions(900001u, pickerOptions(pinned)))) }
        compose.mainClock.advanceTimeBy(1_000)
        compose.waitForIdle()
    }

    private fun pickerOptions(pinned: Boolean) = listOf(
        SourceOption("hotaru-open-archive", "Hotaru Open Archive", 13u, 13u, false, null, emptyList(), null),
        SourceOption("mukashi-archive", "Mukashi Archive", 6u, 13u, false, null, emptyList(), null),
        SourceOption(
            "torrents", "Torrent playback", 12u, 13u, pinned, null,
            listOf(
                SourceCandidate("batch-1080", "torrents", "Stardust Wayfarer complete batch", 1080u, "From a trusted uploader", "14.2 GiB", true, false, true),
                SourceCandidate("single-1080", "torrents", "Stardust Wayfarer 01", 1080u, null, "1.1 GiB", false, false, false),
                SourceCandidate("single-720", "torrents", "Stardust Wayfarer 01", 720u, null, "480 MiB", false, false, false),
            ),
            "batch-1080".takeIf { pinned },
        ),
        SourceOption("fumi-archive", "Fumi Archive", 0u, 13u, false, SourceNote.Resting, emptyList(), null),
        SourceOption("old-reel", "Old Reel", 0u, 13u, false, SourceNote.Failed("Timed out after 20 seconds"), emptyList(), null),
    )

    // TalkBack and the keyboard.

    /** An episode row says what a tap and a hold do: it plays, and it opens its options. */
    @Test
    @Config(qualifiers = PHONE)
    fun episode_rows_name_their_actions() = shoot("phone-title-episodes", WATCHING, title = 900001u) {
        toEpisodes()
        val row = compose.onNode(hasClickAction() and hasAnyDescendant(hasText("E7 \u00b7 14", substring = true)), useUnmergedTree = true)
        row.assert(SemanticsMatcher("plays on a tap") { it.config.getOrNull(SemanticsActions.OnClick)?.label == "Play" })
        row.assert(SemanticsMatcher("opens its options on a hold") { it.config.getOrNull(SemanticsActions.OnLongClick)?.label == "Episode options" })
    }

    // Font scale at 200 %, the largest the system offers: nothing cut, nothing overlapping.

    @Test
    @Config(qualifiers = PHONE, fontScale = 2.0f)
    fun phone_home_large_text() = shoot("phone-home-large-text", WATCHING)

    /** Lawful sources: a title that does not play here but is on a licensed service the viewer uses. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_title_watch_on() = shoot("phone-title-watch-on", "licensed", title = 900001u)

    /** Search's Available filter over servers and a licensed service. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_search_available() =
        shoot("phone-search-available", "available", start = Destination.Search, search = SearchQuery(genres = listOf("Fantasy")), sheet = "filters")

    @Test
    @Config(qualifiers = PHONE)
    fun phone_search_available_results() =
        shoot("phone-search-available-results", "available", start = Destination.Search, search = SearchQuery(genres = listOf("Fantasy")))

    @Test
    @Config(qualifiers = PHONE, fontScale = 2.0f)
    fun phone_title_large_text() = shoot("phone-title-large-text", WATCHING, title = 900001u)

    @Test
    @Config(qualifiers = PHONE, fontScale = 2.0f)
    fun phone_title_episodes_large_text() = shoot("phone-title-episodes-large-text", WATCHING, title = 900001u) { toEpisodes() }

    @Test
    @Config(qualifiers = PHONE, fontScale = 2.0f)
    fun phone_calendar_large_text() = shoot("phone-calendar-large-text", WATCHING, start = Destination.Calendar)

    @Test
    @Config(qualifiers = PHONE, fontScale = 2.0f)
    fun phone_library_large_text() = shoot("phone-library-large-text", WATCHING, start = Destination.Library)

    @Test
    @Config(qualifiers = TABLET, fontScale = 2.0f)
    fun tablet_home_large_text() = shoot("tablet-home-large-text", WATCHING)

    /** Watching on a tablet is the same poster grid as the other lists. */
    @Test
    @Config(qualifiers = TABLET)
    fun tablet_library() = shoot("tablet-library", WATCHING, start = Destination.Library)

    @Test
    @Config(qualifiers = TABLET)
    fun tablet_more_about() = shoot("tablet-more-about", WATCHING, title = 900001u) { open("More about this show") }

    @Test
    @Config(qualifiers = TABLET)
    fun tablet_title() = shoot("tablet-title", WATCHING, title = 900001u)

    // A title no source has: its episodes AniList's alone, marked watched as any are, and the way
    // to a source in Play's place.

    @Test
    @Config(qualifiers = PHONE)
    fun phone_title_no_sources() = shoot("phone-title-no-sources", WATCHING, title = 900001u, sourceless = true)

    /** Its rows as any are, but a tap opens their options rather than playing. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_title_episodes_no_sources() =
        shoot("phone-title-episodes-no-sources", WATCHING, title = 900001u, sourceless = true) {
            toEpisodes()
            val rows = compose.onAllNodes(hasClickAction() and hasAnyDescendant(hasText("E8 \u00b7", substring = true)), useUnmergedTree = true)
            rows.onFirst().assert(SemanticsMatcher("opens its options on a tap") { it.config.getOrNull(SemanticsActions.OnClick)?.label == "Episode options" })
        }

    /** A row's options offer to add a source, which opens Providers. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_title_no_sources_to_providers() =
        shoot("phone-title-no-sources-to-providers", WATCHING, title = 900001u, sourceless = true) {
            toEpisodes()
            compose.onAllNodesWithText("E8 \u00b7", substring = true).onFirst().performClick()
            compose.waitForIdle()
            // The page's own button, scrolled away, and the menu's item, which is the last.
            compose.onAllNodesWithText(context.getString(R.string.title_button_add_source)).onLast().performClick()
            compose.waitForIdle()
            compose.onAllNodesWithText(context.getString(R.string.title_button_add_source)).assertCountEquals(0)
        }

    @Test
    @Config(qualifiers = TABLET)
    fun tablet_title_no_sources() = shoot("tablet-title-no-sources", WATCHING, title = 900001u, sourceless = true)

    // The episodes from the last one watched, the watched ones before it folded, and unfolded.

    @Test
    @Config(qualifiers = PHONE)
    fun phone_title_episodes() = shoot("phone-title-episodes", WATCHING, title = 900001u) { toEpisodes() }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_title_episodes_unfolded() = shoot("phone-title-episodes-unfolded", WATCHING, title = 900001u) { unfoldInPlace() }

    @Test
    @Config(qualifiers = TABLET)
    fun tablet_title_episodes() = shoot("tablet-title-episodes", WATCHING, title = 900001u) { toEpisodes() }

    @Test
    @Config(qualifiers = TABLET)
    fun tablet_title_episodes_unfolded() = shoot("tablet-title-episodes-unfolded", WATCHING, title = 900001u) { unfoldInPlace() }

    // A long franchise's picker: twelve seasons of Cloudcutter Airship, more than fit across, so
    // it scrolls, the season on screen in the middle.

    @Test
    @Config(qualifiers = PHONE)
    fun phone_title_long() = shoot("phone-title-long", WATCHING, title = 920009u)

    @Test
    @Config(qualifiers = PHONE)
    fun phone_title_long_first() = shoot("phone-title-long-first", WATCHING, title = 920001u)

    /** A jump from the ninth season to the twelfth, whose page was not laid out: settled on its own page. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_title_long_jump() = shoot("phone-title-long-jump", WATCHING, title = 920009u) { pickSeason("Season 12") }

    /**
     * The same jump halfway through its crossfade: the stand-in, with the twelfth season's name and
     * cover and T8's placeholders, fading in over the ninth.
     */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_title_long_jump_halfway() = shoot("phone-title-long-jump-halfway", WATCHING, title = 920009u, settle = false) {
        compose.onNodeWithText("Season 12").performScrollTo()
        compose.waitForIdle()
        compose.mainClock.autoAdvance = false
        compose.onNodeWithText("Season 12").performClick()
        compose.mainClock.advanceTimeBy(120)
    }

    /** A jump to a season whose title has not arrived: its card's name and cover, and T8's placeholders. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_title_long_waiting() = shoot("phone-title-long-waiting", WATCHING, title = 920009u) { pickSeason("Season 5") }

    @Test
    @Config(qualifiers = PHONE, fontScale = 2.0f)
    fun phone_title_long_large_text() = shoot("phone-title-long-large-text", WATCHING, title = 920009u)

    @Test
    @Config(qualifiers = TABLET)
    fun tablet_title_long() = shoot("tablet-title-long", WATCHING, title = 920009u)

    // A title long enough to wrap, on the lightest banner a title can have: its first line over
    // the art must still read.
    @Test
    @Config(qualifiers = PHONE)
    fun phone_title_pale() = shoot("phone-title-pale", WATCHING, title = 900001u, pale = true)

    @Test
    @Config(qualifiers = TABLET)
    fun tablet_title_pale() = shoot("tablet-title-pale", WATCHING, title = 900001u, pale = true)

    /** Tap the season [label] names in the picker, scrolled to first. */
    private fun pickSeason(label: String) {
        compose.onNodeWithText(label).performScrollTo().performClick()
    }

    private fun toRanges() {
        compose.onNode(hasScrollToKeyAction() and hasAnyDescendant(hasText("More about this show")))
            .performScrollToKey("range-row")
        compose.waitForIdle()
    }

    /** The title page scrolled to the row its watched episodes fold under. */
    private fun toEpisodes() {
        compose.onNode(hasScrollToKeyAction() and hasAnyDescendant(hasText("More about this show")))
            .performScrollToKey(watchedFold(900001u))
        compose.waitForIdle()
    }

    /**
     * Unfold the watched episodes, checking that the last one watched stays where it was on screen
     * while they open above it, then scroll to the row that folds them again.
     */
    private fun unfoldInPlace() {
        toEpisodes()
        val lastWatched = compose.onNodeWithText("E6 \u00b7", substring = true)
        val before = lastWatched.getUnclippedBoundsInRoot().top
        compose.onNodeWithText("watched", substring = true).performClick()
        compose.waitForIdle()
        assertEquals("the last episode watched stays put", before, lastWatched.getUnclippedBoundsInRoot().top)
        // The unfolded rows push the fold off the top: find the list by the row kept in place.
        compose.onNode(hasScrollToKeyAction() and hasAnyDescendant(hasText("E6 \u00b7", substring = true)))
            .performScrollToKey(watchedFold(900001u))
        compose.waitForIdle()
        compose.onNodeWithText("Hide watched episodes").assertExists()
    }

    /**
     * [scene] replayed into a store, the app drawn on it from [start], and captured once the art
     * has landed and every transition has run.
     */
    // Watch together: the place, the in-party bar, the lobby's states, and the picker. The lobby
    // states lay the engine's party events over the recorded Together scene.

    @Test
    @Config(qualifiers = PHONE)
    fun phone_home_people() = shoot("phone-party-home", TOGETHER)

    @Test
    @Config(qualifiers = PHONE)
    fun phone_together() = shoot("phone-party-place", TOGETHER, page = TOGETHER_ROUTE)

    @Test
    @Config(qualifiers = PHONE)
    fun phone_together_in_party() = shoot("phone-party-place-in-party", TOGETHER, page = TOGETHER_ROUTE, party = PartyStates.inRoom())

    @Test
    @Config(qualifiers = PHONE)
    fun phone_together_looked_in() = shoot("phone-party-place-looked-in", TOGETHER, page = TOGETHER_ROUTE) { store ->
        compose.runOnUiThread { store.apply(listOf(PartyStates.glimpse())) }
    }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_together_bad_code() = shoot("phone-party-place-bad-code", TOGETHER, page = TOGETHER_ROUTE) {
        compose.onNode(androidx.compose.ui.test.hasSetTextAction()).performTextInput("not a code")
        compose.onNodeWithText("Join").performClick()
    }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_party_bar() = shoot("phone-party-bar", TOGETHER, party = PartyStates.inRoom())

    /** The bar rises from behind the navigation bar: frames at 0, 60, 120, 200 and 400 ms. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_party_bar_rising() = shoot("phone-party-bar-rising-end", TOGETHER, settle = false) { store ->
        frames("phone-party-bar-rising", listOf(0L, 60L, 120L, 200L, 400L)) { compose.runOnUiThread { store.apply(PartyStates.inRoom()) } }
    }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_lobby_invited() = shoot("phone-party-lobby-invited", TOGETHER, page = LOBBY_ROUTE, before = { store -> lobby(store, by = "Mika") }, party = listOf(PartyStates.glimpse()))

    @Test
    @Config(qualifiers = PHONE)
    fun phone_lobby_looking() = shoot("phone-party-lobby-looking", TOGETHER, page = LOBBY_ROUTE, before = { store -> lobby(store, title = null) })

    @Test
    @Config(qualifiers = PHONE)
    fun phone_lobby_unreachable() = shoot("phone-party-lobby-unreachable", TOGETHER, page = LOBBY_ROUTE, before = { store -> lobby(store) }, party = listOf(PartyStates.unreachable()))

    @Test
    @Config(qualifiers = PHONE)
    fun phone_lobby_empty_code() = shoot("phone-party-lobby-empty-code", TOGETHER, page = LOBBY_ROUTE, before = { store -> lobby(store, code = true, title = null) }, party = listOf(PartyStates.glimpse(members = emptyList(), roomEpisode = null)))

    @Test
    @Config(qualifiers = PHONE)
    fun phone_lobby_filling() = shoot("phone-party-lobby-filling", TOGETHER, page = LOBBY_ROUTE, party = PartyStates.inRoom())

    @Test
    @Config(qualifiers = PHONE)
    fun phone_lobby_ready() = shoot("phone-party-lobby-ready", TOGETHER, page = LOBBY_ROUTE, party = PartyStates.inRoom(PartyStates.view(ready = true), PartyStates.members(ready = 1, youReady = true)))

    @Test
    @Config(qualifiers = PHONE)
    fun phone_lobby_starting() = shoot(
        "phone-party-lobby-starting",
        TOGETHER,
        page = LOBBY_ROUTE,
        party = PartyStates.inRoom(PartyStates.view(ready = true, startingIn = 3u), PartyStates.members(ready = 2, youReady = true)),
    )

    @Test
    @Config(qualifiers = PHONE)
    fun phone_lobby_alone() = shoot("phone-party-lobby-alone", TOGETHER, page = LOBBY_ROUTE, party = PartyStates.inRoom(members = listOf(PartyStates.member("Sol", false, you = true))))

    @Test
    @Config(qualifiers = PHONE)
    fun phone_lobby_start_anyway() = shoot("phone-party-lobby-start-anyway", TOGETHER, page = LOBBY_ROUTE, party = PartyStates.inRoom(PartyStates.view(ready = true, canStartAnyway = true), PartyStates.members(ready = 1, youReady = true)))

    @Test
    @Config(qualifiers = PHONE)
    fun phone_lobby_nothing_open() = shoot("phone-party-lobby-nothing-open", TOGETHER, page = LOBBY_ROUTE, party = PartyStates.inRoom(PartyStates.view(roomEpisode = null, sources = null)))

    @Test
    @Config(qualifiers = PHONE)
    fun phone_lobby_guesses() = shoot(
        "phone-party-lobby-guesses",
        TOGETHER,
        page = LOBBY_ROUTE,
        party = PartyStates.inRoom(PartyStates.view(roomEpisode = PartyStates.episode(PartyFound.UNMATCHED, id = null, plainBy = "Mika"), sources = null, guesses = PartyStates.guesses())),
    )

    @Test
    @Config(qualifiers = PHONE)
    fun phone_lobby_not_found() = shoot(
        "phone-party-lobby-not-found",
        TOGETHER,
        page = LOBBY_ROUTE,
        party = PartyStates.inRoom(PartyStates.view(roomEpisode = PartyStates.episode(PartyFound.UNMATCHED, id = null, plainBy = "Mika"), sources = null)),
    )

    @Test
    @Config(qualifiers = PHONE)
    fun phone_lobby_found_by_name() = shoot("phone-party-lobby-found-by-name", TOGETHER, page = LOBBY_ROUTE, party = PartyStates.inRoom(PartyStates.view(roomEpisode = PartyStates.episode(PartyFound.NAME))))

    @Test
    @Config(qualifiers = PHONE)
    fun phone_lobby_checking() = shoot("phone-party-lobby-checking", TOGETHER, page = LOBBY_ROUTE, party = PartyStates.inRoom(PartyStates.view(sources = PartySources.Checking)))

    @Test
    @Config(qualifiers = PHONE)
    fun phone_lobby_not_aired() = shoot("phone-party-lobby-not-aired", TOGETHER, page = LOBBY_ROUTE, party = PartyStates.inRoom(PartyStates.view(sources = PartyStates.notAired())))

    @Test
    @Config(qualifiers = PHONE)
    fun phone_lobby_last_out() = shoot(
        "phone-party-lobby-last-out",
        TOGETHER,
        page = LOBBY_ROUTE,
        party = PartyStates.inRoom(PartyStates.view(playing = false)) + Event.PartyLastOut("9", null),
    )

    @Test
    @Config(qualifiers = PHONE)
    fun phone_lobby_connection_lost() = shoot("phone-party-lobby-connection-lost", TOGETHER, page = LOBBY_ROUTE, party = PartyStates.inRoom(PartyStates.view(connection = PartyConnection.Reconnecting(2u, 5u))))

    @Test
    @Config(qualifiers = PHONE)
    fun phone_lobby_playing() = shoot("phone-party-lobby-playing", TOGETHER, page = LOBBY_ROUTE, party = PartyStates.inRoom(PartyStates.view(ready = true, playing = true), PartyStates.members(ready = 2, youReady = true)))

    /** Mika says ready: her row's mark fades in and the counts and wait line crossfade. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_lobby_member_ready() = shoot("phone-party-lobby-member-ready-end", TOGETHER, page = LOBBY_ROUTE, party = PartyStates.inRoom(PartyStates.view(), PartyStates.members(ready = 0)), settle = false) { store ->
        compose.mainClock.advanceTimeBy(3_000)
        frames("phone-party-lobby-member-ready", listOf(0L, 100L, 200L, 300L)) {
            compose.runOnUiThread { store.apply(listOf(Event.PartyMembers(PartyStates.members(ready = 1)))) }
        }
    }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_party_pick() = shoot("phone-party-pick", TOGETHER, page = TOGETHER_ROUTE, screen = true) {
        compose.onNodeWithText("Start a party").performClick()
    }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_party_first_server() = shoot("phone-party-first-server", TOGETHER, page = TOGETHER_ROUTE, screen = true, exclude = { it is Event.PartyForm }, party = listOf(firstForm())) {
        compose.onNodeWithText("Start a party").performClick()
        compose.waitForIdle()
        compose.onAllNodesWithText("Stardust Wayfarer").onLast().performClick()
    }

    /** The lobby's More: Change episode, Copy invite link and Leave watch party. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_party_lobby_menu() = shoot("phone-party-lobby-menu", TOGETHER, page = LOBBY_ROUTE, screen = true, party = PartyStates.inRoom()) {
        compose.onNodeWithContentDescription("More").performClick()
    }

    /** Open the lobby a link opened, as the app does for one from a chat. */
    private fun lobby(store: AppStore, by: String? = null, title: String? = "Natsukaze Relay", code: Boolean = false) {
        store.openParty(
            app.kaeri.engine.model.PartyTarget(
                server = PartyStates.SERVER,
                room = PartyStates.CODE,
                by = by,
                title = title,
                episode = title?.let { "9" },
                id = title?.let { PartyStates.RELAY },
                fromCode = code,
            ),
        )
    }

    /** Start a party's form with no server used before: the first time asks for one. */
    private fun firstForm() = Event.PartyForm(
        app.kaeri.engine.model.PartyForm(recentServers = emptyList(), suggestedServer = PartyStates.SERVER, suggestedRunBy = "Syncplay", room = "quiet-harbour", name = "Sol"),
    )

    /** Capture [name]-<ms> at each of [at] after [change], the clock held between frames. */
    private fun frames(name: String, at: List<Long>, screen: Boolean = false, change: () -> Unit) {
        compose.mainClock.autoAdvance = false
        change()
        var now = 0L
        for (ms in at) {
            compose.mainClock.advanceTimeBy(ms - now)
            now = ms
            val file = "${roborazziSystemPropertyOutputDirectory()}/$name-$ms.png"
            if (screen) captureScreenRoboImage(file) else compose.onRoot().captureRoboImage(file)
        }
        compose.mainClock.autoAdvance = true
    }

    // Episode synopses (the synopsis board, section 3): text and stills laid on the scene's table.

    /** Spoiler protection off for 900001, as its own setting says, laid over the scene's own sheet: every row takes its text. */
    private val protectionOff: List<Event> =
            listOf(
                Event.ShowSettings(
                    app.kaeri.engine.model.ShowSettingsView(
                        id = 900001u,
                        rows = listOf(
                            app.kaeri.engine.model.ShowSettingRow(
                                key = app.kaeri.engine.model.ShowSettingKey.SPOILER_PROTECTION,
                                own = app.kaeri.engine.core.SettingValue.Bool(false),
                                value = "off",
                                default = "Use Default · On",
                            ),
                        ),
                        remembered = emptyList(),
                        source = null,
                    ),
                ),
            )

    /** The title page scrolled so the line where protection starts shows. */
    private fun toProtection() {
        compose.onNode(hasScrollToKeyAction() and hasAnyDescendant(hasText("More about this show")))
            .performScrollToKey("protection-900001")
        compose.waitForIdle()
    }

    /** Protection off: every row from the next episode on keeps its text. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_title_synopsis() = shoot("phone-title", WATCHING, title = 900001u, episodes = SynopsisFixtures::withSynopses, party = protectionOff) { toEpisodes() }

    /** Protection on: facts and a blurred still after the next episode, under the line where it starts. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_title_synopsis_protected() = shoot("phone-protected", WATCHING, title = 900001u, episodes = SynopsisFixtures::withSynopses) { toProtection() }

    /** One protected row revealed from its sheet: the still sharp, the row grown, its text in. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_title_synopsis_revealed() = shoot("phone-revealed", WATCHING, title = 900001u, episodes = SynopsisFixtures::withSynopses) {
        toProtection()
        compose.onNodeWithText("E8", substring = true).performTouchInput { longClick() }
        compose.waitForIdle()
        compose.onNodeWithText("Show synopsis").performClick()
        compose.waitForIdle()
    }

    /** The line's Show: every row after it revealed, the line saying what it shows, with Hide. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_title_synopsis_showing() = shoot("phone-showing", WATCHING, title = 900001u, episodes = SynopsisFixtures::withSynopses) {
        toProtection()
        compose.onNodeWithText("Show").performClick()
        compose.waitForIdle()
        compose.onNodeWithText("Hide").assertExists()
    }

    /** No synopses and none coming: every row is the short form. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_title_synopsis_none() = shoot("phone-none", WATCHING, title = 900001u, episodes = SynopsisFixtures::withoutText) { toEpisodes() }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_title_synopsis_ranges() = shoot("phone-ranges", WATCHING, title = 900001u, episodes = SynopsisFixtures::long, screen = true) {
        toRanges()
        compose.onNodeWithContentDescription("Episode range").performClick()
        compose.waitForIdle()
    }

    @Test
    @Config(qualifiers = TABLET)
    fun tablet_title_synopsis() = shoot("tablet-title-synopsis", WATCHING, title = 900001u, episodes = SynopsisFixtures::withSynopses) { toEpisodes() }

    @Test
    @Config(qualifiers = TABLET)
    fun tablet_title_synopsis_ranged() = shoot("tablet-title-synopsis-ranged", WATCHING, title = 900001u, episodes = SynopsisFixtures::long) { toRanges() }

    /** The tablet's range menu open over the beside rows. */
    @Test
    @Config(qualifiers = TABLET)
    fun tablet_title_synopsis_ranges() = shoot("tablet-ranges", WATCHING, title = 900001u, episodes = SynopsisFixtures::long, screen = true) {
        toRanges()
        compose.onNodeWithContentDescription("Episode range").performClick()
        compose.waitForIdle()
    }

    /** A protected row revealed on the tablet: still and text together, the row's height unchanged. */
    @Test
    @Config(qualifiers = TABLET)
    fun tablet_title_synopsis_revealed() = shoot("tablet-revealed", WATCHING, title = 900001u, episodes = SynopsisFixtures::withSynopses) {
        toProtection()
        compose.onNodeWithText("E8", substring = true).performTouchInput { longClick() }
        compose.waitForIdle()
        compose.onNodeWithText("Show synopsis").performClick()
        compose.waitForIdle()
    }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_title_synopsis_ranged() = shoot("phone-ranged", WATCHING, title = 900001u, episodes = SynopsisFixtures::long) { toRanges() }

    /** Long press: the sheet with the whole synopsis above the actions. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_title_synopsis_sheet() = shoot("phone-sheet", WATCHING, title = 900001u, episodes = SynopsisFixtures::withSynopses, screen = true) {
        toEpisodes()
        compose.onNodeWithText("E7", substring = true).performTouchInput { longClick() }
        compose.waitForIdle()
    }

    /** A protected episode's sheet: Show synopsis first with the open eye, the still blurred. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_title_synopsis_sheet_protected() = shoot("phone-sheet-protected", WATCHING, title = 900001u, episodes = SynopsisFixtures::withSynopses, screen = true) {
        toProtection()
        compose.onNodeWithText("E9", substring = true).performTouchInput { longClick() }
        compose.waitForIdle()
    }

    @Test
    @Config(qualifiers = PHONE, fontScale = 2.0f)
    fun phone_title_synopsis_large_text() = shoot("phone-large-text", WATCHING, title = 900001u, episodes = SynopsisFixtures::withSynopses) { toEpisodes() }

    // Browsing extras (the browsing board): count lines, Trending periods, Surprise me, Another, Go to episode.

    @Test
    @Config(qualifiers = PHONE)
    fun phone_browse_search_start() = shoot("phone-browse-search-start", BROWSING, start = Destination.Search, before = { it.chooseTrending(app.kaeri.engine.model.TrendPeriod.TODAY) })

    @Test
    @Config(qualifiers = PHONE)
    fun phone_browse_trending_week() = shoot("phone-browse-trending-week", BROWSING, start = Destination.Search, before = { it.chooseTrending(app.kaeri.engine.model.TrendPeriod.TODAY) }) { store ->
        store.chooseTrending(app.kaeri.engine.model.TrendPeriod.WEEK)
    }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_browse_trending_menu() = shoot("phone-browse-trending-menu", BROWSING, start = Destination.Search, screen = true, before = { it.chooseTrending(app.kaeri.engine.model.TrendPeriod.TODAY) }) {
        compose.onNodeWithText("Trending today").performClick()
        compose.waitForIdle()
    }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_browse_results() = shoot("phone-browse-results", BROWSING, start = Destination.Search, search = SearchQuery(text = "hoshi"))

    @Test
    @Config(qualifiers = PHONE)
    fun phone_browse_title_another() = shoot("phone-browse-title-another", BROWSING, start = Destination.Search) { store ->
        store.apply(listOf(Event.Surprise(app.kaeri.engine.model.SurprisePool.Planning, app.kaeri.engine.model.SurpriseOutcome.Picked(900001u, true))))
    }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_browse_go_to_episode() = shoot("phone-browse-go-to-episode", BROWSING, title = 900001u, episodes = SynopsisFixtures::long, screen = true, settle = false) {
        toRanges()
        compose.onNodeWithContentDescription("Episode range").performClick()
        compose.waitForIdle()
        // The field's cursor blinks for ever, so the clock is stepped, not waited for.
        compose.mainClock.autoAdvance = false
        compose.onNodeWithText("Go to episode", substring = true).performClick()
        compose.mainClock.advanceTimeBy(600)
    }

    /** The page scrolled by hand into the middle of a range: the chip pins under the floating bar, not at the top of the screen. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_browse_pinned_chip() = shoot("phone-browse-pinned-chip", BROWSING, title = 900001u, episodes = SynopsisFixtures::long, screen = true) {
        toRanges()
        compose.onRoot()
            .performTouchInput { swipeUp(startY = bottom * 0.8f, endY = top + height * 0.1f, durationMillis = 300) }
        compose.waitForIdle()
    }

    // Recordings, as frames 33 ms apart that make a 30 fps clip.

    private val clip = (0L..900L step 33L).toList()

    /** Loop A: a title's dub arrives while its cell is on screen; the dub part fades in on the line's end. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_browse_dub_arrives_clip() = shoot("phone-browse-dub-clip-end", BROWSING, start = Destination.Search, search = SearchQuery(text = "hoshi")) { store ->
        frames("phone-browse-dub-clip", clip) {
            store.apply((900002u..900006u).map { Event.TitleDub(it, app.kaeri.engine.model.DubExtent(through = 3u, complete = false)) })
        }
    }

    /** Loop B: the period switch; the shelf crossfades, the heading label on quick. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_browse_period_clip() = shoot("phone-browse-period-clip-end", BROWSING, start = Destination.Search, screen = true, before = { it.chooseTrending(app.kaeri.engine.model.TrendPeriod.TODAY) }) {
        compose.onNodeWithText("Trending today").performClick()
        compose.waitForIdle()
        frames("phone-browse-period-clip", clip, screen = true) { compose.onNodeWithText("This week").performClick() }
    }

    /** Loop C: Surprise me opens a title, then Another takes its place on a crossfade. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_browse_another_clip() = shoot("phone-browse-another-clip-end", BROWSING, start = Destination.Search) { store ->
        store.apply(listOf(Event.Surprise(app.kaeri.engine.model.SurprisePool.Planning, app.kaeri.engine.model.SurpriseOutcome.Picked(900001u, true))))
        compose.waitForIdle()
        frames("phone-browse-another-clip", (0L..900L step 33L).toList()) {
            store.surprise(app.kaeri.engine.model.SurprisePool.Planning, inPlace = true)
            store.apply(listOf(Event.Surprise(app.kaeri.engine.model.SurprisePool.Planning, app.kaeri.engine.model.SurpriseOutcome.Picked(900003u, true))))
        }
    }

    /** Loop D's setup: the long runner's range menu, then Go to episode with [typed] in its field, the clock held. */
    private fun goToSetup(typed: String) {
        // Only a recording draws the dialog's window, which its semantics need.
        assumeTrue(com.github.takahirom.roborazzi.roborazziSystemPropertyTaskType().isEnabled())
        toRanges()
        compose.onNodeWithContentDescription("Episode range").performClick()
        compose.waitForIdle()
        // The field's cursor blinks for ever, so the clock is stepped, not waited for.
        compose.mainClock.autoAdvance = false
        compose.onNodeWithText("Go to episode", substring = true).performClick()
        compose.mainClock.advanceTimeBy(600)
        // Robolectric lays a window out as it draws one, so the dialog is drawn once to be read.
        captureScreenRoboImage("${roborazziSystemPropertyOutputDirectory()}/phone-browse-go-to-scratch.png")
        // The dialog's field blinks its cursor for ever and the test's idle wait never ends, so its
        // semantics are read from the dialog's own window, its actions invoked on the UI thread and
        // the clock stepped.
        val field = dialogNodes().first { SemanticsActions.SetText in it.config }
        compose.runOnUiThread { field.config[SemanticsActions.SetText].action?.invoke(androidx.compose.ui.text.AnnotatedString(typed)) }
        compose.mainClock.advanceTimeBy(300)
        captureScreenRoboImage("${roborazziSystemPropertyOutputDirectory()}/phone-browse-go-to-scratch.png")
    }

    /** The semantics of the dialog on screen, read without the test's idle wait. */
    @OptIn(androidx.compose.ui.InternalComposeUiApi::class)
    private fun dialogNodes(): List<androidx.compose.ui.semantics.SemanticsNode> {
        fun find(view: android.view.View): androidx.compose.ui.node.RootForTest? =
            (view as? androidx.compose.ui.node.RootForTest)
                ?: (view as? android.view.ViewGroup)?.let { group -> (0 until group.childCount).firstNotNullOfOrNull { find(group.getChildAt(it)) } }
        // Every window the app has: the activity's and a dialog's, each with a compose root.
        val global = Class.forName("android.view.WindowManagerGlobal")
        val manager = global.getMethod("getInstance").invoke(null)
        @Suppress("UNCHECKED_CAST")
        val names = global.getMethod("getViewRootNames").invoke(manager) as Array<String>
        val getRoot = global.getMethod("getRootView", String::class.java)
        return names.mapNotNull { find(getRoot.invoke(manager, it) as android.view.View) }
            .flatMap { root -> runCatching { root.semanticsOwner.getAllSemanticsNodes(mergingEnabled = true) }.getOrDefault(emptyList()) }
    }

    /** The dialog's Go button, pressed on the UI thread. */
    private fun pressGo() {
        val go = dialogNodes().first { node -> node.config.getOrNull(SemanticsProperties.Text)?.any { it.text == "Go" } == true && SemanticsActions.OnClick in node.config }
        compose.runOnUiThread { go.config[SemanticsActions.OnClick].action?.invoke() }
    }

    /** Loop D: Go to episode 215 from the range menu: the list and the chip's label crossfade to the other range. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_browse_go_to_clip() = shoot("phone-browse-go-to-clip-end", BROWSING, title = 900001u, episodes = SynopsisFixtures::long, screen = true, settle = false) {
        goToSetup("215")
        frames("phone-browse-go-to-clip", clip, screen = true) { pressGo() }
    }

    /** Loop D with Remove animations on: still a crossfade, both ranges drawn, never a cut. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_browse_go_to_clip_removed() = shoot("phone-browse-go-to-removed-clip-end", BROWSING, title = 900001u, episodes = SynopsisFixtures::long, screen = true, settle = false, removed = true) {
        goToSetup("215")
        frames("phone-browse-go-to-removed-clip", clip, screen = true) { pressGo() }
    }

    /** Loop E: a short jump inside the range on screen, 154 from 151: the list scrolls and the row takes the selected fill. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_browse_short_jump_clip() = shoot("phone-browse-short-jump-clip-end", BROWSING, title = 900001u, episodes = SynopsisFixtures::long, screen = true, settle = false) {
        goToSetup("154")
        frames("phone-browse-short-jump-clip", clip, screen = true) { pressGo() }
    }

    /** Loop E with Remove animations on: the scrolled list crossfades in instead. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_browse_short_jump_clip_removed() = shoot("phone-browse-short-jump-removed-clip-end", BROWSING, title = 900001u, episodes = SynopsisFixtures::long, screen = true, settle = false, removed = true) {
        goToSetup("154")
        frames("phone-browse-short-jump-removed-clip", clip, screen = true) { pressGo() }
    }

    /** Loops B, A and C with Remove animations on, recorded again as crossfades. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_browse_period_clip_removed() = shoot("phone-browse-period-removed-clip-end", BROWSING, start = Destination.Search, screen = true, removed = true, before = { it.chooseTrending(app.kaeri.engine.model.TrendPeriod.TODAY) }) {
        compose.onNodeWithText("Trending today").performClick()
        compose.waitForIdle()
        frames("phone-browse-period-removed-clip", clip, screen = true) { compose.onNodeWithText("This week").performClick() }
    }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_browse_dub_arrives_clip_removed() = shoot("phone-browse-dub-removed-clip-end", BROWSING, start = Destination.Search, search = SearchQuery(text = "hoshi"), removed = true) { store ->
        frames("phone-browse-dub-removed-clip", clip) {
            store.apply((900002u..900006u).map { Event.TitleDub(it, app.kaeri.engine.model.DubExtent(through = 3u, complete = false)) })
        }
    }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_browse_another_clip_removed() = shoot("phone-browse-another-removed-clip-end", BROWSING, start = Destination.Search, removed = true) { store ->
        store.apply(listOf(Event.Surprise(app.kaeri.engine.model.SurprisePool.Planning, app.kaeri.engine.model.SurpriseOutcome.Picked(900001u, true))))
        compose.waitForIdle()
        // Robolectric draws only to capture, so the page is drawn once for the layer that records it.
        compose.onRoot().captureToImage()
        frames("phone-browse-another-removed-clip", clip) {
            store.surprise(app.kaeri.engine.model.SurprisePool.Planning, inPlace = true)
            store.apply(listOf(Event.Surprise(app.kaeri.engine.model.SurprisePool.Planning, app.kaeri.engine.model.SurpriseOutcome.Picked(900003u, true))))
        }
    }

    /** The line's Show: the line crossfades, each row grows, then its text fades in. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_title_synopsis_reveal_clip() = shoot("phone-reveal-clip-end", WATCHING, title = 900001u, episodes = SynopsisFixtures::withSynopses) {
        toProtection()
        frames("phone-reveal-clip", clip) { compose.onNodeWithText("Show").performClick() }
    }

    /** A range switch: the rows crossfade in place. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_title_synopsis_range_clip() = shoot("phone-range-clip-end", WATCHING, title = 900001u, episodes = SynopsisFixtures::long) {
        toRanges()
        compose.onNodeWithContentDescription("Episode range").performClick()
        compose.waitForIdle()
        frames("phone-range-clip", clip, screen = true) { compose.onNodeWithText("Episodes 201 to 230").performClick() }
    }

    private fun shoot(
        name: String,
        scene: String,
        start: Destination = Destination.Home,
        title: AnilistId? = null,
        search: SearchQuery? = null,
        sheet: String? = null,
        /** Whether every transition runs to its end before the capture; else it is taken where [then] left the clock. */
        settle: Boolean = true,
        sourceless: Boolean = false,
        /** Whether 900001 has the long title and the near-white banner of [PaleBanner]. */
        pale: Boolean = false,
        /** Events of the scene left out, as a list that never answered. */
        exclude: (Event) -> Boolean = { false },
        /** Whether every show on Continue was last watched dubbed. */
        dub: Boolean = false,
        /** Whether another device of the group played Continue last. */
        device: Boolean = false,
        /** Whether the whole screen is captured, so a menu's window shows. */
        screen: Boolean = false,
        /** Whether first launch's pages show, as on the app's first launch. */
        firstLaunch: Boolean = false,
        /** The page opened over [start] as the app opens, by its route. */
        page: String? = null,
        /** Events laid over the scene each time it replays: a watch party's, say. */
        party: List<Event> = emptyList(),
        /** Run on the store before the screens show: a lobby a link opened, say. */
        before: ((AppStore) -> Unit)? = null,
        /** The episode tables of the scene as they are sent: text and stills laid on, say. */
        episodes: ((AnilistId, List<EpisodeRow>) -> List<EpisodeRow>)? = null,
        /** Remove animations on, the system's animator duration scale at 0, as a person turns it on. */
        removed: Boolean = false,
        then: ((AppStore) -> Unit)? = null,
    ) {
        if (removed) removeAnimations()
        val recorded = Scene.named(scene)
        AppClock.clock = Clock.fixed(recorded.recordedAt, ZONE)
        // The art the screens ask for, as the fake AniList serves it.
        val store = AppStore(scope, clock = { 0L }, artworkWithoutEngine = { url -> PaleBanner.bytes(url) ?: FixtureArt.bytes(url) })
        val events = recorded.events.filter { replayed(it) && !exclude(it) }.map { if (sourceless) withoutSources(it) else it }.map { if (dub) dubbed(it) else it }.map { if (device) fromDevice(it) else it }
            .map { if (episodes != null && it is Event.Episodes) Event.Episodes(it.id, episodes(it.id, it.rows)) else it }
        // After the scene each time, so its own detail never wins over the pale one.
        fun replay() {
            store.apply(events)
            if (pale) store.apply(listOf(Event.Detail(PaleBanner.retitle(checkNotNull(store.titles[PaleBanner.ID])))))
            if (party.isNotEmpty()) store.apply(party)
        }
        replay()
        before?.invoke(store)

        compose.runOnUiThread { WindowCompat.setDecorFitsSystemWindows(compose.activity.window, false) }
        compose.setContent {
            KaeriTheme {
                AppRoot(store, if (search != null) Destination.Search else start, startTitle = title, startSearch = search, startSheet = sheet, firstLaunch = firstLaunch, startPage = page)
            }
        }
        compose.waitForIdle()
        compose.runOnUiThread { systemBars(compose.activity) }
        // What the screens asked for as they opened is answered as the engine answered it.
        replay()
        awaitArt(store)
        // First launch decides once the engine has said what is set up, or after its wait.
        if (firstLaunch) {
            compose.mainClock.advanceTimeBy(3_500)
            compose.waitForIdle()
        }
        then?.let {
            it(store)
            if (settle) {
                compose.waitForIdle()
                awaitArt(store)
            }
        }
        if (settle) {
            compose.mainClock.advanceTimeBy(3_000)
            compose.waitForIdle()
        }
        val file = "${roborazziSystemPropertyOutputDirectory()}/$name.png"
        if (screen) captureScreenRoboImage(file) else compose.onRoot().captureRoboImage(file)
    }


    /** Remove animations, as the Settings switch sets it: the animator duration scale at 0. */
    private fun removeAnimations() {
        android.provider.Settings.Global.putFloat(androidx.test.core.app.ApplicationProvider.getApplicationContext<android.content.Context>().contentResolver, android.provider.Settings.Global.ANIMATOR_DURATION_SCALE, 0f)
    }

    /** Tap what [label] names, scrolled to first where it sits in a list. */
    private fun open(label: String) {
        val node = compose.onNodeWithText(label)
        runCatching { node.performScrollTo() }
        node.performClick()
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

    /**
     * The status bar and the gesture bar, as a phone and a tablet have them, so edge-to-edge
     * screens sit where they do on the device: 24 dp each.
     */
    private fun systemBars(activity: ComponentActivity) {
        val bar = (24 * activity.resources.displayMetrics.density).roundToInt()
        val status = WindowInsetsCompat.Type.statusBars()
        val navigation = WindowInsetsCompat.Type.navigationBars()
        val insets = WindowInsetsCompat.Builder()
            .setInsets(status, Insets.of(0, bar, 0, 0))
            .setInsets(navigation, Insets.of(0, 0, 0, bar))
            .setVisible(status, true)
            .setVisible(navigation, true)
            .build()
        // The compose view the rule set the content in, which reads the insets it is handed.
        val content = activity.findViewById<ViewGroup>(android.R.id.content).getChildAt(0)
        ViewCompat.dispatchApplyWindowInsets(content, insets)
    }

    private companion object {
        /** The phone boards: 412 × 915 dp at 420 dpi, as the emulator's phone. */
        const val PHONE = "w412dp-h915dp-port-420dpi"

        /** The tablet boards: 1280 × 800 dp, landscape, at 320 dpi. */
        const val TABLET = "w1280dp-h800dp-land-xhdpi"

        /** A list's failure as the engine says it: a request error's whole chain. */
        const val FAILURE = "error sending request for url (https://graphql.anilist.example/): connection refused"

        /** Home as it was last sent, offline: the kept Home from [since], or with nothing kept. */
        fun offline(store: AppStore, since: Long?) {
            val view = checkNotNull(store.home.view.value)
            store.apply(listOf(Event.Connectivity(offline = true), Event.Home(view.copy(offline = true, offlineSince = since))))
        }

        fun continueList(event: Event) = event is Event.Content && event.view == ContentFor.Continue
        fun calendarList(event: Event) =
            (event is Event.Content && event.view == ContentFor.Calendar) || event is Event.CalendarWindow || event is Event.CalendarBackfill
        fun seasonalList(event: Event) = event is Event.Content && event.view == ContentFor.Seasonal

        const val WATCHING = "watching"
        const val BROWSING = "browsing"

        const val HOME_WATCHING = "home-watching"
        const val FRESH = "fresh"
        const val TOGETHER = "together"

        /** The zone the shots read their days and times in, wherever they run. */
        val ZONE: ZoneId = ZoneId.of("Europe/Copenhagen")

        /**
         * [event] as the engine says it with no source set up: no providers, and every table from
         * AniList's data alone. The fixtures' source lists the same numbers, titles and stills as
         * AniList's listings, so its tables stand in for AniList's, marked so.
         */
        fun withoutSources(event: Event): Event = when (event) {
            is Event.Providers -> Event.Providers(emptyList())
            is Event.Episodes -> event.copy(rows = event.rows.map { it.copy(metadataOnly = true) })
            else -> event
        }

        /** What a screenshot replays: not the jobs, whose progress line would show half-run. */
        fun replayed(event: Event): Boolean = when (event) {
            is Event.JobStarted, is Event.JobNote, is Event.JobDone, is Event.Toast -> false
            else -> true
        }
    }
}

/** [event], when it is the Home view, with Continue as another device of the group played it last, as the engine would say it. */
private fun fromDevice(event: Event): Event = when (event) {
    is Event.Home -> Event.Home(
        event.v1.copy(
            spotlight = event.v1.spotlight?.let { if (it.case == SpotlightCase.CONTINUING) it.copy(lastDevice = "Study Mac") else it },
            sections = event.v1.sections.map { section ->
                if (section.kind == SectionKind.CONTINUE) section.copy(items = section.items.map { it.copy(lastDevice = "Study Mac") }) else section
            },
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
