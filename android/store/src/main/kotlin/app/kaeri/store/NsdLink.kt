// DNS-SD through the system's NsdManager on every Android the apps run on. Android 14 calls back
// on the executor it is given and follows a found service through each change to it. Below 14
// the older calls are used, with their callbacks moved onto that same executor, and a found
// service is resolved once, one resolve at a time across the app, since a second resolve while
// one runs fails there.

package app.kaeri.store

import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import android.os.Build
import androidx.annotation.ChecksSdkIntAtLeast
import java.util.IdentityHashMap
import java.util.concurrent.Executor

/** A found service as it resolved: its name, addresses, port and TXT record. */
class NsdResolved(val name: String, val addresses: List<String>, val port: Int, val attributes: Map<String, ByteArray?>) {
    companion object {
        internal fun of(info: NsdServiceInfo, addresses: List<String>) =
            NsdResolved(info.serviceName, addresses, info.port, info.attributes.orEmpty())

        /** An address as the engine takes it: without a link-local zone. */
        internal fun address(host: java.net.InetAddress?): String? = host?.hostAddress?.substringBefore('%')
    }
}

/** What follows one found service: unfollowed with [NsdLink.unfollow]. */
class NsdFollowing internal constructor(internal val stop: () -> Unit)

/** The calls the browsers and the sync network make, on whichever Android this is. */
class NsdLink(private val nsd: NsdManager, private val executor: Executor) {
    private val discoveries = IdentityHashMap<NsdManager.DiscoveryListener, NsdManager.DiscoveryListener>()
    private val registrations = IdentityHashMap<NsdManager.RegistrationListener, NsdManager.RegistrationListener>()

    fun discover(type: String, listener: NsdManager.DiscoveryListener) {
        if (current) {
            nsd.discoverServices(type, NsdManager.PROTOCOL_DNS_SD, null, executor, listener)
        } else {
            val moved = MovedDiscovery(listener, executor)
            discoveries[listener] = moved
            nsd.discoverServices(type, NsdManager.PROTOCOL_DNS_SD, moved)
        }
    }

    fun stopDiscovery(listener: NsdManager.DiscoveryListener) {
        nsd.stopServiceDiscovery(discoveries.remove(listener) ?: listener)
    }

    fun register(info: NsdServiceInfo, listener: NsdManager.RegistrationListener) {
        if (current) {
            nsd.registerService(info, NsdManager.PROTOCOL_DNS_SD, executor, listener)
        } else {
            val moved = MovedRegistration(listener, executor)
            registrations[listener] = moved
            nsd.registerService(info, NsdManager.PROTOCOL_DNS_SD, moved)
        }
    }

    fun unregister(listener: NsdManager.RegistrationListener) {
        nsd.unregisterService(registrations.remove(listener) ?: listener)
    }

    /**
     * Follow [info] for what it resolves to: [found] with each resolution that has an address
     * and a port, [lost] where Android 14 says it went (below, the discovery's own lost says
     * it), [failed] with the error where it could not be followed.
     */
    fun follow(info: NsdServiceInfo, found: (NsdResolved) -> Unit, lost: () -> Unit, failed: (Int) -> Unit): NsdFollowing {
        if (current) {
            val callback = object : NsdManager.ServiceInfoCallback {
                override fun onServiceUpdated(resolved: NsdServiceInfo) {
                    val addresses = resolved.hostAddresses.mapNotNull(NsdResolved::address)
                    if (addresses.isNotEmpty() && resolved.port != 0) found(NsdResolved.of(resolved, addresses))
                }
                override fun onServiceLost() = lost()
                override fun onServiceInfoCallbackRegistrationFailed(errorCode: Int) = failed(errorCode)
                override fun onServiceInfoCallbackUnregistered() {}
            }
            nsd.registerServiceInfoCallback(info, executor, callback)
            return NsdFollowing { runCatching { nsd.unregisterServiceInfoCallback(callback) } }
        }
        // Answered on the executor, which the resolve's own callbacks move to.
        val request = Resolves.Request(info) { resolved, error ->
            if (resolved == null) {
                failed(error)
            } else {
                @Suppress("DEPRECATION")
                val addresses = listOfNotNull(NsdResolved.address(resolved.host))
                if (addresses.isNotEmpty() && resolved.port != 0) found(NsdResolved.of(resolved, addresses))
            }
        }
        Resolves.queue.add(request) { item, done ->
            val listener = object : NsdManager.ResolveListener {
                override fun onServiceResolved(resolved: NsdServiceInfo) = executor.execute { done(resolved, 0) }
                override fun onResolveFailed(info: NsdServiceInfo, errorCode: Int) = executor.execute { done(null, errorCode) }
            }
            @Suppress("DEPRECATION")
            runCatching { nsd.resolveService(item.info, listener) }
                .onFailure { done(null, NsdManager.FAILURE_INTERNAL_ERROR) }
        }
        return NsdFollowing { Resolves.queue.remove(request) }
    }

