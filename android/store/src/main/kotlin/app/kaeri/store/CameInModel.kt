// What came in after a source was added: a book server, a folder or a plugin store, or why nothing
// did. The engine says it with Event.CameIn; this holds the form that asked (which one is open,
// what is typed in it), the last answer for it, the 600 ms rule for the waiting line, and the
// notice said once the form closes.

package app.kaeri.store

import app.kaeri.engine.model.BookServerKind
import app.kaeri.engine.model.BookSourceCommand
import app.kaeri.engine.model.CameIn
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.FolderReport
import app.kaeri.engine.model.NothingCameIn
import app.kaeri.engine.model.PluginsInstalled
import app.kaeri.engine.model.ServerFound
import app.kaeri.engine.model.StoreReport
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/** The form a source is being added with. */
enum class Adding { SERVER, FOLDER, STORE }

class CameInModel(scope: CoroutineScope, private val send: (Command) -> Unit) {
    private val waiting = WaitingLine(scope)

    private val _open = MutableStateFlow<Adding?>(null)

    /** The form open now, in Settings or first run, or null. */
    val open: StateFlow<Adding?> = _open.asStateFlow()

    private val _inPage = MutableStateFlow(false)

    /** The open form is drawn in first run's page, not as a dialog over the app. */
    val inPage: StateFlow<Boolean> = _inPage.asStateFlow()

    private val _folder = MutableStateFlow<String?>(null)

    /** The name of the folder being scanned, as the picker gave it, until its report names it. */
    val folder: StateFlow<String?> = _folder.asStateFlow()

    private val _address = MutableStateFlow("")
    private val _user = MutableStateFlow("")
    private val _password = MutableStateFlow("")

    /** The server form's fields, as typed. The password stays in memory, and goes when the form closes. */
    val address: StateFlow<String> = _address.asStateFlow()
    val user: StateFlow<String> = _user.asStateFlow()
    val password: StateFlow<String> = _password.asStateFlow()

    private val _outcome = MutableStateFlow<CameIn?>(null)

    /** The last answer: what came in, or why nothing did. It stays until the next one replaces it. */
    val outcome: StateFlow<CameIn?> = _outcome.asStateFlow()

    private val _report = MutableStateFlow<StoreReport?>(null)

    /** The store just joined, as its report said it: the plugins an install's answer is about. */
    val report: StateFlow<StoreReport?> = _report.asStateFlow()

    private val _asked = MutableStateFlow<Asked?>(null)

    /** The server look under way, or the one [outcome] answers. */
    val asked: StateFlow<Asked?> = _asked.asStateFlow()

    /** The question has taken 600 ms: its waiting line shows, and then stays at least 0.3 s. */
    val looking: StateFlow<Boolean> = waiting.shown

    private val _asking = MutableStateFlow(false)

    /** A question is under way, whether or not its waiting line shows yet. */
    val asking: StateFlow<Boolean> = _asking.asStateFlow()

    private val _justAdded = MutableStateFlow<Set<String>>(emptySet())

    /** Servers and folders added this session: their row lands in Your sources, and says so. */
    val justAdded: StateFlow<Set<String>> = _justAdded.asStateFlow()

    private val _installedHere = MutableStateFlow<Set<String>>(emptySet())

    /** Plugins installed one by one from the store form, for its notice. */
    val installedHere: StateFlow<Set<String>> = _installedHere.asStateFlow()

    private val _notice = MutableStateFlow<Notice?>(null)

    /** What to say once a form has closed, for 4 s, until [noticed]. */
    val notice: StateFlow<Notice?> = _notice.asStateFlow()

    private val _choosing = MutableStateFlow(false)

    /**
     * Asked from outside Your sources (the demo's last page): Your sources opens with Add a
     * source over it.
     */
    val choosing: StateFlow<Boolean> = _choosing.asStateFlow()

    fun choose() {
        _choosing.value = true
    }

    fun chose() {
        _choosing.value = false
    }

    /** A look at a server: the address as asked, and whether a password went with it. */
    data class Asked(val address: String, val withPassword: Boolean)

    /** A notice after a form closed. */
    sealed interface Notice {
        data class Added(val name: String, val series: Int) : Notice
        data class Plugins(val count: Int, val store: String) : Notice
    }

