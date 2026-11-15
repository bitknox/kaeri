// Crash reports on the TV, in Settings, About: the switch, Ask after a crash, the saved reports,
// and Report a bug, a page with the summary and a QR code of the issue link. The summary is cut to
// 900 bytes so the code stays readable from the sofa.

package app.kaeri.tv.screens

import android.content.Context
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import app.kaeri.design.ObiSilk
import app.kaeri.engine.core.SettingValue
import app.kaeri.engine.model.DeviceClass
import app.kaeri.engine.model.SettingKey
import app.kaeri.i18n.R
import app.kaeri.store.AppStore
import app.kaeri.store.BugContext
import app.kaeri.store.BugReportDevice
import app.kaeri.store.BugReports
import app.kaeri.tv.remote.QrCode
import app.kaeri.tv.ui.ActionSheet
import app.kaeri.tv.ui.LocalActions
import app.kaeri.tv.ui.LocalAppStore
import app.kaeri.tv.ui.TvListRow

/** The issue address is cut so that its summary is at most this many bytes. */
internal const val TV_REPORT_BYTES = 900

private fun device(context: Context): BugReportDevice {
    val version = runCatching { context.packageManager.getPackageInfo(context.packageName, 0).versionName }.getOrNull().orEmpty()
    val build = if (context.packageName.endsWith(".fixtures")) "fixtures" else "release"
    return BugReportDevice(appVersion = version, osVersion = android.os.Build.VERSION.RELEASE.orEmpty(), device = DeviceClass.TV, build = build)
}

/** The Report a bug page: the exact summary, and a code that opens the issue on the phone. */
@Composable
private fun ReportPage(store: AppStore, ticked: List<String>) {
    val context = LocalContext.current
    val device = device(context)
    val providers by store.providers.rows.collectAsStateWithLifecycle()
    val log by store.notices.log.collectAsStateWithLifecycle()
    val url = BugReports.urlCapped(device, providers, log.firstOrNull { it.isAlert }?.let { BugContext("message", it.notice.text.orEmpty()) }, ticked, TV_REPORT_BYTES)
    val text = BugReports.text(device, providers, null, ticked)
    Row(horizontalArrangement = Arrangement.spacedBy(32.dp), verticalAlignment = Alignment.Top) {
        QrCode(url, label = stringResource(R.string.report_tv_scan))
        Column(Modifier.widthIn(max = 520.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(stringResource(R.string.report_tv_scan), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk)
            Text(text, style = MaterialTheme.typography.bodySmall.copy(fontFamily = FontFamily.Monospace), color = ObiSilk.Haze)
            Text(stringResource(R.string.report_privacy), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
        }
    }
}

/** About's Crash reports rows. */
@Composable
internal fun CrashRows(store: AppStore) {
    val view by store.settings.view.collectAsStateWithLifecycle()
    val saved by store.crashes.saved.collectAsStateWithLifecycle()
    val saving = store.settings.isOn(SettingKey.SAVE_CRASH_REPORTS, view)
    val actions = LocalActions.current
    val context = LocalContext.current
    val resources = LocalResources.current
    LaunchedEffect(saving) { store.crashes.refresh() }
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        TvListRow(
            stringResource(R.string.settings_crash_save),
            onClick = { store.setSetting(SettingKey.SAVE_CRASH_REPORTS, SettingValue.Bool(!saving)) },
            supporting = stringResource(R.string.settings_crash_save_detail),
            supportingLines = 3,
            height = 96.dp,
            switch = saving,
        )
        if (saving) {
            TvListRow(
                stringResource(R.string.settings_crash_ask),
                onClick = { store.setSetting(SettingKey.ASK_AFTER_CRASH, SettingValue.Bool(!store.settings.isOn(SettingKey.ASK_AFTER_CRASH, view))) },
                supporting = stringResource(R.string.settings_crash_ask_detail),
                switch = store.settings.isOn(SettingKey.ASK_AFTER_CRASH, view),
            )
        }
        if (saving || saved.isNotEmpty()) {
            TvListRow(
                stringResource(R.string.settings_crash_saved),
                onClick = { store.crashes.deleteAll() },
                supporting = if (saved.isEmpty()) stringResource(R.string.settings_crash_none) else stringResource(R.string.settings_crash_delete_all),
                value = saved.size.toString(),
                enabled = saved.isNotEmpty(),
            )
        }
        TvListRow(
            stringResource(R.string.about_report_bug),
            onClick = {
                val ids = if (saving) saved.map { it.id }.take(1) else emptyList()
                actions?.open(ActionSheet(resources.getString(R.string.report_title), width = 720.dp, body = { ReportPage(store, ids) }))
            },
            supporting = if (saving) null else stringResource(R.string.report_crash_off),
            supportingLines = 2,
            height = if (saving) 56.dp else 72.dp,
        )
        Text(stringResource(R.string.settings_crash_footnote), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp))
    }
}
