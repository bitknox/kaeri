// Tap zones on Android. The engine says what a tap asks for (tapAction) and lays out the guide
// and the setting's diagrams (tapMap); this file says what is open, keeps the guide's coming and
// going, and draws the engine's cells: the phone's guide over the page, and the small diagram the
// phone's tiles and the e-ink radio rows share.

package app.kaeri.reader

import android.view.accessibility.AccessibilityManager
import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.kaeri.design.Motion
import app.kaeri.design.atFullSpeed
import app.kaeri.design.rememberAnimationsRemoved
import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.ReaderMode
import app.kaeri.engine.model.ReaderSettingKey
import app.kaeri.engine.model.ReaderValue
import app.kaeri.engine.model.TapAction
import app.kaeri.engine.model.TapArrow
import app.kaeri.engine.model.TapCell
import app.kaeri.engine.model.TapFlow
import app.kaeri.engine.model.TapMap
import app.kaeri.engine.model.TapReading
import app.kaeri.engine.model.TapZoneSettings
import app.kaeri.engine.model.ZoneLayout
import app.kaeri.engine.model.ZoneSize
import app.kaeri.engine.model.tapAction
import app.kaeri.engine.model.tapGuideDue
import app.kaeri.engine.model.tapMap
import app.kaeri.i18n.Strings

/** A phone's tap zones before the engine has sent the settings: Auto for pages, Off for scrolling. */
private val PHONE_ZONES = TapZoneSettings(
    paged = ZoneLayout.AUTO,
    scrolling = ZoneLayout.OFF,
    size = ZoneSize.STANDARD,
    flipSides = false,
    flipEnds = false,
    eink = false,
    swipesTurn = true,
    guideSeenPaged = "",
    guideSeenScrolling = "",
)

/** E-ink's before the engine has sent them: Sides, the 40/30/30 split. */
private val EINK_ZONES = PHONE_ZONES.copy(paged = ZoneLayout.SIDES, eink = true)

/** The tap zone settings the engine sent with the reader's settings, or the screen's defaults. */
fun ReaderState.tapZones(eink: Boolean = false): TapZoneSettings =
    settings?.tapZones ?: if (eink) EINK_ZONES else PHONE_ZONES

/**
 * What is open, as the zones are worked out for: the view's mode and direction, a novel's paged or
 * scrolled pages with its progression. Null before a chapter opens.
 */
fun ReaderState.tapReading(zoomed: Boolean = false, screenReader: Boolean = false): TapReading? {
    val view = view ?: return null
    val mode = if (view.medium == Medium.NOVEL) {
        val word = (settings?.rows?.firstOrNull { it.key == ReaderSettingKey.MODE }?.value as? ReaderValue.Choice)?.name
        if (word == "scrolled") ReaderMode.SCROLLED else ReaderMode.PAGED
    } else {
        view.mode
    }
    return TapReading(mode = mode, direction = view.direction, zoomed = zoomed, screenReader = screenReader)
}

/** The action a tap at ([x], [y]), fractions of the reader from its top left, asks for. */
fun TapZoneSettings.actionAt(reading: TapReading, x: Float, y: Float): TapAction =
    tapAction(this, reading, x.toDouble(), y.toDouble())

/** Whether TalkBack or another touch exploring screen reader runs, followed as it changes. */
@Composable
fun rememberScreenReader(): Boolean {
    val context = LocalContext.current
    val manager = remember(context) { context.getSystemService(AccessibilityManager::class.java) }
    var on by remember(manager) { mutableStateOf(manager?.isTouchExplorationEnabled == true) }
    DisposableEffect(manager) {
        val listener = AccessibilityManager.TouchExplorationStateChangeListener { on = it }
        manager?.addTouchExplorationStateChangeListener(listener)
        onDispose { manager?.removeTouchExplorationStateChangeListener(listener) }
    }
    return on
}

/**
 * The tap zone guide over the page: shown when the engine says it is due, after a change of the
 * zones once the settings sheet closes, and from Show zones. It stays with no timer until the
 * first touch, which clears it and still does what it touched.
 */
@Stable
class TapGuide {
    /** The zones drawn; kept while the guide fades out. */
    var map by mutableStateOf<TapMap?>(null)
        private set

    /** Up, or coming up: false once the first touch cleared it. */
    var up by mutableStateOf(false)
        private set

    fun show(map: TapMap) {
        // Off has no zones to show.
        if (map.layout == ZoneLayout.OFF) return
        this.map = map
        up = true
    }

    /** The first touch: the guide goes, and what it showed is returned to keep as seen. */
    fun clear(): TapMap? {
        if (!up) return null
        up = false
        return map
    }

