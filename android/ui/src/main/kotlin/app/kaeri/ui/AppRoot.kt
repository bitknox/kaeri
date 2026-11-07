// The app's frame: the navigation bar on a phone, the rail on a tablet or an unfolded foldable,
// and the five destinations behind it, each with its own back stack of title pages.

package app.kaeri.ui

import androidx.annotation.StringRes
import app.kaeri.i18n.R
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.animation.AnimatedContentTransitionScope
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.ExperimentalSharedTransitionApi
import androidx.compose.animation.SharedTransitionLayout
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.consumeWindowInsets
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.ShortNavigationBar
import androidx.compose.material3.ShortNavigationBarItem
import androidx.compose.material3.ShortNavigationBarItemDefaults
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.ModalWideNavigationRail
import androidx.compose.material3.WideNavigationRailDefaults
import androidx.compose.material3.WideNavigationRailItem
import androidx.compose.material3.WideNavigationRailItemDefaults
import androidx.compose.material3.WideNavigationRailValue
import androidx.compose.material3.rememberWideNavigationRailState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Stable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import app.kaeri.design.stillFade
import androidx.compose.runtime.SideEffect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.isSpecified
import androidx.compose.material3.LocalTextStyle
import androidx.compose.ui.unit.sp
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.navigation.NavBackStackEntry
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.navigation
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.emptyFlow
import kotlinx.coroutines.launch
import app.kaeri.design.Motion
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.PlayerCommand
import app.kaeri.store.AppStore
import app.kaeri.engine.core.AnilistId
import app.kaeri.engine.core.ChapterId
import app.kaeri.engine.core.WorkId
import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.engine.model.NoticeHint
import app.kaeri.engine.model.SearchQuery

/** The top-level destinations, in the navigation bar's order. */
enum class Destination(val route: String, @StringRes val label: Int, val icon: ImageVector, val selectedIcon: ImageVector) {
    Home("home", R.string.home_title, Icons.Continue, Icons.ContinueSelected),
    Calendar("calendar", R.string.nav_calendar, Icons.Calendar, Icons.CalendarSelected),
    Seasonal("seasonal", R.string.nav_seasonal, Icons.Seasonal, Icons.SeasonalSelected),
    Library("library", R.string.nav_library, Icons.Library, Icons.LibrarySelected),
    Search("search", R.string.nav_search, Icons.Search, Icons.SearchSelected),

    /** Reading's Home beside Watch's, with Both. With Read alone, [Home] is reading's Home. */
    Read("read", R.string.nav_read, Icons.Read, Icons.ReadSelected),

    /** New chapters of the series followed: a bar item with Read alone, Read's heading with Both. */
    Updates("updates", R.string.nav_updates, Icons.Updates, Icons.UpdatesSelected);

    companion object {
        /** The destination [name] names, by its route, case aside. */
        fun named(name: String): Destination? =
            // "continue" is what links and saved state called Home before it had its own name.
            if (name.equals("continue", ignoreCase = true)) Home else entries.firstOrNull { it.route.equals(name, ignoreCase = true) }
    }
}

/**
 * The destinations the bar and rail show for what Kaeri is for: Watch alone keeps its five, Read
 * alone has Home, Updates, Library and Search, and Both has Watch, Read, Library and Search, with
 * Calendar, Seasonal and Updates opened from the headings on each Home.
 */
fun barDestinations(answer: KaeriIsFor): List<Destination> = when (answer) {
    KaeriIsFor.WATCH -> listOf(Destination.Home, Destination.Calendar, Destination.Seasonal, Destination.Library, Destination.Search)
    KaeriIsFor.READ -> listOf(Destination.Home, Destination.Updates, Destination.Library, Destination.Search)
    KaeriIsFor.BOTH -> listOf(Destination.Home, Destination.Read, Destination.Library, Destination.Search)
}

/** The bar item that stands for [destination] when it has none of its own: Watch's or Read's. */
internal fun barOwner(destination: Destination, answer: KaeriIsFor): Destination = when {
    destination in barDestinations(answer) -> destination
    destination == Destination.Updates && answer == KaeriIsFor.BOTH -> Destination.Read
    destination == Destination.Read -> Destination.Home
    else -> Destination.Home
}

