// TVSignIn: signing in to a tracker without a browser. AniList and Kitsu through the TV's own
// sign-in page, opened on a phone from a QR code; Simkl and Trakt with the service's device code.
// MyAnimeList has no way in on a TV: its redirect must be the one address it has registered, so
// Accounts leaves it out.

package app.kaeri.tv.signin

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
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.net.toUri
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import app.kaeri.design.Nagare
import app.kaeri.design.ObiSilk
import app.kaeri.store.AccountsModel
import app.kaeri.engine.model.DeviceCodePrompt
import app.kaeri.store.AppStore
import app.kaeri.store.CodeCountdown
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import app.kaeri.tv.ui.LocalNotices
import app.kaeri.tv.remote.QrCode
import app.kaeri.tv.ui.ButtonLabel
import app.kaeri.tv.ui.Canvas
import app.kaeri.tv.ui.LocalAppStore
import app.kaeri.tv.ui.TonalButton
import app.kaeri.tv.ui.entryFocus
import app.kaeri.tv.ui.focusSoon
import app.kaeri.tv.ui.rememberScreenReader
import app.kaeri.tv.ui.spelled
import app.kaeri.tv.ui.spokenBlock

/** How [tracker] signs in on a TV, or that it does not ([None]). */
enum class SignInWay { Page, DeviceCode, None }

fun signInWay(tracker: String): SignInWay = when (tracker) {
    "anilist", "kitsu" -> SignInWay.Page
    "simkl", "trakt" -> SignInWay.DeviceCode
    else -> SignInWay.None
}

/** A device code read in groups of four, as the brief shows it: "8F3K 2QPL". */
fun spaced(code: String): String = code.chunked(4).joinToString(" ")

/** TVSignIn for [tracker], until it is signed in ([onDone]) or Back. */
@Composable
fun SignInScreen(tracker: String, onDone: () -> Unit, cancelLabel: String? = null) {
    val context = LocalContext.current
    val cancelText = cancelLabel ?: stringResource(R.string.action_cancel)
    val store = LocalAppStore.current
    val notices = LocalNotices.current
    val name = AccountsModel.name(tracker)
    val trackers by store.accounts.trackers.collectAsStateWithLifecycle()
    val done by rememberUpdatedState(onDone)
    val wasSignedIn = remember { trackers[tracker]?.connected == true }
    val signedIn = trackers[tracker]?.connected == true
    LaunchedEffect(signedIn) {
        if (signedIn && !wasSignedIn) {
            val user = trackers[tracker]?.user
            notices?.say(if (user != null) Strings.tvSigninDoneAs(context, name, user) else Strings.tvSigninDone(context, name))
            done()
        }
    }
    BackHandler(onBack = onDone)
    // The engine's word on a failed sign-in for this tracker comes here, for the screen to say in
    // place of a wait that would never end.
    DisposableEffect(tracker) {
        store.accounts.credentialsSheet(tracker)
        onDispose { store.accounts.credentialsSheet(null) }
    }

    val cancel = remember { FocusRequester() }
    Row(
        Modifier
            .entryFocus(cancel)
            .fillMaxSize()
            .background(ObiSilk.Ground)
            .padding(start = Canvas.Keyline, end = Canvas.SafeX, top = 96.dp),
        horizontalArrangement = Arrangement.spacedBy(64.dp),
    ) {
        Box(Modifier.width(160.dp)) { Nagare(cell = 5) }
        Column(verticalArrangement = Arrangement.spacedBy(20.dp)) {
            Text(stringResource(R.string.accounts_sign_in_to, name), style = MaterialTheme.typography.titleLarge, color = ObiSilk.Silk, modifier = Modifier.semantics { heading() })
            when (signInWay(tracker)) {
                SignInWay.Page -> PageWay(store, tracker, name, cancel, onCancel = onDone, cancelLabel = cancelText)
                SignInWay.DeviceCode -> DeviceCodeWay(store, tracker, cancel, onCancel = onDone, cancelLabel = cancelText)
                SignInWay.None -> Text(
                    stringResource(R.string.tv_signin_not_on_tv, name),
                    style = MaterialTheme.typography.bodyLarge,
                    color = ObiSilk.Haze,
                    modifier = Modifier.widthIn(max = 640.dp),
                )
            }
            if (signInWay(tracker) == SignInWay.None) {
                TonalButton(onClick = onDone, modifier = Modifier.focusRequester(cancel)) { ButtonLabel(cancelText) }
            }
        }
    }
}

