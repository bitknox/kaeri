// The reader's settings sheet: this series' reading mode (manga and strips only), the medium's
// other choices (a novel's size, font, look as five swatches, spacing, margins and pages), Paper
// mode and its parts on a page of their own, the tap zones on theirs, and the rest on one
// searchable page. The engine
// keeps every value; a change goes out as a command and the sheet draws the settings the engine
// sends back. Every choice and row has a written name; one without a name is not drawn, and the
// unit test fails on it.

package app.kaeri.reader.phone

import android.content.Context
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.foundation.text.TextAutoSize
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.layout.size
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.withLink
import app.kaeri.engine.model.ReaderMode
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.IconButton
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.minimumInteractiveComponentSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.ReaderSettingKey as Key
import app.kaeri.engine.model.ReaderSettingRow
import app.kaeri.engine.model.ReaderValue
import app.kaeri.engine.model.SettingScope
import app.kaeri.i18n.Strings
import app.kaeri.reader.ReaderSession
import app.kaeri.reader.ReaderState
import app.kaeri.reader.layoutName
import app.kaeri.reader.zoneLayout
import kotlin.math.abs

/** The sheet's pages; the configuration of the Quiet rule opens it on Paper. */
enum class SettingsPane { Main, Mode, More, Paper, Font, TapZones }

private typealias Pane = SettingsPane

/** The rows a phone has: the e-ink panel's own are left to its reader. */
internal val phoneKeys = listOf(
    Key.MODE, Key.DIRECTION, Key.COLOUR, Key.FIT, Key.BACKGROUND, Key.CROP_BORDERS, Key.PAGE_LAYOUT, Key.SPREADS,
    Key.LOOK, Key.FONT, Key.TEXT_SIZE, Key.MARGINS, Key.LINE_SPACING, Key.JUSTIFY,
    Key.TAP_PAGED, Key.TAP_SCROLL, Key.TAP_SIZE, Key.TAP_FLIP_SIDES, Key.TAP_FLIP_ENDS, Key.VOLUME_KEYS_TURN, Key.PAPER_MODE, Key.GREYSCALE_PAGES, Key.QUIET_PHONE, Key.FOOT_VALUE, Key.FOLIO,
    Key.STRIP_GAP, Key.PAPER_INSTANT, Key.PAPER_LOOK, Key.PAPER_DIM, Key.PAPER_VOLUME,
)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ReaderSettingsSheet(
    session: ReaderSession,
    state: ReaderState,
    onDismiss: () -> Unit,
    toggle: (Key, Boolean) -> Unit = { key, on -> session.setSetting(key, SettingScope.DEVICE, ReaderValue.Toggle(on)) },
    /** Paper mode: the sheet comes and goes at once. */
    instant: Boolean = false,
    /** The page it opens on. */
    start: SettingsPane = SettingsPane.Main,
    /** Do Not Disturb access was refused the last time Quiet asked for it. */
    quietDenied: Boolean = false,
    /** Show zones on the Tap zones page: the sheet closes and the guide shows. */
    onShowZones: () -> Unit = {},
) {
    val context = LocalContext.current
    val view = state.view ?: return
    val settings = state.settings
    var pane by remember { mutableStateOf(start) }
    // Paper's page goes back to where it was opened from.
    var paperBack by remember { mutableStateOf(Pane.Main) }
    val rows = settings?.rows.orEmpty().filter { it.key in phoneKeys }
    val novel = view.medium == Medium.NOVEL
    val strip = view.mode == ReaderMode.STRIP || view.mode == ReaderMode.VERTICAL
    // The Tap zones page runs past a half-open sheet: it opens the sheet all the way.
    // Back from a page of the sheet goes to the page it came from, not out of the sheet.
    val back = {
        val from = pane
        pane = when (from) {
            Pane.Main -> Pane.Main
            Pane.Paper -> paperBack
            Pane.TapZones -> Pane.More
            else -> Pane.Main
        }
        from != Pane.Main
    }
    ReaderSheet(instant = instant, onDismiss = onDismiss, expanded = pane == Pane.TapZones, onBack = back) {
        Column(Modifier.verticalScroll(rememberScrollState()).navigationBarsPadding().padding(horizontal = 20.dp, vertical = 4.dp)) {
            when (pane) {
                Pane.Main -> Main(
                    context, session, rows, novel, strip, view.title, toggle,
                    onMode = { pane = Pane.Mode }, onPaper = { paperBack = Pane.Main; pane = Pane.Paper }, onMore = { pane = Pane.More }, onFont = { pane = Pane.Font },
                )
                Pane.Mode -> ModePicker(context, session, rows, view.title) { pane = Pane.Main }
                Pane.More -> More(context, session, state, rows, novel, toggle, onPaper = { paperBack = Pane.More; pane = Pane.Paper }, onTapZones = { pane = Pane.TapZones }) { pane = Pane.Main }
                Pane.TapZones -> TapZonesPage(context, session, state, rows, toggle, onShowZones) { pane = Pane.More }
                Pane.Paper -> PaperPage(context, session, rows, novel, toggle, quietDenied) { pane = paperBack }
                Pane.Font -> FontPicker(context, session, rows) { pane = Pane.Main }
            }
            Spacer(Modifier.height(16.dp))
        }
    }
}

