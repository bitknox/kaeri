// The Tap zones page of the reader's settings sheet, from the Navigation group's row: Paged and
// Scrolling as a segmented button, opened on the kind of reading that is open; the layouts as
// tiles drawn from the engine's cells for the open book's direction, one radio group; Zone size;
// the two flips; and Show zones beside where the setting is kept. Every change is live.

package app.kaeri.reader.phone

import android.content.Context
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
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
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.kaeri.engine.model.ReaderSettingKey as Key
import app.kaeri.engine.model.ReaderSettingRow
import app.kaeri.engine.model.ReaderValue
import app.kaeri.engine.model.SettingScope
import app.kaeri.engine.model.TapFlow
import app.kaeri.engine.model.ZoneLayout
import app.kaeri.engine.model.ZoneSize
import app.kaeri.engine.model.tapMap
import app.kaeri.i18n.Strings
import app.kaeri.reader.ReaderSession
import app.kaeri.reader.ReaderState
import app.kaeri.reader.TapZoneDiagram
import app.kaeri.reader.configName
import app.kaeri.reader.forKind
import app.kaeri.reader.layoutName
import app.kaeri.reader.rememberScreenReader
import app.kaeri.reader.tapReading
import app.kaeri.reader.tapZones
import app.kaeri.reader.zoneChoices

/** The Tap zones row's value: the layout chosen for the kind of reading that is open. */
fun tapZonesValue(context: Context, state: ReaderState, eink: Boolean = false): String? {
    val reading = state.tapReading() ?: return null
    return layoutName(context, tapMap(state.tapZones(eink), reading, null).choice)
}

@Composable
internal fun TapZonesPage(
    context: Context,
    session: ReaderSession,
    state: ReaderState,
    rows: List<ReaderSettingRow>,
    toggle: (Key, Boolean) -> Unit,
    onShowZones: () -> Unit,
    onBack: () -> Unit,
) {
    BackRow(Strings.readerRowTap(context), context, onBack)
    val zones = state.tapZones()
    val open = state.tapReading() ?: return
    val openScrolling = tapMap(zones, open, null).flow == TapFlow.SCROLLING
    val scrollRow = rows.firstOrNull { it.key == Key.TAP_SCROLL }
    // The page opens on the kind of reading that is open.
    var scrolling by remember { mutableStateOf(openScrolling && scrollRow != null) }
    if (scrollRow != null) {
        Spacer(Modifier.height(6.dp))
        Segments(listOf(Strings.readerTapPaged(context), Strings.readerTapScrolling(context)), if (scrolling) 1 else 0) { scrolling = it == 1 }
        Spacer(Modifier.height(14.dp))
    }
    val key = if (scrolling) Key.TAP_SCROLL else Key.TAP_PAGED
    val row = rows.firstOrNull { it.key == key }
    val chosen = if (scrolling) zones.scrolling else zones.paged
    val reading = open.forKind(scrolling)
    val choices = state.zoneChoices(key)
    Column(Modifier.fillMaxWidth().selectableGroup(), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        choices.chunked(4).forEach { line ->
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                line.forEach { layout ->
                    ZoneTile(context, layout, layout == chosen, tapMap(zones, reading, layout), Modifier.weight(1f)) {
                        session.setSetting(key, row?.scope ?: SettingScope.DEVICE, ReaderValue.Choice(layout.configName))
                    }
                }
                repeat(4 - line.size) { Spacer(Modifier.weight(1f)) }
            }
        }
    }
    Spacer(Modifier.height(10.dp))
    rows.firstOrNull { it.key == Key.TAP_SIZE }?.let { size ->
        Row(Modifier.fillMaxWidth().heightIn(min = 52.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(Strings.readerTapSize(context), color = Chrome.text, fontSize = 16.sp, modifier = Modifier.weight(1f))
            val sizes = listOf(ZoneSize.STANDARD to Strings.readerTapStandard(context), ZoneSize.SMALL to Strings.readerTapSmall(context))
            Segments(sizes.map { it.second }, sizes.indexOfFirst { it.first == zones.size }, Modifier.width(196.dp)) {
                session.setSetting(Key.TAP_SIZE, size.scope, ReaderValue.Choice(if (sizes[it].first == ZoneSize.SMALL) "small" else "standard"))
            }
        }
    }
    if (rows.any { it.key == Key.TAP_FLIP_SIDES }) {
        SwitchLine(Strings.readerTapFlipSides(context), null, zones.flipSides) { toggle(Key.TAP_FLIP_SIDES, it) }
    }
    if (rows.any { it.key == Key.TAP_FLIP_ENDS }) {
        SwitchLine(Strings.readerTapFlipEnds(context), null, zones.flipEnds) { toggle(Key.TAP_FLIP_ENDS, it) }
    }
    val screenReader = rememberScreenReader()
    val kind = if (LocalConfiguration.current.screenWidthDp >= 600) Strings.deviceKindTablet(context) else Strings.deviceKindPhone(context)
    Row(Modifier.fillMaxWidth().padding(top = 10.dp), verticalAlignment = Alignment.CenterVertically) {
        // The guide cannot be seen with a screen reader, and Off has no zones to show.
        if (!screenReader) {
            val shows = tapMap(zones, open, null).layout != ZoneLayout.OFF
            TextButton(onClick = onShowZones, enabled = shows) {
                Text(Strings.readerTapShow(context), fontSize = 15.sp, fontWeight = FontWeight.Medium, color = if (shows) Chrome.text else Chrome.faint)
            }
        }
        Spacer(Modifier.weight(1f))
        Text(Strings.readerTapHere(context, kind), color = Chrome.faint, fontSize = 13.sp, textAlign = TextAlign.End)
    }
}

