// The one activity: edge to edge, portrait on phones, the navigation bar and its destinations.

package app.kaeri.app

import android.text.format.DateFormat
import app.kaeri.store.AirTime
import android.Manifest
import android.content.Context
import android.content.Intent
import android.app.PictureInPictureUiState
import android.content.pm.ActivityInfo
import android.content.pm.ApplicationInfo
import android.content.pm.PackageManager
import android.content.res.Configuration
import android.graphics.Color
import android.os.Build
import android.os.Bundle
import android.view.KeyEvent
import android.provider.Settings
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.collectAsState
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.content.pm.ShortcutManagerCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.repeatOnLifecycle
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.receiveAsFlow
import app.kaeri.store.chaptersBusy
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import app.kaeri.app.tv.CastService
import app.kaeri.app.tv.TvRemoteService
import app.kaeri.design.KaeriTheme
import app.kaeri.engine.ffi.FfiTvCommand
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.Bounds
import app.kaeri.engine.model.MediaFormat
import app.kaeri.engine.model.SearchQuery
import app.kaeri.engine.model.Season
import app.kaeri.engine.model.SettingKey
import app.kaeri.player.takeBackBeforeShown
import app.kaeri.player.ui.PictureInPictureOnLeave
import app.kaeri.player.ui.PlayerFrame
import app.kaeri.store.DownloadsModel
import app.kaeri.store.InviteLinks
import app.kaeri.store.readAtMost
import app.kaeri.store.TvPlayRequest
import app.kaeri.store.TvRemoteModel
import app.kaeri.store.traced
import app.kaeri.ui.AppActions
import app.kaeri.ui.AppLink
import app.kaeri.ui.AppRoot
import app.kaeri.ui.NotificationActions
import app.kaeri.ui.Destination
import app.kaeri.ui.reading.LocalEinkLayout
import app.kaeri.eink.EinkAnswerPrefs
import app.kaeri.eink.EinkLook
import app.kaeri.reader.phone.ReaderLauncher
import app.kaeri.reader.phone.ReaderRequest
import app.kaeri.reader.phone.SettingsPane
import app.kaeri.store.AppLanguage

class MainActivity : ComponentActivity() {
    // Below Android 13 the chosen language is the app's own to apply.
    override fun attachBaseContext(newBase: android.content.Context) = super.attachBaseContext(AppLanguage.wrap(newBase))

    private val app get() = application as KaeriApp

    private val askNotifications = registerForActivityResult(ActivityResultContracts.RequestPermission()) {
        app.notifier.asked()
    }