@Composable
private fun Heading(text: String) {
    Text(text, color = Chrome.faint, fontSize = 13.sp, fontWeight = FontWeight.SemiBold, modifier = Modifier.padding(top = 14.dp, bottom = 4.dp))
}

@Composable
internal fun BackRow(title: String, context: Context, onBack: () -> Unit) {
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        IconButton(onClick = onBack) {
            GlyphIcon(Glyph.Back, Chrome.text, mirror = androidx.compose.ui.platform.LocalLayoutDirection.current == androidx.compose.ui.unit.LayoutDirection.Rtl)
        }
        Text(title, color = Chrome.text, fontSize = 18.sp, fontWeight = FontWeight.Medium, modifier = Modifier.semantics { contentDescription = Strings.readerSettingsBack(context) + ", " + title })
    }
}

/** A row at least 48 dp tall with a value and a chevron. */
@Composable
private fun NavRow(title: String, value: String?, onClick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().heightIn(min = 48.dp).clip(RoundedCornerShape(12.dp)).clickable(onClick = onClick),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(title, color = Chrome.text, fontSize = 16.sp, modifier = Modifier.weight(1f))
        if (value != null) Text(value, color = Chrome.faint, fontSize = 15.sp)
        GlyphIcon(Glyph.Chevron, Chrome.faint, size = 20.dp)
    }
}

@Composable
private fun Main(
    context: Context,
    session: ReaderSession,
    rows: List<ReaderSettingRow>,
    novel: Boolean,
    strip: Boolean,
    title: String,
    toggle: (Key, Boolean) -> Unit,
    onMode: () -> Unit,
    onPaper: () -> Unit,
    onMore: () -> Unit,
    onFont: () -> Unit,
) {
    val mode = rows.firstOrNull { it.key == Key.MODE }
    val direction = rows.firstOrNull { it.key == Key.DIRECTION }
    fun row(key: Key) = rows.firstOrNull { it.key == key }
    if (novel) {
        // Novels have no mode or direction per series: one group, as the board has it.
        Heading(Strings.readerSheetAllNovels(context))
        row(Key.TEXT_SIZE)?.let { SizeSlider(context, session, it) }
        row(Key.FONT)?.let { font ->
            NavRow(Strings.readerRowFont(context), choiceLabel(context, Key.FONT, font.value), onFont)
        }
        row(Key.LOOK)?.let { LookSwatches(context, session, it) }
        listOf(Key.LINE_SPACING, Key.MARGINS, Key.MODE).forEach { key -> row(key)?.let { SettingLine(context, session, it, toggle, novel = true) } }
    } else {
        Heading(Strings.readerSheetSeries(context))
        NavRow(Strings.readerRowReadingMode(context), modeSummary(context, mode, direction), onMode)
        ModeNote(context, session, mode, direction, title)
        Heading(if (strip) Strings.readerSheetAllWebtoons(context) else Strings.readerSheetAllManga(context))
        // A strip has no fit or borders to crop; its images can have a gap instead.
        val keys = if (strip) listOf(Key.STRIP_GAP, Key.BACKGROUND) else listOf(Key.FIT, Key.CROP_BORDERS, Key.BACKGROUND)
        keys.forEach { key -> row(key)?.let { SettingLine(context, session, it, toggle) } }
        val paper = row(Key.PAPER_MODE)
        if (paper != null) {
            val on = (paper.value as? ReaderValue.Toggle)?.on == true
            NavRow(Strings.readerRowPaper(context), if (on) Strings.readerValueOn(context) else Strings.readerValueOff(context), onPaper)
        }
    }
    Spacer(Modifier.height(6.dp))
    NavRow(Strings.readerSheetMore(context), null, onMore)
}

/** The text size between a small and a large serif A, with no title, as the board has it. */
@Composable
private fun SizeSlider(context: Context, session: ReaderSession, row: ReaderSettingRow) {
    val value = row.value as? ReaderValue.Amount ?: return
    Row(Modifier.fillMaxWidth().padding(top = 8.dp, bottom = 10.dp), verticalAlignment = Alignment.CenterVertically) {
        SizeStep(14.sp, Strings.readerSizeSmall(context), value.value > sizeRange.start + 0.05) { stepSize(session, row, value, -1) }
        Slider(
            value = value.value.toFloat().coerceIn(sizeRange),
            onValueChange = { session.setSetting(row.key, row.scope, ReaderValue.Amount(((it * 10).toInt() / 10.0))) },
            valueRange = sizeRange,
            steps = 12,
            colors = SliderDefaults.colors(
                thumbColor = Chrome.text,
                activeTrackColor = Chrome.text,
                inactiveTrackColor = Chrome.rule,
                activeTickColor = Color.Transparent,
                inactiveTickColor = Color.Transparent,
            ),
            modifier = Modifier.weight(1f).padding(horizontal = 14.dp).semantics {
                contentDescription = Strings.readerRowTextSize(context)
                stateDescription = "%.1f".format(value.value)
            },
        )
        SizeStep(24.sp, Strings.readerSizeLarge(context), value.value < sizeRange.endInclusive - 0.05) { stepSize(session, row, value, 1) }
    }
    UseDefaultNote(context, null, row.overridden) { session.useDefault(row.key) }
}

