// How the rest of the app opens the reader: a request on a flow the app's reader overlay reads.

package app.kaeri.reader.phone

import app.kaeri.engine.core.ChapterId
import app.kaeri.engine.core.WorkId
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/** A chapter to read: the work's resume place when [chapter] is null. */
data class ReaderRequest(val work: WorkId, val chapter: ChapterId? = null, val eink: Boolean = false)

object ReaderLauncher {
    private val _request = MutableStateFlow<ReaderRequest?>(null)
    val request: StateFlow<ReaderRequest?> = _request.asStateFlow()

    fun open(request: ReaderRequest) {
        _request.value = request
    }

    fun close() {
        _request.value = null
    }
}
