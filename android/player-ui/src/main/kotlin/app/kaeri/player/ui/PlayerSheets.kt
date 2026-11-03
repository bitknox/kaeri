// The player's menus as modal bottom sheets (T7): subtitles and audio, with the picture's
// upscaling, night mode and speed; the episodes, with the sources; and the releases to choose
// from when a start finds nothing that plays. On a phone held sideways the subtitles and audio
// panel is a 360 dp side sheet on the end edge instead.

package app.kaeri.player.ui

import java.text.NumberFormat
import androidx.annotation.StringRes
import app.kaeri.i18n.Strings
import app.kaeri.i18n.R
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.FastOutLinearInEasing
import androidx.compose.animation.core.MutableTransitionState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.displayCutout
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Surface
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.foundation.hoverable
import androidx.compose.foundation.interaction.collectIsHoveredAsState
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.unit.sp
import androidx.compose.foundation.layout.height
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.BottomSheetDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.addPathNodes
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.DialogWindowProvider
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import java.util.Locale
import app.kaeri.design.ObiSilk
import app.kaeri.engine.model.PlayerChoice
import app.kaeri.player.PlayerModel

/** Which of the player's sheets is open. */
enum class PlayerSheet { Tracks, Episodes, Sources, SkipTimes, People }

/** The speeds the Speed list offers. */
private val SPEEDS = listOf(0.5, 0.75, 1.0, 1.25, 1.5, 2.0)

/** "1.0×", "1.25×". */
internal fun speedLabel(speed: Double): String {
    val digits = NumberFormat.getNumberInstance().apply { minimumFractionDigits = 1; maximumFractionDigits = 2 }
    return digits.format(speed) + "×" // i18n-skip
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun PlayerSheets(model: PlayerModel, sheet: PlayerSheet?, onMoreSubtitleSettings: () -> Unit = {}, onDismiss: () -> Unit) {
    if (sheet == null) return
    // The controls stay up while a sheet is open.
    DisposableEffect(sheet) {
        model.hold("sheet", true)
        onDispose { model.hold("sheet", false) }
    }
    val configuration = LocalConfiguration.current
    if (sheet == PlayerSheet.Tracks && usesSideSheet(configuration.screenWidthDp, configuration.screenHeightDp)) {
        // A phone held sideways has no room for a bottom sheet over the picture: the quick panel is
        // the board's 360 dp side sheet on the end edge, and the picture plays on beside it.
        SideSheet(onDismiss) { SheetList(model, sheet, onMoreSubtitleSettings, onDismiss, PaddingValues(top = 20.dp, bottom = 24.dp)) }
        return
    }
    ModalBottomSheet(
        onDismissRequest = {
            if (sheet == PlayerSheet.Sources) model.cancelChoosingSource()
            onDismiss()
        },
        // The Episodes sheet opens at its own height, its list capped below, so the footer under the
        // list is never left past the bottom of a half-open sheet.
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = sheet == PlayerSheet.Episodes),
        containerColor = ObiSilk.Raised,
        scrimColor = androidx.compose.material3.MaterialTheme.colorScheme.scrim.copy(alpha = 0.32f),
        dragHandle = { BottomSheetDefaults.DragHandle(color = ObiSilk.Haze.copy(alpha = 0.4f)) },
    ) {
        ImmersiveSheet()
        SheetList(model, sheet, onMoreSubtitleSettings, onDismiss, PaddingValues(bottom = 24.dp))
    }
}

/** The side sheet's width on the board, on the end edge. */
internal val SIDE_SHEET_WIDTH = 360.dp

/** Whether the width and height (dp) are a phone held sideways, where the quick panel is a side sheet. */
internal fun usesSideSheet(widthDp: Int, heightDp: Int): Boolean = widthDp > heightDp && heightDp < 480

