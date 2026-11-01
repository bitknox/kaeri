// Watch together as the screens show it: the party this device is in, who is in the room, the
// rooms kept on the device, what a look into a room found, and the lobby a link or code opened.
// Each viewer plays from their own sources; only names, file names and positions travel.

package app.kaeri.store

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import app.kaeri.engine.core.AnilistId
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.Msg
import app.kaeri.engine.model.NoSourceWhy
import app.kaeri.engine.model.PartyForm
import app.kaeri.engine.model.PartyGlimpse
import app.kaeri.engine.model.PartyMember
import app.kaeri.engine.model.PartyMessage
import app.kaeri.engine.model.PartyRecentRoom
import app.kaeri.engine.model.PartyTarget
import app.kaeri.engine.model.PartyView

/** A room on a server: what a link, a code or a kept room leads to. */
data class RoomKey(val server: String, val room: String)

/** What a look into a room has found so far. */
sealed interface Glimpse {
    /** Asked, and not answered yet: "Looking in" after 600 ms. */
    data object Looking : Glimpse

    data class Found(val glimpse: PartyGlimpse) : Glimpse

    /** The server could not be reached, in the engine's words. */
    data class Failed(val reason: Msg) : Glimpse
}

/**
 * The lobby a link or a code opened, before this device is in the room: who invited, and what
 * the link said the room watches, until the room says its own.
 */
data class LobbyTarget(val target: PartyTarget) {
    val key: RoomKey get() = RoomKey(target.server, target.room)
}

/** The party's episode had no source here: the lobby's sources line says why, still in the room. */
data class PartyNoSource(val id: AnilistId, val episode: String, val why: NoSourceWhy, val reason: Msg)

/** The room finished the last episode that is out: "That was the last episode out." */
data class PartyLastOut(val episode: String, val airs: Msg?)

class PartyModel {
    private val _view = MutableStateFlow<PartyView?>(null)

    /** The party this device is in, as it stands; `null` when in none. */
    val view: StateFlow<PartyView?> = _view.asStateFlow()

    private val _members = MutableStateFlow<List<PartyMember>>(emptyList())

    /** Who is in the room, this device's member first. */
    val members: StateFlow<List<PartyMember>> = _members.asStateFlow()

    private val _recent = MutableStateFlow<List<PartyRecentRoom>?>(null)

    /** The last ten rooms, newest first; `null` until the engine said. */
    val recent: StateFlow<List<PartyRecentRoom>?> = _recent.asStateFlow()

    private val _glimpses = MutableStateFlow<Map<RoomKey, Glimpse>>(emptyMap())

    /** What each room looked into this session showed. */
    val glimpses: StateFlow<Map<RoomKey, Glimpse>> = _glimpses.asStateFlow()

    private val _form = MutableStateFlow<PartyForm?>(null)

    /** The servers used before, the suggested one, a new room's name and the settings' name. */
    val form: StateFlow<PartyForm?> = _form.asStateFlow()

    private val _target = MutableStateFlow<LobbyTarget?>(null)

    /** The room a link or code opened, while its lobby is up. */
    val target: StateFlow<LobbyTarget?> = _target.asStateFlow()

    private val _noSource = MutableStateFlow<PartyNoSource?>(null)
    val noSource: StateFlow<PartyNoSource?> = _noSource.asStateFlow()

    private val _lastOut = MutableStateFlow<PartyLastOut?>(null)
    val lastOut: StateFlow<PartyLastOut?> = _lastOut.asStateFlow()

    private val _failure = MutableStateFlow<Msg?>(null)

    /** Why the last lobby or join ended, when the server would not have it; cleared on the next try. */
    val failure: StateFlow<Msg?> = _failure.asStateFlow()

    private val _joining = MutableStateFlow(false)

    /** A join or a start was asked and the room has not answered yet. */
    val joining: StateFlow<Boolean> = _joining.asStateFlow()

    private val _linked = MutableStateFlow<PartyTarget?>(null)

    /** A party link the app was opened with, until the screens took it: they open its lobby. */
    val linked: StateFlow<PartyTarget?> = _linked.asStateFlow()

    internal fun linked(target: PartyTarget) {
        _linked.value = target
    }

    /** The screens took [linked]. */
    fun tookLink() {
        _linked.value = null
    }

    /** The room's words worth a notice off the player: who came, who left, a new episode. */
    var onMessage: ((PartyMessage) -> Unit)? = null

    /** Whether this device is in [key]'s room. */
    fun isIn(key: RoomKey): Boolean = _view.value?.let { it.server == key.server && it.room == key.room } == true

    internal fun opened(target: PartyTarget) {
        _target.value = LobbyTarget(target)
        _failure.value = null
    }

