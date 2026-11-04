// Settings, Devices: this device's name, sync (off, or the devices in the group and the cloud
// folder, the recovery key and Turn off sync), and Play on TV, as the sync board draws them.

package app.kaeri.ui

import app.kaeri.design.SyncIcons
import android.content.Context
import android.os.Build
import android.provider.Settings
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.engine.model.SyncDeviceView
import app.kaeri.engine.model.SyncFolderKind
import app.kaeri.engine.model.SyncProblem
import app.kaeri.engine.model.SyncStatus
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.store.setDeviceSync
import app.kaeri.store.SyncSheet
import app.kaeri.store.Wording
import app.kaeri.store.loadSync
import app.kaeri.store.leaveSync
import app.kaeri.store.removeSyncDevice
import app.kaeri.store.showRecoveryKey
import app.kaeri.store.syncFolderNow
import app.kaeri.store.syncSignInAgain
import app.kaeri.store.syncSignOut

/** The name a device takes before the person edits it: the system's own where there is one, else the model. */
internal fun systemDeviceName(context: Context): String {
    val named = runCatching { Settings.Global.getString(context.contentResolver, "device_name") }.getOrNull()
    return named?.takeIf { it.isNotBlank() } ?: Build.MODEL.ifBlank { "Android" }
}

/** What a cloud service is called, in the app's words: the services keep their own names. */
internal fun cloudName(kind: SyncFolderKind): String = when (kind) {
    SyncFolderKind.GOOGLE_DRIVE -> "Google Drive" // i18n-skip
    SyncFolderKind.DROPBOX -> "Dropbox" // i18n-skip
    SyncFolderKind.ONE_DRIVE -> "OneDrive" // i18n-skip
    SyncFolderKind.NEXTCLOUD -> "Nextcloud" // i18n-skip
    SyncFolderKind.WEB_DAV -> "WebDAV" // i18n-skip
    SyncFolderKind.DIRECTORY -> "" // the caller says a folder in its own words
}

/** "this phone", "this tablet": the device word a sentence uses. */
@Composable
internal fun kindWord(): String = stringResource(if (wideWindow()) R.string.device_kind_tablet else R.string.device_kind_phone)

/** The icon a device wears, from what its name says it is. */
internal fun deviceIcon(name: String): ImageVector {
    val n = name.lowercase()
    return when {
        "tv" in n || "television" in n -> SyncIcons.Tv
        "ipad" in n || "tablet" in n -> SyncIcons.Tablet
        "mac" in n || "laptop" in n || "notebook" in n -> SyncIcons.Laptop
        "pc" in n || "desktop" in n || "studio" in n -> SyncIcons.Desktop
        else -> SyncIcons.Phone
    }
}

/** "Synced 2 min ago", "Synced just now", or "Not seen for 41 days" past 30 days. */
internal fun lastSeenLine(context: Context, lastSeen: Long?, now: Long = System.currentTimeMillis() / 1000): String? {
    lastSeen ?: return null
    val seconds = (now - lastSeen).coerceAtLeast(0)
    val days = seconds / 86_400
    return if (days > 30) Strings.devicesNotSeen(context, days.toInt()) else Strings.devicesRowSynced(context, Wording.ago(context, seconds))
}

