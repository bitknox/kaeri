// Open link or file (AndroidOpenLink): a magnet link, info hash or torrent file the person has.
// The engine guesses the title and episode from its name and the sheet asks "Is this ...?", with
// Change to pick another title and Play without tracking for a link that is nothing in the
// catalogue. Confirmed, it plays and downloads as any episode does. One amber action at a time:
// Open, then Play, or Turn on torrent playback while that is off.

package app.kaeri.ui

import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.IconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import app.kaeri.engine.model.Entry
import app.kaeri.engine.model.SettingKey
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.store.OpenedModel
import app.kaeri.store.readAtMost
import app.kaeri.store.text

/** The largest torrent file read: they hold a list of pieces, not the media, and are small. */
private const val MAX_FILE_BYTES = 8 * 1024 * 1024

/** The sheet over any screen, while one is open. */
@Composable
internal fun OpenedSheetHost() {
    val store = LocalAppStore.current
    val stage by store.opened.stage.collectAsStateWithLifecycle()
    val shown = stage ?: return
    SettingsSheet(onDismiss = store.opened::close, tall = true) {
        when (shown) {
            is OpenedModel.Stage.Paste -> PasteStage(shown)
            is OpenedModel.Stage.Confirm -> ConfirmStage(shown, onTurnOn = { store.setSwitch(SettingKey.TORRENTS, true) })
            is OpenedModel.Stage.Change -> ChangeStage(shown)
        }
    }
}

@Composable
private fun PasteStage(stage: OpenedModel.Stage.Paste) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var link by rememberSaveable { mutableStateOf("") }
    val picker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri: Uri? ->
        uri ?: return@rememberLauncherForActivityResult
        scope.launch {
            val bytes = withContext(Dispatchers.IO) {
                runCatching {
                    context.contentResolver.openInputStream(uri)?.use { it.readAtMost(MAX_FILE_BYTES + 1) }
                }.getOrNull()
            }
            if (bytes != null && bytes.size <= MAX_FILE_BYTES) store.openTorrentFile(bytes) else store.opened.unusable()
        }
    }
    SheetTitle(Strings.openedPasteTitle(context))
    SheetField(
        link,
        { link = it },
        Strings.openedPastePlaceholder(context),
        keyboard = KeyboardOptions(keyboardType = KeyboardType.Uri, autoCorrectEnabled = false),
        enabled = !stage.reading,
        error = stage.unusable,
        trailing = if (link.isNotEmpty()) {
            { IconButton(onClick = { link = "" }) { Icon(Icons.Clear, contentDescription = stringResource(R.string.action_clear)) } }
        } else {
            null
        },
    )
    if (stage.unusable) SheetProblem(Strings.torrentLinkUnusable(context))
    if (stage.reading) {
        Row(Modifier.padding(top = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            WorkRing(true)
        }
    }
    SheetButtons {
        QuietButton(Strings.openedChooseFile(context), enabled = !stage.reading) { picker.launch(arrayOf("application/x-bittorrent", "application/octet-stream", "*/*")) }
        Button(onClick = { store.openLink(link) }, enabled = link.isNotBlank() && !stage.reading) { Text(stringResource(R.string.action_open)) }
    }
}

@Composable
private fun ConfirmStage(stage: OpenedModel.Stage.Confirm, onTurnOn: () -> Unit) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val on = stage.opened.playbackOn || store.settings.isOn(SettingKey.TORRENTS, view)
    val title = stage.title
    val episode = stage.episode.trim()
    SheetTitle(Strings.openedOpen(context))
    stage.opened.name?.let { SheetText(it, Modifier.padding(bottom = 8.dp)) }
    Text(
        when {
            title == null -> Strings.openedNoGuess(context)
            episode.isEmpty() -> Strings.openedConfirmTitle(context, title.title)
            else -> Strings.openedConfirm(context, title.title, episode)
        },
        style = MaterialTheme.typography.bodyLarge,
    )
    if (title != null) {
        SheetField(
            stage.episode,
            store.opened::episode,
            Strings.openedEpisode(context),
            Modifier.padding(top = 16.dp),
            keyboard = KeyboardOptions(keyboardType = KeyboardType.Text, autoCorrectEnabled = false),
        )
    }
    SheetButtons {
        QuietButton(Strings.openedChange(context), onClick = store.opened::change)
        if (title != null) {
            QuietButton(Strings.openedDownload(context), enabled = on && episode.isNotEmpty()) {
                store.downloadOpened(stage.opened.link, title.id, episode)
            }
        }
        if (on) {
            Button(onClick = { title?.let { store.playOpened(stage.opened.link, it.id, episode, it.title) } }, enabled = title != null && episode.isNotEmpty()) {
                Text(Strings.openedPlay(context))
            }
        } else {
            Button(onClick = onTurnOn) { Text(Strings.openedTurnOn(context)) }
        }
    }
    Column(Modifier.padding(top = 8.dp)) {
        QuietButton(Strings.openedPlayUntracked(context), enabled = on) {
            store.playOpenedUntracked(stage.opened.link, stage.opened.name ?: Strings.openedOpen(context))
        }
        SheetText(Strings.openedUntrackedDetail(context))
    }
}

@Composable
private fun ChangeStage(stage: OpenedModel.Stage.Change) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    var query by rememberSaveable { mutableStateOf(stage.query) }
    LaunchedEffect(query) {
        if (query.isBlank() || query.trim() == stage.query) return@LaunchedEffect
        delay(400)
        store.findOpened(stage.opened.link, query)
    }
    SheetTitle(Strings.openedChangeTitle(context))
    SheetField(
        query,
        { query = it },
        Strings.openedChangeSearch(context),
        keyboard = KeyboardOptions(keyboardType = KeyboardType.Text, autoCorrectEnabled = false),
    )
    stage.failed?.let { SheetProblem(Strings.openedSearchFailed(context, it.text(context))) }
    if (stage.searching) {
        Row(Modifier.padding(top = 16.dp)) { WorkRing(true) }
    }
    Column(Modifier.padding(top = 8.dp)) {
        stage.results.orEmpty().forEach { entry -> ChangeRow(entry) { store.opened.pick(entry) } }
    }
    SheetButtons { QuietButton(stringResource(R.string.action_back), onClick = store.opened::back) }
}

/** One title the link could be: its name, and its year and format in haze. */
@Composable
private fun ChangeRow(entry: Entry, onClick: () -> Unit) {
    val line = listOfNotNull(entry.year?.toString(), entry.format).joinToString(" · ") // i18n-skip
    Column(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 56.dp)
            .clickable(onClick = onClick)
            .padding(vertical = 8.dp),
        verticalArrangement = Arrangement.Center,
    ) {
        Text(entry.title, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
        if (line.isNotEmpty()) SheetText(line)
    }
}
