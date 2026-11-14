// Reading's settings: the Reading pane (reader defaults per format, when a chapter counts as read,
// which series settings follow you), Downloads' reading rows, Notifications' chapter rows, General's
// Open at, and Accounts' Progress group with the trackers that serve reading. Every row changes the
// engine's setting it shows. The OPDS feed is the desktop's alone, and e-readers wait for a default
// the engine keeps.

package app.kaeri.ui.reading

import android.content.Context
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.engine.model.ReadingCommand
import app.kaeri.engine.model.ReaderSettingKey
import app.kaeri.engine.model.ReaderSettingsView
import app.kaeri.engine.model.ReaderValue
import app.kaeri.engine.model.SettingKey
import app.kaeri.engine.model.SettingsView
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.store.AppStore
import app.kaeri.store.SettingsCatalog
import app.kaeri.ui.LocalAppStore
import app.kaeri.ui.MenuChoice
import app.kaeri.ui.MenuRow
import app.kaeri.ui.Pane
import app.kaeri.ui.Place
import app.kaeri.ui.SettingSwitch
import app.kaeri.ui.SettingsGroup
import app.kaeri.ui.SwitchRow
import app.kaeri.ui.kindWord
import app.kaeri.ui.sentence

// Words

/** A token setting's choices as the rows say them, by the token the engine keeps. */
@Composable
private fun tokenWords(key: SettingKey): Map<String, String> = when (key) {
    SettingKey.WEBTOON_MODE -> mapOf(
        "strip" to stringResource(R.string.settings_reader_strip),
        "paged" to stringResource(R.string.settings_reader_pages),
    )
    SettingKey.COUNTS_AS_READ -> mapOf(
        "last-page" to stringResource(R.string.settings_counts_read_value),
        "moving-on" to stringResource(R.string.settings_counts_read_moving_on),
        "marked" to stringResource(R.string.settings_counts_read_marked),
    )
    SettingKey.NEW_CHAPTERS -> mapOf(
        "off" to stringResource(R.string.value_off),
        "reading" to stringResource(R.string.settings_new_chapters_reading),
    )
    SettingKey.SEND_PROGRESS_AFTER -> mapOf(
        "chapter" to stringResource(R.string.settings_progress_chapter),
        "close" to stringResource(R.string.settings_progress_close),
    )
    SettingKey.WHEN_TRACKERS_DISAGREE -> mapOf(
        "furthest" to stringResource(R.string.settings_disagree_furthest),
        "latest" to stringResource(R.string.settings_disagree_latest),
    )
    SettingKey.OPEN_AT -> mapOf(
        "watch" to stringResource(R.string.nav_watch),
        "read" to stringResource(R.string.nav_read),
    )
    else -> emptyMap()
}

/** The token a setting stands on, as the engine says it. */
internal fun settingToken(store: AppStore, key: SettingKey, view: SettingsView?): String? =
    SettingsCatalog.current(store.settings.row(key, view))?.label ?: store.settings.row(key, view)?.value

/**
 * A setting whose engine value is a token ("last-page", "reading"), chosen from a menu in the
 * person's words. [noteFor] gives the line under it for the choice it stands on, where the line
 * belongs to one choice only.
 */
@Composable
internal fun ReadingSettingMenu(
    place: Place,
    key: SettingKey,
    title: String,
    note: String? = null,
    noteFor: (@Composable (String?) -> String?)? = null,
    focus: String? = null,
) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val row = store.settings.row(key, view)
    val words = tokenWords(key)
    val current = SettingsCatalog.current(row)
    val token = current?.label ?: row?.value
    val shownNote = noteFor?.invoke(token) ?: note
    MenuRow(
        place = place,
        title = title,
        value = token?.let { words[it] ?: sentence(it) },
        choices = SettingsCatalog.options(key).map { MenuChoice(it.value, words[it.label] ?: sentence(it.label)) },
        current = current?.value,
        note = listOfNotNull(store.settings.overridden(context, key, view), shownNote).joinToString("\n").ifEmpty { null }, // i18n-skip
        enabled = SettingsCatalog.editable(row),
        focus = focus,
    ) { store.setSetting(key, it) }
}

// Reader defaults

/** Manga's default as one choice of mode and direction: pages right to left, left to right, or a long strip. */
internal enum class MangaDefault { RTL, LTR, STRIP }

