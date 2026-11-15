// The title's next episode aired and no source lists it: what the main action's slot says in
// Play's place, Check again and the episode before it, and the crossfade to amber Play once a
// source lists it.

package app.kaeri.ui

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.shrinkVertically
import androidx.compose.animation.ContentTransform
import androidx.compose.animation.Crossfade
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.SizeTransform
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.onClick
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.delay
import app.kaeri.design.Motion
import app.kaeri.design.tabular
import app.kaeri.engine.model.CheckOutcome
import app.kaeri.engine.model.Entry
import app.kaeri.engine.model.MissingEpisode
import app.kaeri.engine.model.MissingPhase
import app.kaeri.i18n.Strings
import app.kaeri.store.AppClock
import app.kaeri.store.CheckAgainPage
import app.kaeri.store.MissingWords
import app.kaeri.store.minuteTicks
import app.kaeri.store.missingIn

/** One title page's missing episode, as its slot reads it. */
@Stable
internal class MissingView(
    /** The missing episode the slot shows: held through Checking's last 0.3 s once a source lists it. */
    val shown: MissingEpisode?,
    /** What the last check the person asked for said of it, when that changes the words. */
    val outcome: CheckOutcome?,
    val offline: Boolean,
    /** Now, in Unix seconds, turned on the minute. */
    val now: Long,
    val page: CheckAgainPage,
    val checking: Boolean,
)

/**
 * [entry]'s missing episode as its page shows it, with the page's Check again, and what a screen
 * reader hears once of each answer, through [Announcer].
 */
@Composable
internal fun rememberMissingView(entry: Entry): Pair<MissingView, String?> {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val all by store.titles.episodes.collectAsStateWithLifecycle()
    val now = rememberMinute()
    val scope = rememberCoroutineScope()
    val page = remember(entry.id) { CheckAgainPage(store.missing, entry.id, scope) }
    val checking by page.wait.checking.collectAsStateWithLifecycle()
    val asked by store.missing.asked.collectAsStateWithLifecycle()
    val offline by store.offline.collectAsStateWithLifecycle()
    val missing = entry.missingIn(all[entry.id], now)
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
    LaunchedEffect(said) { if (said != null) { delay(SAID_FOR_MS); said = null } }
    return MissingView(shown, outcome, offline, now, page, checking) to said
}

private const val SAID_FOR_MS = 4_000L

