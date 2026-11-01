// The TV process's one engine, store and player, which outlive the activity.

package app.kaeri.tv

import android.app.Application
import android.content.ActivityNotFoundException
import android.content.ComponentName
import android.content.Intent
import android.graphics.Bitmap
import android.hardware.display.DisplayManager
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.util.Log
import android.view.Display
import androidx.core.net.toUri
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.ProcessLifecycleOwner
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.plus
import kotlinx.coroutines.launch
import app.kaeri.engine.ffi.startLog
import app.kaeri.player.SystemCaptions
import kotlinx.coroutines.flow.map
import app.kaeri.player.CaptionStyleWatch
import app.kaeri.player.Device
import app.kaeri.player.MpvBackend
import app.kaeri.player.PlayerConnection
import app.kaeri.player.PlayerHost
import app.kaeri.player.PlayerModel
import app.kaeri.store.AppHost
import app.kaeri.store.AppPhases
import app.kaeri.store.AppStore
import app.kaeri.store.EngineLaunch
import app.kaeri.store.KeystoreKey
import app.kaeri.store.SealedFileSecrets
import app.kaeri.store.traced
import app.kaeri.tv.home.WatchNext
import app.kaeri.store.collectCrashes
import app.kaeri.tv.remote.TvRemoteHost
import app.kaeri.tv.ui.TV_ART_BUDGET
import app.kaeri.tv.ui.TvArtDecoder

class TvApp : Application(), PlayerHost {
    /** Where the store's work runs: the main thread, for as long as the process. */
    private val scope = MainScope() + kotlinx.coroutines.CoroutineExceptionHandler { _, e ->
        // A coroutine that fails in the store is logged; it does not take the process with it.
        android.util.Log.e("kaeri", "the store's work failed", e)
    }

    // The TV updates from the release's TV APK, never the phone's: the one built for the ABI this
    // system runs the app as, 32-bit on most TV boxes and 64-bit on the rest.
    val store: AppStore by lazy {
        AppStore(scope, decoder = TvArtDecoder, artworkBudget = TV_ART_BUDGET, updateAsset = ::tvUpdateAsset).also { it.collectCrashes(this, scope) }
    }

    /** The TV's own update, where the flavour has one: the release build's, or a staged fixture. */
    var updater: UpdateLauncher? = null
        private set

    private var updaterAsked = false

    /** Attach the flavour's updater, once per process, from the first activity's launch. */
    fun attachUpdater(intent: Intent) {
        if (updaterAsked) return
        updaterAsked = true
        updater = Flavour.updater(this, scope, intent)
    }

    /** The player, for the life of the process: mpv and the media session outlive the activity. */
    override val player: PlayerModel by lazy {
        PlayerModel(
            send = { store.send(it) },
            backend = { MpvBackend(this) },
            scope = scope,
            power = { Device.power(this) },
            display = {
                getSystemService(DisplayManager::class.java)?.getDisplay(Display.DEFAULT_DISPLAY)
                    ?.let { Device.display(it, resources.displayMetrics.density) }
            },
            systemCaptionStyle = { SystemCaptions.read(this) },
        ).also { player ->
            store.onPlayback = player::apply
            store.onPlayAsked = player::asked
            PlayerConnection(this, player, scope)
            captions = CaptionStyleWatch(this, player, scope, store.settings.view.map { it?.subtitleStyle })
        }
    }

    /** The remote phones drive this TV with, listening while the app is on screen. */
    val remote: TvRemoteHost by lazy { TvRemoteHost(this, store, scope) }

    /** The launcher's Watch Next row, written from the Continue queue. */
    val watchNext: WatchNext by lazy {
        WatchNext(this, store, scope, ComponentName(this, TvActivity::class.java))
    }

    /** The player's caption style, following the setting and the system's style. */
    private var captions: CaptionStyleWatch? = null

    /** The art the screens have decoded, for the media session. */
    override fun artwork(url: String): Bitmap? = store.artwork.image(url)?.image

    private var started = false

    /** Start the engine [launch] describes, once per process: the first activity's launch wins. */
    fun start(launch: EngineLaunch) {
        if (started) return
        started = true
        Log.i(TAG, "starting the ${if (launch.engine == EngineLaunch.Engine.App) "app" else "testing"} engine")
        // Before the engine, so no playback event finds no player.
        player
        val prepare: () -> Unit = {
            traced("startLog") { startLog(launch.home.cache.path) }
            traced("flavour") { Flavour.prepare(this) }
        }
        store.start(launch, host(), prepare)
        watchNetwork()
        ProcessLifecycleOwner.get().lifecycle.addObserver(remote)
        ProcessLifecycleOwner.get().lifecycle.addObserver(phases())
        watchNext.start()
    }

    /**
     * The app's place, told to the engine as the phone tells it: the foreground, or the
     * background, where torrents wait. Leaving the app pauses the episode (TvActivity.onStop),
     * so nothing plays there.
     */
    private fun phases() = object : DefaultLifecycleObserver {
        private val phases = AppPhases(store::send)

        override fun onStart(owner: LifecycleOwner) = phases.foreground()

        override fun onStop(owner: LifecycleOwner) = phases.background(playing = false)
    }

    /**
     * The network as the engine plans around it, as the phone says it: a metered link, Data Saver,
     * or none at all, which is the one offline state the screens show.
     */
    private fun watchNetwork() {
        val connectivity = getSystemService(ConnectivityManager::class.java) ?: return
        var said: Triple<Boolean, Boolean, Boolean>? = null
        val say = { capabilities: NetworkCapabilities? ->
            val cellular = capabilities?.hasTransport(NetworkCapabilities.TRANSPORT_CELLULAR) == true
            val constrained = connectivity.restrictBackgroundStatus == ConnectivityManager.RESTRICT_BACKGROUND_STATUS_ENABLED
            val offline = capabilities?.hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET) != true
            val now = Triple(cellular, constrained, offline)
            if (now != said) {
                said = now
                scope.launch { store.networkPath(cellular, constrained, offline) }
            }
        }
        say(connectivity.getNetworkCapabilities(connectivity.activeNetwork))
        connectivity.registerDefaultNetworkCallback(object : ConnectivityManager.NetworkCallback() {
            override fun onCapabilitiesChanged(network: Network, capabilities: NetworkCapabilities) = say(capabilities)

            // Another network may already be the default; only none at all is offline.
            override fun onLost(network: Network) = say(connectivity.activeNetwork?.let(connectivity::getNetworkCapabilities))
        })
    }

    private fun host() = AppHost(
        secrets = SealedFileSecrets(filesDir.resolve("secrets")) { KeystoreKey.get() },
        open = ::open,
    )

    /** Open [url] in the app that handles it, where the TV has one. */
    private fun open(url: String): Boolean = try {
        startActivity(Intent(Intent.ACTION_VIEW, url.toUri()).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
        true
    } catch (_: ActivityNotFoundException) {
        false
    } catch (_: SecurityException) {
        false
    }

    companion object {
        const val TAG = "kaeri-tv"
    }
}
