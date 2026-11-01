// A store owner's invites on the phone: making one for someone new with how often and how long it
// works, the line that reads the choice back, and the store's invite links with Cancel.
package app.kaeri.ui

import android.content.Context
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import androidx.compose.foundation.text.KeyboardOptions
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle
import app.kaeri.engine.model.InviteEndReason
import app.kaeri.engine.model.InviteLasts
import app.kaeri.engine.model.InviteUses
import app.kaeri.engine.model.StoreInviteRow
import app.kaeri.i18n.Strings
import app.kaeri.store.AirTime

internal fun shortDate(seconds: Long): String =
    DateTimeFormatter.ofLocalizedDate(FormatStyle.MEDIUM).withLocale(AirTime.locale)
        .format(Instant.ofEpochSecond(seconds).atZone(ZoneId.systemDefault()))

/** The line under an invite's choices: "Works 5 times, until 7 Nov 2026.", from the same numbers the engine makes the link with. */
internal fun inviteReadBack(context: Context, max: Int, until: Long?): String = when {
    max == 1 && until != null -> Strings.storesInviteOnceUntil(context, shortDate(until))
    max == 1 -> Strings.storesInviteOnceForever(context)
    max == 0 && until != null -> Strings.storesInviteWorksAny(context, shortDate(until))
    max == 0 -> Strings.storesInviteWorksAnyForever(context)
    until != null -> Strings.storesInviteWorksTimes(context, max, shortDate(until))
    else -> Strings.storesInviteWorksTimesForever(context, max)
}

private fun InviteLasts.seconds(): Long? = when (this) {
    InviteLasts.DAY -> 86_400L
    InviteLasts.WEEK -> 7 * 86_400L
    InviteLasts.MONTH -> 30 * 86_400L
    InviteLasts.FOREVER -> null
}

private fun InviteLasts.label(context: Context): String = when (this) {
    InviteLasts.DAY -> Strings.storesInviteLastsDay(context)
    InviteLasts.WEEK -> Strings.storesInviteLastsWeek(context)
    InviteLasts.MONTH -> Strings.storesInviteLastsMonth(context)
    InviteLasts.FOREVER -> Strings.storesInviteLastsForever(context)
}

private fun InviteUses.label(context: Context): String = when (this) {
    InviteUses.ONCE -> Strings.storesInviteUsesOnce(context)
    InviteUses.FIVE -> Strings.storesInviteUsesFive(context)
    InviteUses.ANY -> Strings.storesInviteUsesAny(context)
}

/** Invite someone: a name, how many times it works and for how long, with the choice read back. */
@Composable
internal fun InviteSomeoneSheet(storeId: String, choices: Boolean, onDismiss: () -> Unit) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    var name by remember { mutableStateOf("") }
    var uses by remember { mutableStateOf(InviteUses.ONCE) }
    var lasts by remember { mutableStateOf(InviteLasts.WEEK) }
    var menu by remember { mutableStateOf(false) }
    val working by store.stores.working.collectAsStateWithLifecycle()
    val busy = store.stores.isWorking(working, app.kaeri.store.StoresModel.Work.Invite(storeId))
    val max = when (uses) { InviteUses.ONCE -> 1; InviteUses.FIVE -> 5; InviteUses.ANY -> 0 }
    val now = System.currentTimeMillis() / 1000
    val until = if (choices) lasts.seconds()?.let { now + it } else now + 7 * 86_400L
    SettingsSheet(onDismiss, tall = true) {
        SheetTitle(Strings.storesInviteTitle(context))
        SheetField(
            name,
            { name = it },
            if (uses == InviteUses.ONCE) Strings.storesInviteFor(context) else Strings.storesInviteGroupName(context),
            keyboard = KeyboardOptions(capitalization = KeyboardCapitalization.Words),
            supporting = if (uses == InviteUses.ONCE) null else Strings.storesInviteGroupDetail(context),
        )
        if (choices) {
            Text(Strings.storesInviteUses(context), style = MaterialTheme.typography.labelLarge, modifier = Modifier.padding(top = 16.dp, bottom = 8.dp))
            SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth()) {
                InviteUses.entries.forEachIndexed { i, option ->
                    SegmentedButton(
                        selected = uses == option,
                        onClick = { uses = option },
                        shape = SegmentedButtonDefaults.itemShape(i, InviteUses.entries.size),
                    ) { Text(option.label(context), style = MaterialTheme.typography.labelMedium, textAlign = androidx.compose.ui.text.style.TextAlign.Center) }
                }
            }
            Text(Strings.storesInviteLasts(context), style = MaterialTheme.typography.labelLarge, modifier = Modifier.padding(top = 16.dp, bottom = 8.dp))
            Box {
                OutlinedButton(onClick = { menu = true }, modifier = Modifier.fillMaxWidth()) { Text(lasts.label(context)) }
                DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                    InviteLasts.entries.forEach { option ->
                        DropdownMenuItem(text = { Text(option.label(context)) }, onClick = { lasts = option; menu = false })
                    }
                }
            }
        } else {
            SheetText(Strings.storesServerOlderInvites(context), Modifier.padding(top = 16.dp))
        }
        SheetText(inviteReadBack(context, max, until), Modifier.padding(top = 16.dp))
        SheetButtons {
            QuietButton(Strings.actionCancel(context), onClick = onDismiss)
            Button(
                enabled = !busy,
                onClick = {
                    store.createInvite(
                        storeId,
                        name.trim().ifEmpty { null },
                        uses.takeIf { choices },
                        lasts.takeIf { choices },
                    )
                    onDismiss()
                },
            ) { WorkLabel(Strings.storesMakeLink(context), busy) }
        }
    }
}

