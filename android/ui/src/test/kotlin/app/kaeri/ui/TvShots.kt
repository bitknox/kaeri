package app.kaeri.ui

import android.view.ViewGroup
import androidx.activity.ComponentActivity
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getAllSemanticsNodes
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performScrollTo
import androidx.core.graphics.Insets
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import app.kaeri.testkit.captureRoboImage
import app.kaeri.testkit.captureScreenRoboImage
import com.github.takahirom.roborazzi.roborazziSystemPropertyOutputDirectory
import java.io.File
import java.nio.file.Files
import java.time.Clock
import java.time.ZoneId
import kotlin.math.roundToInt
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.newSingleThreadContext
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeout
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
import app.kaeri.design.KaeriTheme
import app.kaeri.engine.model.Event
import app.kaeri.store.AppClock
import app.kaeri.store.AppHost
import app.kaeri.store.AppStore
import app.kaeri.store.EngineHome
import app.kaeri.store.EngineLaunch
import app.kaeri.store.FixtureScene
import app.kaeri.store.FixtureTvs
import app.kaeri.store.HAS_TESTING_ENGINE
import app.kaeri.store.MemorySecrets
import app.kaeri.store.TvPlayRequest
import app.kaeri.store.TvRemoteModel
import app.kaeri.store.TvScene
import app.kaeri.testkit.FixtureArt
import app.kaeri.testkit.Scene

/**
 * Play on TV, drawn on the JVM over the watching scene: the picker, the code, the card, the card of a TV gone, the bar
 * that leads back to it, and the paired TVs in Settings. The TVs are the fixtures' two fake ones,
 * real remote servers on the loopback interface, driven through a second, real testing engine, as
 * the Fixtures app drives them.
 */
