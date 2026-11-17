// Casting to TVs with Chromecast built in: the Chromecast rows of the Play on TV picker, and the
// card of the cast running now.
//
// The engine's sender holds every rule, as it does for Play on TV: which Cast devices the picker
// lists, how a cast starts, falls back and ends, and what the card reads. This model keeps its
// latest answers on the main thread for the screens, hands the platform's `_googlecast._tcp`
// browse to it (NsdManager in the app, made-up records in the fixtures), and says when the card
// starts, fails, plays and ends.

package app.kaeri.store

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import app.kaeri.engine.core.AnilistId
import app.kaeri.engine.ffi.FfiCastBrowser
import app.kaeri.engine.ffi.FfiCastCard
import app.kaeri.engine.ffi.FfiCastRow
import app.kaeri.engine.ffi.FfiCastSession
import app.kaeri.engine.ffi.FfiLinkPhase
import app.kaeri.engine.ffi.FfiNowCard
import app.kaeri.engine.ffi.FfiRemoteCard
import app.kaeri.engine.ffi.FfiScreen
import app.kaeri.engine.ffi.FfiTvCommand
import app.kaeri.engine.ffi.FfiTvException
import app.kaeri.engine.ffi.FfiTvRowKind
import app.kaeri.engine.ffi.FfiVolumeControl
import app.kaeri.engine.ffi.tvPlayhead
import app.kaeri.engine.model.Msg

/**
 * A cast as the card reads it and drives it: the engine's [FfiCastSession], or the fixtures'
 * scripted TV.
 */
interface CastLink {
    val tvName: String

    /** The card as it stands. */
    fun card(): FfiCastCard

    /** The card once it may read differently; `null` once the engine has stopped. */
    suspend fun nextCard(): FfiCastCard?

    fun command(command: FfiTvCommand)

    /** Stop casting, and the position to play on from here when it is past the first seconds. */
    fun playHere(): Double?

    /** Up next's Play now. */
    fun playNext()

    /** Up next's Cancel: the episode does not roll on. */
    fun cancelNext()

    /** The Undo note's button: back to where an automatic skip began. */
    fun undoSkip()

    /** Cast the same episode to the same TV again, from its position; `false` when it cannot. */
    fun tryAgain(): Boolean

    fun stop()
}

/** The engine's cast session as a [CastLink]. */
class EngineCastLink(private val session: FfiCastSession) : CastLink {
    override val tvName: String = session.deviceName()

    override fun card(): FfiCastCard = session.card()

    override suspend fun nextCard(): FfiCastCard? = session.nextCard()

    override fun command(command: FfiTvCommand) = session.command(command)

    override fun playHere(): Double? = session.playHere()

    override fun playNext() = session.playNext()

    override fun cancelNext() = session.cancelNext()

    override fun undoSkip() = session.undoSkip()

    override fun tryAgain(): Boolean = session.tryAgain()

    override fun stop() = session.stop()
}

/** Where a cast is, as the card draws it. */
enum class CastStage {
    /** Connecting, opening the receiver, preparing the stream or waiting for the TV. */
    Starting,

    /** The TV plays the episode, paused or not. */
    Playing,

    /** Nothing played: every way of playing it ran out. The card says why until dismissed. */
    Failed,

    /** The TV dropped off mid-cast. The card says so until dismissed. */
    Lost,
}

/**
 * The cast card: the engine's card, when on [CastModel]'s clock it came for the playhead, what was
 * asked of the TV, and when, for the starting line's counter.
 */
