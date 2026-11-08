// Where books come from: folders picked through the system's folder picker, Komga, Kavita and OPDS
// servers with their health in words, and plugins that read. The list is the content; Add a source
// is secondary and offers three kinds, nothing more, each opening its form (CameInForms.kt), which
// says what came in or why nothing did. A source just added lands in its group 70% of the way into
// the form's close, and says so; pulling the page down looks again at servers that showed no
// series. Check matches puts the folder names that need a look beside a way to find their AniList
// match.

package app.kaeri.ui.reading

import android.content.ActivityNotFoundException
import android.content.Intent
import android.net.Uri
import android.provider.DocumentsContract
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
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
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.navigation.NavGraphBuilder
import androidx.navigation.compose.composable
import app.kaeri.engine.model.BookFolderRow
import app.kaeri.engine.model.BookFolderScan
import app.kaeri.engine.model.BookGroupRow
import app.kaeri.engine.model.BookPluginRow
import app.kaeri.engine.model.BookServerKind
import app.kaeri.engine.model.BookServerRow
import app.kaeri.engine.model.BookSourcesView
import app.kaeri.engine.model.FolderRef
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.store.Adding
import app.kaeri.ui.Icons
import app.kaeri.ui.InfoRow
import app.kaeri.ui.LocalAppStore
import app.kaeri.ui.LocalNavigator
import app.kaeri.ui.Place
import app.kaeri.ui.QuietButton
import app.kaeri.ui.STORES_ROUTE
import app.kaeri.ui.ScreenFrame
import app.kaeri.ui.SettingsFootnote
import app.kaeri.ui.SettingsGroup
import app.kaeri.ui.SettingsSheet
import app.kaeri.ui.SheetField
import app.kaeri.ui.SheetProblem
import app.kaeri.ui.SheetText
import app.kaeri.ui.SwitchRow
import app.kaeri.ui.kindWord
import app.kaeri.ui.readingWidth
import kotlinx.coroutines.flow.first

/** The book sources' own page: Settings' Sources with both media on opens it. */
internal const val BOOK_SOURCES_ROUTE = "sources/books"

/** The book sources' page among the app's routes. */
internal fun NavGraphBuilder.bookSourcePages() {
    composable(BOOK_SOURCES_ROUTE) { BookSourcesScreen(stringResource(R.string.settings_book_sources)) }
}

// Picking a folder

/**
 * The system's folder picker: a folder chosen is kept with a lasting read permission and given to
 * the engine by its tree, and the folder form opens on it (in first run's page with [inPage]).
 * [onFailed] hears a picker that would not open or a permission refused.
 */
@Composable
internal fun rememberFolderPicker(onFailed: () -> Unit, inPage: Boolean = false): () -> Unit {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val failed by rememberUpdatedState(onFailed)
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocumentTree()) { uri: Uri? ->
        uri ?: return@rememberLauncherForActivityResult
        val kept = runCatching {
            context.contentResolver.takePersistableUriPermission(uri, Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }.isSuccess
        if (!kept) {
            failed()
            return@rememberLauncherForActivityResult
        }
        val name = folderName(uri)
        // The folder form opens on it and says what its scan finds.
        store.cameIn.open(Adding.FOLDER, inPage)
        store.cameIn.scanning(name)
        store.bookSources.addFolder(FolderRef(path = null, treeUri = uri.toString(), bookmark = null, name = name))
    }
    return {
        try {
            launcher.launch(null)
        } catch (_: ActivityNotFoundException) {
            failed()
        }
    }
}

/** A tree's last part: "Manga" for primary:Books/Manga, as the picker showed it. */
private fun folderName(uri: Uri): String? = runCatching {
    DocumentsContract.getTreeDocumentId(uri).substringAfter(':').substringAfterLast('/').ifEmpty { null }
}.getOrNull()

// Words

/** "Kavita", "Komga", or "OPDS catalogue". */
@Composable
internal fun serverKindWords(kind: BookServerKind): String = when (kind) {
    BookServerKind.KOMGA -> "Komga" // i18n-skip
    BookServerKind.KAVITA -> "Kavita" // i18n-skip
    BookServerKind.OPDS -> stringResource(R.string.book_sources_opds)
}

