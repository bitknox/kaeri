// Library's grid of series, as a folder of thousands fills it: a cover is one composition, the
// grid's own for its cell, with none of its own beside it, so a fling composes each new cover
// once. The covers still fill each row to the pixel, as the grid lays its columns out.

package app.kaeri.ui.reading

import androidx.activity.ComponentActivity
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.tooling.CompositionData
import androidx.compose.runtime.tooling.LocalInspectionTables
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onRoot
import androidx.navigation.compose.rememberNavController
import app.kaeri.design.KaeriTheme
import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.LibrarySegmentRead
import app.kaeri.engine.model.ReadingEvent
import app.kaeri.engine.model.WorkTile
import app.kaeri.store.AppStore
import app.kaeri.store.HAS_TESTING_ENGINE
import app.kaeri.ui.AppActions
import app.kaeri.ui.Destination
import app.kaeri.ui.LocalAppStore
import app.kaeri.ui.LocalNavigator
import app.kaeri.ui.Navigator
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import org.junit.Assert.assertEquals
import org.junit.Assume.assumeTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class ReadLibraryGridTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val store = AppStore(CoroutineScope(SupervisorJob() + Dispatchers.Main))

    /** Every composition under the screen while it lives: its own and each one inside it. */
    private val compositions = mutableSetOf<CompositionData>()

    private fun tiles(count: Int) = List(count) { WorkTile(work = "local:$it", title = TITLES[it], medium = Medium.MANGA) }

    private fun files(count: Int) {
        compose.runOnUiThread { store.apply(Event.Reading(ReadingEvent.Library(LibrarySegmentRead.YOUR_FILES, tiles(count)))) }
        compose.waitForIdle()
    }

    /** Your files open in Library with [count] series in it. */
    private fun open(count: Int) {
        assumeTrue(HAS_TESTING_ENGINE)
        ReadPlaces.list = LibrarySegmentRead.YOUR_FILES
        files(count)
        compose.setContent {
            KaeriTheme {
                CompositionLocalProvider(
                    LocalAppStore provides store,
                    LocalNavigator provides Navigator(rememberNavController(), Destination.Library),
                    LocalInspectionTables provides compositions,
                ) { ReadLibraryScreen(AppActions()) }
            }
        }
        compose.waitForIdle()
    }

    /** Where each series' cover and words sit, by its title. */
    private fun cells(count: Int): List<Rect> = (0 until count).map { index ->
        compose.onNode(hasClickAction() and hasContentDescription(TITLES[index], substring = true)).fetchSemanticsNode().boundsInRoot
    }

    @Test
    @Config(qualifiers = "w412dp-h1400dp-port-420dpi")
    fun a_cover_adds_only_its_cells_composition() {
        // One row of three, then a second row comes into view.
        open(3)
        val oneRow = compositions.size
        files(6)
        cells(6)
        assertEquals("compositions for three more covers", 3, compositions.size - oneRow)
    }

    /** The first row's covers sit [gap] apart, [padding] in from the right, [columns] of them. */
    private fun assertRowFills(columns: Int, padding: Float, gap: Float) {
        val row = cells(columns + 1)
        val first = row.take(columns)
        assertEquals("one row of $columns", listOf(true, false), listOf(first.all { it.top == row[0].top }, row[columns].top == row[0].top))
        first.zipWithNext { left, right -> assertEquals(gap, right.left - left.right, 0.5f) }
        val width = compose.onRoot().fetchSemanticsNode().boundsInRoot.right
        assertEquals(width - padding, first.last().right, 0.5f)
    }

    @Test
    @Config(qualifiers = "w412dp-h1400dp-port-420dpi")
    fun a_phones_three_covers_fill_their_row() {
        open(6)
        // 16 and 13 dp at 2.625 pixels a dp, rounded as the grid rounds them.
        assertRowFills(columns = 3, padding = 42f, gap = 34f)
    }

    @Test
    @Config(qualifiers = "w1280dp-h1400dp-land-xhdpi")
    fun a_wide_windows_covers_fill_their_row() {
        open(TITLES.size)
        val first = cells(TITLES.size)
        val columns = first.count { it.top == first[0].top }
        assertRowFills(columns = columns, padding = 32f, gap = 26f)
    }

    private companion object {
        val TITLES = listOf(
            "Amber Lantern", "Brass Heron", "Cinder Orchard", "Drift Kettle", "Ember Loom", "Fern Quay",
            "Glass Meadow", "Hollow Reed", "Ivory Ferry", "Juniper Bell", "Kite Harbor", "Linen Moth",
        )
    }
}
