// Media servers on the phone (AndroidServers): Add a media server as pushed steps (Find, Sign in,
// Quick Connect, Libraries), and a server's page. Each failure sits under the field or button
// that caused it, as a vermilion glyph and a sentence; the field keeps its text.

package app.kaeri.ui

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.size
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.toggleableState
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.navigation.NavGraphBuilder
import androidx.navigation.compose.composable
import app.kaeri.design.Motion
import app.kaeri.design.tabular
import app.kaeri.engine.model.LibraryChoice
import app.kaeri.engine.model.ServerFailure
import app.kaeri.engine.model.ServerRow
import app.kaeri.engine.model.ServerStep
import app.kaeri.i18n.R
import app.kaeri.store.AddServerState
import app.kaeri.store.AddServerStep
import androidx.compose.ui.platform.LocalResources

internal const val ADD_SERVER_ROUTE = "servers/add"

internal fun serverRoute(id: String) = "server/${android.net.Uri.encode(id)}"

/** The servers' routes: Add a media server, and a server's page. */
internal fun NavGraphBuilder.serverPages() {
    composable(ADD_SERVER_ROUTE) { AddServerScreen() }
    composable("server/{id}") { entry ->
        ServerScreen(entry.arguments?.getString("id") ?: return@composable)
    }
}

/** What a failure says, in the app's own words. */
@Composable
internal fun serverProblemText(failure: ServerFailure): String = when (failure) {
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

/** The steps of Add a media server, one pushed page whose body slides from step to step. */
@Composable
internal fun AddServerScreen() {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val flow by store.servers.flow.collectAsStateWithLifecycle()
    // Opened once per visit; closed with the page, which also stops a Quick Connect wait.
    DisposableEffect(Unit) {
        store.openAddServer()
        onDispose { store.closeAddServer() }
    }
    val state = flow
    val step = state?.step ?: AddServerStep.Find
    val title = when {
        state?.problem?.failure is ServerFailure.Certificate -> stringResource(R.string.servers_cert_title)
        step == AddServerStep.Find -> stringResource(R.string.servers_add_title)
        step == AddServerStep.SignIn -> stringResource(R.string.servers_sign_in_title, state?.checked?.name.orEmpty())
        step == AddServerStep.QuickConnect -> stringResource(R.string.servers_qc_title)
        else -> stringResource(R.string.servers_libraries_title)
    }
    ScreenFrame(
        title = title,
        onBack = { if (step == AddServerStep.Find || step == AddServerStep.Libraries) navigator.back() else store.addServerBack() },
    ) { padding ->
        if (state == null) return@ScreenFrame
        AnimatedContent(
            targetState = step,
            transitionSpec = {
                val forward = targetState.ordinal > initialState.ordinal
                (slideInHorizontally(Motion.standard()) { if (forward) 72 else -72 } + fadeIn(Motion.fadeIn())) togetherWith
                    (slideOutHorizontally(Motion.standard()) { if (forward) -72 else 72 } + fadeOut(Motion.fadeOut()))
            },
            label = "add-server-step", // i18n-skip
        ) { shown ->
            Column(
                Modifier
                    .fillMaxSize()
                    .verticalScroll(rememberScrollState())
                    .padding(top = padding.calculateTopPadding() + 8.dp, bottom = 48.dp)
                    .padding(horizontal = 16.dp)
                    .readingWidth(),
            ) {
                when {
                    state.problem?.failure is ServerFailure.Certificate -> CertificateStep(state) { navigator.back() }
                    shown == AddServerStep.Find -> FindStep(state)
                    shown == AddServerStep.SignIn -> SignInStep(state)
                    shown == AddServerStep.QuickConnect -> QuickConnectStep(state)
                    else -> LibrariesStep(state) { navigator.back() }
                }
            }
        }
    }
}

/** Step 1: servers on the network as they arrive, and the address field. */
@Composable
private fun FindStep(state: AddServerState) {
    val store = LocalAppStore.current
    var address by rememberSaveable { mutableStateOf(state.address) }
    // Silent for 600 ms, then the looking line; after the broadcast ended with nothing, the hint.
    val looking = after(!state.discoveryDone && state.found.isEmpty())
    SettingsHeader(stringResource(R.string.servers_on_network))
    Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
        val rows = state.found
        rows.forEachIndexed { index, found ->
            InfoRow(
                Place.of(index, rows.size),
                found.name,
                found.address.removePrefix("http://").removePrefix("https://"),
                onClick = { address = found.address.removePrefix("http://").removePrefix("https://") },
            )
        }
        if (rows.isEmpty() && looking) {
            InfoRow(Place.Only, stringResource(R.string.servers_looking), null, leading = { WorkRing(true) })
        }
    }
    if (state.found.isEmpty() && state.discoveryDone) {
        SettingsFootnote(stringResource(R.string.servers_none_found))
    }
    Spacer(Modifier.height(24.dp))
    val go = { if (address.isNotBlank() && !state.checking) store.checkServer(address) }
    SheetField(
        address,
        { address = it },
        stringResource(R.string.servers_address),
        keyboard = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Go, autoCorrectEnabled = false),
        actions = KeyboardActions(onGo = { go() }),
        supporting = if (address.isEmpty()) stringResource(R.string.servers_address_hint) else null,
        error = state.problem?.step == ServerStep.ADDRESS,
    )
    state.problem?.takeIf { it.step == ServerStep.ADDRESS }?.let { SheetProblem(serverProblemText(it.failure)) }
    SheetButtons {
        Button(onClick = go, enabled = address.isNotBlank() && !state.checking) {
            WorkLabel(stringResource(R.string.servers_continue), state.checking)
        }
    }
}

