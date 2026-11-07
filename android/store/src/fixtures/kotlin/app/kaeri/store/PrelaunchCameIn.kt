// Prelaunch board piece 1, "Say what came in, or why nothing did", staged for shots on the
// fixtures engine (see Prelaunch.kt). Everything is made up: a Kavita server at books.home.example
// with 212 series in 2 libraries, a Komga server at comics.home.example, a folder of manga, and
// Kim's shelf, a plugin store at shelf.kim.example. Each state is the form as it stands once the
// engine has answered: `p1-settings-NN` is Settings' Your sources with the form over it, and
// `p1-first-NN` first run's books page with the form in its place. The TV states,
// `p1-tv-settings-NN` and `p1-tv-first-NN`, are the store form only (11 to 16).

package app.kaeri.store

import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.BookPluginRow
import app.kaeri.engine.model.BookServerKind
import app.kaeri.engine.model.BookServerRow
import app.kaeri.engine.model.BookSourceEvent
import app.kaeri.engine.model.CameIn
import app.kaeri.engine.model.FirstRunPage
import app.kaeri.engine.model.FolderReport
import app.kaeri.engine.model.FormatCount
import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.engine.model.NothingCameIn
import app.kaeri.engine.model.PluginsInstalled
import app.kaeri.engine.model.ServerFound
import app.kaeri.engine.model.ServerLibrary
import app.kaeri.engine.model.StorePluginRow
import app.kaeri.engine.model.StorePluginStatus
import app.kaeri.engine.model.StoreReport
import app.kaeri.engine.model.StoreRow
import app.kaeri.engine.model.StoreState
import kotlinx.coroutines.Job
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.launch

object PrelaunchCameIn {
    private val NUMBERS = (1..16).map { "%02d".format(it) }
    private val TV_NUMBERS = (11..16).map { "%02d".format(it) }

    val STATES: List<String> = NUMBERS.map { "p1-settings-$it" } + NUMBERS.map { "p1-first-$it" } +
        TV_NUMBERS.map { "p1-tv-settings-$it" } + TV_NUMBERS.map { "p1-tv-first-$it" }

    const val BOOKS = "books.home.example"
    const val COMICS = "comics.home.example"
    const val SHELF = "shelf.kim.example"

    /** A Kavita OPDS link: its key's last four are 3f9a. */
    const val KAVITA_LINK = "https://$BOOKS/api/opds/0f3c9e2d-77aa-4b1e-9d2b-5c1e8a0b3f9a"

    /** An invite to Kim's shelf: its code's last four are 7Q2M. */
    const val INVITE = "https://$SHELF/join/HX4K7Q2M#c2hlbGYta2V5LWZvci1zaG90cw"

    const val STORE_ID = "kim-shelf"
    private const val STORE_NAME = "Kim’s shelf" // i18n-skip: a made-up store

    private var keeping: Job? = null

    private val found = ServerFound(
        pending = "pending-books",
        kind = BookServerKind.KAVITA,
        name = BOOKS,
        host = BOOKS,
        address = "https://$BOOKS",
        series = 212u,
        libraries = listOf(ServerLibrary("manga", "Manga", 168u), ServerLibrary("comics", "Comics", 44u)), // i18n-skip
        keyLast4 = "3f9a",
    )

    private fun server(id: String, kind: BookServerKind, host: String, series: UInt, libraries: UInt) = BookServerRow(
        id = id,
        kind = kind,
        name = host,
        address = "https://$host",
        series = series,
        reportProgress = true,
        off = false,
        signedOut = false,
        libraries = libraries,
        empty = false,
        rechecksAt = null,
        keyLast4 = null,
        progressPending = 0u,
        progressSending = false,
        progressSavedAt = null,
        progressProblem = null,
        progressProblemAt = null,
    )

    private fun plugin(id: String, name: String, version: String, hosts: Int, reading: Boolean, installed: Boolean = false) = StorePluginRow(
        id = id,
        name = name,
        version = version,
        hosts = (1..hosts).map { "host$it.$id.example" },
        status = if (installed) StorePluginStatus.Installed else StorePluginStatus.Available,
        books = reading,
        media = if (reading) listOf(Medium.MANGA) else listOf(Medium.ANIME),
    )

    private fun shelf(plugins: List<StorePluginRow>) = StoreRow(
        id = STORE_ID,
        name = STORE_NAME,
        url = "https://$SHELF",
        host = SHELF,
        fingerprint = "SHA256:kim-shelf-shots",
        member = "Phone", // i18n-skip
        owner = false,
        state = StoreState.Ready,
        checkedAt = null,
        plugins = plugins,
        owners = listOf("Kim"),
        open = false,
        anonymous = false,
    )

