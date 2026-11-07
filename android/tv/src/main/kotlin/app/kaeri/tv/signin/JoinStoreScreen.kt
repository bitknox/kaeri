// TVJoinStore: adding a plugin store on the TV itself, no paired phone needed. The TV's own page
// takes an invite link pasted on any phone, from a QR code; "Type the link" opens the system
// keyboard as the last resort. Either way the TV shows the store before joining, with the amber
// Join. Once joined it says what came in: the plugins for watching, each with Install, and how
// many more are for reading, which a TV doesn't do; or why nothing did. Done leaves, with a notice
// when something was installed.

package app.kaeri.tv.signin

import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import app.kaeri.i18n.Strings
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
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
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
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import app.kaeri.design.ObiSilk
import app.kaeri.engine.model.InvitedStore
import app.kaeri.store.InviteLinks
import app.kaeri.store.Adding
import app.kaeri.store.CameInModel
import app.kaeri.store.CameInSteps
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.animation.togetherWith
import app.kaeri.store.StoresModel
import app.kaeri.store.runByLine
import app.kaeri.tv.remote.QrCode
import app.kaeri.tv.ui.AmberButton
import app.kaeri.tv.ui.ButtonLabel
import app.kaeri.tv.ui.Canvas
import app.kaeri.tv.ui.LocalAppStore
import app.kaeri.tv.ui.LocalNotices
import app.kaeri.tv.player.Spinner
import app.kaeri.tv.ui.TonalButton
import app.kaeri.tv.ui.entryFocus
import app.kaeri.tv.ui.focusSoon
import app.kaeri.store.plainReason

/** The line under every Join. */

/**
 * TVJoinStore, until a store is joined ([onDone] with true) or Back ([onDone] with false). On
 * first launch [cancelLabel] is "Skip for now".
 */
