// TVTorrents: the SOCKS5 proxy's form on the TV, opened by turning "Route torrent traffic through a
// proxy" on and by Edit proxy once one is set. The proxy's address comes from the TV's own page on
// a phone, or is typed with the system keyboard as the last resort. A new or changed proxy is saved
// only once its check passes, and a failed check saves nothing and keeps the form open.

package app.kaeri.tv.signin

import androidx.compose.ui.res.stringResource
import app.kaeri.i18n.R
import android.content.Context
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
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import app.kaeri.design.ObiSilk
import app.kaeri.engine.model.TorrentProxy
import app.kaeri.store.ProvidersModel
import app.kaeri.store.TorrentWords
import app.kaeri.tv.player.Spinner
import app.kaeri.tv.remote.QrCode
import app.kaeri.tv.remote.localAddress
import app.kaeri.tv.ui.AmberButton
import app.kaeri.tv.ui.ButtonLabel
import app.kaeri.tv.ui.Canvas
import app.kaeri.tv.ui.LocalAppStore
import app.kaeri.tv.ui.LocalNotices
import app.kaeri.tv.ui.TonalButton
import app.kaeri.tv.ui.TvInfoRow
import app.kaeri.tv.ui.TvListRow
import app.kaeri.tv.ui.focusSoon

/** Where the QR page's address is reached on this network: this TV's own address, or none. */
val LocalPageHost = staticCompositionLocalOf<(Context) -> String?> { ::localAddress }

/**
 * The proxy's form. With a proxy saved it is Edit proxy: the details filled in but for the
 * password, changed from the page or typed. Save checks them first and only a passed check saves
 * them; a failed one says why, and the saved proxy stays.
 */
