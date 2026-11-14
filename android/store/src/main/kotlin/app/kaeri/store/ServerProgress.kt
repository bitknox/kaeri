// Did my server get it? What became of the reading progress sent back to a Komga or Kavita server,
// as one line on the page between chapters and one on the server's row in Your sources. The
// engine keeps the queue and says the facts; this decides what the lines say and when, following
// the waiting rule: nothing for 600 ms, then Sending for at least 0.3 s.

package app.kaeri.store

import app.kaeri.engine.core.ChapterId
import app.kaeri.engine.core.WorkId
import app.kaeri.engine.model.BookServerRow
import app.kaeri.engine.model.BookSourceCommand
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.WriteBackProblem
import app.kaeri.engine.model.WriteBackResult
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update

/** Why a send was not saved, as the line words it. */
enum class WhyNot { KEY, PASSWORD, GONE, GAVE_UP }

/** The reason a problem is held for, or null for a problem that passes. */
fun WriteBackProblem.why(): WhyNot? = when (this) {
    WriteBackProblem.REFUSED_KEY -> WhyNot.KEY
    WriteBackProblem.REFUSED_PASSWORD -> WhyNot.PASSWORD
    WriteBackProblem.SERIES_GONE -> WhyNot.GONE
    WriteBackProblem.NO_ANSWER_FOR_A_DAY -> WhyNot.GAVE_UP
    else -> null
}

/** The line on the page between chapters. [host] is the server's host, as the line names it. */
sealed interface ServerLine {
    val host: String

    data class Saved(override val host: String) : ServerLine
    data class Sending(override val host: String) : ServerLine
    data class Offline(override val host: String) : ServerLine
    data class Retrying(override val host: String) : ServerLine
    data class Failed(override val host: String, val why: WhyNot) : ServerLine

    /** Retrying and Failed carry Try again. */
    val canRetry: Boolean get() = this is Retrying || this is Failed
}

/**
 * What a result says on the line: the same for a fresh answer and for one that was waited for.
 * Null for a server set not to receive progress: that was a choice, so the page says nothing.
 */
fun WriteBackResult.line(host: String): ServerLine? = when (this) {
    is WriteBackResult.Saved -> ServerLine.Saved(host)
    is WriteBackResult.Waiting -> problemLine(problem, host)
    is WriteBackResult.NotSaved -> problemLine(problem, host)
}

private fun problemLine(problem: WriteBackProblem, host: String): ServerLine? = when (problem) {
    WriteBackProblem.NOT_SET_TO_RECEIVE -> null
    WriteBackProblem.OFFLINE -> ServerLine.Offline(host)
    else -> problem.why()?.let { ServerLine.Failed(host, it) } ?: ServerLine.Retrying(host)
}

/** Where a server's queue stands, read from its row. */
internal sealed interface Stand {
    /** Nothing to say: no queue and nothing saved yet, or the server is gone. */
    data object None : Stand

    /** Set not to receive progress. */
    data object Off : Stand

    /** Chapters wait with no problem: the queue is sending them, or about to. */
    data class Going(val count: Int) : Stand

    data class Saved(val at: Long) : Stand
    data class Offline(val count: Int) : Stand
    data class Retrying(val count: Int) : Stand
    data class Failed(val count: Int, val why: WhyNot) : Stand
}

internal fun BookServerRow?.stand(): Stand {
    val row = this ?: return Stand.None
    if (!row.reportProgress || row.progressProblem == WriteBackProblem.NOT_SET_TO_RECEIVE) return Stand.Off
    val count = row.progressPending.toInt()
    if (count == 0) return row.progressSavedAt?.let { Stand.Saved(it) } ?: Stand.None
    val problem = row.progressProblem ?: return Stand.Going(count)
    val why = problem.why()
    return when {
        problem == WriteBackProblem.OFFLINE -> Stand.Offline(count)
        why != null -> Stand.Failed(count, why)
        else -> Stand.Retrying(count)
    }
}

internal fun BookServerRow.stamp(): Pair<Long?, Long?> = progressSavedAt to progressProblemAt

/**
 * A Try again waiting for the engine's answer. The engine publishes the row at once on the press,
 * still with the old facts, then again as the send starts and once it is over. A press is answered
 * when the facts moved, when a send seen since is over, or when nothing can be sent now.
 */
internal class Press(private val at: Pair<Long?, Long?>) {
    private var sent = false

    fun answeredBy(row: BookServerRow?): Boolean {
        if (row == null) return true
        if (row.progressSending) {
            sent = true
            return false
        }
        return when (row.stand()) {
            is Stand.Offline, Stand.Off, Stand.None, is Stand.Saved -> true
            else -> sent || row.stamp() != at
        }
    }
}