    /** The fade out ended. */
    fun gone() {
        if (!up) map = null
    }
}

/**
 * Shows [guide] when the engine says it is due for [zones] and [reading], once [ready] (the page is
 * drawn and nothing covers it). The engine stops saying so once the guide is kept as seen.
 */
@Composable
fun ShowTapGuideWhenDue(guide: TapGuide, zones: TapZoneSettings?, reading: TapReading?, ready: Boolean) {
    val due = zones != null && reading != null && remember(zones, reading) { tapGuideDue(zones, reading) }
    LaunchedEffect(due && ready) {
        if (due && ready && !guide.up && zones != null && reading != null) guide.show(tapMap(zones, reading, null))
    }
}

/**
 * The first touch anywhere under this clears [guide], and is not taken from what it touches. The
 * guide goes as the finger lands; [onCleared] runs as the last finger lifts, just before what
 * it touched takes the lift, so keeping the guide as seen changes nothing under a touch still
 * going on, and on e-ink the guide's clean refresh comes before the bars the touch opens.
 */
@Composable
fun Modifier.clearsTapGuide(guide: TapGuide, onCleared: (TapMap) -> Unit): Modifier {
    val cleared by rememberUpdatedState(onCleared)
    return pointerInput(guide) {
        awaitEachGesture {
            awaitFirstDown(requireUnconsumed = false, pass = PointerEventPass.Initial)
            val map = guide.clear() ?: return@awaitEachGesture
            do {
                val event = awaitPointerEvent(PointerEventPass.Initial)
            } while (event.changes.any { it.pressed })
            cleared(map)
        }
    }
}

/** The guide's colours on a phone: next in the zone blue, previous pale, controls cream. */
private object GuideColours {
    val next = Color(0xDB7FC7D9) // 86%
    val previous = Color(0xD1C3E3EB) // 82%
    val controls = Color(0xD1EEEAE0) // 82%
    val line = Color(0x66161A2E)
    val ink = Color(0xFF161A2E)
}

/**
 * The phone's guide: the engine's cells over the whole reader, a hairline where two kinds meet,
 * and each labelled cell's name and arrow; the Controls zone says where to change them. It fades
 * in on fade-in and out on fade-out, and Remove animations keeps both fades; with [fades] off
 * (Paper mode) it comes and goes at once.
 */
@Composable
fun BoxScope.TapGuideLayer(guide: TapGuide, fades: Boolean) {
    val map = guide.map ?: return
    val alpha = remember(map) { Animatable(0f) }
    val removed = rememberAnimationsRemoved()
    LaunchedEffect(map, guide.up, fades) {
        val target = if (guide.up) 1f else 0f
        val move: suspend () -> Unit = { alpha.animateTo(target, if (guide.up) Motion.fadeIn() else Motion.fadeOut()) }
        when {
            !fades -> alpha.snapTo(target)
            removed -> atFullSpeed { move() }
            else -> move()
        }
        if (!guide.up) guide.gone()
    }
    val context = LocalContext.current
    val change = Strings.readerZoneChange(context)
    BoxWithConstraints(
        Modifier
            .matchParentSize()
            .graphicsLayer { this.alpha = alpha.value },
    ) {
        val w = maxWidth
        val h = maxHeight
        Canvas(Modifier.fillMaxSize()) {
            drawCells(map.cells, colour = { cellColour(it.action) }, line = GuideColours.line, lineWidth = 1.dp.toPx())
        }
        map.cells.filter { it.labelled }.forEach { cell ->
            CellLabel(cell, w, h) {
                cell.arrow?.let { Arrow(it, 22.dp, GuideColours.ink) }
                Text(cell.name, color = GuideColours.ink, fontSize = 16.sp, lineHeight = 22.sp, fontWeight = FontWeight.Medium, textAlign = TextAlign.Center)
                if (cell.action == TapAction.CONTROLS) {
                    Text(change, color = GuideColours.ink.copy(alpha = 0.78f), fontSize = 13.sp, lineHeight = 17.sp, fontWeight = FontWeight.Medium, textAlign = TextAlign.Center)
                }
            }
        }
    }
}

private fun cellColour(action: TapAction): Color = when (action) {
    TapAction.NEXT -> GuideColours.next
    TapAction.PREVIOUS -> GuideColours.previous
    TapAction.CONTROLS -> GuideColours.controls
}

