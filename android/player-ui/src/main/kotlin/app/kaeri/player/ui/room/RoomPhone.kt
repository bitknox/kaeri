// The room on an Android phone, upright: the video on top at 232 dp, then the room's header, its
// line, Carry on when the wait reaches 20 s, the faces, and the chat with its bar. Material 3's
// own controls on solid Raised surfaces; Android draws no glass. Sideways is the full picture with
// the faces strip at the top centre (RoomStrip).

package app.kaeri.player.ui.room

import androidx.compose.animation.core.animateDpAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MenuDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.design.FaceRing
import app.kaeri.design.MoreFace
import app.kaeri.design.ObiSilk
import app.kaeri.design.RoomFace
import app.kaeri.design.RoomGlyphs
import app.kaeri.design.ShowTitle
import app.kaeri.design.SwapFade
import app.kaeri.design.rememberAnimationsRemoved
import app.kaeri.design.rememberRoomNow
import app.kaeri.design.tabular
import app.kaeri.store.room.MemberState
import app.kaeri.store.room.Ring
import app.kaeri.store.room.RoomAsk
import app.kaeri.store.room.RoomCopy
import app.kaeri.store.room.RoomLine
import app.kaeri.store.room.RoomMember
import app.kaeri.store.room.RoomModel
import app.kaeri.store.room.RoomView
import app.kaeri.store.room.RoomWords
import app.kaeri.ui.ChatConversation
import app.kaeri.ui.ChatField
import app.kaeri.ui.LocalAppStore
import app.kaeri.ui.REACTIONS
import app.kaeri.ui.reactionGlyph
import app.kaeri.ui.reactionLabel

/** The video's height when the room is upright. */
val RoomVideoHeight = 232.dp

internal fun Ring.face(): FaceRing = when (this) {
    Ring.Water -> FaceRing.Water
    Ring.Haze -> FaceRing.Haze
    Ring.Arc -> FaceRing.Arc
    Ring.None -> FaceRing.None
}

/** One person's face as the room sees them now. */
@Composable
fun PersonFace(view: RoomView, member: RoomMember, now: Long, disc: Dp, modifier: Modifier = Modifier, ground: Color = ObiSilk.Ground) {
    val look = RoomWords.look(view, member, now)
    RoomFace(
        initial = member.name,
        disc = disc,
        ring = look.ring.face(),
        modifier = modifier,
        arc = look.arc,
        dim = look.dim,
        badge = look.badge,
        error = look.error,
        ground = ground,
    )
}

/** Which sheet or dialog the room has open over it. */
enum class RoomOpen { None, Menu, Leave, Invite, Details, EditRelays, Move, Everyone, React }

/**
 * The upright room under the video. [video] draws the picture into the box it is given; [panel]
 * lays a panel over the video (before play, invited, no source, choosing) when there is one.
 * [positionS] is the room's place, for the line. [full] turns the phone sideways (or, under
 * rotation lock, the full screen button asks to).
 */
