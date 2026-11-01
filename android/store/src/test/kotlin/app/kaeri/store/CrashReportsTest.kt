package app.kaeri.store

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import app.kaeri.engine.model.CrashKind
import app.kaeri.engine.model.CrashOrigin
import app.kaeri.engine.model.CrashPlace

/** What the app hands the engine for a crash: the frames it keeps, the reasons it names, and that nothing is saved while saving is off. */
class CrashReportsTest {
    private val facts = CrashFacts(appVersion = "0.1.0", osVersion = "16", sources = 2, demoOn = true)

    private val tombstone = """
        backtrace:
            #00 pc 00000000001a42c0  /data/app/~~x/app.kaeri.app/lib/arm64/libmpv.so (mpv_render_context_render+88)
            #01 pc 000000000009f118  /data/app/~~x/app.kaeri.app/lib/arm64/libmpv.so
            #02 pc 0000000000012abc  /apex/com.android.runtime/lib64/bionic/libc.so (abort+168)
            #03 pc 0000000000004444  /data/app/~~x/app.kaeri.app/lib/arm64/libkaeri_ffi.so
    """.trimIndent()

    @Test
    fun a_tombstone_keeps_kaeri_and_libmpv_frames_as_module_and_offset() {
        val frames = ExitRecords.frames(tombstone)
        assertEquals(listOf("libmpv.so", "libmpv.so", "libkaeri_ffi.so"), frames.map { it.module })
        assertEquals(0x1a42c0uL, frames[0].offset)
        assertTrue(frames.all { it.symbol == null })
    }

    @Test
    fun a_trace_with_none_of_ours_keeps_its_first_frames() {
        val other = (0 until 12).joinToString("\n") { "#%02d pc %016x  /system/lib64/libc.so".format(it, it) }
        assertEquals(8, ExitRecords.frames(other).size)
    }

    @Test
    fun an_anr_keeps_our_java_frames_as_class_and_method() {
        val dump = """
            "main" prio=5 tid=1 Blocked
              at java.lang.Object.wait(Native Method)
              at app.kaeri.ui.HomeKt.draw(Home.kt:88)
              at app.kaeri.store.AppStore.send(AppStore.kt:12)
        """.trimIndent()
        val frames = ExitRecords.frames(dump)
        assertEquals(listOf("app.kaeri.ui.HomeKt", "app.kaeri.store.AppStore"), frames.map { it.module })
        assertEquals("draw", frames[0].symbol)
        assertEquals(88uL, frames[0].offset)
    }

    @Test
    fun no_trace_gives_no_frames() {
        assertTrue(ExitRecords.frames(null).isEmpty())
        assertTrue(ExitRecords.frames("  ").isEmpty())
    }

    @Test
    fun a_native_crash_names_its_signal() {
        val exit = SystemExit(CrashKind.SIGNAL, atMs = 1_000, signal = 11, place = CrashPlace.PLAYER, trace = tombstone)
        val record = ExitRecords.record(exit, facts, CrashOrigin.SAVED, nowMs = 2_000)
        assertEquals("SIGSEGV", record.reason)
        assertEquals("Android", record.platform)
        assertEquals(CrashPlace.PLAYER, record.place)
        assertEquals(2u, record.sources)
        assertTrue(record.demoOn)
        assertEquals(CrashOrigin.SAVED, record.origin)
    }

    @Test
    fun a_record_the_system_kept_is_marked_as_held() {
        val exit = SystemExit(CrashKind.ANR, atMs = 5, signal = null, place = CrashPlace.OTHER, trace = null)
        val record = ExitRecords.record(exit, facts, CrashOrigin.HELD_BY_SYSTEM, nowMs = 9)
        assertEquals(CrashOrigin.HELD_BY_SYSTEM, record.origin)
        assertEquals("not responding", record.reason)
        assertTrue(record.id.startsWith("held-"))
    }

    @Test
    fun an_uncaught_exception_keeps_our_frames_and_the_class_only() {
        val error = IllegalStateException("The title Kumo no Ue was missing")
        val record = ExitRecords.record(error, facts, CrashPlace.HOME, nowMs = 3, uptimeSecs = 40)
        assertEquals(CrashKind.EXCEPTION, record.kind)
        assertEquals("java.lang.IllegalStateException", record.reason)
        assertFalse("the message never reaches the record", record.detail.contains("Kumo"))
        assertEquals(40uL, record.uptimeSecs)
    }

    @Test
    fun a_signal_without_a_number_still_has_a_reason() {
        assertEquals("native crash", ExitRecords.signalName(null))
        assertEquals("signal 31", ExitRecords.signalName(31))
    }

    @Test
    fun saving_is_off_so_nothing_is_saved_or_listed() {
        val reports = CrashReports()
        val exit = SystemExit(CrashKind.SIGNAL, atMs = 1, signal = 6, place = CrashPlace.OTHER, trace = tombstone)
        assertFalse(reports.save(ExitRecords.record(exit, facts, CrashOrigin.SAVED, 2)))
        reports.refresh()
        assertTrue(reports.saved.value.isEmpty())
        assertTrue(reports.held.value.isEmpty())
        assertEquals(null, reports.pendingNotice())
    }
}
