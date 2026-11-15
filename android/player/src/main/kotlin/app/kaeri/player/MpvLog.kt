// mpv's log lines, as logcat and the engine's player log may hold them: every address cut to its
// scheme, host and port, as the engine's log says one (`logged::address`), since a stream's carries
// its signed token in its path or query.

package app.kaeri.player

import android.util.Log
import app.kaeri.engine.ffi.FfiLogLevel
import app.kaeri.engine.ffi.logLine

object MpvLog {
    /** The tag mpv's lines go under. */
    const val TAG = "mpv"

    /** An address in running text: a scheme and `://`, and everything up to a space or a quote. */
    private val addresses = Regex("""[A-Za-z][A-Za-z0-9+.\-]*://[^\s"'<>]+""")

    /**
     * `https://cdn.example`, from `https://cdn.example/a/b.m3u8?token=…`: the scheme, the host and
     * the port, never the credentials, the path or the query.
     */
    fun address(url: String): String {
        val scheme = url.substringBefore("://", "")
        if (scheme.isEmpty()) return if (url.startsWith("/")) "a local file" else "an address"
        val authority = url.substringAfter("://").split('/', '?', '#').first()
        return "$scheme://${authority.substringAfterLast('@')}"
    }

    /** [text] with every address in it said as [address] says one. */
    fun redacting(text: String): String = addresses.replace(text) { address(it.value) }

    /** One of mpv's `log-message` events as logcat is to say it, at its level, or null for none. */
    fun said(event: MpvEvent): Pair<Int, String>? {
        val value = event.value
        val text = value["text"].string ?: return null
        val priority = when (value["level"].string) {
            "fatal", "error" -> Log.ERROR
            "warn" -> Log.WARN
            "info" -> Log.INFO
            "v", "debug" -> Log.DEBUG
            else -> Log.VERBOSE
        }
        return priority to "[${event.name ?: "mpv"}] ${redacting(text)}"
    }

    /** The engine's level for a line [said] at [priority], or null below info: the player log keeps info and up. */
    fun kept(priority: Int): FfiLogLevel? = when {
        priority >= Log.ERROR -> FfiLogLevel.ERROR
        priority == Log.WARN -> FfiLogLevel.WARN
        priority == Log.INFO -> FfiLogLevel.INFO
        else -> null
    }

    /**
     * Hands one line [said] to the engine's player log under source `mpv`, its module in brackets
     * as mpv names it. The engine keeps it with the playback under way, and nothing before one.
     */
    fun keep(priority: Int, line: String) {
        val level = kept(priority) ?: return
        runCatching { logLine(level, TAG, line) }
    }
}
