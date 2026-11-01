// Licensed services: the viewer's choices and the services AniList lists, what each title is
// available through, and the return notice offered after a service's episode.

package app.kaeri.store

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import app.kaeri.engine.model.AvailabilityCase
import app.kaeri.engine.model.LicensedReturn
import app.kaeri.engine.model.LicensedView
import app.kaeri.engine.model.TitleAvailability

/** What the engine last said of licensed services. */
class LicensedModel {
    private val _view = MutableStateFlow<LicensedView?>(null)

    /** The Licensed services page: choices, region and the services AniList lists. `null` before it is said. */
    val view: StateFlow<LicensedView?> = _view.asStateFlow()

    private val _availability = MutableStateFlow<Map<UInt, TitleAvailability>>(emptyMap())

    /** Each title's availability, by AniList id, as last said. */
    val availability: StateFlow<Map<UInt, TitleAvailability>> = _availability.asStateFlow()

    private val _returned = MutableStateFlow<LicensedReturn?>(null)

    /** The return notice waiting to be shown; `null` once shown or answered. */
    val returned: StateFlow<LicensedReturn?> = _returned.asStateFlow()

    internal fun apply(view: LicensedView) {
        _view.value = view
    }

    internal fun apply(availability: TitleAvailability) {
        _availability.value = _availability.value + (availability.id to availability)
    }

    internal fun returned(value: LicensedReturn) {
        _returned.value = value
    }

    /** The return notice was shown or answered. */
    fun returnedShown(value: LicensedReturn) {
        if (_returned.value == value) _returned.value = null
    }
}

/** Whether the title's main action opens a service rather than playing here. */
val TitleAvailability.opensService: Boolean get() = case == AvailabilityCase.ON_SERVICE && main != null

/** The service names a person has ticked, as the page shows them. */
val LicensedView.ticked: List<String> get() = settings.services

/** The services the page lists first: the ticked ones, then the rest by name. */
fun LicensedView.listed(all: Boolean, first: Int = 8): List<String> {
    val sorted = available.sortedBy { it.lowercase() }
    val ticked = sorted.filter { name -> settings.services.any { it.equals(name, ignoreCase = true) } }
    val rest = sorted - ticked.toSet()
    val list = ticked + rest
    return if (all || list.size <= first) list else list.take(maxOf(first, ticked.size))
}
