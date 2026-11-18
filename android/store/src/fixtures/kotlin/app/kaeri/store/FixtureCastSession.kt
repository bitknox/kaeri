// The fixtures' cast session: a scripted TV held across plays, standing in for the engine's
// `cast_connect` and `cast_watch`. Connecting takes a moment, plays start on a FixtureCastLink, and
// nothing ever reaches a real TV.

package app.kaeri.store

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import app.kaeri.engine.core.AnilistId
import app.kaeri.engine.ffi.FfiCastCard
import app.kaeri.engine.ffi.FfiTvCommand
import app.kaeri.engine.model.Msg

/**
 * A scripted cast session.
 *
 * @param link the scripted TV a play starts on: its name, whether it takes a volume level, and the play.
 * @param episodeTitle an episode's title, for the item's line.
 */
class FixtureCastSession(
    private val scope: CoroutineScope,
    private val link: (name: String, level: Boolean, request: TvPlayRequest) -> FixtureCastLink,
    private val episodeTitle: (AnilistId, String) -> String? = { _, _ -> null },
) : CastSessionSource {
    private val changes = MutableStateFlow(0L)
    private var seen = 0L
    private var current = CastSession.IDLE
    private var rev = 0uL
    private var playing: FixtureCastLink? = null
    private var following: Job? = null
    private var connecting: Job? = null
    private var queue = emptyList<CastItem>()

    /** How long connecting takes; `null` never connects, as the connecting shot holds it. */
    var connectAfter: Long? = 1_200

    /** The row key of a session left running, for the joining shot. */
    var left: String? = null

    /** Seconds the TV has waited for the stream mid-file, as the waiting shot scripts. */
    var waiting: Long? = null
        set(value) {
            field = value
            playing?.let { publish(current.copy(item = item(it.card()))) }
        }

    override val queues: Boolean = true

    override fun state(): CastSession = current

    override suspend fun next(): CastSession? {
        changes.first { it != seen }
        seen = changes.value
        return current
    }

    private fun publish(view: CastSession) {
        rev += 1u
        current = view.copy(rev = rev, queue = queue)
        changes.value += 1
    }

    override fun connect(key: String, from: CastFrom?) {
        val kitchen = key.endsWith("fixture-kitchen")
        val name = if (kitchen) "Kitchen display" else "Living room"
        connecting?.cancel()
        left = null
        publish(CastSession.IDLE.copy(state = CastSessionState.Connecting, deviceKey = key, deviceName = name, levelControl = kitchen, canMute = kitchen, level = if (kitchen) 0.45 else null, muted = false))
        val after = connectAfter ?: return
        connecting = scope.launch {
            delay(after)
            publish(current.copy(state = CastSessionState.Connected))
        }
    }

    override fun disconnect() {
        connecting?.cancel()
        following?.cancel()
        playing?.stop()
        playing = null
        queue = emptyList()
        publish(CastSession.IDLE)
    }

    override fun command(command: FfiTvCommand) {
        val tv = playing
        if (tv != null) {
            tv.command(command)
            return
        }
        when (command) {
            is FfiTvCommand.Mute -> if (current.canMute) publish(current.copy(muted = command.muted))
            is FfiTvCommand.Volume -> if (current.levelControl) publish(current.copy(level = ((current.level ?: 0.0) + command.step / 100).coerceIn(0.0, 1.0)))
            else -> Unit
        }
    }

    override fun play(request: TvPlayRequest) {
        if (!current.held) return
        following?.cancel()
        playing?.stop()
        val tv = link(current.deviceName.orEmpty(), current.levelControl, request)
        playing = tv
        publish(current.copy(state = CastSessionState.Playing, item = item(tv.card())))
        following = scope.launch {
            tv.changed.collect {
                val card = tv.card()
                val notice = card.notice
                when {
                    card.card.over && notice != null && !notice.endsCasting() -> publish(current.copy(item = item(card), reconnecting = false))
                    // As the engine: casting ends, and Play here still has the item from where the TV was.
                    card.card.over && notice != null -> {
                        playing = null
                        publish(CastSession.IDLE.copy(state = CastSessionState.Error, notice = notice, item = item(card)))
                    }
                    card.card.over -> {
                        playing = null
                        publish(current.copy(state = CastSessionState.Connected, item = null))
                    }
                    else -> publish(current.copy(item = item(card), muted = card.muted ?: current.muted, reconnecting = card.reconnecting))
                }
            }
        }
    }

    private fun item(card: FfiCastCard): CastItem? {
        val now = card.card.now ?: return null
        return CastItem(
            id = now.id,
            title = now.title,
            episode = now.episode,
            episodeTitle = episodeTitle(now.id, now.episode),
            image = null,
            position = now.position,
            duration = now.duration,
            paused = now.paused,
            loading = now.loading,
            waiting = waiting ?: card.waiting?.toLong(),
        )
    }

    /** The TV lost, or stopped from outside: the engine ends casting with these. */
    private fun Msg.endsCasting() = this is Msg.NoticeCastLost || this is Msg.NoticeCastStoppedElsewhere

    /** A note on the held TV, such as its own player taking the cast, as the engine publishes one. */
    fun note(msg: Msg) {
        publish(current.copy(note = msg))
    }

    override fun playNext(id: AnilistId, episode: String): Boolean {
        queue = queue + CastItem(id, "", episode, episodeTitle(id, episode), null, 0.0, null, paused = false, loading = false, waiting = null)
        publish(current)
        return true
    }

    override fun playHere(): CastFrom? {
        val item = current.item
        val at = playing?.playHere()
        disconnect()
        return CastFrom(item?.id ?: return null, item.episode ?: return null, at ?: 0.0)
    }

    override fun leftOn(): String? = left

    override fun card(): CastLink? = playing

}