@Composable
fun JoinStoreScreen(onDone: (joined: Boolean) -> Unit, cancelLabel: String? = null, title: String? = null, line: String? = null) {
    val context = LocalContext.current
    val cancelLabel = cancelLabel ?: stringResource(R.string.action_cancel)
    val title = title ?: stringResource(R.string.stores_add_title)
    val store = LocalAppStore.current
    val notices = LocalNotices.current
    val preview by store.stores.preview.collectAsStateWithLifecycle()
    val joined by store.stores.joined.collectAsStateWithLifecycle()
    val stores by store.stores.stores.collectAsStateWithLifecycle()
    val done by rememberUpdatedState(onDone)
    val cameIn = store.cameIn
    val outcome by cameIn.outcome.collectAsStateWithLifecycle()
    val looking by cameIn.looking.collectAsStateWithLifecycle()
    // The join's answer comes to the store form, which this page is.
    LaunchedEffect(Unit) {
        store.stores.clearJoined()
        if (cameIn.open.value != Adding.STORE) cameIn.open(Adding.STORE)
    }
    LaunchedEffect(joined) {
        val id = joined ?: return@LaunchedEffect
        store.stores.clearJoined()
        // What came in follows the join and is said here; only a page that is not the form leaves at once.
        if (cameIn.open.value == Adding.STORE) return@LaunchedEffect
        val name = stores.firstOrNull { it.id == id }?.name
        notices?.say(if (name != null) Strings.storesTvJoined(context, name) else Strings.storesTvJoinedPlain(context))
        done(true)
    }
    DisposableEffect(Unit) {
        onDispose {
            store.stores.clearPreview()
            if (cameIn.open.value == Adding.STORE) cameIn.closeQuietly()
        }
    }
    val step = CameInSteps.store(outcome)
    if (step != CameInSteps.StoreStep.Preview) {
        val installedHere by cameIn.installedHere.collectAsStateWithLifecycle()
        JoinedResult(
            step,
            title = title,
            onDone = {
                CameInModel.closingNotice(outcome, installedHere.size)?.let { notices?.say(it.words(context)) }
                cameIn.closeQuietly()
                done(true)
            },
            onAgain = {
                cameIn.joining()
                store.joinStore(cameIn.address.value)
            },
            onCancel = { done(false) },
            cancelLabel = cancelLabel,
        )
        return
    }
    var typing by remember { mutableStateOf(false) }
    var link by remember { mutableStateOf("") }
    BackHandler {
        when {
            preview != null -> store.stores.clearPreview()
            typing -> typing = false
            else -> done(false)
        }
    }
    val check: (String) -> Unit = { text ->
        val invite = InviteLinks.invite(text) ?: text.trim()
        link = invite
        typing = false
        store.previewStore(invite)
    }

    Column(
        Modifier
            .fillMaxSize()
            .background(ObiSilk.Ground)
            .padding(start = Canvas.Keyline, end = Canvas.SafeX, top = 72.dp),
        verticalArrangement = Arrangement.spacedBy(20.dp),
    ) {
        Text(title, style = MaterialTheme.typography.titleLarge, color = ObiSilk.Silk, modifier = Modifier.semantics { heading() })
        if (line != null) Text(line, style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk, modifier = Modifier.widthIn(max = 640.dp))
        when (val shown = preview) {
            null -> if (typing) {
                TypeLink(link, onCheck = check, onCancel = { typing = false })
            } else {
                Ways(onAnswer = check, onType = { typing = true }, onCancel = { done(false) }, cancelLabel = cancelLabel)
            }
            is StoresModel.Preview.Checking -> Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                // The spinner comes after a moment, so a quick answer never flashes it.
                var spin by remember { mutableStateOf(false) }
                LaunchedEffect(Unit) {
                    delay(SPINNER_WAIT)
                    spin = true
                }
                Box(Modifier.size(32.dp)) { if (spin) Spinner() }
                Text(stringResource(R.string.stores_checking_store), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze)
            }
            is StoresModel.Preview.Failed -> Failed(shown.reason, shown.contact, onAgain = { store.stores.clearPreview() }, onCancel = { done(false) }, cancelLabel = cancelLabel)
            is StoresModel.Preview.Store -> Preview(
                shown.store,
                onJoin = {
                    cameIn.typeAddress(link)
                    cameIn.joining()
                    store.joinStore(link)
                },
                onCancel = { store.stores.clearPreview() },
            )
        }
    }
}

/**
 * The ways to hand the TV a link: the TV's page from a QR code, or typing it. The page closes
 * after its 10 minutes; the QR code goes with it, "Code expired" in its place, and Get a new code
 * opens another page.
 */
