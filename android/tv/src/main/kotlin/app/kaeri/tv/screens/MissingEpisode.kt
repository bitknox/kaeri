// The title's next episode aired and no source lists it: Check again and Play episode N-1 lead
// the button row, the status sits under the row, and amber Play takes their place once a source
// lists it.

package app.kaeri.tv.screens

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.ContentTransform
import androidx.compose.animation.Crossfade
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.SizeTransform
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.focusable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.InlineTextContent
import androidx.compose.foundation.text.appendInlineContent
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.onClick
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.Placeholder
import androidx.compose.ui.text.PlaceholderVerticalAlign
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.Icon
import androidx.tv.material3.LocalContentColor
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import kotlinx.coroutines.delay
import app.kaeri.design.Motion
import app.kaeri.design.ObiSilk
import app.kaeri.engine.model.CheckOutcome
import app.kaeri.engine.model.Entry
import app.kaeri.engine.model.EpisodeRow
import app.kaeri.engine.model.MissingEpisode
import app.kaeri.engine.model.MissingPhase
import app.kaeri.i18n.Strings
import app.kaeri.store.AppClock
import app.kaeri.store.CheckAgainPage
import app.kaeri.store.MissingWords
import app.kaeri.store.minuteTicks
import app.kaeri.store.missingIn
import app.kaeri.tv.ui.ButtonLabel
import app.kaeri.tv.ui.FocusMemory
import app.kaeri.tv.ui.LocalAppStore
import app.kaeri.tv.ui.LocalReducedMotion
import app.kaeri.tv.ui.TonalButton
import app.kaeri.tv.ui.TvIcons
import app.kaeri.tv.ui.firstFocus
import app.kaeri.tv.ui.rememberScreenReader
import app.kaeri.tv.ui.remembered

/** One title page's missing episode, as its button row and the line under it read it. */
@Stable
internal class MissingView(
    /** The missing episode shown: held through Checking's last 0.3 s once a source lists it. */
    val shown: MissingEpisode?,
    /** What the last check the person asked for said of it, when that changes the words. */
    val outcome: CheckOutcome?,
    val offline: Boolean,
    /** Now, in Unix seconds, turned on the minute. */
    val now: Long,
    val page: CheckAgainPage,
    val checking: Boolean,
    /** What a screen reader hears once, from [Announcer]. */
    val said: String?,
)

/** Now, in Unix seconds, turned as each minute does: what the time since airing reads. */
@Composable
internal fun rememberMinute(): Long {
    val ticks = remember { minuteTicks() }
    val now by ticks.collectAsStateWithLifecycle(AppClock.now().epochSecond)
    return now
}

/** [entry]'s missing episode in [rows], with the page's Check again and what is said of its answers. */
@Composable
internal fun rememberMissingView(entry: Entry, rows: List<EpisodeRow>?): MissingView {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val now = rememberMinute()
    val scope = rememberCoroutineScope()
    val page = remember(entry.id) { CheckAgainPage(store.missing, entry.id, scope) }
    val checking by page.wait.checking.collectAsStateWithLifecycle()
    val asked by store.missing.asked.collectAsStateWithLifecycle()
    val offline by store.offline.collectAsStateWithLifecycle()
    val missing = entry.missingIn(rows, now)
    val last = remember(entry.id) { arrayOfNulls<MissingEpisode>(1) }
    if (missing != null) last[0] = missing
    // Found while Checking shows: the status stays until Checking has had its 0.3 s.
    val shown = missing ?: last[0].takeIf { checking }
    val outcome = shown?.let { m -> asked[entry.id]?.takeIf { it.episode == m.episode }?.outcome }
    var said by remember(entry.id) { mutableStateOf<String?>(null) }
    LaunchedEffect(page) { page.listen { said = MissingWords.said(context, it) } }
    // Checking is said only when it shows.
    LaunchedEffect(checking) { if (checking) said = Strings.actionChecking(context) }
    // Cleared after a while, so the same words said again are heard again.
    LaunchedEffect(said) { if (said != null) { delay(4_000); said = null } }
    return MissingView(shown, outcome, offline, now, page, checking, said)
}

