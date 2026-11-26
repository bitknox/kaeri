// The room on Android TV. The full picture comes first, with the faces strip top right; Down opens
// the people rail beside a 640 × 360 video, with the line, the faces, the chat read-only and
// "Write from your phone". Up reaches Carry on once the wait reaches 20 s; focus never moves on
// its own. Back closes the rail, then asks "Leave the room?" with Leave and Keep watching.

package app.kaeri.tv.player.room

import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.Text
import app.kaeri.design.FaceRing
import app.kaeri.design.KeptFade
import app.kaeri.design.MoreFace
import app.kaeri.design.Motion
import app.kaeri.design.ObiSilk
import app.kaeri.design.RoomFace
import app.kaeri.design.ShowTitle
import app.kaeri.design.SwapFade
import app.kaeri.design.rememberAnimationsRemoved
import app.kaeri.design.rememberRoomNow
import app.kaeri.design.tabular
import app.kaeri.engine.model.MemberState
import app.kaeri.engine.model.RoomChatBody
import app.kaeri.engine.model.RoomCommand
import app.kaeri.engine.model.RoomMember
import app.kaeri.engine.model.RoomView
import app.kaeri.store.room.Ring
import app.kaeri.store.room.RoomCopy
import app.kaeri.store.room.RoomLine
import app.kaeri.store.room.RoomModel
import app.kaeri.store.room.RoomWords
import app.kaeri.tv.ui.AmberButton
import app.kaeri.tv.ui.LocalAppStore
import app.kaeri.tv.ui.TonalButton
import app.kaeri.tv.ui.TvIcons

private fun Ring.face(): FaceRing = when (this) {
    Ring.Water -> FaceRing.Water
    Ring.Haze -> FaceRing.Haze
    Ring.Arc -> FaceRing.Arc
    Ring.None -> FaceRing.None
}

@Composable
private fun TvFace(view: RoomView, member: RoomMember, now: Long, disc: Dp, ground: Color) {
    val look = RoomWords.look(view, member, now)
    RoomFace(member.name, disc, look.ring.face(), arc = look.arc, dim = look.dim, badge = look.badge, error = look.error, ground = ground)
}

/** What the TV room shows. */
enum class TvRoomMode { Full, Rail, Leave }

/**
 * Where the TV room is, held by the player so its keys reach it: the player's root keeps focus
 * while the controls are hidden, so Down, Up and Back come to [down], [up] and [back] from there.
 */
@Stable
class TvRoomState(start: TvRoomMode = TvRoomMode.Full) {
    var mode by mutableStateOf(start)

    /** What shows under the Leave dialog, and what Back from it returns to. */
    var under by mutableStateOf(if (start == TvRoomMode.Rail) TvRoomMode.Rail else TvRoomMode.Full)
    val carryFocus = FocusRequester()

    /** Whether Carry on shows now, so Up reaches it. */
    var carry by mutableStateOf(false)

    /** Whether focus is on the room's own buttons: the player leaves it there. */
    var holdsFocus by mutableStateOf(false)

    val rail: Boolean get() = (if (mode == TvRoomMode.Leave) under else mode) == TvRoomMode.Rail

    /** Down on the full picture opens the people rail. */
    fun down(): Boolean {
        if (mode != TvRoomMode.Full) return false
        mode = TvRoomMode.Rail
        under = TvRoomMode.Rail
        return true
    }

    /** Back to the full picture, as OK on the rail does to bring the controls up. */
    fun full() {
        mode = TvRoomMode.Full
        under = TvRoomMode.Full
    }

    /** Up reaches Carry on when there is one. */
    fun up(): Boolean = carry && runCatching { carryFocus.requestFocus() }.isSuccess

    /** Back: the rail goes back to the picture, the picture asks to leave, the dialog goes. */
    fun back() {
        when (mode) {
            TvRoomMode.Rail -> { mode = TvRoomMode.Full; under = TvRoomMode.Full }
            TvRoomMode.Full -> { under = TvRoomMode.Full; mode = TvRoomMode.Leave }
            TvRoomMode.Leave -> mode = under
        }
    }
}

