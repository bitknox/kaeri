// What TalkBack gets for a tap zone tile on the phone: the layout and where its zones are as the
// words, and the radio's own state, Selected or Not selected, which a state description of its own
// would replace.

package app.kaeri.reader

import android.view.View
import android.view.ViewGroup
import android.view.accessibility.AccessibilityNodeInfo
import androidx.activity.ComponentActivity
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import app.kaeri.design.ObiSilk
import app.kaeri.engine.model.ReadDirection
import app.kaeri.engine.model.ReaderMode
import app.kaeri.engine.model.TapReading
import app.kaeri.engine.model.ZoneLayout
import app.kaeri.engine.model.tapMap
import app.kaeri.reader.phone.ReaderSettingsSheet
import app.kaeri.reader.phone.SettingsPane
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class TapZoneSpeechTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val session = ReaderSession {}
    private val dir = kotlin.io.path.createTempDirectory("tap-zone-speech").toFile()

    private fun spokenAs(start: String) = SemanticsMatcher("words that start with $start") { node ->
        node.config.getOrNull(SemanticsProperties.ContentDescription).orEmpty().any { it.startsWith(start) }
    }

    /** What Android hands TalkBack for the node that says [start]. */
    private fun info(start: String): AccessibilityNodeInfo {
        val id = compose.onNode(spokenAs(start)).fetchSemanticsNode().id
        val content = compose.activity.findViewById<ViewGroup>(android.R.id.content)
        val view = generateSequence<View>(content) { (it as? ViewGroup)?.takeIf { group -> group.childCount > 0 }?.getChildAt(0) }
            .first { it.javaClass.name.endsWith("AndroidComposeView") }
        return compose.runOnIdle { view.accessibilityNodeProvider.createAccessibilityNodeInfo(id)!! }
    }

    @Test
    fun a_tile_says_its_layout_and_keeps_its_selected_state() {
        val state = TapZonesScenes.state(dir).copy(leaves = ReaderFixtures.leaves(3))
        compose.setContent {
            Box(Modifier.fillMaxSize().background(ObiSilk.Ground)) {
                ReaderSettingsSheet(session = session, state = state, onDismiss = {}, instant = true, start = SettingsPane.TapZones)
            }
        }
        compose.waitForIdle()
        val reading = TapReading(ReaderMode.PAGED, ReadDirection.RIGHT_TO_LEFT, false, false)
        val zones = TapZonesScenes.zones()

        val auto = info("Auto, ")
        assertEquals("Auto, " + tapMap(zones, reading, ZoneLayout.AUTO).about, auto.contentDescription?.toString())
        assertEquals("Selected", auto.stateDescription?.toString())

        val top = info("Top and bottom, ")
        assertEquals("Top and bottom, " + tapMap(zones, reading, ZoneLayout.TOP_BOTTOM).about, top.contentDescription?.toString())
        assertEquals("Not selected", top.stateDescription?.toString())
    }
}
