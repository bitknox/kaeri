// The room's words, as the approved board writes them (design/room board v5, Strings). The
// English sits in RoomCopy until feat/room-engine adds the room's catalogue to i18n/; then each
// RoomCopy entry becomes its Strings call and the i18n-skip marks go. Nothing here is a source
// name or a host.

package app.kaeri.store.room

/** A line of the room: the bold words and the haze words under them. */
data class RoomLine(val title: String, val detail: String)

/** The board's English for the room, one place to swap for the catalogue. */
object RoomCopy {
    const val WATCH_TOGETHER = "Watch together" // i18n-skip
    const val NOT_STARTED = "Not started yet" // i18n-skip
    const val PLAY_STARTS_IT = "Play starts it for everyone" // i18n-skip
    const val EVERYONE_READY = "Everyone is ready" // i18n-skip
    const val WAITED_20 = "The room has waited 20 s" // i18n-skip
    const val RECONNECTING_TITLE = "Reconnecting to the room" // i18n-skip
    const val RECONNECTING_DETAIL = "No relay is answering · your episode keeps playing" // i18n-skip
    const val PAUSED_FOR_YOU = "Paused for you" // i18n-skip
    const val YOU_PAUSED = "You paused" // i18n-skip
    const val YOU = "You" // i18n-skip
    const val MORE = "More" // i18n-skip
    const val ALL_WATCHING = "All watching" // i18n-skip
    const val PLAY_FOR_EVERYONE = "Play for everyone" // i18n-skip
    const val WAITING_FIRST_FRAMES = "Waiting for first frames" // i18n-skip
    const val DONT_WAIT = "Don't wait for me" // i18n-skip
    const val CARRY_ON_THEM = "Carry on without them" // i18n-skip
    const val START_THEM = "Start without them" // i18n-skip
    const val JOIN = "Join the room" // i18n-skip
    const val JOIN_ANYWAY = "Join anyway" // i18n-skip
    const val NOT_NOW = "Not now" // i18n-skip
    const val CHAT_OPENS = "The chat opens when you join" // i18n-skip
    const val SAY_SOMETHING = "Say something" // i18n-skip
    const val REACT = "React" // i18n-skip
    const val INVITE = "Invite" // i18n-skip
    const val INVITE_BODY = "Anyone with this link can join. The link carries the room's key, so only people who have it can read the room." // i18n-skip
    const val INVITE_SCAN = "Someone beside you can scan this with their camera." // i18n-skip
    const val COPY_LINK = "Copy link" // i18n-skip
    const val SHARE = "Share…" // i18n-skip
    const val NEW_INVITE = "New invite" // i18n-skip
    const val NEW_INVITE_NOTE = "Stops the old link" // i18n-skip
    const val NEW_INVITE_BODY = "New invite stops the old link working. Everyone in the room moves to the new one." // i18n-skip
    const val DETAILS = "Details" // i18n-skip
    const val ENCRYPTION = "Encryption" // i18n-skip
    const val END_TO_END = "End to end" // i18n-skip
    const val RELAYS = "Relays" // i18n-skip
    const val NOT_ANSWERING = "Not answering" // i18n-skip
    const val RELAYS_ABOUT = "Relays pass the room's messages along and can't read them. These are public relays run by others; Kaeri doesn't run any of them. If one stops answering, the room swaps in a spare." // i18n-skip
    const val EDIT_RELAYS_MORE = "Edit relays…" // i18n-skip
    const val EDIT_RELAYS = "Edit relays" // i18n-skip
    const val EDIT_RELAYS_BODY = "Changes this room's relays for everyone in it. New invites carry the list." // i18n-skip
    const val RELAY_FIELD = "wss://" // i18n-skip
    const val ADD = "Add" // i18n-skip
    const val RELAYS_SETTINGS = "Settings ▸ Playback ▸ Relays sets the list for new rooms." // i18n-skip
    const val DONE = "Done" // i18n-skip
    const val MOVE_MORE = "Move to a new room…" // i18n-skip
    const val MOVE = "Move to a new room" // i18n-skip
    const val MOVE_BODY = "Everyone you tick is asked to go along. Nobody moves without saying yes, and anyone not ticked stays in this room." // i18n-skip
    const val ALWAYS_GOES = "Always goes" // i18n-skip
    const val MUTED_UNTICKED = "Muted, so not ticked" // i18n-skip
    const val GO_ALONG = "Go along" // i18n-skip
    const val STAY_HERE = "Stay here" // i18n-skip
    const val CANCEL = "Cancel" // i18n-skip
    const val LEAVE = "Leave" // i18n-skip
    const val LEAVE_MORE = "Leave the room…" // i18n-skip
    const val LEAVE_TITLE = "Leave the room?" // i18n-skip
    const val LEAVE_BODY = "The others keep watching." // i18n-skip
    const val KEEP_WATCHING = "Keep watching" // i18n-skip
    const val COPY_INVITE = "Copy invite link" // i18n-skip
    const val SUBTITLES_AUDIO = "Subtitles and audio" // i18n-skip
    const val EPISODES = "Episodes" // i18n-skip
    const val CLOSE_TO_BAR = "Close the room to the bar" // i18n-skip
    const val FULL_SCREEN = "Full screen" // i18n-skip
    const val PLAY_IN_ROOM = "Play in the room" // i18n-skip
    const val CHOOSE_BODY = "The room keeps watching; you join at its place." // i18n-skip
    const val OPEN_SOURCES = "Open sources" // i18n-skip
    const val SEARCH_TITLES = "Search titles" // i18n-skip
    const val STAY_IN_ROOM = "You stay in the room and the chat." // i18n-skip
    const val WRITE_ON_PHONE = "Write from your phone" // i18n-skip
    const val TV_HINT = "OK for the controls · Back closes the room" // i18n-skip
    const val UNREACHABLE_TITLE = "Couldn't reach this room's relays" // i18n-skip
    const val UNREACHABLE_BODY = "Ask for a new invite." // i18n-skip
    const val QUIET_TITLE = "Nobody answered in this room" // i18n-skip
    const val QUIET_BODY = "If someone made a new invite, the old one stopped working. Ask them for the new link." // i18n-skip
    const val OLD_INVITE = "This invite is from an older version of Kaeri. Ask for a new one." // i18n-skip
    const val TRY_AGAIN = "Try again" // i18n-skip
    const val CLOSE = "Close" // i18n-skip
    const val OVER_CAP = "people are here already · rooms work best with 10 or fewer" // i18n-skip