/** The rows of the open sheet. The Episodes sheet opens on the episode playing, the Tracks sheet on the subtitles in use. */
@Composable
private fun SheetList(model: PlayerModel, sheet: PlayerSheet, onMoreSubtitleSettings: () -> Unit, onDismiss: () -> Unit, padding: PaddingValues) {
    val current = when (sheet) {
        PlayerSheet.Episodes -> model.menus?.episodes?.indexOfFirst { it.current }?.takeIf { it > 1 } ?: 0
        PlayerSheet.Tracks -> tracksOpeningRow(model.subtitleTracks.map { it.id }, model.subtitleTrack)
        PlayerSheet.Sources, PlayerSheet.SkipTimes, PlayerSheet.People -> 0
    }
    // The episode under focus or the pointer, for the footer under the Episodes list.
    var pointed by androidx.compose.runtime.remember { androidx.compose.runtime.mutableStateOf<String?>(null) }
    androidx.compose.foundation.layout.Column {
        LazyColumn(
            state = rememberLazyListState(initialFirstVisibleItemIndex = current),
            contentPadding = if (sheet == PlayerSheet.Episodes) PaddingValues(top = padding.calculateTopPadding()) else padding,
            modifier = Modifier.weight(1f, fill = false)
                .then(if (sheet == PlayerSheet.Episodes) Modifier.heightIn(max = (LocalConfiguration.current.screenHeightDp * 0.45f).dp) else Modifier),
        ) {
            when (sheet) {
                PlayerSheet.Tracks -> tracks(model, onMoreSubtitleSettings)
                PlayerSheet.Episodes -> episodes(model) { number, on -> if (on) pointed = number else if (pointed == number) pointed = null }
                PlayerSheet.Sources -> sources(model, onDismiss)
                PlayerSheet.SkipTimes -> skipTimes(model, onDismiss)
                PlayerSheet.People -> item(key = "people") { app.kaeri.ui.PartyPeopleContent(onDone = onDismiss) } // i18n-skip
            }
        }
        if (sheet == PlayerSheet.Episodes) {
            // Under the list, for the episode under focus or the pointer, else the one playing.
            val episodes = model.menus?.episodes.orEmpty()
            val shown = episodes.firstOrNull { it.number == pointed } ?: episodes.firstOrNull { it.current }
            if (shown != null) EpisodeFooter(model.titleOnScreen, shown.number, shown.label, shown.title)
            androidx.compose.foundation.layout.Spacer(Modifier.height(padding.calculateBottomPadding()))
        }
    }
}

/**
 * A 360 dp sheet sliding in from the end edge over the playing picture, with no scrim. It enters
 * with the emphasized decelerate curve over 300 ms and leaves in 200 ms; the system's animation
 * scale applies, so with animations off it simply appears. A tap beside it, Back or a swipe away closes it.
 */
@Composable
private fun SideSheet(onDismiss: () -> Unit, content: @Composable () -> Unit) {
    val visible = remember { MutableTransitionState(false).apply { targetState = true } }
    LaunchedEffect(visible.currentState, visible.targetState) {
        if (!visible.targetState && !visible.currentState) onDismiss()
    }
    Dialog(
        onDismissRequest = { visible.targetState = false },
        properties = DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false),
    ) {
        ImmersiveSheet(dim = false)
        Box(Modifier.fillMaxSize().pointerInput(Unit) { detectTapGestures { visible.targetState = false } }) {
            AnimatedVisibility(
                visibleState = visible,
                modifier = Modifier.align(Alignment.CenterEnd),
                enter = slideInHorizontally(tween(300, easing = CubicBezierEasing(0.05f, 0.7f, 0.1f, 1f))) { it } + fadeIn(tween(150)),
                exit = slideOutHorizontally(tween(200, easing = FastOutLinearInEasing)) { it } + fadeOut(tween(150, delayMillis = 50)),
            ) {
                Surface(
                    color = ObiSilk.RaisedHigh,
                    shape = RoundedCornerShape(topStart = 16.dp, bottomStart = 16.dp),
                    shadowElevation = 8.dp,
                    modifier = Modifier.width(SIDE_SHEET_WIDTH).fillMaxHeight().testTag("side-sheet").pointerInput(Unit) { detectTapGestures { } },
                ) {
                    Box(Modifier.windowInsetsPadding(WindowInsets.displayCutout.only(WindowInsetsSides.End))) { content() }
                }
            }
        }
    }
}

/** The sheet's own window keeps the system bars hidden, as the player's does. */
@Composable
private fun ImmersiveSheet(dim: Boolean = true) {
    val view = LocalView.current
    DisposableEffect(view) {
        val window = generateSequence(view.parent) { it.parent }.filterIsInstance<DialogWindowProvider>().firstOrNull()?.window
        if (window != null) {
            if (!dim) window.setDimAmount(0f)
            WindowCompat.getInsetsController(window, view).apply {
                systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
                hide(WindowInsetsCompat.Type.systemBars())
            }
        }
        onDispose {}
    }
}

/**
 * The row the Tracks sheet opens on: the one above the subtitles in use, so a list of many
 * tracks shows it. The sheet's title, the Subtitles heading and Off come before the tracks.
 */
internal fun tracksOpeningRow(subtitles: List<Long>, current: Long?): Int {
    val inUse = current?.let { id -> subtitles.indexOf(id).takeIf { it >= 0 }?.let { it + 3 } } ?: 2
    // Near the top, the top shows it already, with the sheet's title.
    return if (inUse > 5) inUse - 1 else 0
}

