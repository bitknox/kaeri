package app.kaeri.ui

import androidx.activity.ComponentActivity
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.performClick
import app.kaeri.design.KaeriTheme
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.SyncCommand
import app.kaeri.engine.model.SyncFolderKind
import app.kaeri.engine.model.SyncFolderSpec
import app.kaeri.engine.model.SyncProblem
import app.kaeri.store.AppStore
import app.kaeri.store.SyncSheet
import app.kaeri.testkit.captureScreenRoboImage
import com.github.takahirom.roborazzi.roborazziSystemPropertyOutputDirectory
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * The restore sheet while a start over waits for its first step to show: from the confirmed tap,
 * and from Try again on its stop, nothing on the sheet sends a second start over or closes it.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class StartOverSheetTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val store = AppStore(CoroutineScope(SupervisorJob() + Dispatchers.Main))
    private val drive = SyncFolderSpec(SyncFolderKind.GOOGLE_DRIVE, null, null, null)

    private fun startsOver() = store.waiting.filterIsInstance<Command.SyncGroup>().count { it.v1 is SyncCommand.ClearFolderAndStart }

    private fun button(text: String) = compose.onNode(hasText(text) and hasClickAction(), useUnmergedTree = false)

    private fun openRestoreSheet() {
        store.sync.askKey(drive)
        compose.setContent {
            KaeriTheme {
                CompositionLocalProvider(LocalAppStore provides store) { EnterKeySheet() }
            }
        }
        compose.waitForIdle()
    }

    @Test
    fun the_form_holds_still_from_the_confirmed_tap() {
        openRestoreSheet()
        button("Start over without the key").performClick()
        compose.waitForIdle()
        // The 600 ms before the deleting step shows: the form stays on screen, dimmed.
        compose.mainClock.autoAdvance = false
        button("Delete and start over").performClick()
        compose.mainClock.advanceTimeBy(100)
        assertEquals(1, startsOver())
        button("Start over without the key").assertIsNotEnabled()
        button("Cancel").assertIsNotEnabled()
        button("Restore").assertIsNotEnabled()
        captureScreenRoboImage("${roborazziSystemPropertyOutputDirectory()}/phone-start-over-2b-held.png")
        button("Start over without the key").performClick()
        button("Cancel").performClick()
        compose.mainClock.advanceTimeBy(100)
        assertEquals(1, startsOver())
        assertEquals(SyncSheet.KEY, store.sync.sheet.value)
    }

    @Test
    fun the_stop_holds_still_after_try_again() {
        openRestoreSheet()
        button("Start over without the key").performClick()
        compose.waitForIdle()
        button("Delete and start over").performClick()
        compose.waitForIdle()
        store.apply(Event.SyncProblem(SyncProblem.ClearRefused))
        compose.waitForIdle()
        button("Try again").assertIsEnabled()
        // The stop's 400 ms hold after Try again: Cancel and Try again are there, dimmed.
        compose.mainClock.autoAdvance = false
        button("Try again").performClick()
        compose.mainClock.advanceTimeBy(100)
        assertEquals(2, startsOver())
        button("Try again").assertIsNotEnabled()
        button("Cancel").assertIsNotEnabled()
        captureScreenRoboImage("${roborazziSystemPropertyOutputDirectory()}/phone-start-over-5c-held.png")
        button("Try again").performClick()
        button("Cancel").performClick()
        compose.mainClock.advanceTimeBy(100)
        assertEquals(2, startsOver())
        assertEquals(SyncFolderKind.GOOGLE_DRIVE, store.sync.startOver.value)
    }
}
