// The player's Episodes and Sources panel on the TV (player panel board, Android TV): the 400 dp
// column at the end edge with two pill tabs at its top. Focus fills a row in silk with ground
// text and scales it 1.02, and a row focus rests on for 250 ms reveals its synopsis and buttons.
// Right moves into a row's buttons and Left back; Up from the first row reaches the tabs, Down from
// the tabs enters at the playing row; Left and Right on the tabs switch tab at once; OK does the
// amber action. Back closes the panel (the player screen's Back).

package app.kaeri.tv.player

import android.content.Context
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.Crossfade
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.snap
import androidx.compose.animation.core.tween
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.focusGroup
import androidx.compose.foundation.focusable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.onClick
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.Icon
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import app.kaeri.design.Motion
import app.kaeri.design.ObiSilk
import app.kaeri.design.PanelGlyphs
import app.kaeri.engine.core.Translation
import app.kaeri.engine.model.DubExtent
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
import app.kaeri.tv.ui.AmberButton
import app.kaeri.tv.ui.ArtImage
import app.kaeri.tv.ui.ButtonLabel
import app.kaeri.tv.ui.LocalAppStore
import app.kaeri.tv.ui.LocalReducedMotion
import app.kaeri.tv.ui.PANEL_END
import app.kaeri.tv.ui.TonalButton
import app.kaeri.tv.ui.panelFocus
import app.kaeri.tv.ui.panelWidth
import app.kaeri.tv.ui.pixels
import app.kaeri.tv.ui.spoilerProtected
import app.kaeri.tv.ui.tryFocus
import kotlinx.coroutines.delay

private val TitleStyle = TextStyle(fontSize = 16.sp, lineHeight = 22.sp, fontWeight = FontWeight.Medium)
private val DetailStyle = TextStyle(fontSize = 14.sp, lineHeight = 20.sp)

/** How long focus rests on a row before it reveals: holding Down does not open every row it passes. */
private const val DWELL = 250L

/** The scale of a focused row, and of a focused tab. */
private const val ROW_SCALE = 1.02f
private const val TAB_SCALE = 1.05f

