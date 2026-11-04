// The icons the sync board draws, from its own symbols: 24 dp, stroked at 1.6, a few filled.

package app.kaeri.design

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.addPathNodes
import androidx.compose.ui.unit.dp

object SyncIcons {
    private fun icon(name: String, width: Float, vararg paths: Pair<String, Boolean>) = ImageVector.Builder(
        name = name, defaultWidth = 24.dp, defaultHeight = 24.dp, viewportWidth = 24f, viewportHeight = 24f,
    ).apply {
        for ((path, filled) in paths) {
            addPath(
                pathData = addPathNodes(path),
                fill = if (filled) SolidColor(Color.Black) else null,
                stroke = if (filled) null else SolidColor(Color.Black),
                strokeLineWidth = width,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
            )
        }
    }.build()

    val Cloud = icon("Cloud", 1.6f,
            "M7 18.5h10.5a4 4 0 00.6-7.95A6 6 0 006.4 9.6 4.5 4.5 0 007 18.5z" to false,
        )

    val Folder = icon("Folder", 1.6f,
            "M3 7.2c0-1 .8-1.7 1.7-1.7h4.6l2 2h8c1 0 1.7.8 1.7 1.7v8.6c0 1-.8 1.7-1.7 1.7H4.7c-1 0-1.7-.8-1.7-1.7z" to false,
        )

    val Key = icon("Key", 1.6f,
            "M4.0 12.0a4.0 4.0 0 1 0 8.0 0a4.0 4.0 0 1 0 -8.0 0z" to false,
            "M12 12h9m-3 0v3m-3-3v2" to false,
        )

    val Laptop = icon("Laptop", 1.6f,
            "M6.0 5.0h12.0a1.5 1.5 0 0 1 1.5 1.5v7.5a1.5 1.5 0 0 1 -1.5 1.5h-12.0a1.5 1.5 0 0 1 -1.5 -1.5v-7.5a1.5 1.5 0 0 1 1.5 -1.5z" to false,
            "M2.5 18.5h19l-1.5-3h-16z" to false,
        )

    val Phone = icon("Phone", 1.6f,
            "M9.4 2.8h5.2a2.4 2.4 0 0 1 2.4 2.4v13.599999999999998a2.4 2.4 0 0 1 -2.4 2.4h-5.2a2.4 2.4 0 0 1 -2.4 -2.4v-13.599999999999998a2.4 2.4 0 0 1 2.4 -2.4z" to false,
            "M10.5 18.2h3" to false,
        )

    val Tablet = icon("Tablet", 1.6f,
            "M6.7 2.8h10.6a2.2 2.2 0 0 1 2.2 2.2v13.999999999999998a2.2 2.2 0 0 1 -2.2 2.2h-10.6a2.2 2.2 0 0 1 -2.2 -2.2v-13.999999999999998a2.2 2.2 0 0 1 2.2 -2.2z" to false,
            "M10.5 18.2h3" to false,
        )

    val Desktop = icon("Desktop", 1.6f,
            "M4.6 4.0h14.8a1.6 1.6 0 0 1 1.6 1.6v8.8a1.6 1.6 0 0 1 -1.6 1.6h-14.8a1.6 1.6 0 0 1 -1.6 -1.6v-8.8a1.6 1.6 0 0 1 1.6 -1.6z" to false,
            "M12 16v4m-4 0h8" to false,
        )

    val Devices = icon("Devices", 1.6f,
            "M3.9 5.0h10.2a1.4 1.4 0 0 1 1.4 1.4v6.7a1.4 1.4 0 0 1 -1.4 1.4h-10.2a1.4 1.4 0 0 1 -1.4 -1.4v-6.7a1.4 1.4 0 0 1 1.4 -1.4z" to false,
            "M1.5 17.5h11" to false,
            "M17.4 8.0h3.2a1.4 1.4 0 0 1 1.4 1.4v8.2a1.4 1.4 0 0 1 -1.4 1.4h-3.2a1.4 1.4 0 0 1 -1.4 -1.4v-8.2a1.4 1.4 0 0 1 1.4 -1.4z" to false,
        )

    val Server = icon("Server", 1.8f,
            "M6.0 4.0h12.0a2.0 2.0 0 0 1 2.0 2.0v3.0a2.0 2.0 0 0 1 -2.0 2.0h-12.0a2.0 2.0 0 0 1 -2.0 -2.0v-3.0a2.0 2.0 0 0 1 2.0 -2.0z" to false,
            "M6.0 13.0h12.0a2.0 2.0 0 0 1 2.0 2.0v3.0a2.0 2.0 0 0 1 -2.0 2.0h-12.0a2.0 2.0 0 0 1 -2.0 -2.0v-3.0a2.0 2.0 0 0 1 2.0 -2.0z" to false,
            "M6.9 7.5a1.1 1.1 0 1 0 2.2 0a1.1 1.1 0 1 0 -2.2 0z" to true,
            "M6.9 16.5a1.1 1.1 0 1 0 2.2 0a1.1 1.1 0 1 0 -2.2 0z" to true,
        )

