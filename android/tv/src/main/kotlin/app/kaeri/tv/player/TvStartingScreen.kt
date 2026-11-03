// The TV's starting screen, as the board draws it: the title's art over the whole screen under a
// ground, and a 680 dp column in the middle with the show, the episode, Kaeri's line, the step,
// what the start knows, and the buttons that apply. Nothing takes focus while a start runs; the
// first direction press lands on Choose source. After a failure, focus is on the primary.

package app.kaeri.tv.player

import android.content.Context
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.snap
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import kotlinx.coroutines.launch
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shadow
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.foundation.Canvas
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeJoin
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.LocalContentColor
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import app.kaeri.design.FrameCrossfade
import app.kaeri.design.KaeriLoaderLine
import app.kaeri.design.ReservedHeight
import app.kaeri.design.Motion
import app.kaeri.design.ObiSilk
import app.kaeri.design.StartChevron
import app.kaeri.design.StartColumn
import app.kaeri.design.tabular
import app.kaeri.engine.core.Translation
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.player.ArtLuminance
import app.kaeri.player.DetailRow
import app.kaeri.player.PlaybackFailure
import app.kaeri.player.PlayerModel
import app.kaeri.player.RowKind
import app.kaeri.player.StartAction
import app.kaeri.player.StartDetailsMemory
import app.kaeri.player.StartProgress
import app.kaeri.player.StartTiming
import app.kaeri.player.label
import app.kaeri.player.primaryAction
import app.kaeri.player.startActions
import app.kaeri.player.startingAnnouncement
import app.kaeri.player.startingWords
import app.kaeri.player.startWaiting
import app.kaeri.store.HeroArt
import app.kaeri.tv.ui.AmberButton
import app.kaeri.tv.ui.ButtonLabel
import app.kaeri.tv.ui.LocalAppStore
import app.kaeri.tv.ui.LocalReducedMotion
import app.kaeri.tv.ui.TonalButton
import app.kaeri.tv.ui.TvTitle
import app.kaeri.tv.ui.focusSoon
import app.kaeri.tv.ui.tryFocus

/** Where focus stands on the starting screen, shared with the player's key handling. */
class StartFocus {
    /** The button the first direction press lands on while a start runs. */
    val target = FocusRequester()

    /** Whether the row has more than Cancel, so a press has somewhere to land. */
    var ready by mutableStateOf(false)

    /** Whether a button of the screen holds focus. */
    var held by mutableStateOf(false)
}

private val SHADOW = Shadow(Color.Black.copy(alpha = 0.7f), Offset(0f, 1f), 1f)

/**
 * The starting screen, over the video until the first frame lifts it. [up]: it is showing; [onClose]
 * stops the start, or closes the failure.
 */
@Composable
internal fun StartingLayer(model: PlayerModel, up: Boolean, focus: StartFocus, onClose: () -> Unit) {
    val reduced = LocalReducedMotion.current
    val shown by animateFloatAsState(if (up) 1f else 0f, if (reduced) snap() else Motion.fadeIn(), label = "starting") // i18n-skip
    if (shown == 0f) return
    val store = LocalAppStore.current
    val entries by store.titles.entries.collectAsStateWithLifecycle()
    val tables by store.titles.episodes.collectAsStateWithLifecycle()
    val id = model.titleId
    val wide = id?.let { entries[it] }?.let { HeroArt.of(it, tables[id]).url } ?: model.starting?.bannerUrl ?: model.bannerUrl
    Box(Modifier.fillMaxSize().graphicsLayer { alpha = shown }.background(ObiSilk.Ground)) {
        StartingArt(wide, Modifier.fillMaxSize())
        StartingColumn(model, reduced, focus, onClose, Modifier.fillMaxSize())
    }
}

/** The art over the whole screen, and the ground over it, 55 % or 80 % by how bright the art is. */
@Composable
private fun StartingArt(url: String?, modifier: Modifier) {
    val store = LocalAppStore.current
    val arrived = remember(url) { url?.let(store.artwork::arrivals) }
    val arrivals = arrived?.collectAsStateWithLifecycle()?.value
    val art = url?.let { arrivals; store.artwork.image(it) }
    LaunchedEffect(url, art == null) { if (url != null) store.artwork.request(url, 1920) }
    val ground = remember(art?.image) { art?.image?.let(ArtLuminance::ground) ?: ArtLuminance.NORMAL_GROUND }
    val reduced = LocalReducedMotion.current
    val fade = remember(url) { Animatable(if (art != null) 1f else 0f) }
    LaunchedEffect(url, art != null) {
        if (art != null) if (reduced) fade.snapTo(1f) else fade.animateTo(1f, Motion.fadeIn())
    }
    Box(modifier) {
        if (art != null) {
            val bitmap = remember(art) { art.image.asImageBitmap() }
            Image(bitmap, contentDescription = null, contentScale = ContentScale.Crop, modifier = Modifier.fillMaxSize().graphicsLayer { alpha = fade.value })
        }
        Box(Modifier.fillMaxSize().background(ObiSilk.Ground.copy(alpha = ground)))
    }
}

