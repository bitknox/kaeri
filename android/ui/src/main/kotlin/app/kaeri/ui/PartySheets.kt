// Watch together's sheets on the phone: Choose a server, the first time a party starts, and where
// to join when this phone drives a TV. What the room watches is picked by pressing Play anywhere.

package app.kaeri.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings

/**
 * Start a party, the first time: Choose a server, then the lobby, with nothing picked yet. Once
 * in, Play on any episode picks it for the room. [onStarted] gets the new room's name.
 */
@Composable
internal fun PartyStartSheet(onDismiss: () -> Unit, onStarted: (String) -> Unit) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val form by store.party.form.collectAsStateWithLifecycle()
    SettingsSheet(onDismiss = onDismiss) {
        ServerStep(form, line = Strings.partyFormPickLater(context), onStart = { server, name ->
            val shown = form
            if (shown != null) {
                store.startParty(server, shown.room, name)
                onStarted(shown.room)
            }
        })
    }
}

/** Choose a server, the first time: the server, the suggested one to use, your name, Start party. */
@Composable
private fun ServerStep(form: app.kaeri.engine.model.PartyForm?, line: String? = null, onStart: (String, String?) -> Unit) {
    val context = LocalContext.current
    var server by remember(form) { mutableStateOf(form?.suggestedServer.orEmpty()) }
    var name by remember(form) { mutableStateOf(form?.name.orEmpty()) }
    Column {
        SheetTitle(stringResource(R.string.party_first_server_title))
        if (line != null) SheetText(line, Modifier.padding(bottom = 12.dp))
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