    val Camera = icon("Camera", 1.6f,
            "M3.5 8.5c0-1 .8-1.8 1.8-1.8h2.3L9.2 4.5h5.6l1.6 2.2h2.3c1 0 1.8.8 1.8 1.8v9.2c0 1-.8 1.8-1.8 1.8H5.3c-1 0-1.8-.8-1.8-1.8z" to false,
            "M8.4 12.8a3.6 3.6 0 1 0 7.2 0a3.6 3.6 0 1 0 -7.2 0z" to false,
        )

    val Copy = icon("Copy", 1.6f,
            "M10.5 8.5h7.0a2.0 2.0 0 0 1 2.0 2.0v8.0a2.0 2.0 0 0 1 -2.0 2.0h-7.0a2.0 2.0 0 0 1 -2.0 -2.0v-8.0a2.0 2.0 0 0 1 2.0 -2.0z" to false,
            "M15.5 8.5V6a2 2 0 00-2-2h-7a2 2 0 00-2 2v9a2 2 0 002 2h2" to false,
        )

    val Qr = icon("Qr", 1.6f,
            "M4.5 3.5h4.5a1.0 1.0 0 0 1 1.0 1.0v4.5a1.0 1.0 0 0 1 -1.0 1.0h-4.5a1.0 1.0 0 0 1 -1.0 -1.0v-4.5a1.0 1.0 0 0 1 1.0 -1.0z" to false,
            "M15.0 3.5h4.5a1.0 1.0 0 0 1 1.0 1.0v4.5a1.0 1.0 0 0 1 -1.0 1.0h-4.5a1.0 1.0 0 0 1 -1.0 -1.0v-4.5a1.0 1.0 0 0 1 1.0 -1.0z" to false,
            "M4.5 14.0h4.5a1.0 1.0 0 0 1 1.0 1.0v4.5a1.0 1.0 0 0 1 -1.0 1.0h-4.5a1.0 1.0 0 0 1 -1.0 -1.0v-4.5a1.0 1.0 0 0 1 1.0 -1.0z" to false,
            "M14 14h3v3h-3zM17 17h3.5v3.5H17z" to false,
        )

    val Save = icon("Save", 1.6f,
            "M12 3.5v11m-4.5-4.5L12 14.5l4.5-4.5M4.5 15v3.5c0 1 .8 1.8 1.8 1.8h11.4c1 0 1.8-.8 1.8-1.8V15" to false,
        )

    val Warn = icon("Warn", 2f,
            "M12 3.2L1.8 20.5h20.4z" to true,
            "M12 9.5v5" to false,
            "M10.8 17.4a1.2 1.2 0 1 0 2.4 0a1.2 1.2 0 1 0 -2.4 0z" to false,
        )

    val Lock = icon("Lock", 1.6f,
            "M7.0 10.5h10.0a2.0 2.0 0 0 1 2.0 2.0v6.0a2.0 2.0 0 0 1 -2.0 2.0h-10.0a2.0 2.0 0 0 1 -2.0 -2.0v-6.0a2.0 2.0 0 0 1 2.0 -2.0z" to false,
            "M8.5 10.5V8a3.5 3.5 0 017 0v2.5" to false,
        )

    val Tv = icon("Tv", 1.6f,
            "M4.5 4.5h15.0a2.0 2.0 0 0 1 2.0 2.0v8.5a2.0 2.0 0 0 1 -2.0 2.0h-15.0a2.0 2.0 0 0 1 -2.0 -2.0v-8.5a2.0 2.0 0 0 1 2.0 -2.0z" to false,
            "M8 20h8" to false,
        )

    val People = icon("People", 1.6f,
            "M5.8 8.5a3.2 3.2 0 1 0 6.4 0a3.2 3.2 0 1 0 -6.4 0z" to false,
            "M3 19.5c.5-3.2 3-5 6-5s5.5 1.8 6 5z" to false,
            "M15.2 5.6a3.2 3.2 0 010 5.8M17.5 14.6c2 .6 3.2 2.4 3.5 4.9" to false,
        )

    val Sync = icon("Sync", 1.8f,
            "M19.5 9A8 8 0 0 0 5 7.5M4.5 15A8 8 0 0 0 19 16.5M19.5 4v5h-5M4.5 20v-5h5" to false,
        )

    val Power = icon("Power", 1.8f,
            "M12 3v8M7 6.3a7.5 7.5 0 1 0 10 0" to false,
        )
}