/** The small or large A beside the size slider: a button that steps the size by a tenth. */
@Composable
private fun SizeStep(size: androidx.compose.ui.unit.TextUnit, description: String, enabled: Boolean, onClick: () -> Unit) {
    Box(
        Modifier
            .size(48.dp)
            .clip(CircleShape)
            .clickable(enabled = enabled, role = Role.Button, onClickLabel = description, onClick = onClick)
            .semantics { contentDescription = description },
        contentAlignment = Alignment.Center,
    ) {
        Text("A", color = if (enabled) Chrome.text else Chrome.faint, fontSize = size, fontFamily = FontFamily.Serif, fontWeight = FontWeight.SemiBold)
    }
}

private fun stepSize(session: ReaderSession, row: ReaderSettingRow, value: ReaderValue.Amount, by: Int) {
    val next = (Math.round(value.value * 10) + by) / 10.0
    session.setSetting(row.key, row.scope, ReaderValue.Amount(next.coerceIn(sizeRange.start.toDouble(), sizeRange.endInclusive.toDouble())))
}

/** The five looks as round "Aa" swatches in their own paper and ink, the chosen one ringed in silk. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun LookSwatches(context: Context, session: ReaderSession, row: ReaderSettingRow) {
    val chosen = (row.value as? ReaderValue.Choice)?.name
    val order = listOf("paper", "light", "sepia", "dark", "system")
    val looks = order.filter { name -> row.choices.any { (it as? ReaderValue.Choice)?.name == name } }
    // The five in one row; at the largest text sizes the last ones go to a second row whole.
    FlowRow(
        Modifier.fillMaxWidth().padding(top = 6.dp, bottom = 10.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        looks.forEach { name ->
            val selected = name == chosen
            val label = choiceLabel(context, Key.LOOK, ReaderValue.Choice(name)).orEmpty()
            Column(
                Modifier
                    .clip(RoundedCornerShape(12.dp))
                    .selectable(selected = selected, role = Role.RadioButton) { session.setSetting(row.key, row.scope, ReaderValue.Choice(name)) }
                    .padding(4.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                LookSwatch(name, selected)
                Text(
                    label,
                    color = if (selected) Chrome.text else Chrome.faint,
                    fontSize = 12.sp,
                    fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Normal,
                    maxLines = 1,
                )
            }
        }
    }
    UseDefaultNote(context, null, row.overridden) { session.useDefault(row.key) }
}

/** The board's look colours: paper, ink. */
private val lookColours = mapOf(
    "paper" to (Color(0xFFEFE9DC) to Color(0xFF2A2620)),
    "light" to (Color(0xFFF7F5F0) to Color(0xFF1E2033)),
    "sepia" to (Color(0xFFE9DCC4) to Color(0xFF3B2F22)),
    "dark" to (Color(0xFF161A2E) to Color(0xFFD9D5CB)),
)

@Composable
private fun LookSwatch(name: String, selected: Boolean) {
    val ring = Chrome.text
    val gap = Chrome.plate
    val rule = Chrome.rule
    Box(
        Modifier
            .size(54.dp)
            .drawBehind {
                // Selected: a 2 dp gap in the sheet's colour, then a 2 dp silk ring.
                if (selected) drawCircle(ring, radius = size.minDimension / 2)
                drawCircle(gap, radius = size.minDimension / 2 - 2.dp.toPx())
            },
        contentAlignment = Alignment.Center,
    ) {
        Box(Modifier.size(46.dp).clip(CircleShape)) {
            if (name == "system") {
                // System: light on the left half, dark on the right.
                Half(lookColours.getValue("dark"), Modifier.fillMaxSize())
                Half(lookColours.getValue("light"), Modifier.fillMaxSize().clipLeftHalf())
            } else {
                Half(lookColours[name] ?: lookColours.getValue("dark"), Modifier.fillMaxSize())
            }
            Box(Modifier.fillMaxSize().border(1.dp, rule, CircleShape))
        }
    }
}

@Composable
private fun Half(colours: Pair<Color, Color>, modifier: Modifier) {
    Box(modifier.background(colours.first), contentAlignment = Alignment.Center) {
        Text("Aa", color = colours.second, fontSize = 16.sp, fontFamily = FontFamily.Serif, fontWeight = FontWeight.SemiBold)
    }
}

private fun Modifier.clipLeftHalf(): Modifier = drawWithContent {
    clipRect(right = size.width / 2) { this@drawWithContent.drawContent() }
}

