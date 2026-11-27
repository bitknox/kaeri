// The room's words, as the approved board writes them (design/room board v5), from the room's
// catalogue, and how each face reads.

package app.kaeri.store.room

import android.content.Context
import app.kaeri.engine.model.MemberState
import app.kaeri.engine.model.PausedFor
import app.kaeri.engine.model.Reaction
import app.kaeri.engine.model.RoomChatBody
import app.kaeri.engine.model.RoomChatLine
import app.kaeri.engine.model.RoomConnection
import app.kaeri.engine.model.RoomMember
import app.kaeri.engine.model.RoomView
import app.kaeri.engine.model.WaitReason
import app.kaeri.i18n.Strings

/** Carry on comes forward when the room has waited this long. */
const val CARRY_ON_AFTER_S = 20

/** Rooms work best with this many people or fewer: past it, Join asks first. */
const val ROOM_SOFT_CAP = 10

/** A line of the room: the bold words and the haze words under them. */
data class RoomLine(val title: String, val detail: String)

/** The room's words from the catalogue (i18n/en/ui-room.ftl), read through [context]. */
class RoomCopy(private val context: Context) {
    private val x get() = context
    private fun ep(e: String?) = e ?: ""
    val WATCH_TOGETHER get() = Strings.roomLineWatching(x)
    val NOT_STARTED get() = Strings.roomLineNotStarted(x)
    val PLAY_STARTS_IT get() = Strings.roomLinePlayStarts(x)
    val EVERYONE_READY get() = Strings.roomLineEveryoneReady(x)
    val RECONNECTING_TITLE get() = Strings.roomLineReconnecting(x)
    val RECONNECTING_DETAIL get() = Strings.roomLineReconnectingDetail(x)
    val PAUSED_FOR_YOU get() = Strings.roomLinePausedForYou(x)
    val YOU_PAUSED get() = Strings.roomLineYouPaused(x)
    val YOU get() = Strings.roomYou(x)
    val MORE get() = Strings.roomMore(x)
    val MORE_ACTIONS get() = Strings.roomMoreActions(x)
    val ALL_WATCHING get() = Strings.roomMoreAllWatching(x)
    val PLAY_FOR_EVERYONE get() = Strings.roomPlayForEveryone(x)
    val WAITING_FIRST_FRAMES get() = Strings.roomWaitingFirstFramesShort(x)
    val DONT_WAIT get() = Strings.roomDontWait(x)
    val CARRY_ON_THEM get() = Strings.roomCarryOnWithoutThem(x)
    val START_THEM get() = Strings.roomStartWithoutThem(x)
    val JOIN get() = Strings.roomJoin(x)
    val JOIN_ANYWAY get() = Strings.roomJoinAnyway(x)
    val NOT_NOW get() = Strings.actionNotNow(x)
    val CHAT_OPENS get() = Strings.roomChatOpens(x)
    val SAY_SOMETHING get() = Strings.roomChatField(x)
    val SEND get() = Strings.roomChatSend(x)
    val REACT get() = Strings.roomReact(x)
    val INVITE get() = Strings.roomInvite(x)
    val INVITE_BODY get() = Strings.roomInviteAnyone(x) + " " + Strings.roomInviteKey(x)
    val INVITE_SCAN get() = Strings.roomInviteQrLine(x)
    val COPY_LINK get() = Strings.roomCopyLink(x)
    val SHARE get() = Strings.roomShare(x)
    val NEW_INVITE get() = Strings.roomNewInvite(x)
    val NEW_INVITE_NOTE get() = Strings.roomNewInviteShort(x)
    val NEW_INVITE_BODY get() = Strings.roomNewInviteLine(x)
    val DETAILS get() = Strings.roomDetails(x)
    val ENCRYPTION get() = Strings.roomDetailsEncryption(x)
    val END_TO_END get() = Strings.roomDetailsEndToEnd(x)
    val RELAYS get() = Strings.roomRelays(x)
    val NOT_ANSWERING get() = Strings.roomRelayNotAnswering(x)
    val RELAYS_ABOUT get() = Strings.roomDetailsRelaysLine(x)
    val EDIT_RELAYS_MORE get() = Strings.roomEditRelays(x)
    val EDIT_RELAYS get() = Strings.roomEditRelaysTitle(x)
    val EDIT_RELAYS_BODY get() = Strings.roomEditRelaysLine(x)
    val RELAY_FIELD = "wss://" // i18n-skip: a scheme, not words
    val ADD get() = Strings.roomRelayAdd(x)
    val RELAYS_SETTINGS get() = Strings.roomEditRelaysSettings(x)
    val DONE get() = Strings.actionDone(x)
    val MOVE_MORE get() = Strings.roomMoveOpen(x)
    val MOVE get() = Strings.roomMoveTitle(x)
    val MOVE_BODY get() = Strings.roomMoveBody(x)
    val ALWAYS_GOES get() = Strings.roomMoveAlwaysGoes(x)
    val MUTED_UNTICKED get() = Strings.roomMoveMuted(x)
    val GO_ALONG get() = Strings.roomGoAlong(x)
    val STAY_HERE get() = Strings.roomStay(x)
    val CANCEL get() = Strings.actionCancel(x)
    val LEAVE get() = Strings.actionLeave(x)
    val LEAVE_MORE get() = Strings.roomLeaveMenu(x)
    val LEAVE_TITLE get() = Strings.roomLeaveTitle(x)
    val LEAVE_BODY get() = Strings.roomLeaveLine(x)
    val KEEP_WATCHING get() = Strings.roomKeepWatching(x)
    val COPY_INVITE get() = Strings.roomCopyInviteLink(x)
    val SUBTITLES_AUDIO get() = Strings.playerSubtitlesAndAudio(x)
    val EPISODES get() = Strings.playerEpisodes(x)
    val CLOSE_TO_BAR get() = Strings.roomFold(x)
    val PLAY_IN_ROOM get() = Strings.roomPlayInRoom(x)
    val CHOOSE_BODY get() = Strings.roomChooseSourceLine(x)
    val OPEN_SOURCES get() = Strings.roomOpenSources(x)
    val SEARCH_TITLES get() = Strings.roomSearchTitles(x)
    val STAY_IN_ROOM get() = Strings.roomNoSourceLineShort(x)
    val WRITE_ON_PHONE get() = Strings.roomTvWriteFromPhone(x)
    val TV_HINT get() = Strings.roomTvKeys(x)
    val UNREACHABLE_TITLE get() = Strings.roomErrorUnreachableTitle(x)
    val UNREACHABLE_BODY get() = Strings.roomErrorAskNewInvite(x)
    val QUIET_TITLE get() = Strings.roomErrorNobodyAnswered(x)
    val QUIET_BODY get() = Strings.roomErrorNobodyAnsweredLine(x)
    val FULL get() = Strings.roomErrorFull(x)
    val OLD_INVITE get() = Strings.roomErrorOldInvite(x)
    val TRY_AGAIN get() = Strings.actionTryAgain(x)
    val CLOSE get() = Strings.actionClose(x)
    val UP_NEXT get() = Strings.roomUpNext(x)
    val PLAY_NEXT_NOW get() = Strings.roomPlayNextNow(x)
    val UP_IN_ROOM get() = Strings.roomUpInRoom(x)
    val ROOM_IS_WATCHING get() = Strings.roomRoomIsWatching(x)