/** The label [destination] has for [answer]: Home is Watch beside Read. */
@StringRes
internal fun Destination.labelFor(answer: KaeriIsFor): Int =
    if (this == Destination.Home && answer == KaeriIsFor.BOTH) R.string.nav_watch else label

/** What Kaeri is for on this device, as the screens read it: Watch until the engine says. */
@Composable
internal fun kaeriIsFor(): KaeriIsFor {
    val view by LocalAppStore.current.medium.view.collectAsStateWithLifecycle()
    return view?.answer ?: KaeriIsFor.WATCH
}

/** What the screens' actions open, which later phases fill in. */
data class AppActions(
    /** Connect AniList: the engine's sign-in in a Custom Tab, which [AppRoot] starts itself. */
    val connectAniList: () -> Unit = {},
    /** Join a plugin store: the join sheet, which [AppRoot] opens itself. */
    val addStore: () -> Unit = {},
    /** Accounts and settings: Settings, which [AppRoot] opens itself. */
    val openSettings: () -> Unit = {},
    /** Play an episode: the player, from A3. */
    val play: (id: AnilistId, episode: String) -> Unit = { _, _ -> },
    /** Play an episode from its first frame, whatever position it kept: Play from the beginning. */
    val playFromStart: (id: AnilistId, episode: String) -> Unit = { _, _ -> },
    /**
     * What a notice's button asks for: the stores, the player. Kitsu's sign-in [AppRoot] opens
     * itself.
     */
    val hint: (NoticeHint) -> Unit = {},
    /**
     * What Settings' Notifications pane asks of the app: turning new-episode notifications on,
     * which asks for the permission to post first, and the system's own settings for them. `null`
     * turns the setting itself, as the screenshots do.
     */
    val notifications: NotificationActions? = null,
    /**
     * Open the reader at [chapter] of [work], or where it was left. [cover] is the cover's art, and
     * [from] the cover's place on screen when the reader grows out of it.
     */
    val openReader: (work: WorkId, chapter: ChapterId?, cover: String?, from: String?) -> Unit = { _, _, _, _ -> },
    /** Hand prepared files to the share sheet, for Send to e-reader; then is called once it is up. */
    val shareFiles: (files: List<app.kaeri.engine.model.ExportFile>, then: () -> Unit) -> Unit = { _, then -> then() },
)

/** The app's side of the Notifications pane, and of a show's own New episodes switch. */
class NotificationActions(
    /** Turn new-episode notifications on, once they can show, or off. */
    val turn: (Boolean) -> Unit,
    /** Whether Android did not let them go on, or stay on: the switch's line says so. */
    val refused: StateFlow<Boolean>,
    /**
     * Let notifications show, then do what it is handed: at once where they can, after asking for
     * the permission the first time, and not at all where Android says no. What switching a show
     * on for itself goes through, as Settings' switch does.
     */
    val permit: (then: () -> Unit) -> Unit = { it() },
    /** The system's settings for the channel: sound, vibration and the lock screen. */
    val openSystemSettings: () -> Unit,
)

/** The app's notification actions, for a show's own switch: `null` where there is no app, as in the screenshots. */
val LocalNotificationActions = staticCompositionLocalOf<NotificationActions?> { null }

/**
 * The player the app hosts, as the frame shows it: a destination of its own over the navigation
 * bar, grown from the banner it was opened from (T2) and closing back into it (T3).
 */
@Stable
interface PlayerSlot {
    /** Whether the player is up, which puts its destination on screen. */
    val presented: Boolean

    /** The title it plays: the banner it grows from and closes into. */
    val title: AnilistId?

    /** The player itself, filling the screen. */
    @Composable
    fun Content()
}

/** The player's route: outside the destinations' graphs, over whichever is on screen. */
const val PLAYER_ROUTE = "player"

/** Whether the window is wide enough for the rail: 600 dp and up, a tablet or an unfolded foldable. */
@Composable
fun wideWindow(): Boolean = LocalConfiguration.current.screenWidthDp >= 600