/** The font's own page: each font as a row with a check. */
@Composable
private fun FontPicker(context: Context, session: ReaderSession, rows: List<ReaderSettingRow>, onBack: () -> Unit) {
    val row = rows.firstOrNull { it.key == Key.FONT } ?: return
    BackRow(Strings.readerRowFont(context), context, onBack)
    row.choices.forEach { choice ->
        val label = choiceLabel(context, Key.FONT, choice) ?: return@forEach
        ModeOption(label, null, choice == row.value) { session.setSetting(row.key, row.scope, choice) }
    }
    UseDefaultNote(context, null, row.overridden) { session.useDefault(row.key) }
}

private fun modeSummary(context: Context, mode: ReaderSettingRow?, direction: ReaderSettingRow?): String {
    val m = (mode?.value as? ReaderValue.Choice)?.name
    val d = (direction?.value as? ReaderValue.Choice)?.name
    val way = when {
        m == "strip" -> Strings.readerModeStrip(context)
        m == "vertical" -> Strings.readerModeVertical(context)
        d == "ltr" -> Strings.readerModeLtr(context)
        else -> Strings.readerModeRtl(context)
    }
    val auto = mode?.detected == true || direction?.detected == true
    return if (auto) Strings.readerModeAutoWay(context, way) else way
}

/** Under the mode row: where the mode came from, with Use default inline when the series has its own. */
@Composable
private fun ModeNote(context: Context, session: ReaderSession, mode: ReaderSettingRow?, direction: ReaderSettingRow?, title: String) {
    val overridden = mode?.overridden == true || direction?.overridden == true
    UseDefaultNote(context, if (overridden) Strings.readerModeSet(context, title) else Strings.readerModeDetected(context), overridden) {
        session.useDefault(Key.MODE)
        session.useDefault(Key.DIRECTION)
    }
}

/**
 * A note in haze, 13 on 18, with Use default underlined at its end when [overridden]: one line of
 * text, not a button with its own padding. With no [note], Use default stands alone.
 */
@Composable
private fun UseDefaultNote(context: Context, note: String?, overridden: Boolean, onUseDefault: () -> Unit) {
    if (note == null && !overridden) return
    val silk = Chrome.text
    val text = buildAnnotatedString {
        if (note != null) append(note)
        if (overridden) {
            if (note != null) append(" ")
            withLink(
                LinkAnnotation.Clickable(
                    "use-default",
                    TextLinkStyles(SpanStyle(color = silk, fontWeight = FontWeight.SemiBold, textDecoration = TextDecoration.Underline)),
                ) { onUseDefault() },
            ) { append(Strings.readerModeUseDefault(context)) }
        }
    }
    Text(
        text,
        color = Chrome.faint,
        fontSize = 13.sp,
        lineHeight = 18.sp,
        modifier = Modifier.fillMaxWidth().padding(top = 2.dp, bottom = 8.dp),
    )
}

@Composable
private fun ModePicker(context: Context, session: ReaderSession, rows: List<ReaderSettingRow>, title: String, onBack: () -> Unit) {
    val mode = rows.firstOrNull { it.key == Key.MODE }
    val direction = rows.firstOrNull { it.key == Key.DIRECTION }
    val m = (mode?.value as? ReaderValue.Choice)?.name
    val d = (direction?.value as? ReaderValue.Choice)?.name
    val auto = mode?.overridden != true && direction?.overridden != true
    BackRow(Strings.readerRowReadingMode(context), context, onBack)
    val way = when {
        m == "strip" -> Strings.readerModeStrip(context)
        m == "vertical" -> Strings.readerModeVertical(context)
        d == "ltr" -> Strings.readerModeLtr(context)
        else -> Strings.readerModeRtl(context)
    }
    fun series(mode: String, direction: String?) {
        session.setSetting(Key.MODE, SettingScope.SERIES, ReaderValue.Choice(mode))
        if (direction != null) session.setSetting(Key.DIRECTION, SettingScope.SERIES, ReaderValue.Choice(direction))
    }
    ModeOption(Strings.readerModeAutoName(context), Strings.readerModeAutoLine(context, way), auto) {
        session.useDefault(Key.MODE)
        session.useDefault(Key.DIRECTION)
    }
    ModeOption(Strings.readerModeRtl(context), null, !auto && m == "paged" && d != "ltr") { series("paged", "rtl") }
    ModeOption(Strings.readerModeLtr(context), null, !auto && m == "paged" && d == "ltr") { series("paged", "ltr") }
    ModeOption(Strings.readerModeVertical(context), Strings.readerModeVerticalLine(context), !auto && m == "vertical") { series("vertical", null) }
    ModeOption(Strings.readerModeStrip(context), Strings.readerModeStripLine(context), !auto && m == "strip") { series("strip", null) }
    Spacer(Modifier.height(10.dp))
    Text(
        if (auto) Strings.readerModeDetected(context) else Strings.readerModeSaved(context),
        color = Chrome.faint,
        fontSize = 13.sp,
    )
}

