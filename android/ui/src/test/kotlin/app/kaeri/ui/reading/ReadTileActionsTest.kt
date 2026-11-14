// A reading cover's long-press menu reaches TalkBack as the cover's custom actions, as Watch's
// covers do: Details, then an "Add to" action for each list but the one on screen. The cover is
// read in Library as TalkBack finds it, by its words.
package app.kaeri.ui.reading

import androidx.activity.ComponentActivity
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.navigation.compose.rememberNavController
import app.kaeri.design.KaeriTheme
import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.LibrarySegmentRead
import app.kaeri.engine.model.ReadingCommand
import app.kaeri.engine.model.ReadingEvent
import app.kaeri.engine.model.WorkTile
import app.kaeri.store.AppStore
import app.kaeri.ui.AppActions
import app.kaeri.ui.Destination
import app.kaeri.ui.LocalAppStore
import app.kaeri.ui.LocalNavigator
import app.kaeri.ui.Navigator
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class ReadTileActionsTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val store = AppStore(CoroutineScope(SupervisorJob() + Dispatchers.Main))
    private val title = "Lantern Ferry" // i18n-skip
    private val tile = WorkTile(work = "a:7", title = title, medium = Medium.MANGA)

    /** Library on the Reading list, holding the one cover. */
    private fun showLibrary() {
        ReadPlaces.list = LibrarySegmentRead.READING
        store.apply(Event.Reading(ReadingEvent.Library(LibrarySegmentRead.READING, listOf(tile))))
        compose.setContent {
            KaeriTheme {
                CompositionLocalProvider(
                    LocalAppStore provides store,
                    LocalNavigator provides Navigator(rememberNavController(), Destination.Home),
                ) {
                    ReadLibraryScreen(AppActions())
                }
            }
        }
        compose.waitForIdle()
    }

    /** The custom actions of the node TalkBack reads as the cover, found by the title in its words. */
    private fun actions(): List<CustomAccessibilityAction> {
        val covers = compose.onAllNodes(hasContentDescription(title, substring = true)).fetchSemanticsNodes()
        assertTrue("the cover shows", covers.isNotEmpty())
        return covers.flatMap { it.config.getOrNull(SemanticsActions.CustomActions).orEmpty() }
    }

    @Test
    fun aLibraryCoverOffersDetailsAndEveryOtherList() {
        showLibrary()
        assertEquals(
            listOf("Details", "Add to Planning", "Add to Completed", "Add to Paused", "Add to Dropped", "Add to Rereading"), // i18n-skip
            actions().map { it.label },
        )
    }

    @Test
    fun anAddActionPutsTheSeriesOnThatList() {
        showLibrary()
        val completed = actions().firstOrNull { it.label == "Add to Completed" } // i18n-skip
        assertTrue("the cover offers Add to Completed", completed != null)
        compose.runOnUiThread { completed!!.action() }
        compose.waitForIdle()
        assertTrue(store.waiting.any { it == Command.Reading(ReadingCommand.SetStatus("a:7", LibrarySegmentRead.COMPLETED)) })
    }
}