    /** Open [form], from nothing: an earlier answer is forgotten. [inPage] for first run's page. */
    fun open(form: Adding, inPage: Boolean = false) {
        if (_open.value != form) reset()
        _open.value = form
        _inPage.value = inPage
    }

    /** The form closed, by its close button or once it is done. */
    fun close() {
        _notice.value = closingNotice(_outcome.value, _installedHere.value.size) ?: _notice.value
        _open.value = null
        reset()
    }

    /** Close without a notice: first run lists what came in instead. */
    fun closeQuietly() {
        _open.value = null
        _notice.value = null
        reset()
    }

    /** The notice was said. */
    fun noticed() {
        _notice.value = null
    }

    fun typeAddress(text: String) {
        _address.value = text
    }

    fun typeUser(text: String) {
        _user.value = text
    }

    fun typePassword(text: String) {
        _password.value = text
    }

    /** Look at [address], a pasted Kavita link included, and sign in when a user is given. */
    fun probe(address: String, user: String? = null, password: String? = null) {
        val asked = address.trim()
        if (asked.isEmpty()) return
        val withPassword = !user.isNullOrBlank() && !password.isNullOrEmpty()
        _asked.value = Asked(asked, withPassword)
        _asking.value = true
        waiting.begin()
        send(
            Command.BookSources(
                BookSourceCommand.Probe(asked, user?.trim()?.takeIf { withPassword }, password?.takeIf { withPassword }),
            ),
        )
    }

    /** Add the server the look found, with the libraries ticked (none ticked asks every one). */
    fun addServer(found: ServerFound, libraries: List<String>, reportProgress: Boolean) {
        _asking.value = true
        send(Command.BookSources(BookSourceCommand.AddServer(found.pending, libraries, reportProgress)))
    }

    /** A folder was handed to the engine: its scan is under way, and its report follows. */
    fun scanning(name: String?) {
        _folder.value = name
        _outcome.value = null
        _asking.value = true
        waiting.begin()
    }

    /** A plugin installed from the store form, one by one. */
    fun installedOne(plugin: String) {
        _installedHere.value += plugin
    }

    /** Install every plugin a joined store offers for reading. */
    fun installAll(store: String) {
        _asking.value = true
        waiting.begin()
        send(Command.InstallAllForReading(store))
    }

    /** A join went out from the store form (the store model sends it): its answer comes here. */
    fun joining() {
        _asking.value = true
        waiting.begin()
    }

    /** Look at servers that showed no series, now. */
    fun recheck() = send(Command.BookSources(BookSourceCommand.Recheck))

    private fun reset() {
        waiting.cancel()
        _asking.value = false
        _outcome.value = null
        _report.value = null
        _folder.value = null
        _asked.value = null
        _address.value = ""
        _user.value = ""
        _password.value = ""
        _installedHere.value = emptySet()
    }

    internal fun apply(answer: CameIn) {
        when (answer) {
            is CameIn.ServerAdded -> {
                _justAdded.value += answer.v1.id
                if (_open.value == Adding.SERVER) {
                    // Added from the form: it closes, the row lands, and the notice says it.
                    _open.value = null
                    reset()
                }
                // A server that showed nothing and now holds some is said as when it was added.
                _notice.value = Notice.Added(answer.v1.name, answer.v1.series?.toInt() ?: 0)
                return
            }
            is CameIn.Folder -> _justAdded.value += answer.v1.id
            else -> {}
        }
        if (_open.value != formFor(answer)) {
            // An answer for a form that has closed: only what came in is worth a notice.
            closingNotice(answer, 0)?.let { _notice.value = it }
            return
        }
        reportOf(answer)?.let { _report.value = it }
        // The waiting line, once shown, stays 0.3 s: the answer, and the end of the asking, wait for it.
        waiting.end {
            _asking.value = false
            _outcome.value = answer
        }
    }