/**
 * AniList and Kitsu: the TV's own page, its QR code and address, while the screen is open. A wait
 * that can't end says why: no network a phone can reach, the sign-in failing to start, or the
 * page closed after its 10 minutes, with a way to start again.
 */
@Composable
private fun PageWay(store: AppStore, tracker: String, name: String, cancel: FocusRequester, onCancel: () -> Unit, cancelLabel: String) {
    val context = LocalContext.current
    val pageHost = LocalPageHost.current
    val scope = androidx.compose.runtime.rememberCoroutineScope()
    val request by store.accounts.signIn.collectAsStateWithLifecycle()
    val failure by store.accounts.signInFailure.collectAsStateWithLifecycle()
    val anilist = tracker == "anilist"
    // Each start in turn: again once the page has closed or the start failed and Get a new code or
    // Try again is pressed.
    var opened by remember { mutableIntStateOf(0) }
    // AniList's sign-in starts in the engine, which gives the address to open and the state.
    LaunchedEffect(tracker, opened) { if (anilist && (opened == 0 || store.accounts.signIn.value?.tracker != tracker)) store.connect(tracker) }
    val asked = request?.takeIf { it.tracker == tracker }
    val ready = !anilist || asked != null
    var link by remember { mutableStateOf<String?>(null) }
    var started by remember { mutableStateOf(false) }
    var expired by remember { mutableStateOf(false) }
    var sent by remember { mutableStateOf(false) }
    // A sign-in that ran and ended with no account: the answer was turned down. A success closes
    // the screen before this is seen.
    val working by store.accounts.working.collectAsStateWithLifecycle()
    val busy = store.accounts.isWorking(working, tracker)
    var ran by remember { mutableStateOf(false) }
    LaunchedEffect(busy) { if (busy) ran = true }
    val failed = sent && ran && !busy
    // AniList's start failed (offline, say): the engine said why, or it ran and gave no address.
    val startFailed = anilist && asked == null && (failure != null || ran && !busy && !sent)
    DisposableEffect(ready, opened) {
        if (!ready) return@DisposableEffect onDispose {}
        expired = false
        started = false
        link = null
        val page = SignInPage(
            context = context,
            tracker = name,
            form = if (anilist) PageForm.Code else PageForm.Password,
            authorizeUrl = asked?.url,
        ) { answer ->
            scope.launch {
                sent = true
                ran = false
                when (answer) {
                    is PageAnswer.Code -> asked?.let { request ->
                        val state = request.url.toUri().getQueryParameter("state")
                        store.finishSignIn(request, anilistCallback(answer.text, request.callbackScheme, state))
                    }
                    is PageAnswer.Password -> store.connectKitsu(answer.username, answer.password)
                    is PageAnswer.Link, is PageAnswer.Torrent -> Unit
                }
            }
        }
        val job = scope.launch {
            val port = withContext(Dispatchers.IO) { page.start() }
            val address = pageHost(context)
            link = if (port != null && address != null) page.address(address, port) else null
            started = true
            if (port != null) {
                // The page stops itself after its lifetime; the screen says so.
                delay(SignInPage.LIFETIME_MILLIS)
                expired = true
            }
        }
        onDispose {
            job.cancel()
            page.stop()
            // Closed before AniList came back: the engine stops waiting for it.
            if (anilist) store.accounts.signIn.value?.takeIf { it.tracker == tracker }?.let(store::cancelSignIn)
        }
    }
    val steps = if (anilist) {
        stringResource(R.string.accounts_scan_code, name)
    } else {
        stringResource(R.string.accounts_scan_password, name)
    }
    val didNot = failure ?: stringResource(R.string.tv_signin_did_not, name)
    Text(steps, style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk, modifier = Modifier.widthIn(max = 640.dp))
    Row(horizontalArrangement = Arrangement.spacedBy(40.dp), verticalAlignment = Alignment.CenterVertically) {
        val shown = link.takeIf { !expired }
        if (shown != null) {
            QrCode(shown, label = stringResource(R.string.tv_signin_qr_page))
        } else {
            Box(Modifier.size(180.dp).background(ObiSilk.Raised, RoundedCornerShape(12.dp)), contentAlignment = Alignment.Center) {
                if (expired) Text(stringResource(R.string.stores_code_expired), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk)
            }
        }
        Column(verticalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.widthIn(max = 520.dp)) {
            Text(stringResource(R.string.stores_or_open_address), style = MaterialTheme.typography.labelLarge, color = ObiSilk.Haze)
            // A failure leads with the error's dot, its words in silk: vermilion is never text.
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                if (startFailed) Box(Modifier.size(8.dp).background(ObiSilk.Vermilion, CircleShape))
                Text(
                    when {
                        expired -> stringResource(R.string.stores_page_closed)
                        shown != null -> shown
                        startFailed -> didNot
                        started -> stringResource(R.string.stores_tv_no_network)
                        else -> stringResource(R.string.downloads_notice_starting)
                    },
                    style = MaterialTheme.typography.bodyMedium,
                    color = ObiSilk.Silk,
                )
            }
            if (shown != null) Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                if (failed) Box(Modifier.size(8.dp).background(ObiSilk.Vermilion, CircleShape))
                Text(
                    when {
                        !sent -> stringResource(R.string.stores_page_closes)
                        failed -> didNot
                        else -> stringResource(R.string.accounts_signing_in_action)
                    },
                    style = MaterialTheme.typography.bodyMedium,
                    color = if (failed) ObiSilk.Silk else ObiSilk.Haze,
                )
            }
        }
    }
    // Once the page has closed or the start failed, focus rests on starting again.
    val again = remember { FocusRequester() }
    val stuck = expired || startFailed
    LaunchedEffect(stuck) { if (stuck) again.focusSoon() }
    Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        if (stuck) {
            TonalButton(
                onClick = {
                    // A new start begins clean: the last failure leaves the screen.
                    store.accounts.credentialsSheet(tracker)
                    sent = false
                    ran = false
                    opened += 1
                },
                modifier = Modifier.focusRequester(again),
            ) { ButtonLabel(stringResource(if (expired) R.string.action_new_code else R.string.action_try_again)) }
        }
        TonalButton(onClick = onCancel, modifier = Modifier.focusRequester(cancel)) { ButtonLabel(cancelLabel) }
    }
}

