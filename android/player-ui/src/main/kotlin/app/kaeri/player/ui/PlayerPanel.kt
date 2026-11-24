// The player's Episodes and Sources panel on a phone held sideways (player panel board, Android
// phone): a side panel 400 dp from the end edge, full height, in the style of Material 3's modal
// side sheet, over the picture dimmed to the ground at 35 %. Back or a tap on the dimmed picture
// closes it. Episodes reveals a row on the first tap, and Play then plays it; Sources switches the
// playing episode to another stream or release from the playhead.

package app.kaeri.player.ui

import android.content.Context
import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.Crossfade
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.snap
import androidx.compose.animation.core.tween
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.displayCutout
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.relocation.BringIntoViewRequester
import androidx.compose.foundation.relocation.bringIntoViewRequester
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.PrimaryTabRow
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Surface
import androidx.compose.material3.Tab
import androidx.compose.material3.TabRowDefaults
import androidx.compose.material3.TabRowDefaults.tabIndicatorOffset
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.design.Motion
import app.kaeri.design.ObiSilk
import app.kaeri.design.PanelGlyphs
import app.kaeri.design.rememberSystemReducedMotion
import app.kaeri.engine.core.AnilistId
import app.kaeri.engine.core.SourceCandidate
import app.kaeri.engine.core.Translation
import app.kaeri.engine.model.EpisodeRow
import app.kaeri.engine.model.ListingState
import app.kaeri.engine.model.SourceOption
import app.kaeri.i18n.Strings
import app.kaeri.player.HeldPicture
import app.kaeri.player.ListingLine
import app.kaeri.player.PanelTab
import app.kaeri.player.PanelWords
import app.kaeri.player.PlayerModel
import app.kaeri.player.SwitchTarget
import app.kaeri.store.line
import app.kaeri.store.text
import app.kaeri.ui.ArtworkImage
import app.kaeri.ui.LocalAppStore
import app.kaeri.ui.spoilerProtected

/** The panel's width from the end edge. */
internal val PANEL_WIDTH = 400.dp

/** The panel's fill, a step above the ground. */
private val PanelFill = ObiSilk.Raised

/** The picture beside the open panel: dimmed to the ground at 35 %. */
private const val PANEL_DIM = 0.35f

/** The panel, over the player: the dim, and the panel sliding in from the end edge. */
@Composable
internal fun PlayerPanelLayer(model: PlayerModel) {
    val open = model.panelOpen && !model.inPip
    val reduced = rememberSystemReducedMotion()
    BackHandler(open) { model.closePanel() }
    AnimatedVisibility(
        open,
        enter = fadeIn(if (reduced) tween(150) else tween(300)),
        exit = fadeOut(tween(200)),
    ) {
        Box(
            Modifier
                .fillMaxSize()
                .background(ObiSilk.Ground.copy(alpha = PANEL_DIM))
                .pointerInput(Unit) { detectTapGestures { model.closePanel() } }
                .testTag("panel-dim"),
        )
    }
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.CenterEnd) {
        AnimatedVisibility(
            open,
            // 0.3 s on standard in from the trailing edge, 0.2 s quick out; a fade alone with
            // Remove animations on.
            enter = if (reduced) fadeIn(tween(150)) else slideInHorizontally(Motion.standard()) { it } + fadeIn(tween(150)),
            exit = if (reduced) fadeOut(tween(150)) else slideOutHorizontally(Motion.quick()) { it } + fadeOut(tween(150, delayMillis = 50)),
        ) {
            val label = Strings.playerPanel(LocalContext.current)
            Surface(
                color = PanelFill,
                shape = RoundedCornerShape(topStart = 16.dp, bottomStart = 16.dp),
                shadowElevation = 8.dp,
                modifier = Modifier
                    .width(PANEL_WIDTH)
                    .fillMaxHeight()
                    .semantics { paneTitle = label }
                    .testTag("player-panel")
                    // Taps inside never reach the dim or the video.
                    .pointerInput(Unit) { detectTapGestures { } },
            ) {
                Box(Modifier.windowInsetsPadding(WindowInsets.displayCutout.only(WindowInsetsSides.End))) {
                    PanelContent(model, reduced)
                }
            }
        }
    }
}

