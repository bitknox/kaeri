// Settings' panes: Playback (AndroidSettingsPlayback, AndroidSettingsTitles), Downloads, Sources
// (AndroidSettingsSources), Accounts and About, each a column of groups, the same on a phone's page
// and in a tablet's middle column.

package app.kaeri.ui

import android.os.Build
import app.kaeri.i18n.Language
import app.kaeri.store.AppLanguage
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import app.kaeri.store.reads
import app.kaeri.ui.reading.chapterNoticesRow
import app.kaeri.store.text
import app.kaeri.ui.reading.downloadsFinishedRow
import app.kaeri.store.watches
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import java.time.Instant
import java.util.Locale
import app.kaeri.design.Nagare
import app.kaeri.design.ShowTitle
import app.kaeri.design.tabular
import app.kaeri.engine.core.AnilistId
import app.kaeri.engine.core.SettingValue
import app.kaeri.engine.model.NoticeReason
import app.kaeri.engine.model.UpcomingNotice
import app.kaeri.engine.model.Entry
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.ProxyStatus
import app.kaeri.engine.model.ProxyView
import app.kaeri.engine.model.SettingKey
import app.kaeri.engine.model.StorePluginStatus
import app.kaeri.engine.model.TitleLanguage
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import android.content.Context
import java.text.NumberFormat
import androidx.compose.ui.res.pluralStringResource
import app.kaeri.store.AccountsModel
import app.kaeri.store.AirTime
import app.kaeri.store.ComingModel
import app.kaeri.store.DevicePreferences
import app.kaeri.store.LibraryModel
import app.kaeri.store.TorrentWords
import app.kaeri.store.SettingsCatalog
import app.kaeri.store.Wording
import app.kaeri.store.updates

/** The focus keys of the rows a tablet's column shows the effect of. */
internal const val FOCUS_UPSCALING = "upscaling"
internal const val FOCUS_TITLES = "titles"

/** A value as a row says it, in sentence case, for a token the app itself does not word. */
internal fun sentence(value: String): String = value.replaceFirstChar { it.titlecase(Locale.getDefault()) }

/** "Subbed" or "Dubbed", for sub or dub as the engine says it. */
internal fun translationWords(context: Context, value: String): String = when (value.substringBefore(" · ")) {
    "sub" -> Strings.audioSubbed(context)
    "dub" -> Strings.audioDubbed(context)
    else -> sentence(value)
}

/**
 * A stepped setting as a menu of its choices, each named as its row will say it, with how many
 * shows set it for themselves under it.
 */
@Composable
internal fun SettingMenu(
    place: Place,
    key: SettingKey,
    title: String,
    note: String? = null,
    enabled: Boolean = true,
    focus: String? = null,
) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val row = store.settings.row(key, view)
    val current = SettingsCatalog.current(row)
    MenuRow(
        place = place,
        title = title,
        value = SettingsCatalog.words(context, row),
        choices = SettingsCatalog.options(key).map { MenuChoice(it.value, SettingsCatalog.words(context, key, it)) },
        current = current?.value,
        note = listOfNotNull(store.settings.overridden(LocalContext.current, key, view), note).joinToString("\n").ifEmpty { null }, // i18n-skip
        enabled = enabled && SettingsCatalog.editable(row),
        focus = focus,
    ) { store.setSetting(key, it) }
}

/** A setting that is on or off, with how many shows set it for themselves under it. */
@Composable
internal fun SettingSwitch(place: Place, key: SettingKey, title: String, note: String? = null, enabled: Boolean = true, extra: String? = null) {
    val store = LocalAppStore.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val row = store.settings.row(key, view)
    SwitchRow(
        place = place,
        title = title,
        checked = SettingsCatalog.isOn(row),
        note = listOfNotNull(store.settings.overridden(LocalContext.current, key, view), note).joinToString("\n").ifEmpty { null }, // i18n-skip
        enabled = enabled && SettingsCatalog.editable(row),
        extra = extra,
    ) { store.setSetting(key, SettingValue.Bool(it)) }
}

/** Groups 8 apart, so a header's own 8 above it makes 16. */
@Composable
internal fun Pane(content: @Composable () -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) { content() }
}

// Notifications

/**
 * Notifications, as AndroidSettingsNotify draws it: the New episodes switch for this device on a
 * card of its own, with the shows that set their own, and the note that the permission is asked
 * then; then the system's settings for the channel. Turning the switch on goes through the app,
 * which asks for the permission to post then, and only then; where Android says no, the switch
 * stays off and its line says why. Under it, "Coming up · next 7 days": the episodes of the shows
 * the check follows, dimmed while the switch is off, each opening its show's settings, where its
 * own switch is.
 */
