// A title's next episode that aired and that no source lists: what its page says in Play's place,
// the Check again the person asks for, and what each source's line in Source for this title says
// it has. The phone and the TV word it the same way; where each puts it is their own.

package app.kaeri.store

import android.content.Context
import java.time.Instant
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.launch
import app.kaeri.engine.core.AnilistId
import app.kaeri.engine.model.AiredAgo
import app.kaeri.engine.model.CheckOutcome
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.CoverageLine
import app.kaeri.engine.model.Entry
import app.kaeri.engine.model.EpisodeCheck
import app.kaeri.engine.model.EpisodeRow
import app.kaeri.engine.model.Lacking
import app.kaeri.engine.model.MissingCommand
import app.kaeri.engine.model.MissingEpisode
import app.kaeri.engine.model.MissingPhase
import app.kaeri.engine.model.SourceOption
import app.kaeri.engine.model.airedAgo
import app.kaeri.engine.model.coverageLine
import app.kaeri.engine.model.missingEpisode
import app.kaeri.engine.model.unsourcedYet
import app.kaeri.i18n.Strings

/** What the last check the person asked for said of a title's missing episode. */
data class AskedCheck(val episode: UInt, val outcome: CheckOutcome)

/** Check again for each title's missing episode, and what the checks came to. */
class MissingModel(private val send: (Command) -> Unit) {
    private val _asked = MutableStateFlow<Map<AnilistId, AskedCheck>>(emptyMap())

    /**
     * Per title, the last answer to a check the person asked for that changes the status's words:
     * still missing, or no answer. Offline asks nothing and changes nothing; a find ends the status.
     */
    val asked: StateFlow<Map<AnilistId, AskedCheck>> = _asked.asStateFlow()

    private val _answers = MutableSharedFlow<EpisodeCheck>(extraBufferCapacity = 8)

    /** Every answer as it comes, a quiet recheck's find among them: what an open page says once. */
    val answers: SharedFlow<EpisodeCheck> = _answers.asSharedFlow()

    /** Ask the sources again for [id]'s [episode]. A fresh episode table comes ahead of the answer when found. */
    fun checkAgain(id: AnilistId, episode: UInt) {
        send(Command.Missing(MissingCommand.CheckAgain(id, episode)))
    }

    /** What the last asked check said of [id]'s [episode], when it changes the words. */
    fun outcome(id: AnilistId, episode: UInt): CheckOutcome? =
        _asked.value[id]?.takeIf { it.episode == episode }?.outcome

    internal fun apply(check: EpisodeCheck) {
        if (check.asked && (check.outcome == CheckOutcome.MISSING || check.outcome == CheckOutcome.NO_ANSWER)) {
            _asked.value = _asked.value + (check.id to AskedCheck(check.episode, check.outcome))
        }
        _answers.tryEmit(check)
    }
}

/**
 * One Check again on screen, by the waiting rule: nothing for 600 ms after the press, then
 * Checking inside the button, held at least 0.3 s once it shows. A page makes one per title and
 * hands it the answers.
 */
class CheckWait(private val scope: CoroutineScope) {
    private val _running = MutableStateFlow(false)

    /** A check was asked for and has not been answered. */
    val running: StateFlow<Boolean> = _running.asStateFlow()

    private val _checking = MutableStateFlow(false)

    /** Whether the button shows Checking. */
    val checking: StateFlow<Boolean> = _checking.asStateFlow()

    private var show: Job? = null
    private var hold: Job? = null

    /** The press. False while a check runs already, which the press then leaves alone. */
    fun press(): Boolean {
        if (_running.value) return false
        _running.value = true
        show = scope.launch {
            delay(SHOW_AFTER_MS)
            if (!_running.value) return@launch
            _checking.value = true
            hold = scope.launch { delay(HOLD_MS) }
        }
        return true
    }

    /** The answer came. Checking, when it shows, goes once it has been up for its 0.3 s. */
    fun answered() {
        if (!_running.value) return
        _running.value = false
        show?.cancel()
        if (!_checking.value) return
        val held = hold
        scope.launch {
            held?.join()
            if (!_running.value) _checking.value = false
        }
    }

    companion object {
        /** How long a check runs before Checking shows: most answers come sooner, and show none. */
        const val SHOW_AFTER_MS = 600L

        /** How long Checking stays once it shows, so it never flickers. */
        const val HOLD_MS = 300L
    }
}