/**
 * Whether a NavHost change switches destinations rather than pushing inside one. A page beyond
 * them, Settings and what it opens, fades through as a switch does, going and coming back. The
 * player belongs to no destination: it is pushed over one, as a title page is.
 */
private fun AnimatedContentTransitionScope<NavBackStackEntry>.switching(): Boolean {
    if (initialState.destination.route == PLAYER_ROUTE || targetState.destination.route == PLAYER_ROUTE) {
        return false
    }
    return initialState.destinationOf() != targetState.destinationOf() || initialState.isPage || targetState.isPage
}

/**
 * The frame. A switch between destinations fades the page through on the quick spring (T6); a
 * title page pushed inside one keeps the page underneath, and the shared elements do the moving
 * (T1), seeked by predictive back.
 */
@OptIn(ExperimentalSharedTransitionApi::class)
@Composable
fun AppRoot(
    store: AppStore,
    start: Destination = Destination.Home,
    actions: AppActions = AppActions(),
    /** A title page to open on [start] as the app starts: the fixtures' `title` extra. */
    startTitle: AnilistId? = null,
    /** A search to run on Search as the app starts: the fixtures' `query` and `genre` extras. */
    startSearch: SearchQuery? = null,
    /** A sheet to open on [start] as the app starts: the fixtures' `sheet` extra, `filters` on Search. */
    startSheet: String? = null,
    /** A page to open over [start] as the app starts, by its route: the fixtures' `page` extra. */
    startPage: String? = null,
    /** The player, which the frame shows over everything while it is up. */
    player: PlayerSlot? = null,
    /** The links the system opens the app on, followed as they come: shortcuts, notifications. */
    links: Flow<AppLink> = emptyFlow(),
    /** The destination on screen, each time it changes: where the Updated notification comes back to. */
    onSection: (Destination) -> Unit = {},
    /** Whether first launch's pages may show, until this device has been through them. */
    firstLaunch: Boolean = false,
) {
    val nav = rememberNavController()
    val navigator = remember(nav) { Navigator(nav, start) }
    // With Remove animations on, the page on screen is recorded, so Another can fade a still of it out.
    val removed = app.kaeri.design.rememberAnimationsRemoved()
    val pageFade = app.kaeri.design.rememberStillFade()
    SideEffect { navigator.fade = pageFade }
    val sheets = remember { AppSheets() }
    // Whether Search is to open its field as it shows: a link to Search asked for it.
    var searchOpening by remember { mutableStateOf(false) }
    val routed = remember(actions, navigator, store) {
        actions.copy(
            openSettings = { navigator.openSettings() },
            connectAniList = { store.connect("anilist") },
            addStore = { store.stores.openJoin() },
            hint = { hint ->
                when (hint) {
                    NoticeHint.KITSU_CREDENTIALS -> sheets.kitsu = true
                    NoticeHint.OPEN_STORES -> navigator.open(STORES_ROUTE)
                    // The engine stops whatever plays, or whatever it is still finding.
                    NoticeHint.STOP_PLAYBACK -> store.send(Command.Player(PlayerCommand.Stop))
                    NoticeHint.UNDO_REMOVE_DEMO -> store.send(Command.AddDemo)
                    NoticeHint.REMOVE_DEMO -> store.send(Command.RemoveDemo)
                    NoticeHint.REPORT_CRASH -> store.crashes.askReport(null)
                    else -> actions.hint(hint)
                }
            },
        )
    }
    val entry by nav.currentBackStackEntryAsState()
    val current = entry?.destinationOf()
    val sectionChanged by rememberUpdatedState(onSection)
    androidx.compose.runtime.LaunchedEffect(current) { current?.let(sectionChanged) }
    // Where the app is, for a crash's record. Only the collector, when saving is on, listens.
    androidx.compose.runtime.LaunchedEffect(entry?.destination?.route) { store.crashes.place(crashPlaceOf(entry?.destination?.route)) }
    val onPage = entry?.isPage == true
    val fullWindow = entry?.isFullWindow == true
    val onPlayer = entry?.destination?.route == PLAYER_ROUTE
    val scope = rememberCoroutineScope()
    val context = androidx.compose.ui.platform.LocalContext.current
    val snackbars = remember { Snackbars(SnackbarHostState(), scope, context) }
    val wide = wideWindow()
    val mediumView by store.medium.view.collectAsStateWithLifecycle()
    val answer = mediumView?.answer ?: KaeriIsFor.WATCH

    CompositionLocalProvider(
        LocalAppStore provides store,
        LocalNavigator provides navigator,
        LocalNavigatorOrNull provides navigator,
        LocalSnackbars provides snackbars,
        LocalAppSheets provides sheets,
        LocalNotificationActions provides actions.notifications,
    ) {
        EngineNotices(onHint = routed.hint)
        SourcePinNotice()
        StoreJoinNotices()
        LicensedReturnNotice()
        CrashNoticeAndReport()
        UpdatePrompts()
        AccountSheets()
        ProfileSwitchSheet(actions)
        TvSheetHost()
        CastSheetHost()
        DiscussionHost()
        JoinStoreSheetHost()
        OpenedSheetHost()
        // Two links in a row: the later one's sheet replaces the earlier, never stacks on it.
        val joining by store.stores.joining.collectAsStateWithLifecycle()
        val tvSheet by store.tv.sheet.collectAsStateWithLifecycle()
        androidx.compose.runtime.LaunchedEffect(joining) { if (joining != null) store.tv.sheetClosed() }
        androidx.compose.runtime.LaunchedEffect(tvSheet) { if (tvSheet != null) store.stores.closeJoin() }
        androidx.compose.runtime.LaunchedEffect(startTitle) {
            startTitle?.let { navigator.openTitle(it, "launch") }
        }
        androidx.compose.runtime.LaunchedEffect(startPage) {
            startPage?.let(navigator::open)
        }
        // The player comes up over whatever is on screen, and goes back to it as it closes.
        if (player != null) {
            androidx.compose.runtime.LaunchedEffect(player.presented) {
                val there = nav.currentBackStackEntry?.destination?.route == PLAYER_ROUTE
                if (player.presented && !there) nav.navigate(PLAYER_ROUTE) { launchSingleTop = true }
                if (!player.presented && there) nav.popBackStack()
            }
        }
        // A link or a code opened a room: its lobby comes up over whatever is on screen.
        // With a TV driven from this phone and no party here, the phone asks where to join first.
        val linked by store.party.linked.collectAsStateWithLifecycle()
        var tvChoice by remember { mutableStateOf<app.kaeri.engine.model.PartyTarget?>(null) }
        androidx.compose.runtime.LaunchedEffect(linked) {
            val target = linked ?: return@LaunchedEffect
            store.party.tookLink()
            val drivenTv = store.tv.card.value?.card?.takeIf { it.party == null }
            if (drivenTv != null && store.party.view.value == null) tvChoice = target else navigator.open(LOBBY_ROUTE)
        }
        tvChoice?.let { target ->
            PartyTvChoiceSheet(
                target,
                tvName = store.tv.card.value?.card?.tvName.orEmpty(),
                onTv = {
                    store.tv.send(app.kaeri.engine.ffi.FfiTvCommand.PartyJoin(target.server, target.room))
                    store.closePartyLobby()
                    tvChoice = null
                },
                onPhone = {
                    tvChoice = null
                    navigator.open(LOBBY_ROUTE)
                },
                onDismiss = {
                    store.closePartyLobby()
                    tvChoice = null
                },
            )
        }
        val following by rememberUpdatedState(routed)
        androidx.compose.runtime.LaunchedEffect(links, navigator) {
            // A new link takes over from one still waiting to play.
            links.collectLatest { link -> navigator.follow(link, store, following) { searchOpening = true } }
        }
        // First launch's pages stand over the app, which TalkBack then does not reach.
        var covered by remember { mutableStateOf(false) }
        Scaffold(
            modifier = if (covered) Modifier.clearAndSetSemantics {} else Modifier,
            containerColor = MaterialTheme.colorScheme.surface,
            contentWindowInsets = WindowInsets(0),
            // The player says its own notices on the video; the snackbars wait for it to close.
            snackbarHost = {
                if (!onPlayer) {
                    androidx.compose.foundation.layout.Column {
                        UpdateNotice(hidden = entry?.destination?.route?.startsWith(SETTINGS_ROUTE) == true)
                        SnackbarHost(snackbars.host) { NoticeSnackbar(it) }
                    }
                }
            },
            bottomBar = {
                androidx.compose.foundation.layout.Column {
                    if (!onPlayer) TvNowPlayingBar()
                    if (!onPlayer) CastNowPlayingBar()
                    // Watch together's bar, above the navigation bar while a party is on.
                    if (!onPlayer) PartyBar(hidden = entry?.destination?.route == LOBBY_ROUTE)
                    if (!wide && !onPage && !onPlayer && !sheets.barHidden) PhoneBar(current ?: start, answer, navigator::go)
                }
            },
        ) { padding ->
            Row(Modifier.padding(padding).consumeWindowInsets(padding)) {
                if (wide && !fullWindow && !onPlayer) Rail(current, answer, entry?.destination?.route, navigator, routed)
                SharedTransitionLayout(if (removed) Modifier.stillFade(pageFade) else Modifier) {
                    CompositionLocalProvider(
                        LocalSharedTransitions provides this,
                        LocalSharedTransitionsView provides androidx.compose.ui.platform.LocalView.current,
                    ) {
                        NavHost(
                            nav,
                            startDestination = graphRoute(start),
                            enterTransition = { if (navigator.replacing) fadeIn(Motion.standard()) else if (switching()) fadeIn(Motion.quick()) else EnterTransition.None },
                            exitTransition = { if (navigator.replacing) fadeOut(Motion.standard()) else if (switching()) fadeOut(Motion.quick()) else ExitTransition.KeepUntilTransitionsFinished },
                            popEnterTransition = { if (switching()) fadeIn(Motion.quick()) else EnterTransition.None },
                            popExitTransition = { if (switching()) fadeOut(Motion.quick()) else ExitTransition.KeepUntilTransitionsFinished },
                        ) {
                            if (player != null) {
                                composable(PLAYER_ROUTE) {
                                    CompositionLocalProvider(LocalPageAnimation provides this) {
                                        val id = player.title
                                        Box(Modifier.fillMaxSize().then(if (id != null) Modifier.shared(Shared.Player, id) else Modifier)) {
                                            player.Content()
                                        }
                                    }
                                }
                            }
                            for (destination in Destination.entries) {
                                navigation(route = graphRoute(destination), startDestination = destination.route) {
                                    composable(destination.route) {
                                        CompositionLocalProvider(LocalPageAnimation provides this) {
                                            DestinationScreen(destination, routed, startSearch, startSheet, searchOpening) { searchOpening = false }
                                        }
                                    }
                                    composable(
                                        titlePattern(destination),
                                        arguments = listOf(navArgument("id") { type = NavType.LongType }),
                                    ) { page ->
                                        val id = page.arguments?.getLong("id")?.toUInt() ?: return@composable
                                        CompositionLocalProvider(LocalPageAnimation provides this) {
                                            TitleScreen(id, routed)
                                        }
                                    }
                                    composable(
                                        seriesPattern(destination),
                                        arguments = listOf(navArgument("work") { type = NavType.StringType }),
                                    ) { page ->
                                        val work = page.arguments?.getString("work")?.let(android.net.Uri::decode) ?: return@composable
                                        CompositionLocalProvider(LocalPageAnimation provides this) {
                                            app.kaeri.ui.reading.SeriesScreen(work, routed)
                                        }
                                    }
                                    // Watch together sits in Home's back stack, under its bar.
                                    if (destination == Destination.Home) {
                                        composable(TOGETHER_ROUTE) {
                                            CompositionLocalProvider(LocalPageAnimation provides this) { TogetherScreen() }
                                        }
                                    }
                                }
                            }
                            settingsPages(routed)
                            providerPages(routed)
                            storePages()
                            downloadPages(routed)
                            composable(LOBBY_ROUTE) {
                                CompositionLocalProvider(LocalPageAnimation provides this) { PartyLobbyScreen() }
                            }
                        }
                    }
                }
            }
        }
        if (firstLaunch) FirstLaunch(onCover = { covered = it })
        app.kaeri.ui.reading.ReadingAppHost(start, firstLaunch, covered)
        MatchPickerHost()
        SurpriseHost()
        EngineDown()
    }
}