/** Step 3: user name and password, or Quick Connect when the server has it. */
@Composable
private fun SignInStep(state: AddServerState) {
    val store = LocalAppStore.current
    var user by rememberSaveable { mutableStateOf("") }
    // Never saved: the password lives only as long as the page.
    var password by remember { mutableStateOf("") }
    val address = state.checked?.address ?: state.address
    val go = { if (user.isNotBlank() && password.isNotEmpty() && !state.signingIn) store.signInToServer(address, user, password) }
    SheetField(
        user,
        { user = it },
        stringResource(R.string.servers_user),
        keyboard = KeyboardOptions(imeAction = ImeAction.Next, autoCorrectEnabled = false),
    )
    Spacer(Modifier.height(12.dp))
    SheetField(
        password,
        { password = it },
        stringResource(R.string.servers_password),
        password = true,
        keyboard = KeyboardOptions(keyboardType = KeyboardType.Password, imeAction = ImeAction.Done, autoCorrectEnabled = false),
        actions = KeyboardActions(onDone = { go() }),
        error = state.problem?.failure == ServerFailure.Credentials,
    )
    state.problem?.takeIf { it.step == ServerStep.SIGN_IN }?.let { SheetProblem(serverProblemText(it.failure)) }
    val qcOff = state.problem?.failure == ServerFailure.QuickConnectOff
    if (state.checked?.quickConnect == true && !qcOff) {
        QuietButton(stringResource(R.string.servers_use_quick_connect)) { store.startQuickConnect(address) }
    }
    SheetText(stringResource(R.string.servers_token_note, stringResource(R.string.secure_store_android)), Modifier.padding(top = 16.dp))
    SheetButtons {
        Button(onClick = go, enabled = user.isNotBlank() && password.isNotEmpty() && !state.signingIn) {
            WorkLabel(stringResource(R.string.servers_sign_in), state.signingIn)
        }
    }
}