@Composable
private fun Ways(onAnswer: (String) -> Unit, onType: () -> Unit, onCancel: () -> Unit, cancelLabel: String) {
    val context = LocalContext.current
    val pageHost = LocalPageHost.current
    val scope = rememberCoroutineScope()
    val answer by rememberUpdatedState(onAnswer)
    var address by remember { mutableStateOf<String?>(null) }
    var started by remember { mutableStateOf(false) }
    // Each page in turn: a new one once the last has expired and Get a new code is pressed.
    var opened by remember { mutableIntStateOf(0) }
    var expired by remember { mutableStateOf(false) }
    DisposableEffect(opened) {
        expired = false
        val page = SignInPage(context, tracker = Strings.storesFallbackTitle(context), form = PageForm.Link, authorizeUrl = null) { pasted ->
            if (pasted is PageAnswer.Link) scope.launch { answer(pasted.text) }
        }
        val job = scope.launch {
            val port = withContext(Dispatchers.IO) { page.start() }
            val local = pageHost(context)
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
    val type = remember { FocusRequester() }
    val again = remember { FocusRequester() }
    LaunchedEffect(expired) { if (expired) again.focusSoon() }
    Text(
        stringResource(R.string.stores_qr_instructions),
        style = MaterialTheme.typography.bodyLarge,
        color = ObiSilk.Silk,
        modifier = Modifier.widthIn(max = 640.dp),
    )
    Row(horizontalArrangement = Arrangement.spacedBy(40.dp), verticalAlignment = Alignment.CenterVertically) {
        val shown = address.takeIf { !expired }
        if (shown != null) {
            QrCode(shown, label = stringResource(R.string.tv_qr_label_link))
        } else {
            Box(Modifier.size(180.dp).background(ObiSilk.Raised, RoundedCornerShape(12.dp)), contentAlignment = Alignment.Center) {
                if (expired) Text(stringResource(R.string.stores_code_expired), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk)
            }
        }
        Column(verticalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.widthIn(max = 520.dp)) {
            Text(stringResource(R.string.stores_or_open_address), style = MaterialTheme.typography.labelLarge, color = ObiSilk.Haze)
            Text(
                when {
                    expired -> stringResource(R.string.stores_page_closed)
                    shown != null -> shown
                    started -> stringResource(R.string.stores_tv_no_network)
                    else -> stringResource(R.string.downloads_notice_starting)
                },
                style = MaterialTheme.typography.bodyMedium,
                color = ObiSilk.Silk,
            )
            Text(stringResource(R.string.stores_page_closes), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
        }
    }
    Row(Modifier.entryFocus(type), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        if (expired) {
            TonalButton(onClick = { opened += 1 }, modifier = Modifier.focusRequester(again)) { ButtonLabel(stringResource(R.string.action_new_code)) }
        }
        TonalButton(onClick = onType, modifier = Modifier.focusRequester(type)) { ButtonLabel(stringResource(R.string.stores_type_link)) }
        TonalButton(onClick = onCancel) { ButtonLabel(cancelLabel) }
    }
}

/** The link typed with the system keyboard, then checked. */
@Composable
private fun TypeLink(start: String, onCheck: (String) -> Unit, onCancel: () -> Unit) {
    var text by remember { mutableStateOf(start) }
    val field = remember { FocusRequester() }
    val keyboard = LocalSoftwareKeyboardController.current
    LaunchedEffect(Unit) {
        field.focusSoon()
        keyboard?.show()
    }
    Box(
        Modifier.widthIn(max = 640.dp).fillMaxWidth().height(56.dp).background(ObiSilk.Raised, CircleShape).padding(horizontal = 24.dp),
        contentAlignment = Alignment.CenterStart,
    ) {
        if (text.isEmpty()) Text(stringResource(R.string.stores_invite_link), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze)
        BasicTextField(
            value = text,
            onValueChange = { text = it },
            singleLine = true,
            textStyle = MaterialTheme.typography.bodyLarge.copy(color = ObiSilk.Silk),
            cursorBrush = SolidColor(ObiSilk.Amber),
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Go, autoCorrectEnabled = false),
            keyboardActions = KeyboardActions(onGo = { if (text.isNotBlank()) onCheck(text) }),
            modifier = Modifier.fillMaxWidth().focusRequester(field),
        )
    }
    Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        TonalButton(onClick = { onCheck(text) }, enabled = text.isNotBlank()) { ButtonLabel(stringResource(R.string.stores_check_link)) }
        TonalButton(onClick = onCancel) { ButtonLabel(stringResource(R.string.action_back)) }
    }
}

/** The link did not lead to a store: an error's vermilion dot, why in plain words, another try, and Cancel. */
@Composable
private fun Failed(reason: String, contact: String?, onAgain: () -> Unit, onCancel: () -> Unit, cancelLabel: String) {
    val again = remember { FocusRequester() }
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Box(Modifier.size(8.dp).background(ObiSilk.Vermilion, CircleShape))
        Text(stringResource(R.string.stores_link_failed), style = MaterialTheme.typography.headlineSmall, color = ObiSilk.Silk)
    }
    Text(plainReason(LocalContext.current, reason), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk, modifier = Modifier.widthIn(max = 640.dp))
    if (contact != null) Text(stringResource(R.string.store_invite_contact, contact), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze, modifier = Modifier.widthIn(max = 640.dp))
    Row(Modifier.entryFocus(again), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        TonalButton(onClick = onAgain, modifier = Modifier.focusRequester(again)) { ButtonLabel(stringResource(R.string.stores_try_another_link)) }
        TonalButton(onClick = onCancel) { ButtonLabel(cancelLabel) }
    }
}

