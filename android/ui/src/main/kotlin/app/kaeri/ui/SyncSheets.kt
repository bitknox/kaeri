// The sheets over Settings, Devices: how the devices share, the cloud folder picker and its
// sign-ins, the recovery key, pairing and the device's name, as the sync board draws them.

package app.kaeri.ui

import app.kaeri.design.SyncIcons
import android.app.Activity
import android.app.KeyguardManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.engine.ffi.qrMatrix
import app.kaeri.engine.model.SyncCloudWay
import app.kaeri.engine.model.SyncFolderKind
import app.kaeri.engine.model.SyncFolderSpec
import app.kaeri.engine.model.SyncProblem
import app.kaeri.engine.model.SyncSignInStep
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.store.AppStore
import app.kaeri.store.SyncSheet
import app.kaeri.store.cancelSyncSignIn
import app.kaeri.store.joinSync
import app.kaeri.store.newRecoveryKey
import app.kaeri.store.pairInvite
import app.kaeri.store.pairJoin
import app.kaeri.store.pairStop
import app.kaeri.store.startSync
import kotlinx.coroutines.delay

/** A confirmation as a dialog: a question, what follows, and the one action named for what it does. */
@Composable
internal fun ConfirmAlert(title: String, text: String, confirm: String, onDismiss: () -> Unit, onConfirm: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        properties = AskingDialog,
        title = { Text(title) },
        text = { Text(text) },
        confirmButton = { TextButton(onClick = onConfirm) { Text(confirm, color = MaterialTheme.colorScheme.error) } },
        dismissButton = { QuietButton(stringResource(R.string.action_cancel), onClick = onDismiss) },
        containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
    )
}

/**
 * The device's own lock before the recovery key shows again: the system's screen lock where the
 * device has one, and no question where it has none.
 */
@Composable
internal fun rememberDeviceUnlock(onUnlocked: () -> Unit): () -> Unit {
    val context = LocalContext.current
    val unlocked = rememberUpdatedState(onUnlocked)
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.StartActivityForResult()) {
        if (it.resultCode == Activity.RESULT_OK) unlocked.value()
    }
    return {
        val keys = context.getSystemService(Context.KEYGUARD_SERVICE) as? KeyguardManager
        val intent = if (keys?.isDeviceSecure == true) keys.createConfirmDeviceCredentialIntent(null, null) else null
        if (intent != null) launcher.launch(intent) else unlocked.value()
    }
}

/** The name this device shows: the one just given, the engine's, or the system's. */
@Composable
internal fun deviceName(): String {
    val store = LocalAppStore.current
    val given by store.sync.name.collectAsStateWithLifecycle()
    val status by store.sync.status.collectAsStateWithLifecycle()
    return given ?: status?.deviceName ?: systemDeviceName(LocalContext.current)
}

/** A choice in a sheet: its glyph, its name, and a line under it. */
@Composable
internal fun SheetChoice(icon: ImageVector, title: String, line: String, busy: Boolean = false, enabled: Boolean = true, onClick: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    Row(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .clickable(enabled = enabled && !busy, onClick = onClick)
            .heightIn(min = 56.dp)
            .padding(vertical = 12.dp),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalAlignment = Alignment.Top,
    ) {
        Icon(icon, null, tint = colors.onSurfaceVariant, modifier = Modifier.padding(top = 2.dp))
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.titleMedium, color = if (enabled) colors.onSurface else colors.onSurfaceVariant)
            Text(line, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant)
        }
        if (busy) CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp, color = colors.onSurfaceVariant)
    }
}

/** How should your devices share? Nearby, through a cloud folder, or later. */
@Composable
internal fun RoadSheet() {
    val store = LocalAppStore.current
    SettingsSheet(onDismiss = { store.sync.show(null) }) {
        SheetTitle(stringResource(R.string.sync_road_title))
        SheetChoice(SyncIcons.Devices, stringResource(R.string.sync_road_nearby), stringResource(R.string.sync_road_nearby_line)) { store.sync.show(SyncSheet.ADD) }
        SheetChoice(SyncIcons.Cloud, stringResource(R.string.sync_road_cloud), stringResource(R.string.sync_road_cloud_line)) { store.sync.show(SyncSheet.PICKER) }
        SheetText(stringResource(R.string.sync_road_later), Modifier.padding(top = 8.dp))
        SheetButtons { QuietButton(stringResource(R.string.action_cancel)) { store.sync.show(null) } }
    }
}