    fun names(n: List<String>): String = when (n.size) {
        0 -> ""
        1 -> n[0]
        2 -> "${n[0]} and ${n[1]}" // i18n-skip
        else -> "${n.size} people" // i18n-skip
    }
    fun list(n: List<String>): String = when (n.size) {
        0 -> ""
        1 -> n[0]
        else -> n.dropLast(1).joinToString(", ") + " and " + n.last() // i18n-skip
    }
    fun seconds(s: Int) = "$s s" // i18n-skip
    fun episodeAt(episode: String, at: String) = "Episode $episode · $at" // i18n-skip
    fun inTheRoom(episode: String, n: Int) = "Episode $episode · $n in the room" // i18n-skip
    fun pausedBy(name: String) = "$name paused" // i18n-skip
    fun at(time: String) = "at $time" // i18n-skip
    fun pausedFor(names: String) = "Paused for $names" // i18n-skip
    fun stillLoading(names: String, many: Boolean) = if (many) "$names are still loading" else "$names is still loading" // i18n-skip
    fun startingWhen(names: String, many: Boolean) = if (many) "Starting when $names are ready" else "Starting when $names is ready" // i18n-skip
    fun startingIn(n: Int) = "Starting in $n" // i18n-skip
    fun reasonFor(reason: WaitReason, s: Int) = (if (reason == WaitReason.Buffering) "Buffering" else "Loading") + " · " + seconds(s) // i18n-skip
    fun othersWaiting(names: String, s: Int) = "$names are waiting · ${seconds(s)}" // i18n-skip
    fun carriedOn(by: String, names: String, many: Boolean) = "$by carried on · " + (if (many) "they join" else "$names joins") + " when ready" // i18n-skip
    fun behindYou(s: Int) = "You're $s s behind · catching up" // i18n-skip
    fun relaysDown(down: Int, of: Int) = "$down of $of relays isn't answering · the room carries on" // i18n-skip
    fun upNextIn(s: Int) = "Up next in $s s" // i18n-skip
    fun upNextFor(episode: String) = "Episode $episode plays for everyone" // i18n-skip
    fun invited(by: String) = "$by invited you" // i18n-skip
    fun overCap(n: Int) = "$n $OVER_CAP" // i18n-skip
    fun titleEpisode(title: String, episode: String) = "$title · Episode $episode" // i18n-skip
    fun carryOn(name: String) = "Carry on without $name" // i18n-skip
    fun startWithout(name: String) = "Start without $name" // i18n-skip
    fun relaysAnswering(up: Int, of: Int) = if (up == of) "$up answering" else "$up of $of answering" // i18n-skip
    fun ms(ms: Int) = "$ms ms" // i18n-skip
    fun movedTheRoom(by: String) = "$by moved the room" // i18n-skip
    fun goAlong(others: List<String>, at: String) =
        "Go along? " + (if (others.isEmpty()) "" else "${list(others)} ${if (others.size == 1) "is" else "are"} asked too. ") + "The episode carries on from $at." // i18n-skip
    fun movedAway(by: String) = "$by moved to a new room" // i18n-skip
    fun wentAlong(names: List<String>) = "${list(names)} went along · this room carries on" // i18n-skip
    fun moveWith(n: Int) = if (n == 1) "Move with 1 other" else "Move with $n others" // i18n-skip
    fun moveFoot(at: String) = "The new room picks up at $at with a new key. Its link reaches only the people ticked." // i18n-skip
    fun chooseFor(episode: String) = "Choose a source for episode $episode" // i18n-skip
    fun noneHas(episode: String) = "None of your sources has episode $episode" // i18n-skip
    fun pausedUntil(s: Int) = "The room is paused until this starts · ${seconds(s)}" // i18n-skip
    fun notWatching(n: Int) = "$n not watching" // i18n-skip
    fun plus(n: Int) = "+$n" // i18n-skip
    fun upNextCard() = "Up next for the room" // i18n-skip
    fun playsIn(s: Int) = "Plays for everyone in $s s" // i18n-skip