/** The panel column, open on [model]'s tab. */
@Composable
internal fun TvPlayerPanel(model: PlayerModel, modifier: Modifier) {
    val context = LocalContext.current
    val tabs = remember { PanelTab.entries.associateWith { FocusRequester() } }
    val entry = remember { FocusRequester() }
    Column(
        modifier
            .width(panelWidth())
            .panelFocus(Strings.playerPanel(context))
            .fillMaxHeight()
            .background(ObiSilk.RaisedHigh)
            .padding(start = 16.dp, end = PANEL_END, top = 27.dp)
            .testTag("tv-player-panel"),
    ) {
        Row(Modifier.padding(bottom = 12.dp).focusGroup(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            for (tab in listOf(PanelTab.Episodes, PanelTab.Sources)) {
                val requester = tabs.getValue(tab)
                PanelTabPill(
                    if (tab == PanelTab.Episodes) Strings.playerEpisodes(context) else Strings.playerSources(context),
                    selected = model.panelTab == tab,
                    onFocus = { model.panelTab = tab },
                    onDown = { entry.tryFocus() },
                    modifier = Modifier.focusRequester(requester).testTag("tv-panel-tab-${tab.name.lowercase()}"), // i18n-skip
                )
            }
        }
        val reduced = LocalReducedMotion.current
        val toTabs = { tabs[model.panelTab]?.tryFocus() ?: false }
        Crossfade(model.panelTab, animationSpec = if (reduced) snap() else tween(300), label = "panel-tab") { tab -> // i18n-skip
            when (tab) {
                PanelTab.Episodes -> TvPanelEpisodes(model, entry, toTabs)
                PanelTab.Sources -> TvPanelSources(model, entry, toTabs)
            }
        }
    }
    // Focus starts on the playing row, or on Sources' first row.
    LaunchedEffect(Unit) {
        repeat(8) {
            androidx.compose.runtime.withFrameNanos { }
            if (entry.tryFocus()) return@LaunchedEffect
        }
        tabs[model.panelTab]?.tryFocus()
    }
}

/** A pill tab, 36 high: selected in the raised fill, focused silk with ground text at 1.05. */
@Composable
private fun PanelTabPill(label: String, selected: Boolean, onFocus: () -> Unit, onDown: () -> Boolean, modifier: Modifier) {
    var focused by remember { mutableStateOf(false) }
    // Reduce Motion keeps the focus ring and drops the scale.
    val reduced = LocalReducedMotion.current
    val scale by animateFloatAsState(if (focused && !reduced) TAB_SCALE else 1f, Motion.quick(), label = "tab") // i18n-skip
    Box(
        modifier
            .graphicsLayer { scaleX = scale; scaleY = scale }
            .height(36.dp)
            .clip(CircleShape)
            .background(
                when {
                    focused -> ObiSilk.Silk
                    selected -> ObiSilk.RaisedSelected
                    else -> Color.Transparent
                },
            )
            .onFocusChanged {
                focused = it.isFocused
                // Left and Right on the tabs switch tab at once.
                if (it.isFocused) onFocus()
            }
            .onPreviewKeyEvent { event ->
                if (event.type == KeyEventType.KeyDown && event.key == Key.DirectionDown) onDown() else false
            }
            .semantics {
                role = Role.Tab
                this.selected = selected
            }
            .focusable()
            .padding(horizontal = 18.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(label, style = MaterialTheme.typography.titleSmall, color = if (focused) ObiSilk.Ground else if (selected) ObiSilk.Silk else ObiSilk.Haze)
    }
}

// Episodes

@Composable
private fun TvPanelEpisodes(model: PlayerModel, entry: FocusRequester, toTabs: () -> Boolean) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val id = model.staged?.id ?: return
    val tables by store.titles.episodes.collectAsStateWithLifecycle()
    val entries by store.titles.entries.collectAsStateWithLifecycle()
    LaunchedEffect(id) { if (store.titles.episodes.value[id] == null) store.loadEpisodes(id) }
    val rows = tables[id]
    if (rows == null) {
        Text(Strings.playerSheetEpisodesLoading(context), style = DetailStyle, color = ObiSilk.Haze, modifier = Modifier.padding(16.dp).focusRequester(entry).focusable())
        return
    }
    val current = model.menus?.episodes?.firstOrNull { it.current }?.number ?: model.staged?.episode
    val metadataOnly = rows.any { it.metadataOnly }
    val spoilers = spoilerProtected(id)
    // The row revealed: the last one focus rested on. The playing one as the panel opens.
    var revealed by remember(id) { mutableStateOf(current) }
    val at = rows.indexOfFirst { it.number == current }.coerceAtLeast(0)
    val state = rememberLazyListState((at - 1).coerceAtLeast(0))
    LazyColumn(
        state = state,
        modifier = Modifier.fillMaxSize().focusGroup().testTag("tv-panel-episodes"),
        contentPadding = PaddingValues(bottom = 27.dp, top = 4.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        if (metadataOnly) item(key = "metadata-only") {
            Text(Strings.playerPanelMetadataOnly(context), style = DetailStyle, color = ObiSilk.Haze, modifier = Modifier.padding(horizontal = 12.dp, vertical = 8.dp))
        }
        items(rows, key = { it.number }) { row ->
            val first = row.number == rows.first().number
            TvEpisodeRow(
                model, row,
                banner = entries[id]?.bannerUrl,
                current = !metadataOnly && row.number == current,
                metadataOnly = metadataOnly,
                hidden = spoilers && row.spoilerAhead && row.number != current,
                revealed = revealed == row.number,
                onDwell = { revealed = row.number },
                onUp = if (first) toTabs else null,
                modifier = if (row.number == (current ?: rows.first().number)) Modifier.focusRequester(entry) else Modifier,
            )
        }
    }
}

@Composable
private fun TvEpisodeRow(
    model: PlayerModel,
    row: EpisodeRow,
    banner: String?,
    current: Boolean,
    metadataOnly: Boolean,
    hidden: Boolean,
    revealed: Boolean,
    onDwell: () -> Unit,
    onUp: (() -> Boolean)?,
    modifier: Modifier,
) {
    val context = LocalContext.current
    var focused by remember { mutableStateOf(false) }
    LaunchedEffect(focused) {
        if (focused) {
            delay(DWELL)
            onDwell()
        }
    }
    val notOut = PanelWords.notOut(row)
    val actions = tvEpisodeActions(model, row, current, metadataOnly, notOut)
    val first = remember { FocusRequester() }
    val self = remember { FocusRequester() }
    val reduced = LocalReducedMotion.current
    val scale by animateFloatAsState(if (focused && !reduced) ROW_SCALE else 1f, Motion.quick(), label = "row") // i18n-skip
    val ink = if (focused) ObiSilk.Ground else ObiSilk.Silk
    val soft = if (focused) ObiSilk.Ground.copy(alpha = 0.72f) else ObiSilk.Haze
    Column(
        modifier
            .graphicsLayer { scaleX = scale; scaleY = scale }
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .background(
                when {
                    focused -> ObiSilk.Silk
                    current -> ObiSilk.RaisedSelected
                    else -> Color.Transparent
                },
            )
            .focusRequester(self)
            .onFocusChanged { focused = it.isFocused }
            .onPreviewKeyEvent { event ->
                if (!focused || event.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
                when (event.key) {
                    // Right moves into the row's buttons, or its More alone, revealing it at once.
                    Key.DirectionRight -> {
                        onDwell()
                        first.tryFocus()
                        true
                    }
                    Key.DirectionUp -> onUp?.invoke() ?: false
                    // OK does the amber action, or the first there is.
                    Key.DirectionCenter, Key.Enter, Key.NumPadEnter -> {
                        (actions.firstOrNull { it.primary } ?: actions.firstOrNull())?.run?.invoke()
                        true
                    }
                    else -> false
                }
            }
            .semantics {
                if (current) selected = true
                actions.firstOrNull { it.primary }?.let { action -> onClick(action.kind.title(context)) { action.run(); true } }
            }
            .focusable()
            .padding(8.dp)
            .animateContentSize(if (reduced) snap() else Motion.standard())
            .testTag("tv-panel-episode-${row.number}"), // i18n-skip
    ) {
        Row(horizontalArrangement = Arrangement.spacedBy(16.dp), verticalAlignment = Alignment.CenterVertically) {
            Box(
                Modifier.size(128.dp, 72.dp).clip(RoundedCornerShape(8.dp)).background(ObiSilk.Silk.copy(alpha = 0.07f))
                    .graphicsLayer { alpha = if (row.completed && !current && !metadataOnly) 0.62f else 1f },
                contentAlignment = Alignment.BottomCenter,
            ) {
                val url = if (hidden || metadataOnly || notOut) null else row.thumbnail ?: banner
                if (url != null) ArtImage(url, plate = null, maxPixelSize = pixels(128.dp), modifier = Modifier.fillMaxSize())
                PanelWords.progress(row, current, metadataOnly, model.position, model.duration)?.takeIf { it > 0f }?.let { progress ->
                    Box(Modifier.fillMaxWidth().padding(4.dp).height(3.dp).clip(RoundedCornerShape(2.dp)).background(ObiSilk.Silk.copy(alpha = 0.32f))) {
                        Box(Modifier.fillMaxWidth(progress).fillMaxHeight().background(ObiSilk.Silk))
                    }
                }
            }
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                val title = row.title?.takeIf { it.isNotBlank() }
                Text(
                    buildAnnotatedString {
                        if (title != null) {
                            withStyle(SpanStyle(color = soft)) { append(PanelWords.number(context, row.number)) }
                            append(" ")
                            append(title)
                        } else {
                            append(Strings.episodeNumberPlain(context, row.number))
                        }
                    },
                    style = TitleStyle,
                    color = ink,
                    // Revealed, the whole title: cut text is never left unreadable.
                    maxLines = if (revealed) Int.MAX_VALUE else 2,
                    overflow = TextOverflow.Ellipsis,
                )
                val detail = PanelWords.detail(context, row, current, metadataOnly, model.position, model.duration)
                if (detail.isNotEmpty()) Text(detail, style = DetailStyle, color = soft, maxLines = if (revealed) Int.MAX_VALUE else 2, overflow = TextOverflow.Ellipsis)
            }
        }
        AnimatedVisibility(
            revealed,
            enter = if (reduced) fadeIn(tween(200)) else expandVertically(Motion.standard()) + fadeIn(tween(200, delayMillis = 100)),
            exit = if (reduced) fadeOut(tween(100)) else shrinkVertically(Motion.quick()) + fadeOut(tween(100)),
        ) {
            Column(Modifier.padding(top = 10.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                val synopsis = row.description?.takeIf { it.isNotBlank() }
                var whole by remember(row.number) { mutableStateOf(false) }
                var cut by remember(row.number) { mutableStateOf(false) }
                when {
                    hidden && synopsis != null -> Text(Strings.episodeSpoilerHidden(context), style = DetailStyle.copy(fontStyle = FontStyle.Italic), color = soft)
                    synopsis != null && !notOut -> Text(
                        synopsis,
                        style = DetailStyle,
                        color = soft,
                        maxLines = if (whole) Int.MAX_VALUE else 3,
                        overflow = TextOverflow.Ellipsis,
                        onTextLayout = { if (!whole) cut = it.hasVisualOverflow },
                    )
                }
                val more = synopsis != null && !hidden && !notOut && (cut || whole)
                if (actions.isNotEmpty() || more) {
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        actions.forEachIndexed { index, action ->
                            val back = Modifier.onPreviewKeyEvent { event ->
                                // Left from the first button comes back to the row.
                                if (index == 0 && event.type == KeyEventType.KeyDown && event.key == Key.DirectionLeft) self.tryFocus() else false
                            }
                            val tag = Modifier.testTag("tv-panel-action-${action.kind.name}") // i18n-skip
                            // Mark watched goes once it is pressed, so focus moves to the row's first button, or the row.
                            val run = if (action.kind != TvEpisodeActionKind.MarkWatched) action.run else ({
                                (if (index == 0) self else first).tryFocus()
                                action.run()
                            })
                            if (action.primary) {
                                AmberButton(onClick = run, icon = null, height = 40.dp, modifier = (if (index == 0) Modifier.focusRequester(first) else Modifier).then(back).then(tag)) {
                                    ButtonLabel(action.kind.title(context))
                                }
                            } else {
                                TonalButton(onClick = run, height = 40.dp, modifier = (if (index == 0) Modifier.focusRequester(first) else Modifier).then(back).then(tag)) {
                                    ButtonLabel(action.kind.title(context))
                                }
                            }
                        }
                        if (more) {
                            // More reads the synopsis whole; Less folds it back.
                            TonalButton(
                                onClick = { whole = !whole },
                                height = 40.dp,
                                modifier = (if (actions.isEmpty()) Modifier.focusRequester(first) else Modifier)
                                    // Left from More, the only button, comes back to the row.
                                    .onPreviewKeyEvent { event ->
                                        if (actions.isEmpty() && event.type == KeyEventType.KeyDown && event.key == Key.DirectionLeft) self.tryFocus() else false
                                    }
                                    .testTag("tv-panel-more"),
                            ) {
                                ButtonLabel(if (whole) Strings.playerPanelLess(context) else Strings.playerPanelMore(context))
                            }
                        }
                    }
                }
            }
        }
    }
}

private enum class TvEpisodeActionKind { Play, Resume, FromBeginning, FromBeginningPlaying, MarkWatched }

private class TvEpisodeAction(val kind: TvEpisodeActionKind, val run: () -> Unit) {
    val primary: Boolean get() = kind == TvEpisodeActionKind.Play || kind == TvEpisodeActionKind.Resume
}

private fun TvEpisodeActionKind.title(context: Context): String = when (this) {
    TvEpisodeActionKind.Play -> Strings.actionPlay(context)
    TvEpisodeActionKind.Resume -> Strings.actionResume(context)
    TvEpisodeActionKind.FromBeginning, TvEpisodeActionKind.FromBeginningPlaying -> Strings.episodePlayFromBeginning(context)
    TvEpisodeActionKind.MarkWatched -> Strings.episodeMarkWatched(context)
}

/** What a revealed row can do, as the board has it. */
private fun tvEpisodeActions(model: PlayerModel, row: EpisodeRow, current: Boolean, metadataOnly: Boolean, notOut: Boolean): List<TvEpisodeAction> {
    val kinds = when {
        metadataOnly -> if (row.completed) emptyList() else listOf(TvEpisodeActionKind.MarkWatched)
        notOut -> emptyList()
        row.unsourced -> if (row.completed) emptyList() else listOf(TvEpisodeActionKind.MarkWatched)
        current -> listOf(TvEpisodeActionKind.FromBeginningPlaying)
        row.watched > 0 && !row.completed -> listOf(TvEpisodeActionKind.Resume, TvEpisodeActionKind.FromBeginning)
        row.completed -> listOf(TvEpisodeActionKind.Play)
        else -> listOf(TvEpisodeActionKind.Play, TvEpisodeActionKind.MarkWatched)
    }
    return kinds.map { kind ->
        TvEpisodeAction(kind) {
            when (kind) {
                TvEpisodeActionKind.MarkWatched -> model.markWatched(row.number)
                TvEpisodeActionKind.FromBeginningPlaying -> {
                    model.startOver()
                    model.closePanel()
                }
                TvEpisodeActionKind.Play, TvEpisodeActionKind.Resume -> {
                    model.playEpisode(row.number, fromStart = false)
                    model.closePanel()
                }
                TvEpisodeActionKind.FromBeginning -> {
                    model.playEpisode(row.number, fromStart = true)
                    model.closePanel()
                }
            }
        }
    }
}

// Sources

@Composable
private fun TvPanelSources(model: PlayerModel, entry: FocusRequester, toTabs: () -> Boolean) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val id = model.staged?.id ?: return
    val optionsAll by store.sourceChoices.options.collectAsStateWithLifecycle()
    val dubs by store.titles.dubLines.collectAsStateWithLifecycle()
    val inRoom = store.room.view.collectAsStateWithLifecycle().value != null
    LaunchedEffect(id) { store.loadSourceOptions(id) }
    // Asked again when another episode plays under the open tab.
    LaunchedEffect(id, model.staged?.episode) { model.loadPanelReleases() }
    var expanded by remember(id, model.staged?.episode) { mutableStateOf(emptySet<String>()) }
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
    // A landing moves the picked release up to Playing now. Focus that went with the row it left
    // follows it there; focus resting elsewhere in the list stays where it is.
    var holdsFocus by remember { mutableStateOf(false) }
    LaunchedEffect(model.landings) {
        if (model.landings == 0) return@LaunchedEffect
        state.animateScrollToItem(0)
        if (!holdsFocus) entry.tryFocus()
    }
    // Up from the first row that takes focus reaches the tabs.
    var firstKey: String? = null
    fun firstOf(key: String): Boolean {
        if (firstKey == null) firstKey = key
        return firstKey == key
    }
    LazyColumn(
        state = state,
        modifier = Modifier.fillMaxSize().onFocusChanged { holdsFocus = it.hasFocus && !it.isFocused }.focusGroup().testTag("tv-panel-sources"),
        contentPadding = PaddingValues(bottom = 27.dp, top = 4.dp),
    ) {
        item(key = "playing-heading") { Heading(Strings.playerPanelPlayingNow(context)) }
        val name = model.playingSource ?: playingOption?.name
        if (name != null) item(key = "playing-source") {
            Column(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 4.dp)) {
                Text(name, style = TitleStyle.copy(fontSize = 17.sp, fontWeight = FontWeight.SemiBold), color = ObiSilk.Silk)
                playingOption?.let { Text(PanelWords.source(context, it), style = DetailStyle, color = ObiSilk.Haze) }
            }
        }
        val release = model.playingRelease
        if (release != null) {
            val key = "playing-release"
            val top = firstOf(key)
            item(key = key) {
                TvChoiceRow(model, release.title, PanelWords.release(context, release), SwitchTarget(release.providerId, release.providerName, release = release.id), on = true, sub = true, onClick = null, onUp = if (top) toTabs else null, modifier = if (top) Modifier.focusRequester(entry) else Modifier)
            }
        } else {
            val streams = model.menus?.sources.orEmpty()
            for (stream in streams) {
                val key = "stream/${stream.id}" // i18n-skip
                val top = firstOf(key)
                item(key = key) {
                    TvChoiceRow(
                        model, stream.label, stream.health ?: stream.detail,
                        SwitchTarget("", name.orEmpty(), stream = stream.id),
                        on = stream.current,
                        onClick = if (stream.current) null else ({ model.switchTo(stream) }),
                        onUp = if (top) toTabs else null,
                        modifier = if (top) Modifier.focusRequester(entry) else Modifier,
                    )
                }
            }
        }
        val translationTop = firstOf("translation")
        item(key = "translation") { TvTranslationSwitch(model, dubs[id], inRoom, onUp = if (translationTop) toTabs else null, modifier = if (translationTop) Modifier.focusRequester(entry) else Modifier) }
        item(key = "rule") { Box(Modifier.padding(horizontal = 12.dp, vertical = 8.dp).fillMaxWidth().height(1.dp).background(ObiSilk.Silk.copy(alpha = 0.10f))) }
        item(key = "others-heading") { Heading(Strings.playerPanelOtherSources(context)) }
        tvOthers(model, options, playingProvider, expanded) { provider -> expanded = if (provider in expanded) expanded - provider else expanded + provider }
        item(key = "foot") {
            // Focusable, so the D-pad can bring the foot into view under the last source.
            Text(
                "${Strings.playerPanelOrder(context)} ${Strings.playerPanelPickScope(context)}", // i18n-skip
                style = DetailStyle,
                color = ObiSilk.Haze,
                modifier = Modifier.padding(horizontal = 12.dp, vertical = 10.dp).focusable(),
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
        modifier = Modifier.padding(start = 12.dp, end = 12.dp, top = 10.dp, bottom = 6.dp).semantics { heading() },
    )
}

/**
 * Subtitled or Dubbed, a two-segment pill 40 high: each segment focuses, OK picks it. The picked
 * segment spins from 600 ms while the engine finds that version, a note counts from 3 s, and a
 * failure says so under the pill.
 */
@Composable
private fun TvTranslationSwitch(model: PlayerModel, dub: DubExtent?, inRoom: Boolean, onUp: (() -> Boolean)?, modifier: Modifier) {
    val context = LocalContext.current
    val pending = model.versionSwitch
    val current = pending?.takeIf { it.failed == null }?.to ?: model.menus?.translation ?: Translation.SUB
    val now = model.clock
    val number = (model.menus?.episodes?.firstOrNull { it.current }?.number ?: model.staged?.episode)?.toDoubleOrNull()
    val pastDub = dub != null && !dub.complete && number != null && number > dub.through.toDouble()
    Column(Modifier.padding(horizontal = 12.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row(
            Modifier.fillMaxWidth().height(40.dp).clip(CircleShape).background(ObiSilk.RaisedSelected.copy(alpha = 0.6f)).padding(2.dp).focusGroup(),
        ) {
            for ((index, option) in listOf(Translation.SUB, Translation.DUB).withIndex()) {
                var focused by remember { mutableStateOf(false) }
                val selected = current == option
                val enabled = selected || !(option == Translation.DUB && pastDub)
                Box(
                    (if (index == 0) modifier else Modifier)
                        .weight(1f)
                        .fillMaxHeight()
                        .clip(CircleShape)
                        .background(
                            when {
                                focused -> ObiSilk.Silk
                                selected -> ObiSilk.Edge
                                else -> Color.Transparent
                            },
                        )
                        .onFocusChanged { focused = it.isFocused }
                        .onPreviewKeyEvent { event ->
                            if (event.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
                            when (event.key) {
                                Key.DirectionCenter, Key.Enter, Key.NumPadEnter -> {
                                    if (!selected && enabled) model.switchTo(option)
                                    true
                                }
                                Key.DirectionUp -> onUp?.invoke() ?: false
                                else -> false
                            }
                        }
                        .semantics {
                            role = Role.RadioButton
                            this.selected = selected
                        }
                        .focusable()
                        .testTag("tv-panel-${if (option == Translation.SUB) "sub" else "dub"}"), // i18n-skip
                    contentAlignment = Alignment.Center,
                ) {
                    val ink = when {
                        focused -> ObiSilk.Ground
                        !enabled -> ObiSilk.Silk.copy(alpha = 0.38f)
                        else -> ObiSilk.Silk
                    }
                    val finding = pending?.takeIf { it.to == option && it.failed == null }
                    if (finding != null) {
                        // At the segment's leading edge, so its label stays where it was.
                        val shown by animateFloatAsState(if (finding.spinning(now)) 1f else 0f, Motion.fadeIn(), label = "version-spinner") // i18n-skip
                        Box(Modifier.align(Alignment.CenterStart).padding(start = 6.dp).graphicsLayer { alpha = shown; scaleX = 0.6f; scaleY = 0.6f }.testTag("tv-panel-version-spinner")) { Spinner() } // i18n-skip
                    }
                    Text(
                        if (option == Translation.SUB) Strings.playerStartSubtitled(context) else Strings.audioDubbed(context),
                        style = MaterialTheme.typography.labelLarge,
                        color = ink,
                    )
                }
            }
        }
        val episode = model.menus?.episodes?.firstOrNull { it.current }?.number ?: model.staged?.episode
        val finding = episode?.let { pending?.note(context, it, now) }
        AnimatedVisibility(finding != null, enter = fadeIn(Motion.fadeIn()) + expandVertically(Motion.standard()), exit = fadeOut(Motion.fadeOut()) + shrinkVertically(Motion.quick())) {
            Text(finding.orEmpty(), style = DetailStyle, color = ObiSilk.Haze)
        }
        val failed = pending?.failed
        AnimatedVisibility(failed != null, enter = fadeIn(Motion.fadeIn()) + expandVertically(Motion.standard()), exit = fadeOut(Motion.fadeOut()) + shrinkVertically(Motion.quick())) {
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                Icon(PanelGlyphs.Warning, contentDescription = null, tint = ObiSilk.Vermilion, modifier = Modifier.padding(top = 2.dp).size(16.dp))
                Text(failed?.text(context).orEmpty(), style = DetailStyle, color = ObiSilk.Vermilion)
            }
        }
        dub?.let { Text(it.line(context), style = DetailStyle, color = ObiSilk.Haze) }
        if (inRoom) Text(Strings.roomVersionOwn(context), style = DetailStyle, color = ObiSilk.Haze)
    }
}

private fun LazyListScope.tvOthers(
    model: PlayerModel,
    options: List<SourceOption>?,
    skip: String?,
    expanded: Set<String>,
    toggle: (String) -> Unit,
) {
    model.panelReleases?.failed?.let { why ->
        // No source was asked: why, in place of the rows.
        item(key = "sources-failed") { // i18n-skip
            val context = LocalContext.current
            Text(why.text(context), style = DetailStyle, color = ObiSilk.Haze, modifier = Modifier.padding(horizontal = 12.dp, vertical = 8.dp).testTag("tv-panel-sources-failed")) // i18n-skip
        }
        return
    }
    model.panelReleases?.takeIf { it.rows != null }?.let { asked ->
        tvListings(model, asked, options, skip, expanded, toggle)
        return
    }
    // Asked, the rows not said yet: each source's row with its line held empty, as the first rows
    // will have it, so no cached line shows for a moment and fades out.
    val pending = model.panelReleases?.let { it.rows == null && it.candidates == null } == true
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
                    TvChoiceRow(
                        model, group.name, group.option?.let { PanelWords.source(context, it) },
                        SwitchTarget(group.providerId, group.name),
                        disclosure = Strings.playerPanelReleases(context, releases.size) to open,
                        onClick = { toggle(group.providerId) },
                    )
                }
                if (open) items(releases, key = { "release/${it.providerId}/${it.id}" }) { release -> // i18n-skip
                    val context = LocalContext.current
                    TvChoiceRow(
                        model, release.title, PanelWords.release(context, release),
                        SwitchTarget(release.providerId, group.name, release = release.id),
                        sub = true, inset = true,
                        onClick = { model.switchTo(release) },
                    )
                }
            }
            releases.size == 1 && !resting -> item(key = "source/${group.providerId}") { // i18n-skip
                val context = LocalContext.current
                val release = releases.first()
                TvChoiceRow(
                    model, group.name, group.option?.let { PanelWords.source(context, it) } ?: PanelWords.release(context, release),
                    SwitchTarget(release.providerId, group.name, release = release.id),
                    onClick = { model.switchTo(release) },
                )
            }
            else -> item(key = "source/${group.providerId}") { // i18n-skip
                val context = LocalContext.current
                // Resting, or nothing for this episode: said, and focusable so a screen reader and
                // the D-pad still reach it, with nothing to pick.
                TvChoiceRow(
                    model, group.name, group.option?.takeIf { !pending }?.let { PanelWords.source(context, it) },
                    SwitchTarget(group.providerId, group.name),
                    dim = !pending && (resting || known),
                    waiting = !pending && !known && !resting,
                    listing = group.providerId.takeIf { pending },
                    onClick = null,
                )
            }
        }
    }
}

