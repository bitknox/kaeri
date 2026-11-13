// The e-ink controls from the board: one solid button a screen, outlined and text buttons, the
// segmented control, the pager row that pages a list in place of scrolling, list rows, the New
// dot and the rules. None of them animates or ripples. A press inverts the control in one still
// update the moment the finger lands, and holds until its result is drawn: a chapter that takes
// a while to open keeps its button pressed, never a screen that looks as if nothing happened.

package app.kaeri.eink

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.IndicationNodeFactory
import androidx.compose.foundation.interaction.FocusInteraction
import androidx.compose.foundation.focusGroup
import androidx.compose.ui.focus.FocusDirection
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.foundation.interaction.InteractionSource
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.PressInteraction
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
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.ColorMatrix
import androidx.compose.ui.graphics.Paint
import androidx.compose.ui.graphics.drawscope.ContentDrawScope
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.drawOutline
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.input.InputMode
import androidx.compose.ui.node.CompositionLocalConsumerModifierNode
import androidx.compose.ui.node.currentValueOf
import androidx.compose.ui.platform.LocalInputModeManager
import androidx.compose.ui.node.DelegatableNode
import androidx.compose.ui.node.DrawModifierNode
import androidx.compose.ui.node.invalidateDraw
import androidx.compose.ui.Modifier.Node
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
import app.kaeri.engine.model.EinkPaint
import app.kaeri.i18n.Strings
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/**
 * A click with no ripple and no press of its own drawn: for what is not a control, such as a
 * cover. Keyboard and D-pad focus draws the focus mark along [shape], [inset] inside its edge.
 */
@Composable
fun Modifier.einkClick(
    enabled: Boolean = true,
    role: Role = Role.Button,
    shape: Shape = RectangleShape,
    inset: Dp = 0.dp,
    onClick: () -> Unit,
): Modifier =
    clickable(interactionSource = remember { MutableInteractionSource() }, indication = einkFocus(shape, inset), enabled = enabled, role = role, onClick = onClick)

/**
 * Something the person asked for is under way here, such as a chapter opening: the control
 * pressed for it stays inverted until it ends.
 */
val LocalEinkBusy = compositionLocalOf { false }

/** Where a press and a waiting line are told to the refresh policy: the reader's painter, else nowhere. */
val LocalEinkPaint = compositionLocalOf<(EinkPaint) -> Unit> { {} }

/** The least a press stays drawn, so the panel's fast update shows it before the control is restored. */
const val PRESS_MS = 150L

/** No result named: the press holds [PRESS_MS], or while [LocalEinkBusy] says its work is under way. */
private object NoResult

/**
 * A control's press, as the board draws it: inverted the moment the finger lands, then held from
 * the click until its result. The result is the work [LocalEinkBusy] says is under way, ending, or
 * the control's own result value changing; one that does not change lets go at [PAPER_QUIET_MS].
 * With neither the press holds [PRESS_MS]. A press that came while other work was under way draws
 * only under the finger.
 */
@Stable
class EinkPress internal constructor() {
    internal val source = MutableInteractionSource()
    internal var down by mutableStateOf(false)
    internal var held by mutableStateOf(false)
    internal var heldFor: Any? = NoResult
    internal var sawBusy = false
    internal var busy = false
    internal var result: Any? = NoResult
    internal var paint: (EinkPaint) -> Unit = {}

    /** Drawn inverted: under the finger, or held for its result. */
    val inverted: Boolean get() = down || (held && (busy || (!sawBusy && result == heldFor)))

    internal fun click(onClick: () -> Unit) {
        if (!busy) {
            heldFor = result
            sawBusy = false
            held = true
        }
        paint(EinkPaint.Press)
        onClick()
    }
}

/** A press for a control whose result is [result] changing, or the work [LocalEinkBusy] names. */
@Composable
fun rememberEinkPress(result: Any? = NoResult): EinkPress {
    val press = remember { EinkPress() }
    val busy = LocalEinkBusy.current
    press.busy = busy
    press.result = result
    press.paint = LocalEinkPaint.current
    LaunchedEffect(press) {
        press.source.interactions.collect {
            when (it) {
                is PressInteraction.Press -> press.down = true
                is PressInteraction.Release, is PressInteraction.Cancel -> press.down = false
            }
        }
    }
    LaunchedEffect(press, press.held, busy, result) {
        if (!press.held) return@LaunchedEffect
        if (busy) {
            press.sawBusy = true
            return@LaunchedEffect
        }
        // The work ended or the result came: the control is drawn as it was with it.
        if (press.sawBusy || result != press.heldFor) {
            press.held = false
            return@LaunchedEffect
        }
        delay(if (result === NoResult) PRESS_MS else PAPER_QUIET_MS)
        press.held = false
    }
    return press
}

