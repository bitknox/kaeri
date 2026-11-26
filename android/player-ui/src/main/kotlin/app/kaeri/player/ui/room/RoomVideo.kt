// What the room lays over the picture on the phone: the faces strip when sideways (with Carry on at
// 20 s), and the panels on the upright video: before play, invited, no source, choosing a source,
// and Don't wait for me while this device loads. Also the room's bar above the navigation bar.

package app.kaeri.player.ui.room

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
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
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.design.KeptFade
import app.kaeri.design.MoreFace
import app.kaeri.design.ObiSilk
import app.kaeri.design.RoomGlyphs
import app.kaeri.design.ShowTitle
import app.kaeri.design.rememberRoomNow
import app.kaeri.design.tabular
import app.kaeri.engine.model.MemberState
import app.kaeri.engine.model.RoomCommand
import app.kaeri.engine.model.RoomMember
import app.kaeri.engine.model.RoomView
import app.kaeri.engine.model.WaitReason
import app.kaeri.store.room.CARRY_ON_AFTER_S
import app.kaeri.store.room.RoomModel
import app.kaeri.store.room.RoomWords

/**
 * The faces strip: up to four faces (or three and +N), the line's title, its seconds while the
 * room waits, and Carry on in the strip once the wait reaches 20 s. Solid Raised with a hairline.
 */
@Composable
fun RoomStrip(view: RoomView, members: List<RoomMember>, now: Long, disc: Dp, positionS: Double, modifier: Modifier = Modifier, touch: Boolean = true, onCarry: () -> Unit = {}) {
    val c = roomCopy()
    val (shown, hidden) = RoomWords.capped(members.filter { it.state != MemberState.LEFT })
    val line = RoomWords.line(c, view, members, now, positionS)
    val carry = RoomWords.carryLabel(c, view, now)
    val waiting = RoomWords.shownWaits(view, now).isNotEmpty() && carry == null
    val gap = if (disc >= 32.dp) 8.dp else 6.dp
    val height = if (carry != null && touch) 52.dp else disc + 16.dp
    val fs = if (disc >= 28.dp) 14.sp else 13.sp
    val spoken = (shown.map { c.spoken(if (it.you) c.YOU else it.name, RoomWords.word(c, view, it, now)) } + line.title).joinToString(". ") // i18n-skip
    Row(
        modifier
            .shadow(18.dp, CircleShape, ambientColor = Color.Black, spotColor = Color.Black)
            .height(height)
            .clip(CircleShape)
            .background(ObiSilk.Raised)
            .border(1.dp, ObiSilk.Silk.copy(alpha = 0.10f), CircleShape)
            .padding(horizontal = gap + 2.dp),
        horizontalArrangement = Arrangement.spacedBy(gap),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Row(Modifier.clearAndSetSemantics { contentDescription = spoken; liveRegion = LiveRegionMode.Polite }, horizontalArrangement = Arrangement.spacedBy(gap), verticalAlignment = Alignment.CenterVertically) {
            for (m in shown) PersonFace(view, m, now, disc, ground = ObiSilk.Raised)
            if (hidden.isNotEmpty()) MoreFace(c.plus(hidden.size), disc)
            Text(
                androidx.compose.ui.text.buildAnnotatedString {
                    append(line.title)
                    if (waiting) {
                        // "Paused for Ren · 3 s": the seconds in haze, after the bold words.
                        val whole = c.withSeconds(line.title, RoomWords.waitSeconds(view, now))
                        pushStyle(androidx.compose.ui.text.SpanStyle(color = ObiSilk.Haze, fontWeight = FontWeight.Normal))
                        append(whole.removePrefix(line.title))
                        pop()
                    }
                },
                style = TextStyle(fontSize = fs, fontWeight = FontWeight.SemiBold).tabular(),
                color = ObiSilk.Silk,
                maxLines = 1,
                modifier = Modifier.padding(start = 4.dp, end = 6.dp),
            )
        }
        if (carry != null) {
            CarryOnButton(carry, Modifier.offset(x = 2.dp), height = if (touch) 44.dp else disc + 6.dp, onClick = onCarry)
        }
    }
}