@Composable
fun RoomUpright(
    room: RoomModel,
    positionS: Double,
    video: @Composable (Modifier) -> Unit,
    modifier: Modifier = Modifier,
    panel: (@Composable (Modifier) -> Unit)? = null,
    onBack: () -> Unit = {},
    onSubtitles: () -> Unit = {},
    onCopyLink: () -> Unit = {},
    opened: RoomOpen = RoomOpen.None,
) {
    val view by room.view.collectAsStateWithLifecycle()
    val members by room.members.collectAsStateWithLifecycle()
    val invited by room.invited.collectAsStateWithLifecycle()
    val offer by room.offer.collectAsStateWithLifecycle()
    val moved by room.moved.collectAsStateWithLifecycle()
    val carried by room.carried.collectAsStateWithLifecycle()
    val now = rememberRoomNow()
    var open by remember(opened) { mutableStateOf(opened) }
    val shown = view ?: return

    Column(modifier.fillMaxSize().background(ObiSilk.Ground)) {
        Box(Modifier.fillMaxWidth().background(Color.Black).statusBarsPadding().height(RoomVideoHeight)) {
            video(Modifier.fillMaxSize())
            panel?.invoke(Modifier.fillMaxSize())
        }
        RoomHeader(
            view = shown,
            count = members.count { it.state != MemberState.Left },
            onBack = onBack,
            onInvite = { open = RoomOpen.Invite },
            onMore = { open = RoomOpen.Menu },
            menu = {
                RoomMenu(
                    expanded = open == RoomOpen.Menu,
                    onDismiss = { open = RoomOpen.None },
                    onLeave = { open = RoomOpen.Leave },
                    onCopy = { open = RoomOpen.None; onCopyLink() },
                    onMove = { open = RoomOpen.Move },
                    onDetails = { open = RoomOpen.Details },
                    onSubtitles = { open = RoomOpen.None; onSubtitles() },
                )
            },
        )
        // The line, and what it brings forward: Carry on at 20 s, Join / Not now, Go along / Stay here.
        val line: RoomLine
        var buttons: (@Composable () -> Unit)? = null
        val inv = invited
        val off = offer
        if (inv != null) {
            line = RoomLine(RoomCopy.invited(inv.by), if (inv.over) RoomCopy.overCap(inv.names.size) else RoomCopy.titleEpisode(inv.title, inv.episode))
            buttons = {
                PairButtons(if (inv.over) RoomCopy.JOIN_ANYWAY else RoomCopy.JOIN, RoomCopy.NOT_NOW, { room.ask(RoomAsk.Join(inv.over)) }, { room.ask(RoomAsk.NotNow) })
            }
        } else if (off != null) {
            line = RoomLine(RoomCopy.movedTheRoom(off.by), RoomCopy.goAlong(off.names, RoomWords.clock(positionS)))
            buttons = {
                PairButtons(RoomCopy.GO_ALONG, RoomCopy.STAY_HERE, { room.ask(RoomAsk.AnswerMove(off.id, true)) }, { room.ask(RoomAsk.AnswerMove(off.id, false)) })
            }
        } else if (moved != null) {
            line = RoomLine(RoomCopy.movedAway(moved!!.by), RoomCopy.wentAlong(moved!!.went))
        } else {
            line = RoomWords.line(shown, members, now, positionS, carried?.let { it.by to it.past })
            RoomWords.carryLabel(shown, now)?.let { label -> buttons = { CarryOnButton(label) { room.ask(RoomAsk.CarryOn) } } }
        }
        RoomLineBlock(line, Modifier.padding(start = 16.dp, end = 16.dp, top = 4.dp))
        SwapFade(buttons != null, label = "room-carry") { has -> // i18n-skip
            if (has) Box(Modifier.padding(start = 16.dp, end = 16.dp, top = 12.dp)) { buttons?.invoke() } else Spacer(Modifier.height(0.dp))
        }
        FacesRow(shown, members, now, Modifier.padding(start = 16.dp, end = 16.dp, top = 14.dp)) { open = RoomOpen.Everyone }
        HorizontalDivider(Modifier.padding(start = 16.dp, end = 16.dp, top = 14.dp), color = ObiSilk.FaintRule)
        Box(Modifier.weight(1f).fillMaxWidth().padding(horizontal = 16.dp, vertical = 10.dp), contentAlignment = Alignment.BottomStart) {
            if (inv != null) {
                Text(RoomCopy.CHAT_OPENS, style = TextStyle(fontSize = 14.sp, lineHeight = 20.sp, color = ObiSilk.Haze))
            } else {
                ChatConversation(Modifier.fillMaxWidth())
            }
        }
        if (inv == null) RoomChatBar(open == RoomOpen.React, { open = if (it) RoomOpen.React else RoomOpen.None })
    }

    when (open) {
        RoomOpen.Leave -> LeaveDialog(onLeave = { open = RoomOpen.None; room.ask(RoomAsk.Leave) }, onCancel = { open = RoomOpen.None })
        RoomOpen.Invite -> InviteSheet(shown, onCopy = onCopyLink, onNew = { room.ask(RoomAsk.NewInvite) }, onDismiss = { open = RoomOpen.None })
        RoomOpen.Details -> DetailsSheet(shown, onEdit = { open = RoomOpen.EditRelays }, onDismiss = { open = RoomOpen.None })
        RoomOpen.EditRelays -> EditRelaysSheet(shown, onDone = { room.ask(RoomAsk.SetRelays(it)); open = RoomOpen.None }, onDismiss = { open = RoomOpen.None })
        RoomOpen.Move -> MoveSheet(shown, positionS, members.firstOrNull { it.you }?.name ?: RoomCopy.YOU, onMove = { room.ask(RoomAsk.MoveRoom(it)); open = RoomOpen.None }, onDismiss = { open = RoomOpen.None })
        RoomOpen.Everyone -> EveryoneSheet(shown, members, now, onDismiss = { open = RoomOpen.None })
        else -> Unit
    }
}