/** What an invite row says under its name: "Used 2 of 5 · until 7 Nov". */
internal fun StoreInviteRow.standing(context: Context): String {
    val used = when {
        max.toInt() == 1 -> null
        max.toInt() != 0 -> Strings.storesInviteUsedOf(context, used.toInt(), max.toInt())
        used.toInt() == 0 -> Strings.storesInviteNotUsed(context)
        else -> Strings.storesInviteUsed(context, used.toInt())
    }
    val why = ended?.let {
        when (it.reason) {
            InviteEndReason.USED_UP -> Strings.storesInviteUsedUp(context, shortDate(it.at))
            InviteEndReason.RAN_OUT -> Strings.storesInviteRanOut(context, shortDate(it.at))
            InviteEndReason.CANCELLED -> Strings.storesInviteCancelledOn(context, shortDate(it.at))
        }
    }
    val until = if (ended == null && max.toInt() == 1) until?.let { Strings.storesInvitedUntil(context, shortDate(it)) } else null
    return listOfNotNull(used, why, until).joinToString(" · ") // i18n-skip
}

/** The row's name: who it is for, or "Another device". */
internal fun StoreInviteRow.title(context: Context): String = when {
    device -> Strings.storesInviteUnnamed(context)
    max.toInt() != 1 -> Strings.storesInviteForName(context, name)
    else -> name
}

/** Cancel an invite; says the people who joined with it stay. */
@Composable
internal fun CancelInviteDialog(invite: StoreInviteRow, onDismiss: () -> Unit, onCancel: () -> Unit) {
    val context = LocalContext.current
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(Strings.storesCancelInvite(context)) },
        text = { Text(Strings.storesCancelInviteDetail(context, invite.used.toInt())) },
        confirmButton = { TextButton(onClick = onCancel) { Text(Strings.storesCancelInvite(context), color = MaterialTheme.colorScheme.onSurface) } },
        dismissButton = { QuietButton(Strings.actionCancel(context), onClick = onDismiss) },
        containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
    )
}

/** One invite link: how it stands, who joined with it, and Cancel while it still works. */
@Composable
internal fun InviteDetailSheet(invite: StoreInviteRow, storeId: String, onCancel: (() -> Unit)?, onDismiss: () -> Unit) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    var removing by remember { mutableStateOf<app.kaeri.engine.model.InviteUseRow?>(null) }
    removing?.let { use ->
        AlertDialog(
            onDismissRequest = { removing = null },
            title = { Text(Strings.storesRemoveMemberTitle(context, use.name)) },
            text = { Text(Strings.storesRemoveMemberDetail(context)) },
            confirmButton = {
                TextButton(onClick = { removing = null; store.removeMember(storeId, use.member) }) {
                    Text(Strings.storesRemoveMember(context), color = MaterialTheme.colorScheme.onSurface)
                }
            },
            dismissButton = { QuietButton(Strings.actionCancel(context)) { removing = null } },
            containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
        )
    }
    SettingsSheet(onDismiss) {
        SheetTitle(invite.title(context))
        SheetText(invite.standing(context))
        if (invite.uses.isNotEmpty()) {
            Text(Strings.storesInviteShowUses(context), style = MaterialTheme.typography.labelLarge, modifier = Modifier.padding(top = 16.dp, bottom = 4.dp))
            invite.uses.forEach { use ->
                Row(Modifier.fillMaxWidth(), verticalAlignment = androidx.compose.ui.Alignment.CenterVertically) {
                    Text(
                        Strings.storesInviteJoined(context, use.name, shortDate(use.at)),
                        style = MaterialTheme.typography.bodyMedium,
                        color = if (use.removed) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.onSurface,
                        modifier = Modifier.weight(1f),
                    )
                    if (!use.removed) {
                        TextButton(onClick = { removing = use }) {
                            Text(Strings.storesRemoveMember(context), color = MaterialTheme.colorScheme.onSurface)
                        }
                    }
                }
            }
        }
        SheetButtons {
            QuietButton(Strings.actionClose(context), onClick = onDismiss)
            if (onCancel != null) Button(onClick = onCancel) { Text(Strings.storesCancelInvite(context)) }
        }
    }
}
