// Search's extras on the phone: the count line under a title, Surprise me and Another, the
// Trending heading with its period menu, and Go to episode's number field.

package app.kaeri.ui

import app.kaeri.design.SwapFade
import androidx.compose.material3.ButtonDefaults
import androidx.compose.ui.text.font.FontWeight
import app.kaeri.design.ObiSilk
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.ui.draw.clip
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.PlainTooltip
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TooltipBox
import androidx.compose.material3.TooltipDefaults
import androidx.compose.material3.rememberTooltipState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.design.Motion
import app.kaeri.design.tabular
import app.kaeri.engine.ffi.entryCountLine
import app.kaeri.engine.ffi.trendPeriodChoice
import app.kaeri.engine.ffi.trendPeriodLabel
import app.kaeri.engine.ffi.trendPeriods
import app.kaeri.engine.model.CountLine
import app.kaeri.engine.model.DubKnown
import app.kaeri.engine.model.Entry
import app.kaeri.engine.model.SurprisePool
import app.kaeri.engine.model.TrendPeriod
import app.kaeri.store.SurprisePick
import app.kaeri.i18n.R
import kotlinx.coroutines.delay

// The count line

/**
 * The count line of [this] as the cards say it: the engine's own, said again when a title page
 * has since learned how far its dub goes.
 */
@Composable
internal fun Entry.countLineNow(): CountLine? {
    val store = LocalAppStore.current
    val live = store.titles.dubLines.collectAsStateWithLifecycle().value[id]
    return if (live == null) countLine else entryCountLine(this, DubKnown(through = live.through, complete = live.complete))
}

/**
 * "12 ep · Dub 8", the short form in a narrow cell and the long one in a wide. The dub part fades
 * in when it arrives after the cell is on screen, and the line does not move: the text only
 * grows to the right.
 */
@Composable
internal fun CountText(line: CountLine, short: Boolean, modifier: Modifier = Modifier) {
    val colors = MaterialTheme.colorScheme
    val text = if (short) line.short else line.long
    val dub = line.dub
    val base = if (dub != null && text.endsWith(" · $dub")) text.removeSuffix(" · $dub") else text
    val tail = if (dub != null && base != text) " · $dub" else null
    val fade = remember { Animatable(if (tail != null) 1f else 0f) }
    LaunchedEffect(tail != null) { fade.animateTo(if (tail != null) 1f else 0f, Motion.fadeIn()) }
    val haze = colors.onSurfaceVariant
    Text(
        buildAnnotatedString {
            append(base)
            if (tail != null) withStyle(SpanStyle(color = haze.copy(alpha = fade.value))) { append(tail) }
        },
        style = MaterialTheme.typography.bodySmall,
        color = haze,
        maxLines = 1,
        overflow = TextOverflow.Ellipsis,
        modifier = modifier.semantics { contentDescription = line.spoken },
    )
}

// Surprise me

/** Whether a pick has been on its way for 600 ms: the glyph gives way to a spinner. */
@Composable
private fun slowPick(pool: SurprisePool?): Boolean {
    var slow by remember { mutableStateOf(false) }
    LaunchedEffect(pool != null) {
        slow = false
        if (pool != null) {
            delay(600)
            slow = true
        }
    }
    return slow && pool != null
}

/**
 * Surprise me as an icon button, with its name as the description and the long-press tooltip.
 * It picks from [pool]: the filters in Search, shows' or reading's, the Planning list in the Library.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun SurpriseButton(pool: SurprisePool, modifier: Modifier = Modifier) {
    val store = LocalAppStore.current
    val asking by store.surprise.asking.collectAsStateWithLifecycle()
    val slow = slowPick(asking)
    val name = stringResource(R.string.surprise_me)
    val hint = stringResource(if (pool is SurprisePool.Planning) R.string.surprise_hint_planning else R.string.surprise_hint_filters)
    TooltipBox(
        positionProvider = TooltipDefaults.rememberTooltipPositionProvider(),
        tooltip = { PlainTooltip { Text(name) } },
        state = rememberTooltipState(),
        modifier = modifier,
    ) {
        IconButton(
            onClick = { store.surprise(pool) },
            modifier = Modifier.semantics { contentDescription = "$name. $hint" }, // i18n-skip
        ) {
            SwapFade(slow, animationSpec = Motion.standard(), label = "surprise") { spinning -> // i18n-skip
                if (spinning) {
                    CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp)
                } else {
                    Icon(Icons.Dice, contentDescription = null)
                }
            }
        }
    }
}

/** Another, the text button of a picked title's top app bar: the pick's pool asked again, in place. */
@Composable
internal fun AnotherButton(pick: SurprisePick, modifier: Modifier = Modifier) {
    val store = LocalAppStore.current
    val asking by store.surprise.asking.collectAsStateWithLifecycle()
    val slow = slowPick(asking)
    val hint = stringResource(R.string.surprise_another_hint)
    TextButton(
        onClick = { store.surprise(pick.pool, inPlace = true) },
        modifier = modifier.background(MaterialTheme.colorScheme.surfaceContainer, androidx.compose.foundation.shape.CircleShape).semantics { contentDescription = hint },
    ) {
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            if (slow) CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp)
            Text(stringResource(R.string.surprise_another))
        }
    }
}

