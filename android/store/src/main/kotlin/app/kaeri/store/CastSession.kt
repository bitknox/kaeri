// The cast session: a TV held across plays. Connecting holds it, every play goes to it while it
// is held, and the receiver stays up between plays. The engine's sender holds the rules
// (`cast_connect`, `cast_watch`); this file reads its views for the cast button, the mini and the
// notification, and stands the fixtures' scripted session in for it.

package app.kaeri.store

import app.kaeri.engine.core.AnilistId
import app.kaeri.engine.ffi.FfiCastButton
import app.kaeri.engine.ffi.FfiCastFrom
import app.kaeri.engine.ffi.FfiCastItem
import app.kaeri.engine.ffi.FfiCastSessionState
import app.kaeri.engine.ffi.FfiCastSessionView
import app.kaeri.engine.ffi.FfiCastSessionWatch
import app.kaeri.engine.ffi.FfiTvCommand
import app.kaeri.engine.ffi.FfiTvSender
import app.kaeri.engine.ffi.FfiVolumeControl
import app.kaeri.engine.model.Msg

/** Where the session is, as the engine says it. */
enum class CastSessionState {
    Idle,

    /** The receiver is launching on the TV. */
    Connecting,

    /** The receiver is up and nothing plays. */
    Connected,

    /** Playing, starting, paused or waiting for the stream. */
    Playing,

    /** The TV or the network ended it; [CastSession.notice] says why. */
    Error,
}

/** The cast button in the top bars. */
enum class CastButtonState {
    /** No TV found, or TVs turned off in Settings. */
    Hidden,
    Available,
    Connecting,
    Connected,
}

/** What plays on the held TV. */
data class CastItem(
    /** `null` for media from elsewhere, in a session joined again. */
    val id: AnilistId?,
    val title: String,
    val episode: String?,
    val episodeTitle: String?,
    val image: String?,
    val position: Double,
    val duration: Double?,
    val paused: Boolean,
    /** A start or a reload: the starting state. */
    val loading: Boolean,
    /** Whole seconds the TV has waited for the stream mid-file, or `null`. */
    val waiting: Long?,
)

/** The session as the engine last drew it. */
data class CastSession(
    val state: CastSessionState,
    /** The engine's own word for the button, once it gives one. */
    val button: CastButtonState?,
    val rev: ULong,
    val deviceKey: String?,
    val deviceName: String?,
    val item: CastItem?,
    val queue: List<CastItem>,
    val notice: Msg?,
    /** A note while it plays, said once as it first reads differently. */
    val note: Msg?,
    /** The TV's volume, 0 to 1. */
    val level: Double?,
    val muted: Boolean?,
    /** The TV takes a level; otherwise its volume is fixed and Mute is all there is. */
    val levelControl: Boolean,
    /**
     * The connection to the TV closed and is made again, for the engine's 20 s: the mini and the
     * remote say Reconnecting, and [state] and [item] stay as they were until it joins again or
     * the session ends.
     */
    val reconnecting: Boolean = false,
) {
    /** A TV is held: plays go to it. */
    val held: Boolean get() = state == CastSessionState.Connecting || state == CastSessionState.Connected || state == CastSessionState.Playing

    companion object {
        val IDLE = CastSession(CastSessionState.Idle, null, 0u, null, null, null, emptyList(), null, null, null, null, false)
    }
}

/** Where an episode playing on this device was, for the TV to take it on, or for Play here. */
data class CastFrom(val id: AnilistId, val episode: String, val position: Double)

/** The cast session as [CastModel] drives it: the engine's, or the fixtures' scripted one. */
interface CastSessionSource {
    fun state(): CastSession

    /** The session once it reads differently; `null` once the engine has stopped. */
    suspend fun next(): CastSession?

    /**
     * Hold the Cast row [key]'s TV; with [from], the episode playing here goes on there. Throws
     * the engine's [app.kaeri.engine.ffi.FfiTvException].
     */
    fun connect(key: String, from: CastFrom? = null)

    /** Stop what plays on the TV, close the receiver and let the TV go. */
    fun disconnect()

    fun command(command: FfiTvCommand)

    /** Play [request] on the held TV. */
    fun play(request: TvPlayRequest)

    /** Whether [playNext] queues: the engine's queue call is there. */
    val queues: Boolean

    /** Queue an episode after what plays: `false` where the engine cannot yet. */
    fun playNext(id: AnilistId, episode: String): Boolean

    /** The TV stops and is let go: where it was, for this device to play on from. */
    fun playHere(): CastFrom?

    /** The Cast row key of the TV a session was left running on. */
    fun leftOn(): String?

    /** The controller's card for what plays now: tracks, Up next, skip and the notices. */
    fun card(): CastLink?
}

/** The engine's session, read through [FfiTvSender]'s watch. */
class EngineCastSession(
    private val sender: FfiTvSender,
    private val watch: FfiCastSessionWatch,
    /** Send the engine its play: while a TV is held, it goes there. */
    private val send: (id: AnilistId, episode: String, fromStart: Boolean) -> Unit,
) : CastSessionSource {
    override fun state(): CastSession = watch.state().shown()

    override suspend fun next(): CastSession? = watch.next()?.shown()

    override fun connect(key: String, from: CastFrom?) =
        sender.castConnect(key, from?.let { FfiCastFrom(it.id, it.episode, it.position) })

    override fun disconnect() = sender.castDisconnect()

    override fun command(command: FfiTvCommand) = sender.castCommand(command)

    override fun play(request: TvPlayRequest) = send(request.id, request.episode, request.from == 0.0)

    override val queues: Boolean = false

    override fun playNext(id: AnilistId, episode: String): Boolean = false

    override fun playHere(): CastFrom? = sender.castPlayHere()?.let { CastFrom(it.id, it.episode, it.position) }

    override fun leftOn(): String? = sender.castLeftOn()

    override fun card(): CastLink? = sender.castSession()?.let(::EngineCastLink)
}

private fun FfiCastSessionView.shown() = CastSession(
    state = when (state) {
        FfiCastSessionState.IDLE -> CastSessionState.Idle
        FfiCastSessionState.CONNECTING -> CastSessionState.Connecting
        FfiCastSessionState.CONNECTED -> CastSessionState.Connected
        FfiCastSessionState.PLAYING -> CastSessionState.Playing
        FfiCastSessionState.ERROR -> CastSessionState.Error
    },
    button = when (button) {
        FfiCastButton.HIDDEN -> CastButtonState.Hidden
        FfiCastButton.AVAILABLE -> CastButtonState.Available
        FfiCastButton.CONNECTING -> CastButtonState.Connecting
        FfiCastButton.CONNECTED -> CastButtonState.Connected
    },
    rev = rev,
    deviceKey = deviceKey,
    deviceName = deviceName,
    item = item?.shown(),
    queue = queue.map { it.shown() },
    notice = notice,
    note = note,
    level = level,
    muted = muted,
    levelControl = volumeControl == FfiVolumeControl.LEVEL,
    reconnecting = reconnecting,
)

private fun FfiCastItem.shown() = CastItem(
    id = id,
    title = title,
    episode = episode,
    episodeTitle = episodeTitle,
    image = image,
    position = position,
    duration = duration,
    paused = paused,
    loading = loading,
    waiting = waiting?.toLong(),
)