/** A labelled cell's name, centred in it, kept above [foot] at the bottom of the reader. */
@Composable
fun CellLabel(cell: TapCell, width: Dp, height: Dp, foot: Dp = 0.dp, content: @Composable () -> Unit) {
    val top = height * cell.y.toFloat()
    val tall = minOf(height * cell.height.toFloat(), height - foot - top).coerceAtLeast(60.dp)
    Box(
        Modifier
            .offset(x = width * cell.x.toFloat(), y = top)
            .size(width * cell.width.toFloat(), tall)
            .padding(horizontal = 6.dp),
        contentAlignment = Alignment.Center,
    ) {
        Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(4.dp)) { content() }
    }
}

/** The arrow a zone wears, pointing the way the page moves. */
@Composable
fun Arrow(arrow: TapArrow, size: Dp, colour: Color) {
    Canvas(Modifier.size(size)) {
        val u = this.size.width / 22f
        fun p(x: Float, y: Float) = Offset(x * u, y * u)
        val path = Path()
        val (shaft, head) = when (arrow) {
            TapArrow.RIGHT -> listOf(p(4f, 11f), p(17f, 11f)) to listOf(p(12f, 6f), p(17f, 11f), p(12f, 16f))
            TapArrow.LEFT -> listOf(p(18f, 11f), p(5f, 11f)) to listOf(p(10f, 6f), p(5f, 11f), p(10f, 16f))
            TapArrow.DOWN -> listOf(p(11f, 4f), p(11f, 17f)) to listOf(p(6f, 12f), p(11f, 17f), p(16f, 12f))
            TapArrow.UP -> listOf(p(11f, 18f), p(11f, 5f)) to listOf(p(6f, 10f), p(11f, 5f), p(16f, 10f))
        }
        path.moveTo(shaft[0].x, shaft[0].y)
        path.lineTo(shaft[1].x, shaft[1].y)
        path.moveTo(head[0].x, head[0].y)
        head.drop(1).forEach { path.lineTo(it.x, it.y) }
        drawPath(path, colour, style = Stroke(width = 2f * u, cap = StrokeCap.Round, join = StrokeJoin.Round))
    }
}

/** The nine cells filled, with a line of [lineWidth] on every edge where two kinds meet. */
fun DrawScope.drawCells(cells: List<TapCell>, colour: (TapCell) -> Color, line: Color, lineWidth: Float) {
    val w = size.width
    val h = size.height
    fun at(row: Int, column: Int) = cells.firstOrNull { it.row.toInt() == row && it.column.toInt() == column }
    // Edges on whole pixels: neighbours meet with no seam and no overlap, which a see-through
    // colour would show twice.
    fun px(fraction: Double, of: Float) = kotlin.math.round(fraction * of).toFloat()
    cells.forEach { cell ->
        val left = px(cell.x, w)
        val top = px(cell.y, h)
        val right = px(cell.x + cell.width, w)
        val bottom = px(cell.y + cell.height, h)
        if (right <= left || bottom <= top) return@forEach
        drawRect(colour(cell), Offset(left, top), Size(right - left, bottom - top))
    }
    cells.forEach { cell ->
        val row = cell.row.toInt()
        val column = cell.column.toInt()
        val left = px(cell.x, w)
        val top = px(cell.y, h)
        val right = px(cell.x + cell.width, w)
        val bottom = px(cell.y + cell.height, h)
        at(row, column + 1)?.takeIf { it.action != cell.action }?.let {
            drawLine(line, Offset(right, top), Offset(right, bottom), lineWidth)
        }
        at(row + 1, column)?.takeIf { it.action != cell.action }?.let {
            drawLine(line, Offset(left, bottom), Offset(right, bottom), lineWidth)
        }
    }
}

/** The small diagram's colours: the guide's, opaque, or e-ink's black, grey and white. */
private val DiagramNext = Color(0xFF7FC7D9)
private val DiagramPrevious = Color(0xFFC2E2EA)
private val DiagramControls = Color(0xFFEEEAE0)
private val EinkNext = Color(0xFF4A4A4A)
private val EinkPrevious = Color(0xFFD4D4D4)

/**
 * A layout as a small diagram: its cells from [map], a line where two kinds meet, and a chevron in
 * the labelled Next and Previous cells, pointing the way the page moves. [eink] draws it in black,
 * grey and white, its chevrons in solid black and white.
 */
