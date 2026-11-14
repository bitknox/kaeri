// The e-ink tile diagram keeps to the panel's inks: every pixel a grey, and the Previous chevron
// solid black, not a navy tint at part strength that dithers and shows through a colour filter.

package app.kaeri.reader

import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.size
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.toPixelMap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.unit.dp
import app.kaeri.engine.model.ReadDirection
import app.kaeri.engine.model.ReaderMode
import app.kaeri.engine.model.TapAction
import app.kaeri.engine.model.TapReading
import app.kaeri.engine.model.ZoneLayout
import app.kaeri.engine.model.tapMap
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class TapZoneDiagramTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    @Test
    fun the_eink_diagram_draws_its_chevrons_in_solid_ink() {
        val zones = TapZonesScenes.zones(ZoneLayout.TOP_BOTTOM).copy(eink = true)
        val map = tapMap(zones, TapReading(ReaderMode.PAGED, ReadDirection.LEFT_TO_RIGHT, false, false), null)
        compose.setContent { TapZoneDiagram(map, Modifier.size(60.dp, 84.dp).testTag("diagram"), eink = true) }
        val pixels = compose.onNodeWithTag("diagram").captureToImage().toPixelMap()

        var tinted = 0
        for (y in 0 until pixels.height) for (x in 0 until pixels.width) {
            val c = pixels[x, y]
            if (kotlin.math.abs(c.red - c.green) > 0.02f || kotlin.math.abs(c.green - c.blue) > 0.02f) tinted++
        }
        assertTrue("$tinted pixels are not grey", tinted == 0)

        // Inside the labelled Previous cell, away from its edges, the chevron is the darkest ink.
        val previous = map.cells.first { it.action == TapAction.PREVIOUS && it.labelled }
        var darkest = 1f
        val x0 = ((previous.x + previous.width * 0.15) * pixels.width).toInt()
        val x1 = ((previous.x + previous.width * 0.85) * pixels.width).toInt()
        val y0 = ((previous.y + previous.height * 0.15) * pixels.height).toInt()
        val y1 = ((previous.y + previous.height * 0.85) * pixels.height).toInt()
        for (y in y0 until y1) for (x in x0 until x1) darkest = minOf(darkest, pixels[x, y].red)
        assertTrue("the Previous chevron is at $darkest, not black", darkest < 0.06f)
    }
}
