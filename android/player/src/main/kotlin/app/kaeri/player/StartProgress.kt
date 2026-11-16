// What a start is doing, as the starting screen tells it: the engine's steps kept in the order they
// came, and what the step line, the detail line, the buttons and the Details list say of them at a
// given moment. A pure value, so the same words stand on a phone and a TV, and a test can ask what
// a start says at 10 s.

package app.kaeri.player

import android.content.Context
import android.icu.text.MeasureFormat
import android.icu.text.NumberFormat
import android.icu.util.Measure
import android.icu.util.MeasureUnit
import app.kaeri.engine.core.AnilistId
import app.kaeri.engine.core.Translation
import app.kaeri.engine.model.Msg
import app.kaeri.engine.model.SourceHold
import app.kaeri.engine.model.SourceMiss
import app.kaeri.engine.model.StartStep
import app.kaeri.i18n.Strings
import app.kaeri.store.text
import kotlin.math.max

/** The moments the starting screen keeps, in milliseconds since the play was staged. */
object StartTiming {
    /** The shortest a step line stays on screen. */
    const val HOLD = 600L

    /** The detail line, and Play from the beginning, from here. */
    const val DETAIL = 1_000L

    /** Choose source and Details join the row, and the seconds the detail line. */
    const val TOOLS = 3_000L

    /** Skip joins the row once one source or stream has been tried this long, with another to try. */
    const val SKIP = 8_000L

    /** The step line says it is still waiting; a screen reader says so once. */
    const val SLOW = 20_000L
}

/** One step the engine told, and when. */
data class Told(val at: Long, val step: StartStep)

/** A step line the engine told, and when. */
data class ToldLine(val at: Long, val msg: Msg)

/** What a Details row stands for. */
enum class RowKind { Done, Empty, Now, Rest }

/** A row of the Details list. [seconds] is how long it took, or has taken so far, when a second or more. */
data class DetailRow(val kind: RowKind, val text: String, val seconds: Int?, val skip: String? = null)

/** The detail line in two parts: the facts, which crossfade when they change, and the seconds, which only count. */
data class DetailText(val facts: String, val seconds: String?) {
    val full: String get() = listOfNotNull(facts.takeIf { it.isNotEmpty() }, seconds).joinToString(" \u00b7 ")
}

/** A button of the row under the lines, in the order they arrive. */
enum class StartButton { Cancel, FromBeginning, ChooseSource, Details, Skip, ShowDownloads }

/**
 * The start of [id]'s [episode], staged at [since]. [resume] is where the episode was left, in
 * seconds, until the viewer begins it again ([beginning]).
 */
