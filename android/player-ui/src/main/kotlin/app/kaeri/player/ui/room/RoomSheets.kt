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
import app.kaeri.store.room.RoomCopy
import app.kaeri.store.room.RoomMember
import app.kaeri.store.room.RoomView
import app.kaeri.store.room.RoomWords
import app.kaeri.ui.QrCode

private val Title = TextStyle(fontSize = 22.sp, lineHeight = 28.sp, fontWeight = FontWeight.Medium, color = ObiSilk.Silk)
private val Body = TextStyle(fontSize = 14.sp, lineHeight = 20.sp, color = ObiSilk.Haze)
private val Mono = TextStyle(fontFamily = FontFamily.Monospace, fontSize = 13.sp, lineHeight = 18.sp, color = ObiSilk.Silk)

/** Leave always asks: "Leave the room?" with Cancel and Leave. */
@Composable
fun LeaveDialog(onLeave: () -> Unit, onCancel: () -> Unit) {
    AlertDialog(
        onDismissRequest = onCancel,
        containerColor = ObiSilk.RaisedHigh,
        title = { Text(RoomCopy.LEAVE_TITLE, color = ObiSilk.Silk) },
        text = { Text(RoomCopy.LEAVE_BODY, color = ObiSilk.Haze) },
        dismissButton = { TextButton(onClick = onCancel) { Text(RoomCopy.CANCEL, color = ObiSilk.Amber) } },
        confirmButton = { TextButton(onClick = onLeave) { Text(RoomCopy.LEAVE, color = ObiSilk.Amber) } },
    )
}

