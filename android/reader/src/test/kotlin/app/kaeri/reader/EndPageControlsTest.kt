// The end page's buttons and the reader's controls: with the controls brought back over the end page
// by a tap, Continue and Back to the series sit above their group, on a phone and on a tablet, and
// go back to the foot once the controls are away.

package app.kaeri.reader

import android.content.Context
import androidx.activity.ComponentActivity
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.click
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performTouchInput
import androidx.test.core.app.ApplicationProvider
import app.kaeri.design.ObiSilk
import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.ReaderMode
import app.kaeri.i18n.Strings
import app.kaeri.reader.phone.NovelPlace
import app.kaeri.reader.phone.PhoneReader
import app.kaeri.reader.phone.PhoneReaderActions
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
class EndPageControlsTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val context: Context = ApplicationProvider.getApplicationContext()
    private val session = ReaderSession {}
    private val dir = kotlin.io.path.createTempDirectory("end-controls").toFile()

    private fun novel() = TapZonesScenes.state(dir, mode = ReaderMode.PAGED).let { state ->
        state.copy(view = state.view?.copy(medium = Medium.NOVEL), leaves = emptyList())
    }

    private fun bounds(matcher: SemanticsMatcher): List<Rect> =
        compose.onAllNodes(matcher, useUnmergedTree = false).fetchSemanticsNodes().map { it.boundsInRoot }

    /** The end page's buttons, which the page always has one or both of. */
    private fun buttons(): List<Rect> = bounds(
        hasClickAction() and (hasText(Strings.readerEndContinue(context)) or hasText(Strings.readerEndSeries(context))),
    )

    /** Where the controls' group starts: its scrubber or its Chapters button, whichever is higher. */
    private fun groupTop(): Float = (
        bounds(hasContentDescription(Strings.readerChapters(context))) +
            bounds(SemanticsMatcher.keyIsDefined(SemanticsActions.SetProgress))
        ).minOf { it.top }

    private fun endPageWithControls() {
        var shown: NovelPlace? = null
        compose.setContent {
            Box(Modifier.fillMaxSize().background(ObiSilk.Ground)) {
                PhoneReader(
                    session = session,
                    state = novel(),
                    images = PageImages(),
                    actions = PhoneReaderActions(onClose = {}),
                    novel = { place, _ -> shown = place },
                )
            }
        }
        compose.waitForIdle()
        compose.runOnIdle { shown!!.ended = true }
        compose.waitForIdle()
        val atFoot = buttons()
        assertTrue("the end page shows its buttons", atFoot.isNotEmpty())

        // A tap in the middle brings the controls back over the end page.
        compose.onRoot().performTouchInput { click(center) }
        compose.mainClock.advanceTimeBy(1_000)
        compose.waitForIdle()
        val top = groupTop()
        for (button in buttons()) assertTrue("a button ends at ${button.bottom}, the controls start at $top", button.bottom <= top)

        // Another hides them, and the buttons are back where they were.
        compose.onRoot().performTouchInput { click(center) }
        compose.mainClock.advanceTimeBy(1_000)
        compose.waitForIdle()
        val back = buttons()
        assertTrue("back at the foot: $back, first $atFoot", back.map { it.bottom } == atFoot.map { it.bottom })
    }

    @Test
    @Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
    fun on_a_phone_the_shown_controls_leave_the_buttons_clear() = endPageWithControls()

    @Test
    @Config(sdk = [36], qualifiers = "w1280dp-h800dp-land-xhdpi")
    fun on_a_tablet_the_shown_controls_leave_the_buttons_clear() = endPageWithControls()
}