/** Step 4: the six-digit code in large tabular figures, and the wait for approval. */
@Composable
private fun QuickConnectStep(state: AddServerState) {
    val store = LocalAppStore.current
    val expired = state.problem?.failure == ServerFailure.QuickConnectExpired
    SheetText(stringResource(R.string.servers_qc_steps))
    Text(
        state.code.orEmpty().chunked(3).joinToString(" "),
        style = MaterialTheme.typography.displayMedium.tabular(),
        color = if (expired) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.onSurface,
        textAlign = TextAlign.Start,
        modifier = Modifier.padding(vertical = 32.dp),
    )
    if (expired) {
        SheetProblem(stringResource(R.string.servers_qc_expired))
        SheetButtons {
            Button(onClick = { store.startQuickConnect(state.checked?.address ?: state.address) }) {
                Text(stringResource(R.string.servers_qc_new_code))
            }
        }
    } else {
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            WorkRing(true)
            SheetText(stringResource(R.string.servers_qc_waiting))
        }
        QuietButton(stringResource(R.string.servers_use_password)) { store.usePasswordInstead() }
    }
}

/** Step 5: a check row per library, the reporting switch, and Add server. */
@Composable
private fun LibrariesStep(state: AddServerState, done: () -> Unit) {
    val store = LocalAppStore.current
    val server = state.server ?: return
    // The engine starts anime-looking libraries ticked; the viewer's changes are kept here.
    var changed by remember(state.libraries) { mutableStateOf<Map<String, Boolean>>(emptyMap()) }
    var report by rememberSaveable { mutableStateOf(true) }
    val ticked = state.libraries.filter { changed[it.id] ?: it.ticked }
    SheetText(stringResource(R.string.servers_libraries_body))
    Spacer(Modifier.height(16.dp))
    Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
        state.libraries.forEachIndexed { index, library ->
            val on = changed[library.id] ?: library.ticked
            CheckRow(
                Place.of(index, state.libraries.size),
                library.name,
                libraryLine(library),
                on,
            ) { changed = changed + (library.id to !on) }
        }
    }
    Spacer(Modifier.height(24.dp))
    Column { SwitchRow(Place.Only, stringResource(R.string.servers_report), report, note = stringResource(R.string.servers_report_line)) { report = it } }
    SheetButtons {
        Button(
            onClick = {
                store.setServerLibraries(server, ticked.map { it.id })
                store.setServerReport(server, report)
                store.loadServers()
                done()
            },
            enabled = state.libraries.isNotEmpty(),
        ) { Text(stringResource(R.string.servers_add)) }
    }
}

/** A library's kind and how many series it holds, as its row says them. */
@Composable
private fun libraryLine(library: LibraryChoice): String? =
    library.kind?.replaceFirstChar { it.uppercase() }

/** Step for an untrusted certificate: why, the fingerprint to compare, and Allow this certificate as a secondary button. */
@Composable
private fun CertificateStep(state: AddServerState, cancel: () -> Unit) {
    val store = LocalAppStore.current
    val failure = state.problem?.failure as? ServerFailure.Certificate ?: return
    val certificate = state.certificate
    SheetText(stringResource(R.string.servers_cert_body, failure.host))
    if (certificate != null) {
        Text(
            certificate.groups.chunked(4).joinToString("\n") { it.joinToString(" ") }, // i18n-skip
            style = MaterialTheme.typography.bodyMedium.copy(fontFamily = androidx.compose.ui.text.font.FontFamily.Monospace),
            modifier = Modifier.padding(vertical = 12.dp).semantics { contentDescription = certificate.groups.joinToString(" ") }, // i18n-skip
        )
    }
    SheetButtons {
        QuietButton(stringResource(R.string.action_cancel), onClick = cancel)
        if (certificate != null) {
            OutlinedButton(onClick = { store.trustCertificate(certificate.address, certificate.fingerprint) }) {
                Text(stringResource(R.string.servers_cert_allow))
            }
        }
    }
}

/** A row with a check mark at its end: a library to read. */
@Composable
internal fun CheckRow(place: Place, title: String, note: String?, checked: Boolean, onChange: () -> Unit) {
    RowFrame(
        place,
        onClick = onChange,
        modifier = Modifier.heightIn(min = 64.dp).semantics(mergeDescendants = true) {
            role = androidx.compose.ui.semantics.Role.Checkbox
            toggleableState = if (checked) androidx.compose.ui.state.ToggleableState.On else androidx.compose.ui.state.ToggleableState.Off
        },
    ) { selected ->
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            RowText(title, null, note, selected, Modifier.weight(1f))
            if (checked) Icon(Icons.Check, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
            else Spacer(Modifier.size(24.dp))
        }
    }
}

