// The clock the player keeps time by in the fixtures flavour: the system's, until a still shot turns
// the engine's still clock on, and from then that clock, so the starting screen's seconds and
// wording are those of the moment the shot is held at, however slow the emulator. Not in the app
// flavour, which never links the engine's testing calls.

package app.kaeri.player

import android.os.SystemClock
import app.kaeri.engine.ffi.testingStillClockHold
import app.kaeri.engine.ffi.testingStillClockHoldSeconds
import app.kaeri.engine.ffi.testingStillClockNowMs
import kotlinx.coroutines.delay
import kotlinx.coroutines.withTimeoutOrNull

/**
 * Now for the player's cards and starting screen, in milliseconds: the system's clock since boot,
 * or, once [holdSeconds] or [hold] has turned the still clock on, the moment it was turned on plus
 * the still clock's time. It only moves forward, as the system's does.
 */
object PlayerTime {
    /** The system's clock when the still clock was turned on; null while it is off. */
    @Volatile private var base: Long? = null

    fun now(): Long {
        val base = base ?: return SystemClock.elapsedRealtime()
        return base + (testingStillClockNowMs()?.toLong() ?: 0L)
    }

    /** Now on the still clock, in milliseconds since it was turned on; null while it is off. */
    fun still(): Long? = if (base == null) null else testingStillClockNowMs()?.toLong()

    /**
     * Turn the still clock on held at [seconds] as a launch argument says them ("24", "0.3"), or
     * move the hold on: the fixture scene's made-up waits due by then end at once. False, and the
     * clock left as it was, for words that are not seconds.
     */
    fun holdSeconds(seconds: String): Boolean {
        if (seconds.trim().toDoubleOrNull()?.takeIf { it.isFinite() && it >= 0 } == null) return false
        turningOn()
        return testingStillClockHoldSeconds(seconds)
    }

    /** Turn the still clock on held at [atMs], or move the hold on to it (never back). */
    fun hold(atMs: Long) {
        turningOn()
        testingStillClockHold(atMs.coerceAtLeast(0).toULong())
    }

    /**
     * Hold the still clock at [atMs] and wait, in real time, until it is there: it rests a moment
     * on each made-up wait that ends on the way. False if it never got there.
     */
    suspend fun reach(atMs: Long, timeoutMillis: Long = 10_000): Boolean {
        hold(atMs)
        return withTimeoutOrNull(timeoutMillis) {
            while ((still() ?: 0L) < atMs) delay(5)
            true
        } ?: false
    }

    @Synchronized
    private fun turningOn() {
        // Before the first hold, while the still clock is still at nought.
        if (base == null) base = SystemClock.elapsedRealtime()
    }
}
