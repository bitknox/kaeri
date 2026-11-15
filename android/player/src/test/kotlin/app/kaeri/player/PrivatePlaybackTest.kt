// A private playback on the system's surfaces: the media session says "Playing in Kaeri" and
// nothing of the show, the episode or its art, while its controls keep working.

package app.kaeri.player

import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import androidx.media3.common.Player
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.PrivateTitle

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class PrivatePlaybackTest {
    private val context = RuntimeEnvironment.getApplication()

    @Test
    fun anOrdinaryPlaybackNamesItsShowAndEpisode() = runTest {
        val rig = Rig(this)
        rig.model.apply(Event.PlaybackStaged(900_001u, "Stardust Wayfarer", "7", null, 1.0))
        val session = SessionPlayer(context, rig.model)
        assertFalse(rig.model.playsPrivately)
        assertEquals("Stardust Wayfarer", session.mediaMetadata.artist?.toString())
        assertEquals("900001-7", session.currentMediaItem?.mediaId)
        session.release()
    }

    @Test
    fun privateModeSaysOnlyPlayingInKaeri() = runTest {
        val rig = Rig(this)
        rig.model.apply(Event.Private(true))
        rig.model.apply(Event.PlaybackStaged(900_001u, "Stardust Wayfarer", "7", null, 1.0))
        val session = SessionPlayer(context, rig.model)
        assertTrue(rig.model.playsPrivately)
        val said = session.mediaMetadata
        assertEquals("Playing in Kaeri", said.title?.toString())
        assertEquals("Playing in Kaeri", said.displayTitle?.toString())
        assertNull(said.artist)
        assertNull(said.subtitle)
        assertNull(said.albumTitle)
        assertNull(said.artworkData)
        assertEquals(SessionPlayer.PRIVATE_MEDIA_ID, session.currentMediaItem?.mediaId)
        // The controls stay: play, pause and seek still go to the engine.
        assertTrue(session.isCommandAvailable(Player.COMMAND_PLAY_PAUSE))
        assertTrue(session.isCommandAvailable(Player.COMMAND_SEEK_IN_CURRENT_MEDIA_ITEM))
        session.release()
    }

    @Test
    fun anAlwaysPrivateShowIsPrivateWithTheModeOff() = runTest {
        val rig = Rig(this)
        rig.model.apply(Event.PlaybackStaged(900_001u, "Stardust Wayfarer", "7", null, 1.0, alwaysPrivate = true))
        assertTrue(rig.model.playsPrivately)
        val session = SessionPlayer(context, rig.model)
        assertEquals("Playing in Kaeri", session.mediaMetadata.title?.toString())
        assertNull(session.mediaMetadata.artist)
        session.release()
    }

    @Test
    fun turningTheModeOffOrTheShowsFlagFollowsAtOnce() = runTest {
        val rig = Rig(this)
        rig.model.apply(Event.Private(true))
        rig.model.apply(Event.PlaybackStaged(900_001u, "Stardust Wayfarer", "7", null, 1.0))
        rig.model.apply(Event.Private(false))
        assertFalse(rig.model.playsPrivately)
        rig.model.apply(Event.AlwaysPrivate(PrivateTitle.Show(900_001u), true))
        assertTrue(rig.model.playsPrivately)
        // Another show's flag changes nothing here.
        rig.model.apply(Event.AlwaysPrivate(PrivateTitle.Show(900_002u), false))
        assertTrue(rig.model.playsPrivately)
        rig.model.apply(Event.AlwaysPrivate(PrivateTitle.Show(900_001u), false))
        assertFalse(rig.model.playsPrivately)
    }
}