    // Words on a face.
    const val WATCHING = "Watching" // i18n-skip
    const val READY = "Ready" // i18n-skip
    const val PAUSED_IT = "Paused it" // i18n-skip
    const val PAUSED = "Paused" // i18n-skip
    const val BEHIND = "Behind" // i18n-skip
    const val LOADING = "Loading" // i18n-skip
    const val BUFFERING = "Buffering" // i18n-skip
    const val CHOOSING = "Choosing a source" // i18n-skip
    const val PLAYER_CLOSED = "Player closed" // i18n-skip
    const val RECONNECTING = "Reconnecting" // i18n-skip
    const val LEFT = "Left" // i18n-skip
    const val MUTED = "Muted" // i18n-skip
    fun onEpisode(e: String) = "On episode $e" // i18n-skip
    fun noSourceFor(e: String) = "No source for episode $e" // i18n-skip
    fun withSeconds(word: String, s: Int) = "$word · ${seconds(s)}" // i18n-skip
    fun spoken(name: String, word: String) = "$name, $word" // i18n-skip
}

/** The ring a face wears: with the room, the room waits for them, or not with the room. */
enum class Ring { Water, Haze, Arc, None }

/** How a face reads: its ring, how full the arc is, how dim the initial, the pause badge, the error dot. */
data class FaceLook(val ring: Ring, val arc: Float = 0f, val dim: Float = 1f, val badge: Boolean = false, val error: Boolean = false)

object RoomWords {
    /** "12:04", or "1:12:04" past the hour. */
    fun clock(seconds: Double): String {
        val s = seconds.toLong().coerceAtLeast(0)
        return if (s >= 3600) "%d:%02d:%02d".format(s / 3600, s / 60 % 60, s % 60) else "%d:%02d".format(s / 60, s % 60) // i18n-skip
    }