    /**
     * The permission asked for as new-episode notifications are turned on: they go on with it,
     * and without it stay off, the switch's line saying why.
     */
    private val askForEpisodes = registerForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        app.notifier.asked()
        if (granted) app.episodeAlerts.set(true) else app.episodeAlerts.refuse()
    }

    /**
     * Android 17's local network permission, asked the first time Play on TV is used: the TVs are
     * found and reached on the network this phone is on. The browse starts again once it is given.
     */
    private val askLocalNetwork = registerForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        app.store.cast.localNetwork(granted)
        if (granted) {
            app.store.tv.browseAgain()
            app.store.tv.retryNow()
        }
    }

    private fun askLocalNetworkOnce() {
        if (Build.VERSION.SDK_INT < LOCAL_NETWORK_SDK) return
        if (checkSelfPermission(Manifest.permission.ACCESS_LOCAL_NETWORK) == PackageManager.PERMISSION_GRANTED) return
        askLocalNetwork.launch(Manifest.permission.ACCESS_LOCAL_NETWORK)
    }

    /** What waits on the permission a show's own switch asked for: switching it on. */
    private var permitted: (() -> Unit)? = null

    /** The permission asked for as a show is switched on for itself: on with it, left as it was without. */
    private val askForShow = registerForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        app.notifier.asked()
        if (granted) permitted?.invoke()
        permitted = null
    }

    /**
     * Do [then] once new-episode notifications can show, as a show's own switch turns on: at once
     * where they can, after the permission where it was never given, and where Android says no,
     * not at all, with the system's settings open instead.
     */
    private fun permitNotifications(then: () -> Unit) {
        val granted = NotificationPermission.granted(this)
        when (EpisodeAlerts.Turn.of(true, app.episodeNotifier.canPost, granted)) {
            EpisodeAlerts.Turn.On -> then()
            EpisodeAlerts.Turn.Ask -> if (NotificationPermission.askable) {
                permitted = then
                askForShow.launch(NotificationPermission.name)
            }
            EpisodeAlerts.Turn.SystemSettings -> openNotificationSettings()
            EpisodeAlerts.Turn.Off -> {}
        }
    }

    /** New-episode notifications, on Settings' switch: on only once they can show. */
    private fun setEpisodeNotifications(on: Boolean) {
        val permitted = NotificationPermission.granted(this)
        when (EpisodeAlerts.Turn.of(on, app.episodeNotifier.canPost, permitted)) {
            EpisodeAlerts.Turn.Off -> app.episodeAlerts.set(false)
            EpisodeAlerts.Turn.On -> app.episodeAlerts.set(true)
            EpisodeAlerts.Turn.Ask -> if (NotificationPermission.askable) askForEpisodes.launch(NotificationPermission.name)
            EpisodeAlerts.Turn.SystemSettings -> {
                app.episodeAlerts.refuse()
                openNotificationSettings()
            }
        }
    }

    /**
     * The system's settings for the New episodes channel: its sound, vibration and the lock screen,
     * and the app's notifications as a whole where those are off.
     */
    private fun openNotificationSettings() {
        // The notifier makes the channel, which the page needs to be there.
        app.episodeNotifier
        startActivity(
            Intent(Settings.ACTION_CHANNEL_NOTIFICATION_SETTINGS)
                .putExtra(Settings.EXTRA_APP_PACKAGE, packageName)
                .putExtra(Settings.EXTRA_CHANNEL_ID, EpisodeNotifier.CHANNEL),
        )
    }

    /** AniList's and MyAnimeList's sign-in in a Custom Tab, and the redirect that ends it. */
    private val signIn by lazy { BrowserSignIn(this, app.store) }

    /**
     * The app links the activity is opened on, launcher shortcuts and notifications, kept until
     * the frame is there to follow them.
     */
    private val links = Channel<AppLink>(Channel.BUFFERED)
    private val linkFlow = links.receiveAsFlow()

    override fun onCreate(savedInstanceState: Bundle?) {
        // Out of the launch theme, whose window draws Android 11's splash screen. On a panel the
        // last run found to be e-ink, the window is paper from the start, so a cold start never
        // paints the dark ground before the paper shell; the engine's answer then confirms it.
        val paper = EinkAnswerPrefs(this).load()?.eink == true
        setTheme(if (paper) R.style.Theme_Kaeri_Paper else R.style.Theme_Kaeri)
        // Light icons on the dark ground and dark ones on paper, over bars the app draws itself.
        val bars = if (paper) SystemBarStyle.light(Color.TRANSPARENT, Color.TRANSPARENT) else SystemBarStyle.dark(Color.TRANSPARENT)
        enableEdgeToEdge(statusBarStyle = bars, navigationBarStyle = bars)
        super.onCreate(savedInstanceState)
        app.player.hostStarted()
        // Back a moment after Play, before the player's screen is there to take it.
        takeBackBeforeShown(app.player)
        // The navigation bar's own colour runs under the gesture bar, with no scrim of the system's.
        window.isNavigationBarContrastEnforced = false
        holdPortraitOnPhones(resources.configuration)

        traced("launch") { app.start(Flavour.launch(this, intent)) }
        app.store.tv.firstUse = ::askLocalNetworkOnce
        // Where Android stopped the app as "Install unknown apps" turned on, the press goes on.
        app.appUpdates?.kept()?.let(app.store.updates::returning)
        Flavour.started(this, app.store, intent, app.episodeNotifier, app.player)
        val steerable = debuggable || Flavour.FIXTURES
        val start = intent.getStringExtra(SECTION)?.takeIf { steerable }
            ?.let(Destination::named) ?: Destination.Home
        val title = intent.getStringExtra(TITLE)?.takeIf { steerable }?.toUIntOrNull()
        val query = intent.getStringExtra(QUERY)?.takeIf { steerable }
        val genre = intent.getStringExtra(GENRE)?.takeIf { steerable }
        val filters = intent.getStringExtra(FILTERS)?.takeIf { steerable }
        val search = when {
            // Every row filter set, and two of the sheet's, which the fixtures' catalogue answers
            // with two titles (the Fixtures build turns the catalogue on for this extra).
            filters == "set" -> SearchQuery(
                text = query.orEmpty(), genres = listOf("Fantasy"), formats = listOf(MediaFormat.TV),
                season = Season.SPRING, years = Bounds(min = 2024u, max = 2024u), minScore = 70u, licensedOnly = true,
            )
            query != null || genre != null -> SearchQuery(text = query.orEmpty(), genres = listOfNotNull(genre))
            else -> null
        }
        val sheet = intent.getStringExtra(SHEET)?.takeIf { steerable }
        val page = intent.getStringExtra(PAGE)?.takeIf { steerable }
        // The fixtures and debug builds open on the screen asked for; their pages only when asked.
        val firstLaunch = if (steerable) intent.getBooleanExtra(FIRST_LAUNCH, false) else true
        // Air times in the clock the system is set to.
        AirTime.twentyFourHour = DateFormat.is24HourFormat(this)

        // Play from the title page, the Continue hero or an episode row: the engine finds the
        // stream and opens the player, which the frame then shows.
        val actions = AppActions(
            play = { id, episode -> app.store.send(Command.Play(id, episode)) },
            playFromStart = { id, episode -> app.store.send(Command.Play(id, episode, fromStart = true)) },
            notifications = NotificationActions(
                turn = ::setEpisodeNotifications,
                refused = app.episodeAlerts.refused,
                permit = ::permitNotifications,
                openSystemSettings = ::openNotificationSettings,
            ),
            openReader = { work, chapter, cover, from ->
                ReaderLauncher.open(ReaderRequest(work, chapter, eink = app.eink.state.value.eink, cover = cover, from = from))
            },
            shareFiles = { files, then -> shareExport(files); then() },
        )
        // Play on TV from the player: the episode and where it is, which the TV starts from.
        val player = PlayerFrame(app.player) {
            val tv = app.store.tv
            val model = app.player
            val id = model.titleId
            val episode = model.startingEpisode
            if (!tv.available || id == null || episode == null) {
                null
            } else {
                { tv.offer(TvPlayRequest(id, model.showTitle, episode, model.position, fromPlayer = true)) }
            }
        }
        // The window's recomposer, so an e-ink panel can stop every animation in it.
        val motion = PanelMotion(contentResolver).also { it.install(this) }
        traced("setContent") { setContent {
            KaeriTheme {
                // On an e-ink panel the app draws the e-ink shell: paper, ink and nothing moving.
                val einkState by app.eink.state.collectAsState()
                val einkLayout = if (einkState.eink) EinkLook(colour = einkState.amber) else null
                androidx.compose.runtime.SideEffect { motion.still = einkState.eink }
                // Dark icons on paper; light ones on the dark ground.
                val decor = androidx.compose.ui.platform.LocalView.current
                androidx.compose.runtime.LaunchedEffect(einkState.eink) {
                    androidx.core.view.WindowCompat.getInsetsController(window, decor).run {
                        isAppearanceLightStatusBars = einkState.eink
                        isAppearanceLightNavigationBars = einkState.eink
                    }
                    paperSplash(einkState.eink, paper)
                }
                CompositionLocalProvider(LocalEinkLayout provides einkLayout) {
                ReaderHost(app.store, app.eink) { AppRoot(
                    app.store,
                    if (search != null) Destination.Search else start,
                    actions = actions,
                    startTitle = title,
                    startSearch = search,
                    startSheet = sheet,
                    startPage = page,
                    player = player,
                    links = linkFlow,
                    onSection = { section -> app.appUpdates?.section = section },
                    firstLaunch = firstLaunch,
                ) }
                }
            }
        } }
        if (Flavour.FIXTURES || debuggable) announceReady()
        askForNotificationsWhenNeeded()
        signIn.start()
        // A restored activity, or one opened again from Recents, has its navigation already.
        val fresh = savedInstanceState == null && intent.flags and Intent.FLAG_ACTIVITY_LAUNCHED_FROM_HISTORY == 0
        if (!signIn.handle(intent) && !(fresh && follow(intent)) && !invite(intent) && !party(intent) && !opened(intent)) remote(intent)
        keepDownloadsRunning()
        startUpdateActivities()
    }

    /** Whether the system's splash screen was last set to paper, from [paperSplash]. */
    private var splashPaper: Boolean? = null

    /**
     * The system's own splash screen in paper once the panel is known to be e-ink, and back to the
     * ground when it no longer is (Android 13 and up, where the system keeps the choice for the
     * next launch). Asked only when the answer differs from the one this launch started with.
     */
    private fun paperSplash(eink: Boolean, atLaunch: Boolean) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return
        val was = splashPaper ?: atLaunch
        splashPaper = eink
        if (eink == was) return
        runCatching { splashScreen.setSplashScreenTheme(if (eink) R.style.Theme_Kaeri_Paper else android.content.res.Resources.ID_NULL) }
            .onFailure { Log.w(KaeriApp.TAG, "the splash screen kept its theme", it) }
    }

    /**
     * What the update asks to be started from an activity the viewer is looking at: Android's
     * "Install unknown apps" page for the app, and the installer's confirmation.
     */
    private fun startUpdateActivities() {
        val updates = app.appUpdates ?: return
        lifecycleScope.launch {
            repeatOnLifecycle(Lifecycle.State.RESUMED) {
                updates.launch.collect { intent ->
                    intent ?: return@collect
                    val started = runCatching { startActivity(intent) }
                        .onFailure { Log.w(KaeriApp.TAG, "update: ${intent.action} did not open: ${it.message}") }
                        .isSuccess
                    updates.launched(intent, started)
                }
            }
        }
    }

    /**
     * Send to e-reader: hand the prepared files to the share sheet, where the person picks the app
     * that carries them to the e-reader (Send to Kindle, a cloud folder, mail).
     */
    private fun shareExport(files: List<app.kaeri.engine.model.ExportFile>) {
        val uris = files.mapNotNull { file ->
            runCatching {
                androidx.core.content.FileProvider.getUriForFile(this, "$packageName.exports", java.io.File(file.path), file.name)
            }.onFailure { Log.w(KaeriApp.TAG, "an export could not be shared", it) }.getOrNull()
        }
        if (uris.isEmpty()) return
        val send = if (uris.size == 1) {
            Intent(Intent.ACTION_SEND).putExtra(Intent.EXTRA_STREAM, uris.first())
        } else {
            Intent(Intent.ACTION_SEND_MULTIPLE).putParcelableArrayListExtra(Intent.EXTRA_STREAM, ArrayList(uris))
        }
        send.type = if (files.all { it.name.endsWith(".cbz") }) "application/vnd.comicbook+zip" else "application/epub+zip"
        send.clipData = android.content.ClipData.newRawUri(null, uris.first()).apply { uris.drop(1).forEach { addItem(android.content.ClipData.Item(it)) } }
        send.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        startActivity(Intent.createChooser(send, null))
    }

    /**
     * While the app is in front, ask for the download job whenever an episode or a chapter waits or runs: a
     * user-initiated job is asked for from a screen the person is using.
     */
    private fun keepDownloadsRunning() {
        lifecycleScope.launch {
            repeatOnLifecycle(Lifecycle.State.RESUMED) {
                combine(app.store.downloads.rows, app.store.reading.downloads, ::Pair).collect { (rows, books) ->
                    val wifiOnly = !app.store.settings.isOn(SettingKey.DOWNLOADS_ON_CELLULAR)
                    val chapters = chaptersBusy(books)
                    if (DownloadsModel.busy(rows) || chapters > 0) app.notifier.wantPermission()
                    Downloads.ensure(this@MainActivity, rows, wifiOnly, chapters)
                }
            }
        }
    }

    /**
     * An app link the activity was opened on goes to the frame to follow: a shortcut's, which the
     * launcher is told was used, or a notification's.
     */
    private fun follow(intent: Intent?): Boolean {
        val link = intent?.appLink() ?: return false
        // Taken off the intent, so a recreated activity does not follow it again.
        intent.action = null
        links.trySend(link)
        // A notification's action leaves its notification up; following it takes it away.
        intent?.getStringExtra(EpisodeNotifier.EXTRA_NOTIFICATION)?.let { tag ->
            if (ReadingNotifier.isChapters(tag)) app.readingNotifier.dismiss(tag) else app.episodeNotifier.dismiss(tag)
        }
        ContinueShortcut.shortcutOf(link)?.let { ShortcutManagerCompat.reportShortcutUsed(this, it) }
        return true
    }

    /** An invite link the app was opened with opens the join sheet with it, checked as it opens. */
    private fun invite(intent: Intent?): Boolean {
        val link = intent?.data?.toString()?.let(InviteLinks::invite) ?: return false
        app.store.stores.openJoin(link)
        // Taken off the intent, so a recreated activity does not open the sheet again.
        intent.data = null
        return true
    }

    /** A watch party invite, the https one or on kaeri://party, opens that room's lobby. */
    private fun party(intent: Intent?): Boolean {
        val link = intent?.data?.toString() ?: return false
        if (!app.store.followPartyLink(link)) return false
        // Taken off the intent, so a recreated activity does not open the lobby again.
        intent.data = null
        return true
    }

    /**
     * A magnet link or a torrent file the system handed over opens the same sheet as Open link or
     * file does, guessing what it is. A file is read here, small as it is, and its bytes go on.
     */
    private fun opened(intent: Intent?): Boolean {
        val uri = intent?.data ?: return false
        if (intent.action != Intent.ACTION_VIEW) return false
        when {
            uri.scheme.equals("magnet", ignoreCase = true) -> app.store.openLink(uri.toString())
            uri.scheme == "content" || uri.scheme == "file" -> lifecycleScope.launch {
                val bytes = withContext(Dispatchers.IO) {
                    runCatching { contentResolver.openInputStream(uri)?.use { it.readAtMost(MAX_TORRENT_BYTES + 1) } }.getOrNull()
                }
                if (bytes != null && bytes.size <= MAX_TORRENT_BYTES) {
                    app.store.openTorrentFile(bytes)
                } else {
                    app.store.opened.openPaste()
                    app.store.opened.unusable()
                }
            }
            else -> return false
        }
        // Taken off the intent, so a recreated activity does not open the sheet again.
        intent.data = null
        return true
    }

    /**
     * A TV's pairing link, from its QR code read by the system's camera, pairs with it; the TV
     * notification's tap opens its card.
     */
    private fun remote(intent: Intent?): Boolean {
        intent ?: return false
        if (intent.getBooleanExtra(TvRemoteService.EXTRA_REMOTE, false)) {
            intent.removeExtra(TvRemoteService.EXTRA_REMOTE)
            app.store.tv.openRemote()
            return true
        }
        if (intent.getBooleanExtra(CastService.EXTRA_CAST, false)) {
            intent.removeExtra(CastService.EXTRA_CAST)
            app.store.cast.openCard()
            return true
        }
        val link = intent.data?.toString()?.takeIf { it.startsWith("${TvRemoteModel.SCHEME}://") } ?: return false
        intent.data = null
        return app.store.tv.open(link)
    }

    /**
     * The volume keys move the TV's volume while its card is open, or while it is driven and
     * plays; the TV's media session takes them once the app is behind another.
     */
    override fun dispatchKeyEvent(event: KeyEvent): Boolean {
        val step = when (event.keyCode) {
            KeyEvent.KEYCODE_VOLUME_UP -> TvRemoteModel.VOLUME_STEP
            KeyEvent.KEYCODE_VOLUME_DOWN -> -TvRemoteModel.VOLUME_STEP
            else -> return super.dispatchKeyEvent(event)
        }
        val tv = app.store.tv
        val driving = tv.connected.value != null &&
            (tv.sheet.value == TvRemoteModel.Sheet.Remote || tv.card.value?.card?.now != null)
        if (!driving || app.player.isPresented) return super.dispatchKeyEvent(event)
        if (event.action == KeyEvent.ACTION_DOWN) tv.send(FfiTvCommand.Volume(step))
        return true
    }

    override fun onDestroy() {
        // Back a moment after Play: the engine has not opened the player yet, and must not.
        if (isFinishing) app.player.hostFinished()
        super.onDestroy()
    }

    override fun onStart() {
        super.onStart()
        // Back to the app after a Back put it behind: a play from here has somewhere to show.
        app.player.hostStarted()
    }

    override fun onResume() {
        super.onResume()
        signIn.resumed()
        AirTime.twentyFourHour = DateFormat.is24HourFormat(this)
        // Back from Android's "Install unknown apps" page: the update goes on, if it may.
        app.store.updates.resumed()
        // Back from the app's settings, where the local network may have been allowed.
        if (Build.VERSION.SDK_INT >= LOCAL_NETWORK_SDK && app.store.cast.blocked.value &&
            checkSelfPermission(Manifest.permission.ACCESS_LOCAL_NETWORK) == PackageManager.PERMISSION_GRANTED
        ) {
            app.store.cast.localNetwork(allowed = true)
        }
    }

    /**
     * Into Picture in Picture, or out of it: the window shows the video alone while in. Closed
     * from its window, the activity is already stopped when this says so, and the player closes,
     * keeping the place; expanded, the controls come up and hide after 3 s.
     */
    override fun onPictureInPictureModeChanged(isInPictureInPictureMode: Boolean, newConfig: Configuration) {
        super.onPictureInPictureModeChanged(isInPictureInPictureMode, newConfig)
        val player = app.player
        player.inPip = isInPictureInPictureMode
        if (isInPictureInPictureMode) return
        if (lifecycle.currentState == Lifecycle.State.CREATED) player.viewerClosed() else player.poke()
    }

    /**
     * On the way into the window the overlays go at once, before the system moves the video:
     * Android 15 says when that starts. Android 12 to 14 say only once it is in.
     */
    override fun onPictureInPictureUiStateChanged(pipState: PictureInPictureUiState) {
        super.onPictureInPictureUiStateChanged(pipState)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.VANILLA_ICE_CREAM && pipState.isTransitioningToPip) app.player.inPip = true
    }

    /**
     * Android 11 has no auto-enter: going home while a video plays opens the window here, the
     * overlays gone first as Android 15's transition has them.
     */
    override fun onUserLeaveHint() {
        super.onUserLeaveHint()
        PictureInPictureOnLeave.enter(this) { app.player.inPip = it }
    }

    override fun onConfigurationChanged(newConfig: Configuration) {
        super.onConfigurationChanged(newConfig)
        // The player asks for landscape on a phone while it is up, and the reader follows the
        // sensor while it is open; each asks for portrait again as it closes.
        if (!app.player.isPresented && ReaderLauncher.request.value == null) holdPortraitOnPhones(newConfig)
    }

    /**
     * On a phone, under 600 dp at its smallest, the browse screens are portrait only; tablets and
     * foldables turn freely, and a folded foldable's outer screen follows the phone rule.
     */
    private fun holdPortraitOnPhones(configuration: Configuration) {
        requestedOrientation = if (configuration.smallestScreenWidthDp < 600) {
            ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
        } else {
            ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
        }
    }

    private val debuggable: Boolean get() = applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE != 0

    /** Say `KAERI_READY` in the log once the screens have what they asked for: what shots.sh waits on. */
    private fun announceReady() {
        lifecycleScope.launch {
            // Up to three minutes: an emulator on a machine busy with builds starts the engine slowly.
            if (app.store.settle(timeoutMillis = 180_000) && Flavour.reachedTv(intent, app.store)) {
                Log.i(KaeriApp.TAG, READY)
            } else {
                Log.w(KaeriApp.TAG, "the screens did not settle")
            }
        }
    }

    /** Ask for the notification permission the first time a notification needs it, and only then. */
    private fun askForNotificationsWhenNeeded() {
        val preferences = getSharedPreferences("kaeri", Context.MODE_PRIVATE)
        lifecycleScope.launch {
            repeatOnLifecycle(Lifecycle.State.RESUMED) {
                app.notifier.wantsPermission.collect { wants ->
                    if (wants && NotificationPermission.askable && !preferences.getBoolean(NOTIFICATIONS_ASKED, false)) {
                        preferences.edit().putBoolean(NOTIFICATIONS_ASKED, true).apply()
                        askNotifications.launch(NotificationPermission.name)
                    }
                }
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        // The redirect a sign-in comes back with is the engine's, and is not kept as the
        // activity's intent.
        if (signIn.handle(intent) || quietRule(intent) || follow(intent) || invite(intent) || party(intent) || opened(intent) || remote(intent)) return
        setIntent(intent)
    }

    /**
     * The reader's Do Not Disturb rule names this activity as its settings: from the system's
     * Do Not Disturb page it opens the reader's Paper mode page, where Quiet is turned off.
     */
    private fun quietRule(intent: Intent): Boolean {
        if (intent.getStringExtra(android.app.NotificationManager.EXTRA_AUTOMATIC_RULE_ID) == null) return false
        ReaderLauncher.showSettings(SettingsPane.Paper)
        return true
    }

    companion object {
        /** The largest torrent file read: a list of pieces, not the media. */
        private const val MAX_TORRENT_BYTES = 8 * 1024 * 1024

        /** The marker shots.sh waits for in the log. */
        const val READY = "KAERI_READY"

        /** The destination to open on: `continue`, `calendar`, `seasonal`, `library` or `search`. */
        const val SECTION = "section"

        /** A title page to open there, by its AniList id. */
        const val TITLE = "title"

        /** A search to run, on Search. */
        const val QUERY = "query"

        /** A genre to browse, on Search, alone or with the query. */
        const val GENRE = "genre"

        /** Search's filters: `set`, every row filter and two of the sheet's, with titles that match. */
        const val FILTERS = "filters"

        /** A sheet to open over the destination: `filters`, Search's filters. */
        const val SHEET = "sheet"

        /** A page to open over the destination, by its route: `settings`, `settings/playback`. */
        const val PAGE = "page"

        /** Whether first launch's pages show, on a fixtures or debug build: `--ez firstLaunch true`. */
        const val FIRST_LAUNCH = "firstLaunch"

        private const val NOTIFICATIONS_ASKED = "notificationsAsked"

        /** Android 17, where reaching the local network asks first. */
        private const val LOCAL_NETWORK_SDK = 37
    }
}