/** Devices: the page behind the Settings row of the same name. */
@Composable
internal fun DevicesPane(actions: AppActions) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val status by store.sync.status.collectAsStateWithLifecycle()
    val settingUp by store.sync.settingUp.collectAsStateWithLifecycle()
    var removing by remember { mutableStateOf<SyncDeviceView?>(null) }
    var turningOff by remember { mutableStateOf(false) }
    var opened by remember { mutableStateOf<SyncDeviceView?>(null) }
    LaunchedEffect(Unit) { store.loadSync() }
    val synced = status
    Pane {
        SettingsGroup(
            title = stringResource(R.string.devices_this_device),
            rows = listOf { place ->
                InfoRow(place, stringResource(R.string.devices_name), synced?.deviceName ?: systemDeviceName(context), onClick = { store.sync.show(SyncSheet.NAME) })
            },
        )
        if (synced?.enabled != true) {
            SyncOffGroup()
            TvSettingsGroup()
        } else {
            YourDevices(synced, onOpen = { opened = it }, onRemove = { removing = it })
            CloudGroup(synced, settingUp)
            if (synced.recoveryKey) RecoveryKeyRow(synced)
            TvSettingsGroup()
            SettingsGroup(
                rows = listOf { place -> ActionRow(place, stringResource(R.string.sync_turn_off, kindWord())) { turningOff = true } },
            )
        }
    }
    DevicesSheets()
    removing?.let { device ->
        ConfirmAlert(
            title = stringResource(R.string.devices_remove_alert, device.name),
            text = stringResource(R.string.devices_remove_alert_line, device.name),
            confirm = stringResource(R.string.devices_remove),
            onDismiss = { removing = null },
        ) { store.removeSyncDevice(device.id); removing = null }
    }
    if (turningOff) {
        val name = synced?.deviceName ?: systemDeviceName(context)
        ConfirmAlert(
            title = stringResource(R.string.sync_turn_off_alert, name),
            text = stringResource(R.string.sync_turn_off_alert_line, kindWord()),
            confirm = stringResource(R.string.sync_turn_off, kindWord()),
            onDismiss = { turningOff = false },
        ) { store.leaveSync(); turningOff = false }
    }
    opened?.let { shown -> val device = store.sync.status.value?.devices?.firstOrNull { it.id == shown.id } ?: shown; DevicePage(device, onRemove = { removing = device; opened = null }) { opened = null } }
}

/** Sync is off: what it does, and the button that asks how the devices should share. */
@Composable
private fun SyncOffGroup() {
    val store = LocalAppStore.current
    val colors = MaterialTheme.colorScheme
    Column {
        SettingsHeader(stringResource(R.string.sync_group))
        Column(
            Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(16.dp))
                .background(colors.surfaceContainer)
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text(stringResource(R.string.sync_off_title), style = MaterialTheme.typography.titleMedium)
            Text(stringResource(R.string.sync_off_line), style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant)
            Row(Modifier.fillMaxWidth().padding(top = 8.dp), horizontalArrangement = Arrangement.End) {
                Button(onClick = { store.sync.show(SyncSheet.ROAD) }) { Text(stringResource(R.string.sync_turn_on)) }
            }
        }
    }
}

/** The devices in the group, each with its line and a Sync switch, then Add a device. */
@Composable
private fun YourDevices(status: SyncStatus, onOpen: (SyncDeviceView) -> Unit, onRemove: (SyncDeviceView) -> Unit) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val tvs by store.tv.paired.collectAsStateWithLifecycle()
    val others = status.devices.filterNot { it.thisDevice }
    val remoteOnly = tvs.filter { tv -> others.none { it.name == tv.name } }
    SettingsGroup(
        title = stringResource(R.string.devices_yours),
        rows = buildList<@Composable (Place) -> Unit> {
            if (others.isEmpty() && remoteOnly.isEmpty()) {
                add { place ->
                    InfoRow(place, stringResource(R.string.devices_only_this), stringResource(R.string.devices_only_this_line))
                }
            }
            others.forEach { device ->
                add { place ->
                    val line = listOfNotNull(
                        lastSeenLine(context, device.lastSeen),
                        if (device.remoteKey != null && !device.remoteOff) stringResource(R.string.devices_remote_switch) else null,
                    ).joinToString(" · ") // i18n-skip
                    DeviceRow(place, device.name, line, status.error == null, on = !device.paused, onClick = { onOpen(device) }, onSwitch = { store.setDeviceSync(device.name, device.paused) })
                }
            }
            remoteOnly.forEach { tv ->
                add { place ->
                    InfoRow(
                        place,
                        tv.name,
                        stringResource(R.string.devices_remote_only),
                        leading = { Icon(SyncIcons.Tv, null, tint = MaterialTheme.colorScheme.onSurfaceVariant) },
                        trailing = { QuietButton(stringResource(R.string.devices_also_sync, tv.name)) { store.sync.show(SyncSheet.ADD) } },
                    )
                }
            }
            add { place ->
                RowFrame(place, onClick = { store.sync.show(SyncSheet.ADD) }, modifier = Modifier.heightIn(min = 56.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                        Icon(Icons.Plus, null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
                        Text(stringResource(R.string.devices_add), style = MaterialTheme.typography.titleMedium)
                    }
                }
            }
        },
    )
}

