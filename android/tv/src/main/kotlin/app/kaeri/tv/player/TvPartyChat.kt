// Watch together's chat on the TV, as the board draws it: party notices and chat lines in a column
// at the trailing edge under the clock (never focusable), a reaction's moment above the controls,
// and React, whose OK opens four glyphs. Typing is on a paired phone, so nothing here takes text.

package app.kaeri.tv.player

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.ClickableSurfaceDefaults
import androidx.tv.material3.Icon
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Surface
import androidx.tv.material3.Text
import app.kaeri.design.ChatGlyphs
import app.kaeri.design.ColumnLine
import app.kaeri.design.FoldingColumn
import app.kaeri.design.Motion
import app.kaeri.design.ObiSilk
import app.kaeri.engine.model.PartyChatBody
import app.kaeri.engine.model.PartyChatLine
import app.kaeri.engine.model.Reaction
import app.kaeri.i18n.Strings
import app.kaeri.player.PlayerModel
import app.kaeri.store.PartyModel
import app.kaeri.tv.ui.LocalAppStore
import app.kaeri.tv.ui.LocalReducedMotion
import kotlinx.coroutines.delay

private val REACTIONS = listOf(Reaction.LAUGH, Reaction.GASP, Reaction.CLAP, Reaction.CRY)
private const val COLUMN_LINES = 3
private const val FADE_OUT_MS = 300L

internal fun glyph(reaction: Reaction) = when (reaction) {
    Reaction.LAUGH -> ChatGlyphs.Laugh
    Reaction.GASP -> ChatGlyphs.Gasp
    Reaction.CLAP -> ChatGlyphs.Clap
    Reaction.CRY -> ChatGlyphs.Cry
}

private fun label(context: android.content.Context, reaction: Reaction) = when (reaction) {
    Reaction.LAUGH -> Strings.partyReactLaugh(context)
    Reaction.GASP -> Strings.partyReactGasp(context)
    Reaction.CLAP -> Strings.partyReactClap(context)
    Reaction.CRY -> Strings.partyReactCry(context)
}

/** Chat is on for this party on this TV: the server relays it and Hide chat is off. */
@Composable
internal fun rememberTvChatOn(): Boolean {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val chat by store.party.chat.collectAsStateWithLifecycle()
    val hidden by store.party.hideChat.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { store.party.loadHideChat(context) }
    return chat?.allowed != false && !hidden
}

@Composable
private fun TvLine(modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    Row(
        modifier
            .widthIn(max = 380.dp)
            .clip(RoundedCornerShape(12.dp))
            .background(Color(0xF02A2F45))
            .padding(horizontal = 18.dp, vertical = 11.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) { content() }
}

/**
 * Notices and chat lines at the trailing edge, 48 from it and 104 from the top: the party's
 * notice first, then chat lines. Three at most; none takes focus.
 */
@Composable
fun TvPartyColumn(model: PlayerModel, modifier: Modifier = Modifier) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val on = rememberTvChatOn()
    val conversation by store.party.conversation.collectAsStateWithLifecycle()
    val standing = remember { mutableStateListOf<ColumnLine<PartyChatLine>>() }
    LaunchedEffect(conversation.lines, on) {
        val fresh = conversation.lines.filter { it.id > store.party.seenId && !it.muted && it.body is PartyChatBody.Said }
        conversation.lines.lastOrNull()?.let { store.party.seenThrough(it.id) }
        if (on) standing.addAll(fresh.map { ColumnLine(it, PartyModel.holdMillis((it.body as PartyChatBody.Said).text)) })
    }
    LaunchedEffect(on) { if (!on) standing.clear() }
    val chatTexts = conversation.lines.mapNotNull { (it.body as? PartyChatBody.Said)?.let { s -> it.from + ": " + s.text } } // i18n-skip
    Column(modifier.clearAndSetSemantics { }, horizontalAlignment = Alignment.End, verticalArrangement = Arrangement.spacedBy(6.dp)) {
        val alert = model.alert?.takeUnless { a -> chatTexts.any { a.text == it || a.text.endsWith(it) } }
        AnimatedVisibility(alert != null, enter = fadeIn(Motion.fadeIn()), exit = fadeOut(Motion.fadeOut())) {
            TvLine {
                Box(Modifier.size(8.dp).clip(CircleShape).background(ObiSilk.Haze))
                Text(alert?.text.orEmpty(), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk)
            }
        }
        Column(horizontalAlignment = Alignment.End) {
            FoldingColumn(
                standing,
                keep = COLUMN_LINES,
                line = { line ->
                    val said = (line.body as? PartyChatBody.Said)?.text.orEmpty()
                    val name = if (line.you) Strings.partyChatYou(context) else line.from
                    TvLine {
                        Text(
                            buildAnnotatedString {
                                withStyle(SpanStyle(fontWeight = FontWeight.SemiBold, color = if (line.you) ObiSilk.Haze else ObiSilk.Silk)) { append(name) }
                                append("  ")
                                append(said)
                            },
                            style = MaterialTheme.typography.bodyLarge.copy(fontSize = 16.sp, lineHeight = 22.sp),
                            color = ObiSilk.Silk,
                            modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite; contentDescription = Strings.partyChatAnnounce(context, name, said) },
                        )
                    }
                },
                fold = { more -> TvLine { Text(Strings.partyChatMore(context, more), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze) } },
            )
        }
    }
}

