// What Kaeri is for: the three radio rows first run, the sheets and Settings share; the sheet that
// asks once after the update, or when this device joins devices that answer differently; Settings'
// page; and the confirmation before a medium leaves, which says what leaves and what stays.

package app.kaeri.ui.reading

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.RadioButtonDefaults
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.engine.model.MediumAsk
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.store.Wording
import app.kaeri.store.DevicePreferences
import app.kaeri.engine.model.SettingKey
import app.kaeri.ui.Destination
import app.kaeri.ui.FIRST_LAUNCH_DONE
import app.kaeri.ui.LocalNavigator
import app.kaeri.store.reads
import app.kaeri.store.watches
import app.kaeri.ui.LocalAppStore
import app.kaeri.ui.QuietButton
import app.kaeri.ui.SettingSwitchMark
import app.kaeri.ui.SettingsFootnote
import app.kaeri.ui.SettingsSheet
import app.kaeri.ui.SheetButtons
import app.kaeri.ui.SheetText
import app.kaeri.ui.SheetTitle
import app.kaeri.ui.kindWord

/** The three answers, in the order every place lists them. */
internal val MEDIUM_CHOICES = listOf(KaeriIsFor.WATCH, KaeriIsFor.READ, KaeriIsFor.BOTH)

/** "Watch", "Read", "Both": a choice's name. */
@Composable
internal fun mediumName(answer: KaeriIsFor): String = stringResource(
    when (answer) {
        KaeriIsFor.WATCH -> R.string.medium_watch
        KaeriIsFor.READ -> R.string.medium_read
        KaeriIsFor.BOTH -> R.string.medium_both
    },
)

/** The line under a choice, in the person's terms. */
@Composable
internal fun mediumLine(answer: KaeriIsFor): String = stringResource(
    when (answer) {
        KaeriIsFor.WATCH -> R.string.medium_watch_line
        KaeriIsFor.READ -> R.string.medium_read_line
        KaeriIsFor.BOTH -> R.string.medium_both_line
    },
)

/** The answer as Settings says it: "Watching", "Reading", "Watching and reading". */
internal fun mediumAnswerWords(context: android.content.Context, answer: KaeriIsFor): String = when (answer) {
    KaeriIsFor.WATCH -> Strings.mediumAnswerWatch(context)
    KaeriIsFor.READ -> Strings.mediumAnswerRead(context)
    KaeriIsFor.BOTH -> Strings.mediumAnswerBoth(context)
}

/**
 * Watch, Read and Both as M3 radio rows, each one target with its name and what it brings, a
 * rule between them. [radio] is the chosen ring's colour: silk on first run's pages and sheets,
 * amber in Settings, as its selection is.
 */
