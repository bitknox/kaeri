// Watch together's chat as the phone and the TV's remote screen draw it: the conversation as a list,
// the one-line field with Send, the names and times a reaction line says, and the line's capsule over
// the video. Lines go through the engine; the field only types, counts and waits.

package app.kaeri.ui

import androidx.compose.animation.Crossfade
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.disabled
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.design.ChatGlyphs
import app.kaeri.design.Motion
import app.kaeri.design.ObiSilk
import app.kaeri.engine.model.PartyChatBody
import app.kaeri.engine.model.PartyChatLine
import app.kaeri.engine.model.PartyChatRefusal
import app.kaeri.engine.model.Reaction
import app.kaeri.i18n.Strings
import kotlinx.coroutines.delay

/** The four reactions in the order the controls list them. */
val REACTIONS = listOf(Reaction.LAUGH, Reaction.GASP, Reaction.CLAP, Reaction.CRY)

fun reactionGlyph(reaction: Reaction): ImageVector = when (reaction) {
    Reaction.LAUGH -> ChatGlyphs.Laugh
    Reaction.GASP -> ChatGlyphs.Gasp
    Reaction.CLAP -> ChatGlyphs.Clap
    Reaction.CRY -> ChatGlyphs.Cry
}

fun reactionLabel(context: android.content.Context, reaction: Reaction): String = when (reaction) {
    Reaction.LAUGH -> Strings.partyReactLaugh(context)
    Reaction.GASP -> Strings.partyReactGasp(context)
    Reaction.CLAP -> Strings.partyReactClap(context)
    Reaction.CRY -> Strings.partyReactCry(context)
}

/** "12:04", or "1:12:04" past the hour. */
fun chatClock(seconds: Double): String {
    val total = seconds.toLong().coerceAtLeast(0)
    val h = total / 3600
    val m = total % 3600 / 60
    val s = total % 60
    return if (h > 0) "%d:%02d:%02d".format(h, m, s) else "%02d:%02d".format(m, s)
}

/** The name a line shows: "You" for this device's own. */
fun chatName(context: android.content.Context, line: PartyChatLine): String =
    if (line.you) Strings.partyChatYou(context) else line.from

/** "Ren", "Ren and Mika", "Ren, Mika and Sol", "Ren, Mika and 2 others". */
fun chatNames(context: android.content.Context, first: String, with: List<String>): String = when (with.size) {
    0 -> first
    1 -> Strings.partyNamesTwo(context, first, with[0])
    2 -> Strings.partyNamesThree(context, first, with[0], with[1])
    else -> Strings.partyNamesOthers(context, first, with[0], with.size - 1)
}

/** "Ren and Mika laughed at 12:04". */
fun reactedLine(context: android.content.Context, line: PartyChatLine, body: PartyChatBody.Reacted): String {
    val names = chatNames(context, chatName(context, line), body.with)
    val said = when (body.reaction) {
        Reaction.LAUGH -> Strings.partyReactedLaugh(context, names)
        Reaction.GASP -> Strings.partyReactedGasp(context, names)
        Reaction.CLAP -> Strings.partyReactedClap(context, names)
        Reaction.CRY -> Strings.partyReactedCry(context, names)
    }
    return Strings.partyReactedAt(context, said, chatClock(body.at))
}

/** A typed line: the name in weight 600, then the words as written. Never a link. */
fun saidText(name: String, text: String, nameColor: Color = Color.Unspecified): AnnotatedString = buildAnnotatedString {
    withStyle(SpanStyle(fontWeight = FontWeight.SemiBold, color = nameColor)) { append(name) }
    append("  ")
    append(text)
}

/**
 * One line of the conversation: [saidText] for typed lines, a haze sentence for a reaction, and
 * "Ren is muted · Unmute" for a muted member's. Tags a typed line with no language of its own.
 */
@Composable
fun ChatLineRow(line: PartyChatLine, modifier: Modifier = Modifier, onUnmute: (String) -> Unit = {}) {
    val context = LocalContext.current
    val body = line.body
    val style = MaterialTheme.typography.bodyMedium
    when {
        line.muted -> Row(modifier, verticalAlignment = Alignment.CenterVertically) {
            Text(Strings.partyMutedLine(context, line.from) + " · ", style = style, color = ObiSilk.Haze) // i18n-skip
            Text(
                Strings.partyUnmuteShort(context),
                style = style,
                color = ObiSilk.Silk,
                modifier = Modifier.clickable(role = androidx.compose.ui.semantics.Role.Button) { onUnmute(line.from) }.heightIn(min = 24.dp),
            )
        }
        body is PartyChatBody.Said -> Text(
            saidText(chatName(context, line), body.text, if (line.you) ObiSilk.Haze else Color.Unspecified),
            modifier = modifier.semantics { contentDescription = Strings.partyChatAnnounce(context, chatName(context, line), body.text) },
            style = style,
            color = ObiSilk.Silk,
        )
        body is PartyChatBody.Reacted -> Text(reactedLine(context, line, body), modifier = modifier, style = style, color = ObiSilk.Haze)
    }
}

