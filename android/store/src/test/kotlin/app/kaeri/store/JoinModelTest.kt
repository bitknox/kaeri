// First run's join page on the clock: the waiting line, the hint, the code running out, the setup
// arriving or not, and Continue waiting on the stores it asked.

package app.kaeri.store

import app.kaeri.engine.model.FirstRunPage
import app.kaeri.engine.model.JoinedSetup
import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.engine.model.SyncStoreOffer
import app.kaeri.engine.model.SyncStoreProblem
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class JoinModelTest {
    private var now = 1_000_000L
    private val model = JoinModel { now }

    private fun setup(answered: List<FirstRunPage> = FirstRunPage.entries) = JoinedSetup(
        device = "kim",
        deviceName = "Kim’s laptop",
        medium = KaeriIsFor.READ,
        stores = listOf(SyncStoreOffer(address = "https://shelf.example", name = "Shelf", inviteFrom = "kim")),
        servers = emptyList(),
        signedIn = listOf("anilist"),
        folders = 0u,
        answered = answered,
    )

    private fun showCode() {
        model.begin()
        model.invite("482913", 41234, now + JoinModel.CODE_SECONDS * 1_000)
    }

    @Test
    fun theWaitingLineComesAfter600msAndTheHintAfter30s() {
        showCode()
        val state = model.state.value
        assertFalse(state.waiting(now + 599))
        assertTrue(state.waiting(now + 600))
        assertFalse(state.slow(now + 29_999))
        assertTrue(state.slow(now + 30_000))
        assertEquals(now + 600, state.nextChange(now))
    }

    @Test
    fun theCodeRunsOutAtTwoMinutes() {
        showCode()
        val state = model.state.value
        assertEquals(120L, state.secondsLeft(now))
        assertEquals(102L, state.secondsLeft(now + 18_000))
        assertFalse(state.expired(now + 119_999))
        assertTrue(state.expired(now + 120_000))
        // Run out, the waiting line and the hint give way to the error.
        assertFalse(state.waiting(now + 120_000))
        assertFalse(state.slow(now + 120_000))
        assertEquals(0L, state.secondsLeft(now + 130_000))
    }

    @Test
    fun pairedTheLookingRowShowsAfter600msUntilTheSetupLands() {
        showCode()
        model.paired("Kim’s laptop")
        val state = model.state.value
        assertEquals(JoinStage.PAIRED, state.stage)
        assertFalse(state.looking(now + 599))
        assertTrue(state.looking(now + 600))
        assertFalse(state.landed(now + 600))
        model.setup(setup())
        val landed = model.state.value
        assertFalse(landed.looking(now + 600))
        assertTrue(landed.landed(now + 600))
        assertEquals("Kim’s laptop", landed.deviceName)
    }

    @Test
    fun withoutASetupThePageGoesOnAfterTheWait() {
        showCode()
        model.paired("Kim’s laptop")
        val state = model.state.value
        assertFalse(state.landed(now + JoinModel.SETUP_MILLIS - 1))
        assertTrue(state.gaveUp(now + JoinModel.SETUP_MILLIS))
        assertTrue(state.landed(now + JoinModel.SETUP_MILLIS))
        assertFalse(state.looking(now + JoinModel.SETUP_MILLIS))
    }

    @Test
    fun theSetupArrivingWhileTheCodeShowsPairs() {
        showCode()
        model.setup(setup())
        assertEquals(JoinStage.PAIRED, model.state.value.stage)
    }

    @Test
    fun nothingArrivesOnTheTwoWaysIn() {
        model.setup(setup())
        model.paired("Kim’s laptop")
        assertEquals(JoinState(), model.state.value)
    }

    @Test
    fun continueWaitsForTheStoresItAskedThenLeaves() {
        showCode()
        model.setup(setup())
        assertEquals(setOf("Shelf"), model.proceed(listOf("Shelf")))
        assertFalse(model.state.value.leaving(now))
        assertFalse(model.state.value.working(now + 599))
        assertTrue(model.state.value.working(now + 600))
        model.storeJoin(StoreJoin.Joined("kim", "Shelf"))
        assertTrue(model.state.value.leaving(now + 600))
    }

    @Test
    fun aStoreThatSaidNoKeepsThePageUntilContinueAgain() {
        showCode()
        model.setup(setup())
        model.proceed(listOf("Shelf"))
        model.storeJoin(StoreJoin.Failed("kim", "Shelf", SyncStoreProblem.REFUSED))
        val refused = model.state.value
        assertFalse(refused.leaving(now))
        assertEquals(SyncStoreProblem.REFUSED, refused.refusal("Shelf")?.problem)
        // The second press asks nothing again and goes on.
        assertTrue(model.proceed(listOf("Shelf")).isEmpty())
        assertTrue(model.state.value.leaving(now))
    }

    @Test
    fun aStoreThatNeverAnswersStopsHoldingContinue() {
        showCode()
        model.setup(setup())
        model.proceed(listOf("Shelf"))
        assertFalse(model.state.value.leaving(now + JoinModel.JOIN_MILLIS - 1))
        assertTrue(model.state.value.leaving(now + JoinModel.JOIN_MILLIS))
    }

    @Test
    fun anotherDevicesStoreAnswerIsNotOurs() {
        showCode()
        model.setup(setup())
        model.proceed(listOf("Shelf"))
        model.storeJoin(StoreJoin.Failed("someone-else", "Shelf", SyncStoreProblem.REFUSED))
        assertNull(model.state.value.refusal("Shelf"))
    }

    @Test
    fun answeredPagesAreSkippedOnlyWhenWhatAnsweredThemCameOver() {
        val all = setup()
        assertEquals(emptySet<FirstRunPage>(), all.pagesLeft(signIns = true, sources = true))
        assertEquals(setOf(FirstRunPage.LISTS), all.pagesLeft(signIns = false, sources = true))
        assertEquals(setOf(FirstRunPage.LISTS, FirstRunPage.SOURCES), all.pagesLeft(signIns = false, sources = false))
        assertEquals(FirstRunPage.entries.toSet(), setup(answered = emptyList()).pagesLeft(signIns = true, sources = true))
        assertEquals(FirstRunPage.entries.toSet(), (null as JoinedSetup?).pagesLeft(signIns = true, sources = true))
    }

    @Test
    fun aStagedStateHoldsItsClock() {
        model.stage(JoinState(stage = JoinStage.CODE, code = "482913", codeAt = 10, until = 120_010), 18_010)
        now += 50_000
        assertEquals(18_010L, model.now())
        assertTrue(model.opened.value)
        model.reset()
        assertEquals(now, model.now())
        assertFalse(model.opened.value)
    }
}