/** A click on a control that draws [press], with the focus mark along [shape], [inset] inside its edge. */
fun Modifier.einkPress(
    press: EinkPress,
    enabled: Boolean = true,
    role: Role = Role.Button,
    shape: Shape = RectangleShape,
    inset: Dp = 0.dp,
    onClick: () -> Unit,
): Modifier =
    clickable(interactionSource = press.source, indication = einkFocus(shape, inset), enabled = enabled, role = role) { press.click(onClick) }

private val negative = ColorFilter.colorMatrix(
    ColorMatrix(
        floatArrayOf(
            -1f, 0f, 0f, 0f, 255f,
            0f, -1f, 0f, 0f, 255f,
            0f, 0f, -1f, 0f, 255f,
            0f, 0f, 0f, 1f, 0f,
        ),
    ),
)

/** Draws [content] in negative on paper, so paper turns ink and ink paper: the board's 1-bit press. */
private fun DrawScope.inverted(content: () -> Unit) {
    drawIntoCanvas { canvas ->
        canvas.saveLayer(Rect(Offset.Zero, size), Paint().apply { colorFilter = negative })
        drawRect(EinkColours.Paper)
        content()
        canvas.restore()
    }
}

/** What follows in the chain drawn inverted while [on]: put it after a clip, before the fill and border. */
fun Modifier.einkInverted(on: Boolean): Modifier = if (!on) this else drawWithContent { inverted { drawContent() } }

/**
 * The press for clickable rows outside the e-ink controls, such as a series page's chapter rows:
 * the row inverts while the finger is down. A chapter opened from it covers it at once. Keyboard
 * and D-pad focus draws the focus mark round the row.
 */
val EinkPressIndication: IndicationNodeFactory = EinkMark(RectangleShape, 0.dp, invertOnPress = true)

/**
 * A focus group the D-pad's arrows stay in, and with [tab] Tab and Shift Tab too: a move that
 * would leave it leaves the focus where it is. Focus asked for elsewhere, as when a sheet opens
 * or closes, still goes.
 */
fun Modifier.keepsArrows(tab: Boolean): Modifier = focusProperties {
    onExit = {
        val way = requestedFocusDirection
        val arrow = way == FocusDirection.Up || way == FocusDirection.Down || way == FocusDirection.Left || way == FocusDirection.Right
        if (arrow || (tab && (way == FocusDirection.Next || way == FocusDirection.Previous))) cancelFocusChange()
    }
}.focusGroup()

/** Puts the focus here if it can go here now; a clickable cannot while the screen is in touch mode. */
fun FocusRequester.tryFocus(): Boolean = runCatching { requestFocus(FocusDirection.Enter) }.getOrDefault(false)

/**
 * The focus mark for keyboard and D-pad users, for any clickable on paper: a 3 dp ink ring along
 * [shape], [inset] inside the control's edge, with a 2 dp paper line inside the ring. A negative
 * [inset] draws it outside a row whose words start at its edge, where nothing clips it. It is one
 * still paint, with no motion, so the panel's fast update draws it, and it shows only while keys
 * move the focus: a touch never draws it, and the first touch after keys takes it away.
 */
fun einkFocus(shape: Shape = RectangleShape, inset: Dp = 0.dp): IndicationNodeFactory = EinkMark(shape, inset, invertOnPress = false)

private data class EinkMark(val shape: Shape, val inset: Dp, val invertOnPress: Boolean) : IndicationNodeFactory {
    override fun create(interactionSource: InteractionSource): DelegatableNode = MarkNode(interactionSource, shape, inset, invertOnPress)

