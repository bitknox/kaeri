// The private mode board's eye-slash: the badge, the line under a title's main action, the switch
// and the player's label all draw it, so colour is never the only sign. Stroked at 1.5 on a 16
// grid, as the board draws it.

package app.kaeri.design

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.addPathNodes
import androidx.compose.ui.unit.dp

object PrivateGlyph {
    /** An eye with a line through it, tinted like any icon. */
    val EyeSlash: ImageVector = ImageVector.Builder(
        name = "EyeSlash", defaultWidth = 24.dp, defaultHeight = 24.dp, viewportWidth = 16f, viewportHeight = 16f,
    ).apply {
        for (path in listOf(
            "M1.6 8 C3.4 4.9 6 3.8 8 3.8 C10 3.8 12.6 4.9 14.4 8 C12.6 11.1 10 12.2 8 12.2 C6 12.2 3.4 11.1 1.6 8 Z",
            "M8 6.1 A1.9 1.9 0 1 1 7.99 6.1 Z",
            "M2.6 2.6 L13.4 13.4",
        )) {
            addPath(
                pathData = addPathNodes(path),
                stroke = SolidColor(Color.Black),
                strokeLineWidth = 1.5f,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
            )
        }
    }.build()
}
