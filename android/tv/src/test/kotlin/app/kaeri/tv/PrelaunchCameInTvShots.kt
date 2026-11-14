// Prelaunch board piece 1 on TV, "Say what came in, or why nothing did": the store form after
// joining, states 11 to 15, and Your sources with the notice, state 16, at the board's 960 × 540,
// each staged as the Fixtures app's `prelaunch` extra stages it.

package app.kaeri.tv

import android.content.ComponentName
import androidx.activity.ComponentActivity
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onRoot
import app.kaeri.player.PlayerModel
import app.kaeri.player.ScriptedMpv
import app.kaeri.store.AppClock
import app.kaeri.store.AppStore
import app.kaeri.store.PrelaunchCameIn
import app.kaeri.testkit.FixtureArt
import app.kaeri.testkit.Scene
import app.kaeri.testkit.captureRoboImage
import app.kaeri.tv.screens.SettingsSection
import app.kaeri.tv.shell.Destination
import app.kaeri.tv.shell.TvNavigator
import app.kaeri.tv.shell.TvRoot
import app.kaeri.tv.ui.TV_ART_BUDGET
import app.kaeri.tv.ui.TvArtDecoder
import app.kaeri.tv.ui.TvTheme
import com.github.takahirom.roborazzi.roborazziSystemPropertyOutputDirectory
import java.time.Clock
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Rule
import org.junit.Test
import org.junit.rules.ExternalResource
import org.junit.rules.RuleChain
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@OptIn(com.github.takahirom.roborazzi.ExperimentalRoborazziApi::class)
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = TvShots.TV)
class PrelaunchCameInTvShots {
    val compose = createAndroidComposeRule<ComponentActivity>()

    @get:Rule
    val rules: RuleChain = RuleChain.outerRule(
        object : ExternalResource() {
            override fun before() {
                val app = RuntimeEnvironment.getApplication()
                shadowOf(app.packageManager).addActivityIfNotPresent(ComponentName(app, ComponentActivity::class.java))
            }
        },
    ).around(compose)

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)

    @After
    fun tearDown() = scope.cancel()

    /** [n] staged over Settings: the join page for 11 to 15, Sources for 16. */
    private fun shoot(n: Int) {
        val recorded = Scene.named(TvShots.WATCHING)
        AppClock.clock = Clock.fixed(recorded.recordedAt, TvShots.ZONE)
        val store = AppStore(scope, decoder = TvArtDecoder, artworkBudget = TV_ART_BUDGET, clock = { 0L }, artworkWithoutEngine = FixtureArt::bytes)
        val events = recorded.events.filter { TvShots.replayed(it) }
        store.apply(events)
        val player = PlayerModel(send = {}, backend = { ScriptedMpv() }, scope = scope, now = { 0L }, hwdec = "no")
        val nav = TvNavigator(Destination.Home)
        compose.setContent { TvTheme { TvRoot(store, player, nav, play = { _, _ -> }) } }
        compose.waitForIdle()
        store.apply(events)
        settle()
        runBlocking { check(PrelaunchCameIn.stage(store, "p1-tv-settings-%02d".format(n))) }
        compose.runOnUiThread { if (n == 16) nav.goSettings(SettingsSection.Sources.name) else nav.openJoinStore() }
        // The motion settles; a notice is still up at 1.5 s of its time.
        repeat(3) {
            compose.mainClock.advanceTimeBy(500)
            compose.waitForIdle()
        }
        compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/pl-p1-tv-%02d.png".format(n))
    }

    private fun settle() {
        compose.mainClock.advanceTimeBy(3_500)
        compose.waitForIdle()
    }

    @Test fun p1_tv_11() = shoot(11)
    @Test fun p1_tv_12() = shoot(12)
    @Test fun p1_tv_13() = shoot(13)
    @Test fun p1_tv_14() = shoot(14)
    @Test fun p1_tv_15() = shoot(15)
    @Test fun p1_tv_16() = shoot(16)
}