/**
 * One layout as a tile: its diagram 64 by 100 dp for the open book, the chosen one ringed in silk
 * with a check, and its name under it. A radio button that says the layout in words.
 */
@Composable
private fun ZoneTile(context: Context, layout: ZoneLayout, selected: Boolean, map: app.kaeri.engine.model.TapMap, modifier: Modifier, onClick: () -> Unit) {
    val name = layoutName(context, layout)
    val ring = Chrome.text
    val gap = Chrome.plate
    Column(
        modifier
            .clip(RoundedCornerShape(12.dp))
            .selectable(selected = selected, role = Role.RadioButton, onClick = onClick)
            // The layout and where its zones are; the radio's own state, Selected or Not selected,
            // is read after it, so nothing here may set a state description. The words replace the
            // name and diagram below, as Android drops a description on a tile that merges them.
            .clearAndSetSemantics { contentDescription = Strings.readerTapLayoutSpoken(context, name, map.about) }
            .padding(vertical = 4.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        val shape = RoundedCornerShape(12.dp)
        Box(
            Modifier
                .size(72.dp, 108.dp)
                .then(if (selected) Modifier.border(2.dp, ring, RoundedCornerShape(16.dp)) else Modifier)
                .padding(4.dp),
        ) {
            Box(Modifier.fillMaxSize().clip(shape).border(1.dp, Chrome.rule, shape)) {
                TapZoneDiagram(map, Modifier.fillMaxSize())
            }
            if (selected) {
                Box(
                    Modifier.align(Alignment.BottomEnd).padding(4.dp).size(16.dp).background(ring, CircleShape),
                    contentAlignment = Alignment.Center,
                ) {
                    GlyphIcon(Glyph.Check, gap, size = 12.dp)
                }
            }
        }
        Text(
            name,
            color = if (selected) Chrome.text else Chrome.faint,
            fontSize = 12.sp,
            lineHeight = 15.sp,
            fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Normal,
            textAlign = TextAlign.Center,
            maxLines = 2,
        )
    }
}
