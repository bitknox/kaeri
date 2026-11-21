// What the player's Episodes and Sources panel draws from besides the engine's menus: the
// episode's releases across every source, a switch to one of them as the engine says it goes, the
// picture held over the video while the switch loads, and the words its rows share between the
// phone and the TV (player panel board). After Apple's PanelState.swift.

package app.kaeri.player

import android.content.Context
import android.graphics.Bitmap
import android.text.format.DateFormat
import app.kaeri.engine.core.SourceCandidate
import app.kaeri.engine.model.EpisodeKind
import app.kaeri.engine.model.EpisodeRow
import app.kaeri.engine.model.Msg
import app.kaeri.engine.model.SourceNote
import app.kaeri.engine.model.SourceOption
import app.kaeri.engine.model.SwitchTo
import app.kaeri.i18n.Strings
import app.kaeri.store.AirTime
import app.kaeri.store.AppClock
import app.kaeri.store.plainReason
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.temporal.ChronoUnit
import java.util.Locale

/** The panel's two tabs. It opens on Episodes. */
enum class PanelTab { Episodes, Sources }

/** What a switch of the playing episode goes to, by the ids the panel's rows carry. */
data class SwitchTarget(
    /** The source, as `SourceCandidate.providerId` names it; empty for a stream of the source playing. */
    val providerId: String,
    /** The name the source goes by. */
    val name: String,
    /** The release, by its `SourceCandidate.id`; null for a stream. */
    val release: String? = null,
    /** The stream, by its `MenuSource.id`; null for a release. */
    val stream: String? = null,
) {
    constructor(to: SwitchTo) : this(to.providerId, to.name, to.release, to.stream)

    /** The same row: a stream by its id, a release by its source and id. */
    fun same(other: SwitchTarget): Boolean =
        if (stream != null) stream == other.stream else other.stream == null && release == other.release && providerId == other.providerId
}

/**
 * A switch picked in the panel, from the tap until it lands or fails. The row it was picked on
 * shows it: nothing for 600 ms, then a spinner, then "Starting · 3 s"; a failure stays on the row
 * until the next pick, with Try again.
 */
data class PanelSwitch(
    val target: SwitchTarget,
    val phase: Phase,
    /** When it was picked, on the model's clock: what the row's wait counts from. */
    val since: Long,
    /** The release a switch was picked from, for Try again. */
    val candidate: SourceCandidate? = null,
) {
    sealed interface Phase {
        /** Sent, and the engine has said nothing yet. */
        data object Asked : Phase
        /** The release is being found; the episode plays on. */
        data object Finding : Phase
        /** Found and loading at the playhead; the picture is held. */
        data object Loading : Phase
        /** It did not start, and the episode plays on from what played before: the engine's line. */
        data class Failed(val text: Msg) : Phase
    }

    /** Whether [target] is the row this switch was picked on. */
    fun on(target: SwitchTarget): Boolean = this.target.same(target)

    val failed: Msg? get() = (phase as? Phase.Failed)?.text

    /** Whether the spinner shows at [now]: past the silent 600 ms, and not failed. */
    fun spinning(now: Long): Boolean = failed == null && now - since >= SILENT

    /** The whole seconds waited at [now], once "Starting · 3 s" takes the detail line's place; else null. */
    fun counting(now: Long): Int? = if (failed == null && now - since >= COUNTING) ((now - since) / 1000).toInt() else null

    companion object {
        /** Waiting is silent this long: most switches land inside it. */
        const val SILENT = 600L
        /** "Starting · 3 s" from here. */
        const val COUNTING = 3_000L
    }
}

/** The episode's releases across every source: the engine's answer to `LoadSources`, in the order the sources gave them. */
data class PanelReleases(val id: app.kaeri.engine.core.AnilistId, val episode: String, val candidates: List<SourceCandidate>?) {
    /** One source's releases, in the order it gave them. */
    fun of(providerId: String): List<SourceCandidate> = candidates.orEmpty().filter { it.providerId == providerId }
}

/**
 * The picture on screen just before a switch loads, held over the video until the new stream's
 * first frame, then crossfaded away: mpv clears its picture on `loadfile`.
 */
class HeldPicture(val bitmap: Bitmap, val serial: Int)

/** The words the panel's rows share between the phone and the TV. */
object PanelWords {
    /** "E7", or the number as it is when it is not whole ("7.5", "OVA 2"). */
    fun number(context: Context, number: String): String =
        number.toIntOrNull()?.let { Strings.episodeNumberShort(context, it) } ?: number

    /** The row's title: its own, or "Episode 11" while it has none. */
    fun title(context: Context, row: EpisodeRow): String =
        row.title?.takeIf { it.isNotBlank() } ?: Strings.episodeNumberPlain(context, row.number)

    /** Whether the episode has not aired yet at [now], in Unix seconds. */
    fun notOut(row: EpisodeRow, now: Long = AppClock.now().epochSecond): Boolean = row.airsAt?.let { it > now } == true