@Composable
private fun PanelContent(model: PlayerModel, reduced: Boolean) {
    val context = LocalContext.current
    Column(Modifier.fillMaxSize()) {
        val tabs = listOf(PanelTab.Episodes, PanelTab.Sources)
        val selected = tabs.indexOf(model.panelTab)
        PrimaryTabRow(
            selectedTabIndex = selected,
            containerColor = Color.Transparent,
            contentColor = ObiSilk.Silk,
            divider = { Box(Modifier.fillMaxWidth().height(1.dp).background(ObiSilk.Silk.copy(alpha = 0.10f))) },
            indicator = {
                TabRowDefaults.PrimaryIndicator(
                    Modifier.tabIndicatorOffset(selected, matchContentSize = false),
                    width = 64.dp,
                    height = 3.dp,
                    color = ObiSilk.Silk,
                )
            },
            modifier = Modifier.padding(horizontal = 16.dp),
        ) {
            for (tab in tabs) {
                Tab(
                    selected = model.panelTab == tab,
                    onClick = { model.panelTab = tab },
                    selectedContentColor = ObiSilk.Silk,
                    unselectedContentColor = ObiSilk.Haze,
                    modifier = Modifier.height(48.dp).testTag("panel-tab-${tab.name.lowercase()}"), // i18n-skip
                ) {
                    Text(
                        if (tab == PanelTab.Episodes) Strings.playerEpisodes(context) else Strings.playerSources(context),
                        style = MaterialTheme.typography.titleSmall,
                    )
                }
            }
        }
        // The tabs crossfade over 0.3 s.
        Crossfade(model.panelTab, animationSpec = if (reduced) snap() else tween(300), label = "panel-tab") { tab -> // i18n-skip
            when (tab) {
                PanelTab.Episodes -> PanelEpisodes(model)
                PanelTab.Sources -> PanelSources(model)
            }
        }
    }
}

// The Episodes tab

/** The episode playing, by number. */
internal fun currentEpisode(model: PlayerModel): String? =
    model.menus?.episodes?.firstOrNull { it.current }?.number ?: model.staged?.episode

@Composable
private fun PanelEpisodes(model: PlayerModel) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val id = model.staged?.id ?: return
    val tables by store.titles.episodes.collectAsStateWithLifecycle()
    val entries by store.titles.entries.collectAsStateWithLifecycle()
    val rows = tables[id]
    LaunchedEffect(id) { if (store.titles.episodes.value[id] == null) store.loadEpisodes(id) }
    var revealed by rememberSaveable(id) { mutableStateOf<String?>(null) }
    if (rows == null) {
        PanelLoading()
        return
    }
    val current = currentEpisode(model)
    val metadataOnly = rows.any { it.metadataOnly }
    val spoilers = spoilerProtected(id)
    val state = rememberLazyListState(
        // The playing row near the top as the list opens, with the one before it above.
        initialFirstVisibleItemIndex = (rows.indexOfFirst { it.number == current } - 1).coerceAtLeast(0) + if (metadataOnly) 1 else 0,
    )
    LazyColumn(
        state = state,
        modifier = Modifier.fillMaxSize().testTag("panel-episodes"),
        contentPadding = androidx.compose.foundation.layout.PaddingValues(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 16.dp),
        verticalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        if (metadataOnly) item(key = "metadata-only") {
            Text(
                Strings.playerPanelMetadataOnly(context),
                style = DetailStyle,
                color = ObiSilk.Haze,
                modifier = Modifier.padding(start = 10.dp, end = 10.dp, top = 4.dp, bottom = 12.dp),
            )
        }
        items(rows, key = { it.number }) { row ->
            PanelEpisodeRow(
                model, id, row,
                banner = entries[id]?.bannerUrl,
                current = !metadataOnly && row.number == current,
                metadataOnly = metadataOnly,
                hidden = spoilers && row.spoilerAhead && row.number != current,
                revealed = revealed == row.number,
                reveal = { revealed = if (revealed == row.number) null else row.number },
            )
        }
    }
}

