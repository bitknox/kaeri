// Crash reports, kept on the device and only while the viewer has turned saving on. The engine
// owns the files, the redaction and the limits; this file turns what Android knows of an exit
// into the engine's record, and holds the lists the screens show.

package app.kaeri.store

import app.kaeri.engine.ffi.crashAddHeld
import app.kaeri.engine.ffi.crashDelete
import app.kaeri.engine.ffi.crashDeleteAll
import app.kaeri.engine.ffi.crashList
import app.kaeri.engine.ffi.crashOfferHeld
import app.kaeri.engine.ffi.crashPendingNotice
import app.kaeri.engine.ffi.crashSave
import app.kaeri.engine.ffi.crashText
import app.kaeri.engine.model.CrashFrame
import app.kaeri.engine.model.CrashKind
import app.kaeri.engine.model.CrashOrigin
import app.kaeri.engine.model.CrashPlace
import app.kaeri.engine.model.CrashRecord
import app.kaeri.engine.model.CrashSummary
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/** What the system kept of one process exit, with the trace when it kept one. */
data class SystemExit(
    val kind: CrashKind,
    val atMs: Long,
    /** The signal number of a native crash. */
    val signal: Int?,
    val place: CrashPlace,
    val trace: String?,
)

/** What a record says of the app and the device, which does not change within a run. */
data class CrashFacts(
    val appVersion: String,
    val osVersion: String,
    val sources: Int,
    val demoOn: Boolean,
)

object ExitRecords {
    /** Native frames of a tombstone: `#00 pc 00000000001a42c0  /data/app/x/lib/arm64/libmpv.so (name+12)`. */
    private val NATIVE = Regex("""#\d+\s+pc\s+([0-9a-fA-F]+)\s+(\S+)""")

    /** Java frames of an ANR's thread dump: `at app.kaeri.ui.Home.draw(Home.kt:88)`. */
    private val JAVA = Regex("""\bat\s+([\w.$]+)\.([\w$<>]+)\(([^)]*)\)""")

    private const val OURS = "app.kaeri"
    private const val FALLBACK_NATIVE = 8

    /** Frames from a trace, Kaeri and libmpv only, as module and offset or class and method. */
    fun frames(trace: String?): List<CrashFrame> {
        if (trace.isNullOrBlank()) return emptyList()
        val native = NATIVE.findAll(trace).map { m ->
            CrashFrame(module = m.groupValues[2].substringAfterLast('/'), symbol = null, offset = m.groupValues[1].toULongOrNull(16) ?: 0uL)
        }.toList()
        val ours = native.filter { it.module.startsWith("libmpv") || it.module.contains("kaeri") }
        val java = JAVA.findAll(trace).filter { it.groupValues[1].startsWith(OURS) }.map { m ->
            val line = m.groupValues[3].substringAfterLast(':', "").toULongOrNull() ?: 0uL
            CrashFrame(module = m.groupValues[1], symbol = m.groupValues[2], offset = line)
        }.toList()
        // A trace with none of ours still says where it was, with the first few frames.
        val chosen = ours.ifEmpty { native.take(FALLBACK_NATIVE) } + java
        return chosen.take(MAX_FRAMES)
    }

    /** The signal's name, for a native crash's reason. */
    fun signalName(signal: Int?): String = when (signal) {
        4 -> "SIGILL"
        6 -> "SIGABRT"
        7 -> "SIGBUS"
        8 -> "SIGFPE"
        11 -> "SIGSEGV"
        null -> "native crash"
        else -> "signal $signal"
    }

