// Marking skip times on the phone and tablet: the panel (a side sheet in landscape, a bottom sheet
// in portrait), the prompts in the corner, the sharing question and the Skip times list. The
// engine holds the steps and the rules; this draws what it says and sends the presses back.

package app.kaeri.player.ui

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.togetherWith
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
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
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import app.kaeri.design.Motion
import app.kaeri.design.ObiSilk
import app.kaeri.design.tabular
import app.kaeri.engine.model.SkipEdge
import app.kaeri.engine.model.SkipKind
import app.kaeri.engine.model.SkipMarkCheck
import app.kaeri.engine.model.SkipMarkStep
import app.kaeri.engine.model.SkipMarkView
import app.kaeri.engine.model.SkipPrompt
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.player.PlayerClock
import app.kaeri.player.PlayerModel
import app.kaeri.player.SkipKinds
import app.kaeri.player.SkipWords

/** The panel's width in landscape. */
private val PanelWidth = 360.dp

/**
 * The marking panel. It holds the controls up while it is open, so the seek line, with the
 * provisional segment on it, stays in view.
 */
@Composable
fun MarkingPanel(model: PlayerModel, modifier: Modifier = Modifier) {
    val view = model.marking
    // Kept while the panel fades out, so it does not empty before it goes.
    var last by remember { mutableStateOf<SkipMarkView?>(null) }
    if (view != null) last = view
    val title = stringResource(R.string.player_mark_skip_times)
    DisposableEffect(view != null) {
        model.hold("marking", view != null) // i18n-skip
        onDispose { model.hold("marking", false) } // i18n-skip
    }
    BoxWithConstraints(modifier.fillMaxSize()) {
        val landscape = maxWidth > maxHeight
        AnimatedVisibility(
            view != null,
            modifier = Modifier.align(if (landscape) Alignment.CenterEnd else Alignment.BottomCenter),
            enter = fadeIn(Motion.fadeIn()) + if (landscape) slideInHorizontally(Motion.standard()) { it / 4 } else androidx.compose.animation.EnterTransition.None,
            exit = fadeOut(Motion.fadeOut()) + if (landscape) slideOutHorizontally(Motion.quick()) { it / 4 } else androidx.compose.animation.ExitTransition.None,
        ) {
            val shown = view ?: last ?: return@AnimatedVisibility
            val shape = if (landscape) RoundedCornerShape(28.dp) else RoundedCornerShape(topStart = 28.dp, topEnd = 28.dp)
            val sized = if (landscape) Modifier.padding(end = 24.dp).width(PanelWidth) else Modifier.fillMaxWidth()
            Column(
                sized
                    .clip(shape)
                    .background(ObiSilk.Raised)
                    .animateContentSize(Motion.standard())
                    .padding(horizontal = 24.dp, vertical = 20.dp)
                    .semantics { paneTitle = title },
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                MarkingContent(model, shown)
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun MarkingContent(model: PlayerModel, view: SkipMarkView) {
    val context = LocalContext.current
    val haptics = LocalHapticFeedback.current
    val review = view.step == SkipMarkStep.REVIEW
    // The title: the kind choice on the first two steps, the stretch's length on the review.
    Row(verticalAlignment = Alignment.CenterVertically) {
        if (review) {
            Text(
                Strings.playerMarkReviewTitle(context, SkipWords.kind(context, view.kind), SkipWords.length(context, view.length)),
                style = MaterialTheme.typography.titleMedium.tabular(),
                color = ObiSilk.Silk,
                modifier = Modifier.weight(1f),
            )
        } else {
            SingleChoiceSegmentedButtonRow(Modifier.weight(1f)) {
                SkipKinds.forEachIndexed { index, kind ->
                    SegmentedButton(
                        selected = kind == view.kind,
                        onClick = {
                            haptics.performHapticFeedback(HapticFeedbackType.SegmentTick)
                            model.setMarkKind(kind)
                        },
                        shape = SegmentedButtonDefaults.itemShape(index, SkipKinds.size),
                        colors = SegmentedButtonDefaults.colors(
                            activeContainerColor = ObiSilk.RaisedSelected,
                            activeContentColor = ObiSilk.Silk,
                            inactiveContainerColor = Color.Transparent,
                            inactiveContentColor = ObiSilk.Haze,
                        ),
                        label = { Text(SkipWords.kind(context, kind), maxLines = 1) },
                    )
                }
            }
        }
        TextButton(onClick = model::cancelMark) { Text(stringResource(R.string.action_cancel), color = ObiSilk.Haze) }
    }
    if (review) Review(model, view) else Steps(model, view)
}

@Composable
private fun Steps(model: PlayerModel, view: SkipMarkView) {
    val context = LocalContext.current
    val haptics = LocalHapticFeedback.current
    val start = view.step == SkipMarkStep.START
    Text(SkipWords.step(context, view.kind, start), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Silk)
    view.start?.let {
        Text(
            Strings.playerMarkStarts(context) + " " + PlayerClock.label(it),
            style = MaterialTheme.typography.bodySmall.tabular(),
            color = ObiSilk.Haze,
        )
    }
    // Where the playhead is: it is what gets marked.
    Text(PlayerClock.label(model.position), style = MaterialTheme.typography.displaySmall.tabular(), color = ObiSilk.Silk)
    val edge = if (start) SkipEdge.START else SkipEdge.END
    Nudges(
        name = if (start) Strings.playerMarkEdgeStart(context) else Strings.playerMarkEdgeEnd(context),
        onBack = { haptics.performHapticFeedback(HapticFeedbackType.SegmentTick); model.nudgeEdge(edge, -1000) },
        onForward = { haptics.performHapticFeedback(HapticFeedbackType.SegmentTick); model.nudgeEdge(edge, 1000) },
    )
    Button(
        onClick = {
            haptics.performHapticFeedback(HapticFeedbackType.ContextClick)
            model.markEdge()
        },
        modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp),
        colors = ButtonDefaults.buttonColors(containerColor = ObiSilk.Amber, contentColor = ObiSilk.Ground),
    ) { Text(if (start) Strings.playerMarkStart(context) else Strings.playerMarkEnd(context)) }
}

@Composable
private fun Nudges(name: String, onBack: () -> Unit, onForward: () -> Unit, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    Row(modifier, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        FilledTonalButton(
            onClick = onBack,
            colors = ButtonDefaults.filledTonalButtonColors(containerColor = ObiSilk.RaisedSelected, contentColor = ObiSilk.Silk),
            modifier = Modifier.semantics { contentDescription = Strings.playerMarkBack1Label(context, name) },
        ) { Text(Strings.playerMarkBack1(context), style = MaterialTheme.typography.labelLarge.tabular()) }
        FilledTonalButton(
            onClick = onForward,
            colors = ButtonDefaults.filledTonalButtonColors(containerColor = ObiSilk.RaisedSelected, contentColor = ObiSilk.Silk),
            modifier = Modifier.semantics { contentDescription = Strings.playerMarkForward1Label(context, name) },
        ) { Text(Strings.playerMarkForward1(context), style = MaterialTheme.typography.labelLarge.tabular()) }
    }
}

@Composable
private fun Review(model: PlayerModel, view: SkipMarkView) {
    val context = LocalContext.current
    val haptics = LocalHapticFeedback.current
    EdgeRow(Strings.playerMarkStarts(context), Strings.playerMarkEdgeStart(context), SkipEdge.START, view.start ?: 0.0, model, haptics)
    EdgeRow(Strings.playerMarkEnds(context), Strings.playerMarkEdgeEnd(context), SkipEdge.END, view.end ?: 0.0, model, haptics)
    SkipWords.check(context, view.kind, view.check)?.let {
        // Too short is an alert; too long is a dim line, as Save still keeps it on this device.
        val short = view.check == SkipMarkCheck.TooShort
        Text(it, style = MaterialTheme.typography.bodySmall, color = if (short) ObiSilk.Vermilion else ObiSilk.Haze)
    }
    // The notes are one label with a line break between them: a tight note, not paragraphs with a gap.
    if (view.notes.isNotEmpty()) {
        Text(view.notes.joinToString("\n") { SkipWords.note(context, it) }, style = MaterialTheme.typography.bodySmall, color = ObiSilk.Haze)
    }
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End), verticalAlignment = Alignment.CenterVertically) {
        TextButton(onClick = model::previewEdges, enabled = !view.previewing) {
            Text(Strings.playerMarkWatchEdges(context), color = if (view.previewing) ObiSilk.Haze else ObiSilk.Silk)
        }
        Spacer(Modifier.weight(1f))
        Button(
            onClick = {
                haptics.performHapticFeedback(HapticFeedbackType.ContextClick)
                model.saveMark()
            },
            enabled = view.canSave,
            colors = ButtonDefaults.buttonColors(containerColor = ObiSilk.Amber, contentColor = ObiSilk.Ground),
        ) { Text(Strings.playerMarkSave(context)) }
    }
}

@Composable
private fun EdgeRow(label: String, name: String, edge: SkipEdge, at: Double, model: PlayerModel, haptics: androidx.compose.ui.hapticfeedback.HapticFeedback) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Column(Modifier.weight(1f)) {
            Text(label, style = MaterialTheme.typography.bodySmall, color = ObiSilk.Haze)
            Text(PlayerClock.label(at), style = MaterialTheme.typography.titleLarge.tabular(), color = ObiSilk.Silk)
        }
        Nudges(
            name,
            onBack = { haptics.performHapticFeedback(HapticFeedbackType.SegmentTick); model.nudgeEdge(edge, -1000) },
            onForward = { haptics.performHapticFeedback(HapticFeedbackType.SegmentTick); model.nudgeEdge(edge, 1000) },
        )
    }
}

