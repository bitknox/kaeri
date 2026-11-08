// A title's wide art: the banner, else a still from an episode already seen, else nothing wide,
// and the screens set the cover beside the title instead. The player's starting screen follows
// the same rule. The still comes from the episode table once it is here, and before that from the
// one the engine sends with the title (`Entry.seenStill`), by the same rule.

package app.kaeri.store

import app.kaeri.engine.model.Entry
import app.kaeri.engine.model.EpisodeRow

sealed interface HeroArt {
    data class Banner(override val url: String) : HeroArt
    data class Still(override val url: String) : HeroArt

    /**
     * No banner and no still known, and the episodes whose stills might stand in are on their
     * way: the cover's colour as a plate, without the cover, so nothing shows twice. For [WAIT_MS]
     * at most, then [Compact] until a still lands.
     */
    data object Waiting : HeroArt

    /** Nothing wide to show. */
    data object Compact : HeroArt

    /** The URL to draw, when there is one. */
    val url: String? get() = null

    companion object {
        /**
         * How long the plate waits for the episodes that might bring a still: as long as a screen
         * waits before it shows placeholders.
         */
        const val WAIT_MS = 600L

        /**
         * The art for [entry], with its episode table as far as it is known: `null` while it
         * loads. [waited] says the plate has shown for [WAIT_MS]: the art is then what it would
         * be with no still.
         */
        fun of(entry: Entry, episodes: List<EpisodeRow>?, waited: Boolean = false): HeroArt {
            entry.bannerUrl?.let { return Banner(it) }
            episodes?.let { seenStill(it, entry.progress?.done ?: 0u) }?.let { return Still(it) }
            entry.seenStill?.let { return Still(it) }
            return if (episodes == null && !waited) Waiting else Compact
        }

        /**
         * The still of the latest episode watched that has one, else episode 1's: never one from
         * an episode not yet seen past the first. Watched is the table's own word, or within what
         * the tracker counts as done.
         */
        fun seenStill(rows: List<EpisodeRow>, done: UInt): String? {
            val watched = rows.filter { row ->
                row.completed || (row.number.toDoubleOrNull()?.let { it <= done.toDouble() } == true)
            }
            watched.filter { it.thumbnail != null }
                .maxByOrNull { it.number.toDoubleOrNull() ?: 0.0 }
                ?.thumbnail?.let { return it }
            val first = rows.firstOrNull { it.number.toDoubleOrNull() == 1.0 } ?: rows.firstOrNull()
            return first?.thumbnail
        }
    }
}
