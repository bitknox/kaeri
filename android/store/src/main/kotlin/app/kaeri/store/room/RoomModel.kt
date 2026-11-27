// The room as the screens read it, and what they ask of it: the engine's Room* events in, its
// RoomCommand out. Tests and previews set the flows with show().

package app.kaeri.store.room

import app.kaeri.engine.model.Event
import app.kaeri.engine.model.RoomChatState
import app.kaeri.engine.model.RoomCommand
import app.kaeri.engine.model.RoomConnection
import app.kaeri.engine.model.RoomConversation
import app.kaeri.engine.model.RoomEpisode
import app.kaeri.engine.model.RoomEpisodeInfo
import app.kaeri.engine.model.RoomGlimpse
import app.kaeri.engine.model.RoomLeftWhy
import app.kaeri.engine.model.RoomMember
import app.kaeri.engine.model.RoomMoveOffer
import app.kaeri.engine.model.RoomRecent
import app.kaeri.engine.model.RoomRelaySettings
import app.kaeri.engine.model.RoomTarget
import app.kaeri.engine.model.RoomView
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/** A room an invite opened, before joining: who invited, what it watches, who is in it. */
data class RoomInvited(
    val invite: String,
    val by: String?,
    val title: String,
    val episode: String,
    val names: List<String> = emptyList(),
    /** What the room's relays said of it, once they did. */
    val glimpse: RoomGlimpse? = null,
    /** The look into it found nothing: its alert shows over the invite, and closing it closes the invite. */
    val failed: RoomLeftWhy? = null,
) {
    /** Rooms work best with ten or fewer: past it, Join asks first. */
    val over: Boolean get() = glimpse?.full ?: (names.size >= ROOM_SOFT_CAP)

    /** The room as the Join screen draws it before joining: who is there and what it watches. */
    fun asView(): RoomView {
        val info = glimpse?.roomEpisode ?: RoomEpisodeInfo(0u, episode, title, null, null, null)
        val playing = glimpse?.playing == true
        return RoomView(
            roomId = glimpse?.roomId ?: "",
            name = title,
            episode = RoomEpisode(info.id, info.episode),
            roomEpisode = info,
            position = null,
            positionAtMs = 0,
            connection = RoomConnection.Joining,
            link = invite,
            beforePlay = !playing,
            playing = playing,
            waiting = emptyList(),
            canCarryOn = false,
            relays = emptyList(),
            moveCandidates = emptyList(),
        )
    }
}

class RoomModel(private val out: (RoomCommand) -> Unit = {}) {
    private val _view = MutableStateFlow<RoomView?>(null)

    /** The room this device is in; `null` when in none. */
    val view: StateFlow<RoomView?> = _view.asStateFlow()

    private val _members = MutableStateFlow<List<RoomMember>>(emptyList())

    /** Who is in the room, in join order. */
    val members: StateFlow<List<RoomMember>> = _members.asStateFlow()

    private val _invited = MutableStateFlow<RoomInvited?>(null)

    /** A room an invite opened, before joining. */
    val invited: StateFlow<RoomInvited?> = _invited.asStateFlow()

    private val _offer = MutableStateFlow<RoomMoveOffer?>(null)

    /** Someone moved the room and ticked this device: Go along or Stay. */
    val offer: StateFlow<RoomMoveOffer?> = _offer.asStateFlow()

    private val _noSource = MutableStateFlow<String?>(null)

    /** The room's episode this device's play failed on, "No source for episode 9", until it starts again. */
    val noSource: StateFlow<String?> = _noSource.asStateFlow()

    private val _conversation = MutableStateFlow<RoomConversation?>(null)

    /** The room's chat, its own lines and the room's words among them. */
    val conversation: StateFlow<RoomConversation?> = _conversation.asStateFlow()

    private val _chat = MutableStateFlow<RoomChatState?>(null)
    val chat: StateFlow<RoomChatState?> = _chat.asStateFlow()

    private val _left = MutableStateFlow<RoomLeftWhy?>(null)

    /** Why this device is out of the room, when it didn't choose to be: the alert reads it. */
    val left: StateFlow<RoomLeftWhy?> = _left.asStateFlow()

    private val _recent = MutableStateFlow<List<RoomRecent>>(emptyList())

    /** The last ten rooms kept on this device, newest first: Watch together's Recent rooms. */
    val recent: StateFlow<List<RoomRecent>> = _recent.asStateFlow()

    private val _joinFailed = MutableStateFlow<String?>(null)

    /** The link Watch together's Join a room couldn't reach: its line shows under the field. */
    val joinFailed: StateFlow<String?> = _joinFailed.asStateFlow()

    private val _rejoinFailed = MutableStateFlow<String?>(null)