@Composable
private fun DestinationScreen(
    destination: Destination,
    actions: AppActions,
    startSearch: SearchQuery?,
    startSheet: String?,
    searchOpening: Boolean,
    searchOpened: () -> Unit,
) {
    val navigator = LocalNavigator.current
    val answer = kaeriIsFor()
    when (destination) {
        Destination.Home -> if (answer == KaeriIsFor.READ) app.kaeri.ui.reading.ReadHomeScreen(actions) else HomeScreen(actions)
        Destination.Read -> app.kaeri.ui.reading.ReadHomeScreen(actions)
        Destination.Updates -> app.kaeri.ui.reading.UpdatesScreen(actions)
        Destination.Calendar -> CalendarScreen()
        Destination.Seasonal -> SeasonalScreen()
        Destination.Library -> LibraryScreen(actions)
        Destination.Search -> SearchScreen(startSearch, startFilters = startSheet == "filters", open = searchOpening, opened = searchOpened)
    }
}

/** The filled icons carry their own two colours; the rest take the item's. */
@Composable
private fun DestinationIcon(destination: Destination, selected: Boolean, answer: KaeriIsFor) {
    // Home beside Read is Watch, and draws a screen.
    val watch = destination == Destination.Home && answer == KaeriIsFor.BOTH
    val icon = if (watch) Icons.Watch else destination.icon
    val selectedIcon = if (watch) Icons.WatchSelected else destination.selectedIcon
    val filled = selected && selectedIcon != icon
    Icon(
        if (selected) selectedIcon else icon,
        contentDescription = null,
        tint = if (filled) Color.Unspecified else LocalContentColor.current,
    )
}

