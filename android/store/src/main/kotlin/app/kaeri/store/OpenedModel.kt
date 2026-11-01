// Opening your own magnet link or torrent file: guess what it is, then confirm. The sheet's stage
// lives here so a link handed over by the system opens the same sheet as the Open link or file
// item does, on the phone and on the TV.

package app.kaeri.store

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import app.kaeri.engine.model.Entry
import app.kaeri.engine.model.Msg
import app.kaeri.engine.model.OpenedLink

class OpenedModel {
    /** Where the person is in opening a link. */
    sealed interface Stage {
        /** Asking for a link or a file. [reading] while the engine reads what was given. */
        data class Paste(val reading: Boolean = false, val unusable: Boolean = false) : Stage

        /**
         * "Is this <title>, episode N?": [title] and [episode] are the guess or what Change picked.
         * [title] is null when the name matched nothing.
         */
        data class Confirm(val opened: OpenedLink, val title: Entry?, val episode: String) : Stage

        /** Change: [results] for the words searched, null until the first search, [failed] with why when none could be made. */
        data class Change(
            val opened: OpenedLink,
            val query: String,
            val results: List<Entry>?,
            val searching: Boolean = false,
            val failed: Msg? = null,
        ) : Stage
    }

    private val _stage = MutableStateFlow<Stage?>(null)
    val stage: StateFlow<Stage?> = _stage.asStateFlow()

    /** Open the sheet on the field for a link or a file. */
    fun openPaste() {
        _stage.value = Stage.Paste()
    }

    /** A link or file was sent to the engine: wait for what it makes of it. */
    fun reading() {
        _stage.value = Stage.Paste(reading = true)
    }

    /** The engine read the link and guessed. */
    internal fun linkOpened(opened: OpenedLink) {
        val guess = opened.guess
        _stage.value = Stage.Confirm(opened, guess?.title, guess?.episode.orEmpty())
    }

    /** The engine could not use the link or file: the field says so. */
    fun unusable() {
        if (_stage.value is Stage.Paste) _stage.value = Stage.Paste(unusable = true)
    }

    /** Change: list what the link could be, the engine's candidates first. */
    fun change() {
        val confirm = _stage.value as? Stage.Confirm ?: return
        _stage.value = Stage.Change(confirm.opened, query = "", results = confirm.opened.candidates)
    }

    /** A search for other words was sent. */
    fun searching(query: String) {
        val change = _stage.value as? Stage.Change ?: return
        _stage.value = change.copy(query = query, searching = true, failed = null)
    }

    /** The answer to a search: stale answers, for words no longer in the field, are dropped. */
    internal fun choices(link: String, query: String, candidates: List<Entry>, failed: Msg?) {
        val change = _stage.value as? Stage.Change ?: return
        if (change.opened.link != link || change.query != query) return
        _stage.value = change.copy(results = candidates, searching = false, failed = failed)
    }

    /** A title chosen under Change: back to the confirmation, asking for its episode. */
    fun pick(entry: Entry) {
        val change = _stage.value as? Stage.Change ?: return
        _stage.value = Stage.Confirm(change.opened, entry, episode = "")
    }

    /** Back from Change to the confirmation it came from. */
    fun back() {
        val change = _stage.value as? Stage.Change ?: return
        val guess = change.opened.guess
        _stage.value = Stage.Confirm(change.opened, guess?.title, guess?.episode.orEmpty())
    }

    /** Change the episode typed under the question. */
    fun episode(text: String) {
        val confirm = _stage.value as? Stage.Confirm ?: return
        _stage.value = confirm.copy(episode = text)
    }

    fun close() {
        _stage.value = null
    }
}
