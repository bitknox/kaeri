package app.kaeri.ui

import android.view.ViewGroup
import androidx.activity.ComponentActivity
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.hasSetTextAction
import androidx.core.graphics.Insets
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import app.kaeri.design.KaeriTheme
import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.i18n.R
import app.kaeri.store.AppHost
import app.kaeri.store.AppStore
import app.kaeri.store.EngineHome
import app.kaeri.store.EngineLaunch
import app.kaeri.store.FixtureScene
import app.kaeri.store.MemorySecrets
import app.kaeri.store.ServerAnswer
import app.kaeri.testkit.captureRoboImage
import app.kaeri.testkit.captureScreenRoboImage
import app.kaeri.ui.reading.BOOK_SOURCES_ROUTE
import com.github.takahirom.roborazzi.roborazziSystemPropertyOutputDirectory
import java.io.File
import java.nio.file.Files
import kotlin.math.roundToInt
import org.junit.After
import org.junit.Test
import org.robolectric.annotation.Config
import org.robolectric.shadows.ShadowLooper

/**
 * Reading's first run, the medium's sheets, Settings with reading on, the book sources and the
 * reading trackers, drawn from the testing engine. First run needs AppRoot's pages, and some
 * places are reached by tapping, so these shots open the frame themselves.
 */
@OptIn(com.github.takahirom.roborazzi.ExperimentalRoborazziApi::class)
class ReadSettingsShots : ReadingShotHarness() {
    private val home: File = Files.createTempDirectory("kaeri-settings-shots").toFile()

    @After
    fun dropHome() {
        home.deleteRecursively()
    }

    private fun text(id: Int): String = compose.activity.getString(id)

    /** Tap the node showing [words], the first of them when there are several, and let it land. */
    private fun tap(words: String, last: Boolean = false) {
        val nodes = compose.onAllNodesWithText(words)
        val count = nodes.fetchSemanticsNodes().size
        val node = nodes[if (last) count - 1 else 0]
        runCatching { node.performScrollTo() }
        node.performClick()
        run(30)
    }

    private fun run(frames: Int) {
        repeat(frames) {
            ShadowLooper.idleMainLooper()
            compose.mainClock.advanceTimeBy(50)
            Thread.sleep(10)
        }
    }

    /**
     * [scene] on the testing engine, the frame on [start] with [page] over it and first launch's
     * pages when [firstLaunch]; [act] taps its way to the place, and the capture waits for [ready].
     */
    private fun snap(
        name: String,
        scene: FixtureScene,
        start: Destination = Destination.Home,
        page: String? = null,
        medium: KaeriIsFor? = null,
        screen: Boolean = false,
        firstLaunch: Boolean = false,
        act: (AppStore) -> Unit = {},
        ready: (AppStore) -> Boolean = { true },
    ) {
        val store = AppStore(scope)
        this.store = store
        store.start(EngineLaunch(EngineLaunch.Engine.Fixtures(scene), EngineHome.override(home)), AppHost(MemorySecrets()))
        compose.runOnUiThread { WindowCompat.setDecorFitsSystemWindows(compose.activity.window, false) }
        compose.setContent {
            KaeriTheme { AppRoot(store, start, startPage = page, firstLaunch = firstLaunch) }
        }
        compose.waitForIdle()
        compose.runOnUiThread { bars(compose.activity) }
        pump("the engine to start") { it.phase.value == AppStore.Phase.Running && it.medium.view.value != null }
        if (medium != null && store.medium.view.value?.answer != medium) {
            compose.runOnUiThread { store.medium.choose(medium) }
            pump("the medium $medium") { it.medium.view.value?.answer == medium }
        }
        run(40)
        act(store)
        pump(name, ready)
        run(60)
        compose.mainClock.advanceTimeBy(1_000)
        compose.waitForIdle()
        val file = "${roborazziSystemPropertyOutputDirectory()}/$name.png"
        if (screen) captureScreenRoboImage(file) else compose.onRoot().captureRoboImage(file)
    }

    private fun bars(activity: ComponentActivity) {
        val bar = (24 * activity.resources.displayMetrics.density).roundToInt()
        val status = WindowInsetsCompat.Type.statusBars()
        val navigation = WindowInsetsCompat.Type.navigationBars()
        val insets = WindowInsetsCompat.Builder()
            .setInsets(status, Insets.of(0, bar, 0, 0))
            .setInsets(navigation, Insets.of(0, 0, 0, bar))
            .setVisible(status, true)
            .setVisible(navigation, true)
            .build()
        val content = activity.findViewById<ViewGroup>(android.R.id.content).getChildAt(0)
        ViewCompat.dispatchApplyWindowInsets(content, insets)
    }

    /** First run up to page 2, with [choice]'s row tapped when there is one. */
    private fun toMedium(choice: Int?) {
        pump("first launch") { compose.onAllNodesWithText(text(R.string.onboarding_continue)).fetchSemanticsNodes().isNotEmpty() }
        tap(text(R.string.onboarding_continue))
        // The join page: start here.
        tap(text(R.string.first_other_no))
        choice?.let { tap(text(it)) }
    }

