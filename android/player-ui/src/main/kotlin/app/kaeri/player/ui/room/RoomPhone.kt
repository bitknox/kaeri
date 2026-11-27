// The room on an Android phone, upright: the video on top at 232 dp, then the room's header, its
// line, Carry on when the wait reaches 20 s, the faces, and the chat with its bar. Material 3's
// own controls on solid Raised surfaces; Android draws no glass. Sideways is the full picture with
// the faces strip at the top centre (RoomStrip).

package app.kaeri.player.ui.room

import androidx.compose.animation.core.animateDpAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.material3.TextButton
import androidx.compose.ui.platform.LocalContext
import app.kaeri.i18n.Strings
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.union
import androidx.compose.foundation.layout.ime
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.windowInsetsPadding
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
import app.kaeri.engine.model.MemberState
import app.kaeri.engine.model.RoomCommand
import app.kaeri.engine.model.RoomMember
import app.kaeri.engine.model.RoomView
import app.kaeri.store.room.Ring
import app.kaeri.store.room.RoomLine
import app.kaeri.store.room.RoomInvited
import app.kaeri.store.room.RoomModel
import app.kaeri.store.room.RoomWords
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
    /** Without the video's box: the room beside a sideways picture, which is the video. */
    showVideo: Boolean = true,
    /** The Join screen with a TV driven from this phone: "Join on Living room" hands it the invite. */
    joinOnTv: Pair<String, () -> Unit>? = null,
    /**
     * The Join screen's invite: Join and Not now in the line, no Invite or More in the header, no
     * chat. Only the Join screen passes it, so an invite never takes over the room you are in.
     */
    invited: RoomInvited? = null,
) {
    // Once you leave, the room fades out as it was: its view, people and offer are kept while it does.
    val live = room.view.collectAsStateWithLifecycle().value
    val view = app.kaeri.design.keptWhile(live != null, live)
    val members = app.kaeri.design.keptWhile(live != null, room.members.collectAsStateWithLifecycle().value)
    val offer = app.kaeri.design.keptWhile(live != null, room.offer.collectAsStateWithLifecycle().value)
    val now = rememberRoomNow()
    val c = roomCopy()
    var open by remember(opened) { mutableStateOf(opened) }
    // Hide chat in rooms: the people and the room's own lines stay; what people said, reactions and the field go.
    val context = LocalContext.current
    androidx.compose.runtime.LaunchedEffect(room) { room.loadHideChat(context) }
    val hideChat by room.hideChat.collectAsStateWithLifecycle()
    val shown = view ?: return

    // The Join screen has no chat to keep at the bottom, so it scrolls: over a sideways player a
    // phone is too short for the still, the line, Join and the faces at once.
    val joinScreen = invited != null
    Column(
        modifier.fillMaxSize().background(ObiSilk.Ground)
            .then(if (joinScreen) Modifier.verticalScroll(androidx.compose.foundation.rememberScrollState()) else Modifier),
    ) {
        if (showVideo) {
            Box(Modifier.fillMaxWidth().background(Color.Black).statusBarsPadding().height(RoomVideoHeight)) {
                video(Modifier.fillMaxSize())
                panel?.invoke(Modifier.fillMaxSize())
            }
        } else {
            Spacer(Modifier.statusBarsPadding().height(8.dp))
        }
        RoomHeader(
            view = shown,
            count = members.count { it.state != MemberState.LEFT },
            onBack = onBack,
            // Not joined yet: nothing in the room is yours to invite to, move or leave.
            onInvite = if (invited != null) null else { { open = RoomOpen.Invite } },
            onMore = if (invited != null) null else { { open = RoomOpen.Menu } },
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
            line = RoomLine(inv.by?.let { c.invited(it) } ?: c.JOIN, if (inv.over) c.overCap(inv.names.size) else c.titleEpisode(inv.title, inv.episode))
            buttons = {
                Column {
                    PairButtons(if (inv.over) c.JOIN_ANYWAY else c.JOIN, c.NOT_NOW, { room.join(inv.over) }, { room.notNow(); onBack() })
                    joinOnTv?.let { (tv, join) ->
                        TextButton(onClick = join, modifier = Modifier.padding(top = 4.dp)) {
                            Text(Strings.partyJoinOnDevice(LocalContext.current, tv), color = ObiSilk.Silk, fontSize = 15.sp)
                        }
                    }
                }
            }
        } else if (off != null) {
            line = RoomLine(c.movedTheRoom(off.by), c.goAlong(off.names, RoomWords.clock(positionS)))
            buttons = {
                PairButtons(c.GO_ALONG, c.STAY_HERE, { room.send(RoomCommand.AnswerMove(off.id, true)) }, { room.send(RoomCommand.AnswerMove(off.id, false)) })
            }
        } else {
            line = RoomWords.line(c, shown, members, now, positionS)
            RoomWords.carryLabel(c, shown, now)?.let { label -> buttons = { CarryOnButton(label) { room.send(RoomCommand.CarryOn) } } }
        }
        RoomLineBlock(line, Modifier.padding(start = 16.dp, end = 16.dp, top = 4.dp))
        // The buttons as they last were, so Carry on or Go along fades out drawn, not as nothing.
        val drawn = app.kaeri.design.keptWhile(buttons != null, buttons)
        SwapFade(buttons != null, label = "room-carry") { has -> // i18n-skip
            if (has) Box(Modifier.padding(start = 16.dp, end = 16.dp, top = 12.dp)) { drawn?.invoke() } else Spacer(Modifier.height(0.dp))
        }
        FacesRow(shown, members, now, Modifier.padding(start = 16.dp, end = 16.dp, top = 14.dp)) { open = RoomOpen.Everyone }
        HorizontalDivider(Modifier.padding(start = 16.dp, end = 16.dp, top = 14.dp), color = ObiSilk.FaintRule)
        Box(Modifier.then(if (joinScreen) Modifier else Modifier.weight(1f)).fillMaxWidth().padding(horizontal = 16.dp, vertical = 10.dp), contentAlignment = Alignment.BottomStart) {
            if (inv != null) {
                // No composer below it on the Join screen, so it clears the gesture bar itself.
                Text(c.CHAT_OPENS, Modifier.navigationBarsPadding(), style = TextStyle(fontSize = 14.sp, lineHeight = 20.sp, color = ObiSilk.Haze))
            } else {
                RoomChatList(room, Modifier.fillMaxWidth().then(if (hideChat) Modifier.navigationBarsPadding() else Modifier), roomOnly = hideChat, held = live == null)
            }
        }
        if (inv == null && !hideChat) RoomChatBar(room, open == RoomOpen.React, { open = if (it) RoomOpen.React else RoomOpen.None })
    }

    when (open) {
        RoomOpen.Leave -> LeaveDialog(onLeave = { open = RoomOpen.None; room.send(RoomCommand.Leave); onBack() }, onCancel = { open = RoomOpen.None })
        RoomOpen.Invite -> InviteSheet(shown, onCopy = onCopyLink, onNew = { room.send(RoomCommand.NewInvite) }, onDismiss = { open = RoomOpen.None })
        RoomOpen.Details -> DetailsSheet(shown, onEdit = { open = RoomOpen.EditRelays }, onDismiss = { open = RoomOpen.None })
        RoomOpen.EditRelays -> EditRelaysSheet(shown, onDone = { room.send(RoomCommand.EditRelays(it)); open = RoomOpen.None }, onDismiss = { open = RoomOpen.None })
        RoomOpen.Move -> MoveSheet(shown, positionS, members.firstOrNull { it.you }?.name ?: c.YOU, onMove = { room.send(RoomCommand.MoveRoom(it)); open = RoomOpen.None }, onDismiss = { open = RoomOpen.None })
        RoomOpen.Everyone -> EveryoneSheet(shown, members, now, onDismiss = { open = RoomOpen.None })
        else -> Unit
    }
}

