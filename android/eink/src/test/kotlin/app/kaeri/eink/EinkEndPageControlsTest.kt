// The e-ink end page with the bars brought back over it: Continue and Back to the series stay
// above the bottom bar, on a small panel and on a 10 inch one.

package app.kaeri.eink

import android.content.ComponentName
import android.content.Context
import androidx.activity.ComponentActivity
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.pressKey
import androidx.test.core.app.ApplicationProvider
import app.kaeri.i18n.Strings
import app.kaeri.engine.model.ReaderEvent
import app.kaeri.reader.PageImages
import app.kaeri.reader.ReaderSession
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.ExternalResource
import org.junit.rules.RuleChain
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
class EinkEndPageControlsTest {
    private val compose = createAndroidComposeRule<ComponentActivity>()
    private val dir = TemporaryFolder()

    @get:Rule
    val rules: RuleChain = RuleChain.outerRule(dir).around(
        object : ExternalResource() {
            override fun before() {
                val app = RuntimeEnvironment.getApplication()
                shadowOf(app.packageManager).addActivityIfNotPresent(ComponentName(app, ComponentActivity::class.java))
            }
        },
    ).around(compose)

    private val context: Context = ApplicationProvider.getApplicationContext()
    private val session = ReaderSession { }

    private fun endPageWithBars() {
        session.open(EinkFixtures.WORK, EinkFixtures.surface.copy(eink = true))
        session.apply(ReaderEvent.Opened(EinkFixtures.view(start = 0u)))
        session.apply(ReaderEvent.Leaves(EinkFixtures.SESSION, EinkFixtures.CHAPTER, EinkFixtures.leaves(3) { EinkFixtures.grey(dir.root, "$it.gray", 80, 120) }))
        val painter = EinkPainter.defaults(app.kaeri.engine.model.EinkPanel.GREY, FakeRefresh())
        compose.setContent {
            EinkTheme {
                val state = session.state.collectAsState().value
                EinkReader(session, state, PageImages(), painter, EinkReaderActions({}, {}, {}), showZones = false)
            }
        }
        compose.waitForIdle()
        repeat(3) {
            compose.onRoot().performKeyInput { pressKey(Key.VolumeDown) }
            compose.waitForIdle()
        }
        val series = hasClickAction() and hasText(Strings.readerEndSeries(context))
        assertTrue("on the end page", compose.onAllNodes(series).fetchSemanticsNodes().isNotEmpty())

        compose.onRoot().performKeyInput { pressKey(Key.Menu) }
        compose.waitForIdle()
        val density = compose.activity.resources.displayMetrics.density
        val root = compose.onRoot().fetchSemanticsNode().boundsInRoot
        // The bottom bar is at least this tall, from the foot.
        val barTop = root.bottom - EinkSizes.BottomBar.value * density
        val buttons = compose.onAllNodes(hasClickAction() and (series or hasText("Continue", substring = true)))
            .fetchSemanticsNodes().map { it.boundsInRoot }
        assertTrue(buttons.isNotEmpty())
        for (button in buttons) assertTrue("a button ends at ${button.bottom}, the bar starts by $barTop", button.bottom <= barTop)
    }

    @Test
    @Config(sdk = [36], qualifiers = "w571dp-h772dp-port-300dpi")
    fun on_a_small_panel_the_bars_leave_the_buttons_clear() = endPageWithBars()

    @Test
    @Config(sdk = [36], qualifiers = "w992dp-h1323dp-port-300dpi")
    fun on_a_ten_inch_panel_the_bars_leave_the_buttons_clear() = endPageWithBars()
}