/**
 * A navigation bar label: one centred line, shrinking to fit as the text grows. A label that does
 * not fit one line at the smallest size breaks at its spaces and the bar grows to hold it; a
 * single word wider than the item still ends in an ellipsis.
 */
@Composable
internal fun BarLabel(text: String) {
    val style = LocalTextStyle.current
    val measurer = rememberTextMeasurer()
    val density = LocalDensity.current
    val centre = androidx.compose.ui.text.style.TextAlign.Center
    // The item's width, known after its first measure (the bar asks its items for intrinsic sizes,
    // which a subcomposing layout cannot answer).
    var available by remember { androidx.compose.runtime.mutableIntStateOf(0) }
    run {
        val largest = if (style.fontSize.isSpecified) style.fontSize else 12.sp
        val smallest = 8.sp
        val plan = remember(text, available, style, density) {
            if (available == 0) return@remember BarLabelPlan(largest, wraps = false)
            fun width(candidate: String, size: androidx.compose.ui.unit.TextUnit) =
                measurer.measure(candidate, style.copy(fontSize = size), softWrap = false, maxLines = 1).size.width
            var size = largest
            var chosen: androidx.compose.ui.unit.TextUnit? = null
            while (size >= smallest) {
                if (width(text, size) <= available) { chosen = size; break }
                size = (size.value - 0.5f).sp
            }
            when {
                chosen != null -> BarLabelPlan(chosen, wraps = false)
                text.split(' ').all { width(it, smallest) <= available } -> BarLabelPlan(smallest, wraps = true)
                else -> BarLabelPlan(smallest, wraps = false)
            }
        }
        Text(
            text,
            modifier = Modifier.fillMaxWidth().onSizeChanged { available = it.width },
            textAlign = centre,
            fontSize = plan.size,
            maxLines = if (plan.wraps) Int.MAX_VALUE else 1,
            softWrap = plan.wraps,
            overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis,
        )
    }
}