/**
 * Every other source's row as the engine says it while the tab asks, in the order set in Sources.
 * Each is there from the first event and takes focus from the start, so focus never jumps as rows
 * answer; OK does nothing on a row until it listed a release.
 */
private fun LazyListScope.tvListings(
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
            TvChoiceRow(
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
            val context = LocalContext.current
            TvChoiceRow(
                model, release.title, PanelWords.release(context, release),
                SwitchTarget(release.providerId, row.name, release = release.id),
                sub = true, inset = true,
                onClick = { model.switchTo(release) },
            )
        }
    }
}

/** What a row's detail line says: crossfaded as one, old and new together. */
private enum class Line { Detail, Starting, Silent, Asking, Counting }

/** The 16 dp spinner in a source's check slot while it is asked: silk, ground on a focused row. */
@Composable
private fun AskingSpinner(ink: Color, modifier: Modifier = Modifier) {
    // A preview, and a screenshot, draws it still.
    val still = androidx.compose.ui.platform.LocalInspectionMode.current || LocalReducedMotion.current
    val turning = rememberInfiniteTransition(label = "asking") // i18n-skip
    val turned by turning.animateFloat(0f, 360f, infiniteRepeatable(tween(900, easing = LinearEasing)), label = "angle") // i18n-skip
    val angle = if (still) 0f else turned
    Canvas(modifier.size(16.dp)) {
        val stroke = 2.dp.toPx()
        val inset = stroke / 2
        val arcSize = androidx.compose.ui.geometry.Size(size.width - stroke, size.height - stroke)
        val at = androidx.compose.ui.geometry.Offset(inset, inset)
        drawArc(ink.copy(alpha = 0.25f), 0f, 360f, useCenter = false, topLeft = at, size = arcSize, style = Stroke(stroke))
        drawArc(ink, angle - 90f, 90f, useCenter = false, topLeft = at, size = arcSize, style = Stroke(stroke))
    }
}

