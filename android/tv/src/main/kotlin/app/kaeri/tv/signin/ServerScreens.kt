// TVServers: Add a media server on the TV, one full-screen page per step. Typing with a remote is
// slow, so Quick Connect leads when the server has it and a password is the second button. A
// failure sits under the step that caused it: a vermilion dot and a sentence.

package app.kaeri.tv.signin

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import kotlinx.coroutines.delay
import app.kaeri.design.ObiSilk
import app.kaeri.design.tabular
import app.kaeri.engine.model.ServerFailure
import app.kaeri.engine.model.ServerStep
import app.kaeri.i18n.R
import app.kaeri.store.AddServerState
import app.kaeri.store.AddServerStep
import app.kaeri.tv.player.Spinner
import app.kaeri.tv.ui.AmberButton
import app.kaeri.tv.ui.ButtonLabel
import app.kaeri.tv.ui.Canvas
import app.kaeri.tv.ui.LocalAppStore
import app.kaeri.tv.ui.TonalButton
import app.kaeri.tv.ui.TvListRow
import app.kaeri.tv.ui.entryFocus
import app.kaeri.tv.ui.focusSoon

/** What a failure says, in the app's own words. */
@Composable
internal fun serverProblem(failure: ServerFailure): String = when (failure) {
    is ServerFailure.Unreachable -> stringResource(R.string.servers_error_unreachable, failure.address)
    ServerFailure.NotJellyfin -> stringResource(R.string.servers_error_not_jellyfin)
    is ServerFailure.TooOld -> stringResource(R.string.servers_error_too_old, failure.version)
    ServerFailure.Credentials -> stringResource(R.string.servers_error_credentials)
    ServerFailure.QuickConnectOff -> stringResource(R.string.servers_error_qc_off)
    ServerFailure.QuickConnectExpired -> stringResource(R.string.servers_qc_expired)
    ServerFailure.PlainHttp -> stringResource(R.string.servers_error_plain_http)
    is ServerFailure.Certificate -> stringResource(R.string.servers_cert_body, failure.host)
    ServerFailure.SignedOut -> stringResource(R.string.servers_row_signed_out)
    ServerFailure.BadAddress -> stringResource(R.string.servers_error_bad_address)
    ServerFailure.Other -> stringResource(R.string.servers_error_other)
}

@Composable
fun AddServerScreen(onDone: () -> Unit) {
    val store = LocalAppStore.current
    val flow by store.servers.flow.collectAsStateWithLifecycle()
    DisposableEffect(Unit) {
        store.openAddServer()
        onDispose { store.closeAddServer() }
    }
    val state = flow ?: return
    BackHandler {
        if (state.step == AddServerStep.Find || state.step == AddServerStep.Libraries) onDone() else store.addServerBack()
    }
    // Quick Connect leads once, when the server has it; a password is the second button.
    var tried by remember(state.checked?.id) { mutableStateOf(false) }
    LaunchedEffect(state.checked?.id, state.step) {
        val checked = state.checked ?: return@LaunchedEffect
        if (checked.quickConnect && !tried && state.step == AddServerStep.SignIn && state.problem == null) {
            tried = true
            store.startQuickConnect(checked.address)
        }
    }
    val certificate = state.problem?.failure as? ServerFailure.Certificate
    val title = when {
        certificate != null -> stringResource(R.string.servers_cert_title)
        state.step == AddServerStep.Find -> stringResource(R.string.servers_add_title)
        state.step == AddServerStep.SignIn -> stringResource(R.string.servers_sign_in_title, state.checked?.name.orEmpty())
        state.step == AddServerStep.QuickConnect -> stringResource(R.string.servers_qc_title)
        else -> stringResource(R.string.servers_libraries_title)
    }
    Column(
        Modifier
            .fillMaxSize()
            .background(ObiSilk.Ground)
            .padding(start = Canvas.Keyline, end = Canvas.SafeX, top = 72.dp),
        verticalArrangement = Arrangement.spacedBy(20.dp),
    ) {
        Text(title, style = MaterialTheme.typography.titleLarge, color = ObiSilk.Silk, modifier = Modifier.semantics { heading() })
        when {
            certificate != null -> {
                Text(stringResource(R.string.servers_cert_body, certificate.host), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk, modifier = Modifier.widthIn(max = 640.dp))
                val cancel = remember { FocusRequester() }
                Row(Modifier.entryFocus(cancel)) { TonalButton(onClick = onDone, modifier = Modifier.focusRequester(cancel)) { ButtonLabel(stringResource(R.string.action_cancel)) } }
            }
            state.step == AddServerStep.Find -> FindStep(state, onDone)
            state.step == AddServerStep.SignIn -> PasswordStep(state)
            state.step == AddServerStep.QuickConnect -> QuickConnectStep(state)
            else -> LibrariesStep(state, onDone)
        }
    }
}

