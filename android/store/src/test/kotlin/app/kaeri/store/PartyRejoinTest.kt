// Rejoining a recent room: the lobby opens on it and this device joins, with no look into the
// room first, which would hold this device's name there while the join came in.

package app.kaeri.store

import app.kaeri.engine.model.Command
import app.kaeri.engine.model.PartyCommand
import app.kaeri.engine.model.PartyTarget
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import org.junit.Assert.assertEquals
import org.junit.Test

class PartyRejoinTest {
    private val target = PartyTarget("party.example:8999", "kite-7qm4-xr9d", null, "Stardust Wayfarer", "7", 900001u, false)

    @Test
    fun rejoin_joins_the_room_without_looking_in() {
        val store = AppStore(CoroutineScope(Dispatchers.Unconfined))
        store.rejoinParty(target)
        assertEquals(
            listOf<Command>(Command.Party(PartyCommand.Lobby(target.server, target.room, null, null))),
            store.waiting,
        )
        assertEquals(target, store.party.target.value?.target)
    }

    @Test
    fun opening_a_room_looks_in() {
        val store = AppStore(CoroutineScope(Dispatchers.Unconfined))
        store.openParty(target)
        assertEquals(
            listOf<Command>(Command.Party(PartyCommand.LookIn(target.server, target.room, null))),
            store.waiting,
        )
    }
}
