// What the player's bars sit on: the quiet 35 % ground dim over the whole frame, and a gradient
// behind each bar that reaches 72 % at the screen edge, so Silk text and icons hold 5:1 on a white
// frame. Between the bars the picture keeps its 35 %.

package app.kaeri.design

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/** The flat dim over the whole frame. */
const val PlayerDimAlpha = 0.35f

/** What the dim and the scrim reach together at the screen edge. */
const val PlayerScrimEdgeAlpha = 0.72f

/** The extra opacity the gradient adds at the edge, on top of the flat dim, to reach [PlayerScrimEdgeAlpha]. */
private const val ExtraAtEdge = 1f - (1f - PlayerScrimEdgeAlpha) / (1f - PlayerDimAlpha)

/**
 * The flat dim and, behind the top and bottom bars, a gradient from [edge] dp of ground at 72 %
 * down to nothing. [edge] is how far in the bar's content reaches, plus a little.
 */
fun Modifier.playerScrim(edge: Dp = 128.dp): Modifier = drawBehind {
    drawRect(ObiSilk.Ground.copy(alpha = PlayerDimAlpha))
    val reach = minOf(edge.toPx(), size.height / 2)
    val strong = ObiSilk.Ground.copy(alpha = ExtraAtEdge)
    drawRect(
        Brush.verticalGradient(listOf(strong, Color.Transparent), startY = 0f, endY = reach),
        size = Size(size.width, reach),
    )
    drawRect(
        Brush.verticalGradient(listOf(Color.Transparent, strong), startY = size.height - reach, endY = size.height),
        topLeft = Offset(0f, size.height - reach),
        size = Size(size.width, reach),
    )
}

/** The scrim as its own layer, for a screen that lays chrome over it. */
@Composable
fun PlayerScrim(edge: Dp = 128.dp) {
    Box(Modifier.fillMaxSize().playerScrim(edge))
}
