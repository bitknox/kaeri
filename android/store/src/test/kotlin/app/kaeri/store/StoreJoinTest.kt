package app.kaeri.store

import app.kaeri.engine.model.Event
import app.kaeri.engine.model.SyncStoreProblem
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Test

/** The answers to joining plugin stores through a paired device: kept in order until they are shown. */
class StoreJoinTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)
    private val store = AppStore(scope, clock = { 0L })

    @After
    fun tearDown() = scope.cancel()

    @Test
    fun answers_are_kept_in_order_until_seen() {
        store.apply(Event.SyncStoreJoined("kim", "s1", "https://plugins.example", "Example plugins"))
        store.apply(Event.SyncStoreJoinFailed("kim", "https://more.example", "More plugins", SyncStoreProblem.NOT_ALLOWED))
        assertEquals(
            listOf(
                StoreJoin.Joined("kim", "Example plugins"),
                StoreJoin.Failed("kim", "More plugins", SyncStoreProblem.NOT_ALLOWED),
            ),
            store.sync.storeJoins.value,
        )
        store.sync.storeJoinsSeen()
        assertEquals(emptyList<StoreJoin>(), store.sync.storeJoins.value)
    }
}
