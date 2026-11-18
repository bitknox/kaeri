// What the title page says of the people the viewer follows on AniList: the phone's row and sheet,
// the TV's row and panel, and the marks on the episodes, alike on the phone, the tablet and the TV.

package app.kaeri.store

import android.content.Context
import app.kaeri.engine.model.FollowedEntry
import app.kaeri.engine.model.FollowedMark
import app.kaeri.engine.model.LibrarySegment
import app.kaeri.i18n.Strings
import kotlin.math.roundToInt

object FollowingWords {
    /** How many pictures a mark shows before "+N". */
    const val MARK_PICTURES = 3

    /** How many pictures the row shows. */
    const val ROW_PICTURES = 3

    /**
     * The line under a name: "Watching · Episode 8" for Watching, Paused and Dropped with some
     * watched; the status word alone for the rest.
     */
    fun line(context: Context, person: FollowedEntry): String {
        val status = person.status ?: return if (person.progress > 0u) Strings.episodeNumber(context, person.progress.toInt()) else ""
        val word = status.label(context)
        val counts = status == LibrarySegment.WATCHING || status == LibrarySegment.PAUSED || status == LibrarySegment.DROPPED
        return if (counts && person.progress > 0u) {
            Strings.moreFollowedProgress(context, word, Strings.episodeNumber(context, person.progress.toInt()))
        } else {
            word
        }
    }

    /** The score out of 10, from AniList's 100: none when they gave none, or it rounds to 0. */
    fun score(person: FollowedEntry): Int? = person.score?.let { (it.toDouble() / 10).roundToInt() }?.takeIf { it > 0 }

    /** "8/10". */
    fun scoreText(context: Context, score: Int): String = Strings.moreFollowedScore(context, score)

    /** "Score 8 out of 10", said in place of [scoreText]. */
    fun scoreSpoken(context: Context, score: Int): String = Strings.followingScoreSpoken(context, score)

    /** The row's second line: "Ines", "Ines and Lumi", or "Ines, Lumi and 8 more". */
    fun names(context: Context, names: List<String>): String = when (names.size) {
        0 -> ""
        1 -> names[0]
        2 -> Strings.followingNamesTwo(context, names[0], names[1])
        else -> Strings.followingNames(context, names[0], names[1], names.size - 2)
    }

    /** "+1", after a mark's three pictures. */
    fun more(context: Context, count: Int): String = Strings.followingMarkMore(context, count)

    /** "Ines, Dario and Oskar watched up to here.", in the language's own list format. */
    fun spoken(context: Context, marks: List<FollowedMark>): String = Strings.followingMarkSpoken(context, Wording.list(marks.map { it.name }))

    /** A person's row as a screen reader says it: name, line, and the score in words. */
    fun said(context: Context, person: FollowedEntry): String = listOfNotNull(
        person.name,
        line(context, person).ifEmpty { null },
        score(person)?.let { scoreSpoken(context, it) },
    ).joinToString(", ") // i18n-skip

    /** The marks of the episodes a folded line holds, each person once, in the rows' order. */
    fun folded(rows: List<app.kaeri.engine.model.EpisodeRow>): List<FollowedMark> =
        rows.flatMap { it.followed }.distinctBy { it.name }
}
