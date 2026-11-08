// The phone's navigation bar on an e-ink panel, as the e-ink frames of the discovery board draw it:
// words only on paper, 62 tall under a 2 dp rule, the selected one in bold ink with a 3 dp line
// under it. The same destinations as the phone's bar, in the same order.

package app.kaeri.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.kaeri.eink.EinkColours
import app.kaeri.eink.EinkSizes
import app.kaeri.eink.EinkType
import app.kaeri.engine.model.KaeriIsFor

/**
 * The e-ink bar: [current]'s destination selected, a tap goes there at once. On an e-reader
 * ([reader]) with Read alone, the board's five tabs with Downloads and More.
 */
@Composable
internal fun EinkTabBar(current: Destination, answer: KaeriIsFor, reader: Boolean, go: (Destination) -> Unit) {
    val owner = einkBarOwner(current, answer, reader)
    Column(
        Modifier
            .fillMaxWidth()
            .background(EinkColours.Paper)
            .windowInsetsPadding(WindowInsets.navigationBars),
    ) {
        Box(Modifier.fillMaxWidth().height(EinkSizes.Rule).background(EinkColours.Ink))
        Row(Modifier.fillMaxWidth().heightIn(min = EinkSizes.BottomNav).selectableGroup()) {
            for (destination in einkBarDestinations(answer, reader)) {
                EinkTab(stringResource(destination.labelFor(answer)), destination == owner, Modifier.weight(1f)) { go(destination) }
            }
        }
    }
}

@Composable
private fun EinkTab(label: String, selected: Boolean, modifier: Modifier, onClick: () -> Unit) {
    var baseline by remember { mutableFloatStateOf(0f) }
    Box(
        modifier
            .heightIn(min = EinkSizes.BottomNav)
            .selectable(selected = selected, role = Role.Tab, onClick = onClick)
            .padding(horizontal = 4.dp, vertical = 10.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            label,
            style = EinkType.Secondary.copy(
                fontSize = 15.sp,
                fontWeight = if (selected) FontWeight.Bold else FontWeight.Medium,
                color = if (selected) EinkColours.Ink else EinkColours.Secondary,
            ),
            textAlign = TextAlign.Center,
            maxLines = 2,
            onTextLayout = { baseline = it.lastBaseline },
            // The board's underline: 3 thick, 8 below the line it marks.
            modifier = Modifier.drawBehind {
                if (selected && baseline > 0f) {
                    val thick = 3.dp.toPx()
                    drawRect(EinkColours.Ink, Offset(0f, baseline + 8.dp.toPx() - thick / 2), Size(size.width, thick))
                }
            },
        )
    }
}
