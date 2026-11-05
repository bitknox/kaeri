package app.kaeri.player.ui

import app.kaeri.design.breathing
import org.junit.Assert.assertEquals
import org.junit.Test

class StartBreathingTest {
    @Test
    fun theBreathingIsATwoSecondFallAndATwoSecondRise() {
        assertEquals(1.0f, breathing(0), 0.001f)
        assertEquals(0.7f, breathing(1_000), 0.001f)
        assertEquals(0.4f, breathing(2_000), 0.001f)
        assertEquals(0.7f, breathing(3_000), 0.001f)
        assertEquals(1.0f, breathing(4_000), 0.001f)
        assertEquals(0.4f, breathing(6_000), 0.001f)
    }
}