@Composable
internal fun MediumChoiceRows(
    chosen: KaeriIsFor?,
    modifier: Modifier = Modifier,
    radio: Color = MaterialTheme.colorScheme.onSurface,
    inset: Boolean = false,
    onChoose: (KaeriIsFor) -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    val context = LocalContext.current
    Column(modifier.selectableGroup()) {
        MEDIUM_CHOICES.forEachIndexed { index, answer ->
            if (index > 0) HorizontalDivider(color = colors.outlineVariant, modifier = Modifier.padding(horizontal = if (inset) 16.dp else 0.dp))
            val on = chosen == answer
            val position = Strings.mediumChoicePosition(context, index + 1, MEDIUM_CHOICES.size)
            Row(
                Modifier
                    .fillMaxWidth()
                    .heightIn(min = 72.dp)
                    .selectable(selected = on, role = Role.RadioButton, onClick = { onChoose(answer) })
                    .semantics { stateDescription = position }
                    .padding(horizontal = if (inset) 16.dp else 0.dp, vertical = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                RadioButton(
                    selected = on,
                    onClick = null,
                    colors = RadioButtonDefaults.colors(selectedColor = radio, unselectedColor = colors.onSurfaceVariant),
                )
                Column(Modifier.weight(1f)) {
                    Text(mediumName(answer), style = MaterialTheme.typography.bodyLarge, color = colors.onSurface)
                    Text(mediumLine(answer), style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant)
                }
            }
        }
    }
}

// The question outside first run, and Open at

/**
 * What the app hosts over every screen for reading: the medium's question when first launch's
 * pages are not asking it, and, once per launch with both media on and Open at on Read, the move
 * to Read's Home. [firstLaunch] and [covered] are AppRoot's: whether the pages may show, and
 * whether they cover the app now.
 */
@Composable
fun ReadingAppHost(start: Destination, firstLaunch: Boolean, covered: Boolean) {
    val context = LocalContext.current
    val pending = firstLaunch && (covered || !DevicePreferences.of(context).getBoolean(FIRST_LAUNCH_DONE, false))
    MediumAskHost(firstRunElsewhere = pending)
    OpenAtLaunch(start)
}

/** Read's Home at launch, when Open at says so with both media on and nothing else was opened first. */
@Composable
private fun OpenAtLaunch(start: Destination) {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    var decided by rememberSaveable { mutableStateOf(false) }
    val settings by store.settings.view.collectAsStateWithLifecycle()
    val medium by store.medium.view.collectAsStateWithLifecycle()
    LaunchedEffect(settings != null, medium != null) {
        val answer = medium?.answer
        if (decided || settings == null || answer == null) return@LaunchedEffect
        decided = true
        val readFirst = answer == KaeriIsFor.BOTH && settingToken(store, SettingKey.OPEN_AT, settings) == "read" // i18n-skip
        if (readFirst && start == Destination.Home && navigator.current() == Destination.Home) navigator.go(Destination.Read)
    }
}

/**
 * The question the engine asks once, over the app: after the update with Watch chosen, or on
 * joining with the other devices' answer chosen and the devices named. A first run's question
 * shows here only when first launch's pages are not asking it themselves ([firstRunElsewhere]).
 */
@Composable
fun MediumAskHost(firstRunElsewhere: Boolean) {
    val store = LocalAppStore.current
    val ask by store.medium.asking.collectAsStateWithLifecycle()
    val shown = ask ?: return
    if (shown is MediumAsk.FirstRun && firstRunElsewhere) return
    MediumAskSheet(shown)
}

/** What a sheet has chosen as it opens: Watch after the update, the others' answer on joining, nothing on a first run. */
internal fun presetAnswer(ask: MediumAsk): KaeriIsFor? = when (ask) {
    is MediumAsk.FirstRun -> null
    is MediumAsk.Updated -> KaeriIsFor.WATCH
    is MediumAsk.Joined -> ask.answer
}

/**
 * "Watch, read, or both?" or "Kim's tablet is set to reading": the line, the three rows, that it
 * can change in Settings, and Continue, filled at the bottom right, the one action. Swiped away,
 * it keeps what is chosen, as Continue would.
 */
@Composable
private fun MediumAskSheet(ask: MediumAsk) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    var chosen by rememberSaveable(ask) { mutableStateOf(presetAnswer(ask)) }
    val device = kindWord()
    val answerWith = { answer: KaeriIsFor -> store.medium.answer(ask, answer) }
    SettingsSheet(
        onDismiss = { answerWith(chosen ?: store.medium.answer) },
        // Nothing chosen on a first run: it stays until a row is.
        dismissible = chosen != null,
    ) {
        val title = when (ask) {
            is MediumAsk.Joined -> joinTitle(context, ask.devices, ask.answer)
            else -> stringResource(R.string.onboarding_medium_title)
        }
        val line = when (ask) {
            is MediumAsk.FirstRun -> stringResource(R.string.onboarding_medium_line)
            is MediumAsk.Updated -> Strings.mediumUpdateLine(context, device)
            is MediumAsk.Joined -> Strings.mediumJoinLine(context, device)
        }
        SheetTitle(title)
        SheetText(line)
        MediumChoiceRows(chosen, Modifier.padding(top = 16.dp)) { chosen = it }
        if (ask !is MediumAsk.FirstRun) {
            Text(
                stringResource(R.string.medium_change_later),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 8.dp),
            )
        }
        SheetButtons {
            val hint = stringResource(R.string.onboarding_medium_hint)
            Button(
                onClick = { chosen?.let(answerWith) },
                enabled = chosen != null,
                modifier = Modifier.semantics { if (chosen == null) stateDescription = hint },
            ) { Text(stringResource(R.string.onboarding_continue)) }
        }
    }
}

/** "Kim's phone and Kim's tablet are set to watching and reading". */
private fun joinTitle(context: android.content.Context, devices: List<String>, answer: KaeriIsFor): String {
    val names = Wording.list(devices)
    return when (answer) {
        KaeriIsFor.WATCH -> Strings.mediumJoinTitleWatch(context, names, devices.size)
        KaeriIsFor.READ -> Strings.mediumJoinTitleRead(context, names, devices.size)
        KaeriIsFor.BOTH -> Strings.mediumJoinTitleBoth(context, names, devices.size)
    }
}