@Composable
fun TapZoneDiagram(map: TapMap, modifier: Modifier = Modifier, eink: Boolean = false) {
    Canvas(modifier.background(if (eink) Color.White else DiagramControls)) {
        val line = if (eink) Color.Black else Color(0x59161A2E)
        drawCells(
            map.cells,
            colour = {
                when (it.action) {
                    TapAction.NEXT -> if (eink) EinkNext else DiagramNext
                    TapAction.PREVIOUS -> if (eink) EinkPrevious else DiagramPrevious
                    TapAction.CONTROLS -> if (eink) Color.White else DiagramControls
                }
            },
            line = line,
            lineWidth = (if (eink) 1.5f else 1f) * density,
        )
        if (map.layout == ZoneLayout.OFF) return@Canvas
        val z = (size.width / density / 5f).coerceIn(9f, 14f) * density
        map.cells.filter { it.labelled && it.action != TapAction.CONTROLS }.forEach { cell ->
            val arrow = cell.arrow ?: return@forEach
            val next = cell.action == TapAction.NEXT
            val cx = ((cell.x + cell.width / 2) * size.width).toFloat()
            val cy = ((cell.y + cell.height / 2) * size.height).toFloat()
            val a = z / 2f
            val path = Path()
            when (arrow) {
                TapArrow.RIGHT -> { path.moveTo(cx - a / 2, cy - a); path.lineTo(cx + a / 2, cy); path.lineTo(cx - a / 2, cy + a) }
                TapArrow.LEFT -> { path.moveTo(cx + a / 2, cy - a); path.lineTo(cx - a / 2, cy); path.lineTo(cx + a / 2, cy + a) }
                TapArrow.DOWN -> { path.moveTo(cx - a, cy - a / 2); path.lineTo(cx, cy + a / 2); path.lineTo(cx + a, cy - a / 2) }
                TapArrow.UP -> { path.moveTo(cx - a, cy + a / 2); path.lineTo(cx, cy - a / 2); path.lineTo(cx + a, cy + a / 2) }
            }
            // E-ink draws in pure ink at full strength: a tint or a see-through grey dithers on
            // the panel and shows through a colour filter.
            val ink = when {
                eink -> if (next) Color.White else Color.Black
                else -> GuideColours.ink.copy(alpha = if (next) 0.9f else 0.6f)
            }
            drawPath(
                path,
                ink,
                style = Stroke(width = (if (next || eink) 2f else 1.5f) * density, cap = StrokeCap.Round, join = StrokeJoin.Round),
            )
        }
    }
}

/** Which of the two choices [map] belongs to: scrolling, or paged (e-ink has only that one). */
fun TapZoneSettings.scrollingKind(map: TapMap): Boolean = !eink && map.flow == TapFlow.SCROLLING

/** A layout's name in the config, as the engine sends its choices. */
val ZoneLayout.configName: String
    get() = when (this) {
        ZoneLayout.AUTO -> "auto"
        ZoneLayout.SIDES -> "sides"
        ZoneLayout.L_SHAPE -> "l_shape"
        ZoneLayout.WIDE_NEXT -> "wide_next"
        ZoneLayout.TOP_BOTTOM -> "top_bottom"
        ZoneLayout.BOTH_NEXT -> "both_next"
        ZoneLayout.OFF -> "off"
    }

/** The layout a choice the engine sends names, or null for one this app does not know. */
fun zoneLayout(name: String): ZoneLayout? = ZoneLayout.entries.firstOrNull { it.configName == name }

/** A layout's written name, for its tile, its radio row and the row's value. */
fun layoutName(context: android.content.Context, layout: ZoneLayout): String = when (layout) {
    ZoneLayout.AUTO -> Strings.readerTapAuto(context)
    ZoneLayout.SIDES -> Strings.readerTapSides(context)
    ZoneLayout.L_SHAPE -> Strings.readerTapLShape(context)
    ZoneLayout.WIDE_NEXT -> Strings.readerTapWideNext(context)
    ZoneLayout.TOP_BOTTOM -> Strings.readerTapTopBottom(context)
    ZoneLayout.BOTH_NEXT -> Strings.readerTapBothNext(context)
    ZoneLayout.OFF -> Strings.readerTapOff(context)
}

/** The choices the engine offers for [key] (Tap zones for pages or for scrolling), in its order. */
fun ReaderState.zoneChoices(key: ReaderSettingKey): List<ZoneLayout> =
    settings?.rows?.firstOrNull { it.key == key }?.choices.orEmpty()
        .mapNotNull { (it as? ReaderValue.Choice)?.name?.let(::zoneLayout) }

/**
 * The reading the setting's tiles are drawn for: what is open when its kind is the one shown, or
 * else the same book read the other way (a strip for Scrolling, pages for Paged).
 */
fun TapReading.forKind(scrolling: Boolean): TapReading {
    val isScrolling = mode == ReaderMode.STRIP || mode == ReaderMode.SCROLLED
    return when {
        scrolling == isScrolling -> this
        scrolling -> copy(mode = ReaderMode.STRIP)
        else -> copy(mode = ReaderMode.PAGED)
    }
}
