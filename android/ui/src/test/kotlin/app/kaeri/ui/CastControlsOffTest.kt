// While the TV reconnects, the controller's dimmed controls are off, and so are the words in
// them: Compose lists a control's words as a node of their own, and a dimmed Play here read as
// enabled there. A screen reader is handed a control's name on a node of its own when the control
// has nodes under it, and that node can't be off, so Back 10, Forward 10 and the seek line keep
// their name on the control's own node. Stop casting stays live. The TV's button says
// Reconnecting then, as the line beside it does.

package app.kaeri.ui

import android.view.View
import android.view.ViewGroup
import android.view.accessibility.AccessibilityNodeInfo
import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.Column
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.platform.ViewRootForTest
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import app.kaeri.design.KaeriTheme
import app.kaeri.engine.ffi.FfiNowCard
import app.kaeri.i18n.R
import app.kaeri.store.CastModel
import app.kaeri.store.CastShown
import app.kaeri.store.TvPlayRequest
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class CastControlsOffTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private fun text(id: Int) = compose.activity.getString(id)

    /** The node with [words], and the words' own node under it, are both off or both on. */
    private fun assertOff(words: String, off: Boolean) {
        for (unmerged in listOf(false, true)) {
            val node = compose.onNodeWithText(words, useUnmergedTree = unmerged)
            if (off) node.assertIsNotEnabled() else node.assertIsEnabled()
        }
    }

    /**
     * The nodes a screen reader is handed that carry [label]. A merged control with nodes under it
     * also hands its name and its role on extra nodes, at these offsets from its own id.
     */
    private fun spoken(label: String): List<AccessibilityNodeInfo> {
        fun root(view: View): View? = when {
            view is ViewRootForTest -> view
            view is ViewGroup -> (0 until view.childCount).firstNotNullOfOrNull { root(view.getChildAt(it)) }
            else -> null
        }
        val provider = root(compose.activity.window.decorView)!!.accessibilityNodeProvider!!
        val ids = compose.onAllNodes(SemanticsMatcher("any node") { true }, useUnmergedTree = true).fetchSemanticsNodes().map { it.id } // i18n-skip
        return ids.flatMap { listOf(it, it + 1_000_000_000, it + 2_000_000_000) }
            .mapNotNull { provider.createAccessibilityNodeInfo(it) }
            .filter { it.contentDescription?.toString() == label }
    }

    /** Every node that names [label] to a screen reader is off: it finds the control dimmed. */
    private fun assertSpokenOff(label: String) {
        val nodes = spoken(label)
        assertTrue(nodes.isNotEmpty())
        nodes.forEach { assertFalse(label, it.isEnabled) }
    }

    @Test
    fun whileItReconnectsPlayHereAndItsWordsAreOffAndStopCastingIsNot() {
        var played = 0
        var stopped = 0
        val playHere = text(R.string.cast_play_here)
        val stop = text(R.string.cast_stop)
        compose.setContent {
            KaeriTheme {
                Column {
                    CastFooterButton(CastAction(playHere, primary = false, enabled = false) { played++ }, reduced = true)
                    CastFooterButton(CastAction(stop, primary = false, enabled = true) { stopped++ }, reduced = true)
                }
            }
        }
        assertOff(playHere, off = true)
        assertOff(stop, off = false)
        compose.onNodeWithText(playHere).performClick()
        compose.onNodeWithText(stop).performClick()
        assertEquals(0, played)
        assertEquals(1, stopped)
    }

    @Test
    fun aDimmedVolumeRowsUnmuteIsOff() {
        val cast = CastModel(CoroutineScope(Dispatchers.Unconfined))
        compose.setContent {
            KaeriTheme { CastVolumeRow(cast, level = false, muted = true, volume = null, enabled = false) }
        }
        assertOff(text(R.string.cast_card_unmute), off = true)
    }

    @Test
    fun dimmedStepsTenAreOffByTheirNames() {
        compose.setContent {
            KaeriTheme {
                Column {
                    TvStep(Icons.Replay, text(R.string.player_back_10), enabled = false) {}
                    TvStep(Icons.Ahead, text(R.string.player_forward_10), enabled = false) {}
                }
            }
        }
        assertSpokenOff(text(R.string.player_back_10))
        assertSpokenOff(text(R.string.player_forward_10))
    }

    @Test
    fun whileItReconnectsTheSeekLineIsOffByItsName() {
        val cast = CastModel(CoroutineScope(Dispatchers.Unconfined))
        val now = FfiNowCard(
            id = 900001u, title = "The Lantern Road", episode = "7", episodeLine = "Episode 7", loading = false, // i18n-skip
            position = 120.0, duration = 1420.0, paused = false, speed = 1.0, volume = 0.5, audio = emptyList(),
            subtitles = emptyList(), subtitlesOff = false, skip = null,
        )
        val start = CastModel.starting("Living room", level = true) // i18n-skip
        val card = start.copy(card = start.card.copy(now = now), reconnecting = true)
        val shown = CastShown(
            cast = card, at = 0L, tvName = "Living room", // i18n-skip
            request = TvPlayRequest(id = 900001u, title = "The Lantern Road", episode = "7"), // i18n-skip
            startedAt = 0L, played = true,
        )
        compose.setContent {
            KaeriTheme { CastSeekLine(cast, now, shown, usable = false, dimmed = true) }
        }
        assertSpokenOff(text(R.string.tv_sheet_position))
    }

    @Test
    fun whileItReconnectsTheTvsButtonSaysReconnecting() {
        val name = "Living room" // i18n-skip
        val reconnecting = mutableStateOf(true)
        compose.setContent {
            KaeriTheme { CastHeldButton(name, reconnecting.value) {} }
        }
        val again = compose.activity.getString(R.string.tv_status_reconnecting, name)
        val held = compose.activity.getString(R.string.cast_button_connected, name)
        compose.onNodeWithContentDescription(again).assertExists()
        compose.onAllNodesWithContentDescription(held).assertCountEquals(0)
        reconnecting.value = false
        compose.onNodeWithContentDescription(held).assertExists()
        compose.onAllNodesWithContentDescription(again).assertCountEquals(0)
    }
}
