package app.kaeri.ui

import app.kaeri.engine.model.Command
import app.kaeri.engine.model.ReadingCommand
import app.kaeri.store.AppStore
import app.kaeri.store.HAS_TESTING_ENGINE
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import org.junit.Assert.assertEquals
import org.junit.Assume.assumeTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/** Pull to refresh on Downloads asks for the books' downloads again too, when reading is on. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class DownloadsRefreshTest {
    private fun asked(reads: Boolean): List<Command> {
        val store = AppStore(CoroutineScope(SupervisorJob() + Dispatchers.Main))
        refreshDownloads(store, reads)
        return store.waiting.filter { it is Command.LoadDownloads || it == Command.Reading(ReadingCommand.LoadDownloads) }
    }

    @Test
    fun with_reading_on_both_media_are_asked_again() {
        assumeTrue(HAS_TESTING_ENGINE)
        assertEquals(listOf(Command.LoadDownloads, Command.Reading(ReadingCommand.LoadDownloads)), asked(reads = true))
    }

    @Test
    fun with_reading_off_only_the_shows_are() {
        assumeTrue(HAS_TESTING_ENGINE)
        assertEquals(listOf<Command>(Command.LoadDownloads), asked(reads = false))
    }
}
