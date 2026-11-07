package app.kaeri.ui

import android.view.ViewGroup
import androidx.activity.ComponentActivity
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onRoot
import androidx.core.graphics.Insets
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import app.kaeri.design.KaeriTheme
import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.store.AppHost
import app.kaeri.store.AppStore
import app.kaeri.store.EngineHome
import app.kaeri.store.EngineLaunch
import app.kaeri.store.FixtureScene
import app.kaeri.store.HAS_TESTING_ENGINE
import app.kaeri.store.MemorySecrets
import app.kaeri.testkit.captureRoboImage
import app.kaeri.testkit.captureScreenRoboImage
import com.github.takahirom.roborazzi.roborazziSystemPropertyOutputDirectory
import java.io.File
import java.nio.file.Files
import kotlin.math.roundToInt
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import org.junit.After
import org.junit.Assume.assumeTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import org.robolectric.shadows.ShadowLooper

/**
 * Reading's places drawn on the JVM from the testing engine itself: a reading scene started
 * through JNA as the Fixtures app starts it, the app's own frame on it, and the place captured once
 * the engine has answered what it shows. Each place's shots extend this. Record with
 * `android/scripts/test.sh :ui:recordRoborazziFixturesDebug --tests 'app.kaeri.ui.Read*Shots'`;
 * the pictures land in android/.shots/roborazzi/.
 */
@OptIn(com.github.takahirom.roborazzi.ExperimentalRoborazziApi::class)
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36])
abstract class ReadingShotHarness {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    protected val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)
    private val root: File = Files.createTempDirectory("kaeri-reading-shots").toFile()
    protected var store: AppStore? = null

    @Before
    fun setUp() {
        assumeTrue(HAS_TESTING_ENGINE)
        val home = System.getenv("HOME").orEmpty()
        check(home.isNotEmpty() && home != System.getProperty("user.home")) { "HOME is the user's own: $home" }
    }

    @After
    fun tearDown() {
        store?.shutdown()
        scope.cancel()
        root.deleteRecursively()
    }

    /**
     * [scene] started on the testing engine, the frame on [start] (with [page] opened over it), and
     * the screen captured once [ready] holds and the art has landed. [medium] is chosen first, as
     * Settings would. [then] runs once it is ready, on the UI thread, before the capture; [screen]
     * captures every window, for a sheet or a menu.
     */
    protected fun shoot(
        name: String,
        scene: FixtureScene,
        start: Destination,
        page: String? = null,
        medium: KaeriIsFor? = null,
        screen: Boolean = false,
        then: (AppStore) -> Unit = {},
        /** A step on the screen once it is ready, such as a tap, and what says it has landed. */
        act: (() -> Unit)? = null,
        acted: (AppStore) -> Boolean = { true },
        ready: (AppStore) -> Boolean,
    ) {
        val store = AppStore(scope)
        this.store = store
        store.start(EngineLaunch(EngineLaunch.Engine.Fixtures(scene), EngineHome.override(root)), AppHost(MemorySecrets()))
        compose.runOnUiThread { WindowCompat.setDecorFitsSystemWindows(compose.activity.window, false) }
        compose.setContent {
            KaeriTheme { AppRoot(store, start, startPage = page) }
        }
        compose.waitForIdle()
        compose.runOnUiThread { systemBars(compose.activity) }
        pump("the engine to start") { it.phase.value == AppStore.Phase.Running && it.medium.view.value != null }
        if (medium != null && store.medium.view.value?.answer != medium) {
            compose.runOnUiThread { store.medium.choose(medium) }
            pump("the medium $medium") { it.medium.view.value?.answer == medium }
        }
        pump(name, ready)
        compose.runOnUiThread { then(store) }
        pump(name, ready)
        if (act != null) {
            settle(store)
            act()
            pump(name, acted)
        }
        settle(store)
        val file = "${roborazziSystemPropertyOutputDirectory()}/$name.png"
        if (screen) captureScreenRoboImage(file) else compose.onRoot().captureRoboImage(file)
    }

    /** Run the main looper, where the engine's answers land, until [until] holds. */
    protected fun pump(what: String, until: (AppStore) -> Boolean) {
        val store = requireNotNull(store)
        val deadline = System.currentTimeMillis() + 30_000
        while (!until(store)) {
            check(System.currentTimeMillis() < deadline) { "waited too long for $what" }
            ShadowLooper.idleMainLooper()
            compose.mainClock.advanceTimeBy(50)
            Thread.sleep(20)
        }
    }

    /** Let the screen settle: the art fetched, decoded and kept, and the motion done. */
    private fun settle(store: AppStore) {
        val deadline = System.currentTimeMillis() + 20_000
        var landed = -1
        var still = System.currentTimeMillis()
        while (System.currentTimeMillis() < deadline) {
            ShadowLooper.idleMainLooper()
            compose.mainClock.advanceTimeBy(100)
            val now = store.artwork.landed.value
            if (now != landed) {
                landed = now
                still = System.currentTimeMillis()
            } else if (System.currentTimeMillis() - still > 800) {
                break
            }
            Thread.sleep(20)
        }
        compose.mainClock.advanceTimeBy(1_000)
        compose.waitForIdle()
    }

    /** The status bar and the gesture bar, 24 dp each, as the boards draw them. */
    private fun systemBars(activity: ComponentActivity) {
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

    companion object {
        /** The phone boards: 412 × 915 dp at 420 dpi, as the emulator's phone. */
        const val PHONE = "w412dp-h915dp-port-420dpi"

        /** The tablet boards: 1280 × 800 dp, landscape, at 320 dpi. */
        const val TABLET = "w1280dp-h800dp-land-xhdpi"
    }
}