/** "64 series", or what a scan or a missing drive says; "Manga · 64 series · 23 files skipped" when some were. */
@Composable
internal fun folderLine(folder: BookFolderRow): String {
    val context = LocalContext.current
    if (folder.scan == BookFolderScan.Done && folder.skipped > 0u) {
        return Strings.sourceRowFolder(context, folder.path ?: folder.name, folder.series.toInt(), folder.skipped.toInt())
    }
    return listOfNotNull(
        folder.path,
        when (val scan = folder.scan) {
            is BookFolderScan.Scanning -> Strings.bookSourcesScanning(context, scan.found.toInt())
            BookFolderScan.Missing -> Strings.bookSourcesMissing(context)
            BookFolderScan.Done -> Strings.bookSourcesSeries(context, folder.series.toInt())
        },
    ).joinToString(" · ") // i18n-skip
}

/**
 * "Kavita · 212 series in 2 libraries" once a server has settled, as on Apple; "Kavita · 212
 * series in 2 libraries · added just now" for one added this session; "Kavita · no series yet ·
 * looks again in 30 minutes" for one that showed none; "Kavita · books.home.example · 212 series
 * · off" for one switched off or with no libraries to name.
 */
@Composable
internal fun serverLine(server: BookServerRow, justAdded: Boolean, progressShown: Boolean = false): String {
    val context = LocalContext.current
    if (server.empty) return Strings.sourceRowServerEmpty(context, serverKindWords(server.kind))
    if (justAdded) return Strings.sourceRowServer(context, serverKindWords(server.kind), server.series.toInt(), server.libraries.toInt())
    if (server.libraries.toInt() > 0 && !server.off) {
        return Strings.sourceRowServerSeries(context, serverKindWords(server.kind), server.series.toInt(), server.libraries.toInt())
    }
    return listOfNotNull(
        serverKindWords(server.kind),
        server.address.takeIf { it != server.name && !progressShown },
        Strings.bookSourcesSeries(context, server.series.toInt()),
        if (server.off) Strings.bookSourcesOff(context) else null,
        // The second line says what became of the progress, so the first doesn't promise it.
        if (server.reportProgress && !progressShown) Strings.bookSourcesProgressBack(context) else null,
    ).joinToString(" · ") // i18n-skip
}

/** "From Kim's store · version 1.4". */
@Composable
private fun pluginLine(plugin: BookPluginRow): String {
    val context = LocalContext.current
    return listOfNotNull(
        plugin.store?.let { Strings.bookSourcesFromStore(context, it) },
        Strings.bookSourcesVersion(context, plugin.version),
    ).joinToString(" · ") // i18n-skip
}

/** The folder groups that need a look: not matched, and not kept as they are. */
internal fun BookSourcesView.unsure(): List<BookGroupRow> = groups.filter { it.work == null && !it.kept }

/** "2 folders · 2 servers · 1 plugin", for the Settings list. */
internal fun bookSourcesSummary(context: android.content.Context, view: BookSourcesView?): String? {
    view ?: return null
    val folders = view.folders.count { !it.removed }
    if (folders == 0 && view.servers.isEmpty() && view.plugins.isEmpty()) return Strings.settingsNoneYet(context)
    return listOfNotNull(
        folders.takeIf { it > 0 }?.let { Strings.settingsSummaryBookFolders(context, it) },
        view.servers.size.takeIf { it > 0 }?.let { Strings.settingsSummaryBookServers(context, it) },
        view.plugins.size.takeIf { it > 0 }?.let { Strings.settingsSummaryBookPlugins(context, it) },
    ).joinToString(" · ") // i18n-skip
}

// The page and the list

/**
 * The book sources as a page of their own, titled: Add a source in the top bar, secondary, and the
 * list under it.
 */
@Composable
internal fun BookSourcesScreen(title: String) {
    val navigator = LocalNavigator.current
    val store = LocalAppStore.current
    val view by store.bookSources.view.collectAsStateWithLifecycle()
    var adding by rememberSaveable { mutableStateOf(false) }
    val choosing by store.cameIn.choosing.collectAsStateWithLifecycle()
    LaunchedEffect(choosing) {
        if (choosing) {
            adding = true
            store.cameIn.chose()
        }
    }
    // With nothing added, the empty state's own button is the one Add.
    val empty = view?.let { v -> v.folders.none { !it.removed } && v.servers.isEmpty() && v.plugins.isEmpty() } ?: true
    ScreenFrame(
        title = title,
        onBack = navigator::back,
        actions = {
            if (!empty) {
                TextButton(
                    onClick = { adding = true },
                    colors = androidx.compose.material3.ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.onSurface),
                ) { Text(stringResource(R.string.sources_add)) }
            }
        },
    ) { padding ->
        BookSourcesRefresh(true, Modifier.padding(top = padding.calculateTopPadding())) {
            Column(
                Modifier
                    .fillMaxSize()
                    .verticalScroll(rememberScrollState())
                    .padding(top = 8.dp, bottom = 48.dp)
                    .padding(horizontal = 16.dp)
                    .readingWidth(),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                BookSourcesList(onAdd = { adding = true })
            }
        }
    }
    if (adding) AddSourceSheet { adding = false }
}

