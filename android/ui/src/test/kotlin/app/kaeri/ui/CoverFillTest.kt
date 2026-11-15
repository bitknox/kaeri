package app.kaeri.ui

import androidx.compose.ui.unit.dp
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * How a cover fills a card with no banner (the phone board's direction A): cropped around its
 * upper third while the file enlarges no more than 1.6 times on a card up to 300 dp wide, or 1.33
 * in a wider frame; blurred behind itself past that.
 */
class CoverFillTest {
    /** A frame [width] wide in pixels at [density]. */
    private fun px(width: Float, density: Float) = Math.round(width * density)

    @Test
    fun cards_up_to_300_dp_crop_to_1_6_and_wider_frames_to_1_33() {
        assertEquals(1.6f, CoverFillMode.limit(208.dp), 0f)
        assertEquals(1.6f, CoverFillMode.limit(300.dp), 0f)
        assertEquals(1.33f, CoverFillMode.limit(301.dp), 0f)
        assertEquals(1.33f, CoverFillMode.limit(380.dp), 0f)
    }

    @Test
    fun the_largest_cover_crops_on_a_continue_card_at_every_phone_density() {
        for (density in listOf(2.625f, 3f, 3.5f)) {
            assertEquals("at $density", CoverFillMode.CROP, CoverFillMode.of(208.dp, px(208f, density), 460))
        }
    }

    @Test
    fun a_cover_too_small_to_crop_sharply_blurs() {
        assertEquals(CoverFillMode.BLUR, CoverFillMode.of(208.dp, px(208f, 2.625f), 230))
        // 1.6 times 300 is 480: just inside, then just past.
        assertEquals(CoverFillMode.CROP, CoverFillMode.of(208.dp, 480, 300))
        assertEquals(CoverFillMode.BLUR, CoverFillMode.of(208.dp, 481, 300))
    }

    @Test
    fun the_spotlight_blurs_the_largest_cover() {
        assertEquals(CoverFillMode.BLUR, CoverFillMode.of(380.dp, px(380f, 2.625f), 460))
    }

    @Test
    fun a_cover_of_unknown_size_crops_until_its_file_says() {
        assertEquals(CoverFillMode.CROP, CoverFillMode.of(208.dp, 546, null))
        assertEquals(CoverFillMode.CROP, CoverFillMode.of(208.dp, 546, 0))
    }

    @Test
    fun the_crop_centres_the_upper_third_without_showing_past_either_end() {
        // A 208 wide cover is 312 tall; its focus at 104 sits at the 117 frame's middle, 58.5.
        assertEquals(-45.5f, CoverFillMode.cropTop(208f, 117f), 0.001f)
        // A focus low on the cover stops at the cover's foot.
        assertEquals(117f - 312f, CoverFillMode.cropTop(208f, 117f, focus = 1f), 0.001f)
        // A focus at the top stops at the cover's head.
        assertEquals(0f, CoverFillMode.cropTop(208f, 117f, focus = 0f), 0.001f)
    }
}
