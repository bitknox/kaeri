// The e-ink reader's settings and contents, as the e-ink board draws them. The sheet is opaque over
// the lower part of the page with a 2 dp rule, and the page above changes as a setting does; This
// series holds the mode, the medium's group holds the rest of the first level, and More holds every
// other row, with Tap zones on a page of its own. Contents is the chapters as a paged list, opened on the page with the one being read.
// Nothing animates: each change is one paint through the refresh policy.

package app.kaeri.eink

import android.content.Context
import androidx.compose.foundation.background
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.runtime.withFrameNanos
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.border
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.sp
import app.kaeri.engine.model.TapAction
import app.kaeri.engine.model.TapMap
import app.kaeri.engine.model.ZoneLayout
import app.kaeri.engine.model.ZoneSize
import app.kaeri.engine.model.tapMap
import app.kaeri.reader.TapZoneDiagram
import app.kaeri.reader.configName
import app.kaeri.reader.layoutName
import app.kaeri.reader.rememberScreenReader
import app.kaeri.reader.tapReading
import app.kaeri.reader.tapZones
import app.kaeri.reader.zoneChoices
import app.kaeri.reader.phone.tapZonesValue
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import app.kaeri.engine.core.ChapterId
import app.kaeri.engine.model.ChapterRow
import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.ReaderMode
import app.kaeri.engine.model.ReaderSettingKey
import app.kaeri.engine.model.ReaderSettingRow
import app.kaeri.engine.model.ReaderValue
import app.kaeri.engine.model.SettingScope
import app.kaeri.i18n.Strings
import app.kaeri.reader.phone.ReaderLook
import app.kaeri.reader.ReaderSession
import app.kaeri.reader.ReaderState
import app.kaeri.reader.phone.chapterWords
import app.kaeri.reader.phone.choiceLabel
import app.kaeri.reader.phone.rowTitle
import kotlin.math.abs
import kotlin.math.roundToInt

private typealias Key = ReaderSettingKey

/** What covers the e-ink page: nothing, the settings sheet, its More or Tap zones page, or Contents. */
enum class EinkLayer { None, Settings, More, TapZones, Contents }

/** The keys the sheet's first level shows; every other row the engine sends is on More. */
private val mangaFirst = listOf(Key.FIT, Key.CROP_BORDERS, Key.REFRESH_IMAGE, Key.VOLUME_KEYS_TURN)
private val novelFirst = listOf(Key.TEXT_SIZE, Key.FONT, Key.LINE_SPACING, Key.MARGINS, Key.LOOK, Key.REFRESH_TEXT)

/**
 * The rows manga and webtoons leave out on e-ink. Look is a novel's: the e-ink board draws manga on
 * paper only, and its manga sheet has no Look, where the novel's sheet has Paper and Dark.
 */
private val mangaLeftOut = setOf(Key.LOOK)

/** The rows the Tap zones page holds, which More leaves out. */
private val tapKeys = setOf(Key.TAP_LAYOUT, Key.TAP_PAGED, Key.TAP_SCROLL, Key.TAP_SIZE, Key.TAP_FLIP_SIDES, Key.TAP_FLIP_ENDS, Key.SWIPE_TURNS)

/** The rows for the e-ink screen itself, under their own heading on More. */
private val screenKeys = listOf(Key.REFRESH_IMAGE, Key.REFRESH_TEXT, Key.REFRESH_LIST, Key.SCREEN_MODE, Key.RAINBOW_FILTER, Key.SATURATION, Key.CONTRAST)

/**
 * The settings sheet over the lower part of the page: Reader settings and Done, This series with
 * the mode for manga, the medium's own rows, and More reader settings.
 */
@Composable
fun BoxScope.EinkSettingsSheet(session: ReaderSession, state: ReaderState, onMore: () -> Unit, onDone: () -> Unit, moreFocus: FocusRequester? = null) {
    val context = LocalContext.current
    val view = state.view ?: return
    val rows = state.settings?.rows.orEmpty()
    fun row(key: Key) = rows.firstOrNull { it.key == key }
    val novel = view.medium == Medium.NOVEL
    SheetFrame(Strings.readerSettings(context), onDone, Modifier.align(Alignment.BottomCenter)) {
        if (novel) {
            SheetHeading(Strings.readerSheetAllNovels(context))
            novelFirst.forEach { key -> row(key)?.let { EinkSettingLine(context, session, it) } }
        } else {
            SheetHeading(Strings.readerSheetSeries(context))
            EinkModeLine(context, session, row(Key.MODE), row(Key.DIRECTION), view.title)
            val strip = view.mode == ReaderMode.STRIP
            SheetHeading(if (strip) Strings.readerSheetAllWebtoons(context) else Strings.readerSheetAllManga(context))
            mangaFirst.forEach { key -> row(key)?.let { EinkSettingLine(context, session, it) } }
        }
        Spacer(Modifier.height(8.dp))
        EinkRow(onClick = onMore, height = EinkSizes.SheetRow, modifier = moreFocus?.let { Modifier.focusRequester(it) } ?: Modifier) {
            Text(Strings.readerSheetMore(context), style = EinkType.Label, modifier = Modifier.weight(1f))
            Text("›", style = EinkType.Button)
        }
    }
}

