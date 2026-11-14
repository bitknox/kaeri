// Fit on a portrait phone: Screen fits the whole page, Height fills the height of a page that
// leaves bands and pans along its width, Width is Screen's own for such a page, and a pan stays on
// the page.

package app.kaeri.reader

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import app.kaeri.engine.model.Leaf
import app.kaeri.engine.model.PageEncoding
import app.kaeri.engine.model.PageFile
import org.junit.Assert.assertEquals
import org.junit.Test

class PagedFitTest {
    private val file = PageFile("/pages/1.png", PageEncoding.Original("image/png"), 1400u, 2000u, null, false, 1uL)
    private val leaf = Leaf(0u, listOf(ReaderFixtures.part(0u, file)), false)
    private val state = ReaderState(leaves = listOf(leaf))

    @Test
    fun height_fills_the_height_and_width_is_screen_for_a_portrait_page() {
        // 1080 by 2400: the page is 1080 by about 1543 fitted, with bands above and below.
        val screen = fitOf(leaf, state, "screen", 1080f, 2400f)
        assertEquals(1f, screen.scale, 0.001f)
        val height = fitOf(leaf, state, "height", 1080f, 2400f)
        assertEquals(2400f / (2000f * 1080f / 1400f), height.scale, 0.001f)
        assertEquals(1f, fitOf(leaf, state, "width", 1080f, 2400f).scale, 0.001f)
        // In landscape the same page leaves bands at the sides: Width fills them.
        assertEquals(2400f / (1400f * 1080f / 2000f), fitOf(leaf, state, "width", 2400f, 1080f).scale, 0.001f)
    }

    @Test
    fun a_fitted_page_pans_only_along_the_side_past_the_screen() {
        val fit = fitOf(leaf, state, "height", 1080f, 2400f)
        val far = clampPan(Offset(10_000f, 10_000f), fit.scale, 1080f, 2400f, fit.content)
        assertEquals((fit.content.width * fit.scale - 1080f) / 2, far.x, 0.5f)
        assertEquals(0f, far.y, 0.001f)
        // With the whole screen as the content, a zoom pans as before.
        assertEquals(Offset(540f, 1200f), clampPan(Offset(10_000f, 10_000f), 2f, 1080f, 2400f, Size(1080f, 2400f)))
    }
}
