// Where streams come from, as Providers and Settings show it: each provider, why there are none,
// the torrent proxy's check, and the order failover walks them in.

package app.kaeri.store

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import app.kaeri.engine.model.Msg
import app.kaeri.engine.model.ProviderRow
import app.kaeri.engine.model.ProxyStatus
import app.kaeri.engine.model.TorrentProxy

/** The providers, as the engine last said them. */
class ProvidersModel {
    private val _rows = MutableStateFlow<List<ProviderRow>>(emptyList())

    /** Each provider, in the order the engine lists them. */
    val rows: StateFlow<List<ProviderRow>> = _rows.asStateFlow()

    private val _loaded = MutableStateFlow(false)

    /** Whether the engine has said the providers at all. */
    val loaded: StateFlow<Boolean> = _loaded.asStateFlow()

    private val _note = MutableStateFlow<Msg?>(null)

    /** Why there are no sources, when there are none. */
    val note: StateFlow<Msg?> = _note.asStateFlow()

    private val _rebuilding = MutableStateFlow(false)

    /** Whether the sources are being rebuilt, from `SourcesChanged` until the rebuilt list. */
    val rebuilding: StateFlow<Boolean> = _rebuilding.asStateFlow()

    private val _check = MutableStateFlow<ProxyStatus?>(null)

    /**
     * The saved proxy's check, as the engine last said it; `null` while torrents are off, and
     * [ProxyStatus.DIRECT] while they are on with no proxy.
     */
    val check: StateFlow<ProxyStatus?> = _check.asStateFlow()

    /**
     * A proxy checked before it is saved: under way, then the check's verdict on it. Each carries
     * the [proxy] it checked, its password as given, so a form saves exactly what passed and sets
     * aside a verdict on details it no longer holds.
     */
    sealed interface Trial {
        val proxy: TorrentProxy

        class Checking internal constructor(override val proxy: TorrentProxy) : Trial

        class Said internal constructor(override val proxy: TorrentProxy, val status: ProxyStatus) : Trial {
            /** Whether the proxy may be saved. */
            val passed: Boolean get() = status == ProxyStatus.PASSED
        }
    }

    private val _trial = MutableStateFlow<Trial?>(null)

    /** The check a form asked for; `null` until it asks, and once it is done with it. */
    val trial: StateFlow<Trial?> = _trial.asStateFlow()

    // The engine runs each check on its own and its verdict names none of them, so checks go one
    // at a time: the verdict that comes is the one sent's. One asked for meanwhile waits, and a
    // newer ask takes its place.
    private var sent: Trial.Checking? = null
    private var waiting: Trial.Checking? = null

    /** A check of [proxy] is asked for: the one to send now, or null while another runs. */
    internal fun trialAsked(proxy: TorrentProxy): Trial.Checking? = synchronized(this) {
        val asked = Trial.Checking(proxy)
        _trial.value = asked
        if (sent == null) {
            sent = asked
            asked
        } else {
            waiting = asked
            null
        }
    }

    /** The verdict on the check sent; the one waiting, to send next. */
    internal fun trialSaid(status: ProxyStatus): Trial.Checking? = synchronized(this) {
        val said = sent ?: return null
        // A verdict no form is waiting for any more is not kept.
        if (_trial.value === said) _trial.value = Trial.Said(said.proxy, status)
        sent = waiting
        waiting = null
        sent
    }

    /** Done with the check: its form closed, or its details changed. */
    fun clearTrial() {
        _trial.value = null
    }

    private val _preference = MutableStateFlow<String?>(null)

    /**
     * The source pinned for the title last asked about (`ProviderPreference`), `null` for
     * automatic: what a show's Source row stands on.
     */
    val preference: StateFlow<String?> = _preference.asStateFlow()

    internal fun preference(provider: String?) {
        _preference.value = provider
    }

    internal fun apply(rows: List<ProviderRow>) {
        _rows.value = rows
        _loaded.value = true
        _rebuilding.value = false
        if (rows.isNotEmpty()) _note.value = null
    }

    internal fun note(msg: Msg) {
        _note.value = msg
    }

    internal fun sourcesChanged() {
        _rebuilding.value = true
    }

    internal fun proxySaid(status: ProxyStatus?) {
        _check.value = status
    }

    fun row(id: String): ProviderRow? = _rows.value.firstOrNull { it.id == id }
}

/** How a provider stands, as the Providers list says it beside the name. */
enum class ProviderStanding { Working, Slow, Failing, HeldBack }

val ProviderRow.standing: ProviderStanding
    get() = when {
        heldBack -> ProviderStanding.HeldBack
        !usable || state == "down" -> ProviderStanding.Failing
        state == "degraded" -> ProviderStanding.Slow
        else -> ProviderStanding.Working
    }

/**
 * Where streams come from, as Providers moves them: ports of `kaeri_model::sources`'s order
 * rules, so a drag writes the order the terminal's keys would.
 */