/** A list row with a trailing check on the selected one. */
@Composable
private fun ModeOption(name: String, line: String?, selected: Boolean, onClick: () -> Unit) {
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 56.dp)
            .clip(RoundedCornerShape(12.dp))
            .background(if (selected) Chrome.selected else Color.Transparent)
            .clickable(onClick = onClick)
            .padding(horizontal = 12.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(name, color = Chrome.text, fontSize = 16.sp, fontWeight = FontWeight.Medium)
            if (line != null) Text(line, color = Chrome.faint, fontSize = 13.sp)
        }
        if (selected) GlyphIcon(Glyph.Check, Chrome.text)
    }
}

/**
 * Paper mode's page: the switch, what it does, then each part it changes, on by default except
 * Greyscale pages and Quiet. A part only acts while Paper mode is on.
 */
@Composable
private fun PaperPage(
    context: Context,
    session: ReaderSession,
    rows: List<ReaderSettingRow>,
    novel: Boolean,
    toggle: (Key, Boolean) -> Unit,
    quietDenied: Boolean,
    onBack: () -> Unit,
) {
    BackRow(Strings.readerRowPaper(context), context, onBack)
    fun row(key: Key) = rows.firstOrNull { it.key == key }
    row(Key.PAPER_MODE)?.let { paper ->
        SwitchLine(Strings.readerRowPaper(context), null, (paper.value as? ReaderValue.Toggle)?.on == true, big = true) { toggle(Key.PAPER_MODE, it) }
    }
    Text(Strings.readerPaperLine(context), color = Chrome.faint, fontSize = 13.sp, lineHeight = 18.sp, modifier = Modifier.padding(top = 2.dp, bottom = 8.dp))
    Heading(Strings.readerPaperParts(context))
    val parts = if (novel) {
        listOf(Key.PAPER_INSTANT, Key.PAPER_LOOK, Key.PAPER_DIM, Key.PAPER_VOLUME, Key.QUIET_PHONE)
    } else {
        listOf(Key.PAPER_INSTANT, Key.PAPER_LOOK, Key.PAPER_DIM, Key.GREYSCALE_PAGES, Key.PAPER_VOLUME, Key.QUIET_PHONE)
    }
    parts.forEach { key -> row(key)?.let { SettingLine(context, session, it, toggle) } }
    if (quietDenied) {
        Text(Strings.readerQuietDenied(context), color = Chrome.error, fontSize = 13.sp, lineHeight = 18.sp, modifier = Modifier.padding(bottom = 8.dp))
    }
}