    private class MarkNode(
        private val source: InteractionSource,
        private val shape: Shape,
        private val inset: Dp,
        private val invertOnPress: Boolean,
    ) : Node(), DrawModifierNode, CompositionLocalConsumerModifierNode {
        private var pressed = false
        private var focused = false

        override fun onAttach() {
            coroutineScope.launch {
                source.interactions.collect {
                    val press = when (it) {
                        is PressInteraction.Press -> invertOnPress
                        is PressInteraction.Release, is PressInteraction.Cancel -> false
                        else -> pressed
                    }
                    val focus = when (it) {
                        is FocusInteraction.Focus -> true
                        is FocusInteraction.Unfocus -> false
                        else -> focused
                    }
                    if (press != pressed || focus != focused) {
                        pressed = press
                        focused = focus
                        invalidateDraw()
                    }
                }
            }
        }

        override fun ContentDrawScope.draw() {
            if (pressed) inverted { drawContent() } else drawContent()
            // The input mode is state: the first touch after keys redraws without the mark.
            if (focused && currentValueOf(LocalInputModeManager).inputMode == InputMode.Keyboard) focusMark(shape, inset)
        }
    }
}

/** The focus mark: the ink ring along [shape], [inset] in from the edge, and the paper line inside it. */
private fun DrawScope.focusMark(shape: Shape, inset: Dp) {
    val ring = EinkSizes.FocusRing.toPx()
    val line = EinkSizes.FocusLine.toPx()
    fun stroke(from: Float, width: Float, colour: Color) {
        val at = from + width / 2
        val box = Size(size.width - 2 * at, size.height - 2 * at)
        if (box.width <= 0f || box.height <= 0f) return
        val outline = shape.createOutline(box, layoutDirection, this)
        translate(at, at) { drawOutline(outline, colour, style = Stroke(width)) }
    }
    stroke(inset.toPx(), ring, EinkColours.Ink)
    stroke(inset.toPx() + ring, line, EinkColours.Paper)
}

/**
 * The screen's one primary action: 54 dp, fully round, ink (amber on a colour panel). Pressed it
 * turns paper with an ink border and ink words, which reads the same on a colour panel.
 */
@Composable
fun EinkSolidButton(text: String, onClick: () -> Unit, modifier: Modifier = Modifier, enabled: Boolean = true) {
    val look = Eink.look
    val press = rememberEinkPress()
    val on = press.inverted
    val shape = RoundedCornerShape(EinkSizes.ButtonRadius)
    Box(
        modifier
            .heightIn(min = EinkSizes.ButtonHeight)
            .clip(shape)
            // Until it can be pressed: light grey with grey words, no fade on paper.
            .background(if (!enabled) EinkColours.Selected else if (on) EinkColours.Paper else look.primary)
            .then(if (on && enabled) Modifier.border(BorderStroke(EinkSizes.Rule, EinkColours.Ink), shape) else Modifier)
            .einkPress(press, enabled = enabled, shape = shape, onClick = onClick)
            .padding(horizontal = EinkSizes.ButtonSide),
        contentAlignment = Alignment.Center,
    ) {
        Text(text, style = EinkType.Button.copy(color = if (!enabled) EinkColours.Secondary else if (on) EinkColours.Ink else look.onPrimary), textAlign = TextAlign.Center)
    }
}

/** A second action: the same shape with a 2 dp ink border. */
@Composable
fun EinkOutlinedButton(text: String, onClick: () -> Unit, modifier: Modifier = Modifier, height: Dp = EinkSizes.ButtonHeight) {
    val press = rememberEinkPress()
    Box(
        modifier
            .heightIn(min = height)
            .clip(RoundedCornerShape(height / 2))
            .einkInverted(press.inverted)
            .border(BorderStroke(EinkSizes.Rule, EinkColours.Ink), RoundedCornerShape(height / 2))
            .einkPress(press, shape = RoundedCornerShape(height / 2), inset = EinkSizes.FocusInsideBorder, onClick = onClick)
            .padding(horizontal = EinkSizes.ButtonSide),
        contentAlignment = Alignment.Center,
    ) {
        Text(text, style = EinkType.Button, textAlign = TextAlign.Center)
    }
}

/**
 * A quiet action: words on paper, 52 dp tall and 48 dp wide at the least. A short word sits by
 * [align] in that width: the middle, or the end for a link that ends on the page margin.
 */
