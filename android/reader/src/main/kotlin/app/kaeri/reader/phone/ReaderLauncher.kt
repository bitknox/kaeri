// How the rest of the app opens the reader: a request on a flow the app's reader overlay reads.

package app.kaeri.reader.phone

import app.kaeri.engine.core.ChapterId
import app.kaeri.engine.core.WorkId
import app.kaeri.engine.model.ReadPlace
import app.kaeri.reader.PageImages
import app.kaeri.reader.ReaderSession
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * A chapter to read: the work's resume place when [chapter] is null. [cover] is the series'
 * cover, shown dimmed where the page goes while the chapter opens.
 */
data class ReaderRequest(
    val work: WorkId,
    val chapter: ChapterId? = null,
    val eink: Boolean = false,
    val cover: String? = null,
    /**
     * The cover's place on screen the reader grows out of and closes back into, as the series
     * screens key it; `null` opens it with the plain zoom.
     */
    val from: String? = null,
)

object ReaderLauncher {
    private val _request = MutableStateFlow<ReaderRequest?>(null)
    val request: StateFlow<ReaderRequest?> = _request.asStateFlow()

    private val _settings = MutableStateFlow<SettingsPane?>(null)

    /** A settings page asked for from outside the reader, such as the Do Not Disturb rule's. */
    val settings: StateFlow<SettingsPane?> = _settings.asStateFlow()

    /** The open request's place, kept while the activity is made anew (font size, language). */
    private var kept: Pair<ReaderRequest, ReadPlace>? = null

    /**
     * The open reader, still open, while its activity is made anew for a change of font size,
     * display size or language: the next reader for its request takes it on, pages and place and
     * all, with nothing asked of the source again.
     */
    class Retained internal constructor(
        val request: ReaderRequest,
        val session: ReaderSession,
        val images: PageImages,
        /** Closes the session, for when no reader takes it on. */
        val release: () -> Unit,
    )

    private var retained: Retained? = null

    fun open(request: ReaderRequest) {
        kept = null
        drop()
        _request.value = request
    }

    fun close() {
        kept = null
        drop()
        _request.value = null
    }

    /**
     * The reader of [request] goes with its activity, which is made anew at once: its open
     * [session] waits for the next reader of the request. Closed at once when the request went.
     */
    fun retain(request: ReaderRequest, session: ReaderSession, images: PageImages, release: () -> Unit) {
        drop()
        if (_request.value != request) {
            release()
            return
        }
        retained = Retained(request, session, images, release)
    }

    /** The open session the reader of [request] takes on, once; null when there is none. */
    fun takeRetained(request: ReaderRequest): Retained? {
        val held = retained?.takeIf { it.request == request } ?: return null
        retained = null
        return held
    }

    private fun drop() {
        val held = retained ?: return
        retained = null
        held.release()
    }

    /**
     * The reader of [request] goes with its activity while the request stays: the next reader for
     * it opens at [place], the chapter and page it showed, and not where the request began.
     */
    fun keep(request: ReaderRequest, place: ReadPlace?) {
        if (_request.value != request) return
        kept = place?.let { request to it }
    }

    /** Where the reader of [request] was when its activity went, if it went while open. */
    fun keptPlace(request: ReaderRequest): ReadPlace? = kept?.takeIf { it.first == request }?.second

    /** Show the open reader's settings on [pane]. */
    fun showSettings(pane: SettingsPane) {
        _settings.value = pane
    }

    /** The reader showed the settings asked for. */
    fun settingsShown() {
        _settings.value = null
    }
}
