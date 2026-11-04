// Sync on the TV: Settings, Devices and Profiles, the cloud folder to join, the code screens for a
// cloud sign-in, Dropbox through a paired phone, the TV's own pairing code, and Who's watching?

package app.kaeri.tv.sync

import android.content.Context
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import app.kaeri.design.Nagare
import app.kaeri.design.ObiSilk
import app.kaeri.engine.model.DeviceCodePrompt
import app.kaeri.engine.model.SyncCloudWay
import app.kaeri.engine.model.SyncCopy
import app.kaeri.engine.model.SyncDeviceView
import app.kaeri.engine.model.SyncFolderKind
import app.kaeri.engine.model.SyncProblem
import app.kaeri.engine.model.SyncSignInStep
import app.kaeri.engine.model.SyncStatus
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.store.AppStore
import app.kaeri.store.SyncSheet
import app.kaeri.store.Wording
import app.kaeri.store.cancelSyncSignIn
import app.kaeri.store.copySignIns
import app.kaeri.store.deleteProfile
import app.kaeri.store.leaveSync
import app.kaeri.store.loadSync
import app.kaeri.store.pairInvite
import app.kaeri.store.pairStop
import app.kaeri.store.removeSyncDevice
import app.kaeri.store.startSync
import app.kaeri.store.switchProfile
import app.kaeri.store.syncFolderNow
import app.kaeri.store.syncSignInAgain
import app.kaeri.store.syncSignOut
import app.kaeri.tv.remote.ConfirmDialog
import app.kaeri.tv.remote.QrCode
import app.kaeri.tv.signin.CodeExpiry
import app.kaeri.tv.signin.spaced
import app.kaeri.tv.ui.AmberButton
import app.kaeri.tv.ui.ButtonLabel
import app.kaeri.tv.ui.Canvas
import app.kaeri.tv.ui.FocusMemory
import app.kaeri.tv.ui.LocalAppStore
import app.kaeri.tv.ui.TonalButton
import app.kaeri.tv.ui.TvListRow
import app.kaeri.tv.ui.entryFocus
import app.kaeri.tv.ui.remembered
import app.kaeri.design.SyncIcons
import kotlinx.coroutines.delay

/** The name this TV gives itself: the system's own, or "TV". */
internal fun tvDeviceName(context: Context): String {
    val named = runCatching { android.provider.Settings.Global.getString(context.contentResolver, "device_name") }.getOrNull()
    return named?.takeIf { it.isNotBlank() } ?: Strings.deviceKindTv(context)
}

/** What a cloud service is called: the services keep their own names. */
internal fun cloudName(kind: SyncFolderKind): String = when (kind) {
    SyncFolderKind.GOOGLE_DRIVE -> "Google Drive" // i18n-skip
    SyncFolderKind.DROPBOX -> "Dropbox" // i18n-skip
    SyncFolderKind.ONE_DRIVE -> "OneDrive" // i18n-skip
    SyncFolderKind.NEXTCLOUD -> "Nextcloud" // i18n-skip
    SyncFolderKind.WEB_DAV -> "WebDAV" // i18n-skip
    SyncFolderKind.DIRECTORY -> "" // the caller says it in its own words
}

private fun deviceIcon(name: String) = run {
    val n = name.lowercase()
    when {
        "tv" in n -> SyncIcons.Tv
        "ipad" in n || "tablet" in n -> SyncIcons.Tablet
        "mac" in n || "laptop" in n || "notebook" in n -> SyncIcons.Laptop
        "pc" in n || "desktop" in n -> SyncIcons.Desktop
        else -> SyncIcons.Phone
    }
}

private fun agoLine(context: Context, lastSeen: Long?): String? {
    lastSeen ?: return null
    val seconds = (System.currentTimeMillis() / 1000 - lastSeen).coerceAtLeast(0)
    val days = seconds / 86_400
    return if (days > 30) Strings.devicesNotSeen(context, days.toInt()) else Strings.devicesRowSynced(context, Wording.ago(context, seconds))
}