/** Light novels' default: pages, or scrolling. */
internal enum class NovelDefault { PAGED, SCROLLED }

private fun ReaderSettingsView.choice(key: ReaderSettingKey): String? =
    (rows.firstOrNull { it.key == key }?.value as? ReaderValue.Choice)?.name

internal fun mangaDefault(view: ReaderSettingsView?): MangaDefault? {
    view ?: return null
    if (view.choice(ReaderSettingKey.MODE) == "strip") return MangaDefault.STRIP // i18n-skip
    return if (view.choice(ReaderSettingKey.DIRECTION) == "ltr") MangaDefault.LTR else MangaDefault.RTL // i18n-skip
}

internal fun novelDefault(view: ReaderSettingsView?): NovelDefault? {
    view ?: return null
    return if (view.choice(ReaderSettingKey.MODE) == "scrolled") NovelDefault.SCROLLED else NovelDefault.PAGED // i18n-skip
}

private fun setManga(store: AppStore, value: MangaDefault) {
    when (value) {
        MangaDefault.STRIP -> store.reading.setReaderDefault(Medium.MANGA, ReaderSettingKey.MODE, ReaderValue.Choice("strip")) // i18n-skip
        MangaDefault.RTL, MangaDefault.LTR -> {
            store.reading.setReaderDefault(Medium.MANGA, ReaderSettingKey.MODE, ReaderValue.Choice("paged")) // i18n-skip
            store.reading.setReaderDefault(Medium.MANGA, ReaderSettingKey.DIRECTION, ReaderValue.Choice(if (value == MangaDefault.LTR) "ltr" else "rtl")) // i18n-skip
        }
    }
}

internal fun mangaWords(context: Context, value: MangaDefault): String = when (value) {
    MangaDefault.RTL -> Strings.settingsReaderRtl(context)
    MangaDefault.LTR -> Strings.settingsReaderLtr(context)
    MangaDefault.STRIP -> Strings.settingsReaderStrip(context)
}

private fun novelWords(context: Context, value: NovelDefault): String = when (value) {
    NovelDefault.PAGED -> Strings.settingsReaderPages(context)
    NovelDefault.SCROLLED -> Strings.settingsReaderScrolled(context)
}

@Composable
private fun MangaDefaultRow(place: Place) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val defaults by store.reading.readerDefaults.collectAsStateWithLifecycle()
    val view = defaults[Medium.MANGA]
    val current = mangaDefault(view)
    MenuRow(
        place = place,
        title = stringResource(R.string.settings_reader_manga),
        value = current?.let { mangaWords(context, it) },
        choices = MangaDefault.entries.map { MenuChoice(it, mangaWords(context, it)) },
        current = current,
        enabled = view != null,
    ) { setManga(store, it) }
}

@Composable
private fun NovelDefaultRow(place: Place) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val defaults by store.reading.readerDefaults.collectAsStateWithLifecycle()
    val view = defaults[Medium.NOVEL]
    val current = novelDefault(view)
    MenuRow(
        place = place,
        title = stringResource(R.string.settings_reader_novels),
        value = current?.let { novelWords(context, it) },
        choices = NovelDefault.entries.map { MenuChoice(it, novelWords(context, it)) },
        current = current,
        enabled = view != null,
    ) { store.reading.setReaderDefault(Medium.NOVEL, ReaderSettingKey.MODE, ReaderValue.Choice(if (it == NovelDefault.SCROLLED) "scrolled" else "paged")) } // i18n-skip
}

/** Counts as read, with the line that belongs to the choice it stands on. */
@Composable
private fun CountsAsReadRow(place: Place) {
    ReadingSettingMenu(
        place,
        SettingKey.COUNTS_AS_READ,
        stringResource(R.string.settings_counts_read),
        noteFor = { token ->
            when (token) {
                "moving-on" -> stringResource(R.string.settings_counts_read_moving_on_note)
                "marked" -> stringResource(R.string.settings_counts_read_marked_note)
                else -> stringResource(R.string.settings_counts_read_note)
            }
        },
    )
}

