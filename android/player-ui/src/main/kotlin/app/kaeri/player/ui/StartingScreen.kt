// The player's starting screen on a phone, as the board draws it: the title's art under a ground,
// and one 520 dp column in the middle with the show, the episode, Kaeri's line, the step the start
// is on, what it knows of it, and the few buttons that apply. It stays where it is while a start
// fills it in. When a start fails the same column says why and what to do next.

package app.kaeri.player.ui

import android.content.Context
import androidx.compose.animation.Crossfade
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.snap
import androidx.compose.foundation.Canvas
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
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
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
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.Shadow
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.foundation.Image
import androidx.compose.ui.graphics.asImageBitmap
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.design.KaeriLoaderLine
import app.kaeri.design.Motion
import app.kaeri.design.ObiSilk
import app.kaeri.design.ShowTitle
import app.kaeri.design.StartChevron
import app.kaeri.design.StartColumn
import app.kaeri.design.rememberSystemReducedMotion
import app.kaeri.design.tabular
import app.kaeri.engine.core.Translation
import app.kaeri.engine.model.PlayerChoice
import androidx.compose.ui.res.stringResource
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.player.ArtLuminance
import app.kaeri.player.DetailRow
import app.kaeri.player.PlaybackFailure
import app.kaeri.player.PlayerModel
import app.kaeri.player.RowKind
import app.kaeri.player.StartAction
import app.kaeri.player.startingAnnouncement
import app.kaeri.player.startingWords
import app.kaeri.player.label
import app.kaeri.player.primaryAction
import app.kaeri.player.startActions
import app.kaeri.player.StartDetailsMemory
import app.kaeri.player.StartProgress
import app.kaeri.player.StartTiming
import app.kaeri.store.HeroArt
import app.kaeri.ui.LocalAppStore
import app.kaeri.ui.LocalNavigatorOrNull
import app.kaeri.ui.openDownloads
import app.kaeri.ui.openStores

/** The column's width on the board: centred in the window, its content leading-aligned. */
private val COLUMN = 520.dp

/**
 * The starting screen, in the player at full size: the ground, the title's wide art cropped to the
 * video rect under a ground of 55 % (80 % over bright art), and the column, until the first frame
 * lifts it on the fade-in curve (T2).
 */
@Composable
internal fun StartingLayer(model: PlayerModel, rect: VideoRect, art: Boolean, onChooseSource: () -> Unit) {
    val up = model.isPresented && (model.starting != null || !model.hasPicture || model.failure != null)
    val reduced = rememberSystemReducedMotion()
    val shown by animateFloatAsState(if (up) 1f else 0f, if (reduced) snap() else Motion.fadeIn(), label = "starting") // i18n-skip
    if (shown == 0f) return
    val store = LocalAppStore.current
    val entries by store.titles.entries.collectAsStateWithLifecycle()
    val tables by store.titles.episodes.collectAsStateWithLifecycle()
    val id = model.titleId
    // The hero-art rule, as the title page has it: the banner, else a still already seen.
    val wide = if (!art) null else id?.let { entries[it] }?.let { HeroArt.of(it, tables[id]).url } ?: model.starting?.bannerUrl ?: model.bannerUrl
    Box(Modifier.fillMaxSize().graphicsLayer { alpha = shown }.background(ObiSilk.Ground)) {
        Box(Modifier.offset(rect.x, rect.y).size(rect.width, rect.height)) {
            StartingArt(wide, Modifier.fillMaxSize())
            StartingColumn(model, reduced, onChooseSource, Modifier.fillMaxSize())
        }
    }
}

/**
 * The art and the ground over it. How bright the art is where the column sits is measured once, as
 * the art is here and before it fades in, so the ground's depth never changes on screen.
 */