/** A section's own heading, over its rows. */
@Composable
private fun GroupLabel(text: String) {
    Text(text, style = MaterialTheme.typography.titleMedium, color = ObiSilk.Haze, modifier = Modifier.padding(start = 16.dp, top = 24.dp, bottom = 8.dp).semantics { heading() })
}

@Composable
private fun Footnote(text: String) {
    Text(text, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 12.dp))
}

/** Settings, Devices: the devices in the group, the cloud folder, or the cloud folders to join. */
fun LazyListScope.devices(memory: FocusMemory, first: Modifier) {
    item(key = "devices", contentType = "list") {
        val store = LocalAppStore.current
        val context = LocalContext.current
        val status by store.sync.status.collectAsStateWithLifecycle()
        val sheet by store.sync.sheet.collectAsStateWithLifecycle()
        val settingUp by store.sync.settingUp.collectAsStateWithLifecycle()
        var turningOff by remember { mutableStateOf(false) }
        var signingOut by remember { mutableStateOf<SyncFolderKind?>(null) }
        var removing by remember { mutableStateOf<SyncDeviceView?>(null) }
        LaunchedEffect(Unit) { store.loadSync() }
        val synced = status
        val name = synced?.deviceName ?: tvDeviceName(context)
        if (sheet == SyncSheet.PICKER) {
            BackHandler { store.cancelSyncSignIn(); store.sync.show(null) }
            CloudPicker(store, memory, first, name)
        } else Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            if (synced?.enabled != true) {
                GroupLabel(stringResource(R.string.sync_group))
                TvListRow(
                    stringResource(R.string.sync_off_title),
                    onClick = {},
                    supporting = stringResource(R.string.sync_off_line),
                    supportingLines = 3,
                    height = 96.dp,
                    modifier = first.remembered(memory, "devices/off"),
                )
                TvListRow(stringResource(R.string.sync_turn_on), onClick = { store.sync.show(SyncSheet.ROAD) }, icon = SyncIcons.Sync, modifier = Modifier.remembered(memory, "devices/on"))
            } else {
                GroupLabel(stringResource(R.string.devices_yours))
                val others = synced.devices.filterNot { it.thisDevice }
                if (others.isEmpty()) {
                    TvListRow(stringResource(R.string.devices_only_this), onClick = {}, supporting = stringResource(R.string.devices_only_this_line), modifier = first.remembered(memory, "devices/only"))
                }
                others.forEachIndexed { index, device ->
                    val line = listOfNotNull(
                        agoLine(context, device.lastSeen),
                        stringResource(R.string.devices_sync_switch),
                        if ("tv" in device.name.lowercase()) stringResource(R.string.devices_remote_switch) else null,
                    ).joinToString(" · ") // i18n-skip
                    TvListRow(
                        device.name,
                        onClick = { removing = device },
                        supporting = line,
                        icon = deviceIcon(device.name),
                        dot = if (synced.error == null) ObiSilk.Water else ObiSilk.Vermilion,
                        modifier = (if (index == 0) first else Modifier).remembered(memory, "devices/${device.id}"),
                    )
                }
                TvListRow(
                    stringResource(R.string.devices_add),
                    onClick = { store.pairInvite(name); store.sync.show(SyncSheet.CODE) },
                    supporting = stringResource(R.string.tv_devices_add_line),
                    icon = SyncIcons.Devices,
                    modifier = Modifier.remembered(memory, "devices/add"),
                )
                GroupLabel(stringResource(R.string.cloud_group))
                val folder = synced.folder
                if (folder == null) {
                    TvListRow(
                        stringResource(R.string.cloud_add),
                        onClick = { store.sync.show(SyncSheet.PICKER) },
                        supporting = if (settingUp) stringResource(R.string.tv_cloud_finishing) else null,
                        icon = SyncIcons.Cloud,
                        modifier = Modifier.remembered(memory, "devices/cloud"),
                    )
                } else {
                    val service = cloudName(folder.kind).ifEmpty { stringResource(R.string.devices_this_device) }
                    val problem = synced.problem
                    val failed = problem != null || synced.error != null
                    val signedOut = problem is SyncProblem.SignInNeeded
                    TvListRow(
                        if (failed) Strings.cloudErrTitle(context, service) else service,
                        onClick = { if (signedOut) store.syncSignInAgain(folder.kind) else if (failed) store.syncFolderNow() else {} },
                        supporting = when {
                            settingUp -> stringResource(R.string.tv_cloud_finishing)
                            signedOut -> Strings.tvCloudRevokedLine(context, service)
                            failed -> Strings.cloudErrOffline(context, service)
                            synced.syncing -> stringResource(R.string.sync_syncing)
                            else -> listOfNotNull(agoLine(context, synced.lastSynced), folder.account).joinToString(" · ") // i18n-skip
                        },
                        icon = SyncIcons.Cloud,
                        dot = if (failed) ObiSilk.Vermilion else ObiSilk.Water,
                        modifier = Modifier.remembered(memory, "devices/folder"),
                    )
                    if (!signedOut) {
                        TvListRow(stringResource(R.string.cloud_sync_now), onClick = { if (!synced.syncing) store.syncFolderNow() }, modifier = Modifier.remembered(memory, "devices/now"))
                        TvListRow(stringResource(R.string.cloud_sign_out), onClick = { signingOut = folder.kind }, modifier = Modifier.remembered(memory, "devices/out"))
                    }
                }
                Footnote(stringResource(R.string.cloud_foot))
                GroupLabel("")
                TvListRow(stringResource(R.string.sync_turn_off, Strings.deviceKindTv(context)), onClick = { turningOff = true }, icon = SyncIcons.Power, modifier = Modifier.remembered(memory, "devices/off-now"))
            }
        }
        signingOut?.let { kind ->
            val service = cloudName(kind)
            ConfirmDialog(
                title = Strings.tvCloudSignOutTitle(context, service),
                body = Strings.tvCloudSignOutLine(context, service),
                action = stringResource(R.string.cloud_sign_out),
                onConfirm = { store.syncSignOut(kind); signingOut = null },
                onCancel = { signingOut = null },
            )
        }
        removing?.let { device ->
            ConfirmDialog(
                title = stringResource(R.string.devices_remove_alert, device.name),
                body = stringResource(R.string.devices_remove_alert_line, device.name),
                action = stringResource(R.string.devices_remove),
                onConfirm = { store.removeSyncDevice(device.id); removing = null },
                onCancel = { removing = null },
            )
        }
        if (turningOff) {
            ConfirmDialog(
                title = stringResource(R.string.sync_turn_off_alert, name),
                body = stringResource(R.string.sync_turn_off_alert_line, Strings.deviceKindTv(context)),
                action = stringResource(R.string.sync_turn_off, Strings.deviceKindTv(context)),
                onConfirm = { store.leaveSync(); turningOff = false },
                onCancel = { turningOff = false },
            )
        }
    }
}

