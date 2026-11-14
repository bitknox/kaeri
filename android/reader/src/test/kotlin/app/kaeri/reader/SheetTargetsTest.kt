// Every touch target in the reader's settings sheet is at least 48 dp each way: rows, switches,
// choices, the search field and the tap zone tiles, on the pages of manga and of novels.

package app.kaeri.reader

import androidx.activity.ComponentActivity
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.unit.dp
import app.kaeri.design.ObiSilk
import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.ReaderSettingKey as Key
import app.kaeri.engine.model.ReaderSettingRow
import app.kaeri.engine.model.ReaderSettingsView
import app.kaeri.engine.model.ReaderValue
import app.kaeri.engine.model.SettingScope
import app.kaeri.reader.phone.ReaderSettingsSheet
import app.kaeri.reader.phone.SettingsPane
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class SheetTargetsTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val session = ReaderSession {}

    private fun choice(key: Key, value: String, vararg all: String) =
        ReaderSettingRow(key, ReaderValue.Choice(value), SettingScope.DEVICE, false, false, all.map { ReaderValue.Choice(it) })
    private fun toggle(key: Key, on: Boolean) = ReaderSettingRow(key, ReaderValue.Toggle(on), SettingScope.DEVICE, false, false, emptyList())
    private fun amount(key: Key, value: Double) = ReaderSettingRow(key, ReaderValue.Amount(value), SettingScope.DEVICE, false, false, emptyList())

    private val zones = TapZonesScenes.zones()
    private val tapRows = TapZonesScenes.settings(zones).rows

    private val mangaRows = tapRows + listOf(
        choice(Key.MODE, "paged", "paged", "vertical", "strip"),
        choice(Key.DIRECTION, "rtl", "rtl", "ltr"),
        choice(Key.FIT, "screen", "screen", "width", "height"),
        choice(Key.BACKGROUND, "theme", "theme", "black", "white"),
        choice(Key.PAGE_LAYOUT, "auto", "auto", "single", "double"),
        choice(Key.SPREADS, "split", "split", "rotate", "whole"),
        choice(Key.COLOUR, "auto", "auto", "colour", "greys"),
        toggle(Key.CROP_BORDERS, false),
        toggle(Key.VOLUME_KEYS_TURN, true),
        choice(Key.FOOT_VALUE, "pages_left", "none", "pages_left"),
        toggle(Key.PAPER_MODE, false),
    )

    private val novelRows = tapRows + listOf(
        amount(Key.TEXT_SIZE, 1.0),
        choice(Key.FONT, "serif", "serif", "sans"),
        choice(Key.LOOK, "system", "system", "paper", "light", "sepia", "dark"),
        amount(Key.LINE_SPACING, 1.5),
        amount(Key.MARGINS, 1.0),
        toggle(Key.JUSTIFY, true),
        choice(Key.MODE, "paged", "paged", "scrolled"),
        toggle(Key.VOLUME_KEYS_TURN, false),
    )

    private fun state(novel: Boolean): ReaderState {
        val view = ReaderFixtures.view().let { if (novel) it.copy(medium = Medium.NOVEL) else it }
        return ReaderState(view = view, leaves = ReaderFixtures.leaves(3), settings = ReaderSettingsView(view.medium, ReaderFixtures.WORK, if (novel) novelRows else mangaRows, zones))
    }

    /** The click targets smaller than 48 dp on each of [panes], with what they say. */
    private fun small(panes: List<SettingsPane>, novel: Boolean = false): List<String> {
        var shown by mutableStateOf(panes.first())
        val state = state(novel)
        compose.setContent {
            Box(Modifier.fillMaxSize().background(ObiSilk.Ground)) {
                // A new sheet for each page, as start is read when the sheet opens.
                key(shown) { ReaderSettingsSheet(session = session, state = state, onDismiss = {}, instant = true, start = shown) }
            }
        }
        val least = with(compose.density) { 48.dp.roundToPx() } - 1
        return panes.flatMap { pane ->
            shown = pane
            compose.waitForIdle()
            compose.onAllNodes(hasClickAction(), useUnmergedTree = true).fetchSemanticsNodes()
                .filter { it.layoutInfo.width < least || it.layoutInfo.height < least }
                .map { node ->
                    val words = node.config.getOrNull(SemanticsProperties.Text)?.joinToString()
                        ?: node.config.getOrNull(SemanticsProperties.ContentDescription)?.joinToString()
                        ?: node.children.flatMap { it.config.getOrNull(SemanticsProperties.Text).orEmpty() }.joinToString()
                    val size = with(compose.density) { "${node.layoutInfo.width.toDp()} x ${node.layoutInfo.height.toDp()}" }
                    "$pane '$words' $size"
                }
        }
    }

    @Test
    fun the_manga_pages_have_48_dp_targets() {
        val small = small(SettingsPane.entries.filter { it != SettingsPane.Font })
        assertTrue(small.joinToString("\n"), small.isEmpty())
    }

    @Test
    fun the_novel_pages_have_48_dp_targets() {
        val small = small(listOf(SettingsPane.Main, SettingsPane.More, SettingsPane.Font), novel = true)
        assertTrue(small.joinToString("\n"), small.isEmpty())
    }
}
