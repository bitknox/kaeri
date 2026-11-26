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

    /** Whether this device is in a room. */
    val inRoom: Boolean get() = _view.value != null

    fun send(command: RoomCommand) {
        when (command) {
            is RoomCommand.AnswerMove -> _offer.value = null
            is RoomCommand.Leave -> {
                _view.value = null
                _members.value = emptyList()
                _conversation.value = null
            }
            else -> Unit
        }
        out(command)
    }

    /** An invite opened: show who invited and what the room watches while its glimpse comes. */
    fun invitedBy(target: RoomTarget) {
        _invited.value = RoomInvited(target.link, target.by, target.title ?: "", target.episode ?: "")
        out(RoomCommand.PeekInvite(target.link))
    }

    /** Join the room the invite opened. */
    fun join(anyway: Boolean = false) {
        val invite = _invited.value?.invite ?: return
        out(RoomCommand.Lobby(invite, null, null, anyway))
    }

    /** Not now: the invite goes, nobody in the room sees it. */
    fun notNow() {
        _invited.value = null
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
                _view.value = event.v1
                _invited.value = null
                _left.value = null
                _noSource.value = null
            }
            is Event.RoomNoSource -> _noSource.value = event.episode
            is Event.RoomState -> _view.value = event.v1
            is Event.RoomMembers -> _members.value = event.v1
            is Event.RoomChat -> _conversation.value = event.v1
            is Event.RoomChatState -> _chat.value = event.v1
            is Event.RoomMoveOffer -> _offer.value = event.v1
            is Event.RoomMoveOfferGone -> if (_offer.value?.id == event.id) _offer.value = null
            is Event.RoomGlimpse -> _invited.value?.let { asked ->
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
                _noSource.value = null
                _view.value = null
                _members.value = emptyList()
                _conversation.value = null
                _offer.value = null
                if (event.why !is RoomLeftWhy.Left && event.why !is RoomLeftWhy.Moved) _left.value = event.why
            }
            else -> return false
        }
        return true
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
}