/**
 * The TV room around the player's picture. [state] says where it is; the player sends it Down,
 * Up and Back. [video] draws the picture into the box it is given, which moves between the full
 * screen and the rail, so it is drawn once.
 */
@Composable
fun TvRoom(
    room: RoomModel,
    positionS: Double,
    episodeLine: String,
    durationS: Double,
    video: @Composable (Modifier) -> Unit,
    modifier: Modifier = Modifier,
    start: TvRoomMode = TvRoomMode.Full,
    state: TvRoomState = remember(start) { TvRoomState(start) },
    focusCarry: Boolean = false,
    forceStrip: Boolean = false,
    stripTop: Dp = 27.dp,
    onLeft: () -> Unit = {},
) {
    val view by room.view.collectAsStateWithLifecycle()
    val members by room.members.collectAsStateWithLifecycle()
    val offer by room.offer.collectAsStateWithLifecycle()
    val now = rememberRoomNow()
    val c = tvRoomCopy()
    val v = view ?: return
    val carry = RoomWords.carryLabel(c, v, now)
    val carryFocus = state.carryFocus
    SideEffect { state.carry = carry != null }
    LaunchedEffect(focusCarry, carry != null) { if (focusCarry && carry != null) runCatching { carryFocus.requestFocus() } }
    val mode = state.mode

    BoxWithConstraints(modifier.fillMaxSize().background(ObiSilk.Ground).onFocusChanged { state.holdsFocus = it.hasFocus }) {
        // One video box, drawn once, that moves between the full screen and the rail's 640 x 360,
        // so the picture's surface lives on; the rail's words crossfade around it.
        val removed = rememberAnimationsRemoved()
        val rail = state.rail
        val railAlpha by animateFloatAsState(if (rail) 1f else 0f, Motion.fadeIn(), label = "tv-rail") // i18n-skip
        if (rail || railAlpha > 0f) {
            Box(Modifier.fillMaxSize().graphicsLayer { alpha = railAlpha }) {
                TvRail(v, members, now, positionS, durationS, episodeLine, carry, carryFocus) { room.send(RoomCommand.CarryOn) }
            }
        }
        @Composable
        fun moving(target: Dp, label: String): Dp =
            if (removed) target else animateDpAsState(target, Motion.standard(), label = label).value
        val x = moving(if (rail) 48.dp else 0.dp, "tv-video-x") // i18n-skip
        val y = moving(if (rail) 27.dp else 0.dp, "tv-video-y") // i18n-skip
        val w = moving(if (rail) 640.dp else maxWidth, "tv-video-w") // i18n-skip
        val h = moving(if (rail) 360.dp else maxHeight, "tv-video-h") // i18n-skip
        val corner = moving(if (rail) 12.dp else 0.dp, "tv-video-corner") // i18n-skip
        Box(Modifier.offset(x, y).size(w, h).clip(RoundedCornerShape(corner)).background(Color.Black)) {
            video(Modifier.fillMaxSize())
            if (rail && v.beforePlay) {
                TvBeforePlay(v.roomEpisode?.title ?: v.name, episodeLine, started = v.startsAtMs != null || v.waiting.isNotEmpty()) { room.send(RoomCommand.Play(true)) }
            }
        }
        val top = moving(stripTop, "tv-strip-top") // i18n-skip
        if (!rail) {
            val o = offer
            val wanted = forceStrip || RoomWords.stripWanted(v, members) || o != null || carry != null
            KeptFade(if (wanted) v else null, Modifier.align(Alignment.TopEnd).padding(top = top, end = 48.dp)) { sv ->
                Column(horizontalAlignment = Alignment.End, verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    TvStrip(sv, members, now, positionS, carry, carryFocus) { room.send(RoomCommand.CarryOn) }
                    if (o != null) TvOffer(o.by, o.names, RoomWords.clock(positionS), { room.send(RoomCommand.AnswerMove(o.id, true)) }, { room.send(RoomCommand.AnswerMove(o.id, false)) })
                }
            }
        }
        if (mode == TvRoomMode.Leave) {
            TvLeaveDialog(onLeave = { room.send(RoomCommand.Leave); onLeft() }, onKeep = { state.mode = state.under })
        }
    }
}

