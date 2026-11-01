package app.kaeri.store

import android.view.ViewGroup
import app.kaeri.engine.model.EmbedStyle
import app.kaeri.engine.model.Trailer
import app.kaeri.engine.model.TrailerHost
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

/**
 * The trailer's web view fills the place it is put in. A WebView whose height is `WRAP_CONTENT`
 * lays its page out zero pixels tall, so the player's frame collapses and only the sound plays;
 * Compose's `AndroidView` gives a view with no layout params of its own exactly that.
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
}
