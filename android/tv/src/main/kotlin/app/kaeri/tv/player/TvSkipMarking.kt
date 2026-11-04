// Marking skip times on the TV: a 400 dp panel on the right, the prompts in the corner, the
// sharing question and the Skip times list. The engine holds the steps and the rules; this draws
// what it says. The D-pad: Left and Right step the time one second (five while held) on the row
// that holds it, Up and Down move between rows, Center marks, Back steps back and then closes.

package app.kaeri.tv.player

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.focusable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.background
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.material3.Text
import app.kaeri.design.Motion
import app.kaeri.design.ObiSilk
import app.kaeri.engine.model.SkipEdge
import app.kaeri.engine.model.SkipKind
import app.kaeri.engine.model.SkipMarkCheck
import app.kaeri.engine.model.SkipMarkStep
import app.kaeri.engine.model.SkipMarkView
import app.kaeri.engine.model.SkipOrigin
import app.kaeri.engine.model.SkipPrompt
import app.kaeri.i18n.Strings
import app.kaeri.player.PlayerClock
import app.kaeri.player.PlayerModel
import app.kaeri.player.SkipKinds
import app.kaeri.player.SkipWords
import app.kaeri.tv.ui.ButtonLabel
import app.kaeri.tv.ui.PANEL_END
import app.kaeri.tv.ui.TonalButton
import app.kaeri.tv.ui.TvListRow
import app.kaeri.tv.ui.entryFocus
import app.kaeri.tv.ui.tryFocus

/** Left and Right on a row: one second, five while the key is held. */
private fun Modifier.stepping(model: PlayerModel, edge: SkipEdge?, onKind: ((Int) -> Unit)? = null): Modifier =
    onPreviewKeyEvent { event ->
        if (event.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
        val sign = when (event.key) {
            Key.DirectionLeft -> -1
            Key.DirectionRight -> 1
            else -> return@onPreviewKeyEvent false
        }
        if (onKind != null) {
            onKind(sign)
        } else if (edge != null) {
            val seconds = if (event.nativeKeyEvent.repeatCount > 0) 5 else 1
            model.nudgeEdge(edge, sign * seconds * 1000L)
        }
        true
    }

/** The marking panel; it holds the chrome up, so the seek line and its provisional segment show. */
@Composable
fun MarkPanel(model: PlayerModel, view: SkipMarkView, modifier: Modifier) {
    val context = LocalContext.current
    val action = remember { FocusRequester() }
    // A review of found times starts on Watch the edges, which has played them once by itself.
    val watch = remember { FocusRequester() }
    val title = Strings.playerMarkSkipTimes(context)
    val review = view.step == SkipMarkStep.REVIEW
    Column(
        modifier
            .width(400.dp)
            .fillMaxHeight()
            .background(ObiSilk.RaisedHigh)
            .padding(start = 16.dp, end = PANEL_END, top = 27.dp, bottom = 27.dp)
            .semantics { paneTitle = title },
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Text(
            if (review) Strings.playerMarkReviewTitle(context, SkipWords.kind(context, view.kind), SkipWords.length(context, view.length)) else title,
            style = MaterialTheme.typography.headlineSmall,
            color = ObiSilk.Silk,
            modifier = Modifier.padding(start = 16.dp, bottom = 12.dp).semantics { heading() },
        )
        if (review) {
            TvListRow(
                Strings.playerMarkStarts(context),
                onClick = {},
                value = PlayerClock.label(view.start ?: 0.0),
                supporting = Strings.playerMarkBack1(context) + "  " + Strings.playerMarkForward1(context),
                modifier = Modifier.stepping(model, SkipEdge.START).semantics { contentDescription = Strings.playerMarkBack1Label(context, Strings.playerMarkEdgeStart(context)) },
            )
            TvListRow(
                Strings.playerMarkEnds(context),
                onClick = {},
                value = PlayerClock.label(view.end ?: 0.0),
                supporting = Strings.playerMarkBack1(context) + "  " + Strings.playerMarkForward1(context),
                modifier = Modifier.stepping(model, SkipEdge.END).semantics { contentDescription = Strings.playerMarkBack1Label(context, Strings.playerMarkEdgeEnd(context)) },
            )
            val lines = listOfNotNull(SkipWords.check(context, view.kind, view.check)) + view.notes.map { SkipWords.note(context, it) }
            for (line in lines) {
                val short = view.check == SkipMarkCheck.TooShort && line == SkipWords.check(context, view.kind, view.check)
                Text(line, style = MaterialTheme.typography.bodyMedium, color = if (short) ObiSilk.Vermilion else ObiSilk.Haze, modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp))
            }
            TvListRow(Strings.playerMarkWatchEdges(context), onClick = model::previewEdges, enabled = !view.previewing, modifier = Modifier.focusRequester(watch))
            TonalButton(onClick = model::saveMark, enabled = view.canSave, modifier = Modifier.padding(start = 16.dp, top = 8.dp).focusRequester(action)) {
                ButtonLabel(Strings.playerMarkSave(context))
            }
        } else {
            val start = view.step == SkipMarkStep.START
            TvListRow(
                SkipWords.kind(context, view.kind),
                onClick = {},
                modifier = Modifier.stepping(model, null) { sign ->
                    val at = SkipKinds.indexOf(view.kind)
                    model.setMarkKind(SkipKinds[(at + sign + SkipKinds.size) % SkipKinds.size])
                },
            )
            Text(SkipWords.step(context, view.kind, start), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Silk, modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp))
            view.start?.let {
                Text(Strings.playerMarkStarts(context) + " " + PlayerClock.label(it), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, modifier = Modifier.padding(horizontal = 16.dp))
            }
            val edge = if (start) SkipEdge.START else SkipEdge.END
            TvListRow(
                PlayerClock.label(model.position),
                onClick = {},
                supporting = Strings.playerMarkBack1(context) + "  " + Strings.playerMarkForward1(context),
                modifier = Modifier.stepping(model, edge),
            )
            TonalButton(onClick = model::markEdge, modifier = Modifier.padding(start = 16.dp, top = 8.dp).focusRequester(action)) {
                ButtonLabel(if (start) Strings.playerMarkStart(context) else Strings.playerMarkEnd(context))
            }
        }
    }
    // Focus starts on the action, and returns to it as the step changes.
    LaunchedEffect(view.step) {
        repeat(5) {
            androidx.compose.runtime.withFrameNanos { }
            if ((if (view.found && view.step == SkipMarkStep.REVIEW) watch else action).tryFocus()) return@LaunchedEffect
        }
    }
}

