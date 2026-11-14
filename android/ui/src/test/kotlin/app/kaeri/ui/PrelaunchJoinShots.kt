package app.kaeri.ui

import android.view.ViewGroup
import androidx.activity.ComponentActivity
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onRoot
import androidx.core.graphics.Insets
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import app.kaeri.design.KaeriTheme
import app.kaeri.i18n.R
import app.kaeri.store.AppHost
import app.kaeri.store.AppStore
import app.kaeri.store.EngineHome
import app.kaeri.store.EngineLaunch
import app.kaeri.store.FixtureScene
import app.kaeri.store.MemorySecrets
import app.kaeri.store.PrelaunchJoin
import app.kaeri.testkit.captureRoboImage
import app.kaeri.testkit.captureScreenRoboImage
import com.github.takahirom.roborazzi.roborazziSystemPropertyOutputDirectory
import java.io.File
import java.nio.file.Files
import kotlin.math.roundToInt
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Test
import org.robolectric.annotation.Config
import org.robolectric.shadows.ShadowLooper

/**
 * Prelaunch board piece 3, "Already use Kaeri on another device?", first run's page 2 of 5, on
 * the phone and the tablet: each state staged as the Fixtures app's `prelaunch` extra stages it.
 */
@OptIn(com.github.takahirom.roborazzi.ExperimentalRoborazziApi::class)
class PrelaunchJoinShots : ReadingShotHarness() {
    private val home: File = Files.createTempDirectory("kaeri-join-shots").toFile()

    @After
    fun dropHome() {
        home.deleteRecursively()
    }

    private fun run(frames: Int) {
        repeat(frames) {
            ShadowLooper.idleMainLooper()
            compose.mainClock.advanceTimeBy(50)
            Thread.sleep(10)
        }
    }

    /**
     * The state [state] staged on reading's first-run scene, over first launch's pages, or on
     * Settings' Devices page for the other device's side.
     */
    private fun snap(name: String, state: String, other: Boolean = false) {
        val store = AppStore(scope)
        this.store = store
        store.start(EngineLaunch(EngineLaunch.Engine.Fixtures(FixtureScene.READING_FIRST_RUN), EngineHome.override(home)), AppHost(MemorySecrets()))
        compose.runOnUiThread { WindowCompat.setDecorFitsSystemWindows(compose.activity.window, false) }
        compose.setContent {
            KaeriTheme {
                if (other) AppRoot(store, Destination.Home, startPage = "settings/devices") else AppRoot(store, Destination.Home, firstLaunch = true)
            }
        }
        compose.waitForIdle()
        compose.runOnUiThread { bars(compose.activity) }
        pump("the engine to start") { it.phase.value == AppStore.Phase.Running && it.medium.view.value != null }
        if (!other) {
            val welcome = compose.activity.getString(R.string.onboarding_continue)
            pump("first launch") { compose.onAllNodesWithText(welcome).fetchSemanticsNodes().isNotEmpty() }
        }
        run(20)
        runBlocking { check(PrelaunchJoin.stage(store, state)) { "no state $state" } }
        // Home after joining: the medium the other device has, answered.
        if (state == "p3-08") pump("the medium") { it.medium.view.value?.answer == app.kaeri.engine.model.KaeriIsFor.READ && it.medium.asking.value == null }
        // The staged clock holds; the motion settles, and a notice is still up at its 4 s.
        run(40)
        compose.waitForIdle()
        val file = "${roborazziSystemPropertyOutputDirectory()}/$name.png"
        if (other) captureScreenRoboImage(file) else compose.onRoot().captureRoboImage(file)
    }

    private fun bars(activity: ComponentActivity) {
        val bar = (24 * activity.resources.displayMetrics.density).roundToInt()
        val status = WindowInsetsCompat.Type.statusBars()
        val navigation = WindowInsetsCompat.Type.navigationBars()
        val insets = WindowInsetsCompat.Builder()
            .setInsets(status, Insets.of(0, bar, 0, 0))
            .setInsets(navigation, Insets.of(0, 0, 0, bar))
            .setVisible(status, true)
            .setVisible(navigation, true)
            .build()
        val content = activity.findViewById<ViewGroup>(android.R.id.content).getChildAt(0)
        ViewCompat.dispatchApplyWindowInsets(content, insets)
    }

    private fun both(n: Int, phone: Boolean) = snap("pl-p3-%02d-%s".format(n, if (phone) "phone" else "tablet"), "p3-%02d".format(n), other = n == 9)

    @Test @Config(qualifiers = PHONE) fun p3_01_phone() = both(1, true)
    @Test @Config(qualifiers = PHONE) fun p3_02_phone() = both(2, true)
    @Test @Config(qualifiers = PHONE) fun p3_03_phone() = both(3, true)
    @Test @Config(qualifiers = PHONE) fun p3_04_phone() = both(4, true)
    @Test @Config(qualifiers = PHONE) fun p3_05_phone() = both(5, true)
    @Test @Config(qualifiers = PHONE) fun p3_06_phone() = both(6, true)
    @Test @Config(qualifiers = PHONE) fun p3_07_phone() = both(7, true)
    @Test @Config(qualifiers = PHONE) fun p3_08_phone() = both(8, true)
    @Test @Config(qualifiers = PHONE) fun p3_09_phone() = both(9, true)

    @Test @Config(qualifiers = TABLET) fun p3_01_tablet() = both(1, false)
    @Test @Config(qualifiers = TABLET) fun p3_02_tablet() = both(2, false)
    @Test @Config(qualifiers = TABLET) fun p3_03_tablet() = both(3, false)
    @Test @Config(qualifiers = TABLET) fun p3_04_tablet() = both(4, false)
    @Test @Config(qualifiers = TABLET) fun p3_05_tablet() = both(5, false)
    @Test @Config(qualifiers = TABLET) fun p3_06_tablet() = both(6, false)
    @Test @Config(qualifiers = TABLET) fun p3_07_tablet() = both(7, false)
    @Test @Config(qualifiers = TABLET) fun p3_08_tablet() = both(8, false)
    @Test @Config(qualifiers = TABLET) fun p3_09_tablet() = both(9, false)
}
