// The clock the player keeps time by in the app flavour: the system's, always. The fixtures flavour
// has one a still shot can hold (src/fixtures).

package app.kaeri.player

import android.os.SystemClock

/** Now for the player's cards and starting screen, in milliseconds since the device booted. */
object PlayerTime {
    fun now(): Long = SystemClock.elapsedRealtime()
}
