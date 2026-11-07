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
import app.kaeri.store.PrelaunchDemo
import app.kaeri.testkit.captureRoboImage
import app.kaeri.ui.reading.comic
import app.kaeri.ui.reading.demoWork
import app.kaeri.ui.reading.readingDemo
import app.kaeri.ui.reading.workId
import com.github.takahirom.roborazzi.roborazziSystemPropertyOutputDirectory
import java.io.File
import java.nio.file.Files
import kotlin.math.roundToInt
import kotlinx.coroutines.launch
import org.junit.After
import org.junit.Assume.assumeTrue
import org.junit.Test
import org.robolectric.annotation.Config
import org.robolectric.shadows.ShadowLooper

/**
 * Prelaunch board piece 2, the reading demo, on the phone and the tablet: each state staged as the
 * Fixtures app's `prelaunch` extra stages it, on the place its steers open.
 */
@OptIn(com.github.takahirom.roborazzi.ExperimentalRoborazziApi::class)
class PrelaunchDemoShots : ReadingShotHarness() {
    private val home: File = Files.createTempDirectory("kaeri-demo-shots").toFile()

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

    /** [state] staged over first run's pages, or on [start] with [page] opened, once [ready] holds. */
    private fun snap(name: String, state: String, start: Destination = Destination.Home, page: String? = null, ready: (AppStore) -> Boolean) {
        val store = AppStore(scope)
        this.store = store
        store.start(EngineLaunch(EngineLaunch.Engine.Fixtures(FixtureScene.READING_FIRST_RUN), EngineHome.override(home)), AppHost(MemorySecrets()))
        val first = state == "p2-01" || state == "p2-02"
        compose.runOnUiThread { WindowCompat.setDecorFitsSystemWindows(compose.activity.window, false) }
        compose.setContent {
            KaeriTheme {
                if (first) AppRoot(store, Destination.Home, firstLaunch = true) else AppRoot(store, start, startPage = page)
            }
        }
        compose.waitForIdle()
        compose.runOnUiThread { bars(compose.activity) }
        pump("the engine to start") { it.phase.value == AppStore.Phase.Running && it.medium.view.value != null }
        if (first) {
            val welcome = compose.activity.getString(R.string.onboarding_continue)
            pump("first launch") { compose.onAllNodesWithText(welcome).fetchSemanticsNodes().isNotEmpty() }
        }
        run(20)
        // Staged on the main thread as the app stages it, with the looper run so the engine's answers land.
        var staged = false
        scope.launch {
            check(PrelaunchDemo.stage(store, state)) { "no state $state" }
            staged = true
        }
        pump("staging $state") { staged }
        try {
            pump(name, ready)
        } catch (e: IllegalStateException) {
            println("XX $name demo=${store.demo.value} medium=${store.medium.view.value?.answer} providers=${store.providers.rows.value.map { it.id }} books=${store.bookSources.view.value != null}")
            throw e
        }
        run(60)
        compose.waitForIdle()
        compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/$name.png")
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

    private fun heading(): Boolean {
        val words = compose.activity.getString(R.string.onboarding_demo_heading)
        return compose.onAllNodesWithText(words).fetchSemanticsNodes().isNotEmpty()
    }

    private fun shot(n: String, phone: Boolean) {
        val name = "pl-p2-$n-${if (phone) "phone" else "tablet"}"
        when (n) {
            "01", "02" -> snap(name, "p2-$n") { heading() }
            "03" -> snap(name, "p2-03") { it.demo.value.onlyBooks && it.reading.home.value != null }
            "04" -> {
                val work = readingDemo.comic?.workId
                assumeTrue("the engine names no work for the demo's comic", work != null)
                snap(name, "p2-04", page = seriesRoute(Destination.Home, requireNotNull(work))) { it.reading.series.value[work]?.demoWork != null }
            }
            "07" -> snap(name, "p2-07", page = providerRoute("demo")) { s -> s.demo.value.reading && s.providers.rows.value.any { it.id == "demo" } }
            "08" -> snap(name, "p2-08", start = Destination.Search) {
                it.demo.value.onlyBooks && !it.reading.search.value.loading && it.reading.search.value.query.text == PrelaunchDemo.SEARCH
            }
            else -> snap(name, "p2-row", page = PROVIDERS_ROUTE) { it.demo.value.reading && it.bookSources.view.value != null }
        }
    }

    @Test @Config(qualifiers = PHONE) fun p2_01_phone() = shot("01", true)
    @Test @Config(qualifiers = PHONE) fun p2_02_phone() = shot("02", true)
    @Test @Config(qualifiers = PHONE) fun p2_03_phone() = shot("03", true)
    @Test @Config(qualifiers = PHONE) fun p2_04_phone() = shot("04", true)
    @Test @Config(qualifiers = PHONE) fun p2_07_phone() = shot("07", true)
    @Test @Config(qualifiers = PHONE) fun p2_08_phone() = shot("08", true)
    @Test @Config(qualifiers = PHONE) fun p2_row_phone() = shot("row", true)

    @Test @Config(qualifiers = TABLET) fun p2_01_tablet() = shot("01", false)
    @Test @Config(qualifiers = TABLET) fun p2_02_tablet() = shot("02", false)
    @Test @Config(qualifiers = TABLET) fun p2_03_tablet() = shot("03", false)
    @Test @Config(qualifiers = TABLET) fun p2_04_tablet() = shot("04", false)
    @Test @Config(qualifiers = TABLET) fun p2_07_tablet() = shot("07", false)
    @Test @Config(qualifiers = TABLET) fun p2_08_tablet() = shot("08", false)
    @Test @Config(qualifiers = TABLET) fun p2_row_tablet() = shot("row", false)
}