@Composable
fun TorrentScreen(onDone: () -> Unit) {
    val context = LocalContext.current
    val store = LocalAppStore.current
    val notices = LocalNotices.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val saved = view?.torrent?.proxy
    var host by remember { mutableStateOf(saved?.host.orEmpty()) }
    var port by remember { mutableStateOf(saved?.port?.toString().orEmpty()) }
    var user by remember { mutableStateOf(saved?.user.orEmpty()) }
    // Never kept with the screen's state: it goes to the keystore-sealed store, and nowhere else.
    var password by remember { mutableStateOf("") }
    val form = rememberForm()
    val whole = TorrentWords.proxyOk(host, port)
    val details = proxyOf(host, port, user, password)
    val done by rememberUpdatedState(onDone)
    val check = proxyCheck(details) { checked ->
        store.setTorrentProxy(checked)
        password = ""
        notices?.say(context.getString(TorrentWords.SAID_PROXY))
        done()
    }
    BackHandler { if (form.typing) form.stopTyping() else done() }
    // A form with nothing to save yet starts on Type it, which the page's Use a phone leaves room for.
    LaunchedEffect(Unit) { if (whole) form.first.focusSoon() else form.type.focusSoon() }

    Column(
        Modifier
            .fillMaxSize()
            .background(ObiSilk.Ground)
            .padding(start = Canvas.Keyline, end = Canvas.SafeX, top = 72.dp),
        verticalArrangement = Arrangement.spacedBy(20.dp),
    ) {
        Text(stringResource(if (saved != null) R.string.torrents_sheet_edit_proxy_title else R.string.torrents_proxy_route), style = MaterialTheme.typography.titleLarge, color = ObiSilk.Silk, modifier = Modifier.semantics { heading() })
        Text(stringResource(if (saved != null) TorrentWords.EDIT_NOTE else TorrentWords.WHY), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk, modifier = Modifier.widthIn(max = 640.dp))
        if (form.typing) {
            TypeDetails(
                proxy = true,
                host = host,
                port = port,
                user = user,
                password = password,
                onHost = { host = it },
                onPort = { port = it.filter(Char::isDigit).take(5) },
                onUser = { user = it },
                onPassword = { password = it },
                onDone = form::stopTyping,
            )
        } else {
            Column(Modifier.widthIn(max = 880.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
                if (form.asked > 0 || !whole) {
                    DetailsPage(PageForm.Proxy, opened = form.asked, filled = if (saved != null) mapOf("host" to host, "port" to port, "user" to user) else emptyMap()) { answer ->
                        host = answer.host
                        port = answer.port
                        user = answer.user
                        password = answer.password
                        form.answered()
                    }
                } else {
                    TvInfoRow(stringResource(R.string.torrents_proxy_route), supporting = proxyLine(context, host, port, user))
                }
                Status(
                    STATUS,
                    checking = check.checking,
                    failed = if (check.failed) TorrentWords.proxyFailed(context) else null,
                    otherwise = stringResource(if (saved != null) TorrentWords.PASSWORD_KEPT else TorrentWords.CHECK_NOTE),
                )
                FormButtons(
                    form,
                    primary = stringResource(R.string.action_save),
                    whole = whole,
                    checking = check.checking,
                    onPrimary = { if (!check.checking && details != null) store.checkTorrentProxy(details) },
                    onCancel = done,
                    primaryFirst = true,
                )
            }
        }
    }
}

/**
 * A torrents form's moving parts: the page asked for again ([asked]), the details being typed,
 * and where focus goes as they come and go. [first] is the screen's first focus; [phone] and
 * [type] are the buttons that open the page and the typing.
 */
private class FormState {
    var asked by mutableIntStateOf(0)
        private set
    var typing by mutableStateOf(false)
        private set

    /** Bumped as the typing closes, so Type it takes focus back. */
    var typed by mutableIntStateOf(0)
        private set
    val first = FocusRequester()
    val type = FocusRequester()

    fun askPhone() {
        asked += 1
    }

    fun answered() {
        asked = 0
    }

    fun startTyping() {
        typing = true
    }

    fun stopTyping() {
        typing = false
        typed += 1
    }
}

@Composable
private fun rememberForm(): FormState = remember { FormState() }

/**
 * The form's buttons, all one height: [primary] in amber, "Use a phone" while the details are
 * whole and no page is open, "Type it" and "Cancel". While a check runs the others are locked,
 * and the amber one keeps focus. Focus comes back to Type it as the typing or a new page takes the
 * place of the button pressed. [primaryFirst] focuses the amber one as the screen opens.
 */
@Composable
private fun FormButtons(
    form: FormState,
    primary: String,
    whole: Boolean,
    checking: Boolean,
    onPrimary: () -> Unit,
    onCancel: () -> Unit,
    primaryFirst: Boolean = false,
) {
    // The page replaces Use a phone, and Type it moves into its place: focus goes with it.
    LaunchedEffect(form.asked) { if (form.asked > 0) form.type.focusSoon() }
    LaunchedEffect(form.typed) { if (form.typed > 0) form.type.focusSoon() }
    Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        AmberButton(onClick = onPrimary, icon = null, enabled = whole, modifier = if (primaryFirst) Modifier.focusRequester(form.first) else Modifier) {
            ButtonLabel(primary)
        }
        if (whole && form.asked == 0) {
            TonalButton(onClick = form::askPhone, enabled = !checking, height = BUTTON_HEIGHT) { ButtonLabel(stringResource(R.string.tv_use_phone)) }
        }
        TonalButton(onClick = form::startTyping, enabled = !checking, height = BUTTON_HEIGHT, modifier = Modifier.focusRequester(form.type)) { ButtonLabel(stringResource(R.string.tv_type_it)) }
        TonalButton(onClick = onCancel, height = BUTTON_HEIGHT) { ButtonLabel(stringResource(R.string.action_cancel)) }
    }
}

/** The amber action's height, which the row's tonal buttons take too. */
private val BUTTON_HEIGHT = 56.dp

/** What a proxy's check before saving says on screen: under way, or why it failed. */
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

/** "proxy.example:1080 · as mika": where the proxy is, and who it lets in. */
internal fun proxyLine(context: Context, host: String, port: String, user: String): String = TorrentWords.proxyLine(context, host, port, user)