/** Choose a cloud folder, on a TV: the cloud folders to join, and a line that a new one is added on a phone. */
@Composable
private fun CloudPicker(store: AppStore, memory: FocusMemory, first: Modifier, name: String) {
    val context = LocalContext.current
    val status by store.sync.status.collectAsStateWithLifecycle()
    val signIn by store.sync.signIn.collectAsStateWithLifecycle()
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        GroupLabel(stringResource(R.string.cloud_pick_title))
        val rows = buildList {
            store.sync.cloud(SyncFolderKind.DROPBOX)?.let { add(SyncFolderKind.DROPBOX to it.way) }
            store.sync.cloud(SyncFolderKind.GOOGLE_DRIVE)?.let { add(SyncFolderKind.GOOGLE_DRIVE to it.way) }
            add(SyncFolderKind.NEXTCLOUD to SyncCloudWay.Browser)
            store.sync.cloud(SyncFolderKind.ONE_DRIVE)?.let { add(SyncFolderKind.ONE_DRIVE to it.way) }
        }
        rows.forEachIndexed { index, (kind, way) ->
            val needsPhone = way is SyncCloudWay.Unavailable
            TvListRow(
                cloudName(kind),
                onClick = {
                    when {
                        needsPhone -> store.sync.show(SyncSheet.KEY)
                        else -> store.startSync(name, app.kaeri.engine.model.SyncFolderSpec(kind, null, null, null))
                    }
                },
                supporting = when {
                    signIn?.service == kind -> Strings.cloudWaiting(context, cloudName(kind))
                    needsPhone -> stringResource(R.string.tv_cloud_dropbox_line)
                    kind == SyncFolderKind.NEXTCLOUD -> stringResource(R.string.cloud_nextcloud_line)
                    kind == SyncFolderKind.GOOGLE_DRIVE -> stringResource(R.string.cloud_google_line)
                    kind == SyncFolderKind.ONE_DRIVE -> stringResource(R.string.cloud_onedrive_line)
                    else -> stringResource(R.string.cloud_dropbox_line)
                },
                icon = if (kind == SyncFolderKind.NEXTCLOUD) SyncIcons.Server else SyncIcons.Cloud,
                modifier = (if (index == 0) first else Modifier).remembered(memory, "picker/${kind.name}"),
            )
        }
        TvListRow(
            stringResource(R.string.cloud_webdav),
            onClick = {},
            supporting = stringResource(R.string.cloud_webdav_line),
            icon = SyncIcons.Server,
            modifier = Modifier.remembered(memory, "picker/webdav"),
        )
        Footnote(stringResource(R.string.tv_cloud_pick_foot))
        if (status == null) Box(Modifier.size(1.dp))
    }
}