    /** "Ren", "Ren and Mika", "Ada, Ren and Mika". */
    fun list(n: List<String>): String = when (n.size) {
        0 -> ""
        1 -> n[0]
        else -> Strings.roomNamesAnd(x, n.dropLast(1).reduce { a, b -> Strings.roomNamesComma(x, a, b) }, n.last())
    }
    /** Up to two names, then "3 people". */
    fun names(n: List<String>): String = if (n.size <= 2) list(n) else Strings.roomNamesPeople(x, n.size)
    fun episodeAt(episode: String, at: String) = Strings.roomLineWatchingDetail(x, ep(episode), at)
    fun inTheRoom(episode: String, n: Int) = Strings.roomHeaderLine(x, ep(episode), n)
    fun pausedBy(name: String) = Strings.roomLinePausedBy(x, name)
    fun at(time: String) = Strings.roomLinePausedAt(x, time)
    fun pausedFor(names: String) = Strings.roomLinePausedFor(x, names)
    fun stillLoading(names: String, count: Int) = Strings.roomLineStillLoading(x, names, count)
    fun stillSeeking(names: String, count: Int) = Strings.roomLineStillSeeking(x, names, count)
    fun stillBuffering(names: String, count: Int) = Strings.roomLineStillBuffering(x, names, count)

