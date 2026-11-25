// The room as the screens read it, and what they ask of it. Until feat/room-engine's FFI lands
// the flows are set by tests and previews (RoomModel.show) and the commands go to [send]; after
// it, apply() takes the engine's RoomState, RoomMembers and RoomMoveOffer events and the
// commands become Command.Room.

package app.kaeri.store.room

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/** What the room's screens ask of it: section 6's commands, by their engine names. */
sealed interface RoomAsk {
    data object PlayForEveryone : RoomAsk
    data object CarryOn : RoomAsk
    data object DontWait : RoomAsk
    data object NewInvite : RoomAsk
    data object Leave : RoomAsk
    data object ClosePlayer : RoomAsk
    data object OpenPlayer : RoomAsk
    data class Join(val anyway: Boolean) : RoomAsk
    data object NotNow : RoomAsk
    data class MoveRoom(val take: List<String>) : RoomAsk
    data class AnswerMove(val offer: String, val go: Boolean) : RoomAsk
    data class SetRelays(val relays: List<String>) : RoomAsk
    data class ChoosingSource(val open: Boolean) : RoomAsk
}

/** The old room after a Move, for whoever stayed: who moved it and who went along. */
data class RoomMoved(val by: String, val went: List<String>)

/** A Carry on that went past people still loading: who pressed it, and whom it went past. */
data class RoomCarried(val by: String, val past: List<String>)

class RoomModel(private val send: (RoomAsk) -> Unit = {}) {
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

    /** Someone moved the room and ticked this device: Go along or Stay here. */
    val offer: StateFlow<RoomMoveOffer?> = _offer.asStateFlow()

    private val _moved = MutableStateFlow<RoomMoved?>(null)
    val moved: StateFlow<RoomMoved?> = _moved.asStateFlow()

    private val _carried = MutableStateFlow<RoomCarried?>(null)
    val carried: StateFlow<RoomCarried?> = _carried.asStateFlow()

    fun ask(what: RoomAsk) {
        when (what) {
            is RoomAsk.AnswerMove -> _offer.value = null
            else -> Unit
        }
        send(what)
    }

    /** Sets the room as the screens should draw it: for tests, previews and the stand-in phase. */
    fun show(
        view: RoomView?,
        members: List<RoomMember> = emptyList(),
        invited: RoomInvited? = null,
        offer: RoomMoveOffer? = null,
        moved: RoomMoved? = null,
        carried: RoomCarried? = null,
    ) {
        _view.value = view
        _members.value = members
        _invited.value = invited
        _offer.value = offer
        _moved.value = moved
        _carried.value = carried
    }
}
