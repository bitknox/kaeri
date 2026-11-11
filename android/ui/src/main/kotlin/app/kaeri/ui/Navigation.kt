// Where the screens go: each destination's own back stack with title pages pushed on it, and the
// shared elements that carry a cover or banner into its title page (T1).

package app.kaeri.ui

import android.view.View
import androidx.compose.animation.AnimatedVisibilityScope
import androidx.compose.animation.BoundsTransform
import androidx.compose.animation.ExperimentalSharedTransitionApi
import androidx.compose.animation.SharedTransitionScope
import androidx.compose.animation.fadeIn
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.Transition
import androidx.compose.animation.core.VisibilityThreshold
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.isSpecified
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalView
import androidx.navigation.NavBackStackEntry
import androidx.navigation.NavGraph.Companion.findStartDestination
import androidx.navigation.NavHostController
import app.kaeri.design.Motion
import app.kaeri.engine.core.AnilistId
import app.kaeri.engine.core.WorkId

/** A title page's route inside [destination]'s back stack. */
internal fun titleRoute(destination: Destination, id: AnilistId) = "${destination.route}/title/$id"

/** The route pattern of the title page in [destination]'s back stack. */
internal fun titlePattern(destination: Destination) = "${destination.route}/title/{id}"

/** A reading series page's route inside [destination]'s back stack. */
internal fun seriesRoute(destination: Destination, work: WorkId) = "${destination.route}/series/${android.net.Uri.encode(work)}"

/** The route pattern of the series page in [destination]'s back stack. */
internal fun seriesPattern(destination: Destination) = "${destination.route}/series/{work}"

/** The route pattern of a reading page pushed onto [destination]'s back stack: View all, e-ink Browse. */
internal fun readPagePattern(destination: Destination) = "${destination.route}/read/{page}"

/** The graph that holds [destination]'s back stack. */
internal fun graphRoute(destination: Destination) = "${destination.route}-tab"

/** The top-level destination an entry belongs to, by the graph it sits in. */
internal fun NavBackStackEntry.destinationOf(): Destination? =
    destination.parent?.route?.removeSuffix("-tab")?.let(Destination::named)

/**
 * Whether the entry is a page beyond the five destinations, Settings and what it opens, which
 * sits outside every destination's graph.
 */
internal val NavBackStackEntry.isPage: Boolean get() = destinationOf() == null

/**
 * Whether the entry takes the whole window, with no navigation bar or rail: Settings, as
 * AndroidSettings and AndroidTabletSettings draw it.
 */
internal val NavBackStackEntry.isFullWindow: Boolean
    get() = isPage && destination.route?.startsWith(SETTINGS_ROUTE) == true

/**
 * What the screens navigate with: the destinations, title pages, and which element a title page
 * was opened from, so that only that element flies into the page and back.
 */
@Stable
class Navigator(private val nav: NavHostController, private val start: Destination) {
    /**
     * The element a title page was last opened from, as `<where>:<id>`: only it carries the shared
     * keys, since the same title can show twice on one screen and a key must be unique there.
     */
    var source: String? by mutableStateOf(null)
        private set

    /** The destination on screen. */
    fun current(): Destination = nav.currentBackStackEntry?.destinationOf() ?: start

    /** Open [id]'s page in the current destination's back stack, flying from [from]. */
    fun openTitle(id: AnilistId, from: String) {
        source = "$from:$id"
        nav.navigate(titleRoute(current(), id))
    }

    /** Open a reading series' page in the current destination's back stack, its cover flying from [from]. */
    fun openSeries(work: WorkId, from: String) {
        source = "$from:$work"
        nav.navigate(seriesRoute(current(), work))
    }

    /** Whether a title page is being swapped for another in place (Another): the pages crossfade. */
    var replacing: Boolean by mutableStateOf(false)
        private set

    /**
     * Another: [id]'s page takes the place of the title page on screen, so Back returns to where
     * Surprise me was pressed. The two crossfade; nothing flies.
     */
    fun replaceTitle(id: AnilistId) {
        val here = current()
        source = null
        replacing = true
        nav.navigate(titleRoute(here, id)) { popUpTo(titlePattern(here)) { inclusive = true } }
    }

    /** The container the host is drawn in, for a still of the page being replaced (Another with Remove animations on). */
    var fade: app.kaeri.design.StillFade? = null

