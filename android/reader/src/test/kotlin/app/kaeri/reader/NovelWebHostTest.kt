// The novel's web view is let go of in the order the system WebView asks for, and a renderer that
// goes away takes only that view with it: the screen opens the chapter again in a new one.

package app.kaeri.reader

import android.os.Looper
import android.view.View
import android.view.ViewGroup
import android.webkit.RenderProcessGoneDetail
import android.webkit.WebView
import android.widget.FrameLayout
import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import app.kaeri.engine.model.NovelAssets
import app.kaeri.engine.model.ReaderMode
import app.kaeri.reader.phone.NovelPlace
import app.kaeri.reader.phone.NovelScreen
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotSame
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import java.time.Duration

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class NovelWebHostTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val dir = kotlin.io.path.createTempDirectory("novel-host").toFile()

    private val assets = NovelAssets("", "", "")

    private fun gone(view: WebView): Boolean = shadowOf(view).webViewClient.onRenderProcessGone(view, Crashed)

    private object Crashed : RenderProcessGoneDetail() {
        override fun didCrash() = true

        override fun rendererPriorityAtExit() = WebView.RENDERER_PRIORITY_IMPORTANT
    }

    private fun nextFrames() = shadowOf(Looper.getMainLooper()).idleFor(Duration.ofMillis(100))

    @Test
    fun the_view_leaves_the_window_and_its_channel_before_it_is_destroyed() {
        val messages = mutableListOf<NovelMessage>()
        val host = NovelWebHost(compose.activity, assets, { null }) { messages += it }
        val frame = FrameLayout(compose.activity)
        compose.runOnUiThread {
            compose.activity.setContentView(frame)
            frame.addView(host.view)
        }
        compose.runOnUiThread { host.destroy() }
        val shadow = shadowOf(host.view)
        assertNull(host.view.parent)
        assertNull(shadow.getJavascriptInterface(NovelWebHost.HOST))
        assertEquals(NovelWebHost.BLANK, shadow.lastLoadedUrl)
        // Destroyed only on the next frame, once nothing draws it.
        assertFalse(shadow.wasDestroyCalled())
        compose.runOnUiThread { nextFrames() }
        assertTrue(shadow.wasDestroyCalled())
        // Twice is no harm, and nothing reaches the screen after.
        compose.runOnUiThread { host.destroy() }
        assertTrue(messages.isEmpty())
    }

    @Test
    fun a_renderer_that_goes_is_handled_and_said_once() {
        val messages = mutableListOf<NovelMessage>()
        val host = NovelWebHost(compose.activity, assets, { null }) { messages += it }
        compose.runOnUiThread {
            assertTrue(gone(host.view))
            assertTrue(gone(host.view))
        }
        assertEquals(listOf<NovelMessage>(NovelMessage.Gone), messages)
        // A dead view is only destroyed: nothing is loaded into it any more.
        val before = shadowOf(host.view).lastLoadedUrl
        compose.runOnUiThread {
            host.destroy()
            nextFrames()
        }
        assertEquals(before, shadowOf(host.view).lastLoadedUrl)
        assertTrue(shadowOf(host.view).wasDestroyCalled())
    }

    @Test
    fun the_screen_opens_the_chapter_again_in_a_new_view_after_its_renderer_went() {
        val state = TapZonesScenes.state(dir, mode = ReaderMode.PAGED).copy(leaves = ReaderFixtures.leaves(3))
        compose.setContent {
            Box(Modifier.fillMaxSize()) {
                NovelScreen(ReaderSession {}, state, NovelPlace(), assets, resource = { null }, onControls = {})
            }
        }
        compose.waitForIdle()
        val first = webViews().single()
        compose.runOnUiThread { assertTrue(gone(first)) }
        compose.waitForIdle()
        compose.runOnUiThread { nextFrames() }
        val second = webViews().single()
        assertNotSame(first, second)
        assertNull(first.parent)
        assertTrue(shadowOf(first).wasDestroyCalled())
        assertFalse(shadowOf(second).wasDestroyCalled())
    }

    @Test
    fun leaving_the_screen_lets_go_of_its_view() {
        var shown by mutableStateOf(true)
        val state = TapZonesScenes.state(dir, mode = ReaderMode.PAGED).copy(leaves = ReaderFixtures.leaves(3))
        compose.setContent {
            Box(Modifier.fillMaxSize()) {
                if (shown) NovelScreen(ReaderSession {}, state, NovelPlace(), assets, resource = { null }, onControls = {})
            }
        }
        compose.waitForIdle()
        val view = webViews().single()
        shown = false
        compose.waitForIdle()
        compose.runOnUiThread { nextFrames() }
        assertTrue(webViews().isEmpty())
        assertNull(view.parent)
        assertNull(shadowOf(view).getJavascriptInterface(NovelWebHost.HOST))
        assertTrue(shadowOf(view).wasDestroyCalled())
    }

    private fun webViews(): List<WebView> {
        val found = mutableListOf<WebView>()
        fun walk(view: View) {
            if (view is WebView) found += view
            if (view is ViewGroup) for (i in 0 until view.childCount) walk(view.getChildAt(i))
        }
        compose.runOnUiThread { walk(compose.activity.window.decorView) }
        return found
    }
}