/**
 * The conversation as a list, the newest at the foot: lines, then "Couldn't send · Try again" when
 * the last one was never echoed. Scrolls to the newest as it arrives.
 */
@Composable
fun ChatConversation(modifier: Modifier = Modifier, maxHeight: Dp = Dp.Unspecified, gap: Dp = 8.dp) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val conversation by store.party.conversation.collectAsStateWithLifecycle()
    val lines = conversation.lines
    val failed = conversation.failed
    val state = rememberLazyListState()
    val count = lines.size + if (failed != null) 1 else 0
    LaunchedEffect(count) { if (count > 0) state.scrollToItem(count - 1) }
    // What a list shows is seen: the video's column carries on from the newest.
    LaunchedEffect(lines.lastOrNull()?.id) { lines.lastOrNull()?.let { store.party.seenThrough(it.id) } }
    LazyColumn(
        modifier.let { if (maxHeight != Dp.Unspecified) it.heightIn(max = maxHeight) else it },
        state = state,
        verticalArrangement = Arrangement.spacedBy(gap),
    ) {
        items(lines, key = { it.id.toLong() }) { line -> ChatLineRow(line, Modifier.fillMaxWidth()) { store.partyMute(it, false) } }
        if (failed != null) item(key = "failed") { NotSent(failed) }
    }
}

/** "Couldn't send · Try again": the words and the one action that sends the text again. */
@Composable
fun NotSent(text: String, modifier: Modifier = Modifier) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    Row(modifier.semantics { liveRegion = LiveRegionMode.Polite }, verticalAlignment = Alignment.CenterVertically) {
        Text(Strings.partyChatNotSent(context) + " · ", style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze) // i18n-skip
        Text(
            Strings.partyChatRetry(context),
            style = MaterialTheme.typography.bodyMedium,
            color = ObiSilk.Silk,
            modifier = Modifier.heightIn(min = 24.dp).clickable(role = androidx.compose.ui.semantics.Role.Button) { store.partySay(text) },
        )
    }
}

/**
 * The room's line capsule over the video: RaisedHigh at radius 20, bodyMedium, 360 wide at most.
 * [leading] is a notice's mark.
 */
@Composable
fun ChatCapsule(modifier: Modifier = Modifier, leading: (@Composable () -> Unit)? = null, content: @Composable () -> Unit) {
    Row(
        modifier
            .widthIn(max = 360.dp)
            .clip(RoundedCornerShape(20.dp))
            .background(ObiSilk.RaisedHigh)
            .padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        leading?.invoke()
        content()
    }
}

/** What the field says it is waiting out: the seconds left of a refusal or of the server's rate. */
@Composable
private fun rememberWait(): Int {
    val store = LocalAppStore.current
    val chat by store.party.chat.collectAsStateWithLifecycle()
    val refusal by store.party.refusal.collectAsStateWithLifecycle()
    var left by remember { mutableIntStateOf(0) }
    LaunchedEffect(refusal, chat?.waitSeconds) {
        val asked = (refusal?.what as? PartyChatRefusal.Wait)?.seconds?.toInt() ?: 0
        left = maxOf(asked, chat?.waitSeconds?.toInt() ?: 0)
        while (left > 0) {
            delay(1000)
            left -= 1
        }
    }
    return left
}

/**
 * Say something: a pill field and a tonal Send. The count shows at 20 characters left, and "Wait 3 s"
 * takes its place while the room's rate holds Send. The text of a line that was never echoed comes
 * back into the field. [onSent] hears a line going out.
 */
