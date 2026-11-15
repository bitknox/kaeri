// The torrent proxy's form (AndroidTorrentSheet): host, port, user name and password, opened by
// turning "Route torrent traffic through a proxy" on and by Edit proxy once one is set. A new or
// changed proxy is saved only once its check passes, and a failed check saves nothing and keeps the
// form open. Cancel leaves the toggle as it was.

package app.kaeri.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.res.stringResource
import app.kaeri.i18n.R
import androidx.compose.ui.Modifier
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.engine.model.TorrentProxy
import app.kaeri.store.ProvidersModel
import app.kaeri.store.TorrentWords

/**
 * The proxy's form. Save is the one amber action, and only once the host and port are whole. With
 * a proxy already saved it is Edit proxy: the fields filled in, the password blank to keep the
 * saved one.
 */
@Composable
internal fun TorrentSheet(onDismiss: () -> Unit) {
    val store = LocalAppStore.current
    val snackbars = LocalSnackbars.current
    val context = LocalContext.current
    val resources = LocalResources.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val saved = view?.torrent?.proxy
    var host by rememberSaveable { mutableStateOf(saved?.host.orEmpty()) }
    var port by rememberSaveable { mutableStateOf(saved?.port?.toString().orEmpty()) }
    var user by rememberSaveable { mutableStateOf(saved?.user.orEmpty()) }
    // Never saved with the screen's state: it goes to the keystore-sealed store, and nowhere else.
    var password by remember { mutableStateOf("") }
    val details = proxyOf(host, port, user, password)
    val check = proxyCheck(details) { checked ->
        store.setTorrentProxy(checked)
        password = ""
        snackbars.show(resources.getString(TorrentWords.SAID_PROXY))
        onDismiss()
    }
    val editing = saved != null
    SettingsSheet(onDismiss, tall = true, holdsInput = changedSinceOpened(host, port, user, password)) {
        SheetTitle(stringResource(if (editing) R.string.torrents_sheet_edit_proxy_title else R.string.torrents_proxy_route))
        SheetText(stringResource(if (editing) TorrentWords.EDIT_NOTE else TorrentWords.WHY))
        ProxyFields(
            host, port, user, password,
            onHost = { host = it }, onPort = { port = it }, onUser = { user = it }, onPassword = { password = it },
            Modifier.padding(top = 16.dp),
            note = stringResource(if (editing) TorrentWords.PASSWORD_KEPT else R.string.torrents_sheet_proxy_auth_note),
            enabled = !check.checking,
        )
        SheetText(stringResource(TorrentWords.CHECK_NOTE), Modifier.padding(top = 16.dp))
        CheckStatus(checking = check.checking, failed = if (check.failed) TorrentWords.proxyFailed(context) else null)
        SheetButtons {
            QuietButton(stringResource(R.string.action_cancel), onClick = onDismiss)
            Button(
                enabled = details != null && !check.checking,
                onClick = { details?.let { store.checkTorrentProxy(it) } },
            ) { Text(stringResource(R.string.action_save)) }
        }
    }
}

/** A SOCKS5 proxy's host, port, user name and password, as the sheet and Edit proxy ask for them. */
@Composable
private fun ProxyFields(
    host: String,
    port: String,
    user: String,
    password: String,
    onHost: (String) -> Unit,
    onPort: (String) -> Unit,
    onUser: (String) -> Unit,
    onPassword: (String) -> Unit,
    modifier: Modifier = Modifier,
    note: String = stringResource(R.string.torrents_sheet_proxy_auth_note),
    enabled: Boolean = true,
) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            SheetField(host, onHost, stringResource(R.string.torrents_proxy_host), Modifier.weight(2f), keyboard = KeyboardOptions(keyboardType = KeyboardType.Uri, autoCorrectEnabled = false, imeAction = ImeAction.Next), enabled = enabled)
            SheetField(port, { onPort(it.filter(Char::isDigit).take(5)) }, stringResource(R.string.torrents_proxy_port), Modifier.weight(1f), keyboard = KeyboardOptions(keyboardType = KeyboardType.Number, imeAction = ImeAction.Next), enabled = enabled)
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            SheetField(user, onUser, stringResource(R.string.torrents_proxy_user), Modifier.weight(1f), keyboard = KeyboardOptions(autoCorrectEnabled = false, imeAction = ImeAction.Next), enabled = enabled)
            SheetField(password, onPassword, stringResource(R.string.torrents_proxy_password), Modifier.weight(1f), password = true, keyboard = KeyboardOptions(keyboardType = KeyboardType.Password, imeAction = ImeAction.Done), enabled = enabled)
        }
        SheetText(note)
    }
}

/** What a proxy's check before saving says on screen: under way, or that it failed. */
private class ProxyCheck(val checking: Boolean, val failed: Boolean)

/**
 * The check of [asked], the details the form holds now: a verdict on any other details, ones
 * since changed or a check from before, is set aside. A pass hands [onPassed] exactly what was
 * checked, the password as given included.
 */
@Composable
private fun proxyCheck(asked: TorrentProxy?, onPassed: (TorrentProxy) -> Unit): ProxyCheck {
    val store = LocalAppStore.current
    val trial by store.providers.trial.collectAsStateWithLifecycle()
    val mine = trial?.takeIf { asked != null && it.proxy == asked }
    val passed by rememberUpdatedState(onPassed)
    DisposableEffect(Unit) { onDispose { store.providers.clearTrial() } }
    LaunchedEffect(mine) {
        val said = mine as? ProvidersModel.Trial.Said ?: return@LaunchedEffect
        if (said.passed) {
            store.providers.clearTrial()
            passed(said.proxy)
        }
    }
    return ProxyCheck(
        checking = mine is ProvidersModel.Trial.Checking,
        failed = (mine as? ProvidersModel.Trial.Said)?.passed == false,
    )
}

/**
 * The check's line, in a place held for two lines whatever is in it, so the sheet neither grows
 * nor moves its buttons: the ring and "Checking the proxy" while it runs, or the error glyph and
 * why it failed, with Reject. TalkBack says each as it comes.
 */
@Composable
private fun CheckStatus(checking: Boolean, failed: String?) {
    Box(
        Modifier
            .fillMaxWidth()
            .padding(top = 16.dp)
            .heightIn(min = STATUS_HEIGHT)
            .semantics(mergeDescendants = true) { liveRegion = LiveRegionMode.Polite },
        contentAlignment = Alignment.CenterStart,
    ) {
        when {
            checking -> Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                WorkRing(true)
                SheetText(stringResource(TorrentWords.CHECKING_PROXY))
            }
            failed != null -> {
                val haptics = LocalHapticFeedback.current
                LaunchedEffect(failed) { haptics.performHapticFeedback(HapticFeedbackType.Reject) }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                    Icon(Icons.Problem, contentDescription = stringResource(R.string.error_label), tint = MaterialTheme.colorScheme.error, modifier = Modifier.size(18.dp))
                    Text(failed, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurface)
                }
            }
        }
    }
}

/** Two lines of bodyMedium: the longest a failed check says. */
private val STATUS_HEIGHT = 40.dp

/** The proxy the fields make, once its host and port are whole. A password left empty keeps the one saved. */
private fun proxyOf(host: String, port: String, user: String, password: String): TorrentProxy? {
    if (!TorrentWords.proxyOk(host, port)) return null
    return TorrentProxy(
        host = host.trim(),
        port = port.trim().toInt().toUShort(),
        user = user.trim().ifEmpty { null },
        password = password.takeIf { it.isNotEmpty() },
    )
}