/**
 * "Was that the opening?", "Wrong opening times?" or "Kaeri found this opening. Were the edges
 * right?", in the corner with its buttons and focus on the first, the one that answers yes. The
 * answer to a found card crossfades in its place, and holds focus so the remote still works.
 */
@Composable
fun SkipPromptCard(model: PlayerModel, offer: app.kaeri.player.SkipPromptOffer, first: FocusRequester) {
    AnimatedContent(
        offer.prompt,
        transitionSpec = { fadeIn(Motion.standard()) togetherWith fadeOut(Motion.standard()) },
        label = "skip card", // i18n-skip
    ) { prompt ->
        SkipPromptBody(model, prompt, first)
    }
}

@Composable
private fun SkipPromptBody(model: PlayerModel, prompt: SkipPrompt, first: FocusRequester) {
    val context = LocalContext.current
    val (line, note) = SkipWords.promptLines(context, prompt)
    // Yes first, then the rest, left to right.
    val buttons: List<Pair<String, () -> Unit>> = when (prompt) {
        is SkipPrompt.WasOpening -> listOf(
            Strings.playerMarkIt(context) to { model.dismissSkipPrompt(); model.beginMarking(SkipKind.OPENING) },
            Strings.playerDontAskAgain(context) to { model.neverAskWasOpening() },
        )
        is SkipPrompt.WrongTimes -> listOf(
            Strings.playerSkipMark(context) to { model.dismissSkipPrompt(); model.beginMarking(prompt.kind) },
            Strings.playerSkipReport(context) to { model.dismissSkipPrompt(); model.reportSkip(prompt.kind) },
        )
        is SkipPrompt.FoundEdges -> listOf(
            Strings.playerFoundYes(context) to { model.confirmFoundSkip(prompt.kind) },
            Strings.playerFoundAdjust(context) to { model.adjustFoundSkip(prompt.kind) },
            Strings.playerDontAskAgain(context) to { model.neverAskWasOpening() },
        )
        else -> emptyList()
    }
    Column(
        Modifier
            .entryFocus(first)
            .width(UP_NEXT_CARD)
            .background(ObiSilk.RaisedHigh, RoundedCornerShape(16.dp))
            .padding(16.dp)
            .then(if (buttons.isEmpty()) Modifier.focusRequester(first).focusable() else Modifier),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Column(Modifier.semantics(mergeDescendants = true) { liveRegion = LiveRegionMode.Polite }, verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text(line, style = MaterialTheme.typography.titleMedium, color = ObiSilk.Silk)
            note?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze) }
        }
        if (buttons.isNotEmpty()) {
            Row(Modifier.padding(top = 4.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                // Left and Right walk the buttons by order: the controls behind the card, when they are up,
                // must not catch the focus by where they sit.
                val stops = remember(buttons.size) { List(buttons.size) { FocusRequester() } }
                buttons.forEachIndexed { index, (label, action) ->
                    val own = if (index == 0) Modifier.focusRequester(first).focusRequester(stops[0]) else Modifier.focusRequester(stops[index])
                    TonalButton(
                        onClick = action,
                        modifier = own.focusProperties {
                            stops.getOrNull(index - 1)?.let { left = it }
                            stops.getOrNull(index + 1)?.let { right = it }
                        },
                    ) { ButtonLabel(label) }
                }
            }
        }
    }
}