/** The folder on this phone sync keeps its sealed copy in, where the app may write without a grant. */
private fun localSyncFolder(context: Context): String =
    (context.getExternalFilesDir(null) ?: context.filesDir).resolve("Kaeri sync").absolutePath

/** Choose a cloud folder: the services this build can sign in to, then Other. */
@Composable
internal fun CloudPickerSheet() {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val status by store.sync.status.collectAsStateWithLifecycle()
    val signIn by store.sync.signIn.collectAsStateWithLifecycle()
    val problem by store.sync.problem.collectAsStateWithLifecycle()
    val name = deviceName()
    val kind = kindWord()
    val signing = signIn
    val cloud = { service: SyncFolderKind -> status?.let { store.sync.cloud(service) } }
    fun begin(spec: SyncFolderSpec) = store.startSync(name, spec)
    SettingsSheet(onDismiss = { store.cancelSyncSignIn(); store.sync.show(null) }, tall = true) {
        SheetTitle(stringResource(R.string.cloud_pick_title))
        val rows = buildList<Triple<SyncFolderKind, ImageVector, String>> {
            if (cloud(SyncFolderKind.DROPBOX) != null) add(Triple(SyncFolderKind.DROPBOX, SyncIcons.Cloud, stringResource(R.string.cloud_dropbox_line)))
            if (cloud(SyncFolderKind.GOOGLE_DRIVE) != null) add(Triple(SyncFolderKind.GOOGLE_DRIVE, SyncIcons.Cloud, stringResource(R.string.cloud_google_line)))
            add(Triple(SyncFolderKind.NEXTCLOUD, SyncIcons.Cloud, stringResource(R.string.cloud_nextcloud_line)))
            if (cloud(SyncFolderKind.ONE_DRIVE) != null) add(Triple(SyncFolderKind.ONE_DRIVE, SyncIcons.Cloud, stringResource(R.string.cloud_onedrive_line)))
        }
        rows.forEach { (service, icon, line) ->
            val waiting = signing?.service == service
            SheetChoice(
                icon,
                cloudName(service),
                if (waiting) Strings.cloudWaiting(context, cloudName(service)) else line,
                busy = waiting,
                enabled = signing == null,
            ) {
                if (service == SyncFolderKind.NEXTCLOUD) store.sync.show(SyncSheet.NEXTCLOUD) else begin(SyncFolderSpec(service, null, null, null))
            }
        }
        SettingsHeader(stringResource(R.string.cloud_other), Modifier.padding(start = 0.dp, top = 8.dp))
        SheetChoice(SyncIcons.Server, stringResource(R.string.cloud_webdav), stringResource(R.string.cloud_webdav_line), enabled = signing == null) { store.sync.show(SyncSheet.WEBDAV) }
        SheetChoice(SyncIcons.Folder, Strings.cloudFolder(context, kind), stringResource(R.string.cloud_folder_line), enabled = signing == null) {
            begin(SyncFolderSpec(SyncFolderKind.DIRECTORY, localSyncFolder(context), null, null))
        }
        problemWords(context, problem)?.let { SheetProblem(it) }
        SheetText(stringResource(R.string.cloud_pick_foot), Modifier.padding(top = 16.dp))
        SheetButtons { QuietButton(stringResource(R.string.action_cancel)) { store.cancelSyncSignIn(); store.sync.show(null) } }
    }
}

/** What a set-up or join problem says, for the sheet it came from. Null where the problem has its own place. */
internal fun problemWords(context: Context, problem: SyncProblem?, host: String? = null): String? = when (problem) {
    null, SyncProblem.WrongRecoveryKey, SyncProblem.GroupExists -> null
    SyncProblem.Unreachable -> if (host != null) Strings.cloudErrNoServer(context, host) else Strings.cloudErrOffline(context, "")
    is SyncProblem.SignInDenied -> Strings.cloudErrDenied(context, cloudName(problem.service))
    is SyncProblem.SignInExpired -> Strings.cloudErrDenied(context, cloudName(problem.service))
    is SyncProblem.SignInNeeded -> Strings.cloudErrSignedOut(context, cloudName(problem.service), "")
    is SyncProblem.Other -> problem.v1
    else -> Strings.cloudErrOffline(context, "")
}

