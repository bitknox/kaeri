// Stats for nerds on the phone (player-stats.md), after the iPhone's StatsPanel.swift: the
// engine's rows on the player's control background, top trailing over the picture, below the
// top bar's buttons. The engine words every row and picks the one amber value; this only lays
// them out, with Copy stats and a close button. No border, shadow or blur of its own.

package app.kaeri.player.ui

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.os.Build
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.kaeri.design.ObiSilk
import app.kaeri.design.StatsLine
import app.kaeri.design.StatsRows
import app.kaeri.engine.model.StatsAbout
import app.kaeri.i18n.R
import app.kaeri.player.PlayerModel
import app.kaeri.player.StatsPanelState

/** The panel's width beside the picture: the usual values keep to one line. */
internal val STATS_WIDTH = 400.dp

/**
 * The panel, from [panel]. It stays up while the controls hide; values change in place, in
 * tabular digits, so nothing moves as they tick. Offered less height than its rows need, the rows
 * scroll between the title and Copy stats. [compact], on a phone held sideways, moves Copy stats
 * up beside the close button and sets the rows closer, so the panel ends above the subtitles.
 */
@Composable
internal fun StatsPanel(model: PlayerModel, panel: StatsPanelState, width: Dp, compact: Boolean, modifier: Modifier = Modifier) {
    val title = stringResource(R.string.player_stats)
    val text = TextStyle(fontSize = if (compact) 12.sp else 13.sp, lineHeight = if (compact) 16.sp else 18.sp, color = ObiSilk.Silk)
    val waiting = model.statsWaitingSince != null
    Column(
        modifier
            .width(width)
            .background(ObiSilk.Raised, RoundedCornerShape(16.dp))
            .padding(start = 14.dp, end = 6.dp, top = 6.dp, bottom = 12.dp)
            .semantics { paneTitle = title },
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(title, style = MaterialTheme.typography.titleSmall, color = ObiSilk.Silk, modifier = Modifier.weight(1f))
            if (compact) CopyStats(model, waiting)
            val close = stringResource(R.string.player_stats_close)
            IconButton(onClick = { model.showStats(false) }, modifier = Modifier.size(40.dp).semantics { contentDescription = close }) {
                Icon(PlayerIcons.Close, contentDescription = null, tint = ObiSilk.Silk, modifier = Modifier.size(18.dp))
            }
        }
        Column(Modifier.padding(end = 8.dp)) {
            if (panel.waitingLine) {
                Text(stringResource(R.string.player_stats_waiting), style = text, color = ObiSilk.Haze, modifier = Modifier.padding(top = 2.dp))
            }
            Box(Modifier.weight(1f, fill = false).padding(top = if (compact) 4.dp else 8.dp).verticalScroll(rememberScrollState())) {
                StatsRows(panel.rows.map { StatsLine(it.label, it.value, it.warn) }, text, rowGap = if (compact) 3.dp else 4.dp)
            }
            if (!compact) Box(Modifier.padding(top = 10.dp)) { CopyStats(model, waiting) }
        }
    }
}

/** Copy stats, a tonal pill as the player's other buttons, and "Copied" in its place a moment after. */
@Composable
private fun CopyStats(model: PlayerModel, waiting: Boolean) {
    if (model.statsCopiedAt != null) {
        // Where the pill's word was.
        Box(Modifier.height(40.dp).padding(horizontal = 16.dp), contentAlignment = Alignment.Center) {
            Text(stringResource(R.string.action_copied), style = MaterialTheme.typography.labelLarge, color = ObiSilk.Haze)
        }
    } else {
        FilledTonalButton(
            onClick = { model.copyStats() },
            enabled = !waiting,
            modifier = Modifier.height(40.dp).alpha(if (waiting) 0.4f else 1f),
            contentPadding = PaddingValues(horizontal = 16.dp),
            colors = ButtonDefaults.filledTonalButtonColors(
                containerColor = ObiSilk.RaisedSelected,
                contentColor = ObiSilk.Silk,
                disabledContainerColor = ObiSilk.RaisedSelected,
                disabledContentColor = ObiSilk.Silk,
            ),
        ) { Text(stringResource(R.string.player_stats_copy), style = MaterialTheme.typography.labelLarge) }
    }
}

/** This app, system and device, for Copy stats: nothing that names the viewer. */
internal fun statsAbout(context: Context): StatsAbout {
    val version = runCatching { context.packageManager.getPackageInfo(context.packageName, 0).versionName }.getOrNull().orEmpty()
    return StatsAbout(appVersion = version, system = "Android ${Build.VERSION.RELEASE}", device = Build.MODEL.orEmpty()) // i18n-skip
}

/** The system's clipboard, for Copy stats' lines. */
internal fun copyToClipboard(context: Context, text: String) {
    context.getSystemService(ClipboardManager::class.java)?.setPrimaryClip(ClipData.newPlainText("stats", text)) // i18n-skip
}
