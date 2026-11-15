// Report a bug's Player log: the last row of Included, the line under it while it is ticked (Save
// log… through the system's file picker, then the file's name), and What is sent, which gains a
// Report and a Player log segment while the log is ticked.

package app.kaeri.ui

import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.Crossfade
import androidx.compose.animation.core.LinearOutSlowInEasing
import androidx.compose.animation.core.tween
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Checkbox
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.kaeri.design.Motion
import app.kaeri.engine.model.BugReport
import app.kaeri.engine.model.PlayerLogStats
import app.kaeri.i18n.Strings
import app.kaeri.store.BugReports
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/** The Player log row's state for one Report: what the log holds, the tick, and how the save went. */
@Stable
internal class PlayerLogState {
    /** What the log holds, once the engine has said; null for the moment the dialog opens. */
    var stats by mutableStateOf<PlayerLogStats?>(null)

    var ticked by mutableStateOf(false)

    /** The saved file's name, once Save log… wrote it. */
    var saved by mutableStateOf<String?>(null)

    /** The last save could not write the file. */
    var failed by mutableStateOf(false)

    /** Open on GitHub asked for the save: GitHub opens once the file is written. */
    var openAfterSave by mutableStateOf(false)

    /** Something has played, so there is a log to tick. */
    val logged: Boolean get() = (stats?.playbacks ?: 0u) > 0u

    fun tick(on: Boolean) {
        ticked = on && logged
        // Unticked, the line closes; a file already saved stays where it is.
        if (!on) {
            saved = null
            failed = false
        }
    }
}

/** Where the text in Player log's line starts: past the checkbox's 48 dp target. */
private val LogIndent = 48.dp

/**
 * Player log, the last row of Included: "The last 3 playbacks · 412 kB", or disabled with "No
 * playback logged yet". Ticked, Save log… and the attach sentence open under it.
 */
@Composable
internal fun PlayerLogRow(log: PlayerLogState, onSave: () -> Unit) {
    val context = LocalContext.current
    val colors = MaterialTheme.colorScheme
    val stats = log.stats
    val enabled = log.logged
    val title = Strings.reportPlayerLog(context)
    val detail = when {
        stats == null -> ""
        enabled -> Strings.reportPlayerLogDetail(context, stats.playbacks.toInt(), android.text.format.Formatter.formatShortFileSize(context, stats.bytes.toLong()))
        else -> Strings.reportPlayerLogEmpty(context)
    }
    val hint = Strings.reportPlayerLogA11yHint(context)
    Column {
        Row(
            Modifier
                .fillMaxWidth()
                .toggleable(value = log.ticked, enabled = enabled, role = Role.Checkbox, onValueChange = log::tick)
                .semantics { contentDescription = listOf(title, detail, hint.takeIf { enabled }).filter { !it.isNullOrEmpty() }.joinToString(", ") },
            verticalAlignment = Alignment.Top,
        ) {
            Box(Modifier.size(48.dp), contentAlignment = Alignment.Center) {
                Checkbox(log.ticked, onCheckedChange = null, enabled = enabled)
            }
            Column(Modifier.padding(top = 12.dp, bottom = 8.dp).alpha(if (enabled || stats == null) 1f else 0.45f)) {
                Text(title, style = MaterialTheme.typography.bodyLarge)
                Text(detail, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant)
            }
        }
        // The line opens on the standard spring, its words 60 ms later, so no words sit in a gap
        // too small for them.
        AnimatedVisibility(
            log.ticked,
            enter = expandVertically(Motion.standard()) + fadeIn(tween(durationMillis = 200, delayMillis = 60, easing = LinearOutSlowInEasing)),
            exit = fadeOut(Motion.fadeOut()) + shrinkVertically(Motion.standard()),
        ) {
            SaveLine(log, onSave)
        }
        // Screen readers hear "Player log saved" once the file is written.
        val said = if (log.saved != null) Strings.reportPlayerLogA11ySaved(context) else ""
        Box(Modifier.size(1.dp).semantics { liveRegion = LiveRegionMode.Polite; contentDescription = said })
    }
}