/**
 * Simkl and Trakt: the service's code and its address, while the engine polls, with Get a new code
 * and Cancel side by side under it. A code that runs out stays on screen, dimmed, under "Code
 * expired", and focus moves to Get a new code: nothing swaps the code by itself while someone
 * may be typing it.
 */
@Composable
private fun DeviceCodeWay(store: AppStore, tracker: String, again: FocusRequester, onCancel: () -> Unit, cancelLabel: String) {
    val prompt by store.accounts.deviceCode.collectAsStateWithLifecycle()
    val failure by store.accounts.signInFailure.collectAsStateWithLifecycle()
    LaunchedEffect(tracker) { store.connect(tracker) }
    val shown = prompt?.takeIf { it.tracker == tracker }
    var expired by remember(shown?.code) { mutableStateOf(false) }
    LaunchedEffect(expired) { if (expired) runCatching { again.requestFocus() } }
    Text(
        stringResource(R.string.tv_signin_device_steps),
        style = MaterialTheme.typography.bodyLarge,
        color = ObiSilk.Silk,
        modifier = Modifier.widthIn(max = 640.dp),
    )
    Row(horizontalArrangement = Arrangement.spacedBy(40.dp), verticalAlignment = Alignment.CenterVertically) {
        if (shown != null) {
            QrCode(shown.url, label = stringResource(R.string.tv_signin_qr_activation))
        } else {
            Box(Modifier.size(180.dp).background(ObiSilk.Raised, RoundedCornerShape(12.dp)))
        }
        val codeFocus = remember { FocusRequester() }
        val reader by rememberScreenReader()
        // A screen reader starts on the code and the address, and says a new code when one comes.
        LaunchedEffect(reader, shown?.code) { if (reader && shown?.code != null) codeFocus.focusSoon() }
        val spoken = shown?.let { "${spelled(it.code)}. ${it.url}" } // i18n-skip
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
          Column(
            (if (spoken != null) Modifier.spokenBlock(spoken, codeFocus) else Modifier),
            verticalArrangement = Arrangement.spacedBy(8.dp),
          ) {
            val code = shown?.code
            if (code != null) {
                Text(
                    spaced(code),
                    style = MaterialTheme.typography.displaySmall.copy(fontFamily = FontFamily.Monospace, letterSpacing = 4.sp),
                    color = if (expired) ObiSilk.Haze else ObiSilk.Silk,
                )
            } else if (failure != null) {
                // Asking for a code failed: the engine's reason, after the error's dot, in place of a wait.
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                    Box(Modifier.size(8.dp).background(ObiSilk.Vermilion, CircleShape))
                    Text(failure.orEmpty(), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk, modifier = Modifier.widthIn(max = 520.dp))
                }
            } else {
                Text(stringResource(R.string.tv_signin_asking_code), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze)
            }
            shown?.let { Text(it.url, style = MaterialTheme.typography.labelLarge, color = ObiSilk.Haze) }
          }
            shown?.let { CodeExpiry(it, onExpired = { expired = true }) }
        }
    }
    // The code waits, so focus rests on asking for a fresh one.
    Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        TonalButton(
            onClick = {
                // A new code starts clean: the last failure leaves the screen.
                store.accounts.credentialsSheet(tracker)
                store.connect(tracker)
            },
            modifier = Modifier.focusRequester(again),
        ) { ButtonLabel(stringResource(R.string.action_new_code)) }
        TonalButton(onClick = onCancel) { ButtonLabel(cancelLabel) }
    }
}

