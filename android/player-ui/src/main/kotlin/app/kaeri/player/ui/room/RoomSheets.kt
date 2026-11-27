// The room's sheets and dialogs on the phone: Leave asks first, Invite with its QR code and link,
// Details with the relays, Edit relays, Move to a new room, everyone behind +N, and the alerts for
// a room nobody can reach. Material 3 sheets and dialogs on RaisedHigh.

package app.kaeri.player.ui.room

import android.content.Intent
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.kaeri.design.ObiSilk
import app.kaeri.design.PanelGlyphs
import app.kaeri.design.RoomFace
import app.kaeri.design.FaceRing
import app.kaeri.design.RoomGlyphs
import app.kaeri.design.tabular
import app.kaeri.engine.model.RoomMember
import app.kaeri.engine.model.RoomView
import app.kaeri.store.room.RoomModel
import app.kaeri.store.text
import app.kaeri.store.room.RoomWords
import app.kaeri.ui.QrCode

private val Title = TextStyle(fontSize = 22.sp, lineHeight = 28.sp, fontWeight = FontWeight.Medium, color = ObiSilk.Silk)
private val Body = TextStyle(fontSize = 14.sp, lineHeight = 20.sp, color = ObiSilk.Haze)
private val Mono = TextStyle(fontFamily = FontFamily.Monospace, fontSize = 13.sp, lineHeight = 18.sp, color = ObiSilk.Silk)

/** Leave always asks: "Leave the room?" with Cancel and Leave. */
@Composable
fun LeaveDialog(onLeave: () -> Unit, onCancel: () -> Unit) {
    val c = roomCopy()
    AlertDialog(
        onDismissRequest = onCancel,
        // It asks a question: Back is its Cancel, a tap outside does nothing.
        properties = Asking,
        containerColor = ObiSilk.RaisedHigh,
        title = { Text(c.LEAVE_TITLE, color = ObiSilk.Silk) },
        text = { Text(c.LEAVE_BODY, color = ObiSilk.Haze) },
        dismissButton = { TextButton(onClick = onCancel) { Text(c.CANCEL, color = ObiSilk.Amber) } },
        confirmButton = { TextButton(onClick = onLeave) { Text(c.LEAVE, color = ObiSilk.Amber) } },
    )
}

/** A room nobody answered in, or whose relays nobody could reach: Try again or Close. */
@Composable
fun RoomAlert(title: String, body: String, onRetry: (() -> Unit)?, onClose: () -> Unit) {
    val c = roomCopy()
    AlertDialog(
        onDismissRequest = onClose,
        containerColor = ObiSilk.RaisedHigh,
        title = { Text(title, color = ObiSilk.Silk) },
        text = if (body.isNotBlank()) ({ Text(body, color = ObiSilk.Haze) }) else null,
        dismissButton = if (onRetry != null) ({ TextButton(onClick = onClose) { Text(c.CLOSE, color = ObiSilk.Amber) } }) else null,
        confirmButton = {
            if (onRetry != null) TextButton(onClick = onRetry) { Text(c.TRY_AGAIN, color = ObiSilk.Amber) }
            else TextButton(onClick = onClose) { Text(c.CLOSE, color = ObiSilk.Amber) }
        },
    )
}