/** The header row, 64 dp: back, the show and "Episode 9 · 3 in the room", Invite, More. */
@Composable
internal fun RoomHeader(view: RoomView, count: Int, onBack: () -> Unit, onInvite: () -> Unit, onMore: () -> Unit, menu: @Composable () -> Unit) {
    Row(Modifier.fillMaxWidth().height(64.dp).padding(horizontal = 4.dp), verticalAlignment = Alignment.CenterVertically) {
        IconButton(onClick = onBack, modifier = Modifier.size(48.dp)) {
            Icon(RoomGlyphs.Back, contentDescription = RoomCopy.CLOSE_TO_BAR, tint = ObiSilk.Silk, modifier = Modifier.size(24.dp))
        }
        Column(Modifier.weight(1f).padding(start = 4.dp)) {
            Text(view.title, style = ShowTitle.row.copy(lineHeight = 22.sp), color = ObiSilk.Silk, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(
                RoomCopy.inTheRoom(view.episode?.episode ?: "", count),
                style = TextStyle(fontSize = 14.sp, lineHeight = 20.sp).tabular(),
                color = ObiSilk.Haze,
                maxLines = 1,
            )
        }
        IconButton(onClick = onInvite, modifier = Modifier.size(48.dp)) {
            Icon(RoomGlyphs.Invite, contentDescription = RoomCopy.INVITE, tint = ObiSilk.Silk, modifier = Modifier.size(24.dp))
        }
        Box {
            IconButton(onClick = onMore, modifier = Modifier.size(48.dp)) {
                Icon(RoomGlyphs.More, contentDescription = RoomCopy.MORE, tint = ObiSilk.Silk, modifier = Modifier.size(24.dp))
            }
            menu()
        }
    }
}

/** The ⋮ menu: Leave the room…, Copy invite link, Move to a new room…, Details, Subtitles and audio. */
@Composable
internal fun RoomMenu(
    expanded: Boolean,
    onDismiss: () -> Unit,
    onLeave: () -> Unit,
    onCopy: () -> Unit,
    onMove: () -> Unit,
    onDetails: () -> Unit,
    onSubtitles: () -> Unit,
) {
    DropdownMenu(expanded, onDismiss, containerColor = ObiSilk.RaisedHigh, modifier = Modifier.width(240.dp)) {
        val colors = MenuDefaults.itemColors(textColor = ObiSilk.Silk, leadingIconColor = ObiSilk.Silk)
        @Composable
        fun item(icon: ImageVector, text: String, onClick: () -> Unit) = DropdownMenuItem(
            text = { Text(text, fontSize = 15.sp) },
            onClick = onClick,
            leadingIcon = { Icon(icon, contentDescription = null, modifier = Modifier.size(22.dp)) },
            colors = colors,
            modifier = Modifier.height(48.dp),
        )
        item(RoomGlyphs.Leave, RoomCopy.LEAVE_MORE, onLeave)
        item(RoomGlyphs.Copy, RoomCopy.COPY_INVITE, onCopy)
        item(RoomGlyphs.People, RoomCopy.MOVE_MORE, onMove)
        item(RoomGlyphs.Info, RoomCopy.DETAILS, onDetails)
        item(RoomGlyphs.Captions, RoomCopy.SUBTITLES_AUDIO, onSubtitles)
    }
}

/** The room's line: 22/500 over a 14 haze detail, read out as it changes. */
@Composable
fun RoomLineBlock(line: RoomLine, modifier: Modifier = Modifier, titleSize: Int = 22, titleLine: Int = 28) {
    Column(modifier.fillMaxWidth().semantics(mergeDescendants = true) { liveRegion = LiveRegionMode.Polite }) {
        Text(line.title, style = TextStyle(fontSize = titleSize.sp, lineHeight = titleLine.sp, fontWeight = FontWeight.Medium), color = ObiSilk.Silk)
        Text(line.detail, style = TextStyle(fontSize = 14.sp, lineHeight = 20.sp).tabular(), color = ObiSilk.Haze)
    }
}

/** Carry on: the amber pill, 44 high. */
@Composable
fun CarryOnButton(label: String, modifier: Modifier = Modifier, height: Dp = 44.dp, onClick: () -> Unit) {
    Button(
        onClick = onClick,
        modifier = modifier.height(height),
        shape = CircleShape,
        colors = ButtonDefaults.buttonColors(containerColor = ObiSilk.Amber, contentColor = ObiSilk.Ground),
        contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 20.dp),
    ) { Text(label, fontSize = 15.sp, fontWeight = FontWeight.Medium, maxLines = 1) }
}