/** Skip credit pages, on unless turned off: a reader default for manga, which comics and strips share. */
@Composable
private fun SkipCreditsRow(place: Place) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val defaults by store.reading.readerDefaults.collectAsStateWithLifecycle()
    val view = defaults[Medium.MANGA]
    val on = (view?.rows?.firstOrNull { it.key == ReaderSettingKey.SKIP_CREDIT_PAGES }?.value as? ReaderValue.Toggle)?.on ?: true
    SwitchRow(
        place,
        Strings.settingsSkipCredits(context),
        checked = on,
        note = Strings.settingsSkipCreditsNote(context),
        enabled = view != null,
    ) { store.reading.setReaderDefault(Medium.MANGA, ReaderSettingKey.SKIP_CREDIT_PAGES, ReaderValue.Toggle(it)) }
}

// The pane

/**
 * Reading: the reader defaults per format and when a chapter counts as read, then the series
 * settings that follow the person to their other devices, and what stays on this one.
 */
@Composable
internal fun ReadingPane() {
    val store = LocalAppStore.current
    LaunchedEffect(Unit) { store.reading.loadReaderDefaults() }
    Pane {
        SettingsGroup(
            title = stringResource(R.string.settings_reader_defaults),
            footer = stringResource(R.string.settings_reader_defaults_footer),
            rows = listOf(
                { place -> MangaDefaultRow(place) },
                { place -> ReadingSettingMenu(place, SettingKey.WEBTOON_MODE, stringResource(R.string.settings_reader_webtoons)) },
                { place -> NovelDefaultRow(place) },
                { place -> CountsAsReadRow(place) },
                { place -> SkipCreditsRow(place) },
            ),
        )
        ScreenGroup()
        SettingsGroup(
            title = stringResource(R.string.settings_follow_group),
            footer = Strings.settingsFollowFooter(LocalContext.current, kindWord()),
            rows = listOf(
                { place -> SettingSwitch(place, SettingKey.SERIES_CHOICES_FOLLOW, stringResource(R.string.settings_mode_follows)) },
                { place -> SettingSwitch(place, SettingKey.LOOK_FOLLOWS, stringResource(R.string.settings_look_follows), note = stringResource(R.string.settings_look_follows_note)) },
            ),
        )
    }
}

/**
 * Screen: the e-ink layout on or off, and with it on whether the panel shows colour. The engine
 * keeps each answer, as the e-ink questions do, so a wrong one is undone here.
 */
@Composable
private fun ScreenGroup() {
    val store = LocalAppStore.current
    val look = LocalEinkLayout.current
    fun answer(eink: Boolean, colour: Boolean?) = store.send(Command.Reading(ReadingCommand.AnswerEink(eink, colour)))
    SettingsGroup(
        title = stringResource(R.string.settings_screen_group),
        rows = listOfNotNull<@Composable (Place) -> Unit>(
            { place ->
                SwitchRow(place, stringResource(R.string.settings_eink_layout), checked = look != null, note = stringResource(R.string.settings_eink_layout_note)) {
                    // Turned on, the colour question is asked again in the reader.
                    answer(it, null)
                }
            },
            look?.let {
                { place ->
                    SwitchRow(place, stringResource(R.string.settings_eink_colour), checked = look.colour, note = stringResource(R.string.settings_eink_colour_note)) {
                        answer(true, it)
                    }
                }
            },
        ),
    )
}

/** What the Settings list says under Reading: "Pages, right to left · After the last page". */
@Composable
internal fun readingSummary(): String? {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val defaults by store.reading.readerDefaults.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { store.reading.loadReaderDefaults() }
    val manga = mangaDefault(defaults[Medium.MANGA])?.let { mangaWords(context, it) }
    val counts = settingToken(store, SettingKey.COUNTS_AS_READ, view)?.let { tokenWords(SettingKey.COUNTS_AS_READ)[it] }
    return listOfNotNull(manga, counts).joinToString(" · ").ifEmpty { null } // i18n-skip
}

// Downloads

/** Downloads' reading rows: chapters over mobile data, which new chapters download, deleting after reading, and showing volumes. */
internal fun readingDownloadRows(): List<@Composable (Place) -> Unit> = listOf(
    { place -> SettingSwitch(place, SettingKey.CHAPTER_DOWNLOADS_ON_CELLULAR, stringResource(R.string.settings_chapter_downloads_cellular)) },
    { place ->
        ReadingSettingMenu(
            place,
            SettingKey.NEW_CHAPTERS,
            stringResource(R.string.settings_new_chapters_download),
            // The line belongs to Series on Reading alone.
            noteFor = { token -> if (token == "reading") stringResource(R.string.settings_new_chapters_note) else null },
        )
    },
    { place -> SettingSwitch(place, SettingKey.DELETE_AFTER_READING, stringResource(R.string.settings_delete_after_reading), note = stringResource(R.string.settings_delete_after_reading_note)) },
    { place -> SettingSwitch(place, SettingKey.SHOW_VOLUMES, stringResource(R.string.settings_show_volumes), note = stringResource(R.string.settings_show_volumes_note)) },
)