/** Why the room dropped this device, when it didn't leave or go along: the alert to say so. */
@Composable
fun RoomLeftAlert(room: RoomModel, why: app.kaeri.engine.model.RoomLeftWhy, onClose: () -> Unit = room::dismissLeft) {
    val c = roomCopy()
    val context = androidx.compose.ui.platform.LocalContext.current
    val (title, body) = when (why) {
        is app.kaeri.engine.model.RoomLeftWhy.Unreachable -> c.UNREACHABLE_TITLE to c.UNREACHABLE_BODY
        is app.kaeri.engine.model.RoomLeftWhy.Full -> c.FULL to ""
        is app.kaeri.engine.model.RoomLeftWhy.OldInvite -> c.OLD_INVITE to ""
        is app.kaeri.engine.model.RoomLeftWhy.Failed -> c.UNREACHABLE_TITLE to why.reason.text(context)
        else -> return
    }
    RoomAlert(title, body, onRetry = null, onClose = onClose)
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun RoomSheet(onDismiss: () -> Unit, holdsInput: Boolean = false, content: @Composable ColumnScope.() -> Unit) {
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        // A tap on the scrim closes it until something in it is typed or changed; Back always does.
        properties = androidx.compose.material3.ModalBottomSheetProperties(shouldDismissOnClickOutside = !holdsInput),
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = ObiSilk.RaisedHigh,
        contentColor = ObiSilk.Silk,
    ) {
        Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).padding(start = 24.dp, end = 24.dp, bottom = 24.dp).navigationBarsPadding(), content = content)
    }
}

/** A grouped list on Raised, its rows split by faint rules. */
@Composable
private fun Group(modifier: Modifier = Modifier, content: @Composable ColumnScope.() -> Unit) {
    Column(modifier.fillMaxWidth().clip(RoundedCornerShape(16.dp)).background(ObiSilk.Raised).padding(horizontal = 16.dp), content = content)
}

@Composable
private fun Rule() = Box(Modifier.fillMaxWidth().height(1.dp).background(ObiSilk.FaintRule))

@Composable
private fun Tonal(text: String, modifier: Modifier = Modifier, icon: androidx.compose.ui.graphics.vector.ImageVector? = null, enabled: Boolean = true, onClick: () -> Unit) {
    FilledTonalButton(
        onClick = onClick,
        enabled = enabled,
        modifier = modifier.height(48.dp),
        shape = CircleShape,
        colors = ButtonDefaults.filledTonalButtonColors(containerColor = ObiSilk.RaisedSelected, contentColor = ObiSilk.Silk),
    ) {
        if (icon != null) Icon(icon, contentDescription = null, modifier = Modifier.size(18.dp).padding(end = 0.dp))
        if (icon != null) Box(Modifier.size(8.dp))
        Text(text, fontSize = 15.sp, fontWeight = FontWeight.Medium, maxLines = 1)
    }
}

@Composable
private fun Amber(text: String, modifier: Modifier = Modifier, icon: androidx.compose.ui.graphics.vector.ImageVector? = null, enabled: Boolean = true, onClick: () -> Unit) {
    androidx.compose.material3.Button(
        onClick = onClick,
        enabled = enabled,
        modifier = modifier.height(48.dp),
        shape = CircleShape,
        colors = ButtonDefaults.buttonColors(containerColor = ObiSilk.Amber, contentColor = ObiSilk.Ground),
    ) {
        if (icon != null) Icon(icon, contentDescription = null, modifier = Modifier.size(18.dp))
        if (icon != null) Box(Modifier.size(8.dp))
        Text(text, fontSize = 15.sp, fontWeight = FontWeight.Medium, maxLines = 1)
    }
}

/** Invite: the words, the QR code, the link, Copy link and Share…, and New invite. */
@Composable
fun InviteSheet(view: RoomView, onCopy: () -> Unit, onNew: () -> Unit, onDismiss: () -> Unit) {
    val c = roomCopy()
    var asking by remember { mutableStateOf(false) }
    RoomSheet(onDismiss) { InviteContent(view, onCopy, onNew = { asking = true }) }
    if (asking) {
        AlertDialog(
            onDismissRequest = { asking = false },
            properties = Asking,
            containerColor = ObiSilk.RaisedHigh,
            title = { Text(c.NEW_INVITE, color = ObiSilk.Silk) },
            text = { Text(c.NEW_INVITE_BODY, color = ObiSilk.Haze) },
            dismissButton = { TextButton(onClick = { asking = false }) { Text(c.CANCEL, color = ObiSilk.Amber) } },
            confirmButton = { TextButton(onClick = { asking = false; onNew() }) { Text(c.NEW_INVITE, color = ObiSilk.Amber) } },
        )
    }
}

