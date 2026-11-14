// The Read from sheet's Not found in rows, staged for shots on the fixtures engine: its sources find
// every series, so a shot says the misses itself. One source has no series by the work's names and
// one has another edition, offered as a possible match. Names are made up.

package app.kaeri.store

import app.kaeri.engine.model.EditionMiss
import app.kaeri.engine.model.Msg
import app.kaeri.engine.model.ReadingMsg
import app.kaeri.engine.model.SourceSeriesChoice

object ReadFromStage {
    /** Show [work]'s misses the next time the engine says which sources hold it. */
    fun stage(store: AppStore, work: String) {
        val cover = store.reading.series.value[work]?.cover
        store.bookSources.stagedMisses = mapOf(
            work to listOf(
                EditionMiss(
                    sourceId = "fixture-comics", // i18n-skip
                    sourceName = "Comics server", // i18n-skip
                    reason = Msg.Reading(ReadingMsg.ReadingMissNoSeries),
                    possible = null,
                    query = null,
                ),
                EditionMiss(
                    sourceId = "fixture-northwind", // i18n-skip
                    sourceName = "Northwind", // i18n-skip
                    reason = Msg.Reading(ReadingMsg.ReadingMissPossible),
                    possible = SourceSeriesChoice(
                        seriesRef = "ferry-colour", // i18n-skip
                        title = "The Lantern Ferry - Full Color", // i18n-skip
                        cover = cover,
                        year = 2019u,
                        language = "en", // i18n-skip
                        confidence = 70u,
                    ),
                    query = null,
                ),
            ),
        )
    }
}
