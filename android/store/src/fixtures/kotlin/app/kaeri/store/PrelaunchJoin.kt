// Prelaunch board piece 3, "Already use Kaeri on another device?", staged for shots on the fixtures
// engine (see Prelaunch.kt). The paired device, its plugin store and its book server are made up;
// the clock stays where each state needs it, so the countdown reads as the board's.

package app.kaeri.store

import app.kaeri.engine.model.BookServerKind
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.FirstRunPage
import app.kaeri.engine.model.JoinedSetup
import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.engine.model.MediumAsk
import app.kaeri.engine.model.SyncBookServerOffer
import app.kaeri.engine.model.SyncDeviceView
import app.kaeri.engine.model.SyncStatus
import app.kaeri.engine.model.SyncStoreOffer
import app.kaeri.engine.model.SyncStoreProblem

object PrelaunchJoin {
    private val PHONE = (1..9).map { "p3-%02d".format(it) }
    private val TV = (1..8).map { "p3-tv-%02d".format(it) }

    val STATES: List<String> = PHONE + TV

    private const val CODE = "482913"
    private const val STORE = "Kim’s shelf" // i18n-skip

    /** The paired device on [tv]: a MacBook for a phone or tablet, a phone for a TV. */
    fun device(tv: Boolean): String = if (tv) "Kim’s phone" else "Kim’s MacBook" // i18n-skip

    /** What the paired device brings: a TV gets no book server, and only the watching tracker. */
    fun setup(tv: Boolean, folders: Int = 1): JoinedSetup {
        val store = SyncStoreOffer(address = "https://shelf.example", name = STORE, inviteFrom = "kim")
        val server = SyncBookServerOffer(
            from = "kim",
            id = "books-home",
            kind = BookServerKind.KAVITA,
            name = "Home books", // i18n-skip
            host = "books.home.example",
            address = "https://books.home.example",
            user = null,
            keyLast4 = "7f3a",
            libraries = emptyList(),
            added = false,
        )
        return JoinedSetup(
            device = "kim",
            deviceName = device(tv),
            medium = KaeriIsFor.READ,
            stores = listOf(store),
            servers = if (tv) emptyList() else listOf(server),
            signedIn = if (tv) listOf("anilist") else listOf("anilist", "mangaupdates"),
            folders = if (tv) 0u else folders.toUInt(),
            answered = listOf(FirstRunPage.MEDIUM, FirstRunPage.LISTS, FirstRunPage.SOURCES),
        )
    }

    suspend fun stage(store: AppStore, state: String): Boolean {
        if (state !in STATES) return false
        val tv = state.startsWith("p3-tv-")
        val step = state.takeLast(2).toInt()
        val join = store.sync.join
        val at = System.currentTimeMillis()
        val name = device(tv)
        val code = JoinState(stage = JoinStage.CODE, code = CODE, port = 41234, codeAt = at - 18_000, until = at + 102_000)
        val kim = SyncDeviceView(id = "kim", name = name, thisDevice = false, trackers = setup(tv).signedIn, signIns = emptyList())
        val paired = JoinState(stage = JoinStage.PAIRED, pairedAt = at - 2_000, deviceName = name, signIns = setup(tv).signedIn)
        when (step) {
            1 -> join.stage(JoinState(), at)
            2 -> join.stage(code, at)
            3 -> join.stage(code.copy(codeAt = at - 31_000, until = at + 89_000), at)
            4 -> join.stage(code.copy(codeAt = at - 120_000, until = at), at)
            5 -> {
                store.apply(Event.SyncStatus(SyncStatus(enabled = true, syncing = false, devices = listOf(kim))))
                join.stage(paired, at)
            }
            6, 7, 8 -> {
                // Continue answered the medium as the other device has it, as the page does.
                if (step == 8 && !tv) {
                    val ask = store.medium.asking.value
                    if (ask is MediumAsk.FirstRun) store.medium.answer(ask, KaeriIsFor.READ) else store.medium.choose(KaeriIsFor.READ)
                }
                store.apply(Event.SyncStatus(SyncStatus(enabled = true, syncing = false, devices = listOf(kim))))
                val setup = setup(tv, folders = if (step == 6) 1 else 0)
                val landed = paired.copy(setup = setup)
                join.stage(
                    when (step) {
                        6 -> landed
                        // Continue asked, and the store said no: the row says so and the page stays.
                        7 -> landed.copy(
                            asked = setOf(STORE),
                            askedAt = at - 3_000,
                            answered = setOf(STORE),
                            refused = listOf(StoreJoin.Failed("kim", STORE, SyncStoreProblem.REFUSED)),
                            presses = 1,
                        )
                        // Continue pressed and every answer in: first run ends, Home says it syncs.
                        else -> landed.copy(presses = 1, askedAt = at - 3_000)
                    },
                    at,
                )
            }
            9 -> {
                // The other device: Add a device with the code typed.
                store.apply(Event.SyncStatus(SyncStatus(enabled = true, syncing = false, devices = emptyList())))
                join.stageTyped(CODE)
                store.sync.show(SyncSheet.ADD)
            }
        }
        return true
    }
}
