// Crash reports, in Settings, About: the switch, the system's records offered after it is turned
// on, and the saved reports. And Report a bug, the full-screen dialog that opens the pre-filled
// issue only when the viewer chooses to.

package app.kaeri.ui

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.design.Motion
import app.kaeri.engine.model.CrashPlace
import app.kaeri.engine.model.CrashSummary
import app.kaeri.engine.model.SettingKey
import app.kaeri.i18n.R
import app.kaeri.store.BugReports
import app.kaeri.store.bugReport
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/** "30 October, 21:14", in the viewer's language. */
internal fun crashWhen(atMs: Long): String {
    val pattern = android.text.format.DateFormat.getBestDateTimePattern(java.util.Locale.getDefault(), "d MMMM HH:mm")
    return java.text.SimpleDateFormat(pattern, java.util.Locale.getDefault()).format(java.util.Date(atMs))
}

@Composable
internal fun crashPlaceWords(place: CrashPlace): String = stringResource(
    when (place) {
        CrashPlace.HOME -> R.string.crash_place_home
        CrashPlace.PLAYER -> R.string.crash_place_player
        CrashPlace.TITLE -> R.string.crash_place_title
        CrashPlace.SEARCH -> R.string.crash_place_search
        CrashPlace.SETTINGS -> R.string.crash_place_settings
        CrashPlace.OTHER -> R.string.crash_place_other
    },
)

/** The Crash reports group of About. The switch is off until the viewer turns it on. */
@Composable
internal fun CrashReportsGroup() {
    val store = LocalAppStore.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val saved by store.crashes.saved.collectAsStateWithLifecycle()
    val held by store.crashes.held.collectAsStateWithLifecycle()
    val saving = store.settings.isOn(SettingKey.SAVE_CRASH_REPORTS, view)
    var viewing by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(saving) { store.crashes.refresh() }
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        SettingsGroup(
            title = stringResource(R.string.settings_group_crashes),
            rows = listOfNotNull<@Composable (Place) -> Unit>(
                { place -> SettingSwitch(place, SettingKey.SAVE_CRASH_REPORTS, stringResource(R.string.settings_crash_save), note = stringResource(R.string.settings_crash_save_detail)) },
            ),
        )
        AnimatedVisibility(saving, enter = expandVertically(Motion.standard()) + fadeIn(Motion.fadeIn()), exit = shrinkVertically(Motion.standard()) + fadeOut(Motion.fadeOut())) {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                SettingsGroup(
                    rows = listOf<@Composable (Place) -> Unit>(
                        { place -> SettingSwitch(place, SettingKey.ASK_AFTER_CRASH, stringResource(R.string.settings_crash_ask), note = stringResource(R.string.settings_crash_ask_detail)) },
                    ),
                )
                if (held.isNotEmpty()) {
                    SettingsGroup(
                        title = stringResource(R.string.settings_crash_os_heading, "Android"), // i18n-skip
                        footer = stringResource(R.string.settings_crash_os_detail, "Android"), // i18n-skip
                        rows = held.map { crash ->
                            @Composable { place: Place ->
                                InfoRow(
                                    place,
                                    stringResource(R.string.settings_crash_os_item, crashWhen(crash.atMs)),
                                    null,
                                    trailing = {
                                        Row {
                                            TextButton(onClick = { viewing = crash.id }) { Text(stringResource(R.string.settings_crash_view), color = MaterialTheme.colorScheme.onSurface) }
                                            TextButton(onClick = { store.crashes.addHeld(crash.id) }) { Text(stringResource(R.string.settings_crash_os_add), color = MaterialTheme.colorScheme.onSurface) }
                                        }
                                    },
                                )
                            }
                        },
                    )
                }
            }
        }
        if (saving || saved.isNotEmpty()) {
            SettingsGroup(
                title = stringResource(R.string.settings_crash_saved),
                rows = if (saved.isEmpty()) {
                    listOf { place -> InfoRow(place, stringResource(R.string.settings_crash_none), null) }
                } else {
                    saved.map { crash ->
                        @Composable { place: Place ->
                            InfoRow(
                                place,
                                stringResource(R.string.report_crash_item, crashWhen(crash.atMs), crashPlaceWords(crash.place)),
                                null,
                                trailing = {
                                    Row {
                                        TextButton(onClick = { viewing = crash.id }) { Text(stringResource(R.string.settings_crash_view), color = MaterialTheme.colorScheme.onSurface) }
                                        TextButton(onClick = { store.crashes.delete(crash.id) }) { Text(stringResource(R.string.settings_crash_delete), color = MaterialTheme.colorScheme.onSurface) }
                                    }
                                },
                            )
                        }
                    } + listOf<@Composable (Place) -> Unit>({ place -> ActionRow(place, stringResource(R.string.settings_crash_delete_all)) { store.crashes.deleteAll() } })
                },
            )
        }
        SettingsFootnote(stringResource(R.string.settings_crash_footnote))
    }
    viewing?.let { id ->
        val text = remember(id) { store.crashes.text(id).orEmpty() }
        AlertDialog(
            onDismissRequest = { viewing = null },
            text = { SelectionContainer { Text(text, fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodySmall, modifier = Modifier.verticalScroll(rememberScrollState())) } },
            confirmButton = { QuietButton(stringResource(R.string.action_done)) { viewing = null } },
            containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
        )
    }
}