/**
 * Pulling [content] down looks again at the servers that showed no series, as the 30-minute check
 * would, with the system's indicator while it does. Only where [enabled].
 */
@Composable
internal fun BookSourcesRefresh(enabled: Boolean, modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    if (!enabled) {
        Box(modifier) { content() }
        return
    }
    val store = LocalAppStore.current
    var refreshing by remember { mutableStateOf(false) }
    LaunchedEffect(refreshing) {
        if (!refreshing) return@LaunchedEffect
        val before = store.bookSources.view.value
        // Until the view changes, or briefly when nothing new came in.
        kotlinx.coroutines.withTimeoutOrNull(REFRESH_MILLIS) { store.bookSources.view.first { it != before } }
        refreshing = false
    }
    androidx.compose.material3.pulltorefresh.PullToRefreshBox(
        isRefreshing = refreshing,
        onRefresh = {
            refreshing = true
            store.cameIn.recheck()
        },
        modifier = modifier,
    ) { content() }
}

/** The longest the refresh indicator waits on a recheck that changes nothing. */
private const val REFRESH_MILLIS = 2_500L

/**
 * Folders, servers and plugins, each with what it holds and its health in words; Check matches
 * under the folders while some need a look; and that plugins come from plugin stores. Empty once
 * the engine said so, with Add a source. The forms Add a source opens sit over it, and a source
 * just added lands in its group, its notice said once the form has closed.
 */
@Composable
internal fun BookSourcesList(onAdd: () -> Unit, storesLink: Boolean = true) {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val view by store.bookSources.view.collectAsStateWithLifecycle()
    val justAdded by store.cameIn.justAdded.collectAsStateWithLifecycle()
    var matches by rememberSaveable { mutableStateOf(false) }
    LaunchedEffect(Unit) { store.bookSources.load() }
    BookSourceDialogs()
    CameInNoticeHost()
    // The rows there when the list first showed were not just added here: only later ones land.
    val there = remember { mutableStateOf<Set<String>?>(null) }
    val shown = view ?: return
    BookDemoGroup()
    val ids = shown.folders.map { it.id } + shown.servers.map { it.id }
    if (there.value == null) there.value = ids.toSet()
    val lands = { id: String -> id in justAdded && id !in there.value.orEmpty() }
    val folders = shown.folders.filter { !it.removed }
    if (folders.isEmpty() && shown.servers.isEmpty() && shown.plugins.isEmpty()) {
        // As every reading empty state: a title, a line and one outlined button, no picture.
        PlainEmpty(
            title = stringResource(R.string.sources_add_title),
            line = stringResource(R.string.book_add_line),
            action = stringResource(R.string.sources_add),
            inset = 0.dp,
            onAction = onAdd,
        )
        return
    }
    val unsure = shown.unsure().size
    val chevron: @Composable () -> Unit = { Icon(Icons.Chevron, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant) }
    if (folders.isNotEmpty()) {
        SettingsGroup(
            title = stringResource(R.string.book_sources_folders),
            rows = folders.map { folder ->
                @Composable { place: Place -> LandingRow(lands(folder.id)) { FolderRow(place, folder) } }
            } + listOfNotNull(
                if (unsure > 0) @Composable { place: Place ->
                    InfoRow(place, Strings.bookSourcesCheckMatches(LocalContext.current, unsure), null, onClick = { matches = true }, trailing = chevron)
                } else null,
            ),
        )
    }
    if (shown.servers.isNotEmpty()) {
        SettingsGroup(
            title = stringResource(R.string.book_sources_servers),
            rows = shown.servers.map { server -> @Composable { place: Place -> LandingRow(lands(server.id)) { ServerRow(place, server, server.id in justAdded) } } },
        )
    }
    if (shown.plugins.isNotEmpty()) {
        SettingsGroup(
            title = stringResource(R.string.book_sources_plugins),
            footer = if (storesLink) null else stringResource(R.string.book_sources_plugins_footer),
            rows = shown.plugins.map { plugin ->
                @Composable { place: Place ->
                    SwitchRow(place, plugin.name, checked = !plugin.off, note = pluginLine(plugin)) { on -> store.bookSources.setOff(plugin.id, !on) }
                }
            },
        )
    }
    if (storesLink) {
        SettingsGroup(
            footer = stringResource(R.string.book_sources_plugins_footer),
            rows = listOf({ place ->
                InfoRow(place, stringResource(R.string.nav_plugin_stores), null, onClick = { navigator.open(STORES_ROUTE) }, trailing = chevron)
            }),
        )
    }
    if (matches) CheckMatchesSheet { matches = false }
}