    /** "Ren is still loading", or seeking or buffering when that is why everyone waited for is. */
    fun still(waits: List<app.kaeri.engine.model.PausedFor>): String {
        val who = names(waits.map { it.name })
        return when {
            waits.all { it.reason == WaitReason.SEEKING } -> stillSeeking(who, waits.size)
            waits.all { it.reason == WaitReason.BUFFERING } -> stillBuffering(who, waits.size)
            else -> stillLoading(who, waits.size)
        }
    }
    fun startingWhen(names: String, count: Int) = Strings.roomLineStartingWhen(x, names, count)
    fun startingIn(n: Int) = Strings.roomLineStartingIn(x, n)
    fun waited(s: Int) = Strings.roomLineWaited(x, s)
    fun reasonFor(reason: WaitReason, s: Int) = when (reason) {
        WaitReason.BUFFERING -> Strings.roomStateBuffering(x, s)
        WaitReason.SEEKING -> Strings.roomStateSeeking(x, s)
        WaitReason.LOADING -> Strings.roomStateLoading(x, s)
    }
    val PEOPLE get() = Strings.roomPeople(x)

    /** A recent room's line: "Episode 8 · with Mika and Ren · Tuesday", or without the names. */
    fun recentLine(room: app.kaeri.engine.model.RoomRecent): String {
        val at = RoomWords.lastIn(room.lastAt)
        return if (room.names.isEmpty()) Strings.roomRecentRowAlone(x, room.episode, at) else Strings.roomRecentRow(x, room.episode, list(room.names), at)
    }
    fun othersWaiting(names: String, count: Int, s: Int) = Strings.roomLineWaitingForYou(x, names, count, s)
    fun carriedOn(by: String, past: List<String>) = if (past.size == 1) Strings.roomLineCarriedOnOne(x, by, past[0]) else Strings.roomLineCarriedOnMany(x, by)
    fun behindYou(s: Int) = Strings.roomLineYouBehind(x, s)
    fun relaysDown(down: Int, of: Int) = Strings.roomLineRelaysDown(x, down, of)
    fun upNextIn(s: Int) = Strings.roomLineUpNext(x, s)
    fun upNextFor(episode: String) = Strings.roomLineUpNextDetail(x, ep(episode))
    fun upNextCount(s: Int) = Strings.roomUpNextCount(x, s)
    fun upNextEpisode(episode: String, title: String?) = if (title.isNullOrEmpty()) Strings.roomUpNextEpisodePlain(x, ep(episode)) else Strings.roomUpNextEpisode(x, ep(episode), title)
    fun upNextWaiting(name: String) = Strings.roomUpNextWaiting(x, name)
    fun invited(by: String) = Strings.roomInvitedBy(x, by)
    fun overCap(n: Int) = Strings.roomInvitedOver(x, n)
    fun titleEpisode(title: String, episode: String) = Strings.roomInvitedDetail(x, title, ep(episode))
    fun carryOn(name: String) = Strings.roomCarryOnWithout(x, name)
    fun startWithout(name: String) = Strings.roomStartWithout(x, name)
    fun relaysAnswering(up: Int, of: Int) = if (up == of) Strings.roomRelaysAnswering(x, up) else Strings.roomRelaysAnsweringOf(x, up, of)
    fun ms(ms: Int) = Strings.roomRelayLatency(x, ms)
    fun relayRemove(relay: String) = Strings.roomRelayRemove(x, relay)
    fun movedTheRoom(by: String) = Strings.roomLineMoved(x, by)
    fun goAlong(others: List<String>, at: String) =
        if (others.isEmpty()) Strings.roomLineMovedDetailAlone(x, at) else Strings.roomLineMovedDetail(x, list(others), others.size, at)
    fun movedAway(by: String) = Strings.roomLineMovedAway(x, by)
    fun wentAlong(names: List<String>) = Strings.roomLineWentAlong(x, list(names))
    fun moveWith(n: Int) = Strings.roomMoveWith(x, n)
    fun moveFoot(at: String) = Strings.roomMoveFoot(x, at)
    fun chooseFor(episode: String) = Strings.roomChooseSource(x, ep(episode))
    fun noneHas(episode: String) = Strings.roomNoSourceTitle(x, ep(episode))
    fun pausedUntil(s: Int) = Strings.roomPausedUntilStart(x, s)
    fun notWatching(n: Int) = Strings.roomMoreNotWatching(x, n)
    fun plus(n: Int) = Strings.roomMoreFaces(x, n)
    fun plusLabel(n: Int) = Strings.roomMoreFacesLabel(x, n)
    fun withSeconds(line: String, s: Int) = Strings.roomLineWithSeconds(x, line, s)