/**
 * One place under the details for what the check says, [height] high whatever is in it so the
 * buttons never move: Checking the proxy while it runs, why it failed, or [otherwise]. TalkBack
 * says each change as it comes.
 */
@Composable
private fun Status(height: Dp, checking: Boolean, failed: String?, otherwise: String?) {
    Box(
        Modifier
            .fillMaxWidth()
            .height(height)
            .semantics(mergeDescendants = true) { liveRegion = LiveRegionMode.Polite },
        contentAlignment = Alignment.CenterStart,
    ) {
        when {
            checking -> CheckingLine()
            failed != null -> Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Box(Modifier.size(8.dp).background(ObiSilk.Vermilion, CircleShape))
                Text(failed, style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk, maxLines = 2, overflow = TextOverflow.Ellipsis)
            }
            otherwise != null -> Text(otherwise, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, maxLines = 3, overflow = TextOverflow.Ellipsis)
        }
    }
}

/** The status line's height: the note's two lines, or what the check says in its place. */
private val STATUS = 64.dp

/** Checking the proxy, its spinner after a moment so a quick answer never flashes it. */
@Composable
private fun CheckingLine() {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        var spin by remember { mutableStateOf(false) }
        LaunchedEffect(Unit) {
            delay(SPINNER_WAIT)
            spin = true
        }
        Box(Modifier.size(32.dp)) { if (spin) Spinner() }
        Text(stringResource(TorrentWords.CHECKING_PROXY), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze)
    }
}

/** How long Checking the proxy waits before its spinner shows, as Checking the store does. */
private const val SPINNER_WAIT = 400L

/**
 * The TV's page for the details, behind a QR code: it takes one answer and closes, or closes
 * after its 10 minutes, "Code expired" then in the code's place. [opened] counts the pages asked
 * for, and a new page opens with each.
 */
@Composable
private fun DetailsPage(form: PageForm, opened: Int, filled: Map<String, String> = emptyMap(), onAnswer: (PageAnswer.Torrent) -> Unit) {
    val context = LocalContext.current
    val host = LocalPageHost.current
    val scope = rememberCoroutineScope()
    val answer by rememberUpdatedState(onAnswer)
    var address by remember { mutableStateOf<String?>(null) }
    var started by remember { mutableStateOf(false) }
    var expired by remember { mutableStateOf(false) }
    var again by remember { mutableIntStateOf(0) }
    DisposableEffect(form, opened, again) {
        expired = false
        started = false
        address = null
        val page = SignInPage(context, tracker = "torrents", form = form, authorizeUrl = null, filled = filled) { sent ->
            if (sent is PageAnswer.Torrent) scope.launch { answer(sent) }
        }
        val job = scope.launch {
            val port = withContext(Dispatchers.IO) { page.start() }
            val local = host(context)
            address = if (port != null && local != null) page.address(local, port) else null
            started = true
            if (port != null) {
                delay(SignInPage.LIFETIME_MILLIS)
                expired = true
                page.stop()
            }
        }
        onDispose {
            job.cancel()
            page.stop()
        }
    }
    Row(horizontalArrangement = Arrangement.spacedBy(20.dp), verticalAlignment = Alignment.CenterVertically) {
        val shown = address.takeIf { !expired }
        if (shown != null) {
            QrCode(shown, size = QR, label = stringResource(R.string.tv_qr_label_proxy))
        } else {
            // The code's place kept while the page starts, so nothing moves when it comes.
            Box(Modifier.size(QR).background(ObiSilk.Raised, RoundedCornerShape(12.dp)), contentAlignment = Alignment.Center) {
                if (expired) Text(stringResource(R.string.stores_code_expired), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Silk)
            }
        }
        Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Text(
                stringResource(R.string.tv_scan_proxy),
                style = MaterialTheme.typography.bodyMedium,
                color = ObiSilk.Silk,
            )
            Text(
                when {
                    expired -> stringResource(R.string.stores_page_closed)
                    shown != null -> shown
                    started -> stringResource(R.string.stores_tv_no_network)
                    else -> stringResource(R.string.downloads_notice_starting)
                },
                style = MaterialTheme.typography.bodySmall,
                color = ObiSilk.Haze,
                // The whole address, its key included, so it can be typed as well as scanned.
                maxLines = 4,
                overflow = TextOverflow.Ellipsis,
            )
            if (!expired) Text(stringResource(R.string.stores_page_closes), style = MaterialTheme.typography.bodySmall, color = ObiSilk.Haze)
            if (expired) {
                TonalButton(onClick = { again += 1 }) { ButtonLabel(stringResource(R.string.action_new_code)) }
            }
        }
    }
}