object SourceOrder {
    /** The order entry that stands for every source of [kind] the order does not name by id. */
    fun groupEntry(kind: String): String? = when (kind) {
        "plugin" -> "plugins"
        "library" -> "library"
        "server" -> "servers"
        "catalogue" -> "catalogues"
        "demo" -> "demo"
        else -> null
    }

    private fun firstParty(kind: String) = kind == "library" || kind == "server" || kind == "catalogue"

    /**
     * [order] with the source [id] moved one place up the chain ([step] below zero) or down it,
     * or `null` when it is already at that end or has no row. `order_with_source_moved`.
     *
     * Every source on screen is first named in the order, at its group entry's place, or first
     * for a first-party source an older order never named. Entries with no row are stepped over.
     */
    fun moved(order: List<String>, rows: List<ProviderRow>, id: String, step: Int): List<String>? {
        val spelled = order.toMutableList()
        var firsts = 0
        for (row in rows) {
            if (row.id in spelled) continue
            val at = groupEntry(row.kind)?.let { spelled.indexOf(it) }?.takeIf { it >= 0 }
            when {
                at != null -> spelled.add(at, row.id)
                firstParty(row.kind) -> spelled.add(firsts++, row.id)
                else -> spelled.add(row.id)
            }
        }
        val from = spelled.indexOf(id).takeIf { it >= 0 } ?: return null
        val shown = { entry: String -> rows.any { it.id == entry } }
        val to = if (step < 0) {
            (from - 1 downTo 0).firstOrNull { shown(spelled[it]) } ?: return null
        } else {
            (from + 1 until spelled.size).firstOrNull { shown(spelled[it]) } ?: return null
        }
        val entry = spelled.removeAt(from)
        spelled.add(to, entry)
        return spelled
    }

    /**
     * [order] with [id] moved to where the row at [target] is on screen: one place at a time, by
     * the rule above, so a drag writes what the same number of key presses would.
     */
    fun movedTo(order: List<String>, rows: List<ProviderRow>, id: String, target: Int): List<String>? {
        val from = rows.indexOfFirst { it.id == id }
        if (from < 0 || target == from || target !in rows.indices) return null
        var moved = order
        repeat(kotlin.math.abs(target - from)) {
            moved = moved(moved, rows, id, if (target < from) -1 else 1) ?: return moved
        }
        return moved
    }

    /**
     * The rows in the order failover tries them: by where each stands in [order], else where its
     * group entry stands, else first for first-party kinds and last for the rest (`shown_ids`).
     */
    fun sorted(rows: List<ProviderRow>, order: List<String>): List<ProviderRow> {
        fun rank(row: ProviderRow): Int {
            order.indexOf(row.id).takeIf { it >= 0 }?.let { return it }
            groupEntry(row.kind)?.let { entry -> order.indexOf(entry).takeIf { it >= 0 }?.let { return it } }
            return if (firstParty(row.kind)) -1 else order.size
        }
        return rows.withIndex().sortedWith(compareBy({ rank(it.value) }, { it.index })).map { it.value }
    }
}

/** A run of the Your sources page: a section, the store a plugin section belongs to, and its rows. */
data class SourceGroup(val section: app.kaeri.engine.model.SourceSection, val store: String?, val rows: List<ProviderRow>)

/**
 * The page's sections in order, as `source_groups` makes them: a section per kind, each store's
 * plugins apart, the demo and the other built-in sources last. A kind with no rows has no section.
 */
fun sourceGroups(rows: List<ProviderRow>): List<SourceGroup> {
    fun sectionOf(row: ProviderRow) = when (row.kind) {
        "library" -> app.kaeri.engine.model.SourceSection.YOUR_MEDIA
        "server" -> app.kaeri.engine.model.SourceSection.MEDIA_SERVERS
        "licensed" -> app.kaeri.engine.model.SourceSection.LICENSED_SERVICES
        "catalogue" -> app.kaeri.engine.model.SourceSection.FREE_CATALOGUES
        "plugin" -> app.kaeri.engine.model.SourceSection.PLUGINS
        else -> app.kaeri.engine.model.SourceSection.BUILT_IN
    }
    val groups = mutableListOf<Triple<app.kaeri.engine.model.SourceSection, String?, MutableList<ProviderRow>>>()
    for (row in rows) {
        val section = sectionOf(row)
        val store = (row.origin as? app.kaeri.engine.model.ProviderOrigin.Store)?.id.takeIf { section == app.kaeri.engine.model.SourceSection.PLUGINS }
        val group = groups.firstOrNull { it.first == section && it.second == store }
            ?: Triple(section, store, mutableListOf<ProviderRow>()).also { groups.add(it) }
        group.third.add(row)
    }
    return groups.sortedBy { it.first.ordinal }.map { SourceGroup(it.first, it.second, it.third) }
}

/** Whether nothing but built-in sources is there: the page's empty state. */
val List<ProviderRow>.hasNoSources: Boolean
    get() = all { it.kind != "library" && it.kind != "server" && it.kind != "licensed" && it.kind != "catalogue" && it.kind != "plugin" }