@Composable
private fun StartingColumn(model: PlayerModel, reduced: Boolean, focus: StartFocus, onClose: () -> Unit, modifier: Modifier) {
    val context = LocalContext.current
    val failure = model.failure
    val progress = failure?.trail ?: model.progress
    val now = model.clock
    var open by remember { mutableStateOf(StartDetailsMemory.open(context)) }
    val hasDetails = if (failure != null) progress != null && !failure.notAired && !failure.noSources else progress != null && progress.waited(now) >= StartTiming.TOOLS && !model.startWaiting(progress)
    val fraction by animateFloatAsState(if (open && hasDetails) 1f else 0f, if (reduced) Motion.fadeIn() else Motion.standard(), label = "details") // i18n-skip
    val said = startingWords(context, model, progress, failure, now)
    val addStore = LocalAddStore.current
    val store = LocalAppStore.current
    val providers by store.providers.rows.collectAsStateWithLifecycle()
    val actions = startActions(progress, failure, model.startWaiting(progress), now, store.match.canFix(providers), hasDetails)
        .filter { it != StartAction.Downloads && (it != StartAction.AddStore || addStore != null) }

    StartColumn(
        width = 680.dp,
        open = fraction,
        top = 35.dp,
        bottom = 27.dp,
        modifier = modifier,
        details = { DetailsList(model, progress, failure, now, focus) },
    ) {
        Column(Modifier.fillMaxWidth()) {
            Text(model.showTitle, style = TvTitle.page.copy(shadow = SHADOW), color = ObiSilk.Silk, maxLines = 2, overflow = TextOverflow.Ellipsis)
            Text(
                model.episodeLine(context),
                style = MaterialTheme.typography.bodyLarge.copy(fontSize = 16.sp, lineHeight = 24.sp, shadow = SHADOW),
                color = ObiSilk.Haze,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.padding(top = 6.dp),
            )
            Box(Modifier.padding(top = 14.dp).alpha(if (failure == null) 1f else 0f)) {
                KaeriLoaderLine(progress?.determinate(now).takeIf { failure == null }, reduced, thickness = 3.dp)
            }
            // One line for the step and room for two under it, always, so nothing arriving or
            // wrapping moves what is below. The two lines are read as one.
            val density = LocalDensity.current
            val stepRoom = with(density) { 24.sp.toDp() }
            val detailRoom = with(density) { 48.sp.toDp() }
            Column(Modifier.padding(top = 12.dp).semantics(mergeDescendants = true) { }) {
                Box(Modifier.heightIn(min = stepRoom)) {
                    FrameCrossfade(said.step to said.failed) { (text, failed) ->
                        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                            if (failed) Box(Modifier.size(10.dp).clip(CircleShape).background(ObiSilk.Vermilion))
                            Text(
                                text,
                                style = MaterialTheme.typography.titleMedium.copy(fontSize = 18.sp, lineHeight = 24.sp, fontWeight = FontWeight.Medium, shadow = SHADOW),
                                color = ObiSilk.Silk,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                    }
                }
                Box(Modifier.padding(top = 3.dp).heightIn(min = detailRoom)) {
                    FrameCrossfade(said.facts) { facts ->
                        Text(
                            if (said.seconds != null && !said.failed) listOfNotNull(facts, said.seconds).filter { it.isNotEmpty() }.joinToString(" · ") else facts,
                            style = MaterialTheme.typography.bodyLarge.copy(fontSize = 16.sp, lineHeight = 24.sp, shadow = SHADOW).tabular(),
                            color = ObiSilk.Haze,
                            maxLines = if (said.failed) 2 else 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
            }
            if (model.startingDemo) {
                Text(stringResource(R.string.player_demo_credit), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, modifier = Modifier.padding(top = 4.dp))
            }
            Box(Modifier.padding(top = 24.dp)) {
                Buttons(model, progress, failure, actions, open, reduced, focus, addStore, onToggle = {
                    open = !open
                    StartDetailsMemory.set(context, open)
                }, onClose)
            }
        }
        val message = startingAnnouncement(context, model, progress, failure, now)
        Box(
            Modifier.size(1.dp).semantics {
                liveRegion = if (failure != null) LiveRegionMode.Assertive else LiveRegionMode.Polite
                contentDescription = message
            },
        )
    }
}

@Composable
private fun Buttons(
    model: PlayerModel,
    progress: StartProgress?,
    failure: PlaybackFailure?,
    wanted: List<StartAction>,
    open: Boolean,
    reduced: Boolean,
    focus: StartFocus,
    addStore: (() -> Unit)?,
    onToggle: () -> Unit,
    onClose: () -> Unit,
) {
    val context = LocalContext.current
    val store = LocalAppStore.current
    // Buttons keep their room once they have arrived, and the ones before them never move.
    val arrived = remember(failure != null) { mutableStateListOf<StartAction>() }
    for (action in wanted) if (action !in arrived) arrived += action
    val primary = if (failure != null) primaryAction(wanted) else null
    val land = if (failure != null) null else (wanted.firstOrNull { it == StartAction.Choose } ?: wanted.firstOrNull { it != StartAction.Cancel })
    focus.ready = failure == null && wanted.size > 1
    val first = primary ?: wanted.firstOrNull()
    val cancel = remember { FocusRequester() }
    val scope = rememberCoroutineScope()
    val wantedNow by rememberUpdatedState(wanted)
    // The row's height is the tallest it comes to, from the start, and it wraps like the phone's:
    // the buttons that can arrive are laid out unseen, so nothing below or above moves.
    ReservedHeight(reserve = {
        Box {
            for (set in listOf(
                listOf(StartAction.Cancel, StartAction.FromBeginning, StartAction.Choose, StartAction.Details, StartAction.Skip),
                listOf(StartAction.Choose, StartAction.TryAgain, StartAction.Details, StartAction.Close),
                listOf(StartAction.FixMatch, StartAction.Choose, StartAction.Details, StartAction.Close),
            )) {
                ButtonsFlow(Modifier) {
                    for (action in set) TonalButton(onClick = {}, enabled = false, height = 56.dp) { ButtonLabel(action.label(context, progress, failure ?: FAILURE_SAMPLE)) }
                }
            }
        }
    }) {
        ButtonsFlow(Modifier.onFocusChanged { focus.held = it.hasFocus }) {
            for (action in arrived) {
                val applies = action in wanted
                val alpha by animateFloatAsState(if (applies) 1f else 0f, if (reduced) snap() else if (applies) Motion.fadeIn() else Motion.fadeOut(), label = "button") // i18n-skip
                val tall = primary != null
                val mine = when {
                    failure != null && action == first -> Modifier.focusRequester(focus.target)
                    action == land -> Modifier.focusRequester(focus.target)
                    action == StartAction.Cancel -> Modifier.focusRequester(cancel)
                    else -> Modifier
                }
                // A button that holds focus and stops applying hands it on, to Choose source or Cancel,
                // rather than let it fall to nowhere; a press that moved it elsewhere is left alone.
                val handOn = Modifier.onFocusChanged { state ->
                    if (!state.hasFocus && action !in wantedNow && failure == null) {
                        val choose = StartAction.Choose
                        scope.launch { if (choose in wantedNow && choose != action) focus.target.focusSoon() else cancel.focusSoon() }
                    }
                }
                Box(Modifier.alpha(alpha).then(if (applies) handOn else Modifier.then(handOn).clearAndSetSemantics { })) {
                    val label = action.label(context, progress, failure)
                    val go = {
                        when (action) {
                            StartAction.Cancel, StartAction.Close -> onClose()
                            StartAction.FromBeginning -> model.playFromBeginning()
                            StartAction.Choose -> model.chooseSource()
                            StartAction.Details -> onToggle()
                            StartAction.Skip -> model.skipSource()
                            StartAction.TryAgain -> model.tryAgain()
                            StartAction.AddStore -> addStore?.invoke()
                            StartAction.FixMatch -> failure?.id?.let { id ->
                                model.viewerClosed()
                                store.match.open(id, fromMenu = false)
                            }
                            StartAction.Downloads -> {}
                        }
                        Unit
                    }
                    if (action == primary) {
                        AmberButton(onClick = go, icon = null, enabled = applies, modifier = mine) { ButtonLabel(label) }
                    } else {
                        TonalButton(onClick = go, enabled = applies, height = if (tall) 56.dp else 48.dp, modifier = mine) {
                            ButtonLabel(label)
                            if (action == StartAction.Details) {
                                Spacer(Modifier.width(8.dp))
                                StartChevron(open, reduced, LocalContentColor.current)
                            }
                        }
                    }
                }
            }
        }
    }
    // After a failure the primary takes focus at once; during a start nothing does.
    LaunchedEffect(failure) { if (failure != null) focus.target.focusSoon() }
}

private val FAILURE_SAMPLE = PlaybackFailure(null, "")

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun ButtonsFlow(modifier: Modifier, content: @Composable () -> Unit) {
    FlowRow(
        modifier.fillMaxWidth().heightIn(min = 56.dp),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalArrangement = Arrangement.Center,
        itemVerticalAlignment = Alignment.CenterVertically,
    ) { content() }
}

/** The steps of this start, a row each, scrolling under the buttons when they do not fit. */
@Composable
private fun DetailsList(model: PlayerModel, progress: StartProgress?, failure: PlaybackFailure?, now: Long, focus: StartFocus) {
    val context = LocalContext.current
    val shown = failure?.trail?.details(context, failure.endedAt) ?: progress?.details(context, now).orEmpty()
    val translation = progress?.translation() ?: failure?.trail?.translation()
    Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).padding(top = 6.dp)) {
        for (row in shown) DetailsRow(row, model)
        if (translation != null) {
            Box(Modifier.fillMaxWidth().padding(vertical = 6.dp).height(1.dp).background(ObiSilk.Silk.copy(alpha = 0.10f)))
            Row(Modifier.fillMaxWidth().heightIn(min = 52.dp), verticalAlignment = Alignment.CenterVertically) {
                Spacer(Modifier.width(28.dp))
                Text(Strings.playerStartTranslation(context), style = MaterialTheme.typography.bodyLarge.copy(fontSize = 16.sp, lineHeight = 24.sp), color = ObiSilk.Haze, modifier = Modifier.weight(1f))
                val can = model.menus != null && failure == null
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    for (option in listOf(Translation.SUB, Translation.DUB)) {
                        val on = translation == option
                        Box(Modifier.alpha(if (on) 1f else 0.6f)) {
                            TonalButton(onClick = { if (!on) model.chooseTranslation(option) }, enabled = can || on, height = 40.dp) {
                                ButtonLabel(if (option == Translation.SUB) Strings.audioSubbed(context) else Strings.audioDubbed(context))
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun DetailsRow(row: DetailRow, model: PlayerModel) {
    Row(Modifier.fillMaxWidth().heightIn(min = 44.dp), verticalAlignment = Alignment.CenterVertically) {
        Canvas(Modifier.width(16.dp).height(12.dp)) {
            val c = Offset(size.width / 2, size.height / 2)
            when (row.kind) {
                RowKind.Done -> drawPath(
                    Path().apply { moveTo(c.x - 4f, c.y); lineTo(c.x - 1.5f, c.y + 3f); lineTo(c.x + 4f, c.y - 3f) },
                    ObiSilk.Haze, style = Stroke(1.6.dp.toPx(), cap = StrokeCap.Round, join = StrokeJoin.Round),
                )
                RowKind.Empty -> drawLine(ObiSilk.Haze, Offset(c.x - 3.dp.toPx(), c.y), Offset(c.x + 3.dp.toPx(), c.y), 1.6.dp.toPx(), StrokeCap.Round)
                RowKind.Now -> drawCircle(ObiSilk.Silk, 3.2.dp.toPx(), c)
                RowKind.Rest -> drawLine(ObiSilk.Edge, Offset(c.x - 3.dp.toPx(), c.y), Offset(c.x + 3.dp.toPx(), c.y), 1.6.dp.toPx(), StrokeCap.Round)
            }
        }
        Spacer(Modifier.width(12.dp))
        Text(
            row.text,
            style = MaterialTheme.typography.bodyLarge.copy(fontSize = 16.sp, lineHeight = 24.sp).tabular(),
            color = if (row.kind == RowKind.Now) ObiSilk.Silk else ObiSilk.Haze,
            fontWeight = if (row.kind == RowKind.Now) FontWeight.Medium else null,
            modifier = Modifier.weight(1f),
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
        )
        row.seconds?.let {
            Text(Strings.playerStartSeconds(LocalContext.current, it), style = MaterialTheme.typography.bodyLarge.copy(fontSize = 16.sp, lineHeight = 24.sp).tabular(), color = ObiSilk.Haze, modifier = Modifier.padding(start = 8.dp))
        }
        row.skip?.let {
            Spacer(Modifier.width(12.dp))
            TonalButton(onClick = { model.skipSource() }, height = 40.dp) { ButtonLabel(it) }
        }
    }
}