    /** The record for [exit]: [origin] says whether the viewer's own saving kept it or the system did. */
    fun record(exit: SystemExit, facts: CrashFacts, origin: CrashOrigin, nowMs: Long, uptimeSecs: Long = 0): CrashRecord = CrashRecord(
        id = "${if (origin == CrashOrigin.HELD_BY_SYSTEM) "held" else "exit"}-${exit.kind.name.lowercase()}-${exit.atMs}",
        atMs = exit.atMs,
        uptimeSecs = uptimeSecs.toULong(),
        platform = "Android",
        osVersion = facts.osVersion,
        appVersion = facts.appVersion,
        arch = "arm64",
        place = exit.place,
        detail = "",
        kind = exit.kind,
        reason = when (exit.kind) {
            CrashKind.SIGNAL -> signalName(exit.signal)
            CrashKind.ANR -> "not responding"
            else -> "uncaught exception"
        },
        frames = frames(exit.trace),
        sources = facts.sources.toUInt(),
        demoOn = facts.demoOn,
        origin = origin,
    )

    /** The record for an exception nothing caught, in the Kotlin code: class and method of each frame. */
    fun record(error: Throwable, facts: CrashFacts, place: CrashPlace, nowMs: Long, uptimeSecs: Long): CrashRecord = CrashRecord(
        id = "kt-$nowMs",
        atMs = nowMs,
        uptimeSecs = uptimeSecs.toULong(),
        platform = "Android",
        osVersion = facts.osVersion,
        appVersion = facts.appVersion,
        arch = "arm64",
        place = place,
        detail = error.javaClass.name,
        kind = CrashKind.EXCEPTION,
        reason = error.javaClass.name,
        frames = error.stackTrace.filter { it.className.startsWith(OURS) }.take(MAX_FRAMES).map {
            CrashFrame(module = it.className, symbol = it.methodName, offset = it.lineNumber.coerceAtLeast(0).toULong())
        },
        sources = facts.sources.toUInt(),
        demoOn = facts.demoOn,
        origin = CrashOrigin.SAVED,
    )

    const val MAX_FRAMES = 40
}

/**
 * The saved reports, the system's records offered after saving is switched on, and the crash to
 * mention at launch. Every call here is the engine's; nothing is sent.
 */
class CrashReports {
    private val _saved = MutableStateFlow<List<CrashSummary>>(emptyList())
    val saved: StateFlow<List<CrashSummary>> = _saved.asStateFlow()

    private val _held = MutableStateFlow<List<CrashSummary>>(emptyList())

    /** The records the system kept from before saving was turned on, each with View and Add. */
    val held: StateFlow<List<CrashSummary>> = _held.asStateFlow()

    /** A Report a bug that is open: its last error, and the crashes ticked as it opened. */
    data class ReportRequest(val problem: BugContext?, val ticked: Set<String>)

    private val _report = MutableStateFlow<ReportRequest?>(null)
    val report: StateFlow<ReportRequest?> = _report.asStateFlow()

    fun askReport(problem: BugContext?, ticked: Set<String> = emptySet()) {
        _report.value = ReportRequest(problem, ticked)
    }

    fun closeReport() {
        _report.value = null
    }

    /** Set by the collector: says where the app is. */
    var onPlace: (CrashPlace) -> Unit = {}

    fun place(place: CrashPlace) = onPlace(place)

    fun refresh() {
        _saved.value = runCatching { crashList() }.getOrDefault(emptyList())
    }

    fun text(id: String): String? = runCatching { crashText(id) }.getOrNull()

    fun delete(id: String) {
        crashDelete(id)
        refresh()
    }

    fun deleteAll() {
        crashDeleteAll()
        refresh()
    }

    fun save(record: CrashRecord): Boolean = runCatching { crashSave(record) }.getOrDefault(false).also { if (it) refresh() }

    fun offerHeld(records: List<CrashRecord>) {
        _held.value = runCatching { crashOfferHeld(records) }.getOrDefault(emptyList())
    }

    fun addHeld(id: String) {
        if (crashAddHeld(id)) _held.value = _held.value.filterNot { it.id == id }
        refresh()
    }

    fun clearHeld() {
        _held.value = emptyList()
    }

    /** The newest crash not yet mentioned, once, and only while saving and asking are both on. */
    fun pendingNotice(): CrashSummary? = runCatching { crashPendingNotice() }.getOrNull()
}