/** Save log… (with the failure beside it), crossfading to "Saved as …"; the attach sentence under both. */
@Composable
private fun SaveLine(log: PlayerLogState, onSave: () -> Unit) {
    val context = LocalContext.current
    val colors = MaterialTheme.colorScheme
    Column(Modifier.fillMaxWidth().padding(start = LogIndent, bottom = 4.dp)) {
        // Both layers share one height, so nothing below moves as they cross.
        Crossfade(log.saved, animationSpec = Motion.reveal(), label = "player-log-save") { saved -> // i18n-skip
            Box(Modifier.fillMaxWidth().heightIn(min = 40.dp), contentAlignment = Alignment.CenterStart) {
                if (saved != null) {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Icon(Icons.Check, contentDescription = null, tint = colors.secondary, modifier = Modifier.size(16.dp))
                        Text(Strings.reportPlayerLogSaved(context, saved), style = MaterialTheme.typography.bodyMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    }
                } else {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        FilledTonalButton(onClick = onSave, enabled = log.stats != null) { Text(Strings.reportPlayerLogSave(context)) }
                        if (log.failed) {
                            Text(Strings.reportPlayerLogFailed(context), style = MaterialTheme.typography.bodyMedium, color = colors.error, modifier = Modifier.weight(1f))
                        }
                    }
                }
            }
        }
        Text(
            Strings.reportPlayerLogAttach(context),
            style = MaterialTheme.typography.bodyMedium,
            color = colors.onSurfaceVariant,
            modifier = Modifier.padding(top = 6.dp),
        )
    }
}

/**
 * What is sent: the report's exact text in a box of its own height. While Player log is ticked,
 * Report and Player log segments sit above it; Player log shows the redacted file from its first
 * line, worked out off the main thread the first time it is asked for.
 */
@Composable
internal fun WhatIsSent(text: String, report: BugReport, logTicked: Boolean) {
    val context = LocalContext.current
    val colors = MaterialTheme.colorScheme
    var tab by remember { mutableIntStateOf(0) }
    val showingLog = logTicked && tab == 1
    val logText by produceState<String?>(null, report, showingLog) {
        if (showingLog) value = withContext(Dispatchers.Default) { runCatching { BugReports.playerLogText(report) }.getOrNull() }
    }
    Column(Modifier.fillMaxWidth().padding(top = 8.dp)) {
        AnimatedVisibility(
            logTicked,
            enter = expandVertically(Motion.standard()) + fadeIn(Motion.fadeIn()),
            exit = fadeOut(Motion.fadeOut()) + shrinkVertically(Motion.standard()),
        ) {
            SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth().padding(bottom = 10.dp)) {
                listOf(Strings.reportSentReport(context), Strings.reportPlayerLog(context)).forEachIndexed { i, label ->
                    SegmentedButton(
                        selected = (if (logTicked) tab else 0) == i,
                        onClick = { tab = i },
                        shape = SegmentedButtonDefaults.itemShape(i, 2),
                    ) { Text(label) }
                }
            }
        }
        // The box keeps its height as the text crossfades, so nothing below it moves.
        Crossfade(showingLog, animationSpec = Motion.reveal(), label = "sent-text", modifier = Modifier.fillMaxWidth().height(230.dp).clip(RoundedCornerShape(12.dp)).background(colors.surfaceContainerHigh)) { log -> // i18n-skip
            val mono = MaterialTheme.typography.bodySmall.copy(fontFamily = FontFamily.Monospace, fontSize = 12.sp, lineHeight = 17.5.sp)
            if (log) {
                val lines = remember(logText) { logText?.lines().orEmpty() }
                LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(12.dp)) {
                    items(lines) { line -> Text(line, style = mono) }
                }
            } else {
                SelectionContainer(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(12.dp)) {
                    Text(text, style = mono)
                }
            }
        }
    }
}

/**
 * Writes the player log for [report] to [uri], the file the picker made, and answers its name as
 * the picker's folder shows it, "" when it does not say, or null when it could not be written.
 */
internal fun writePlayerLog(context: Context, uri: Uri, report: BugReport): String? = runCatching {
    val body = BugReports.playerLogText(report)
    val out = context.contentResolver.openOutputStream(uri, "w") ?: error("no stream for the file")
    out.use { it.write(body.toByteArray()) }
    context.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { cursor ->
        if (cursor.moveToFirst()) cursor.getString(0) else null
    }.orEmpty()
}.getOrNull()
