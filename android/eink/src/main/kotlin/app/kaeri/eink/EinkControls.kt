// The e-ink controls from the board: one solid button a screen, outlined and text buttons, the
// segmented control, the pager row that pages a list in place of scrolling, list rows, the New
// dot and the rules. None of them animates or ripples: a press is drawn by the screen's own
// refresh, so these use no indication.

package app.kaeri.eink

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.foundation.text.TextAutoSize
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.sp
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.kaeri.i18n.Strings
import kotlinx.coroutines.delay

/** A click with no ripple: e-ink draws the press itself. */
@Composable
fun Modifier.einkClick(enabled: Boolean = true, role: Role = Role.Button, onClick: () -> Unit): Modifier =
    clickable(interactionSource = remember { MutableInteractionSource() }, indication = null, enabled = enabled, role = role, onClick = onClick)

/** The screen's one primary action: 54 dp, fully round, ink (amber on a colour panel). */
@Composable
fun EinkSolidButton(text: String, onClick: () -> Unit, modifier: Modifier = Modifier) {
    val look = Eink.look
    Box(
        modifier
            .heightIn(min = EinkSizes.ButtonHeight)
            .clip(RoundedCornerShape(EinkSizes.ButtonRadius))
            .background(look.primary)
            .einkClick(onClick = onClick)
            .padding(horizontal = EinkSizes.ButtonSide),
        contentAlignment = Alignment.Center,
    ) {
        Text(text, style = EinkType.Button.copy(color = look.onPrimary), textAlign = TextAlign.Center)
    }
}

/** A second action: the same shape with a 2 dp ink border. */
@Composable
fun EinkOutlinedButton(text: String, onClick: () -> Unit, modifier: Modifier = Modifier, height: Dp = EinkSizes.ButtonHeight) {
    Box(
        modifier
            .heightIn(min = height)
            .clip(RoundedCornerShape(height / 2))
            .border(BorderStroke(EinkSizes.Rule, EinkColours.Ink), RoundedCornerShape(height / 2))
            .einkClick(onClick = onClick)
            .padding(horizontal = EinkSizes.ButtonSide),
        contentAlignment = Alignment.Center,
    ) {
        Text(text, style = EinkType.Button, textAlign = TextAlign.Center)
    }
}

/** A quiet action: words on paper, 52 dp tall. */
@Composable
fun EinkTextButton(text: String, onClick: () -> Unit, modifier: Modifier = Modifier, underline: Boolean = false, oneLine: Boolean = false) {
    Box(
        modifier.heightIn(min = EinkSizes.TextButtonHeight).einkClick(onClick = onClick).padding(horizontal = 12.dp),
        contentAlignment = Alignment.Center,
    ) {
        val style = EinkType.Label.copy(textDecoration = if (underline) TextDecoration.Underline else null)
        if (oneLine) {
            // One line in a full bar: the words shrink a little before they would wrap.
            Text(text, style = style, maxLines = 1, overflow = TextOverflow.Ellipsis, autoSize = TextAutoSize.StepBased(minFontSize = 12.sp, maxFontSize = style.fontSize))
        } else {
            Text(text, style = style)
        }
    }
}

/** The segmented control: 48 dp, a 2 dp border, the chosen part filled light grey and bold. */
@Composable
fun <T> EinkSegmented(choices: List<T>, chosen: T, label: @Composable (T) -> String, onChoose: (T) -> Unit, modifier: Modifier = Modifier) {
    val shape = RoundedCornerShape(EinkSizes.SegmentRadius)
    Row(modifier.height(EinkSizes.SegmentHeight).clip(shape).border(EinkSizes.Rule, EinkColours.Ink, shape)) {
        choices.forEachIndexed { index, choice ->
            if (index > 0) Box(Modifier.width(EinkSizes.Rule).fillMaxHeight().background(EinkColours.Ink))
            val on = choice == chosen
            Box(
                Modifier
                    .weight(1f)
                    .fillMaxHeight()
                    .background(if (on) EinkColours.Selected else EinkColours.Paper)
                    .semantics { selected = on }
                    .einkClick(role = Role.Tab) { onChoose(choice) },
                contentAlignment = Alignment.Center,
            ) {
                // One line: a long choice shrinks a little to fit its part, never wrapping onto the border.
                val style = EinkType.Label.copy(fontWeight = if (on) FontWeight.Bold else FontWeight.Medium)
                Text(
                    label(choice),
                    style = style,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    autoSize = TextAutoSize.StepBased(minFontSize = 12.sp, maxFontSize = style.fontSize),
                    modifier = Modifier.padding(horizontal = 6.dp),
                )
            }
        }
    }
}

