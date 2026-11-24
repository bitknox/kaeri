// The player panel's Sources tab while its sources answer, shot on a device: the Fixtures app on
// the SourcesLoading scene plays episode 7, opens Sources once the picture shows, and this test
// takes the screen 0.3, 1, 4, 12 and 21 s after the tab asked its sources. The app logs that moment as
// "KAERI_PANEL sources <uptime ms>" under the tag KaeriPanel (PanelDemo.openSources).
//
// The shots land in the app's external files, sources-loading/phone-<moment>.png, for adb pull.
// The instrumentation argument media (am instrument -e media <path>) names the episode the scene
// plays, a file the app can read such as one pushed to /data/local/tmp. CI runs no emulator, so
// the same moments are checked on the JVM by PlayerShots and SourcesLoadingTest.
package app.kaeri.app.panel

import android.content.Intent
import android.os.SystemClock
import android.util.Log
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class SourcesLoadingShots {
    private val instrumentation = InstrumentationRegistry.getInstrumentation()

    @Test
    fun sourcesTabAsItsSourcesAnswer() {
        shell("logcat -c")
        // The system's one-time note on full screen would sit over the player (the emulator's
        // read-only AVD forgets this when it stops).
        shell("settings put secure immersive_mode_confirmations confirmed")
        val context = instrumentation.targetContext
        val intent = Intent()
            .setClassName(context.packageName, "app.kaeri.app.MainActivity")
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK)
            .putExtra("scene", "sourcesloading")
            .putExtra("panel", "sources")
        InstrumentationRegistry.getArguments().getString("media")?.let { intent.putExtra("media", it) }
        context.startActivity(intent)
        val opened = opened()
        val out = File(context.getExternalFilesDir(null), "sources-loading").apply { mkdirs() }
        val late = mutableListOf<String>()
        // Each shot is written once all are taken: a full-screen PNG took over a second to encode
        // on the PC's emulator, which pushed the next moment past its 0.3 s.
        val shots = mutableListOf<Pair<String, android.graphics.Bitmap>>()
        for ((moment, at) in MOMENTS) {
            SystemClock.sleep((opened + at - SystemClock.uptimeMillis()).coerceAtLeast(0))
            val taken = SystemClock.uptimeMillis() - opened
            shots += "phone-$moment" to instrumentation.uiAutomation.takeScreenshot()
            Log.i(TAG, "KAERI_SHOT phone-$moment at $taken ms")
            if (taken - at !in 0..300) late += "$moment at $taken ms"
            // The sheet shows the first two other sources; the rest are a scroll down, as a
            // person would look, before the 4 s moment.
            if (moment == "1s") scrollSheet()
        }
        for ((name, shot) in shots) {
            File(out, "$name.png").outputStream().use { shot.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, it) }
        }
        assertTrue("taken outside 0.3 s of the moment: $late", late.isEmpty())
    }

    /** A slow drag up the side sheet, which stops where the finger lifts. */
    private fun scrollSheet() {
        val device = androidx.test.uiautomator.UiDevice.getInstance(instrumentation)
        val x = device.displayWidth - (180 * instrumentation.targetContext.resources.displayMetrics.density).toInt()
        device.swipe(x, device.displayHeight * 9 / 10, x, device.displayHeight * 3 / 10, 40)
    }

    /** The uptime the app logged when it opened Sources, waited for while the scene starts. */
    private fun opened(): Long {
        val deadline = SystemClock.uptimeMillis() + 150_000
        while (SystemClock.uptimeMillis() < deadline) {
            Regex("KAERI_PANEL sources (\\d+)").find(shell("logcat -d -s KaeriPanel:I"))?.let { return it.groupValues[1].toLong() }
            SystemClock.sleep(20)
        }
        throw AssertionError("the app never opened Sources: ${shell("logcat -d -s KaeriPanel:I")}")
    }

    private fun shell(command: String): String =
        instrumentation.uiAutomation.executeShellCommand(command).let { fd ->
            android.os.ParcelFileDescriptor.AutoCloseInputStream(fd).use { it.readBytes().decodeToString() }
        }

    private companion object {
        const val TAG = "KaeriPanel"
        val MOMENTS = listOf("0s" to 300L, "1s" to 1_000L, "4s" to 4_000L, "12s" to 12_000L, "21s" to 21_000L)
    }
}