/**
 * The card in the corner: "Was that the opening?", "Wrong opening times?", or "Kaeri found this
 * opening. Were the edges right?" with Yes, Adjust and Don't ask again. The answer to a found
 * card crossfades in its place, the card never empty between them.
 */
@Composable
internal fun SkipPromptCard(model: PlayerModel) {
    val offer = model.skipPrompt ?: return
    AnimatedContent(
        offer.prompt,
        transitionSpec = { fadeIn(Motion.standard()) togetherWith fadeOut(Motion.standard()) },
        label = "skip card", // i18n-skip
    ) { prompt ->
        SkipPromptBody(model, prompt)
    }
}

@Composable
private fun SkipPromptBody(model: PlayerModel, prompt: SkipPrompt) {
    val context = LocalContext.current
    val haptics = LocalHapticFeedback.current
    val (line, note) = SkipWords.promptLines(context, prompt)
    val buttons: List<Pair<String, () -> Unit>> = when (prompt) {
        is SkipPrompt.WasOpening -> listOf(
            Strings.playerDontAskAgain(context) to { model.neverAskWasOpening() },
            Strings.playerMarkIt(context) to { model.dismissSkipPrompt(); model.beginMarking(SkipKind.OPENING) },
        )
        is SkipPrompt.WrongTimes -> listOf(
            Strings.playerSkipReport(context) to { model.dismissSkipPrompt(); model.reportSkip(prompt.kind) },
            Strings.playerSkipMark(context) to { model.dismissSkipPrompt(); model.beginMarking(prompt.kind) },
        )
        is SkipPrompt.FoundEdges -> listOf(
            Strings.playerDontAskAgain(context) to { model.neverAskWasOpening() },
            Strings.playerFoundAdjust(context) to { model.adjustFoundSkip(prompt.kind) },
            Strings.playerFoundYes(context) to { model.confirmFoundSkip(prompt.kind) },
        )
        else -> emptyList()
    }
    Column(
        Modifier
            .width(300.dp)
            .clip(RoundedCornerShape(20.dp))
            .background(ObiSilk.Raised)
            .padding(start = 16.dp, end = 8.dp, top = 12.dp, bottom = if (buttons.isEmpty()) 12.dp else 4.dp),
    ) {
        // Announced as it appears, and read as one: the question and what Yes would do.
        Column(Modifier.semantics(mergeDescendants = true) { liveRegion = LiveRegionMode.Polite }) {
            Text(line, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Silk)
            note?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = ObiSilk.Haze, modifier = Modifier.padding(top = 4.dp)) }
        }
        if (buttons.isNotEmpty()) {
            Row(Modifier.fillMaxWidth().padding(top = 4.dp), horizontalArrangement = Arrangement.End) {
                buttons.forEachIndexed { index, (label, action) ->
                    val last = index == buttons.lastIndex
                    TextButton(onClick = { if (last) haptics.performHapticFeedback(HapticFeedbackType.ContextClick); action() }) {
                        Text(label, style = MaterialTheme.typography.labelLarge, color = if (index == 0) ObiSilk.Haze else ObiSilk.Silk)
                    }
                }
            }
        }
    }
}