/**
 * The store before joining: its name, who runs it, its key, its plugins with the hosts each
 * contacts, and whether anyone with the link can join. The amber Join, focused, and Cancel.
 */
@Composable
private fun Preview(invited: InvitedStore, onJoin: () -> Unit, onCancel: () -> Unit) {
    val store = LocalAppStore.current
    val working by store.stores.working.collectAsStateWithLifecycle()
    val joining = store.stores.isWorking(working, StoresModel.Work.Join)
    val join = remember { FocusRequester() }
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Text(invited.name, style = MaterialTheme.typography.headlineSmall, color = ObiSilk.Silk)
        Text(invited.runByLine(LocalContext.current), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
        Text(stringResource(R.string.tv_preview_key, invited.fingerprint.chunked(4).take(4).joinToString(" ")), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
        if (invited.open) Text(stringResource(R.string.stores_anyone_with_link), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
    }
    if (invited.plugins.isNotEmpty()) {
        Column(verticalArrangement = Arrangement.spacedBy(4.dp), modifier = Modifier.widthIn(max = 720.dp)) {
            invited.plugins.take(PREVIEW_PLUGINS).forEach { plugin ->
                Row {
                    Text(plugin.name, style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk, modifier = Modifier.width(200.dp), maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text(plugin.hosts.joinToString(", "), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
            }
            if (invited.plugins.size > PREVIEW_PLUGINS) {
                Text(stringResource(R.string.tv_preview_more, invited.plugins.size - PREVIEW_PLUGINS), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
            }
        }
    }
    // A link that works more than once asks who is joining; the store's owners see the name.
    if (invited.namesJoiner && !invited.alreadyJoined) {
        val name by store.stores.deviceName.collectAsStateWithLifecycle()
        Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Box(
                Modifier.widthIn(max = 640.dp).fillMaxWidth().height(56.dp).background(ObiSilk.Raised, CircleShape).padding(horizontal = 24.dp),
                contentAlignment = Alignment.CenterStart,
            ) {
                if (name.isEmpty()) Text(stringResource(R.string.stores_join_your_name), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze)
                BasicTextField(
                    value = name,
                    onValueChange = store.stores::nameDevice,
                    singleLine = true,
                    textStyle = MaterialTheme.typography.bodyLarge.copy(color = ObiSilk.Silk),
                    cursorBrush = SolidColor(ObiSilk.Amber),
                    keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Words, imeAction = ImeAction.Done),
                    modifier = Modifier.fillMaxWidth(),
                )
            }
            Text(stringResource(R.string.stores_join_your_name_detail), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, modifier = Modifier.widthIn(max = 640.dp))
        }
    }
    Column(Modifier.entryFocus(join)) {
        Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            if (invited.alreadyJoined) {
                TonalButton(onClick = onCancel, modifier = Modifier.focusRequester(join)) { ButtonLabel(stringResource(R.string.tv_already_joined)) }
            } else {
                AmberButton(onClick = onJoin, icon = null, enabled = !joining, modifier = Modifier.focusRequester(join)) { ButtonLabel(stringResource(if (joining) R.string.tv_joining else R.string.action_join)) }
                TonalButton(onClick = onCancel) { ButtonLabel(stringResource(R.string.action_cancel)) }
            }
        }
    }
}

/** A notice a store form left, in words. */
internal fun CameInModel.Notice.words(context: Context): String = when (this) {
    is CameInModel.Notice.Added -> Strings.sourceAdded(context, name, series)
    is CameInModel.Notice.Plugins -> Strings.sourceAddedPlugins(context, count, store)
}

/** Says what a store form left once it has closed, where the TV's sources show. */
@Composable
internal fun CameInTvNotices() {
    val store = LocalAppStore.current
    val notices = LocalNotices.current
    val context = LocalContext.current
    val notice by store.cameIn.notice.collectAsStateWithLifecycle()
    val open by store.cameIn.open.collectAsStateWithLifecycle()
    LaunchedEffect(notice, open) {
        val said = notice ?: return@LaunchedEffect
        if (open != null) return@LaunchedEffect
        notices?.say(said.words(context))
        store.cameIn.noticed()
    }
}

/**
 * What joining brought, on the TV: the title and what the store offers on the left; on the right
 * the result, the plugins for watching with Install, how many more are for reading, and Done. A
 * store that did not answer has Try again and [cancelLabel] instead. Every change crossfades.
 */
@Composable
private fun JoinedResult(
    step: CameInSteps.StoreStep,
    title: String,
    onDone: () -> Unit,
    onAgain: () -> Unit,
    onCancel: () -> Unit,
    cancelLabel: String,
) {
    val context = LocalContext.current
    val store = LocalAppStore.current
    val stores by store.stores.stores.collectAsStateWithLifecycle()
    val working by store.stores.working.collectAsStateWithLifecycle()
    val report by store.cameIn.report.collectAsStateWithLifecycle()
    val installedHere by store.cameIn.installedHere.collectAsStateWithLifecycle()
    val joined = when (step) {
        is CameInSteps.StoreStep.Joined -> step.report
        is CameInSteps.StoreStep.OtherMedium -> step.report
        is CameInSteps.StoreStep.Empty -> step.report
        is CameInSteps.StoreStep.Installed -> report
        else -> null
    }
    val row = joined?.let { r -> stores.firstOrNull { it.id == r.store } }
    val doneFocus = remember { FocusRequester() }
    val failed = step is CameInSteps.StoreStep.Unreachable
    BackHandler { if (failed) onCancel() else onDone() }
    Row(
        Modifier
            .fillMaxSize()
            .background(ObiSilk.Ground)
            .padding(start = Canvas.Keyline, end = Canvas.SafeX, top = 56.dp),
        horizontalArrangement = Arrangement.spacedBy(40.dp),
    ) {
        Column(Modifier.width(300.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Text(
                joined?.let { Strings.storeJoinedTitle(context, it.name) } ?: title,
                style = MaterialTheme.typography.headlineMedium,
                color = ObiSilk.Silk,
                modifier = Modifier.padding(top = 20.dp).semantics { heading() },
            )
            if (step is CameInSteps.StoreStep.Joined) {
                val offered = step.report.forReading.size + step.report.forWatching.size + step.report.otherMedium.toInt()
                Text(Strings.storeJoinedLine(context, step.report.owner ?: step.report.name, offered), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
            }
        }
        androidx.compose.animation.AnimatedContent(
            step,
            transitionSpec = {
                androidx.compose.animation.fadeIn(androidx.compose.animation.core.tween(300)) togetherWith
                    androidx.compose.animation.fadeOut(androidx.compose.animation.core.tween(300)) using
                    androidx.compose.animation.SizeTransform(clip = false)
            },
            label = "joined", // i18n-skip
            modifier = Modifier.widthIn(max = 560.dp),
        ) { shown ->
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                when (shown) {
                    is CameInSteps.StoreStep.Installed -> TvNote(TvNoteKind.FOUND, Strings.storeInstalled(context, shown.installed.installed.toInt()))
                    is CameInSteps.StoreStep.OtherMedium -> TvNote(TvNoteKind.INFO, Strings.storeTvAllReading(context, shown.report.name))
                    is CameInSteps.StoreStep.Empty -> TvNote(TvNoteKind.INFO, Strings.storeEmpty(context, shown.report.name), Strings.storeEmptyLine(context, shown.report.owner ?: shown.report.name))
                    is CameInSteps.StoreStep.Unreachable -> TvNote(TvNoteKind.FAILED, Strings.storeErrUnreachable(context, shown.host), Strings.storeErrUnreachableLine(context, shown.host))
                    else -> {}
                }
                val list = joined?.takeIf { shown is CameInSteps.StoreStep.Joined || shown is CameInSteps.StoreStep.Installed }
                if (list != null) {
                    val plugins = row?.plugins.orEmpty().associateBy { it.id }
                    val shownPlugins = list.forWatching.mapNotNull { plugins[it] }
                    if (shownPlugins.isNotEmpty()) {
                        Text(stringResource(R.string.store_group_watching), style = MaterialTheme.typography.labelLarge, color = ObiSilk.Haze, modifier = Modifier.padding(start = 4.dp))
                        Column(Modifier.background(ObiSilk.Raised, RoundedCornerShape(12.dp))) {
                            shownPlugins.forEach { plugin ->
                                val installed = plugin.id in installedHere || plugin.status == app.kaeri.engine.model.StorePluginStatus.Installed ||
                                    shown is CameInSteps.StoreStep.Installed
                                val busy = store.stores.isWorking(working, StoresModel.Work.Install(list.store, plugin.id))
                                TvPluginRow(plugin, installed = installed, busy = busy, onInstall = {
                                    store.installPlugin(list.store, plugin.id)
                                    store.cameIn.installedOne(plugin.id)
                                }, then = doneFocus)
                            }
                        }
                    }
                    if (list.otherIsReading && list.otherMedium > 0u) {
                        Text(Strings.storeTvReadingOnly(context, list.otherMedium.toInt()), style = MaterialTheme.typography.bodySmall, color = ObiSilk.Haze, modifier = Modifier.padding(start = 4.dp))
                    }
                }
                Row(Modifier.padding(top = 8.dp).entryFocus(doneFocus), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    if (failed) {
                        TonalButton(onClick = onAgain, modifier = Modifier.focusRequester(doneFocus)) { ButtonLabel(stringResource(R.string.action_try_again)) }
                        TonalButton(onClick = onCancel) { ButtonLabel(cancelLabel) }
                    } else {
                        TonalButton(onClick = onDone, modifier = Modifier.focusRequester(doneFocus)) { ButtonLabel(stringResource(R.string.action_done)) }
                    }
                }
            }
        }
    }
}

private enum class TvNoteKind { FOUND, INFO, FAILED }

/** A result line: a silk check, the haze info glyph or the vermilion error one, a title and its line. */
@Composable
private fun TvNote(kind: TvNoteKind, title: String, line: String? = null) {
    Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        Box(Modifier.padding(top = 2.dp).size(18.dp), contentAlignment = Alignment.Center) {
            when (kind) {
                TvNoteKind.FOUND -> androidx.tv.material3.Icon(app.kaeri.tv.ui.TvIcons.Check, contentDescription = null, tint = ObiSilk.Silk, modifier = Modifier.size(18.dp))
                TvNoteKind.INFO -> androidx.tv.material3.Icon(app.kaeri.tv.ui.TvIcons.Info, contentDescription = null, tint = ObiSilk.Haze, modifier = Modifier.size(18.dp))
                TvNoteKind.FAILED -> androidx.compose.foundation.Canvas(Modifier.size(16.dp)) {
                    val path = androidx.compose.ui.graphics.Path().apply {
                        moveTo(size.width / 2, 0f)
                        lineTo(size.width, size.height)
                        lineTo(0f, size.height)
                        close()
                    }
                    drawPath(path, ObiSilk.Vermilion)
                    drawLine(ObiSilk.Ground, androidx.compose.ui.geometry.Offset(size.width / 2, size.height * 0.38f), androidx.compose.ui.geometry.Offset(size.width / 2, size.height * 0.66f), strokeWidth = size.width * 0.11f)
                    drawCircle(ObiSilk.Ground, radius = size.width * 0.06f, center = androidx.compose.ui.geometry.Offset(size.width / 2, size.height * 0.82f))
                }
            }
        }
        Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(title, style = MaterialTheme.typography.titleSmall, color = ObiSilk.Silk)
            line?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = ObiSilk.Haze) }
        }
    }
}