    /** For shots: the form [open] with [answer] as the engine would send it, for [asked]. */
    fun stage(
        open: Adding?,
        answer: CameIn?,
        address: String = "",
        user: String = "",
        password: String = "",
        asked: Asked? = null,
        looking: Boolean = false,
        asking: Boolean = looking,
        inPage: Boolean = false,
        folder: String? = null,
    ) {
        _open.value = open
        _inPage.value = inPage
        _folder.value = folder
        _address.value = address
        _user.value = user
        _password.value = password
        _asked.value = asked
        _outcome.value = answer
        reportOf(answer)?.let { _report.value = it }
        if (answer is CameIn.Folder) _justAdded.value += answer.v1.id
        waiting.cancel()
        _asking.value = asking
        if (looking) waiting.show()
    }

    /** For shots: the store's report, under an install's answer. */
    fun stageReport(report: StoreReport) {
        _report.value = report
    }

    /** For shots: a source added this session, and the notice it left. */
    fun stageAdded(id: String, notice: Notice?) {
        _justAdded.value += id
        _notice.value = notice
    }

    companion object {
        /** The store report an answer carries, if it carries one. */
        fun reportOf(answer: CameIn?): StoreReport? = when (answer) {
            is CameIn.Store -> answer.v1
            is CameIn.Empty -> when (val why = answer.v1) {
                is NothingCameIn.OtherMedium -> why.store
                is NothingCameIn.EmptyStore -> why.store
                else -> null
            }
            else -> null
        }

        /** The form an answer belongs to. */
        fun formFor(answer: CameIn): Adding = when (answer) {
            is CameIn.Found, is CameIn.ServerAdded -> Adding.SERVER
            is CameIn.Folder -> Adding.FOLDER
            is CameIn.Store, is CameIn.Installed -> Adding.STORE
            is CameIn.Empty -> when (answer.v1) {
                is NothingCameIn.Unreachable, is NothingCameIn.WrongPassword, is NothingCameIn.WrongKey,
                is NothingCameIn.NotABookServer, is NothingCameIn.EmptyCatalogue,
                -> Adding.SERVER
                is NothingCameIn.UnsupportedFiles, is NothingCameIn.EmptyFolder -> Adding.FOLDER
                is NothingCameIn.EmptyStore, is NothingCameIn.OtherMedium, is NothingCameIn.StoreUnreachable -> Adding.STORE
            }
        }

        /** What a form says once it closes on [answer]: what came in, or nothing to say. */
        fun closingNotice(answer: CameIn?, installedOneByOne: Int): Notice? = when (answer) {
            is CameIn.Folder -> answer.v1.takeIf { it.series > 0u }?.let { Notice.Added(it.name, it.series.toInt()) }
            is CameIn.Installed -> answer.v1.takeIf { it.installed > 0u }?.let { Notice.Plugins(it.installed.toInt(), it.name) }
            is CameIn.Store, is CameIn.Empty ->
                (answer as? CameIn.Store)?.v1?.name?.takeIf { installedOneByOne > 0 }?.let { Notice.Plugins(installedOneByOne, it) }
            null, is CameIn.Found, is CameIn.ServerAdded -> null
        }
    }
}

/** Where the server form stands. */
sealed interface ServerStep {
    /** Nothing asked about this address yet, or asked under 600 ms ago. */
    data object Typing : ServerStep

    /** Asked, and 600 ms have gone by. */
    data object Looking : ServerStep

    /** A server that wants a username and password: [failed] once a password was refused. */
    data class SignIn(val kind: BookServerKind, val host: String, val failed: Boolean) : ServerStep

    data class Found(val found: ServerFound) : ServerStep

    /** It let us in, and shows no series. */
    data class Empty(val found: ServerFound) : ServerStep

    /** Unreachable, a refused Kavita key, or not a book server. */
    data class Failed(val why: NothingCameIn) : ServerStep
}

/** The server form: where it stands, whether the sign-in fields show, and whether Add server is on. */
data class ServerForm(val step: ServerStep, val signIn: Boolean, val canAdd: Boolean, val kavita: KavitaLink?)