/** Nextcloud: its address, then the wait for its own login page, with an app password as the way round. */
@Composable
internal fun NextcloudSheet() {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val signIn by store.sync.signIn.collectAsStateWithLifecycle()
    val problem by store.sync.problem.collectAsStateWithLifecycle()
    val uri = LocalUriHandler.current
    val name = deviceName()
    var address by remember { mutableStateOf("") }
    var appPassword by remember { mutableStateOf(false) }
    var user by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }
    val step = signIn?.takeIf { it.service == SyncFolderKind.NEXTCLOUD }?.step
    LaunchedEffect(step) { (step as? SyncSignInStep.Page)?.let { runCatching { uri.openUri(it.url) } } }
    val host = address.removePrefix("https://").removePrefix("http://").trimEnd('/')
    val back = { store.cancelSyncSignIn(); store.sync.show(SyncSheet.PICKER) }
    SettingsSheet(onDismiss = { store.cancelSyncSignIn(); store.sync.show(null) }, holdsInput = address.isNotEmpty()) {
        if (step != null && !appPassword) {
            SheetTitle(Strings.cloudNextcloudWaitTitle(context, host))
            SheetText(stringResource(R.string.cloud_nextcloud_wait_line))
            Row(Modifier.padding(top = 16.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Text(Strings.cloudWaiting(context, "Nextcloud"), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant) // i18n-skip
            }
            SheetButtons {
                QuietButton(stringResource(R.string.cloud_app_password)) { appPassword = true }
                QuietButton(stringResource(R.string.cloud_open_again)) { (step as? SyncSignInStep.Page)?.let { runCatching { uri.openUri(it.url) } } }
                QuietButton(stringResource(R.string.action_cancel), onClick = back)
            }
        } else if (appPassword) {
            SheetTitle("Nextcloud") // i18n-skip
            SheetField(address, { address = it }, stringResource(R.string.cloud_nextcloud_address), keyboard = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Next))
            SheetField(user, { user = it }, stringResource(R.string.accounts_user_or_email), modifier = Modifier.padding(top = 8.dp), keyboard = KeyboardOptions(imeAction = ImeAction.Next))
            SheetField(password, { password = it }, stringResource(R.string.accounts_password), modifier = Modifier.padding(top = 8.dp), password = true, supporting = Strings.cloudAppPasswordNote(context, kindWord()))
            problemWords(context, problem, host)?.let { SheetProblem(it) }
            SheetButtons {
                QuietButton(stringResource(R.string.action_cancel), onClick = back)
                Button(enabled = address.isNotBlank() && user.isNotBlank() && password.isNotBlank(), onClick = {
                    store.startSync(name, SyncFolderSpec(SyncFolderKind.NEXTCLOUD, address.trim(), user.trim(), password))
                }) { Text(stringResource(R.string.cloud_connect)) }
            }
        } else {
            SheetTitle("Nextcloud") // i18n-skip
            SheetField(
                address, { address = it }, stringResource(R.string.cloud_nextcloud_address),
                keyboard = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Go),
                actions = KeyboardActions(onGo = { if (address.isNotBlank()) store.startSync(name, SyncFolderSpec(SyncFolderKind.NEXTCLOUD, address.trim(), null, null)) }),
                supporting = stringResource(R.string.cloud_nextcloud_address_note),
                error = problem is SyncProblem.Unreachable,
            )
            problemWords(context, problem, host)?.let { SheetProblem(it) }
            SheetButtons {
                QuietButton(stringResource(R.string.action_cancel), onClick = back)
                Button(enabled = address.isNotBlank(), onClick = {
                    store.startSync(name, SyncFolderSpec(SyncFolderKind.NEXTCLOUD, address.trim(), null, null))
                }) { Text(stringResource(R.string.cloud_connect)) }
            }
        }
    }
}