    internal fun closedLobby() {
        _target.value = null
    }

    internal fun asked() {
        _joining.value = true
        _failure.value = null
    }

    internal fun looking(key: RoomKey) {
        _glimpses.value = _glimpses.value + (key to Glimpse.Looking)
    }

    /** Apply one of the engine's party events; false for an event that is not one. */
    fun apply(event: Event): Boolean {
        when (event) {
            is Event.PartyForm -> _form.value = event.v1
            is Event.PartyJoined -> {
                _joining.value = false
                _view.value = event.v1
                _noSource.value = null
                _lastOut.value = null
            }
            is Event.PartyState -> if (_view.value != null || _joining.value) {
                _joining.value = false
                _view.value = event.v1
            }
            is Event.PartyMembers -> _members.value = event.v1
            is Event.PartyMessage -> onMessage?.invoke(event.v1)
            is Event.PartyLeft -> {
                _joining.value = false
                _view.value = null
                _members.value = emptyList()
                _noSource.value = null
                _failure.value = event.reason
            }
            is Event.PartyNoSource -> _noSource.value = PartyNoSource(event.id, event.episode, event.why, event.reason)
            is Event.PartyStarted -> {
                _noSource.value = null
                _lastOut.value = null
            }
            is Event.PartyGlimpse -> _glimpses.value =
                _glimpses.value + (RoomKey(event.v1.server, event.v1.room) to Glimpse.Found(event.v1))
            is Event.PartyLookFailed -> _glimpses.value =
                _glimpses.value + (RoomKey(event.server, event.room) to Glimpse.Failed(event.reason))
            is Event.PartyRecent -> _recent.value = event.v1
            is Event.PartyLastOut -> _lastOut.value = PartyLastOut(event.episode, event.airs)
            else -> return false
        }
        return true
    }

    companion object {
        /** When a room was last in: the time today, the weekday this week, else the day and month. */
        fun lastIn(seconds: Long): String {
            val at = java.time.Instant.ofEpochSecond(seconds).atZone(AppClock.zone)
            val now = java.time.ZonedDateTime.now(AppClock.clock)
            val days = java.time.temporal.ChronoUnit.DAYS.between(at.toLocalDate(), now.toLocalDate())
            val pattern = when {
                days <= 0L -> java.time.format.DateTimeFormatter.ofLocalizedTime(java.time.format.FormatStyle.SHORT)
                days < 7L -> java.time.format.DateTimeFormatter.ofPattern("EEEE") // i18n-skip
                else -> java.time.format.DateTimeFormatter.ofPattern("d MMMM") // i18n-skip
            }
            return at.format(pattern.withLocale(AirTime.locale))
        }

        /** A kept room's line: "Episode 8 · with Mika and Ren · Tuesday", or without the names. */
        fun recentLine(context: android.content.Context, room: PartyRecentRoom): String {
            val whenText = lastIn(room.lastAt)
            return if (room.names.isEmpty()) {
                listOf(app.kaeri.i18n.Strings.partyFormEpisode(context, room.episode.toDoubleOrNull()?.toInt() ?: 0), whenText).joinToString(" · ") // i18n-skip
            } else {
                app.kaeri.i18n.Strings.togetherRecentRow(context, room.episode, Wording.list(room.names), whenText)
            }
        }

        /** The ready count and the members' count, as "2 of 3 ready" says them. */
        fun counts(members: List<PartyMember>): Pair<Int, Int> = members.count { it.ready == true } to members.size

        /** Who play waits for: "Play starts once Mika is ready", or alone, or everyone ready. */
        fun waitLine(context: android.content.Context, members: List<PartyMember>): String {
            if (members.size <= 1) return app.kaeri.i18n.Strings.partyAlone(context)
            val waiting = waiting(members)
            return when {
                waiting.isEmpty() -> app.kaeri.i18n.Strings.partyAllReady(context)
                waiting.size == 1 && waiting[0].you -> app.kaeri.i18n.Strings.partyWaitYou(context)
                waiting.size == 1 -> app.kaeri.i18n.Strings.partyWaitOne(context, waiting[0].name)
                waiting.size == 2 -> app.kaeri.i18n.Strings.partyWaitTwo(context, waiting[0].name, waiting[1].name)
                else -> app.kaeri.i18n.Strings.partyWaitMany(context, waiting.size)
            }
        }

        /**
         * The members play waits for, for the start line: the others not ready yet, or this
         * device's member alone when only they are left. I'm ready already asks this device.
         */
        fun waiting(members: List<PartyMember>): List<PartyMember> {
            val others = members.filter { it.ready != true && !it.you }
            return others.ifEmpty { members.filter { it.ready != true && it.you } }
        }
    }
}
