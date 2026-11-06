// The reader's settings sheet: this series' reading mode, the medium's other choices, and the
// rest on a pane of their own. The engine keeps every value; a change goes out as a command and
// the sheet draws the settings the engine sends back.

package app.kaeri.reader.phone

import android.content.Context
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.kaeri.design.ObiSilk
import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.ReaderSettingKey as Key
import app.kaeri.engine.model.ReaderSettingRow
import app.kaeri.engine.model.ReaderValue
import app.kaeri.engine.model.SettingScope
import app.kaeri.i18n.Strings
import app.kaeri.reader.ReaderSession
import app.kaeri.reader.ReaderState

private enum class Pane { Main, Mode, More }

/** The rows a phone has: the e-ink panel's own are left to its reader. */
private val phoneKeys = listOf(
    Key.MODE, Key.DIRECTION, Key.COLOUR, Key.FIT, Key.CROP_BORDERS, Key.PAGE_LAYOUT, Key.SPREADS, Key.STRIP_WIDTH,
    Key.LOOK, Key.FONT, Key.TEXT_SIZE, Key.MARGINS, Key.LINE_SPACING, Key.JUSTIFY,
    Key.TAP_LAYOUT, Key.VOLUME_KEYS_TURN, Key.PAPER_MODE, Key.QUIET_PHONE, Key.FOOT_VALUE, Key.FOLIO,
)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ReaderSettingsSheet(session: ReaderSession, state: ReaderState, onDismiss: () -> Unit) {
    val context = LocalContext.current
    val view = state.view ?: return
    val settings = state.settings
    var pane by remember { mutableStateOf(Pane.Main) }
    val rows = settings?.rows.orEmpty().filter { it.key in phoneKeys }
    val novel = view.medium == Medium.NOVEL
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        containerColor = ObiSilk.Raised,
        contentColor = ObiSilk.Silk,
    ) {
        Column(Modifier.verticalScroll(rememberScrollState()).navigationBarsPadding().padding(horizontal = 20.dp, vertical = 4.dp)) {
            when (pane) {
                Pane.Main -> Main(context, session, rows, novel, view.title, onMode = { pane = Pane.Mode }, onMore = { pane = Pane.More })
                Pane.Mode -> ModePicker(context, session, state, rows, view.title) { pane = Pane.Main }
                Pane.More -> More(context, session, rows, novel) { pane = Pane.Main }
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
private fun Main(
    context: Context,
    session: ReaderSession,
    rows: List<ReaderSettingRow>,
    novel: Boolean,
    title: String,
    onMode: () -> Unit,
    onMore: () -> Unit,
) {
    val mode = rows.firstOrNull { it.key == Key.MODE }
    val direction = rows.firstOrNull { it.key == Key.DIRECTION }
    Heading(Strings.readerSheetSeries(context))
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).clickable(onClick = onMode).padding(vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(Strings.readerRowReadingMode(context), fontSize = 16.sp, modifier = Modifier.weight(1f))
        Text(modeSummary(context, mode, direction, novel), color = Chrome.accent, fontSize = 15.sp)
    }
    Heading(if (novel) Strings.readerSheetAllNovels(context) else Strings.readerSheetAllManga(context))
    val quick = if (novel) listOf(Key.LOOK, Key.FONT, Key.TEXT_SIZE) else listOf(Key.FIT, Key.CROP_BORDERS, Key.PAPER_MODE, Key.QUIET_PHONE)
    quick.forEach { key -> rows.firstOrNull { it.key == key }?.let { SettingLine(context, session, it) } }
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).clickable(onClick = onMore).padding(vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(Strings.readerSheetMore(context), color = Chrome.accent, fontSize = 16.sp, modifier = Modifier.weight(1f))
        Text("›", color = Chrome.faint, fontSize = 18.sp)
    }
}

private fun modeSummary(context: Context, mode: ReaderSettingRow?, direction: ReaderSettingRow?, novel: Boolean): String {
    val m = (mode?.value as? ReaderValue.Choice)?.name
    val d = (direction?.value as? ReaderValue.Choice)?.name
    val way = when {
        novel -> if (m == "scrolled") Strings.readerPagesScroll(context) else Strings.readerPagesPaged(context)
        m == "strip" -> Strings.readerModeStrip(context)
        d == "ltr" -> Strings.readerModeLtr(context)
        else -> Strings.readerModeRtl(context)
    }
    val auto = mode?.detected == true || direction?.detected == true
    return if (auto && !novel) Strings.readerModeAutoWay(context, way) else way
}

@Composable
private fun ModePicker(context: Context, session: ReaderSession, state: ReaderState, rows: List<ReaderSettingRow>, title: String, onBack: () -> Unit) {
    val mode = rows.firstOrNull { it.key == Key.MODE }
    val direction = rows.firstOrNull { it.key == Key.DIRECTION }
    val m = (mode?.value as? ReaderValue.Choice)?.name
    val d = (direction?.value as? ReaderValue.Choice)?.name
    val auto = mode?.overridden != true && direction?.overridden != true
    TextButton(Strings.readerSettingsBack(context), onBack)
    Heading(Strings.readerRowReadingMode(context))
    val way = when {
        m == "strip" -> Strings.readerModeStrip(context)
        d == "ltr" -> Strings.readerModeLtr(context)
        else -> Strings.readerModeRtl(context)
    }
    ModeOption(Strings.readerModeAutoName(context), Strings.readerModeAutoLine(context, way), auto) {
        session.useDefault(Key.MODE)
        session.useDefault(Key.DIRECTION)
    }
    ModeOption(Strings.readerModeRtl(context), null, !auto && m != "strip" && d != "ltr") {
        session.setSetting(Key.MODE, SettingScope.SERIES, ReaderValue.Choice("paged"))
        session.setSetting(Key.DIRECTION, SettingScope.SERIES, ReaderValue.Choice("rtl"))
    }
    ModeOption(Strings.readerModeLtr(context), null, !auto && m != "strip" && d == "ltr") {
        session.setSetting(Key.MODE, SettingScope.SERIES, ReaderValue.Choice("paged"))
        session.setSetting(Key.DIRECTION, SettingScope.SERIES, ReaderValue.Choice("ltr"))
    }
    ModeOption(Strings.readerModeStrip(context), Strings.readerModeStripLine(context), !auto && m == "strip") {
        session.setSetting(Key.MODE, SettingScope.SERIES, ReaderValue.Choice("strip"))
    }
    Spacer(Modifier.height(10.dp))
    Text(
        if (auto) Strings.readerModeDetected(context) else Strings.readerModeSaved(context),
        color = Chrome.faint,
        fontSize = 13.sp,
    )
}

@Composable
private fun ModeOption(name: String, line: String?, selected: Boolean, onClick: () -> Unit) {
    val shape = RoundedCornerShape(12.dp)
    Row(
        Modifier
            .fillMaxWidth()
            .padding(vertical = 4.dp)
            .clip(shape)
            .background(if (selected) ObiSilk.RaisedSelected else ObiSilk.RaisedLow)
            .then(if (selected) Modifier.border(1.dp, Chrome.accent, shape) else Modifier)
            .clickable(onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(name, fontSize = 16.sp, fontWeight = FontWeight.Medium)
            if (line != null) Text(line, color = Chrome.faint, fontSize = 13.sp)
        }
    }
}

@Composable
private fun More(context: Context, session: ReaderSession, rows: List<ReaderSettingRow>, novel: Boolean, onBack: () -> Unit) {
    TextButton(Strings.readerSettingsBack(context), onBack)
    val groups = if (novel) {
        listOf(
            Strings.readerMoreHeadingText(context) to listOf(Key.LOOK, Key.FONT, Key.TEXT_SIZE, Key.LINE_SPACING, Key.MARGINS, Key.JUSTIFY, Key.MODE),
            Strings.readerMoreHeadingNavigation(context) to listOf(Key.TAP_LAYOUT, Key.VOLUME_KEYS_TURN, Key.FOOT_VALUE, Key.FOLIO),
            Strings.readerMoreHeadingPaper(context) to listOf(Key.PAPER_MODE, Key.QUIET_PHONE),
        )
    } else {
        listOf(
            Strings.readerMoreHeadingLayout(context) to listOf(Key.FIT, Key.PAGE_LAYOUT, Key.SPREADS, Key.STRIP_WIDTH),
            Strings.readerMoreHeadingNavigation(context) to listOf(Key.TAP_LAYOUT, Key.VOLUME_KEYS_TURN, Key.FOOT_VALUE, Key.FOLIO),
            Strings.readerMoreHeadingImages(context) to listOf(Key.COLOUR, Key.CROP_BORDERS),
            Strings.readerMoreHeadingPaper(context) to listOf(Key.PAPER_MODE, Key.QUIET_PHONE),
        )
    }
    groups.forEach { (heading, keys) ->
        val shown = keys.mapNotNull { key -> rows.firstOrNull { it.key == key } }
        if (shown.isNotEmpty()) {
            Heading(heading)
            shown.forEach { SettingLine(context, session, it) }
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun SettingLine(context: Context, session: ReaderSession, row: ReaderSettingRow) {
    val value = row.value
    val scope = if (row.key.series) SettingScope.SERIES else row.scope
    Column(Modifier.fillMaxWidth().padding(vertical = 6.dp)) {
        when {
            value is ReaderValue.Toggle -> {
                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                    Column(Modifier.weight(1f)) {
                        Text(rowTitle(context, row.key), fontSize = 16.sp)
                        extra(context, row.key)?.let { Text(it, color = Chrome.faint, fontSize = 12.sp) }
                    }
                    Switch(
                        checked = value.on,
                        onCheckedChange = { session.setSetting(row.key, row.scope, ReaderValue.Toggle(it)) },
                        colors = SwitchDefaults.colors(checkedTrackColor = Chrome.accent, checkedThumbColor = ObiSilk.Ground),
                    )
                }
            }
            row.choices.isNotEmpty() -> {
                Text(rowTitle(context, row.key), fontSize = 16.sp)
                Spacer(Modifier.height(6.dp))
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    row.choices.forEach { choice ->
                        val selected = choice == value
                        val shape = RoundedCornerShape(50)
                        Text(
                            choiceLabel(context, row.key, choice),
                            color = if (selected) ObiSilk.Ground else Chrome.text,
                            fontSize = 14.sp,
                            modifier = Modifier
                                .clip(shape)
                                .background(if (selected) Chrome.accent else ObiSilk.RaisedLow)
                                .clickable { session.setSetting(row.key, scope, choice) }
                                .padding(horizontal = 14.dp, vertical = 8.dp),
                        )
                    }
                }
            }
            value is ReaderValue.Amount -> {
                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                    Text(rowTitle(context, row.key), fontSize = 16.sp, modifier = Modifier.weight(1f))
                    TextButton("−", { session.setSetting(row.key, row.scope, ReaderValue.Amount(value.value - step(row.key))) }, colour = Chrome.text)
                    Text(amount(value.value), fontSize = 15.sp)
                    TextButton("+", { session.setSetting(row.key, row.scope, ReaderValue.Amount(value.value + step(row.key))) }, colour = Chrome.text)
                }
            }
        }
        if (row.overridden) {
            TextButton(Strings.readerModeUseDefault(context), { session.useDefault(row.key) })
        }
    }
}

private val Key.series: Boolean get() = this == Key.MODE || this == Key.DIRECTION || this == Key.COLOUR

private fun step(key: Key): Double = if (key == Key.TEXT_SIZE) 10.0 else 0.1

private fun amount(value: Double): String = if (value >= 10) value.toInt().toString() else "%.1f".format(value)

private fun extra(context: Context, key: Key): String? = when (key) {
    Key.QUIET_PHONE -> Strings.readerQuietLine(context)
    Key.PAPER_MODE -> Strings.readerPaperLine(context)
    else -> null
}

internal fun rowTitle(context: Context, key: Key): String = when (key) {
    Key.MODE -> Strings.readerRowReadingMode(context)
    Key.DIRECTION -> Strings.readerRowDirection(context)
    Key.COLOUR -> Strings.readerRowColour(context)
    Key.LOOK -> Strings.readerRowLook(context)
    Key.FONT -> Strings.readerRowFont(context)
    Key.CROP_BORDERS -> Strings.readerRowCrop(context)
    Key.FIT -> Strings.readerRowFit(context)
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
    Key.QUIET_PHONE -> Strings.readerRowQuiet(context)
    Key.FOOT_VALUE -> Strings.readerRowFoot(context)
    Key.FOLIO -> Strings.readerRowFolio(context)
    else -> key.name
}

internal fun choiceLabel(context: Context, key: Key, value: ReaderValue): String {
    val name = (value as? ReaderValue.Choice)?.name ?: return ""
    return when (key to name) {
        Key.MODE to "paged" -> Strings.readerPagesPaged(context)
        Key.MODE to "scrolled" -> Strings.readerPagesScroll(context)
        Key.MODE to "strip" -> Strings.readerModeStrip(context)
        Key.DIRECTION to "rtl" -> Strings.readerModeRtl(context)
        Key.DIRECTION to "ltr" -> Strings.readerModeLtr(context)
        Key.COLOUR to "auto" -> Strings.readerColourAuto(context)
        Key.COLOUR to "colour" -> Strings.readerColourColour(context)
        Key.COLOUR to "greys" -> Strings.readerColourGreys(context)
        Key.LOOK to "paper" -> Strings.readerLookPaper(context)
        Key.LOOK to "light" -> Strings.readerLookLight(context)
        Key.LOOK to "sepia" -> Strings.readerLookSepia(context)
        Key.LOOK to "dark" -> Strings.readerLookDark(context)
        Key.FONT to "serif" -> Strings.readerFontSerif(context)
        Key.FONT to "sans" -> Strings.readerFontSans(context)
        Key.FIT to "screen" -> Strings.readerFitScreen(context)
        Key.FIT to "width" -> Strings.readerFitWidth(context)
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
        else -> name.replaceFirstChar(Char::uppercase)
    }
}
