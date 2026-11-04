// Watch together's chat on the phone's player, as the board draws it: one column at the top leading
// corner with the room's notices first and chat lines under them, four reactions in a column at the
// trailing edge, a reaction's moment at the bottom trailing corner, a Chat action that opens the
// field above the keyboard, and ticks on the seek line where somebody reacted.

package app.kaeri.player.ui

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.design.ColumnLine
import app.kaeri.design.FoldingColumn
import app.kaeri.design.Motion
import app.kaeri.design.ObiSilk
import app.kaeri.engine.model.PartyChatBody
import app.kaeri.engine.model.PartyChatLine
import app.kaeri.engine.model.PartyReactionMark
import app.kaeri.engine.model.Reaction
import app.kaeri.i18n.Strings
import app.kaeri.player.PlayerModel
import app.kaeri.store.PartyModel
import app.kaeri.ui.ChatCapsule
import app.kaeri.ui.ChatField
import app.kaeri.ui.LocalAppStore
import app.kaeri.ui.NotSent
import app.kaeri.ui.REACTIONS
import app.kaeri.ui.reactionGlyph
import app.kaeri.ui.reactionLabel
import app.kaeri.ui.saidText
import app.kaeri.ui.chatName
import kotlinx.coroutines.delay

/** At most this many lines stand in the column; the rest fold into "n more in chat". */
private const val COLUMN_LINES = 3

/** The fade-out token's length, for removing a line once it has gone. */
private const val FADE_OUT_MS = 300L

/** Whether chat is on for this party on this device: the server relays it and Hide chat is off. */
@Composable
fun rememberChatOn(): Boolean {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val chat by store.party.chat.collectAsStateWithLifecycle()
    val hidden by store.party.hideChat.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { store.party.loadHideChat(context) }
    return chat?.allowed != false && !hidden
}

/**
 * The column at the top leading corner, 48 from the edge and 80 from the top: the engine's notice
 * first, then chat lines in the order they came. A line holds 4 s, and 1 s for every 10 words past
 * 12; the column holds three and folds the rest into "n more in chat", which opens the people sheet.
 */
@Composable
fun PartyColumn(model: PlayerModel, onMore: () -> Unit, modifier: Modifier = Modifier, compact: Boolean = false) {
    val store = LocalAppStore.current
    val on = rememberChatOn()
    val conversation by store.party.conversation.collectAsStateWithLifecycle()
    val standing = remember { mutableStateListOf<ColumnLine<PartyChatLine>>() }
    LaunchedEffect(conversation.lines, on) {
        val fresh = conversation.lines.filter { it.id > store.party.seenId && !it.muted && it.body is PartyChatBody.Said }
        conversation.lines.lastOrNull()?.let { store.party.seenThrough(it.id) }
        if (on) standing.addAll(fresh.map { ColumnLine(it, PartyModel.holdMillis((it.body as PartyChatBody.Said).text)) })
    }
    LaunchedEffect(on) { if (!on) standing.clear() }
    val context = LocalContext.current
    val chatTexts = conversation.lines.mapNotNull { (it.body as? PartyChatBody.Said)?.let { s -> it.from + ": " + s.text } } // i18n-skip
    Column(modifier, verticalArrangement = Arrangement.spacedBy(6.dp)) {
        // The room's own notices lead; a typed line that the engine also posted as a notice is the line below.
        AlertNote(model, suppress = { text -> chatTexts.any { text == it || text.endsWith(it) } })
        Column {
            FoldingColumn(
                standing,
                keep = if (compact) 1 else COLUMN_LINES,
                showFold = !compact,
                line = { line ->
                    val said = (line.body as? PartyChatBody.Said)?.text.orEmpty()
                    ChatCapsule(Modifier.semantics { liveRegion = LiveRegionMode.Polite; contentDescription = Strings.partyChatAnnounce(context, chatName(context, line), said) }) {
                        Text(saidText(chatName(context, line), said, if (line.you) ObiSilk.Haze else Color.Unspecified), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Silk)
                    }
                },
                fold = { more ->
                    ChatCapsule(Modifier.clickable(onClick = onMore)) {
                        Text(Strings.partyChatMore(context, more), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
                    }
                },
            )
        }
        val failed = conversation.failed
        AnimatedVisibility(failed != null && on, enter = fadeIn(Motion.fadeIn()), exit = fadeOut(Motion.fadeOut())) {
            ChatCapsule { NotSent(failed.orEmpty()) }
        }
    }
}

/**
 * The four reactions in a column at the trailing edge, 48 dp each in a glass capsule. A reaction
 * waits its turn: one per 2 s, so the glyphs dim until the next may go.
 */
@Composable
fun ReactionColumn(modifier: Modifier = Modifier) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val chat by store.party.chat.collectAsStateWithLifecycle()
    var cool by remember { mutableStateOf(false) }
    val cooling = cool || (chat?.reactWaitSeconds ?: 0u) > 0u
    LaunchedEffect(cool) { if (cool) { delay(2000); cool = false } }
    Column(
        modifier
            .clip(RoundedCornerShape(26.dp))
            .background(ObiSilk.Ground.copy(alpha = 0.55f))
            .border(1.dp, Color.White.copy(alpha = 0.08f), RoundedCornerShape(26.dp))
            .padding(2.dp),
    ) {
        for (reaction in REACTIONS) {
            val label = reactionLabel(context, reaction)
            Box(
                Modifier.size(48.dp).clip(CircleShape)
                    .clickable(enabled = !cooling, onClickLabel = Strings.partyReact(context)) {
                        store.partyReact(reaction)
                        cool = true
                    }
                    .semantics { contentDescription = label },
                contentAlignment = Alignment.Center,
            ) {
                Icon(reactionGlyph(reaction), contentDescription = null, tint = ObiSilk.Silk.copy(alpha = if (cooling) 0.4f else 1f), modifier = Modifier.size(24.dp))
            }
        }
    }
}

