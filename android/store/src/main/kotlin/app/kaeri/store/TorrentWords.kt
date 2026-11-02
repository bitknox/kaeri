// What the phone and the TV say of the torrent proxy and its check, in one place so both say it
// the same way.

package app.kaeri.store

import android.content.Context
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.engine.model.ProxyStatus

object TorrentWords {
    /** What the proxy check does, said over the proxy's form. */
    val CHECK_NOTE = R.string.torrents_check_note

    /** The line over the proxy's form: why a proxy is worth setting. */
    val WHY = R.string.torrents_risk

    /** What the switch says under it while torrents are off. */
    val OFF_NOTE = R.string.torrents_off_detail

    /** Edit proxy's line over its form. */
    val EDIT_NOTE = R.string.torrents_proxy_keeps_saved

    /** Under the password, on Edit proxy. */
    val PASSWORD_KEPT = R.string.torrents_password_keep

    /** Said once a proxy passes its check and is saved. */
    val SAID_PROXY = R.string.torrents_proxy_saved

    /** While a proxy's check runs, before it is saved. */
    val CHECKING_PROXY = R.string.torrents_checking_proxy

    /** Why a proxy was not saved: the check failed, and the saved proxy stays. */
    fun proxyFailed(context: Context): String =
        Strings.torrentsProxyNotSaved(context, Strings.torrentsCheckNotMadeProxy(context))

    /** Under the switch while torrents are on: where traffic goes. */
    fun summary(context: Context, host: String?, port: String?): String =
        if (host == null || port == null) Strings.torrentsSummaryDirect(context) else Strings.torrentsSummaryProxyAt(context, host, port)

    /** "proxy.example:1080 · as mika": where the proxy is, and who it lets in. */
    fun proxyLine(context: Context, host: String, port: String, user: String): String =
        listOfNotNull("${host.trim()}:${port.trim()}", user.trim().takeIf { it.isNotEmpty() }?.let { Strings.torrentsProxyAs(context, it) })
            .joinToString(" · ") // i18n-skip

    /** Whether a proxy's [host] and [port] are whole. */
    fun proxyOk(host: String, port: String): Boolean =
        host.isNotBlank() && port.trim().toIntOrNull()?.let { it in 1..65535 } == true

    /** The check's verdict as a row says it: its status, the line under it, and its dot. */
    data class Check(val status: String, val note: String?, val problem: Boolean, val passed: Boolean)

    /** The saved proxy's check as its row says it; the row is shown only while a proxy is set. */
    fun check(context: Context, status: ProxyStatus?): Check = when (status) {
        ProxyStatus.PASSED -> Check(Strings.torrentsCheckPassed(context), null, problem = false, passed = true)
        ProxyStatus.CHECKING -> Check(Strings.torrentsCheckRunning(context), null, problem = false, passed = false)
        ProxyStatus.FAILED -> Check(Strings.torrentsCheckFailed(context), Strings.torrentsCheckFailedLine(context), problem = true, passed = false)
        ProxyStatus.DIRECT, null -> Check(Strings.torrentsNotChecked(context), null, problem = false, passed = false)
    }
}