// Settings

/**
 * Settings' choice: Both, or a medium turned on, applies at once; one turned off waits for the
 * confirmation, which says what leaves and what stays.
 */
internal class MediumSwitch {
    /** The medium about to leave, and the answer after it. */
    var leaving by mutableStateOf<Pair<KaeriIsFor, KaeriIsFor>?>(null)

    fun choose(store: app.kaeri.store.AppStore, answer: KaeriIsFor) {
        val current = store.medium.answer
        if (answer == current) return
        val leaves = when {
            answer == KaeriIsFor.WATCH && current.reads -> KaeriIsFor.READ
            answer == KaeriIsFor.READ && current.watches -> KaeriIsFor.WATCH
            else -> null
        }
        if (leaves == null) store.medium.choose(answer) else leaving = leaves to answer
    }
}

/**
 * Kaeri is for, Settings' page: the three rows on a card, the chosen one marked in amber, and
 * that each device keeps its own answer. The mark moves only once the answer has changed.
 */
@Composable
internal fun MediumPane() {
    val store = LocalAppStore.current
    val view by store.medium.view.collectAsStateWithLifecycle()
    val switch = remember { MediumSwitch() }
    LaunchedEffect(view?.answer) { if (view?.answer?.reads == true) store.reading.loadDownloads() }
    Column {
        MediumChoiceRows(
            view?.answer,
            Modifier.clip(RoundedCornerShape(16.dp)).background(MaterialTheme.colorScheme.surfaceContainer),
            radio = MaterialTheme.colorScheme.primary,
            inset = true,
        ) { switch.choose(store, it) }
        SettingsFootnote(stringResource(R.string.settings_medium_footer))
    }
    switch.leaving?.let { (medium, to) ->
        MediumOffSheet(medium, onDismiss = { switch.leaving = null }) { delete ->
            store.medium.choose(to, delete)
            switch.leaving = null
        }
    }
}

/**
 * "Turn off watching?": what leaves, that the list, progress and downloads stay, and Delete
 * downloaded episodes as its own switch, off, with what they take on this device. The amber
 * action turns the medium off; Cancel leaves everything as it was.
 */
@Composable
internal fun MediumOffSheet(medium: KaeriIsFor, onDismiss: () -> Unit, confirm: (deleteDownloads: Boolean) -> Unit) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val device = kindWord()
    var delete by rememberSaveable { mutableStateOf(false) }
    val watching = medium == KaeriIsFor.WATCH
    val episodes by store.downloads.rows.collectAsStateWithLifecycle()
    val chapters by store.reading.downloads.collectAsStateWithLifecycle()
    val bytes = if (watching) store.downloads.bytes(episodes) else chapters?.meter?.used?.toLong() ?: 0L
    SettingsSheet(onDismiss, holdsInput = delete) {
        SheetTitle(stringResource(if (watching) R.string.medium_off_title_watch else R.string.medium_off_title_read))
        SheetText(if (watching) Strings.mediumOffWatchBody(context, device) else Strings.mediumOffReadBody(context, device))
        val onWord = stringResource(R.string.value_on)
        val offWord = stringResource(R.string.value_off)
        Row(
            Modifier
                .padding(top = 16.dp)
                .fillMaxWidth()
                .heightIn(min = 72.dp)
                .clip(RoundedCornerShape(16.dp))
                .background(MaterialTheme.colorScheme.surfaceContainerHigh)
                .toggleable(delete, role = Role.Switch) { delete = it }
                .semantics { stateDescription = if (delete) onWord else offWord }
                .padding(horizontal = 16.dp, vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Column(Modifier.weight(1f)) {
                Text(
                    stringResource(if (watching) R.string.medium_off_delete_watch else R.string.medium_off_delete_read),
                    style = MaterialTheme.typography.bodyLarge,
                )
                if (bytes > 0) {
                    Text(
                        Strings.mediumOffDeleteSize(context, Wording.bytes(context, bytes), device),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
            SettingSwitchMark(delete)
        }
        SheetButtons {
            QuietButton(stringResource(R.string.action_cancel), onClick = onDismiss)
            Button(onClick = { confirm(delete) }) {
                Text(stringResource(if (watching) R.string.medium_off_confirm_watch else R.string.medium_off_confirm_read))
            }
        }
        Spacer(Modifier.height(4.dp))
    }
}
