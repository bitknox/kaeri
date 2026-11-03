// Made-up episode text and stills for the synopsis shots: the synopsis board's titles and lines,
// laid over a scene's table, so no scene needs re-recording.

package app.kaeri.testkit

import app.kaeri.engine.core.AnilistId
import app.kaeri.engine.model.EpisodeRow

object SynopsisFixtures {
    val TITLES = listOf(
        "Paper Lanterns", "The Quiet Harbour", "Dust on the Compass", "Salt and Copper", "Mail for the Far Shore", "Low Tide at Noon",
        "Rain over Kiriya Pass", "A Letter Never Sent", "The Last Lamplighter", "Where the Roads Cross", "A Map for Nobody", "The Long Way Down",
    )

    /** One line per title; the tenth has none, as the board's last row. */
    val TEXT = listOf(
        "The crew leave port with a hold full of lanterns and no one to light them for.",
        "The crew wait out a calm at a floating harbour where nobody has docked in years. Ryo keeps the engine warm through the night.",
        "A broken compass sends the ship in circles until Toma trusts the stars instead of the needle.",
        "Trading at a copper market, Ryo finds a buyer for something he never meant to sell.",
        "A parcel addressed to a harbour that no longer exists must still reach somebody.",
        "The sea pulls back farther than anyone remembers, and the ship sits on dry sand for a day.",
        "A storm closes Kiriya Pass with the mail still aboard. Toma has to choose between turning back and flying the old route under the cloud line, the one her mentor never came back from.",
        "Ryo finds the letter Toma has carried since the first night, and reads it before she can stop him. Neither of them speaks again until the lamps of Hoshimi come up out of the cloud.",
        "On an island with one lamp left, the crew meet the keeper who lights it every night for a ship that stopped coming. Ryo stays behind to help, and nearly misses the morning departure. The keeper gives Toma a map that was never meant to leave the island.",
        "",
        "A map with no names on it leads the crew to a coast that has none either.",
        "The pass opens again, and the way down is longer than the way up.",
    )

    /** What the engine marks on a table: unfinished rows after the first unfinished one, the next episode, are ahead. */
    private fun marked(rows: List<EpisodeRow>): List<EpisodeRow> {
        val next = rows.indexOfFirst { !it.completed }
        return rows.mapIndexed { index, row -> row.copy(spoilerAhead = next >= 0 && index > next && !row.completed) }
    }

    /** The scene's episodes with the board's titles, text and stills (the fixtures serve one wide drawing for every still). */
    fun withSynopses(id: AnilistId, rows: List<EpisodeRow>): List<EpisodeRow> = marked(rows).mapIndexed { index, row ->
        row.copy(
            title = TITLES.getOrNull(index) ?: row.title,
            description = TEXT.getOrNull(index)?.takeIf { it.isNotEmpty() },
            thumbnail = "https://img.example/ep/$id/${row.number}.jpg",
            durationSecs = 1440u,
            airsAt = 1_400_000_000L + index * 604_800L,
        )
    }

    /** The same without any text coming, nor a still. */
    fun withoutText(id: AnilistId, rows: List<EpisodeRow>): List<EpisodeRow> =
        withSynopses(id, rows).map { it.copy(description = null) }

    /** 230 episodes with the first 150 watched, as the ranges board's long runner. */
    fun long(id: AnilistId, rows: List<EpisodeRow>): List<EpisodeRow> = marked((1..230).map { n ->
        val base = rows[(n - 1) % rows.size]
        base.copy(
            number = n.toString(), title = TITLES[(n - 1) % TITLES.size], description = TEXT[(n - 1) % TEXT.size].takeIf { it.isNotEmpty() },
            thumbnail = "https://img.example/ep/$id/$n.jpg", durationSecs = 1440u, completed = n <= 150, watched = if (n <= 150) 1.0 else 0.0,
            airsAt = 1_300_000_000L + n * 604_800L, kind = null, skippable = false,
        )
    })
}