@Composable
private fun DeviceRow(place: Place, name: String, line: String, well: Boolean, on: Boolean, onClick: () -> Unit, onSwitch: () -> Unit) {
    RowFrame(place, onClick = onClick, modifier = Modifier.heightIn(min = 72.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            Icon(deviceIcon(name), null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
            Column(Modifier.weight(1f)) {
                Text(name, style = MaterialTheme.typography.titleMedium)
                if (line.isNotEmpty()) {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        StateDot(if (well) MaterialTheme.colorScheme.secondary else MaterialTheme.colorScheme.error)
                        Text(line, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            }
            Box(Modifier.clickable(onClick = onSwitch)) { SettingSwitchMark(on) }
        }
    }
}

/** The cloud folder: its service and state, then Sync now and Sign out; or Add a cloud folder. */
@Composable
private fun CloudGroup(status: SyncStatus, settingUp: Boolean) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val colors = MaterialTheme.colorScheme
    val folder = status.folder
    val problem = status.problem
    val service = folder?.kind
    val signedOut = problem is SyncProblem.SignInNeeded
    val kind = kindWord()
    if (folder == null) {
        val working = settingUp
        SettingsGroup(
            title = stringResource(R.string.cloud_group),
            footer = stringResource(R.string.cloud_foot),
            rows = listOf { place ->
                if (working) {
                    CloudRow(place, stringResource(R.string.cloud_group), null) { SettingUpLine() }
                } else {
                    RowFrame(place, onClick = { store.sync.show(SyncSheet.PICKER) }, modifier = Modifier.heightIn(min = 56.dp)) {
                        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                            Icon(SyncIcons.Cloud, null, tint = colors.onSurfaceVariant)
                            Text(stringResource(R.string.cloud_add), style = MaterialTheme.typography.titleMedium)
                        }
                    }
                }
            },
        )
        return
    }
    val name = if (service == SyncFolderKind.DIRECTORY) stringResource(R.string.cloud_folder, kind) else cloudName(service ?: SyncFolderKind.DIRECTORY).ifEmpty { folder.label }
    val failed = status.error != null || problem != null
    SettingsGroup(
        title = stringResource(R.string.cloud_group),
        footer = stringResource(R.string.cloud_foot),
        rows = buildList<@Composable (Place) -> Unit> {
            add { place ->
                when {
                    settingUp -> CloudRow(place, name, null) { SettingUpLine() }
                    failed -> CloudRow(place, stringResource(R.string.cloud_err_title, name), null) {
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            Box(Modifier.padding(top = 7.dp)) { StateDot(colors.error) }
                            Text(problemLine(context, status, name, kind), style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant)
                        }
                    }
                    else -> CloudRow(place, name, null) {
                        val account = folder.account
                        val line = when {
                            status.syncing -> Strings.syncSyncing(context)
                            status.lastSynced != null -> lastSeenLine(context, status.lastSynced)
                            else -> null
                        }
                        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            StateDot(colors.secondary)
                            Text(listOfNotNull(line, account).joinToString(" · "), style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant) // i18n-skip
                        }
                    }
                }
            }
            if (signedOut) {
                add { place -> ActionRow(place, stringResource(R.string.cloud_sign_in_again)) { store.syncSignInAgain(folder.kind) } }
            } else if (problem is SyncProblem.Unreachable && status.error != null && failed && false) {
                add { place -> ActionRow(place, stringResource(R.string.cloud_choose_folder)) { store.sync.show(SyncSheet.PICKER) } }
            } else {
                add { place -> ActionRow(place, stringResource(R.string.cloud_sync_now), enabled = !status.syncing) { store.syncFolderNow() } }
                add { place -> ActionRow(place, stringResource(R.string.cloud_sign_out)) { if (service != null && service != SyncFolderKind.DIRECTORY) store.syncSignOut(service) } }
            }
        },
    )
}

/** The words a failed pass says, for the cloud row. */
private fun problemLine(context: Context, status: SyncStatus, name: String, kind: String): String = when (val p = status.problem) {
    is SyncProblem.SignInNeeded -> Strings.cloudErrSignedOut(context, name, kind)
    is SyncProblem.Unreachable -> Strings.cloudErrOffline(context, name)
    else -> status.error ?: Strings.cloudErrOffline(context, name)
}

