// Prelaunch board piece 4, "Did my server get it?", staged for shots on the fixtures engine (see
// Prelaunch.kt). The book server is made up: a Kavita server at books.home.example with 212 series
// in 2 libraries, whose progress facts say where its queue stands. Its row in Your sources takes
// the second line from those facts, as it does from the engine's. The fixtures engine has no book
// server of its own, so the staged one is put back each time the engine says its sources.

package app.kaeri.store

import app.kaeri.engine.model.BookServerKind
import app.kaeri.engine.model.BookServerRow
import app.kaeri.engine.model.BookSourceEvent
import app.kaeri.engine.model.WriteBackProblem
import kotlinx.coroutines.Job
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.launch

object PrelaunchProgress {
    /** Your sources: saved, waiting offline, trying again, not saved, and not set to receive. */
    val STATES: List<String> = (6..10).map { "p4-%02d".format(it) }

    const val HOST = "books.home.example"
    private var keeping: Job? = null

    /** The server's row as the engine would say it in [state], at [now] in Unix seconds. */
    fun server(state: String, now: Long = System.currentTimeMillis() / 1000): BookServerRow? {
        val base = BookServerRow(
            id = "books-home",
            kind = BookServerKind.KAVITA,
            name = HOST,
            address = "https://$HOST",
            series = 212u,
            reportProgress = true,
            off = false,
            signedOut = false,
            libraries = 2u,
            empty = false,
            rechecksAt = null,
            keyLast4 = null,
            progressPending = 0u,
            progressSending = false,
            progressSavedAt = null,
            progressProblem = null,
            progressProblemAt = null,
        )
        val waiting = base.copy(progressPending = 3u, progressSavedAt = now - 3_600, progressProblemAt = now - 60)
        return when (state) {
            "p4-06" -> base.copy(progressSavedAt = now - 120)
            "p4-07" -> waiting.copy(progressProblem = WriteBackProblem.OFFLINE)
            "p4-08" -> waiting.copy(progressProblem = WriteBackProblem.NO_ANSWER)
            "p4-09" -> waiting.copy(progressProblem = WriteBackProblem.REFUSED_KEY)
            "p4-10" -> base.copy(reportProgress = false)
            else -> null
        }
    }

    suspend fun stage(store: AppStore, state: String): Boolean {
        val row = server(state) ?: return false
        keeping?.cancel()
        keeping = MainScope().launch {
            store.bookSources.view.collect { view ->
                val now = view ?: return@collect
                if (now.servers.any { it.id == row.id }) return@collect
                store.bookSources.apply(BookSourceEvent.View(now.copy(servers = now.servers + row, order = now.order + row.id)))
            }
        }
        if (store.bookSources.view.value == null) store.bookSources.load()
        return true
    }
}