    /** The recent room nobody answered in: its line shows under its row. */
    val rejoinFailed: StateFlow<String?> = _rejoinFailed.asStateFlow()

    /** What Watch together asked for, so a failure lands in its place rather than an alert. */
    private var placeAsked: PlaceAsk? = null

    private sealed interface PlaceAsk {
        data class Join(val link: String) : PlaceAsk
        data class Rejoin(val roomId: String) : PlaceAsk
    }

    /** The look this device waits on: only its glimpse or its failure shows, never a slower one's. */
    private var looking: Look? = null

    private sealed interface Look {
        data class Invite(val link: String) : Look
        data class Recent(val roomId: String) : Look
    }

    /** A join or rejoin asked and not yet answered: its RoomLeft is its failure. */
    private var joining = false

    private val _relays = MutableStateFlow<RoomRelaySettings?>(null)

    /** Settings ▸ Relays: the list for new rooms and the spares, once the engine said them. */
    val relays: StateFlow<RoomRelaySettings?> = _relays.asStateFlow()

    /** Ask for Settings ▸ Relays. */
    fun loadRelays() = out(RoomCommand.LoadRelays)

    /** The list for new rooms; the defaults' own list counts as none chosen. */
    fun setRelays(relays: List<String>) {
        val defaults = _relays.value?.defaults
        out(RoomCommand.SetRelays(if (relays == defaults) emptyList() else relays))
    }

    /** The spares, in order. */
    fun setSpares(relays: List<String>) = out(RoomCommand.SetSpareRelays(relays))

    /** Restore the defaults: the built-in list for new rooms and the built-in spares. */
    fun restoreRelays() {
        out(RoomCommand.SetRelays(emptyList()))
        out(RoomCommand.SetSpareRelays(_relays.value?.defaultSpares ?: return))
    }

    private val _hideChat = MutableStateFlow(false)

    /** Hide chat in rooms, on this device: lines, reactions and the field stay away; the people stay. */
    val hideChat: StateFlow<Boolean> = _hideChat.asStateFlow()

    private var hideRead = false

    fun loadHideChat(context: android.content.Context) {
        if (hideRead) return
        hideRead = true
        _hideChat.value = app.kaeri.store.DevicePreferences.of(context).getBoolean(HIDE_CHAT, false)
    }

    fun setHideChat(context: android.content.Context, on: Boolean) {
        hideRead = true
        _hideChat.value = on
        app.kaeri.store.DevicePreferences.of(context).edit().putBoolean(HIDE_CHAT, on).apply()
    }

    /** Ask for the recent rooms. */
    fun loadRecent() = out(RoomCommand.LoadRecent)

    /** Join a room with the invite link pasted into Watch together. */
    fun joinLink(link: String) {
        _joinFailed.value = null
        placeAsked = PlaceAsk.Join(link)
        joining = true
        out(RoomCommand.Lobby(link, null, null, false))
    }

    /** Rejoin a recent room: its key is kept, so nobody is asked. */
    fun rejoin(roomId: String) {
        _rejoinFailed.value = null
        placeAsked = PlaceAsk.Rejoin(roomId)
        // A room no longer kept answers as a look that failed, under its row.
        looking = Look.Recent(roomId)
        joining = true
        out(RoomCommand.Rejoin(roomId))
    }

    /** Take a recent room off the list. */
    fun forget(roomId: String) {
        _recent.value = _recent.value.filterNot { it.roomId == roomId }
        if (_rejoinFailed.value == roomId) _rejoinFailed.value = null
        out(RoomCommand.ForgetRoom(roomId))
    }

    /** Whether this device is in a room. */
    val inRoom: Boolean get() = _view.value != null

    fun send(command: RoomCommand) {
        when (command) {
            is RoomCommand.AnswerMove -> _offer.value = null
            is RoomCommand.Leave -> {
                _view.value = null
                forgetRoom()
            }
            else -> Unit
        }
        out(command)
    }

    /** An invite opened: show who invited and what the room watches while its glimpse comes. */
    fun invitedBy(target: RoomTarget) {
        _invited.value = RoomInvited(target.link, target.by, target.title ?: "", target.episode ?: "")
        // Asked from an invite, not from Watch together: a failure is an alert, not that place's line.
        placeAsked = null
        looking = Look.Invite(target.link)
        out(RoomCommand.PeekInvite(target.link))
    }

    /** Join the room the invite opened. */
    fun join(anyway: Boolean = false) {
        val invite = _invited.value?.invite ?: return
        placeAsked = null
        joining = true
        out(RoomCommand.Lobby(invite, null, null, anyway))
    }

    /** Not now: the invite goes, nobody in the room sees it. */
    fun notNow() {
        _invited.value = null
        if (looking is Look.Invite) looking = null
    }