@Composable
internal fun NotificationsPane(actions: AppActions) {
    val store = LocalAppStore.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val key = SettingKey.EPISODE_NOTIFICATIONS
    val row = store.settings.row(key, view)
    val app = actions.notifications
    val refused = app?.refused?.collectAsStateWithLifecycle()?.value == true
    val week by store.coming.week.collectAsStateWithLifecycle()
    var opened by remember { mutableStateOf<AnilistId?>(null) }
    val answer = kaeriIsFor()
    LaunchedEffect(answer.watches) { if (answer.watches) store.loadComing() }
    Pane {
        // One group: New chapters with reading on, New episodes with watching on, then Downloads finished.
        val episodes: @Composable (Place) -> Unit = { place ->
                    SwitchRow(
                        place = place,
                        title = stringResource(R.string.settings_new_episodes),
                        checked = SettingsCatalog.isOn(row),
                        note = listOfNotNull(
                            if (refused) stringResource(R.string.settings_notify_refused) else stringResource(R.string.settings_new_episodes_detail),
                            store.settings.overridden(LocalContext.current, key, view),
                        ).joinToString("\n"),
                        enabled = SettingsCatalog.editable(row),
                    ) { on -> app?.turn?.invoke(on) ?: store.setSwitch(key, on) }
        }
        SettingsGroup(
            footer = listOfNotNull(
                if (answer.reads) stringResource(R.string.settings_chapter_notifications_footer) else null,
                // The system asks for the permission from Android 13; before that there is nothing to ask.
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) stringResource(R.string.settings_notify_permission_note) else null,
            ).joinToString(" "), // i18n-skip
            rows = listOfNotNull(
                if (answer.reads) chapterNoticesRow else null,
                if (answer.watches) episodes else null,
                if (answer.reads) downloadsFinishedRow else null,
            ),
        )
        if (answer.watches) week?.let { week ->
            SettingsGroup(
                title = stringResource(R.string.settings_coming_up),
                footer = ComingModel.footer(LocalContext.current, week),
                rows = ComingModel.listed(week).map { episode ->
                    @Composable { place: Place -> ComingRow(place, episode) { opened = episode.id } }
                },
            )
        }
        if (app != null) {
            SettingsGroup(
                modifier = Modifier.padding(top = 8.dp),
                rows = listOf(
                    { place ->
                        InfoRow(
                            place,
                            stringResource(R.string.settings_notify_system_title),
                            stringResource(R.string.settings_notify_system_detail),
                            onClick = app.openSystemSettings,
                            trailing = { RowMark(Icons.External) },
                        )
                    },
                ),
            )
        }
    }

    // A show's settings, over the pane; its own switch may change what is coming up.
    opened?.let { id ->
        val titles by store.titles.entries.collectAsStateWithLifecycle()
        val entry = titles[id]
        if (entry == null) {
            LaunchedEffect(id) { store.loadDetail(id) }
        } else {
            ShowSettingsSheet(entry) {
                opened = null
                store.loadComing()
            }
        }
    }
}

/**
 * One episode coming up: the show's cover, its title, "E4 · Today 22:00", and "On in this show's
 * settings" for a show followed by its own switch. Dimmed when it does not notify, as the engine
 * says: a show following the switch while it is off. A show switched on stays.
 */
@Composable
private fun ComingRow(place: Place, episode: UpcomingNotice, onClick: () -> Unit) {
    val dimmed = !episode.notifies
    val note = if (episode.reason == NoticeReason.SWITCHED_ON) stringResource(R.string.settings_notifications_show_on) else null
    val context = LocalContext.current
    InfoRow(
        place,
        episode.title,
        Strings.calendarRowTime(context, episode.episode.toInt(), AirTime.coming(context, Instant.ofEpochSecond(episode.airsAt))),
        note = note,
        onClick = onClick,
        leading = {
            ArtworkImage(
                episode.coverUrl,
                plate = null,
                maxPixelSize = pixels(60.dp),
                modifier = Modifier.size(width = 40.dp, height = 60.dp).clip(RoundedCornerShape(6.dp)),
            )
        },
        trailing = { RowMark(Icons.Chevron) },
        modifier = Modifier.alpha(if (dimmed) 0.4f else 1f),
        // A show's title, so the show-title face, as everywhere a title is named.
        titleStyle = ShowTitle.row,
    )
}

// General

/** How titles read and which are listed: nothing here is about playing an episode. */
@Composable
internal fun GeneralPane() {
    val answer = kaeriIsFor()
    Pane {
        // Which Home the app starts on, with both media on.
        if (answer == app.kaeri.engine.model.KaeriIsFor.BOTH) {
            SettingsGroup(rows = listOf(app.kaeri.ui.reading.openAtRow))
        }
        SettingsGroup(
            title = stringResource(R.string.settings_language_app),
            rows = listOf({ place -> LanguageRow(place) }),
        )
        SettingsGroup(
            title = stringResource(R.string.settings_group_titles),
            rows = listOf(
                { place -> TitleLanguageRow(place) },
                { place -> SettingSwitch(place, SettingKey.ADULT_TITLES, stringResource(R.string.settings_adult), note = stringResource(if (answer == app.kaeri.engine.model.KaeriIsFor.READ) R.string.settings_adult_detail_read else R.string.settings_adult_detail, "AniList")) },
            ),
        )
    }
}

// Playback

