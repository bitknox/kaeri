// First run's "Already use Kaeri on another device?": the ask, the code this device shows, and
// what the paired device brings. A state on the clock, with no scope of its own: the page asks it
// what shows at a given moment and when that changes next.

package app.kaeri.store

import app.kaeri.engine.model.FirstRunPage
import app.kaeri.engine.model.JoinedSetup
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/** Where the join page stands: the two ways in, the code, or paired with what came over. */
enum class JoinStage { ASK, CODE, PAIRED }

/**
 * The join page's state. Times are on the clock's milliseconds; 0 is not yet.
 * [codeAt] is when the code showed, [until] when it runs out, [pairedAt] when a device paired.
 * [signIns] are the tracker ids the pairing copied, before the setup says its own.
 * [asked] are the stores Continue asked to join, by name, at [askedAt]; [answered] the stores
 * that said yes or no since, by name, and [refused] the ones that said no.
 */
data class JoinState(
    val stage: JoinStage = JoinStage.ASK,
    val code: String? = null,
    val port: Int = 0,
    val codeAt: Long = 0,
    val until: Long = 0,
    val pairedAt: Long = 0,
    val deviceName: String? = null,
    val signIns: List<String>? = null,
    val setup: JoinedSetup? = null,
    val asked: Set<String> = emptySet(),
    val askedAt: Long = 0,
    val answered: Set<String> = emptySet(),
    val refused: List<StoreJoin.Failed> = emptyList(),
    val presses: Int = 0,
) {
    /** The waiting line under the code: 600 ms after it showed, until it runs out. */
    fun waiting(now: Long): Boolean = stage == JoinStage.CODE && code != null && !expired(now) && now - codeAt >= Waiting.WAIT_MILLIS

    /** The hint that both need the same Wi-Fi: 30 s after the code showed with nothing yet. */
    fun slow(now: Long): Boolean = waiting(now) && now - codeAt >= JoinModel.HINT_MILLIS

    /** The code ran out: its digits dim and the error takes the waiting line's place. */
    fun expired(now: Long): Boolean = stage == JoinStage.CODE && code != null && now >= until

    /** Seconds the code has left, for "Good for 2 minutes · 1:42". */
    fun secondsLeft(now: Long): Long = ((until - now + 999) / 1000).coerceIn(0, JoinModel.CODE_SECONDS)

    /** The setup did not come in time: the page goes on with what it has. */
    fun gaveUp(now: Long): Boolean = stage == JoinStage.PAIRED && setup == null && now - pairedAt >= JoinModel.SETUP_MILLIS

    /** "Looking at what … has": 600 ms after pairing, until the setup lands or the wait is over. */
    fun looking(now: Long): Boolean = stage == JoinStage.PAIRED && setup == null && !gaveUp(now) && now - pairedAt >= Waiting.WAIT_MILLIS

    /** Everything that will come has come: Continue turns amber. */
    fun landed(now: Long): Boolean = stage == JoinStage.PAIRED && (setup != null || gaveUp(now))

    /** Stores Continue asked to join that have not answered yet, while the wait for them lasts. */
    fun pending(now: Long): Set<String> = if (now - askedAt >= JoinModel.JOIN_MILLIS) emptySet() else asked - answered

    /** Continue is working on the stores: its spinner shows after 600 ms. */
    fun working(now: Long): Boolean = pending(now).isNotEmpty() && now - askedAt >= Waiting.WAIT_MILLIS

    /**
     * The page can go on: Continue was pressed, the stores it asked answered, and none of them
     * said no. A no stays on the page, in its row, until Continue is pressed again.
     */
    fun leaving(now: Long): Boolean = presses > 0 && pending(now).isEmpty() && refused.none { it.store in asked }

    /** The store [name]'s refusal, when it said no. */
    fun refusal(name: String): StoreJoin.Failed? = refused.lastOrNull { it.store == name }

    /** The next moment anything above changes, after [now]; null when nothing will on its own. */
    fun nextChange(now: Long): Long? {
        val marks = mutableListOf<Long>()
        if (stage == JoinStage.CODE && code != null) {
            marks += codeAt + Waiting.WAIT_MILLIS
            marks += codeAt + JoinModel.HINT_MILLIS
            marks += until
            // The countdown, each second.
            if (now < until) marks += until - ((until - now - 1) / 1000) * 1000
        }
        if (stage == JoinStage.PAIRED && setup == null) {
            marks += pairedAt + Waiting.WAIT_MILLIS
            marks += pairedAt + JoinModel.SETUP_MILLIS
        }
        if (asked.isNotEmpty()) {
            marks += askedAt + Waiting.WAIT_MILLIS
            marks += askedAt + JoinModel.JOIN_MILLIS
        }
        return marks.filter { it > now }.minOrNull()
    }
}

