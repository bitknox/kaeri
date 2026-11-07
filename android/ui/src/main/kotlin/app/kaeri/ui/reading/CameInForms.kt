// Add a server and Add a folder, as forms that say what came in or why nothing did. The server
// form looks at the address as it is typed (at once for a pasted Kavita link, whose key shows only
// as its last four), asks for a sign-in only when the server wants one, and lists the libraries it
// found to pick from; Add server turns amber only once there is a result. The folder form says
// what the scan found, counts the files Kaeri can't open by format, and offers another folder when
// nothing in it can be read. Each is a dialog in Settings and first run's page in first run.

package app.kaeri.ui.reading

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.ButtonDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.engine.model.BookFolderScan
import app.kaeri.engine.model.FolderReport
import app.kaeri.engine.model.NothingCameIn
import app.kaeri.engine.model.ServerFound
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.store.Adding
import app.kaeri.store.CameInSteps
import app.kaeri.store.ServerStep
import app.kaeri.ui.Icons
import app.kaeri.ui.LocalAppStore
import kotlinx.coroutines.delay

/** How long typing rests before the address is looked at. A pasted Kavita link is looked at at once. */
private const val TYPING_REST_MILLIS = 800L

/** Whether [address] could name a server yet: a dot, a port or localhost in it. */
private fun looksLikeAddress(address: String): Boolean {
    val text = address.trim().removePrefix("https://").removePrefix("http://")
    return text.length >= 3 && (text.contains('.') || text.contains(':') || text.startsWith("localhost"))
}

// Add a server

/** How long a form waits for its window before it moves focus. */
private const val FOCUS_WAIT_MILLIS = 50L