/** A filled pair: the amber action and a tonal one beside it. */
@Composable
internal fun PairButtons(first: String, second: String, onFirst: () -> Unit, onSecond: () -> Unit) {
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        CarryOnButton(first, onClick = onFirst)
        FilledTonalButton(
            onClick = onSecond,
            modifier = Modifier.height(44.dp),
            shape = CircleShape,
            colors = ButtonDefaults.filledTonalButtonColors(containerColor = ObiSilk.RaisedSelected, contentColor = ObiSilk.Silk),
            contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 20.dp),
        ) { Text(second, fontSize = 15.sp, fontWeight = FontWeight.Medium, maxLines = 1) }
    }
}

/**
 * The faces, 48 dp discs in 88 dp columns, each with its name and its one word: up to four, then
 * three and More with "All watching" or "2 not watching", which opens everyone.
 */
@Composable
fun FacesRow(view: RoomView, members: List<RoomMember>, now: Long, modifier: Modifier = Modifier, onMore: () -> Unit) {
    val (shown, hidden) = RoomWords.capped(members.filter { it.state != MemberState.Left || it.sinceMs?.let { s -> now - s < LEFT_KEPT_MS } != false })
    Row(modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.Top) {
        for (m in shown) {
            val name = if (m.you) RoomCopy.YOU else m.name
            val word = RoomWords.word(view, m, now)
            Column(
                Modifier.width(88.dp).clearAndSetSemantics { contentDescription = RoomCopy.spoken(name, word) },
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                PersonFace(view, m, now, 48.dp)
                Text(name, fontSize = 14.sp, fontWeight = FontWeight.Medium, color = ObiSilk.Silk, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 4.dp))
                Text(word, style = TextStyle(fontSize = 12.sp, lineHeight = 16.sp, textAlign = TextAlign.Center).tabular(), color = ObiSilk.Haze, maxLines = 2)
            }
        }
        if (hidden.isNotEmpty()) {
            val word = RoomWords.moreWord(hidden)
            Column(
                Modifier.width(88.dp).clip(androidx.compose.foundation.shape.RoundedCornerShape(12.dp)).clickable(onClick = onMore)
                    .clearAndSetSemantics { contentDescription = RoomCopy.spoken(RoomCopy.MORE, word) },
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                MoreFace(RoomCopy.plus(hidden.size), 48.dp)
                Text(RoomCopy.MORE, fontSize = 14.sp, fontWeight = FontWeight.Medium, color = ObiSilk.Silk, modifier = Modifier.padding(top = 4.dp))
                Text(word, fontSize = 12.sp, lineHeight = 16.sp, color = ObiSilk.Haze, textAlign = TextAlign.Center)
            }
        }
    }
}

/** How long a Left face stays before it goes. */
const val LEFT_KEPT_MS = 4000L

