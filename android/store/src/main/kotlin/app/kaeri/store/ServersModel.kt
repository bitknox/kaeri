// Media servers: the Add a media server flow's state, and the saved servers as the Sources page
// and a server's page show them.

package app.kaeri.store

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import app.kaeri.engine.model.CertificateCheck
import app.kaeri.engine.model.CheckedServer
import app.kaeri.engine.model.FoundServer
import app.kaeri.engine.model.LibraryChoice
import app.kaeri.engine.model.ServerEvent
import app.kaeri.engine.model.ServerFailure
import app.kaeri.engine.model.ServerRow
import app.kaeri.engine.model.ServerStep

/** A step of the Add a media server flow. */
enum class AddServerStep { Find, SignIn, QuickConnect, Libraries }

/** A failure and the step whose field or button it sits under. */
data class ServerProblem(val step: ServerStep, val failure: ServerFailure)

/** What the flow knows, from the first address typed until the server is added. */
data class AddServerState(
    val step: AddServerStep = AddServerStep.Find,
    /** Servers found on the network, as they arrive. */
    val found: List<FoundServer> = emptyList(),
    /** Whether the broadcast has ended. */
    val discoveryDone: Boolean = false,
    /** The address being checked or signed in to. */
    val address: String = "",
    val checking: Boolean = false,
    val checked: CheckedServer? = null,
    val signingIn: Boolean = false,
    val problem: ServerProblem? = null,
    /** The certificate to compare when the server's is not trusted. */
    val certificate: CertificateCheck? = null,
    val code: String? = null,
    /** The server just signed in to, whose libraries are chosen next. */
    val server: String? = null,
    val libraries: List<LibraryChoice> = emptyList(),
)

class ServersModel {
    private val _flow = MutableStateFlow<AddServerState?>(null)

    /** The open Add a media server flow; `null` when none is open. */
    val flow: StateFlow<AddServerState?> = _flow.asStateFlow()

    private val _rows = MutableStateFlow<List<ServerRow>>(emptyList())

    /** The saved servers. */
    val rows: StateFlow<List<ServerRow>> = _rows.asStateFlow()

    private val _loaded = MutableStateFlow(false)
    val loaded: StateFlow<Boolean> = _loaded.asStateFlow()

    internal fun opened() {
        _flow.value = AddServerState()
    }

    fun closed() {
        _flow.value = null
    }

    internal fun update(change: (AddServerState) -> AddServerState) {
        _flow.value = _flow.value?.let(change)
    }

    internal fun checking(address: String) = update { it.copy(address = address, checking = true, problem = null, certificate = null) }

    internal fun signingIn(address: String) = update { it.copy(address = address, signingIn = true, problem = null) }

    internal fun back() = update {
        when (it.step) {
            AddServerStep.Find -> it
            else -> it.copy(step = AddServerStep.Find, problem = null, checked = null, code = null, signingIn = false, checking = false)
        }
    }

    internal fun usePassword() = update { it.copy(step = AddServerStep.SignIn, problem = null, code = null) }

    internal fun apply(event: ServerEvent) {
        when (event) {
            is ServerEvent.Found -> update { s ->
                if (s.found.any { it.id == event.v1.id }) s else s.copy(found = s.found + event.v1)
            }
            ServerEvent.DiscoveryDone -> update { it.copy(discoveryDone = true) }
            is ServerEvent.Checked -> update {
                it.copy(checking = false, checked = event.v1, address = event.v1.address, problem = null, step = AddServerStep.SignIn)
            }
            is ServerEvent.Failed -> update {
                it.copy(checking = false, signingIn = false, problem = ServerProblem(event.step, event.failure))
            }
            is ServerEvent.QuickConnectCode -> update {
                it.copy(step = AddServerStep.QuickConnect, code = event.code, problem = null)
            }
            is ServerEvent.SignedIn -> update {
                it.copy(step = AddServerStep.Libraries, server = event.server, signingIn = false, problem = null, code = null)
            }
            is ServerEvent.Libraries -> update {
                if (it.server == null || it.server == event.server) it.copy(libraries = event.libraries) else it
            }
            is ServerEvent.Certificate -> update { it.copy(certificate = event.v1) }
            is ServerEvent.Servers -> {
                _rows.value = event.v1
                _loaded.value = true
            }
        }
    }

    fun row(id: String): ServerRow? = _rows.value.firstOrNull { it.id == id }
}