/** The shared frame of the full-screen sync pages: the character beside a column, Back closing it. */
@Composable
private fun SyncPage(onBack: () -> Unit, focus: FocusRequester, content: @Composable () -> Unit) {
    BackHandler(onBack = onBack)
    Row(
        Modifier
            .entryFocus(focus)
            .fillMaxSize()
            .background(ObiSilk.Ground)
            .padding(start = Canvas.Keyline, end = Canvas.SafeX, top = 96.dp),
        horizontalArrangement = Arrangement.spacedBy(64.dp),
    ) {
        Box(Modifier.width(160.dp)) { Nagare(cell = 5) }
        Column(verticalArrangement = Arrangement.spacedBy(20.dp)) { content() }
    }
}

/** The full-screen sync pages, over the whole TV while one is under way. */
@Composable
fun TvSyncHost() {
    val store = LocalAppStore.current
    val sheet by store.sync.sheet.collectAsStateWithLifecycle()
    val signIn by store.sync.signIn.collectAsStateWithLifecycle()
    val status by store.sync.status.collectAsStateWithLifecycle()
    val shown = signIn
    val step = shown?.step
    when {
        shown != null && step is SyncSignInStep.Code -> CloudCodePage(shown.service, step)
        shown != null && step is SyncSignInStep.Page -> NextcloudQrPage(step)
        sheet == SyncSheet.KEY -> DropboxPhonePage(status)
        sheet == SyncSheet.ROAD -> RoadPage()
        sheet == SyncSheet.CODE || sheet == SyncSheet.ADD -> PairCodePage()
        sheet == SyncSheet.SWITCH -> TvWhosWatching(onChosen = { store.sync.show(null) })
    }
}