private data class BarLabelPlan(val size: androidx.compose.ui.unit.TextUnit, val wraps: Boolean)

/**
 * The phone's navigation bar: the Expressive short bar on surfaceContainer, the selected
 * destination in a primaryContainer pill with an amber label. It never moves and never takes part
 * in a transition.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun PhoneBar(current: Destination, answer: KaeriIsFor, go: (Destination) -> Unit) {
    val colors = MaterialTheme.colorScheme
    val owner = barOwner(current, answer)
    ShortNavigationBar(containerColor = colors.surfaceContainer) {
        for (destination in barDestinations(answer)) {
            val selected = destination == owner
            ShortNavigationBarItem(
                selected = selected,
                onClick = { go(destination) },
                icon = { DestinationIcon(destination, selected, answer) },
                // Five labels share the width: each shrinks to fit one line, then breaks at a
                // space and the bar grows.
                label = { BarLabel(stringResource(destination.labelFor(answer))) },
                colors = ShortNavigationBarItemDefaults.colors(
                    selectedIconColor = colors.onPrimaryContainer,
                    selectedTextColorTopIconPosition = colors.primary,
                    selectedIndicatorColor = colors.primaryContainer,
                    unselectedIconColor = colors.onSurfaceVariant,
                    unselectedTextColor = colors.onSurfaceVariant,
                ),
            )
        }
    }
}

/**
 * The drawer's pages beyond the five destinations (AT1): Downloads, then Providers and Plugin
 * stores under a Sources header.
 */
