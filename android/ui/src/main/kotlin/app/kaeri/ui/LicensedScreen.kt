// Licensed services (AndroidLicensed): the services the viewer pays for, ticked from AniList's own
// list, the region that hides links for other countries, and whether title pages name the rest.
// Also the notice offered on coming back from a service's episode.

package app.kaeri.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.navigation.NavGraphBuilder
import androidx.navigation.compose.composable
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.LicensedView
import app.kaeri.i18n.R
import app.kaeri.store.listed
import app.kaeri.store.text

internal const val LICENSED_ROUTE = "licensed"

internal fun NavGraphBuilder.licensedPages() {
    composable(LICENSED_ROUTE) { LicensedScreen() }
}

/** A region's name in the person's language: "Denmark". */
internal fun regionName(code: String): String =
    java.util.Locale.Builder().setRegion(code).build().getDisplayCountry(java.util.Locale.getDefault()).ifEmpty { code }

/** The Sources row's line: the ticked services and the region, or the way to choose. */
@Composable
internal fun licensedRowLine(view: LicensedView?): String {
    val services = view?.settings?.services.orEmpty()
    if (services.isEmpty()) return stringResource(R.string.licensed_row_none)
    val region = view?.region?.let(::regionName) ?: stringResource(R.string.licensed_region_device)
    return stringResource(R.string.licensed_row_line, joinServices(services), region)
}

@Composable
internal fun LicensedScreen() {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val view by store.licensed.view.collectAsStateWithLifecycle()
    var all by rememberSaveable { mutableStateOf(false) }
    LaunchedEffect(Unit) { store.loadLicensed() }
    ScreenFrame(title = stringResource(R.string.licensed_title), onBack = navigator::back) { padding ->
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(top = padding.calculateTopPadding() + 8.dp, bottom = 48.dp)
                .padding(horizontal = 16.dp)
                .readingWidth(),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            SettingsFootnote(stringResource(R.string.licensed_body))
            val shown = view
            if (shown == null) {
                PlaceholderRows()
                return@Column
            }
            val regions = shown.regions.map { MenuChoice<String?>(it, regionName(it)) }
            SettingsGroup(
                footer = stringResource(R.string.licensed_region_line),
                rows = listOf { place ->
                    MenuRow(
                        place,
                        stringResource(R.string.licensed_region),
                        shown.region?.let(::regionName) ?: stringResource(R.string.licensed_region_device),
                        regions,
                        shown.region,
                    ) { store.setLicensedRegion(it) }
                },
            )
            val names = shown.listed(all)
            SettingsGroup(
                title = stringResource(R.string.licensed_yours),
                rows = names.map { name ->
                    @Composable { place: Place ->
                        val on = shown.settings.services.any { it.equals(name, ignoreCase = true) }
                        CheckRow(place, name, null, on) {
                            store.setLicensedServices(if (on) shown.settings.services.filterNot { it.equals(name, ignoreCase = true) } else shown.settings.services + name)
                        }
                    }
                } + listOfNotNull(
                    if (!all && shown.available.size > names.size) {
                        @Composable { place: Place -> LinkRow(place, stringResource(R.string.licensed_show_all, shown.available.size), null, onClick = { all = true }) }
                    } else null,
                ),
            )
            SettingsGroup(
                footer = stringResource(R.string.licensed_show_others_line),
                rows = listOf { place ->
                    SwitchRow(place, stringResource(R.string.licensed_show_others), shown.settings.showOthers) { store.setShowOtherServices(it) }
                },
            )
        }
    }
}

/**
 * Coming back to the app asks the engine whether an episode on a service was probably watched;
 * its answer is the notice "Watched episode 4 on Hikari Library?" holding Mark as watched for 8 s.
 */
@Composable
internal fun LicensedReturnNotice() {
    val store = LocalAppStore.current
    val snackbars = LocalSnackbars.current
    val context = LocalContext.current
    val owner = LocalLifecycleOwner.current
    DisposableEffect(owner) {
        val observer = LifecycleEventObserver { _, event -> if (event == Lifecycle.Event.ON_START) store.returnedToApp() }
        owner.lifecycle.addObserver(observer)
        onDispose { owner.lifecycle.removeObserver(observer) }
    }
    val returned by store.licensed.returned.collectAsStateWithLifecycle()
    LaunchedEffect(returned) {
        val said = returned ?: return@LaunchedEffect
        store.licensed.returnedShown(said)
        snackbars.show(said.prompt.text(context), context.getString(R.string.licensed_mark_watched)) {
            store.setWatched(said.id, listOf(said.episode.toString()), true)
            snackbars.show(said.done.text(context), context.getString(R.string.action_undo)) {
                store.setWatched(said.id, listOf(said.episode.toString()), false)
            }
        }
    }
}