@Composable
internal fun PlaybackPane() {
    val store = LocalAppStore.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val autoNext = store.settings.isOn(SettingKey.AUTO_NEXT, view)
    Pane {
        SettingsGroup(
            title = stringResource(R.string.settings_language),
            footer = stringResource(R.string.settings_audio_detail),
            rows = listOf(
                { place -> SettingMenu(place, SettingKey.TRANSLATION, stringResource(R.string.settings_audio)) },
                { place -> SubtitlesRow(place) },
                { place -> SettingMenu(place, SettingKey.QUALITY, stringResource(R.string.settings_quality)) },
            ),
        )
        val snackbars = LocalSnackbars.current
        val context = LocalContext.current
        SubtitlesGroup { before ->
            snackbars?.show(context.getString(R.string.settings_subs_reset_done), context.getString(R.string.action_undo)) {
                store.restoreSubtitleStyle(before)
            }
        }
        SettingsGroup(
            title = stringResource(R.string.settings_group_episodes),
            rows = listOf(
                { place -> SettingSwitch(place, SettingKey.AUTO_NEXT, stringResource(R.string.settings_auto_play)) },
                { place -> SettingMenu(place, SettingKey.UP_NEXT_COUNTDOWN, stringResource(R.string.settings_up_next_countdown), enabled = autoNext) },
                { place -> SettingSwitch(place, SettingKey.SKIP_OPENING, stringResource(R.string.settings_skip_opening)) },
                { place -> SettingSwitch(place, SettingKey.SKIP_RECAP, stringResource(R.string.settings_skip_recap)) },
                { place -> SettingSwitch(place, SettingKey.SKIP_FILLER, stringResource(R.string.settings_skip_filler)) },
                { place -> SettingSwitch(place, SettingKey.MARK_CHAPTERS, stringResource(R.string.settings_mark_opening)) },
                { place -> SettingSwitch(place, SettingKey.SKIP_BUTTON, stringResource(R.string.settings_skip_button), note = stringResource(R.string.settings_skip_button_detail)) },
                { place ->
                    // Paused says nothing in the player: the reason is here and in the Skip times panel.
                    val paused = app.kaeri.store.skipFindPausedLine(context, view?.skipFindPaused)
                    SettingSwitch(place, SettingKey.FIND_SKIP_TIMES, stringResource(R.string.settings_find_skips), note = stringResource(R.string.settings_find_skips_detail), extra = paused)
                },
                { place -> SettingSwitch(place, SettingKey.ASK_WAS_OPENING, stringResource(R.string.settings_ask_was_opening), note = stringResource(R.string.settings_ask_was_opening_detail)) },
                { place -> SettingSwitch(place, SettingKey.SHARE_SKIP_TIMES, stringResource(R.string.settings_share_skips), note = stringResource(R.string.settings_share_skips_detail)) },
                { place -> ActionRow(place, stringResource(R.string.settings_clear_skips_android), note = stringResource(R.string.settings_clear_skips_android_detail)) { store.send(Command.ClearSkipMarks) } },
                { place -> SettingMenu(place, SettingKey.COMMIT_THRESHOLD, stringResource(R.string.settings_counts_watched), note = stringResource(R.string.settings_counts_watched_detail)) },
                { place -> SettingSwitch(place, SettingKey.SPOILER_PROTECTION, stringResource(R.string.settings_spoiler), note = stringResource(R.string.settings_spoiler_detail)) },
            ),
        )
        SettingsGroup(
            title = stringResource(R.string.player_picture_and_sound),
            rows = listOf(
                { place -> SettingMenu(place, SettingKey.UPSCALING, stringResource(R.string.player_upscaling), focus = FOCUS_UPSCALING) },
                { place -> SettingSwitch(place, SettingKey.NIGHT_MODE, stringResource(R.string.player_night_mode), note = stringResource(R.string.player_night_mode_detail)) },
            ),
        )
        // Hide chat is this device's own: the engine always sends the room's words.
        val hidden by store.party.hideChat.collectAsStateWithLifecycle()
        LaunchedEffect(Unit) { store.party.loadHideChat(context) }
        SettingsGroup(
            title = stringResource(R.string.together_title),
            rows = listOf(
                { place -> SwitchRow(place, Strings.settingsHideChat(context), checked = hidden, note = Strings.settingsHideChatLine(context)) { store.party.setHideChat(context, it) } },
            ),
        )
    }
}

/** The three forms a title can be named in, as the settings file names them. */
internal enum class TitleChoice(val value: String) {
    AniList("anilist"),
    English("english"),
    Romaji("romaji");

    /** The choice as the menu names it. */
    fun label(context: Context): String = when (this) {
        AniList -> Strings.settingsTitleTracker(context, "AniList") // i18n-skip
        English -> Strings.settingsTitleEnglish(context)
        Romaji -> Strings.settingsTitleRomaji(context)
    }

    companion object {
        fun of(value: SettingValue?): TitleChoice = entries.firstOrNull { (value as? SettingValue.Str)?.v1 == it.value } ?: AniList
    }
}

/** What the title-language row and its preview say: metadata.md, "The title-language row". */
internal object TitleLanguageWords {
    /** The line under the label: what the choice does now. */
    fun line(context: Context, choice: TitleChoice, signedIn: Boolean, viewer: TitleLanguage?): String = when (choice) {
        TitleChoice.AniList -> when {
            !signedIn -> Strings.settingsTitlesLineSignedOut(context)
            viewer != null -> Strings.settingsTitlesLineAccountLanguage(context, name(context, viewer))
            else -> Strings.settingsTitlesLineFollows(context)
        }
        TitleChoice.English -> Strings.settingsTitlesLineEnglish(context)
        TitleChoice.Romaji -> Strings.settingsTitlesLineRomaji(context)
    }

    /** The note under the tablet's examples. */
    fun note(context: Context, choice: TitleChoice, signedIn: Boolean, viewer: TitleLanguage?, examples: List<Entry>): String = when (choice) {
        TitleChoice.AniList ->
            if (signedIn && viewer != null) Strings.settingsTitlesNoteAccount(context, name(context, viewer))
            else Strings.settingsTitlesNoteFollows(context)
        TitleChoice.English -> examples.firstOrNull { it.titleEnglish.isNullOrEmpty() }
            ?.let { Strings.settingsTitlesNoteEnglishMissing(context, it.titleRomaji ?: it.title) }
            ?: Strings.settingsTitlesNoteEnglish(context)
        TitleChoice.Romaji -> Strings.settingsTitlesNoteRomaji(context)
    }

    /** A title language as every sentence names it, with its capital: "English". */
    fun name(context: Context, language: TitleLanguage): String = when (language) {
        TitleLanguage.ROMAJI -> Strings.settingsTitleRomaji(context)
        TitleLanguage.ENGLISH -> Strings.settingsTitleEnglish(context)
        TitleLanguage.NATIVE -> Strings.settingsTitleNative(context)
    }

