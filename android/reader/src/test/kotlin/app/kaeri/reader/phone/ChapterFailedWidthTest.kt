// A chapter that would not open, on a wide tablet: its words and buttons sit in a centred column
// as wide as the reader's sheets, never across the whole screen.

package app.kaeri.reader.phone

import android.content.Context
import androidx.activity.ComponentActivity
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.BottomSheetDefaults
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.unit.dp
import androidx.test.core.app.ApplicationProvider
import app.kaeri.design.ObiSilk
import app.kaeri.engine.model.ReadingFailure
import app.kaeri.i18n.Strings
import app.kaeri.reader.PageImages
import app.kaeri.reader.ReaderFixtures
import app.kaeri.reader.ReaderSession
import app.kaeri.reader.ReaderState
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w1280dp-h800dp-land-xhdpi")
class ChapterFailedWidthTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val context: Context = ApplicationProvider.getApplicationContext()

    @Test
    fun the_buttons_keep_to_a_sheets_width_centred_on_a_wide_screen() {
        val state = ReaderState(failure = ReadingFailure.Offline, openingChapter = ReaderFixtures.chapter())
        compose.setContent {
            Box(Modifier.fillMaxSize().background(ObiSilk.Ground)) {
                PhoneReader(session = ReaderSession {}, state = state, images = PageImages(), actions = PhoneReaderActions(onClose = {}), novel = { _, _ -> })
            }
        }
        compose.waitForIdle()
        val density = compose.activity.resources.displayMetrics.density
        val screen = compose.onRoot().fetchSemanticsNode().boundsInRoot
        val button = compose.onNode(hasClickAction() and hasText(Strings.readerTryAgain(context))).fetchSemanticsNode().boundsInRoot
        val cap = BottomSheetDefaults.SheetMaxWidth.value * density
        assertTrue("Try again is ${button.width} px wide", button.width <= cap)
        // Centred: as much room on its left as on its right.
        val left = button.left - screen.left
        val right = screen.right - button.right
        assertTrue("left $left, right $right", kotlin.math.abs(left - right) <= 2 * density)
        assertTrue(screen.width > cap + 200.dp.value * density)
    }
}