/** How should your devices share? On a TV: with a device nearby, or a cloud folder to join. */
@Composable
private fun RoadPage() {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val focus = remember { FocusRequester() }
    SyncPage(onBack = { store.sync.show(null) }, focus) {
        Text(stringResource(R.string.sync_road_title), style = MaterialTheme.typography.titleLarge, color = ObiSilk.Silk, modifier = Modifier.semantics { heading() })
        Column(Modifier.widthIn(max = 720.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            TvListRow(
                stringResource(R.string.sync_road_nearby),
                onClick = { store.pairInvite(tvDeviceName(context)); store.sync.show(SyncSheet.CODE) },
                supporting = stringResource(R.string.sync_road_nearby_line),
                supportingLines = 2,
                icon = SyncIcons.Devices,
                height = 96.dp,
                modifier = Modifier.focusRequester(focus),
            )
            TvListRow(
                stringResource(R.string.sync_road_cloud),
                onClick = { store.sync.show(SyncSheet.PICKER) },
                supporting = stringResource(R.string.sync_road_cloud_line),
                supportingLines = 3,
                icon = SyncIcons.Cloud,
                height = 120.dp,
            )
        }
        Text(stringResource(R.string.sync_road_later), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
        TonalButton(onClick = { store.sync.show(null) }) { ButtonLabel(stringResource(R.string.action_cancel)) }
    }
}

/** A cloud service's device code: scan or type, with the time left, then a fresh code once it runs out. */
@Composable
private fun CloudCodePage(service: SyncFolderKind, step: SyncSignInStep.Code) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val name = cloudName(service)
    val prompt = remember(step) { DeviceCodePrompt(name, step.userCode, step.verificationUrl, step.expiresAt.toLong()) }
    var expired by remember(step.userCode) { mutableStateOf(false) }
    val again = remember { FocusRequester() }
    LaunchedEffect(expired) { if (expired) runCatching { again.requestFocus() } }
    SyncPage(onBack = { store.cancelSyncSignIn() }, again) {
        Text(stringResource(R.string.accounts_sign_in_to, name), style = MaterialTheme.typography.titleLarge, color = ObiSilk.Silk, modifier = Modifier.semantics { heading() })
        Text(stringResource(R.string.tv_signin_device_steps), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk, modifier = Modifier.widthIn(max = 720.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(40.dp), verticalAlignment = Alignment.CenterVertically) {
            QrCode(step.verificationUrl, label = stringResource(R.string.tv_signin_qr_activation))
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    spaced(step.userCode).replace(" ", "-"),
                    style = MaterialTheme.typography.displaySmall.copy(fontFamily = FontFamily.Monospace, letterSpacing = 4.sp),
                    color = if (expired) ObiSilk.Haze else ObiSilk.Silk,
                )
                Text(step.verificationUrl, style = MaterialTheme.typography.labelLarge, color = ObiSilk.Haze)
            }
        }
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            if (!expired) Text(Strings.cloudWaiting(context, name), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
            CodeExpiry(prompt, onExpired = { expired = true })
        }
        Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            if (expired) {
                AmberButton(onClick = { store.syncSignInAgain(service) }, icon = null, modifier = Modifier.focusRequester(again)) { ButtonLabel(stringResource(R.string.action_new_code)) }
            }
            TonalButton(onClick = { store.cancelSyncSignIn() }, modifier = if (expired) Modifier else Modifier.focusRequester(again)) { ButtonLabel(stringResource(R.string.action_cancel)) }
        }
    }
}

/** Nextcloud on a TV: a QR code of its login page, for the phone to open and allow access on. */
@Composable
private fun NextcloudQrPage(step: SyncSignInStep.Page) {
    val store = LocalAppStore.current
    val focus = remember { FocusRequester() }
    SyncPage(onBack = { store.cancelSyncSignIn() }, focus) {
        Text(stringResource(R.string.accounts_sign_in_to, "Nextcloud"), style = MaterialTheme.typography.titleLarge, color = ObiSilk.Silk, modifier = Modifier.semantics { heading() }) // i18n-skip
        Text(stringResource(R.string.tv_nextcloud_line), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk, modifier = Modifier.widthIn(max = 720.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(40.dp), verticalAlignment = Alignment.CenterVertically) {
            QrCode(step.url, label = stringResource(R.string.tv_qr_open_page))
            Column(verticalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.widthIn(max = 560.dp)) {
                Text(stringResource(R.string.tv_qr_open_page), style = MaterialTheme.typography.labelLarge, color = ObiSilk.Haze)
                Text(step.url, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Silk)
            }
        }
        TonalButton(onClick = { store.cancelSyncSignIn() }, modifier = Modifier.focusRequester(focus)) { ButtonLabel(stringResource(R.string.action_cancel)) }
    }
}

/** Dropbox signs in on a phone: use the paired phone's sign-in, or say where to sign in. */
@Composable
private fun DropboxPhonePage(status: SyncStatus?) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val focus = remember { FocusRequester() }
    val phone = status?.devices?.firstOrNull { !it.thisDevice && SyncFolderKind.DROPBOX in it.signIns }
    val close = { store.sync.show(SyncSheet.PICKER) }
    SyncPage(onBack = close, focus) {
        Text(stringResource(R.string.tv_cloud_needs_phone_title), style = MaterialTheme.typography.titleLarge, color = ObiSilk.Silk, modifier = Modifier.semantics { heading() })
        Text(
            if (phone != null) Strings.tvCloudNeedsPhoneHas(context, phone.name) else stringResource(R.string.tv_cloud_needs_phone_none),
            style = MaterialTheme.typography.bodyLarge,
            color = ObiSilk.Silk,
            modifier = Modifier.widthIn(max = 1000.dp),
        )
        Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            if (phone != null) {
                AmberButton(
                    onClick = { store.copySignIns(phone.name, listOf(SyncCopy.Cloud(SyncFolderKind.DROPBOX))); store.sync.show(SyncSheet.PICKER) },
                    icon = null,
                    modifier = Modifier.focusRequester(focus),
                ) { ButtonLabel(Strings.tvUsePhoneSignin(context, phone.name)) }
                TonalButton(onClick = close) { ButtonLabel(stringResource(R.string.action_cancel)) }
            } else {
                TonalButton(onClick = close, modifier = Modifier.focusRequester(focus)) { ButtonLabel(stringResource(R.string.action_cancel)) }
            }
        }
    }
}

