package app.kaeri.store

import android.view.ViewGroup
import android.webkit.CookieManager
import app.kaeri.engine.model.EmbedStyle
import app.kaeri.engine.model.Trailer
import app.kaeri.engine.model.TrailerHost
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config

/**
 * The trailer's web view fills the place it is put in. A WebView whose height is `WRAP_CONTENT`
 * lays its page out zero pixels tall, so the player's frame collapses and only the sound plays;
 * Compose's `AndroidView` gives a view with no layout params of its own exactly that. And what
 * the trailers keep goes with the last one, without taking the HTTP cache each time.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class TrailerPlayerTest {
    /** A holder that adds views the way Compose's `AndroidView` does: with ViewGroup's default params. */
    private class Holder : ViewGroup(RuntimeEnvironment.getApplication()) {
        override fun onLayout(changed: Boolean, l: Int, t: Int, r: Int, b: Int) {}
    }

    @Test
    fun the_view_fills_its_place_in_every_style() {
        for (style in EmbedStyle.entries) {
            val player = TrailerPlayer(
                RuntimeEnvironment.getApplication(),
                Trailer(host = TrailerHost.YOU_TUBE, id = "fixture0001"),
                style,
                subtitles = "",
            ) {}
            Holder().addView(player.view)
            val params = player.view.layoutParams
            assertEquals("$style width", ViewGroup.LayoutParams.MATCH_PARENT, params.width)
            assertEquals("$style height", ViewGroup.LayoutParams.MATCH_PARENT, params.height)
            player.release()
        }
    }

    private fun player(style: EmbedStyle = EmbedStyle.BANNER) = TrailerPlayer(
        RuntimeEnvironment.getApplication(),
        Trailer(host = TrailerHost.YOU_TUBE, id = "fixture0001"),
        style,
        subtitles = "",
    ) {}

    @Test
    fun cookies_go_with_the_last_trailer_and_the_cache_stays() {
        val site = "https://www.youtube-nocookie.com"
        val jar = CookieManager.getInstance()
        // This launch's first trailer has cleared what an earlier run left.
        player().release()
        assertTrue(TrailerData.launched)

        val first = player()
        val second = player()
        jar.setCookie(site, "visitor=fixture")
        first.release()
        assertEquals("one trailer still open keeps them", "visitor=fixture", jar.getCookie(site))
        assertFalse("a release leaves the cache", shadowOf(first.view).wasClearCacheCalled())
        second.release()
        assertNull("the last one takes them", jar.getCookie(site))
        assertFalse(shadowOf(second.view).wasClearCacheCalled())
        assertEquals(0, TrailerData.open)
    }
}