/**
 * The page between chapters' server line per chapter. The engine says a result at once when it is
 * known before any send (offline, held for Try again, not set to receive), and that comes before
 * the chapter's ChapterRead; otherwise it comes once the server answers. A chapter read with no
 * result yet starts a wait on the server its work reads from ([hostFor]): Sending shows after
 * 600 ms with no answer, and the answer then waits until Sending has shown 0.3 s.
 *
 * A line that is not Saved follows its server's row from there: back online the queue sends on its
 * own and Saved to comes in; Try again sends every waiting chapter of the server, and the engine
 * answers it on the row. With progress sent on closing the reader ([quiet]) the page has no line.
 */
class ServerLines(
    private val scope: CoroutineScope,
    private val send: (Command) -> Unit = {},
    private val hostFor: (WorkId) -> String? = { null },
    private val quiet: () -> Boolean = { false },
) {
    private val _lines = MutableStateFlow<Map<ChapterId, ServerLine>>(emptyMap())

    /** The line each chapter read in this reader has, when it has one. */
    val lines: StateFlow<Map<ChapterId, ServerLine>> = _lines.asStateFlow()

    private val waits = HashMap<ChapterId, WaitingLine>()
    private val servers = HashMap<ChapterId, String>()
    private val hosts = HashMap<ChapterId, String>()

    /** Chapters the engine said a result for, a null one included. */
    private val answered = HashSet<ChapterId>()

    /** Chapters whose line follows their server's row. */
    private val follows = LinkedHashSet<ChapterId>()

    /** Servers whose Try again was pressed on a line and is not answered yet. */
    private val pressed = HashMap<String, Press>()
    private var rows: List<BookServerRow> = emptyList()

    /**
     * Whether the page between chapters holds the line's two-line space for [chapter] of [work]:
     * it has a line, or one may come.
     */
    fun holds(work: WorkId, chapter: ChapterId): Boolean =
        chapter in _lines.value || (!quiet() && (chapter in waits || hostFor(work) != null))

    /** The read rule fired for [chapter] of [work]. */
    fun chapterRead(work: WorkId, chapter: ChapterId) {
        if (quiet() || chapter in answered || chapter in waits) return
        val host = hostFor(work) ?: return
        hosts[chapter] = host
        wait(chapter)
    }

    /** The engine said what became of [chapter]'s send to [server]. */
    fun sent(chapter: ChapterId, server: String, host: String, result: WriteBackResult) {
        answered += chapter
        servers[chapter] = server
        hosts[chapter] = host
        val said = if (quiet()) null else result.line(host)
        if (said == null || said is ServerLine.Saved) follows -= chapter else follows += chapter
        show(chapter, said)
    }

    /**
     * Try again on the line: every waiting chapter of its server goes out now. The engine answers
     * a retry on the server's row, not per chapter, so the line follows the row from here.
     */
    fun retry(chapter: ChapterId) {
        val server = servers[chapter] ?: return
        if (_lines.value[chapter]?.canRetry != true || server in pressed) return
        pressed[server] = Press(rows.firstOrNull { it.id == server }?.stamp() ?: (null to null))
        follows.filter { servers[it] == server && it !in waits }.forEach(::wait)
        send(Command.BookSources(BookSourceCommand.RetryProgress(server)))
    }

    /** The servers' facts changed: lines that follow their server take its news. */
    fun rows(now: List<BookServerRow>) {
        rows = now
        pressed.entries.removeAll { (server, press) -> press.answeredBy(now.firstOrNull { it.id == server }) }
        follows.toList().forEach { chapter ->
            val server = servers[chapter] ?: return@forEach
            if (server in pressed) return@forEach
            val host = hosts[chapter] ?: return@forEach
            when (val stand = now.firstOrNull { it.id == server }.stand()) {
                is Stand.Going -> if (chapter !in waits) wait(chapter)
                is Stand.Saved -> settle(chapter, ServerLine.Saved(host))
                is Stand.Offline -> settle(chapter, ServerLine.Offline(host))
                is Stand.Retrying -> settle(chapter, ServerLine.Retrying(host))
                is Stand.Failed -> settle(chapter, ServerLine.Failed(host, stand.why))
                Stand.Off, Stand.None -> settle(chapter, null)
            }
        }
    }

    private fun settle(chapter: ChapterId, line: ServerLine?) {
        if (line == null || line is ServerLine.Saved) follows -= chapter
        if (chapter !in waits && _lines.value[chapter] == line) return
        show(chapter, line)
    }

    private fun wait(chapter: ChapterId) {
        waits.remove(chapter)?.cancel()
        waits[chapter] = WaitingLine(scope) {
            hosts[chapter]?.let { host -> _lines.update { it + (chapter to ServerLine.Sending(host)) } }
        }.also { it.begin() }
    }

    /** [line] on [chapter]'s page, after the waiting line's hold when one showed. */
    private fun show(chapter: ChapterId, line: ServerLine?) {
        val put = { _lines.update { if (line == null) it - chapter else it + (chapter to line) } }
        val wait = waits.remove(chapter)
        if (wait == null) put() else wait.end(put)
    }

    /** The reader closed: nothing is waited for any more. */
    fun clear() {
        waits.values.forEach { it.cancel() }
        waits.clear()
        servers.clear()
        hosts.clear()
        answered.clear()
        follows.clear()
        pressed.clear()
        _lines.value = emptyMap()
    }

    /** For shots: [chapter]'s line as it would stand, from [server]. */
    fun stage(chapter: ChapterId, line: ServerLine?, server: String = "") {
        servers[chapter] = server
        answered += chapter
        line?.let { hosts[chapter] = it.host }
        if (line == null || line is ServerLine.Saved || line is ServerLine.Sending) follows -= chapter else follows += chapter
        _lines.update { if (line == null) it - chapter else it + (chapter to line) }
    }
}

