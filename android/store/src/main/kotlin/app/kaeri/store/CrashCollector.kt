// Collects crashes on Android, and only while saving crash reports is on: a default uncaught
// exception handler for the Kotlin stack, and the system's own exit records (ApplicationExitInfo)
// read at launch. While saving is off nothing here is installed and nothing is read.

package app.kaeri.store

import android.app.ActivityManager
import android.app.ApplicationExitInfo
import android.content.Context
import app.kaeri.engine.model.CrashKind
import app.kaeri.engine.model.CrashOrigin
import app.kaeri.engine.model.CrashPlace
import app.kaeri.engine.model.SettingKey
import kotlinx.coroutines.launch

class CrashCollector(
    private val context: Context,
    private val reports: CrashReports,
    private val facts: () -> CrashFacts,
    private val now: () -> Long = System::currentTimeMillis,
) {
    private val started = now()
    private var installed = false
    private var previous: Thread.UncaughtExceptionHandler? = null
    private var handling = false
    private var seenSetting = false
    private var wasOn = false

    @Volatile
    private var place = CrashPlace.OTHER

    private val prefs get() = context.getSharedPreferences("crash", Context.MODE_PRIVATE)

    /** Where the app is, for a crash's record: set from the screen shown, and kept with the system for its exit record. */
    fun place(place: CrashPlace) {
        if (this.place == place) return
        this.place = place
        if (!wasOn) return
        runCatching { context.getSystemService(ActivityManager::class.java).setProcessStateSummary(byteArrayOf(place.ordinal.toByte())) }
    }

    /**
     * The setting's value, now and each time it changes. At launch with saving on, the exits since
     * the last read are saved; when it is switched on later, the week before is offered, never taken.
     */
    fun saving(on: Boolean) {
        val atLaunch = !seenSetting
        seenSetting = true
        if (on == wasOn && !atLaunch) return
        wasOn = on
        if (!on) {
            uninstall()
            reports.clearHeld()
            return
        }
        install()
        reports.refresh()
        if (atLaunch) {
            saveSince(prefs.getLong(READ_UNTIL, started))
        } else {
            prefs.edit().putLong(READ_UNTIL, started).apply()
            reports.offerHeld(exits(since = started - HELD_MS).map { ExitRecords.record(it, facts(), CrashOrigin.HELD_BY_SYSTEM, now()) })
        }
    }

    private fun install() {
        if (installed) return
        installed = true
        val previous = Thread.getDefaultUncaughtExceptionHandler().also { this.previous = it }
        Thread.setDefaultUncaughtExceptionHandler { thread, error ->
            // Never throws, and never loops if saving itself fails.
            if (!handling) {
                handling = true
                runCatching {
                    val at = now()
                    reports.save(ExitRecords.record(error, facts(), place, at, (at - started) / 1000))
                }
            }
            previous?.uncaughtException(thread, error)
        }
    }

    private fun uninstall() {
        if (!installed) return
        installed = false
        Thread.setDefaultUncaughtExceptionHandler(previous)
        previous = null
    }

    private fun saveSince(since: Long) {
        // The Kotlin handler already kept a plain exception, so only native crashes and not responding are read.
        exits(since).filter { it.kind != CrashKind.EXCEPTION }.forEach {
            reports.save(ExitRecords.record(it, facts(), CrashOrigin.SAVED, now()))
        }
        prefs.edit().putLong(READ_UNTIL, started).apply()
    }

    /** The system's exit records for this app after [since]: crashes, native crashes and ANRs only. */
    private fun exits(since: Long): List<SystemExit> = runCatching {
        val am = context.getSystemService(ActivityManager::class.java)
        am.getHistoricalProcessExitReasons(context.packageName, 0, MAX_EXITS).mapNotNull { info ->
            if (info.timestamp <= since) return@mapNotNull null
            val kind = when (info.reason) {
                ApplicationExitInfo.REASON_CRASH -> CrashKind.EXCEPTION
                ApplicationExitInfo.REASON_CRASH_NATIVE -> CrashKind.SIGNAL
                ApplicationExitInfo.REASON_ANR -> CrashKind.ANR
                else -> return@mapNotNull null
            }
            val trace = if (kind == CrashKind.EXCEPTION) null else runCatching {
                info.traceInputStream?.bufferedReader()?.use { it.readText().take(MAX_TRACE) }
            }.getOrNull()
            val at = info.processStateSummary?.firstOrNull()?.toInt()?.let { CrashPlace.entries.getOrNull(it) } ?: CrashPlace.OTHER
            SystemExit(kind, info.timestamp, info.status.takeIf { kind == CrashKind.SIGNAL }, at, trace)
        }
    }.getOrDefault(emptyList())

    private companion object {
        const val READ_UNTIL = "exits_read_until"
        const val HELD_MS = 7L * 24 * 3600 * 1000
        const val MAX_EXITS = 32
        const val MAX_TRACE = 200_000
    }
}

/**
 * Start collecting crashes for this app, following the Save crash reports setting. Nothing is
 * installed or read until the setting is on.
 */
fun AppStore.collectCrashes(context: Context, scope: kotlinx.coroutines.CoroutineScope) {
    val app = context.applicationContext
    val version = runCatching { app.packageManager.getPackageInfo(app.packageName, 0).versionName }.getOrNull().orEmpty()
    val collector = CrashCollector(
        app,
        crashes,
        facts = {
            CrashFacts(
                appVersion = version,
                osVersion = android.os.Build.VERSION.RELEASE.orEmpty(),
                sources = providers.rows.value.count { !it.isDemo },
                demoOn = demo.value.on,
            )
        },
    )
    crashes.onPlace = collector::place
    scope.launch {
        settings.view.collect { view ->
            if (view != null) collector.saving(settings.isOn(SettingKey.SAVE_CRASH_REPORTS, view))
        }
    }
}