/**
 * Report a bug: what is included, the exact text, and Open on GitHub. Crashes from the last
 * seven days are ticked only when the dialog opened from the crash notice. Player log is the last
 * row of Included: ticked, it is saved as a file through the system's picker, for the reporter to
 * attach on GitHub; Open on GitHub saves it first.
 */
@Composable
internal fun ReportBugDialog() {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val request by store.crashes.report.collectAsStateWithLifecycle()
    val asked = request ?: return
    val view by store.settings.view.collectAsStateWithLifecycle()
    val saving = store.settings.isOn(SettingKey.SAVE_CRASH_REPORTS, view)
    val saved by store.crashes.saved.collectAsStateWithLifecycle()
    val recent = remember(saved) { saved.filter { System.currentTimeMillis() - it.atMs < 7L * 24 * 3600 * 1000 } }
    var error by rememberSaveable(asked) { mutableStateOf(true) }
    var ticked by remember(asked) { mutableStateOf(asked.ticked) }
    var shown by rememberSaveable(asked) { mutableStateOf(false) }
    val navigator = LocalNavigator.current
    val device = remember { bugReportDevice(context) }
    val problem = if (error) asked.problem else null
    val ids = recent.map { it.id }.filter { it in ticked }
    val log = remember(asked) { PlayerLogState() }
    // What the log holds, and whether it starts ticked: from a playback error or with a crash in
    // the player. The engine may read the log's file for it, so off the main thread.
    LaunchedEffect(asked) {
        val opening = store.bugReport(device, asked.problem, asked.ticked.toList())
        val (stats, starts) = withContext(Dispatchers.IO) {
            val stats = BugReports.playerLogStats()
            stats to (stats.playbacks > 0u && BugReports.playerLogStartsTicked(opening))
        }
        log.stats = stats
        if (starts) log.ticked = true
    }
    val report = remember(problem, ids, log.ticked) { store.bugReport(device, problem, ids, log.ticked) }
    val text = remember(report) { BugReports.text(report) }
    var copied by remember(asked, ticked, error, log.ticked) { mutableStateOf(false) }
    val scope = rememberCoroutineScope()
    val open = {
        openIssue(context, BugReports.url(report))
        store.crashes.closeReport()
    }
    val saveLog = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("text/plain")) { uri: Uri? ->
        if (uri == null) {
            // Cancelled: nothing is written, and GitHub does not open.
            log.openAfterSave = false
            return@rememberLauncherForActivityResult
        }
        scope.launch {
            val name = withContext(Dispatchers.IO) { writePlayerLog(context, uri, report) }
            if (name == null) {
                log.failed = true
                log.openAfterSave = false
                return@launch
            }
            log.failed = false
            log.saved = name.ifEmpty { log.stats?.fileName.orEmpty() }
            if (log.openAfterSave) {
                log.openAfterSave = false
                openIssue(context, BugReports.url(store.bugReport(device, problem, ids, playerLog = true)))
                store.crashes.closeReport()
            }
        }
    }
    val save: () -> Unit = { log.stats?.fileName?.let { saveLog.launch(it) } }
    Dialog(onDismissRequest = store.crashes::closeReport, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        androidx.compose.material3.Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.surface) {
            Column(
                Modifier.fillMaxSize().windowInsetsPadding(androidx.compose.foundation.layout.WindowInsets.safeDrawing).verticalScroll(rememberScrollState()).padding(horizontal = 24.dp, vertical = 24.dp).readingWidth(),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Text(stringResource(R.string.report_title), style = MaterialTheme.typography.headlineSmall)
                Text(stringResource(R.string.report_detail), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Text(stringResource(R.string.report_included), style = MaterialTheme.typography.titleMedium)
                Column {
                    IncludedRow(stringResource(R.string.report_versions), checked = true, enabled = false) {}
                    if (asked.problem != null) IncludedRow(stringResource(R.string.report_last_error), error) { error = it }
                    if (saving) {
                        recent.forEach { crash ->
                            IncludedRow(
                                stringResource(R.string.report_crash_item, crashWhen(crash.atMs), crashPlaceWords(crash.place)),
                                crash.id in ticked,
                            ) { on -> ticked = if (on) ticked + crash.id else ticked - crash.id }
                        }
                    } else {
                        Text(stringResource(R.string.report_crash_off), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        TextButton(onClick = {
                            store.crashes.closeReport()
                            navigator.openSettings(SettingsPane.About)
                        }) { Text(stringResource(R.string.report_crash_settings), color = MaterialTheme.colorScheme.onSurface) }
                    }
                    PlayerLogRow(log, onSave = save)
                }
                TextButton(onClick = { shown = !shown }) { Text(stringResource(if (shown) R.string.report_hide_sent else R.string.report_show_sent), color = MaterialTheme.colorScheme.onSurface) }
                AnimatedVisibility(shown, enter = expandVertically(Motion.standard()) + fadeIn(Motion.fadeIn()), exit = shrinkVertically(Motion.standard()) + fadeOut(Motion.fadeOut())) {
                    WhatIsSent(text, report, log.ticked)
                }
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End)) {
                    OutlinedButton(onClick = {
                        context.getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText("report", text))
                        copied = true
                    }) { Text(stringResource(if (copied) R.string.report_copied else R.string.report_copy)) }
                    TextButton(onClick = store.crashes::closeReport) { Text(stringResource(R.string.action_cancel), color = MaterialTheme.colorScheme.onSurface) }
                    Button(onClick = {
                        // A ticked log not saved yet is saved first; GitHub opens once it is.
                        if (log.ticked && log.saved == null) {
                            log.openAfterSave = true
                            save()
                        } else {
                            open()
                        }
                    }) { Text(stringResource(R.string.report_open_github)) }
                }
                Text(stringResource(R.string.report_privacy), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}

@Composable
private fun IncludedRow(label: String, checked: Boolean, enabled: Boolean = true, onChange: (Boolean) -> Unit) {
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Checkbox(checked, onChange, enabled = enabled)
        Text(label, style = MaterialTheme.typography.bodyLarge)
    }
}

/**
 * The notice after a crash, 1 s after the app shows, with Report… as its button, and the Report a
 * bug dialog. Nothing asks while saving is off: the engine gives no crash then.
 */
@Composable
internal fun CrashNoticeAndReport() {
    val store = LocalAppStore.current
    val snackbars = LocalSnackbars.current
    val context = LocalContext.current
    val resources = LocalResources.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val saving = view != null && store.settings.isOn(SettingKey.SAVE_CRASH_REPORTS, view)
    LaunchedEffect(saving) {
        if (!saving) return@LaunchedEffect
        kotlinx.coroutines.delay(1_000)
        val crash = store.crashes.pendingNotice() ?: return@LaunchedEffect
        snackbars.show(resources.getString(R.string.crash_notice), resources.getString(R.string.crash_notice_report)) {
            openBugReport(context, store, null, setOf(crash.id))
        }
    }
    ReportBugDialog()
}
