// Home before anything says what Kaeri is for: titled Home over the loading column every setup
// shares, as the launch rail board draws it. When the answer comes, the setup's Home takes over
// the wait as it stands: its placeholders and activity line stay, and nothing starts again.

package app.kaeri.ui

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.core.MutableTransitionState
import androidx.compose.animation.fadeIn
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.design.Motion
import app.kaeri.eink.EinkType
import app.kaeri.eink.PaperWaiting
import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.ui.reading.LocalEinkLayout
import app.kaeri.ui.reading.PaperPage
import app.kaeri.ui.reading.ReadHomeScreen
import kotlinx.coroutines.delay

/** How long Home waits before its activity line says it in words, in every setup. */
internal const val HOME_ACTIVITY_AFTER = 4_000L

/** Home's wait before the answer, as far as it got: what the setup's Home takes over. */
internal class HomeWait {
    /** Whether Home opened before the answer came. */
    var waited = false

    /** Whether the placeholders stand: past 600 ms, or the paper's Loading line. */
    var placeholders = false

    /** Whether the activity line stands: past 4 s. */
    var activity = false
}

/** What the setup's Home carries on from the wait, read once as it opens. */
internal data class HomeHandover(
    /** Home opened before the answer, so what the answer brings comes in with the rail. */
    val arriving: Boolean = false,
    /** The placeholders already stand: they stay, with no new wait and no fade. */
    val placeholders: Boolean = false,
    /** The activity line already stands: it stays while Home still waits. */
    val activity: Boolean = false,
)

internal val LocalHomeWait = staticCompositionLocalOf<HomeWait?> { null }

/** What the Home on screen takes over from the wait, as it was when this Home opened. */
@Composable
internal fun homeHandover(): HomeHandover {
    val wait = LocalHomeWait.current
    return remember { wait?.let { HomeHandover(it.waited, it.placeholders, it.activity) } ?: HomeHandover() }
}

/**
 * The Home destination: the wait while nothing says what Kaeri is for, then reading's Home with
 * Read alone and the Watch Home otherwise, which take the wait over.
 */
@Composable
internal fun HomeDestination(actions: AppActions) {
    val answer = knownKaeriIsFor()
    val wait = remember { HomeWait() }
    CompositionLocalProvider(LocalHomeWait provides wait) {
        when (answer) {
            null -> WaitingHome(actions, wait)
            KaeriIsFor.READ -> ReadHomeScreen(actions)
            else -> HomeScreen(actions)
        }
    }
}

/**
 * Home with no answer yet: the frame titled Home with no actions over the shared loading column,
 * the placeholders from 600 ms and Loading suggestions from 4 s. On paper, the paper Home's own
 * wait: its title, and Loading after 600 ms.
 */
@Composable
private fun WaitingHome(actions: AppActions, wait: HomeWait) {
    val context = LocalContext.current
    SideEffect { wait.waited = true }
    if (LocalEinkLayout.current != null) {
        PaperPage(back = null, title = Strings.homeTitle(context)) {
            PaperWaiting(onShown = { wait.placeholders = true }) {
                Text(Strings.stateLoading(context), style = EinkType.Body, modifier = Modifier.padding(top = 16.dp))
            }
        }
        return
    }
    val store = LocalAppStore.current
    val trackers by store.accounts.trackers.collectAsStateWithLifecycle()
    val user = trackers.values.firstOrNull { it.connected }?.user
    var slow by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) {
        delay(HOME_ACTIVITY_AFTER)
        slow = true
        wait.activity = true
    }
    ScreenFrame(
        title = stringResource(R.string.home_title),
        // Wide windows have the account at the foot of the rail already.
        account = if (wideWindow()) null else profileAccount(user, actions.openSettings),
        ownLine = true,
        activity = if (slow) Strings.homeLoading(context) else null,
    ) { padding ->
        LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(top = padding.calculateTopPadding(), bottom = 24.dp), userScrollEnabled = false) {
            item(key = "loading") { AfterWait(onShown = { wait.placeholders = true }) { HomeLoadingColumn() } }
        }
    }
}

/** Home's loading column in every setup: the spotlight's plate and lines, then one shelf. */
@Composable
internal fun HomeLoadingColumn() {
    Column {
        SpotlightPlaceholder()
        ShelfPlaceholder()
    }
}

/** A shelf before it arrives: its heading's bar and four covers' plates. */
@Composable
internal fun ShelfPlaceholder() {
    Column(Modifier.padding(top = shelfTop())) {
        TextPlaceholder(0.3f, 48.dp, Modifier.padding(start = 16.dp))
        LazyRow(
            userScrollEnabled = false,
            contentPadding = PaddingValues(horizontal = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(cardGap()),
        ) {
            items(4) { Box(Modifier.width(posterWidth())) { PosterPlaceholder() } }
        }
    }
}

/**
 * [content], coming in on the standard spring when the answer has just [arriving] with the rail's
 * destinations, else there at once.
 */
@Composable
internal fun Arriving(arriving: Boolean, content: @Composable () -> Unit) {
    if (!arriving) {
        content()
        return
    }
    val shown = remember { MutableTransitionState(false).apply { targetState = true } }
    AnimatedVisibility(shown, enter = fadeIn(Motion.standard()), exit = ExitTransition.None) { content() }
}