    /**
     * [entry]'s title as [choice] would name it, the way the engine picks: the form asked for,
     * else English, else romaji, else native. AniList's choice follows the signed-in account, and
     * English while nobody is.
     */
    fun example(entry: Entry, choice: TitleChoice, signedIn: Boolean, viewer: TitleLanguage?): String {
        val language = when (choice) {
            TitleChoice.AniList -> if (signedIn) viewer ?: TitleLanguage.ENGLISH else TitleLanguage.ENGLISH
            TitleChoice.English -> TitleLanguage.ENGLISH
            TitleChoice.Romaji -> TitleLanguage.ROMAJI
        }
        val chosen = when (language) {
            TitleLanguage.ROMAJI -> entry.titleRomaji
            TitleLanguage.ENGLISH -> entry.titleEnglish
            TitleLanguage.NATIVE -> entry.titleNative
        }
        val forms = listOf(chosen, entry.titleEnglish, entry.titleRomaji, entry.titleNative)
        return forms.firstOrNull { !it.isNullOrEmpty() } ?: entry.title
    }
}

/** Three titles to show the choice on: Continue's, then the Library's, then this season's, with covers. */
@Composable
internal fun titleLanguageExamples(): List<Entry> {
    val store = LocalAppStore.current
    val titles by store.titles.entries.collectAsStateWithLifecycle()
    val continuing by store.continuing.list.state.collectAsStateWithLifecycle()
    val seasonal by store.seasonal.state.collectAsStateWithLifecycle()
    val library = LibraryModel.order.flatMap { store.library.segment(it).state.value.ids }
    return (continuing.ids + library + seasonal.ids)
        .distinct()
        .mapNotNull { titles[it] }
        .filter { it.coverUrl != null }
        .take(3)
}

/** Whether AniList is signed in, for the title-language row's words. */
@Composable
internal fun anilistSignedIn(): Boolean {
    val trackers by LocalAppStore.current.accounts.trackers.collectAsStateWithLifecycle()
    return trackers["anilist"]?.connected == true
}

/**
 * Title language, the first row: AniList's choice, English or Romaji, each in the menu with an
 * example title under it, and a line under the label saying what the choice does now.
 */
@Composable
private fun TitleLanguageRow(place: Place) {
    val store = LocalAppStore.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val viewer by store.accounts.viewerTitleLanguage.collectAsStateWithLifecycle()
    val signedIn = anilistSignedIn()
    val row = store.settings.row(SettingKey.TITLES, view)
    val choice = TitleChoice.of(SettingsCatalog.current(row)?.value)
    val example = titleLanguageExamples().firstOrNull()
    val context = LocalContext.current
    LaunchedEffect(Unit) { store.loadContinue() }
    MenuRow(
        place = place,
        title = stringResource(R.string.settings_title_language),
        value = choice.label(context),
        choices = TitleChoice.entries.map { MenuChoice(it, it.label(context), example?.let { e -> TitleLanguageWords.example(e, it, signedIn, viewer) }) },
        current = choice,
        note = TitleLanguageWords.line(context, choice, signedIn, viewer),
        enabled = row != null,
        focus = FOCUS_TITLES,
        menuWidth = 300,
    ) { if (it != choice) store.setSetting(SettingKey.TITLES, SettingValue.Str(it.value)) }
}

/**
 * The app's language: the system's per-app setting from Android 13, the app's own choice below.
 * Only the languages this build ships are listed, a draft one labelled so; it follows the
 * system's until one is chosen.
 */
@Composable
private fun LanguageRow(place: Place) {
    val context = LocalContext.current
    var chosen by remember { mutableStateOf(AppLanguage.chosen(context)) }
    val follow = stringResource(R.string.language_follow_system)
    val draft = stringResource(R.string.language_draft_label)
    fun name(language: Language) = if (language.draft) "${language.name} · $draft" else language.name // i18n-skip
    MenuRow(
        place = place,
        title = stringResource(R.string.settings_language),
        value = chosen?.let(::name) ?: follow,
        valueFlag = chosen?.flagRes,
        choices = listOf(MenuChoice<Language?>(null, follow)) + AppLanguage.shipped.map { MenuChoice<Language?>(it, name(it), flagRes = it.flagRes) },
        current = chosen,
        note = if (chosen?.draft == true) stringResource(R.string.language_draft_note) else null,
    ) { picked ->
        chosen = picked
        AppLanguage.choose(context, picked)
        AppLanguage.applyToApp(context.applicationContext)
        // From Android 13 the system recreates the activity; below, the app does.
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) generateSequence(context) { (it as? android.content.ContextWrapper)?.baseContext }.filterIsInstance<android.app.Activity>().firstOrNull()?.recreate()
    }
}

/** The subtitle language, by name, from the languages people pick most; any other is typed in the file. */
@Composable
private fun SubtitlesRow(place: Place) {
    val store = LocalAppStore.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val row = store.settings.row(SettingKey.SUBTITLES, view)
    val code = SettingsCatalog.typedText(row) ?: row?.value ?: ""
    val languages = listOf("en", "ja", "es", "pt", "fr", "de", "it", "ru", "ar", "id", "ko", "zh")
    val choices = (languages + listOf(code).filter { it.isNotEmpty() && it !in languages }).map { MenuChoice(it, languageName(it)) }
    MenuRow(place, stringResource(R.string.player_subtitles), languageName(code), choices, code, enabled = SettingsCatalog.editable(row)) {
        store.setSetting(SettingKey.SUBTITLES, SettingValue.Str(it))
    }
}

