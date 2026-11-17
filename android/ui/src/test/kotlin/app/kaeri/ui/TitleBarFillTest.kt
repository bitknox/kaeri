package app.kaeri.ui

import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** The title page's top bar fills with the scroll, and reverses with it. */
class TitleBarFillTest {
    private fun fill(index: Int, scrolled: Float, instant: Boolean = false) =
        titleBarFill(index, scrolled, start = 48f, span = 64f, snap = 80f, instant = instant)

    /** The bar's fill on a page with this range, scrolled [scrolled] dp, at density 1. */
    private fun TitleBarFillRange.at(scrolled: Float, index: Int = 0, instant: Boolean = false) =
        titleBarFill(index, scrolled, start.value, TitleBarFillSpan.value, snap.value, instant)

    private val phone = TitleBarFillRange.overBanner(PhoneBannerHeight, PhoneBannerParallax)
    private val tablet = TitleBarFillRange.overBanner(TabletBannerHeight)

    /** Where the phone's banner foot stands below the status bar once the page has scrolled [scrolled]. */
    private fun phoneBannerFoot(scrolled: Dp): Dp = PhoneBannerHeight - scrolled * (1f - PhoneBannerParallax)

    @Test
    fun clear_while_the_head_is_below_the_bar() {
        assertEquals(0f, fill(0, 0f), 0f)
        assertEquals(0f, fill(0, 48f), 0f)
    }

    @Test
    fun fills_with_the_scroll_and_reverses_with_it() {
        assertEquals(0.25f, fill(0, 64f), 0.001f)
        assertEquals(0.5f, fill(0, 80f), 0.001f)
        assertEquals(1f, fill(0, 112f), 0f)
        assertEquals(1f, fill(0, 400f), 0f)
        // Back down: the same place, the same fill.
        assertEquals(0.25f, fill(0, 64f), 0.001f)
    }

    @Test
    fun solid_past_the_head() {
        assertEquals(1f, fill(1, 0f), 0f)
        assertEquals(1f, fill(7, 12f), 0f)
    }

    @Test
    fun switches_at_once_at_its_snap_with_animations_removed() {
        assertEquals(0f, fill(0, 64f, instant = true), 0f)
        assertEquals(0f, fill(0, 79f, instant = true), 0f)
        assertEquals(1f, fill(0, 80f, instant = true), 0f)
        assertEquals(1f, fill(3, 0f, instant = true), 0f)
    }

    @Test
    fun the_name_comes_in_over_the_second_half() {
        assertEquals(0f, titleBarNameAlpha(0f), 0f)
        assertEquals(0f, titleBarNameAlpha(0.5f), 0f)
        assertEquals(0.5f, titleBarNameAlpha(0.75f), 0.001f)
        assertEquals(1f, titleBarNameAlpha(1f), 0f)
    }

    @Test
    fun phone_solid_just_as_the_banner_leaves_the_bar() {
        // The banner goes up at half the page's speed: its foot meets the bar's foot after 184 dp.
        assertEquals(184f, phone.solid.value, 0.001f)
        assertEquals(TitleBarHeight.value, phoneBannerFoot(phone.solid).value, 0.001f)
        assertEquals(1f, phone.at(184f), 0f)
        assertEquals(1f, phone.at(400f), 0f)
        // While any of the banner shows below the bar, the bar is not solid.
        assertTrue(phoneBannerFoot(183.dp) > TitleBarHeight)
        assertTrue(phone.at(183f) < 1f)
    }

    @Test
    fun phone_fills_over_the_last_stretch_before_the_banner_leaves() {
        assertEquals(120f, phone.start.value, 0.001f)
        // Clear while the header and most of the banner pass under it.
        assertEquals(0f, phone.at(0f), 0f)
        assertEquals(0f, phone.at(80f), 0f)
        assertEquals(0f, phone.at(120f), 0f)
        assertEquals(0.5f, phone.at(152f), 0.001f)
        // The name only once the banner is nearly gone.
        assertEquals(0f, titleBarNameAlpha(phone.at(152f)), 0.001f)
        assertEquals(1f, titleBarNameAlpha(phone.at(184f)), 0f)
    }

    @Test
    fun tablet_solid_just_as_the_banner_leaves_the_bar() {
        // No parallax: the banner's foot meets the bar's after 208 - 64 dp.
        assertEquals(144f, tablet.solid.value, 0.001f)
        assertEquals(80f, tablet.start.value, 0.001f)
        assertEquals(0f, tablet.at(80f), 0f)
        assertTrue(tablet.at(143f) < 1f)
        assertEquals(1f, tablet.at(144f), 0f)
    }

    @Test
    fun banner_snaps_where_the_banner_is_gone_with_animations_removed() {
        assertEquals(phone.solid, phone.snap)
        assertEquals(0f, phone.at(152f, instant = true), 0f)
        assertEquals(0f, phone.at(183f, instant = true), 0f)
        assertEquals(1f, phone.at(184f, instant = true), 0f)
        assertEquals(tablet.solid, tablet.snap)
        assertEquals(0f, tablet.at(112f, instant = true), 0f)
        assertEquals(0f, tablet.at(143f, instant = true), 0f)
        assertEquals(1f, tablet.at(144f, instant = true), 0f)
    }

    @Test
    fun compact_head_fills_as_its_header_passes_under_the_bar() {
        // No banner: the header 64 dp below the status bar reaches the bar at once.
        val compact = TitleBarFillRange.compact(64.dp)
        assertEquals(0f, compact.start.value, 0f)
        assertEquals(64f, compact.solid.value, 0f)
        assertEquals(0.5f, compact.at(32f), 0.001f)
        assertEquals(1f, compact.at(64f), 0f)
        // With animations removed it switches halfway, as before.
        assertEquals(0f, compact.at(31f, instant = true), 0f)
        assertEquals(1f, compact.at(32f, instant = true), 0f)
    }
}