/** The TV's own pairing code, for a phone to scan or type, good for 2 minutes. */
@Composable
private fun PairCodePage() {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val invite by store.sync.invite.collectAsStateWithLifecycle()
    val focus = remember { FocusRequester() }
    var left by remember { mutableStateOf("") }
    LaunchedEffect(invite) {
        val until = invite?.until ?: return@LaunchedEffect
        while (true) {
            val seconds = ((until - System.currentTimeMillis()) / 1000).coerceAtLeast(0)
            left = "%d:%02d".format(seconds / 60, seconds % 60)
            if (seconds == 0L) {
                store.sync.inviteEnded()
                break
            }
            delay(1000)
        }
    }
    val close = { store.pairStop(); store.sync.show(null) }
    SyncPage(onBack = close, focus) {
        Text(stringResource(R.string.pair_show_title), style = MaterialTheme.typography.titleLarge, color = ObiSilk.Silk, modifier = Modifier.semantics { heading() })
        Text(stringResource(R.string.pair_tv_line), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk, modifier = Modifier.widthIn(max = 880.dp))
        val shown = invite
        if (shown != null) {
            Row(horizontalArrangement = Arrangement.spacedBy(40.dp), verticalAlignment = Alignment.CenterVertically) {
                QrCode("kaeri://pair?port=${shown.port}&code=${shown.code}", label = stringResource(R.string.pair_show_scan))
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(
                        shown.code.chunked(3).joinToString(" "), // i18n-skip
                        style = MaterialTheme.typography.displaySmall.copy(fontFamily = FontFamily.Monospace, letterSpacing = 4.sp),
                        color = ObiSilk.Silk,
                    )
                    Text(Strings.pairGoodFor(context, left), style = MaterialTheme.typography.labelLarge, color = ObiSilk.Haze)
                }
            }
        } else {
            Text(stringResource(R.string.pair_err_expired), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze)
        }
        Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            if (shown == null) AmberButton(onClick = { store.pairInvite(tvDeviceName(context)) }, icon = null) { ButtonLabel(stringResource(R.string.pair_show_code)) }
            TonalButton(onClick = close, modifier = Modifier.focusRequester(focus)) { ButtonLabel(stringResource(R.string.action_cancel)) }
        }
    }
}

