// Settings in two panes, as Android TV's own: sections on the left, the section's rows on the
// right. Moving through the sections shows their rows; Right goes into them.

package app.kaeri.tv.screens

import app.kaeri.store.AppLanguage
import app.kaeri.i18n.Language
import android.os.Build
import android.content.ContextWrapper
import android.app.Activity
import app.kaeri.store.percentText
import app.kaeri.store.hasNoSources
import app.kaeri.store.sourceGroups
import androidx.compose.ui.res.stringResource
import app.kaeri.i18n.Strings
import app.kaeri.i18n.R
import androidx.annotation.StringRes
import android.content.Context
import android.provider.Settings as SystemSettings
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.rememberLazyListState
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
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import kotlinx.coroutines.launch
import app.kaeri.design.Nagare
import app.kaeri.design.ObiSilk
import app.kaeri.engine.model.HoldReason
import app.kaeri.engine.model.ProviderOrigin
import app.kaeri.store.TorrentWords
import app.kaeri.engine.model.ProviderRow
import app.kaeri.engine.model.StoreRow
import app.kaeri.engine.model.StoreState
import app.kaeri.store.ProviderStanding
import app.kaeri.store.standing
import app.kaeri.engine.ffi.FfiPairedPhone
import app.kaeri.engine.model.SettingChange
import app.kaeri.engine.model.ProxyStatus
import app.kaeri.engine.model.SettingKey
import app.kaeri.engine.model.SettingRow
import app.kaeri.engine.model.SettingsView
import app.kaeri.engine.model.SyncState
import app.kaeri.store.AccountsModel
import app.kaeri.store.AppClock
import app.kaeri.store.AppStore
import app.kaeri.store.SettingsCatalog
import app.kaeri.store.UpdateCheck
import app.kaeri.store.UpdateState
import app.kaeri.store.Wording
import app.kaeri.engine.core.SettingValue
import app.kaeri.engine.model.TitleLanguage
import app.kaeri.store.plainReason
import app.kaeri.tv.home.LocalWatchNext
import app.kaeri.tv.remote.ConfirmDialog
import app.kaeri.tv.remote.LocalRemoteHost
import app.kaeri.tv.remote.QrCode
import app.kaeri.tv.remote.RemovePhoneDialog
import app.kaeri.tv.shell.TvNavigator
import app.kaeri.tv.ui.ActionSheet
import app.kaeri.tv.ui.Canvas
import app.kaeri.tv.ui.CardAction
import app.kaeri.tv.ui.LocalActions
import app.kaeri.tv.ui.focusSoon
import app.kaeri.tv.ui.languageName
import app.kaeri.tv.ui.EnterFocus
import app.kaeri.tv.ui.FocusMemory
import app.kaeri.tv.ui.LocalAppStore
import app.kaeri.tv.ui.SafeSpec
import app.kaeri.tv.sync.devices
import app.kaeri.tv.sync.profiles
import app.kaeri.tv.ui.ScrollRule
import app.kaeri.tv.ui.TvIcons
import app.kaeri.tv.ui.TvListRow
import app.kaeri.tv.ui.firstFocus
import app.kaeri.tv.ui.remembered

/** The sections, top to bottom. */
enum class SettingsSection(@StringRes val label: Int) {
    General(R.string.settings_group_general),
    Playback(R.string.settings_group_playback),
    Sources(R.string.sources_title),
    PluginStores(R.string.nav_plugin_stores),
    Accounts(R.string.accounts_title),
    Devices(R.string.devices_title),
    Profiles(R.string.profiles_title),
    Remotes(R.string.tv_remotes),
    ThisTv(R.string.tv_section_this_tv),
    About(R.string.about_title),
}

/** Playback's rows: the phone's, less what does not apply on a TV (upscaling, fullscreen). */
val PlaybackRows: List<Pair<SettingKey, Int>> = listOf(
    SettingKey.TRANSLATION to R.string.settings_audio,
    SettingKey.SUBTITLES to R.string.player_subtitles,
    SettingKey.QUALITY to R.string.settings_quality,
    SettingKey.AUTO_NEXT to R.string.settings_auto_play,
    SettingKey.SKIP_OPENING to R.string.settings_skip_opening,
    SettingKey.SKIP_RECAP to R.string.settings_skip_recap,
    SettingKey.SKIP_FILLER to R.string.settings_skip_filler,
    SettingKey.MARK_CHAPTERS to R.string.settings_mark_opening,
    SettingKey.FIND_SKIP_TIMES to R.string.settings_find_skips,
    SettingKey.ASK_WAS_OPENING to R.string.settings_ask_was_opening,
    SettingKey.SHARE_SKIP_TIMES to R.string.settings_share_skips,
    SettingKey.COMMIT_THRESHOLD to R.string.settings_counts_watched,
    SettingKey.SPOILER_PROTECTION to R.string.settings_spoiler,
    SettingKey.NIGHT_MODE to R.string.player_night_mode,
)

/**
 * The trackers a TV signs in to, in the phone's order: their engine ids and names. MyAnimeList is
 * left out, as rows that only work elsewhere are: it has no sign-in a TV can finish.
 */
private val Trackers = listOf( // i18n-skip
    "anilist" to "AniList",
    "kitsu" to "Kitsu",
    "simkl" to "Simkl",
    "trakt" to "Trakt",
)

