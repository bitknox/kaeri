// Stats for nerds' rows (player-stats.md), as the phone's panel and the TV's live panel lay them
// out: labels in haze, values in tabular digits, the one problem value in amber.

package app.kaeri.design

import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.layout.FirstBaseline
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/** One row as the engine words it: "Buffer", "42 s ahead · 61 MB", amber when it is the problem now. */
class StatsLine(val label: String, val value: String, val warn: Boolean)

/**
 * Label and value, the labels' column as wide as its longest label and the values taking the
 * rest, where a long one wraps under itself; each row's first lines share a baseline. The phone's
 * panel and the TV's live panel.
 */
@Composable
fun StatsRows(rows: List<StatsLine>, style: TextStyle, rowGap: Dp, labelGap: Dp = 14.dp, modifier: Modifier = Modifier) {
    Layout(
        content = {
            for (row in rows) {
                Text(row.label, style = style, color = ObiSilk.Haze, maxLines = 1, softWrap = false)
                Text(
                    statsWrapping(row.value),
                    style = style.tabular(),
                    color = if (row.warn) ObiSilk.Amber else ObiSilk.Silk,
                    modifier = Modifier.semantics { contentDescription = "${row.label}, ${row.value}" }, // i18n-skip
                )
            }
        },
        modifier = modifier,
    ) { measurables, constraints ->
        val labels = measurables.filterIndexed { i, _ -> i % 2 == 0 }.map { it.measure(Constraints()) }
        val labelWidth = labels.maxOfOrNull { it.width } ?: 0
        val gap = labelGap.roundToPx()
        val valueWidth = (constraints.maxWidth - labelWidth - gap).coerceAtLeast(0)
        val values = measurables.filterIndexed { i, _ -> i % 2 == 1 }.map { it.measure(Constraints(maxWidth = valueWidth)) }
        val between = rowGap.roundToPx()
        var y = 0
        val tops = labels.indices.map { i ->
            val label = labels[i]
            val value = values[i]
            // The first lines on one baseline.
            val lift = (label[FirstBaseline] - value[FirstBaseline]).coerceAtLeast(0)
            val drop = (value[FirstBaseline] - label[FirstBaseline]).coerceAtLeast(0)
            val top = y
            y += maxOf(label.height + drop, value.height + lift) + if (i < labels.lastIndex) between else 0
            Triple(top + drop, top + lift, top)
        }
        val width = if (constraints.hasBoundedWidth) constraints.maxWidth else labelWidth + gap + (values.maxOfOrNull { it.width } ?: 0)
        layout(width, y) {
            labels.indices.forEach { i ->
                labels[i].place(0, tops[i].first)
                values[i].place(labelWidth + gap, tops[i].second)
            }
        }
    }
}

/**
 * [value] as it may wrap in the panel's narrow column: a number stays with its unit and a size
 * with its "×", and a " · " ends the line it is on rather than starting the next. Only for
 * showing: Copy stats keeps the engine's plain spaces.
 */
fun statsWrapping(value: String): String {
    val keep = " "
    return value
        .replace(" × ", "$keep×$keep")
        .replace(" · ", "$keep· ")
        .replace(Regex("""(\d) (fps|Hz|kHz|MB/s|kB/s|GB/s|MB|kB|GB|s\b|threads?)"""), "$1$keep$2")
        .replace(Regex(""" (\d+-bit)"""), "$keep$1")
}

