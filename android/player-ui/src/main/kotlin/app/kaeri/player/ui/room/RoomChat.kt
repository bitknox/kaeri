// The room's chat on the phone: its lines under the faces and the field in the chat bar. The
// room's own words ("Ren paused at 12:04") sit among the typed lines in haze.

package app.kaeri.player.ui.room

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.design.ObiSilk
import app.kaeri.engine.model.RoomChatBody
import app.kaeri.engine.model.RoomCommand
import app.kaeri.store.room.RoomCopy
import app.kaeri.store.room.RoomModel
import app.kaeri.ui.SendButton
import app.kaeri.ui.saidText

/** The room's words for this screen's context. */
@Composable
fun roomCopy(): RoomCopy {
    val context = LocalContext.current
    return remember(context) { RoomCopy(context) }
}

/**
 * The conversation, newest at the foot, then "Couldn't send" when a line never came back.
 * [roomOnly]: Hide chat in rooms is on, so only the room's own lines show.
 */
@Composable
fun RoomChatList(room: RoomModel, modifier: Modifier = Modifier, roomOnly: Boolean = false, held: Boolean = false) {
    val c = roomCopy()
    // Held while the room fades out after you left: its lines as they were.
    val conversation = app.kaeri.design.keptWhile(!held, room.conversation.collectAsStateWithLifecycle().value)
    val lines = conversation?.lines.orEmpty().filterNot { it.muted || (roomOnly && it.body !is RoomChatBody.Room) }
    val failed = conversation?.failed?.takeUnless { roomOnly }
    val state = rememberLazyListState()
    val count = lines.size + if (failed != null) 1 else 0
    LaunchedEffect(count) { if (count > 0) state.scrollToItem(count - 1) }
    val style = MaterialTheme.typography.bodyMedium
    LazyColumn(modifier, state = state, verticalArrangement = Arrangement.spacedBy(8.dp)) {
        items(lines, key = { it.id.toLong() }) { line ->
            val body = line.body
            if (body is RoomChatBody.Said) {
                Text(saidText(c.chatName(line), body.text, if (line.you) ObiSilk.Haze else Color.Unspecified), style = style, color = ObiSilk.Silk)
            } else {
                Text(c.chatSentence(line).orEmpty(), style = style, color = ObiSilk.Haze)
            }
        }
        if (failed != null) item(key = "failed") { Text(c.NOT_SENT, style = style, color = ObiSilk.Haze) } // i18n-skip: a key
    }
}

/** Say something: the pill field on RaisedHigh and Send on RaisedSelected. */
@Composable
fun RoomChatField(room: RoomModel, modifier: Modifier = Modifier) {
    val c = roomCopy()
    val chat by room.chat.collectAsStateWithLifecycle()
    val conversation by room.conversation.collectAsStateWithLifecycle()
    val max = chat?.maxLength?.toInt() ?: 150
    var value by remember { mutableStateOf("") }
    val failed = conversation?.failed
    LaunchedEffect(failed) { if (failed != null && value.isBlank()) value = failed }
    val send = {
        val said = value.trim()
        if (said.isNotEmpty()) {
            room.send(RoomCommand.Say(said))
            value = ""
        }
    }
    Row(modifier, horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
        Row(
            Modifier.weight(1f).height(48.dp).clip(CircleShape).background(ObiSilk.RaisedHigh).padding(horizontal = 20.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Box(Modifier.weight(1f)) {
                if (value.isEmpty()) Text(c.SAY_SOMETHING, style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze, maxLines = 1)
                BasicTextField(
                    value = value,
                    onValueChange = { if (it.length <= max && '\n' !in it) value = it },
                    modifier = Modifier.fillMaxWidth(),
                    singleLine = true,
                    textStyle = MaterialTheme.typography.bodyLarge.copy(color = ObiSilk.Silk),
                    cursorBrush = SolidColor(ObiSilk.Silk),
                    keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences, imeAction = ImeAction.Send),
                    keyboardActions = KeyboardActions(onSend = { send() }),
                )
            }
        }
        SendButton(enabled = value.isNotBlank(), color = ObiSilk.RaisedSelected, onClick = send)
    }
}