/** The strip on the full picture: faces at 32, the line's title, and Carry on (focusable) at 20 s. */
@Composable
fun TvStrip(view: RoomView, members: List<RoomMember>, now: Long, positionS: Double, carry: String?, carryFocus: FocusRequester, onCarry: () -> Unit) {
    val c = tvRoomCopy()
    val (shown, hidden) = RoomWords.capped(members.filter { it.state != MemberState.LEFT })
    val line = RoomWords.line(c, view, members, now, positionS)
    val waiting = RoomWords.shownWaits(view, now).isNotEmpty() && carry == null
    val disc = 32.dp
    val spoken = (shown.map { c.spoken(it.name, RoomWords.word(c, view, it, now)) } + line.title).joinToString(". ") // i18n-skip
    Row(
        Modifier.shadow(18.dp, CircleShape).height(disc + 16.dp).clip(CircleShape).background(ObiSilk.Raised)
            .border(1.dp, ObiSilk.Silk.copy(alpha = 0.10f), CircleShape).padding(horizontal = 10.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Row(Modifier.clearAndSetSemantics { contentDescription = spoken; liveRegion = LiveRegionMode.Polite }, horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            for (m in shown) TvFace(view, m, now, disc, ObiSilk.Raised)
            if (hidden.isNotEmpty()) MoreFace(c.plus(hidden.size), disc)
            Text(
                buildAnnotatedString {
                    append(line.title)
                    if (waiting) withStyle(SpanStyle(color = ObiSilk.Haze, fontWeight = FontWeight.Normal)) { append(c.withSeconds(line.title, RoomWords.waitSeconds(view, now)).removePrefix(line.title)) }
                },
                style = TextStyle(fontSize = 14.sp, fontWeight = FontWeight.SemiBold, color = ObiSilk.Silk).tabular(),
                maxLines = 1,
                modifier = Modifier.padding(start = 4.dp, end = 6.dp),
            )
        }
        if (carry != null) {
            AmberButton(onClick = onCarry, modifier = Modifier.focusRequester(carryFocus).offset(x = 4.dp), icon = null, height = disc + 6.dp) {
                Text(carry, fontSize = 14.sp, fontWeight = FontWeight.Medium, maxLines = 1)
            }
        }
    }
}

/** Someone moved the room and ticked this TV: the words, Go along and Stay here. */
@Composable
private fun TvOffer(by: String, others: List<String>, at: String, onGo: () -> Unit, onStay: () -> Unit) {
    val c = tvRoomCopy()
    Column(
        Modifier.width(360.dp).clip(RoundedCornerShape(16.dp)).background(ObiSilk.Raised).border(1.dp, ObiSilk.Silk.copy(alpha = 0.10f), RoundedCornerShape(16.dp)).padding(16.dp),
    ) {
        Text(c.movedTheRoom(by), fontSize = 18.sp, fontWeight = FontWeight.Medium, color = ObiSilk.Silk)
        Text(c.goAlong(others, at), fontSize = 14.sp, lineHeight = 20.sp, color = ObiSilk.Haze, modifier = Modifier.padding(top = 4.dp))
        Row(Modifier.padding(top = 12.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            AmberButton(onClick = onGo, icon = null, height = 44.dp) { Text(c.GO_ALONG, fontSize = 16.sp) }
            TonalButton(onClick = onStay, height = 44.dp) { Text(c.STAY_HERE, fontSize = 16.sp) }
        }
    }
}

/** The rail: the video at 640 × 360 with the show and its place under it, the room at the side. */
@Composable
private fun TvRail(
    view: RoomView,
    members: List<RoomMember>,
    now: Long,
    positionS: Double,
    durationS: Double,
    episodeLine: String,
    carry: String?,
    carryFocus: FocusRequester,
    onCarry: () -> Unit,
) {
    val c = tvRoomCopy()
    val line = RoomWords.line(c, view, members, now, positionS)
    Box(Modifier.fillMaxSize().padding(horizontal = 48.dp, vertical = 27.dp)) {
        Column(Modifier.width(640.dp)) {
            // The video's place: TvRoom draws the one picture over it.
            Spacer(Modifier.size(640.dp, 360.dp))
            Row(Modifier.padding(top = 12.dp).fillMaxWidth(), verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(view.roomEpisode?.title ?: view.name, style = ShowTitle.page.copy(lineHeight = 30.sp), color = ObiSilk.Silk, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f, fill = false))
                Text(episodeLine, fontSize = 16.sp, color = ObiSilk.Silk, maxLines = 1)
                Spacer(Modifier.weight(1f))
                Text(RoomWords.clock(positionS) + " / " + RoomWords.clock(durationS), style = TextStyle(fontSize = 16.sp, color = ObiSilk.Haze).tabular()) // i18n-skip
            }
            Box(Modifier.padding(top = 10.dp).fillMaxWidth().height(6.dp).clip(CircleShape).background(ObiSilk.Silk.copy(alpha = 0.25f))) {
                Box(Modifier.fillMaxWidth(if (durationS > 0) (positionS / durationS).toFloat().coerceIn(0f, 1f) else 0f).height(6.dp).background(ObiSilk.Silk))
            }
            Text(c.TV_HINT, fontSize = 14.sp, lineHeight = 20.sp, color = ObiSilk.Haze, modifier = Modifier.padding(top = 10.dp))
        }
        Column(Modifier.align(Alignment.TopEnd).width(200.dp).fillMaxHeight()) {
            TvLineBlock(line)
            if (carry != null) {
                AmberButton(onClick = onCarry, modifier = Modifier.padding(bottom = 14.dp).focusRequester(carryFocus), icon = null, height = 44.dp) {
                    Text(carry, fontSize = 15.sp, maxLines = 2)
                }
            }
            val (shown, hidden) = RoomWords.capped(members.filter { it.state != MemberState.LEFT })
            for (m in shown) {
                // The TV goes by its device name, as the others see it.
                val name = m.name
                val word = RoomWords.word(c, view, m, now)
                Row(
                    Modifier.height(56.dp).fillMaxWidth().clearAndSetSemantics { contentDescription = c.spoken(name, word) },
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    TvFace(view, m, now, 36.dp, ObiSilk.Ground)
                    Column {
                        Text(name, fontSize = 16.sp, lineHeight = 22.sp, fontWeight = FontWeight.SemiBold, color = ObiSilk.Silk, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        Text(word, style = TextStyle(fontSize = 14.sp, lineHeight = 18.sp, color = ObiSilk.Haze).tabular(), maxLines = 1)
                    }
                }
            }
            if (hidden.isNotEmpty()) {
                Row(Modifier.height(56.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    MoreFace(c.plus(hidden.size), 36.dp)
                    Column {
                        Text(c.MORE, fontSize = 16.sp, lineHeight = 22.sp, fontWeight = FontWeight.SemiBold, color = ObiSilk.Silk)
                        Text(RoomWords.moreWord(c, hidden), fontSize = 14.sp, lineHeight = 18.sp, color = ObiSilk.Haze)
                    }
                }
            }
            Box(Modifier.padding(vertical = 10.dp).fillMaxWidth().height(1.dp).background(ObiSilk.FaintRule))
            Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.BottomStart) { TvReadOnlyChat() }
            Text(c.WRITE_ON_PHONE, fontSize = 13.sp, lineHeight = 18.sp, color = ObiSilk.Haze, modifier = Modifier.padding(top = 8.dp))
        }
    }
}

@Composable
private fun TvLineBlock(line: RoomLine) {
    Column(Modifier.padding(bottom = 14.dp).semantics(mergeDescendants = true) { liveRegion = LiveRegionMode.Polite }) {
        Text(line.title, fontSize = 20.sp, lineHeight = 26.sp, fontWeight = FontWeight.Medium, color = ObiSilk.Silk)
        Text(line.detail, style = TextStyle(fontSize = 14.sp, lineHeight = 20.sp, color = ObiSilk.Haze).tabular())
    }
}

/** The last few lines, read-only: the TV has no field; people write from a phone. */
@Composable
private fun TvReadOnlyChat() {
    val store = LocalAppStore.current
    val c = tvRoomCopy()
    val conversation by store.room.conversation.collectAsStateWithLifecycle()
    val lines = conversation?.lines.orEmpty().filter { !it.muted }.takeLast(6)
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        for (l in lines) {
            when (val b = l.body) {
                is RoomChatBody.Said -> Text(
                    buildAnnotatedString {
                        withStyle(SpanStyle(fontWeight = FontWeight.SemiBold, color = if (l.you) ObiSilk.Haze else ObiSilk.Silk)) { append(c.chatName(l) + "  ") } // i18n-skip: two spaces
                        append(b.text)
                    },
                    fontSize = 15.sp, lineHeight = 20.sp, color = ObiSilk.Silk,
                )
                else -> Text(c.chatSentence(l).orEmpty(), fontSize = 15.sp, lineHeight = 20.sp, color = ObiSilk.Haze)
            }
        }
    }
}

/** Leave the room? Leave and Keep watching; Back keeps watching. */
@Composable
fun TvLeaveDialog(onLeave: () -> Unit, onKeep: () -> Unit) {
    val c = tvRoomCopy()
    val first = remember { FocusRequester() }
    LaunchedEffect(Unit) { runCatching { first.requestFocus() } }
    Box(Modifier.fillMaxSize().background(Color.Black.copy(alpha = 0.55f)), contentAlignment = Alignment.Center) {
        Column(Modifier.width(440.dp).clip(RoundedCornerShape(16.dp)).background(ObiSilk.RaisedHigh).padding(28.dp)) {
            Text(c.LEAVE_TITLE, fontSize = 24.sp, lineHeight = 32.sp, fontWeight = FontWeight.Medium, color = ObiSilk.Silk)
            Text(c.LEAVE_BODY, fontSize = 16.sp, lineHeight = 22.sp, color = ObiSilk.Haze, modifier = Modifier.padding(top = 8.dp))
            Row(Modifier.padding(top = 24.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                TonalButton(onClick = onLeave, modifier = Modifier.focusRequester(first), height = 44.dp) { Text(c.LEAVE, fontSize = 16.sp) }
                TonalButton(onClick = onKeep, height = 44.dp) { Text(c.KEEP_WATCHING, fontSize = 16.sp) }
            }
        }
    }
}

/**
 * Before play, on the TV: the still at 640 × 360 under a fade, the show and episode, and Play for
 * everyone, focused; the rail beside it as in the room.
 */
@Composable
fun TvBeforePlay(title: String, episodeLine: String, started: Boolean, onPlay: () -> Unit) {
    val c = tvRoomCopy()
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
    Box(Modifier.fillMaxSize()) {
        Box(Modifier.fillMaxSize().background(Brush.verticalGradient(listOf(Color(0x4D0B0D18), Color(0xE60B0D18)))))
        Column(Modifier.align(Alignment.BottomStart).padding(start = 24.dp, bottom = 20.dp)) {
            Text(title, style = ShowTitle.page.copy(fontSize = 30.sp, lineHeight = 38.sp), color = ObiSilk.Silk)
            Text(episodeLine, fontSize = 16.sp, color = ObiSilk.Silk)
            TonalButton(
                onClick = onPlay,
                modifier = Modifier.padding(top = 14.dp).focusRequester(focus),
                icon = TvIcons.Play,
                enabled = !started,
                height = 44.dp,
            ) { Text(if (started) c.WAITING_FIRST_FRAMES else c.PLAY_FOR_EVERYONE, fontSize = 16.sp) }
        }
    }
}

/** The room's words for this screen's context. */
@Composable
internal fun tvRoomCopy(): RoomCopy {
    val context = LocalContext.current
    return remember(context) { RoomCopy(context) }
}