/** A WebDAV server: its address, a user name and a password. */
@Composable
internal fun WebDavSheet() {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val problem by store.sync.problem.collectAsStateWithLifecycle()
    val name = deviceName()
    var address by remember { mutableStateOf("") }
    var user by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }
    val refused = problem is SyncProblem.SignInDenied || problem is SyncProblem.Other
    SettingsSheet(onDismiss = { store.sync.show(null) }, holdsInput = address.isNotEmpty()) {
        SheetTitle(stringResource(R.string.cloud_webdav))
        SheetField(address, { address = it }, stringResource(R.string.cloud_nextcloud_address), keyboard = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Next))
        SheetField(user, { user = it }, stringResource(R.string.accounts_user_or_email), modifier = Modifier.padding(top = 8.dp), keyboard = KeyboardOptions(imeAction = ImeAction.Next), error = refused)
        SheetField(password, { password = it }, stringResource(R.string.accounts_password), modifier = Modifier.padding(top = 8.dp), password = true, error = refused)
        if (refused) SheetProblem(stringResource(R.string.cloud_err_webdav))
        else problemWords(context, problem, address.removePrefix("https://").trimEnd('/'))?.let { SheetProblem(it) }
        SheetButtons {
            QuietButton(stringResource(R.string.action_cancel)) { store.sync.show(SyncSheet.PICKER) }
            Button(enabled = address.isNotBlank() && user.isNotBlank() && password.isNotBlank(), onClick = {
                store.startSync(name, SyncFolderSpec(SyncFolderKind.WEB_DAV, address.trim(), user.trim(), password))
            }) { Text(stringResource(R.string.cloud_connect)) }
        }
    }
}

/** Enter your recovery key: the folder holds a group already, and its key opens it. */
@Composable
internal fun EnterKeySheet() {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val problem by store.sync.problem.collectAsStateWithLifecycle()
    val name = deviceName()
    val folder = store.sync.joining
    var key by remember { mutableStateOf("") }
    val service = folder?.let { cloudName(it.kind).ifEmpty { Strings.cloudFolder(context, kindWord()) } } ?: ""
    SettingsSheet(onDismiss = { store.sync.show(null) }, holdsInput = key.isNotEmpty()) {
        SheetTitle(stringResource(R.string.rkey_enter_title))
        SheetText(Strings.rkeyEnterLine(context, service, 1))
        SheetField(
            key, { key = it.uppercase() }, stringResource(R.string.rkey_row),
            modifier = Modifier.padding(top = 16.dp),
            keyboard = KeyboardOptions(imeAction = ImeAction.Done),
            supporting = stringResource(R.string.rkey_enter_note),
            error = problem is SyncProblem.WrongRecoveryKey,
        )
        if (problem is SyncProblem.WrongRecoveryKey) SheetProblem(stringResource(R.string.rkey_wrong))
        SheetButtons {
            QuietButton(stringResource(R.string.action_cancel)) { store.sync.show(SyncSheet.PICKER) }
            Button(enabled = key.isNotBlank() && folder != null, onClick = { folder?.let { store.joinSync(name, it, key) } }) {
                Text(stringResource(R.string.rkey_restore))
            }
        }
    }
}

/** Name: this device's name, kept for the next time it pairs. */
@Composable
internal fun NameSheet() {
    val store = LocalAppStore.current
    var name by remember { mutableStateOf(store.sync.name.value ?: store.sync.status.value?.deviceName ?: "") }
    SettingsSheet(onDismiss = { store.sync.show(null) }, holdsInput = false) {
        SheetTitle(stringResource(R.string.devices_name))
        SheetField(name, { name = it }, stringResource(R.string.devices_name), keyboard = KeyboardOptions(imeAction = ImeAction.Done))
        SheetButtons {
            QuietButton(stringResource(R.string.action_cancel)) { store.sync.show(null) }
            Button(enabled = name.isNotBlank(), onClick = { store.sync.rename(name); store.sync.show(null) }) { Text(stringResource(R.string.action_save)) }
        }
    }
}

