// Round 5 audit: an episode download whose number is not whole (the engine knows episodes such
// as 5.5) in the download notification. Ignored with its bug id until it is fixed.

package app.kaeri.store

import app.kaeri.engine.model.DownloadRow
import app.kaeri.engine.model.DownloadState
import org.junit.Assert.assertNotNull
import org.junit.Ignore
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class R5AuditLiveDownloadsTest {
    private val context = RuntimeEnvironment.getApplication()

    /** LiveDownloads.kt:66-73 call `episode.toInt()`, which throws NumberFormatException on "5.5". */
    @Ignore("R5A-5")
    @Test
    fun a_half_episode_downloading_is_described() {
        val row = DownloadRow(
            id = 1, anilistId = 900001u, title = "Stardust Wayfarer", episode = "5.5", state = DownloadState.QUEUED,
            fraction = 0.0, downloaded = 0u, total = 0u, error = null, path = null,
        )
        assertNotNull(LiveDownloads.of(context, listOf(row), emptyMap()))
    }
}
