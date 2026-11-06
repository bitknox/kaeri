// The reader's own colours and glyphs. The chrome follows the look: dark with the app, light with
// the Paper, Light and Sepia looks. Amber stays for Continue and a failed page's Try again.

package app.kaeri.reader.phone

import androidx.compose.foundation.Canvas
import androidx.compose.runtime.Composable
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.foundation.layout.size
import app.kaeri.design.ObiSilk

/** One set of chrome colours. */
internal data class ChromeColours(
    val ground: Color,
    val raised: Color,
    val selected: Color,
    val text: Color,
    val faint: Color,
    val rule: Color,
    val light: Boolean,
)

internal val DarkChrome = ChromeColours(
    ground = ObiSilk.Ground,
    raised = ObiSilk.Raised,
    selected = ObiSilk.RaisedSelected,
    text = ObiSilk.Silk,
    faint = ObiSilk.Haze,
    rule = ObiSilk.Rule,
    light = false,
)

/** The board's light tokens: ground F0EDE4, raised E3DFD4, silk 1E2033, haze 4F546E. */
internal val LightChrome = ChromeColours(
    ground = Color(0xFFF0EDE4),
    raised = Color(0xFFE3DFD4),
    selected = Color(0xFFD3CEC0),
    text = Color(0xFF1E2033),
    faint = Color(0xFF4F546E),
    rule = Color(0xFFC3BEAF),
    light = true,
)

internal val LocalChrome = compositionLocalOf { DarkChrome }

/** The colours of the reader's own chrome, read where they are drawn. */
internal object Chrome {
    val ground: Color @Composable get() = LocalChrome.current.ground
    val plate: Color @Composable get() = LocalChrome.current.raised
    val selected: Color @Composable get() = LocalChrome.current.selected
    val text: Color @Composable get() = LocalChrome.current.text
    val faint: Color @Composable get() = LocalChrome.current.faint
    val rule: Color @Composable get() = LocalChrome.current.rule
    val accent: Color get() = ObiSilk.Amber
    val onAccent: Color get() = ObiSilk.Ground
    val error: Color get() = ObiSilk.Vermilion

    /** The Paper look's ground, `look-paper` on the board. */
    val paper = Color(0xFFEFE9DC)
}

internal enum class Glyph { Close, List, Back, Check, Warning, Chevron, Search }

/** A line glyph in [colour], 24 dp by default, drawn from the board's paths. */
@Composable
internal fun GlyphIcon(glyph: Glyph, colour: Color, modifier: Modifier = Modifier, size: Dp = 24.dp, mirror: Boolean = false) {
    Canvas(modifier.size(size)) {
        val s = this.size.minDimension / 24f
        val width = 2f * s
        val stroke = Stroke(width = width, cap = StrokeCap.Round, join = StrokeJoin.Round)
        fun p(x: Float, y: Float) = Offset((if (mirror) 24f - x else x) * s, y * s)
        fun line(a: Offset, b: Offset) = drawLine(colour, a, b, strokeWidth = width, cap = StrokeCap.Round)
        when (glyph) {
            Glyph.Close -> {
                line(p(6f, 6f), p(18f, 18f))
                line(p(18f, 6f), p(6f, 18f))
            }
            Glyph.List -> {
                for (y in listOf(7f, 12f, 17f)) {
                    line(p(9f, y), p(19f, y))
                    drawCircle(colour, radius = 1.1f * s, center = p(5.5f, y))
                }
            }
            Glyph.Back -> {
                line(p(19f, 12f), p(5f, 12f))
                val path = Path().apply {
                    moveTo(p(11f, 6f).x, p(11f, 6f).y)
                    lineTo(p(5f, 12f).x, p(5f, 12f).y)
                    lineTo(p(11f, 18f).x, p(11f, 18f).y)
                }
                drawPath(path, colour, style = stroke)
            }
            Glyph.Check -> {
                val path = Path().apply {
                    moveTo(p(5f, 12.5f).x, p(5f, 12.5f).y)
                    lineTo(p(10f, 17.5f).x, p(10f, 17.5f).y)
                    lineTo(p(19f, 7f).x, p(19f, 7f).y)
                }
                drawPath(path, colour, style = stroke)
            }
            Glyph.Chevron -> {
                val path = Path().apply {
                    moveTo(p(9f, 6f).x, p(9f, 6f).y)
                    lineTo(p(15f, 12f).x, p(15f, 12f).y)
                    lineTo(p(9f, 18f).x, p(9f, 18f).y)
                }
                drawPath(path, colour, style = stroke)
            }
            Glyph.Warning -> {
                val path = Path().apply {
                    moveTo(p(12f, 4f).x, p(12f, 4f).y)
                    lineTo(p(21f, 19f).x, p(21f, 19f).y)
                    lineTo(p(3f, 19f).x, p(3f, 19f).y)
                    close()
                }
                drawPath(path, colour, style = stroke)
                line(p(12f, 10f), p(12f, 14f))
                drawCircle(colour, radius = 1f * s, center = p(12f, 16.8f))
            }
            Glyph.Search -> {
                drawCircle(colour, radius = 5.5f * s, center = p(11f, 11f), style = stroke)
                line(p(15.5f, 15.5f), p(20f, 20f))
            }
        }
    }
}