/**
 * One title page's Check again: the press, the waiting rule, and the answers the page hears. A
 * page makes one for its title and runs [listen] while it is open.
 */
class CheckAgainPage(private val model: MissingModel, private val id: AnilistId, scope: CoroutineScope) {
    /** The waiting rule of this page's Check again. */
    val wait = CheckWait(scope)

    private var foundSaid: UInt? = null

    /** The press, for [episode]. A press while a check runs does nothing. */
    fun press(episode: UInt) {
        if (wait.press()) model.checkAgain(id, episode)
    }

    /**
     * The answers for this title while the page is open: an asked check's answer ends the wait,
     * and [say] hears each answer a screen reader is told once. A find from Kaeri's own recheck is
     * said too; a second word of the same find is not.
     */
    suspend fun listen(say: (EpisodeCheck) -> Unit) {
        model.answers.collect { check ->
            if (check.id != id) return@collect
            if (check.asked) wait.answered()
            if (check.outcome == CheckOutcome.FOUND) {
                if (foundSaid == check.episode) return@collect
                foundSaid = check.episode
                say(check)
            } else if (check.asked) {
                say(check)
            }
        }
    }
}

/** Now in Unix seconds, at once and then as each minute turns: what the time since airing reads. */
fun minuteTicks(): Flow<Long> = flow {
    while (true) {
        val now = AppClock.now()
        emit(now.epochSecond)
        delay(60_000L - now.toEpochMilli().mod(60_000L))
    }
}

/** [entry]'s missing episode in [rows] at [now] (Unix seconds), or null when its page plays as it does today. */
fun Entry.missingIn(rows: List<EpisodeRow>?, now: Long): MissingEpisode? {
    if (rows == null) return null
    val next = progress?.next ?: 1u
    return missingEpisode(rows, next, resume != null, now)
}

/** The words of a missing episode's status, its row and the source picker's lines. */
object MissingWords {
    /** "7 Nov", as the status's first sentence dates an episode that aired a day or more ago. */
    fun shortDate(at: Long): String =
        DateTimeFormatter.ofPattern(AirTime.bestPattern("dMMM", "d MMM"), AirTime.locale)
            .format(Instant.ofEpochSecond(at).atZone(AppClock.zone))

    /** "7 Nov 2026", as a row dates an episode, in the platform's medium form. */
    fun rowDate(at: Long): String =
        DateTimeFormatter.ofLocalizedDate(FormatStyle.MEDIUM).withLocale(AirTime.locale)
            .format(Instant.ofEpochSecond(at).atZone(AppClock.zone))

    /** "25 min ago", "2 h ago", or null from a day on, when a date says it instead. */
    fun ago(context: Context, airedAt: Long, now: Long): String? = when (val ago = airedAgo(airedAt, now)) {
        is AiredAgo.Minutes -> Strings.timeAgo(context, Strings.timeMinutes(context, ago.minutes.toInt()))
        is AiredAgo.Hours -> Strings.timeAgo(context, Strings.timeHours(context, ago.hours.toInt()))
        else -> null
    }

    /** "Episode 6 aired 25 min ago.", or "Episode 6 aired on 7 Nov."; null when the schedule does not date it. */
    fun head(context: Context, missing: MissingEpisode, now: Long): String? {
        val at = missing.airedAt ?: return null
        if (missing.phase == MissingPhase.UNDATED) return null
        val episode = missing.episode.toInt()
        return ago(context, at, now)?.let { Strings.episodeMissingAired(context, episode, it) }
            ?: Strings.episodeMissingAiredOn(context, episode, shortDate(at))
    }

    /**
     * The sentences after the head: what the sources have, and what Kaeri does next. [outcome] is
     * what the last check the person asked for said, and [offline] the app's offline state.
     */
    fun tail(context: Context, missing: MissingEpisode, outcome: CheckOutcome?, offline: Boolean): String = when (missing.phase) {
        MissingPhase.QUIET -> {
            val first = when (outcome) {
                CheckOutcome.MISSING -> Strings.episodeMissingStill(context)
                CheckOutcome.NO_ANSWER -> Strings.episodeMissingNoAnswer(context)
                else -> Strings.episodeMissingNotYet(context)
            }
            val next = if (offline) Strings.episodeMissingOffline(context) else Strings.episodeMissingLooksAgain(context, missing.everyMinutes.toInt())
            "$first $next" // i18n-skip
        }
        MissingPhase.AFTER -> if (outcome == CheckOutcome.NO_ANSWER) Strings.episodeMissingNoAnswer(context) else Strings.episodeMissingStill(context)
        MissingPhase.LONG_AGO -> Strings.episodeMissingNone(context)
        MissingPhase.UNDATED -> Strings.episodeUnsourced(context)
    }

