// The TV's icons: the phone's set, 24 dp, stroked at 2 with round caps and joins, tinted by the
// content colour, so a focused (silk) container draws them in the ground colour.

package app.kaeri.tv.ui

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.addPathNodes
import androidx.compose.ui.unit.dp

object TvIcons {
    private fun icon(name: String, stroked: List<String>, filled: List<String> = emptyList()) = ImageVector.Builder(
        name = name, defaultWidth = 24.dp, defaultHeight = 24.dp, viewportWidth = 24f, viewportHeight = 24f,
    ).apply {
        for (path in stroked) {
            addPath(
                pathData = addPathNodes(path),
                stroke = SolidColor(Color.Black),
                strokeLineWidth = 2f,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
            )
        }
        for (path in filled) addPath(pathData = addPathNodes(path), fill = SolidColor(Color.Black))
    }.build()

    private const val CIRCLE_9 = "M12 3 A9 9 0 1 1 11.99 3 Z"
    private const val CALENDAR_BODY = "M7 5 H17 A3 3 0 0 1 20 8 V17 A3 3 0 0 1 17 20 H7 A3 3 0 0 1 4 17 V8 A3 3 0 0 1 7 5 Z"

    // The rail.
    val Search = icon("Search", listOf("M10.5 4.5 A6 6 0 1 1 10.49 4.5 Z", "M15 15 L19.5 19.5"))
    val Continue = icon("Continue", listOf(CIRCLE_9, "M10 8.5 V15.5 L15.5 12 Z"))
    val Calendar = icon("Calendar", listOf(CALENDAR_BODY, "M4 10 H20 M8.5 3 V6.5 M15.5 3 V6.5"))
    val Seasonal = icon("Seasonal", listOf("M5 19 C5 11 11 5 19 5 C19 13 13 19 5 19 Z", "M5 19 L12.5 11.5"))
    val Library = icon("Library", listOf("M5.5 4.5 V19.5 M10 4.5 V19.5 M14 5.5 L18.5 19"))
    val People = icon(
        "People",
        listOf(
            "M9 3.9 A3.75 3.75 0 1 1 8.985 3.9 Z",
            "M2.25 20.25 C3.3 15.3 14.7 15.3 15.75 20.25",
            "M15.9 4.35 A3.45 3.45 0 0 1 15.9 11.25",
            "M18.45 15.45 C20.25 16.2 21.3 17.85 21.75 20.25",
        ),
    )
    val Settings = icon(
        "Settings",
        listOf(
            "M12 9 A3 3 0 1 1 11.99 9 Z",
            "M12 2.5 V5 M12 19 V21.5 M21.5 12 H19 M5 12 H2.5 M18.7 5.3 L17 7 M7 17 L5.3 18.7 M18.7 18.7 L17 17 M7 7 L5.3 5.3",
        ),
    )

    // Actions.
    val Play = icon("Play", emptyList(), listOf("M8 5.5 V18.5 L18.5 12 Z"))
    val Pause = icon(
        "Pause",
        emptyList(),
        listOf(
            "M7.5 5 H8.5 A1.5 1.5 0 0 1 10 6.5 V17.5 A1.5 1.5 0 0 1 8.5 19 H7.5 A1.5 1.5 0 0 1 6 17.5 V6.5 A1.5 1.5 0 0 1 7.5 5 Z",
            "M15.5 5 H16.5 A1.5 1.5 0 0 1 18 6.5 V17.5 A1.5 1.5 0 0 1 16.5 19 H15.5 A1.5 1.5 0 0 1 14 17.5 V6.5 A1.5 1.5 0 0 1 15.5 5 Z",
        ),
    )
    val More = icon(
        "More",
        emptyList(),
        listOf(6f, 12f, 18f).map { y -> "M12 ${y - 1.5f} A1.5 1.5 0 1 1 11.99 ${y - 1.5f} Z" },
    )
    val Down = icon("Down", listOf("M7 10 L12 15 L17 10"))
    val Update = icon("Update", listOf("M12 4 V15 M7.5 10.5 L12 15 L16.5 10.5 M5 19.5 H19"))
    val Check = icon("Check", listOf("M5 12.5 L10 17.5 L19 7"))

    /** The eye struck through: a still spoiler protection covers. */
    val Hidden = icon(
        "Hidden",
        listOf(
            "M3 3 L21 21",
            "M10.6 6.1 C11.06 6.04 11.53 6 12 6 C16.5 6 20 9.5 21.5 12 C20.9 13 20 14.2 18.8 15.2",
            "M15.4 17.3 C14.3 17.75 13.2 18 12 18 C7.5 18 4 14.5 2.5 12 C3.3 10.6 4.6 9 6.3 7.8",
            "M9.9 9.9 A3 3 0 0 0 14.1 14.1",
        ),
    )
    val Sort = icon("Sort", listOf("M4 7 H20 M7 12 H17 M10 17 H14"))
    val Keyboard = icon(
        "Keyboard",
        listOf(
            "M5 6 H19 A2 2 0 0 1 21 8 V16 A2 2 0 0 1 19 18 H5 A2 2 0 0 1 3 16 V8 A2 2 0 0 1 5 6 Z",
            "M7 10 H7.01 M11 10 H11.01 M15 10 H15.01 M17 10 H17.01 M8 14 H16",
        ),
    )
    val Phone = icon(
        "Phone",
        listOf("M9 3 H15 A2 2 0 0 1 17 5 V19 A2 2 0 0 1 15 21 H9 A2 2 0 0 1 7 19 V5 A2 2 0 0 1 9 3 Z", "M11 18 H13"),
    )
    val Delete = icon("Delete", listOf("M9 6 H20 V18 H9 L3.5 12 Z", "M12.5 9.5 L16.5 13.5 M16.5 9.5 L12.5 13.5"))

    // The player.
    val Episodes = icon("Episodes", listOf("M4 6.5 H17 M4 11.5 H17 M4 16.5 H10"), listOf("M14 14.5 V20 L19 17.25 Z"))
    val Subtitles = icon(
        "Subtitles",
        listOf(
            "M6 5 H18 A3 3 0 0 1 21 8 V16 A3 3 0 0 1 18 19 H6 A3 3 0 0 1 3 16 V8 A3 3 0 0 1 6 5 Z",
            "M7 12 H9.5 M12.5 12 H17 M7 15.5 H13 M16 15.5 H17",
        ),
    )
    val Source = icon("Source", listOf("M8.5 3 V7 M15.5 3 V7 M6 7 H18 V11 C18 14.5 15.3 17 12 17 C8.7 17 6 14.5 6 11 Z M12 17 V21"))
    val Info = icon("Info", listOf(CIRCLE_9, "M12 11 V16.5"), listOf("M12 6.65 A1.1 1.1 0 1 1 11.99 6.65 Z"))
    val SkipNext = icon("SkipNext", listOf("M17.5 6 V18"), listOf("M6 6.5 V17.5 L14 12 Z"))
}