@Composable
fun SettingsScreen(nav: TvNavigator, memory: FocusMemory, start: SettingsSection = SettingsSection.General) {
    val store = LocalAppStore.current
    var section by rememberSaveable { mutableStateOf(start) }
    val view by store.settings.view.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) {
        store.loadStores()
        // What the title language's line says of AniList's choice.
        store.loadTitleLanguage()
    }
    ToTop(nav, memory, scrolled = { false }) {}

    Row(Modifier.fillMaxSize().padding(start = Canvas.Keyline, top = Canvas.SafeY, end = Canvas.SafeX)) {
        Column(Modifier.width(256.dp).fillMaxHeight().focusRestorer(), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(stringResource(R.string.settings_title), style = MaterialTheme.typography.headlineSmall, color = ObiSilk.Silk, modifier = Modifier.padding(start = 16.dp, bottom = 16.dp).semantics { heading() })
            // Ten sections at 40 each end inside the safe area, under the heading.
            SettingsSection.entries.forEach { each ->
                TvListRow(
                    stringResource(each.label),
                    onClick = {},
                    selected = each == section,
                    height = 40.dp,
                    modifier = Modifier
                        .onFocusChanged { if (it.isFocused) section = each }
                        .firstFocus(memory, "section/${each.name}", each == start),
                )
            }
        }
        Spacer(Modifier.width(32.dp))
        val density = LocalDensity.current
        val spec = remember(density) { with(density) { SafeSpec(Canvas.FocusRoom.toPx(), 40.dp.toPx()) } }
        // Coming over from the sections, focus goes back to the row it left in this section, or to
        // the section's first row, never to whichever row happens to sit level with the section.
        val firstRow = remember { FocusRequester() }
        val first = Modifier.focusRequester(firstRow)
        // What the providers answer and where torrent traffic leaves from, as the phone's pane asks
        // on opening: once as Sources opens, not each time its rows scroll back into view.
        if (section == SettingsSection.Sources) LaunchedEffect(Unit) { store.checkProviders() }
        ScrollRule(spec) {
            LazyColumn(
                state = rememberLazyListState(),
                modifier = Modifier.fillMaxHeight().padding(top = 56.dp).focusRestorer(firstRow),
                contentPadding = PaddingValues(bottom = 60.dp),
                verticalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                when (section) {
                    SettingsSection.General -> general(store, view, memory, first)
                    SettingsSection.Playback -> playback(store, view, memory, first)
                    SettingsSection.Sources -> sources(nav, store, view, memory, first)
                    SettingsSection.PluginStores -> pluginStores(nav, memory, first)
                    SettingsSection.Accounts -> accounts(nav, memory, first)
                    SettingsSection.Devices -> devices(memory, first)
                    SettingsSection.Profiles -> profiles(memory, first)
                    SettingsSection.Remotes -> remotes(memory, first)
                    SettingsSection.ThisTv -> thisTv(memory, first)
                    SettingsSection.About -> about(nav, memory, first)
                }
            }
        }
    }
    EnterFocus(memory)
}

/** General: how titles are named, which is about every screen rather than playback. */
private fun LazyListScope.general(store: AppStore, view: SettingsView?, memory: FocusMemory, first: Modifier) {
    item(key = "titles", contentType = "setting") { TitleLanguageRow(store, view, memory, first) }
    item(key = "language", contentType = "setting") { AppLanguageRow(memory) }
    item(key = "hidden", contentType = "setting") { HiddenTitlesRow(store, memory) }
}

/** Hidden titles: the ones marked Not interested, in a sheet where choosing one restores it. */
@Composable
private fun HiddenTitlesRow(store: AppStore, memory: FocusMemory) {
    val actions = LocalActions.current
    val context = LocalContext.current
    val hidden by store.home.hidden.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { store.loadHiddenTitles() }
    TvListRow(
        Strings.settingsHiddenTitles(context),
        onClick = {
            val titles = hidden.orEmpty()
            actions?.open(
                if (titles.isEmpty()) {
                    ActionSheet(Strings.settingsHiddenTitles(context), body = { Text(Strings.settingsHiddenEmpty(context), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze) })
                } else {
                    ActionSheet(
                        Strings.settingsHiddenTitles(context),
                        titles.map { each ->
                            CardAction(each.entry.title, supporting = listOfNotNull(app.kaeri.store.Wording.hiddenLine(context, each), Strings.settingsRestore(context)).joinToString(" · ")) { // i18n-skip
                                store.restoreTitle(each.entry.id)
                                store.loadHiddenTitles()
                            }
                        },
                    )
                },
            )
        },
        supporting = Strings.settingsHiddenTitlesBody(context),
        supportingLines = 2,
        modifier = Modifier.remembered(memory, "row/hidden"),
    )
}

/**
 * The app's language: the system's per-app setting from Android 13, the app's own choice below.
 * Only the languages this build ships are listed, a draft one labelled so.
 */
@Composable
private fun AppLanguageRow(memory: FocusMemory) {
    val actions = LocalActions.current
    val context = LocalContext.current
    var chosen by remember { mutableStateOf(AppLanguage.chosen(context)) }
    val follow = stringResource(R.string.language_follow_system)
    val draft = stringResource(R.string.language_draft_label)
    fun name(language: Language) = if (language.draft) "${language.name} · $draft" else language.name // i18n-skip
    fun pick(language: Language?) {
        chosen = language
        AppLanguage.choose(context, language)
        // From Android 13 the system recreates the activity; below, the app does.
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) {
            generateSequence(context) { (it as? ContextWrapper)?.baseContext }.filterIsInstance<Activity>().firstOrNull()?.recreate()
        }
    }
    TvListRow(
        stringResource(R.string.settings_language),
        onClick = {
            actions?.open(
                ActionSheet(
                    Strings.settingsLanguageApp(context),
                    listOf(CardAction(follow, selected = chosen == null) { pick(null) }) +
                        AppLanguage.shipped.map { each -> CardAction(name(each), selected = chosen == each, flagRes = each.flagRes) { pick(each) } },
                ),
            )
        },
        supporting = if (chosen?.draft == true) stringResource(R.string.language_draft_note) else null,
        value = chosen?.let(::name) ?: follow,
        valueFlagRes = chosen?.flagRes,
        modifier = Modifier.remembered(memory, "row/language"),
    )
}

private fun LazyListScope.playback(store: AppStore, view: SettingsView?, memory: FocusMemory, first: Modifier) {
    playbackRows(store, view, memory, first)
    item(key = "subs/heading", contentType = "heading") { GroupLabel(stringResource(R.string.settings_group_subtitles)) }
    subtitleSettingsRows(store, view?.subtitleStyle)
}

