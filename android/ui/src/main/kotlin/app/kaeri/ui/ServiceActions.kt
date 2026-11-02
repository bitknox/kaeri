// A title through licensed services (AndroidTitleLinks): case B puts "Watch on <service>" in the
// main action's place, case A and B add one haze line under the actions, case C names where it is
// listed. The engine works out the case (`TitleAvailability`); these draw it.

package app.kaeri.ui

import androidx.compose.animation.Crossfade
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.design.Motion
import app.kaeri.engine.core.AnilistId
import app.kaeri.engine.model.AvailabilityCase
import app.kaeri.engine.model.ServiceLink
import app.kaeri.engine.model.TitleAvailability
import app.kaeri.i18n.R
import app.kaeri.store.AppStore

/** The services named in one line: "Hikari Library and Tsuki TV", in the person's language. */
internal fun joinServices(names: List<String>): String =
    android.icu.text.ListFormatter.getInstance().format(names)

/** Hand a service's page to the system, and tell the engine, so coming back offers Mark as watched. */
internal fun AppStore.openService(id: AnilistId, link: ServiceLink, episode: Int? = null, url: String = link.url) {
    openedService(id, link.service, url, episode)
    openExternal(url)
}

/**
 * Ask what [id] is available through, again when whether a source plays it changes, and the
 * answer once it is in.
 */
@Composable
internal fun availabilityOf(id: AnilistId, playsHere: Boolean): TitleAvailability? {
    val store = LocalAppStore.current
    LaunchedEffect(id, playsHere) { store.loadAvailability(id, playsHere) }
    val all by store.licensed.availability.collectAsStateWithLifecycle()
    return all[id]
}

/** Case B's main action: amber, with the external glyph, opening the service. */
@Composable
internal fun WatchOnButton(entry: app.kaeri.engine.model.Entry, link: ServiceLink, modifier: Modifier = Modifier) {
    val store = LocalAppStore.current
    val opens = stringResource(R.string.licensed_opens_service)
    Button(
        onClick = { store.openService(entry.id, link) },
        modifier = modifier,
        contentPadding = PaddingValues(horizontal = 24.dp),
    ) {
        Text(stringResource(R.string.title_watch_on, link.service), style = MaterialTheme.typography.titleMedium, maxLines = 1)
        Spacer(Modifier.width(8.dp))
        Icon(Icons.External, contentDescription = opens, modifier = Modifier.size(18.dp))
    }
}

/**
 * The haze line under the actions. Case A and B name the person's other services, each case C
 * names where the title is listed, and offers the choice of services while none is ticked.
 */
@Composable
internal fun ServiceLines(availability: TitleAvailability?, entry: app.kaeri.engine.model.Entry, modifier: Modifier = Modifier) {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val colors = MaterialTheme.colorScheme
    Crossfade(availability, animationSpec = Motion.standard(), label = "service-lines", modifier = modifier) { shown -> // i18n-skip
        if (shown == null) return@Crossfade
        Column(Modifier.padding(top = 8.dp)) {
            val also = shown.alsoOn
            when (shown.case) {
                AvailabilityCase.PLAYS_ALSO_ON, AvailabilityCase.ON_SERVICE -> if (also.isNotEmpty()) {
                    // The first of them opens: the line is a link to the service.
                    TextButton(
                        onClick = { store.openService(entry.id, also.first()) },
                        contentPadding = PaddingValues(horizontal = 0.dp),
                        modifier = Modifier.heightIn(min = 48.dp),
                    ) {
                        Text(
                            stringResource(R.string.licensed_also_on, joinServices(also.map { it.service })),
                            style = MaterialTheme.typography.bodyMedium,
                            color = colors.onSurfaceVariant,
                        )
                        Spacer(Modifier.width(6.dp))
                        Icon(Icons.External, contentDescription = stringResource(R.string.licensed_opens_service), tint = colors.onSurfaceVariant, modifier = Modifier.size(16.dp))
                    }
                }
                AvailabilityCase.NOT_IN_SOURCES -> if (shown.others.isNotEmpty()) {
                    Text(
                        stringResource(R.string.title_listed_on, joinServices(shown.others)),
                        style = MaterialTheme.typography.bodyMedium,
                        color = colors.onSurfaceVariant,
                    )
                    TextButton(
                        onClick = { navigator.open(LICENSED_ROUTE) },
                        contentPadding = PaddingValues(horizontal = 0.dp),
                        modifier = Modifier.heightIn(min = 48.dp),
                    ) { Text(stringResource(R.string.title_choose_services), color = colors.onSurface) }
                }
                AvailabilityCase.PLAYS_HERE -> {}
            }
        }
    }
}