/** The question the first Save asks, drawn in the player as "Still watching?" is. */
@Composable
fun ShareSkipTimes(model: PlayerModel) {
    val context = LocalContext.current
    AnimatedVisibility(model.asksToShare, enter = fadeIn(Motion.standard()), exit = fadeOut(Motion.quick())) {
        val title = Strings.playerShareTitle(context)
        Box(Modifier.fillMaxSize().background(Color.Black.copy(alpha = 0.32f)).clickable(interactionSource = null, indication = null) {}, contentAlignment = Alignment.Center) {
            Column(
                Modifier.width(312.dp).clip(RoundedCornerShape(28.dp)).background(ObiSilk.RaisedHigh).padding(start = 24.dp, end = 24.dp, top = 24.dp, bottom = 16.dp)
                    .semantics { paneTitle = title },
            ) {
                Text(title, style = MaterialTheme.typography.titleLarge, color = ObiSilk.Silk)
                Text(Strings.playerShareBody(context), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, modifier = Modifier.padding(top = 16.dp))
                Text(Strings.playerShareSent(context), style = MaterialTheme.typography.bodySmall, color = ObiSilk.Haze, modifier = Modifier.padding(top = 12.dp))
                Row(Modifier.padding(top = 24.dp).fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End)) {
                    TextButton(onClick = { model.answerShare(false) }) { Text(Strings.playerShareNo(context), color = ObiSilk.Silk) }
                    FilledTonalButton(
                        onClick = { model.answerShare(true) },
                        colors = ButtonDefaults.filledTonalButtonColors(containerColor = ObiSilk.RaisedSelected, contentColor = ObiSilk.Silk),
                    ) { Text(Strings.playerShareYes(context)) }
                }
            }
        }
    }
}