private fun LazyListScope.playbackRows(store: AppStore, view: SettingsView?, memory: FocusMemory, first: Modifier) {
    PlaybackRows.forEachIndexed { index, (key, labelRes) ->
        item(key = key.name, contentType = "setting") {
            val actions = LocalActions.current
            val context = LocalContext.current
            val label = stringResource(labelRes)
            val row = store.settings.row(key, view)
            val switch = SettingsCatalog.isSwitch(key)
            val overridden = store.settings.overridden(context, key, view)
            val options = settingChoices(context, key, row)
            TvListRow(
                label,
                onClick = {
                    // A row with choices opens its radio panel; the choice applies at once.
                    if (!switch && options.isNotEmpty() && actions != null) {
                        actions.open(ActionSheet(label, options.map { (value, words, selected) ->
                            CardAction(words, selected = selected) { store.setSetting(key, value) }
                        }))
                    } else {
                        change(store, key, row?.change, switch, view)
                    }
                },
                // Why finding is paused, where someone would look: a TV has only low storage to say.
                supporting = overridden ?: if (key == SettingKey.FIND_SKIP_TIMES) app.kaeri.store.skipFindPausedLine(context, view?.skipFindPaused) else null,
                value = if (switch) null else settingValue(context, key, row),
                switch = if (switch) store.settings.isOn(key, view) else null,
                enabled = SettingsCatalog.editable(row),
                modifier = (if (index == 0) first else Modifier).remembered(memory, "row/${key.name}"),
            )
        }
    }
    // Removes every skip time marked on this device; the engine answers with a notice of how many.
    item(key = "clear-skips", contentType = "setting") {
        TvListRow(
            stringResource(R.string.settings_clear_skips),
            onClick = { store.send(app.kaeri.engine.model.Command.ClearSkipMarks) },
            modifier = Modifier.remembered(memory, "row/clear-skips"),
        )
    }
}

/** The subtitle languages a panel offers, the phone's: the ones people pick most, then any other the file names. */
private val SubtitleLanguages = listOf("en", "ja", "es", "pt", "fr", "de", "it", "ru", "ar", "id", "ko", "zh")

/** A setting's radio panel: each value, its words, and whether the row stands on it. */
private fun settingChoices(context: Context, key: SettingKey, row: SettingRow?): List<Triple<SettingValue, String, Boolean>> {
    if (key == SettingKey.SUBTITLES) {
        val code = SettingsCatalog.typedText(row) ?: row?.value ?: ""
        return (SubtitleLanguages + listOf(code).filter { it.isNotEmpty() && it !in SubtitleLanguages })
            .map { Triple(SettingValue.Str(it), languageName(it), it == code) }
    }
    val current = SettingsCatalog.current(row)
    return SettingsCatalog.options(key).map { Triple(it.value, SettingsCatalog.words(context, key, it), it == current) }
}

/**
 * A row's value as it says it, in the phone's words: "Subbed" or "Dubbed" for sub or dub, a
 * language by its name, anything else in sentence case: "Off", "1080p".
 */
internal fun settingValue(context: Context, key: SettingKey, row: SettingRow?): String? =
    if (key == SettingKey.SUBTITLES) row?.let { languageName(it.value) } else SettingsCatalog.words(context, row)

/**
 * Title language, General's row, worded as the phone's: AniList's choice, English or Romaji, in a
 * radio panel, with a line under the label saying what the choice does now.
 */
@Composable
private fun TitleLanguageRow(store: AppStore, view: SettingsView?, memory: FocusMemory, first: Modifier) {
    val actions = LocalActions.current
    val trackers by store.accounts.trackers.collectAsStateWithLifecycle()
    val viewer by store.accounts.viewerTitleLanguage.collectAsStateWithLifecycle()
    val signedIn = trackers["anilist"]?.connected == true
    val context = LocalContext.current
    val row = store.settings.row(SettingKey.TITLES, view)
    val choice = TitleChoice.of(SettingsCatalog.current(row)?.value)
    TvListRow(
        stringResource(R.string.settings_title_language),
        onClick = {
            actions?.open(
                ActionSheet(
                    Strings.settingsTitleLanguage(context),
                    TitleChoice.entries.map { each ->
                        CardAction(each.label(context), selected = each == choice) {
                            if (each != choice) store.setSetting(SettingKey.TITLES, SettingValue.Str(each.value))
                        }
                    },
                ),
            )
        },
        supporting = TitleChoice.line(context, choice, signedIn, viewer),
        value = choice.label(context),
        enabled = row != null,
        modifier = first.remembered(memory, "row/titles"),
    )
}

/** The three forms a title can be named in, as the settings file names them. */
enum class TitleChoice(val value: String) {
    AniList("anilist"),
    English("english"),
    Romaji("romaji"),
    ;

    /** The choice as the menu names it. */
    fun label(context: Context): String = when (this) {
        AniList -> Strings.settingsTitleTracker(context, "AniList") // i18n-skip
        English -> Strings.settingsTitleEnglish(context)
        Romaji -> Strings.settingsTitleRomaji(context)
    }

    companion object {
        fun of(value: SettingValue?): TitleChoice = entries.firstOrNull { (value as? SettingValue.Str)?.v1 == it.value } ?: AniList

        /** The line under the label: what the choice does now, the phone's words. */
        fun line(context: Context, choice: TitleChoice, signedIn: Boolean, viewer: TitleLanguage?): String = when (choice) {
            AniList -> when {
                !signedIn -> Strings.settingsTitlesLineSignedOut(context)
                viewer != null -> Strings.settingsTitlesLineAccountLanguage(context, name(context, viewer))
                else -> Strings.settingsTitlesLineFollows(context)
            }
            English -> Strings.settingsTitlesLineEnglish(context)
            Romaji -> Strings.settingsTitlesLineRomaji(context)
        }

        private fun name(context: Context, language: TitleLanguage): String = when (language) {
            TitleLanguage.ROMAJI -> Strings.settingsTitleRomaji(context)
            TitleLanguage.ENGLISH -> Strings.settingsTitleEnglish(context)
            TitleLanguage.NATIVE -> Strings.settingsTitleNative(context)
        }
    }
}

