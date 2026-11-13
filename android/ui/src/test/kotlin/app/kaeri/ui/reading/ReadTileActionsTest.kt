// A reading cover's long-press menu reaches TalkBack as the cover's custom actions, as Watch's
// covers do: Details, then an "Add to" action for each list but the one on screen. A cover of
// the person's own offers Details alone.
package app.kaeri.ui.reading

import androidx.activity.ComponentActivity
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.unit.dp
import androidx.navigation.compose.rememberNavController
import app.kaeri.design.KaeriTheme
import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.LibrarySegmentRead
import app.kaeri.engine.model.ReadingCommand
import app.kaeri.engine.model.WorkTile
import app.kaeri.store.AppStore
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
    private val tile = WorkTile(work = "a:7", title = "Lantern Ferry", medium = Medium.MANGA) // i18n-skip

    private fun show(current: LibrarySegmentRead?, lists: Boolean = true) {
        compose.setContent {
            KaeriTheme {
                CompositionLocalProvider(
                    LocalAppStore provides store,
                    LocalNavigator provides Navigator(rememberNavController(), Destination.Home),
                ) {
                    ReadTile(tile, 120.dp, from = "test", actions = readTileActions(tile, current, lists))
                }
            }
        }
        compose.waitForIdle()
    }

    /** The cover's actions: its title is cleared under the cover's own words, so it is found by them. */
    private fun actions() = compose.onAllNodes(SemanticsMatcher.keyIsDefined(SemanticsActions.CustomActions))
        .fetchSemanticsNodes().single().config[SemanticsActions.CustomActions]

    @Test
    fun aLibraryCoverOffersDetailsAndEveryOtherList() {
        show(current = LibrarySegmentRead.READING)
        assertEquals(
            listOf("Details", "Add to Planning", "Add to Completed", "Add to Paused", "Add to Dropped", "Add to Rereading"), // i18n-skip
            actions().map { it.label },
        )
    }

    @Test
    fun aSearchCoverOffersEveryList() {
        show(current = null)
        assertEquals(
            listOf("Details", "Add to Reading", "Add to Planning", "Add to Completed", "Add to Paused", "Add to Dropped", "Add to Rereading"), // i18n-skip
            actions().map { it.label },
        )
    }

    @Test
    fun aCoverOfThePersonsOwnOffersDetailsAlone() {
        show(current = null, lists = false)
        assertEquals(listOf("Details"), actions().map { it.label }) // i18n-skip
    }

    @Test
    fun anAddActionPutsTheSeriesOnThatList() {
        show(current = LibrarySegmentRead.READING)
        val completed = actions().first { it.label == "Add to Completed" } // i18n-skip
        compose.runOnUiThread { completed.action() }
        compose.waitForIdle()
        assertTrue(store.waiting.any { it == Command.Reading(ReadingCommand.SetStatus("a:7", LibrarySegmentRead.COMPLETED)) })
    }
}