    // Words on a face.
    val WATCHING get() = Strings.roomStateWatching(x)
    val READY get() = Strings.roomStateReady(x)
    val PAUSED_IT get() = Strings.roomStatePausedIt(x)
    val PAUSED get() = Strings.roomStatePaused(x)
    val CHOOSING get() = Strings.roomStateChoosing(x)
    val PLAYER_CLOSED get() = Strings.roomStatePlayerClosed(x)
    val RECONNECTING get() = Strings.roomStateReconnecting(x)
    val LEFT get() = Strings.roomStateLeft(x)
    fun behind(s: Int) = Strings.roomStateBehind(x, s)
    fun loading(s: Int) = Strings.roomStateLoading(x, s)
    fun buffering(s: Int) = Strings.roomStateBuffering(x, s)
    fun onEpisode(e: String) = Strings.roomStateOtherEpisode(x, ep(e))
    fun noSourceFor(e: String) = Strings.roomStateNoSource(x, ep(e))
    fun muted(word: String) = Strings.roomStateAndMuted(x, word)
    fun spoken(name: String, word: String) = Strings.roomFaceLabel(x, name, word)

    val NOT_SENT get() = Strings.roomChatNotSent(x)

    /** "Ren laughed at 12:04", or "Ren and Mika laughed at 12:04". */
    fun reacted(reaction: Reaction, names: String, at: String) = when (reaction) {
        Reaction.LAUGH -> Strings.roomReactedLaugh(x, names, at)
        Reaction.GASP -> Strings.roomReactedGasp(x, names, at)
        Reaction.CLAP -> Strings.roomReactedClap(x, names, at)
        Reaction.CRY -> Strings.roomReactedCry(x, names, at)
    }

    /** A chat line's name: "You" for this device's own. */
    fun chatName(line: RoomChatLine) = if (line.you) YOU else line.from

    /** A reaction or the room's own words, as one haze sentence; null for a typed line. */
    fun chatSentence(line: RoomChatLine): String? = when (val body = line.body) {
        is RoomChatBody.Said -> null
        is RoomChatBody.Reacted -> reacted(body.reaction, list(listOf(chatName(line)) + body.with), RoomWords.clock(body.at))
        is RoomChatBody.Room -> body.message.text
    }
}

/** The ring a face wears: with the room, the room waits for them, or not with the room. */
enum class Ring { Water, Haze, Arc, None }

/** How a face reads: its ring, how full the arc is, how dim the initial, the pause badge, the error dot. */
data class FaceLook(val ring: Ring, val arc: Float = 0f, val dim: Float = 1f, val badge: Boolean = false, val error: Boolean = false)

object RoomWords {
    /** When a recent room was last in: the time today, the weekday this week, else "18 November". */
    fun lastIn(seconds: Long): String {
        val at = java.time.Instant.ofEpochSecond(seconds).atZone(app.kaeri.store.AppClock.zone)
        val now = java.time.ZonedDateTime.now(app.kaeri.store.AppClock.clock)
        val days = java.time.temporal.ChronoUnit.DAYS.between(at.toLocalDate(), now.toLocalDate())
        val pattern = when {
            days <= 0L -> java.time.format.DateTimeFormatter.ofLocalizedTime(java.time.format.FormatStyle.SHORT)
            days < 7L -> java.time.format.DateTimeFormatter.ofPattern("EEEE") // i18n-skip
            else -> java.time.format.DateTimeFormatter.ofPattern("d MMMM") // i18n-skip
        }
        return at.format(pattern.withLocale(app.kaeri.store.AirTime.locale))
    }

    /** "12:04", or "1:12:04" past the hour. */
    fun clock(seconds: Double): String {
        val s = seconds.toLong().coerceAtLeast(0)
        return if (s >= 3600) "%d:%02d:%02d".format(s / 3600, s / 60 % 60, s % 60) else "%d:%02d".format(s / 60, s % 60) // i18n-skip
    }

    private fun secs(sinceMs: Long?, nowMs: Long): Int = if (sinceMs == null) 0 else ((nowMs - sinceMs) / 1000).toInt().coerceAtLeast(0)