/** A switch flips; a ladder steps forward, and from its end back to its start. */
private fun change(store: AppStore, key: SettingKey, change: SettingChange?, switch: Boolean, view: SettingsView?) {
    if (switch) {
        store.setSwitch(key, !store.settings.isOn(key, view))
        return
    }
    val steps = change as? SettingChange.Steps ?: return
    if (steps.forward) {
        store.stepSetting(key, 1)
    } else {
        val options = SettingsCatalog.options(key).size
        store.stepSetting(key, -maxOf(1, options - 1).toLong())
    }
}

private fun LazyListScope.sources(nav: TvNavigator, store: AppStore, view: SettingsView?, memory: FocusMemory, first: Modifier) {
    item(key = "sources", contentType = "list") {
        val rows by store.providers.rows.collectAsStateWithLifecycle()
        val servers by store.servers.rows.collectAsStateWithLifecycle()
        val licensed by store.licensed.view.collectAsStateWithLifecycle()
        LaunchedEffect(Unit) {
            store.loadServers()
            store.loadLicensed()
        }
        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            // Add a source leads: the amber of an empty list is a media server, the store one step away.
            TvListRow(
                stringResource(R.string.sources_add_server_button),
                onClick = nav::openAddServer,
                modifier = first.remembered(memory, "sources/add"),
            )
            if (rows.hasNoSources) Quiet(stringResource(R.string.sources_empty))
            val demo by store.demo.collectAsStateWithLifecycle()
            val actions = LocalActions.current
            val context = LocalContext.current
            // A section per kind, as the page draws them: servers, then licensed services, then the rest.
            for (group in sourceGroups(rows)) {
                if (group.section.ordinal >= app.kaeri.engine.model.SourceSection.FREE_CATALOGUES.ordinal) continue
                SectionLabel(stringResource(R.string.sources_section_servers).takeIf { group.section == app.kaeri.engine.model.SourceSection.MEDIA_SERVERS } ?: stringResource(R.string.sources_section_media))
                group.rows.forEach { provider ->
                    val server = provider.id.removePrefix("jellyfin-").takeIf { provider.kind == "server" }?.let { id -> servers.firstOrNull { it.id == id } }
                    TvListRow(
                        provider.name.ifEmpty { provider.id },
                        onClick = { if (server != null) nav.openServer(server.id) else actions?.open(providerSheet(context, provider)) },
                        supporting = server?.let { serverLine(it) } ?: providerOrigin(context, provider),
                        value = providerProblem(context, provider),
                        modifier = Modifier.remembered(memory, "provider/${provider.id}"),
                    )
                }
            }
            SectionLabel(stringResource(R.string.sources_section_licensed))
            TvListRow(
                stringResource(R.string.licensed_title),
                onClick = nav::openLicensed,
                supporting = licensedLine(licensed),
                modifier = Modifier.remembered(memory, "sources/licensed"),
            )
            for (group in sourceGroups(rows)) {
                if (group.section.ordinal < app.kaeri.engine.model.SourceSection.FREE_CATALOGUES.ordinal) continue
                SectionLabel(
                    when (group.section) {
                        app.kaeri.engine.model.SourceSection.FREE_CATALOGUES -> stringResource(R.string.sources_section_catalogues)
                        app.kaeri.engine.model.SourceSection.PLUGINS -> stringResource(R.string.sources_section_store, providerOrigin(context, group.rows.first()))
                        else -> stringResource(R.string.sources_section_builtin)
                    },
                )
                // The demo is always last in what is built in, with no switch or order of its own.
                group.rows.sortedBy { it.isDemo }.forEach { provider ->
                    TvListRow(
                        if (provider.isDemo) Strings.sourcesDemoName(context) else provider.name.ifEmpty { provider.id },
                        onClick = { actions?.open(if (provider.isDemo) demoSheet(context, store) else providerSheet(context, provider)) },
                        supporting = when {
                            provider.isDemo -> Strings.sourcesDemoLine(context)
                            provider.torrents -> Strings.sourcesPlaysTorrents(context)
                            else -> providerOrigin(context, provider)
                        },
                        // A word only for a problem.
                        value = providerProblem(context, provider),
                        modifier = Modifier.remembered(memory, "provider/${provider.id}"),
                    )
                }
            }
            if (demo.offer && rows.none { it.isDemo }) {
                TvListRow(stringResource(R.string.sources_demo_add), onClick = { store.addDemo() })
            }
        }
    }
    torrents(nav, store, view, memory)
}

private fun LazyListScope.pluginStores(nav: TvNavigator, memory: FocusMemory, first: Modifier) {
    item(key = "stores", contentType = "list") {
        val store = LocalAppStore.current
        val actions = LocalActions.current
        val context = LocalContext.current
        val stores by store.stores.stores.collectAsStateWithLifecycle()
        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            stores.forEachIndexed { index, row ->
                TvListRow(
                    row.name,
                    onClick = { actions?.open(storeSheet(context, store, row)) },
                    supporting = storeLine(context, row),
                    modifier = (if (index == 0) first else Modifier).remembered(memory, "store/${row.id}"),
                )
            }
            TvListRow(
                stringResource(R.string.stores_add_title),
                onClick = nav::openJoinStore,
                modifier = (if (stores.isEmpty()) first else Modifier).remembered(memory, "store/add"),
            )
        }
    }
}

/**
 * Torrents, the phone's group under its sources: the switch, which turns on at once, then the
 * proxy toggle (Edit proxy and the proxy check while one is set), and Seeding while on.
 */
