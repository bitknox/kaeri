// The reader's settings sheet: this series' reading mode (manga and strips only), the medium's
// other choices, Paper mode on a page of its own, and the rest on one searchable page. The engine
// keeps every value; a change goes out as a command and the sheet draws the settings the engine
// sends back. Every choice and row has a written name; one without a name is not drawn, and the
// unit test fails on it.

package app.kaeri.reader.phone

import android.content.Context
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
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
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
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
import kotlin.math.abs

private enum class Pane { Main, Mode, More, Paper }

/** The rows a phone has: the e-ink panel's own are left to its reader. */
internal val phoneKeys = listOf(
    Key.MODE, Key.DIRECTION, Key.COLOUR, Key.FIT, Key.BACKGROUND, Key.CROP_BORDERS, Key.PAGE_LAYOUT, Key.SPREADS,
    Key.LOOK, Key.FONT, Key.TEXT_SIZE, Key.MARGINS, Key.LINE_SPACING, Key.JUSTIFY,
    Key.TAP_LAYOUT, Key.VOLUME_KEYS_TURN, Key.PAPER_MODE, Key.GREYSCALE_PAGES, Key.QUIET_PHONE, Key.FOOT_VALUE, Key.FOLIO,
)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ReaderSettingsSheet(
    session: ReaderSession,
    state: ReaderState,
    onDismiss: () -> Unit,
    toggle: (Key, Boolean) -> Unit = { key, on -> session.setSetting(key, SettingScope.DEVICE, ReaderValue.Toggle(on)) },
) {
    val context = LocalContext.current
    val view = state.view ?: return
    val settings = state.settings
    var pane by remember { mutableStateOf(Pane.Main) }
    val rows = settings?.rows.orEmpty().filter { it.key in phoneKeys }
    val novel = view.medium == Medium.NOVEL
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        containerColor = Chrome.plate,
        contentColor = Chrome.text,
        // The page behind stays undimmed.
        scrimColor = Color.Transparent,
    ) {
        Column(Modifier.verticalScroll(rememberScrollState()).navigationBarsPadding().padding(horizontal = 20.dp, vertical = 4.dp)) {
            when (pane) {
                Pane.Main -> Main(context, session, rows, novel, view.title, toggle, onMode = { pane = Pane.Mode }, onPaper = { pane = Pane.Paper }, onMore = { pane = Pane.More })
                Pane.Mode -> ModePicker(context, session, rows, view.title) { pane = Pane.Main }
                Pane.More -> More(context, session, rows, novel, toggle) { pane = Pane.Main }
                Pane.Paper -> PaperPage(context, session, rows, toggle) { pane = Pane.Main }
            }
            Spacer(Modifier.height(16.dp))
        }
    }
}

@Composable
private fun Heading(text: String) {
    Text(text, color = Chrome.faint, fontSize = 13.sp, fontWeight = FontWeight.Medium, modifier = Modifier.padding(top = 14.dp, bottom = 6.dp))
}

@Composable
private fun BackRow(title: String, context: Context, onBack: () -> Unit) {
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        IconButton(onClick = onBack) {
            GlyphIcon(Glyph.Back, Chrome.text, mirror = androidx.compose.ui.platform.LocalLayoutDirection.current == androidx.compose.ui.unit.LayoutDirection.Rtl)
        }
        Text(title, color = Chrome.text, fontSize = 18.sp, fontWeight = FontWeight.Medium, modifier = Modifier.semantics { contentDescription = Strings.readerSettingsBack(context) + ", " + title })
    }
}

/** A row 46 dp tall with a value and a chevron. */
@Composable
private fun NavRow(title: String, value: String?, onClick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().heightIn(min = 46.dp).clip(RoundedCornerShape(12.dp)).clickable(onClick = onClick),
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
    title: String,
    toggle: (Key, Boolean) -> Unit,
    onMode: () -> Unit,
    onPaper: () -> Unit,
    onMore: () -> Unit,
) {
    val mode = rows.firstOrNull { it.key == Key.MODE }
    val direction = rows.firstOrNull { it.key == Key.DIRECTION }
    if (!novel) {
        Heading(Strings.readerSheetSeries(context))
        NavRow(Strings.readerRowReadingMode(context), modeSummary(context, mode, direction), onMode)
        ModeNote(context, session, mode, direction, title)
    }
    Heading(if (novel) Strings.readerSheetAllNovels(context) else Strings.readerSheetAllManga(context))
    if (novel) {
        listOf(Key.TEXT_SIZE, Key.FONT, Key.LOOK, Key.LINE_SPACING, Key.MARGINS, Key.MODE).forEach { key ->
            rows.firstOrNull { it.key == key }?.let { SettingLine(context, session, it, toggle) }
        }
    } else {
        listOf(Key.FIT, Key.CROP_BORDERS, Key.BACKGROUND).forEach { key ->
            rows.firstOrNull { it.key == key }?.let { SettingLine(context, session, it, toggle) }
        }
        val paper = rows.firstOrNull { it.key == Key.PAPER_MODE }
        if (paper != null) {
            val on = (paper.value as? ReaderValue.Toggle)?.on == true
            NavRow(Strings.readerRowPaper(context), if (on) Strings.readerValueOn(context) else Strings.readerValueOff(context), onPaper)
        }
    }
    Spacer(Modifier.height(6.dp))
    NavRow(Strings.readerSheetMore(context), null, onMore)
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
    return if (auto) Strings.readerModeAuto(context, way) else way
}

