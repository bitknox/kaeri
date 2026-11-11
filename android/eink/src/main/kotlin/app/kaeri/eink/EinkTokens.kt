// The e-ink token set from the e-ink board: paper and ink, three greys for text and rules, solid
// shapes and no motion. A colour panel adds one amber for the one primary action, with black text
// on it. Everything else is the same on every panel.

package app.kaeri.eink

import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.LineHeightStyle
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import app.kaeri.design.ShowTitleFamily
import app.kaeri.design.robotoFlex

/** The board's colours. */
object EinkColours {
    val Paper = Color(0xFFFFFFFF)
    val Ink = Color(0xFF000000)
    /** Secondary text. */
    val Secondary = Color(0xFF333333)
    /** Tertiary text and hairlines. */
    val Tertiary = Color(0xFF555555)
    /** A selected row, segment or tab. */
    val Selected = Color(0xFFDDDDDD)
    /** The band two strip cuts share. */
    val Overlap = Color(0xFFEEEEEE)
    /** A cover not yet drawn, and its edge. */
    val Placeholder = Color(0xFFEEEEEE)
    val PlaceholderEdge = Color(0xFFCCCCCC)
    /** The one primary action on a colour panel, with ink on it. */
    val Amber = Color(0xFFF2A64B)

    /** The colour question's four squares: red, green, blue, amber. */
    val Squares = listOf(Color(0xFFD8483A), Color(0xFF3A9A58), Color(0xFF3A62C8), Color(0xFFF2A64B))
}

/** The board's sizes, in dp. */
object EinkSizes {
    val Hairline: Dp = 1.dp
    val Rule: Dp = 2.dp
    val ButtonHeight: Dp = 54.dp
    val ButtonRadius: Dp = 27.dp
    val ButtonSide: Dp = 26.dp
    val TextButtonHeight: Dp = 52.dp
    val TextButtonLeast: Dp = 48.dp // a short word still has a 48 dp target
    val SegmentHeight: Dp = 48.dp
    val SegmentRadius: Dp = 10.dp
    val Row: Dp = 66.dp
    val ChapterRow: Dp = 62.dp
    val ChapterNumber: Dp = 44.dp
    val PagerRow: Dp = 68.dp
    val PagerArrowWidth: Dp = 76.dp
    val PagerArrowHeight: Dp = 52.dp
    val BottomNav: Dp = 62.dp
    val TopBar: Dp = 88.dp
    val BottomBar: Dp = 170.dp
    val SheetPadding: Dp = 24.dp
    val SheetRow: Dp = 60.dp
    val SheetLabel: Dp = 150.dp
    val NewDot: Dp = 10.dp
}

/** The board's type: titles in Zen Kaku Gothic New, the rest in Roboto at 15 and up. */
object EinkType {
    private fun title(size: TextUnit) = TextStyle(fontFamily = ShowTitleFamily, fontWeight = FontWeight.Bold, fontSize = size, color = EinkColours.Ink)

    private fun ui(weight: FontWeight, size: TextUnit, colour: Color = EinkColours.Ink) =
        TextStyle(fontFamily = robotoFlex(weight, size), fontWeight = weight, fontSize = size, color = colour)

    val PageTitle = title(28.sp)
    val SeriesTitle = title(30.sp)
    val RowTitle = title(18.sp)
    val CoverTitle = title(15.sp)
    val ReaderTitle = title(21.sp)
    val Body = ui(FontWeight.Medium, 17.sp)
    val Secondary = ui(FontWeight.Medium, 15.sp, EinkColours.Secondary)
    val Tertiary = ui(FontWeight.Medium, 15.sp, EinkColours.Tertiary)
    val Label = ui(FontWeight.SemiBold, 17.sp)
    val Button = ui(FontWeight.Bold, 17.sp)
    val Folio = ui(FontWeight.Medium, 15.sp, EinkColours.Secondary).copy(fontFeatureSettings = "tnum")
    val Waiting = ui(FontWeight.SemiBold, 18.sp)

    /** Prose: Literata 500 at 20 with a line height of 1.5 where the font is there, a serif otherwise. */
    val Prose = TextStyle(
        fontFamily = FontFamily.Serif,
        fontWeight = FontWeight.Medium,
        fontSize = 20.sp,
        lineHeight = 1.5.em,
        lineHeightStyle = LineHeightStyle(LineHeightStyle.Alignment.Center, LineHeightStyle.Trim.None),
        color = EinkColours.Ink,
    )
}

/** What the e-ink screens know about the panel they are on. */
@Immutable
data class EinkLook(
    /** A colour panel: the one primary action is amber. */
    val colour: Boolean = false,
) {
    /** The solid button's fill and its text. */
    val primary: Color get() = if (colour) EinkColours.Amber else EinkColours.Ink
    val onPrimary: Color get() = if (colour) EinkColours.Ink else EinkColours.Paper
}

val LocalEinkLook = staticCompositionLocalOf { EinkLook() }

/** The e-ink look for what is inside. */
@Composable
fun EinkTheme(look: EinkLook = EinkLook(), content: @Composable () -> Unit) {
    CompositionLocalProvider(LocalEinkLook provides look, content = content)
}

object Eink {
    val look: EinkLook
        @Composable @ReadOnlyComposable
        get() = LocalEinkLook.current
}