private fun LazyListScope.torrents(nav: TvNavigator, store: AppStore, view: SettingsView?, memory: FocusMemory) {
    val on = store.settings.isOn(SettingKey.TORRENTS, view)
    item(key = "torrents", contentType = "list") {
        val context = LocalContext.current
        val proxy = view?.torrent?.proxy
        val check by store.providers.check.collectAsStateWithLifecycle()
        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            GroupLabel(stringResource(R.string.sources_torrents))
            TvListRow(
                stringResource(R.string.sources_torrent_source),
                onClick = { store.setSwitch(SettingKey.TORRENTS, !on) },
                // With a proxy set, its own rows below say where traffic goes.
                supporting = when {
                    !on -> stringResource(TorrentWords.OFF_NOTE)
                    proxy == null -> TorrentWords.summary(context, null, null)
                    else -> null
                },
                supportingLines = 2,
                height = 88.dp,
                switch = on,
                modifier = Modifier.remembered(memory, "torrent/source"),
            )
            // Always there, so a proxy can be set before the first torrent starts.
            TvListRow(
                stringResource(R.string.torrents_proxy_route),
                onClick = { if (proxy != null) store.setTorrentProxy(null) else nav.openTorrents() },
                supporting = stringResource(R.string.torrents_proxy_route_detail),
                supportingLines = 2,
                height = 88.dp,
                switch = proxy != null,
                modifier = Modifier.remembered(memory, "torrent/route"),
            )
            // Beside the toggle, once a proxy is saved: its details, applied only once they pass the check.
            if (proxy != null) {
                TvListRow(
                    stringResource(R.string.settings_edit_proxy),
                    onClick = { nav.openTorrents() },
                    supporting = TorrentWords.proxyLine(context, proxy.host, proxy.port.toString(), proxy.user.orEmpty()),
                    modifier = Modifier.remembered(memory, "torrent/proxy"),
                )
                if (on && check != null && check != ProxyStatus.DIRECT) LastCheckRow(store, check, memory)
            }
        }
    }
    if (on) {
        item(key = "seeding", contentType = "list") {
            val actions = LocalActions.current
            val context = LocalContext.current
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                GroupLabel(stringResource(R.string.downloads_seeding_title))
                SeedingRows.forEach { (key, labelRes) ->
                    val label = stringResource(labelRes)
                    val row = store.settings.row(key, view)
                    val options = settingChoices(context, key, row)
                    TvListRow(
                        label,
                        onClick = {
                            actions?.open(ActionSheet(label, options.map { (value, words, selected) ->
                                CardAction(words, selected = selected) { store.setSetting(key, value) }
                            }))
                        },
                        value = settingValue(context, key, row),
                        enabled = SettingsCatalog.editable(row) && options.isNotEmpty(),
                        modifier = Modifier.remembered(memory, "row/${key.name}"),
                    )
                }
                Quiet(stringResource(R.string.tv_seeding_note))
            }
        }
    }
}

/** Seeding's rows, the phone's. */
private val SeedingRows = listOf(
    SettingKey.SEED_RATIO to R.string.settings_seed_after,
    SettingKey.SEED_HOURS to R.string.settings_seed_for,
    SettingKey.UPLOAD_LIMIT to R.string.settings_upload_limit,
)

/**
 * The saved proxy's check: its dot, vermilion for a failed check and water for a passed one, and
 * Center to check now.
 */
@Composable
private fun LastCheckRow(store: AppStore, check: ProxyStatus?, memory: FocusMemory) {
    val context = LocalContext.current
    val words = TorrentWords.check(context, check)
    val checking = check == ProxyStatus.CHECKING
    TvListRow(
        stringResource(R.string.torrents_last_check),
        onClick = { if (!checking) store.checkProxy() },
        supporting = listOfNotNull(words.status, words.note).joinToString(" · "),
        supportingLines = 2,
        height = 88.dp,
        dot = when {
            words.problem -> ObiSilk.Vermilion
            words.passed -> ObiSilk.Water
            else -> null
        },
        value = if (checking) null else stringResource(R.string.action_check_now),
        modifier = Modifier.remembered(memory, "torrent/check"),
    )
}

/** A group's name over its rows, the phone's settings groups. */
@Composable
private fun GroupLabel(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.titleMedium,
        color = ObiSilk.Haze,
        modifier = Modifier.padding(start = 16.dp, top = 24.dp, bottom = 4.dp).semantics { heading() },
    )
}

/** Where a source came from: its store, by hand, or built in. */
private fun providerOrigin(context: Context, row: ProviderRow): String = when (val origin = row.origin) {
    is ProviderOrigin.Store -> origin.name ?: Strings.sourcesOriginLeftStore(context)
    ProviderOrigin.ByHand -> Strings.sourcesOriginByHand(context)
    ProviderOrigin.BuiltIn -> Strings.sourcesOriginBuiltIn(context)
}

/** A source's word, only for a problem: "Not answering", "Held back". */
private fun providerProblem(context: Context, row: ProviderRow): String? = when (row.standing) {
    ProviderStanding.Failing -> Strings.sourcesStatusNotAnswering(context)
    ProviderStanding.HeldBack ->
        if (row.heldBackFor == HoldReason.TORRENTS_OFF) Strings.sourcesStatusTorrentsOff(context) else Strings.sourcesStatusHeldBack(context)
    else -> null
}

/** A source's page as a panel: its version, its hosts and how fast it answers. */
private fun providerSheet(context: Context, row: ProviderRow): ActionSheet = ActionSheet(
    row.name.ifEmpty { row.id },
    body = {
        InfoLines(
            listOfNotNull(
                Strings.tvInfoFrom(context) to providerOrigin(context, row),
                row.version?.let { Strings.sourcesVersion(context) to it },
                row.hosts.takeIf { it.isNotEmpty() }?.let { Strings.sourcesContactsLabel(context) to it.joinToString(", ") }, // i18n-skip
                row.latencyMs?.let { Strings.sourcesResponseTime(context) to Strings.sourcesResponseMs(context, it.toInt()) },
                providerProblem(context, row)?.let { Strings.tvInfoState(context) to it },
            ),
        )
    },
)

