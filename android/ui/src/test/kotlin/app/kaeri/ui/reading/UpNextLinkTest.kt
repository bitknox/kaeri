// Read Home's Up next: View all opens Planning, so it shows only with Planning series in the row.
// A row of suggestions alone, as the engine gives with nothing on Planning, has no View all.

package app.kaeri.ui.reading

import android.content.Context
import androidx.activity.ComponentActivity
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.navigation.compose.rememberNavController
import androidx.test.core.app.ApplicationProvider
import app.kaeri.design.KaeriTheme
import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.Msg
import app.kaeri.engine.model.ReadingMsg
import app.kaeri.engine.model.WorkTile
import app.kaeri.i18n.Strings
import app.kaeri.store.AppStore
import app.kaeri.ui.Destination
import app.kaeri.ui.LocalAppStore
import app.kaeri.ui.LocalNavigator
import app.kaeri.ui.Navigator
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class UpNextLinkTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val store = AppStore(CoroutineScope(SupervisorJob() + Dispatchers.Main))
    private val context: Context = ApplicationProvider.getApplicationContext()

    // What the heading's link says to a screen reader.
    private val link get() = "${Strings.homeViewAll(context)}, ${Strings.homeUpNext(context)}" // i18n-skip

    private fun tile(work: String, planning: Boolean) = WorkTile(
        work = work,
        title = "Lantern Street Notes", // i18n-skip
        medium = Medium.MANGA,
        facts = listOf(Msg.Reading(ReadingMsg.ReadingKindManga)) +
            if (planning) listOf(Msg.Reading(ReadingMsg.ReadingOnPlanning)) else emptyList(),
    )

    private fun show(tiles: List<WorkTile>) {
        compose.setContent {
            KaeriTheme {
                CompositionLocalProvider(
                    LocalAppStore provides store,
                    LocalNavigator provides Navigator(rememberNavController(), Destination.Read),
                ) { UpNextShelf(tiles) {} }
            }
        }
        compose.waitForIdle()
    }

    @Test
    fun suggestions_alone_have_no_view_all() {
        val row = listOf(tile("a:900301", planning = false), tile("a:900302", planning = false))
        assertFalse(upNextLinksPlanning(row))
        show(row)
        compose.onNodeWithContentDescription(link).assertDoesNotExist()
    }

    @Test
    fun a_row_with_planning_leads_to_planning() {
        val row = listOf(tile("a:900301", planning = true), tile("a:900302", planning = false))
        assertTrue(upNextLinksPlanning(row))
        show(row)
        compose.onNodeWithContentDescription(link).assertExists()
    }
}
