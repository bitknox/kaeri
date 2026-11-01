// TVOpened: a magnet link a paired phone sent. The TV asks "Is this <title>, episode N?" with Play,
// Change and Play without tracking. A guess with no episode starts the stepper on episode 1.
// Change lists the titles the name could be; typing a search is the phone's, which sent the link.

package app.kaeri.tv.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import app.kaeri.design.ObiSilk
import app.kaeri.engine.model.SettingKey
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.store.AppStore
import app.kaeri.store.OpenedModel

/** Titles listed under Change: what fits on a TV without scrolling. */
private const val MAX_LISTED = 6

/** The dialog over any screen, while a link is open. [onTurnOn] opens torrent playback's own page. */
@Composable
fun OpenedDialog(store: AppStore, onTurnOn: () -> Unit) {
    val stage by store.opened.stage.collectAsStateWithLifecycle()
    val shown = stage ?: return
    val close = store.opened::close
    BackHandler { if (shown is OpenedModel.Stage.Change) store.opened.back() else close() }
    Dialog(onDismissRequest = close) {
        val first = remember { FocusRequester() }
        Column(
            Modifier
                .entryFocus(first)
                .widthIn(max = 640.dp)
                .background(ObiSilk.RaisedHigh, RoundedCornerShape(28.dp))
                .padding(32.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            when (shown) {
                is OpenedModel.Stage.Confirm -> Confirm(store, shown, first, onTurnOn = { close(); onTurnOn() })
                is OpenedModel.Stage.Change -> Change(store, shown, first)
                // The TV is only ever sent a link: the field is the phone's.
                is OpenedModel.Stage.Paste -> Unit
            }
        }
    }
}

@Composable
private fun Confirm(store: AppStore, stage: OpenedModel.Stage.Confirm, first: FocusRequester, onTurnOn: () -> Unit) {
    val context = LocalContext.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val on = stage.opened.playbackOn || store.settings.isOn(SettingKey.TORRENTS, view)
    val title = stage.title
    val episode = stage.episode.trim().ifEmpty { if (title != null) "1" else "" }
    Text(
        when {
            title == null -> Strings.openedNoGuess(context)
            else -> Strings.openedConfirm(context, title.title, episode)
        },
        style = MaterialTheme.typography.headlineSmall,
        color = ObiSilk.Silk,
    )
    stage.opened.name?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze) }
    // A guess with no episode: step to the right one.
    val number = episode.toIntOrNull()
    if (title != null && stage.opened.guess?.episode == null && number != null) {
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            TonalButton(onClick = { store.opened.episode((number - 1).coerceAtLeast(1).toString()) }) { ButtonLabel("−") } // i18n-skip
            TonalButton(onClick = { store.opened.episode((number + 1).toString()) }) { ButtonLabel("+") } // i18n-skip
        }
    }
    Spacer(Modifier.height(8.dp))
    Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        if (on) {
            AmberButton(
                onClick = { title?.let { store.playOpened(stage.opened.link, it.id, episode, it.title) } },
                modifier = Modifier.focusRequester(first),
            ) { ButtonLabel(Strings.openedPlay(context)) }
        } else {
            AmberButton(onClick = onTurnOn, modifier = Modifier.focusRequester(first)) { ButtonLabel(Strings.openedTurnOn(context)) }
        }
        TonalButton(onClick = store.opened::change) { ButtonLabel(Strings.openedChange(context)) }
        if (on) {
            TonalButton(onClick = { store.playOpenedUntracked(stage.opened.link, stage.opened.name ?: Strings.openedOpen(context)) }) {
                ButtonLabel(Strings.openedPlayUntracked(context))
            }
        }
    }
    if (on) Text(Strings.openedUntrackedDetail(context), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
}

@Composable
private fun Change(store: AppStore, stage: OpenedModel.Stage.Change, first: FocusRequester) {
    val context = LocalContext.current
    Text(Strings.openedChangeTitle(context), style = MaterialTheme.typography.headlineSmall, color = ObiSilk.Silk)
    val listed = stage.results.orEmpty().take(MAX_LISTED)
    listed.forEachIndexed { index, entry ->
        TonalButton(
            onClick = { store.opened.pick(entry) },
            modifier = if (index == 0) Modifier.focusRequester(first) else Modifier,
        ) { ButtonLabel(entry.title) }
    }
    TonalButton(onClick = store.opened::back, modifier = if (listed.isEmpty()) Modifier.focusRequester(first) else Modifier) {
        ButtonLabel(stringResource(R.string.action_back))
    }
}
