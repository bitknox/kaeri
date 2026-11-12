// Round 5 audit: an episode download whose number is not whole (the engine knows episodes such
// as 5.5) in the download notification.

package app.kaeri.store

import app.kaeri.engine.model.DownloadRow
import app.kaeri.engine.model.DownloadState
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class R5AuditLiveDownloadsTest {
    private val context = RuntimeEnvironment.getApplication()

    private fun row(id: Long, episode: String, state: DownloadState = DownloadState.QUEUED) = DownloadRow(
        id = id, anilistId = 900001u, title = "Stardust Wayfarer", episode = episode, state = state,
        fraction = 0.0, downloaded = 0u, total = 0u, error = null, path = null,
    )

    @Test
    fun a_half_episode_downloading_is_described() {
        val live = assertNotNull(LiveDownloads.of(context, listOf(row(1, "5.5")), emptyMap()))
        assertTrue(live!!.text, live.text.startsWith("Episode 5.5"))
    }

    @Test
    fun a_half_episode_leading_several_is_named() {
        val live = LiveDownloads.of(context, listOf(row(1, "5.5", DownloadState.ACTIVE), row(2, "6")), emptyMap())!!
        assertTrue(live.text, live.text.startsWith("Stardust Wayfarer · Episode 5.5"))
    }

    @Test
    fun a_half_episode_with_time_left_says_it() {
        val active = row(1, "5.5", DownloadState.ACTIVE).copy(downloaded = 100_000_000u, total = 400_000_000u)
        val live = LiveDownloads.of(context, listOf(active), mapOf(1L to 1_000_000.0))!!
        assertTrue(live.text, live.text.startsWith("Episode 5.5 · ") && live.text.endsWith(" left"))
    }

    @Test
    fun whole_episodes_keep_their_words() {
        assertEquals("Episode 7", episodeName(context, "7"))
        assertEquals("Episode 7.5", episodeName(context, "7.5"))
        assertEquals(7, episodeFloor("7.5"))
        assertEquals(0, episodeFloor("special"))
        assertEquals("Episode special", episodeName(context, "special"))
    }
}
