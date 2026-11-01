package app.kaeri.tv

import org.junit.Assert.assertEquals
import org.robolectric.annotation.Config
import org.robolectric.RuntimeEnvironment
import org.robolectric.RobolectricTestRunner
import org.junit.runner.RunWith
import org.junit.Assert.assertTrue
import org.junit.Test
import app.kaeri.engine.ffi.FfiPairedPhone
import app.kaeri.tv.remote.grouped
import app.kaeri.engine.ffi.qrMatrix
import app.kaeri.tv.screens.megabytes
import app.kaeri.tv.screens.phoneLine

/** What the remote's screens write: the code in groups, the QR code's quiet zone, a phone's line. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class RemoteTest {
    private val context = RuntimeEnvironment.getApplication()
    @Test
    fun a_code_reads_in_two_groups_of_three() {
        assertEquals("042 917", grouped("042917"))
        assertEquals("12345", grouped("12345"))
    }

    @Test
    fun the_qr_code_comes_from_the_engine_with_a_quiet_zone_of_four() {
        val matrix = qrMatrix(TvShots.PAIR_LINK)
        assertEquals(4u, matrix.quietZone)
        assertTrue(matrix.size > 0u)
        assertEquals((matrix.size * matrix.size).toInt(), matrix.modules.size)
        // A finder pattern's corner is dark, the module at the origin.
        assertTrue(matrix.modules[0])
    }

    @Test
    fun a_phone_reads_as_when_it_was_last_used() {
        val now = 1_800_000_000L
        val phone = FfiPairedPhone("ab", "Mika's phone", pairedAt = now - 10 * 86_400, lastSeen = null)
        assertEquals("Paired 10 days ago", phoneLine(context, phone, now))
        assertEquals("Used today", phoneLine(context, phone.copy(lastSeen = now - 60), now))
        assertEquals("Used yesterday", phoneLine(context, phone.copy(lastSeen = now - 86_400), now))
    }

    @Test
    fun the_image_cache_reads_in_whole_megabytes() {
        assertEquals("0 MB", megabytes(context, 0))
        assertEquals("40 MB", megabytes(context, 40L shl 20))
        assertEquals("2 MB", megabytes(context, (3L shl 19)))
    }
}