/** A polite live region a screen reader hears [said] from, once; it shows nothing. */
@Composable
internal fun Announcer(said: String?) {
    Box(
        Modifier.size(1.dp).clearAndSetSemantics {
            if (said != null) {
                contentDescription = said
                liveRegion = LiveRegionMode.Polite
            }
        },
    )
}

/**
 * The lead of the button row: Check again (the page's first focus) and Play episode N-1, both
 * tonal, while [view] shows a missing episode; else [play]. When a source lists it, amber Play
 * crossfades in on the reveal token while the lead's width settles on standard, and focus on
 * either button moves to Play.
 */
@Composable
internal fun MissingLead(entry: Entry, view: MissingView, memory: FocusMemory, onPlay: (UInt) -> Unit, play: @Composable () -> Unit) {
    val reduced = LocalReducedMotion.current
    var held by remember(entry.id) { mutableStateOf(false) }
    val gone = view.shown == null
    LaunchedEffect(gone) {
        if (gone && held) {
            // A frame, so amber Play has attached its requester; the focus scale is the TV's own.
            withFrameNanos { }
            memory.focus("play")
            held = false
        }
    }
    AnimatedContent(
        targetState = view.shown,
        contentKey = { it != null },
        contentAlignment = Alignment.CenterStart,
        transitionSpec = {
            if (initialState != null && targetState == null) {
                (fadeIn(Motion.reveal()) togetherWith fadeOut(Motion.reveal()))
                    .using(if (reduced) null else SizeTransform(clip = false) { _, _ -> Motion.standard() })
            } else {
                ContentTransform(EnterTransition.None, ExitTransition.None, sizeTransform = null)
            }
        },
        label = "lead", // i18n-skip
    ) { shown ->
        if (shown == null) {
            play()
            return@AnimatedContent
        }
        Row(Modifier.onFocusChanged { held = it.hasFocus }, verticalAlignment = Alignment.CenterVertically) {
            val longAgo = shown.phase == MissingPhase.LONG_AGO
            if (!longAgo) {
                CheckAgainButton(shown, view, Modifier.firstFocus(memory, "check"))
            }
            shown.previous?.let { previous ->
                if (!longAgo) Spacer(Modifier.width(16.dp))
                TonalButton(
                    onClick = { onPlay(previous) },
                    icon = TvIcons.Play,
                    modifier = Modifier.firstFocus(memory, "prev", isFirst = longAgo),
                ) { ButtonLabel(Strings.titleButtonPlay(LocalContext.current, previous.toInt())) }
            }
        }
    }
}

/**
 * Check again. While a check runs past 600 ms, a spinner and Checking crossfade in over its
 * words; both are always laid out, so the button keeps its width.
 */
@Composable
private fun CheckAgainButton(missing: MissingEpisode, view: MissingView, modifier: Modifier) {
    val context = LocalContext.current
    val busy by animateFloatAsState(if (view.checking) 1f else 0f, Motion.reveal(), label = "checking") // i18n-skip
    val label = if (view.checking) Strings.actionChecking(context) else Strings.actionCheckAgain(context)
    // Compose has no hint of its own: the hint follows the name.
    val described = MissingWords.checkHint(context, missing)?.takeIf { !view.checking }?.let { "$label. $it" } ?: label // i18n-skip
    val press = { view.page.press(missing.episode) }
    TonalButton(
        onClick = press,
        modifier = modifier.clearAndSetSemantics {
            contentDescription = described
            role = Role.Button
            onClick { press(); true }
        },
    ) {
        Box(contentAlignment = Alignment.Center) {
            Row(Modifier.graphicsLayer { alpha = 1f - busy }, verticalAlignment = Alignment.CenterVertically) {
                Icon(TvIcons.Redo, contentDescription = null, modifier = Modifier.size(20.dp))
                Spacer(Modifier.width(10.dp))
                ButtonLabel(Strings.actionCheckAgain(context))
            }
            Row(Modifier.graphicsLayer { alpha = busy }, verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.size(20.dp), contentAlignment = Alignment.Center) { if (busy > 0f) SmallSpinner() }
                Spacer(Modifier.width(10.dp))
                ButtonLabel(Strings.actionChecking(context))
            }
        }
    }
}