    /** The whole status, as a screen reader reads it in one: the head, then the tail. */
    fun status(context: Context, missing: MissingEpisode, outcome: CheckOutcome?, offline: Boolean, now: Long): String {
        val tail = tail(context, missing, outcome, offline)
        return head(context, missing, now)?.let { "$it $tail" } ?: tail // i18n-skip
    }

    /**
     * What a screen reader hears of [check], once: the find, still missing, no answer, or for a
     * press while offline the offline line.
     */
    fun said(context: Context, check: EpisodeCheck): String = when (check.outcome) {
        CheckOutcome.FOUND -> Strings.episodeFoundAnnouncement(context, check.episode.toInt())
        CheckOutcome.MISSING -> Strings.episodeMissingStill(context)
        CheckOutcome.NO_ANSWER -> Strings.episodeMissingNoAnswer(context)
        CheckOutcome.OFFLINE -> Strings.stateOfflineLine(context)
    }

    /** Check again's hint, during the quiet window only: Kaeri also looks by itself. */
    fun checkHint(context: Context, missing: MissingEpisode): String? =
        if (missing.phase == MissingPhase.QUIET) Strings.episodeCheckAgainHelp(context, missing.everyMinutes.toInt(), missing.forHours.toInt()) else null

    /**
     * The line of an aired episode no source lists: "Aired 25 min ago · Not in your sources yet"
     * within a day, "Aired 7 Nov 2026 · Not in your sources yet" for the rest of the week, then
     * "Aired 7 Nov 2026 · No source has this episode". Null for one the schedule does not date,
     * which keeps today's words.
     */
    fun unsourcedLine(context: Context, airsAt: Long?, now: Long): String? {
        val at = airsAt?.takeIf { it <= now } ?: return null
        val aired = ago(context, at, now)?.let { Strings.episodeRowAiredAgo(context, it) } ?: Strings.mediaAiredOn(context, rowDate(at))
        val have = if (unsourcedYet(at, now)) Strings.episodeUnsourcedYet(context) else Strings.episodeUnsourced(context)
        return Strings.episodeFactsJoin(context, aired, have)
    }

    /** "6", "5 or 6", "4 to 6": the episodes a source lacks, inside its line. */
    fun lacking(context: Context, lacking: Lacking): String = when (lacking) {
        is Lacking.One -> java.text.NumberFormat.getIntegerInstance(AirTime.locale).format(lacking.episode.toLong())
        is Lacking.Pair -> Strings.sourcePickerMissingPair(context, lacking.first.toInt(), lacking.second.toInt())
        is Lacking.Range -> Strings.sourcePickerMissingRange(context, lacking.first.toInt(), lacking.last.toInt())
    }

    /**
     * What a source has of the title, for its line in Source for this title: "Episodes 1 to 6",
     * "Episodes 1 to 5. Not 6 yet", "5 of 6 episodes. Not 3", or the count. Null for a source with
     * a note, whose line says the note.
     */
    fun coverage(context: Context, option: SourceOption, now: Long): String? = when (val line = coverageLine(option, now)) {
        null -> null
        is CoverageLine.Episodes -> Strings.sourcePickerEpisodes(context, line.count.toInt(), line.first.toInt(), line.last.toInt())
        is CoverageLine.NotYet -> Strings.sourcePickerEpisodesNotYet(context, line.first.toInt(), line.last.toInt(), lacking(context, line.lacking))
        is CoverageLine.Not -> Strings.sourcePickerEpisodesNot(context, line.first.toInt(), line.last.toInt(), lacking(context, line.lacking))
        is CoverageLine.Gap -> Strings.sourcePickerCoverageMissing(context, line.covered.toInt(), line.of.toInt(), lacking(context, line.lacking))
        is CoverageLine.Count -> Strings.sourcePickerCoverage(context, line.covered.toInt(), line.of.toInt())
    }
}