/** The Skip times list: each kind with its times and where they came from, and what can be done to them. */
internal fun LazyListScope.skipTimes(model: PlayerModel, onDismiss: () -> Unit) {
    item(key = "title") {
        Text(
            stringResource(R.string.player_skip_times),
            style = MaterialTheme.typography.titleLarge,
            color = ObiSilk.Silk,
            modifier = Modifier.padding(start = 24.dp, end = 24.dp, bottom = 8.dp),
        )
    }
    items(SkipKinds.size, key = { "kind-$it" }) { index ->
        val kind = SkipKinds[index]
        val found = model.timeline?.firstOrNull { it.kind == kind }
        SkipTimeRow(model, kind, found, onDismiss)
    }
}

@Composable
private fun SkipTimeRow(model: PlayerModel, kind: SkipKind, found: app.kaeri.engine.model.TimelineMark?, onDismiss: () -> Unit) {
    val context = LocalContext.current
    val settings by app.kaeri.ui.LocalAppStore.current.settings.view.collectAsStateWithLifecycle()
    // A kind with no times says why there are none, when finding is paused. Recaps are not found.
    val paused = if (found == null && kind != SkipKind.RECAP) app.kaeri.store.skipFindPausedLine(context, settings?.skipFindPaused) else null
    Row(Modifier.fillMaxWidth().heightIn(min = 64.dp).padding(start = 24.dp, end = 8.dp), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            Text(SkipWords.kind(context, kind), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk)
            Text(
                if (found == null) listOfNotNull(Strings.playerSkipNone(context), paused).joinToString(" · ") // i18n-skip
                else "${PlayerClock.label(found.start)}–${PlayerClock.label(found.end)} · ${SkipWords.origin(context, found.origin)}", // i18n-skip
                style = MaterialTheme.typography.bodyMedium.tabular(),
                color = ObiSilk.Haze,
            )
        }
        when (found?.origin) {
            app.kaeri.engine.model.SkipOrigin.ANI_SKIP ->
                TextButton(onClick = { onDismiss(); model.reportSkip(kind) }) { Text(Strings.playerSkipReport(context), color = ObiSilk.Silk) }
            app.kaeri.engine.model.SkipOrigin.YOURS ->
                TextButton(onClick = { onDismiss(); model.clearMySkip(kind) }) { Text(Strings.playerSkipClear(context), color = ObiSilk.Silk) }
            // Found times have a button of their own: Adjust opens the review on them.
            app.kaeri.engine.model.SkipOrigin.FOUND ->
                TextButton(onClick = { onDismiss(); model.adjustFoundSkip(kind) }) { Text(Strings.playerFoundAdjust(context), color = ObiSilk.Silk) }
            else -> {}
        }
        if (found?.origin != app.kaeri.engine.model.SkipOrigin.FOUND) {
            TextButton(onClick = { onDismiss(); model.beginMarking(kind) }) { Text(Strings.playerSkipMark(context), color = ObiSilk.Silk) }
        }
    }
}