/**
 * Sideways: the full picture, the strip at the top centre. The strip shows while anyone isn't
 * watching and for 8 s after a change; [forceStrip] holds it for shots.
 */
@Composable
fun RoomSideways(room: RoomModel, positionS: Double, video: @Composable (Modifier) -> Unit, modifier: Modifier = Modifier, controls: Boolean = true, forceStrip: Boolean = true) {
    val view by room.view.collectAsStateWithLifecycle()
    val members by room.members.collectAsStateWithLifecycle()
    val now = rememberRoomNow()
    val shown = view ?: return
    Box(modifier.fillMaxSize().background(Color.Black)) {
        video(Modifier.fillMaxSize())
        val wanted = forceStrip || RoomWords.stripWanted(shown, members)
        KeptFade(if (wanted) shown else null, Modifier.align(Alignment.TopCenter).padding(top = if (controls) 78.dp else 14.dp)) { v ->
            RoomStrip(v, members, now, 28.dp, positionS) { room.send(RoomCommand.CarryOn) }
        }
    }
}

/** A dimmed still under a panel: the ground at half, or more for No source. */
@Composable
private fun Scrim(alpha: Float, content: @Composable () -> Unit) {
    Box(Modifier.fillMaxSize().background(Color(0xFF0B0D18).copy(alpha = alpha))) { content() }
}

/** Before play: the round Play and "Play for everyone", or "Waiting for first frames" once pressed. */
@Composable
fun BeforePlayPanel(started: Boolean, modifier: Modifier = Modifier, onPlay: () -> Unit) {
    val c = roomCopy()
    Box(modifier) {
        Scrim(0.5f) {
            Column(Modifier.align(Alignment.Center), horizontalAlignment = Alignment.CenterHorizontally) {
                val label = if (started) c.WAITING_FIRST_FRAMES else c.PLAY_FOR_EVERYONE
                Box(
                    Modifier.size(64.dp).clip(CircleShape).background(ObiSilk.Raised)
                        .border(1.dp, ObiSilk.Silk.copy(alpha = 0.10f), CircleShape)
                        .clickable(enabled = !started, role = Role.Button, onClickLabel = label, onClick = onPlay),
                    contentAlignment = Alignment.Center,
                ) { Icon(RoomGlyphs.Play, contentDescription = null, tint = ObiSilk.Silk.copy(alpha = if (started) 0.5f else 1f), modifier = Modifier.size(30.dp)) }
                Text(label, fontSize = 13.sp, color = ObiSilk.Silk, modifier = Modifier.padding(top = 8.dp))
            }
        }
    }
}

/** Invited: the show and episode over the still, bottom leading. Join and Not now sit under the line. */
@Composable
fun InvitedPanel(title: String, episodeLine: String, modifier: Modifier = Modifier) {
    Box(modifier) {
        Scrim(0.5f) {
            Column(Modifier.align(Alignment.BottomStart).padding(start = 16.dp, bottom = 14.dp)) {
                Text(title, style = ShowTitle.page.copy(fontSize = 20.sp, lineHeight = 26.sp), color = ObiSilk.Silk)
                Text(episodeLine, fontSize = 13.sp, color = ObiSilk.Silk)
            }
        }
    }
}

/** No source for the room's episode: the words, Open sources and Search titles. */
@Composable
fun NoSourcePanel(episode: String, modifier: Modifier = Modifier, onSources: () -> Unit = {}, onSearch: () -> Unit = {}) {
    val c = roomCopy()
    Box(modifier) {
        Scrim(0.8f) {
            Column(Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, top = 52.dp)) {
                Text(c.noneHas(episode), fontSize = 17.sp, lineHeight = 22.sp, fontWeight = FontWeight.SemiBold, color = ObiSilk.Silk)
                Text(c.STAY_IN_ROOM, fontSize = 13.sp, lineHeight = 18.sp, color = ObiSilk.Haze, modifier = Modifier.padding(top = 4.dp))
                Box(Modifier.padding(top = 14.dp)) { PairButtons(c.OPEN_SOURCES, c.SEARCH_TITLES, onSources, onSearch) }
            }
        }
    }
}