data class StartProgress(
    val id: AnilistId,
    val episode: String,
    val since: Long,
    val resume: Double? = null,
    val beginning: Boolean = false,
    val lines: List<ToldLine> = emptyList(),
    val trail: List<Told> = emptyList(),
    val percent: Int? = null,
) {
    /** This step, heard at [now]. */
    fun heard(step: StartStep, line: Msg?, now: Long): StartProgress {
        val lines = if (line != null && lines.lastOrNull()?.msg != line) lines + ToldLine(now, line) else lines
        val percent = when (step) {
            is StartStep.Buffering -> step.percent.toInt().coerceIn(0, 100)
            else -> percent
        }
        return copy(lines = lines, trail = trail + Told(now, step), percent = percent)
    }

    /** The viewer asked to begin at 0: the line says so, and the button goes. */
    fun fromTheBeginning(): StartProgress = copy(beginning = true)

    /** How long this start has waited at [now]. */
    fun waited(now: Long): Long = max(0L, now - since)

    /**
     * The step line the engine told that is on screen at [now]: each stays at least
     * [StartTiming.HOLD], and a newer one that came during the hold replaces any waiting, so only
     * the latest shows after it.
     */
    fun heldLine(now: Long): Msg? {
        if (lines.isEmpty()) return null
        var shown = 0
        var since = lines[0].at
        while (shown + 1 < lines.size) {
            val at = max(since + StartTiming.HOLD, lines[shown + 1].at)
            if (at > now) break
            var pick = shown + 1
            while (pick + 1 < lines.size && lines[pick + 1].at <= at) pick++
            shown = pick
            since = at
        }
        return lines[shown].msg
    }

    // What the steps added up to

    private val asking: Told? get() = trail.lastOrNull { it.step is StartStep.AskingSource }
    private val found: Told? get() = trail.lastOrNull { it.step is StartStep.Found }
    private val opening: Told? get() = trail.lastOrNull { it.step is StartStep.OpeningStream || it.step is StartStep.TryingAnotherStream }
    private val pinMissed: String? get() = (trail.lastOrNull { it.step is StartStep.PinMissed }?.step as? StartStep.PinMissed)?.name
    private val peers: Told? get() = trail.lastOrNull { it.step is StartStep.FindingPeers }
    private val swarm: StartStep.Swarm? get() = trail.lastOrNull { it.step is StartStep.Swarm }?.step as? StartStep.Swarm
    private val speed: ULong? get() = (trail.lastOrNull { it.step is StartStep.Speed }?.step as? StartStep.Speed)?.bytesPerSecond
    private val another: StartStep.TryingAnotherStream? get() = trail.lastOrNull { it.step is StartStep.TryingAnotherStream }?.step as? StartStep.TryingAnotherStream

    /** The source being asked, or the one the stream came from. */
    fun sourceName(): String? {
        (found?.step as? StartStep.Found)?.source?.let { return it }
        return (asking?.step as? StartStep.AskingSource)?.name
    }

    /** The translation the streams were found in, once they are. */
    fun translation(): Translation? = (found?.step as? StartStep.Found)?.translation

    /** Whether the engine is holding the start until the device has a connection. */
    val waiting: Boolean get() = trail.lastOrNull()?.step is StartStep.WaitingForConnection

    /** Whether the streams have been found and the player is opening one. */
    val opened: Boolean get() = found != null

    /** The miss of the source asked before the one being asked now, if it came since. */
    private fun miss(): StartStep.SourceMissed? =
        if (found != null) null else trail.lastOrNull { it.step is StartStep.SourceMissed }?.step as? StartStep.SourceMissed

    // Words

    /** The step line at [now]: the held line, with a percent when mpv gives one, or "Still waiting" past 20 s. */
    fun stepLine(context: Context, now: Long): String {
        val held = heldLine(now)
        val text = held?.text(context) ?: Strings.playerStartingText(context, episode)
        if (held == Msg.StartBuffering && percent != null) return Strings.playerStartBufferingPercent(context, percentText(percent))
        val name = sourceName()
        if (waited(now) >= StartTiming.SLOW && name != null && held != null) return Strings.playerStartStillWaiting(context, name)
        return text
    }

    /** The detail line at [now]: facts joined by a middle dot, then the seconds from 3 s. Empty before 1 s. */
    fun detailLine(context: Context, now: Long): DetailText {
        val waited = waited(now)
        if (waited < StartTiming.DETAIL) return DetailText("", null)
        val seconds = if (waited >= StartTiming.TOOLS) Strings.playerStartSeconds(context, (waited / 1000).toInt()) else null
        return DetailText(facts(context).joinToString(" \u00b7 "), seconds)
    }

    /** The detail line's facts, without the seconds. */
    private fun facts(context: Context): List<String> {
        val parts = mutableListOf<String>()
        // Where it begins leads, for the whole start: the position it resumes at, or the beginning.
        if (beginning) parts += Strings.playerStartBeginning(context)
        else if (resume != null) parts += Strings.playerStartResuming(context, clock(resume))
        val found = found?.step as? StartStep.Found
        if (found != null) {
            pinMissed?.let { parts += Strings.playerStartPinMissed(context, it, episode) }
            val swarm = swarm
            another?.let {
                parts += Strings.playerStartNoVideo(context)
                parts += Strings.playerStartStreamPlace(context, it.next.toInt(), it.total.toInt())
            }
            if (swarm != null) {
                parts += Strings.playerStartPeers(context, swarm.peers.toInt())
                if (swarm.bytesPerSecond > 0uL) parts += Strings.playerStartSpeed(context, speedText(context, swarm.bytesPerSecond))
            } else {
                if (another == null) found.source?.let { parts += Strings.playerStartFrom(context, it) }
                speed?.takeIf { it > 0uL }?.let { parts += Strings.playerStartSpeed(context, speedText(context, it)) }
            }
            return parts
        }
        val waitingOnPeers = peers
        if (waitingOnPeers != null && waitingOnPeers.at >= (asking?.at ?: 0L)) {
            (waitingOnPeers.step as StartStep.FindingPeers).name?.let { parts += Strings.playerStartFrom(context, it) }
            return parts
        }
        val ask = asking?.step as? StartStep.AskingSource ?: return parts
        if (ask.pinned) parts += Strings.playerStartPinned(context)
        miss()?.let { parts += missText(context, it) }
        if (ask.total > 1u) parts += Strings.playerStartPlace(context, ask.place.toInt(), ask.total.toInt())
        return parts
    }

    private fun missText(context: Context, missed: StartStep.SourceMissed): String = when (missed.miss) {
        SourceMiss.NO_MATCH -> Strings.playerStartMissNoMatch(context, missed.name)
        SourceMiss.NO_EPISODE -> Strings.playerStartMissNoEpisode(context, missed.name, episode)
        SourceMiss.FAILED -> Strings.playerStartMissFailed(context, missed.name)
        SourceMiss.TIMED_OUT -> Strings.playerStartMissTimedOut(context, missed.name)
        SourceMiss.SKIPPED -> Strings.playerStartMissSkipped(context, missed.name)
    }

    /** Whether the start is resuming, which Play from the beginning answers: until its first frame. */
    fun resuming(now: Long): Boolean = resume != null && !beginning && waited(now) >= StartTiming.DETAIL

    /** The percent buffering shows, only when mpv gave one. */
    fun determinate(now: Long): Int? = percent?.takeIf { heldLine(now) == Msg.StartBuffering }

    // Buttons

    /** Which of the buttons apply at [now], in the order they arrived. [offline]: the start waits for a connection. */
    fun buttons(now: Long, offline: Boolean): List<StartButton> {
        if (offline) return listOf(StartButton.Cancel, StartButton.ShowDownloads)
        val waited = waited(now)
        return buildList {
            add(StartButton.Cancel)
            if (resuming(now)) add(StartButton.FromBeginning)
            if (waited >= StartTiming.TOOLS) {
                add(StartButton.ChooseSource)
                add(StartButton.Details)
            }
            if (!canSkip) return@buildList
            if (now - rungAt() >= StartTiming.SKIP) add(StartButton.Skip)
        }
    }

    /** When the source or stream being tried was started on. */
    private fun rungAt(): Long = opening?.at ?: found?.at ?: asking?.at ?: since

    /** Whether there is another source or stream to move on to. */
    private val canSkip: Boolean
        get() {
            val ask = asking?.step as? StartStep.AskingSource
            val found = found
            return when {
                found != null -> (opening?.step as? StartStep.OpeningStream)?.let { it.attempt < it.total }
                    ?: (opening?.step as? StartStep.TryingAnotherStream)?.let { it.next < it.total }
                ask != null -> ask.place < ask.total
                else -> null
            } == true
        }

    /** The Skip button's label: the source's name, or "Skip this stream" once the streams are found. */
    fun skipText(context: Context): String =
        if (found != null) Strings.playerStartSkipStream(context)
        else Strings.playerStartSkipSource(context, sourceName() ?: "")

    // Details

    /** The list under the row at [now]: each source asked and what it said, then the stream. */
    fun details(context: Context, now: Long): List<DetailRow> {
        val rows = mutableListOf<DetailRow>()
        val total = trail
        fun seconds(from: Long, to: Long): Int = ((to - from) / 1000).toInt().coerceAtLeast(0)
        val first = total.firstOrNull { it.step !is StartStep.LookingUpTitle }
        total.firstOrNull { it.step is StartStep.LookingUpTitle }?.let { start ->
            rows += if (first != null) DetailRow(RowKind.Done, Strings.playerStartLookedUp(context), seconds(start.at, first.at))
            else DetailRow(RowKind.Now, Strings.startLookingUpTitle(context), seconds(start.at, now))
        }
        // Held back by a setting or a check, so never asked: after the asked ones, hollow, with no seconds.
        val held = mutableListOf<DetailRow>()
        total.forEachIndexed { index, told ->
            val next = total.drop(index + 1).firstOrNull { it.step !is StartStep.SourcesHeldBack }?.at ?: now
            when (val step = told.step) {
                is StartStep.SourcesHeldBack -> for (name in step.names) held += DetailRow(RowKind.Rest, "$name · ${heldNote(context, step.why)}", null)
                is StartStep.AskingSource -> {
                    val missed = total.drop(index + 1).firstOrNull { it.step is StartStep.SourceMissed && (it.step as StartStep.SourceMissed).name == step.name }
                    val gone = total.drop(index + 1).firstOrNull { it.step is StartStep.Found }
                    when {
                        missed != null -> rows += DetailRow(RowKind.Empty, missText(context, missed.step as StartStep.SourceMissed), seconds(told.at, missed.at))
                        gone != null -> rows += DetailRow(RowKind.Done, step.name, seconds(told.at, gone.at))
                        index == total.indexOfLast { it.step is StartStep.AskingSource } && found == null ->
                            rows += DetailRow(RowKind.Now, Strings.startAskingSource(context, step.name), seconds(told.at, now), skip = if (canSkip) Strings.playerStartSkipSource(context, step.name) else null)
                    }
                }
                is StartStep.Found -> rows += DetailRow(RowKind.Done, Strings.playerStartFrom(context, step.source ?: ""), seconds(told.at, next))
                is StartStep.TryingAnotherStream -> rows += DetailRow(RowKind.Empty, Strings.playerStartNoVideo(context), null)
                is StartStep.OpeningStream -> if (index == total.indexOfLast { it.step is StartStep.OpeningStream || it.step is StartStep.TryingAnotherStream }) {
                    rows += DetailRow(RowKind.Now, Strings.playerStartStreamPlace(context, step.attempt.toInt(), step.total.toInt()), seconds(told.at, now), skip = if (canSkip) Strings.playerStartSkipStream(context) else null)
                }
                is StartStep.FindingPeers -> if (index == total.indexOfLast { it.step is StartStep.FindingPeers } && found == null) {
                    rows += DetailRow(RowKind.Now, Strings.startFindingPeers(context), seconds(told.at, now))
                }
                else -> {}
            }
        }
        return rows + held
    }

    private companion object {
        fun percentText(percent: Int): String = java.text.NumberFormat.getPercentInstance().format(percent / 100.0)

        /** A rate as "3.4 MB": one decimal, the unit the number reads best in. */
        fun speedText(context: Context, bytes: ULong): String {
            val locale = context.resources.configuration.locales[0]
            val (value, unit) = when {
                bytes >= 1_000_000_000uL -> bytes.toDouble() / 1e9 to MeasureUnit.GIGABYTE
                bytes >= 1_000_000uL -> bytes.toDouble() / 1e6 to MeasureUnit.MEGABYTE
                bytes >= 1_000uL -> bytes.toDouble() / 1e3 to MeasureUnit.KILOBYTE
                else -> bytes.toDouble() to MeasureUnit.BYTE
            }
            val digits = if (unit == MeasureUnit.BYTE) 0 else 1
            val number = NumberFormat.getInstance(locale).apply { minimumFractionDigits = digits; maximumFractionDigits = digits }
            return MeasureFormat.getInstance(locale, MeasureFormat.FormatWidth.SHORT, number).format(Measure(value, unit))
        }

        /** 606 seconds as "10:06"; over an hour as "1:02:03". */
        fun clock(seconds: Double): String {
            val total = seconds.toLong().coerceAtLeast(0)
            val h = total / 3600
            val m = total % 3600 / 60
            val s = total % 60
            return if (h > 0) "%d:%02d:%02d".format(h, m, s) else "%d:%02d".format(m, s)
        }
    }
}

