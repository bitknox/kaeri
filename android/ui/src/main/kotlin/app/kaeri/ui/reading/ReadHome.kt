// Reading: filled in by the places work.

package app.kaeri.ui.reading

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import app.kaeri.ui.AppActions
import app.kaeri.ui.LocalAppStore

@Composable
internal fun ReadHomeScreen(actions: AppActions) {
    val store = LocalAppStore.current
    LaunchedEffect(Unit) { store.reading.loadHome() }
}
