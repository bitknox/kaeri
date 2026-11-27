// Settings ▸ Playback ▸ Watch together ▸ Relays: the relays rooms you start use and your invites
// carry, the spares a room swaps in, and the way back to the defaults. A room in progress keeps
// its own list, edited in its Details.

package app.kaeri.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.navigation.NavGraphBuilder
import androidx.navigation.compose.composable
import app.kaeri.engine.model.RoomRelaySettings
import app.kaeri.i18n.Strings
import app.kaeri.store.room.RoomWords

/** The Relays page's route, opened from its row in Playback's Watch together group. */
internal const val ROOM_RELAYS_ROUTE = "settings/playback/relays"

/** What the address field holds before anything is typed. */
private const val RELAY_FIELD = "wss://" // i18n-skip

internal fun NavGraphBuilder.roomRelaysPage() {
    composable(ROOM_RELAYS_ROUTE) { RoomRelaysScreen() }
}

/** The list rooms you start use: the chosen one, or the defaults while none is chosen. */
internal fun RoomRelaySettings.forNewRooms(): List<String> = chosen.ifEmpty { defaults }

/** Watch together's Relays row: how many relays new rooms use, and the page behind it. */
@Composable
internal fun RoomRelaysRow(place: Place) {
    val context = LocalContext.current
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val relays by store.room.relays.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { store.room.loadRelays() }
    InfoRow(
        place,
        Strings.roomRelays(context),
        relays?.let { Strings.roomSettingsRelaysCount(context, it.forNewRooms().size) },
        onClick = { navigator.open(ROOM_RELAYS_ROUTE) },
    )
}

@Composable
internal fun RoomRelaysScreen() {
    val context = LocalContext.current
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val relays by store.room.relays.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { store.room.loadRelays() }
    ScreenFrame(title = Strings.roomRelays(context), onBack = navigator::back) { padding ->
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(top = padding.calculateTopPadding() + 8.dp, bottom = 48.dp)
                .padding(horizontal = 16.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            RoomRelaysContent(relays)
        }
    }
}

/** The page's words and lists, for the screen and its shots. */
@Composable
internal fun RoomRelaysContent(relays: RoomRelaySettings?) {
    val context = LocalContext.current
    val room = LocalAppStore.current.room
    SettingsFootnote(Strings.roomRelaysIntroShort(context), Modifier.padding(bottom = 8.dp))
    relays ?: return
    val forNew = relays.forNewRooms()
    SettingsGroup(
        title = Strings.roomRelaysNewRooms(context),
        rows = forNew.map { url ->
            @Composable { place: Place ->
                // The last relay stays: a list with none would be the defaults again.
                RelayRow(place, url, removable = forNew.size > 1) { room.setRelays(forNew - url) }
            }
        } + listOf<@Composable (Place) -> Unit>({ place -> AddRelayRow(place, forNew) { room.setRelays(forNew + it) } }),
    )
    SettingsGroup(
        title = Strings.roomRelaysSpares(context),
        footer = Strings.roomRelaysSparesLineShort(context),
        rows = relays.spares.map { url ->
            @Composable { place: Place -> RelayRow(place, url, removable = true) { room.setSpares(relays.spares - url) } }
        },
    )
    // Back to the built-in lists: there to press once either differs from them.
    val changed = relays.chosen.isNotEmpty() || relays.spares != relays.defaultSpares
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.Center) {
        TextButton(onClick = room::restoreRelays, enabled = changed) { Text(Strings.roomRelaysRestore(context)) }
    }
}

/** A relay's address, with × to take it off its list. */
@Composable
private fun RelayRow(place: Place, url: String, removable: Boolean, onRemove: () -> Unit) {
    val context = LocalContext.current
    InfoRow(
        place,
        url,
        value = null,
        titleStyle = MaterialTheme.typography.bodyLarge.copy(fontFamily = FontFamily.Monospace),
        trailing = if (removable) {
            {
                IconButton(onClick = onRemove) {
                    Icon(Icons.Clear, contentDescription = Strings.roomRelayRemove(context, url), tint = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
        } else {
            null
        },
    )
}

/** The address field and Add: only a relay the engine takes, and one the list lacks. */
@Composable
private fun AddRelayRow(place: Place, list: List<String>, onAdd: (String) -> Unit) {
    val context = LocalContext.current
    var typed by rememberSaveable { mutableStateOf(RELAY_FIELD) }
    val addable = RoomWords.relayAddable(typed, list)
    val add = {
        if (addable) {
            onAdd(typed.trim())
            typed = RELAY_FIELD
        }
    }
    RowFrame(place, onClick = null) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedTextField(
                value = typed,
                onValueChange = { typed = it },
                singleLine = true,
                textStyle = MaterialTheme.typography.bodyLarge.copy(fontFamily = FontFamily.Monospace),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, autoCorrectEnabled = false, imeAction = ImeAction.Done),
                keyboardActions = KeyboardActions(onDone = { add() }),
                modifier = Modifier.weight(1f),
            )
            TextButton(onClick = add, enabled = addable) { Text(Strings.roomRelayAdd(context)) }
        }
    }
}
