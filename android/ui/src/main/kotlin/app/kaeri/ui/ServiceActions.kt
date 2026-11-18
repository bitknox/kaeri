// A title through licensed services (AndroidTitleLinks): case B puts "Watch on <service>" in the
// main action's place, case A and B add one haze line under the actions, case C names where it is
// listed. The engine works out the case (`TitleAvailability`); these draw it.

package app.kaeri.ui

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.Crossfade
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
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
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.clearAndSetSemantics
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

/** Whether there is a line to draw under the actions: services to name, or where it is listed. */
internal val TitleAvailability?.hasLine: Boolean
    get() = when (this?.case) {
        AvailabilityCase.PLAYS_ALSO_ON, AvailabilityCase.ON_SERVICE -> alsoOn.isNotEmpty()
        AvailabilityCase.NOT_IN_SOURCES -> others.isNotEmpty()
        AvailabilityCase.PLAYS_HERE, null -> false
    }

/**
 * The haze line under the actions. Case A and B name the person's other services, each case C
 * names where the title is listed, and offers the choice of services while none is ticked.
 * Until it is known where the title is listed, a placeholder line keeps the line's room from the
 * first frame, like the title page's other placeholders, and the answer replaces it in one
 * crossfade. An answer with no line, or none by [OFFERS_WAIT], closes the room in that crossfade,
 * on standard, as rows fit once known.
 */
@Composable
internal fun ServiceLines(availability: TitleAvailability?, entry: app.kaeri.engine.model.Entry, modifier: Modifier = Modifier) {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val colors = MaterialTheme.colorScheme
    // No answer comes when AniList cannot be reached: the room closes once the main button stops
    // waiting for one.
    var waited by remember(entry.id) { mutableStateOf(false) }
    LaunchedEffect(entry.id) {
        kotlinx.coroutines.delay(OFFERS_WAIT)
        waited = true
    }
    // The answer counts once it is for what the episodes say of the sources. Before they come the
    // title is taken to play here, and the answer for that would show only to give way to the next.
    val rows = store.titles.episodes.collectAsStateWithLifecycle().value[entry.id]
    val unlisted = rows?.firstOrNull()?.metadataOnly == true
    val counted = availability?.takeIf { if (rows != null) it.playsHere == !unlisted else waited }
    // The last answer that had a line, which the lines keep showing while they close.
    var kept by remember(entry.id) { mutableStateOf(counted?.takeIf { it.hasLine }) }
    if (counted.hasLine) kept = counted
    val pending = counted == null && kept == null && !waited
    AnimatedVisibility(
        pending || counted.hasLine,
        modifier = modifier,
        enter = expandVertically(Motion.standard(), expandFrom = Alignment.Top) + fadeIn(Motion.fadeIn()),
        exit = shrinkVertically(Motion.standard(), shrinkTowards = Alignment.Top) + fadeOut(Motion.standard()),
        label = "service-lines", // i18n-skip
    ) {
        // The placeholder giving way to the answer, or one answer with a line to another,
        // crossfades in place.
        Crossfade(kept, animationSpec = Motion.standard(), label = "service-lines-change") { shown -> // i18n-skip
            if (shown == null) {
                ServiceLinePlaceholder(unlisted)
                return@Crossfade
            }
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
}

/** How long the services line waits for its answer before its room closes: the Apple app's 2 s. */
internal const val OFFERS_WAIT = 2_000L

/** Whether a source set up plays the title, by the answer's case. */
internal val TitleAvailability.playsHere: Boolean
    get() = case == AvailabilityCase.PLAYS_HERE || case == AvailabilityCase.PLAYS_ALSO_ON

/**
 * The services line's room while it is not yet known where the title is listed. Once the episodes
 * say no source plays it, [unlisted], the answer will be where it is listed and the way to choose
 * services: the room holds both, the first wrapped as its words will be.
 */
@Composable
private fun ServiceLinePlaceholder(unlisted: Boolean) {
    Column(Modifier.testTag(TITLE_PLACEHOLDER).padding(top = 8.dp).fillMaxWidth().animateContentSize(Motion.standard())) {
        if (unlisted) {
            Box {
                Text(
                    stringResource(R.string.title_listed_on, "Lantern Library"), // i18n-skip
                    style = MaterialTheme.typography.bodyMedium,
                    color = Color.Transparent,
                    modifier = Modifier.clearAndSetSemantics {},
                )
                TextPlaceholder(0.85f, 20.dp)
            }
        }
        Box(Modifier.testTag(SERVICES_PLACEHOLDER).fillMaxWidth().heightIn(min = 48.dp), contentAlignment = Alignment.CenterStart) {
            TextPlaceholder(if (unlisted) 0.4f else 0.5f, 20.dp)
        }
    }
}
