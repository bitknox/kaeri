// The four reactions, drawn once for every platform: laugh, gasp, clap and cry as monochrome
// glyphs on a 24 grid, stroked at 1.6 as the player's controls are, and the chat and send marks
// beside them. Never emoji.

package app.kaeri.design

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.addPathNodes
import androidx.compose.ui.graphics.vector.group
import androidx.compose.ui.unit.dp

object ChatGlyphs {
    private const val STROKE = 1.6f

    private fun glyph(name: String, build: ImageVector.Builder.() -> Unit): ImageVector =
        ImageVector.Builder(name = name, defaultWidth = 24.dp, defaultHeight = 24.dp, viewportWidth = 24f, viewportHeight = 24f)
            .apply(build).build()

    private fun ImageVector.Builder.line(data: String) {
        addPath(
            pathData = addPathNodes(data),
            stroke = SolidColor(Color.Black),
            strokeLineWidth = STROKE,
            strokeLineCap = StrokeCap.Round,
            strokeLineJoin = StrokeJoin.Round,
        )
    }

    private fun ImageVector.Builder.solid(data: String) {
        addPath(pathData = addPathNodes(data), fill = SolidColor(Color.Black))
    }

    private const val FACE = "M21 12 A9 9 0 1 1 3 12 A9 9 0 1 1 21 12 Z"

    val Laugh: ImageVector = glyph("Laugh") {
        line(FACE)
        line("M8 10.3 Q9.5 8.4 11 10.3 M13 10.3 Q14.5 8.4 16 10.3")
        line("M7.6 13.4 H16.4 A4.4 4.4 0 0 1 7.6 13.4 Z")
    }

    val Gasp: ImageVector = glyph("Gasp") {
        line(FACE)
        solid("M10 10 A1 1 0 1 1 8 10 A1 1 0 1 1 10 10 Z")
        solid("M16 10 A1 1 0 1 1 14 10 A1 1 0 1 1 16 10 Z")
        line("M14.2 15.3 A2.2 2.2 0 1 1 9.8 15.3 A2.2 2.2 0 1 1 14.2 15.3 Z")
    }

    val Clap: ImageVector = glyph("Clap") {
        group(rotate = -16f, pivotX = 8.5f, pivotY = 14.6f) {
            line("M8.5 8.6 A2.9 2.9 0 0 1 11.4 11.5 V17.7 A2.9 2.9 0 0 1 5.6 17.7 V11.5 A2.9 2.9 0 0 1 8.5 8.6 Z")
        }
        group(rotate = 16f, pivotX = 15.5f, pivotY = 14.6f) {
            line("M15.5 8.6 A2.9 2.9 0 0 1 18.4 11.5 V17.7 A2.9 2.9 0 0 1 12.6 17.7 V11.5 A2.9 2.9 0 0 1 15.5 8.6 Z")
        }
        line("M12 2.6 V4.9 M7.4 3.8 L8.6 5.7 M16.6 3.8 L15.4 5.7")
    }

    val Cry: ImageVector = glyph("Cry") {
        line(FACE)
        line("M8 10 H10.5 M13.5 10 H16")
        line("M9 16.6 Q12 14.3 15 16.6")
        solid("M16.2 11.7 C16.2 11.7 15 13.5 15 14.3 A1.2 1.2 0 0 0 17.4 14.3 C17.4 13.5 16.2 11.7 16.2 11.7 Z")
    }

    /** A speech bubble: the Chat action. */
    val Chat: ImageVector = glyph("Chat") {
        line("M4 5 H20 A2 2 0 0 1 22 7 V15 A2 2 0 0 1 20 17 H10 L5.5 21 V17 H4 A2 2 0 0 1 2 15 V7 A2 2 0 0 1 4 5 Z")
    }

    /** Send: the paper arrow, filled. */
    val Send: ImageVector = glyph("Send") {
        solid("M3.4 20.4 L21 12 L3.4 3.6 L3.4 10.2 L15.5 12 L3.4 13.8 Z")
    }
}
