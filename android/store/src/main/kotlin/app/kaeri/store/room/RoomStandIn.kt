// Stand-ins for the engine's room records (room-nostr spec, section 6), shaped as uniffi will
// generate them, so the room's screens can be built before the engine's FFI lands. When
// feat/room-engine's FFI commit is in, these go and the screens import app.kaeri.engine.model.

package app.kaeri.store.room

/** What a person in the room is doing, highest first as a row shows it: Left, Reconnecting, their own, Behind, Paused, Watching. */
enum class MemberState { Watching, Paused, Loading, Buffering, Behind, OtherEpisode, Choosing, NoSource, PlayerClosed, Reconnecting, Left }

/** Why the room waits for someone. */
enum class WaitReason { Loading, Seeking, Buffering }

/** The episode a member or the room is on: AniList id and the episode's number as text. */
data class RoomEpisode(val id: Long, val episode: String)

/**
 * One person in the room. [sinceMs] is epoch ms on this device's clock: the screens count the
 * seconds of Loading and Buffering from it on their own one-second tick.
 */
data class RoomMember(
    val id: String,
    val name: String,
    val you: Boolean,
    val state: MemberState,
    val sinceMs: Long? = null,
    val behindS: Float? = null,
    val episode: RoomEpisode? = null,
    /** This member paused the room: their face carries the pause badge. */
    val pausedRoom: Boolean = false,
    val background: Boolean = false,
    val muted: Boolean = false,
)

/** Someone the room is waiting for, and since when. */
data class PausedFor(val name: String, val reason: WaitReason, val sinceMs: Long)

/** The room's Up next: [endsAtMs] is null while the card waits for a longer file, named in [waitingFor]. */
data class RoomUpNext(val episode: String, val endsAtMs: Long?, val waitingFor: List<String>, val cancelledBy: String?)

/** One relay of the room and how it answers. */
data class RelayHealth(val url: String, val healthy: Boolean, val rttMs: Int?)

/** This device's own link to the room's relays. */
sealed interface RoomConnection {
    data object Connected : RoomConnection
    data class Reconnecting(val relaysUp: Int) : RoomConnection
}

/** A row of Move to a new room's list: everyone present, ticked unless muted. */
data class MoveCandidate(val id: String, val name: String, val muted: Boolean, val ticked: Boolean)

/** Someone moved the room and ticked this device's member: Go along or Stay here. */
data class RoomMoveOffer(
    val id: String,
    val by: String,
    /** Who else was ticked. */
    val names: List<String>,
    val title: String,
    val episode: String,
    val expiresAtMs: Long,
)

/**
 * The room as it stands. [playing] is the effective state: the room plays when nobody it waits
 * for is still loading. [beforePlay] holds from joining until somebody presses Play for everyone;
 * [startingIn] is the count after it, while everyone is ready (both are stand-in fields the spec
 * leaves to the engine's lobby).
 */
data class RoomView(
    val title: String,
    val episode: RoomEpisode?,
    val name: String,
    val connection: RoomConnection = RoomConnection.Connected,
    val link: String? = null,
    val beforePlay: Boolean = false,
    val startingIn: Int? = null,
    val playing: Boolean = true,
    val pausedBy: String? = null,
    val waiting: List<PausedFor> = emptyList(),
    val canCarryOn: Boolean = false,
    val canDontWait: Boolean = false,
    val upNext: RoomUpNext? = null,
    val relays: List<RelayHealth> = emptyList(),
    val moveCandidates: List<MoveCandidate> = emptyList(),
)

/**
 * A room an invite link opened, before this device joins: who invited, what it watches, who is
 * there, and whether it is past the soft cap of 10. Nobody sees this device until it joins.
 */
data class RoomInvited(val by: String, val title: String, val episode: String, val names: List<String>) {
    val over: Boolean get() = names.size >= ROOM_SOFT_CAP
}

/** Rooms work best with 10 or fewer: past it, the invite warns and Join anyway lets people in. */
const val ROOM_SOFT_CAP = 10

/** The seconds a wait runs before Carry on comes forward. */
const val CARRY_ON_AFTER_S = 20