    fun unfollow(following: NsdFollowing) = following.stop()

    private companion object {
        /** Android 14: the executor's calls, and following a service through its changes. */
        @ChecksSdkIntAtLeast(api = Build.VERSION_CODES.UPSIDE_DOWN_CAKE)
        val current: Boolean = Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE
    }
}

/**
 * The resolves below Android 14, one at a time across the app. Every user's executor is the main
 * thread's, where the queue is kept.
 */
internal object Resolves {
    class Request(val info: NsdServiceInfo, val answer: (NsdServiceInfo?, Int) -> Unit)

    val queue = ResolveQueue<Request, NsdServiceInfo> { request, resolved, error -> request.answer(resolved, error) }
}

/**
 * Items resolved one at a time: the next starts once the last answered. One taken off while it
 * waits never starts; one taken off while it runs answers no one. Used from one thread.
 */
internal class ResolveQueue<T : Any, R>(private val answer: (T, R?, Int) -> Unit) {
    private val waiting = ArrayDeque<Pair<T, (T, (R?, Int) -> Unit) -> Unit>>()
    private var running: T? = null
    private var dropped = false

    /** Queue [item], which [start] resolves when its turn comes, calling back once with the result. */
    fun add(item: T, start: (T, (R?, Int) -> Unit) -> Unit) {
        waiting.addLast(item to start)
        next()
    }

    fun remove(item: T) {
        waiting.removeAll { it.first === item }
        if (running === item) dropped = true
    }

    /** Whether nothing runs or waits. */
    val idle: Boolean get() = running == null && waiting.isEmpty()

    private fun next() {
        if (running != null) return
        val (item, start) = waiting.removeFirstOrNull() ?: return
        running = item
        dropped = false
        var answered = false
        start(item) { result, error ->
            if (answered) return@start
            answered = true
            val keep = !dropped
            running = null
            if (keep) answer(item, result, error)
            next()
        }
    }
}

/** A discovery's callbacks, from NsdManager's own thread below Android 14 onto [executor]. */
private class MovedDiscovery(private val to: NsdManager.DiscoveryListener, private val executor: Executor) : NsdManager.DiscoveryListener {
    override fun onServiceFound(info: NsdServiceInfo) = executor.execute { to.onServiceFound(info) }
    override fun onServiceLost(info: NsdServiceInfo) = executor.execute { to.onServiceLost(info) }
    override fun onDiscoveryStarted(serviceType: String) = executor.execute { to.onDiscoveryStarted(serviceType) }
    override fun onDiscoveryStopped(serviceType: String) = executor.execute { to.onDiscoveryStopped(serviceType) }
    override fun onStartDiscoveryFailed(serviceType: String, errorCode: Int) = executor.execute { to.onStartDiscoveryFailed(serviceType, errorCode) }
    override fun onStopDiscoveryFailed(serviceType: String, errorCode: Int) = executor.execute { to.onStopDiscoveryFailed(serviceType, errorCode) }
}

/** A registration's callbacks, moved the same way. */
private class MovedRegistration(private val to: NsdManager.RegistrationListener, private val executor: Executor) : NsdManager.RegistrationListener {
    override fun onServiceRegistered(info: NsdServiceInfo) = executor.execute { to.onServiceRegistered(info) }
    override fun onRegistrationFailed(info: NsdServiceInfo, errorCode: Int) = executor.execute { to.onRegistrationFailed(info, errorCode) }
    override fun onServiceUnregistered(info: NsdServiceInfo) = executor.execute { to.onServiceUnregistered(info) }
    override fun onUnregistrationFailed(info: NsdServiceInfo, errorCode: Int) = executor.execute { to.onUnregistrationFailed(info, errorCode) }
}