/** The second line of a server's row. */
sealed interface RowLine {
    /** Not set to receive: "Progress stays on this phone". */
    data object Off : RowLine
    data class Saved(val at: Long) : RowLine
    data class Sending(val count: Int) : RowLine
    data class Offline(val count: Int) : RowLine
    data class Retrying(val count: Int) : RowLine
    data class Failed(val count: Int, val why: WhyNot) : RowLine

    /** Retrying and Failed carry Try again, which a press dims until the answer. */
    val canRetry: Boolean get() = this is Retrying || this is Failed
}

/** What the engine's facts about a server say once settled; null while its queue is going. */
fun BookServerRow.rowFacts(): RowLine? = when (val stand = stand()) {
    Stand.None, is Stand.Going -> null
    Stand.Off -> RowLine.Off
    is Stand.Saved -> RowLine.Saved(stand.at)
    is Stand.Offline -> RowLine.Offline(stand.count)
    is Stand.Retrying -> RowLine.Retrying(stand.count)
    is Stand.Failed -> RowLine.Failed(stand.count, stand.why)
}

/**
 * The rows' lines. A queue that is going shows Sending only once it has gone 600 ms, and a Sending
 * that showed stays 0.3 s; until then the row keeps what it said. Try again dims its button at
 * once; the button, the line and the chevron then follow the engine's answer together.
 */
class RowLines(private val scope: CoroutineScope, private val send: (Command) -> Unit = {}) {
    private val _lines = MutableStateFlow<Map<String, RowLine>>(emptyMap())

    /** Each server's line by its id; a server with nothing to say has none. */
    val lines: StateFlow<Map<String, RowLine>> = _lines.asStateFlow()

    private val _retrying = MutableStateFlow<Set<String>>(emptySet())

    /** Servers whose Try again was pressed and is not answered yet: the button is at 38%. */
    val retrying: StateFlow<Set<String>> = _retrying.asStateFlow()

    private val waits = HashMap<String, WaitingLine>()
    private val rows = HashMap<String, BookServerRow>()
    private val pressed = HashMap<String, Press>()

    /** The count each waiting line names, kept current while it shows. */
    private val counts = HashMap<String, Int>()

    /** The book sources' view changed. */
    fun update(now: List<BookServerRow>) {
        val ids = now.map { it.id }.toSet()
        (rows.keys - ids).forEach(::forget)
        now.forEach { row ->
            rows[row.id] = row
            one(row.id)
        }
    }

    private fun one(id: String) {
        val row = rows[id]
        pressed[id]?.let { press ->
            if (!press.answeredBy(row)) {
                going(id, row?.progressPending?.toInt() ?: 0)
                return
            }
            pressed -= id
        }
        when (val stand = row.stand()) {
            is Stand.Going -> going(id, stand.count)
            else -> settle(id, row?.rowFacts())
        }
    }

    /** The queue is going: Sending after 600 ms, its count kept current. */
    private fun going(id: String, count: Int) {
        if (count > 0) counts[id] = count
        val wait = waits[id]
        if (wait == null) {
            waits[id] = WaitingLine(scope) { _lines.update { it + (id to RowLine.Sending(counts[id] ?: 1)) } }.also { it.begin() }
        } else if (wait.shown.value) {
            _lines.update { it + (id to RowLine.Sending(counts[id] ?: 1)) }
        }
    }

    private fun settle(id: String, line: RowLine?) {
        val put = {
            _retrying.update { it - id }
            _lines.update { map -> if (line == null) map - id else map + (id to line) }
        }
        val wait = waits.remove(id)
        if (wait == null) put() else wait.end(put)
    }

    /** Try again on a server's row. */
    fun retry(id: String) {
        if (id in pressed || _lines.value[id]?.canRetry != true) return
        pressed[id] = Press(rows[id]?.stamp() ?: (null to null))
        _retrying.update { it + id }
        send(Command.BookSources(BookSourceCommand.RetryProgress(id)))
        one(id)
    }

    /** For shots: [id]'s line as it would stand, its Try again pressed when [pressing]. */
    fun stage(id: String, line: RowLine?, pressing: Boolean = false) {
        _lines.update { if (line == null) it - id else it + (id to line) }
        _retrying.update { if (pressing) it + id else it - id }
    }

    private fun forget(id: String) {
        waits.remove(id)?.cancel()
        rows.remove(id)
        pressed.remove(id)
        counts.remove(id)
        _retrying.update { it - id }
        _lines.update { it - id }
    }
}
