// A strip that starts on a credit page: its line, and its caption once shown, sit below the status
// bar, which the controls bring over the top of the strip, so the bar's clock and icons never
// cover Show or Not a credit page.

package app.kaeri.reader

import android.view.View
import androidx.activity.ComponentActivity
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.core.graphics.Insets
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import app.kaeri.design.ObiSilk
import app.kaeri.engine.model.CreditPage
import app.kaeri.engine.model.Msg
import app.kaeri.engine.model.ReaderEvent
import app.kaeri.engine.model.ReaderMode
import app.kaeri.engine.model.ReadingMsg
import app.kaeri.i18n.Strings
import app.kaeri.reader.ReaderFixtures.CHAPTER
import app.kaeri.reader.ReaderFixtures.SESSION
import app.kaeri.reader.ReaderFixtures.WORK
import app.kaeri.reader.phone.PhoneReader
import app.kaeri.reader.phone.PhoneReaderActions
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class StripCreditsTopTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    @get:Rule
    val dir = TemporaryFolder()

    private val session = ReaderSession {}
    private val images = PageImages()
    private lateinit var view: View

    /** The status bar's height in px, as a phone's is at 420 dpi. */
    private val bar = 63

    /** A strip of 6 images with a credit page before the first, read from the top. */
    private fun opened(): CreditPage {
        session.open(WORK, ReaderFixtures.surface)
        session.apply(ReaderEvent.Opened(ReaderFixtures.view(start = 0u, mode = ReaderMode.STRIP)))
        val story = (0u until 6u).associateWith { ReaderFixtures.grey(dir.root, "p$it", 60, 120) }
        session.apply(ReaderEvent.Leaves(SESSION, CHAPTER, ReaderFixtures.leaves(6) { story[it] }))
        story.values.forEach(images::load)
        val credit = CreditPage(0u, false, ReaderFixtures.grey(dir.root, "credit", 60, 90), 11uL)
        images.load(credit.file)
        session.apply(ReaderEvent.CreditPages(SESSION, CHAPTER, listOf(credit), Msg.Reading(ReadingMsg.ReaderCreditPagesSkipped(1))))
        compose.setContent {
            view = LocalView.current
            Box(Modifier.fillMaxSize().background(ObiSilk.Ground)) {
                val state = session.state.collectAsState().value
                PhoneReader(session = session, state = state, images = images, actions = PhoneReaderActions(onClose = {}), novel = { _, _ -> })
            }
        }
        settle()
        return credit
    }

    /** The status bar shown over the top of the window, as the controls bring it. */
    private fun statusBar() {
        val top = Insets.of(0, bar, 0, 0)
        val insets = WindowInsetsCompat.Builder()
            .setInsets(WindowInsetsCompat.Type.statusBars(), top)
            .setInsetsIgnoringVisibility(WindowInsetsCompat.Type.statusBars(), top)
            .setVisible(WindowInsetsCompat.Type.statusBars(), true)
            .build()
        compose.runOnUiThread { ViewCompat.dispatchApplyWindowInsets(view, insets) }
        settle()
    }

    private fun settle() {
        compose.mainClock.advanceTimeBy(1_200)
        compose.waitForIdle()
    }

    @Test
    fun the_line_and_the_caption_at_the_top_of_a_strip_sit_below_the_status_bar() {
        val credit = opened()
        val context = compose.activity
        statusBar()
        val line = compose.onNodeWithContentDescription(Strings.readerCreditPagesSkipped(context, 1)).fetchSemanticsNode().boundsInRoot
        assertTrue("the line starts at ${line.top}, under a bar $bar px tall", line.top >= bar)

        session.reveal(listOf(credit), CHAPTER)
        settle()
        statusBar()
        val caption = compose.onNodeWithText(Strings.readerCreditShown(context), useUnmergedTree = true).fetchSemanticsNode().boundsInRoot
        assertTrue("the caption starts at ${caption.top}, under a bar $bar px tall", caption.top >= bar)
    }
}