    private fun toSources(choice: Int) {
        toMedium(choice)
        tap(text(R.string.onboarding_continue))
        tap(text(R.string.onboarding_not_now))
    }

    // First run

    @Test
    @Config(qualifiers = PHONE)
    fun phone_read_first_run_medium() = snap("phone-read-first-run-medium", FixtureScene.READING_FIRST_RUN, firstLaunch = true, act = { toMedium(R.string.medium_read_line) })

    @Test
    @Config(qualifiers = PHONE)
    fun phone_read_first_run_none() = snap("phone-read-first-run-none", FixtureScene.READING_FIRST_RUN, firstLaunch = true, act = { toMedium(null) })

    @Test
    @Config(qualifiers = PHONE)
    fun phone_read_first_run_lists() = snap("phone-read-first-run-lists", FixtureScene.READING_FIRST_RUN, firstLaunch = true, act = {
        toMedium(R.string.medium_both_line)
        tap(text(R.string.onboarding_continue))
    })

    @Test
    @Config(qualifiers = PHONE)
    fun phone_read_first_run_books() = snap("phone-read-first-run-books", FixtureScene.READING_FIRST_RUN, firstLaunch = true, act = { toSources(R.string.medium_read_line) })

    @Test
    @Config(qualifiers = PHONE)
    fun phone_read_first_run_both() = snap("phone-read-first-run-both", FixtureScene.READING_FIRST_RUN, firstLaunch = true, act = { toSources(R.string.medium_both_line) })

    // First run in large text: the welcome, the medium and page 4 at 130% and 200%.

    @Test
    @Config(qualifiers = PHONE, fontScale = 1.3f)
    fun phone_read_first_run_welcome_text_130() = snap("phone-read-first-run-welcome-text-130", FixtureScene.READING_FIRST_RUN, firstLaunch = true, act = {
        pump("first launch") { compose.onAllNodesWithText(text(R.string.onboarding_continue)).fetchSemanticsNodes().isNotEmpty() }
    })

    @Test
    @Config(qualifiers = PHONE, fontScale = 2.0f)
    fun phone_read_first_run_welcome_text_200() = snap("phone-read-first-run-welcome-text-200", FixtureScene.READING_FIRST_RUN, firstLaunch = true, act = {
        pump("first launch") { compose.onAllNodesWithText(text(R.string.onboarding_continue)).fetchSemanticsNodes().isNotEmpty() }
    })

    @Test
    @Config(qualifiers = PHONE, fontScale = 1.3f)
    fun phone_read_first_run_medium_text_130() = snap("phone-read-first-run-medium-text-130", FixtureScene.READING_FIRST_RUN, firstLaunch = true, act = { toMedium(R.string.medium_read_line) })

    @Test
    @Config(qualifiers = PHONE, fontScale = 2.0f)
    fun phone_read_first_run_medium_text_200() = snap("phone-read-first-run-medium-text-200", FixtureScene.READING_FIRST_RUN, firstLaunch = true, act = { toMedium(R.string.medium_read_line) })

    @Test
    @Config(qualifiers = PHONE, fontScale = 1.3f)
    fun phone_read_first_run_lists_text_130() = snap("phone-read-first-run-lists-text-130", FixtureScene.READING_FIRST_RUN, firstLaunch = true, act = {
        toMedium(R.string.medium_both_line)
        tap(text(R.string.onboarding_continue))
    })

    @Test
    @Config(qualifiers = PHONE, fontScale = 2.0f)
    fun phone_read_first_run_lists_text_200() = snap("phone-read-first-run-lists-text-200", FixtureScene.READING_FIRST_RUN, firstLaunch = true, act = {
        toMedium(R.string.medium_both_line)
        tap(text(R.string.onboarding_continue))
    })

    @Test
    @Config(qualifiers = PHONE, fontScale = 1.3f)
    fun phone_read_first_run_books_text_130() = snap("phone-read-first-run-books-text-130", FixtureScene.READING_FIRST_RUN, firstLaunch = true, act = { toSources(R.string.medium_read_line) })

    @Test
    @Config(qualifiers = PHONE, fontScale = 2.0f)
    fun phone_read_first_run_books_text_200() = snap("phone-read-first-run-books-text-200", FixtureScene.READING_FIRST_RUN, firstLaunch = true, act = { toSources(R.string.medium_read_line) })

    @Test
    @Config(qualifiers = TABLET)
    fun tablet_read_first_run_medium() = snap("tablet-read-first-run-medium", FixtureScene.READING_FIRST_RUN, firstLaunch = true, act = { toMedium(R.string.medium_read_line) })

    // The medium's sheets

    @Test
    @Config(qualifiers = PHONE)
    fun phone_read_update_sheet() = snap("phone-read-update-sheet", FixtureScene.READING_UPDATE_SHEET, screen = true) { it.medium.asking.value != null }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_read_join_sheet() = snap("phone-read-join-sheet", FixtureScene.READING_JOIN_ASK, screen = true) { it.medium.asking.value != null }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_read_medium_off() = snap(
        "phone-read-medium-off", FixtureScene.READING_HOME, page = settingsRoute(SettingsPane.Medium), screen = true,
        act = { tap(text(R.string.medium_read_line)) },
    )