private fun LazyListScope.tracks(model: PlayerModel, onMore: () -> Unit) {
    item(key = "title") {
        Text(
            stringResource(R.string.player_subtitles_audio_picture),
            style = MaterialTheme.typography.titleLarge,
            color = ObiSilk.Silk,
            modifier = Modifier.padding(start = 24.dp, end = 24.dp, bottom = 8.dp).semantics { heading() },
        )
    }
    heading(R.string.player_subtitles)
    item(key = "sub-off") { Choice(stringResource(R.string.value_off), checked = model.subtitleTrack == null) { model.selectSubtitle(null) } }
    items(model.subtitleTracks, key = { "sub-${it.id}" }) { track ->
        Choice(track.name(LocalContext.current), checked = model.subtitleTrack == track.id) { model.selectSubtitle(track.id) }
    }
    divider("tracks-style")
    subtitleStyleSection(onMore)
    divider("tracks-audio")
    heading(R.string.player_audio)
    items(model.audioTracks, key = { "audio-${it.id}" }) { track ->
        Choice(track.name(LocalContext.current), checked = model.audioTrack == track.id) { model.selectAudio(track.id) }
    }
    item(key = "remembered") {
        Text(
            stringResource(R.string.player_remembered),
            style = MaterialTheme.typography.bodyMedium,
            color = ObiSilk.Haze,
            modifier = Modifier.padding(start = 56.dp, top = 4.dp, bottom = 8.dp),
        )
    }
    val menus = model.menus
    if (menus != null && menus.upscaling.options.isNotEmpty()) {
        divider("tracks-upscaling")
        heading(R.string.player_upscaling)
        items(menus.upscaling.options, key = { "look-${it.id}" }) { choice ->
            Choice(choice.label, detail = choice.hint, checked = menus.upscaling.current == choice.id) { model.setUpscaling(choice.id) }
        }
    }
    if (menus != null) {
        divider("tracks-night")
        item(key = "night") {
            Row(
                Modifier.fillMaxWidth().heightIn(min = 56.dp)
                    .toggleable(value = menus.nightMode, role = Role.Switch, onValueChange = { model.setNightMode(it) })
                    .padding(start = 56.dp, end = 24.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column(Modifier.weight(1f)) {
                    Text(stringResource(R.string.player_night_mode), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk)
                    Text(stringResource(R.string.player_night_mode_detail), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
                }
                Switch(
                    checked = menus.nightMode,
                    onCheckedChange = null,
                    colors = SwitchDefaults.colors(checkedTrackColor = ObiSilk.Amber, checkedThumbColor = ObiSilk.Ground),
                )
            }
        }
    }
    divider("tracks-speed")
    heading(R.string.player_speed)
    items(SPEEDS, key = { "speed-$it" }) { speed ->
        Choice(speedLabel(speed), checked = kotlin.math.abs(model.speed - speed) < 0.01) { model.playAt(speed) }
    }
}

private fun LazyListScope.episodes(model: PlayerModel, onPoint: (number: String, on: Boolean) -> Unit) {
    val menus = model.menus
    heading(R.string.player_episodes)
    if (menus == null || menus.episodes.isEmpty()) {
        item(key = "no-episodes") { Note(stringResource(R.string.player_sheet_episodes_loading)) }
    } else {
        items(menus.episodes, key = { "ep-${it.number}" }) { row ->
            val hover = androidx.compose.runtime.remember { androidx.compose.foundation.interaction.MutableInteractionSource() }
            val hovered by hover.collectIsHoveredAsState()
            androidx.compose.runtime.LaunchedEffect(hovered) { onPoint(row.number, hovered) }
            Choice(
                row.title?.let { "${row.label} · $it" } ?: row.label,
                detail = if (row.filler) stringResource(R.string.episode_filler) else if (row.watched) stringResource(R.string.episode_watched) else null,
                checked = row.current,
                modifier = Modifier
                    .hoverable(hover)
                    .onFocusChanged { onPoint(row.number, it.hasFocus) },
            ) { model.choose(PlayerChoice.Episode(row.number)) }
        }
    }
    if (menus != null && menus.sources.isNotEmpty()) {
        divider("episodes-sources")
        heading(R.string.player_sources)
        items(menus.sources, key = { "source-${it.id}" }) { source ->
            Choice(source.label, detail = source.health ?: source.detail, checked = source.current) {
                model.choose(PlayerChoice.Source(source.id))
            }
        }
    }
}

private fun LazyListScope.sources(model: PlayerModel, onDismiss: () -> Unit) {
    heading(R.string.player_sources)
    val choices = model.sourceChoices
    when {
        choices == null -> item(key = "finding") { Note(stringResource(R.string.player_sheet_finding)) }
        choices.isEmpty() -> item(key = "none") { Note(stringResource(R.string.player_sheet_none)) }
        else -> items(choices, key = { "${it.providerId}-${it.id}" }) { candidate ->
            val detail = listOfNotNull(
                candidate.quality?.let { "${it}p" }, // i18n-skip
                candidate.size,
                if (candidate.dubbed) stringResource(R.string.audio_dubbed) else null,
                if (candidate.dualAudio) stringResource(R.string.player_dual_audio) else null,
            ).joinToString(" · ").ifEmpty { null }
            Choice(candidate.title, detail = detail, checked = false, radio = false) {
                model.play(candidate)
                onDismiss()
            }
        }
    }
}

private fun LazyListScope.heading(@StringRes title: Int) {
    item(key = "heading-$title") {
        Text(
            stringResource(title),
            style = MaterialTheme.typography.titleMedium,
            color = ObiSilk.Haze,
            modifier = Modifier.padding(start = 56.dp, top = 12.dp, bottom = 4.dp).semantics { heading() },
        )
    }
}

private fun LazyListScope.divider(key: String) {
    item(key = key) { HorizontalDivider(Modifier.padding(vertical = 8.dp), color = ObiSilk.FaintRule) }
}

@Composable
private fun Note(text: String) {
    Text(text, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, modifier = Modifier.padding(start = 56.dp, top = 8.dp, bottom = 8.dp))
}

/** A row with the 24 dp check slot every row keeps, so the labels line up; the current one checked in silk. */
@Composable
private fun Choice(title: String, detail: String? = null, checked: Boolean, radio: Boolean = true, modifier: Modifier = Modifier, onClick: () -> Unit) {
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = 56.dp)
            .semantics {
                // A list of plain choices (the sources to pick from) is buttons, not a radio group.
                if (radio) {
                    selected = checked
                    role = Role.RadioButton
                }
            }
            .clickable(onClick = onClick)
            .padding(start = 16.dp, end = 24.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.size(24.dp)) {
            if (checked) Icon(Check, contentDescription = null, tint = ObiSilk.Silk)
        }
        Spacer(Modifier.width(16.dp))
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.Center) {
            Text(title, style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk, maxLines = 2, overflow = TextOverflow.Ellipsis)
            detail?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, maxLines = 1, overflow = TextOverflow.Ellipsis) }
        }
    }
}

