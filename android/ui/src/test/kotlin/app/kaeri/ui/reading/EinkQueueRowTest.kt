package app.kaeri.ui.reading

import androidx.activity.ComponentActivity
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.navigation.compose.rememberNavController
import app.kaeri.design.KaeriTheme
import app.kaeri.eink.EinkLook
import app.kaeri.engine.model.BookDownloadRow
import app.kaeri.engine.model.BookDownloadSeries
import app.kaeri.engine.model.BookDownloadsView
import app.kaeri.engine.model.ChapterDownloadState
import app.kaeri.engine.model.ChapterReadState
import app.kaeri.engine.model.ChapterRow
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.ReadingCommand
import app.kaeri.engine.model.ReadingEvent
import app.kaeri.engine.model.ReadingFailure
import app.kaeri.engine.model.StorageMeter
import app.kaeri.store.AppStore
import app.kaeri.store.HAS_TESTING_ENGINE
import app.kaeri.ui.Destination
import app.kaeri.ui.LocalAppStore
import app.kaeri.ui.LocalNavigator
import app.kaeri.ui.Navigator
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

/**
 * A chapter that failed to download on a 10 inch panel's Downloads, with a reason long enough to
 * fill the row: the series and chapter still show, Retry and Remove stay whole, and Remove takes
 * the failure off the list. At the panel's own text size and at twice it.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w992dp-h1323dp-port-xhdpi")
class EinkQueueRowTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val store = AppStore(CoroutineScope(SupervisorJob() + Dispatchers.Main))

    private val chapter = ChapterRow(
        id = "3", label = "3", state = ChapterReadState.Unread, number = "3", title = null, volume = null,
        published = null, pages = null, newChapter = false, download = null, versions = 1u, bookmarked = false,
    )
    private val long = "/data/user/0/app.example/files/kaeri/book-downloads/" + "a-long-folder-name/".repeat(12) + "chapter.part"
    private val failed = ChapterDownloadState.Failed(ReadingFailure.FileMissing(long))
    private val view = BookDownloadsView(
        rows = listOf(BookDownloadRow("l:paper-kite", "Paper Kite Club", chapter, failed)),
        meter = StorageMeter(used = 0u, free = 0u, chapters = 0u, readBytes = 0u),
        series = listOf(BookDownloadSeries(work = "l:paper-kite", title = "Paper Kite Club", cover = null, chapters = 0u, bytes = 0u, read = 0u, queued = 0u, failed = listOf("3"), line = emptyList(), progress = null)),
        total = emptyList(),
    )

    private fun page() {
        // Known before the page draws, so the panel's pacing has nothing to hold back.
        compose.runOnUiThread { store.apply(Event.Reading(ReadingEvent.Downloads(view))) }
        compose.setContent {
            KaeriTheme {
                CompositionLocalProvider(
                    LocalAppStore provides store,
                    LocalNavigator provides Navigator(rememberNavController(), Destination.Home),
                    LocalEinkLayout provides EinkLook(),
                ) { EinkDownloadsPage() }
            }
        }
        compose.waitForIdle()
    }

    private fun aFailedRowShowsItsTitleAndBothActions() {
        page()
        compose.onNodeWithText("Paper Kite Club", substring = true).assertIsDisplayed()
        compose.onNodeWithText("Retry").assertIsDisplayed()
        compose.onNodeWithText("Remove").assertIsDisplayed().performClick()
        compose.waitForIdle()
        assertTrue(store.waiting.any { it is Command.Reading && it.v1 == ReadingCommand.DownloadCancel("l:paper-kite", listOf("3")) })
    }

    @Test
    fun a_failed_row_with_a_long_reason_keeps_its_title_retry_and_remove() {
        assumeTrue(HAS_TESTING_ENGINE)
        aFailedRowShowsItsTitleAndBothActions()
    }

    @Test
    fun at_twice_the_text_size_the_failed_row_still_holds() {
        assumeTrue(HAS_TESTING_ENGINE)
        RuntimeEnvironment.setFontScale(2f)
        aFailedRowShowsItsTitleAndBothActions()
    }
}
