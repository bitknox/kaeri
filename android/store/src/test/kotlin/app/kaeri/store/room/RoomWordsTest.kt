// What a screen reader is told as the room ticks, and which relay addresses a room may use.

package app.kaeri.store.room

import app.kaeri.engine.model.RoomConnection
import app.kaeri.engine.model.RoomEpisode
import app.kaeri.engine.model.RoomView
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

    @Test
    fun addReadsTheAddressAsItSavesIt() {
        val list = listOf("wss://relay.home.example")
        assertTrue(RoomWords.relayAddable("  wss://spare.home.example \n", list))
        assertFalse(RoomWords.relayAddable(" wss://relay.home.example ", list))
    }

    @Test
    fun anAddressHasNoSpacesAndANameBeforeLocal() {
        assertFalse(RoomWords.validRelay("wss://relay home.example"))
        assertFalse(RoomWords.validRelay("wss:// relay.example"))
        assertFalse(RoomWords.validRelay("ws://.local"))
        assertFalse(RoomWords.validRelay("ws://.local:7777"))
        assertTrue(RoomWords.validRelay("ws://box.local:7777"))
    }

    private fun view(playing: Boolean, episode: String = "9") = RoomView(
        roomId = "room-a",
        name = "Mika's room",
        episode = RoomEpisode(900001u, episode),
        position = 12.0,
        positionAtMs = 0,
        connection = RoomConnection.Connected,
        link = "https://kaeri.app/r#v1.room-a",
        playing = playing,
    )

    @Test
    fun theLineIsHeardWhenTheEpisodeChanges() {
        val line = RoomLine("Watching together", "Episode 9 · 12:04")
        val next = RoomLine("Watching together", "Episode 10 · 0:00")
        assertNotEquals(RoomWords.lineKey(line, view(true, "9"), 724.0), RoomWords.lineKey(next, view(true, "10"), 0.0))
    }

    @Test
    fun theClockTicksQuietlyWhilePlaying() {
        val now = RoomLine("Watching together", "Episode 9 · 12:04")
        val later = RoomLine("Watching together", "Episode 9 · 12:05")
        assertEquals(RoomWords.lineKey(now, view(true), 724.0), RoomWords.lineKey(later, view(true), 725.0))
    }

    @Test
    fun aSeekWhilePausedIsHeard() {
        val before = RoomLine("Paused", "Episode 9 · 12:04")
        val after = RoomLine("Paused", "Episode 9 · 3:10")
        assertNotEquals(RoomWords.lineKey(before, view(false), 724.0), RoomWords.lineKey(after, view(false), 190.0))
    }
}