/** The header row, 64 dp: back, the show and "Episode 9 · 3 in the room", Invite, More. */
@Composable
internal fun RoomHeader(view: RoomView, count: Int, onBack: () -> Unit, onInvite: (() -> Unit)?, onMore: (() -> Unit)?, menu: @Composable () -> Unit) {
    val c = roomCopy()
    Row(Modifier.fillMaxWidth().height(64.dp).padding(horizontal = 4.dp), verticalAlignment = Alignment.CenterVertically) {
        IconButton(onClick = onBack, modifier = Modifier.size(48.dp)) {
            Icon(RoomGlyphs.Back, contentDescription = c.CLOSE_TO_BAR, tint = ObiSilk.Silk, modifier = Modifier.size(24.dp))
        }
        Column(Modifier.weight(1f).padding(start = 4.dp)) {
            Text(view.roomEpisode?.title ?: view.name, style = ShowTitle.row.copy(lineHeight = 22.sp), color = ObiSilk.Silk, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(
                c.inTheRoom(view.episode?.episode ?: "", count),
                style = TextStyle(fontSize = 14.sp, lineHeight = 20.sp).tabular(),
                color = ObiSilk.Haze,
                maxLines = 1,
            )
        }
        if (onInvite != null) IconButton(onClick = onInvite, modifier = Modifier.size(48.dp)) {
            Icon(RoomGlyphs.Invite, contentDescription = c.INVITE, tint = ObiSilk.Silk, modifier = Modifier.size(24.dp))
        }
        if (onMore != null) Box {
            IconButton(onClick = onMore, modifier = Modifier.size(48.dp)) {
                Icon(RoomGlyphs.More, contentDescription = c.MORE, tint = ObiSilk.Silk, modifier = Modifier.size(24.dp))
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
    val c = roomCopy()
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
        item(RoomGlyphs.Leave, c.LEAVE_MORE, onLeave)
        item(RoomGlyphs.Copy, c.COPY_INVITE, onCopy)
        item(RoomGlyphs.People, c.MOVE_MORE, onMove)
        item(RoomGlyphs.Info, c.DETAILS, onDetails)
        item(RoomGlyphs.Captions, c.SUBTITLES_AUDIO, onSubtitles)
    }
}

/**
 * The room's line: 22/500 over a 14 haze detail, read out when its words change. Its clock and
 * seconds tick every second, so what is said holds while only they move.
 */
@Composable
fun RoomLineBlock(line: RoomLine, modifier: Modifier = Modifier, titleSize: Int = 22, titleLine: Int = 28) {
    val words = line.title + ". " + line.detail // i18n-skip
    val spoken = remember(line.key) { words }
    Column(modifier.fillMaxWidth().clearAndSetSemantics { contentDescription = spoken; liveRegion = LiveRegionMode.Polite }) {
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
    val c = roomCopy()
    val (shown, hidden) = RoomWords.capped(members.filter { it.state != MemberState.LEFT || it.sinceMs?.let { s -> now - s < LEFT_KEPT_MS } != false })
    // Named as a group for TalkBack: "People in the room", then each face.
    Row(modifier.fillMaxWidth().semantics { contentDescription = c.PEOPLE }, horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.Top) {
        for (m in shown) {
            val name = if (m.you) c.YOU else m.name
            val word = RoomWords.word(c, view, m, now)
            Column(
                Modifier.width(88.dp).clearAndSetSemantics { contentDescription = c.spoken(name, word) },
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                PersonFace(view, m, now, 48.dp)
                Text(name, fontSize = 14.sp, fontWeight = FontWeight.Medium, color = ObiSilk.Silk, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 4.dp))
                Text(word, style = TextStyle(fontSize = 12.sp, lineHeight = 16.sp, textAlign = TextAlign.Center).tabular(), color = ObiSilk.Haze, maxLines = 2)
            }
        }
        if (hidden.isNotEmpty()) {
            val word = RoomWords.moreWord(c, hidden)
            Column(
                Modifier.width(88.dp).clip(androidx.compose.foundation.shape.RoundedCornerShape(12.dp)).clickable(onClick = onMore)
                    .clearAndSetSemantics { contentDescription = c.spoken(c.MORE, word) },
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                MoreFace(c.plus(hidden.size), 48.dp)
                Text(c.MORE, fontSize = 14.sp, fontWeight = FontWeight.Medium, color = ObiSilk.Silk, modifier = Modifier.padding(top = 4.dp))
                Text(word, fontSize = 12.sp, lineHeight = 16.sp, color = ObiSilk.Haze, textAlign = TextAlign.Center)
            }
        }
    }
}

/** How long a Left face stays before it goes. */
const val LEFT_KEPT_MS = 4000L

/** The chat bar on the side colour: React, the field, Send. */
@Composable
internal fun RoomChatBar(room: RoomModel, reacting: Boolean, onReacting: (Boolean) -> Unit) {
    val context = androidx.compose.ui.platform.LocalContext.current
    val c = roomCopy()
    Row(
        // The app draws edge to edge, so the window doesn't resize for the keyboard: the bar
        // rises above it, or clears the gesture bar when it's down.
        Modifier.fillMaxWidth().background(ObiSilk.RaisedLow)
            .windowInsetsPadding(androidx.compose.foundation.layout.WindowInsets.navigationBars.union(androidx.compose.foundation.layout.WindowInsets.ime))
            .padding(start = 12.dp, end = 12.dp, top = 8.dp, bottom = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Box {
            IconButton(onClick = { onReacting(true) }, modifier = Modifier.size(48.dp)) {
                Icon(reactionGlyph(REACTIONS.first()), contentDescription = c.REACT, tint = ObiSilk.Silk, modifier = Modifier.size(24.dp))
            }
            DropdownMenu(reacting, { onReacting(false) }, containerColor = ObiSilk.RaisedHigh) {
                Row {
                    for (r in REACTIONS) {
                        IconButton(onClick = { room.send(RoomCommand.React(r)); onReacting(false) }, modifier = Modifier.size(48.dp)) {
                            Icon(reactionGlyph(r), contentDescription = reactionLabel(context, r), tint = ObiSilk.Silk, modifier = Modifier.size(24.dp))
                        }
                    }
                }
            }
        }
        RoomChatField(room, Modifier.weight(1f))
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
    /** The Join screen's invite, as [RoomUpright] takes it. */
    invited: RoomInvited? = null,
) {
    SwapFade(full, modifier.fillMaxSize(), label = "room-full") { sideways -> // i18n-skip
        if (sideways) {
            RoomSideways(room, positionS, video, controls = controls)
        } else {
            RoomUpright(room, positionS, video, panel = panel, onBack = onBack, onSubtitles = onSubtitles, onCopyLink = onCopyLink, opened = opened, invited = invited)
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

/**
 * The player in the room, on a phone: upright, the player sits in the room's video box over the
 * room; sideways ([sideways]), it fills the screen with the strip over it. [player] is composed
 * in one place whatever the shape, so its video surface lives on through a turn, and the box
 * grows and shrinks between the two (the morph) as the room comes and goes. With Remove
 * animations the box takes its size at once and the room crossfades.
 */
@Composable
fun RoomAround(
    room: RoomModel,
    inRoom: Boolean,
    sideways: Boolean,
    positionS: Double,
    controls: Boolean,
    player: @Composable (Modifier) -> Unit,
    modifier: Modifier = Modifier,
    panel: (@Composable (Modifier) -> Unit)? = null,
    onBack: () -> Unit = {},
    onSubtitles: () -> Unit = {},
    onCopyLink: () -> Unit = {},
    /** Sideways, the room beside the picture: the chrome's Watch together button opens it. */
    roomOpen: Boolean = false,
    onRoomClose: () -> Unit = {},
) {
    val removed = rememberAnimationsRemoved()
    val upright = inRoom && !sideways
    BoxWithConstraints(modifier.fillMaxSize().background(if (upright) ObiSilk.Ground else Color.Black)) {
        // Still a crossfade with Remove animations, at full speed: the room and the picture both drawn.
        val roomAlpha = app.kaeri.design.animateKeptAlpha(
            upright,
            if (removed) app.kaeri.design.Motion.fadeIn() else app.kaeri.design.Motion.standard(),
        )
        if (upright || roomAlpha > 0f) {
            Box(Modifier.fillMaxSize().graphicsAlpha(roomAlpha)) {
                RoomUpright(room, positionS, video = { Box(it) }, onBack = onBack, onSubtitles = onSubtitles, onCopyLink = onCopyLink)
            }
        }
        val target = if (upright) RoomVideoHeight else maxHeight
        val height = if (removed) target else animateDpAsState(target, app.kaeri.design.Motion.standard(), label = "room-video").value // i18n-skip
        Box(
            Modifier.fillMaxWidth()
                .then(if (upright) Modifier.statusBarsPadding() else Modifier)
                .height(height)
                .background(Color.Black),
        ) {
            player(Modifier.fillMaxSize())
            // Before play, No source and your own loading lie over the video sideways too.
            if (inRoom) panel?.invoke(Modifier.fillMaxSize())
        }
        if (inRoom && sideways) RoomStripOver(room, positionS, controls, Modifier.align(Alignment.TopCenter))
        // Sideways, every room control is one tap away: Watch together in the chrome opens the
        // upright room, less its video, at the end edge over the picture; Back or a tap on the
        // picture closes it.
        val side = inRoom && sideways && roomOpen
        androidx.compose.animation.AnimatedVisibility(
            side,
            modifier = Modifier.fillMaxSize(),
            enter = androidx.compose.animation.fadeIn(app.kaeri.design.Motion.fadeIn()),
            exit = androidx.compose.animation.fadeOut(app.kaeri.design.Motion.fadeOut()),
        ) {
            androidx.activity.compose.BackHandler(onBack = onRoomClose)
            Box(Modifier.fillMaxSize()) {
                Box(
                    Modifier.fillMaxSize()
                        .background(Color.Black.copy(alpha = 0.4f))
                        .clickable(interactionSource = null, indication = null, onClickLabel = null, onClick = onRoomClose),
                )
                RoomUpright(
                    room,
                    positionS,
                    video = {},
                    showVideo = false,
                    onBack = onRoomClose,
                    onSubtitles = onSubtitles,
                    onCopyLink = onCopyLink,
                    modifier = Modifier.align(Alignment.CenterEnd)
                        .width(minOf(RoomSideWidth, maxWidth * 0.5f))
                        .windowInsetsPadding(androidx.compose.foundation.layout.WindowInsets.navigationBars.only(androidx.compose.foundation.layout.WindowInsetsSides.End))
                        .windowInsetsPadding(androidx.compose.foundation.layout.WindowInsets.displayCutout.only(androidx.compose.foundation.layout.WindowInsetsSides.End)),
                )
            }
        }
    }
}

/** The room beside a sideways picture: as wide as an upright phone's room. */
val RoomSideWidth = 412.dp

/**
 * A room an invite opened, before joining, as its own upright screen over the app: the still,
 * who invited, what it watches and who is there, then Join and Not now.
 */
@Composable
fun RoomInvitedScreen(room: RoomModel, invited: RoomInvited, onClose: () -> Unit, joinOnTv: Pair<String, () -> Unit>? = null) {
    // A model of its own shows the glimpse as the room; Join goes out through the real one.
    val glimpse = remember(room) { RoomModel { room.send(it) } }
    androidx.compose.runtime.SideEffect {
        if (glimpse.invited.value != invited) glimpse.show(invited.asView(), invited.glimpse?.members.orEmpty(), invited)
    }
    val art = invited.glimpse?.roomEpisode?.let { it.bannerUrl ?: it.coverUrl }
    RoomUpright(
        glimpse,
        positionS = 0.0,
        video = { m -> app.kaeri.ui.ArtworkImage(art, ObiSilk.Raised, app.kaeri.ui.pixels(RoomVideoHeight * 2), m) },
        onBack = onClose,
        joinOnTv = joinOnTv,
        invited = invited,
    )
}
