// A book that is a file on this device, as a picked folder's books are, offers no Download in the
// 10 inch reader's top bar; a chapter from a source still does.

package app.kaeri.eink

import android.content.ComponentName
import android.content.Context
import androidx.activity.ComponentActivity
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.test.click
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performTouchInput
import androidx.test.core.app.ApplicationProvider
import app.kaeri.engine.model.ReaderEvent
import app.kaeri.engine.model.ReaderView
import app.kaeri.engine.model.VolumeGroup
import app.kaeri.i18n.Strings
import app.kaeri.reader.PageImages
import app.kaeri.reader.ReaderSession
import org.junit.Assert.assertFalse
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
@Config(sdk = [36], qualifiers = "w992dp-h1323dp-port-300dpi")
class EinkOnDeviceDownloadTest {
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
    private val painter = EinkPainter.defaults(app.kaeri.engine.model.EinkPanel.GREY, FakeRefresh())

    /** Opens [view] with its pages, the chapter list after it when [listed] is given, and shows the bars. */
    private fun chrome(view: ReaderView, listed: VolumeGroup? = null): Boolean {
        session.open(EinkFixtures.WORK, EinkFixtures.surface.copy(eink = true))
        session.apply(ReaderEvent.Opened(view))
        session.apply(ReaderEvent.Leaves(EinkFixtures.SESSION, EinkFixtures.CHAPTER, EinkFixtures.leaves(4) { EinkFixtures.grey(dir.root, "$it.gray", 70, 100) }))
        if (listed != null) {
            session.apply(
                app.kaeri.engine.model.Event.Reading(app.kaeri.engine.model.ReadingEvent.Chapters(EinkFixtures.WORK, null, listOf(listed), null)),
            )
        }
        compose.setContent {
            EinkTheme {
                val state = session.state.collectAsState().value
                EinkReader(session, state, PageImages(), painter, EinkReaderActions({}, {}, {}, onDownload = {}), showZones = false)
            }
        }
        compose.waitForIdle()
        compose.onRoot().performTouchInput { click(androidx.compose.ui.geometry.Offset(width * 0.55f, height * 0.5f)) }
        compose.waitForIdle()
        return compose.onAllNodesWithText(Strings.einkDownload(context)).fetchSemanticsNodes().isNotEmpty()
    }

    @Test
    fun a_chapter_from_a_source_offers_download() {
        assertTrue(chrome(EinkFixtures.view(start = 0u)))
    }

    @Test
    fun a_book_the_engine_marks_on_the_device_offers_none() {
        val view = EinkFixtures.view(start = 0u)
        assertFalse(chrome(view.copy(chapter = view.chapter.copy(onDevice = true))))
    }

    @Test
    fun a_book_the_chapter_list_marks_on_the_device_offers_none() {
        val view = EinkFixtures.view(start = 0u)
        assertFalse(chrome(view, VolumeGroup(null, null, listOf(view.chapter.copy(onDevice = true)), false)))
    }
}