/** A source to choose for the room's episode: its name and what it offers. */
data class RoomSourceChoice(val name: String, val detail: String)

/**
 * Choosing a source for the room's episode: the list on Raised, Play in the room on the picked
 * one. The room keeps watching meanwhile, so this device joins at its place.
 */
@Composable
fun ChoosingPanel(episode: String, choices: List<RoomSourceChoice>, picked: Int, modifier: Modifier = Modifier, onPick: (Int) -> Unit = {}, onPlay: () -> Unit = {}) {
    val c = roomCopy()
    Box(modifier) {
        Scrim(0.5f) {
            Column(Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, top = 12.dp)) {
                Text(c.chooseFor(episode), fontSize = 15.sp, fontWeight = FontWeight.SemiBold, color = ObiSilk.Silk)
                Column(Modifier.padding(top = 8.dp).fillMaxWidth().clip(RoundedCornerShape(16.dp)).background(ObiSilk.Raised)) {
                    choices.forEachIndexed { i, c ->
                        if (i > 0) Box(Modifier.fillMaxWidth().height(1.dp).background(ObiSilk.FaintRule))
                        Row(
                            Modifier.fillMaxWidth().height(44.dp).clickable(role = Role.RadioButton) { onPick(i) }.padding(horizontal = 14.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Text(c.name, fontSize = 15.sp, color = ObiSilk.Silk, modifier = Modifier.weight(1f), maxLines = 1, overflow = TextOverflow.Ellipsis)
                            Text(c.detail, fontSize = 13.sp, color = ObiSilk.Haze)
                            if (i == picked) Icon(app.kaeri.design.PanelGlyphs.Check, null, tint = ObiSilk.Silk, modifier = Modifier.padding(start = 10.dp).size(18.dp))
                        }
                    }
                }
                Row(Modifier.padding(top = 10.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    CarryOnButton(c.PLAY_IN_ROOM, onClick = onPlay)
                    Text(c.CHOOSE_BODY, fontSize = 12.sp, lineHeight = 16.sp, color = ObiSilk.Silk, modifier = Modifier.weight(1f))
                }
            }
        }
    }
}

/** This device is loading and the room waits for it: the seconds, and Don't wait for me. */
@Composable
fun LoadingPanel(seconds: Int, canDontWait: Boolean, modifier: Modifier = Modifier, onDontWait: () -> Unit = {}) {
    val c = roomCopy()
    Box(modifier) {
        Scrim(0.5f) {
            Column(Modifier.align(Alignment.Center).semantics(mergeDescendants = true) { liveRegion = LiveRegionMode.Polite }, horizontalAlignment = Alignment.CenterHorizontally) {
                androidx.compose.material3.CircularProgressIndicator(
                    progress = { (seconds / CARRY_ON_AFTER_S.toFloat()).coerceIn(0.04f, 1f) },
                    modifier = Modifier.size(44.dp),
                    color = ObiSilk.Silk,
                    trackColor = ObiSilk.Rule,
                    strokeWidth = 3.dp,
                )
                Text(c.reasonFor(WaitReason.LOADING, seconds), fontSize = 13.sp, color = ObiSilk.Silk, modifier = Modifier.padding(top = 10.dp))
                if (canDontWait) {
                    androidx.compose.material3.FilledTonalButton(
                        onClick = onDontWait,
                        modifier = Modifier.padding(top = 12.dp).height(44.dp),
                        shape = CircleShape,
                        colors = androidx.compose.material3.ButtonDefaults.filledTonalButtonColors(containerColor = ObiSilk.Raised, contentColor = ObiSilk.Silk),
                    ) { Text(c.DONT_WAIT, fontSize = 15.sp, fontWeight = FontWeight.Medium) }
                }
            }
        }
    }
}

/**
 * The room's bar above the navigation bar once the room is closed to it: the still, the show,
 * the line's title, the faces overlapping, and Play/Pause. Tapping it opens the room.
 */
@Composable
fun RoomBar(room: RoomModel, positionS: Double, still: @Composable (Modifier) -> Unit, modifier: Modifier = Modifier, onOpen: () -> Unit = {}) {
    val c = roomCopy()
    val view by room.view.collectAsStateWithLifecycle()
    val members by room.members.collectAsStateWithLifecycle()
    val now = rememberRoomNow()
    val v = view ?: return
    val line = RoomWords.line(c, v, members, now, positionS)
    val waiting = RoomWords.shownWaits(v, now).isNotEmpty()
    Row(
        modifier.fillMaxWidth().height(64.dp).clip(RoundedCornerShape(16.dp)).background(ObiSilk.RaisedHigh)
            .clickable(role = Role.Button, onClick = onOpen).padding(horizontal = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        still(Modifier.size(width = 64.dp, height = 44.dp).clip(RoundedCornerShape(10.dp)))
        Column(Modifier.weight(1f)) {
            Text(v.roomEpisode?.title ?: v.name, style = ShowTitle.caption.copy(lineHeight = 18.sp), color = ObiSilk.Silk, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(line.title, style = TextStyle(fontSize = 13.sp, lineHeight = 17.sp).tabular(), color = if (waiting) ObiSilk.Silk else ObiSilk.Haze, maxLines = 1)
        }
        Row {
            val (shown, _) = RoomWords.capped(members.filter { it.state != MemberState.LEFT })
            shown.forEachIndexed { i, m -> PersonFace(v, m, now, 22.dp, Modifier.offset(x = (-6 * i).dp), ground = ObiSilk.RaisedHigh) }
        }
        Box(Modifier.size(48.dp), contentAlignment = Alignment.Center) {
            Icon(if (v.playing) RoomGlyphs.Pause else RoomGlyphs.Play, contentDescription = null, tint = ObiSilk.Silk, modifier = Modifier.size(22.dp))
        }
    }
}

/**
 * The strip over the full picture: while anyone isn't watching, and for 8 s after anyone's state
 * changes. It sits under the controls' top bar while they are up.
 */
@Composable
fun RoomStripOver(room: RoomModel, positionS: Double, controls: Boolean, modifier: Modifier = Modifier) {
    val view by room.view.collectAsStateWithLifecycle()
    val members by room.members.collectAsStateWithLifecycle()
    val now = rememberRoomNow()
    val shown = view ?: return
    val change = members.map { it.id to it.state } to shown.waiting.map { it.id }
    var recent by remember { mutableStateOf(false) }
    LaunchedEffect(change) {
        recent = true
        kotlinx.coroutines.delay(RoomWords.STRIP_AFTER_CHANGE_MS)
        recent = false
    }
    val wanted = recent || RoomWords.stripWanted(shown, members) || RoomWords.carryShown(shown, now)
    KeptFade(if (wanted) shown else null, modifier.padding(top = if (controls) 78.dp else 14.dp)) { v ->
        RoomStrip(v, members, now, 28.dp, positionS) { room.send(RoomCommand.CarryOn) }
    }
}

/**
 * What lies over the room's video for this device, if anything: before play, its own No source,
 * or its own loading with Don't wait for me. Choosing a source is the app's source sheet.
 */
@Composable
fun RoomPanel(room: RoomModel, modifier: Modifier = Modifier, onSources: () -> Unit = {}, onSearch: () -> Unit = {}) {
    val view by room.view.collectAsStateWithLifecycle()
    val members by room.members.collectAsStateWithLifecycle()
    val now = rememberRoomNow()
    val v = view ?: return
    val you = members.firstOrNull { it.you }
    when {
        v.beforePlay -> BeforePlayPanel(started = v.startsAtMs != null || v.waiting.isNotEmpty(), modifier = modifier) { room.send(RoomCommand.Play(true)) }
        you?.state == MemberState.NO_SOURCE -> NoSourcePanel(v.episode?.episode ?: "", modifier, onSources, onSearch)
        v.waiting.any { it.you } -> LoadingPanel(RoomWords.waitSeconds(v, now), v.canDontWait, modifier) { room.send(RoomCommand.DontWait) }
        else -> Unit
    }
}