    // Settings

    @Test
    @Config(qualifiers = PHONE)
    fun phone_read_settings_alone() = snap("phone-read-settings-alone", FixtureScene.READING_HOME, page = settingsRoute(null), medium = KaeriIsFor.READ)

    @Test
    @Config(qualifiers = PHONE)
    fun phone_read_settings_both() = snap("phone-read-settings-both", FixtureScene.READING_HOME, page = settingsRoute(null))

    @Test
    @Config(qualifiers = PHONE)
    fun phone_read_settings_medium() = snap("phone-read-settings-medium", FixtureScene.READING_HOME, page = settingsRoute(SettingsPane.Medium))

    @Test
    @Config(qualifiers = PHONE)
    fun phone_read_settings_reading() = snap("phone-read-settings-reading", FixtureScene.READING_HOME, page = settingsRoute(SettingsPane.Reading)) {
        it.reading.readerDefaults.value.isNotEmpty()
    }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_read_settings_general() = snap("phone-read-settings-general", FixtureScene.READING_HOME, page = settingsRoute(SettingsPane.General))

    @Test
    @Config(qualifiers = PHONE)
    fun phone_read_settings_downloads() = snap("phone-read-settings-downloads", FixtureScene.READING_HOME, page = settingsRoute(SettingsPane.Downloads))

    @Test
    @Config(qualifiers = PHONE)
    fun phone_read_settings_downloads_alone() = snap("phone-read-settings-downloads-alone", FixtureScene.READING_HOME, page = settingsRoute(SettingsPane.Downloads), medium = KaeriIsFor.READ)

    @Test
    @Config(qualifiers = PHONE)
    fun phone_read_settings_notifications() = snap("phone-read-settings-notifications", FixtureScene.READING_HOME, page = settingsRoute(SettingsPane.Notifications))

    @Test
    @Config(qualifiers = PHONE)
    fun phone_read_settings_sources_alone() = snap("phone-read-settings-sources-alone", FixtureScene.READING_HOME, page = settingsRoute(SettingsPane.Sources), medium = KaeriIsFor.READ) {
        it.bookSources.view.value != null
    }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_read_settings_accounts_alone() = snap("phone-read-settings-accounts-alone", FixtureScene.READING_HOME, page = settingsRoute(SettingsPane.Accounts), medium = KaeriIsFor.READ)

    @Test
    @Config(qualifiers = PHONE)
    fun phone_read_settings_accounts_both() = snap("phone-read-settings-accounts-both", FixtureScene.READING_HOME, page = settingsRoute(SettingsPane.Accounts))

    @Test
    @Config(qualifiers = PHONE)
    fun phone_read_mangaupdates_sheet() = snap(
        "phone-read-mangaupdates-sheet", FixtureScene.READING_HOME, page = settingsRoute(SettingsPane.Accounts), medium = KaeriIsFor.READ, screen = true,
        act = { tap(text(R.string.accounts_sign_in), last = true) },
    )

    // Book sources

    @Test
    @Config(qualifiers = PHONE)
    fun phone_read_book_sources() = snap("phone-read-book-sources", FixtureScene.READING_HOME, page = BOOK_SOURCES_ROUTE) { it.bookSources.view.value != null }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_read_add_source() = snap(
        "phone-read-add-source", FixtureScene.READING_HOME, page = BOOK_SOURCES_ROUTE, screen = true,
        act = {
            pump("the book sources") { s -> s.bookSources.view.value != null }
            run(20)
            tap(text(R.string.sources_add))
        },
    )

    @Test
    @Config(qualifiers = PHONE)
    fun phone_read_add_server_unreachable() = snap(
        "phone-read-add-server-unreachable", FixtureScene.READING_HOME, page = BOOK_SOURCES_ROUTE, screen = true,
        act = {
            pump("the book sources") { s -> s.bookSources.view.value != null }
            run(20)
            tap(text(R.string.sources_add))
            tap(text(R.string.book_add_server))
            compose.onAllNodes(hasSetTextAction())[0].performTextInput("books.home.example") // i18n-skip: a made-up host
            run(30)
        },
    ) { it.bookSources.server.value is ServerAnswer.Failed }

    // The tablet

    @Test
    @Config(qualifiers = TABLET)
    fun tablet_read_settings_alone() = snap("tablet-read-settings-alone", FixtureScene.READING_HOME, page = settingsRoute(SettingsPane.Reading), medium = KaeriIsFor.READ) {
        it.reading.readerDefaults.value.isNotEmpty()
    }

    @Test
    @Config(qualifiers = TABLET)
    fun tablet_read_settings_accounts() = snap("tablet-read-settings-accounts", FixtureScene.READING_HOME, page = settingsRoute(SettingsPane.Accounts))

    companion object {
        const val PHONE = ReadingShotHarness.PHONE
        const val TABLET = ReadingShotHarness.TABLET
    }
}