/** A language code as its name in the app's language: "English" for "en" or "eng". */
internal fun languageName(code: String): String {
    if (code.isEmpty()) return "–"
    val name = Locale.forLanguageTag(code).getDisplayLanguage(Locale.getDefault())
    return if (name.isNullOrEmpty() || name.equals(code, ignoreCase = true)) code else name
}

// Downloads

@Composable
internal fun DownloadsPane(actions: AppActions) {
    val store = LocalAppStore.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val downloads by store.downloads.rows.collectAsStateWithLifecycle()
    val torrents = store.settings.isOn(SettingKey.TORRENTS, view)
    val answer = kaeriIsFor()
    val chapters by store.reading.downloads.collectAsStateWithLifecycle()
    LaunchedEffect(answer.reads) { if (answer.reads) store.reading.loadDownloads() }
    Pane {
        val free = freeBytes()
        // The folder is shared: the storage row counts episodes and chapters together.
        val used = (if (answer.watches) store.downloads.bytes(downloads) else 0L) +
            (if (answer.reads) chapters?.meter?.used?.toLong() ?: 0L else 0L)
        val episodes = if (answer.watches) downloads.size else 0
        val chapterCount = if (answer.reads) chapters?.rows?.size ?: 0 else 0
        SettingsGroup(
            rows = listOf(
                { place -> StorageRow(place, used, free) },
                { place ->
                    val navigator = LocalNavigator.current
                    InfoRow(
                        place,
                        stringResource(R.string.downloads_title),
                        // Episodes and chapters are counted apart, each in its own word.
                        listOfNotNull(
                            if (episodes > 0) pluralStringResource(R.plurals.settings_downloads_count, episodes, episodes) else null,
                            // The engine's own counts name the unit: chapters and a novel's parts, then the size.
                            if (chapterCount > 0) {
                                val context = LocalContext.current
                                chapters?.total?.dropLast(1)?.takeIf { it.isNotEmpty() }?.joinToString(" · ") { it.text(context) }
                                    ?: pluralStringResource(R.plurals.settings_downloads_chapters, chapterCount, chapterCount)
                            } else null,
                        ).joinToString(" · ").ifEmpty { stringResource(R.string.settings_none_yet) },
                        onClick = { navigator.open(DOWNLOADS_ROUTE) },
                    )
                },
            ),
        )
        // A group per medium with both on; with one, its rows under New downloads.
        if (answer.reads) {
            SettingsGroup(
                title = stringResource(if (answer.watches) R.string.settings_group_reading else R.string.settings_new_downloads),
                rows = app.kaeri.ui.reading.readingDownloadRows(),
            )
        }
        if (answer.watches) SettingsGroup(
            title = stringResource(if (answer.reads) R.string.settings_group_shows else R.string.settings_new_downloads),
            rows = listOfNotNull(
                @Composable { place: Place -> SettingMenu(place, SettingKey.DOWNLOAD_QUALITY, stringResource(R.string.settings_quality)) },
                { place -> SettingMenu(place, SettingKey.KEEP_NEXT_EPISODES, stringResource(R.string.settings_keep_next), note = stringResource(R.string.settings_keep_next_detail)) },
                { place -> WifiOnlyRow(place) },
                { place -> SettingSwitch(place, SettingKey.MERGE_SUBTITLES, stringResource(R.string.settings_merge_subtitles)) },
                if (torrents) @Composable { place: Place -> SettingSwitch(place, SettingKey.KEEP_SEEDING, stringResource(R.string.settings_keep_seeding)) } else null,
            ),
        )
    }
}

/**
 * What the downloads take beside what is free, as a bar: amber for the downloads' share of what
 * the app could use, on the rule.
 */
@Composable
private fun StorageRow(place: Place, used: Long, free: Long?) {
    val colors = MaterialTheme.colorScheme
    val description = stringResource(R.string.downloads_storage_share_description, Wording.bytes(LocalContext.current, used))
    RowFrame(place, onClick = null, modifier = Modifier.heightIn(min = 88.dp)) {
        Column(Modifier.fillMaxWidth()) {
            Text(stringResource(R.string.settings_group_storage), style = MaterialTheme.typography.titleMedium)
            Text(storageWords(LocalContext.current, used, free), style = MaterialTheme.typography.bodyMedium.tabular(), color = colors.onSurfaceVariant)
            val share = if (free != null && used + free > 0) used.toFloat() / (used + free) else 0f
            androidx.compose.material3.LinearProgressIndicator(
                progress = { share.coerceIn(0f, 1f) },
                modifier = Modifier.padding(top = 8.dp).fillMaxWidth().height(4.dp)
                    .semantics { contentDescription = description },
                // Water, not amber: a share of the disk is neither an action nor progress.
                color = colors.secondary,
                trackColor = colors.outline,
                gapSize = if (share > 0f) 4.dp else 0.dp,
                drawStopIndicator = {},
            )
        }
    }
}

/** Wi-Fi only: downloads on mobile data, turned over, as a phone says it. */
@Composable
private fun WifiOnlyRow(place: Place) {
    val store = LocalAppStore.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val row = store.settings.row(SettingKey.DOWNLOADS_ON_CELLULAR, view)
    SwitchRow(place, stringResource(R.string.settings_wifi_only), checked = !SettingsCatalog.isOn(row), note = stringResource(R.string.settings_wifi_only_detail), enabled = SettingsCatalog.editable(row)) {
        store.setSetting(SettingKey.DOWNLOADS_ON_CELLULAR, SettingValue.Bool(!it))
    }
}

