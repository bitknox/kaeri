// The painter takes the engine's own policy (einkPaint, through JNA on this Mac's engine) and asks
// the fake for each refresh, so the counts are the e-ink board's.

package app.kaeri.eink

import android.view.View
import app.kaeri.engine.ffi.einkPolicyDefaults
import app.kaeri.engine.model.EinkPaint
import app.kaeri.engine.model.Panel
import app.kaeri.engine.model.Refresh
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class EinkPainterTest {
    private val view = View(RuntimeEnvironment.getApplication())

    @Test
    fun the_engine_policy_reaches_the_fake() {
        val fake = FakeRefresh()
        val painter = EinkPainter.defaults(Panel.GREY, fake)
        val text = List(12) { painter.paint(view, EinkPaint.TextPage) }
        assertEquals(listOf(6, 12), text.withIndex().filter { it.value == Refresh.FULL }.map { it.index + 1 })
        assertEquals(12, fake.requests.size)
        fake.clear()
        assertEquals(Refresh.FULL, painter.paint(view, EinkPaint.ImagePage(colour = false)))
        assertEquals(Refresh.FAST, painter.paint(view, EinkPaint.ChromeOpen))
        assertEquals(Refresh.FULL, painter.paint(view, EinkPaint.ChromeClose))
        assertEquals(Refresh.REGION, painter.paint(view, EinkPaint.SheetChange))
        assertEquals(Refresh.FULL, painter.paint(view, EinkPaint.Clean))
        assertEquals(listOf(Refresh.FULL, Refresh.FAST, Refresh.FULL, Refresh.REGION, Refresh.FULL), fake.requests.map { it.refresh })
    }

    @Test
    fun kaleido_lists_go_full_every_third_and_dark_text_every_fourth() {
        assertEquals(3u.toUByte(), einkPolicyDefaults(Panel.KALEIDO3).listEvery)
        val painter = EinkPainter.defaults(Panel.KALEIDO3, FakeRefresh())
        val lists = List(3) { painter.paint(null, EinkPaint.ListPage(covers = true)) }
        assertEquals(listOf(Refresh.PARTIAL, Refresh.PARTIAL, Refresh.FULL), lists)
        painter.policy = painter.policy.copy(dark = true)
        val dark = List(8) { painter.paint(null, EinkPaint.TextPage) }
        assertEquals(2, dark.count { it == Refresh.FULL })
    }

    @Test
    fun gallery_pages_pick_their_colour_mode() {
        val painter = EinkPainter.defaults(Panel.GALLERY3, FakeRefresh())
        assertEquals(Refresh.BLACK_WHITE, painter.paint(null, EinkPaint.ImagePage(colour = false)))
        assertEquals(Refresh.COLOUR_STANDARD, painter.paint(null, EinkPaint.ImagePage(colour = true)))
    }

    @Test
    fun a_new_policy_starts_the_count_again() {
        val painter = EinkPainter.defaults(Panel.GREY, FakeRefresh())
        repeat(5) { painter.paint(null, EinkPaint.TextPage) }
        assertTrue(painter.counter.partials > 0u)
        painter.policy = painter.policy.copy(textEvery = 3u)
        assertEquals(0u, painter.counter.partials)
    }
}