@Composable
private fun Problem(text: String) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Box(Modifier.size(8.dp).background(ObiSilk.Vermilion, CircleShape))
        Text(text, style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk, modifier = Modifier.widthIn(max = 640.dp))
    }
}

/** The fingerprint to compare, in monospace groups, and Allow this certificate as a tonal button. */
@Composable
private fun CertificateBlock(state: AddServerState) {
    val store = LocalAppStore.current
    val certificate = state.certificate ?: return
    if (state.problem?.failure !is ServerFailure.Certificate) return
    Text(
        certificate.groups.chunked(4).joinToString("\n") { it.joinToString(" ") }, // i18n-skip
        style = MaterialTheme.typography.bodyLarge.copy(fontFamily = androidx.compose.ui.text.font.FontFamily.Monospace),
        color = ObiSilk.Silk,
    )
    TonalButton(onClick = { store.trustCertificate(certificate.address, certificate.fingerprint) }) { ButtonLabel(stringResource(R.string.servers_cert_allow)) }
}

@Composable
private fun Field(value: String, onChange: (String) -> Unit, label: String, modifier: Modifier = Modifier, password: Boolean = false, imeAction: ImeAction = ImeAction.Next, onGo: () -> Unit = {}) {
    Box(
        modifier.widthIn(max = 640.dp).fillMaxWidth().height(56.dp).background(ObiSilk.Raised, CircleShape).padding(horizontal = 24.dp),
        contentAlignment = Alignment.CenterStart,
    ) {
        if (value.isEmpty()) Text(label, style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze)
        BasicTextField(
            value = value,
            onValueChange = onChange,
            singleLine = true,
            textStyle = MaterialTheme.typography.bodyLarge.copy(color = ObiSilk.Silk),
            cursorBrush = SolidColor(ObiSilk.Amber),
            visualTransformation = if (password) PasswordVisualTransformation() else VisualTransformation.None,
            keyboardOptions = KeyboardOptions(keyboardType = if (password) KeyboardType.Password else KeyboardType.Uri, imeAction = imeAction, autoCorrectEnabled = false),
            keyboardActions = KeyboardActions(onGo = { onGo() }, onDone = { onGo() }),
            modifier = Modifier.fillMaxWidth(),
        )
    }
}

/** Servers on the network, as they arrive; or the address typed with the system keyboard. */
@Composable
private fun FindStep(state: AddServerState, onCancel: () -> Unit) {
    val store = LocalAppStore.current
    var typing by remember { mutableStateOf(false) }
    var address by remember { mutableStateOf(state.address) }
    var looking by remember { mutableStateOf(false) }
    LaunchedEffect(state.discoveryDone, state.found.isEmpty()) {
        looking = false
        if (!state.discoveryDone && state.found.isEmpty()) {
            delay(600)
            looking = true
        }
    }
    val first = remember { FocusRequester() }
    val field = remember { FocusRequester() }
    val keyboard = LocalSoftwareKeyboardController.current
    LaunchedEffect(typing) { if (typing) { field.focusSoon(); keyboard?.show() } }
    if (typing) {
        Field(address, { address = it }, stringResource(R.string.servers_address_hint), Modifier.focusRequester(field), imeAction = ImeAction.Go, onGo = { if (address.isNotBlank()) store.checkServer(address) })
        state.problem?.takeIf { it.step == ServerStep.ADDRESS }?.let { Problem(serverProblem(it.failure)) }
        CertificateBlock(state)
        Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            AmberButton(onClick = { store.checkServer(address) }, icon = null, enabled = address.isNotBlank() && !state.checking) { ButtonLabel(stringResource(R.string.servers_continue)) }
            TonalButton(onClick = { typing = false }) { ButtonLabel(stringResource(R.string.action_back)) }
        }
        return
    }
    Text(stringResource(R.string.servers_on_network), style = MaterialTheme.typography.labelLarge, color = ObiSilk.Haze)
    LazyColumn(Modifier.widthIn(max = 640.dp).height(220.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        items(state.found, key = { it.id }) { found ->
            TvListRow(
                found.name,
                onClick = { store.checkServer(found.address) },
                supporting = found.address.removePrefix("http://").removePrefix("https://"),
                modifier = if (found == state.found.first()) Modifier.focusRequester(first) else Modifier,
            )
        }
        if (state.found.isEmpty() && looking) item(key = "looking") {
            Row(Modifier.padding(16.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                Box(Modifier.size(24.dp)) { Spinner() }
                Text(stringResource(R.string.servers_looking), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze)
            }
        }
        if (state.found.isEmpty() && state.discoveryDone) item(key = "none") {
            Text(stringResource(R.string.servers_none_found), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze, modifier = Modifier.padding(16.dp))
        }
    }
    state.problem?.takeIf { it.step == ServerStep.ADDRESS }?.let { Problem(serverProblem(it.failure)) }
    CertificateBlock(state)
    val type = remember { FocusRequester() }
    Row(Modifier.entryFocus(if (state.found.isEmpty()) type else first), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        TonalButton(onClick = { typing = true }, modifier = Modifier.focusRequester(type)) { ButtonLabel(stringResource(R.string.servers_address)) }
        TonalButton(onClick = onCancel) { ButtonLabel(stringResource(R.string.action_cancel)) }
    }
}

/** User name and password, typed with the system keyboard. */
@Composable
private fun PasswordStep(state: AddServerState) {
    val store = LocalAppStore.current
    var user by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }
    val address = state.checked?.address ?: state.address
    val go = { if (user.isNotBlank() && password.isNotEmpty() && !state.signingIn) store.signInToServer(address, user, password) }
    val userField = remember { FocusRequester() }
    LaunchedEffect(Unit) { userField.focusSoon() }
    Field(user, { user = it }, stringResource(R.string.servers_user), Modifier.focusRequester(userField))
    Field(password, { password = it }, stringResource(R.string.servers_password), password = true, imeAction = ImeAction.Done, onGo = go)
    state.problem?.takeIf { it.step == ServerStep.SIGN_IN }?.let { Problem(serverProblem(it.failure)) }
    Text(stringResource(R.string.servers_token_note, stringResource(R.string.secure_store_android)), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, modifier = Modifier.widthIn(max = 640.dp))
    Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        AmberButton(onClick = go, icon = null, enabled = user.isNotBlank() && password.isNotEmpty() && !state.signingIn) { ButtonLabel(stringResource(R.string.servers_sign_in)) }
        if (state.checked?.quickConnect == true && state.problem?.failure != ServerFailure.QuickConnectOff) {
            TonalButton(onClick = { store.startQuickConnect(address) }) { ButtonLabel(stringResource(R.string.servers_use_quick_connect)) }
        }
    }
}