@Composable
private fun More(
    context: Context,
    session: ReaderSession,
    state: ReaderState,
    rows: List<ReaderSettingRow>,
    novel: Boolean,
    toggle: (Key, Boolean) -> Unit,
    onPaper: () -> Unit,
    onTapZones: () -> Unit,
    onBack: () -> Unit,
) {
    BackRow(Strings.readerSheetMore(context), context, onBack)
    var query by remember { mutableStateOf("") }
    Row(
        Modifier.fillMaxWidth().heightIn(min = 48.dp).clip(RoundedCornerShape(24.dp)).background(Chrome.selected).padding(horizontal = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        GlyphIcon(Glyph.Search, Chrome.faint, size = 20.dp)
        Spacer(Modifier.padding(start = 10.dp))
        // The field takes the row's whole height, so a touch anywhere on it starts typing.
        BasicTextField(
            value = query,
            onValueChange = { query = it },
            singleLine = true,
            textStyle = TextStyle(color = Chrome.text, fontSize = 15.sp),
            cursorBrush = SolidColor(Chrome.text),
            modifier = Modifier.weight(1f).heightIn(min = 48.dp),
            decorationBox = { field ->
                Box(Modifier.heightIn(min = 48.dp), contentAlignment = Alignment.CenterStart) {
                    if (query.isEmpty()) Text(Strings.readerSettingsSearch(context), color = Chrome.faint, fontSize = 15.sp)
                    field()
                }
            },
        )
    }
    val groups = if (novel) {
        listOf(
            Strings.readerMoreHeadingText(context) to listOf(Key.LOOK, Key.FONT, Key.TEXT_SIZE, Key.LINE_SPACING, Key.MARGINS, Key.JUSTIFY, Key.MODE),
            Strings.readerMoreHeadingNavigation(context) to listOf(Key.TAP_PAGED, Key.VOLUME_KEYS_TURN, Key.FOOT_VALUE, Key.FOLIO),
        )
    } else {
        listOf(
            Strings.readerMoreHeadingLayout(context) to listOf(Key.FIT, Key.BACKGROUND, Key.STRIP_GAP, Key.PAGE_LAYOUT, Key.SPREADS),
            Strings.readerMoreHeadingNavigation(context) to listOf(Key.TAP_PAGED, Key.VOLUME_KEYS_TURN, Key.FOOT_VALUE, Key.FOLIO),
            Strings.readerMoreHeadingImages(context) to listOf(Key.COLOUR, Key.CROP_BORDERS),
        )
    }
    groups.forEach { (heading, keys) ->
        val shown = keys.mapNotNull { key -> rows.firstOrNull { it.key == key } }
            .filter { query.isBlank() || rowTitle(context, it.key)?.contains(query.trim(), ignoreCase = true) == true }
        if (shown.isNotEmpty()) {
            Heading(heading)
            shown.forEach { row ->
                // The tap zones have a page of their own, as seven drawn choices do not fit a row.
                if (row.key == Key.TAP_PAGED) {
                    NavRow(Strings.readerRowTap(context), tapZonesValue(context, state), onTapZones)
                } else {
                    SettingLine(context, session, row, toggle)
                }
            }
        }
    }
    // Paper mode and its parts have their own page.
    val paperKeys = listOf(Key.PAPER_MODE, Key.PAPER_INSTANT, Key.PAPER_LOOK, Key.PAPER_DIM, Key.GREYSCALE_PAGES, Key.PAPER_VOLUME, Key.QUIET_PHONE)
    val paper = rows.firstOrNull { it.key == Key.PAPER_MODE }
    val matches = query.isBlank() || paperKeys.any { key -> rows.any { it.key == key } && rowTitle(context, key)?.contains(query.trim(), ignoreCase = true) == true }
    if (paper != null && matches) {
        Heading(Strings.readerMoreHeadingPaper(context))
        val on = (paper.value as? ReaderValue.Toggle)?.on == true
        NavRow(Strings.readerRowPaper(context), if (on) Strings.readerValueOn(context) else Strings.readerValueOff(context), onPaper)
    }
}

private val sizeRange = 0.7f..2.0f

@OptIn(ExperimentalLayoutApi::class, ExperimentalMaterial3Api::class)
@Composable
private fun SettingLine(context: Context, session: ReaderSession, row: ReaderSettingRow, toggle: (Key, Boolean) -> Unit, novel: Boolean = false) {
    val title = (if (novel && row.key == Key.MODE) Strings.readerRowPages(context) else rowTitle(context, row.key)) ?: return
    val value = row.value
    val scope = if (row.key.series) SettingScope.SERIES else row.scope
    Column(Modifier.fillMaxWidth().padding(vertical = 6.dp)) {
        when {
            row.key == Key.FOOT_VALUE && value is ReaderValue.Choice -> SwitchLine(title, null, value.name == "pages_left") {
                session.setSetting(row.key, row.scope, ReaderValue.Choice(if (it) "pages_left" else "none"))
            }
            value is ReaderValue.Toggle -> {
                SwitchLine(title, extra(context, row.key), value.on) { toggle(row.key, it) }
                // Which key goes forward, under the switch that lets them turn pages.
                if (value.on && (row.key == Key.VOLUME_KEYS_TURN || row.key == Key.PAPER_VOLUME)) {
                    SwitchLine(Strings.readerRowVolumeSwap(context), null, ReaderLook.volumeSwapped(context)) { ReaderLook.swapVolume(context, it) }
                }
            }
            row.key == Key.LINE_SPACING || row.key == Key.MARGINS -> if (value is ReaderValue.Amount) {
                val presets = if (row.key == Key.LINE_SPACING) spacingPresets else marginPresets
                val labels = if (row.key == Key.LINE_SPACING) {
                    listOf(Strings.readerSpacingTight(context), Strings.readerSpacingNormal(context), Strings.readerSpacingLoose(context))
                } else {
                    listOf(Strings.readerMarginsNarrow(context), Strings.readerSpacingNormal(context), Strings.readerMarginsWide(context))
                }
                val at = presets.indices.minBy { abs(presets[it] - value.value) }
                Text(title, color = Chrome.text, fontSize = 16.sp)
                Spacer(Modifier.height(6.dp))
                Segments(labels, at) { session.setSetting(row.key, row.scope, ReaderValue.Amount(presets[it])) }
            }
            row.choices.isNotEmpty() -> {
                val labelled = row.choices.mapNotNull { choice -> choiceLabel(context, row.key, choice)?.let { choice to it } }
                Text(title, color = Chrome.text, fontSize = 16.sp)
                Spacer(Modifier.height(6.dp))
                if (labelled.size <= 4) {
                    Segments(labelled.map { it.second }, labelled.indexOfFirst { it.first == value }) { session.setSetting(row.key, scope, labelled[it].first) }
                } else {
                    // Each choice takes a 48 dp touch around its pill, so the rows sit closer.
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        labelled.forEach { (choice, label) ->
                            val selected = choice == value
                            Text(
                                label,
                                color = if (selected) Chrome.text else Chrome.faint,
                                fontSize = 14.sp,
                                fontWeight = if (selected) FontWeight.Medium else FontWeight.Normal,
                                modifier = Modifier
                                    .minimumInteractiveComponentSize()
                                    .clip(RoundedCornerShape(50))
                                    .background(if (selected) Chrome.selected else Color.Transparent)
                                    .clickable { session.setSetting(row.key, scope, choice) }
                                    .padding(horizontal = 14.dp, vertical = 8.dp),
                            )
                        }
                    }
                }
            }
            value is ReaderValue.Amount && row.key == Key.PAPER_DIM -> {
                Text(title, color = Chrome.text, fontSize = 16.sp)
                Spacer(Modifier.height(10.dp))
                Slider(
                    value = value.value.toFloat().coerceIn(0f, 1f),
                    onValueChange = { session.setSetting(row.key, row.scope, ReaderValue.Amount(((it * 20).toInt() / 20.0))) },
                    valueRange = 0f..1f,
                    colors = SliderDefaults.colors(
                        thumbColor = Chrome.text,
                        activeTrackColor = Chrome.text,
                        inactiveTrackColor = Chrome.rule,
                    ),
                    modifier = Modifier.fillMaxWidth().semantics {
                        contentDescription = title
                        stateDescription = "${(value.value * 100).toInt()}%"
                    },
                )
            }
            value is ReaderValue.Amount && row.key == Key.TEXT_SIZE -> {
                Text(title, color = Chrome.text, fontSize = 16.sp)
                Row(verticalAlignment = Alignment.CenterVertically) {
                    SizeStep(13.sp, Strings.readerSizeSmall(context), value.value > sizeRange.start + 0.05) { stepSize(session, row, value, -1) }
                    Slider(
                        value = value.value.toFloat().coerceIn(sizeRange),
                        onValueChange = { session.setSetting(row.key, row.scope, ReaderValue.Amount(((it * 10).toInt() / 10.0))) },
                        valueRange = sizeRange,
                        steps = 12,
                        colors = SliderDefaults.colors(
                            thumbColor = Chrome.text,
                            activeTrackColor = Chrome.text,
                            inactiveTrackColor = Chrome.rule,
                            activeTickColor = Color.Transparent,
                            inactiveTickColor = Color.Transparent,
                        ),
                        modifier = Modifier.weight(1f).padding(horizontal = 8.dp).semantics { stateDescription = "%.1f".format(value.value) },
                    )
                    SizeStep(22.sp, Strings.readerSizeLarge(context), value.value < sizeRange.endInclusive - 0.05) { stepSize(session, row, value, 1) }
                }
            }
        }
        if (row.key != Key.MODE && row.key != Key.DIRECTION) {
            UseDefaultNote(context, null, row.overridden) { session.useDefault(row.key) }
        }
    }
}