/** What a button of the starting screen does. */
enum class StartAction { Cancel, FromBeginning, Choose, Details, Skip, Downloads, Close, TryAgain, FixMatch, AddStore }

/**
 * The buttons the starting screen offers at [now], in the order they arrive: while starting, the
 * row by time and state; after a failure, by what failed, the primary first. [canFix] is whether
 * Fix match has a source to match on; [hasDetails] whether there is a list to open.
 */
fun startActions(progress: StartProgress?, failure: PlaybackFailure?, offline: Boolean, now: Long, canFix: Boolean, hasDetails: Boolean): List<StartAction> {
    if (failure != null) {
        return when {
            failure.notAired -> listOf(StartAction.Close)
            failure.noSources -> listOf(StartAction.AddStore, StartAction.Close)
            failure.noConnection -> listOf(StartAction.TryAgain, StartAction.Close)
            // A source failed or was held back: asking again may reach it, so that leads.
            failure.unreached.isNotEmpty() || failure.heldBack.isNotEmpty() ->
                listOf(StartAction.TryAgain, StartAction.Choose, StartAction.Details, StartAction.Close)
            failure.noMatch && failure.id != null && canFix ->
                listOf(StartAction.FixMatch, StartAction.Choose, StartAction.Details, StartAction.Close)
            failure.cause is app.kaeri.engine.model.Cause.SourcesTimedOut ->
                listOf(StartAction.TryAgain, StartAction.Choose, StartAction.Details, StartAction.Close)
            else -> listOf(StartAction.Choose, StartAction.TryAgain, StartAction.Details, StartAction.Close)
        }.filter { it != StartAction.Details || hasDetails }
    }
    return (progress?.buttons(now, offline) ?: listOf(StartButton.Cancel)).map {
        when (it) {
            StartButton.Cancel -> StartAction.Cancel
            StartButton.FromBeginning -> StartAction.FromBeginning
            StartButton.ChooseSource -> StartAction.Choose
            StartButton.Details -> StartAction.Details
            StartButton.Skip -> StartAction.Skip
            StartButton.ShowDownloads -> StartAction.Downloads
        }
    }
}