/** A QR code, drawn from the engine's matrix: silk on the ground's own colour, with its quiet zone. */
@Composable
internal fun QrCode(text: String, modifier: Modifier = Modifier, size: androidx.compose.ui.unit.Dp = 168.dp, label: String) {
    val matrix = remember(text) { qrMatrix(text) }
    val dark = MaterialTheme.colorScheme.onSurface
    val light = MaterialTheme.colorScheme.surfaceContainerHighest
    Canvas(modifier.size(size).clip(RoundedCornerShape(12.dp)).semantics { contentDescription = label }) {
        drawRect(light)
        val side = matrix.size.toInt()
        val zone = matrix.quietZone.toInt()
        val cell = this.size.width / (side + 2 * zone)
        for (y in 0 until side) for (x in 0 until side) {
            if (matrix.modules[y * side + x]) drawRect(dark, Offset((x + zone) * cell, (y + zone) * cell), Size(cell + 0.5f, cell + 0.5f))
        }
    }
}

/** Add a device: the code the other device shows, or a code of this one to show there. */
@Composable
internal fun AddDeviceDialog() {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val problem by store.sync.problem.collectAsStateWithLifecycle()
    val name = deviceName()
    var code by remember { mutableStateOf("") }
    val digits = code.filter { it.isDigit() }
    SettingsSheet(onDismiss = { store.sync.show(null) }, holdsInput = code.isNotEmpty()) {
        SheetTitle(stringResource(R.string.devices_add))
        SheetText(stringResource(R.string.pair_enter_line))
        SheetField(
            if (digits.length > 3) digits.take(3) + " " + digits.drop(3).take(3) else digits,
            { code = it.filter(Char::isDigit).take(6) },
            stringResource(R.string.pair_code_field),
            modifier = Modifier.padding(top = 16.dp),
            keyboard = KeyboardOptions(keyboardType = KeyboardType.NumberPassword, imeAction = ImeAction.Go),
            supporting = stringResource(R.string.pair_same_wifi),
            error = problem != null,
        )
        problemWords(context, problem)?.let { SheetProblem(it) }
        SheetButtons {
            QuietButton(stringResource(R.string.pair_show_instead)) { store.pairInvite(name); store.sync.show(SyncSheet.CODE) }
            Button(enabled = digits.length == 6, onClick = { store.pairJoin(name, "", digits) }) { Text(stringResource(R.string.pair_button)) }
        }
    }
}

/** The code this device shows for another to enter, good for 2 minutes. */
@Composable
internal fun ShowCodeDialog() {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val invite by store.sync.invite.collectAsStateWithLifecycle()
    val colors = MaterialTheme.colorScheme
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
    SettingsSheet(onDismiss = { store.pairStop(); store.sync.show(null) }) {
        SheetTitle(stringResource(R.string.pair_show_title))
        val shown = invite
        if (shown != null) {
            Text(
                shown.code.chunked(3).joinToString(" "), // i18n-skip
                style = MaterialTheme.typography.displayMedium.copy(fontFamily = FontFamily.Monospace, letterSpacing = 4.sp),
                modifier = Modifier.padding(vertical = 16.dp),
            )
            QrCode("kaeri://pair?port=${shown.port}&code=${shown.code}", label = stringResource(R.string.pair_show_scan))
            SheetText(stringResource(R.string.pair_show_scan), Modifier.padding(top = 8.dp))
            Text(Strings.pairGoodFor(context, left), style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant, modifier = Modifier.padding(top = 8.dp))
        } else {
            SheetText(stringResource(R.string.pair_err_expired))
        }
        SheetButtons {
            QuietButton(stringResource(R.string.action_cancel)) { store.pairStop(); store.sync.show(null) }
            if (shown == null) Button(onClick = { store.pairInvite(deviceName(context)) }) { Text(stringResource(R.string.pair_show_code)) }
        }
    }
}

/** Pairing worked: who with, and that the history comes over while the app stays usable. */
@Composable
internal fun PairOkSheet(copied: app.kaeri.store.SignInsCopied) {
    val store = LocalAppStore.current
    SettingsSheet(onDismiss = { store.sync.copiedSeen() }) {
        SheetTitle(stringResource(R.string.pair_ok, copied.device))
        SheetText(stringResource(R.string.pair_ok_line))
        SheetButtons { Button(onClick = { store.sync.copiedSeen() }) { Text(stringResource(R.string.action_done)) } }
    }
}

private fun deviceName(context: Context): String = systemDeviceName(context)

