package app.kaeri.ui

import android.content.Context
import androidx.activity.ComponentActivity
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.performClick
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import app.kaeri.design.KaeriTheme
import app.kaeri.engine.model.AiringStatus
import app.kaeri.engine.model.MediaFormat
import app.kaeri.engine.model.Season
import app.kaeri.engine.model.SearchQuery
import app.kaeri.store.HAS_TESTING_ENGINE

/** What the phone's pinned row says for the query: rest, set, and the pill that opens the sheet. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class FilterRowTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val context: Context get() = ApplicationProvider.getApplicationContext()

    private fun row(query: SearchQuery, phone: Boolean = true): FilterRowState {
        assumeTrue(HAS_TESTING_ENGINE)
        return filterRowState(context, query, phone)
    }

    @Test
    fun at_rest_nothing_is_set_and_there_is_no_clear_all() {
        val state = row(SearchQuery())
        assertFalse(state.clearAll)
        assertEquals(listOf("Genres", "Year", "Season", "Format", "Status"), listOf(state.genres, state.year, state.season, state.format, state.status).map { it.label })
        assertFalse(listOf(state.sort, state.genres, state.year, state.season, state.format, state.status).any { it.set })
        assertEquals(0, state.sheetCount)
        assertEquals("All filters", state.sheetSpoken)
        assertEquals("Genres, any", state.genres.spoken)
    }

    @Test
    fun season_waits_for_a_year() {
        assertFalse(row(SearchQuery()).season.enabled)
        val dated = row(SearchQuery().inYear(2026u))
        assertTrue(dated.season.enabled)
        assertEquals("2026", dated.year.label)
        assertTrue(row(SearchQuery(season = Season.FALL)).season.enabled)
    }

    @Test
    fun set_pills_say_their_values_and_the_filters_button_counts_only_the_sheet() {
        val query = SearchQuery(genres = listOf("Fantasy"), formats = listOf(MediaFormat.TV), season = Season.FALL, minPopularity = 1_000u, licensedOnly = true)
            .inYear(2026u)
        val state = row(query)
        assertTrue(state.clearAll)
        assertEquals("Fantasy", state.genres.label)
        assertEquals("Genres, Fantasy", state.genres.spoken)
        assertEquals("2026", state.year.label)
        assertEquals("Autumn", state.season.label)
        assertEquals("TV", state.format.label)
        assertTrue(listOf(state.genres, state.year, state.season, state.format).all { it.set })
        assertEquals(2, state.sheetCount)
        assertEquals("All filters, 2 set", state.sheetSpoken)
    }

    @Test
    fun a_pill_names_what_is_short_and_counts_what_is_not() {
        assertEquals("TV, Movie", row(SearchQuery(formats = listOf(MediaFormat.TV, MediaFormat.MOVIE))).format.label)
        assertEquals("3 formats", row(SearchQuery(formats = listOf(MediaFormat.TV, MediaFormat.MOVIE, MediaFormat.OVA))).format.label)
        assertEquals("2 genres", row(SearchQuery(genres = listOf("Fantasy", "Mecha"))).genres.label)
        assertEquals("Not Mecha", row(SearchQuery(genresExcluded = listOf("Mecha"))).genres.label)
        assertEquals("2 statuses", row(SearchQuery(statuses = listOf(AiringStatus.RELEASING, AiringStatus.FINISHED))).status.label)
    }

    @Test
    fun a_wide_window_keeps_the_words_it_had() {
        assertEquals("One genre", row(SearchQuery(genres = listOf("Fantasy")), phone = false).genres.label)
        assertEquals("2 formats", row(SearchQuery(formats = listOf(MediaFormat.TV, MediaFormat.MOVIE)), phone = false).format.label)
    }

    @Test
    fun clear_all_goes_with_the_filters_and_keeps_the_words() {
        val query = SearchQuery(text = "lantern", genres = listOf("Fantasy"))
        assertTrue(row(query).clearAll)
        assertFalse(row(query.cleared()).clearAll)
        assertEquals("lantern", query.cleared().text)
    }

    @Test
    fun the_filters_pill_reads_with_its_count_and_opens_the_sheet() {
        var opened = 0
        compose.setContent {
            KaeriTheme { FiltersPill(2, "All filters, 2 set") { opened++ } }
        }
        val pill = compose.onNodeWithContentDescription("All filters, 2 set")
        assertTrue(pill.fetchSemanticsNode().config[SemanticsProperties.Selected])
        pill.performClick()
        assertEquals(1, opened)
    }
}
