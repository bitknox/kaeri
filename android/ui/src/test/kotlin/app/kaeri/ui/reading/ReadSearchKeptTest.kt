package app.kaeri.ui.reading

import android.os.Bundle
import android.os.Parcel
import androidx.activity.ComponentActivity
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.remember
import androidx.compose.ui.test.junit4.StateRestorationTester
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.ReadingCommand
import app.kaeri.store.AppStore
import app.kaeri.store.ReadingModel
import app.kaeri.ui.LocalAppStore
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * Search on reading's side after the process went away: the Reading scope, the words, the genre
 * and Your sources only come back with the screen, and the search is asked again.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class ReadSearchKeptTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private fun newStore() = AppStore(CoroutineScope(SupervisorJob() + Dispatchers.Main))

    private val query = ReadingModel.EMPTY_QUERY.copy(
        text = "paper kite",
        medium = Medium.MANGA,
        formats = listOf("one-shot"),
        origins = listOf("kr"),
        status = "finished",
        genres = listOf("Comedy"),
        excludeGenres = listOf("Drama"),
        sort = "score",
        started = "2020",
        sourcesOnly = true,
        sourceScope = listOf("shelf"),
    )

    @After
    fun reset() {
        ReadPlaces.searchReading = false
    }

    @Test
    fun every_field_of_the_query_survives_a_parcel() {
        val parcel = Parcel.obtain()
        parcel.writeBundle(query.toBundle())
        parcel.setDataPosition(0)
        val back = parcel.readBundle(javaClass.classLoader)!!
        parcel.recycle()
        assertEquals(query, back.readSearchQuery())
        val plain = ReadingModel.EMPTY_QUERY
        assertEquals(plain, plain.toBundle().readSearchQuery())
        assertEquals(plain, Bundle().readSearchQuery())
    }

    @Test
    fun the_reading_scope_words_and_filters_come_back_in_a_new_process() {
        val old = newStore()
        val fresh = newStore()
        var compositions = 0
        val seen = mutableListOf<Boolean>()
        val tester = StateRestorationTester(compose)
        tester.setContent {
            // The first composition is the process that saved; the restored one is a new process,
            // with a store that asked nothing and Search on Shows.
            val store = remember {
                compositions++
                if (compositions == 1) old else fresh.also { ReadPlaces.searchReading = false }
            }
            CompositionLocalProvider(LocalAppStore provides store) {
                KeepReadSearch()
                seen += ReadPlaces.searchReading
            }
        }
        compose.runOnIdle {
            ReadPlaces.searchReading = true
            old.reading.runSearch(query)
        }
        compose.waitForIdle()
        seen.clear()
        tester.emulateSavedInstanceStateRestore()
        compose.waitForIdle()
        assertEquals(2, compositions)
        assertEquals("the restored screen draws on reading's side from its first frame", true, seen.first())
        assertTrue(ReadPlaces.searchReading)
        assertEquals(query, fresh.reading.search.value.query)
        assertTrue("the search is asked again", fresh.waiting.contains(Command.Reading(ReadingCommand.Search(query))))
    }

    @Test
    fun a_process_that_kept_its_search_is_left_alone() {
        val store = newStore()
        val tester = StateRestorationTester(compose)
        tester.setContent {
            CompositionLocalProvider(LocalAppStore provides store) { KeepReadSearch() }
        }
        compose.runOnIdle {
            ReadPlaces.searchReading = true
            store.reading.runSearch(query)
        }
        val newer = query.copy(text = "kite rider")
        compose.runOnIdle { store.reading.runSearch(newer) }
        val asked = store.waiting.size
        tester.emulateSavedInstanceStateRestore()
        compose.waitForIdle()
        assertEquals(newer, store.reading.search.value.query)
        assertEquals("nothing asked twice", asked, store.waiting.size)
    }
}