/** The first-run pages after the join page that are still to ask, in order. */
fun JoinedSetup?.pagesLeft(signIns: Boolean, sources: Boolean): Set<FirstRunPage> {
    val done = this?.answered.orEmpty().filter {
        when (it) {
            FirstRunPage.MEDIUM -> true
            FirstRunPage.LISTS -> signIns
            FirstRunPage.SOURCES -> sources
        }
    }
    return FirstRunPage.entries.toSet() - done.toSet()
}

/** The join page's state, fed by [SyncModel] from the engine's pairing events. */
class JoinModel(private val clock: () -> Long = { System.currentTimeMillis() }) {
    private val _state = MutableStateFlow(JoinState())

    val state: StateFlow<JoinState> = _state.asStateFlow()

    private val _opened = MutableStateFlow(false)

    /** Something outside the page, such as a staged state, asks first run to show the join page. */
    val opened: StateFlow<Boolean> = _opened.asStateFlow()

    /** A moment the clock stays at, for a staged state. */
    private var frozen: Long? = null

    /** The clock the page reads its state against. */
    fun now(): Long = frozen ?: clock()

    /** A code to start the Add a device field with, for a staged state. */
    var typed: String? = null
        internal set

    /** Show a code: the page turns to it, and the code shows when the engine says it. */
    fun begin() {
        frozen = null
        _state.value = JoinState(stage = JoinStage.CODE)
    }

    /** Back to the two ways in, or the page was left: nothing is held. */
    fun reset() {
        _state.value = JoinState()
        _opened.value = false
        frozen = null
    }

    private val _startAt = MutableStateFlow<FirstRunPage?>(null)

    /** A first-run page a staged state asks first run to open on; null once it has. */
    val startAt: StateFlow<FirstRunPage?> = _startAt.asStateFlow()

    /** Open first run on [page], for shots and the Fixtures app. */
    internal fun startAt(page: FirstRunPage) {
        _startAt.value = page
    }

    /** First run opened on the page [startAt] asked for. */
    fun started() {
        _startAt.value = null
    }

    /** First run turned to the join page. */
    fun shown() {
        _opened.value = false
    }

    internal fun open() {
        _opened.value = true
    }

    internal fun invite(code: String, port: Int, until: Long) {
        _state.value.takeIf { it.stage == JoinStage.CODE }?.let { _state.value = it.copy(code = code, port = port, codeAt = now(), until = now() + (until - clock())) }
    }

    /** A device joined the group while the code showed. */
    internal fun paired(deviceName: String?) {
        val now = _state.value
        if (now.stage != JoinStage.CODE) return
        _state.value = now.copy(stage = JoinStage.PAIRED, pairedAt = now(), deviceName = deviceName ?: now.deviceName)
    }

    internal fun copied(copied: SignInsCopied) {
        val now = _state.value
        if (now.stage == JoinStage.ASK) return
        val paired = if (now.stage == JoinStage.CODE) now.copy(stage = JoinStage.PAIRED, pairedAt = now()) else now
        _state.value = paired.copy(deviceName = copied.device, signIns = copied.trackers)
    }

    internal fun setup(setup: JoinedSetup) {
        val now = _state.value
        if (now.stage == JoinStage.ASK) return
        val paired = if (now.stage == JoinStage.CODE) now.copy(stage = JoinStage.PAIRED, pairedAt = now()) else now
        _state.value = paired.copy(deviceName = setup.deviceName, setup = setup)
    }

    internal fun storeJoin(join: StoreJoin) {
        val now = _state.value
        val from = now.setup?.device ?: return
        if (join.device != from) return
        _state.value = now.copy(
            answered = now.answered + join.store,
            refused = if (join is StoreJoin.Failed) now.refused.filter { it.store != join.store } + join else now.refused.filter { it.store != join.store },
        )
    }

    /**
     * Continue was pressed with [stores] chosen, by name. Returns the ones to ask for now: none
     * that were asked before, so a second press goes on past a store that said no.
     */
    fun proceed(stores: Collection<String>): Set<String> {
        val now = _state.value
        val fresh = stores.toSet() - now.answered - now.asked
        _state.value = now.copy(asked = fresh, askedAt = now(), presses = now.presses + 1)
        return fresh
    }

    /** Set a state as it stands for shots and the Fixtures app. */
    internal fun stage(state: JoinState, at: Long?) {
        frozen = at
        _state.value = state
        _opened.value = true
    }

    internal fun stageTyped(code: String?) {
        typed = code
    }

    companion object {
        /** How long the code is good for. */
        const val CODE_SECONDS = 120L

        /** When "Nothing yet?" shows under the waiting line. */
        const val HINT_MILLIS = 30_000L

        /** How long the page waits after pairing for the setup before it goes on without it. */
        const val SETUP_MILLIS = 15_000L

        /** How long Continue waits for the stores it asked to answer. */
        const val JOIN_MILLIS = 15_000L
    }
}