/** Seven rows the shape of the ones to come. */
@Composable
private fun PanelLoading() {
    val context = LocalContext.current
    Column(Modifier.fillMaxSize().padding(16.dp).semantics { paneTitle = Strings.playerSheetEpisodesLoading(context) }) {
        val widths = listOf(0.72f, 0.58f, 0.8f, 0.64f, 0.7f, 0.52f, 0.76f)
        for (w in widths) {
            Row(Modifier.padding(10.dp), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                Box(Modifier.size(112.dp, 63.dp).clip(RoundedCornerShape(8.dp)).background(ObiSilk.Silk.copy(alpha = 0.07f)))
                Column(Modifier.weight(1f).padding(top = 8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Box(Modifier.fillMaxWidth(w).height(14.dp).clip(RoundedCornerShape(4.dp)).background(ObiSilk.Silk.copy(alpha = 0.08f)))
                    Box(Modifier.fillMaxWidth(0.34f).height(11.dp).clip(RoundedCornerShape(4.dp)).background(ObiSilk.Silk.copy(alpha = 0.08f)))
                }
            }
        }
    }
}

/** How long a revealed row takes to open before it is scrolled into view. */
private const val REVEAL_SETTLE = 320L

private val TitleStyle = TextStyle(fontSize = 16.sp, lineHeight = 22.sp, fontWeight = FontWeight.Medium)
private val DetailStyle = TextStyle(fontSize = 14.sp, lineHeight = 20.sp)

/**
 * One episode: its still, number and title, and what the table says of it. Revealed, its whole
 * title, the synopsis cut to three lines with More, and its buttons.
 */
@Composable
private fun PanelEpisodeRow(
    model: PlayerModel,
    id: AnilistId,
    row: EpisodeRow,
    banner: String?,
    current: Boolean,
    metadataOnly: Boolean,
    hidden: Boolean,
    revealed: Boolean,
    reveal: () -> Unit,
) {
    val context = LocalContext.current
    val fill = when {
        current -> ObiSilk.Silk.copy(alpha = 0.12f)
        revealed -> ObiSilk.Silk.copy(alpha = 0.06f)
        else -> Color.Transparent
    }
    val notOut = PanelWords.notOut(row)
    // A row opened near the panel's foot scrolls up far enough to show its buttons, once it has opened.
    val bring = remember { BringIntoViewRequester() }
    LaunchedEffect(revealed) {
        if (revealed) {
            kotlinx.coroutines.delay(REVEAL_SETTLE)
            bring.bringIntoView()
        }
    }
    Column(
        Modifier
            .fillMaxWidth()
            .bringIntoViewRequester(bring)
            .clip(RoundedCornerShape(12.dp))
            .background(fill)
            .clickable(onClickLabel = null, role = Role.Button, onClick = reveal)
            .semantics { if (current) selected = true }
            .padding(10.dp)
            .animateContentSize(Motion.standard())
            .testTag("panel-episode-${row.number}"), // i18n-skip
    ) {
        Row(horizontalArrangement = Arrangement.spacedBy(16.dp), verticalAlignment = Alignment.CenterVertically) {
            Still(row, banner, current, metadataOnly, hidden, notOut, model)
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                val title = row.title?.takeIf { it.isNotBlank() }
                Text(
                    if (title != null) {
                        buildAnnotatedString {
                            withStyle(SpanStyle(color = ObiSilk.Haze)) { append(PanelWords.number(context, row.number)) }
                            append(" ")
                            append(title)
                        }
                    } else {
                        buildAnnotatedString { append(Strings.episodeNumberPlain(context, row.number)) }
                    },
                    style = TitleStyle,
                    color = ObiSilk.Silk,
                    maxLines = if (revealed) Int.MAX_VALUE else 2,
                    overflow = TextOverflow.Ellipsis,
                )
                val detail = PanelWords.detail(context, row, current, metadataOnly, model.position, model.duration)
                if (detail.isNotEmpty()) {
                    Text(detail, style = DetailStyle, color = ObiSilk.Haze, maxLines = if (revealed) Int.MAX_VALUE else 2, overflow = TextOverflow.Ellipsis)
                }
            }
        }
        AnimatedVisibility(
            revealed,
            // The row opens on the standard spring; the synopsis fades in 100 ms on.
            enter = expandVertically(Motion.standard()) + fadeIn(tween(200, delayMillis = 100)),
            exit = shrinkVertically(Motion.quick()) + fadeOut(tween(100)),
        ) {
            Revealed(model, id, row, current, metadataOnly, hidden, notOut)
        }
    }
}

@Composable
private fun Still(row: EpisodeRow, banner: String?, current: Boolean, metadataOnly: Boolean, hidden: Boolean, notOut: Boolean, model: PlayerModel) {
    val url = if (hidden || metadataOnly || notOut) null else row.thumbnail ?: banner
    Box(
        Modifier
            .size(112.dp, 63.dp)
            .clip(RoundedCornerShape(8.dp))
            .background(ObiSilk.Silk.copy(alpha = 0.07f))
            .graphicsLayer { alpha = if (row.completed && !current && !metadataOnly) 0.62f else 1f },
        contentAlignment = Alignment.BottomCenter,
    ) {
        if (url != null) ArtworkImage(url, plate = null, maxPixelSize = 320, modifier = Modifier.fillMaxSize())
        val progress = PanelWords.progress(row, current, metadataOnly, model.position, model.duration)
        if (progress != null && progress > 0f) {
            Box(Modifier.fillMaxWidth().padding(4.dp).height(3.dp).clip(RoundedCornerShape(2.dp)).background(ObiSilk.Silk.copy(alpha = 0.32f))) {
                Box(Modifier.fillMaxWidth(progress).fillMaxHeight().background(ObiSilk.Silk))
            }
        }
    }
}

/** The synopsis, three lines with More while it is cut, and the row's buttons. */
@Composable
private fun Revealed(model: PlayerModel, id: AnilistId, row: EpisodeRow, current: Boolean, metadataOnly: Boolean, hidden: Boolean, notOut: Boolean) {
    val context = LocalContext.current
    Column(Modifier.padding(top = 12.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        val synopsis = row.description?.takeIf { it.isNotBlank() }
        when {
            hidden && synopsis != null -> Text(
                Strings.episodeSpoilerHidden(context),
                style = DetailStyle.copy(fontStyle = androidx.compose.ui.text.font.FontStyle.Italic),
                color = ObiSilk.Haze,
            )
            synopsis != null && !notOut -> Synopsis(synopsis, row.number)
        }
        val actions = episodeActions(model, row, current, metadataOnly, notOut)
        if (actions.isNotEmpty()) {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                for (action in actions) {
                    PanelTextButton(action.kind.title(context), action.primary, Modifier.testTag("panel-action-${action.kind.name}"), onClick = action.run) // i18n-skip
                }
            }
        }
    }
}

