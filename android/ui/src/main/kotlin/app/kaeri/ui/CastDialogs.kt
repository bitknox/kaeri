// The cast button's dialogs: Play on TV, the chooser, while no TV is held, and the held TV's own,
// with what plays, its volume and Stop casting.

package app.kaeri.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.BasicAlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.engine.ffi.FfiTvRowKind
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.store.CastDialog
import app.kaeri.store.CastItem
import app.kaeri.store.CastModel
import app.kaeri.store.CastSessionState

/** The cast button's dialog, while it is up. */
@Composable
internal fun CastDialogHost() {
    val cast = LocalAppStore.current.cast
    val dialog by cast.dialog.collectAsStateWithLifecycle()
    when (dialog) {
        CastDialog.Chooser -> CastChooser(cast)
        CastDialog.Connected -> CastConnectedDialog(cast)
        null -> Unit
    }
}

/** An M3 dialog on surfaceContainerHigh: its title at 24, then [content], then its buttons at the end. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun CastDialogFrame(title: String, onDismiss: () -> Unit, buttons: @Composable () -> Unit, content: @Composable () -> Unit) {
    BasicAlertDialog(onDismissRequest = onDismiss) {
        Surface(shape = RoundedCornerShape(28.dp), color = MaterialTheme.colorScheme.surfaceContainerHigh) {
            Column(Modifier.padding(top = 24.dp, bottom = 16.dp)) {
                Text(
                    title,
                    style = MaterialTheme.typography.headlineSmall,
                    color = MaterialTheme.colorScheme.onSurface,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.padding(horizontal = 24.dp).semantics { heading() },
                )
                Column(Modifier.weight(1f, fill = false).verticalScroll(rememberScrollState()).padding(top = 16.dp)) { content() }
                Row(Modifier.fillMaxWidth().padding(start = 24.dp, end = 16.dp, top = 8.dp), horizontalArrangement = Arrangement.End) { buttons() }
            }
        }
    }
}

/** A group's heading in the chooser: Chromecast, On this network. */
@Composable
private fun ChooserHeading(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.labelLarge,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(start = 24.dp, end = 24.dp, top = 8.dp, bottom = 4.dp).semantics { heading() },
    )
}

/** A TV in the chooser: the TV glyph, its name at 16 and its line at 13, 60 high. */
@Composable
private fun ChooserRow(name: String, detail: String?, enabled: Boolean = true, onClick: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 60.dp)
            .clickable(enabled = enabled, onClick = onClick)
            .alpha(if (enabled) 1f else 0.38f)
            .padding(horizontal = 24.dp, vertical = 8.dp)
            .semantics(mergeDescendants = true) {},
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Icon(Icons.Tv, contentDescription = null, tint = colors.onSurfaceVariant)
        Column(Modifier.weight(1f)) {
            Text(name, style = MaterialTheme.typography.bodyLarge, color = colors.onSurface, maxLines = 1, overflow = TextOverflow.Ellipsis)
            detail?.let { Text(it, style = MaterialTheme.typography.bodySmall.copy(fontSize = 13.sp), color = colors.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis) }
        }
    }
}

/**
 * Play on TV from the cast button: the TVs with Chromecast built in, then the TVs on this
 * network that run Kaeri. Picking a TV connects to it; nothing plays.
 */
@Composable
private fun CastChooser(cast: CastModel) {
    val store = LocalAppStore.current
    val rows by cast.rows.collectAsStateWithLifecycle()
    val tvRows by store.tv.rows.collectAsStateWithLifecycle()
    val blocked by cast.blocked.collectAsStateWithLifecycle()
    val problem by cast.problem.collectAsStateWithLifecycle()
    val kaeri = tvRows.filter { it.kind != FfiTvRowKind.PAIRED_AWAY }
    CastDialogFrame(
        stringResource(R.string.tv_play_on),
        onDismiss = cast::closeDialog,
        buttons = { TextButton(onClick = cast::closeDialog) { Text(stringResource(R.string.action_cancel)) } },
    ) {
        if (blocked) {
            Box(Modifier.padding(horizontal = 24.dp)) { CastLocalNetworkRow() }
            return@CastDialogFrame
        }
        if (rows.isNotEmpty()) {
            ChooserHeading(stringResource(R.string.cast_sheet_group))
            for (row in rows) ChooserRow(row.name, castRowDetail(row)) { cast.connect(row) }
        }
        if (kaeri.isNotEmpty()) {
            ChooserHeading(stringResource(R.string.tv_sheet_on_network))
            for (row in kaeri) ChooserRow(row.name, row.detail, enabled = row.kind != FfiTvRowKind.NEEDS_UPDATE) { cast.choose(row) }
        }
        problem?.let { Box(Modifier.padding(horizontal = 24.dp)) { ProblemLine(it) } }
    }
}

