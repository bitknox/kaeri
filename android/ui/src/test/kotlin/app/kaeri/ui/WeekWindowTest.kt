package app.kaeri.ui

import org.junit.Assert.assertEquals
import org.junit.Test

class WeekWindowTest {
    private val airs = listOf(100L, 200L, 300L, 400L, 500L)

    private fun pick(now: Long, list: List<Long> = airs) = weekWindow(list, now, 3) { it }

    @Test
    fun todayShowsTheWindowAroundNow() {
        assertEquals(listOf(100L, 200L, 300L), pick(50))
        assertEquals(listOf(200L, 300L, 400L), pick(250))
        assertEquals(listOf(300L, 400L, 500L), pick(350))
        assertEquals(listOf(300L, 400L, 500L), pick(450))
        assertEquals(listOf(300L, 400L, 500L), pick(900))
        assertEquals(listOf(100L, 200L), pick(250, listOf(100L, 200L)))
    }
}