private val spacingPresets = listOf(1.3, 1.6, 1.9)
private val marginPresets = listOf(0.6, 1.0, 1.5)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun Segments(labels: List<String>, selected: Int, modifier: Modifier = Modifier.fillMaxWidth(), onSelect: (Int) -> Unit) {
    SingleChoiceSegmentedButtonRow(modifier) {
        labels.forEachIndexed { index, label ->
            SegmentedButton(
                selected = index == selected,
                onClick = { onSelect(index) },
                shape = SegmentedButtonDefaults.itemShape(index, labels.size),
                colors = SegmentedButtonDefaults.colors(
                    activeContainerColor = Chrome.selected,
                    activeContentColor = Chrome.text,
                    activeBorderColor = Chrome.rule,
                    inactiveContainerColor = Color.Transparent,
                    inactiveContentColor = Chrome.faint,
                    inactiveBorderColor = Chrome.rule,
                ),
                icon = {},
            ) {
                // At the largest text sizes a label shrinks to fit its segment, and is cut with an
                // ellipsis only past that.
                Text(
                    label,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    autoSize = TextAutoSize.StepBased(minFontSize = 9.sp, maxFontSize = 14.sp),
                )
            }
        }
    }
}

@Composable
internal fun SwitchLine(title: String, line: String?, checked: Boolean, big: Boolean = false, onChange: (Boolean) -> Unit) {
    Row(
        Modifier.fillMaxWidth().heightIn(min = 48.dp).toggleable(value = checked, role = Role.Switch, onValueChange = onChange),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(title, color = Chrome.text, fontSize = if (big) 17.sp else 16.sp)
            if (line != null) Text(line, color = Chrome.faint, fontSize = 12.sp)
        }
        Switch(
            checked = checked,
            onCheckedChange = null,
            colors = SwitchDefaults.colors(
                checkedTrackColor = Chrome.text,
                checkedThumbColor = Chrome.plate,
                checkedBorderColor = Chrome.text,
                uncheckedTrackColor = Chrome.selected,
                uncheckedThumbColor = Chrome.faint,
                uncheckedBorderColor = Chrome.rule,
            ),
        )
    }
}

private val Key.series: Boolean get() = this == Key.MODE || this == Key.DIRECTION || this == Key.COLOUR

private fun extra(context: Context, key: Key): String? = when (key) {
    Key.QUIET_PHONE -> Strings.readerQuietLine(context)
    else -> null
}

