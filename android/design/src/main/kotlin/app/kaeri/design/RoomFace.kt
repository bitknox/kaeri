// A person in the room: an initial on a disc, and a ring that says where they are. A solid water
// ring is with the room, a haze ring is paused, an arc filling toward 20 s is the room waiting
// for them, and no ring with a dimmed disc is not with the room. The word beside or under the
// face always says the same, so the ring is decoration and colour is never the only signal.

package app.kaeri.design

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** The ring a face wears. */
enum class FaceRing { Water, Haze, Arc, None }

/** The stroke and the gap a face of [disc] gets: 3 and 3 from 48, 2 and 2 from 30, else 1.75 and 2. */
private fun stroke(disc: Dp): Dp = when {
    disc >= 48.dp -> 3.dp
    disc >= 30.dp -> 2.dp
    else -> 1.75.dp
}

private fun gap(disc: Dp): Dp = if (disc >= 48.dp) 3.dp else 2.dp

/** The whole face's size for an initials disc of [disc]: the disc, the gap and the ring. */
fun faceSize(disc: Dp): Dp = disc + (stroke(disc) + gap(disc)) * 2

/**
 * One face. [arc] is how full the waiting arc is, 0 to 1, stepping once a second; [dim] fades the
 * disc for someone not with the room; [badge] marks who paused the room; [error] is the
 * vermilion dot for No source. Drawn without semantics: the row or strip that holds it says the
 * name and the word as one element.
 */
@Composable
fun RoomFace(
    initial: String,
    disc: Dp,
    ring: FaceRing,
    modifier: Modifier = Modifier,
    arc: Float = 0f,
    dim: Float = 1f,
    badge: Boolean = false,
    error: Boolean = false,
    ground: Color = ObiSilk.Ground,
    discColor: Color = ObiSilk.RaisedSelected,
) {
    val sw = stroke(disc)
    val inset = sw + gap(disc)
    val size = faceSize(disc)
    // The arc steps a second at a time; a short ease keeps each step from snapping.
    val shown by animateFloatAsState(arc, Motion.fadeIn(), label = "arc") // i18n-skip
    Box(modifier.size(size)) {
        if (ring != FaceRing.None) {
            Canvas(Modifier.size(size)) {
                val w = sw.toPx()
                val tl = Offset(w / 2, w / 2)
                val sz = Size(this.size.width - w, this.size.height - w)
                when (ring) {
                    FaceRing.Water, FaceRing.Haze -> drawArc(
                        if (ring == FaceRing.Water) ObiSilk.Water else ObiSilk.Haze,
                        0f, 360f, false, tl, sz, style = Stroke(w),
                    )
                    FaceRing.Arc -> {
                        drawArc(ObiSilk.Rule, 0f, 360f, false, tl, sz, style = Stroke(w))
                        drawArc(ObiSilk.Silk, -90f, 360f * shown.coerceIn(0.04f, 1f), false, tl, sz, style = Stroke(w, cap = StrokeCap.Round))
                    }
                    FaceRing.None -> Unit
                }
            }
        }
        Box(
            Modifier.offset(inset, inset).size(disc).alpha(dim).clip(CircleShape).background(discColor),
            contentAlignment = Alignment.Center,
        ) {
            Text(
                initial.take(1).uppercase(),
                style = TextStyle(fontSize = (disc.value * 0.42f).sp, fontWeight = FontWeight.SemiBold, color = ObiSilk.Silk),
                maxLines = 1,
            )
        }
        if (badge) {
            val bz = maxOf(12.dp, disc * 0.42f)
            Box(
                Modifier.align(Alignment.BottomEnd).offset(1.dp, 1.dp).size(bz).clip(CircleShape).background(ground),
                contentAlignment = Alignment.Center,
            ) {
                Icon(RoomGlyphs.Pause, contentDescription = null, tint = ObiSilk.Silk, modifier = Modifier.size(bz - 4.dp))
            }
        }
        if (error) {
            val ez = maxOf(8.dp, disc * 0.28f)
            Box(
                Modifier.align(Alignment.BottomEnd).offset(-sw, -sw).size(ez).border(2.dp, ground, CircleShape).padding(0.dp).clip(CircleShape).background(ObiSilk.Vermilion),
            )
        }
    }
}

/** The +N face that opens everyone: the count on Raised with a hairline, no ring. */
@Composable
fun MoreFace(label: String, disc: Dp, modifier: Modifier = Modifier) {
    val inset = stroke(disc) + gap(disc)
    Box(modifier.size(faceSize(disc))) {
        Box(
            Modifier.offset(inset, inset).size(disc).clip(CircleShape).background(ObiSilk.Raised).border(1.dp, ObiSilk.Rule, CircleShape),
            contentAlignment = Alignment.Center,
        ) {
            Text(label, style = TextStyle(fontSize = (disc.value * 0.36f).sp, fontWeight = FontWeight.SemiBold, color = ObiSilk.Haze), maxLines = 1)
        }
    }
}

/** The room's clock: epoch ms on this device, and whether it ticks. Shots hold it still. */
class RoomClock(val now: () -> Long, val ticks: Boolean = true)

val LocalRoomClock = androidx.compose.runtime.staticCompositionLocalOf { RoomClock(System::currentTimeMillis) }

/** Now, read again on every whole second while the clock ticks: the room counts its seconds on this. */
@Composable
fun rememberRoomNow(): Long {
    val clock = LocalRoomClock.current
    val now = androidx.compose.runtime.remember(clock) { androidx.compose.runtime.mutableLongStateOf(clock.now()) }
    androidx.compose.runtime.LaunchedEffect(clock) {
        if (!clock.ticks) return@LaunchedEffect
        while (true) {
            now.longValue = clock.now()
            kotlinx.coroutines.delay(1000 - now.longValue % 1000)
        }
    }
    return now.longValue
}