/** A synopsis cut to three lines, with More to read it whole and Less to fold it back. */
@Composable
private fun Synopsis(text: String, number: String) {
    val context = LocalContext.current
    var whole by rememberSaveable(number) { mutableStateOf(false) }
    var cut by remember(text) { mutableStateOf(false) }
    Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
        Text(
            text,
            style = DetailStyle,
            color = ObiSilk.Haze,
            maxLines = if (whole) Int.MAX_VALUE else 3,
            overflow = TextOverflow.Ellipsis,
            onTextLayout = { if (!whole) cut = it.hasVisualOverflow },
            modifier = Modifier.animateContentSize(Motion.standard()),
        )
        if (cut || whole) {
            Text(
                if (whole) Strings.playerPanelLess(context) else Strings.playerPanelMore(context),
                style = DetailStyle.copy(fontWeight = FontWeight.Medium),
                color = ObiSilk.Silk,
                modifier = Modifier
                    .clip(RoundedCornerShape(8.dp))
                    .clickable(role = Role.Button) { whole = !whole }
                    .heightIn(min = 40.dp)
                    .padding(vertical = 10.dp)
                    .testTag("panel-more"),
            )
        }
    }
}

private class EpisodeAction(val kind: EpisodeActionKind, val run: () -> Unit) {
    val primary: Boolean get() = kind == EpisodeActionKind.Play || kind == EpisodeActionKind.Resume
}

/** What a revealed row can do, as the board has it. */
private fun episodeActions(model: PlayerModel, row: EpisodeRow, current: Boolean, metadataOnly: Boolean, notOut: Boolean): List<EpisodeAction> =
    episodeActionKinds(row, current, metadataOnly, notOut).map { kind ->
        EpisodeAction(kind) {
            when (kind) {
                EpisodeActionKind.MarkWatched -> model.markWatched(row.number)
                EpisodeActionKind.FromBeginningPlaying -> {
                    model.startOver()
                    model.closePanel()
                }
                EpisodeActionKind.Resume, EpisodeActionKind.Play -> {
                    model.playEpisode(row.number, fromStart = false)
                    model.closePanel()
                }
                EpisodeActionKind.FromBeginning -> {
                    model.playEpisode(row.number, fromStart = true)
                    model.closePanel()
                }
            }
        }
    }

/** A revealed row's buttons, in order. */
internal enum class EpisodeActionKind { Play, Resume, FromBeginning, FromBeginningPlaying, MarkWatched }

internal fun episodeActionKinds(row: EpisodeRow, current: Boolean, metadataOnly: Boolean, notOut: Boolean): List<EpisodeActionKind> = when {
    metadataOnly -> if (row.completed) emptyList() else listOf(EpisodeActionKind.MarkWatched)
    notOut -> emptyList()
    row.unsourced -> if (row.completed) emptyList() else listOf(EpisodeActionKind.MarkWatched)
    current -> listOf(EpisodeActionKind.FromBeginningPlaying)
    row.watched > 0 && !row.completed -> listOf(EpisodeActionKind.Resume, EpisodeActionKind.FromBeginning)
    row.completed -> listOf(EpisodeActionKind.Play)
    else -> listOf(EpisodeActionKind.Play, EpisodeActionKind.MarkWatched)
}

internal fun EpisodeActionKind.title(context: Context): String = when (this) {
    EpisodeActionKind.Play -> Strings.actionPlay(context)
    EpisodeActionKind.Resume -> Strings.actionResume(context)
    EpisodeActionKind.FromBeginning, EpisodeActionKind.FromBeginningPlaying -> Strings.episodePlayFromBeginning(context)
    EpisodeActionKind.MarkWatched -> Strings.episodeMarkWatched(context)
}

/** A filled amber button for the row's action, or an outlined one beside it; 40 dp tall. */
@Composable
private fun PanelTextButton(title: String, primary: Boolean, modifier: Modifier = Modifier, onClick: () -> Unit) {
    val padding = androidx.compose.foundation.layout.PaddingValues(horizontal = 20.dp)
    if (primary) {
        Button(
            onClick = onClick,
            colors = ButtonDefaults.buttonColors(containerColor = ObiSilk.Amber, contentColor = ObiSilk.Ground),
            contentPadding = padding,
            modifier = modifier.height(40.dp),
        ) { Text(title, style = MaterialTheme.typography.labelLarge) }
    } else {
        OutlinedButton(
            onClick = onClick,
            colors = ButtonDefaults.outlinedButtonColors(contentColor = ObiSilk.Silk),
            border = BorderStroke(1.dp, ObiSilk.Edge),
            contentPadding = padding,
            modifier = modifier.height(40.dp),
        ) { Text(title, style = MaterialTheme.typography.labelLarge) }
    }
}

// The Sources tab