// Sources

@Composable
internal fun SourcesPane(actions: AppActions) {
    val answer = kaeriIsFor()
    // Reading alone: the book sources are the pane.
    if (!answer.watches) {
        var adding by rememberSaveable { mutableStateOf(false) }
        Pane { app.kaeri.ui.reading.BookSourcesList(onAdd = { adding = true }) }
        if (adding) app.kaeri.ui.reading.AddSourceSheet { adding = false }
        return
    }
    val store = LocalAppStore.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val providers by store.providers.rows.collectAsStateWithLifecycle()
    val providersLoaded by store.providers.loaded.collectAsStateWithLifecycle()
    val stores by store.stores.stores.collectAsStateWithLifecycle()
    val storesLoaded by store.stores.loaded.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) {
        store.checkProviders()
        store.loadStores()
    }
    val torrents = store.settings.isOn(SettingKey.TORRENTS, view)
    val updates = stores.sumOf { it.updates }
    var proxySheet by rememberSaveable { mutableStateOf(false) }
    val proxy = view?.torrent?.proxy
    val check by store.providers.check.collectAsStateWithLifecycle()
    val context = LocalContext.current
    val cellular = remember { context.packageManager.hasSystemFeature(android.content.pm.PackageManager.FEATURE_TELEPHONY) }
    Pane {
        SettingsGroup(
            rows = listOfNotNull(
                @Composable { place: Place ->
                    val navigator = LocalNavigator.current
                    InfoRow(place, stringResource(R.string.sources_order), pluralStringResource(R.plurals.settings_summary_sources, providers.size, providers.size).takeIf { providersLoaded }, onClick = { navigator.open(PROVIDERS_ROUTE) })
                },
                { place ->
                    val navigator = LocalNavigator.current
                    InfoRow(
                        place,
                        stringResource(R.string.nav_plugin_stores),
                        when {
                            !storesLoaded && stores.isEmpty() -> null
                            stores.isEmpty() -> stringResource(R.string.settings_none_joined)
                            updates > 0 -> stringResource(R.string.settings_stores_with_updates, stores.size, pluralStringResource(R.plurals.stores_updates_waiting, updates, updates))
                            else -> NumberFormat.getIntegerInstance().format(stores.size)
                        },
                        onClick = { navigator.open(STORES_ROUTE) },
                    )
                },
                // With both media on, the books' sources are a page of their own.
                if (answer.reads) @Composable { place: Place ->
                    val navigator = LocalNavigator.current
                    val books by store.bookSources.view.collectAsStateWithLifecycle()
                    LaunchedEffect(Unit) { store.bookSources.load() }
                    InfoRow(
                        place,
                        stringResource(R.string.settings_book_sources),
                        app.kaeri.ui.reading.bookSourcesSummary(context, books),
                        onClick = { navigator.open(app.kaeri.ui.reading.BOOK_SOURCES_ROUTE) },
                    )
                } else null,
            ),
        )
        SettingsGroup(
            title = stringResource(R.string.torrents_title),
            rows = listOfNotNull(
                @Composable { place: Place -> TorrentSourceRow(place, torrents, proxy) },
                @Composable { place: Place -> ProxyRouteRow(place, proxy != null) { proxySheet = true } },
                // Beside the toggle, once a proxy is saved: its details, applied only once they pass the check.
                if (proxy != null) @Composable { place: Place ->
                    InfoRow(
                        place,
                        stringResource(R.string.settings_edit_proxy),
                        TorrentWords.proxyLine(context, proxy.host, proxy.port.toString(), proxy.user.orEmpty()),
                        onClick = { proxySheet = true },
                    )
                } else null,
                if (torrents && store.cast.available) @Composable { place: Place ->
                    SettingSwitch(place, SettingKey.TORRENTS_WHILE_CASTING, stringResource(R.string.cast_settings_torrents), note = stringResource(R.string.cast_settings_torrents_note))
                } else null,
                if (torrents && proxy != null && check != null && check != ProxyStatus.DIRECT) @Composable { place: Place -> LastCheckRow(place) } else null,
                // Only where the device has a cellular radio to spare.
                if (torrents && cellular) @Composable { place: Place -> SettingSwitch(place, SettingKey.TORRENTS_ON_CELLULAR, stringResource(R.string.settings_torrents_cellular)) } else null,
            ),
        )
        if (torrents) {
            SettingsGroup(
                title = stringResource(R.string.downloads_seeding_title),
                footer = stringResource(R.string.settings_seeding_best_effort),
                rows = listOf(
                    { place -> SettingMenu(place, SettingKey.SEED_RATIO, stringResource(R.string.settings_seed_after)) },
                    { place -> SettingMenu(place, SettingKey.SEED_HOURS, stringResource(R.string.settings_seed_for)) },
                    { place -> SettingMenu(place, SettingKey.UPLOAD_LIMIT, stringResource(R.string.settings_upload_limit)) },
                ),
            )
        }
    }
    if (proxySheet) TorrentSheet { proxySheet = false }
}

/**
 * The torrent source's switch: on and off at once. Under it, off says so and on says torrents
 * connect directly; with a proxy set, nothing, as the proxy's own rows below say where traffic goes.
 */
@Composable
private fun TorrentSourceRow(place: Place, on: Boolean, proxy: ProxyView?) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val note = when {
        !on -> stringResource(TorrentWords.OFF_NOTE)
        proxy == null -> TorrentWords.summary(context, null, null)
        else -> null
    }
    SwitchRow(place, stringResource(R.string.sources_torrent_source), checked = on, note = note) { wanted ->
        store.setSwitch(SettingKey.TORRENTS, wanted)
    }
}

