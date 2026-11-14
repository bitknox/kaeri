// Reading's Search through the process going away: the scope, the words and every filter come back
// with the screen, as the rest of the screen's own state does.

package app.kaeri.ui.reading

import android.os.Bundle
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.Saver
import androidx.compose.runtime.saveable.rememberSaveable
import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.ReadSearchQuery
import app.kaeri.store.AppStore
import app.kaeri.ui.LocalAppStore

private const val KEY_READING = "reading" // i18n-skip
private const val KEY_TEXT = "text" // i18n-skip
private const val KEY_MEDIUM = "medium" // i18n-skip
private const val KEY_FORMATS = "formats" // i18n-skip
private const val KEY_ORIGINS = "origins" // i18n-skip
private const val KEY_STATUS = "status" // i18n-skip
private const val KEY_GENRES = "genres" // i18n-skip
private const val KEY_EXCLUDE = "exclude" // i18n-skip
private const val KEY_SORT = "sort" // i18n-skip
private const val KEY_STARTED = "started" // i18n-skip
private const val KEY_SOURCES_ONLY = "sourcesOnly" // i18n-skip
private const val KEY_SCOPE = "scope" // i18n-skip

/** [this] query in a bundle, every field of it. */
internal fun ReadSearchQuery.toBundle(): Bundle = Bundle().apply {
    putString(KEY_TEXT, text)
    putString(KEY_MEDIUM, medium?.name)
    putStringArrayList(KEY_FORMATS, ArrayList(formats))
    putStringArrayList(KEY_ORIGINS, ArrayList(origins))
    putString(KEY_STATUS, status)
    putStringArrayList(KEY_GENRES, ArrayList(genres))
    putStringArrayList(KEY_EXCLUDE, ArrayList(excludeGenres))
    putString(KEY_SORT, sort)
    putString(KEY_STARTED, started)
    putBoolean(KEY_SOURCES_ONLY, sourcesOnly)
    sourceScope?.let { putStringArrayList(KEY_SCOPE, ArrayList(it)) }
}

/** The query [toBundle] put in [this]. */
internal fun Bundle.readSearchQuery(): ReadSearchQuery = app.kaeri.store.ReadingModel.EMPTY_QUERY.copy(
    text = getString(KEY_TEXT).orEmpty(),
    medium = getString(KEY_MEDIUM)?.let { name -> Medium.entries.firstOrNull { it.name == name } },
    formats = getStringArrayList(KEY_FORMATS).orEmpty(),
    origins = getStringArrayList(KEY_ORIGINS).orEmpty(),
    status = getString(KEY_STATUS),
    genres = getStringArrayList(KEY_GENRES).orEmpty(),
    excludeGenres = getStringArrayList(KEY_EXCLUDE).orEmpty(),
    sort = getString(KEY_SORT),
    started = getString(KEY_STARTED),
    sourcesOnly = getBoolean(KEY_SOURCES_ONLY),
    sourceScope = getStringArrayList(KEY_SCOPE),
)

/** What a restored Search had: the scope, and reading's query. Empty when nothing was restored. */
private class KeptReadSearch(val reading: Boolean? = null, val query: ReadSearchQuery? = null)

/** Saved as the activity is: what Search stands on at that moment, from [store] and [ReadPlaces]. */
private fun keptSaver(store: AppStore) = Saver<KeptReadSearch, Bundle>(
    save = {
        Bundle().apply {
            putBoolean(KEY_READING, ReadPlaces.searchReading)
            putBundle(KEY_TEXT, store.reading.search.value.query.toBundle())
        }
    },
    restore = { saved -> KeptReadSearch(saved.getBoolean(KEY_READING), saved.getBundle(KEY_TEXT)?.readSearchQuery()) },
)

/**
 * Keeps Search's side and reading's search when the activity is saved, and puts them back when it
 * is restored in a new process: the Reading scope, the words and every filter. Called before
 * Search reads [ReadPlaces.searchReading], so the restored screen draws on reading's side at once.
 */
@Composable
internal fun KeepReadSearch() {
    val store = LocalAppStore.current
    val kept = rememberSaveable(saver = remember(store) { keptSaver(store) }) { KeptReadSearch() }
    // Restored: the scope and the query as they were, before anything reads them. A process that
    // kept its search has it already.
    remember(kept) {
        kept.reading?.let { ReadPlaces.searchReading = it }
        kept.query?.let { store.reading.restoreQuery(it) }
    }
    // A new process asks again for what was there.
    LaunchedEffect(kept) {
        val query = kept.query ?: return@LaunchedEffect
        val now = store.reading.search.value
        if (now.query == query && now.tiles.isEmpty() && !now.loading && !query.isStart()) store.reading.runSearch(query)
    }
}