/** A folder: its tile, its name, what it holds, and More with Look again and Remove. */
@Composable
private fun FolderRow(place: Place, folder: BookFolderRow) {
    val store = LocalAppStore.current
    InfoRow(
        place,
        folder.name,
        folderLine(folder),
        leading = { CameInTile(CameInGlyphs.Folder) },
        trailing = {
            MoreMenu { close ->
                DropdownMenuItem(text = { Text(stringResource(R.string.book_sources_scan)) }, onClick = { close(); store.bookSources.scan() })
                DropdownMenuItem(
                    text = { Text(stringResource(R.string.action_remove), color = MaterialTheme.colorScheme.error) },
                    onClick = { close(); store.bookSources.removeFolder(folder.id) },
                )
            }
        },
    )
}

/**
 * A server: its name and what it holds, Signed out with the warning glyph when it needs a sign-in,
 * and More with sending progress back, using it, and Remove.
 */
@Composable
private fun ServerRow(place: Place, server: BookServerRow, justAdded: Boolean) {
    val store = LocalAppStore.current
    val lines by store.rowLines.lines.collectAsStateWithLifecycle()
    ServerProgressRow(
        server,
        place,
        server.name,
        serverLine(server, justAdded, progressShown = lines[server.id] != null),
        note = if (server.signedOut) stringResource(R.string.servers_row_signed_out) else null,
        leading = if (server.signedOut) ({ Icon(Icons.Problem, contentDescription = null, tint = MaterialTheme.colorScheme.error, modifier = Modifier.size(18.dp)) }) else ({ CameInTile(CameInGlyphs.Server) }),
        trailing = {
            MoreMenu { close ->
                CheckedItem(stringResource(R.string.book_sources_report_progress), server.reportProgress) {
                    close()
                    store.bookSources.setReportProgress(server.id, !server.reportProgress)
                }
                CheckedItem(stringResource(R.string.book_sources_use), !server.off) {
                    close()
                    store.bookSources.setOff(server.id, !server.off)
                }
                HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
                DropdownMenuItem(
                    text = { Text(stringResource(R.string.action_remove), color = MaterialTheme.colorScheme.error) },
                    onClick = { close(); store.bookSources.removeServer(server.id) },
                )
            }
        },
    )
}