/** The primary of a failure screen: the first action that is not Details or Close. */
fun primaryAction(actions: List<StartAction>): StartAction? =
    actions.firstOrNull { it != StartAction.Details && it != StartAction.Close && it != StartAction.Cancel }

/** An action's label. */
fun StartAction.label(context: Context, progress: StartProgress?, failure: PlaybackFailure?): String = when (this) {
    StartAction.Cancel -> Strings.actionCancel(context)
    StartAction.FromBeginning -> Strings.playerStartFromBeginning(context)
    StartAction.Choose -> if (failure != null) Strings.playerChooseAnotherSource(context) else Strings.playerChooseSource(context)
    StartAction.Details -> Strings.playerStartDetails(context)
    StartAction.Skip -> progress?.skipText(context) ?: ""
    StartAction.Downloads -> Strings.stateOfflineShowDownloads(context)
    StartAction.Close -> Strings.actionClose(context)
    StartAction.TryAgain -> Strings.actionTryAgain(context)
    StartAction.FixMatch -> Strings.playerStartFixMatch(context)
    StartAction.AddStore -> Strings.playerAddPluginStore(context)
}

/** What the step and detail lines say now. */
data class Said(val step: String, val facts: String, val seconds: String?, val failed: Boolean)

/** Whether the start waits for a connection: the engine said so, or the device is offline before any step. */
fun PlayerModel.startWaiting(progress: StartProgress? = this.progress): Boolean =
    progress?.waiting == true || (offline && progress?.trail.isNullOrEmpty())

