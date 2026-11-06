// Pairing by a typed code: this device's DNS-SD for `_kaeri-sync._tcp`, through the system's NsdManager.
// While a code shows, the device is registered under its name; PairWithCode browses for devices
// showing one and the engine tries the typed code on each. No Play services.

package app.kaeri.store

import android.content.Context
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import android.util.Log
import app.kaeri.engine.ffi.FfiSyncAdvert
import app.kaeri.engine.ffi.FfiSyncNetwork
import app.kaeri.engine.ffi.FfiSyncSink
import app.kaeri.engine.ffi.FfiSyncTxt
import java.util.concurrent.Executor

class NsdSyncNetwork(context: Context) : FfiSyncNetwork, NsdRelook {
    private val main: Executor = context.mainExecutor
    private val nsd = context.getSystemService(NsdManager::class.java)?.let { NsdLink(it, main) }

    private val registrations = mutableMapOf<String, NsdManager.RegistrationListener>()
    private var discovery: NsdManager.DiscoveryListener? = null
    private val follows = NsdFollows()

    override fun advertise(advert: FfiSyncAdvert) = main.execute {
        val nsd = nsd ?: return@execute
        withdrawNow(advert.instance)
        val info = NsdServiceInfo().apply {
            serviceName = advert.instance
            serviceType = SERVICE
            port = advert.port.toInt()
            advert.txt.forEach { setAttribute(it.key, it.value) }
        }
        val listener = object : NsdManager.RegistrationListener {
            override fun onServiceRegistered(info: NsdServiceInfo) {}
            override fun onRegistrationFailed(info: NsdServiceInfo, errorCode: Int) {
                Log.w(TAG, "the sync code was not advertised: $errorCode")
            }
            override fun onServiceUnregistered(info: NsdServiceInfo) {}
            override fun onUnregistrationFailed(info: NsdServiceInfo, errorCode: Int) {}
        }
        registrations[advert.instance] = listener
        runCatching { nsd.register(info, listener) }
            .onFailure { registrations.remove(advert.instance) }
    }

    override fun withdraw(instance: String) = main.execute { withdrawNow(instance) }

    private fun withdrawNow(instance: String) {
        val listener = registrations.remove(instance) ?: return
        runCatching { nsd?.unregister(listener) }
    }

    override fun browse(sink: FfiSyncSink) = main.execute {
        val nsd = nsd ?: return@execute
        if (discovery != null) return@execute
        val listener = object : NsdManager.DiscoveryListener {
            override fun onServiceFound(info: NsdServiceInfo) = follow(info, sink)
            override fun onServiceLost(info: NsdServiceInfo) {
                follows.lost(info.serviceName)
                sink.lost(info.serviceName)
            }
            override fun onDiscoveryStarted(serviceType: String) {}
            override fun onDiscoveryStopped(serviceType: String) {}
            override fun onStartDiscoveryFailed(serviceType: String, errorCode: Int) {
                Log.w(TAG, "the sync browse did not start: $errorCode")
                main.execute { if (discovery === this) discovery = null }
            }
            override fun onStopDiscoveryFailed(serviceType: String, errorCode: Int) {}
        }
        discovery = listener
        nsd.discover(SERVICE, listener)
    }

    override fun stopBrowsing() = main.execute {
        val listener = discovery ?: return@execute
        discovery = null
        runCatching { nsd?.stopDiscovery(listener) }
        follows.clear()
    }

    private fun follow(info: NsdServiceInfo, sink: FfiSyncSink) = main.execute {
        val nsd = nsd ?: return@execute
        if (discovery == null) return@execute
        // Found again: below Android 14 it is resolved again, as it may have moved.
        follows.found(info.serviceName) {
            nsd.follow(
                info,
                found = { resolved ->
                    val txt = resolved.attributes.map { (key, value) -> FfiSyncTxt(key, value?.decodeToString().orEmpty()) }
                    sink.found(resolved.name, resolved.addresses, resolved.port.toUShort(), txt)
                },
                lost = { sink.lost(info.serviceName) },
                failed = { Log.w(TAG, "a sync device did not resolve: $it") },
            )
        }
    }

    /** Resolve every service found again, below Android 14, as after a connection to one failed. */
    override fun relook() = main.execute { follows.relook() }

    companion object {
        private const val TAG = "kaeri"
        const val SERVICE = "_kaeri-sync._tcp"
    }
}