/** The proxy's details typed with the system keyboard. */
@Composable
private fun TypeDetails(
    proxy: Boolean,
    host: String,
    port: String,
    user: String,
    password: String,
    onHost: (String) -> Unit,
    onPort: (String) -> Unit,
    onUser: (String) -> Unit,
    onPassword: (String) -> Unit,
    onDone: () -> Unit,
) {
    val first = remember { FocusRequester() }
    val keyboard = LocalSoftwareKeyboardController.current
    LaunchedEffect(Unit) {
        first.focusSoon()
        keyboard?.show()
    }
    Column(Modifier.widthIn(max = 640.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        if (proxy) {
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Field(host, onHost, stringResource(R.string.torrents_proxy_host), KeyboardType.Uri, Modifier.weight(2f), focus = first)
                Field(port, onPort, stringResource(R.string.torrents_proxy_port), KeyboardType.Number, Modifier.weight(1f))
            }
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Field(user, onUser, stringResource(R.string.tv_field_user_optional), KeyboardType.Text, Modifier.weight(1f))
                Field(password, onPassword, stringResource(R.string.tv_field_password_optional), KeyboardType.Password, Modifier.weight(1f), secret = true)
            }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            TonalButton(onClick = onDone) { ButtonLabel(stringResource(R.string.action_done)) }
        }
    }
}

/**
 * One typed field, Search's pill: the raised ground and its name in haze until something is
 * typed, silk with ground ink while focused. TalkBack says its name whatever it holds.
 */
@Composable
private fun Field(
    text: String,
    onChange: (String) -> Unit,
    name: String,
    type: KeyboardType,
    modifier: Modifier = Modifier,
    secret: Boolean = false,
    focus: FocusRequester? = null,
) {
    var focused by remember { mutableStateOf(false) }
    val ink = if (focused) ObiSilk.Ground else ObiSilk.Silk
    Box(
        modifier.fillMaxWidth().height(56.dp).background(if (focused) ObiSilk.Silk else ObiSilk.Raised, CircleShape).padding(horizontal = 24.dp),
        contentAlignment = Alignment.CenterStart,
    ) {
        if (text.isEmpty()) {
            // Drawn only while empty, so it is not what names the field: the field's own semantics do.
            Text(name, style = MaterialTheme.typography.bodyLarge, color = if (focused) ObiSilk.Ground else ObiSilk.Haze, maxLines = 1, modifier = Modifier.clearAndSetSemantics {})
        }
        BasicTextField(
            value = text,
            onValueChange = onChange,
            singleLine = true,
            textStyle = MaterialTheme.typography.bodyLarge.copy(color = ink),
            cursorBrush = SolidColor(if (focused) ObiSilk.Ground else ObiSilk.Amber),
            visualTransformation = if (secret) PasswordVisualTransformation() else VisualTransformation.None,
            keyboardOptions = KeyboardOptions(keyboardType = type, imeAction = ImeAction.Next, autoCorrectEnabled = false),
            modifier = Modifier
                .fillMaxWidth()
                .semantics { contentDescription = name }
                .let { if (focus != null) it.focusRequester(focus) else it }
                .onFocusChanged { focused = it.isFocused },
        )
    }
}

/** The QR code's side: smaller than sign-in's, as the protection rows sit beside it. */
private val QR = 140.dp