/** About the demo: the credit block, the only hosts it reaches, that it never signs in, and Remove demo. */
private fun demoSheet(context: Context, store: AppStore): ActionSheet = ActionSheet(
    Strings.sourcesDemoAbout(context),
    actions = listOf(CardAction(Strings.sourcesDemoRemove(context)) { store.removeDemo() }),
    body = {
        Column(Modifier.padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(app.kaeri.engine.ffi.demoInfo().credit, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Silk)
            Text(Strings.sourcesDemoHosts(context), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
            Text(Strings.sourcesDemoPrivate(context), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
        }
    },
)

/** A store's line: its host, then why it is not answering. */
private fun storeLine(context: Context, row: StoreRow): String = listOfNotNull(
    row.host,
    when (row.state) {
        is StoreState.Offline -> Strings.sourcesStatusNotAnswering(context)
        StoreState.Revoked -> Strings.tvStoreRevoked(context)
        StoreState.Ready -> null
    },
).joinToString(" · ")

/** A store's page as a panel: its plugins with their hosts, its key, and Leave. */
private fun storeSheet(context: Context, store: AppStore, row: StoreRow): ActionSheet = ActionSheet(
    row.name,
    body = {
        InfoLines(
            listOf(Strings.storesAddress(context) to row.host, Strings.storesKeyFingerprint(context) to row.fingerprint) +
                row.plugins.map { plugin -> plugin.name to listOf(Strings.aboutVersion(context, plugin.version), plugin.hosts.joinToString(", ")).filter { it.isNotEmpty() }.joinToString(" · ") }, // i18n-skip
            action = Strings.storesLeave(context),
            onAction = { store.leaveStore(row.id) },
        )
    },
)

private fun LazyListScope.accounts(nav: TvNavigator, memory: FocusMemory, first: Modifier) {
    item(key = "accounts", contentType = "list") {
        val store = LocalAppStore.current
        val trackers by store.accounts.trackers.collectAsStateWithLifecycle()
        var signingOut by remember { mutableStateOf<String?>(null) }
        val context = LocalContext.current
        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Trackers.forEachIndexed { index, (id, name) ->
                TvListRow(
                    name,
                    onClick = {
                        val state = trackers[id]
                        // Expired: Sign in again, rather than offering to sign out of an account that no longer works.
                        if (state?.connected == true && !state.needsReauth) signingOut = id else nav.openSignIn(id)
                    },
                    supporting = accountLine(context, trackers[id]),
                    modifier = (if (index == 0) first else Modifier).remembered(memory, "account/$id"),
                )
            }
            if (trackers.values.any { it.connected && !it.needsReauth }) {
                val syncing = "sync" in store.accounts.working.collectAsStateWithLifecycle().value.values
                TvListRow(
                    stringResource(R.string.accounts_sync_now),
                    onClick = { if (!syncing) store.syncNow() },
                    supporting = if (syncing) stringResource(R.string.accounts_sync_running) else null,
                    modifier = Modifier.remembered(memory, "account/sync"),
                )
            }
        }
        signingOut?.let { id ->
            val name = AccountsModel.name(id)
            ConfirmDialog(
                title = stringResource(R.string.tv_sign_out_title, name),
                body = stringResource(R.string.accounts_sign_out_tv_detail, name),
                action = stringResource(R.string.action_sign_out),
                onConfirm = {
                    store.disconnect(id)
                    signingOut = null
                },
                onCancel = { signingOut = null },
            )
        }
    }
}

/** "Signed in as Mika", or "Not signed in". */
fun accountLine(context: Context, state: SyncState?): String = when {
    state == null || !state.connected -> Strings.tvAccountNotSignedIn(context)
    // A token that no longer works, or a pull that failed, is not "Signed in".
    state.needsReauth -> Strings.accountsStateExpiredShort(context)
    state.pullError != null -> Strings.accountsSyncFailed(context, plainReason(context, state.pullError!!.reason))
    state.user != null -> Strings.tvAccountSignedInAs(context, state.user!!)
    else -> Strings.accountsSignedInPlain(context)
}

private fun LazyListScope.remotes(memory: FocusMemory, first: Modifier) {
    item(key = "remotes", contentType = "list") {
        val remote = LocalRemoteHost.current
        val context = LocalContext.current
        val name = remember {
            remote?.name
                ?: runCatching { SystemSettings.Global.getString(context.contentResolver, SystemSettings.Global.DEVICE_NAME) }.getOrNull()
                ?: Strings.tvSectionThisTv(context)
        }
        var removing by remember { mutableStateOf<FfiPairedPhone?>(null) }
        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            InfoRow(stringResource(R.string.tv_remote_name_label), name)
            TvListRow(
                stringResource(R.string.tv_pair_phone),
                onClick = { remote?.openPairing() },
                enabled = remote != null,
                modifier = first.remembered(memory, "remote/pair"),
            )
            TvListRow(
                stringResource(R.string.tv_remote_findable),
                onClick = { remote?.let { it.allowFinding(!it.findable) } },
                switch = remote?.findable ?: false,
                enabled = remote != null,
                modifier = Modifier.remembered(memory, "remote/find"),
            )
            remote?.phones?.forEach { phone ->
                TvListRow(
                    phone.name,
                    onClick = { removing = phone },
                    supporting = phoneLine(context, phone, System.currentTimeMillis() / 1000),
                    icon = TvIcons.Phone,
                    modifier = Modifier.remembered(memory, "remote/phone/${phone.fingerprint}"),
                )
            }
        }
        removing?.let { phone ->
            RemovePhoneDialog(
                phone.name,
                onRemove = {
                    remote?.remove(phone)
                    removing = null
                    memory.focus("remote/pair")
                },
                onCancel = { removing = null },
            )
        }
    }
}