private val UP_NEXT_CARD = 560.dp

/** The question the first Save asks. */
@Composable
fun ShareSkipTimesDialog(model: PlayerModel) {
    val context = LocalContext.current
    androidx.compose.ui.window.Dialog(onDismissRequest = { model.answerShare(false) }) {
        val share = remember { FocusRequester() }
        Column(
            Modifier
                .entryFocus(share)
                .width(560.dp)
                .background(ObiSilk.RaisedHigh, RoundedCornerShape(28.dp))
                .padding(32.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Text(Strings.playerShareTitle(context), style = MaterialTheme.typography.headlineSmall, color = ObiSilk.Silk)
            Text(Strings.playerShareBody(context), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze)
            Text(Strings.playerShareSent(context), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                TonalButton(onClick = { model.answerShare(true) }, modifier = Modifier.focusRequester(share)) { ButtonLabel(Strings.playerShareYes(context)) }
                TonalButton(onClick = { model.answerShare(false) }) { ButtonLabel(Strings.playerShareNo(context)) }
            }
        }
    }
}

/** The Skip times panel: Mark skip times first, then each kind with its times and what can be done. */
fun LazyListScope.skipTimesRows(model: PlayerModel, current: FocusRequester, onDone: () -> Unit) {
    item(key = "mark") {
        TvListRow(
            androidx.compose.ui.res.stringResource(app.kaeri.i18n.R.string.player_mark_skip_times),
            onClick = { onDone(); model.beginMarking() },
            modifier = Modifier.focusRequester(current),
        )
    }
    for (kind in SkipKinds) {
        item(key = "kind-$kind") {
            val context = LocalContext.current
            val found = model.timeline?.firstOrNull { it.kind == kind }
            val settings by app.kaeri.tv.ui.LocalAppStore.current.settings.view.collectAsStateWithLifecycle()
            val paused = if (found == null && kind != SkipKind.RECAP) app.kaeri.store.skipFindPausedLine(context, settings?.skipFindPaused) else null
            Text(
                SkipWords.kind(context, kind) + " · " + (found?.let { "${PlayerClock.label(it.start)}–${PlayerClock.label(it.end)} · ${SkipWords.origin(context, it.origin)}" } ?: listOfNotNull(Strings.playerSkipNone(context), paused).joinToString(" · ")), // i18n-skip
                style = MaterialTheme.typography.labelLarge,
                color = ObiSilk.Haze,
                modifier = Modifier.padding(start = 16.dp, top = 12.dp, bottom = 4.dp),
            )
        }
        val origin = model.timeline?.firstOrNull { it.kind == kind }?.origin
        item(key = "mark-$kind") {
            val context = LocalContext.current
            if (origin == SkipOrigin.FOUND) {
                TvListRow(Strings.playerFoundAdjust(context), onClick = { onDone(); model.adjustFoundSkip(kind) })
            } else {
                TvListRow(Strings.playerSkipMark(context), onClick = { onDone(); model.beginMarking(kind) })
            }
        }
        if (origin == SkipOrigin.ANI_SKIP) item(key = "report-$kind") {
            TvListRow(Strings.playerSkipReport(LocalContext.current), onClick = { onDone(); model.reportSkip(kind) })
        }
        if (origin == SkipOrigin.YOURS) item(key = "clear-$kind") {
            TvListRow(Strings.playerSkipClear(LocalContext.current), onClick = { onDone(); model.clearMySkip(kind) })
        }
    }
}