    fun dismissLeft() {
        _left.value = null
    }

    /** Takes the engine's room events; true when [event] was the room's. */
    fun apply(event: Event): Boolean {
        // A start opens the player again; the player's own handling still takes the event.
        if (event is Event.RoomStarted) _noSource.value = null
        when (event) {
            is Event.RoomJoined -> {
                // Another room: nothing of the last one's people, chat or offer carries over.
                if (_view.value?.roomId != event.v1.roomId) forgetRoom()
                _view.value = event.v1
                _invited.value = null
                _left.value = null
                _noSource.value = null
                _joinFailed.value = null
                _rejoinFailed.value = null
                placeAsked = null
                looking = null
                joining = false
            }
            is Event.RoomNoSource -> _noSource.value = event.episode
            is Event.RoomState -> _view.value = event.v1
            is Event.RoomMembers -> _members.value = event.v1
            is Event.RoomChat -> _conversation.value = event.v1
            is Event.RoomChatState -> _chat.value = event.v1
            is Event.RoomRecent -> _recent.value = event.v1
            is Event.RoomLookFailed -> lookFailed(event)
            is Event.RoomRelays -> _relays.value = event.v1
            is Event.RoomMoveOffer -> _offer.value = event.v1
            is Event.RoomMoveOfferGone -> if (_offer.value?.id == event.id) _offer.value = null
            is Event.RoomGlimpse -> _invited.value?.takeIf { looksAt(it, event.v1.invite) }?.let { asked ->
                val glimpse = event.v1
                _invited.value = asked.copy(
                    by = asked.by ?: glimpse.by,
                    title = asked.title.ifEmpty { glimpse.roomEpisode?.title ?: "" },
                    episode = asked.episode.ifEmpty { glimpse.roomEpisode?.episode ?: "" },
                    names = glimpse.members.map { it.name },
                    glimpse = glimpse,
                )
            }
            is Event.RoomLeft -> {
                // Only the room this device is in, or the one it asked to join, can be left: a look
                // never leaves anything.
                val ours = _view.value != null || joining
                joining = false
                if (!ours) return true
                _noSource.value = null
                _view.value = null
                forgetRoom()
                val asked = placeAsked
                placeAsked = null
                val why = event.why
                when {
                    why is RoomLeftWhy.Left || why is RoomLeftWhy.Moved -> Unit
                    // Asked from Watch together: the line shows where it was asked, not as an alert.
                    why is RoomLeftWhy.Unreachable && asked is PlaceAsk.Join -> _joinFailed.value = asked.link
                    (why is RoomLeftWhy.Unreachable || why is RoomLeftWhy.Failed) && asked is PlaceAsk.Rejoin -> _rejoinFailed.value = asked.roomId
                    else -> _left.value = why
                }
            }
            else -> return false
        }
        return true
    }

    /** Whether [asked] is the invite this device waits on, and [invite] is its answer. */
    private fun looksAt(asked: RoomInvited, invite: String?): Boolean =
        invite != null && invite == asked.invite && looking == Look.Invite(asked.invite)

    /** A look that found nothing: shown only for the look this device waits on. */
    private fun lookFailed(event: Event.RoomLookFailed) {
        val look = looking
        val invite = event.invite
        when {
            invite != null -> {
                val asked = _invited.value ?: return
                if (!looksAt(asked, invite)) return
                looking = null
                _invited.value = asked.copy(failed = event.why ?: RoomLeftWhy.Unreachable)
            }
            look is Look.Recent && look.roomId == event.roomId -> {
                looking = null
                joining = false
                placeAsked = null
                _rejoinFailed.value = event.roomId
            }
        }
    }

    /** What belongs to the room just left: its people, its chat and its state, a move offer. */
    private fun forgetRoom() {
        _members.value = emptyList()
        _conversation.value = null
        _chat.value = null
        _offer.value = null
    }

    /** Sets Watch together's recent rooms and failures: for tests and previews. */
    fun showPlace(recent: List<RoomRecent> = emptyList(), joinFailed: String? = null, rejoinFailed: String? = null) {
        _recent.value = recent
        _joinFailed.value = joinFailed
        _rejoinFailed.value = rejoinFailed
    }

    /** Sets the room as the screens should draw it: for tests and previews. */
    fun show(
        view: RoomView?,
        members: List<RoomMember> = emptyList(),
        invited: RoomInvited? = null,
        offer: RoomMoveOffer? = null,
        conversation: RoomConversation? = null,
    ) {
        _view.value = view
        _members.value = members
        _invited.value = invited
        _offer.value = offer
        _conversation.value = conversation
    }

    private companion object {
        /** The device's Hide chat; the key kept from before rooms so the choice carries over. */
        const val HIDE_CHAT = "party_hide_chat"
    }
}