    /** The swap has settled. */
    fun replaced() {
        replacing = false
    }

    /**
     * Switch to [destination] as it was left; on the current one, back to its start, scrolled to
     * the top by the screen.
     */
    fun go(destination: Destination) {
        if (destination == current()) {
            nav.popBackStack(destination.route, inclusive = false)
            return
        }
        nav.navigate(graphRoute(destination)) {
            popUpTo(nav.graph.findStartDestination().id) { saveState = true }
            launchSingleTop = true
            restoreState = true
        }
    }

    /**
     * Switch to [destination] at its start, as a link into it from another destination opens it:
     * the pages left open on it last time are closed.
     */
    fun goToStart(destination: Destination) {
        go(destination)
        nav.popBackStack(destination.route, inclusive = false)
    }

    fun back() {
        nav.popBackStack()
    }

    /** Open Settings at [pane], or at its list. */
    fun openSettings(pane: SettingsPane? = null) {
        nav.navigate(settingsRoute(pane)) { launchSingleTop = true }
    }

    /** The destination the last page pushed with [open] was opened from: Downloads puts its medium first. */
    var openedFrom: Destination? by mutableStateOf(null)
        private set

    /** Push the page at [route] over whatever is on screen. */
    /** Push a reading page ([page] names it) onto the current destination's back stack. */
    internal fun openReadPage(page: String) {
        nav.navigate("${current().route}/read/${android.net.Uri.encode(page)}")
    }

    fun open(route: String) {
        nav.currentBackStackEntry?.destinationOf()?.let { openedFrom = it }
        nav.navigate(route) { launchSingleTop = true }
    }
}

/** Open Downloads over whatever is on screen, from the player's offline start. */
fun Navigator.openDownloads() = open(DOWNLOADS_ROUTE)

/** Open the plugin stores page, from a start that has no source to ask. */
fun Navigator.openStores() = open(STORES_ROUTE)

val LocalNavigator = staticCompositionLocalOf<Navigator> { error("no Navigator in this composition") }

/** The same navigator, or null where none is provided (the player on its own, in a test). */
val LocalNavigatorOrNull = staticCompositionLocalOf<Navigator?> { null }

/** The layout the shared elements fly in, around the NavHost. */
@OptIn(ExperimentalSharedTransitionApi::class)
val LocalSharedTransitions = compositionLocalOf<SharedTransitionScope?> { null }

/**
 * The window [LocalSharedTransitions] lays out in. A dialog or popup drawn from a screen, such as
 * Search's full-screen field, is a window of its own: the composition locals reach it, but the
 * layout's lookahead does not, and a shared element measured there throws.
 */
internal val LocalSharedTransitionsView = staticCompositionLocalOf<View?> { null }

/** The NavHost entry a screen is drawn in, which its shared elements animate with. */
val LocalPageAnimation = compositionLocalOf<AnimatedVisibilityScope?> { null }

/** The three things that fly into a title page, by the keys motion-android.md gives them. */
enum class Shared(internal val prefix: String) {
    /**
     * The page's bounds, growing out of the element tapped. They fly as two layers, the page's
     * ground and its content, so that a banner flying with them draws between the two.
     */
    Page("page"),

    /** The banner art. */
    Art("art"),

    /** The poster, drawn above the flying art. */
    Cover("cover"),

    /** The player's bounds, growing out of the banner it was opened from and closing into it (T2, T3). */
    Player("player"),
}

/**
 * This element as one end of [shared]'s flight for [id]. On a source screen pass [from], where
 * the element is (`hero`, `season` …): it joins only when it is the one the page was opened from.
 * On the title page pass none.
 */
@Composable
fun Modifier.shared(shared: Shared, id: AnilistId, from: String? = null, ground: Color = Color.Unspecified): Modifier =
    sharedKeyed(shared, id.toString(), from, ground)

/** [shared] for a reading series, by its work id: its cover flies into the series page and back. */
@Composable
fun Modifier.sharedWork(shared: Shared, work: WorkId, from: String? = null, ground: Color = Color.Unspecified): Modifier =
    sharedKeyed(shared, work, from, ground)