@OptIn(com.github.takahirom.roborazzi.ExperimentalRoborazziApi::class, kotlinx.coroutines.DelicateCoroutinesApi::class, kotlinx.coroutines.ExperimentalCoroutinesApi::class)
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36])
class TvShots {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)
    private val engineThread = newSingleThreadContext("tv-engine")
    private val engineScope = CoroutineScope(SupervisorJob() + engineThread)
    private val root: File = Files.createTempDirectory("kaeri-tv-shots").toFile()
    private var engineStore: AppStore? = null

    @Before
    fun setUp() {
        assumeTrue(HAS_TESTING_ENGINE)
        val home = System.getenv("HOME").orEmpty()
        check(home.isNotEmpty() && home != System.getProperty("user.home")) { "HOME is the user's own: $home" }
    }

    @After
    fun tearDown() {
        scope.cancel()
        engineStore?.let { store -> runBlocking { withContext(engineThread) { store.shutdown() } } }
        engineScope.cancel()
        engineThread.close()
        root.deleteRecursively()
        AppClock.clock = Clock.systemDefaultZone()
    }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_tv_party_chat() = shoot("phone-tv-party-chat", TvScene.PARTY, scrollTo = "Chat")

    /** The remote's Chat heading with its one menu open: Mute or Unmute for each member. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_tv_party_chat_menu() = shoot("phone-tv-party-chat-menu", TvScene.PARTY, scrollTo = "Chat", after = {
        // Only a recording draws a menu's window, which its semantics need.
        org.junit.Assume.assumeTrue(com.github.takahirom.roborazzi.roborazziSystemPropertyTaskType().isEnabled())
        pressWindow(lowest = true) { SemanticsProperties.ContentDescription in it.config && "More" in it.config[SemanticsProperties.ContentDescription] }
        compose.mainClock.advanceTimeBy(600)
        captureScreenRoboImage("${roborazziSystemPropertyOutputDirectory()}/phone-tv-party-chat-scratch.png")
        compose.mainClock.advanceTimeBy(300)
    })

    /** A member muted from that menu: "Name is muted · Unmute" stands as a line of the conversation. */
    @Test
    @Config(qualifiers = PHONE)
    fun phone_tv_party_chat_muted() = shoot("phone-tv-party-chat-muted", TvScene.PARTY, scrollTo = "Chat", after = {
        // Only a recording draws a menu's window, which its semantics need.
        org.junit.Assume.assumeTrue(com.github.takahirom.roborazzi.roborazziSystemPropertyTaskType().isEnabled())
        pressWindow(lowest = true) { SemanticsProperties.ContentDescription in it.config && "More" in it.config[SemanticsProperties.ContentDescription] }
        compose.mainClock.advanceTimeBy(600)
        // Robolectric lays a popup out as it draws one, so the menu is drawn once to be read.
        captureScreenRoboImage("${roborazziSystemPropertyOutputDirectory()}/phone-tv-party-chat-scratch.png")
        compose.mainClock.advanceTimeBy(300)
        captureScreenRoboImage("${roborazziSystemPropertyOutputDirectory()}/phone-tv-party-chat-scratch.png")
        pressWindow { node -> node.config.getOrNull(SemanticsProperties.Text)?.any { it.text.startsWith("Mute ") } == true && SemanticsActions.OnClick in node.config }
        repeat(100) {
            ShadowLooper.idleMainLooper()
            compose.waitForIdle()
            Thread.sleep(20)
        }
        compose.mainClock.advanceTimeBy(600)
        compose.onNodeWithText("Unmute", substring = true).performScrollTo()
    })

    @Test
    @Config(qualifiers = PHONE)
    fun phone_tv_picker() = shoot("phone-tv-picker", TvScene.PICKER)

    @Test
    @Config(qualifiers = PHONE)
    fun phone_tv_code() = shoot("phone-tv-code", TvScene.CODE)

    @Test
    @Config(qualifiers = PHONE)
    fun phone_tv_remote() = shoot("phone-tv-remote", TvScene.REMOTE)

    @Test
    @Config(qualifiers = PHONE)
    fun phone_tv_lost() = shoot("phone-tv-lost", TvScene.LOST)

    @Test
    @Config(qualifiers = PHONE)
    fun phone_tv_bar() = shoot("phone-tv-bar", TvScene.REMOTE, screen = false) { it.sheetClosed() }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_settings_accounts_tv() =
        shoot("phone-settings-accounts-tv", TvScene.PAIRED, page = settingsRoute(SettingsPane.Devices), screen = false, scrollTo = "Pair a TV")

    @Test
    @Config(qualifiers = TABLET)
    fun tablet_tv_remote() = shoot("tablet-tv-remote", TvScene.REMOTE)

    /**
     * The watching scene replayed into the app's store, Play on TV attached to a sender of the
     * second engine's with the fake TVs, [scene] staged on it, and the screen captured once it is
     * up: the whole screen for a sheet's window, the root for the bar and Settings. [then] changes
     * the model once the scene is reached.
     */
    private fun shoot(
        name: String,
        scene: TvScene,
        page: String? = null,
        screen: Boolean = true,
        scrollTo: String? = null,
        after: () -> Unit = {},
        then: (TvRemoteModel) -> Unit = {},
    ) {
        val recorded = Scene.named("watching")
        AppClock.clock = Clock.fixed(recorded.recordedAt, ZONE)
        val store = AppStore(scope, clock = { 0L }, artworkWithoutEngine = FixtureArt::bytes)
        val events = recorded.events.filter(::replayed)
        store.apply(events)

        // The engine behind the TVs, on a thread of its own, away from the looper the shot pumps.
        val engine = runBlocking {
            val started = withContext(engineThread) { AppStore(engineScope) }
            engineStore = started
            withContext(engineThread) {
                started.start(EngineLaunch(EngineLaunch.Engine.Fixtures(FixtureScene.FRESH), EngineHome.override(root)), AppHost(MemorySecrets()))
            }
            withTimeout(30_000) { started.phase.first { it == AppStore.Phase.Running } }
            requireNotNull(started.engine)
        }
        val browser = requireNotNull(FixtureTvs.start(engine))
        store.tv.attach(engine.tvSender(TvRemoteModel.DEVICE_NAME, "kaeri-android/shots"), browser)

        compose.runOnUiThread { WindowCompat.setDecorFitsSystemWindows(compose.activity.window, false) }
        compose.setContent {
            KaeriTheme { AppRoot(store, Destination.Home, startPage = page) }
        }
        compose.waitForIdle()
        compose.runOnUiThread { systemBars(compose.activity) }
        store.apply(events)
        awaitArt(store)

        val title = store.titles[SHOWN]?.showTitle.orEmpty()
        scope.launch { FixtureTvs.stage(store.tv, scene, browser, TvPlayRequest(SHOWN, title, "3")) }
        // What the engine answers from its own threads is posted to the main looper, which the
        // compose rule leaves alone while it is idle: run it until the scene is there.
        val deadline = System.currentTimeMillis() + 30_000
        while (!store.tv.reached(scene)) {
            check(System.currentTimeMillis() < deadline) { "the TV scene $scene did not come up" }
            ShadowLooper.idleMainLooper()
            compose.waitForIdle()
            Thread.sleep(20)
        }
        compose.runOnUiThread { then(store.tv) }
        compose.mainClock.advanceTimeBy(1_000)
        compose.waitForIdle()
        awaitArt(store)
        scrollTo?.let { compose.onNodeWithText(it).performScrollTo() }
        compose.waitForIdle()
        after()
        val file = "${roborazziSystemPropertyOutputDirectory()}/$name.png"
        if (screen) captureScreenRoboImage(file) else compose.onRoot().captureRoboImage(file)
        compose.runOnUiThread { store.tv.disconnect() }
    }

    /** Every window's semantics (the sheet, a menu), read without the test's idle wait; [match] is pressed on the UI thread. */
    @OptIn(androidx.compose.ui.InternalComposeUiApi::class)
    private fun pressWindow(lowest: Boolean = false, match: (androidx.compose.ui.semantics.SemanticsNode) -> Boolean) {
        fun find(view: android.view.View): androidx.compose.ui.node.RootForTest? =
            (view as? androidx.compose.ui.node.RootForTest)
                ?: (view as? android.view.ViewGroup)?.let { group -> (0 until group.childCount).firstNotNullOfOrNull { find(group.getChildAt(it)) } }
        val global = Class.forName("android.view.WindowManagerGlobal")
        val manager = global.getMethod("getInstance").invoke(null)
        @Suppress("UNCHECKED_CAST")
        val names = global.getMethod("getViewRootNames").invoke(manager) as Array<String>
        val getRoot = global.getMethod("getRootView", String::class.java)
        val nodes = names.mapNotNull { find(getRoot.invoke(manager, it) as android.view.View) }
            .flatMap { root -> runCatching { root.semanticsOwner.getAllSemanticsNodes(mergingEnabled = true) }.getOrDefault(emptyList()) }
        val found = nodes.filter { SemanticsActions.OnClick in it.config && match(it) }
        val node = if (lowest) found.maxBy { it.boundsInRoot.top } else found.first()
        compose.runOnUiThread { node.config[SemanticsActions.OnClick].action?.invoke() }
    }

    private fun awaitArt(store: AppStore) {
        val deadline = System.currentTimeMillis() + 20_000
        var landed = -1
        var still = System.currentTimeMillis()
        while (System.currentTimeMillis() < deadline) {
            compose.waitForIdle()
            val now = store.artwork.landed.value
            if (now != landed) {
                landed = now
                still = System.currentTimeMillis()
            } else if (System.currentTimeMillis() - still > 500) {
                return
            }
            Thread.sleep(20)
        }
    }

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

    private companion object {
        const val PHONE = "w412dp-h915dp-port-420dpi"
        const val TABLET = "w1280dp-h800dp-land-xhdpi"
        val ZONE: ZoneId = ZoneId.of("Europe/Copenhagen")

        /** The title the TV plays: one of the watching scene's. */
        const val SHOWN = 900001u

        fun replayed(event: Event): Boolean = when (event) {
            is Event.JobStarted, is Event.JobNote, is Event.JobDone, is Event.Toast -> false
            else -> true
        }
    }
}