/** Every row the first level leaves out, grouped as the phone's More page groups them, with Tap zones first. */
@Composable
fun BoxScope.EinkMoreSheet(session: ReaderSession, state: ReaderState, onBack: () -> Unit, onDone: () -> Unit, onTapZones: () -> Unit, tapZonesFocus: FocusRequester? = null) {
    val context = LocalContext.current
    val view = state.view ?: return
    val rows = state.settings?.rows.orEmpty()
    val novel = view.medium == Medium.NOVEL
    val first = if (novel) novelFirst else mangaFirst + Key.MODE + Key.DIRECTION
    val leftOut = if (novel) emptySet() else mangaLeftOut
    SheetFrame(Strings.readerSheetMore(context), onDone, Modifier.align(Alignment.BottomCenter), onBack = onBack) {
        if (rows.any { it.key == Key.TAP_PAGED }) {
            val value = tapZonesValue(context, state, eink = true)
            val focus = tapZonesFocus?.let { Modifier.focusRequester(it) } ?: Modifier
            EinkRow(onClick = onTapZones, height = EinkSizes.SheetRow, modifier = focus.semantics { if (value != null) stateDescription = value }) {
                Text(Strings.readerRowTap(context), style = EinkType.Label, modifier = Modifier.weight(1f))
                if (value != null) Text(value, style = EinkType.Secondary, modifier = Modifier.padding(end = 10.dp))
                Text("›", style = EinkType.Button)
            }
        }
        val rest = rows.filter { it.key !in first && it.key !in leftOut && it.key !in screenKeys && it.key !in tapKeys && einkRowTitle(context, it.key) != null }
        rest.forEach { EinkSettingLine(context, session, it) }
        val screen = rows.filter { it.key in screenKeys && it.key !in first }
        if (screen.isNotEmpty()) {
            SheetHeading(Strings.einkSheetScreen(context))
            screen.forEach { EinkSettingLine(context, session, it) }
        }
    }
}

/**
 * The Tap zones page: one radio row per layout with its diagram for the open book and what it does
 * in a line, then Zone size, the two flips, Swipes turn pages and Show zones. One choice serves all
 * reading on e-ink, strips too, since they turn a screen at a time.
 */