/** A room nobody answered in, or whose relays nobody could reach: Try again or Close. */
@Composable
fun RoomAlert(title: String, body: String, onRetry: (() -> Unit)?, onClose: () -> Unit) {
    AlertDialog(
        onDismissRequest = onClose,
        containerColor = ObiSilk.RaisedHigh,
        title = { Text(title, color = ObiSilk.Silk) },
        text = { Text(body, color = ObiSilk.Haze) },
        dismissButton = if (onRetry != null) ({ TextButton(onClick = onClose) { Text(RoomCopy.CLOSE, color = ObiSilk.Amber) } }) else null,
        confirmButton = {
            if (onRetry != null) TextButton(onClick = onRetry) { Text(RoomCopy.TRY_AGAIN, color = ObiSilk.Amber) }
            else TextButton(onClick = onClose) { Text(RoomCopy.CLOSE, color = ObiSilk.Amber) }
        },
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun RoomSheet(onDismiss: () -> Unit, content: @Composable ColumnScope.() -> Unit) {
    ModalBottomSheet(
        onDismissRequest = onDismiss,
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
    var asking by remember { mutableStateOf(false) }
    RoomSheet(onDismiss) { InviteContent(view, onCopy, onNew = { asking = true }) }
    if (asking) {
        AlertDialog(
            onDismissRequest = { asking = false },
            containerColor = ObiSilk.RaisedHigh,
            title = { Text(RoomCopy.NEW_INVITE, color = ObiSilk.Silk) },
            text = { Text(RoomCopy.NEW_INVITE_BODY, color = ObiSilk.Haze) },
            dismissButton = { TextButton(onClick = { asking = false }) { Text(RoomCopy.CANCEL, color = ObiSilk.Amber) } },
            confirmButton = { TextButton(onClick = { asking = false; onNew() }) { Text(RoomCopy.NEW_INVITE, color = ObiSilk.Amber) } },
        )
    }
}

@Composable
fun InviteContent(view: RoomView, onCopy: () -> Unit, onNew: () -> Unit) {
    val context = LocalContext.current
    val link = view.link.orEmpty()
    Text(RoomCopy.INVITE, style = Title)
    Text(RoomCopy.INVITE_BODY, style = Body, modifier = Modifier.padding(top = 4.dp))
    Row(Modifier.padding(top = 16.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        if (link.isNotEmpty()) QrCode(link, size = 112.dp, label = RoomCopy.INVITE)
        Text(RoomCopy.INVITE_SCAN, style = Body.copy(fontSize = 13.sp, lineHeight = 18.sp))
    }
    Box(
        Modifier.padding(top = 16.dp).fillMaxWidth().heightIn(min = 48.dp).clip(RoundedCornerShape(12.dp)).background(ObiSilk.Raised).padding(horizontal = 14.dp, vertical = 12.dp),
        contentAlignment = Alignment.CenterStart,
    ) { Text(link, style = Mono, maxLines = 2) }
    Row(Modifier.padding(top = 12.dp).fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        Amber(RoomCopy.COPY_LINK, Modifier.weight(1f), RoomGlyphs.Copy, onClick = onCopy)
        Tonal(RoomCopy.SHARE, Modifier.weight(1f), RoomGlyphs.Share) {
            val send = Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_TEXT, link) // i18n-skip
            runCatching { context.startActivity(Intent.createChooser(send, null).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
        }
    }
    Row(
        Modifier.padding(top = 8.dp).fillMaxWidth().height(52.dp).clickable(role = Role.Button, onClick = onNew),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(RoomCopy.NEW_INVITE, fontSize = 16.sp, color = ObiSilk.Silk, modifier = Modifier.weight(1f))
        Text(RoomCopy.NEW_INVITE_NOTE, style = Body.copy(fontSize = 13.sp))
    }
}

/** Details: Encryption, the relays with how each answers, what relays are, and Edit relays…. */
@Composable
fun DetailsSheet(view: RoomView, onEdit: () -> Unit, onDismiss: () -> Unit) {
    RoomSheet(onDismiss) { DetailsContent(view, onEdit) }
}

@Composable
fun DetailsContent(view: RoomView, onEdit: () -> Unit) {
    val up = view.relays.count { it.healthy }
    Text(RoomCopy.DETAILS, style = Title)
    Group(Modifier.padding(top = 16.dp)) {
        Pair(RoomCopy.ENCRYPTION, RoomCopy.END_TO_END)
        Rule()
        Pair(RoomCopy.RELAYS, RoomCopy.relaysAnswering(up, view.relays.size))
    }
    Group(Modifier.padding(top = 12.dp)) {
        view.relays.forEachIndexed { i, r ->
            if (i > 0) Rule()
            Row(Modifier.fillMaxWidth().height(48.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                Box(Modifier.size(8.dp).clip(CircleShape).background(if (r.healthy) ObiSilk.Water else ObiSilk.Haze))
                Text(r.url, style = Mono, modifier = Modifier.weight(1f), maxLines = 1)
                Text(if (r.healthy) r.rttMs?.let { RoomCopy.ms(it) }.orEmpty() else RoomCopy.NOT_ANSWERING, style = Body.copy(fontSize = 13.sp).tabular())
            }
        }
    }
    Text(RoomCopy.RELAYS_ABOUT, style = Body.copy(fontSize = 13.sp, lineHeight = 18.sp), modifier = Modifier.padding(top = 12.dp))
    Tonal(RoomCopy.EDIT_RELAYS_MORE, Modifier.padding(top = 16.dp), onClick = onEdit)
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
    RoomSheet(onDismiss) { EditRelaysContent(view, onDone) }
}

@Composable
fun EditRelaysContent(view: RoomView, onDone: (List<String>) -> Unit) {
    val list = remember(view.relays) { mutableStateListOf(*view.relays.map { it.url }.toTypedArray()) }
    var typed by remember { mutableStateOf(RoomCopy.RELAY_FIELD) }
    Text(RoomCopy.EDIT_RELAYS, style = Title)
    Text(RoomCopy.EDIT_RELAYS_BODY, style = Body, modifier = Modifier.padding(top = 4.dp))
    Group(Modifier.padding(top = 16.dp)) {
        list.forEachIndexed { i, url ->
            if (i > 0) Rule()
            Row(Modifier.fillMaxWidth().height(52.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                Box(Modifier.size(8.dp).clip(CircleShape).background(ObiSilk.Water))
                Text(url, style = Mono, modifier = Modifier.weight(1f), maxLines = 1)
                IconButton(onClick = { list.remove(url) }, modifier = Modifier.size(48.dp)) {
                    Icon(RoomGlyphs.Close, contentDescription = RoomCopy.CLOSE, tint = ObiSilk.Haze, modifier = Modifier.size(18.dp))
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
        Tonal(RoomCopy.ADD, enabled = addable(typed)) { list.add(typed); typed = RoomCopy.RELAY_FIELD }
    }
    Text(RoomCopy.RELAYS_SETTINGS, style = Body.copy(fontSize = 13.sp, lineHeight = 18.sp), modifier = Modifier.padding(top = 12.dp))
    Row(Modifier.padding(top = 16.dp).fillMaxWidth(), horizontalArrangement = Arrangement.End) {
        Amber(RoomCopy.DONE, enabled = list.isNotEmpty()) { onDone(list.toList()) }
    }
}

/** Add is only live for a wss:// address with a host after it. */
private fun addable(typed: String): Boolean = typed.length > RoomCopy.RELAY_FIELD.length && typed.startsWith(RoomCopy.RELAY_FIELD)

/** Move to a new room: tick who is asked along; you always go; muted people start unticked. */
@Composable
fun MoveSheet(view: RoomView, positionS: Double, onMove: (List<String>) -> Unit, onDismiss: () -> Unit) {
    RoomSheet(onDismiss) { MoveContent(view, positionS, onMove, onDismiss) }
}

@Composable
fun MoveContent(view: RoomView, positionS: Double, onMove: (List<String>) -> Unit, onCancel: () -> Unit) {
    val ticked = remember(view.moveCandidates) { mutableStateListOf(*view.moveCandidates.filter { it.ticked }.map { it.id }.toTypedArray()) }
    Text(RoomCopy.MOVE, style = Title)
    Text(RoomCopy.MOVE_BODY, style = Body, modifier = Modifier.padding(top = 4.dp))
    Group(Modifier.padding(top = 16.dp)) {
        MoveRow(RoomCopy.YOU, RoomCopy.ALWAYS_GOES, on = true, locked = true) {}
        for (c in view.moveCandidates) {
            Rule()
            MoveRow(c.name, if (c.muted) RoomCopy.MUTED_UNTICKED else null, on = c.id in ticked, locked = false) {
                if (c.id in ticked) ticked.remove(c.id) else ticked.add(c.id)
            }
        }
    }
    Text(RoomCopy.moveFoot(RoomWords.clock(positionS)), style = Body.copy(fontSize = 13.sp, lineHeight = 18.sp), modifier = Modifier.padding(top = 10.dp))
    Row(Modifier.padding(top = 16.dp).fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        Tonal(RoomCopy.CANCEL, Modifier.weight(1f), onClick = onCancel)
        Amber(RoomCopy.moveWith(ticked.size), Modifier.weight(1f), enabled = ticked.isNotEmpty()) { onMove(ticked.toList()) }
    }
}

@Composable
private fun MoveRow(name: String, note: String?, on: Boolean, locked: Boolean, onToggle: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().heightIn(min = 56.dp)
            .let { if (locked) it else it.clickable(role = Role.Checkbox, onClick = onToggle) }
            .semantics(mergeDescendants = true) { selected = on },
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        RoomFace(name, 32.dp, FaceRing.Water, ground = ObiSilk.Raised)
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
    val ordered = members.filter { it.you } + members.filterNot { it.you }
    Text(RoomCopy.inTheRoom(view.episode?.episode ?: "", members.size), style = Title)
    Group(Modifier.padding(top = 16.dp)) {
        ordered.forEachIndexed { i, m ->
            if (i > 0) Rule()
            val name = if (m.you) RoomCopy.YOU else m.name
            val word = RoomWords.word(view, m, now)
            Row(
                Modifier.fillMaxWidth().height(60.dp).clearAndSetSemantics { contentDescription = RoomCopy.spoken(name, word) },
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