/** The server form for [state], with [onClose] run once it closes. */
@Composable
internal fun serverForm(state: CameInState, onClose: () -> Unit): CameInForm {
    val store = LocalAppStore.current
    val model = store.cameIn
    val context = LocalContext.current
    val form = CameInSteps.server(state.address, state.user, state.password, state.asked, state.outcome, state.asking, state.looking)
    val step = form.step
    val trimmed = state.address.trim()
    // The address is looked at once typing rests, and a pasted Kavita link at once.
    LaunchedEffect(trimmed, state.open) {
        if (state.open != Adding.SERVER || trimmed.isEmpty()) return@LaunchedEffect
        if (state.asked?.address == trimmed) return@LaunchedEffect
        if (form.kavita == null) {
            if (!looksLikeAddress(trimmed)) return@LaunchedEffect
            delay(TYPING_REST_MILLIS)
        }
        model.probe(trimmed)
    }
    // The address is ready to type in as the form opens; while it looks, and once something is
    // found, the keyboard goes; a failure hands the field back to fix, a refused password its field.
    val addressFocus = remember { FocusRequester() }
    val passwordFocus = remember { FocusRequester() }
    val focus = LocalFocusManager.current
    val focusKey = when (step) {
        ServerStep.Typing -> 0
        ServerStep.Looking, is ServerStep.Found, is ServerStep.Empty -> 1
        is ServerStep.SignIn -> if (step.failed) 2 else 0
        is ServerStep.Failed -> 0
    }
    LaunchedEffect(focusKey, state.open) {
        if (state.open != Adding.SERVER) return@LaunchedEffect
        // A frame for the dialog's window to take focus first.
        delay(FOCUS_WAIT_MILLIS)
        runCatching {
            when (focusKey) {
                0 -> addressFocus.requestFocus()
                2 -> passwordFocus.requestFocus()
                else -> focus.clearFocus()
            }
        }
    }
    val found = (step as? ServerStep.Found)?.found ?: (step as? ServerStep.Empty)?.found
    var ticked by remember(found?.pending) { mutableStateOf(found?.libraries?.map { it.id }?.toSet().orEmpty()) }
    var progress by remember(found?.pending) { mutableStateOf(true) }
    val add = {
        when {
            found != null -> {
                val all = found.libraries.map { it.id }
                // Every library ticked asks for all of them, those made later included.
                model.addServer(found, if (ticked.containsAll(all)) emptyList() else all.filter { it in ticked }, progress)
            }
            step is ServerStep.SignIn -> model.probe(trimmed, state.user, state.password)
        }
    }
    val retry = (step as? ServerStep.Failed)?.why is NothingCameIn.Unreachable
    val secondary = if (retry) CameInAction(stringResource(R.string.action_try_again)) { model.probe(trimmed) } else null
    return CameInForm(
        title = stringResource(R.string.book_add_server_title),
        primary = CameInAction(stringResource(R.string.book_add_server_button), enabled = form.canAdd && (found == null || ticked.isNotEmpty() || found.libraries.isEmpty()), onClick = add),
        secondary = secondary,
        close = onClose,
    ) { inPage ->
        val kavita = form.kavita
        CameInField(
            state.address,
            model::typeAddress,
            stringResource(R.string.book_server_address),
            modifier = Modifier.focusRequester(addressFocus),
            chip = kavita?.let { Strings.bookServerKey(context, it.last4) },
            shownAs = kavita?.let { it.root + "…" }, // i18n-skip: the link's start, its key hidden
            supporting = when {
                kavita != null && (step == ServerStep.Typing || step == ServerStep.Looking) -> stringResource(R.string.book_server_kavita_key)
                kavita == null && step == ServerStep.Typing && !form.signIn -> stringResource(R.string.book_server_address_hint)
                else -> null
            },
            keyboard = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Go, autoCorrectEnabled = false),
            actions = KeyboardActions(onGo = { if (trimmed.isNotEmpty() && state.asked?.address != trimmed) model.probe(trimmed) }),
        )
        CameInReveal(kavita != null && step == ServerStep.Typing) {
            Text(
                Strings.bookServerKeyKept(context, stringResource(R.string.book_server_keystore_android)),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 12.dp),
            )
        }
        // A server that wants a sign-in: found, then the fields. A pasted Kavita link folds them away.
        val signIn = (step as? ServerStep.SignIn) ?: (state.outcome as? app.kaeri.engine.model.CameIn.Empty)?.v1
            ?.let { it as? NothingCameIn.WrongPassword }?.let { ServerStep.SignIn(it.kind, it.host, false) }
        CameInReveal(form.signIn && signIn != null) {
            signIn?.let { CameInNote(NoteKind.FOUND, Strings.sourcesFoundServer(context, serverKindWords(it.kind)), modifier = Modifier.padding(top = 16.dp)) }
            CameInField(
                state.user,
                model::typeUser,
                stringResource(R.string.book_server_username),
                modifier = Modifier.padding(top = 12.dp),
                keyboard = KeyboardOptions(imeAction = ImeAction.Next, autoCorrectEnabled = false),
            )
            CameInField(
                state.password,
                model::typePassword,
                stringResource(R.string.book_server_password),
                modifier = Modifier.padding(top = 8.dp).focusRequester(passwordFocus),
                password = true,
                keyboard = KeyboardOptions(keyboardType = KeyboardType.Password, imeAction = ImeAction.Done, autoCorrectEnabled = false),
                actions = KeyboardActions(onDone = { if (form.canAdd) add() }),
            )
        }
        CameInSwap(serverResultKey(step), Modifier.padding(top = if (step == ServerStep.Typing) 0.dp else 16.dp)) { shown ->
            ServerResult(shown, ticked, { ticked = it }, progress, { progress = it }, secondary, inPage)
        }
    }
}

/** What the result area shows: a sign-in that was not refused shows nothing there. */
private fun serverResultKey(step: ServerStep): ServerStep = if (step is ServerStep.SignIn && !step.failed) ServerStep.Typing else step