/** The pager row under a paged list: two arrows and the underlined "Page 2 of 5". */
@Composable
fun EinkPager(page: Int, pages: Int, onPage: (Int) -> Unit, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    Row(
        modifier.fillMaxWidth().height(EinkSizes.PagerRow).padding(horizontal = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        PagerArrow("‹", enabled = page > 0, description = Strings.readerActionPrevious(context)) { onPage(page - 1) }
        Text(
            Strings.readerPageOf(context, page + 1, pages.coerceAtLeast(1)),
            style = EinkType.Label.copy(textDecoration = TextDecoration.Underline),
        )
        PagerArrow("›", enabled = page + 1 < pages, description = Strings.readerActionNext(context)) { onPage(page + 1) }
    }
}

@Composable
private fun PagerArrow(glyph: String, enabled: Boolean, description: String, onClick: () -> Unit) {
    val ink = if (enabled) EinkColours.Ink else EinkColours.PlaceholderEdge
    Box(
        Modifier
            .size(EinkSizes.PagerArrowWidth, EinkSizes.PagerArrowHeight)
            .clip(RoundedCornerShape(EinkSizes.PagerArrowHeight / 2))
            .border(EinkSizes.Rule, ink, RoundedCornerShape(EinkSizes.PagerArrowHeight / 2))
            .semantics { contentDescription = description }
            .einkClick(enabled = enabled, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Text(glyph, style = EinkType.Button.copy(color = ink))
    }
}

/** A list row: 66 dp, a 1 dp hairline under it, [selected] filled light grey with bold text. */
@Composable
fun EinkRow(
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    selected: Boolean = false,
    height: Dp = EinkSizes.Row,
    content: @Composable RowScope.() -> Unit,
) {
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = height)
            .background(if (selected) EinkColours.Selected else EinkColours.Paper)
            .einkClick(onClick = onClick)
            .hairline()
            .padding(horizontal = 20.dp),
        verticalAlignment = Alignment.CenterVertically,
        content = content,
    )
}

/** The New mark: a 10 dp dot, then the word. */
@Composable
fun EinkNew(modifier: Modifier = Modifier) {
    val context = LocalContext.current
    Row(modifier, verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(EinkSizes.NewDot).clip(CircleShape).background(EinkColours.Ink))
        Spacer(Modifier.width(6.dp))
        Text(Strings.einkNew(context), style = EinkType.Label)
    }
}

/** A 1 dp grey hairline under what it is on. */
fun Modifier.hairline(colour: Color = EinkColours.Tertiary): Modifier = drawBehind {
    val y = size.height - EinkSizes.Hairline.toPx() / 2
    drawLine(colour, Offset(0f, y), Offset(size.width, y), EinkSizes.Hairline.toPx())
}

/** A 2 dp ink rule at the top of what it is on: bars, sheets, the bottom navigation. */
fun Modifier.topRule(): Modifier = drawBehind {
    val y = EinkSizes.Rule.toPx() / 2
    drawLine(EinkColours.Ink, Offset(0f, y), Offset(size.width, y), EinkSizes.Rule.toPx())
}

/** A 2 dp ink rule at the foot of what it is on. */
fun Modifier.bottomRule(): Modifier = drawBehind {
    val y = size.height - EinkSizes.Rule.toPx() / 2
    drawLine(EinkColours.Ink, Offset(0f, y), Offset(size.width, y), EinkSizes.Rule.toPx())
}

/** Splits [items] into pages of [perPage] for a paged list; at least one page, even when empty. */
fun <T> pagesOf(items: List<T>, perPage: Int): List<List<T>> =
    if (items.isEmpty() || perPage <= 0) listOf(emptyList()) else items.chunked(perPage)

/** How many rows of [rowHeight] fit in [space], leaving the pager row's height. */
fun rowsThatFit(space: Dp, rowHeight: Dp = EinkSizes.Row, pager: Dp = EinkSizes.PagerRow): Int =
    ((space - pager) / rowHeight).toInt().coerceAtLeast(1)

/** How long a wait on paper draws nothing, as the board says: most loads end inside it. */
const val PAPER_QUIET_MS = 600L

/**
 * A waiting line on paper: nothing for [PAPER_QUIET_MS], then [content], painted once. A load that
 * ends first costs the panel no update at all.
 */
@Composable
fun PaperWaiting(content: @Composable () -> Unit) {
    var shown by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) {
        delay(PAPER_QUIET_MS)
        shown = true
    }
    if (shown) content()
}
