package app.kaeri.ui

import android.view.ViewGroup
import androidx.activity.ComponentActivity
import androidx.compose.ui.test.onAllNodesWithText
import androidx.core.graphics.Insets
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import app.kaeri.design.KaeriTheme
import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.i18n.R
import app.kaeri.store.AppHost
import app.kaeri.store.AppStore
import app.kaeri.store.EngineHome
import app.kaeri.store.EngineLaunch
import app.kaeri.store.FixtureScene
import app.kaeri.store.MemorySecrets
import app.kaeri.store.PrelaunchCameIn
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
 * Prelaunch board piece 1, "Say what came in, or why nothing did", on the phone and the tablet:
 * each of the 16 states in Settings' Your sources, with the form over it, and on first run's books
 * page, with the form in its place, staged as the Fixtures app's `prelaunch` extra stages it.
 */
@OptIn(com.github.takahirom.roborazzi.ExperimentalRoborazziApi::class)
class PrelaunchCameInShots : ReadingShotHarness() {
    private val home: File = Files.createTempDirectory("kaeri-camein-shots").toFile()

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

    private fun snap(n: Int, first: Boolean, phone: Boolean) {
        val state = "p1-%s-%02d".format(if (first) "first" else "settings", n)
        val store = AppStore(scope)
        this.store = store
        val scene = if (first) FixtureScene.READING_FIRST_RUN else FixtureScene.READING_HOME
        store.start(EngineLaunch(EngineLaunch.Engine.Fixtures(scene), EngineHome.override(home)), AppHost(MemorySecrets()))
        compose.runOnUiThread { WindowCompat.setDecorFitsSystemWindows(compose.activity.window, false) }
        compose.setContent {
            KaeriTheme {
                if (first) AppRoot(store, Destination.Home, firstLaunch = true) else AppRoot(store, Destination.Home, startPage = "settings/sources")
            }
        }
        compose.waitForIdle()
        compose.runOnUiThread { bars(compose.activity) }
        pump("the engine to start") { it.phase.value == AppStore.Phase.Running && it.medium.view.value != null }
        if (first) {
            val welcome = compose.activity.getString(R.string.onboarding_continue)
            pump("first launch") { compose.onAllNodesWithText(welcome).fetchSemanticsNodes().isNotEmpty() }
        } else if (store.medium.view.value?.answer != KaeriIsFor.READ) {
            compose.runOnUiThread { store.medium.choose(KaeriIsFor.READ) }
            pump("reading") { it.medium.view.value?.answer == KaeriIsFor.READ }
        }
        run(20)
        compose.runOnUiThread { runBlocking { check(PrelaunchCameIn.stage(store, state)) { "no state $state" } } }
        // The forms open and settle, a row lands, and a notice is still up within its 4 s.
        run(40)
        compose.waitForIdle()
        captureScreenRoboImage("${roborazziSystemPropertyOutputDirectory()}/pl-p1-%s-%02d-%s.png".format(if (first) "first" else "settings", n, if (phone) "phone" else "tablet"))
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

    @Test @Config(qualifiers = PHONE) fun settings_01_phone() = snap(1, false, true)
    @Test @Config(qualifiers = PHONE) fun settings_02_phone() = snap(2, false, true)
    @Test @Config(qualifiers = PHONE) fun settings_03_phone() = snap(3, false, true)
    @Test @Config(qualifiers = PHONE) fun settings_04_phone() = snap(4, false, true)
    @Test @Config(qualifiers = PHONE) fun settings_05_phone() = snap(5, false, true)
    @Test @Config(qualifiers = PHONE) fun settings_06_phone() = snap(6, false, true)
    @Test @Config(qualifiers = PHONE) fun settings_07_phone() = snap(7, false, true)
    @Test @Config(qualifiers = PHONE) fun settings_08_phone() = snap(8, false, true)
    @Test @Config(qualifiers = PHONE) fun settings_09_phone() = snap(9, false, true)
    @Test @Config(qualifiers = PHONE) fun settings_10_phone() = snap(10, false, true)
    @Test @Config(qualifiers = PHONE) fun settings_11_phone() = snap(11, false, true)
    @Test @Config(qualifiers = PHONE) fun settings_12_phone() = snap(12, false, true)
    @Test @Config(qualifiers = PHONE) fun settings_13_phone() = snap(13, false, true)
    @Test @Config(qualifiers = PHONE) fun settings_14_phone() = snap(14, false, true)
    @Test @Config(qualifiers = PHONE) fun settings_15_phone() = snap(15, false, true)
    @Test @Config(qualifiers = PHONE) fun settings_16_phone() = snap(16, false, true)

    @Test @Config(qualifiers = PHONE) fun first_01_phone() = snap(1, true, true)
    @Test @Config(qualifiers = PHONE) fun first_02_phone() = snap(2, true, true)
    @Test @Config(qualifiers = PHONE) fun first_03_phone() = snap(3, true, true)
    @Test @Config(qualifiers = PHONE) fun first_04_phone() = snap(4, true, true)
    @Test @Config(qualifiers = PHONE) fun first_05_phone() = snap(5, true, true)
    @Test @Config(qualifiers = PHONE) fun first_06_phone() = snap(6, true, true)
    @Test @Config(qualifiers = PHONE) fun first_07_phone() = snap(7, true, true)
    @Test @Config(qualifiers = PHONE) fun first_08_phone() = snap(8, true, true)
    @Test @Config(qualifiers = PHONE) fun first_09_phone() = snap(9, true, true)
    @Test @Config(qualifiers = PHONE) fun first_10_phone() = snap(10, true, true)
    @Test @Config(qualifiers = PHONE) fun first_11_phone() = snap(11, true, true)
    @Test @Config(qualifiers = PHONE) fun first_12_phone() = snap(12, true, true)
    @Test @Config(qualifiers = PHONE) fun first_13_phone() = snap(13, true, true)
    @Test @Config(qualifiers = PHONE) fun first_14_phone() = snap(14, true, true)
    @Test @Config(qualifiers = PHONE) fun first_15_phone() = snap(15, true, true)
    @Test @Config(qualifiers = PHONE) fun first_16_phone() = snap(16, true, true)

    @Test @Config(qualifiers = TABLET) fun settings_01_tablet() = snap(1, false, false)
    @Test @Config(qualifiers = TABLET) fun settings_03_tablet() = snap(3, false, false)
    @Test @Config(qualifiers = TABLET) fun settings_05_tablet() = snap(5, false, false)
    @Test @Config(qualifiers = TABLET) fun settings_09_tablet() = snap(9, false, false)
    @Test @Config(qualifiers = TABLET) fun settings_11_tablet() = snap(11, false, false)
    @Test @Config(qualifiers = TABLET) fun settings_15_tablet() = snap(15, false, false)
    @Test @Config(qualifiers = TABLET) fun settings_16_tablet() = snap(16, false, false)
    @Test @Config(qualifiers = TABLET) fun first_03_tablet() = snap(3, true, false)
    @Test @Config(qualifiers = TABLET) fun first_11_tablet() = snap(11, true, false)
    @Test @Config(qualifiers = TABLET) fun first_16_tablet() = snap(16, true, false)

    companion object {
        const val PHONE = ReadingShotHarness.PHONE
        const val TABLET = ReadingShotHarness.TABLET
    }
}
