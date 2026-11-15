// The e-ink reader names a chapter that would not open as the phone does: by the number the
// screen that asked listed, or by words without a number, never "Couldn't open chapter ".

package app.kaeri.eink

import android.content.ComponentName
import android.content.Context
import androidx.activity.ComponentActivity
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.test.core.app.ApplicationProvider
import app.kaeri.engine.model.ChapterReadState
import app.kaeri.engine.model.ChapterRow
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.ReadingAbout
import app.kaeri.engine.model.ReadingEvent
import app.kaeri.engine.model.ReadingFailure
import app.kaeri.i18n.Strings
import app.kaeri.reader.PageImages
import app.kaeri.reader.ReaderSession
import org.junit.Rule
import org.junit.Test
import org.junit.rules.ExternalResource
import org.junit.rules.RuleChain
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w571dp-h772dp-port-300dpi")
class EinkChapterFailedTitleTest {
    private val compose = createAndroidComposeRule<ComponentActivity>()

    @get:Rule
    val rules: RuleChain = RuleChain.outerRule(
        object : ExternalResource() {
            override fun before() {
                val app = RuntimeEnvironment.getApplication()
                shadowOf(app.packageManager).addActivityIfNotPresent(ComponentName(app, ComponentActivity::class.java))
            }
        },
    ).around(compose)

    private val context: Context = ApplicationProvider.getApplicationContext()
    private val session = ReaderSession { }

    private fun failedOpen(row: ChapterRow?) {
        session.open(EinkFixtures.WORK, EinkFixtures.surface.copy(eink = true), "local:lantern-stair/22", row = row)
        session.apply(Event.Reading(ReadingEvent.Failed(ReadingAbout.READER, ReadingFailure.Offline)))
        val painter = EinkPainter.defaults(app.kaeri.engine.model.EinkPanel.GREY, FakeRefresh())
        compose.setContent {
            EinkTheme {
                val state = session.state.collectAsState().value
                EinkReader(session, state, PageImages(), painter, EinkReaderActions({}, {}, {}), showZones = false)
            }
        }
        compose.waitForIdle()
    }

    @Test
    fun a_listed_chapter_is_named() {
        failedOpen(ChapterRow(id = "local:lantern-stair/22", label = "Chapter 22", state = ChapterReadState.Unread, number = "22"))
        compose.onNodeWithText(Strings.readerFailedChapter(context, "22")).assertExists()
    }

    @Test
    fun a_chapter_never_named_gets_words_without_a_number() {
        failedOpen(null)
        compose.onNodeWithText(Strings.readerFailedChapterPlain(context)).assertExists()
    }
}