    /** What joining said, on a device that reads ([tv] false) or only watches. */
    private fun report(shown: List<String>, other: Int = 0, installAll: Boolean = false, tv: Boolean = false) = StoreReport(
        store = STORE_ID,
        name = STORE_NAME,
        owner = "Kim",
        forReading = if (tv) emptyList() else shown,
        forWatching = if (tv) shown else emptyList(),
        otherMedium = other.toUInt(),
        otherIsReading = tv,
        installAll = installAll,
    )

    private fun tsuru(installed: Boolean = false) = plugin("tsuru", "Tsuru", "1.4", 2, reading = true, installed = installed) // i18n-skip
    private fun lantern(installed: Boolean = false) = plugin("lantern", "Lantern Pages", "0.9", 1, reading = true, installed = installed) // i18n-skip
    private fun kawa(installed: Boolean = false) = plugin("kawa", "Kawa", "2.1", 3, reading = false, installed = installed) // i18n-skip
    private fun hotaru(installed: Boolean = false) = plugin("hotaru", "Hotaru", "1.0", 2, reading = false, installed = installed) // i18n-skip

    private fun folderReport(name: String, series: Int, files: Int, matched: Int, unsupported: List<FormatCount>, locked: List<FormatCount>) = FolderReport(
        id = "folder-${name.lowercase()}",
        name = name,
        path = name,
        series = series.toUInt(),
        books = 0u,
        files = files.toUInt(),
        matched = matched.toUInt(),
        unsupported = unsupported,
        locked = locked,
        listed = (unsupported + locked).flatMap { format -> (1..format.count.toInt()).map { "Volume %02d.%s".format(it, format.format.lowercase()) } }, // i18n-skip
    )

    private val manga = folderReport(
        "Manga", series = 64, files = 1206, matched = 58, // i18n-skip
        unsupported = listOf(FormatCount("MOBI", 14u), FormatCount("AZW3", 6u)),
        locked = listOf(FormatCount("EPUB", 3u)),
    )
    private val downloads = folderReport("Downloads", series = 0, files = 312, matched = 0, unsupported = emptyList(), locked = emptyList()) // i18n-skip

