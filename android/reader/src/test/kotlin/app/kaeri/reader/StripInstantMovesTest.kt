// A strip moved at once still says where it is: with animations removed a key's step is one
// scrollBy, and Home and End always jump. Each starts and ends its scroll in one task, so the
// strip's reporters, which wait for a scroll to come to rest, never saw it, and the engine kept
// the place where the last drag ended.

package app.kaeri.reader

import android.provider.Settings
import androidx.activity.ComponentActivity
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.test.isFocused
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.pressKey
import androidx.test.core.app.ApplicationProvider
import app.kaeri.design.ObiSilk
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.Leaf
import app.kaeri.engine.model.ReaderCommand
import app.kaeri.engine.model.ReaderEvent
import app.kaeri.engine.model.ReaderMode
import app.kaeri.reader.ReaderFixtures.CHAPTER
import app.kaeri.reader.ReaderFixtures.SESSION
import app.kaeri.reader.ReaderFixtures.WORK
import app.kaeri.reader.phone.PhoneReader
import app.kaeri.reader.phone.PhoneReaderActions
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class StripInstantMovesTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val sent = mutableListOf<ReaderCommand>()
    private val session = ReaderSession { sent += (it as Command.Reader).v1 }
    private val list = LazyListState()
    private val images = 8
    private val resolver get() = ApplicationProvider.getApplicationContext<android.content.Context>().contentResolver

    @Before
    fun removeAnimations() {
        Settings.Global.putFloat(resolver, Settings.Global.ANIMATOR_DURATION_SCALE, 0f)
    }

    @After
    fun restoreAnimations() {
        Settings.Global.putFloat(resolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f)
    }

    private fun strip() {
        val leaves = List(images) { Leaf(it.toUInt(), listOf(ReaderFixtures.part(it.toUInt()).copy(ratio = 0.7f)), false) } + Leaf(images.toUInt(), emptyList(), true)
        session.open(WORK, ReaderFixtures.surface)
        session.apply(ReaderEvent.Opened(ReaderFixtures.view(mode = ReaderMode.STRIP)))
        session.apply(ReaderEvent.Leaves(SESSION, CHAPTER, leaves))
        compose.setContent {
            val state by session.state.collectAsState()
            Box(Modifier.fillMaxSize().background(ObiSilk.Ground)) {
                PhoneReader(session = session, state = state, images = PageImages(), actions = PhoneReaderActions(onClose = {}), novel = { _, _ -> }, stripList = list)
            }
        }
        compose.waitForIdle()
    }

    private fun key(key: Key) {
        compose.onNode(isFocused()).performKeyInput { pressKey(key) }
        compose.waitForIdle()
    }

    private fun lastAt(): ReaderCommand.StripAt? = sent.filterIsInstance<ReaderCommand.StripAt>().lastOrNull()

    @Test
    fun a_page_key_step_without_animations_says_the_new_place() {
        strip()
        sent.clear()
        key(Key.PageDown)
        key(Key.PageDown)
        val top = compose.runOnIdle { list.firstVisibleItemIndex }
        assertTrue("two screens down moved past the first image", top > 0)
        val at = lastAt()
        assertNotNull("a StripAt after the instant steps", at)
        assertEquals(top.toUInt(), at?.at?.image)
    }

    @Test
    fun end_and_home_say_the_new_place() {
        strip()
        sent.clear()
        key(Key.MoveEnd)
        val top = compose.runOnIdle { list.firstVisibleItemIndex }
        assertTrue("End moved down the strip", top > 0)
        assertEquals(top.toUInt(), lastAt()?.at?.image)
        sent.clear()
        key(Key.MoveHome)
        assertEquals(0u, lastAt()?.at?.image)
    }
}
