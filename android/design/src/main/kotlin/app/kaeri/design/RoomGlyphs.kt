// The room's glyphs, from the approved board's own paths: Invite, Leave, Details, Copy, Share,
// full screen in and out, the fold-down chevron, and the filled pause for the badge. Stroked at 2
// on 24 with round caps and joins, as the app's icons are; the pause is filled.

package app.kaeri.design

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.addPathNodes
import androidx.compose.ui.unit.dp

object RoomGlyphs {
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

    private fun filled(name: String, vararg paths: String) = ImageVector.Builder(
        name = name, defaultWidth = 24.dp, defaultHeight = 24.dp, viewportWidth = 24f, viewportHeight = 24f,
    ).apply {
        for (path in paths) addPath(pathData = addPathNodes(path), fill = SolidColor(Color.Black))
    }.build()

    val Invite = stroked("Invite", "M9 4.6 A3.2 3.2 0 1 1 8.99 4.6 Z M3 19.5 C3.8 15.2 14.2 15.2 15 19.5 M19 8 V14 M16 11 H22")
    val People = stroked("People", "M9 4.6 A3.2 3.2 0 1 1 8.99 4.6 Z M3 19.5 C3.8 15.2 14.2 15.2 15 19.5 M15.6 5 A3 3 0 0 1 15.6 11 M17.7 14.4 C19.5 15 20.6 16.6 21 19.5")
    val Leave = stroked("Leave", "M14 4.5 H19.5 V19.5 H14 M10 8 L6 12 L10 16 M6 12 H15")
    val Info = stroked("Info", "M12 3 A9 9 0 1 1 11.99 3 Z M12 10.5 V16.5 M12 7.4 V7.6")
    val Copy = stroked("Copy", "M8.5 8.5 H19.5 V19.5 H8.5 Z M4.5 15.5 V4.5 H15.5")
    val Share = stroked("Share", "M12 3 V15 M7.5 7.5 L12 3 L16.5 7.5 M7 11 H5 V21 H19 V11 H17")
    val Captions = stroked("Captions", "M6 5.5 H18 A2.5 2.5 0 0 1 20.5 8 V16 A2.5 2.5 0 0 1 18 18.5 H6 A2.5 2.5 0 0 1 3.5 16 V8 A2.5 2.5 0 0 1 6 5.5 Z M7.5 11 H11.5 M13.5 11 H16.5 M7.5 14 H9.5 M11.5 14 H16.5")
    val Episodes = stroked("Episodes", "M4 7 H20 M4 12 H20 M4 17 H14")
    val Full = stroked("Full", "M4 9 V4 H9 M15 4 H20 V9 M20 15 V20 H15 M9 20 H4 V15")
    val ExitFull = stroked("ExitFull", "M9 4 V9 H4 M20 9 H15 V4 M15 20 V15 H20 M4 15 H9 V20")
    val ChevronDown = stroked("ChevronDown", "M5 9 L12 16 L19 9")
    val Back = stroked("Back", "M20 12 H4 M10 6 L4 12 L10 18")
    val Close = stroked("Close", "M6 6 L18 18 M18 6 L6 18")
    val Pause = filled("Pause", "M7.2 4.5 H8.8 A1.2 1.2 0 0 1 10 5.7 V18.3 A1.2 1.2 0 0 1 8.8 19.5 H7.2 A1.2 1.2 0 0 1 6 18.3 V5.7 A1.2 1.2 0 0 1 7.2 4.5 Z", "M15.2 4.5 H16.8 A1.2 1.2 0 0 1 18 5.7 V18.3 A1.2 1.2 0 0 1 16.8 19.5 H15.2 A1.2 1.2 0 0 1 14 18.3 V5.7 A1.2 1.2 0 0 1 15.2 4.5 Z")
    val Play = filled("Play", "M7 4.5 L19.5 12 L7 19.5 Z")
    val More = filled("More", "M12 3.7 A1.8 1.8 0 1 1 11.99 3.7 Z", "M12 10.2 A1.8 1.8 0 1 1 11.99 10.2 Z", "M12 16.7 A1.8 1.8 0 1 1 11.99 16.7 Z")
}