@Composable
private fun PanelSources(model: PlayerModel) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val id = model.staged?.id ?: return
    val optionsAll by store.sourceChoices.options.collectAsStateWithLifecycle()
    val dubs by store.titles.dubLines.collectAsStateWithLifecycle()
    val inParty = store.party.view.collectAsStateWithLifecycle().value != null
    LaunchedEffect(id) {
        store.loadSourceOptions(id)
        model.loadPanelReleases()
    }
    var expanded by rememberSaveable(id, model.staged?.episode) { mutableStateOf(emptySet<String>()) }
    // A release picked elsewhere, or tried again, shows on its row.
    LaunchedEffect(model.panelSwitch?.target) {
        val target = model.panelSwitch?.target ?: return@LaunchedEffect
        if (target.providerId.isNotEmpty() && target.release != null) expanded = expanded + target.providerId
    }
    val options = optionsAll[id]
    val playingOption = options?.let { all ->
        model.playingRelease?.let { release -> all.firstOrNull { it.providerId == release.providerId } }
            ?: model.playingSource?.let { name -> all.firstOrNull { it.name == name } }
    }
    val playingProvider = model.playingRelease?.providerId ?: playingOption?.providerId
    val state = rememberLazyListState()
    // A switch landed: what plays now is at the top.
    LaunchedEffect(model.landings) { if (model.landings > 0) state.animateScrollToItem(0) }
    LazyColumn(
        state = state,
        modifier = Modifier.fillMaxSize().testTag("panel-sources"),
        contentPadding = androidx.compose.foundation.layout.PaddingValues(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 16.dp),
    ) {
        item(key = "playing-heading") { Heading(Strings.playerPanelPlayingNow(context)) }
        val name = model.playingSource ?: playingOption?.name
        if (name != null) item(key = "playing-source") {
            Column(Modifier.fillMaxWidth().padding(start = 10.dp, end = 10.dp, top = 2.dp, bottom = 4.dp)) {
                Text(name, style = TitleStyle.copy(fontSize = 17.sp, lineHeight = 24.sp, fontWeight = FontWeight.SemiBold), color = ObiSilk.Silk)
                playingOption?.let { Text(PanelWords.source(context, it), style = DetailStyle, color = ObiSilk.Haze) }
            }
        }
        val release = model.playingRelease
        if (release != null) {
            item(key = "playing-release") {
                ChoiceRow(model, release.title, PanelWords.release(context, release), SwitchTarget(release.providerId, release.providerName, release = release.id), on = true, sub = true, onClick = null)
            }
        } else {
            items(model.menus?.sources.orEmpty(), key = { "stream/${it.id}" }) { stream -> // i18n-skip
                ChoiceRow(
                    model, stream.label, stream.health ?: stream.detail,
                    SwitchTarget("", name.orEmpty(), stream = stream.id),
                    on = stream.current,
                    onClick = if (stream.current) null else ({ model.switchTo(stream) }),
                )
            }
        }
        item(key = "translation") { TranslationSwitch(model, id, dubs[id], inParty) }
        item(key = "rule") { Box(Modifier.padding(start = 10.dp, end = 10.dp, top = 10.dp, bottom = 4.dp).fillMaxWidth().height(1.dp).background(ObiSilk.Silk.copy(alpha = 0.10f))) }
        item(key = "others-heading") { Heading(Strings.playerPanelOtherSources(context)) }
        others(model, options, playingProvider, expanded) { provider -> expanded = if (provider in expanded) expanded - provider else expanded + provider }
        item(key = "foot") {
            Text(
                "${Strings.playerPanelOrder(context)} ${Strings.playerPanelPickScope(context)}", // i18n-skip
                style = DetailStyle,
                color = ObiSilk.Haze,
                modifier = Modifier.padding(start = 10.dp, end = 10.dp, top = 10.dp, bottom = 4.dp),
            )
        }
    }
}

@Composable
private fun Heading(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.labelLarge,
        color = ObiSilk.Haze,
        modifier = Modifier.padding(start = 10.dp, end = 10.dp, top = 12.dp, bottom = 6.dp).semantics { heading() },
    )
}

/**
 * Subtitled or Dubbed: the engine finds the other version while the episode plays on, then plays
 * it from the playhead. The picked segment spins from 600 ms, a note counts from 3 s, and a
 * failure says so under the switch.
 */