/** The recovery key, full screen: the words, Copy, Save as a file, QR code, and Done. */
@Composable
internal fun RecoveryKeyDialog(key: String) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val clipboard = LocalClipboardManager.current
    val colors = MaterialTheme.colorScheme
    var qr by remember { mutableStateOf(false) }
    var copied by remember { mutableStateOf(false) }
    val groups = key.filter { !it.isWhitespace() && it != '-' }.chunked(4)
    val save = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("text/plain")) { uri: Uri? ->
        uri?.let { runCatching { context.contentResolver.openOutputStream(it)?.use { out -> out.write((groups.joinToString(" ") + "\n").toByteArray()) } } }
    }
    LaunchedEffect(copied) {
        if (copied) {
            delay(2000)
            copied = false
        }
    }
    Dialog(onDismissRequest = {}, properties = DialogProperties(usePlatformDefaultWidth = false, dismissOnClickOutside = false, dismissOnBackPress = false, decorFitsSystemWindows = false)) {
        Box(Modifier.fillMaxSize().background(colors.background)) {
            Column(Modifier.fillMaxSize().padding(horizontal = 24.dp).padding(top = 72.dp, bottom = 24.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(stringResource(R.string.rkey_title), style = MaterialTheme.typography.headlineMedium)
                Text(stringResource(R.string.rkey_line), style = MaterialTheme.typography.bodyLarge, color = colors.onSurfaceVariant)
                Box(
                    Modifier.padding(vertical = 16.dp).fillMaxWidth().clip(RoundedCornerShape(16.dp)).background(colors.surfaceContainer).padding(20.dp),
                    contentAlignment = Alignment.Center,
                ) {
                    if (qr) {
                        QrCode(groups.joinToString(""), size = 200.dp, label = stringResource(R.string.rkey_qr_short))
                    } else {
                        Column(verticalArrangement = Arrangement.spacedBy(8.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                            groups.chunked(3).forEach { row ->
                                Text(
                                    row.joinToString("   "), // i18n-skip
                                    style = MaterialTheme.typography.titleLarge.copy(fontFamily = FontFamily.Monospace, letterSpacing = 1.sp),
                                )
                            }
                        }
                    }
                }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    TextButton(onClick = { clipboard.setText(AnnotatedString(groups.joinToString(" "))); copied = true }) {
                        Icon(if (copied) Icons.Check else SyncIcons.Copy, null, Modifier.padding(end = 8.dp))
                        Text(if (copied) stringResource(R.string.action_copied) else stringResource(R.string.rkey_copy))
                    }
                    TextButton(onClick = { save.launch("kaeri-recovery-key.txt") }) {
                        Icon(SyncIcons.Save, null, Modifier.padding(end = 8.dp))
                        Text(stringResource(R.string.rkey_save_file))
                    }
                    TextButton(onClick = { qr = !qr }) {
                        Icon(SyncIcons.Qr, null, Modifier.padding(end = 8.dp))
                        Text(stringResource(R.string.rkey_qr_short))
                    }
                }
                Text(Strings.rkeyAgain(context, kindWord()), style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant, modifier = Modifier.padding(top = 8.dp))
                Box(Modifier.weight(1f))
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
                    Button(onClick = { store.sync.recoveryKeySeen() }) { Text(stringResource(R.string.action_done)) }
                }
            }
        }
    }
}

/** The device page: the Sync and Remote switches, and Remove this device. */
@Composable
internal fun DevicePage(device: app.kaeri.engine.model.SyncDeviceView, onRemove: () -> Unit, onDismiss: () -> Unit) {
    val context = LocalContext.current
    val isTv = deviceIcon(device.name) == SyncIcons.Tv
    SettingsSheet(onDismiss = onDismiss) {
        SheetTitle(device.name)
        lastSeenLine(context, device.lastSeen)?.let { SheetText(it) }
        Column(Modifier.padding(top = 16.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            SwitchRow(Place.First, stringResource(R.string.devices_sync_switch), true) {}
            if (isTv) SwitchRow(Place.Last, stringResource(R.string.devices_remote_switch), true) {}
        }
        SheetText(Strings.devicesSwitchesFoot(context, device.name), Modifier.padding(top = 8.dp))
        SheetButtons {
            QuietButton(stringResource(R.string.action_cancel), onClick = onDismiss)
            TextButton(onClick = onRemove) { Text(stringResource(R.string.devices_remove), color = MaterialTheme.colorScheme.error) }
        }
    }
}