/** Route torrent traffic through a proxy: on opens the proxy's form, off removes the proxy at once. */
@Composable
private fun ProxyRouteRow(place: Place, on: Boolean, ask: () -> Unit) {
    val store = LocalAppStore.current
    SwitchRow(place, stringResource(R.string.torrents_proxy_route), checked = on, note = stringResource(R.string.torrents_proxy_route_detail)) { wanted ->
        if (wanted) ask() else store.setTorrentProxy(null)
    }
}

/** The saved proxy's check, with its dot and Check now. */
@Composable
private fun LastCheckRow(place: Place) {
    val store = LocalAppStore.current
    val check by store.providers.check.collectAsStateWithLifecycle()
    val words = TorrentWords.check(LocalContext.current, check)
    val colors = MaterialTheme.colorScheme
    RowFrame(place, onClick = null, modifier = Modifier.heightIn(min = 88.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            Column(Modifier.weight(1f)) {
                Text(stringResource(R.string.torrents_last_check), style = MaterialTheme.typography.titleMedium)
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                    StateDot(
                        when {
                            words.problem -> colors.error
                            words.passed -> colors.secondary
                            else -> null
                        },
                    )
                    Text(words.status, style = MaterialTheme.typography.bodyMedium.tabular(), color = colors.onSurfaceVariant)
                }
                words.note?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant) }
            }
            TextButton(onClick = store::checkProxy, enabled = check != ProxyStatus.CHECKING) {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                    WorkRing(check == ProxyStatus.CHECKING)
                    Text(stringResource(R.string.torrents_check_now), style = MaterialTheme.typography.titleMedium, color = colors.onSurface)
                }
            }
        }
    }
}

// Accounts

@Composable
internal fun AccountsPane(actions: AppActions) {
    val store = LocalAppStore.current
    val sheets = LocalAppSheets.current
    val trackers by store.accounts.trackers.collectAsStateWithLifecycle()
    val syncedAt by store.accounts.syncedAt.collectAsStateWithLifecycle()
    val working by store.accounts.working.collectAsStateWithLifecycle()
    val conflicts by store.accounts.conflicts.collectAsStateWithLifecycle()
    var reviewing by remember { mutableStateOf(false) }
    val context = LocalContext.current
    val anilist = trackers["anilist"]
    val storage = trackers.values.firstNotNullOfOrNull { it.storage }
    val answer = kaeriIsFor()
    /** The tracker's own sign-in: Kitsu's or MangaUpdates' sheet, the browser or a code for the rest. */
    val signIn = { tracker: String ->
        when (tracker) {
            "kitsu" -> sheets.kitsu = true
            "mangaupdates" -> sheets.mangaUpdates = true
            else -> store.connect(tracker)
        }
    }
    Pane {
        SettingsGroup(
            title = "AniList", // i18n-skip
            footer = stringResource(R.string.settings_anilist_footer),
            rows = if (anilist?.connected == true) {
                val status = AccountWords.status(anilist, syncedAt, working)
                listOfNotNull(
                    @Composable { place: Place ->
                        InfoRow(
                            place,
                            anilist.user ?: stringResource(R.string.accounts_signed_in_plain),
                            status.text,
                            leading = { Avatar(anilist.user, 40) },
                            trailing = if (anilist.needsReauth) ({ SignInButton(stringResource(R.string.accounts_sign_in_again_button), busy = store.accounts.isWorking(working, "anilist")) { signIn("anilist") } }) else null,
                        )
                    },
                    @Composable { place: Place -> ActionRow(place, stringResource(R.string.accounts_sync_now), enabled = "sync" !in working.values, onClick = store::syncNow) },
                    if (conflicts.isNotEmpty()) @Composable { place: Place ->
                        ActionRow(place, pluralStringResource(R.plurals.accounts_review_differences, conflicts.size, conflicts.size)) { reviewing = true }
                    } else null,
                    @Composable { place: Place -> ActionRow(place, stringResource(R.string.accounts_sign_out)) { store.disconnect("anilist") } },
                )
            } else {
                listOf(@Composable { place: Place ->
                    InfoRow(
                        place,
                        "AniList", // i18n-skip
                        app.kaeri.ui.reading.anilistBrings(answer),
                        trailing = { SignInButton(stringResource(R.string.accounts_sign_in), busy = store.accounts.isWorking(working, "anilist")) { signIn("anilist") } },
                    )
                })
            },
        )
        // Discussions are about episodes: with reading alone they are gone.
        if (answer.watches) SettingsGroup(
            title = stringResource(R.string.discussion_settings_group),
            rows = listOf(
                { place -> SettingSwitch(place, SettingKey.EPISODE_DISCUSSIONS, stringResource(R.string.settings_discussions_label), note = stringResource(R.string.settings_discussions_note)) },
                { place -> SettingSwitch(place, SettingKey.COMMENT_IMAGES, stringResource(R.string.settings_discussion_images_label), note = stringResource(R.string.settings_discussion_images_note)) },
            ),
        )
        SettingsGroup(
            title = stringResource(R.string.accounts_other_trackers),
            // Where tokens are kept, and with reading on, MangaUpdates' credit.
            footer = listOfNotNull(
                stringResource(if (storage?.contains("file", ignoreCase = true) == true) R.string.accounts_tokens_file else R.string.accounts_tokens_keystore),
                if (answer.reads) stringResource(R.string.accounts_mangaupdates_credit) else null,
            ).joinToString(" "), // i18n-skip
            rows = app.kaeri.ui.reading.otherTrackers(answer).map { tracker ->
                @Composable { place: Place ->
                    val state = trackers[tracker]
                    val connected = state?.connected == true
                    val busy = store.accounts.isWorking(working, tracker)
                    val button: @Composable () -> Unit = when {
                        connected && state?.needsReauth == true -> ({ SignInButton(stringResource(R.string.accounts_sign_in_again_button), busy) { signIn(tracker) } })
                        connected -> ({ TextButton(onClick = { store.disconnect(tracker) }) { Text(stringResource(R.string.accounts_sign_out), color = MaterialTheme.colorScheme.onSurface) } })
                        else -> ({ SignInButton(stringResource(R.string.accounts_sign_in), busy) { signIn(tracker) } })
                    }
                    InfoRow(
                        place,
                        app.kaeri.ui.reading.trackerName(tracker),
                        when {
                            connected -> AccountWords.status(state, syncedAt, working).text
                            // With reading on, what it serves or keeps in step.
                            answer.reads -> app.kaeri.ui.reading.trackerLine(tracker, answer)
                            else -> stringResource(R.string.accounts_state_signed_out)
                        },
                        trailing = button,
                    )
                }
            },
        )
        if (answer.reads) app.kaeri.ui.reading.ProgressGroup()
    }
    if (reviewing) ConflictsSheet { reviewing = false }
}