@Composable
fun ChatField(
    modifier: Modifier = Modifier,
    height: Dp = 48.dp,
    focus: FocusRequester? = null,
    fieldColor: Color = ObiSilk.RaisedHigh,
    sendColor: Color = ObiSilk.RaisedSelected,
    onSent: () -> Unit = {},
    start: String = "",
) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val chat by store.party.chat.collectAsStateWithLifecycle()
    val conversation by store.party.conversation.collectAsStateWithLifecycle()
    val max = chat?.maxLength?.toInt() ?: 150
    var value by remember { mutableStateOf(start) }
    val wait = rememberWait()
    val failed = conversation.failed
    LaunchedEffect(failed) { if (failed != null && value.isBlank()) value = failed }
    val send = {
        val said = value.trim()
        if (said.isNotEmpty() && wait == 0) {
            store.partySay(said)
            value = ""
            onSent()
        }
    }
    val left = max - value.length
    val enabled = value.isNotBlank() && wait == 0
    Row(modifier, horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
        Row(
            Modifier.weight(1f).height(height).clip(CircleShape).background(fieldColor).padding(horizontal = 20.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Box(Modifier.weight(1f)) {
                if (value.isEmpty()) Text(Strings.partyChatField(context), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze, maxLines = 1)
                BasicTextField(
                    value = value,
                    onValueChange = { if (it.length <= max && '\n' !in it) value = it },
                    modifier = Modifier.fillMaxWidth().let { if (focus != null) it.focusRequester(focus) else it },
                    singleLine = true,
                    textStyle = MaterialTheme.typography.bodyLarge.copy(color = ObiSilk.Silk),
                    cursorBrush = SolidColor(ObiSilk.Silk),
                    keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences, imeAction = ImeAction.Send),
                    keyboardActions = KeyboardActions(onSend = { send() }),
                )
            }
            Crossfade(if (wait > 0) 1 else if (left <= 20) 2 else 0, animationSpec = Motion.fadeIn(), label = "count") { shown -> // i18n-skip
                when (shown) {
                    1 -> Text(
                        Strings.partyChatWait(context, wait),
                        style = MaterialTheme.typography.labelMedium,
                        color = ObiSilk.Haze,
                        modifier = Modifier.padding(start = 8.dp).semantics { contentDescription = Strings.partyChatWaitSpoken(context, wait) },
                    )
                    2 -> Text(left.toString(), style = MaterialTheme.typography.labelMedium, color = ObiSilk.Haze, modifier = Modifier.padding(start = 8.dp))
                    else -> Box(Modifier.size(0.dp))
                }
            }
        }
        SendButton(enabled = enabled, color = sendColor, onClick = send)
    }
}

/** The tonal Send: 48 dp, the paper arrow, dim until there is something to send. */
@Composable
fun SendButton(enabled: Boolean, color: Color = ObiSilk.RaisedSelected, onClick: () -> Unit) {
    val context = LocalContext.current
    Box(
        Modifier
            .size(48.dp)
            .clip(CircleShape)
            .background(color)
            .clickable(enabled = enabled, role = androidx.compose.ui.semantics.Role.Button, onClick = onClick)
            .semantics { contentDescription = Strings.partyChatSend(context); if (!enabled) disabled() },
        contentAlignment = Alignment.Center,
    ) {
        Icon(ChatGlyphs.Send, contentDescription = null, tint = if (enabled) ObiSilk.Silk else ObiSilk.Haze.copy(alpha = 0.5f), modifier = Modifier.size(22.dp))
    }
}

/** The privacy line, once per device: shown in the lobby under the field. */
@Composable
fun ChatPrivacy(modifier: Modifier = Modifier) {
    Text(Strings.partyChatPrivacy(LocalContext.current), modifier, style = MaterialTheme.typography.bodySmall, color = ObiSilk.Haze)
}

/**
 * The chat block of a room's place: the conversation and a field, or why there is none: chat is
 * hidden on this device (with Show chat), or the room's server has chat turned off.
 */
@Composable
fun ChatBlock(modifier: Modifier = Modifier, maxHeight: Dp = 180.dp, privacy: Boolean = false) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val chat by store.party.chat.collectAsStateWithLifecycle()
    val hidden by store.party.hideChat.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { store.party.loadHideChat(context) }
    Column(modifier) {
        when {
            hidden -> {
                Text(Strings.partyChatHidden(context), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk)
                Text(Strings.partyChatHiddenLine(context), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, modifier = Modifier.padding(top = 2.dp))
                androidx.compose.material3.OutlinedButton(
                    onClick = { store.party.setHideChat(context, false) },
                    modifier = Modifier.padding(top = 8.dp).heightIn(min = 40.dp),
                    border = androidx.compose.foundation.BorderStroke(1.dp, ObiSilk.Edge),
                    colors = androidx.compose.material3.ButtonDefaults.outlinedButtonColors(contentColor = ObiSilk.Silk),
                ) { Text(Strings.partyChatShow(context)) }
            }
            chat?.allowed == false -> Text(Strings.partyChatOff(context), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
            else -> {
                ChatConversation(Modifier.fillMaxWidth(), maxHeight = maxHeight)
                ChatField(Modifier.fillMaxWidth().padding(top = 12.dp))
                if (privacy) ChatPrivacy(Modifier.padding(start = 20.dp, top = 8.dp))
            }
        }
    }
}