/** Why one source listed no release for [episode]: its name once, as it goes by. */
/**
 * The line over the reasons when no source lists the episode: that none lists it only when every
 * source answered, and that none that answered lists it when any failed, took too long or wasn't asked.
 */
fun noneListsText(context: Context, missed: List<app.kaeri.engine.model.MissedListing>, episode: String): String =
    if (missed.any { it.miss !is app.kaeri.engine.model.ListingMiss.NoMatch && it.miss !is app.kaeri.engine.model.ListingMiss.NoEpisode }) {
        Strings.sourcePickerNoneAnsweredLists(context, episode)
    } else {
        Strings.sourcePickerNoneLists(context, episode)
    }

fun missedText(context: Context, missed: app.kaeri.engine.model.MissedListing, episode: String): String {
    val name = missed.name
    return when (val miss = missed.miss) {
        is app.kaeri.engine.model.ListingMiss.NoMatch -> Strings.playerStartMissNoMatch(context, name)
        is app.kaeri.engine.model.ListingMiss.NoEpisode -> Strings.playerStartMissNoEpisode(context, name, episode)
        is app.kaeri.engine.model.ListingMiss.Failed -> Strings.playerStartMissFailed(context, name)
        is app.kaeri.engine.model.ListingMiss.TimedOut -> Strings.playerStartMissTimedOut(context, name)
        is app.kaeri.engine.model.ListingMiss.HeldBack -> when (miss.why) {
            SourceHold.TORRENTS_OFF -> Strings.sourcePickerHeldTorrentsOff(context, name)
            SourceHold.PROXY_CHECK -> Strings.sourcePickerHeldProxy(context, name)
            SourceHold.CELLULAR -> Strings.sourcePickerHeldCellular(context, name)
            SourceHold.OTHER -> Strings.sourcePickerHeldOther(context, name)
        }
    }
}