/**
 * How long the code has left under it, in the engine's words, ticking each second, then "Code
 * expired" in silk beside an error's dot, when [onExpired] tells the screen. Nothing when the
 * service gave no time.
 */
@Composable
internal fun CodeExpiry(prompt: DeviceCodePrompt, onExpired: () -> Unit) {
    val expiresAt = prompt.expiresAt ?: return
    val expired by rememberUpdatedState(onExpired)
    var left by remember(prompt.code, expiresAt) { mutableLongStateOf(CodeCountdown.secondsLeft(expiresAt, System.currentTimeMillis())) }
    LaunchedEffect(prompt.code, expiresAt) {
        CodeCountdown.ticks(expiresAt).collect { left = it }
        expired()
    }
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
        if (left == 0L) Box(Modifier.size(8.dp).background(ObiSilk.Vermilion, CircleShape))
        Text(
            CodeCountdown.line(left),
            style = MaterialTheme.typography.labelLarge.copy(fontFeatureSettings = "tnum"),
            color = if (left == 0L) ObiSilk.Silk else ObiSilk.Haze,
            // The ticking count is not read out each second; "Code expired" is said once.
            modifier = if (left == 0L) Modifier.semantics { liveRegion = LiveRegionMode.Polite } else Modifier,
        )
    }
}