/**
 * A plugin to install: its tile, name, version and hosts, and Install. The row fills silk while
 * Install has focus; Install shows its spinner after 600 ms, and once installed focus moves on to
 * [then] over 150 ms.
 */
@Composable
private fun TvPluginRow(plugin: app.kaeri.engine.model.StorePluginRow, installed: Boolean, busy: Boolean, onInstall: () -> Unit, then: FocusRequester) {
    val context = LocalContext.current
    var focused by remember { mutableStateOf(false) }
    val fill by androidx.compose.animation.animateColorAsState(if (focused) ObiSilk.Silk else ObiSilk.Raised, androidx.compose.animation.core.tween(150), label = "row") // i18n-skip
    val ink = if (focused) ObiSilk.Ground else ObiSilk.Silk
    val quiet = if (focused) ObiSilk.Ground.copy(alpha = 0.7f) else ObiSilk.Haze
    var spin by remember { mutableStateOf(false) }
    LaunchedEffect(busy) {
        spin = false
        if (busy) {
            delay(INSTALL_SPINNER_WAIT)
            spin = true
        }
    }
    LaunchedEffect(installed) {
        if (installed && focused) {
            focused = false
            then.focusSoon()
        }
    }
    Row(
        Modifier.widthIn(min = 440.dp).background(fill, RoundedCornerShape(12.dp)).padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        Box(Modifier.size(32.dp).background(ObiSilk.RaisedSelected, RoundedCornerShape(8.dp)), contentAlignment = Alignment.Center) {
            androidx.tv.material3.Icon(app.kaeri.tv.ui.TvIcons.Source, contentDescription = null, tint = ObiSilk.Silk, modifier = Modifier.size(18.dp))
        }
        Column(Modifier.weight(1f)) {
            Text(plugin.name, style = MaterialTheme.typography.bodyLarge, color = ink, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(Strings.storePluginRow(context, plugin.version, Strings.storePluginHosts(context, plugin.hosts.size)), style = MaterialTheme.typography.bodySmall, color = quiet)
        }
        if (installed) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(stringResource(R.string.store_installed_row), style = MaterialTheme.typography.bodyMedium, color = ink)
                androidx.tv.material3.Icon(app.kaeri.tv.ui.TvIcons.Check, contentDescription = null, tint = ink, modifier = Modifier.size(18.dp))
            }
        } else {
            androidx.tv.material3.Button(
                onClick = onInstall,
                enabled = !busy,
                modifier = Modifier.onFocusChanged { focused = it.isFocused },
                shape = androidx.tv.material3.ButtonDefaults.shape(CircleShape),
                colors = androidx.tv.material3.ButtonDefaults.colors(
                    containerColor = ObiSilk.RaisedSelected,
                    contentColor = ObiSilk.Silk,
                    focusedContainerColor = ObiSilk.Ground,
                    focusedContentColor = ObiSilk.Silk,
                ),
            ) {
                if (spin) Box(Modifier.size(20.dp)) { Spinner() } else Text(stringResource(R.string.store_install))
            }
        }
    }
}

/** How long Install waits before its spinner shows. */
private const val INSTALL_SPINNER_WAIT = 600L

/** How long Checking the store waits before its spinner shows. */
private const val SPINNER_WAIT = 400L

/** A store with many plugins shows the first few; the store's page in Settings has the rest. */
private const val PREVIEW_PLUGINS = 5
