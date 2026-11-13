// Where the TV is: one of the rail's six destinations, and the pages opened over it (a title page
// for now). Each keeps a focus memory, so returning to it puts focus back on what opened the page.

package app.kaeri.tv.shell

import androidx.annotation.StringRes
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.vector.ImageVector
import app.kaeri.engine.core.AnilistId
import app.kaeri.i18n.R as I18nR
import app.kaeri.tv.ui.FocusMemory
import app.kaeri.tv.ui.TvIcons

/** The rail's destinations, top to bottom: Search, then the phone's order, then Settings. */
enum class Destination(@StringRes val label: Int, val icon: ImageVector) {
    Search(I18nR.string.nav_search, TvIcons.Search),
    Home(I18nR.string.home_title, TvIcons.Continue),
    Calendar(I18nR.string.nav_calendar, TvIcons.Calendar),
    Seasonal(I18nR.string.nav_seasonal, TvIcons.Seasonal),
    Library(I18nR.string.nav_library, TvIcons.Library),
    Together(I18nR.string.together_title, TvIcons.People),
    Settings(I18nR.string.nav_settings, TvIcons.Settings),
    ;

    companion object {
        /** A destination by the fixtures' section names: continue, search, library and so on. */
        fun named(name: String?): Destination? =
            if (name.equals("continue", ignoreCase = true)) Home else entries.firstOrNull { it.name.equals(name, ignoreCase = true) }
    }
}

/** A page over a destination, with its own focus memory. */
sealed interface Page {
    /** The key its state is saved under, unique to this opening. */
    val key: String
    val memory: FocusMemory

    /** A title's page. Its [id] changes in place when another season is picked. */
    class Title(id: AnilistId, override val key: String) : Page {
        var id by mutableStateOf(id)
        override val memory: FocusMemory = FocusMemory()
    }

    /** TVSignIn for a tracker, over Settings › Accounts. */
    data class SignIn(val tracker: String, override val key: String) : Page {
        override val memory: FocusMemory = FocusMemory()
    }

    /** The TV's own update: the newer release, its download and Android's installer. */
    data class Update(override val key: String) : Page {
        override val memory: FocusMemory = FocusMemory()
    }

    /** TVJoinStore: add a plugin store from this TV, with or without a paired phone. */
    data class JoinStore(override val key: String) : Page {
        override val memory: FocusMemory = FocusMemory()
    }

    /** TVServers: Add a media server, one page per step. */
    data class AddServer(override val key: String) : Page {
        override val memory: FocusMemory = FocusMemory()
    }

    /** A saved media server's page. */
    data class Server(val id: String, override val key: String) : Page {
        override val memory: FocusMemory = FocusMemory()
    }

    /** Licensed services: the services ticked, the region and the switch for the rest. */
    data class Licensed(override val key: String) : Page {
        override val memory: FocusMemory = FocusMemory()
    }

    /** TVPartyLobby: the room this TV is in or was asked to join, with its QR code and code. */
    data class Lobby(override val key: String) : Page {
        override val memory: FocusMemory = FocusMemory()
    }

    /** TVPartyPick: what the party starts on, from what this TV is watching. */
    data class PartyPick(override val key: String) : Page {
        override val memory: FocusMemory = FocusMemory()
    }

    /** TVTorrents: the torrent proxy's form, over Settings › Sources. */
    data class Torrents(override val key: String) : Page {
        override val memory: FocusMemory = FocusMemory()
    }
}

/**
 * The TV's place: the destination, the pages over it, and what the screen showing now does when
 * Back asks it to go to its top.
 */
@Stable
class TvNavigator(start: Destination = Destination.Home) {
    var destination by mutableStateOf(start)
        private set

    /** The pages over the destination, the last on top. */
    val pages = mutableStateListOf<Page>()

    private val memories = Destination.entries.associateWith { FocusMemory() }

    private var opened = 0

    /** Search's text, kept while the TV stays on Search, title pages included. */
    var searchText by mutableStateOf("")
        private set

    /** Where the field's cursor is in [searchText]: the end, unless the system keyboard moved it. */
    var searchCursor by mutableIntStateOf(0)
        private set

    /**
     * The field's text either side of where a phone types, from its first text since the TV last
     * typed: each text it sends, whole, goes between the two, so a lost or repeated one leaves the
     * same field.
     */
    private var phoneAround: Pair<String, String>? = null

    /** The TV typed: [text], the cursor at [cursor] (the end when not given). */
    fun typeSearch(text: String, cursor: Int = text.length) {
        phoneAround = null
        searchText = text
        searchCursor = cursor.coerceIn(0, text.length)
    }

