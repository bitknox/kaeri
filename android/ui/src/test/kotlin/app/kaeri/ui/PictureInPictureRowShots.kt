package app.kaeri.ui

import androidx.activity.ComponentActivity
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.unit.dp
import app.kaeri.design.KaeriTheme
import app.kaeri.testkit.captureRoboImage
import com.github.takahirom.roborazzi.roborazziSystemPropertyOutputDirectory
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * Settings, Playback, Picture and sound's Picture in picture row as the Hayase holes board draws
 * it: the switch on and off, the system's permission off with Open settings, and the crossfade
 * back to the switch halfway through. Captured to android/.shots/roborazzi/.
 */
@OptIn(com.github.takahirom.roborazzi.ExperimentalRoborazziApi::class)
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w412dp-h400dp-port-420dpi")
class PictureInPictureRowShots {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private var on by mutableStateOf(true)
    private var allowed by mutableStateOf(true)
    private val changes = mutableListOf<Boolean>()

    private fun show() = compose.setContent {
        KaeriTheme {
            Box(Modifier.background(MaterialTheme.colorScheme.surface).padding(16.dp)) {
                SettingsGroup(
                    title = "Picture and sound", // i18n-skip
                    modifier = Modifier.fillMaxWidth(),
                    rows = listOf(
                        { place -> SwitchRow(place, "Night mode", false, onChange = {}) }, // i18n-skip
                        { place -> PictureInPictureRow(place, on, allowed) { changes += it } },
                    ),
                )
            }
        }
    }

    private fun capture(name: String) {
        compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/$name.png")
    }

    @Test
    fun the_switch_on_reads_as_a_switch_and_turns_off() {
        show()
        compose.waitForIdle()
        val row = compose.onNode(pipSwitch)
        row.assertExists()
        capture("android-pip-row-on")
        row.performClick()
        assertEquals(listOf(false), changes)
    }

    @Test
    fun the_switch_off() {
        on = false
        show()
        compose.waitForIdle()
        capture("android-pip-row-off")
    }

    @Test
    fun the_permission_off_says_so_and_offers_open_settings() {
        allowed = false
        show()
        compose.waitForIdle()
        compose.onNodeWithText("Off in Android settings").assertExists()
        compose.onNodeWithText("Open settings").assertExists()
        // No switch to reach while the system has the say.
        compose.onAllNodes(pipSwitch).assertCountEquals(0)
        capture("android-pip-row-permission-off")
    }

    @Test
    fun coming_back_with_the_permission_on_crosses_to_the_switch() {
        allowed = false
        show()
        compose.waitForIdle()
        compose.mainClock.autoAdvance = false
        allowed = true
        compose.mainClock.advanceTimeByFrame()
        compose.mainClock.advanceTimeBy(150)
        capture("android-pip-row-crossfade-half")
        compose.mainClock.advanceTimeBy(1_000)
        compose.mainClock.autoAdvance = true
        compose.waitForIdle()
        compose.onAllNodesWithText("Open settings").assertCountEquals(0)
        compose.onNode(pipSwitch).assertExists()
        capture("android-pip-row-crossfade-done")
    }

    private val pipSwitch = SemanticsMatcher.expectValue(SemanticsProperties.Role, Role.Switch).and(hasText(TITLE, substring = true))

    private companion object {
        const val TITLE = "Picture in picture when leaving the app" // i18n-skip
    }
}
