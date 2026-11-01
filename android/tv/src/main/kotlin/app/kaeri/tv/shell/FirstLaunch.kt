// First launch on TV: the spec's three pages, Welcome, AniList and Sources, laid out for focus.
// Each page has one focused way on and one way past; every step is in Settings afterwards. Once
// through they do not come back, and a TV that already has an account or a store never sees them.

package app.kaeri.tv.shell

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import kotlinx.coroutines.delay
import app.kaeri.design.Nagare
import app.kaeri.design.ObiSilk
import app.kaeri.i18n.R as I18nR
import app.kaeri.store.AppStore
import app.kaeri.store.DevicePreferences
import app.kaeri.tv.signin.JoinStoreScreen
import app.kaeri.tv.signin.SignInScreen
import app.kaeri.tv.ui.AmberButton
import app.kaeri.tv.ui.TonalButton
import app.kaeri.tv.ui.ButtonLabel
import app.kaeri.tv.ui.Canvas
import app.kaeri.tv.ui.LocalAppStore
import app.kaeri.tv.ui.enterFade
import app.kaeri.tv.ui.entryFocus
import app.kaeri.tv.ui.exitFade

/** The pages, in order. */
enum class FirstLaunchPage { Welcome, AniList, Sources }

/** The preference that says the pages have been through, on this TV: the phone's name for it. */
const val FIRST_LAUNCH_DONE = "firstLaunchDone"

/** How long the pages wait for the engine to say the accounts and stores before deciding. */
private const val DECIDE_MILLIS = 3_000L

/**
 * The pages over the app while this TV has not been through them; [onCover] hears whether they
 * cover it. Until the engine has said whether anything is set up, the ground shows plain.
 */
@Composable
fun FirstLaunch(onCover: (Boolean) -> Unit, start: FirstLaunchPage = FirstLaunchPage.Welcome) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val preferences = remember { DevicePreferences.of(context) }
    var done by rememberSaveable { mutableStateOf(preferences.getBoolean(FIRST_LAUNCH_DONE, false)) }
    val phase by store.phase.collectAsStateWithLifecycle()
    val stopped = phase is AppStore.Phase.Failed || phase is AppStore.Phase.Stopped
    val covering = !done && !stopped
    LaunchedEffect(covering) { onCover(covering) }
    if (!covering) return
    val finish = {
        preferences.edit().putBoolean(FIRST_LAUNCH_DONE, true).apply()
        done = true
    }
    val trackers by store.accounts.trackers.collectAsStateWithLifecycle()
    val stores by store.stores.stores.collectAsStateWithLifecycle()
    val storesLoaded by store.stores.loaded.collectAsStateWithLifecycle()
    var waited by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) {
        store.loadStores()
        delay(DECIDE_MILLIS)
        waited = true
    }
    val setUp = trackers.values.any { it.connected } || stores.isNotEmpty()
    var showing by rememberSaveable { mutableStateOf(false) }
    LaunchedEffect(storesLoaded, waited, setUp) {
        when {
            showing -> {}
            // Set up already, as after an update: the pages have nothing to offer.
            setUp -> finish()
            storesLoaded || waited -> showing = true
        }
    }
    Box(Modifier.fillMaxSize().background(ObiSilk.Ground)) {
        if (showing) Pages(start, finish)
    }
}

@Composable
private fun Pages(start: FirstLaunchPage, finish: () -> Unit) {
    var page by rememberSaveable { mutableStateOf(start) }
    BackHandler(enabled = page == FirstLaunchPage.Sources) { page = FirstLaunchPage.AniList }
    val enter = enterFade()
    val exit = exitFade()
    AnimatedContent(page, transitionSpec = { enter togetherWith exit }, label = "first launch") { shown -> // i18n-skip
        when (shown) {
            FirstLaunchPage.Welcome -> Welcome(onNext = { page = FirstLaunchPage.AniList })
            // Signed in or Not now, Sources follows.
            FirstLaunchPage.AniList -> SignInScreen("anilist", onDone = { page = FirstLaunchPage.Sources }, cancelLabel = stringResource(I18nR.string.onboarding_not_now))
            FirstLaunchPage.Sources -> SourcesPage(onDone = { finish() })
        }
    }
}

/** Welcome: Nagare, what Kaeri is, and Continue focused. */
@Composable
private fun Welcome(onNext: () -> Unit) {
    val next = remember { FocusRequester() }
    Row(
        Modifier
            .entryFocus(next)
            .fillMaxSize()
            .padding(start = Canvas.Keyline, end = Canvas.SafeX, top = 120.dp),
        horizontalArrangement = Arrangement.spacedBy(64.dp),
    ) {
        Box(Modifier.width(160.dp)) { Nagare(cell = 6) }
        Column(verticalArrangement = Arrangement.spacedBy(16.dp), modifier = Modifier.widthIn(max = 560.dp)) {
            Text("Kaeri", style = MaterialTheme.typography.displaySmall, color = ObiSilk.Silk, modifier = Modifier.semantics { heading() }) // i18n-skip
            Text(stringResource(I18nR.string.onboarding_tagline), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk)
            AmberButton(onClick = onNext, icon = null, modifier = Modifier.padding(top = 8.dp).focusRequester(next)) { ButtonLabel(stringResource(I18nR.string.onboarding_continue)) }
        }
    }
}

/**
 * Where episodes come from: the demo is the one amber button and has the focus. Have an invite
 * link? offers Scan with your phone, which opens the QR page, and Skip for now is the last item.
 */
@Composable
private fun SourcesPage(onDone: () -> Unit) {
    val store = LocalAppStore.current
    var joining by rememberSaveable { mutableStateOf(false) }
    if (joining) {
        JoinStoreScreen(
            onDone = { joined -> if (joined) onDone() else joining = false },
            cancelLabel = stringResource(I18nR.string.action_cancel),
            title = stringResource(I18nR.string.onboarding_invite_heading),
        )
        return
    }
    val demo = remember { FocusRequester() }
    Column(
        Modifier
            .entryFocus(demo)
            .fillMaxSize()
            .padding(start = Canvas.Keyline, end = Canvas.SafeX, top = 72.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Text(stringResource(I18nR.string.onboarding_where_title), style = MaterialTheme.typography.titleLarge, color = ObiSilk.Silk, modifier = Modifier.semantics { heading() })
        Column(Modifier.widthIn(max = 640.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(stringResource(I18nR.string.onboarding_where_hosts), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk)
            Text(stringResource(I18nR.string.onboarding_where_stores), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze)
        }
        Text(stringResource(I18nR.string.onboarding_demo_heading), style = MaterialTheme.typography.titleMedium, color = ObiSilk.Silk, modifier = Modifier.padding(top = 8.dp))
        Text(stringResource(I18nR.string.onboarding_demo_detail), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze, modifier = Modifier.widthIn(max = 640.dp))
        AmberButton(
            onClick = {
                store.playDemo()
                onDone()
            },
            icon = null,
            modifier = Modifier.focusRequester(demo),
        ) { ButtonLabel(stringResource(I18nR.string.onboarding_play_demo)) }
        Text(stringResource(I18nR.string.onboarding_invite_heading), style = MaterialTheme.typography.titleMedium, color = ObiSilk.Silk, modifier = Modifier.padding(top = 8.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            TonalButton(onClick = { joining = true }) { ButtonLabel(stringResource(I18nR.string.onboarding_invite_scan)) }
            TonalButton(onClick = onDone) { ButtonLabel(stringResource(I18nR.string.onboarding_skip_for_now)) }
        }
    }
}