data class CastShown(
    val cast: FfiCastCard,
    val at: Long,
    val tvName: String,
    val request: TvPlayRequest,
    val startedAt: Long,
    /** Whether the TV played this cast at some point: one that ends after that stopped, not failed to start. */
    val played: Boolean = false,
    /** The picker row of the TV it plays on, when the cast started from one. */
    val key: String? = null,
    /**
     * Where the playhead stopped as the TV went quiet: reconnecting, or waiting while the engine
     * learns why the receiver left. The clock holds it until the TV says where it is again.
     */
    val frozen: Double? = null,
) {
    val stage: CastStage
        get() {
            val notice = cast.notice
            val now = cast.card.now
            return when {
                notice is Msg.NoticeCastLost -> CastStage.Lost
                notice != null && played -> CastStage.Lost
                notice != null -> CastStage.Failed
                now == null || now.loading -> CastStage.Starting
                else -> CastStage.Playing
            }
        }

    /** The TV takes a volume level; otherwise its own volume is in charge and the card has Mute alone. */
    val level: Boolean get() = cast.volumeControl == FfiVolumeControl.LEVEL

    /** The cast starts or plays: not a card that says why it stopped. */
    val live: Boolean get() = stage == CastStage.Starting || stage == CastStage.Playing

    /** The connection to the TV closed and the engine makes it again: everything but Stop casting waits. */
    val reconnecting: Boolean get() = live && cast.reconnecting

    /** Seconds the TV has waited, for the stream or while the engine learns why it stopped; `null` while it plays. */
    val waiting: UInt? get() = cast.waiting.takeIf { live }

    /** The TV went quiet: its clock holds still. */
    val quiet: Boolean get() = reconnecting || waiting != null
}

/**
 * A cast that ended with something to say once the card has gone: someone else took the TV. The
 * notice offers to play on here, from where the TV was.
 */
data class CastEnded(val notice: Msg, val id: AnilistId?, val episode: String?)

/** The cast button's dialogs: the chooser while no TV is held, the TV's own once one is. */
enum class CastDialog { Chooser, Connected }

/**
 * Casting's state, on the main thread.
 *
 * @param clock milliseconds, for the playhead to move on from the card's time and the starting
 *   line to count.
 */