/** "Used today", "Used 3 days ago", or when it paired if it never connected again. */
fun phoneLine(context: Context, phone: FfiPairedPhone, now: Long): String {
    val days = { at: Long -> ((now - at).coerceAtLeast(0) / 86_400).toInt() }
    val resources = context.resources
    return phone.lastSeen?.let { seen ->
        when (val d = days(seen)) {
            0 -> Strings.tvPhoneUsedToday(context)
            1 -> Strings.tvPhoneUsedYesterday(context)
            else -> resources.getQuantityString(R.plurals.tv_phone_used_days, d, d)
        }
    } ?: when (val d = days(phone.pairedAt)) {
        0 -> Strings.tvPhonePairedToday(context)
        1 -> Strings.tvPhonePairedYesterday(context)
        else -> resources.getQuantityString(R.plurals.tv_phone_paired_days, d, d)
    }
}

private fun LazyListScope.thisTv(memory: FocusMemory, first: Modifier) {
    item(key = "this-tv", contentType = "list") {
        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            val watchNext = LocalWatchNext.current
            TvListRow(
                stringResource(R.string.tv_watch_next_row),
                onClick = { watchNext?.let { it.enable(!it.enabled) } },
                supporting = stringResource(R.string.tv_watch_next_name),
                switch = watchNext?.enabled ?: false,
                enabled = watchNext != null,
                modifier = first.remembered(memory, "tv/watch-next"),
            )
            ImageCacheRow(memory)
        }
    }
}

/** "Clear the image cache", with what it holds now; clearing it shows the new size once done. */
@Composable
private fun ImageCacheRow(memory: FocusMemory) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var bytes by remember { mutableStateOf<Long?>(null) }
    var clearing by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { bytes = store.imageCacheBytes() }
    TvListRow(
        stringResource(R.string.tv_image_cache_clear),
        onClick = {
            if (!clearing) {
                clearing = true
                scope.launch {
                    store.clearImageCache()
                    bytes = store.imageCacheBytes()
                    clearing = false
                }
            }
        },
        value = if (clearing) stringResource(R.string.tv_image_cache_clearing) else bytes?.let { megabytes(context, it) },
        modifier = Modifier.remembered(memory, "tv/cache"),
    )
}

/** "84 MB", to the nearest megabyte. */
fun megabytes(context: Context, bytes: Long): String = Strings.sizeMegabyte(context, ((bytes + (1L shl 19)) shr 20).toString())