@Composable
fun EinkTextButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    underline: Boolean = false,
    oneLine: Boolean = false,
    align: Alignment = Alignment.Center,
) {
    val press = rememberEinkPress()
    Box(
        modifier
            .heightIn(min = EinkSizes.TextButtonHeight)
            .widthIn(min = EinkSizes.TextButtonLeast)
            .clip(RoundedCornerShape(EinkSizes.TextButtonHeight / 2))
            .einkInverted(press.inverted)
            .einkPress(press, shape = RoundedCornerShape(EinkSizes.TextButtonHeight / 2), onClick = onClick)
            .padding(horizontal = 12.dp),
        contentAlignment = align,
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

/** The bars' back glyph, [size] square, inverting round when pressed. */
@Composable
fun EinkBackButton(onBack: () -> Unit, size: Dp) {
    val context = LocalContext.current
    val press = rememberEinkPress()
    Box(
        Modifier
            .size(size)
            .clip(CircleShape)
            .einkInverted(press.inverted)
            .semantics { contentDescription = Strings.einkBack(context) }
            .einkPress(press, shape = CircleShape, onClick = onBack),
        contentAlignment = Alignment.Center,
    ) { Text("‹", style = EinkType.PageTitle) }
}

/** The segmented control: 48 dp, a 2 dp border, the chosen part filled light grey and bold. */
@Composable
fun <T> EinkSegmented(choices: List<T>, chosen: T, label: @Composable (T) -> String, onChoose: (T) -> Unit, modifier: Modifier = Modifier) {
    val shape = RoundedCornerShape(EinkSizes.SegmentRadius)
    Row(modifier.height(EinkSizes.SegmentHeight).clip(shape).border(EinkSizes.Rule, EinkColours.Ink, shape)) {
        choices.forEachIndexed { index, choice ->
            if (index > 0) Box(Modifier.width(EinkSizes.Rule).fillMaxHeight().background(EinkColours.Ink))
            val on = choice == chosen
            // Held until the choice is shown chosen: the engine answers each change.
            val press = rememberEinkPress(result = chosen)
            Box(
                Modifier
                    .weight(1f)
                    .fillMaxHeight()
                    .einkInverted(press.inverted)
                    .background(if (on) EinkColours.Selected else EinkColours.Paper)
                    .semantics { selected = on }
                    .einkPress(press, role = Role.Tab, shape = shape, inset = EinkSizes.FocusInsideBorder) { onChoose(choice) },
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
        PagerArrow("‹", enabled = page > 0, description = Strings.readerActionPrevious(context), page) { onPage(page - 1) }
        Text(
            Strings.readerPageOf(context, page + 1, pages.coerceAtLeast(1)),
            style = EinkType.Label.copy(textDecoration = TextDecoration.Underline),
        )
        PagerArrow("›", enabled = page + 1 < pages, description = Strings.readerActionNext(context), page) { onPage(page + 1) }
    }
}

@Composable
private fun PagerArrow(glyph: String, enabled: Boolean, description: String, page: Int, onClick: () -> Unit) {
    val ink = if (enabled) EinkColours.Ink else EinkColours.PlaceholderEdge
    // Held until the list shows the page it turned to.
    val press = rememberEinkPress(result = page)
    Box(
        Modifier
            .size(EinkSizes.PagerArrowWidth, EinkSizes.PagerArrowHeight)
            .clip(RoundedCornerShape(EinkSizes.PagerArrowHeight / 2))
            .einkInverted(press.inverted)
            .border(EinkSizes.Rule, ink, RoundedCornerShape(EinkSizes.PagerArrowHeight / 2))
            .semantics { contentDescription = description }
            .einkPress(press, enabled = enabled, shape = RoundedCornerShape(EinkSizes.PagerArrowHeight / 2), inset = EinkSizes.FocusInsideBorder, onClick = onClick),
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
    val press = rememberEinkPress()
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = height)
            .einkInverted(press.inverted)
            .background(if (selected) EinkColours.Selected else EinkColours.Paper)
            .einkPress(press, onClick = onClick)
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
    val paint by androidx.compose.runtime.rememberUpdatedState(LocalEinkPaint.current)
    LaunchedEffect(Unit) {
        delay(PAPER_QUIET_MS)
        shown = true
        paint(EinkPaint.Waiting)
    }
    if (shown) content()
}