private val Check: ImageVector = ImageVector.Builder("Check", 24.dp, 24.dp, 24f, 24f).apply {
    addPath(
        pathData = addPathNodes("M5 12.5 L10 17.5 L19 7"),
        stroke = SolidColor(Color.Black),
        strokeLineWidth = 2f,
        strokeLineCap = StrokeCap.Round,
        strokeLineJoin = StrokeJoin.Round,
    )
}.build()

/**
 * A footer under the Episodes list: the focused or playing episode's name and three lines of its
 * synopsis, when the title's table is cached (it never fetches). An episode spoiler protection
 * hides shows its name and no text; the room is kept only where text can land, and text that
 * arrives later fades in.
 */
@Composable
private fun EpisodeFooter(id: app.kaeri.engine.core.AnilistId?, number: String, label: String, title: String?) {
    val store = app.kaeri.ui.LocalAppStore.current
    val tables by store.titles.episodes.collectAsStateWithLifecycle()
    val rows = id?.let { tables[it] } ?: return
    val protect = app.kaeri.ui.spoilerProtected(id)
    val outlook = remember(id, rows.isEmpty()) { app.kaeri.engine.ffi.episodeText(rows) }
    if (outlook == app.kaeri.engine.model.EpisodeText.NONE) return
    val row = rows.firstOrNull { it.number == number }
    val hidden = protect && row?.spoilerAhead == true
    val text = row?.description?.takeIf { it.isNotBlank() && !hidden }
    HorizontalDivider(color = ObiSilk.FaintRule)
    Column(Modifier.fillMaxWidth().padding(start = 56.dp, end = 24.dp, top = 12.dp, bottom = 8.dp)) {
        Text(
            title?.let { "$label · $it" } ?: label, // i18n-skip
            style = MaterialTheme.typography.bodyLarge,
            color = ObiSilk.Silk,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        Box(Modifier.padding(top = 4.dp).fillMaxWidth().height(with(androidx.compose.ui.platform.LocalDensity.current) { (20.sp * 3).toDp() })) {
            androidx.compose.animation.Crossfade(text, animationSpec = app.kaeri.design.Motion.fadeIn(), label = "episode-footer") { shown -> // i18n-skip
                if (shown != null) {
                    Text(app.kaeri.design.rememberTagged(shown), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, maxLines = 3, overflow = TextOverflow.Ellipsis)
                }
            }
        }
    }
}
