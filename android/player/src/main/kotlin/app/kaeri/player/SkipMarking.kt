// Marking skip times: the engine holds the steps and the rules; these are the words and values the
// phone's panel, the TV's panel and the prompts draw. After Windows' and Linux's marking bar.

package app.kaeri.player

import android.content.Context
import app.kaeri.engine.model.SkipKind
import app.kaeri.engine.model.SkipMarkCheck
import app.kaeri.engine.model.SkipMarkNote
import app.kaeri.engine.model.SkipNotice
import app.kaeri.engine.model.SkipOrigin
import app.kaeri.engine.model.SkipPrompt
import app.kaeri.i18n.Strings

/** The kinds the marking bar offers, in its segmented control's order. */
val SkipKinds = listOf(SkipKind.OPENING, SkipKind.ENDING, SkipKind.RECAP)

/** A prompt on the video, since when it has been up. */
data class SkipPromptOffer(val prompt: SkipPrompt, val since: Long) {
    /** Wrong times stays 8 s, Was that the opening 12 s, as on the desktop. */
    val stays: Long get() = if (prompt is SkipPrompt.WrongTimes) 8_000L else 12_000L
}

/** A notice about the skip times, since when. */
data class SkipNoticeOffer(val notice: SkipNotice, val since: Long)

/** The words for skip times. Times are seconds of the file. */
object SkipWords {
    /** "Opening", the segmented control's choice. */
    fun kind(context: Context, kind: SkipKind): String = when (kind) {
        SkipKind.OPENING -> Strings.playerMarkKindOpening(context)
        SkipKind.ENDING -> Strings.playerMarkKindEnding(context)
        SkipKind.RECAP -> Strings.playerMarkKindRecap(context)
    }

    /** "opening", inside a sentence. */
    fun inline(context: Context, kind: SkipKind): String = when (kind) {
        SkipKind.OPENING -> Strings.playerMarkKindOpeningInline(context)
        SkipKind.ENDING -> Strings.playerMarkKindEndingInline(context)
        SkipKind.RECAP -> Strings.playerMarkKindRecapInline(context)
    }

    private fun plural(context: Context, kind: SkipKind): String = when (kind) {
        SkipKind.OPENING -> Strings.playerMarkKindOpeningPlural(context)
        SkipKind.ENDING -> Strings.playerMarkKindEndingPlural(context)
        SkipKind.RECAP -> Strings.playerMarkKindRecapPlural(context)
    }

    /** "1 min 30 s": the review's length. */
    fun length(context: Context, seconds: Double): String {
        val total = Math.round(maxOf(0.0, seconds)).toInt()
        val minutes = total / 60
        val rest = total % 60
        return when {
            minutes == 0 -> Strings.durationSeconds(context, rest)
            rest > 0 -> Strings.durationMinutesSeconds(context, minutes, "%02d".format(rest))
            else -> Strings.durationMinutes(context, minutes)
        }
    }

    private fun most(context: Context, seconds: Double): String =
        Strings.timeMinutes(context, maxOf(1, Math.round(seconds / 60.0).toInt()))

    /** The line under the edges for a check that is not fine, else `null`. */
    fun check(context: Context, kind: SkipKind, check: SkipMarkCheck): String? = when (check) {
        SkipMarkCheck.Fine -> null
        SkipMarkCheck.TooShort -> Strings.playerMarkTooShort(context)
        is SkipMarkCheck.TooLong -> Strings.playerMarkTooLong(context, plural(context, kind), most(context, check.most))
    }

    /** A review line the engine sends. */
    fun note(context: Context, note: SkipMarkNote): String = when (note) {
        SkipMarkNote.Shares -> Strings.playerMarkShares(context)
        is SkipMarkNote.Replaces -> Strings.playerMarkReplaces(context, PlayerClock.label(note.start), PlayerClock.label(note.end))
        SkipMarkNote.NoEntry -> Strings.playerMarkNoMal(context)
    }

    /** The notice an action on the skip times says once. */
    fun notice(context: Context, notice: SkipNotice): String = when (notice) {
        is SkipNotice.Saved -> Strings.playerMarkSaved(context, inline(context, notice.kind))
        is SkipNotice.SavedHere -> Strings.playerMarkTooLong(context, plural(context, notice.kind), most(context, notice.most))
        SkipNotice.Confirmed -> Strings.playerMarkConfirmed(context)
        SkipNotice.Reported -> Strings.playerSkipReported(context)
        SkipNotice.HiddenHere -> Strings.playerSkipHiddenHere(context)
    }

    /** Where a time came from, as the Skip times list says it. */
    fun origin(context: Context, origin: SkipOrigin?): String = when (origin) {
        SkipOrigin.YOURS -> Strings.playerSkipYours(context)
        SkipOrigin.ANI_SKIP -> Strings.playerSkipFromAniskip(context)
        else -> Strings.playerSkipFromFile(context)
    }

    /** The step's instruction. */
    fun step(context: Context, kind: SkipKind, starting: Boolean): String =
        if (starting) Strings.playerMarkStepStart(context, inline(context, kind)) else Strings.playerMarkStepEnd(context, inline(context, kind))
}
