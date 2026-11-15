// Settings, Sources for a person who only reads: the pane is the book sources, and Add a source
// stays on it once a source is there, not only in the empty state.

package app.kaeri.ui

import androidx.activity.ComponentActivity
import androidx.compose.material3.SnackbarHostState
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithText
import androidx.navigation.compose.rememberNavController
import app.kaeri.design.KaeriTheme
import app.kaeri.engine.model.AppEdition
import app.kaeri.engine.model.BookFolderRow
import app.kaeri.engine.model.BookFolderScan
import app.kaeri.engine.model.BookSourceEvent
import app.kaeri.engine.model.BookSourcesView
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.engine.model.MediumView
import app.kaeri.i18n.R
import app.kaeri.store.AppStore
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w1280dp-h800dp-land-xhdpi")
class SourcesPaneAddTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val store = AppStore(CoroutineScope(SupervisorJob() + Dispatchers.Main))

    /** Kaeri is for reading here, as the engine would say. */
    private val reading = MediumView(answer = KaeriIsFor.READ, edition = AppEdition.FULL, fixed = false, readingAvailable = true, ask = null, account = null)

    private fun pane(view: BookSourcesView) {
        compose.runOnUiThread { store.apply(listOf(Event.Medium(reading), Event.BookSources(BookSourceEvent.View(view)))) }
        compose.setContent {
            KaeriTheme {
                val snackbarScope = rememberCoroutineScope()
                val snackbars = remember { Snackbars(SnackbarHostState(), snackbarScope) }
                CompositionLocalProvider(
                    LocalAppStore provides store,
                    LocalSnackbars provides snackbars,
                    LocalNavigator provides Navigator(rememberNavController(), Destination.Home),
                ) { SourcesPane(AppActions()) }
            }
        }
        compose.waitForIdle()
    }

    @Test
    fun with_a_folder_added_add_a_source_is_still_on_the_pane() {
        pane(BookSourcesView(folders = listOf(BookFolderRow(id = "f1", name = "Comics folder", scan = BookFolderScan.Done)))) // i18n-skip
        compose.onNodeWithText(compose.activity.getString(R.string.sources_add_title)).assertIsDisplayed()
    }

    @Test
    fun with_none_the_empty_state_holds_the_one_add() {
        pane(BookSourcesView())
        compose.onAllNodesWithText(compose.activity.getString(R.string.sources_add)).assertCountEquals(1)
        compose.onAllNodesWithText(compose.activity.getString(R.string.sources_add_title)).assertCountEquals(1)
    }
}