    private fun secs(sinceMs: Long?, nowMs: Long): Int = if (sinceMs == null) 0 else ((nowMs - sinceMs) / 1000).toInt().coerceAtLeast(0)

    /** A wait the line shows: past its first second, so a fast seek never flashes it. */
    fun shownWaits(view: RoomView, nowMs: Long): List<PausedFor> = view.waiting.filter { nowMs - it.sinceMs >= 1000 }

    /** The oldest wait's seconds. */
    fun waitSeconds(view: RoomView, nowMs: Long): Int = view.waiting.minOfOrNull { it.sinceMs }?.let { secs(it, nowMs) } ?: 0

    /**
     * The room's line: what is happening, in words. [positionS] is the room's place, [you] this
     * device's member, [carried] who carried on past whom while they still load.
     */
    fun line(view: RoomView, members: List<RoomMember>, nowMs: Long, positionS: Double, carried: Pair<String, List<String>>? = null): RoomLine {
        val c = RoomCopy
        val you = members.firstOrNull { it.you }
        val waits = shownWaits(view, nowMs)
        val names = waits.map { it.name }
        val many = names.size > 1
        val s = waitSeconds(view, nowMs)
        val ep = view.episode?.episode ?: ""
        val up = view.relays.count { it.healthy }
        if (view.connection is RoomConnection.Reconnecting) return RoomLine(c.RECONNECTING_TITLE, c.RECONNECTING_DETAIL)
        if (you != null && waits.any { it.name == you.name }) {
            val others = members.filter { !it.you && it.state != MemberState.Left }.map { it.name }
            return RoomLine(c.PAUSED_FOR_YOU, c.othersWaiting(c.list(others), s))
        }
        if (view.beforePlay) {
            if (view.startingIn != null) return RoomLine(c.startingIn(view.startingIn), c.EVERYONE_READY)
            if (waits.isNotEmpty() && s >= CARRY_ON_AFTER_S) return RoomLine(c.stillLoading(c.names(names), many), c.WAITED_20)
            if (waits.isNotEmpty()) return RoomLine(c.startingWhen(c.names(names), many), c.reasonFor(WaitReason.Loading, s))
            return RoomLine(c.NOT_STARTED, c.PLAY_STARTS_IT)
        }
        view.upNext?.endsAtMs?.takeIf { view.upNext.cancelledBy == null }?.let { ends ->
            return RoomLine(c.upNextIn(((ends - nowMs + 999) / 1000).toInt().coerceAtLeast(0)), c.upNextFor(view.upNext.episode))
        }
        if (waits.isNotEmpty() && s >= CARRY_ON_AFTER_S) return RoomLine(c.stillLoading(c.names(names), many), c.WAITED_20)
        if (waits.isNotEmpty()) return RoomLine(c.pausedFor(c.names(names)), c.reasonFor(waits.first().reason, s))
        if (carried != null && carried.second.isNotEmpty()) {
            return RoomLine(c.WATCH_TOGETHER, c.carriedOn(carried.first, c.names(carried.second), carried.second.size > 1))
        }
        if (view.relays.isNotEmpty() && up < view.relays.size) return RoomLine(c.WATCH_TOGETHER, c.relaysDown(view.relays.size - up, view.relays.size))
        if (you?.state == MemberState.Behind && you.behindS != null) return RoomLine(c.WATCH_TOGETHER, c.behindYou(you.behindS.toInt()))
        if (!view.playing && view.pausedBy != null) {
            val by = if (view.pausedBy == you?.name) c.YOU_PAUSED else c.pausedBy(view.pausedBy)
            return RoomLine(by, c.at(clock(positionS)))
        }
        return RoomLine(c.WATCH_TOGETHER, c.episodeAt(ep, clock(positionS)))
    }

