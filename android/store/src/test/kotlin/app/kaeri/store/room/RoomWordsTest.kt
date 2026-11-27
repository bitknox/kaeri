// What a screen reader is told as the room ticks, and which relay addresses a room may use.

package app.kaeri.store.room

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class RoomWordsTest {
    @Test
    fun aTickingCounterKeepsTheSameWords() {
        assertEquals(RoomWords.numberless("Paused for Ren. Loading · 3 s"), RoomWords.numberless("Paused for Ren. Loading · 14 s"))
        assertEquals(RoomWords.numberless("Watch together. Episode 9 · 12:04"), RoomWords.numberless("Watch together. Episode 9 · 12:05"))
    }

    @Test
    fun aNewStateIsNewWords() {
        assertNotEquals(RoomWords.numberless("Paused for Ren. Loading · 3 s"), RoomWords.numberless("Paused for Ren. Buffering · 3 s"))
        assertNotEquals(RoomWords.numberless("Paused for Ren. Loading · 3 s"), RoomWords.numberless("Paused for Mika. Loading · 3 s"))
    }

    @Test
    fun relaysFollowTheEnginesRule() {
        assertTrue(RoomWords.validRelay("wss://relay.home.example"))
        assertTrue(RoomWords.validRelay("WSS://relay.home.example/path"))
        assertTrue(RoomWords.validRelay("ws://localhost:7777"))
        assertTrue(RoomWords.validRelay("ws://127.0.0.1:7777"))
        assertTrue(RoomWords.validRelay("ws://box.local"))
        assertFalse(RoomWords.validRelay("wss://"))
        assertFalse(RoomWords.validRelay("ws://relay.example"))
        assertFalse(RoomWords.validRelay("https://relay.example"))
    }

    @Test
    fun addTakesOnlyARelayTheListLacks() {
        val list = listOf("wss://relay.home.example")
        assertFalse(RoomWords.relayAddable("wss://relay.home.example", list))
        assertFalse(RoomWords.relayAddable("WSS://Relay.Home.Example", list))
        assertTrue(RoomWords.relayAddable("ws://box.local", list))
        assertFalse(RoomWords.relayAddable("wss://", list))
    }
}