@Composable
private fun TranslationSwitch(model: PlayerModel, id: AnilistId, dub: app.kaeri.engine.model.DubExtent?, inParty: Boolean) {
    val context = LocalContext.current
    val pending = model.versionSwitch
    val current = pending?.takeIf { it.failed == null }?.to ?: model.menus?.translation ?: Translation.SUB
    val now = model.clock
    val number = currentEpisode(model)?.toDoubleOrNull()
    // The dub does not reach this episode yet.
    val pastDub = dub != null && !dub.complete && number != null && number > dub.through.toDouble()
    val label = Strings.playerTranslation(context)
    Column(Modifier.padding(start = 10.dp, end = 10.dp, top = 8.dp, bottom = 4.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth().semantics { paneTitle = label }) {
            for ((index, option) in listOf(Translation.SUB, Translation.DUB).withIndex()) {
                SegmentedButton(
                    selected = current == option,
                    onClick = { if (current != option) model.switchTo(option) },
                    shape = SegmentedButtonDefaults.itemShape(index, 2),
                    enabled = current == option || !(option == Translation.DUB && pastDub),
                    colors = SegmentedButtonDefaults.colors(
                        activeContainerColor = ObiSilk.RaisedSelected,
                        activeContentColor = ObiSilk.Silk,
                        inactiveContainerColor = Color.Transparent,
                        inactiveContentColor = ObiSilk.Silk,
                        activeBorderColor = ObiSilk.Edge,
                        inactiveBorderColor = ObiSilk.Edge,
                    ),
                    modifier = Modifier.height(40.dp).testTag("panel-${if (option == Translation.SUB) "sub" else "dub"}"), // i18n-skip
                    icon = {
                        val finding = pending?.takeIf { it.to == option && it.failed == null }
                        if (finding != null) {
                            val shown by animateFloatAsState(if (finding.spinning(now)) 1f else 0f, Motion.fadeIn(), label = "version-spinner") // i18n-skip
                            CircularProgressIndicator(Modifier.size(16.dp).graphicsLayer { alpha = shown }.testTag("panel-version-spinner"), color = ObiSilk.Silk, strokeWidth = 2.dp) // i18n-skip
                        } else {
                            SegmentedButtonDefaults.Icon(current == option)
                        }
                    },
                    label = { Text(if (option == Translation.SUB) Strings.playerStartSubtitled(context) else Strings.audioDubbed(context)) },
                )
            }
        }
        val episode = currentEpisode(model)
        val finding = episode?.let { pending?.note(context, it, now) }
        AnimatedVisibility(finding != null, enter = fadeIn(Motion.fadeIn()) + expandVertically(Motion.standard()), exit = fadeOut(Motion.fadeOut()) + shrinkVertically(Motion.quick())) {
            Text(finding.orEmpty(), style = DetailStyle, color = ObiSilk.Haze, modifier = Modifier.testTag("panel-version-note")) // i18n-skip
        }
        val failed = pending?.failed
        AnimatedVisibility(failed != null, enter = fadeIn(Motion.fadeIn()) + expandVertically(Motion.standard()), exit = fadeOut(Motion.fadeOut()) + shrinkVertically(Motion.quick())) {
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                Icon(PanelGlyphs.Warning, contentDescription = null, tint = ObiSilk.Vermilion, modifier = Modifier.padding(top = 2.dp).size(16.dp))
                Text(failed?.text(context).orEmpty(), style = DetailStyle, color = ObiSilk.Vermilion)
            }
        }
        val notes = buildList {
            dub?.let { add(it.line(context)) }
            if (inParty) add(Strings.partyVersionOwn(context))
        }
        for (note in notes) Text(note, style = DetailStyle, color = ObiSilk.Haze)
    }
}

/** Every other source, in the order set in Sources; before their lines come, the releases' own sources. */
private fun LazyListScope.others(
    model: PlayerModel,
    options: List<SourceOption>?,
    skip: String?,
    expanded: Set<String>,
    toggle: (String) -> Unit,
) {
    model.panelReleases?.takeIf { it.rows != null }?.let { asked ->
        listings(model, asked, options, skip, expanded, toggle)
        return
    }
    data class Group(val providerId: String, val name: String, val option: SourceOption?)
    val groups = options?.filter { it.providerId != skip }?.map { Group(it.providerId, it.name, it) }
        ?: model.panelReleases?.candidates.orEmpty().filter { it.providerId != skip }.distinctBy { it.providerId }.map { Group(it.providerId, it.providerName, null) }
    val known = model.panelReleases?.candidates != null
    for (group in groups) {
        val releases = model.panelReleases?.of(group.providerId).orEmpty()
        val resting = group.option?.note != null
        val open = group.providerId in expanded
        when {
            releases.size > 1 && !resting -> {
                item(key = "source/${group.providerId}") { // i18n-skip
                    val context = LocalContext.current
                    ChoiceRow(
                        model, group.name, group.option?.let { PanelWords.source(context, it) },
                        SwitchTarget(group.providerId, group.name),
                        disclosure = Strings.playerPanelReleases(context, releases.size) to open,
                        onClick = { toggle(group.providerId) },
                    )
                }
                if (open) items(releases, key = { "release/${it.providerId}/${it.id}" }) { release -> // i18n-skip
                    ReleaseRow(model, release, group.name)
                }
            }
            releases.size == 1 && !resting -> item(key = "source/${group.providerId}") { // i18n-skip
                // One release: the source's row plays it.
                val context = LocalContext.current
                val release = releases.first()
                ChoiceRow(
                    model, group.name, group.option?.let { PanelWords.source(context, it) } ?: PanelWords.release(context, release),
                    SwitchTarget(release.providerId, group.name, release = release.id),
                    onClick = { model.switchTo(release) },
                )
            }
            else -> item(key = "source/${group.providerId}") { // i18n-skip
                // Resting, or nothing for this episode: said, and nothing to pick.
                val context = LocalContext.current
                ChoiceRow(
                    model, group.name, group.option?.let { PanelWords.source(context, it) },
                    SwitchTarget(group.providerId, group.name),
                    dim = resting || known,
                    waiting = !known && !resting,
                    onClick = null,
                )
            }
        }
    }
}