internal enum class DrawerPage(@StringRes val label: Int, val icon: ImageVector, val route: String) {
    Downloads(R.string.nav_downloads, Icons.DownloadsPage, DOWNLOADS_ROUTE),
    Providers(R.string.sources_title, Icons.Providers, PROVIDERS_ROUTE),
    Stores(R.string.nav_plugin_stores, Icons.Stores, STORES_ROUTE),
}

/**
 * The tablet's rail: 96 wide on surface, the five destinations, and the account at its foot,
 * which opens Settings. Its menu button expands it into the modal drawer (AT1), 220 wide on
 * surfaceContainerLow over the scrim, which adds Downloads, the Sources header with Providers and
 * Plugin stores, and "Accounts and settings" at the foot. Back, the scrim, the button or a pick
 * closes it; its motion is the component's own.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun Rail(current: Destination?, answer: KaeriIsFor, route: String?, navigator: Navigator, actions: AppActions) {
    val colors = MaterialTheme.colorScheme
    val store = LocalAppStore.current
    val trackers by store.accounts.trackers.collectAsStateWithLifecycle()
    val user = trackers.values.firstOrNull { it.connected }?.user
    val rail = rememberWideNavigationRailState()
    val scope = rememberCoroutineScope()
    val expanded = rail.targetValue == WideNavigationRailValue.Expanded
    val close = { scope.launch { rail.collapse() } }
    val itemColors = WideNavigationRailItemDefaults.colors(
        selectedIconColor = colors.onPrimaryContainer,
        selectedTextColorTopIconPosition = colors.primary,
        selectedTextColorStartIconPosition = colors.primary,
        selectedIndicatorColor = colors.primaryContainer,
        unselectedIconColor = colors.onSurfaceVariant,
        unselectedTextColor = colors.onSurfaceVariant,
    )
    Box(Modifier.fillMaxHeight()) {
        ModalWideNavigationRail(
            state = rail,
            hideOnCollapse = false,
            expandedShape = RoundedCornerShape(topEnd = 16.dp, bottomEnd = 16.dp),
            colors = WideNavigationRailDefaults.colors(
                containerColor = colors.surface,
                modalContainerColor = colors.surfaceContainerLow,
                modalScrimColor = androidx.compose.material3.MaterialTheme.colorScheme.scrim.copy(alpha = 0.32f),
            ),
            header = {
                val collapseLabel = stringResource(if (expanded) R.string.nav_collapse else R.string.nav_expand)
                IconButton(
                    onClick = { scope.launch { rail.toggle() } },
                    modifier = Modifier.semantics { contentDescription = collapseLabel },
                ) {
                    Icon(if (expanded) Icons.MenuOpen else Icons.Menu, contentDescription = null)
                }
            },
        ) {
            val owner = current?.let { barOwner(it, answer) }
            for (destination in barDestinations(answer)) {
                val selected = destination == owner
                WideNavigationRailItem(
                    selected = selected,
                    onClick = {
                        close()
                        navigator.go(destination)
                    },
                    icon = { DestinationIcon(destination, selected, answer) },
                    label = { Text(stringResource(destination.labelFor(answer))) },
                    railExpanded = expanded,
                    colors = itemColors,
                )
            }
            if (expanded) {
                val downloads = DrawerPage.Downloads
                DrawerItem(downloads, route, itemColors) { close(); navigator.open(downloads.route) }
                Text(
                    stringResource(R.string.sources_title),
                    style = MaterialTheme.typography.labelLarge,
                    color = colors.onSurfaceVariant,
                    modifier = Modifier.padding(start = 28.dp, top = 8.dp).height(32.dp).semantics { heading() },
                )
                for (page in listOf(DrawerPage.Providers, DrawerPage.Stores)) {
                    DrawerItem(page, route, itemColors) { close(); navigator.open(page.route) }
                }
                Spacer(Modifier.height(40.dp))
                WideNavigationRailItem(
                    selected = false,
                    onClick = {
                        close()
                        actions.openSettings()
                    },
                    icon = { RailAvatar(user, 32) },
                    label = { Text(stringResource(R.string.nav_accounts_and_settings)) },
                    railExpanded = true,
                    colors = itemColors,
                )
            }
        }
        if (!expanded) {
            val accountsAndSettings = user?.let { stringResource(R.string.nav_accounts_and_settings_user, it) } ?: stringResource(R.string.nav_accounts_and_settings)
            IconButton(
                onClick = actions.openSettings,
                modifier = Modifier
                    .align(Alignment.BottomCenter)
                    .padding(bottom = 16.dp)
                    .semantics { contentDescription = accountsAndSettings },
            ) {
                RailAvatar(user, 40)
            }
        }
    }
}

/** One of the drawer's own pages, selected while it is on screen. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun DrawerItem(page: DrawerPage, route: String?, colors: androidx.compose.material3.NavigationItemColors, onClick: () -> Unit) {
    WideNavigationRailItem(
        selected = route == page.route && !page.route.startsWith(SETTINGS_ROUTE),
        onClick = onClick,
        icon = { Icon(page.icon, contentDescription = null) },
        label = { Text(stringResource(page.label)) },
        railExpanded = true,
        colors = colors,
    )
}

/** The signed-in user's initial on a raised circle, or the account glyph signed out. */
@Composable
private fun RailAvatar(user: String?, size: Int) {
    if (user == null) {
        Icon(Icons.Account, contentDescription = null)
    } else {
        Box(Modifier.size(size.dp).background(MaterialTheme.colorScheme.surfaceContainerHighest, CircleShape), contentAlignment = Alignment.Center) {
            Text(user.take(1).uppercase(), style = if (size >= 40) MaterialTheme.typography.bodyLarge else MaterialTheme.typography.labelLarge)
        }
    }
}

/** The place a crash record names for a route: Home, the player, a title, Search or Settings. */
internal fun crashPlaceOf(route: String?): app.kaeri.engine.model.CrashPlace = when {
    route == null -> app.kaeri.engine.model.CrashPlace.OTHER
    route == PLAYER_ROUTE -> app.kaeri.engine.model.CrashPlace.PLAYER
    route.contains("/title/") || route.contains("/series/") -> app.kaeri.engine.model.CrashPlace.TITLE
    route.startsWith("search") -> app.kaeri.engine.model.CrashPlace.SEARCH
    route.startsWith("settings") -> app.kaeri.engine.model.CrashPlace.SETTINGS
    route.startsWith("continue") || route.startsWith("seasonal") || route.startsWith("calendar") || route.startsWith("library") || route.startsWith("read") || route.startsWith("updates") -> app.kaeri.engine.model.CrashPlace.HOME
    else -> app.kaeri.engine.model.CrashPlace.OTHER
}