/** A held-back source's row note: not asked, and why. */
fun heldNote(context: Context, why: SourceHold): String = when (why) {
    SourceHold.TORRENTS_OFF -> Strings.playerStartRowHeldTorrentsOff(context)
    SourceHold.PROXY_CHECK -> Strings.playerStartRowHeldProxy(context)
    SourceHold.CELLULAR -> Strings.playerStartRowHeldCellular(context)
    SourceHold.OTHER -> Strings.playerStartRowHeldOther(context)
}

/**
 * A failure's second line. When every source asked lacks the episode: why, and who was asked.
 * Otherwise, source by source as the rows say it: the ones asked that couldn't be reached, the ones
 * asked that don't have the episode, and the ones held back and why. A source held back was never
 * asked, so it is never one the start couldn't reach.
 */
internal fun failureDetail(context: Context, failure: PlaybackFailure): String {
    val list = android.icu.text.ListFormatter.getInstance(context.resources.configuration.locales[0])
    val held = failure.heldBack.any { it.names.isNotEmpty() }
    if (failure.noSources || (failure.unreached.isEmpty() && !held)) {
        val reason = when {
            failure.noSources -> Strings.playerFailedNoSources(context)
            else -> failure.reasonText(context)?.let { "$it." } ?: Strings.playerFailedDetail(context)
        }
        val line = mutableListOf(reason)
        if (failure.asked.isNotEmpty()) line += Strings.playerStartAsked(context, failure.asked.size, list.format(failure.asked))
        return line.joinToString(" ")
    }
    val line = mutableListOf<String>()
    if (failure.unreached.isNotEmpty()) line += Strings.playerStartUnreached(context, list.format(failure.unreached))
    val lacking = failure.asked.filter { it !in failure.unreached }
    if (lacking.isNotEmpty()) line += Strings.playerStartLacking(context, lacking.size, list.format(lacking), failure.episode)
    for (group in failure.heldBack) {
        if (group.names.isEmpty()) continue
        val names = list.format(group.names)
        val count = group.names.size
        line += when (group.why) {
            SourceHold.TORRENTS_OFF -> Strings.playerStartNotAskedTorrentsOff(context, count, names)
            SourceHold.PROXY_CHECK -> Strings.playerStartNotAskedProxy(context, count, names)
            SourceHold.CELLULAR -> Strings.playerStartNotAskedCellular(context, count, names)
            SourceHold.OTHER -> Strings.playerStartNotAskedOther(context, count, names)
        }
    }
    return line.joinToString(" ")
}

/** The lines the starting screen shows at [now]: the failure, the wait for a connection, or the start's own. */
fun startingWords(context: Context, model: PlayerModel, progress: StartProgress?, failure: PlaybackFailure?, now: Long): Said {
    if (failure != null) return Said(Strings.playerFailedTitleText(context, failure.episode), failureDetail(context, failure), null, true)
    if (model.startWaiting(progress)) {
        return Said(Strings.playerStartOffline(context), Strings.playerStartOfflineDetail(context, model.startingEpisode ?: ""), null, false)
    }
    val p = progress ?: return Said(Strings.playerStartingText(context, model.startingEpisode ?: ""), "", null, false)
    val detail = p.detailLine(context, now)
    return Said(p.stepLine(context, now), detail.facts, detail.seconds, false)
}

/** What a screen reader hears of the start: it opening, a slow start once, the wait for a connection, the outcome. */
fun startingAnnouncement(context: Context, model: PlayerModel, progress: StartProgress?, failure: PlaybackFailure?, now: Long): String = when {
    failure != null -> Strings.playerStartFailedAnnounce(context, Strings.playerFailedTitleText(context, failure.episode), failureDetail(context, failure))
    model.startWaiting(progress) -> Strings.playerStartOfflineDetail(context, model.startingEpisode ?: "")
    progress != null && progress.waited(now) >= StartTiming.SKIP -> Strings.playerStartStillStarting(context)
    else -> Strings.playerStartAnnounce(context, model.startingEpisode ?: "", model.showTitle)
}