    /** The button the wait brings forward at 20 s: "Carry on without Ren", or "Start without Ren" before play. */
    fun carryLabel(view: RoomView, nowMs: Long): String? {
        if (!view.canCarryOn && waitSeconds(view, nowMs) < CARRY_ON_AFTER_S) return null
        val names = view.waiting.map { it.name }
        if (names.isEmpty()) return null
        val many = names.size > 1
        return if (view.beforePlay) (if (many) RoomCopy.START_THEM else RoomCopy.startWithout(names[0]))
        else (if (many) RoomCopy.CARRY_ON_THEM else RoomCopy.carryOn(names[0]))
    }

    /** Whether the room waits for [member] now. */
    fun waitedFor(view: RoomView, member: RoomMember): Boolean = view.waiting.any { it.name == member.name }

    /** The one word under a face, with its seconds where it counts them. */
    fun word(view: RoomView, member: RoomMember, nowMs: Long): String {
        val c = RoomCopy
        val base = when (member.state) {
            MemberState.Watching -> if (view.beforePlay) c.READY else c.WATCHING
            MemberState.Paused -> if (member.pausedRoom) c.PAUSED_IT else c.PAUSED
            MemberState.Behind -> c.withSeconds(c.BEHIND, (member.behindS ?: 0f).toInt())
            MemberState.Loading -> c.withSeconds(c.LOADING, secs(member.sinceMs, nowMs))
            MemberState.Buffering -> c.withSeconds(c.BUFFERING, secs(member.sinceMs, nowMs))
            MemberState.OtherEpisode -> c.onEpisode(member.episode?.episode ?: "")
            MemberState.Choosing -> c.CHOOSING
            MemberState.NoSource -> c.noSourceFor(view.episode?.episode ?: "")
            MemberState.PlayerClosed -> c.PLAYER_CLOSED
            MemberState.Reconnecting -> c.RECONNECTING
            MemberState.Left -> c.LEFT
        }
        return if (member.muted) "$base · ${c.MUTED}" else base // i18n-skip
    }

    /** How the face reads: the ring, the arc toward 20 s, how dim, the badge and the dot. */
    fun look(view: RoomView, member: RoomMember, nowMs: Long): FaceLook = when (member.state) {
        MemberState.Watching -> FaceLook(Ring.Water)
        MemberState.Behind -> FaceLook(Ring.Water)
        MemberState.Paused -> FaceLook(Ring.Haze, badge = member.pausedRoom)
        // The arc fills toward the 20 s cap, a step a second.
        MemberState.Loading, MemberState.Buffering ->
            FaceLook(Ring.Arc, arc = (secs(member.sinceMs, nowMs) / CARRY_ON_AFTER_S.toFloat()).coerceIn(0.04f, 1f))
        MemberState.NoSource -> FaceLook(Ring.None, dim = 0.5f, error = true)
        MemberState.Left -> FaceLook(Ring.None, dim = 0.35f)
        else -> FaceLook(Ring.None, dim = 0.5f)
    }

    /** Whether someone is not with the room: the strip stays while anyone is. */
    fun notWatching(member: RoomMember): Boolean = member.state !in setOf(MemberState.Watching, MemberState.Paused, MemberState.Behind)

    /** Up to 4 faces in join order, this device's first; past 4, three and a +N. */
    fun capped(members: List<RoomMember>, max: Int = 4): Pair<List<RoomMember>, List<RoomMember>> {
        val ordered = members.filter { it.you } + members.filterNot { it.you }
        return if (ordered.size <= max) ordered to emptyList() else ordered.take(max - 1) to ordered.drop(max - 1)
    }

    /** The +N face's word: "All watching" or "2 not watching". */
    fun moreWord(hidden: List<RoomMember>): String {
        val n = hidden.count { notWatching(it) }
        return if (n == 0) RoomCopy.ALL_WATCHING else RoomCopy.notWatching(n)
    }

    /** The strip shows while anyone isn't watching, and for 8 s after any change. */
    const val STRIP_AFTER_CHANGE_MS = 8000L

    fun stripWanted(view: RoomView, members: List<RoomMember>): Boolean =
        view.waiting.isNotEmpty() || members.any { notWatching(it) && it.state != MemberState.Left } || view.connection is RoomConnection.Reconnecting
}