    /** A phone's field reads [text]: put in at the TV field's cursor, which is its end unless moved. */
    fun phoneSearch(text: String) {
        val (before, after) = phoneAround ?: run {
            val at = searchCursor.coerceIn(0, searchText.length)
            (searchText.take(at) to searchText.drop(at)).also { phoneAround = it }
        }
        val head = if (before.isNotEmpty() && !before.endsWith(' ') && text.isNotEmpty() && !text.startsWith(' ')) "$before " else before
        val tail = if (after.isNotEmpty() && !after.startsWith(' ') && text.isNotEmpty() && !text.endsWith(' ')) " $after" else after
        searchText = head + text + tail
        searchCursor = head.length + text.length
    }

    /** The phone's typing ended, by its Enter: what it types next goes in afresh. */
    fun phoneDone() {
        phoneAround = null
    }

    /** Counts a phone's Search submits: each moves focus to the first result. */
    var searchSubmits by mutableIntStateOf(0)
        private set

    /** A phone submitted Search, as its Enter does. */
    fun submitSearch() {
        searchSubmits += 1
    }

    /**
     * Forget the saved state under a key: a page closed, or a destination entered afresh. The root
     * points it at its state holder.
     */
    var forget: (String) -> Unit = {}

    fun memory(destination: Destination): FocusMemory = memories.getValue(destination)

    val top: Page? get() = pages.lastOrNull()

    /** What the content shows: the top page, else the destination. */
    val screenKey: String get() = top?.key ?: destination.name

    /** The focus memory of what shows. */
    val focus: FocusMemory get() = top?.memory ?: memory(destination)

    /**
     * Set by the screen showing: scroll to its top and focus its first focus, true when it was not
     * already there. Back at the top of a screen opens the rail instead.
     */
    var toTop: (() -> Boolean)? = null

    /**
     * Go to [to] from the rail. Another destination starts at its first focus with its pages
     * closed; the same one returns to where its focus was.
     */
    fun go(to: Destination) {
        if (to != Destination.Settings) settingsStart = null
        if (to == destination && pages.isEmpty()) return
        pages.forEach { forget(it.key) }
        pages.clear()
        if (to != destination) {
            if (destination == Destination.Search) typeSearch("")
            forget(to.name)
            memory(to).reset()
            destination = to
        }
    }

    /** The Settings section to open at, when something sent the TV to one: Accounts, say. */
    var settingsStart by mutableStateOf<String?>(null)
        private set

    /** Settings, at the section named [section]. */
    fun goSettings(section: String) {
        go(Destination.Settings)
        settingsStart = section
    }

    fun openJoinStore() {
        opened += 1
        pages += Page.JoinStore("join-store/$opened")
    }

    fun openAddServer() {
        opened += 1
        pages += Page.AddServer("add-server/$opened")
    }

    fun openServer(id: String) {
        opened += 1
        pages += Page.Server(id, "server/$id/$opened")
    }

    fun openLicensed() {
        if (pages.lastOrNull() is Page.Licensed) return
        opened += 1
        pages += Page.Licensed("licensed/$opened")
    }

    fun openTorrents() {
        opened += 1
        pages += Page.Torrents("torrents/$opened")
    }

    fun openTitle(id: AnilistId) {
        opened += 1
        pages += Page.Title(id, "title/$id/$opened")
    }

    /**
     * Another: the pick takes the title page's place, so the page crossfades and Back returns to
     * where Surprise me was pressed.
     */
    fun replaceTitle(id: AnilistId) {
        val top = pages.lastOrNull() as? Page.Title ?: return openTitle(id)
        pages.removeAt(pages.lastIndex)
        forget(top.key)
        openTitle(id)
    }

    /**
     * Show another season of the title page on top in its place, so Back still returns to what
     * opened the first, not through every season visited.
     */
    fun switchTitle(id: AnilistId) {
        val top = pages.lastOrNull() as? Page.Title
        if (top == null) {
            openTitle(id)
            return
        }
        if (top.id == id) return
        // The same page, its state and focus, showing the other season: no new screen to fade to.
        top.id = id
        top.memory.focused("season/$id")
    }

    /** The update's page, once: a second press returns to the one open. */
    fun openUpdate() {
        if (pages.lastOrNull() is Page.Update) return
        opened += 1
        pages += Page.Update("update/$opened")
    }

    fun openSignIn(tracker: String) {
        opened += 1
        pages += Page.SignIn(tracker, "sign-in/$tracker/$opened")
    }

    /** The party's lobby, once: a second open leaves the one on top. */
    fun openLobby() {
        if (pages.lastOrNull() is Page.Lobby) return
        pages.filter { it is Page.PartyPick }.forEach { forget(it.key) }
        pages.removeAll { it is Page.PartyPick }
        opened += 1
        pages += Page.Lobby("lobby/$opened")
    }

    fun openPartyPick() {
        opened += 1
        pages += Page.PartyPick("party-pick/$opened")
    }

    /** Close the top page; false when there was none. */
    fun pop(): Boolean {
        val page = pages.removeLastOrNull() ?: return false
        forget(page.key)
        // What shows again keeps where its focus was until it puts it back.
        focus.hold()
        return true
    }
}
