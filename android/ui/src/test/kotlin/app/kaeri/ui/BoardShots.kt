package app.kaeri.ui

import app.kaeri.i18n.R
import android.view.ViewGroup
import org.robolectric.RuntimeEnvironment
import androidx.activity.ComponentActivity
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.semantics.SemanticsActions
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
import androidx.compose.ui.test.onFirst
import androidx.compose.ui.test.onLast
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performScrollToKey
import androidx.compose.ui.test.performTouchInput
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
import app.kaeri.engine.model.SourceNote
import app.kaeri.engine.model.SourceOption
import app.kaeri.engine.model.SearchQuery
import app.kaeri.store.AppClock
import app.kaeri.store.AppStore
import app.kaeri.store.HAS_TESTING_ENGINE
import app.kaeri.testkit.FixtureArt
import app.kaeri.testkit.Scene
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
        compose.onNodeWithText("Hide watched episodes").assertExists()
        toEpisodes()
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
    private fun frames(name: String, at: List<Long>, change: () -> Unit) {
        compose.mainClock.autoAdvance = false
        change()
        var now = 0L
        for (ms in at) {
            compose.mainClock.advanceTimeBy(ms - now)
            now = ms
            compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/$name-$ms.png")
        }
        compose.mainClock.autoAdvance = true
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
        then: ((AppStore) -> Unit)? = null,
    ) {
        val recorded = Scene.named(scene)
        AppClock.clock = Clock.fixed(recorded.recordedAt, ZONE)
        // The art the screens ask for, as the fake AniList serves it.
        val store = AppStore(scope, clock = { 0L }, artworkWithoutEngine = { url -> PaleBanner.bytes(url) ?: FixtureArt.bytes(url) })
        val events = recorded.events.filter { replayed(it) && !exclude(it) }.map { if (sourceless) withoutSources(it) else it }.map { if (dub) dubbed(it) else it }
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