/** The chat bar on the side colour: React, the field, Send. */
@Composable
internal fun RoomChatBar(reacting: Boolean, onReacting: (Boolean) -> Unit) {
    val store = LocalAppStore.current
    val context = androidx.compose.ui.platform.LocalContext.current
    Row(
        Modifier.fillMaxWidth().background(ObiSilk.RaisedLow).navigationBarsPadding().padding(start = 12.dp, end = 12.dp, top = 8.dp, bottom = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Box {
            IconButton(onClick = { onReacting(true) }, modifier = Modifier.size(48.dp)) {
                Icon(reactionGlyph(REACTIONS.first()), contentDescription = RoomCopy.REACT, tint = ObiSilk.Silk, modifier = Modifier.size(24.dp))
            }
            DropdownMenu(reacting, { onReacting(false) }, containerColor = ObiSilk.RaisedHigh) {
                Row {
                    for (r in REACTIONS) {
                        IconButton(onClick = { store.partyReact(r); onReacting(false) }, modifier = Modifier.size(48.dp)) {
                            Icon(reactionGlyph(r), contentDescription = reactionLabel(context, r), tint = ObiSilk.Silk, modifier = Modifier.size(24.dp))
                        }
                    }
                }
            }
        }
        ChatField(Modifier.weight(1f), height = 48.dp, fieldColor = ObiSilk.RaisedHigh, sendColor = ObiSilk.RaisedSelected)
    }
}

/**
 * The room upright and sideways as one screen. Turning to [full] grows the video to the whole
 * screen while the room's panel fades; with Remove animations it crossfades instead, with no
 * slide or zoom.
 */
@Composable
fun RoomPhone(
    room: RoomModel,
    full: Boolean,
    positionS: Double,
    video: @Composable (Modifier) -> Unit,
    modifier: Modifier = Modifier,
    controls: Boolean = true,
    panel: (@Composable (Modifier) -> Unit)? = null,
    onBack: () -> Unit = {},
    onSubtitles: () -> Unit = {},
    onCopyLink: () -> Unit = {},
    opened: RoomOpen = RoomOpen.None,
) {
    SwapFade(full, modifier.fillMaxSize(), label = "room-full") { sideways -> // i18n-skip
        if (sideways) {
            RoomSideways(room, positionS, video, controls = controls)
        } else {
            RoomUpright(room, positionS, video, panel = panel, onBack = onBack, onSubtitles = onSubtitles, onCopyLink = onCopyLink, opened = opened)
        }
    }
}

/**
 * Player to room: the full picture shrinks to the upright video's place while the room fades in
 * under it. [progress] runs 0 (the player) to 1 (the room). With Remove animations the two
 * crossfade in place.
 */
@Composable
fun RoomMorph(room: RoomModel, inRoom: Boolean, positionS: Double, video: @Composable (Modifier) -> Unit, modifier: Modifier = Modifier) {
    val removed = rememberAnimationsRemoved()
    if (removed) {
        SwapFade(inRoom, modifier.fillMaxSize(), label = "room-morph") { here -> // i18n-skip
            if (here) RoomUpright(room, positionS, video) else Box(Modifier.fillMaxSize().background(Color.Black)) { video(Modifier.fillMaxSize()) }
        }
        return
    }
    BoxWithConstraints(modifier.fillMaxSize().background(ObiSilk.Ground)) {
        val height by animateDpAsState(if (inRoom) RoomVideoHeight else maxHeight, app.kaeri.design.Motion.standard(), label = "morph") // i18n-skip
        val alpha by androidx.compose.animation.core.animateFloatAsState(if (inRoom) 1f else 0f, app.kaeri.design.Motion.standard(), label = "morph-room") // i18n-skip
        Box(Modifier.fillMaxSize().graphicsAlpha(alpha)) { RoomUpright(room, positionS, video = { Box(it) }) }
        Box(Modifier.fillMaxWidth().height(height).background(Color.Black)) { video(Modifier.fillMaxSize()) }
    }
}

private fun Modifier.graphicsAlpha(a: Float) = this.graphicsLayer { alpha = a }
