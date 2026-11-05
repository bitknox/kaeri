// Settings, Profiles, and the sheet behind the account button once there is more than one
// profile. Profiles are optional: with one person there is no picker and no names.

package app.kaeri.ui

import app.kaeri.design.SyncIcons
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
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
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.engine.model.ProfileView
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.store.SyncSheet
import app.kaeri.store.addProfile
import app.kaeri.store.deleteProfile
import app.kaeri.store.editProfile
import app.kaeri.store.loadSync
import app.kaeri.store.switchProfile

/** The account button: the active profile's initial once there are profiles, and its sheet then opens. */
@Composable
internal fun profileAccount(user: String?, openSettings: () -> Unit): Account {
    val store = LocalAppStore.current
    val profiles by store.sync.profiles.collectAsStateWithLifecycle()
    val active = profiles.profiles.firstOrNull { it.active }
    return if (profiles.profiles.size > 1 && active != null) {
        Account(active.name) { store.sync.show(SyncSheet.SWITCH) }
    } else {
        Account(user, openSettings)
    }
}

/** The profiles page: one person's own profile with Add a profile, or the list of the household. */
@Composable
internal fun ProfilesPane(actions: AppActions) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val profiles by store.sync.profiles.collectAsStateWithLifecycle()
    val trackers by store.accounts.trackers.collectAsStateWithLifecycle()
    val user = trackers.values.firstOrNull { it.connected }?.user
    var adding by remember { mutableStateOf(false) }
    var editing by remember { mutableStateOf<ProfileView?>(null) }
    LaunchedEffect(Unit) { store.loadSync() }
    val list = profiles.profiles
    Pane {
        if (list.size <= 1) {
            SettingsGroup(
                title = stringResource(R.string.profiles_title),
                footer = stringResource(R.string.profiles_one_foot),
                rows = listOf(
                    { place ->
                        InfoRow(
                            place,
                            list.firstOrNull()?.name ?: user ?: stringResource(R.string.profiles_yours),
                            stringResource(R.string.profiles_yours_note),
                            leading = { Avatar(list.firstOrNull()?.name ?: user, 40) },
                        )
                    },
                    { place -> AddRow(place, stringResource(R.string.profiles_add)) { adding = true } },
                ),
            )
        } else {
            SettingsGroup(
                title = stringResource(R.string.profiles_title),
                footer = stringResource(R.string.profiles_shared_foot),
                rows = list.map { profile ->
                    @Composable { place: Place ->
                        InfoRow(
                            place,
                            profile.name,
                            null,
                            leading = { Avatar(profile.name, 40) },
                            trailing = { if (profile.kids) Text(stringResource(R.string.profiles_kids_tag), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant) },
                            onClick = { editing = profile },
                        )
                    }
                } + listOf<@Composable (Place) -> Unit>({ place -> AddRow(place, stringResource(R.string.profiles_add)) { adding = true } }),
            )
        }
    }
    if (adding) {
        ProfileSheet(null, firstName = list.firstOrNull()?.name ?: user ?: stringResource(R.string.profiles_yours), onDismiss = { adding = false })
    }
    editing?.let { profile ->
        ProfileSheet(profile, firstName = null, onDismiss = { editing = null }, canDelete = list.size > 1)
    }
}

@Composable
private fun AddRow(place: Place, title: String, onClick: () -> Unit) {
    RowFrame(place, onClick = onClick, modifier = Modifier.heightIn(min = 56.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            Icon(Icons.Plus, null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
            Text(title, style = MaterialTheme.typography.titleMedium)
        }
    }
}

/** A profile's name and Kids switch: new, or an existing one to rename, change or delete. */
@Composable
private fun ProfileSheet(profile: ProfileView?, firstName: String?, onDismiss: () -> Unit, canDelete: Boolean = false) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    var name by remember { mutableStateOf(profile?.name ?: "") }
    var kids by remember { mutableStateOf(profile?.kids ?: false) }
    var deleting by remember { mutableStateOf(false) }
    SettingsSheet(onDismiss = onDismiss, holdsInput = profile == null && name.isNotEmpty()) {
        SheetTitle(if (profile == null) stringResource(R.string.profiles_add) else profile.name)
        if (profile == null) SheetText(stringResource(R.string.profiles_add_line))
        SheetField(
            name, { name = it }, stringResource(R.string.devices_name),
            modifier = Modifier.padding(top = 16.dp),
            keyboard = androidx.compose.foundation.text.KeyboardOptions(imeAction = ImeAction.Done),
        )
        Column(Modifier.padding(top = 8.dp)) {
            SwitchRow(Place.Only, stringResource(R.string.profiles_kids), kids, note = stringResource(R.string.profiles_kids_line)) { kids = it }
        }
        SheetButtons {
            if (profile != null && canDelete) TextButton(onClick = { deleting = true }) { Text(stringResource(R.string.profiles_delete), color = MaterialTheme.colorScheme.error) }
            QuietButton(stringResource(R.string.action_cancel), onClick = onDismiss)
            Button(enabled = name.isNotBlank(), onClick = {
                if (profile == null) store.addProfile(name.trim(), kids, firstName) else store.editProfile(profile.id, name.trim(), kids)
                onDismiss()
            }) { Text(stringResource(if (profile == null) R.string.profiles_add_button else R.string.action_save)) }
        }
    }
    if (deleting && profile != null) {
        ConfirmAlert(
            Strings.profilesDeleteAlert(context, profile.name),
            Strings.profilesDeleteAlertLine(context, profile.name),
            stringResource(R.string.profiles_delete),
            onDismiss = { deleting = false },
        ) { store.deleteProfile(profile.id); deleting = false; onDismiss() }
    }
}

/** The account button's sheet: switch profile, Manage profiles and Settings. */
@Composable
internal fun ProfileSwitchSheet(actions: AppActions) {
    val store = LocalAppStore.current
    val sheet by store.sync.sheet.collectAsStateWithLifecycle()
    if (sheet != SyncSheet.SWITCH) return
    val profiles by store.sync.profiles.collectAsStateWithLifecycle()
    val colors = MaterialTheme.colorScheme
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    val close = { store.sync.show(null) }
    SettingsSheet(onDismiss = close) {
        profiles.profiles.forEach { profile ->
            Row(
                Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(28.dp))
                    .then(if (profile.active) Modifier.background(colors.primaryContainer) else Modifier)
                    .clickable { if (!profile.active) store.switchProfile(profile.id); close() }
                    .heightIn(min = 56.dp)
                    .padding(horizontal = 24.dp, vertical = 8.dp)
                    .semantics { contentDescription = Strings.profilesAvatarDesc(context, profile.name) },
                horizontalArrangement = Arrangement.spacedBy(16.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Avatar(profile.name, 40)
                Text(profile.name, style = MaterialTheme.typography.titleMedium, modifier = Modifier.weight(1f))
                if (profile.kids) Text(stringResource(R.string.profiles_kids_tag), style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant)
                if (profile.active) Icon(Icons.Check, null, tint = colors.onSurface)
            }
        }
        HorizontalDivider(Modifier.padding(vertical = 8.dp), color = colors.outlineVariant)
        SheetChoice(SyncIcons.People, stringResource(R.string.profiles_manage), "") { close(); navigator.open(settingsRoute(SettingsPane.Profiles)) }
        SheetChoice(Icons.Settings, stringResource(R.string.settings_title), "") { close(); actions.openSettings() }
    }
}