@Composable
private fun ServerResult(
    step: ServerStep,
    ticked: Set<String>,
    tick: (Set<String>) -> Unit,
    progress: Boolean,
    setProgress: (Boolean) -> Unit,
    secondary: CameInAction?,
    inPage: Boolean,
) {
    val context = LocalContext.current
    Column(Modifier.fillMaxWidth()) {
        when (step) {
            ServerStep.Typing -> {}
            ServerStep.Looking -> CameInWaiting(stringResource(R.string.book_server_looking))
            is ServerStep.SignIn -> CameInNote(
                NoteKind.FAILED,
                Strings.bookServerErrPassword(context, step.host),
                Strings.bookServerErrPasswordLine(context, serverKindWords(step.kind)),
            )
            is ServerStep.Found -> FoundServer(step.found, ticked, tick, progress, setProgress)
            is ServerStep.Empty -> {
                CameInNote(NoteKind.INFO, Strings.bookServerEmpty(context, step.found.host), stringResource(R.string.book_server_empty_line))
                Text(
                    stringResource(R.string.book_server_empty_recheck),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(top = 12.dp),
                )
            }
            is ServerStep.Failed -> {
                when (val why = step.why) {
                    is NothingCameIn.Unreachable -> CameInNote(NoteKind.FAILED, Strings.bookServerErrUnreachable(context, why.host), stringResource(R.string.book_server_err_unreachable_line))
                    is NothingCameIn.WrongKey -> CameInNote(NoteKind.FAILED, Strings.bookServerErrKey(context, why.host), stringResource(R.string.book_server_err_key_line))
                    else -> CameInNote(NoteKind.FAILED, stringResource(R.string.book_sources_not_a_book_server), stringResource(R.string.book_server_not_books_line))
                }
                SecondaryInBody(secondary, inPage)
            }
        }
    }
}

/** Found: how many series in how many libraries, each library to tick, and Send progress to the server. */
@Composable
private fun FoundServer(found: ServerFound, ticked: Set<String>, tick: (Set<String>) -> Unit, progress: Boolean, setProgress: (Boolean) -> Unit) {
    val context = LocalContext.current
    val series = found.series?.toInt() ?: found.libraries.sumOf { it.series?.toInt() ?: 0 }
    CameInNote(
        NoteKind.FOUND,
        Strings.bookServerFoundTitle(context, series, found.libraries.size),
        Strings.bookServerFoundLine(context, serverKindWords(found.kind), found.host),
    )
    if (found.libraries.isNotEmpty()) {
        CameInGroup(
            modifier = Modifier.padding(top = 16.dp),
            rows = found.libraries.map { library ->
                @Composable {
                    LibraryRow(
                        library.name,
                        Strings.bookServerLibraryLine(context, library.series?.toInt() ?: 0),
                        library.id in ticked,
                    ) { on -> tick(if (on) ticked + library.id else ticked - library.id) }
                }
            },
        )
    }
    SwitchCard(
        stringResource(R.string.book_server_sync_progress),
        Strings.bookServerSyncProgressLine(context, found.host),
        progress,
        setProgress,
        Modifier.padding(top = 12.dp),
    )
}

// Add a folder

/**
 * The folder form for [state]: the folder with Change, the scan's waiting line, then what it
 * found. Closing before Add folder takes the folder back out: nothing was meant to be added yet.
 */
@Composable
internal fun folderForm(state: CameInState, pickAgain: () -> Unit, onClose: () -> Unit): CameInForm {
    val store = LocalAppStore.current
    val model = store.cameIn
    val context = LocalContext.current
    val view by store.bookSources.view.collectAsStateWithLifecycle()
    val step = CameInSteps.folder(state.outcome)
    val report: FolderReport? = when (step) {
        is CameInSteps.FolderStep.Found -> step.report
        is CameInSteps.FolderStep.Nothing -> step.report
        CameInSteps.FolderStep.Scanning -> null
    }
    val name = report?.name ?: state.folder.orEmpty()
    val takeBack = { report?.let { store.bookSources.removeFolder(it.id) } }
    val another = {
        takeBack()
        pickAgain()
    }
    val found = step is CameInSteps.FolderStep.Found && step.report.series > 0u
    return CameInForm(
        title = stringResource(R.string.sources_add_folder_button),
        primary = when {
            found -> CameInAction(stringResource(R.string.book_folder_add)) { onClose() }
            step is CameInSteps.FolderStep.Nothing || step is CameInSteps.FolderStep.Found -> CameInAction(stringResource(R.string.book_folder_choose_other), onClick = another)
            else -> CameInAction(stringResource(R.string.book_folder_add), enabled = false) {}
        },
        secondary = null,
        close = {
            if (!found) takeBack()
            onClose()
        },
    ) { _ ->
        FolderCard(name, onChange = another)
        CameInSwap(step, Modifier.padding(top = 16.dp)) { shown ->
            Column(Modifier.fillMaxWidth()) {
                when (shown) {
                    CameInSteps.FolderStep.Scanning -> if (state.looking) {
                        val scanning = view?.folders?.firstOrNull { it.name == name }?.scan as? BookFolderScan.Scanning
                        CameInWaiting(Strings.bookSourcesScanning(context, scanning?.found?.toInt() ?: 0))
                    }
                    is CameInSteps.FolderStep.Found -> FolderFound(shown.report)
                    is CameInSteps.FolderStep.Nothing -> FolderNothing(shown.report)
                }
            }
        }
    }
}