/** More, at a row's end: its menu on surfaceContainerHigh. */
@Composable
private fun MoreMenu(items: @Composable (close: () -> Unit) -> Unit) {
    var open by remember { mutableStateOf(false) }
    Box {
        IconButton(onClick = { open = true }) {
            Icon(Icons.More, contentDescription = stringResource(R.string.action_more), tint = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        DropdownMenu(
            expanded = open,
            onDismissRequest = { open = false },
            shape = MaterialTheme.shapes.extraSmall,
            containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
        ) { items { open = false } }
    }
}

/** A menu item that is on or off: a silk check when on, the slot kept when off. */
@Composable
private fun CheckedItem(text: String, on: Boolean, onClick: () -> Unit) {
    DropdownMenuItem(
        text = { Text(text) },
        leadingIcon = {
            Box(Modifier.size(24.dp), contentAlignment = Alignment.Center) {
                if (on) Icon(Icons.Check, contentDescription = null, tint = MaterialTheme.colorScheme.onSurface)
            }
        },
        onClick = onClick,
    )
}

// Add a source

/** Add a source: a folder, a server, or a plugin store, nothing more. */
@Composable
internal fun AddSourceSheet(onDismiss: () -> Unit) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    var failed by rememberSaveable { mutableStateOf(false) }
    val pick = rememberFolderPicker(onFailed = { failed = true })
    // The folder form opens once the picker hands a folder back: the sheet goes then.
    val open by store.cameIn.open.collectAsStateWithLifecycle()
    LaunchedEffect(open) { if (open == Adding.FOLDER) onDismiss() }
    SettingsSheet(onDismiss) {
        app.kaeri.ui.SheetTitle(stringResource(R.string.sources_add_title))
        SheetText(stringResource(R.string.book_add_line))
        val chevron: @Composable () -> Unit = { Icon(Icons.Chevron, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant) }
        SettingsGroup(
            modifier = Modifier.padding(top = 16.dp),
            rows = listOf(
                { place ->
                    InfoRow(place, stringResource(R.string.book_add_folder), Strings.bookAddFolderLine(context, kindWord()), onClick = {
                        failed = false
                        pick()
                    }, trailing = chevron)
                },
                { place ->
                    InfoRow(place, stringResource(R.string.book_add_server), stringResource(R.string.book_add_server_line), onClick = {
                        onDismiss()
                        store.cameIn.open(Adding.SERVER)
                    }, trailing = chevron)
                },
                { place ->
                    InfoRow(place, stringResource(R.string.book_add_store), stringResource(R.string.book_add_store_line), onClick = {
                        onDismiss()
                        store.stores.openJoin()
                    }, trailing = chevron)
                },
            ),
        )
        if (failed) SheetProblem(stringResource(R.string.book_folder_failed))
        Spacer(Modifier.height(8.dp))
    }
}

// Check matches

/**
 * Check matches: how many series were matched by their details, then each folder name that needs
 * a look, with Keep as it is and Find. Find searches AniList with the name and lists what it
 * found, each with how sure the match is; choosing one matches the series.
 */
@Composable
internal fun CheckMatchesSheet(onDismiss: () -> Unit) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val view by store.bookSources.view.collectAsStateWithLifecycle()
    val found by store.bookSources.matches.collectAsStateWithLifecycle()
    var finding by rememberSaveable { mutableStateOf<String?>(null) }
    var query by rememberSaveable { mutableStateOf("") }
    val groups = view?.groups.orEmpty()
    val unsure = view?.unsure().orEmpty()
    SettingsSheet(onDismiss, tall = true) {
        Row(Modifier.fillMaxWidth().heightIn(min = 48.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(
                stringResource(R.string.book_sources_check_matches_title),
                style = MaterialTheme.typography.titleLarge,
                modifier = Modifier.weight(1f).semantics { heading() },
            )
            OutlinedButton(onClick = onDismiss) { Text(stringResource(R.string.action_done)) }
        }
        val group = groups.firstOrNull { it.group == finding }
        if (group != null) {
            SheetField(
                query,
                { query = it },
                stringResource(R.string.search_placeholder),
                modifier = Modifier.padding(top = 12.dp),
                keyboard = KeyboardOptions(imeAction = ImeAction.Search),
                actions = KeyboardActions(onSearch = { store.bookSources.findMatch(group.group, query) }),
            )
            found[group.group].orEmpty().forEachIndexed { index, choice ->
                if (index > 0) HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
                Column(
                    Modifier
                        .fillMaxWidth()
                        .heightIn(min = 56.dp)
                        .padding(vertical = 8.dp),
                ) {
                    TextButton(
                        onClick = {
                            store.bookSources.chooseMatch(group.group, choice.anilist)
                            finding = null
                        },
                        colors = androidx.compose.material3.ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.onSurface),
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        Column(Modifier.fillMaxWidth()) {
                            Text(choice.title, style = app.kaeri.design.ShowTitle.row)
                            Text(
                                Strings.libraryMatchPercent(context, choice.confidence.toInt()),
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    }
                }
            }
            return@SettingsSheet
        }
        val matched = groups.count { it.work != null }
        if (unsure.isNotEmpty()) SheetText(Strings.bookMatchesLine(context, matched, unsure.size), Modifier.padding(top = 4.dp))
        unsure.forEachIndexed { index, row ->
            if (index > 0) HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
            Row(
                Modifier.fillMaxWidth().heightIn(min = 72.dp).padding(vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Column(Modifier.weight(1f)) {
                    Text(row.name, style = MaterialTheme.typography.bodyLarge)
                    Text(row.matchedTitle ?: stringResource(R.string.book_sources_no_match), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                QuietButton(stringResource(R.string.book_sources_keep)) { store.bookSources.keep(row.group) }
                OutlinedButton(onClick = {
                    query = row.matchedTitle ?: row.name
                    finding = row.group
                    store.bookSources.findMatch(row.group, query)
                }) { Text(stringResource(R.string.book_sources_find)) }
            }
        }
        SettingsFootnote(stringResource(R.string.book_sources_unmatched_note), Modifier.padding(top = 4.dp))
    }
}
