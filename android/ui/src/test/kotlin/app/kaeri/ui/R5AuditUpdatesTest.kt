// Updates with two followed series that each got the same chapter number on the same day: both
// rows are drawn.

package app.kaeri.ui

import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.engine.model.UpdatesView
import app.kaeri.store.AppStore
import app.kaeri.store.FixtureScene
import app.kaeri.store.ReadingModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch
import org.junit.Assert.assertTrue
import org.junit.Test
import org.robolectric.annotation.Config

class R5AuditUpdatesTest : ReadingShotHarness() {
    /**
     * A chapter's id is its key within its work ("12"), and one UpdateDay holds every followed
     * work's new chapters, so a row's key in the list names its work as well as its chapter. The
     * engine answers Updates more than once (again once the covers' details land), so the second
     * series is put back into every answer until the screen is captured.
     */
    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE)
    fun two_series_with_the_same_chapter_on_one_day() {
        var asked = false
        shoot(
            "r5a-updates-same-chapter",
            FixtureScene.READING_HOME,
            Destination.Updates,
            medium = KaeriIsFor.READ,
            then = { store ->
                val updates = updatesFlow(store)
                updates.value = twice(updates.value)
                scope.launch { updates.collect { view -> twice(view).takeIf { it !== view }?.let { updates.value = it } } }
            },
        ) { store ->
            if (!asked) {
                asked = true
                store.reading.loadUpdates()
            }
            store.reading.updates.value?.days?.any { it.rows.isNotEmpty() } == true
        }
        assertTrue("both series' rows were in the list drawn", sameChapterTwice(store?.reading?.updates?.value))
    }

    /** [view] with another followed series whose chapter of the same number came out the same day. */
    private fun twice(view: UpdatesView?): UpdatesView? {
        if (view == null || sameChapterTwice(view)) return view
        val day = view.days.firstOrNull { it.rows.isNotEmpty() } ?: return view
        val row = day.rows.first()
        val other = row.copy(tile = row.tile.copy(work = row.tile.work + "-other"))
        val rows = listOf(row, other) + day.rows.drop(1)
        return view.copy(days = view.days.map { if (it === day) it.copy(rows = rows) else it })
    }

    private fun sameChapterTwice(view: UpdatesView?): Boolean = view?.days.orEmpty().any { day ->
        day.rows.groupBy { it.chapter.id }.values.any { rows -> rows.map { it.tile.work }.distinct().size > 1 }
    }

    @Suppress("UNCHECKED_CAST")
    private fun updatesFlow(store: AppStore): MutableStateFlow<UpdatesView?> =
        ReadingModel::class.java.getDeclaredField("_updates").apply { isAccessible = true }.get(store.reading) as MutableStateFlow<UpdatesView?>
}