private class OnVideo(val reaction: Reaction, val name: String) {
    var shown by mutableStateOf(false)
}

/** A reaction's moment, bottom trailing above the controls: the glyph and the name for 1.5 s. */
@Composable
fun TvReactionMoments(modifier: Modifier = Modifier) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val on = rememberTvChatOn()
    val moment by store.party.moment.collectAsStateWithLifecycle()
    val stack = remember { mutableStateListOf<OnVideo>() }
    val reduced = LocalReducedMotion.current
    val rise = with(androidx.compose.ui.platform.LocalDensity.current) { 6.dp.roundToPx() }
    var lastN by remember { mutableStateOf(moment?.n ?: 0L) }
    LaunchedEffect(moment?.n) {
        val now = moment ?: return@LaunchedEffect
        if (now.n == lastN || !on) return@LaunchedEffect
        lastN = now.n
        stack.add(OnVideo(now.reaction.reaction, if (now.reaction.you) Strings.partyChatYou(context) else now.reaction.from))
        while (stack.size > 3) stack.removeAt(0)
    }
    Column(modifier.clearAndSetSemantics { }, verticalArrangement = Arrangement.spacedBy(6.dp), horizontalAlignment = Alignment.End) {
        for (item in stack.toList()) {
            key(item) {
                LaunchedEffect(Unit) {
                    item.shown = true
                    delay(1500)
                    item.shown = false
                    delay(FADE_OUT_MS)
                    stack.remove(item)
                }
                AnimatedVisibility(item.shown, enter = if (reduced) fadeIn(Motion.fadeIn()) else fadeIn(Motion.fadeIn()) + slideInVertically(Motion.standard()) { rise }, exit = fadeOut(Motion.fadeOut())) {
                    Row(
                        Modifier.height(48.dp).clip(CircleShape).background(Color(0xF02A2F45)).padding(start = 14.dp, end = 20.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        Icon(glyph(item.reaction), contentDescription = null, tint = ObiSilk.Silk, modifier = Modifier.size(26.dp))
                        Text(item.name, style = MaterialTheme.typography.bodyLarge.copy(fontWeight = FontWeight.SemiBold), color = ObiSilk.Silk)
                    }
                }
            }
        }
    }
}

/**
 * The four glyphs above React: 56 dp circles, left and right move along them, OK sends and
 * closes, Back closes. The label of the focused one shows under it.
 */
@Composable
fun TvReactTray(onReact: (Reaction) -> Unit, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val first = remember { FocusRequester() }
    var focused by remember { mutableIntStateOf(0) }
    LaunchedEffect(Unit) { runCatching { first.requestFocus() } }
    val reduced = LocalReducedMotion.current
    Column(
        modifier
            .clip(RoundedCornerShape(24.dp))
            .background(ObiSilk.RaisedHigh)
            .padding(start = 16.dp, end = 16.dp, top = 14.dp, bottom = 10.dp),
    ) {
        Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            REACTIONS.forEachIndexed { i, reaction ->
                Surface(
                    onClick = { onReact(reaction) },
                    shape = ClickableSurfaceDefaults.shape(CircleShape),
                    colors = ClickableSurfaceDefaults.colors(
                        containerColor = ObiSilk.Silk.copy(alpha = 0.10f),
                        contentColor = ObiSilk.Silk,
                        focusedContainerColor = ObiSilk.Silk,
                        focusedContentColor = ObiSilk.Ground,
                    ),
                    scale = ClickableSurfaceDefaults.scale(focusedScale = if (reduced) 1f else 1.05f),
                    modifier = Modifier
                        .size(56.dp)
                        .let { if (i == 0) it.focusRequester(first) else it }
                        .onFocusChanged { if (it.isFocused) focused = i }
                        .semantics { contentDescription = label(context, reaction) },
                ) {
                    Box(Modifier.size(56.dp), contentAlignment = Alignment.Center) {
                        Icon(glyph(reaction), contentDescription = null, modifier = Modifier.size(28.dp))
                    }
                }
            }
        }
        Row(Modifier.padding(top = 6.dp).height(24.dp), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            REACTIONS.forEachIndexed { i, reaction ->
                Box(Modifier.size(56.dp, 24.dp), contentAlignment = Alignment.Center) {
                    if (i == focused) Text(label(context, reaction), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Silk, maxLines = 1)
                }
            }
        }
    }
}
