// While the TV reconnects, the controller's dimmed controls are off, and so are the words in
// them: Compose lists a control's words as a node of their own, and a dimmed Play here read as
// enabled there. Stop casting stays live.

package app.kaeri.ui

import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.Column
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import app.kaeri.design.KaeriTheme
import app.kaeri.i18n.R
import app.kaeri.store.CastModel
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import org.junit.Assert.assertEquals
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
    fun aDimmedStepsTenIsOff() {
        compose.setContent {
            KaeriTheme { TvStep(Icons.Replay, text(R.string.player_back_10), enabled = false) {} }
        }
        compose.onNodeWithText("10", useUnmergedTree = true).assertIsNotEnabled() // i18n-skip
    }
}