/** Sign in, tonal: a row's own action, with the screen's one amber action kept for elsewhere. */
@Composable
private fun SignInButton(label: String, busy: Boolean, onClick: () -> Unit) {
    FilledTonalButton(onClick = onClick, enabled = !busy) { WorkLabel(label, busy) }
}

/** A row that is one action in silk, such as Sync now or Sign out. */
@Composable
internal fun ActionRow(place: Place, title: String, enabled: Boolean = true, note: String? = null, destructive: Boolean = false, onClick: () -> Unit) {
    RowFrame(place, onClick = if (enabled) onClick else null) {
        Column {
            Text(
                title,
                style = MaterialTheme.typography.titleMedium,
                color = when {
                    !enabled -> MaterialTheme.colorScheme.onSurfaceVariant
                    destructive -> MaterialTheme.colorScheme.error
                    else -> MaterialTheme.colorScheme.onSurface
                },
            )
            note?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant) }
        }
    }
}

// About

/** The privacy page: what Kaeri sends, where, and what it keeps on the device. */
internal const val PRIVACY_URL = "https://kaeri.app/docs/privacy/"

/** The legal and contact page, which About opens. */
internal const val LEGAL_URL = "https://kaeri.app/legal/"

/**
 * About: Nagare, the name and the version; Updates, with the newer release's row while there is
 * one and the daily check's switch; then Legal and contact, Privacy and the licences, over the
 * legal line.
 */
@Composable
internal fun AboutPane() {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val update by store.updates.state.collectAsStateWithLifecycle()
    val runningBeta by store.updates.runningBeta.collectAsStateWithLifecycle()
    Pane {
        Column(Modifier.fillMaxWidth().padding(vertical = 16.dp), horizontalAlignment = Alignment.CenterHorizontally) {
            Nagare(cell = 6)
            Text("Kaeri", // i18n-skip
                style = MaterialTheme.typography.titleLarge, modifier = Modifier.padding(top = 16.dp))
            val version = if (runningBeta) stringResource(R.string.updates_version_beta, appVersion()) else appVersion()
            Text(stringResource(R.string.about_version, version), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        SettingsGroup(
            title = stringResource(R.string.updates_title),
            rows = listOfNotNull(
                if (update.offer != null) @Composable { place: Place -> UpdateRow(place) } else null,
                { place -> CheckNowRow(place) },
                { place -> SettingSwitch(place, SettingKey.UPDATE_BETA, stringResource(R.string.updates_beta), note = stringResource(R.string.updates_beta_detail)) },
                { place -> SettingSwitch(place, SettingKey.UPDATE_CHECK, stringResource(R.string.settings_check_updates), note = stringResource(R.string.updates_schedule)) },
            ),
        )
        // Report a bug closes the Crash reports section, as on the board.
        CrashReportsGroup()
        SettingsGroup(
            footer = stringResource(R.string.about_report_bug_detail),
            rows = listOf(
                { place ->
                    InfoRow(
                        place,
                        stringResource(R.string.about_report_bug),
                        stringResource(R.string.settings_report_bug_value),
                        onClick = { openBugReport(context, store) },
                        trailing = { RowMark(Icons.External) },
                    )
                },
            ),
        )
        SettingsGroup(
            footer = stringResource(R.string.legal_hosts_no_content),
            rows = listOf(
                { place ->
                    InfoRow(
                        place,
                        stringResource(R.string.about_legal_link),
                        null,
                        onClick = { store.openExternal(LEGAL_URL) },
                        trailing = { RowMark(Icons.External) },
                    )
                },
                { place ->
                    InfoRow(
                        place,
                        stringResource(R.string.settings_privacy),
                        stringResource(R.string.settings_privacy_detail),
                        onClick = { store.openExternal(PRIVACY_URL) },
                        trailing = { RowMark(Icons.External) },
                    )
                },
                { place -> InfoRow(place, stringResource(R.string.settings_licence_title), stringResource(R.string.settings_licence_value)) },
                { place -> InfoRow(place, stringResource(R.string.settings_typefaces), stringResource(R.string.settings_typefaces_value)) },
                { place -> InfoRow(place, stringResource(R.string.settings_libraries), stringResource(R.string.settings_libraries_value)) },
            ),
        )
    }
}