@Composable
private fun StartingArt(url: String?, modifier: Modifier) {
    val store = LocalAppStore.current
    val arrived = remember(url) { url?.let(store.artwork::arrivals) }
    val arrivals = arrived?.collectAsStateWithLifecycle()?.value
    val context = LocalContext.current
    val art = url?.let { arrivals; store.artwork.image(it) }
    LaunchedEffect(url) { if (url != null) store.artwork.request(url, 1920) }
    val ground = remember(art?.image) { art?.image?.let(ArtLuminance::ground) ?: ArtLuminance.NORMAL_GROUND }
    val fade = remember(url) { androidx.compose.animation.core.Animatable(if (art != null) 1f else 0f) }
    LaunchedEffect(url, art != null) { if (art != null) fade.animateTo(1f, Motion.fadeIn()) }
    Box(modifier) {
        if (art != null) {
            val bitmap = remember(art) { art.image.asImageBitmap() }
            Image(bitmap, contentDescription = null, contentScale = ContentScale.Crop, modifier = Modifier.fillMaxSize().graphicsLayer { alpha = fade.value })
        }
        Box(Modifier.fillMaxSize().background(ObiSilk.Ground.copy(alpha = ground)))
    }
}

private val StepStyle
    @Composable get() = MaterialTheme.typography.titleMedium.copy(
        fontSize = 16.sp,
        lineHeight = 24.sp,
        fontWeight = FontWeight.Medium,
        shadow = Shadow(Color.Black.copy(alpha = 0.7f), Offset(0f, 1f), 1f),
    )

private val DetailStyle
    @Composable get() = MaterialTheme.typography.bodyMedium.copy(
        fontSize = 14.sp,
        lineHeight = 20.sp,
        shadow = Shadow(Color.Black.copy(alpha = 0.7f), Offset(0f, 1f), 1f),
    ).tabular()