@Composable
fun BoxScope.EinkTapZonesSheet(session: ReaderSession, state: ReaderState, onBack: () -> Unit, onDone: () -> Unit, onShowZones: () -> Unit) {
    val context = LocalContext.current
    val rows = state.settings?.rows.orEmpty()
    val zones = state.tapZones(eink = true)
    val reading = state.tapReading() ?: return
    val scope = rows.firstOrNull { it.key == Key.TAP_PAGED }?.scope ?: SettingScope.DEVICE
    fun toggle(key: Key, on: Boolean) = session.setSetting(key, rows.firstOrNull { it.key == key }?.scope ?: SettingScope.DEVICE, ReaderValue.Toggle(on))
    SheetFrame(Strings.readerRowTap(context), onDone, Modifier.align(Alignment.BottomCenter), onBack = onBack, full = true) {
        Column(Modifier.fillMaxWidth().selectableGroup()) {
            state.zoneChoices(Key.TAP_PAGED).forEach { layout ->
                val on = layout == zones.paged
                val map = tapMap(zones, reading, layout)
                Row(
                    Modifier
                        .fillMaxWidth()
                        .heightIn(min = 54.dp)
                        .background(if (on) EinkColours.Selected else EinkColours.Paper)
                        .selectable(selected = on, interactionSource = remember { MutableInteractionSource() }, indication = einkFocus(), role = Role.RadioButton) {
                            session.setSetting(Key.TAP_PAGED, scope, ReaderValue.Choice(layout.configName))
                        }
                        // The layout and where its zones are; the radio's own state is read after it.
                        // The words replace the texts in the row, as Android drops a description on
                        // a row that merges them.
                        .clearAndSetSemantics { contentDescription = Strings.readerTapLayoutSpoken(context, layoutName(context, layout), map.about) }
                        .hairline(EinkColours.Ink)
                        .padding(horizontal = 12.dp, vertical = 6.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(14.dp),
                ) {
                    Box(Modifier.size(30.dp, 42.dp).border(1.5.dp, EinkColours.Ink)) {
                        TapZoneDiagram(map, Modifier.fillMaxSize(), eink = true)
                    }
                    Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                        Text(layoutName(context, layout), style = EinkType.Label.copy(fontSize = 16.sp, fontWeight = if (on) FontWeight.Bold else FontWeight.SemiBold))
                        Text(einkTapNote(context, layout, map, zones.swipesTurn), style = EinkType.Secondary)
                    }
                    EinkRadio(on)
                }
            }
        }
        rows.firstOrNull { it.key == Key.TAP_SIZE }?.let { size ->
            SheetLine(Strings.readerTapSize(context)) {
                EinkSegmented(
                    choices = listOf(ZoneSize.STANDARD, ZoneSize.SMALL),
                    chosen = zones.size,
                    label = { if (it == ZoneSize.SMALL) Strings.readerTapSmall(context) else Strings.readerTapStandard(context) },
                    onChoose = { session.setSetting(Key.TAP_SIZE, size.scope, ReaderValue.Choice(if (it == ZoneSize.SMALL) "small" else "standard")) },
                    modifier = Modifier.weight(1f),
                )
            }
        }
        if (rows.any { it.key == Key.TAP_FLIP_SIDES }) EinkSwitchLine(Strings.readerTapFlipSides(context), zones.flipSides) { toggle(Key.TAP_FLIP_SIDES, it) }
        if (rows.any { it.key == Key.TAP_FLIP_ENDS }) EinkSwitchLine(Strings.readerTapFlipEnds(context), zones.flipEnds) { toggle(Key.TAP_FLIP_ENDS, it) }
        if (rows.any { it.key == Key.SWIPE_TURNS }) EinkSwitchLine(Strings.einkSwipeTurns(context), zones.swipesTurn) { toggle(Key.SWIPE_TURNS, it) }
        // The guide cannot be read aloud, and Off has no zones to show.
        if (!rememberScreenReader() && zones.paged != ZoneLayout.OFF) {
            // The guide takes the page at once, so the press asks for no refresh of its own: the
            // sheet, the bars and the press all go in the guide's one refresh.
            CompositionLocalProvider(LocalEinkPaint provides {}) {
                EinkOutlinedButton(Strings.readerTapShow(context), onShowZones, Modifier.padding(top = 12.dp, bottom = 4.dp), height = 48.dp)
            }
        }
    }
}

/**
 * What a layout does on e-ink, in the line under its name, read from [map]'s cells as drawn for the
 * open book, so the line stays true at either size and with either flip. Off with swipes off too
 * leaves nothing on the page that turns it, which is allowed, and the line says so.
 */
internal fun einkTapNote(context: Context, layout: ZoneLayout, map: TapMap, swipes: Boolean): String {
    fun at(row: Int, column: Int) = map.cells.firstOrNull { it.row.toInt() == row && it.column.toInt() == column }?.action
    return when (layout) {
        ZoneLayout.AUTO -> Strings.einkTapAutoNote(context)
        ZoneLayout.SIDES -> {
            val left = at(1, 0) == TapAction.NEXT
            val next = map.cells.firstOrNull { it.action == TapAction.NEXT }?.width ?: 0.0
            val controls = map.cells.firstOrNull { it.action == TapAction.CONTROLS }?.width ?: 0.0
            when {
                next > controls + 0.01 && left -> Strings.einkTapSidesWideLeftNote(context)
                next > controls + 0.01 -> Strings.einkTapSidesWideRightNote(context)
                left -> Strings.einkTapSidesLeftNote(context)
                else -> Strings.einkTapSidesRightNote(context)
            }
        }
        ZoneLayout.L_SHAPE -> {
            val low = at(2, 1) == TapAction.NEXT
            val right = at(1, 2) == TapAction.NEXT
            when {
                low && right -> Strings.einkTapLShapeBottomRightNote(context)
                low -> Strings.einkTapLShapeBottomLeftNote(context)
                right -> Strings.einkTapLShapeTopRightNote(context)
                else -> Strings.einkTapLShapeTopLeftNote(context)
            }
        }
        ZoneLayout.WIDE_NEXT -> if (at(0, 1) == TapAction.CONTROLS) Strings.einkTapWideNextNote(context) else Strings.einkTapWideNextFlippedNote(context)
        ZoneLayout.TOP_BOTTOM -> if (at(2, 1) == TapAction.NEXT) Strings.einkTapTopBottomNote(context) else Strings.einkTapTopBottomFlippedNote(context)
        ZoneLayout.BOTH_NEXT -> if (at(2, 1) == TapAction.PREVIOUS) Strings.einkTapBothNextNote(context) else Strings.einkTapBothNextFlippedNote(context)
        ZoneLayout.OFF -> if (swipes) Strings.einkTapOffNote(context) else Strings.einkTapOffKeysNote(context)
    }
}