@Composable
private fun CloudRow(place: Place, title: String, onClick: (() -> Unit)?, line: @Composable () -> Unit) {
    RowFrame(place, onClick = onClick, modifier = Modifier.heightIn(min = 72.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            Icon(SyncIcons.Cloud, null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
            Column(Modifier.weight(1f)) {
                Text(title, style = MaterialTheme.typography.titleMedium)
                line()
            }
        }
    }
}

/** "Setting up the folder", with the small ring: shown only once the first write has taken 600 ms. */
@Composable
private fun SettingUpLine() {
    val colors = MaterialTheme.colorScheme
    var shown by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) {
        kotlinx.coroutines.delay(600)
        shown = true
    }
    androidx.compose.animation.AnimatedVisibility(visible = shown, enter = androidx.compose.animation.fadeIn(app.kaeri.design.Motion.fadeIn())) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            androidx.compose.material3.CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp, color = colors.onSurfaceVariant)
            Text(stringResource(R.string.cloud_setting_up), style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant)
        }
    }
}

/** The recovery key row: "Made 2 Nov", opening the key again after the device's own lock. */
@Composable
private fun RecoveryKeyRow(status: SyncStatus) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val unlock = rememberDeviceUnlock { store.showRecoveryKey() }
    val made = status.recoveryKeyMade?.let { java.text.SimpleDateFormat(android.text.format.DateFormat.getBestDateTimePattern(java.util.Locale.getDefault(), "dMMM"), java.util.Locale.getDefault()).format(java.util.Date(it * 1000)) }
    SettingsGroup(
        rows = listOf { place ->
            LinkRow(place, stringResource(R.string.rkey_row), made?.let { Strings.rkeyMade(context, it) }, SyncIcons.Key) { unlock() }
        },
    )
}

/** What the sheet over the Devices page is, when one is open. */
@Composable
private fun DevicesSheets() {
    val store = LocalAppStore.current
    val sheet by store.sync.sheet.collectAsStateWithLifecycle()
    val key by store.sync.recoveryKey.collectAsStateWithLifecycle()
    val problem by store.sync.problem.collectAsStateWithLifecycle()
    val signIn by store.sync.signIn.collectAsStateWithLifecycle()
    LaunchedEffect(problem) {
        if (problem is SyncProblem.GroupExists) store.sync.starting?.let { store.sync.askKey(it) }
    }
    // Past the browser, every sheet closes at once: the cloud row carries the wait.
    LaunchedEffect(signIn?.step) {
        if (signIn?.step is app.kaeri.engine.model.SyncSignInStep.Finishing) store.sync.show(null)
    }
    // A set-up that finished closes the sheet it was begun in.
    LaunchedEffect(store.sync.status.collectAsStateWithLifecycle().value?.folder) {
        if (sheet == SyncSheet.PICKER || sheet == SyncSheet.WEBDAV || sheet == SyncSheet.NEXTCLOUD || sheet == SyncSheet.KEY) {
            if (store.sync.status.value?.folder != null) store.sync.show(null)
        }
    }
    when (sheet) {
        SyncSheet.ROAD -> RoadSheet()
        SyncSheet.PICKER -> CloudPickerSheet()
        SyncSheet.NEXTCLOUD -> NextcloudSheet()
        SyncSheet.WEBDAV -> WebDavSheet()
        SyncSheet.KEY -> EnterKeySheet()
        SyncSheet.ADD -> AddDeviceDialog()
        SyncSheet.CODE -> ShowCodeDialog()
        SyncSheet.NAME -> NameSheet()
        else -> {}
    }
    key?.let { RecoveryKeyDialog(it) }
    val copied by store.sync.copied.collectAsStateWithLifecycle()
    copied?.let { PairOkSheet(it) }
}

/** The Settings row's value: Sync off, Synced 2 min ago, or Couldn't sync. */
internal fun devicesSummary(context: Context, status: SyncStatus?): String = when {
    status == null || !status.enabled -> Strings.devicesRowOff(context)
    status.error != null || status.problem != null -> Strings.devicesRowError(context)
    else -> status.lastSynced?.let { Strings.devicesRowSynced(context, Wording.ago(context, (System.currentTimeMillis() / 1000 - it).coerceAtLeast(0))) } ?: Strings.devicesRowOff(context)
}

/** The Profiles row's value: Only you, or the names. */
internal fun profilesSummary(context: Context, profiles: List<app.kaeri.engine.model.ProfileView>): String =
    if (profiles.size <= 1) Strings.profilesOnlyYou(context) else profiles.joinToString(", ") { it.name } // i18n-skip
