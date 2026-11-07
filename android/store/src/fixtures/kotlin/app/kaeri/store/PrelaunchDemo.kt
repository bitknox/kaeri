// Prelaunch board piece 2, the reading demo, staged for shots on the fixtures engine (see
// Prelaunch.kt). The place each state shows comes from the usual steers: `firstLaunch=1` for the
// books page, `page=` for the demo's series page, About the demo and Your sources, `section=search`.

package app.kaeri.store

import app.kaeri.engine.model.FirstRunPage
import app.kaeri.engine.model.KaeriIsFor

object PrelaunchDemo {
    val STATES: List<String> = listOf("p2-01", "p2-02", "p2-03", "p2-04", "p2-07", "p2-08", "p2-row")

    /** What Search is asked for state 8: the demo's own word, which every demo work matches. */
    const val SEARCH = "Demo" // i18n-skip

    suspend fun stage(store: AppStore, state: String): Boolean {
        if (state !in STATES) return false
        // Both for state 2; Read alone everywhere else, as the board draws them.
        val medium = if (state == "p2-02") KaeriIsFor.BOTH else KaeriIsFor.READ
        // The engine takes each command in turn, so nothing here waits on the one before.
        if (store.medium.view.value?.answer != medium) store.medium.choose(medium)
        when (state) {
            // First run's books page, with Try it now: the demo is offered, not yet on.
            "p2-01", "p2-02" -> store.sync.join.startAt(FirstRunPage.SOURCES)
            else -> {
                if (!store.demo.value.on) store.addDemo()
                when (state) {
                    "p2-03" -> store.reading.loadHome(refresh = true)
                    "p2-08" -> store.reading.runSearch(ReadingModel.EMPTY_QUERY.copy(text = SEARCH))
                    "p2-row" -> store.bookSources.load()
                }
            }
        }
        return true
    }
}
