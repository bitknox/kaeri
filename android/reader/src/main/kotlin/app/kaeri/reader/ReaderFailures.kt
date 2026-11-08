// A reading failure in the person's words: the same message the engine gives it
// (ReadingFailure::msg in kaeri-model), so every app says the same thing.

package app.kaeri.reader

import android.content.Context
import app.kaeri.engine.model.ReadingFailure
import app.kaeri.engine.model.ReadingMsg
import app.kaeri.engine.model.readingFailureMsg
import app.kaeri.store.text

/** The message for this failure. */
fun ReadingFailure.msg(): ReadingMsg = when (this) {
    is ReadingFailure.Offline -> ReadingMsg.ReadingFailedOffline
    is ReadingFailure.SourceDown -> ReadingMsg.ReadingFailedSourceDown(source)
    is ReadingFailure.NotFound -> ReadingMsg.ReadingFailedNotFound
    is ReadingFailure.Blocked -> ReadingMsg.ReadingFailedBlocked(source, reason)
    is ReadingFailure.Parse -> ReadingMsg.ReadingFailedParse(source)
    is ReadingFailure.PageFetch -> ReadingMsg.ReadingFailedPage(status.toLong())
    is ReadingFailure.SignInNeeded -> ReadingMsg.ReadingFailedSignIn(what)
    is ReadingFailure.FileMissing -> ReadingMsg.ReadingFailedFileMissing(path)
    is ReadingFailure.Unreadable -> ReadingMsg.ReadingFailedUnreadable(path, format)
    is ReadingFailure.Unsupported -> ReadingMsg.ReadingFailedUnsupported(extension)
    is ReadingFailure.Protected -> ReadingMsg.ReadingFailedProtected
    is ReadingFailure.DiskFull -> ReadingMsg.ReadingFailedDiskFull
    is ReadingFailure.WebViewTooOld -> ReadingMsg.ReadingFailedWebview(version.toLong())
    is ReadingFailure.NeedsNewerKaeri -> ReadingMsg.ReadingFailedNeedsNewer(world)
    is ReadingFailure.ReadingOff -> ReadingMsg.ReadingFailedOff
    is ReadingFailure.Other -> ReadingMsg.ReadingFailedOther(reason)
    // The send checks carry sizes, which the engine puts at human scale.
    is ReadingFailure.SendTooLarge, is ReadingFailure.SendCheck -> readingFailureMsg(this)
}

/** This failure in the person's language. A reason that came with it is said alone, without "Something went wrong" first. */
fun ReadingFailure.text(context: Context): String =
    if (this is ReadingFailure.Other && reason.isNotBlank()) reason else msg().text(context)

/** The source a failure names, for the page-failed line, or null when it names none. */
val ReadingFailure.source: String?
    get() = when (this) {
        is ReadingFailure.SourceDown -> source
        is ReadingFailure.Blocked -> source
        is ReadingFailure.Parse -> source
        else -> null
    }