/** Under the mode row: where the mode came from, and Use default when the series has its own. */
@Composable
private fun ModeNote(context: Context, session: ReaderSession, mode: ReaderSettingRow?, direction: ReaderSettingRow?, title: String) {
    val overridden = mode?.overridden == true || direction?.overridden == true
    Text(
        if (overridden) Strings.readerModeSet(context, title) else Strings.readerModeDetected(context),
        color = Chrome.faint,
        fontSize = 13.sp,
    )
    if (overridden) {
        TextButton(onClick = {
            session.useDefault(Key.MODE)
            session.useDefault(Key.DIRECTION)
        }) { Text(Strings.readerModeUseDefault(context), color = Chrome.text, textDecoration = androidx.compose.ui.text.style.TextDecoration.Underline) }
    }
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

@Composable
private fun PaperPage(context: Context, session: ReaderSession, rows: List<ReaderSettingRow>, toggle: (Key, Boolean) -> Unit, onBack: () -> Unit) {
    BackRow(Strings.readerRowPaper(context), context, onBack)
    Text(Strings.readerPaperLine(context), color = Chrome.faint, fontSize = 13.sp, modifier = Modifier.padding(bottom = 6.dp))
    listOf(Key.PAPER_MODE, Key.GREYSCALE_PAGES, Key.QUIET_PHONE).forEach { key ->
        rows.firstOrNull { it.key == key }?.let { SettingLine(context, session, it, toggle) }
    }
}

@Composable
private fun More(context: Context, session: ReaderSession, rows: List<ReaderSettingRow>, novel: Boolean, toggle: (Key, Boolean) -> Unit, onBack: () -> Unit) {
    BackRow(Strings.readerSheetMore(context), context, onBack)
    var query by remember { mutableStateOf("") }
    Row(
        Modifier.fillMaxWidth().heightIn(min = 44.dp).clip(RoundedCornerShape(22.dp)).background(Chrome.selected).padding(horizontal = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        GlyphIcon(Glyph.Search, Chrome.faint, size = 20.dp)
        Spacer(Modifier.padding(start = 10.dp))
        Box(Modifier.weight(1f)) {
            if (query.isEmpty()) Text(Strings.readerSearchSettings(context), color = Chrome.faint, fontSize = 15.sp)
            BasicTextField(
                value = query,
                onValueChange = { query = it },
                singleLine = true,
                textStyle = TextStyle(color = Chrome.text, fontSize = 15.sp),
                cursorBrush = SolidColor(Chrome.text),
                modifier = Modifier.fillMaxWidth(),
            )
        }
    }
    val groups = if (novel) {
        listOf(
            Strings.readerMoreHeadingText(context) to listOf(Key.LOOK, Key.FONT, Key.TEXT_SIZE, Key.LINE_SPACING, Key.MARGINS, Key.JUSTIFY, Key.MODE),
            Strings.readerMoreHeadingNavigation(context) to listOf(Key.TAP_LAYOUT, Key.VOLUME_KEYS_TURN, Key.FOOT_VALUE, Key.FOLIO),
            Strings.readerMoreHeadingPaper(context) to listOf(Key.PAPER_MODE, Key.QUIET_PHONE),
        )
    } else {
        listOf(
            Strings.readerMoreHeadingLayout(context) to listOf(Key.FIT, Key.BACKGROUND, Key.PAGE_LAYOUT, Key.SPREADS),
            Strings.readerMoreHeadingNavigation(context) to listOf(Key.TAP_LAYOUT, Key.VOLUME_KEYS_TURN, Key.FOOT_VALUE, Key.FOLIO),
            Strings.readerMoreHeadingImages(context) to listOf(Key.COLOUR, Key.CROP_BORDERS),
            Strings.readerMoreHeadingPaper(context) to listOf(Key.PAPER_MODE, Key.GREYSCALE_PAGES, Key.QUIET_PHONE),
        )
    }
    groups.forEach { (heading, keys) ->
        val shown = keys.mapNotNull { key -> rows.firstOrNull { it.key == key } }
            .filter { query.isBlank() || rowTitle(context, it.key)?.contains(query.trim(), ignoreCase = true) == true }
        if (shown.isNotEmpty()) {
            Heading(heading)
            shown.forEach { SettingLine(context, session, it, toggle) }
        }
    }
}

private val sizeRange = 0.7f..2.0f

@OptIn(ExperimentalLayoutApi::class, ExperimentalMaterial3Api::class)
@Composable
private fun SettingLine(context: Context, session: ReaderSession, row: ReaderSettingRow, toggle: (Key, Boolean) -> Unit) {
    val title = rowTitle(context, row.key) ?: return
    val value = row.value
    val scope = if (row.key.series) SettingScope.SERIES else row.scope
    Column(Modifier.fillMaxWidth().padding(vertical = 6.dp)) {
        when {
            row.key == Key.FOOT_VALUE && value is ReaderValue.Choice -> SwitchLine(title, null, value.name == "pages_left") {
                session.setSetting(row.key, row.scope, ReaderValue.Choice(if (it) "pages_left" else "none"))
            }
            value is ReaderValue.Toggle -> SwitchLine(title, extra(context, row.key), value.on) { toggle(row.key, it) }
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
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        labelled.forEach { (choice, label) ->
                            val selected = choice == value
                            Text(
                                label,
                                color = if (selected) Chrome.text else Chrome.faint,
                                fontSize = 14.sp,
                                fontWeight = if (selected) FontWeight.Medium else FontWeight.Normal,
                                modifier = Modifier
                                    .clip(RoundedCornerShape(50))
                                    .background(if (selected) Chrome.selected else Color.Transparent)
                                    .clickable { session.setSetting(row.key, scope, choice) }
                                    .padding(horizontal = 14.dp, vertical = 8.dp),
                            )
                        }
                    }
                }
            }
            value is ReaderValue.Amount && row.key == Key.TEXT_SIZE -> {
                Text(title, color = Chrome.text, fontSize = 16.sp)
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text("A", color = Chrome.faint, fontSize = 13.sp, modifier = Modifier.semantics { contentDescription = Strings.readerSizeSmall(context) })
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
                    Text("A", color = Chrome.text, fontSize = 22.sp, modifier = Modifier.semantics { contentDescription = Strings.readerSizeLarge(context) })
                }
            }
        }
        if (row.overridden && row.key != Key.MODE && row.key != Key.DIRECTION) {
            TextButton(onClick = { session.useDefault(row.key) }) { Text(Strings.readerModeUseDefault(context), color = Chrome.text) }
        }
    }
}

