// A long series, staged for shots on the fixtures engine: every chapter list from now on holds
// [count] chapters made from its first, so a page of chapters can be seen full. Titles are made up,
// and every fifth chapter has none.

package app.kaeri.store

import app.kaeri.engine.model.VolumeGroup

object LongChaptersStage {
    /** Every chapter list from now on holds [count] chapters; off puts the engine's back. */
    fun stage(store: AppStore, count: Int = 41, on: Boolean = true) {
        store.reading.chaptersShot = if (on) { list -> list.copy(volumes = long(list.volumes, count)) } else null
    }

    fun long(volumes: List<VolumeGroup>, count: Int): List<VolumeGroup> {
        val first = volumes.firstOrNull() ?: return volumes
        val template = volumes.flatMap { it.chapters }.firstOrNull() ?: return volumes
        val rows = (1..count).map { n ->
            template.copy(
                id = "${template.id}-long-$n",
                label = "Chapter $n", // i18n-skip
                number = "$n",
                // Every fifth chapter has no title of its own, as many sources leave it.
                title = if (n % 5 == 0) null else TITLES[(n - 1) % TITLES.size],
                newChapter = false,
            )
        }
        return listOf(first.copy(chapters = rows))
    }

    private val TITLES = listOf(
        "The First Landing", "Rope and Rail", "A Door Ajar", "Second Wind", "Under the Arch", "Paper Moths", "The Quiet Floor", // i18n-skip
        "Old Keys", "A Long Fall", "Counting Steps", "The Turn", "The Long Stair", "Lanterns Out", "After Dark", // i18n-skip
    )
}