@Composable
fun InviteContent(view: RoomView, onCopy: () -> Unit, onNew: () -> Unit) {
    val c = roomCopy()
    val context = LocalContext.current
    val link = view.link.orEmpty()
    Text(c.INVITE, style = Title)
    Text(c.INVITE_BODY, style = Body, modifier = Modifier.padding(top = 4.dp))
    Row(Modifier.padding(top = 16.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        if (link.isNotEmpty()) QrCode(link, size = 112.dp, label = c.INVITE)
        Text(c.INVITE_SCAN, style = Body.copy(fontSize = 13.sp, lineHeight = 18.sp))
    }
    Box(
        Modifier.padding(top = 16.dp).fillMaxWidth().heightIn(min = 48.dp).clip(RoundedCornerShape(12.dp)).background(ObiSilk.Raised).padding(horizontal = 14.dp, vertical = 12.dp),
        contentAlignment = Alignment.CenterStart,
    ) { Text(link, style = Mono, maxLines = 2) }
    Row(Modifier.padding(top = 12.dp).fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        Amber(c.COPY_LINK, Modifier.weight(1f), RoomGlyphs.Copy, onClick = onCopy)
        Tonal(c.SHARE, Modifier.weight(1f), RoomGlyphs.Share) {
            val send = Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_TEXT, link) // i18n-skip
            runCatching { context.startActivity(Intent.createChooser(send, null).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
        }
    }
    Row(
        Modifier.padding(top = 8.dp).fillMaxWidth().height(52.dp).clickable(role = Role.Button, onClick = onNew),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(c.NEW_INVITE, fontSize = 16.sp, color = ObiSilk.Silk, modifier = Modifier.weight(1f))
        Text(c.NEW_INVITE_NOTE, style = Body.copy(fontSize = 13.sp))
    }
}

/** Details: Encryption, the relays with how each answers, what relays are, and Edit relays…. */
@Composable
fun DetailsSheet(view: RoomView, onEdit: () -> Unit, onDismiss: () -> Unit) {
    RoomSheet(onDismiss) { DetailsContent(view, onEdit) }
}

@Composable
fun DetailsContent(view: RoomView, onEdit: () -> Unit) {
    val c = roomCopy()
    val up = view.relays.count { it.healthy }
    Text(c.DETAILS, style = Title)
    Group(Modifier.padding(top = 16.dp)) {
        Pair(c.ENCRYPTION, c.END_TO_END)
        Rule()
        Pair(c.RELAYS, c.relaysAnswering(up, view.relays.size))
    }
    Group(Modifier.padding(top = 12.dp)) {
        view.relays.forEachIndexed { i, r ->
            if (i > 0) Rule()
            Row(Modifier.fillMaxWidth().height(48.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                Box(Modifier.size(8.dp).clip(CircleShape).background(if (r.healthy) ObiSilk.Water else ObiSilk.Haze))
                Text(r.url, style = Mono, modifier = Modifier.weight(1f), maxLines = 1)
                Text(if (r.healthy) r.rttMs?.let { c.ms(it.toInt()) }.orEmpty() else c.NOT_ANSWERING, style = Body.copy(fontSize = 13.sp).tabular())
            }
        }
    }
    Text(c.RELAYS_ABOUT, style = Body.copy(fontSize = 13.sp, lineHeight = 18.sp), modifier = Modifier.padding(top = 12.dp))
    Tonal(c.EDIT_RELAYS_MORE, Modifier.padding(top = 16.dp), onClick = onEdit)
}

@Composable
private fun Pair(label: String, value: String) {
    Row(Modifier.fillMaxWidth().height(48.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(label, fontSize = 15.sp, color = ObiSilk.Haze, modifier = Modifier.weight(1f))
        Text(value, fontSize = 15.sp, color = ObiSilk.Silk)
    }
}

/** Edit relays: this room's list for everyone in it; remove, add, Done. */
@Composable
fun EditRelaysSheet(view: RoomView, onDone: (List<String>) -> Unit, onDismiss: () -> Unit) {
    var changed by remember { mutableStateOf(false) }
    RoomSheet(onDismiss, holdsInput = changed) { EditRelaysContent(view, onDone) { changed = it } }
}

@Composable
fun EditRelaysContent(view: RoomView, onDone: (List<String>) -> Unit, onChanged: (Boolean) -> Unit = {}) {
    val c = roomCopy()
    // The list as the sheet opened: the room's updates (a relay's health, its round trip) come
    // every few seconds and never undo what was removed or added here.
    val opened = remember { view.relays.map { it.url } }
    val list = remember { mutableStateListOf(*opened.toTypedArray()) }
    var typed by remember { mutableStateOf(RELAY_FIELD) }
    val changed = list.toList() != opened || typed != RELAY_FIELD
    androidx.compose.runtime.SideEffect { onChanged(changed) }
    Text(c.EDIT_RELAYS, style = Title)
    Text(c.EDIT_RELAYS_BODY, style = Body, modifier = Modifier.padding(top = 4.dp))
    Group(Modifier.padding(top = 16.dp)) {
        list.forEachIndexed { i, url ->
            if (i > 0) Rule()
            Row(Modifier.fillMaxWidth().height(52.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                Box(Modifier.size(8.dp).clip(CircleShape).background(ObiSilk.Water))
                Text(url, style = Mono, modifier = Modifier.weight(1f), maxLines = 1)
                IconButton(onClick = { list.remove(url) }, modifier = Modifier.size(48.dp)) {
                    Icon(RoomGlyphs.Close, contentDescription = c.relayRemove(url), tint = ObiSilk.Haze, modifier = Modifier.size(18.dp))
                }
            }
        }
    }
    Row(Modifier.padding(top = 10.dp).fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        BasicTextField(
            typed,
            { typed = it.trim() },
            Modifier.weight(1f).height(48.dp).clip(RoundedCornerShape(12.dp)).background(ObiSilk.Raised).border(1.dp, ObiSilk.FaintRule, RoundedCornerShape(12.dp)).padding(horizontal = 14.dp, vertical = 14.dp),
            textStyle = Mono,
            singleLine = true,
            cursorBrush = SolidColor(ObiSilk.Silk),
        )
        Tonal(c.ADD, enabled = RoomWords.relayAddable(typed, list)) { list.add(typed); typed = RELAY_FIELD }
    }
    Text(c.RELAYS_SETTINGS, style = Body.copy(fontSize = 13.sp, lineHeight = 18.sp), modifier = Modifier.padding(top = 12.dp))
    Row(Modifier.padding(top = 16.dp).fillMaxWidth(), horizontalArrangement = Arrangement.End) {
        Amber(c.DONE, enabled = list.isNotEmpty()) { onDone(list.toList()) }
    }
}

/** What a new relay starts as: a scheme, not words. */
private const val RELAY_FIELD = "wss://" // i18n-skip

/** A dialog that asks a question: Back is its Cancel, and a tap outside does nothing. */
private val Asking = androidx.compose.ui.window.DialogProperties(dismissOnClickOutside = false)

/** Move to a new room: tick who is asked along; you always go; muted people start unticked. */
@Composable
fun MoveSheet(view: RoomView, positionS: Double, you: String, onMove: (List<String>) -> Unit, onDismiss: () -> Unit) {
    var changed by remember { mutableStateOf(false) }
    RoomSheet(onDismiss, holdsInput = changed) { MoveContent(view, positionS, you, onMove, onDismiss) { changed = it } }
}

@Composable
fun MoveContent(view: RoomView, positionS: Double, you: String, onMove: (List<String>) -> Unit, onCancel: () -> Unit, onChanged: (Boolean) -> Unit = {}) {
    val c = roomCopy()
    // The ticks as the sheet opened, kept through the room's updates; someone who left since
    // drops out of the list and of what is sent.
    val opened = remember { view.moveCandidates.filter { it.ticked }.map { it.id } }
    val ticked = remember { mutableStateListOf(*opened.toTypedArray()) }
    val here = view.moveCandidates.map { it.id }.toSet()
    val asked = ticked.filter { it in here }
    androidx.compose.runtime.SideEffect { onChanged(ticked.toSet() != opened.toSet()) }
    Text(c.MOVE, style = Title)
    Text(c.MOVE_BODY, style = Body, modifier = Modifier.padding(top = 4.dp))
    Group(Modifier.padding(top = 16.dp)) {
        MoveRow(c.YOU, c.ALWAYS_GOES, on = true, locked = true, initial = you) {}
        for (m in view.moveCandidates) {
            Rule()
            MoveRow(m.name, if (m.muted) c.MUTED_UNTICKED else null, on = m.id in ticked, locked = false) {
                if (m.id in ticked) ticked.remove(m.id) else ticked.add(m.id)
            }
        }
    }
    Text(c.moveFoot(RoomWords.clock(positionS)), style = Body.copy(fontSize = 13.sp, lineHeight = 18.sp), modifier = Modifier.padding(top = 10.dp))
    Row(Modifier.padding(top = 16.dp).fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        Tonal(c.CANCEL, Modifier.weight(1f), onClick = onCancel)
        Amber(c.moveWith(asked.size), Modifier.weight(1f), enabled = asked.isNotEmpty()) { onMove(asked) }
    }
}

@Composable
private fun MoveRow(name: String, note: String?, on: Boolean, locked: Boolean, initial: String = name, onToggle: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().heightIn(min = 56.dp)
            .let { if (locked) it else it.clickable(role = Role.Checkbox, onClick = onToggle) }
            .semantics(mergeDescendants = true) { selected = on },
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        RoomFace(initial, 32.dp, FaceRing.Water, ground = ObiSilk.Raised)
        Column(Modifier.weight(1f)) {
            Text(name, fontSize = 16.sp, color = ObiSilk.Silk)
            if (note != null) Text(note, style = Body.copy(fontSize = 13.sp, lineHeight = 17.sp))
        }
        Box(
            Modifier.size(26.dp).clip(CircleShape)
                .background(if (on) ObiSilk.Amber.copy(alpha = if (locked) 0.45f else 1f) else ObiSilk.Raised)
                .let { if (on) it else it.border(1.5.dp, ObiSilk.Edge, CircleShape) },
            contentAlignment = Alignment.Center,
        ) { if (on) Icon(PanelGlyphs.Check, contentDescription = null, tint = ObiSilk.Ground, modifier = Modifier.size(17.dp)) }
    }
}

/** Everyone behind +N: every face, name and word, this device's first. */
@Composable
fun EveryoneSheet(view: RoomView, members: List<RoomMember>, now: Long, onDismiss: () -> Unit) {
    RoomSheet(onDismiss) { EveryoneContent(view, members, now) }
}

@Composable
fun EveryoneContent(view: RoomView, members: List<RoomMember>, now: Long) {
    val c = roomCopy()
    val ordered = members.filter { it.you } + members.filterNot { it.you }
    Text(c.inTheRoom(view.episode?.episode ?: "", members.size), style = Title)
    Group(Modifier.padding(top = 16.dp)) {
        ordered.forEachIndexed { i, m ->
            if (i > 0) Rule()
            val name = if (m.you) c.YOU else m.name
            val word = RoomWords.word(c, view, m, now)
            Row(
                Modifier.fillMaxWidth().height(60.dp).clearAndSetSemantics { contentDescription = c.spoken(name, word) },
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                PersonFace(view, m, now, 36.dp, ground = ObiSilk.Raised)
                Column {
                    Text(name, fontSize = 16.sp, fontWeight = FontWeight.SemiBold, color = ObiSilk.Silk)
                    Text(word, style = Body.tabular())
                }
            }
        }
    }
}