object CameInSteps {
    /**
     * The server form for what is typed and what was answered. An answer only counts for the
     * address it answers: typing another one hides it. A new look at the same address keeps a
     * sign-in in place until the waiting line shows, so the fields never go through empty.
     */
    fun server(
        address: String,
        user: String,
        password: String,
        asked: CameInModel.Asked?,
        outcome: CameIn?,
        asking: Boolean,
        looking: Boolean,
    ): ServerForm {
        val kavita = KavitaLink.parse(address)
        val current = asked?.takeIf { it.address == address.trim() }
        val answer = outcome?.takeIf { current != null && it.isServerAnswer() }
        val answered: ServerStep? = when (answer) {
            is CameIn.Found -> ServerStep.Found(answer.v1)
            is CameIn.Empty -> when (val why = answer.v1) {
                is NothingCameIn.EmptyCatalogue -> ServerStep.Empty(why.found)
                is NothingCameIn.WrongPassword -> ServerStep.SignIn(why.kind, why.host, current?.withPassword == true)
                else -> ServerStep.Failed(why)
            }
            else -> null
        }
        val step = when {
            current != null && asking && looking -> ServerStep.Looking
            // Asked again: a sign-in for this address stays until the line shows.
            current != null && asking -> answered?.takeIf { it is ServerStep.SignIn } ?: ServerStep.Typing
            else -> answered ?: ServerStep.Typing
        }
        // The sign-in fields stay while a password is being tried.
        val signIn = kavita == null && (step is ServerStep.SignIn || (answered is ServerStep.SignIn && asking))
        val canAdd = when (step) {
            is ServerStep.Found, is ServerStep.Empty -> !asking
            is ServerStep.SignIn -> !asking && user.isNotBlank() && password.isNotEmpty()
            else -> false
        }
        return ServerForm(step, signIn, canAdd, kavita)
    }

    private fun CameIn.isServerAnswer(): Boolean = this is CameIn.Found ||
        (this is CameIn.Empty && CameInModel.formFor(this) == Adding.SERVER)

    /** Where the folder form stands, once its folder was handed over. */
    sealed interface FolderStep {
        data object Scanning : FolderStep
        data class Found(val report: FolderReport) : FolderStep

        /** Nothing readable: [report]'s skipped files, or none at all. */
        data class Nothing(val report: FolderReport) : FolderStep
    }

    fun folder(outcome: CameIn?): FolderStep = when (outcome) {
        is CameIn.Folder -> FolderStep.Found(outcome.v1)
        is CameIn.Empty -> when (val why = outcome.v1) {
            is NothingCameIn.UnsupportedFiles -> FolderStep.Nothing(why.folder)
            is NothingCameIn.EmptyFolder -> FolderStep.Nothing(why.folder)
            else -> FolderStep.Scanning
        }
        else -> FolderStep.Scanning
    }

    /** Where the store form stands after Join. */
    sealed interface StoreStep {
        /** Not joined yet: the invite's preview and Join. */
        data object Preview : StoreStep
        data class Joined(val report: StoreReport) : StoreStep
        data class Installed(val installed: PluginsInstalled) : StoreStep
        data class OtherMedium(val report: StoreReport) : StoreStep
        data class Empty(val report: StoreReport) : StoreStep
        data class Unreachable(val host: String) : StoreStep
    }

    fun store(outcome: CameIn?): StoreStep = when (outcome) {
        is CameIn.Store -> StoreStep.Joined(outcome.v1)
        is CameIn.Installed -> StoreStep.Installed(outcome.v1)
        is CameIn.Empty -> when (val why = outcome.v1) {
            is NothingCameIn.OtherMedium -> StoreStep.OtherMedium(why.store)
            is NothingCameIn.EmptyStore -> StoreStep.Empty(why.store)
            is NothingCameIn.StoreUnreachable -> StoreStep.Unreachable(why.host)
            else -> StoreStep.Preview
        }
        else -> StoreStep.Preview
    }

    /** The store form's one filled action. */
    enum class StoreAction { INSTALL_ALL, DONE, TRY_AGAIN }

    /** Install all only when two or more plugins fit (the engine's `install_all`), Try again after no answer. */
    fun storeAction(step: StoreStep): StoreAction? = when (step) {
        StoreStep.Preview -> null
        is StoreStep.Joined -> if (step.report.installAll) StoreAction.INSTALL_ALL else StoreAction.DONE
        is StoreStep.Installed, is StoreStep.OtherMedium, is StoreStep.Empty -> StoreAction.DONE
        is StoreStep.Unreachable -> StoreAction.TRY_AGAIN
    }
}