/** A radio: a 24 dp ring in ink, filled at the centre when chosen. */
@Composable
private fun EinkRadio(on: Boolean) {
    Box(Modifier.size(24.dp).border(EinkSizes.Rule, EinkColours.Ink, CircleShape), contentAlignment = Alignment.Center) {
        if (on) Box(Modifier.size(12.dp).background(EinkColours.Ink, CircleShape))
    }
}

/** A row with a switch: 52 by 30 dp, a 2 dp ink border, filled ink with a paper knob when on. */
@Composable
private fun EinkSwitchLine(title: String, on: Boolean, onChange: (Boolean) -> Unit) {
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 48.dp)
            .toggleable(value = on, interactionSource = remember { MutableInteractionSource() }, indication = einkFocus(), role = Role.Switch, onValueChange = onChange)
            .hairline(EinkColours.Ink)
            .padding(horizontal = 12.dp, vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(title, style = EinkType.Label.copy(fontSize = 16.sp, fontWeight = FontWeight.Bold), modifier = Modifier.weight(1f))
        Box(
            Modifier
                .size(52.dp, 30.dp)
                .background(if (on) EinkColours.Ink else EinkColours.Paper, CircleShape)
                .border(EinkSizes.Rule, EinkColours.Ink, CircleShape)
                .padding(5.dp),
            contentAlignment = if (on) Alignment.CenterEnd else Alignment.CenterStart,
        ) {
            Box(Modifier.size(20.dp).background(if (on) EinkColours.Paper else EinkColours.Ink, CircleShape))
        }
    }
}

/**
 * The sheet's frame: opaque paper with a 2 dp rule over it, its title and Done, then the rows. A
 * [full] one is a page over the whole screen instead.
 */
@Composable
private fun SheetFrame(
    title: String,
    onDone: () -> Unit,
    modifier: Modifier,
    onBack: (() -> Unit)? = null,
    /** A page of its own over the whole screen, for a page with too many rows to scroll on e-ink. */
    full: Boolean = false,
    content: @Composable ColumnScope.() -> Unit,
) {
    val context = LocalContext.current
    BoxWithConstraints(Modifier.fillMaxSize()) {
        val most = maxHeight * 0.75f
        Column(
            modifier
                .fillMaxWidth()
                .then(if (full) Modifier.fillMaxHeight() else Modifier.heightIn(max = most))
                .background(EinkColours.Paper)
                .then(if (full) Modifier.statusBarsPadding() else Modifier.topRule())
                .holdsTouches()
                .navigationBarsPadding()
                // A full page's header rule runs edge to edge, so there the sides are padded inside.
                .padding(horizontal = if (full) 0.dp else EinkSizes.SheetPadding, vertical = 12.dp),
        ) {
            val sides = if (full) Modifier.padding(horizontal = EinkSizes.SheetPadding) else Modifier
            Row(
                Modifier.fillMaxWidth().then(if (full) Modifier.bottomRule() else Modifier).then(sides).height(EinkSizes.SheetRow),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                if (onBack != null) {
                    EinkBackButton(onBack, 52.dp)
                }
                Text(title, style = EinkType.ReaderTitle, modifier = Modifier.weight(1f).semantics { heading() })
                EinkTextButton(Strings.actionDone(context), onDone)
            }
            Column(sides.verticalScroll(rememberScrollState())) { content() }
        }
    }
}

@Composable
private fun SheetHeading(text: String) {
    Text(
        text,
        style = EinkType.Secondary,
        modifier = Modifier.fillMaxWidth().padding(top = 14.dp, bottom = 4.dp).semantics { heading() },
    )
}