/** Opens each pick as it comes: a new page, or in place of the one on screen for Another. */
@Composable
internal fun SurpriseHost() {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val removed by rememberUpdatedState(app.kaeri.design.rememberAnimationsRemoved())
    LaunchedEffect(store) {
        store.surprise.picks.collect { pick ->
            if (pick.pool is SurprisePool.Reading) {
                // A book opens as a series, as its tile in reading's Search does.
                navigator.openSeries("a:${pick.id}", "surprise")
            } else if (pick.inPlace) {
                // With Remove animations on the host swaps at once, so a still of the old page fades out over the new.
                val fade = navigator.fade
                if (removed && fade != null) {
                    fade.swap(fullSpeed = true) { navigator.replaceTitle(pick.id) }
                } else {
                    navigator.replaceTitle(pick.id)
                    delay(600)
                }
                navigator.replaced()
            } else {
                navigator.openTitle(pick.id, "surprise")
            }
        }
    }
}

// Trending

/** The period of a Trending heading as the menu says it: "Today". */
@Composable
internal fun TrendPeriod.choice(): String = trendPeriodChoice(this)

/**
 * The Trending heading with its period menu: "Trending today ⌄". The label crossfades on quick
 * when the period changes; the menu is the system's own, the current period checked.
 */
@Composable
internal fun TrendingHeading(period: TrendPeriod, onPick: (TrendPeriod) -> Unit, modifier: Modifier = Modifier) {
    var open by remember { mutableStateOf(false) }
    val label = trendPeriodLabel(period)
    Box(modifier) {
        Row(
            Modifier
                .clickable(role = Role.Button) { open = true }
                .padding(vertical = 12.dp)
                .semantics { heading() },
            horizontalArrangement = Arrangement.spacedBy(4.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            SwapFade(label, animationSpec = Motion.quick(), label = "trending heading") { // i18n-skip
                Text(it, style = MaterialTheme.typography.titleMedium)
            }
            Icon(Icons.Down, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(18.dp))
        }
        Menu(expanded = open, onDismissRequest = { open = false }) {
            for (each in trendPeriods()) {
                DropdownMenuItem(
                    text = { Text(trendPeriodChoice(each)) },
                    leadingIcon = { Box(Modifier.size(24.dp), contentAlignment = Alignment.Center) { if (each == period) Icon(Icons.Check, contentDescription = null) } },
                    onClick = {
                        open = false
                        onPick(each)
                    },
                )
            }
        }
    }
}

// Go to episode

/**
 * The number field of Go to episode: the range menu's last item opens it. Go applies; the error
 * is the engine's, in the field, and a number that is a row goes to it.
 */
@Composable
fun GoToEpisodeDialog(
    rows: List<app.kaeri.engine.model.EpisodeRow>,
    onGo: (index: Int, range: Int?) -> Unit,
    onDismiss: () -> Unit,
) {
    val context = LocalContext.current
    var typed by remember { mutableStateOf("") }
    val now = nowSeconds()
    val result = remember(typed, rows) { app.kaeri.engine.ffi.goToEpisode(rows, typed, now) }
    val message = remember(result) { app.kaeri.engine.ffi.goToMessage(result) }
    val hint = remember(rows) { app.kaeri.engine.ffi.goToHint(rows) }
    val label = remember(rows) { app.kaeri.engine.ffi.goToLabel(rows) }
    val episode = stringResource(R.string.library_episode)
    val go = {
        val row = result as? app.kaeri.engine.model.GoTo.Row
        if (row != null) onGo(row.index.toInt(), row.range?.toInt())
    }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.episode_go_to_title)) },
        text = {
            OutlinedTextField(
                value = typed,
                onValueChange = { typed = it.filter(Char::isDigit).take(5) },
                label = { Text(episode) },
                placeholder = { Text(hint) },
                singleLine = true,
                isError = message != null,
                supportingText = { Text(message ?: hint) },
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                keyboardActions = androidx.compose.foundation.text.KeyboardActions(onGo = { go() }),
                modifier = Modifier.semantics { contentDescription = label },
            )
        },
        // No amber in this dialog: both buttons are silk, Go in semibold.
        confirmButton = {
            TextButton(onClick = go, enabled = result is app.kaeri.engine.model.GoTo.Row, colors = ButtonDefaults.textButtonColors(contentColor = ObiSilk.Silk)) {
                Text(stringResource(R.string.episode_go_to_go), fontWeight = FontWeight.SemiBold)
            }
        },
        dismissButton = { TextButton(onClick = onDismiss, colors = ButtonDefaults.textButtonColors(contentColor = ObiSilk.Silk)) { Text(stringResource(R.string.action_cancel)) } },
    )
}