/** Settings, Profiles: the profiles on this TV, Add on a phone, and Ask at launch. */
fun LazyListScope.profiles(memory: FocusMemory, first: Modifier) {
    item(key = "profiles", contentType = "list") {
        val store = LocalAppStore.current
        val profiles by store.sync.profiles.collectAsStateWithLifecycle()
        var deleting by remember { mutableStateOf<app.kaeri.engine.model.ProfileView?>(null) }
        LaunchedEffect(Unit) { store.loadSync() }
        val list = profiles.profiles
        val context = LocalContext.current
        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            GroupLabel(stringResource(R.string.tv_profiles_on_this_tv))
            if (list.size <= 1) {
                TvListRow(
                    list.firstOrNull()?.name ?: stringResource(R.string.profiles_only_you),
                    onClick = {},
                    supporting = stringResource(R.string.tv_profiles_only_you),
                    supportingLines = 2,
                    icon = SyncIcons.People,
                    height = 96.dp,
                    modifier = first.remembered(memory, "profiles/only"),
                )
            } else {
                list.forEachIndexed { index, profile ->
                    TvListRow(
                        profile.name,
                        onClick = { if (!profile.active) store.switchProfile(profile.id) else {} },
                        supporting = listOfNotNull(
                            if (profile.kids) stringResource(R.string.profiles_kids_tag) else null,
                            if (profile.active) stringResource(R.string.profiles_yours) else null,
                        ).joinToString(" · ").ifEmpty { null }, // i18n-skip
                        icon = SyncIcons.People,
                        modifier = (if (index == 0) first else Modifier).remembered(memory, "profiles/${profile.id}"),
                    )
                }
                Footnote(stringResource(R.string.profiles_shared_foot))
                TvListRow(
                    stringResource(R.string.profiles_switch),
                    onClick = { store.sync.show(SyncSheet.SWITCH) },
                    supporting = stringResource(R.string.tv_profiles_opens_picker),
                    modifier = Modifier.remembered(memory, "profiles/switch"),
                )
            }
        }
        deleting?.let { profile ->
            ConfirmDialog(
                title = Strings.profilesDeleteAlert(context, profile.name),
                body = Strings.profilesDeleteAlertLine(context, profile.name),
                action = stringResource(R.string.profiles_delete),
                onConfirm = { store.deleteProfile(profile.id); deleting = null },
                onCancel = { deleting = null },
            )
        }
    }
}

/** Who's watching?: the profiles as large tiles, over the whole screen, Add a profile never among them on a TV. */
@Composable
fun TvWhosWatching(onChosen: () -> Unit) {
    val store = LocalAppStore.current
    val profiles by store.sync.profiles.collectAsStateWithLifecycle()
    val focus = remember { FocusRequester() }
    BackHandler {}
    Column(
        Modifier.entryFocus(focus).fillMaxSize().background(ObiSilk.Ground).padding(top = 120.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(48.dp),
    ) {
        Text(stringResource(R.string.profiles_picker), style = MaterialTheme.typography.headlineMedium, color = ObiSilk.Silk, modifier = Modifier.semantics { heading() })
        Row(horizontalArrangement = Arrangement.spacedBy(40.dp)) {
            profiles.profiles.forEachIndexed { index, profile ->
                ProfileTile(profile, if (index == 0) Modifier.focusRequester(focus) else Modifier) {
                    if (!profile.active) store.switchProfile(profile.id)
                    onChosen()
                }
            }
        }
    }
}

@Composable
private fun ProfileTile(profile: app.kaeri.engine.model.ProfileView, modifier: Modifier, onClick: () -> Unit) {
    val context = LocalContext.current
    androidx.tv.material3.Surface(
        onClick = onClick,
        modifier = modifier.width(200.dp),
        shape = androidx.tv.material3.ClickableSurfaceDefaults.shape(androidx.compose.foundation.shape.RoundedCornerShape(24.dp)),
        colors = androidx.tv.material3.ClickableSurfaceDefaults.colors(
            containerColor = ObiSilk.Raised,
            contentColor = ObiSilk.Silk,
            focusedContainerColor = ObiSilk.Silk,
            focusedContentColor = ObiSilk.Ground,
        ),
        scale = androidx.tv.material3.ClickableSurfaceDefaults.scale(focusedScale = 1.06f),
    ) {
        Column(
            Modifier.padding(24.dp).semantics { contentDescription = Strings.profilesAvatarDesc(context, profile.name) },
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Box(Modifier.size(112.dp).background(ObiSilk.RaisedSelected, CircleShape), contentAlignment = Alignment.Center) {
                Text(profile.initials.ifEmpty { profile.name.take(1).uppercase() }, style = MaterialTheme.typography.displaySmall)
            }
            Text(profile.name, style = MaterialTheme.typography.titleLarge)
            if (profile.kids) Text(stringResource(R.string.profiles_kids_tag), style = MaterialTheme.typography.labelLarge)
        }
    }
}