/** An 18 dp arc in the button's own colour, turning; still with Remove animations on. */
@Composable
private fun SmallSpinner() {
    val color = LocalContentColor.current
    val still = LocalReducedMotion.current
    val turning = rememberInfiniteTransition(label = "spinner") // i18n-skip
    val turned by turning.animateFloat(0f, 360f, infiniteRepeatable(tween(1000, easing = LinearEasing)), label = "angle") // i18n-skip
    Canvas(Modifier.size(18.dp)) {
        val stroke = 2.dp.toPx()
        drawArc(color.copy(alpha = 0.2f), 0f, 360f, useCenter = false, style = Stroke(stroke))
        drawArc(color, if (still) 0f else turned, 90f, useCenter = false, style = Stroke(stroke, cap = StrokeCap.Round))
    }
}

/**
 * The status under the button row: a water dot (haze from a week on), "Episode 6 aired 25 min
 * ago." in silk, then what the sources have and what Kaeri does next in haze. One element to a
 * screen reader, which can stop on it. It closes up on standard as a source lists the episode.
 */
@Composable
internal fun MissingLine(view: MissingView, modifier: Modifier = Modifier) {
    val last = remember { arrayOfNulls<MissingEpisode>(1) }
    view.shown?.let { last[0] = it }
    val reduced = LocalReducedMotion.current
    AnimatedVisibility(
        visible = view.shown != null,
        enter = EnterTransition.None,
        exit = if (reduced) fadeOut(Motion.reveal()) else fadeOut(Motion.reveal()) + shrinkVertically(Motion.standard()),
        label = "status", // i18n-skip
    ) {
        val missing = last[0] ?: return@AnimatedVisibility
        val context = LocalContext.current
        val reader by rememberScreenReader()
        val head = MissingWords.head(context, missing, view.now)
        val tail = MissingWords.tail(context, missing, view.outcome, view.offline)
        val whole = MissingWords.status(context, missing, view.outcome, view.offline, view.now)
        val dot = if (missing.phase == MissingPhase.LONG_AGO) ObiSilk.Haze else ObiSilk.Water
        val dotContent = mapOf(
            "dot" to InlineTextContent(Placeholder(14.sp, 6.sp, PlaceholderVerticalAlign.TextCenter)) {
                Box(Modifier.padding(end = 8.dp).size(6.dp).background(dot, CircleShape))
            },
        )
        Box(
            modifier
                .padding(top = 12.dp)
                .widthIn(max = 792.dp)
                .then(if (reader) Modifier.focusable() else Modifier)
                .clearAndSetSemantics { contentDescription = whole },
        ) {
            // A check's answer crossfades the words on the reveal token; the time turns with no motion.
            Crossfade(tail, animationSpec = Motion.reveal(), label = "tail") { words -> // i18n-skip
                val text = buildAnnotatedString {
                    appendInlineContent("dot", "•") // i18n-skip
                    if (head != null) {
                        withStyle(SpanStyle(color = ObiSilk.Silk, fontWeight = FontWeight.Medium)) { append(head) }
                        append(" ") // i18n-skip
                    }
                    append(words)
                }
                Text(
                    text,
                    inlineContent = dotContent,
                    style = MaterialTheme.typography.bodyLarge.copy(fontSize = 16.sp, lineHeight = 22.sp),
                    color = ObiSilk.Haze,
                )
            }
        }
    }
}