/**
 * The pick the title page for [id] was opened by, while it is on screen: its Another goes when the
 * page closes. `null` for a page opened any other way.
 */
@Composable
internal fun surprisePick(id: app.kaeri.engine.core.AnilistId): SurprisePick? {
    val store = LocalAppStore.current
    val visit by store.surprise.visit.collectAsStateWithLifecycle()
    androidx.compose.runtime.DisposableEffect(id) { onDispose { store.surprise.left(id) } }
    return visit?.takeIf { it.id == id }
}

/** One entry of a range menu: the first and last episode in it, and Watched or Not out yet. */
data class RangeItem(val from: Int, val to: Int, val state: String? = null)

/**
 * A range picker as a chip: "301–400 ⌄", with the menu of ranges and Go to episode last. The
 * player's Episodes panel carries it in its heading row.
 */
@Composable
fun RangeChip(
    items: List<RangeItem>,
    selected: Int,
    onPick: (Int) -> Unit,
    modifier: Modifier = Modifier,
    onGoTo: (() -> Unit)? = null,
    container: androidx.compose.ui.graphics.Color = MaterialTheme.colorScheme.outlineVariant,
    content: androidx.compose.ui.graphics.Color = MaterialTheme.colorScheme.onSurface,
) {
    val context = LocalContext.current
    var open by remember { mutableStateOf(false) }
    val current = items[selected.coerceIn(0, items.size - 1)]
    val label = app.kaeri.i18n.Strings.episodeRangeShort(context, current.from.toString(), current.to.toString())
    val word = stringResource(R.string.episode_range_label)
    val shape = androidx.compose.foundation.shape.RoundedCornerShape(8.dp)
    Box(modifier) {
        Row(
            Modifier
                .height(40.dp)
                .clip(shape)
                .border(1.dp, container, shape)
                .clickable(role = Role.Button, onClickLabel = word) { open = true }
                .semantics(mergeDescendants = true) {
                    contentDescription = word
                    stateDescription = app.kaeri.i18n.Strings.episodeRange(context, current.from, current.to)
                }
                .padding(start = 12.dp, end = 8.dp),
            horizontalArrangement = Arrangement.spacedBy(4.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            SwapFade(label, animationSpec = Motion.standard(), label = "range chip") { // i18n-skip
                Text(it, style = MaterialTheme.typography.bodyMedium.tabular(), color = content)
            }
            Icon(Icons.Down, contentDescription = null, tint = content, modifier = Modifier.size(18.dp))
        }
        Menu(expanded = open, onDismissRequest = { open = false }) {
            items.forEachIndexed { index, item ->
                DropdownMenuItem(
                    text = {
                        Column {
                            Text(app.kaeri.i18n.Strings.episodeRange(context, item.from, item.to), style = MaterialTheme.typography.bodyLarge.tabular())
                            if (item.state != null) Text(item.state, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                    },
                    trailingIcon = { if (index == selected) Icon(Icons.Check, contentDescription = null) },
                    onClick = {
                        open = false
                        onPick(index)
                    },
                )
            }
            if (onGoTo != null) {
                androidx.compose.material3.HorizontalDivider()
                DropdownMenuItem(text = { Text(stringResource(R.string.episode_go_to)) }, onClick = {
                    open = false
                    onGoTo()
                })
            }
        }
    }
}