private fun LazyListScope.about(nav: TvNavigator, memory: FocusMemory, first: Modifier) {
    item(key = "about", contentType = "list") {
        val context = LocalContext.current
        val version = remember {
            runCatching { context.packageManager.getPackageInfo(context.packageName, 0).versionName }.getOrNull() ?: Strings.valueUnknown(context)
        }
        val actions = LocalActions.current
        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Row(Modifier.padding(horizontal = 16.dp, vertical = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                Nagare(cell = 4)
                Spacer(Modifier.width(16.dp))
                Column {
                    Text("Kaeri", style = MaterialTheme.typography.titleMedium, color = ObiSilk.Silk) // i18n-skip
                    val runningBeta by LocalAppStore.current.updates.runningBeta.collectAsStateWithLifecycle()
                    Text(stringResource(R.string.about_version, if (runningBeta) stringResource(R.string.updates_version_beta, version) else version), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
                }
            }
            val updating = UpdateRows(nav, memory, first)
            TvListRow(
                stringResource(R.string.about_licences),
                onClick = { actions?.open(ActionSheet(Strings.aboutLicences(context), body = { InfoLines(licences(context)) })) },
                modifier = (if (updating) Modifier else first).remembered(memory, "about/licences"),
            )
            CrashRows(LocalAppStore.current)
            TvListRow(
                stringResource(R.string.about_legal_link),
                onClick = { actions?.open(ActionSheet(Strings.aboutLegalLink(context), body = { LegalCode() })) },
                modifier = Modifier.remembered(memory, "about/legal"),
            )
            Quiet(stringResource(R.string.legal_hosts_no_content))
        }
    }
}

/** The legal and contact page, which About's Legal and contact opens on the phone. */
private const val LEGAL_URL = "https://kaeri.app/legal/"

/** Legal and contact: the page's address as a QR code, to open on a phone. */
@Composable
private fun LegalCode() {
    Column(Modifier.padding(start = 16.dp, top = 8.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        QrCode(LEGAL_URL, label = stringResource(R.string.licensed_tv_open_phone))
        Text(stringResource(R.string.licensed_tv_open_phone), style = MaterialTheme.typography.labelLarge, color = ObiSilk.Haze)
    }
}

/**
 * About's update rows, where this build updates itself or a release is staged: the newer release,
 * which opens the update's page, or Check now with what the last check said, in the phone's
 * words; then the daily check's switch. Whether there were any, so the first focus goes to the first of them.
 */
@Composable
private fun UpdateRows(nav: TvNavigator, memory: FocusMemory, first: Modifier): Boolean {
    val store = LocalAppStore.current
    val state by store.updates.state.collectAsStateWithLifecycle()
    val check by store.updates.check.collectAsStateWithLifecycle()
    val view by store.settings.view.collectAsStateWithLifecycle()
    val context = LocalContext.current
    val offer = state.offer
    if (offer == null && !store.updates.updatesItself) return false
    if (offer != null) {
        TvListRow(
            stringResource(R.string.updates_update_to, offer.name),
            onClick = nav::openUpdate,
            supporting = updateLine(context, state),
            icon = TvIcons.Update,
            modifier = first.remembered(memory, "about/update"),
        )
    } else {
        TvListRow(
            stringResource(R.string.action_check_now),
            onClick = { if (check != UpdateCheck.Checking) store.checkForUpdates() },
            supporting = when (val now = check) {
                UpdateCheck.Unknown, UpdateCheck.Found -> null
                UpdateCheck.Checking -> stringResource(R.string.updates_checking)
                UpdateCheck.UpToDate -> stringResource(R.string.updates_up_to_date)
                is UpdateCheck.StableBehind -> stringResource(R.string.updates_stable_behind, now.version)
                is UpdateCheck.Failed -> stringResource(R.string.updates_check_failed, plainReason(context, now.reason))
            },
            modifier = first.remembered(memory, "about/check"),
        )
    }
    TvListRow(
        stringResource(R.string.updates_beta),
        onClick = { store.setSwitch(SettingKey.UPDATE_BETA, !store.settings.isOn(SettingKey.UPDATE_BETA, view)) },
        supporting = stringResource(R.string.updates_beta_detail),
        switch = store.settings.isOn(SettingKey.UPDATE_BETA, view),
        enabled = store.settings.row(SettingKey.UPDATE_BETA, view) != null,
        modifier = Modifier.remembered(memory, "about/beta"),
    )
    if (store.updates.updatesItself) {
        TvListRow(
            stringResource(R.string.tv_update_auto_check),
            onClick = { store.setSwitch(SettingKey.UPDATE_CHECK, !store.settings.isOn(SettingKey.UPDATE_CHECK, view)) },
            supporting = stringResource(R.string.tv_update_auto_check_detail),
            switch = store.settings.isOn(SettingKey.UPDATE_CHECK, view),
            enabled = store.settings.row(SettingKey.UPDATE_CHECK, view) != null,
            modifier = Modifier.remembered(memory, "about/auto-check"),
        )
    }
    return true
}

/** The update row's line, as the phone's: "Available · 48 MB", "Downloading 34 %", "Installing", or why it stopped. */
fun updateLine(context: Context, state: UpdateState): String = when (state) {
    UpdateState.None -> ""
    is UpdateState.Available -> listOfNotNull(Strings.tvUpdateAvailable(context), state.offer.bytes.takeIf { it > 0 }?.let { Wording.bytes(context, it) }).joinToString(" · ") // i18n-skip
    is UpdateState.Allowing -> Strings.tvUpdateWaitingInstall(context)
    is UpdateState.Downloading -> state.fraction?.let { Strings.updatesDownloadingPercent(context, percentText((it * 100).toInt())) } ?: Strings.downloadsCaptionRunningPlain(context)
    is UpdateState.Verifying -> Strings.updatesCheckingDownloadShort(context)
    is UpdateState.Installing -> Strings.updatesInstallingPlain(context)
    is UpdateState.Done -> Strings.updatesUpdated(context, state.offer.version)
    is UpdateState.Failed -> state.failure.text(context)
}

/** What Kaeri is under, and what it uses, as the phone's About lists them. */
private fun licences(context: Context) = listOf(
    Strings.settingsLicenceTitle(context) to Strings.settingsLicenceValue(context),
    Strings.settingsTypefaces(context) to Strings.settingsTypefacesValue(context),
    Strings.settingsLibraries(context) to Strings.settingsLibrariesValue(context),
    // Shipped in the TV's QR pages, which a phone's browser runs; the phone app serves no pages.
    Strings.tvLicencePageEncryption(context) to Strings.tvLicencePageEncryptionValue(context),
)

/** A row that only says something: its label and its value, not focusable. */
@Composable
private fun InfoRow(label: String, value: String) {
    Row(Modifier.fillMaxWidth().heightIn(min = 56.dp).padding(horizontal = 16.dp).semantics(mergeDescendants = true) {}, verticalAlignment = Alignment.CenterVertically) {
        Text(label, style = MaterialTheme.typography.titleMedium, color = ObiSilk.Silk, modifier = Modifier.weight(1f))
        Text(value, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
    }
}

/**
 * A panel's lines, each a label over its value, focusable so Up and Down move through them and
 * TalkBack reads each; then one action when there is one.
 */
@Composable
private fun InfoLines(lines: List<Pair<String, String>>, action: String? = null, onAction: () -> Unit = {}) {
    val host = LocalActions.current
    val first = remember { FocusRequester() }
    LazyColumn(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        itemsIndexed(lines) { index, (label, value) ->
            TvListRow(label, onClick = {}, supporting = value, modifier = if (index == 0) Modifier.focusRequester(first) else Modifier)
        }
        if (action != null) {
            item {
                TvListRow(action, onClick = { host?.close(); onAction() })
            }
        }
    }
    LaunchedEffect(Unit) { first.focusSoon() }
}

@Composable
private fun Quiet(text: String) {
    Text(text, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp))
}

@Composable
private fun SectionLabel(text: String) {
    Text(text, style = MaterialTheme.typography.labelLarge, color = app.kaeri.design.ObiSilk.Haze, modifier = Modifier.padding(start = 16.dp, top = 16.dp, bottom = 4.dp))
}

@Composable
private fun serverLine(row: app.kaeri.engine.model.ServerRow): String {
    if (row.signedOut) return stringResource(if (row.signedOutHere) R.string.servers_row_signed_out_here else R.string.servers_row_signed_out)
    if (row.notAnswering) {
        val seen = row.lastSeen
        return if (seen != null) stringResource(R.string.servers_row_not_answering, app.kaeri.store.AirTime.seen(seen)) else stringResource(R.string.sources_status_not_answering)
    }
    val libraries = row.libraries.count { it.ticked }
    val summary = androidx.compose.ui.res.pluralStringResource(R.plurals.servers_row_libraries, libraries, libraries) + " · " + androidx.compose.ui.res.pluralStringResource(R.plurals.servers_row_series, row.series.toInt(), row.series.toInt())
    val unmatched = (row.series - row.matched).toInt()
    return if (unmatched > 0) "$summary · ${stringResource(R.string.servers_row_needs_look, unmatched)}" else summary // i18n-skip
}

@Composable
private fun licensedLine(view: app.kaeri.engine.model.LicensedView?): String {
    val services = view?.settings?.services.orEmpty()
    if (services.isEmpty()) return stringResource(R.string.licensed_row_none)
    val region = view?.region?.let { app.kaeri.tv.signin.regionName(it) } ?: stringResource(R.string.licensed_region_device)
    return stringResource(R.string.licensed_row_line, app.kaeri.tv.signin.joinServices(services), region)
}