/** The row's written name, or null for a key that has none (the unit test fails on it). */
fun rowTitle(context: Context, key: Key): String? = when (key) {
    Key.MODE -> Strings.readerRowReadingMode(context)
    Key.DIRECTION -> Strings.readerRowDirection(context)
    Key.COLOUR -> Strings.readerRowColour(context)
    Key.LOOK -> Strings.readerRowLook(context)
    Key.FONT -> Strings.readerRowFont(context)
    Key.CROP_BORDERS -> Strings.readerRowCrop(context)
    Key.FIT -> Strings.readerRowFit(context)
    Key.BACKGROUND -> Strings.readerRowBackground(context)
    Key.PAGE_LAYOUT -> Strings.readerRowPageLayout(context)
    Key.SPREADS -> Strings.readerRowSpreads(context)
    Key.TEXT_SIZE -> Strings.readerRowTextSize(context)
    Key.MARGINS -> Strings.readerRowMargins(context)
    Key.LINE_SPACING -> Strings.readerRowLineSpacing(context)
    Key.JUSTIFY -> Strings.readerRowJustify(context)
    Key.TAP_PAGED, Key.TAP_SCROLL -> Strings.readerRowTap(context)
    Key.TAP_SIZE -> Strings.readerTapSize(context)
    Key.TAP_FLIP_SIDES -> Strings.readerTapFlipSides(context)
    Key.TAP_FLIP_ENDS -> Strings.readerTapFlipEnds(context)
    Key.SWIPE_TURNS -> Strings.einkSwipeTurns(context)
    Key.VOLUME_KEYS_TURN -> Strings.readerRowVolume(context)
    Key.STRIP_WIDTH -> Strings.readerRowStripWidth(context)
    Key.PAPER_MODE -> Strings.readerRowPaper(context)
    Key.GREYSCALE_PAGES -> Strings.readerRowGreyscale(context)
    Key.QUIET_PHONE -> Strings.readerRowQuiet(context)
    Key.FOOT_VALUE -> Strings.readerRowFoot(context)
    Key.FOLIO -> Strings.readerRowFolio(context)
    Key.STRIP_GAP -> Strings.readerRowStripGap(context)
    Key.PAPER_INSTANT -> Strings.readerPaperTurn(context)
    Key.PAPER_LOOK -> Strings.readerPaperLook(context)
    Key.PAPER_DIM -> Strings.readerPaperDim(context)
    Key.PAPER_VOLUME -> Strings.readerRowVolume(context)
    else -> null
}

/** The choice's written name, or null for a choice that has none (the unit test fails on it). */
fun choiceLabel(context: Context, key: Key, value: ReaderValue): String? {
    val name = (value as? ReaderValue.Choice)?.name ?: return null
    if (key == Key.TAP_PAGED || key == Key.TAP_SCROLL) return zoneLayout(name)?.let { layoutName(context, it) }
    return when (key to name) {
        Key.MODE to "paged" -> Strings.readerPagesPaged(context)
        Key.MODE to "scrolled" -> Strings.readerPagesScroll(context)
        Key.MODE to "vertical" -> Strings.readerModeVertical(context)
        Key.MODE to "strip" -> Strings.readerModeStrip(context)
        Key.DIRECTION to "rtl" -> Strings.readerModeRtl(context)
        Key.DIRECTION to "ltr" -> Strings.readerModeLtr(context)
        Key.COLOUR to "auto" -> Strings.readerColourAuto(context)
        Key.COLOUR to "colour" -> Strings.readerColourColour(context)
        Key.COLOUR to "greys" -> Strings.readerColourGreys(context)
        Key.LOOK to "system" -> Strings.readerLookSystem(context)
        Key.LOOK to "paper" -> Strings.readerLookPaper(context)
        Key.LOOK to "light" -> Strings.readerLookLight(context)
        Key.LOOK to "sepia" -> Strings.readerLookSepia(context)
        Key.LOOK to "dark" -> Strings.readerLookDark(context)
        Key.FONT to "serif" -> Strings.readerFontSerif(context)
        Key.FONT to "sans" -> Strings.readerFontSans(context)
        Key.FIT to "screen" -> Strings.readerFitScreen(context)
        Key.FIT to "width" -> Strings.readerFitWidth(context)
        Key.FIT to "height" -> Strings.readerFitHeight(context)
        Key.BACKGROUND to "theme" -> Strings.readerBackgroundTheme(context)
        Key.BACKGROUND to "black" -> Strings.readerBackgroundBlack(context)
        Key.BACKGROUND to "white" -> Strings.readerBackgroundWhite(context)
        Key.PAGE_LAYOUT to "auto" -> Strings.readerLayoutAuto(context)
        Key.PAGE_LAYOUT to "single" -> Strings.readerLayoutSingle(context)
        Key.PAGE_LAYOUT to "double" -> Strings.readerLayoutDouble(context)
        Key.SPREADS to "split" -> Strings.readerSpreadsSplit(context)
        Key.SPREADS to "rotate" -> Strings.readerSpreadsRotate(context)
        Key.SPREADS to "whole" -> Strings.readerSpreadsWhole(context)
        Key.TAP_SIZE to "standard" -> Strings.readerTapStandard(context)
        Key.TAP_SIZE to "small" -> Strings.readerTapSmall(context)
        Key.FOOT_VALUE to "pages_left" -> Strings.readerRowFoot(context)
        Key.STRIP_GAP to "none" -> Strings.readerGapNone(context)
        Key.STRIP_GAP to "small" -> Strings.readerGapSmall(context)
        else -> null
    }
}