    suspend fun stage(store: AppStore, state: String): Boolean {
        if (state !in STATES) return false
        val tv = state.startsWith("p1-tv-")
        val first = state.startsWith("p1-first-") || state.startsWith("p1-tv-first-")
        val step = state.takeLast(2).toInt()
        val model = store.cameIn
        keeping?.cancel()
        model.closeQuietly()
        if (first) {
            if (!tv) store.medium.choose(KaeriIsFor.READ)
            store.sync.join.startAt(FirstRunPage.SOURCES)
        }
        val inPage = first && !tv
        // The store rows the state needs, kept as the fixtures engine says its own.
        val plugins = when {
            tv && step == 13 -> listOf(tsuru(), lantern())
            tv -> listOf(kawa(installed = step == 12 || step == 16), tsuru(), lantern())
            step == 11 -> listOf(tsuru(), lantern(), kawa())
            step == 12 -> listOf(tsuru(true), lantern(true), kawa())
            step == 13 -> listOf(kawa(), hotaru())
            step == 16 -> listOf(tsuru(true))
            else -> emptyList()
        }
        val stores = if (step in 11..14 || step == 16) listOf(shelf(plugins)) else emptyList()
        // Your sources behind the form: the made-up servers, and Tsuru once it is installed.
        val books = server("books-home", BookServerKind.KAVITA, BOOKS, 212u, 2u)
        val comics = server("comics-home", BookServerKind.OPDS, COMICS, 38u, 1u)
        val servers = when {
            tv -> emptyList()
            step == 16 && first -> listOf(books)
            step == 16 -> listOf(books, comics)
            else -> listOf(comics)
        }
        val readingPlugins = if (step == 16 && !tv && !first) {
            listOf(BookPluginRow(id = "tsuru", name = "Tsuru", version = "1.4", media = listOf(Medium.MANGA), languages = listOf("en"), off = false, store = STORE_NAME)) // i18n-skip
        } else {
            emptyList()
        }
        keeping = MainScope().launch {
            if (stores.isNotEmpty()) {
                launch { store.stores.stores.collect { now -> if (now != stores) store.stores.apply(stores) } }
            }
            if (servers.isNotEmpty()) {
                store.bookSources.view.collect { view ->
                    val now = view ?: return@collect
                    if (servers.all { s -> now.servers.any { it.id == s.id } } && readingPlugins.all { p -> now.plugins.any { it.id == p.id } }) return@collect
                    store.bookSources.apply(
                        BookSourceEvent.View(
                            now.copy(
                                servers = servers + now.servers.filter { s -> servers.none { it.id == s.id } },
                                plugins = readingPlugins + now.plugins.filter { p -> readingPlugins.none { it.id == p.id } },
                                order = servers.map { it.id } + now.order.filter { id -> servers.none { it.id == id } },
                            ),
                        ),
                    )
                }
            }
        }
        if (stores.isNotEmpty()) store.stores.apply(stores)
        if (!tv && store.bookSources.view.value == null) store.bookSources.load()
        val password = "kept-safe!" // i18n-skip: drawn as dots
        val kavita = CameInModel.Asked(KAVITA_LINK, false)
        when (step) {
            1 -> model.stage(Adding.SERVER, null, address = KAVITA_LINK, asked = kavita, looking = false, asking = true, inPage = inPage)
            2 -> model.stage(Adding.SERVER, null, address = KAVITA_LINK, asked = kavita, looking = true, inPage = inPage)
            3 -> model.stage(Adding.SERVER, CameIn.Found(found), address = KAVITA_LINK, asked = kavita, inPage = inPage)
            4 -> model.stage(Adding.SERVER, CameIn.Empty(NothingCameIn.Unreachable(COMICS)), address = COMICS, asked = CameInModel.Asked(COMICS, false), inPage = inPage)
            5 -> model.stage(
                Adding.SERVER,
                CameIn.Empty(NothingCameIn.WrongPassword(COMICS, BookServerKind.KOMGA)),
                address = COMICS,
                user = "kim",
                password = password,
                asked = CameInModel.Asked(COMICS, true),
                inPage = inPage,
            )
            6 -> model.stage(Adding.SERVER, CameIn.Empty(NothingCameIn.WrongKey(BOOKS)), address = KAVITA_LINK, asked = kavita, inPage = inPage)
            7 -> model.stage(
                Adding.SERVER,
                CameIn.Empty(NothingCameIn.EmptyCatalogue(found.copy(series = 0u, libraries = emptyList()))),
                address = KAVITA_LINK,
                asked = kavita,
                inPage = inPage,
            )
            8 -> {
                val media = "media.home.example"
                model.stage(Adding.SERVER, CameIn.Empty(NothingCameIn.NotABookServer(media)), address = media, asked = CameInModel.Asked(media, false), inPage = inPage)
            }
            9 -> model.stage(Adding.FOLDER, CameIn.Folder(manga), folder = manga.name, inPage = inPage)
            10 -> model.stage(Adding.FOLDER, CameIn.Empty(NothingCameIn.EmptyFolder(downloads)), folder = downloads.name, inPage = inPage)
            11 -> {
                val joined = if (tv) report(listOf("kawa"), other = 2, tv = true) else report(listOf("tsuru", "lantern"), other = 1, installAll = true)
                model.stage(Adding.STORE, CameIn.Store(joined), address = INVITE, inPage = inPage)
            }
            12 -> {
                val joined = if (tv) report(listOf("kawa"), other = 2, tv = true) else report(listOf("tsuru", "lantern"), other = 1, installAll = true)
                model.stage(Adding.STORE, CameIn.Installed(PluginsInstalled(STORE_ID, STORE_NAME, if (tv) 1u else 2u, emptyList())), address = INVITE, inPage = inPage)
                model.stageReport(joined)
            }
            13 -> {
                val other = report(emptyList(), other = 2, tv = tv)
                model.stage(Adding.STORE, CameIn.Empty(NothingCameIn.OtherMedium(other)), address = INVITE, inPage = inPage)
            }
            14 -> model.stage(Adding.STORE, CameIn.Empty(NothingCameIn.EmptyStore(report(emptyList(), tv = tv))), address = INVITE, inPage = inPage)
            15 -> model.stage(Adding.STORE, CameIn.Empty(NothingCameIn.StoreUnreachable(SHELF)), address = INVITE, inPage = inPage)
            16 -> if (tv) {
                model.stageAdded(STORE_ID, CameInModel.Notice.Plugins(1, STORE_NAME))
            } else {
                // The server form has closed: its row is in, and in Settings a notice says so.
                model.stageAdded("books-home", if (first) null else CameInModel.Notice.Added(BOOKS, 212))
            }
        }
        return true
    }
}