/**
 * The held TV's dialog: "Connected to Living room", what plays there, which opens the remote, or
 * "Nothing playing yet", its volume, and Stop casting while something plays or Disconnect while
 * nothing does. Both let the TV go. A cast from Play on TV holds no session: its card says what
 * plays and where.
 */
@Composable
private fun CastConnectedDialog(cast: CastModel) {
    val session by cast.session.collectAsStateWithLifecycle()
    val shown by cast.shown.collectAsStateWithLifecycle()
    val joining by cast.joining.collectAsStateWithLifecycle()
    val context = LocalContext.current
    val colors = MaterialTheme.colorScheme
    val linked = shown?.takeIf { !session.held && it.live }
    val name = linked?.tvName ?: session.deviceName ?: joining.orEmpty()
    val item = session.item
    val linkedNow = linked?.let { CastModel.now(it) }
    val playing = linkedNow != null || (session.state == CastSessionState.Playing && item != null)
    val opens = shown != null && (item != null || linkedNow != null)
    val openLabel = stringResource(R.string.cast_mini_opens)
    CastDialogFrame(
        stringResource(R.string.cast_button_connected, name),
        onDismiss = cast::closeDialog,
        buttons = { TextButton(onClick = cast::disconnect) { Text(stringResource(if (playing) R.string.cast_stop else R.string.action_disconnect)) } },
    ) {
        Column(Modifier.padding(horizontal = 24.dp)) {
            Row(
                Modifier
                    .fillMaxWidth()
                    .heightIn(min = 56.dp)
                    .then(
                        if (opens) {
                            Modifier.clickable(onClickLabel = openLabel) {
                                cast.closeDialog()
                                cast.openCard()
                            }
                        } else {
                            Modifier
                        },
                    )
                    .semantics(mergeDescendants = true) {},
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                val title = linkedNow?.title ?: item?.title
                val line = linkedNow?.let { it.episodeLine.ifEmpty { Strings.castEpisode(context, it.episode) } } ?: item?.let { castEpisodeLine(context, it) }
                if (title != null) {
                    CastArt(linkedNow?.id ?: item?.id, item?.image, Modifier.size(64.dp, 36.dp).clip(RoundedCornerShape(6.dp)))
                    Column(Modifier.weight(1f)) {
                        Text(title, style = MaterialTheme.typography.bodyLarge, color = colors.onSurface, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        Text(line.orEmpty(), style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    }
                    if (opens) Icon(Icons.Chevron, contentDescription = null, tint = colors.onSurfaceVariant, modifier = Modifier.size(20.dp))
                } else {
                    CastGlyph(CastGlyphState.Filled, size = 18.dp, tint = colors.onSurfaceVariant)
                    Text(stringResource(R.string.cast_idle), style = MaterialTheme.typography.bodyLarge, color = colors.onSurface)
                }
            }
            Spacer(Modifier.height(4.dp))
            if (linked != null) {
                CastVolumeRow(cast, linked.level, linked.canMute, linked.cast.muted == true, linkedNow?.volume, enabled = !linked.quiet)
            } else {
                CastVolumeRow(cast, session.levelControl, session.canMute, session.muted == true, session.level?.let { it * 100 } ?: shown?.cast?.card?.now?.volume, enabled = session.held)
            }
        }
    }
}

/** "Episode 7 · The Lantern Road", or the number alone. */
internal fun castEpisodeLine(context: android.content.Context, item: CastItem): String {
    val episode = item.episode ?: return ""
    return item.episodeTitle?.let { Strings.castEpisodeTitled(context, episode, it) } ?: Strings.castEpisode(context, episode)
}