class CastModel(
    private val scope: CoroutineScope,
    private val clock: () -> Long = { System.nanoTime() / 1_000_000 },
) {
    private val _rows = MutableStateFlow<List<FfiCastRow>>(emptyList())

    /** The picker's Chromecast rows, while it is open. */
    val rows: StateFlow<List<FfiCastRow>> = _rows.asStateFlow()

    private val _blocked = MutableStateFlow(false)

    /** Android 17's local network permission was turned down: the picker says so, with Open settings. */
    val blocked: StateFlow<Boolean> = _blocked.asStateFlow()

    private val _shown = MutableStateFlow<CastShown?>(null)

    /** The card of the cast running now, or of the one that failed or dropped off until dismissed. */
    val shown: StateFlow<CastShown?> = _shown.asStateFlow()

    private val _open = MutableStateFlow(false)

    /** The card's sheet is up. */
    val open: StateFlow<Boolean> = _open.asStateFlow()

    private val _ended = MutableSharedFlow<CastEnded>(extraBufferCapacity = 1)

    /** Casts that ended with a notice once their card had gone, for the snackbar. */
    val ended: SharedFlow<CastEnded> = _ended.asSharedFlow()

    private val _notes = MutableSharedFlow<Msg>(extraBufferCapacity = 1)

    /** A note while a TV plays, once as it first reads differently: a toast. */
    val notes: SharedFlow<Msg> = _notes.asSharedFlow()

    /** Each cast notice, also kept in the notice log: notes, and why a cast ended. */
    var kept: ((Msg) -> Unit)? = null

    private val _problem = MutableStateFlow<String?>(null)

    /** The engine's words for a cast that could not even be asked for, as the TV left the network. */
    val problem: StateFlow<String?> = _problem.asStateFlow()

    private val _session = MutableStateFlow(CastSession.IDLE)

    /** The held TV's session, as the engine last drew it. */
    val session: StateFlow<CastSession> = _session.asStateFlow()

    /**
     * A cast starts or plays: the stream goes out through this device, so it stays up and its
     * torrents run with the screen off, as for an episode playing here.
     */
    private val _casting = MutableStateFlow(false)
    val casting: StateFlow<Boolean> = _casting.asStateFlow()

    /** Kept with the card and the session as they change, with no coroutine of its own. */
    private fun recast() {
        _casting.value = _shown.value?.live == true || _session.value.state == CastSessionState.Playing
        if (_shown.value?.live == true) _joining.value = null
    }

    private val _button = MutableStateFlow(CastButtonState.Hidden)

    /** The cast button in the top bars. */
    val button: StateFlow<CastButtonState> = _button.asStateFlow()

    private val _dialog = MutableStateFlow<CastDialog?>(null)

    /** The cast button's dialog, while it is up. */
    val dialog: StateFlow<CastDialog?> = _dialog.asStateFlow()

    private val _joining = MutableStateFlow<String?>(null)

    /** The name of the TV a session was left running on, while it is joined again. */
    val joining: StateFlow<String?> = _joining.asStateFlow()

    private val _lastItem = MutableStateFlow<CastItem?>(null)

    /** What the session played last, for the mini while it joins again. */
    val lastItem: StateFlow<CastItem?> = _lastItem.asStateFlow()

    /**
     * Plays an episode on this device: Play here, once the cast has stopped. The app sends the
     * engine its play, which resumes where the TV left off.
     */
    var playHere: ((AnilistId, String) -> Unit)? = null

    /** Sends the engine a play, which goes to the held TV while one is held. */
    var sendPlay: ((id: AnilistId, episode: String, fromStart: Boolean) -> Unit)? = null

    /** Called once the TV plays what the player here was playing: the app closes its player. */
    var handedOff: (() -> Unit)? = null

    /**
     * How a row's cast starts: the engine's sender, unless a fixtures scene stands a scripted TV
     * in. `null` when the engine said no, with its words in [problem].
     */
    var starter: ((key: String, request: TvPlayRequest) -> CastLink?)? = null

    /** The session's source: the engine's watch, or the fixtures' scripted session. */
    var sessions: CastSessionSource? = null
        set(value) {
            field = value
            watching?.cancel()
            watching = value?.let(::watch)
        }

    private var tv: TvRemoteModel? = null
    private var browser: FfiCastBrowser? = null
    private var browsers = 0
    private val browsing get() = browsers > 0
    private var watching: Job? = null
    private var foreground = false

    /** Whether the browses that run while the app is in front were started. */
    private var browsingInFront = false
    private var sessionAt = clock()
    private var pendingGuard: Job? = null
    private var link: CastLink? = null
    private var cards: Job? = null

    /** The session of a cast that failed or dropped off, kept for Try again until the card is dismissed. */
    private var over: CastLink? = null

    /** The last ending the snackbar said, so the card's end and the session's end say it once. */
    private var told: CastEnded? = null

    /** Where the session's item stopped as its connection closed, while it is made again. */
    private var sessionFrozen: Double? = null

    /** Whether casting is there: the engine's sender came with a Cast browse to give it. */
    val available: Boolean get() = tv?.attachedSender != null && browser != null

    /**
     * Cast through [tv]'s sender, browsing with [browser] while the picker is up. The sender's
     * changes reach the rows through [tv], which watches them.
     */
    fun attach(tv: TvRemoteModel, browser: FfiCastBrowser) {
        this.tv = tv
        this.browser = browser
        tv.changed = {
            refresh()
            refreshButton()
        }
        if (starter == null) {
            starter = { key, request ->
                val sender = tv.attachedSender
                try {
                    sender?.cast(key, request.id, request.episode, request.from)?.let(::EngineCastLink)
                } catch (e: FfiTvException) {
                    _problem.value = TvRemoteModel.words(e)
                    TvRemoteModel.relookAfter(e, this.browser)
                    null
                }
            }
        }
        val sender = tv.attachedSender
        if (sessions == null && sender != null) {
            sender.castWatch()?.let { watch ->
                sessions = EngineCastSession(sender, watch) { id, episode, fromStart -> sendPlay?.invoke(id, episode, fromStart) }
            }
        }
        refresh()
        browseInFront()
    }

    internal fun refresh() {
        val sender = tv?.attachedSender ?: return
        takeRows(if (browsing) sender.castRows() else emptyList())
    }

    /** The TVs the browse found now. */
    internal fun takeRows(rows: List<FfiCastRow>) {
        _rows.value = rows
        refreshJoining()
        refreshButton()
    }

    /** A session left running is joined again once its TV is found: the mini says so meanwhile. */
    private fun refreshJoining() {
        _joining.value = joining(_session.value, sessions?.leftOn(), _rows.value, _shown.value?.live == true)
    }

    /**
     * Look for TVs with Chromecast built in while the picker is up. The picker's own browse is
     * started first: it stops this one with it as the picker closes.
     */
    fun startBrowsing() {
        val sender = tv?.attachedSender ?: return
        val browser = browser ?: return
        browsers += 1
        if (browsers == 1) sender.browseCast(browser)
        refresh()
    }

    fun stopBrowsing() {
        if (browsers == 0) return
        browsers -= 1
        if (browsers == 0) _rows.value = emptyList()
        refreshButton()
    }

    /**
     * The app came to the front or left it. In front, both browses run, so the button knows
     * whether a TV is there and a session left running is joined again.
     */
    fun foreground(on: Boolean) {
        if (on == foreground) return
        foreground = on
        browseInFront()
    }

    /**
     * Start or stop the browses that run while the app is in front. Casting may come up after
     * the app is already in front, as at a cold launch: the browses start then.
     */
    private fun browseInFront() {
        val tv = tv ?: return
        val want = foreground && available
        if (want == browsingInFront) return
        browsingInFront = want
        if (want) {
            tv.startBrowsing()
            startBrowsing()
        } else {
            stopBrowsing()
            tv.stopBrowsing()
        }
    }

    /** Browse again from the start, as after the local network was allowed. */
    fun browseAgain() {
        val sender = tv?.attachedSender ?: return
        val browser = browser ?: return
        if (browsing) sender.browseCast(browser)
    }

    // The session

    /** Follow [source]'s views until the engine stops. */
    private fun watch(source: CastSessionSource) = scope.launch {
        takeSession(source.state())
        while (isActive) {
            val next = source.next() ?: break
            takeSession(next)
        }
    }

    /** A view of the session: the button, the mini and the controller's card follow it. */
    internal fun takeSession(view: CastSession) {
        val was = _session.value
        val wasAt = sessionAt
        sessionAt = clock()
        _session.value = view
        recast()
        view.note?.let { if (it != was.note) tellNote(it) }
        if (view.reconnecting && !was.reconnecting) sessionFrozen = itemPlayhead(sessionAt, was, wasAt)
        if (!view.reconnecting) sessionFrozen = null
        view.item?.let { _lastItem.value = it }
        refreshJoining()
        refreshButton()
        when {
            view.state == CastSessionState.Playing && link == null && _shown.value?.stage.let { it == null || it == CastStage.Starting } -> attachCard(view)
            !view.held && was.held -> {
                if (_dialog.value == CastDialog.Connected) _dialog.value = null
                val notice = view.notice.takeIf { view.state == CastSessionState.Error }
                val shown = _shown.value
                when {
                    // The controller is up: its card says why, with Try again, until it is dismissed.
                    notice != null && shown != null && _open.value -> if (shown.live) stopsWith(shown, notice)
                    else -> {
                        if (notice != null) {
                            tellEnded(CastEnded(notice, was.item?.id ?: shown?.request?.id, was.item?.episode ?: shown?.request?.episode))
                        }
                        // A card that says why it ended goes with the session, as one still starting does.
                        if (shown != null && (shown.live || notice != null)) end()
                    }
                }
            }
        }
    }

    /** Say [note] as a toast and keep it in the notice log. */
    private fun tellNote(note: Msg) {
        _notes.tryEmit(note)
        kept?.invoke(note)
    }

    /** The live card [shown] turns into one that says why the cast stopped: [notice]. */
    private fun stopsWith(shown: CastShown, notice: Msg) {
        pendingGuard?.cancel()
        pendingGuard = null
        cards?.cancel()
        cards = null
        over = link
        link = null
        _shown.value = shown.copy(cast = shown.cast.copy(card = shown.cast.card.copy(over = true), notice = notice, reconnecting = false, waiting = null))
        recast()
        kept?.invoke(notice)
    }

    /** The cast ended with [ended] to say once its card has gone: the snackbar, said once. */
    private fun tellEnded(ended: CastEnded) {
        if (ended == told) return
        told = ended
        _ended.tryEmit(ended)
        kept?.invoke(ended.notice)
    }

    /** The controller's card for what the held TV plays, from the engine's cast. */
    private fun attachCard(view: CastSession) {
        val next = sessions?.card() ?: return
        val item = view.item
        val held = _shown.value
        val request = held?.request ?: TvPlayRequest(
            id = item?.id ?: return,
            title = item.title,
            episode = item.episode ?: return,
        )
        pendingGuard?.cancel()
        link = next
        val now = clock()
        _shown.value = CastShown(next.card(), now, view.deviceName ?: next.tvName, request, held?.startedAt ?: now)
        recast()
        follow(next, request)
    }

    /**
     * The button as the engine says it for Chromecast TVs, joined with the TVs that run Kaeri,
     * which only this model knows: one found shows the button, one connected fills it.
     */
    private fun refreshButton() {
        val view = _session.value
        val engine = view.button ?: when {
            view.state == CastSessionState.Connecting -> CastButtonState.Connecting
            view.held -> CastButtonState.Connected
            _rows.value.isNotEmpty() -> CastButtonState.Available
            else -> CastButtonState.Hidden
        }
        val kaeri = tv?.rows?.value.orEmpty().any { it.kind != FfiTvRowKind.PAIRED_AWAY }
        // A cast from Play on TV runs without the session: its card says where it plays.
        val linked = if (link != null) _shown.value?.stage else null
        _button.value = when {
            engine == CastButtonState.Connecting || engine == CastButtonState.Connected -> engine
            linked == CastStage.Starting -> CastButtonState.Connecting
            linked == CastStage.Playing -> CastButtonState.Connected
            tv?.connected?.value != null -> CastButtonState.Connected
            engine == CastButtonState.Available || kaeri -> CastButtonState.Available
            else -> CastButtonState.Hidden
        }
    }

    /** Tell the engine what Kaeri's receiver calls this device: `phone` or `tablet`. */
    fun setDevice(word: String) {
        tv?.attachedSender?.castSetDevice(word)
    }

    /**
     * The cast button, in the top bars, the mini and the remote: the held TV's dialog, also for a
     * cast from Play on TV, which holds its TV without a session. A Kaeri TV opens its own remote,
     * and with no TV held the chooser opens.
     */
    fun pressButton() {
        val tv = tv
        when {
            _session.value.held || linked -> _dialog.value = CastDialog.Connected
            tv?.connected?.value != null -> tv.openRemote()
            else -> _dialog.value = CastDialog.Chooser
        }
    }

    /** A cast from Play on TV starts or plays: it holds its TV with no session. */
    private val linked: Boolean get() = link != null && _shown.value?.live == true

    /** The held TV's dialog goes once nothing holds the TV any more. */
    private fun letDialogGo() {
        if (_dialog.value == CastDialog.Connected && !_session.value.held && !linked) _dialog.value = null
    }

    fun closeDialog() {
        _dialog.value = null
    }

    /** Hold the Chromecast row [row]'s TV. Nothing plays until something is played. */
    fun connect(row: FfiCastRow) {
        _dialog.value = null
        _problem.value = null
        try {
            sessions?.connect(row.key)
        } catch (e: FfiTvException) {
            _problem.value = TvRemoteModel.words(e)
            TvRemoteModel.relookAfter(e, browser)
        }
    }

    /** A Kaeri TV row in the chooser: its own pairing and remote take over. */
    fun choose(row: app.kaeri.engine.ffi.FfiTvRow) {
        _dialog.value = null
        tv?.choose(row)
    }

    /** Stop casting: what plays on the TV stops, the receiver closes and the TV is let go. */
    fun disconnect() {
        _dialog.value = null
        _joining.value = null
        if (_session.value.held) sessions?.disconnect() else link?.stop()
        end()
    }

    /** Plays go to the held TV: a TV is held, or a session is joined again. */
    val held: Boolean get() = _session.value.held

    /**
     * Play [request] on the held TV: the controller comes up at once on its starting state, and
     * the engine's card takes over once the TV starts. `false` when no TV is held.
     */
    fun playOnTv(request: TvPlayRequest): Boolean {
        val source = sessions ?: return false
        if (!_session.value.held) return false
        val name = _session.value.deviceName.orEmpty()
        cards?.cancel()
        cards = null
        link = null
        over = null
        told = null
        val now = clock()
        _shown.value = CastShown(starting(name, _session.value.levelControl), now, name, request, now)
        recast()
        _open.value = true
        source.play(request)
        // A play the engine could not start never reaches the TV: the controller lets it go.
        pendingGuard?.cancel()
        pendingGuard = scope.launch {
            delay(PENDING_FOR)
            if (link == null && _shown.value?.request == request) end()
        }
        return true
    }

    /** "Play next on Living room": queued after what plays. `false` where the engine cannot. */
    fun queueNext(id: AnilistId, episode: String): Boolean =
        _session.value.state == CastSessionState.Playing && sessions?.playNext(id, episode) == true

    /** Whether an episode can be queued on the held TV now. */
    val canQueue: Boolean get() = sessions?.queues == true

    /**
     * What Android said of the local network permission: turned down, the picker says so in
     * place of its rows; allowed, both browses start again.
     */
    fun localNetwork(allowed: Boolean) {
        val was = _blocked.value
        _blocked.value = !allowed
        if (allowed && was) {
            tv?.browseAgain()
            browseAgain()
        }
    }

    /** Cast [request] to the Chromecast row [row]: the picker closes and the card opens on its starting state. */
    fun cast(row: FfiCastRow, request: TvPlayRequest) = cast(row.key, request)

    private fun cast(key: String, request: TvPlayRequest) {
        _problem.value = null
        val next = starter?.invoke(key, request) ?: return
        end()
        told = null
        link = next
        val now = clock()
        _shown.value = CastShown(next.card(), now, next.tvName, request, now, key = key)
        recast()
        _open.value = true
        tv?.sheetClosed()
        refreshButton()
        follow(next, request)
    }

    /** Take the cards [next] sends until the cast is over. */
    private fun follow(next: CastLink, request: TvPlayRequest) {
        cards = scope.launch {
            var handed = !request.fromPlayer
            while (isActive) {
                val card = next.nextCard() ?: break
                if (link !== next) break
                take(card)
                if (!handed && _shown.value?.stage == CastStage.Playing) {
                    handed = true
                    handedOff?.invoke()
                }
            }
        }
    }

    /**
     * The card the engine sent. The playhead moves on from where and when it said; a cast that is
     * over leaves its card up when it has something to say there, and goes, with a notice for the
     * snackbar, when someone else took the TV.
     */
    private fun take(card: FfiCastCard) {
        val held = _shown.value ?: return
        val same = held.cast.card.now?.position == card.card.now?.position && held.cast.card.now?.paused == card.card.now?.paused
        val playing = card.notice == null && card.card.now?.loading == false
        // The TV went on to another episode, as a joined cast does when the device that started it casts the next.
        val tvNow = card.card.now
        val request = if (tvNow != null && (tvNow.id != held.request.id || tvNow.episode != held.request.episode)) {
            held.request.copy(id = tvNow.id, title = tvNow.title, episode = tvNow.episode, from = null)
        } else {
            held.request
        }
        val next = held.copy(cast = card, at = if (same) held.at else clock(), played = held.played || playing, request = request)
        // The clock stops where it was the moment the TV went quiet, and runs again once it speaks.
        val frozen = when {
            !next.quiet -> null
            held.frozen != null -> held.frozen
            else -> playhead(clock(), held)
        }
        _shown.value = next.copy(frozen = frozen)
        recast()
        if (held.stage != _shown.value?.stage) refreshButton()
        // A cast from Play on TV holds no session, so its card carries the note; a held session says it itself.
        card.note?.let { if (it != held.cast.note && !sessionNotes) tellNote(it) }
        if (!card.card.over) return
        val notice = card.notice
        val now = card.card.now
        when {
            notice is Msg.NoticeCastTakenOver -> {
                end()
                tellEnded(CastEnded(notice, now?.id ?: held.request.id, now?.episode ?: held.request.episode))
            }
            // Stopped by the viewer, here or from the notification.
            notice == null -> end()
            // Failed or lost with the controller down: the snackbar says so, with Play here.
            !_open.value -> {
                (browser as? NsdRelook)?.relook()
                end()
                tellEnded(CastEnded(notice, now?.id ?: held.request.id, now?.episode ?: held.request.episode))
            }
            // Failed or lost: the card says so until it is dismissed. The TV may have moved, which
            // a browse below Android 14 finds only by resolving it again.
            else -> {
                kept?.invoke(notice)
                (browser as? NsdRelook)?.relook()
                cards?.cancel()
                cards = null
                over = link
                link = null
                refreshButton()
                letDialogGo()
            }
        }
    }

    /** The session's view carries the note, as it does while it plays or is connected. */
    private val sessionNotes: Boolean
        get() = _session.value.state.let { it == CastSessionState.Playing || it == CastSessionState.Connected }

    /** Let the cast and its card go. */
    private fun end() {
        pendingGuard?.cancel()
        pendingGuard = null
        cards?.cancel()
        cards = null
        link = null
        over = null
        _shown.value = null
        recast()
        _open.value = false
        refreshButton()
        letDialogGo()
    }

    // The card

    fun openCard() {
        if (_shown.value != null) _open.value = true
    }

    fun closeCard() {
        _open.value = false
    }

    /** Send the TV a command from the card, the notification or the volume keys. */
    fun send(command: FfiTvCommand) {
        val source = sessions
        if (source != null && _session.value.held) source.command(command) else link?.command(command)
    }

    /** Mute the TV, or bring its sound back. */
    fun toggleMute() {
        val muted = _shown.value?.cast?.muted ?: _session.value.muted ?: false
        send(FfiTvCommand.Mute(!muted))
    }

    /** Move a TV's volume level to [level] out of 100, in the steps the engine sends. */
    fun setVolume(level: Double) {
        val volume = _shown.value?.cast?.card?.now?.volume ?: return
        val step = level - volume
        if (step != 0.0) send(FfiTvCommand.Volume(step))
    }

    /** A step of the volume keys, out of 100: nothing on a TV whose volume is fixed. */
    fun stepVolume(up: Boolean) {
        if (_shown.value?.level != true && !(_session.value.held && _session.value.levelControl)) return
        send(FfiTvCommand.Volume(if (up) VOLUME_STEP else -VOLUME_STEP))
    }

    /** Stop the TV and play on here, from where it was. */
    fun playHere() {
        val shown = _shown.value
        val source = sessions
        if (source != null && _session.value.held) {
            val from = source.playHere()
            end()
            val id = from?.id ?: shown?.request?.id ?: return
            playHere?.invoke(id, from?.episode ?: shown?.request?.episode ?: return)
            return
        }
        shown ?: return
        link?.playHere()
        val now = shown.cast.card.now
        end()
        playHere?.invoke(now?.id ?: shown.request.id, now?.episode ?: shown.request.episode)
    }

    /** Stop casting: the receiver quits on the TV, the card and the bar go. */
    fun stop() = disconnect()

    /** The card of a cast that failed or dropped off, dismissed. */
    fun dismiss() {
        if (link == null) end()
    }

    /** Up next's Play now: the next episode starts on the TV at once. */
    fun playNext() {
        link?.playNext()
    }

    /** Up next's Cancel: the episode does not roll on. */
    fun cancelNext() {
        link?.cancelNext()
    }

    /** Undo after an automatic skip. */
    fun undoSkip() {
        link?.undoSkip()
    }

    /**
     * Start the same episode on the same TV again from its position, as Try again on a failed
     * card asks. The card starts over.
     */
    fun tryAgain() {
        val shown = _shown.value ?: return
        val again = over ?: return
        if (!again.tryAgain()) return
        over = null
        link = again
        val now = clock()
        _shown.value = shown.copy(cast = again.card(), at = now, startedAt = now, played = false)
        recast()
        refreshButton()
        follow(again, shown.request)
    }

    /**
     * Choose another TV for the episode playing here: the picker, with the episode at the TV's
     * position. The cast goes on until another TV takes it.
     */
    fun chooseAnother() {
        val shown = _shown.value ?: return
        val tv = tv ?: return
        val now = shown.cast.card.now
        _open.value = false
        tv.offer(
            TvPlayRequest(
                id = now?.id ?: shown.request.id,
                title = now?.title ?: shown.request.title,
                episode = now?.episode ?: shown.request.episode,
                from = playhead(),
            ),
        )
    }

    /** Where the TV's playhead is now: moved on from the card on this device's clock while it plays. */
    fun playhead(at: Long = clock()): Double? = playhead(at, _shown.value)

    private fun playhead(at: Long, shown: CastShown?): Double? {
        shown ?: return null
        val now = shown.cast.card.now ?: return null
        shown.frozen?.let { return it }
        if (shown.stage != CastStage.Playing || shown.quiet) return now.position
        return tvPlayhead(now, ((at - shown.at).coerceAtLeast(0)) / 1000.0)
    }

    /** Where the session's item is now, moved on from its view on this device's clock while it plays. */
    fun itemPlayhead(at: Long = clock()): Double? = sessionFrozen ?: itemPlayhead(at, _session.value, sessionAt)

    private fun itemPlayhead(at: Long, view: CastSession, since: Long): Double? {
        val item = view.item ?: return null
        if (item.paused || item.loading || item.waiting != null || view.reconnecting) return item.position
        val moved = item.position + (at - since).coerceAtLeast(0) / 1000.0
        return item.duration?.let { moved.coerceAtMost(it) } ?: moved
    }

    /** Whole seconds since the cast was asked for, for the starting line's counter. */
    fun startingSeconds(at: Long = clock()): Long {
        val shown = _shown.value ?: return 0
        return ((at - shown.startedAt).coerceAtLeast(0)) / 1000
    }

    companion object {
        /**
         * The TV a session was left running on, by name, while it is joined again: only with no
         * session at all and no cast of this device's [live] outside it, which is what plays on
         * that TV then. One that ended with a notice says so, and is looked for again once the
         * browse finds its TV anew, never as a Reconnecting that waits on nothing.
         */
        internal fun joining(session: CastSession, leftOn: String?, rows: List<FfiCastRow>, live: Boolean = false): String? {
            if (session.state != CastSessionState.Idle || live) return null
            return leftOn?.let { left -> rows.firstOrNull { it.key == left }?.name }
        }

        /** The volume keys' step, out of the TV's 100. */
        const val VOLUME_STEP = 5.0

        /** Past this many seconds the starting line counts. */
        const val COUNT_FROM_SECONDS = 4L

        /** How long a play on the held TV may take to start before the controller lets it go. */
        const val PENDING_FOR = 45_000L

        /** The controller's card before the engine's: the TV starting what was asked. */
        fun starting(tvName: String, level: Boolean) = FfiCastCard(
            card = FfiRemoteCard(
                tvName = tvName,
                phase = FfiLinkPhase.CONNECTED,
                phaseLine = null,
                screen = FfiScreen.Player,
                now = null,
                rev = 0u,
                over = false,
                reached = true,
            ),
            volumeControl = if (level) FfiVolumeControl.LEVEL else FfiVolumeControl.MUTE_ONLY,
            muted = null,
            notice = null,
            phaseLine = null,
            upNext = null,
            undo = null,
            route = null,
        )

        /** The picker row of the TV that plays now: the held session's, or that of a cast started from the picker. */
        fun playingKey(shown: CastShown?, session: CastSession): String? = when {
            session.held -> session.deviceKey
            shown?.stage == CastStage.Starting || shown?.stage == CastStage.Playing -> shown.key
            else -> null
        }

        /** What the TV plays, as the card shows it: what the engine says, else what was asked while it starts. */
        fun now(shown: CastShown): FfiNowCard = shown.cast.card.now ?: FfiNowCard(
            id = shown.request.id,
            title = shown.request.title,
            episode = shown.request.episode,
            episodeLine = "",
            loading = true,
            position = shown.request.from ?: 0.0,
            duration = null,
            paused = false,
            speed = 1.0,
            volume = null,
            audio = emptyList(),
            subtitles = emptyList(),
            subtitlesOff = false,
            skip = null,
        )
    }
}