private val spacingPresets = listOf(1.3, 1.6, 1.9)
private val marginPresets = listOf(0.6, 1.0, 1.5)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun Segments(labels: List<String>, selected: Int, onSelect: (Int) -> Unit) {
    SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth()) {
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
            ) { Text(label, fontSize = 14.sp, maxLines = 1) }
        }
    }
}

@Composable
private fun SwitchLine(title: String, line: String?, checked: Boolean, onChange: (Boolean) -> Unit) {
    Row(Modifier.fillMaxWidth().heightIn(min = 46.dp), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            Text(title, color = Chrome.text, fontSize = 16.sp)
            if (line != null) Text(line, color = Chrome.faint, fontSize = 12.sp)
        }
        Switch(
            checked = checked,
            onCheckedChange = onChange,
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
internal fun rowTitle(context: Context, key: Key): String? = when (key) {
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
    Key.TAP_LAYOUT -> Strings.readerRowTap(context)
    Key.VOLUME_KEYS_TURN -> Strings.readerRowVolume(context)
    Key.STRIP_WIDTH -> Strings.readerRowStripWidth(context)
    Key.PAPER_MODE -> Strings.readerRowPaper(context)
    Key.GREYSCALE_PAGES -> Strings.readerRowGreyscale(context)
    Key.QUIET_PHONE -> Strings.readerRowQuiet(context)
    Key.FOOT_VALUE -> Strings.readerRowFoot(context)
    Key.FOLIO -> Strings.readerRowFolio(context)
    else -> null
}

/** The choice's written name, or null for a choice that has none (the unit test fails on it). */
internal fun choiceLabel(context: Context, key: Key, value: ReaderValue): String? {
    val name = (value as? ReaderValue.Choice)?.name ?: return null
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
        Key.TAP_LAYOUT to "edges" -> Strings.readerTapEdges(context)
        Key.TAP_LAYOUT to "sides" -> Strings.readerTapSides(context)
        Key.TAP_LAYOUT to "off" -> Strings.readerTapOff(context)
        Key.FOOT_VALUE to "pages_left" -> Strings.readerRowFoot(context)
        else -> null
    }
}