/** Found: how many series in how many files, how many matched, and the files it can't open. */
@Composable
private fun FolderFound(report: FolderReport) {
    val context = LocalContext.current
    CameInNote(
        NoteKind.FOUND,
        Strings.bookFolderFound(context, report.series.toInt(), report.files.toInt()),
        Strings.bookFolderFoundLine(context, report.path ?: report.name, report.matched.toInt()),
    )
    if (skipped(report) > 0) {
        UnsupportedFiles(report, Modifier.padding(top = 16.dp))
        CameInFootnote(stringResource(R.string.book_folder_opens))
    }
}

/** Nothing readable: what the folder holds, the files by format when some are books, and what Kaeri opens. */
@Composable
private fun FolderNothing(report: FolderReport) {
    val context = LocalContext.current
    CameInNote(
        NoteKind.INFO,
        Strings.bookFolderEmpty(context, report.name),
        Strings.bookFolderEmptyLine(context, report.files.toInt()),
    )
    if (skipped(report) > 0) {
        UnsupportedFiles(report, Modifier.padding(top = 16.dp))
        CameInFootnote(stringResource(R.string.book_folder_opens))
    } else {
        Text(
            stringResource(R.string.book_folder_opens),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(top = 12.dp),
        )
    }
}

private fun skipped(report: FolderReport): Int = report.unsupported.sumOf { it.count.toInt() } + report.locked.sumOf { it.count.toInt() }

/** "14 MOBI, 6 AZW3, 3 EPUB locked with DRM". */
@Composable
private fun kinds(report: FolderReport): String {
    val context = LocalContext.current
    val parts = report.unsupported.map { "${it.count} ${it.format}" }.toMutableList() // i18n-skip: a count and a format's name
    val locked = report.locked.sumOf { it.count.toInt() }
    if (locked > 0) parts += Strings.bookFolderDrm(context, locked)
    return parts.joinToString(", ") // i18n-skip
}

/** The files Kaeri can't open, counted by format, with Show files listing them. */
@Composable
private fun UnsupportedFiles(report: FolderReport, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    var open by rememberSaveable(report.id) { mutableStateOf(false) }
    val rows = mutableListOf<@Composable () -> Unit>({
        Row(
            Modifier.fillMaxWidth().heightIn(min = 72.dp).padding(start = 16.dp, end = 4.dp, top = 12.dp, bottom = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            CameInRowText(
                Strings.bookFolderUnsupported(context, skipped(report)),
                Strings.bookFolderUnsupportedLine(context, kinds(report)),
                Modifier.weight(1f),
            )
            if (report.listed.isNotEmpty()) {
                TextButton(
                    onClick = { open = !open },
                    colors = ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.onSurface),
                ) {
                    Text(stringResource(R.string.book_folder_show_files), style = MaterialTheme.typography.bodyLarge)
                    Icon(
                        Icons.Chevron,
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(start = 4.dp).size(18.dp).then(if (open) Modifier.rotate(90f) else Modifier),
                    )
                }
            }
        }
    })
    if (open) {
        rows += {
            Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 12.dp)) {
                report.listed.forEach { file ->
                    Text(file, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
        }
    }
    CameInGroup(rows, modifier)
}

/** The form open in Settings, over the page: Add a server or Add a folder, while one is open outside first run. */
@Composable
internal fun BookSourceDialogs() {
    val store = LocalAppStore.current
    val state = rememberCameInState()
    // A picker that would not open leaves the form as it was.
    val pick = rememberFolderPicker(onFailed = {})
    val dialog = !state.inPage
    CameInDialog(open = dialog && state.open == Adding.SERVER) { serverForm(state) { store.cameIn.close() } }
    CameInDialog(open = dialog && state.open == Adding.FOLDER) { folderForm(state, pickAgain = pick) { store.cameIn.close() } }
}

/** The form's body under first run's title, for BooksPage. */
@Composable
internal fun ColumnScope.CameInPageBody(form: CameInForm) {
    form.body(this, true)
}
