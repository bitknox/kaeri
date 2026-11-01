// TVLicensed and TVServer: the Licensed services page (services ticked from AniList's list, the
// region, whether title pages name the rest) and a media server's page. Both are lists the D-pad
// walks; a row flips or opens.

package app.kaeri.tv.signin

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import app.kaeri.design.ObiSilk
import app.kaeri.i18n.R
import app.kaeri.store.listed
import app.kaeri.tv.ui.Canvas
import app.kaeri.tv.ui.LocalAppStore
import app.kaeri.tv.ui.TvListRow
import app.kaeri.tv.ui.focusSoon

/** A region's name in the person's language: "Denmark". */
internal fun regionName(code: String): String =
    java.util.Locale.Builder().setRegion(code).build().getDisplayCountry(java.util.Locale.getDefault()).ifEmpty { code }

/** The services named in one line, in the person's language. */
fun joinServices(names: List<String>): String = android.icu.text.ListFormatter.getInstance().format(names)

@Composable
fun LicensedScreen(onDone: () -> Unit) {
    val store = LocalAppStore.current
    val view by store.licensed.view.collectAsStateWithLifecycle()
    var all by remember { mutableStateOf(false) }
    var regions by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { store.loadLicensed() }
    BackHandler { if (regions) regions = false else onDone() }
    val first = remember { FocusRequester() }
    LaunchedEffect(view != null, regions) { first.focusSoon() }
    val shown = view
    LazyColumn(
        Modifier.fillMaxSize().background(ObiSilk.Ground).padding(start = Canvas.Keyline, end = Canvas.SafeX, top = 72.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        item(key = "title") {
            Text(stringResource(R.string.licensed_title), style = MaterialTheme.typography.titleLarge, color = ObiSilk.Silk, modifier = Modifier.padding(bottom = 8.dp).semantics { heading() })
        }
        if (shown == null) return@LazyColumn
        if (regions) {
            items(shown.regions, key = { "region/$it" }) { code ->
                TvListRow(regionName(code), onClick = { store.setLicensedRegion(code); regions = false }, selected = code == shown.region, modifier = if (code == shown.regions.first()) Modifier.focusRequester(first) else Modifier)
            }
            return@LazyColumn
        }
        item(key = "body") {
            Text(stringResource(R.string.licensed_body), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze, modifier = Modifier.widthIn(max = 720.dp).padding(bottom = 12.dp))
        }
        item(key = "region") {
            TvListRow(
                stringResource(R.string.licensed_region),
                onClick = { regions = true },
                supporting = stringResource(R.string.licensed_region_line),
                supportingLines = 2,
                value = shown.region?.let(::regionName) ?: stringResource(R.string.licensed_region_device),
                modifier = Modifier.focusRequester(first),
            )
        }
        item(key = "yours") {
            Text(stringResource(R.string.licensed_yours), style = MaterialTheme.typography.labelLarge, color = ObiSilk.Haze, modifier = Modifier.padding(top = 16.dp, bottom = 4.dp))
        }
        val names = shown.listed(all)
        items(names, key = { "service/$it" }) { name ->
            val on = shown.settings.services.any { it.equals(name, ignoreCase = true) }
            TvListRow(
                name,
                onClick = { store.setLicensedServices(if (on) shown.settings.services.filterNot { it.equals(name, ignoreCase = true) } else shown.settings.services + name) },
                switch = on,
            )
        }
        if (!all && shown.available.size > names.size) item(key = "all") {
            TvListRow(stringResource(R.string.licensed_show_all, shown.available.size), onClick = { all = true })
        }
        item(key = "others") {
            TvListRow(
                stringResource(R.string.licensed_show_others),
                onClick = { store.setShowOtherServices(!shown.settings.showOthers) },
                supporting = stringResource(R.string.licensed_show_others_line),
                switch = shown.settings.showOthers,
            )
        }
    }
}

@Composable
fun ServerScreen(id: String, onDone: () -> Unit) {
    val store = LocalAppStore.current
    val rows by store.servers.rows.collectAsStateWithLifecycle()
    val row = rows.firstOrNull { it.id == id }
    // Removing keeps the page: "Mika's Jellyfin removed" with Undo under it, the one place a
    // remote can reach, and leaving the page is what lets it go.
    var held by remember { mutableStateOf<app.kaeri.engine.model.ServerRow?>(null) }
    LaunchedEffect(Unit) { store.loadServers() }
    BackHandler(onBack = onDone)
    val first = remember { FocusRequester() }
    LaunchedEffect(row != null, held != null) { first.focusSoon() }
    held?.let { removed ->
        Column(
            Modifier.fillMaxSize().background(ObiSilk.Ground).padding(start = Canvas.Keyline, end = Canvas.SafeX, top = 72.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            Text(stringResource(R.string.servers_removed, removed.name), style = MaterialTheme.typography.titleLarge, color = ObiSilk.Silk, modifier = Modifier.semantics { heading() }.padding(bottom = 12.dp))
            TvListRow(stringResource(R.string.action_undo), onClick = {
                store.restoreServer(removed.id)
                held = null
            }, modifier = Modifier.focusRequester(first))
            TvListRow(stringResource(R.string.action_done), onClick = onDone)
        }
        return
    }
    LazyColumn(
        Modifier.fillMaxSize().background(ObiSilk.Ground).padding(start = Canvas.Keyline, end = Canvas.SafeX, top = 72.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        item(key = "title") {
            Text(row?.name.orEmpty(), style = MaterialTheme.typography.titleLarge, color = ObiSilk.Silk, modifier = Modifier.semantics { heading() })
        }
        if (row == null) return@LazyColumn
        item(key = "line") {
            Text(
                stringResource(R.string.servers_page_line, row.address.removePrefix("http://"), row.version, row.user),
                style = MaterialTheme.typography.bodyLarge,
                color = ObiSilk.Haze,
                modifier = Modifier.padding(bottom = 12.dp),
            )
        }
        items(row.libraries, key = { "library/${it.id}" }) { library ->
            TvListRow(library.name, onClick = {}, supporting = library.kind?.replaceFirstChar { it.uppercase() }, enabled = false)
        }
        item(key = "report") {
            TvListRow(stringResource(R.string.servers_report), onClick = { store.setServerReport(row.id, !row.report) }, supporting = stringResource(R.string.servers_report_line), switch = row.report, modifier = Modifier.focusRequester(first))
        }
        item(key = "refresh") { TvListRow(stringResource(R.string.servers_refresh), onClick = { store.refreshServer(row.id) }) }
        if (!row.signedOut) item(key = "signout") {
            TvListRow(stringResource(R.string.servers_sign_out), onClick = { store.signOutOfServer(row.id) }, supporting = stringResource(R.string.servers_sign_out_line))
        }
        item(key = "remove") {
            TvListRow(stringResource(R.string.servers_remove), onClick = {
                held = row
                store.removeServer(row.id)
            }, supporting = stringResource(R.string.servers_remove_line))
        }
    }
}