/**
 * Every other source's row as the engine says it while the tab asks, in the order set in Sources:
 * each is there from the first event and keeps its place and height, its line crossfading as it
 * answers. A row can be picked once it listed a release; its releases open only when tapped.
 */
private fun LazyListScope.listings(
    model: PlayerModel,
    asked: app.kaeri.player.PanelReleases,
    options: List<SourceOption>?,
    skip: String?,
    expanded: Set<String>,
    toggle: (String) -> Unit,
) {
    for (row in asked.others(skip, model.playingSource)) {
        val releases = (row.state as? ListingState.Listed)?.releases.orEmpty()
        val open = releases.size > 1 && row.providerId in expanded
        item(key = "source/${row.providerId}") { // i18n-skip
            val context = LocalContext.current
            val one = releases.singleOrNull()
            ChoiceRow(
                model, row.name, PanelWords.listing(context, row, options?.firstOrNull { it.providerId == row.providerId }, asked.episode),
                if (one != null) SwitchTarget(one.providerId, row.name, release = one.id) else SwitchTarget(row.providerId, row.name),
                dim = row.state !is ListingState.Listed && row.state !is ListingState.Asking,
                listing = row.providerId,
                disclosure = if (releases.size > 1) Strings.playerPanelReleases(context, releases.size) to open else null,
                onClick = when {
                    releases.size > 1 -> ({ toggle(row.providerId) })
                    one != null -> ({ model.switchTo(one) })
                    else -> null
                },
            )
        }
        if (open) items(releases, key = { "release/${it.providerId}/${it.id}" }) { release -> // i18n-skip
            ReleaseRow(model, release, row.name)
        }
    }
}

@Composable
private fun ReleaseRow(model: PlayerModel, release: SourceCandidate, source: String) {
    val context = LocalContext.current
    Box(Modifier.animateItemFade()) {
        ChoiceRow(
            model, release.title, PanelWords.release(context, release),
            SwitchTarget(release.providerId, source, release = release.id),
            sub = true, inset = true,
            onClick = { model.switchTo(release) },
        )
    }
}

/** Releases fade in under their source as it opens. */
@Composable
private fun Modifier.animateItemFade(): Modifier {
    var shown by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { shown = true }
    val alpha by animateFloatAsState(if (shown) 1f else 0f, tween(200), label = "release") // i18n-skip
    return graphicsLayer { this.alpha = alpha }
}

/** The check column's width. */
private val CHECK = 24.dp

/** What a row's detail line says: crossfaded as one, old and new together. */
private enum class Line { Detail, Starting, Silent, Asking, Counting }

/**
 * A stream, source or release: the check on what plays, a spinner on a switch past 600 ms, and
 * "Starting · 3 s" in place of its line from 3 s. A failed switch says so under it, with Try again.
 */