/** Now, in Unix seconds, turned as each minute does: what the time since airing reads. */
@Composable
internal fun rememberMinute(): Long {
    val ticks = remember { minuteTicks() }
    val now by ticks.collectAsStateWithLifecycle(AppClock.now().epochSecond)
    return now
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
 * The slot: the missing episode's status with Check again and Play episode N-1 while [view] shows
 * one, else [play]. When a source lists it, the status crossfades to [play] in place on the reveal
 * token, bottom-aligned so Play takes the button row's place, while the slot's height settles on
 * standard. Rows that first load into a missing episode change at once.
 */
@Composable
internal fun MissingSlot(entry: Entry, view: MissingView, actions: AppActions, modifier: Modifier = Modifier, play: @Composable () -> Unit) {
    AnimatedContent(
        targetState = view.shown,
        contentKey = { it != null },
        contentAlignment = Alignment.BottomStart,
        transitionSpec = {
            if (initialState != null && targetState == null) {
                (fadeIn(Motion.reveal()) togetherWith fadeOut(Motion.reveal()))
                    .using(SizeTransform(clip = true) { _, _ -> Motion.standard() })
            } else {
                ContentTransform(EnterTransition.None, ExitTransition.None, sizeTransform = null)
            }
        },
        modifier = modifier,
        label = "missing", // i18n-skip
    ) { shown ->
        if (shown != null) {
            // 8 more under the buttons, so the row below sits 12 from them; the height settles with the rest.
            Column(Modifier.padding(bottom = 8.dp)) {
                MissingStatus(shown, view)
                MissingButtons(entry, shown, view, actions, Modifier.padding(top = 12.dp))
            }
        } else {
            play()
        }
    }
}

/**
 * The tablet's status, over its row of buttons: it closes up on standard, fading on the reveal
 * token, when a source lists the episode.
 */
@Composable
internal fun MissingAbove(view: MissingView, shows: Boolean, modifier: Modifier = Modifier) {
    val last = remember { arrayOfNulls<MissingEpisode>(1) }
    view.shown?.let { last[0] = it }
    AnimatedVisibility(
        visible = shows,
        enter = EnterTransition.None,
        exit = fadeOut(Motion.reveal()) + shrinkVertically(Motion.standard()),
        label = "status", // i18n-skip
    ) {
        last[0]?.let { MissingStatus(it, view, modifier) }
    }
}

/**
 * The tablet's lead in the row: Check again and Play episode N-1 while [view] shows a missing
 * episode, else [play]. The found crossfade is in place, the width settling on standard.
 */
@Composable
internal fun MissingLead(entry: Entry, view: MissingView, actions: AppActions, play: @Composable () -> Unit) {
    AnimatedContent(
        targetState = view.shown,
        contentKey = { it != null },
        contentAlignment = Alignment.CenterStart,
        transitionSpec = {
            if (initialState != null && targetState == null) {
                (fadeIn(Motion.reveal()) togetherWith fadeOut(Motion.reveal()))
                    .using(SizeTransform(clip = false) { _, _ -> Motion.standard() })
            } else {
                ContentTransform(EnterTransition.None, ExitTransition.None, sizeTransform = null)
            }
        },
        label = "lead", // i18n-skip
    ) { shown ->
        if (shown != null) MissingButtons(entry, shown, view, actions) else play()
    }
}

/**
 * "Episode 6 aired 25 min ago." in silk beside a water dot, haze from a week on, and under it in
 * haze what the sources have and what Kaeri does next. One element to a screen reader. With no
 * airing time known, today's line alone.
 */
@Composable
internal fun MissingStatus(missing: MissingEpisode, view: MissingView, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val colors = MaterialTheme.colorScheme
    val head = MissingWords.head(context, missing, view.now)
    val tail = MissingWords.tail(context, missing, view.outcome, view.offline)
    val whole = MissingWords.status(context, missing, view.outcome, view.offline, view.now)
    val dot = if (missing.phase == MissingPhase.LONG_AGO) colors.onSurfaceVariant else colors.secondary
    Column(modifier.clearAndSetSemantics { contentDescription = whole }) {
        if (head == null) {
            Row(Modifier.heightIn(min = 48.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.size(6.dp).background(dot, CircleShape))
                Text(tail, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant)
            }
            return@Column
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            Box(Modifier.size(6.dp).background(dot, CircleShape))
            // The time since airing turns on the minute, with no motion.
            Text(head, style = MaterialTheme.typography.titleMedium.tabular(), color = colors.onSurface)
        }
        // A check's answer crossfades the words on the reveal token.
        Crossfade(tail, animationSpec = Motion.reveal(), modifier = Modifier.padding(start = 14.dp, top = 2.dp), label = "tail") { // i18n-skip
            Text(it, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant)
        }
    }
}

/** Check again, tonal, and Play episode N-1, outlined: both secondary, no amber until found. */
@Composable
internal fun MissingButtons(entry: Entry, missing: MissingEpisode, view: MissingView, actions: AppActions, modifier: Modifier = Modifier) {
    Row(modifier, horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
        if (missing.phase != MissingPhase.LONG_AGO) CheckAgainButton(missing, view)
        missing.previous?.let { previous ->
            OutlinedButton(onClick = { actions.play(entry.id, previous.toString()) }) {
                Icon(Icons.Play, contentDescription = null, modifier = Modifier.size(16.dp))
                Spacer(Modifier.width(8.dp))
                Text(Strings.titleButtonPlay(LocalContext.current, previous.toInt()), maxLines = 1)
            }
        }
    }
}

/**
 * Check again. While a check runs past 600 ms, a spinner and Checking crossfade in over its
 * words; both are always laid out, so the button keeps its width. During the quiet rechecks its
 * hint says Kaeri also looks by itself.
 */
@Composable
internal fun CheckAgainButton(missing: MissingEpisode, view: MissingView, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val busy by animateFloatAsState(if (view.checking) 1f else 0f, Motion.reveal(), label = "checking") // i18n-skip
    val label = if (view.checking) Strings.actionChecking(context) else Strings.actionCheckAgain(context)
    // Compose has no hint of its own: the hint follows the name.
    val described = MissingWords.checkHint(context, missing)?.takeIf { !view.checking }?.let { "$label. $it" } ?: label // i18n-skip
    val press = { view.page.press(missing.episode) }
    FilledTonalButton(
        onClick = press,
        modifier = modifier.clearAndSetSemantics {
            contentDescription = described
            role = Role.Button
            onClick { press(); true }
        },
    ) {
        Box(contentAlignment = Alignment.Center) {
            Row(Modifier.graphicsLayer { alpha = 1f - busy }, verticalAlignment = Alignment.CenterVertically) {
                Icon(Icons.Redo, contentDescription = null, modifier = Modifier.size(18.dp))
                Spacer(Modifier.width(8.dp))
                Text(Strings.actionCheckAgain(context), maxLines = 1)
            }
            Row(Modifier.graphicsLayer { alpha = busy }, verticalAlignment = Alignment.CenterVertically) {
                // The spinner runs only while it shows.
                if (busy > 0f) CircularProgressIndicator(Modifier.size(16.dp), color = LocalContentColor.current, strokeWidth = 2.dp)
                else Spacer(Modifier.size(16.dp))
                Spacer(Modifier.width(8.dp))
                Text(Strings.actionChecking(context), maxLines = 1)
            }
        }
    }
}
