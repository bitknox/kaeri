package app.kaeri.store

import app.kaeri.engine.model.SyncCopy
import org.junit.Assert.assertEquals
import org.junit.Test

class CopySignInsTest {
    @Test
    fun theSwitchLeftOnKeepsTheSetupsTrackerSignIns() {
        assertEquals(listOf(SyncCopy.Trackers), copiedOnContinue(signIns = true))
    }

    @Test
    fun theSwitchOffDropsThem() {
        assertEquals(emptyList<SyncCopy>(), copiedOnContinue(signIns = false))
    }
}