@Composable
private fun ChoiceRow(
    model: PlayerModel,
    title: String,
    detail: String?,
    target: SwitchTarget,
    on: Boolean = false,
    sub: Boolean = false,
    inset: Boolean = false,
    dim: Boolean = false,
    waiting: Boolean = false,
    /** The source this row says while the tab asks it, by its id: its line follows the engine's rows. */
    listing: String? = null,
    disclosure: Pair<String, Boolean>? = null,
    onClick: (() -> Unit)?,
) {
    val context = LocalContext.current
    val picked = model.panelSwitch?.takeIf { it.on(target) }
    val now = model.clock
    val asking = listing?.let { model.panelReleases?.line(it, now) } ?: ListingLine.Answered
    Column(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .then(if (onClick != null) Modifier.clickable(role = Role.Button, onClick = onClick) else Modifier)
            .semantics { if (on) selected = true }
            .heightIn(min = 48.dp)
            .padding(start = 4.dp + if (inset) CHECK + 6.dp else 0.dp, end = 10.dp, top = 8.dp, bottom = 8.dp)
            .testTag("panel-choice-$title"), // i18n-skip
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            Box(Modifier.width(CHECK), contentAlignment = Alignment.Center) {
                if (picked != null && picked.failed == null) {
                    val shown by animateFloatAsState(if (picked.spinning(now)) 1f else 0f, Motion.fadeIn(), label = "spinner") // i18n-skip
                    CircularProgressIndicator(Modifier.size(18.dp).graphicsLayer { alpha = shown }, color = ObiSilk.Silk, strokeWidth = 2.dp)
                } else if (listing != null) {
                    // Asked: from 600 ms, faded out as the answer fades in.
                    val spins = asking == ListingLine.Asking || asking == ListingLine.Counting
                    val shown by animateFloatAsState(if (spins) 1f else 0f, Motion.fadeIn(), label = "asking-spinner") // i18n-skip
                    if (spins || shown > 0f) {
                        CircularProgressIndicator(Modifier.size(18.dp).graphicsLayer { alpha = shown }.testTag("panel-asking-$title"), color = ObiSilk.Silk, strokeWidth = 2.dp) // i18n-skip
                    }
                } else if (on) {
                    Icon(PanelGlyphs.Check, contentDescription = null, tint = ObiSilk.Silk, modifier = Modifier.size(18.dp))
                }
            }
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(1.dp)) {
                // A source that answered with nothing goes to haze as its answer fades in.
                val ink by animateColorAsState(if (dim) ObiSilk.Haze else ObiSilk.Silk, Motion.fadeIn(), label = "title") // i18n-skip
                Text(
                    title,
                    style = if (sub) DetailStyle.copy(fontSize = 15.sp, lineHeight = 22.sp) else TitleStyle.copy(fontWeight = FontWeight.SemiBold),
                    color = ink,
                )
                val counting = picked?.counting(now)
                val line = when {
                    counting != null -> Line.Starting
                    asking == ListingLine.Silent -> Line.Silent
                    asking == ListingLine.Asking -> Line.Asking
                    asking == ListingLine.Counting -> Line.Counting
                    else -> Line.Detail
                }
                // While sources answer the line is held one line high, so a row never changes height.
                val held = if (listing != null) Modifier.heightIn(min = with(LocalDensity.current) { DetailStyle.lineHeight.toDp() }) else Modifier
                Crossfade(line, modifier = held, animationSpec = Motion.fadeIn(), label = "line") { said -> // i18n-skip
                    when (said) {
                        Line.Starting -> Text(Strings.playerPanelStarting(context, picked?.counting(now) ?: 3), style = DetailStyle, color = ObiSilk.Haze)
                        Line.Silent -> {}
                        Line.Asking -> Text(Strings.playerStartRowAsking(context), style = DetailStyle, color = ObiSilk.Haze)
                        Line.Counting -> Text(
                            Strings.playerPanelAsking(context, listing?.let { model.panelReleases?.seconds(it, now) } ?: 3),
                            style = DetailStyle,
                            color = ObiSilk.Haze,
                            modifier = Modifier.testTag("panel-asking-seconds-$title"), // i18n-skip
                        )
                        Line.Detail -> when {
                            !detail.isNullOrEmpty() -> Text(detail, style = DetailStyle, color = ObiSilk.Haze)
                            waiting -> CircularProgressIndicator(Modifier.padding(vertical = 4.dp).size(12.dp), color = ObiSilk.Haze, strokeWidth = 1.5.dp)
                        }
                    }
                }
            }
            // A source's release count and chevron fade in at the end of its row as it lists them.
            AnimatedVisibility(disclosure != null, enter = fadeIn(Motion.fadeIn()), exit = fadeOut(Motion.fadeOut())) {
                val (count, open) = disclosure ?: return@AnimatedVisibility
                val turn by animateFloatAsState(if (open) 90f else 0f, Motion.standard(), label = "chevron") // i18n-skip
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                    Text(count, style = DetailStyle, color = ObiSilk.Haze)
                    Icon(PanelGlyphs.Chevron, contentDescription = null, tint = ObiSilk.Haze, modifier = Modifier.size(16.dp).rotate(turn))
                }
            }
        }
        val failed = picked?.failed
        AnimatedVisibility(failed != null, enter = fadeIn(Motion.fadeIn()) + expandVertically(Motion.standard()), exit = fadeOut(Motion.fadeOut()) + shrinkVertically(Motion.quick())) {
            Column(Modifier.padding(top = 6.dp, start = CHECK + 6.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    Icon(PanelGlyphs.Warning, contentDescription = null, tint = ObiSilk.Vermilion, modifier = Modifier.padding(top = 2.dp).size(16.dp))
                    Text(failed?.text(context).orEmpty(), style = DetailStyle, color = ObiSilk.Vermilion)
                }
                PanelTextButton(Strings.actionTryAgain(context), primary = false, modifier = Modifier.testTag("panel-try-again")) { model.retrySwitch() }
            }
        }
    }
}

/**
 * The picture held while a switch loads: over the video until the new stream's first frame, then
 * faded out as the new picture comes up under it (0.2 s, a fade alone).
 */
@Composable
internal fun HeldPictureLayer(model: PlayerModel, modifier: Modifier) {
    var last by remember { mutableStateOf<HeldPicture?>(null) }
    val held = model.heldPicture
    if (held != null) last = held
    val alpha by animateFloatAsState(if (model.holding && held != null) 1f else 0f, if (model.holding) snap() else tween(PlayerModel.HELD_FADE.toInt() - 100), label = "held") // i18n-skip
    val shown = last ?: return
    if (alpha == 0f && held == null) return
    val image = remember(shown) { shown.bitmap.asImageBitmap() }
    Image(image, contentDescription = null, contentScale = ContentScale.FillBounds, modifier = modifier.graphicsLayer { this.alpha = alpha })
}
