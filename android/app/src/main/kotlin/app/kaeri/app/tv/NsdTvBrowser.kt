// The TVs on the network, as Android's DNS-SD finds them: `_kaeri._tcp` services, each
// followed for its addresses, port and TXT record, handed to the engine's browse.

package app.kaeri.app.tv

import android.content.Context
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import android.util.Log
import app.kaeri.store.NsdFollowing
import app.kaeri.store.NsdLink
import java.util.concurrent.Executor
import app.kaeri.engine.ffi.FfiDiscoverySink
import app.kaeri.engine.ffi.FfiTvBrowser
import app.kaeri.engine.ffi.FfiTxtPair

/**
 * A browse for the engine's sender. The engine starts it while a picker is up and stops it after;
 * nothing runs between, so it costs nothing while Play on TV is not being chosen.
 */
class NsdTvBrowser(context: Context) : FfiTvBrowser {
    private val main: Executor = context.mainExecutor
    private val nsd = context.getSystemService(NsdManager::class.java)?.let { NsdLink(it, main) }

    private var discovery: NsdManager.DiscoveryListener? = null
    private val following = mutableMapOf<String, NsdFollowing>()

    override fun start(sink: FfiDiscoverySink) = main.execute {
        if (discovery != null || nsd == null) return@execute
        val listener = object : NsdManager.DiscoveryListener {
            override fun onServiceFound(info: NsdServiceInfo) = follow(info, sink)
            override fun onServiceLost(info: NsdServiceInfo) {
                unfollow(info.serviceName)
                sink.lost(info.serviceName)
            }
            override fun onDiscoveryStarted(serviceType: String) {}
            override fun onDiscoveryStopped(serviceType: String) {}
            override fun onStartDiscoveryFailed(serviceType: String, errorCode: Int) {
                Log.w(TAG, "the TV browse did not start: $errorCode")
                main.execute { if (discovery === this) discovery = null }
            }
            override fun onStopDiscoveryFailed(serviceType: String, errorCode: Int) {}
        }
        discovery = listener
        nsd.discover(SERVICE, listener)
    }

    override fun stop() = main.execute {
        val listener = discovery ?: return@execute
        discovery = null
        runCatching { nsd?.stopDiscovery(listener) }
        for (name in following.keys.toList()) unfollow(name)
    }

    /** Follow a service found for what it resolves to, and each change to it, until it goes. */
    private fun follow(info: NsdServiceInfo, sink: FfiDiscoverySink) = main.execute {
        val nsd = nsd ?: return@execute
        if (discovery == null || info.serviceName in following) return@execute
        runCatching {
            following[info.serviceName] = nsd.follow(
                info,
                found = { resolved ->
                    val txt = resolved.attributes.map { (key, value) -> FfiTxtPair(key, value?.decodeToString().orEmpty()) }
                    sink.found(resolved.name, resolved.addresses, resolved.port.toUShort(), txt)
                },
                lost = { sink.lost(info.serviceName) },
                failed = { Log.w(TAG, "a TV did not resolve: $it") },
            )
        }.onFailure { following.remove(info.serviceName) }
    }

    private fun unfollow(name: String) {
        val following = following.remove(name) ?: return
        runCatching { nsd?.unfollow(following) }
    }

    companion object {
        private const val TAG = "kaeri"

        /** The remote's DNS-SD service, as the TV advertises it. */
        const val SERVICE = "_kaeri._tcp"
    }
}