@OptIn(ExperimentalSharedTransitionApi::class)
@Composable
private fun Modifier.sharedKeyed(shared: Shared, id: String, from: String?, ground: Color): Modifier {
    val layout = LocalSharedTransitions.current
    val page = LocalPageAnimation.current
    val outside = LocalView.current !== LocalSharedTransitionsView.current
    // On an e-ink panel nothing flies: the page is there at once, as its NavHost puts it.
    val paper = app.kaeri.ui.reading.LocalEinkLayout.current != null
    if (layout == null || page == null || outside || paper) return if (ground.isSpecified) background(ground) else this
    if (from != null && LocalNavigator.current.source != "$from:$id") return this
    val bounds = flightBounds(page)
    return with(layout) {
        val state = rememberSharedContentState("${shared.prefix}-$id")
        when (shared) {
            // One sharedBounds draws all it holds at one depth in the overlay, so the page flies
            // as two: its ground ([ground], painted on the title page) and, nested in it, its
            // content. The art then draws between them, under the title and the scrim. What the
            // page grows out of draws under the ground, which covers it as the page fades in.
            Shared.Page -> sharedBounds(
                rememberSharedContentState("ground-$id"),
                page,
                enter = fadeIn(Motion.fadeIn()),
                exit = fadeOut(Motion.fadeOut()),
                boundsTransform = bounds,
                resizeMode = SharedTransitionScope.ResizeMode.scaleToBounds(ContentScale.FillWidth, Alignment.TopCenter),
                zIndexInOverlay = GROUND_Z,
            )
                .then(if (ground.isSpecified) Modifier.background(ground) else Modifier)
                .sharedBounds(
                    state,
                    page,
                    enter = fadeIn(Motion.fadeIn()),
                    exit = fadeOut(Motion.fadeOut()),
                    boundsTransform = bounds,
                    resizeMode = SharedTransitionScope.ResizeMode.scaleToBounds(ContentScale.FillWidth, Alignment.TopCenter),
                    zIndexInOverlay = if (from != null) SOURCE_Z else 0f,
                )
            Shared.Player -> sharedBounds(
                state,
                page,
                enter = fadeIn(Motion.fadeIn()),
                exit = fadeOut(Motion.fadeOut()),
                boundsTransform = bounds,
                resizeMode = SharedTransitionScope.ResizeMode.scaleToBounds(ContentScale.FillWidth, Alignment.Center),
            )
            Shared.Art -> sharedElement(state, page, boundsTransform = bounds, zIndexInOverlay = ART_Z)
            // The poster overlaps the banner, so it draws above the flying art.
            Shared.Cover -> sharedElement(state, page, boundsTransform = bounds, zIndexInOverlay = COVER_Z)
        }
    }
}

/**
 * How the elements fly: on the shared elements' own spring, or, while a back swipe seeks the
 * flight, on a straight line in time, so the page shrinks toward its cover as far as the finger
 * has gone rather than most of the way in the first part of the swipe.
 */
@OptIn(ExperimentalSharedTransitionApi::class)
private fun flightBounds(page: AnimatedVisibilityScope) = BoundsTransform { _, _ ->
    if (page.transition.seeking()) SEEKED_FLIGHT else FLIGHT
}

/** Whether a back swipe is seeking this transition or the NavHost's it belongs to. */
private fun Transition<*>.seeking(): Boolean = isSeeking || parentTransition?.seeking() == true

/** The shared elements' own spring, Compose's default for them. */
private val FLIGHT = spring(stiffness = 400f, visibilityThreshold = Rect.VisibilityThreshold)

/** A seeked flight, as long as a seeked page's slide (AppRoot's back swipe). */
private val SEEKED_FLIGHT = tween<Rect>(300, easing = LinearEasing)

/**
 * The page's own [ground] behind what this holds, except while the page flies: then the flight's
 * ground layer paints it, under the art, and this one would hide the art.
 */
@OptIn(ExperimentalSharedTransitionApi::class)
@Composable
fun Modifier.pageGround(ground: Color): Modifier {
    val layout = LocalSharedTransitions.current
    // Read as the layer draws, so the flight starting and ending redraws without recomposing.
    return drawBehind { if (layout?.isTransitionActive != true) drawRect(ground) }
}

// Depths in the overlay, back to front: the source the page grows out of, the ground, the art,
// the page's content, the poster.
private const val SOURCE_Z = -3f
private const val GROUND_Z = -2f
private const val ART_Z = -1f
private const val COVER_Z = 1f