@Composable
private fun StartingColumn(model: PlayerModel, reduced: Boolean, onChooseSource: () -> Unit, modifier: Modifier) {
    val context = LocalContext.current
    val failure = model.failure
    val progress = failure?.trail ?: model.progress
    val now = model.clock
    var open by remember { mutableStateOf(StartDetailsMemory.open(context)) }
    val hasDetails = if (failure != null) progress != null && !failure.notAired && !failure.noSources else progress != null && progress.waited(now) >= StartTiming.TOOLS && !model.offline
    val fraction by animateFloatAsState(if (open && hasDetails) 1f else 0f, if (reduced) Motion.fadeIn() else Motion.standard(), label = "details") // i18n-skip

    StartColumn(
        width = COLUMN,
        open = fraction,
        top = 24.dp,
        bottom = 24.dp,
        modifier = modifier,
        details = { DetailsList(model, progress, failure, now) },
    ) {
        Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.Start) {
            Text(
                model.showTitle,
                style = ShowTitle.page.copy(shadow = Shadow(Color.Black.copy(alpha = 0.7f), Offset(0f, 1f), 1f)),
                color = ObiSilk.Silk,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                model.episodeLine(context),
                style = DetailStyle.copy(fontSize = 14.sp),
                color = ObiSilk.Haze,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.padding(top = 6.dp),
            )
            // Kaeri's line keeps its room through a failure, so the column does not close up.
            Box(Modifier.padding(top = 14.dp).alpha(if (failure == null) 1f else 0f)) {
                KaeriLoaderLine(progress?.determinate(now).takeIf { failure == null }, reduced)
            }
            val said = startingWords(context, model, progress, failure, now)
            Column(Modifier.padding(top = 12.dp).semantics(mergeDescendants = true) { }) {
                Crossfade(said.step to said.failed, animationSpec = if (reduced) Motion.fadeIn() else Motion.standard(), label = "step") { (text, failed) -> // i18n-skip
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        if (failed) Box(Modifier.size(8.dp).clip(CircleShape).background(ObiSilk.Vermilion))
                        Text(text, style = StepStyle, color = ObiSilk.Silk, maxLines = 2, overflow = TextOverflow.Ellipsis)
                    }
                }
                Box(Modifier.padding(top = 3.dp).heightIn(min = 20.dp)) {
                    Crossfade(said.facts, animationSpec = if (reduced) Motion.fadeIn() else Motion.standard(), label = "detail") { facts -> // i18n-skip
                        Text(
                            if (said.seconds != null && !said.failed) listOfNotNull(facts, said.seconds).filter { it.isNotEmpty() }.joinToString(" · ") else facts,
                            style = DetailStyle,
                            color = ObiSilk.Haze,
                            maxLines = 3,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
            }
            if (model.startingDemo) {
                Text(stringResource(R.string.player_demo_credit), style = DetailStyle, color = ObiSilk.Haze, modifier = Modifier.padding(top = 4.dp))
            }
            Box(Modifier.padding(top = 16.dp)) {
                StartingButtons(model, progress, failure, hasDetails, open, reduced, onToggleDetails = {
                    open = !open
                    StartDetailsMemory.set(context, open)
                }, onChooseSource)
            }
        }
        Announcer(model, progress, failure, now)
    }
}

/** Tells a screen reader the start, a slow start once, and how it ended; never every step. */
@Composable
private fun Announcer(model: PlayerModel, progress: StartProgress?, failure: PlaybackFailure?, now: Long) {
    val message = startingAnnouncement(LocalContext.current, model, progress, failure, now)
    Box(
        Modifier.size(1.dp).semantics {
            liveRegion = if (failure != null) LiveRegionMode.Assertive else LiveRegionMode.Polite
            contentDescription = message
        },
    )
}

/** The row under the lines: the buttons that apply, in the order they arrived, each laid out once it has. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun StartingButtons(
    model: PlayerModel,
    progress: StartProgress?,
    failure: PlaybackFailure?,
    hasDetails: Boolean,
    open: Boolean,
    reduced: Boolean,
    onToggleDetails: () -> Unit,
    onChooseSource: () -> Unit,
) {
    val context = LocalContext.current
    // Previews and tests may have none; only Downloads and Add a store need it.
    val navigator = LocalNavigatorOrNull.current
    val store = LocalAppStore.current
    val providers by store.providers.rows.collectAsStateWithLifecycle()
    val now = model.clock
    val canFix = store.match.canFix(providers)
    val wanted = startActions(progress, failure, model.offline, now, canFix, hasDetails)
    // A button that has arrived keeps its room, so the ones before it never move; one that no
    // longer applies fades out and leaves it empty until the start ends.
    val arrived = remember(failure != null) { mutableStateListOf<StartAction>() }
    for (wish in wanted) if (wish !in arrived) arrived += wish
    val primary = if (failure != null) primaryAction(wanted) else null
    FlowRow(
        Modifier.fillMaxWidth().heightIn(min = 48.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.Center,
        itemVerticalAlignment = Alignment.CenterVertically,
    ) {
        for (wish in arrived) {
            val applies = wish in wanted
            val alpha by animateFloatAsState(if (applies) 1f else 0f, if (reduced) snap() else if (applies) Motion.fadeIn() else Motion.fadeOut(), label = "button") // i18n-skip
            Box(Modifier.alpha(alpha).then(if (applies) Modifier else Modifier.clearAndSetSemantics { })) {
                val enabled = applies
                val label = wish.label(context, progress, failure)
                when (wish) {
                    StartAction.Cancel -> TextButton({ model.viewerClosed() }, enabled = enabled) { Text(label, color = ObiSilk.Silk) }
                    StartAction.Close -> TextButton({ model.viewerClosed() }, enabled = enabled) { Text(label, color = ObiSilk.Silk) }
                    StartAction.Details -> TextButton(onToggleDetails, enabled = enabled) {
                        Text(label, color = ObiSilk.Silk)
                        Spacer(Modifier.width(6.dp))
                        StartChevron(open, reduced, ObiSilk.Silk)
                    }
                    else -> if (wish == primary) {
                        Button(
                            onClick = { act(wish, model, failure, store, navigator, onChooseSource) },
                            enabled = enabled,
                            colors = ButtonDefaults.buttonColors(containerColor = ObiSilk.Amber, contentColor = ObiSilk.Ground),
                        ) { Text(label) }
                    } else {
                        FilledTonalButton(
                            onClick = { act(wish, model, failure, store, navigator, onChooseSource) },
                            enabled = enabled,
                            colors = ButtonDefaults.filledTonalButtonColors(containerColor = ObiSilk.RaisedSelected, contentColor = ObiSilk.Silk),
                        ) { Text(label) }
                    }
                }
            }
        }
    }
}

private fun act(
    wish: StartAction,
    model: PlayerModel,
    failure: PlaybackFailure?,
    store: app.kaeri.store.AppStore,
    navigator: app.kaeri.ui.Navigator?,
    onChooseSource: () -> Unit,
) {
    when (wish) {
        StartAction.FromBeginning -> model.playFromBeginning()
        StartAction.Choose -> onChooseSource()
        StartAction.Skip -> model.skipSource()
        StartAction.TryAgain -> model.tryAgain()
        StartAction.Downloads -> {
            model.viewerClosed()
            navigator?.openDownloads()
        }
        StartAction.AddStore -> {
            model.viewerClosed()
            navigator?.openStores()
        }
        StartAction.FixMatch -> {
            val id = failure?.id ?: return
            model.viewerClosed()
            store.match.open(id, fromMenu = false)
        }
        else -> {}
    }
}

/** The steps of this start: each source asked and what it said, then the stream, and the audio. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun DetailsList(model: PlayerModel, progress: StartProgress?, failure: PlaybackFailure?, now: Long) {
    val context = LocalContext.current
    val shown = failure?.trail?.let { trail -> trail.details(context, failure.endedAt) } ?: progress?.details(context, now).orEmpty()
    val translation = progress?.translation() ?: failure?.trail?.translation()
    Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).padding(top = 6.dp)) {
        for (row in shown) DetailsRow(row, model)
        if (translation != null) {
            Box(Modifier.fillMaxWidth().padding(vertical = 6.dp).height(1.dp).background(ObiSilk.Silk.copy(alpha = 0.10f)))
            Row(Modifier.fillMaxWidth().heightIn(min = 52.dp), verticalAlignment = Alignment.CenterVertically) {
                Spacer(Modifier.width(16.dp + 12.dp))
                Text(Strings.playerStartTranslation(context), style = DetailStyle, color = ObiSilk.Haze, modifier = Modifier.weight(1f))
                // The other translation restarts the player behind this screen, so it can be asked
                // for once the player has opened a stream; before that it shows what was found.
                val can = model.menus != null && failure == null
                SingleChoiceSegmentedButtonRow {
                    for ((index, option) in listOf(Translation.SUB, Translation.DUB).withIndex()) {
                        SegmentedButton(
                            selected = translation == option,
                            onClick = { if (translation != option) model.chooseTranslation(option) },
                            shape = SegmentedButtonDefaults.itemShape(index, 2),
                            enabled = can || translation == option,
                            modifier = Modifier.height(40.dp),
                            label = { Text(if (option == Translation.SUB) Strings.audioSubbed(context) else Strings.audioDubbed(context)) },
                        )
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
            val soft = ObiSilk.Haze
            when (row.kind) {
                RowKind.Done -> drawPath(
                    Path().apply { moveTo(c.x - 4f, c.y); lineTo(c.x - 1.5f, c.y + 3f); lineTo(c.x + 4f, c.y - 3f) },
                    soft, style = Stroke(1.6.dp.toPx(), cap = StrokeCap.Round, join = StrokeJoin.Round),
                )
                RowKind.Empty -> drawLine(soft, Offset(c.x - 3.dp.toPx(), c.y), Offset(c.x + 3.dp.toPx(), c.y), 1.6.dp.toPx(), StrokeCap.Round)
                RowKind.Now -> drawCircle(ObiSilk.Silk, 3.2.dp.toPx(), c)
                RowKind.Rest -> drawLine(ObiSilk.Edge, Offset(c.x - 3.dp.toPx(), c.y), Offset(c.x + 3.dp.toPx(), c.y), 1.6.dp.toPx(), StrokeCap.Round)
            }
        }
        Spacer(Modifier.width(12.dp))
        Text(
            row.text,
            style = DetailStyle,
            color = if (row.kind == RowKind.Now) ObiSilk.Silk else ObiSilk.Haze,
            fontWeight = if (row.kind == RowKind.Now) FontWeight.Medium else null,
            modifier = Modifier.weight(1f),
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
        )
        row.seconds?.let { Text(Strings.durationSeconds(LocalContext.current, it), style = DetailStyle, color = ObiSilk.Haze, modifier = Modifier.padding(start = 8.dp)) }
        row.skip?.let { TextButton({ model.skipSource() }) { Text(it, color = ObiSilk.Silk) } }
    }
}
