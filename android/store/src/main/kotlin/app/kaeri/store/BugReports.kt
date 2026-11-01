// "Report a bug": the address of a pre-filled GitHub issue, which the app opens in the browser.
// The engine writes the text, the same on every front end, and strips what is private from it;
// this adds what only the app knows about itself.

package app.kaeri.store

import app.kaeri.engine.ffi.bugReportUrl
import app.kaeri.engine.ffi.bugReportUrlCapped
import app.kaeri.engine.ffi.reportText
import app.kaeri.engine.model.BugError
import app.kaeri.engine.model.BugReport
import app.kaeri.engine.model.DeviceClass
import app.kaeri.engine.model.ProviderRow

/** What a bug report says about this app and this device. */
data class BugReportDevice(
    val appVersion: String,
    val osVersion: String,
    val device: DeviceClass,
    val build: String,
    val platform: String = "Android",
)

/** An error a report starts from: what kind of problem, and what it said. */
data class BugContext(
    val kind: String,
    val message: String,
    /** The title of the show it was about, which the report removes wherever it appears. */
    val title: String? = null,
)

object BugReports {
    /** The tracker's address when the engine's cannot be used. */
    const val FALLBACK = "https://github.com/bitknox/kaeri/issues/new"

    /** The report for [device], the engine's sources, an optional error and the saved crashes to include. */
    fun report(device: BugReportDevice, providers: List<ProviderRow>, context: BugContext?, crashes: List<String> = emptyList()) = BugReport(
        appVersion = device.appVersion,
        platform = device.platform,
        osVersion = device.osVersion,
        device = device.device,
        build = device.build,
        providers = providers,
        error = context?.let { BugError(kind = it.kind, message = it.message) },
        `private` = listOfNotNull(context?.title),
        crashes = crashes,
    )

    /** The address for [device], the engine's sources and an optional error. */
    fun url(device: BugReportDevice, providers: List<ProviderRow>, context: BugContext?, crashes: List<String> = emptyList()): String =
        bugReportUrl(report(device, providers, context, crashes))

    /** The address with the summary cut to [maxBytes]: the TV's code stays readable from the sofa. */
    fun urlCapped(device: BugReportDevice, providers: List<ProviderRow>, context: BugContext?, crashes: List<String>, maxBytes: Int): String =
        bugReportUrlCapped(report(device, providers, context, crashes), maxBytes.toUInt())

    /** The exact text the report view shows, and Copy report puts on the clipboard. */
    fun text(device: BugReportDevice, providers: List<ProviderRow>, context: BugContext?, crashes: List<String>): String =
        reportText(report(device, providers, context, crashes))
}

/** The address of a new issue for this device, with [context] as its last error. */
fun AppStore.bugReportUrl(device: BugReportDevice, context: BugContext? = null, crashes: List<String> = emptyList()): String =
    BugReports.url(device, providers.rows.value, context, crashes)

/** The report's text for [device], with the error and crashes it includes. */
fun AppStore.bugReportText(device: BugReportDevice, context: BugContext?, crashes: List<String>): String =
    BugReports.text(device, providers.rows.value, context, crashes)