    /** The room's place now: its position, run on from when it was read while it plays. */
    fun position(view: RoomView, nowMs: Long): Double {
        val at = view.position ?: return 0.0
        return if (view.playing && view.waiting.isEmpty() && view.positionAtMs > 0) at + (nowMs - view.positionAtMs).coerceAtLeast(0) / 1000.0 else at
    }

    /** A wait the line shows: from the engine's wait_shown_at, or past its first second. */
    fun shownWaits(view: RoomView, nowMs: Long): List<PausedFor> {
        val shown = view.waitShownAtMs
        return if (shown != null) (if (nowMs >= shown) view.waiting else emptyList()) else view.waiting.filter { nowMs - it.sinceMs >= 1000 }
    }

    /**
     * The wait's seconds, from the first waited member's start: the clock that member's row counts,
     * so the line and the row read the same. wait_shown_at only says when the wait may show.
     */
    fun waitSeconds(view: RoomView, nowMs: Long): Int = view.waiting.firstOrNull()?.let { secs(it.sinceMs, nowMs) } ?: 0

    /** Whether Carry on shows: the engine says so, or the wait is past 20 s. */
    fun carryShown(view: RoomView, nowMs: Long): Boolean =
        view.waiting.isNotEmpty() && (view.canCarryOn || view.carryOnAtMs?.let { nowMs >= it } == true || waitSeconds(view, nowMs) >= CARRY_ON_AFTER_S)

    /** The room's line: what is happening, in words. [positionS] is the room's place. */
    fun line(c: RoomCopy, view: RoomView, members: List<RoomMember>, nowMs: Long, positionS: Double): RoomLine {
        val waits = shownWaits(view, nowMs)
        val names = waits.map { it.name }
        val s = waitSeconds(view, nowMs)
        val ep = view.episode?.episode ?: ""
        val up = view.relays.count { it.healthy }
        val you = members.firstOrNull { it.you }
        if (view.connection is RoomConnection.Reconnecting) return RoomLine(c.RECONNECTING_TITLE, c.RECONNECTING_DETAIL)
        if (waits.any { it.you }) {
            val others = members.filter { !it.you && it.state != MemberState.LEFT }.map { it.name }
            return RoomLine(c.PAUSED_FOR_YOU, c.othersWaiting(c.names(others), others.size, s))
        }
        if (view.beforePlay) {
            view.startsAtMs?.let { at -> return RoomLine(c.startingIn(((at - nowMs + 999) / 1000).toInt().coerceAtLeast(0)), c.EVERYONE_READY) }
            if (waits.isNotEmpty() && carryShown(view, nowMs)) return RoomLine(c.still(waits), c.waited(s))
            if (waits.isNotEmpty()) return RoomLine(c.startingWhen(c.names(names), names.size), c.reasonFor(WaitReason.LOADING, s))
            return RoomLine(c.NOT_STARTED, c.PLAY_STARTS_IT)
        }
        view.upNext?.takeIf { it.cancelledBy == null }?.endsAtMs?.let { ends ->
            return RoomLine(c.upNextIn(((ends - nowMs + 999) / 1000).toInt().coerceAtLeast(0)), c.upNextFor(view.upNext!!.episode.episode))
        }
        if (waits.isNotEmpty() && carryShown(view, nowMs)) return RoomLine(c.still(waits), c.waited(s))
        if (waits.isNotEmpty()) return RoomLine(c.pausedFor(c.names(names)), c.reasonFor(waits.first().reason, s))
        view.carriedOn?.takeIf { it.past.isNotEmpty() }?.let { return RoomLine(c.WATCH_TOGETHER, c.carriedOn(if (it.byYou) c.YOU else it.by, it.past)) }
        view.moved?.let { return RoomLine(c.movedAway(it.by), if (it.went.isEmpty()) c.episodeAt(ep, clock(positionS)) else c.wentAlong(it.went)) }
        if (view.relays.isNotEmpty() && up < view.relays.size) return RoomLine(c.WATCH_TOGETHER, c.relaysDown(view.relays.size - up, view.relays.size))
        if (you?.state == MemberState.BEHIND && you.behindS != null) return RoomLine(c.WATCH_TOGETHER, c.behindYou(you.behindS!!.toInt()))
        if (!view.playing && (view.pausedBy != null || view.pausedByYou)) {
            val by = if (view.pausedByYou) c.YOU_PAUSED else c.pausedBy(view.pausedBy!!)
            return RoomLine(by, c.at(clock(positionS)))
        }
        return RoomLine(c.WATCH_TOGETHER, c.episodeAt(ep, clock(positionS)))
    }

