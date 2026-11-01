// Watch together's sheets on the phone: What are you watching?, which starts a party or changes
// the room's episode, and the first time, Choose a server.

package app.kaeri.ui

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
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
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.design.Motion
import app.kaeri.design.ObiSilk
import app.kaeri.design.ShowTitle
import app.kaeri.engine.model.Entry
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings

/** A title the picker offers, and the episode it would start the room on. */
internal data class PickRow(val entry: Entry, val episode: UInt)

/**
 * What the picker offers for [query]: what you are watching first, at the episode that comes next,
 * then every other title this device knows, at episode 1.
 */
internal fun pickRows(continuing: List<PickRow>, known: Collection<Entry>, query: String): List<PickRow> {
    val words = query.trim().lowercase()
    val seen = continuing.map { it.entry.id }.toSet()
    val all = continuing + known.filter { it.id !in seen }.sortedBy { it.showTitle }.map { PickRow(it, 1u) }
    if (words.isEmpty()) return continuing.ifEmpty { all.take(12) }
    return all.filter { row -> row.entry.showTitle.lowercase().contains(words) || row.entry.secondary?.lowercase()?.contains(words) == true }
}

/**
 * What are you watching?: a search over the titles this device knows, what you are watching
 * first. A pick starts the party, the first time after Choose a server; with [changing], it
 * changes the room's episode for everyone.
 */
@Composable
internal fun PartyPickSheet(onDismiss: () -> Unit, onStarted: () -> Unit, changing: Boolean = false) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val form by store.party.form.collectAsStateWithLifecycle()
    val titles by store.titles.entries.collectAsStateWithLifecycle()
    var query by remember { mutableStateOf("") }
    var chosen by remember { mutableStateOf<PickRow?>(null) }
    LaunchedEffect(Unit) { store.loadPartyForm() }
    val continuing = remember(titles) { store.continuing.queue(titles).all.map { PickRow(it.entry, it.episode) } }
    fun start(row: PickRow, server: String, name: String?) {
        val shown = form ?: return
        store.startParty(server, shown.room, name, row.entry.id, row.episode.toString())
        onStarted()
    }
    fun pick(row: PickRow) {
        if (changing) {
            store.changePartyEpisode(row.entry.id, row.episode.toString())
            onStarted()
            return
        }
        val shown = form
        val server = shown?.recentServers?.firstOrNull()
        if (shown != null && server != null) start(row, server, null) else chosen = row
    }
    SettingsSheet(onDismiss = onDismiss, tall = true) {
        AnimatedContent(chosen, transitionSpec = { fadeIn(Motion.fadeIn()) togetherWith fadeOut(Motion.fadeOut()) }, label = "pick") { row -> // i18n-skip
            if (row == null) {
                Column {
                    SheetTitle(stringResource(R.string.party_pick_title))
                    OutlinedTextField(
                        value = query,
                        onValueChange = { query = it },
                        modifier = Modifier.fillMaxWidth().padding(bottom = 8.dp),
                        placeholder = { Text(stringResource(R.string.party_pick_search)) },
                        leadingIcon = { Icon(Icons.Search, contentDescription = null) },
                        singleLine = true,
                    )
                    LazyColumn(Modifier.fillMaxWidth().heightIn(max = 480.dp)) {
                        items(pickRows(continuing, titles.values, query), key = { it.entry.id.toString() }) { pickRow ->
                            PickTitleRow(pickRow) { pick(pickRow) }
                        }
                    }
                }
            } else {
                ServerStep(form, onStart = { server, name -> start(row, server, name) })
            }
        }
    }
}

@Composable
private fun PickTitleRow(row: PickRow, onPick: () -> Unit) {
    val context = LocalContext.current
    Row(
        Modifier.fillMaxWidth().heightIn(min = 72.dp).clickable(onClick = onPick).padding(vertical = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.size(36.dp, 54.dp).clip(RoundedCornerShape(4.dp)).background(ObiSilk.Raised)) {
            CoverArt(row.entry, 36.dp, Modifier.fillMaxSize())
        }
        Column(Modifier.weight(1f)) {
            Text(row.entry.showTitle, style = ShowTitle.row, maxLines = 1, overflow = TextOverflow.Ellipsis)
            PartyHaze(Strings.partyPickNext(context, row.episode.toString()), maxLines = 1)
        }
    }
}

/** Choose a server, the first time: the server, the suggested one to use, your name, Start party. */
@Composable
private fun ServerStep(form: app.kaeri.engine.model.PartyForm?, onStart: (String, String?) -> Unit) {
    val context = LocalContext.current
    var server by remember(form) { mutableStateOf(form?.suggestedServer.orEmpty()) }
    var name by remember(form) { mutableStateOf(form?.name.orEmpty()) }
    Column {
        SheetTitle(stringResource(R.string.party_first_server_title))
        OutlinedTextField(
            value = server,
            onValueChange = { server = it },
            modifier = Modifier.fillMaxWidth(),
            label = { Text(stringResource(R.string.party_server)) },
            placeholder = { Text(stringResource(R.string.party_server_hint)) },
            textStyle = MaterialTheme.typography.bodyLarge.copy(fontFamily = FontFamily.Monospace),
            singleLine = true,
            supportingText = {
                val suggested = form?.suggestedServer
                if (suggested != null && server.trim() == suggested) {
                    Text(Strings.partyServerPublic(context, form.suggestedRunBy))
                } else if (suggested != null) {
                    Text(Strings.partyServerSuggest(context, suggested, form.suggestedRunBy))
                }
            },
            trailingIcon = {
                val suggested = form?.suggestedServer
                if (suggested != null && server.trim() != suggested) {
                    TextButton(colors = quietText(), onClick = { server = suggested }) { Text(stringResource(R.string.party_use_it)) }
                }
            },
        )
        OutlinedTextField(
            value = name,
            onValueChange = { name = it },
            modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
            label = { Text(stringResource(R.string.party_your_name)) },
            singleLine = true,
        )
        SheetButtons {
            Button(onClick = { onStart(server.trim(), name.trim().ifEmpty { null }) }, enabled = server.isNotBlank()) {
                Text(stringResource(R.string.party_start_party))
            }
        }
    }
}


/**
 * A link opened while this phone drives a TV: join on the TV, which plays from its own sources,
 * or on this phone, which opens the lobby.
 */
@Composable
internal fun PartyTvChoiceSheet(target: app.kaeri.engine.model.PartyTarget, tvName: String, onTv: () -> Unit, onPhone: () -> Unit, onDismiss: () -> Unit) {
    val context = LocalContext.current
    SettingsSheet(onDismiss = onDismiss) {
        SheetTitle(target.by?.let { Strings.partyInvitedBy(context, it) } ?: Strings.partyJoinTitle(context))
        val line = listOfNotNull(target.title, target.episode?.let { Strings.partyFormEpisode(context, it.toDoubleOrNull()?.toInt() ?: 0) }).joinToString(" · ") // i18n-skip
        if (line.isNotEmpty()) SheetText(line)
        Column(Modifier.fillMaxWidth().padding(top = 24.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(onClick = onTv, modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp)) { Text(Strings.partyJoinOnDevice(context, tvName)) }
            androidx.compose.material3.FilledTonalButton(onClick = onPhone, modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp)) {
                Text(Strings.partyJoinOnThisPhone(context))
            }
        }
    }
}
