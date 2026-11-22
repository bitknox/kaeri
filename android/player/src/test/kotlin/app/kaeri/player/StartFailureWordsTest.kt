package app.kaeri.player

import app.kaeri.engine.model.Cause
import app.kaeri.engine.model.HeldSources
import app.kaeri.engine.model.ListingMiss
import app.kaeri.engine.model.MissedListing
import app.kaeri.engine.model.Msg
import app.kaeri.engine.model.NoSourceWhy
import app.kaeri.engine.model.SourceHold
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

/** How a failed start and the source picker say what each source did. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class StartFailureWordsTest {
    private val context = RuntimeEnvironment.getApplication()
    private val harbor = HeldSources(listOf("Harbor"), SourceHold.TORRENTS_OFF) // i18n-skip

    @Test
    fun aSourceThatFailedIsOneNotReachedAndAHeldBackOneIsNotAsked() {
        val failure = PlaybackFailure(
            1u, "7", msg = Msg.NoticeReason(Cause.SourcesUnreached("Alpha")), why = NoSourceWhy.NOT_FOUND, // i18n-skip
            asked = listOf("Alpha", "Brook"), unreached = listOf("Alpha"), heldBack = listOf(harbor), // i18n-skip
        )
        assertEquals(
            "Couldn't reach Alpha. Brook doesn't have episode 7. Harbor isn't asked while torrent playback is off.", // i18n-skip
            failureDetail(context, failure),
        )
    }

    @Test
    fun aHeldBackSourceAloneIsNeverSaidAsNotReached() {
        val failure = PlaybackFailure(
            1u, "7", msg = Msg.NoticeReason(Cause.SourcesNotAsked(1L, "Harbor", "7")), why = NoSourceWhy.NOT_FOUND, // i18n-skip
            asked = listOf("Brook"), heldBack = listOf(harbor), // i18n-skip
        )
        val detail = failureDetail(context, failure)
        assertEquals("Brook doesn't have episode 7. Harbor isn't asked while torrent playback is off.", detail) // i18n-skip
        assertFalse(detail, detail.contains("Couldn't reach")) // i18n-skip
    }

    @Test
    fun aSourceHeldWhileTorrentsStartIsNotSaidToBeOff() {
        val starting = HeldSources(listOf("Harbor"), SourceHold.TORRENTS_STARTING) // i18n-skip
        val failure = PlaybackFailure(
            1u, "7", msg = Msg.NoticeReason(Cause.SourcesNotAsked(1L, "Harbor", "7")), why = NoSourceWhy.NOT_FOUND, // i18n-skip
            asked = listOf("Brook"), heldBack = listOf(starting), // i18n-skip
        )
        assertEquals(
            "Brook doesn't have episode 7. Harbor isn't asked until torrent playback has started.", // i18n-skip
            failureDetail(context, failure),
        )
        assertEquals("Not asked · Torrent playback is starting", heldNote(context, SourceHold.TORRENTS_STARTING)) // i18n-skip
        assertEquals("Not asked · Torrent playback couldn't start", heldNote(context, SourceHold.TORRENTS_FAILED)) // i18n-skip
    }

    @Test
    fun thePickerLineClaimsOnlyWhatTheSourcesThatAnsweredSaid() {
        val answered = listOf(MissedListing("Brook", ListingMiss.NoEpisode), MissedListing("Kumo", ListingMiss.NoMatch)) // i18n-skip
        assertEquals("No source lists episode 7.", noneListsText(context, answered, "7")) // i18n-skip
        val failed = listOf(MissedListing("Alpha", ListingMiss.Failed), MissedListing("Brook", ListingMiss.NoEpisode)) // i18n-skip
        assertEquals("No source that answered lists episode 7.", noneListsText(context, failed, "7")) // i18n-skip
        val held = listOf(MissedListing("Harbor", ListingMiss.HeldBack(SourceHold.TORRENTS_OFF))) // i18n-skip
        assertEquals("No source that answered lists episode 7.", noneListsText(context, held, "7")) // i18n-skip
    }
}