private class OnVideo(val reaction: Reaction, val name: String) {
    var shown by mutableStateOf(false)
}

/**
 * A reaction's moment: its glyph and the name, bottom trailing, over the corner card, for 1.5 s on
 * the fades. Your own shows on the echo like anyone's.
 */
@Composable
fun ReactionMoments(modifier: Modifier = Modifier) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val on = rememberChatOn()
    val moment by store.party.moment.collectAsStateWithLifecycle()
    val stack = remember { mutableStateListOf<OnVideo>() }
    val rise = with(androidx.compose.ui.platform.LocalDensity.current) { 6.dp.roundToPx() }
    var lastN by remember { mutableStateOf(moment?.n ?: 0L) }
    LaunchedEffect(moment?.n) {
        val now = moment ?: return@LaunchedEffect
        if (now.n == lastN || !on) return@LaunchedEffect
        lastN = now.n
        val name = if (now.reaction.you) Strings.partyChatYou(context) else now.reaction.from
        stack.add(OnVideo(now.reaction.reaction, name))
        while (stack.size > 3) stack.removeAt(0)
    }
    Column(modifier, verticalArrangement = Arrangement.spacedBy(6.dp), horizontalAlignment = Alignment.End) {
        for (item in stack.toList()) {
            key(item) {
                LaunchedEffect(Unit) {
                    item.shown = true
                    delay(1500)
                    item.shown = false
                    delay(FADE_OUT_MS)
                    stack.remove(item)
                }
                AnimatedVisibility(item.shown, enter = fadeIn(Motion.fadeIn()) + slideInVertically(Motion.standard()) { rise }, exit = fadeOut(Motion.fadeOut())) {
                    Row(
                        Modifier.height(40.dp).clip(RoundedCornerShape(20.dp)).background(ObiSilk.RaisedHigh).padding(start = 11.dp, end = 16.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        Icon(reactionGlyph(item.reaction), contentDescription = null, tint = ObiSilk.Silk, modifier = Modifier.size(22.dp))
                        Text(item.name, style = MaterialTheme.typography.bodyMedium.copy(fontWeight = androidx.compose.ui.text.font.FontWeight.SemiBold), color = ObiSilk.Silk)
                    }
                }
            }
        }
    }
}

/**
 * The field above the keyboard: 56 tall with a tonal Send, on a Raised bar the width of the screen.
 * The keyboard never takes the whole screen (the field asks for no extract mode), so the video stays
 * in view. Closes after Send, or on Back.
 */
@Composable
fun TypingBar(onClose: () -> Unit, modifier: Modifier = Modifier) {
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
    androidx.activity.compose.BackHandler(onBack = onClose)
    ChatField(
        modifier = modifier.fillMaxWidth().background(ObiSilk.Raised).imePadding().padding(horizontal = 48.dp, vertical = 8.dp),
        height = 48.dp,
        focus = focus,
        onSent = onClose,
    )
}

/** Where on the seek line, in seconds, ticks stand: each reaction's place, those within 4 dp one tick. */
internal fun tickSeconds(marks: List<PartyReactionMark>): List<Double> = marks.map { it.at }.distinct().sorted()