/** A stream, source or release on the TV: focused silk with ground text at 1.02. */
@Composable
private fun TvChoiceRow(
    model: PlayerModel,
    title: String,
    detail: String?,
    target: SwitchTarget,
    modifier: Modifier = Modifier,
    on: Boolean = false,
    sub: Boolean = false,
    inset: Boolean = false,
    dim: Boolean = false,
    waiting: Boolean = false,
    /** The source this row says while the tab asks it, by its id: its line follows the engine's rows. */
    listing: String? = null,
    disclosure: Pair<String, Boolean>? = null,
    onClick: (() -> Unit)?,
    onUp: (() -> Boolean)? = null,
) {
    val context = LocalContext.current
    var focused by remember { mutableStateOf(false) }
    val self = remember { FocusRequester() }
    val picked = model.panelSwitch?.takeIf { it.on(target) }
    val now = model.clock
    val asking = listing?.let { model.panelReleases?.line(it, now) } ?: ListingLine.Answered
    val reduced = LocalReducedMotion.current
    val scale by animateFloatAsState(if (focused && !reduced) ROW_SCALE else 1f, Motion.quick(), label = "choice") // i18n-skip
    // A source that answered with nothing goes to haze as its answer fades in.
    val rest by animateColorAsState(if (dim) ObiSilk.Haze else ObiSilk.Silk, Motion.fadeIn(), label = "ink") // i18n-skip
    val ink = if (focused) ObiSilk.Ground else rest
    val soft = if (focused) ObiSilk.Ground.copy(alpha = 0.72f) else ObiSilk.Haze
    Column(Modifier.padding(vertical = 2.dp)) {
        Row(
            modifier
                .graphicsLayer { scaleX = scale; scaleY = scale }
                .fillMaxWidth()
                .clip(RoundedCornerShape(12.dp))
                .background(if (focused) ObiSilk.Silk else Color.Transparent)
                .onFocusChanged { focused = it.isFocused }
                .onPreviewKeyEvent { event ->
                    if (event.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
                    when (event.key) {
                        Key.DirectionCenter, Key.Enter, Key.NumPadEnter -> {
                            onClick?.invoke()
                            true
                        }
                        Key.DirectionUp -> onUp?.invoke() ?: false
                        else -> false
                    }
                }
                // One stop for TalkBack: the name and its line together.
                .semantics(mergeDescendants = true) {
                    if (on) selected = true
                    if (onClick != null) onClick { onClick(); true }
                }
                .focusRequester(self)
                .focusable()
                .heightIn(min = 52.dp)
                .padding(start = 6.dp + if (inset) 30.dp else 0.dp, end = 12.dp, top = 6.dp, bottom = 6.dp)
                .testTag("tv-panel-choice-$title"), // i18n-skip
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            Box(Modifier.width(24.dp), contentAlignment = Alignment.Center) {
                if (picked != null && picked.failed == null) {
                    val shown by animateFloatAsState(if (picked.spinning(now)) 1f else 0f, Motion.fadeIn(), label = "spinner") // i18n-skip
                    Box(Modifier.graphicsLayer { alpha = shown; scaleX = 0.6f; scaleY = 0.6f }) { Spinner() }
                } else if (listing != null) {
                    // Asked: from 600 ms, faded out as the answer fades in.
                    val spins = asking == ListingLine.Asking || asking == ListingLine.Counting
                    val shown by animateFloatAsState(if (spins) 1f else 0f, Motion.fadeIn(), label = "asking-spinner") // i18n-skip
                    if (spins || shown > 0f) AskingSpinner(if (focused) ObiSilk.Ground else ObiSilk.Silk, Modifier.graphicsLayer { alpha = shown }.testTag("tv-panel-asking-$title")) // i18n-skip
                } else if (on) {
                    Icon(PanelGlyphs.Check, contentDescription = null, tint = ink, modifier = Modifier.size(18.dp))
                }
            }
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(1.dp)) {
                Text(title, style = if (sub) DetailStyle.copy(fontSize = 15.sp, lineHeight = 22.sp) else TitleStyle.copy(fontWeight = FontWeight.SemiBold), color = ink)
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
                // A source's answer crossfades in when its words come later too (its coverage after its releases).
                val shownDetail = if (listing != null && line == Line.Detail) detail else null
                Crossfade(line to shownDetail, modifier = held, animationSpec = Motion.fadeIn(), label = "line") { (said, words) -> // i18n-skip
                    when (said) {
                        Line.Starting -> Text(Strings.playerPanelStarting(context, picked?.counting(now) ?: 3), style = DetailStyle, color = soft)
                        Line.Silent -> {}
                        Line.Asking -> Text(Strings.playerStartRowAsking(context), style = DetailStyle, color = soft)
                        Line.Counting -> Text(Strings.playerPanelAsking(context, listing?.let { model.panelReleases?.seconds(it, now) } ?: 3), style = DetailStyle, color = soft)
                        Line.Detail -> when (val shown = if (listing != null) words else detail) {
                            null, "" -> if (waiting) Text("…", style = DetailStyle, color = soft) // i18n-skip
                            else -> Text(shown, style = DetailStyle, color = soft)
                        }
                    }
                }
            }
            // A source's release count and chevron fade in at the end of its row as it lists them.
            // The last count stays while it fades out.
            var lastDisclosure by remember { mutableStateOf(disclosure) }
            if (disclosure != null) lastDisclosure = disclosure
            AnimatedVisibility(disclosure != null, enter = fadeIn(Motion.fadeIn()), exit = fadeOut(Motion.fadeOut())) {
                val (count, open) = lastDisclosure ?: return@AnimatedVisibility
                val turn by animateFloatAsState(if (open) 90f else 0f, Motion.standard(), label = "chevron") // i18n-skip
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    Text(count, style = DetailStyle, color = soft)
                    Icon(PanelGlyphs.Chevron, contentDescription = null, tint = soft, modifier = Modifier.size(16.dp).rotate(turn))
                }
            }
        }
        val failed = picked?.failed
        AnimatedVisibility(failed != null, enter = fadeIn(Motion.fadeIn()) + expandVertically(Motion.standard()), exit = fadeOut(Motion.fadeOut()) + shrinkVertically(Motion.quick())) {
            Column(Modifier.padding(start = 36.dp, top = 4.dp, bottom = 6.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    Icon(PanelGlyphs.Warning, contentDescription = null, tint = ObiSilk.Vermilion, modifier = Modifier.padding(top = 2.dp).size(16.dp))
                    Text(failed?.text(context).orEmpty(), style = DetailStyle, color = ObiSilk.Vermilion)
                }
                TonalButton(
                    // Try again goes while the switch is tried, so focus moves up to its row first.
                    onClick = {
                        self.tryFocus()
                        model.retrySwitch()
                    },
                    height = 40.dp,
                    modifier = Modifier.testTag("tv-panel-try-again"),
                ) {
                    ButtonLabel(Strings.actionTryAgain(context))
                }
            }
        }
    }
}

/** The picture held while a switch loads, faded out as the new picture comes up under it. */
@Composable
internal fun TvHeldPicture(model: PlayerModel, modifier: Modifier) {
    var last by remember { mutableStateOf<HeldPicture?>(null) }
    val held = model.heldPicture
    if (held != null) last = held
    val alpha by animateFloatAsState(if (model.holding && held != null) 1f else 0f, if (model.holding) snap() else tween(PlayerModel.HELD_FADE.toInt() - 100), label = "held") // i18n-skip
    val shown = last ?: return
    if (alpha == 0f && held == null) return
    val image = remember(shown) { shown.bitmap.asImageBitmap() }
    Image(image, contentDescription = null, contentScale = ContentScale.FillBounds, modifier = modifier.graphicsLayer { this.alpha = alpha })
}