    /**
     * The row's one detail line, from the table's own fields: "Playing · 11 min left" on the
     * playing row, else the kind, then airs, unsourced, watched, time left or length, and the
     * source that fills it in.
     */
    fun detail(
        context: Context,
        row: EpisodeRow,
        current: Boolean,
        metadataOnly: Boolean,
        position: Double,
        duration: Double?,
        now: Instant = AppClock.now(),
    ): String {
        if (current) {
            val length = duration ?: row.durationSecs?.toDouble() ?: 0.0
            val left = maxOf(1, Math.round((length - position) / 60).toInt())
            return Strings.episodeRowPlaying(context, Strings.timeMinutesLeft(context, left))
        }
        val parts = mutableListOf<String>()
        when (row.kind) {
            EpisodeKind.FILLER -> parts += Strings.episodeFiller(context)
            EpisodeKind.MIXED -> parts += Strings.episodeRowMixed(context)
            else -> {}
        }
        val airs = row.airsAt
        if (airs != null && airs > now.epochSecond) {
            parts += Strings.episodeRowAirs(context, airsWords(context, Instant.ofEpochSecond(airs), now))
            row.delayedFrom?.let { parts += Strings.episodeRowDelayed(context, airsWords(context, Instant.ofEpochSecond(it), now)) }
            return parts.joinToString(" · ") // i18n-skip
        }
        if (row.unsourced && !metadataOnly) {
            parts += Strings.episodeUnsourced(context)
            return parts.joinToString(" · ") // i18n-skip
        }
        val seconds = row.durationSecs?.toDouble()
        when {
            row.completed -> parts += Strings.episodeWatched(context)
            row.watched > 0 && !metadataOnly && seconds != null ->
                parts += Strings.timeMinutesLeft(context, maxOf(1, Math.round(seconds * (1 - row.watched) / 60).toInt()))
            seconds != null && seconds > 0 -> parts += Strings.episodeRowLength(context, maxOf(1, Math.round(seconds / 60).toInt()))
        }
        if (!metadataOnly) row.source?.let { parts += Strings.episodeRowFrom(context, it) }
        return parts.joinToString(" · ") // i18n-skip
    }

    /** The silk line on the still: the playhead on the playing row, the history's on a started one. */
    fun progress(row: EpisodeRow, current: Boolean, metadataOnly: Boolean, position: Double, duration: Double?): Float? {
        if (metadataOnly) return null
        if (current) {
            val length = duration ?: row.durationSecs?.toDouble() ?: return null
            return if (length > 0) (position / length).toFloat().coerceIn(0f, 1f) else null
        }
        return if (!row.completed && row.watched > 0) row.watched.toFloat().coerceIn(0f, 1f) else null
    }

    /**
     * "1080p · 1.4 GiB · lantern-archive · Dubbed · Automatic picks this", from the release's own
     * fields only. Each part keeps its words on one line.
     */
    fun release(context: Context, candidate: SourceCandidate): String {
        val parts = mutableListOf<String>()
        candidate.quality?.let { parts += "${it}p" } // i18n-skip
        candidate.size?.let { parts += it }
        candidate.label?.takeIf { it.isNotBlank() }?.let { parts += it }
        if (candidate.dubbed) parts += Strings.audioDubbed(context) else if (candidate.dualAudio) parts += Strings.playerDualAudio(context)
        if (candidate.autoPick) parts += Strings.playerReleaseAutomatic(context)
        return parts.joinToString(" · ") { it.replace(' ', ' ') } // i18n-skip
    }

    /** A source's line: why it is resting, or how much of the title it has. */
    fun source(context: Context, option: SourceOption): String = when (val note = option.note) {
        null -> Strings.sourcePickerCoverage(context, option.covered.toInt(), option.of.toInt())
        SourceNote.Off -> Strings.sourceNoteOff(context)
        SourceNote.Resting -> Strings.sourceNoteResting(context)
        SourceNote.Gone -> Strings.sourceNoteGone(context)
        SourceNote.Ambiguous -> Strings.sourceNoteAmbiguous(context)
        is SourceNote.Failed -> plainReason(context, note.reason)
    }

    /**
     * When a broadcast airs: "today at 16:30", "tomorrow at 16:30", "on Thursday at 16:30" within
     * six days, and "on 29 Oct at 16:30" later, as the title page says it.
     */
    fun airsWords(
        context: Context,
        at: Instant,
        now: Instant = AppClock.now(),
        zone: ZoneId = AppClock.zone,
        twentyFourHour: Boolean = DateFormat.is24HourFormat(context),
        locale: Locale = Locale.getDefault(),
    ): String {
        val time = DateTimeFormatter.ofPattern(AirTime.bestPattern(if (twentyFourHour) "Hm" else "hm", if (twentyFourHour) "HH:mm" else "h:mm a"), locale).format(at.atZone(zone))
        val day = at.atZone(zone).toLocalDate()
        val days = ChronoUnit.DAYS.between(now.atZone(zone).toLocalDate(), day)
        return when {
            days == 0L -> Strings.causeAirsToday(context, time)
            days == 1L -> Strings.causeAirsTomorrow(context, time)
            days in 2..6 -> Strings.causeAirsOnDate(context, day.dayOfWeek.getDisplayName(java.time.format.TextStyle.FULL, locale), time)
            else -> Strings.causeAirsOnDate(context, DateTimeFormatter.ofPattern(AirTime.bestPattern("dMMM", "d MMM"), locale).format(day), time)
        }
    }
}
