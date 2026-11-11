// The painter takes the engine's own policy (einkPaint, through JNA on this Mac's engine) and asks
// the fake for each refresh: none out of the box but Clean screen, and the counts a person sets.

package app.kaeri.eink

import android.view.View
import app.kaeri.engine.ffi.einkPolicyDefaults
import app.kaeri.engine.model.EinkMatch
import app.kaeri.engine.model.EinkPaint
import app.kaeri.engine.model.EinkPanel
import app.kaeri.engine.model.EinkProfile
import app.kaeri.engine.model.NovelSupport
import app.kaeri.engine.model.Refresh
import app.kaeri.engine.model.VendorFamily
import org.junit.Assert.assertEquals
import org.junit.Assert.assertSame
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

    /** Every full refresh turned on, as a person may in the Refresh rows. */
    private fun optedIn(panel: EinkPanel) =
        einkPolicyDefaults(panel).copy(imageFull = true, textEvery = 6u, textEveryDark = 4u, listEvery = 3u)

    @Test
    fun out_of_the_box_only_clean_screen_asks_for_a_full_refresh() {
        val fake = FakeRefresh()
        val painter = EinkPainter.defaults(EinkPanel.KALEIDO3, fake)
        val paints = List(30) { painter.paint(view, EinkPaint.TextPage) } +
            List(10) { painter.paint(view, EinkPaint.ImagePage(colour = true)) } +
            List(10) { painter.paint(view, EinkPaint.ListPage(covers = true)) } +
            listOf(painter.paint(view, EinkPaint.ChromeClose), painter.paint(view, EinkPaint.EndPage))
        assertTrue(Refresh.FULL !in paints)
        assertEquals(Refresh.FULL, painter.paint(view, EinkPaint.Clean))
        assertEquals(1, fake.requests.count { it.refresh == Refresh.FULL })
    }

    @Test
    fun the_engine_policy_reaches_the_fake() {
        val fake = FakeRefresh()
        val painter = EinkPainter(optedIn(EinkPanel.GREY), fake)
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
    fun lists_go_full_every_third_and_dark_text_every_fourth_when_asked() {
        assertEquals(0u.toUByte(), einkPolicyDefaults(EinkPanel.KALEIDO3).listEvery)
        val painter = EinkPainter(optedIn(EinkPanel.KALEIDO3), FakeRefresh())
        val lists = List(3) { painter.paint(null, EinkPaint.ListPage(covers = true)) }
        assertEquals(listOf(Refresh.PARTIAL, Refresh.PARTIAL, Refresh.FULL), lists)
        painter.policy = painter.policy.copy(dark = true)
        val dark = List(8) { painter.paint(null, EinkPaint.TextPage) }
        assertEquals(2, dark.count { it == Refresh.FULL })
    }

    @Test
    fun gallery_pages_pick_their_colour_mode() {
        val painter = EinkPainter.defaults(EinkPanel.GALLERY3, FakeRefresh())
        assertEquals(Refresh.BLACK_WHITE, painter.paint(null, EinkPaint.ImagePage(colour = false)))
        assertEquals(Refresh.COLOUR_STANDARD, painter.paint(null, EinkPaint.ImagePage(colour = true)))
    }

    @Test
    fun a_new_policy_starts_the_count_again() {
        val painter = EinkPainter.defaults(EinkPanel.GREY, FakeRefresh())
        repeat(5) { painter.paint(null, EinkPaint.TextPage) }
        assertTrue(painter.counter.partials > 0u)
        painter.policy = painter.policy.copy(textEvery = 3u)
        assertEquals(0u, painter.counter.partials)
    }

    @Test
    fun a_profile_brings_its_vendors_refresh_and_the_persons_settings() {
        val asked = mutableListOf<VendorFamily>()
        val fake = FakeRefresh()
        val settings = einkPolicyDefaults(EinkPanel.GREY).copy(imageFull = false, textEvery = 2u)
        val profile = EinkProfile(
            eink = EinkMatch.SURE, panel = EinkPanel.GREY, ppi = 300u, family = VendorFamily.ONYX, refreshHook = true,
            pageButtons = false, colourSure = true, novels = NovelSupport.FULL, policy = settings,
        )
        val painter = EinkPainter.forProfile(profile) { asked += it; fake }
        assertEquals(listOf(VendorFamily.ONYX), asked)
        assertEquals(settings, painter.policy)
        // The person's settings: no full on an image page, a full every second text page, and
        // each full reaches the vendor's call.
        assertEquals(Refresh.PARTIAL, painter.paint(view, EinkPaint.ImagePage(colour = false)))
        val text = List(4) { painter.paint(view, EinkPaint.TextPage) }
        assertEquals(2, text.count { it == Refresh.FULL })
        assertEquals(2, fake.requests.count { it.refresh == Refresh.FULL })

        // Before the engine sends the settings, the panel's defaults; with no profile, no vendor.
        val early = EinkPainter.forProfile(profile.copy(policy = null)) { fake }
        assertEquals(einkPolicyDefaults(EinkPanel.GREY), early.policy)
        assertEquals(Refresh.PARTIAL, early.paint(view, EinkPaint.ImagePage(colour = false)))
        assertSame(NoRefresh, EinkPainter.forProfile(null) { fake }.controller)
    }
}