/** A server's page: where it is, who is signed in, its libraries, reporting, and the ends. */
@Composable
internal fun ServerScreen(id: String) {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val snackbars = LocalSnackbars.current
    val rows by store.servers.rows.collectAsStateWithLifecycle()
    val row = rows.firstOrNull { it.id == id }
    val context = androidx.compose.ui.platform.LocalContext.current
    val resources = LocalResources.current
    LaunchedEffect(Unit) { store.loadServers() }
    ScreenFrame(title = row?.name.orEmpty(), subtitle = row?.let { stringResource(R.string.servers_page_line, it.address.removePrefix("http://"), it.version, it.user) }, onBack = navigator::back) { padding ->
        if (row == null) return@ScreenFrame
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(top = padding.calculateTopPadding() + 8.dp, bottom = 48.dp)
                .padding(horizontal = 16.dp)
                .readingWidth(),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            if (row.signedOut) {
                Row(Modifier.padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                    StateDot(MaterialTheme.colorScheme.error)
                    Text(stringResource(if (row.signedOutHere) R.string.servers_row_signed_out_here else R.string.servers_row_signed_out), style = MaterialTheme.typography.bodyMedium)
                }
                OutlinedButton(onClick = { navigator.open(ADD_SERVER_ROUTE) }) { Text(stringResource(R.string.servers_sign_in_again)) }
            }
            SettingsGroup(
                title = stringResource(R.string.servers_libraries_title),
                rows = row.libraries.map { library ->
                    { place: Place -> InfoRow(place, library.name, libraryLine(library)) }
                },
            )
            SettingsGroup(
                footer = stringResource(R.string.servers_report_line),
                rows = listOf { place -> SwitchRow(place, stringResource(R.string.servers_report), row.report) { store.setServerReport(row.id, it) } },
            )
            SettingsGroup(
                rows = listOfNotNull<@Composable (Place) -> Unit>(
                    { place -> LinkRow(place, stringResource(R.string.servers_refresh), null, onClick = { store.refreshServer(row.id) }) },

                    if (row.signedOut) null else { place: Place -> LinkRow(place, stringResource(R.string.servers_sign_out), stringResource(R.string.servers_sign_out_line), onClick = { store.signOutOfServer(row.id) }) },
                    { place ->
                        LinkRow(place, stringResource(R.string.servers_remove), null, onClick = {
                            store.removeServer(row.id)
                            snackbars.show(resources.getString(R.string.servers_removed, row.name), resources.getString(R.string.action_undo)) { store.restoreServer(row.id) }
                            navigator.back()
                        })
                    },
                ),
                footer = stringResource(R.string.servers_remove_line),
            )
        }
    }
}

/** The line under a server's row on the Sources page: its libraries and series, and how many need a look. */
@Composable
internal fun serverRowLine(row: ServerRow): String {
    if (row.signedOut) return stringResource(if (row.signedOutHere) R.string.servers_row_signed_out_here else R.string.servers_row_signed_out)
    if (row.notAnswering) {
        val seen = row.lastSeen
        return if (seen != null) stringResource(R.string.servers_row_not_answering, app.kaeri.store.AirTime.seen(seen)) else stringResource(R.string.sources_status_not_answering)
    }
    val libraries = row.libraries.count { it.ticked }
    val summary = pluralStringResource(R.plurals.servers_row_libraries, libraries, libraries) + " · " + pluralStringResource(R.plurals.servers_row_series, row.series.toInt(), row.series.toInt())
    val unmatched = (row.series - row.matched).toInt()
    return if (unmatched > 0) "$summary · ${stringResource(R.string.servers_row_needs_look, unmatched)}" else summary // i18n-skip
}