/** The six-digit code in large tabular figures; approving it on any signed-in device moves on. */
@Composable
private fun QuickConnectStep(state: AddServerState) {
    val store = LocalAppStore.current
    val expired = state.problem?.failure == ServerFailure.QuickConnectExpired
    Text(stringResource(R.string.servers_qc_steps), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk, modifier = Modifier.widthIn(max = 640.dp))
    Text(
        state.code.orEmpty().chunked(3).joinToString(" "),
        style = MaterialTheme.typography.displayLarge.tabular(),
        color = if (expired) ObiSilk.Haze else ObiSilk.Silk,
    )
    val action = remember { FocusRequester() }
    if (expired) Problem(stringResource(R.string.servers_qc_expired))
    else Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        Box(Modifier.size(24.dp)) { Spinner() }
        Text(stringResource(R.string.servers_qc_waiting), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze)
    }
    Row(Modifier.entryFocus(action), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        if (expired) {
            AmberButton(onClick = { store.startQuickConnect(state.checked?.address ?: state.address) }, icon = null, modifier = Modifier.focusRequester(action)) { ButtonLabel(stringResource(R.string.servers_qc_new_code)) }
        } else {
            TonalButton(onClick = { store.usePasswordInstead() }, modifier = Modifier.focusRequester(action)) { ButtonLabel(stringResource(R.string.servers_use_password)) }
        }
    }
}

/** A row per library with its kind; ticked ones are read. */
@Composable
private fun LibrariesStep(state: AddServerState, onDone: () -> Unit) {
    val store = LocalAppStore.current
    val server = state.server ?: return
    var changed by remember(state.libraries) { mutableStateOf<Map<String, Boolean>>(emptyMap()) }
    var report by remember { mutableStateOf(true) }
    val ticked = state.libraries.filter { changed[it.id] ?: it.ticked }
    Text(stringResource(R.string.servers_libraries_body), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze, modifier = Modifier.widthIn(max = 640.dp))
    val first = remember { FocusRequester() }
    Column(Modifier.widthIn(max = 640.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        state.libraries.forEachIndexed { index, library ->
            val on = changed[library.id] ?: library.ticked
            TvListRow(
                library.name,
                onClick = { changed = changed + (library.id to !on) },
                supporting = library.kind?.replaceFirstChar { it.uppercase() },
                switch = on,
                modifier = if (index == 0) Modifier.focusRequester(first) else Modifier,
            )
        }
        TvListRow(stringResource(R.string.servers_report), onClick = { report = !report }, supporting = stringResource(R.string.servers_report_line), switch = report)
    }
    Row(Modifier.entryFocus(first), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        AmberButton(
            onClick = {
                store.setServerLibraries(server, ticked.map { it.id })
                store.setServerReport(server, report)
                store.loadServers()
                onDone()
            },
            icon = null,
            enabled = state.libraries.isNotEmpty(),
        ) { ButtonLabel(stringResource(R.string.servers_add)) }
    }
}