// Notifications

/** Notifications' New chapters row: series on the Reading list. */
internal val chapterNoticesRow: @Composable (Place) -> Unit = { place ->
    SettingSwitch(place, SettingKey.CHAPTER_NOTIFICATIONS, stringResource(R.string.settings_chapter_notifications), note = stringResource(R.string.settings_chapter_notifications_note))
}

/** Notifications' Downloads finished row. */
internal val downloadsFinishedRow: @Composable (Place) -> Unit = { place ->
    SettingSwitch(place, SettingKey.DOWNLOADS_FINISHED, stringResource(R.string.settings_downloads_finished), note = stringResource(R.string.settings_downloads_finished_note))
}

/** "New chapters on", or off, for the Settings list. */
internal fun chapterNoticesSummary(context: Context, store: AppStore, view: SettingsView?): String? {
    store.settings.row(SettingKey.CHAPTER_NOTIFICATIONS, view) ?: return null
    return if (store.settings.isOn(SettingKey.CHAPTER_NOTIFICATIONS, view)) Strings.settingsSummaryChaptersOn(context) else Strings.settingsSummaryChaptersOff(context)
}

// General

/** Open at, with both media on: which Home the app starts on. */
internal val openAtRow: @Composable (Place) -> Unit = { place ->
    ReadingSettingMenu(place, SettingKey.OPEN_AT, stringResource(R.string.settings_open_at), note = stringResource(R.string.settings_open_at_note))
}

// Accounts

/** The trackers after AniList that serve what Kaeri is for: MangaUpdates with reading on, Simkl and Trakt with watching on. */
internal fun otherTrackers(answer: KaeriIsFor): List<String> = when (answer) {
    KaeriIsFor.WATCH -> listOf("mal", "kitsu", "simkl", "trakt")
    KaeriIsFor.READ -> listOf("mal", "kitsu", "mangaupdates")
    KaeriIsFor.BOTH -> listOf("mal", "kitsu", "mangaupdates", "simkl", "trakt")
}

/** A tracker's name, MangaUpdates included. */
internal fun trackerName(tracker: String): String =
    if (tracker == "mangaupdates") "MangaUpdates" else app.kaeri.store.AccountsModel.name(tracker) // i18n-skip

/**
 * What a tracker signed out says: with both media on, which medium it serves; otherwise what it
 * keeps in step, MangaUpdates only reading lists.
 */
@Composable
internal fun trackerLine(tracker: String, answer: KaeriIsFor): String = when {
    answer == KaeriIsFor.BOTH -> when (tracker) {
        "mangaupdates" -> stringResource(R.string.accounts_tracker_serves_reading)
        "simkl", "trakt" -> stringResource(R.string.accounts_tracker_serves_shows)
        else -> stringResource(R.string.accounts_tracker_serves_both)
    }
    tracker == "mangaupdates" -> stringResource(R.string.accounts_tracker_reading_lists)
    else -> stringResource(R.string.accounts_tracker_lists)
}

/** Accounts' Progress group, with reading on: when progress is sent, and which change wins. */
@Composable
internal fun ProgressGroup() {
    SettingsGroup(
        title = stringResource(R.string.settings_progress_group),
        rows = listOf(
            { place -> ReadingSettingMenu(place, SettingKey.SEND_PROGRESS_AFTER, stringResource(R.string.settings_send_progress_after)) },
            { place -> ReadingSettingMenu(place, SettingKey.WHEN_TRACKERS_DISAGREE, stringResource(R.string.settings_when_trackers_disagree), note = stringResource(R.string.settings_disagree_note)) },
        ),
    )
}

/** The sign-in line for AniList while signed out, for what Kaeri is for. */
@Composable
internal fun anilistBrings(answer: KaeriIsFor): String = stringResource(
    when (answer) {
        KaeriIsFor.WATCH -> R.string.accounts_brings_lists
        KaeriIsFor.READ -> R.string.accounts_brings_lists_read
        KaeriIsFor.BOTH -> R.string.accounts_brings_lists_both
    },
)