/** A row of the sheet: its name in the label column, the control beside it. */
@Composable
private fun SheetLine(title: String, content: @Composable RowScope.() -> Unit) {
    Row(Modifier.fillMaxWidth().heightIn(min = EinkSizes.SheetRow).padding(vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(title, style = EinkType.Label, modifier = Modifier.width(EinkSizes.SheetLabel).padding(end = 12.dp))
        content()
    }
}

/**
 * Reading mode for this series: Auto, Right to left, Left to right or Strip, and where it came
 * from under it. Auto takes the series details' mode back; the others are kept for the series.
 */
@Composable
private fun EinkModeLine(context: Context, session: ReaderSession, mode: ReaderSettingRow?, direction: ReaderSettingRow?, title: String) {
    if (mode == null) return
    val m = (mode.value as? ReaderValue.Choice)?.name
    val d = (direction?.value as? ReaderValue.Choice)?.name
    val auto = !mode.overridden && direction?.overridden != true
    val chosen = when {
        auto -> "auto"
        m == "strip" -> "strip"
        d == "ltr" -> "ltr"
        else -> "rtl"
    }
    SheetLine(Strings.readerRowReadingMode(context)) {
        EinkSegmented(
            choices = listOf("auto", "rtl", "ltr", "strip"),
            chosen = chosen,
            label = { choice ->
                when (choice) {
                    "auto" -> Strings.readerModeAutoName(context)
                    "rtl" -> Strings.readerModeRtl(context)
                    "ltr" -> Strings.readerModeLtr(context)
                    else -> Strings.readerModeStrip(context)
                }
            },
            onChoose = { choice ->
                when (choice) {
                    "auto" -> {
                        session.useDefault(Key.MODE)
                        session.useDefault(Key.DIRECTION)
                    }
                    "strip" -> session.setSetting(Key.MODE, SettingScope.SERIES, ReaderValue.Choice("strip"))
                    else -> {
                        session.setSetting(Key.MODE, SettingScope.SERIES, ReaderValue.Choice("paged"))
                        session.setSetting(Key.DIRECTION, SettingScope.SERIES, ReaderValue.Choice(choice))
                    }
                }
            },
            modifier = Modifier.weight(1f),
        )
    }
    Text(
        if (auto) Strings.readerModeDetected(context) else Strings.readerModeSet(context, title),
        style = EinkType.Tertiary,
        modifier = Modifier.padding(bottom = 4.dp),
    )
}

/** One setting as the e-ink sheet draws it: a segmented control, or a stepper for an amount. */
@Composable
private fun EinkSettingLine(context: Context, session: ReaderSession, row: ReaderSettingRow) {
    val title = einkRowTitle(context, row.key) ?: return
    val value = row.value
    fun set(to: ReaderValue) = session.setSetting(row.key, row.scope, to)
    SheetLine(title) {
        when {
            // The foot shows pages left or nothing: On and Off, as on the phone.
            row.key == Key.FOOT_VALUE && value is ReaderValue.Choice -> EinkSegmented(
                choices = listOf(true, false),
                chosen = value.name == "pages_left",
                label = { if (it) Strings.readerValueOn(context) else Strings.readerValueOff(context) },
                onChoose = { set(ReaderValue.Choice(if (it) "pages_left" else "none")) },
                modifier = Modifier.weight(1f),
            )
            value is ReaderValue.Toggle -> EinkSegmented(
                choices = listOf(true, false),
                chosen = value.on,
                label = { if (it) Strings.readerValueOn(context) else Strings.readerValueOff(context) },
                onChoose = { set(ReaderValue.Toggle(it)) },
                modifier = Modifier.weight(1f),
            )
            value is ReaderValue.Amount && (row.key == Key.LINE_SPACING || row.key == Key.MARGINS) -> {
                val presets = if (row.key == Key.LINE_SPACING) spacingPresets else marginPresets
                val labels = if (row.key == Key.LINE_SPACING) {
                    listOf(Strings.readerSpacingTight(context), Strings.readerSpacingNormal(context), Strings.readerSpacingLoose(context))
                } else {
                    listOf(Strings.readerMarginsNarrow(context), Strings.readerSpacingNormal(context), Strings.readerMarginsWide(context))
                }
                val at = presets.indices.minBy { abs(presets[it] - value.value) }
                EinkSegmented(presets.indices.toList(), at, { labels[it] }, { set(ReaderValue.Amount(presets[it])) }, Modifier.weight(1f))
            }
            value is ReaderValue.Amount -> {
                val steps = amountSteps(row.key)
                val words = if (steps.pages && value.value.roundToInt() == 0) {
                    // No full refresh: the device decides.
                    Strings.readerValueOff(context)
                } else if (steps.pages) {
                    Strings.einkRefreshEvery(context, value.value.roundToInt())
                } else {
                    Strings.einkPercent(context, (value.value * 100).roundToInt())
                }
                fun step(by: Int) {
                    val next = ((value.value + by * steps.step) / steps.step).roundToInt() * steps.step
                    set(ReaderValue.Amount(next.coerceIn(steps.least, steps.most)))
                }
                val text = row.key == Key.TEXT_SIZE
                EinkStepper(
                    words = words,
                    title = title,
                    down = if (text) Strings.readerSizeSmall(context) else Strings.einkStepDown(context),
                    up = if (text) Strings.readerSizeLarge(context) else Strings.einkStepUp(context),
                    canDown = value.value > steps.least + 1e-6,
                    canUp = value.value < steps.most - 1e-6,
                    onDown = { step(-1) },
                    onUp = { step(1) },
                    modifier = Modifier.weight(1f),
                )
            }
            row.choices.isNotEmpty() -> {
                // On paper the looks are Paper and Dark.
                val offered = if (row.key == Key.LOOK) row.choices.filter { (it as? ReaderValue.Choice)?.name in setOf("paper", "dark") } else row.choices
                val labelled = offered.mapNotNull { choice -> einkChoiceLabel(context, row.key, choice)?.let { choice to it } }
                if (labelled.size <= 4) {
                    EinkSegmented(labelled.map { it.first }, value, { choice -> labelled.first { it.first == choice }.second }, { set(it) }, Modifier.weight(1f))
                } else {
                    // Too many for one control: the chosen one between arrows, which stop at the
                    // first and the last choice.
                    val at = labelled.indexOfFirst { it.first == value }.coerceAtLeast(0)
                    EinkStepper(
                        words = labelled[at].second,
                        title = title,
                        down = Strings.einkStepPrevious(context),
                        up = Strings.einkStepNext(context),
                        canDown = at > 0,
                        canUp = at < labelled.lastIndex,
                        onDown = { if (at > 0) set(labelled[at - 1].first) },
                        onUp = { if (at < labelled.lastIndex) set(labelled[at + 1].first) },
                        modifier = Modifier.weight(1f),
                        glyphs = "‹" to "›",
                    )
                }
            }
        }
    }
    // Which key goes forward, under the switch that lets them turn pages: the phone's own choice.
    if (row.key == Key.VOLUME_KEYS_TURN && (value as? ReaderValue.Toggle)?.on == true) {
        SheetLine(Strings.readerRowVolumeSwap(context)) {
            EinkSegmented(
                choices = listOf(true, false),
                chosen = ReaderLook.volumeSwapped(context),
                label = { if (it) Strings.readerValueOn(context) else Strings.readerValueOff(context) },
                onChoose = { ReaderLook.swapVolume(context, it) },
                modifier = Modifier.weight(1f),
            )
        }
    }
}

/** Less and more around the value, the value between them as the stepper's state. */
@Composable
private fun EinkStepper(
    words: String,
    title: String,
    down: String,
    up: String,
    canDown: Boolean,
    canUp: Boolean,
    onDown: () -> Unit,
    onUp: () -> Unit,
    modifier: Modifier = Modifier,
    glyphs: Pair<String, String> = "−" to "+",
) {
    Row(modifier.semantics { stateDescription = words }, verticalAlignment = Alignment.CenterVertically) {
        StepButton(glyphs.first, down, canDown, words, onDown)
        Text(
            words,
            style = EinkType.Label.copy(fontWeight = FontWeight.Bold, fontFeatureSettings = "tnum"),
            textAlign = TextAlign.Center,
            modifier = Modifier.weight(1f).semantics { contentDescription = "$title, $words" }, // i18n-skip
        )
        StepButton(glyphs.second, up, canUp, words, onUp)
    }
}

@Composable
private fun StepButton(glyph: String, description: String, enabled: Boolean, value: String, onClick: () -> Unit) {
    val ink = if (enabled) EinkColours.Ink else EinkColours.PlaceholderEdge
    val shape = RoundedCornerShape(EinkSizes.SegmentRadius)
    // Held until the engine's answer shows the new value.
    val press = rememberEinkPress(result = value)
    Box(
        Modifier
            .size(EinkSizes.SegmentHeight + 8.dp, EinkSizes.SegmentHeight)
            .clip(shape)
            .einkInverted(press.inverted)
            .border(EinkSizes.Rule, ink, shape)
            .semantics { contentDescription = description }
            .einkPress(press, enabled = enabled, shape = shape, inset = EinkSizes.FocusInsideBorder, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Text(glyph, style = EinkType.Button.copy(color = ink))
    }
}

private class Steps(val step: Double, val least: Double, val most: Double, val pages: Boolean = false)

private fun amountSteps(key: Key): Steps = when (key) {
    Key.TEXT_SIZE -> Steps(0.1, 0.7, 2.0)
    // Off, then every page to every 20th.
    Key.REFRESH_TEXT, Key.REFRESH_LIST -> Steps(1.0, 0.0, 20.0, pages = true)
    Key.SATURATION, Key.CONTRAST -> Steps(0.1, 0.5, 1.5)
    else -> Steps(0.1, 0.5, 2.0)
}

private val spacingPresets = listOf(1.3, 1.6, 1.9)
private val marginPresets = listOf(0.6, 1.0, 1.5)

/** The row's name on e-ink: the reader's own, or the name of a row only an e-ink screen has. */
fun einkRowTitle(context: Context, key: Key): String? = when (key) {
    Key.VOLUME_KEYS_TURN -> Strings.readerRowVolume(context)
    Key.REFRESH_IMAGE -> Strings.einkRowRefreshImage(context)
    Key.REFRESH_TEXT -> Strings.einkRowRefreshText(context)
    Key.REFRESH_LIST -> Strings.einkRowRefreshList(context)
    Key.SCREEN_MODE -> Strings.einkRowScreenMode(context)
    Key.RAINBOW_FILTER -> Strings.einkRowRainbow(context)
    Key.SATURATION -> Strings.einkRowSaturation(context)
    Key.CONTRAST -> Strings.einkRowContrast(context)
    else -> rowTitle(context, key)
}

/** The choice's name on e-ink: the reader's own, or one of Screen mode's. */
fun einkChoiceLabel(context: Context, key: Key, value: ReaderValue): String? {
    if (key != Key.SCREEN_MODE) return choiceLabel(context, key, value)
    return when ((value as? ReaderValue.Choice)?.name) {
        "device" -> Strings.einkModeDevice(context)
        "hd" -> Strings.einkModeHd(context)
        "regal" -> Strings.einkModeRegal(context)
        "balanced" -> Strings.einkModeBalanced(context)
        "standard" -> Strings.einkSpeedStandard(context)
        "fast" -> Strings.einkSpeedFast(context)
        "best" -> Strings.einkSpeedBest(context)
        else -> null
    }
}

/** A line of Contents: a volume's heading, or a chapter. */
private sealed interface ContentsLine {
    data class Heading(val words: String) : ContentsLine
    data class Chapter(val row: ChapterRow) : ContentsLine
}

/**
 * Contents: the series' chapters as a paged list under a bar with Back, opened on the page with
 * the chapter being read. A chapter opens in the reader; Back returns to the page. [onPage] paints
 * each page of the list.
 */
@Composable
fun EinkContents(
    session: ReaderSession,
    state: ReaderState,
    onBack: () -> Unit,
    onOpen: (ChapterId) -> Unit,
    onPage: () -> Unit,
    keys: ContentsKeys = remember { ContentsKeys() },
) {
    val context = LocalContext.current
    val view = state.view ?: return
    LaunchedEffect(view.work) { session.loadChapters() }
    val current = state.chapter
    Column(Modifier.fillMaxSize().background(EinkColours.Paper).holdsTouches()) {
        Row(
            Modifier.fillMaxWidth().background(EinkColours.Paper).bottomRule().statusBarsPadding().height(EinkSizes.TopBar).padding(horizontal = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            EinkBackButton(onBack, 56.dp)
            Column(Modifier.weight(1f).padding(horizontal = 8.dp)) {
                Text(Strings.einkContents(context), style = EinkType.ReaderTitle, maxLines = 1, modifier = Modifier.semantics { heading() })
                Text(view.title, style = EinkType.Secondary, maxLines = 1)
            }
        }
        val volumes = state.volumes
        if (volumes.isEmpty()) {
            PaperWaiting {
                Text(
                    Strings.readerChaptersLoading(context),
                    style = EinkType.Waiting,
                    modifier = Modifier.fillMaxWidth().padding(24.dp),
                    textAlign = TextAlign.Center,
                )
            }
            return@Column
        }
        val lines = remember(volumes) {
            volumes.flatMap { group ->
                val heading = group.volume?.let { Strings.readerVolumeLine(context, it) } ?: Strings.readerVolumeNone(context)
                // A flat list is the chapters alone, with no heading.
                val head = if (group.flat) emptyList() else listOf<ContentsLine>(ContentsLine.Heading(heading))
                head + group.chapters.map { ContentsLine.Chapter(it) }
            }
        }
        BoxWithConstraints(Modifier.fillMaxWidth().weight(1f).navigationBarsPadding()) {
            val perPage = rowsThatFit(maxHeight, EinkSizes.ChapterRow)
            val pages = remember(lines, perPage) { pagesOf(lines, perPage) }
            val here = lines.indexOfFirst { it is ContentsLine.Chapter && it.row.id == current }.coerceAtLeast(0)
            var page by remember(pages.size) { mutableIntStateOf((here / perPage).coerceIn(0, pages.size - 1)) }
            // TalkBack and the keyboard start on the chapter being read, once, as the page opens.
            val focus = remember { FocusRequester() }
            var focused by remember { mutableStateOf(false) }
            // A turned page drops the row that had the focus, and a pager arrow at the end of the
            // list stops taking it. Then the focus goes to the first chapter on the page while keys
            // are in use, or else back to the reader, so the reader's keys always reach the list.
            val firstRow = remember { FocusRequester() }
            val holds = remember { ListFocus() }
            LaunchedEffect(page) {
                onPage()
                withFrameNanos { }
                if (!holds.held && !firstRow.tryFocus()) keys.lost()
            }
            // The page buttons and volume keys turn the list's pages while it shows, and the D-pad
            // from the reader comes to the chapter being read or the page's first.
            val onThisPage = pages[page].any { it is ContentsLine.Chapter && it.row.id == current }
            SideEffect {
                keys.turn = { by -> page = (page + by).coerceIn(0, pages.size - 1) }
                keys.into = { (onThisPage && focus.tryFocus()) || firstRow.tryFocus() }
            }
            val first = pages[page].firstOrNull { it is ContentsLine.Chapter }
            Column(Modifier.fillMaxSize().onFocusChanged { holds.held = it.hasFocus }) {
                Column(Modifier.weight(1f)) {
                    pages[page].forEach { line ->
                        when (line) {
                            is ContentsLine.Heading -> Text(
                                line.words,
                                style = EinkType.Secondary,
                                modifier = Modifier.fillMaxWidth().height(EinkSizes.ChapterRow).padding(horizontal = 20.dp).padding(top = 28.dp).semantics { heading() },
                            )
                            is ContentsLine.Chapter -> {
                                val reading = line.row.id == current
                                if (reading && !focused) {
                                    LaunchedEffect(Unit) {
                                        runCatching { focus.requestFocus() }
                                        focused = true
                                    }
                                }
                                val firstHere = if (line === first) Modifier.focusRequester(firstRow) else Modifier
                                EinkRow(
                                    onClick = { onOpen(line.row.id) },
                                    modifier = if (reading) firstHere.focusRequester(focus) else firstHere,
                                    selected = reading,
                                    height = EinkSizes.ChapterRow,
                                ) {
                                    Text(
                                        chapterWords(context, line.row),
                                        style = if (reading) EinkType.Label.copy(fontWeight = FontWeight.Bold) else EinkType.Label,
                                        maxLines = 1,
                                        modifier = Modifier.weight(1f),
                                    )
                                    if (reading) Text(Strings.einkContentsReading(context), style = EinkType.Secondary)
                                }
                            }
                        }
                    }
                }
                if (pages.size > 1) EinkPager(page, pages.size, onPage = { page = it.coerceIn(0, pages.size - 1) })
            }
        }
    }
}

/**
 * The reader's keys while Contents shows: its pages turn, not the reader's page under it. [lost]
 * gives the focus back to the reader when nothing in the list can hold it.
 */
class ContentsKeys(internal val lost: () -> Unit = {}) {
    /** Turn [by] pages of the list, -1 back and 1 on. */
    internal var turn: (by: Int) -> Unit = {}

    /** Puts the focus on the chapter being read, or else the page's first; false when it cannot go there. */
    internal var into: () -> Boolean = { false }

    fun forward() = turn(1)

    fun back() = turn(-1)

    /** The D-pad from the reader: the focus goes into the list. False when it cannot. */
    fun enter(): Boolean = into()
}

/** Whether a row or the pager in Contents holds the focus. */
private class ListFocus {
    var held = false
}

/** Touches here stay here, never reaching the page under it. */
internal fun Modifier.holdsTouches(): Modifier = pointerInput(Unit) { detectTapGestures { } }
