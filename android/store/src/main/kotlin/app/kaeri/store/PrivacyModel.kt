// Private watching and reading: whether private mode is on, which titles are always private on
// this device, and which hold a place in memory until the app quits. The engine keeps what is
// written; this is what the screens and the system surfaces read to say so, or to stay quiet.

package app.kaeri.store

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import app.kaeri.engine.core.AnilistId
import app.kaeri.engine.core.WorkId
import app.kaeri.engine.model.PrivateMode
import app.kaeri.engine.model.PrivateTitle

/** The line under a title or series page's main action, when it has one. */
enum class PrivateLine {
    /** "Private · Not saved or synced": private mode is on. */
    Session,

    /** "Private · Your place is kept until Kaeri quits": on, with a place held in memory. */
    Kept,

    /** "Always private · Not saved or synced": the title's own flag, whether the mode is on or not. */
    Always,
}

/**
 * Private mode and each title's standing in it, as the engine last said them. The mode is never
 * saved: every engine starts with it off and says so ([apply] of `Event.Private`).
 */
class PrivacyModel {
    private val _on = MutableStateFlow(false)

    /** Whether private mode is on now. */
    val on: StateFlow<Boolean> = _on.asStateFlow()

    private val _always = MutableStateFlow<Set<PrivateTitle>>(emptySet())

    /** The shows and series known to be always private on this device. */
    val always: StateFlow<Set<PrivateTitle>> = _always.asStateFlow()

    private val _kept = MutableStateFlow<Set<PrivateTitle>>(emptySet())

    /** The titles whose main action continues from a place held in memory until the app quits. */
    val kept: StateFlow<Set<PrivateTitle>> = _kept.asStateFlow()

    /** The mode, as the engine said it or as the switch just asked for it. */
    internal fun mode(on: Boolean) {
        _on.value = on
    }

    /** [title]'s own flag, as the engine said it changed or a view of it carried it. */
    internal fun flag(title: PrivateTitle, on: Boolean) {
        val now = _always.value
        if ((title in now) != on) _always.value = if (on) now + title else now - title
    }

    /** A show's standing, from `Event.Progress`: its own flag, and whether a place is held. */
    internal fun show(id: AnilistId, mode: PrivateMode?, kept: Boolean) = standing(PrivateTitle.Show(id), mode, kept)

    /** A series' standing, from its page's view. */
    internal fun series(work: WorkId, mode: PrivateMode?, kept: Boolean) = standing(PrivateTitle.Series(work), mode, kept)

    private fun standing(title: PrivateTitle, mode: PrivateMode?, kept: Boolean) {
        flag(title, mode == PrivateMode.ALWAYS)
        val now = _kept.value
        if ((title in now) != kept) _kept.value = if (kept) now + title else now - title
    }

    /** Whether [title] is always private on this device, as far as the app has been told. */
    fun isAlways(title: PrivateTitle): Boolean = title in _always.value

    /**
     * Whether what [title] shows outside the app is held: private mode is on, the title is always
     * private, or it has a place held in memory that must not reach a system surface.
     */
    fun holds(title: PrivateTitle): Boolean = _on.value || title in _always.value || title in _kept.value

    /** The line [title]'s page shows now. */
    fun line(title: PrivateTitle): PrivateLine? = line(_on.value, title in _always.value, title in _kept.value)

    companion object {
        /**
         * The line a page shows: the title's own flag first, then the mode, as
         * `PrivateMode::shown` has it. A place kept after the mode was turned off shows nothing.
         */
        fun line(on: Boolean, always: Boolean, kept: Boolean): PrivateLine? = when {
            always -> PrivateLine.Always
            on && kept -> PrivateLine.Kept
            on -> PrivateLine.Session
            else -> null
        }
    }
}