    /** The button the wait brings forward at 20 s: "Carry on without Ren", or "Start without Ren" before play. */
    fun carryLabel(c: RoomCopy, view: RoomView, nowMs: Long): String? {
        if (!carryShown(view, nowMs)) return null
        val names = view.waiting.filterNot { it.you }.map { it.name }
        if (names.isEmpty()) return null
        val many = names.size > 1
        return if (view.beforePlay) (if (many) c.START_THEM else c.startWithout(names[0]))
        else (if (many) c.CARRY_ON_THEM else c.carryOn(names[0]))
    }

    /** Whether the room waits for [member] now. */
    fun waitedFor(view: RoomView, member: RoomMember): Boolean = view.waiting.any { it.id == member.id }

    /** The one word under a face, with its seconds where it counts them. */
    fun word(c: RoomCopy, view: RoomView, member: RoomMember, nowMs: Long): String {
        val base = when (member.state) {
            MemberState.WATCHING -> if (view.beforePlay) c.READY else c.WATCHING
            MemberState.READY -> c.READY
            MemberState.PAUSED -> if (member.pausedRoom) c.PAUSED_IT else c.PAUSED
            MemberState.BEHIND -> c.behind((member.behindS ?: 0f).toInt())
            MemberState.LOADING -> c.loading(secs(member.sinceMs, nowMs))
            MemberState.BUFFERING -> c.buffering(secs(member.sinceMs, nowMs))
            MemberState.OTHER_EPISODE -> c.onEpisode(member.episode?.episode ?: "")
            MemberState.CHOOSING -> c.CHOOSING
            MemberState.NO_SOURCE -> c.noSourceFor(view.episode?.episode ?: "")
            MemberState.PLAYER_CLOSED -> c.PLAYER_CLOSED
            MemberState.RECONNECTING -> c.RECONNECTING
            MemberState.LEFT -> c.LEFT
        }
        return if (member.muted) c.muted(base) else base
    }

    /** How the face reads: the ring, the arc toward 20 s, how dim, the badge and the dot. */
    fun look(view: RoomView, member: RoomMember, nowMs: Long): FaceLook = when (member.state) {
        MemberState.WATCHING, MemberState.READY, MemberState.BEHIND -> FaceLook(Ring.Water)
        MemberState.PAUSED -> FaceLook(Ring.Haze, badge = member.pausedRoom)
        // The arc fills toward the 20 s cap, a step a second.
        MemberState.LOADING, MemberState.BUFFERING ->
            FaceLook(Ring.Arc, arc = (secs(member.sinceMs, nowMs) / CARRY_ON_AFTER_S.toFloat()).coerceIn(0.04f, 1f))
        MemberState.NO_SOURCE -> FaceLook(Ring.None, dim = 0.5f, error = true)
        MemberState.LEFT -> FaceLook(Ring.None, dim = 0.35f)
        else -> FaceLook(Ring.None, dim = 0.5f)
    }

    /** Whether someone is not with the room: the strip stays while anyone is. */
    fun notWatching(member: RoomMember): Boolean =
        member.state !in setOf(MemberState.WATCHING, MemberState.READY, MemberState.PAUSED, MemberState.BEHIND)

    /** Up to 4 faces in join order, this device's first; past 4, three and a +N. */
    fun capped(members: List<RoomMember>, max: Int = 4): Pair<List<RoomMember>, List<RoomMember>> {
        val ordered = members.filter { it.you } + members.filterNot { it.you }
        return if (ordered.size <= max) ordered to emptyList() else ordered.take(max - 1) to ordered.drop(max - 1)
    }

    /** The +N face's word: "All watching" or "2 not watching". */
    fun moreWord(c: RoomCopy, hidden: List<RoomMember>): String {
        val n = hidden.count { notWatching(it) }
        return if (n == 0) c.ALL_WATCHING else c.notWatching(n)
    }

    /** The strip shows while anyone isn't watching, and for 8 s after any change. */
    const val STRIP_AFTER_CHANGE_MS = 8000L

    fun stripWanted(view: RoomView, members: List<RoomMember>): Boolean =
        view.waiting.isNotEmpty() || members.any { notWatching(it) && it.state != MemberState.LEFT } || view.connection is RoomConnection.Reconnecting
}
