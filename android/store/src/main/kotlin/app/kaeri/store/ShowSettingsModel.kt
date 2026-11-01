// One show's own settings: what it sets for itself over Settings, the tracks the player remembers
// for it, and the source it keeps.

package app.kaeri.store

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import app.kaeri.engine.core.AnilistId
import app.kaeri.engine.model.ShowSettingsView

/** Each show's settings sheet, as the engine last said it. */
class ShowSettingsModel {
    private val _sheets = MutableStateFlow<Map<AnilistId, ShowSettingsView>>(emptyMap())

    /** The sheets asked for or changed, by show. */
    val sheets: StateFlow<Map<AnilistId, ShowSettingsView>> = _sheets.asStateFlow()

    internal fun apply(view: ShowSettingsView) {
        _sheets.value = _sheets.value + (view.id to view)
    }
}
