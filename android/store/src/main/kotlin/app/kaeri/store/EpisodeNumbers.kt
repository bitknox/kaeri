// An episode's number as the engine keeps it: text, such as "7", or "5.5" for a half episode.
// The words that take a whole number get one; a half episode is named by the words that take its
// number as text, never cut to a whole one where it names the episode.

package app.kaeri.store

import android.content.Context
import app.kaeri.i18n.Strings

/** The whole number, or null for a half episode such as "5.5" or a number that is not one. */
fun wholeEpisode(number: String): Int? = number.trim().toIntOrNull()

/**
 * As a whole number, for words that count episodes such as a range: a half episode counts as the
 * whole one before it, and a number that is not one as 0.
 */
fun episodeFloor(number: String): Int = wholeEpisode(number) ?: number.trim().toDoubleOrNull()?.toInt() ?: 0

/** "Episode 7", or "Episode 5.5". */
fun episodeName(context: Context, number: String): String =
    wholeEpisode(number)?.let { Strings.episodeNumber(context, it) } ?: Strings.episodeNumberPlain(context, number.trim())

/**
 * The line [whole] words for a whole episode; a half one is named in full, with [rest] after it:
 * "Episode 5.5 · 3 min left".
 */
fun episodeLine(context: Context, number: String, rest: String?, whole: (Int) -> String): String =
    wholeEpisode(number)?.let(whole) ?: listOfNotNull(episodeName(context, number), rest).joinToString(" · ") // i18n-skip
