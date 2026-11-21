// The player panel's few glyphs, shared by the phone and the TV: the check on what plays, the
// chevron on a source's releases, and the warning on a switch that failed. Stroked at 2 on 24,
// with round caps and joins, as the app's icons are.

package app.kaeri.design

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.addPathNodes
import androidx.compose.ui.unit.dp

object PanelGlyphs {
    private fun stroked(name: String, vararg paths: String) = ImageVector.Builder(
        name = name, defaultWidth = 24.dp, defaultHeight = 24.dp, viewportWidth = 24f, viewportHeight = 24f,
    ).apply {
        for (path in paths) {
            addPath(
                pathData = addPathNodes(path),
                stroke = SolidColor(Color.Black),
                strokeLineWidth = 2f,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
            )
        }
    }.build()

    val Check = stroked("Check", "M5 12.5 L10 17.5 L19 7")
    val Chevron = stroked("Chevron", "M9 6 L15 12 L9 18")
    val Warning = stroked("Warning", "M10.3 4.4 A2 2 0 0 1 13.7 4.4 L21 17.5 A2 2 0 0 1 19.3 20.5 H4.7 A2 2 0 0 1 3 17.5 Z", "M12 9.5 V13.5 M12 16.8 V16.81")
}
