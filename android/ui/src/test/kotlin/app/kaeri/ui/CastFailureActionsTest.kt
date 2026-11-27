package app.kaeri.ui

import app.kaeri.engine.model.Msg
import app.kaeri.ui.CastFailureAction.CHOOSE_ANOTHER
import app.kaeri.ui.CastFailureAction.CHOOSE_SOURCE
import app.kaeri.ui.CastFailureAction.PLAY_HERE
import app.kaeri.ui.CastFailureAction.TRY_AGAIN
import org.junit.Assert.assertEquals
import org.junit.Test

/** The footer of a failed cast follows the spec's table, notice by notice. */
class CastFailureActionsTest {
    private val tv = "Living room"

    @Test
    fun aLostStreamOffersTryAgainFirst() {
        assertEquals(listOf(TRY_AGAIN, PLAY_HERE), castFailureActions(Msg.NoticeCastLostStream(tv)))
        assertEquals(listOf(TRY_AGAIN, PLAY_HERE), castFailureActions(Msg.NoticeCastNeverFetches(tv)))
    }

    @Test
    fun aSourceTheTvCantPlayOffersChooseSource() {
        assertEquals(listOf(CHOOSE_SOURCE, PLAY_HERE), castFailureActions(Msg.NoticeCastSegment(tv)))
        assertEquals(listOf(CHOOSE_SOURCE, PLAY_HERE), castFailureActions(Msg.NoticeCastRefused(tv)))
        assertEquals(listOf(CHOOSE_SOURCE), castFailureActions(Msg.NoticeCastDeepLink))
    }

    @Test
    fun aTvGoneOffersAnother() {
        assertEquals(listOf(PLAY_HERE, CHOOSE_ANOTHER), castFailureActions(Msg.NoticeCastLost(tv)))
        assertEquals(listOf(PLAY_HERE, CHOOSE_ANOTHER), castFailureActions(Msg.NoticeCastNoDevice))
    }
}
