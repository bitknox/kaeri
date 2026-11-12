// Round 5 audit: Updates with two followed series that each got the same chapter number on the
// same day. Ignored with its bug id until it is fixed.

package app.kaeri.ui

import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.engine.model.UpdatesView
import app.kaeri.store.AppStore
import app.kaeri.store.FixtureScene
import app.kaeri.store.ReadingModel
import kotlinx.coroutines.flow.MutableStateFlow
import org.junit.Ignore
import org.junit.Test
import org.robolectric.annotation.Config

class R5AuditUpdatesTest : ReadingShotHarness() {
    /**
     * A chapter's id is its key within its work ("12"), and the engine puts every followed work's
     * new chapters into one UpdateDay; Updates.kt:148 keys the rows "r-<day>-<chapter id>" without
     * the work, so the LazyColumn sees the same key twice and throws.
     */
    @Ignore("R5A-1")
    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE)
    fun two_series_with_the_same_chapter_on_one_day() {
        var asked = false
        var shown: UpdatesView? = null
        var held = 0
        shoot(
            "r5a-updates-same-chapter",
            FixtureScene.READING_HOME,
            Destination.Updates,
            medium = KaeriIsFor.READ,
            then = { store ->
                val view = requireNotNull(store.reading.updates.value)
                val day = view.days.first { it.rows.isNotEmpty() }
                val row = day.rows.first()
                // Another followed series whose chapter of the same number came out the same day.
                val other = row.copy(tile = row.tile.copy(work = row.tile.work + "-other"))
                val rows = listOf(row, other) + day.rows.drop(1)
                val twice = view.copy(days = view.days.map { if (it === day) it.copy(rows = rows) else it })
                shown = twice
                updatesFlow(store).value = twice
            },
        ) { store ->
            if (!asked) {
                asked = true
                store.reading.loadUpdates()
            }
            val staged = shown
            if (staged == null) {
                store.reading.updates.value?.days?.any { it.rows.isNotEmpty() } == true
            } else if (store.reading.updates.value !== staged) {
                // The engine's own answer came after the staging: stage it again over it.
                updatesFlow(store).value = staged
                held = 0
                false
            } else {
                // Held for several frames, so the list is composed with both rows.
                ++held >= HELD_FRAMES
            }
        }
        // Reaching here means the list composed both rows: with the bug, LazyColumn throws on
        // the repeated key while the staged list is held.
    }

    private companion object {
        const val HELD_FRAMES = 10
    }

    @Suppress("UNCHECKED_CAST")
    private fun updatesFlow(store: AppStore): MutableStateFlow<UpdatesView?> =
        ReadingModel::class.java.getDeclaredField("_updates").apply { isAccessible = true }.get(store.reading) as MutableStateFlow<UpdatesView?>
}
