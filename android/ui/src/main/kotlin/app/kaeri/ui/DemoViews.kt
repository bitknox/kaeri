// The demo's own views: the Try it now card in first run, About the demo with the credits and the
// hosts it reaches, and the credit block itself, which shows wherever the work shows as a title.

package app.kaeri.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.addPathNodes
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.design.ObiSilk
import app.kaeri.engine.ffi.demoInfo
import app.kaeri.engine.model.BookSourceCommand
import app.kaeri.engine.model.Command
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.ui.reading.DemoCover
import app.kaeri.ui.reading.DemoIconTile
import app.kaeri.ui.reading.DemoLaunch
import app.kaeri.ui.reading.comic
import app.kaeri.ui.reading.demoCredit
import app.kaeri.ui.reading.readingDemo

/**
 * The credit block the licence asks for: the work, its makers and licence, with links to the
 * licence, the file page on Wikimedia Commons and the makers. Nothing here plays or fetches.
 */
@Composable
internal fun DemoCredit(modifier: Modifier = Modifier) {
    val info = androidx.compose.runtime.remember { demoInfo() }
    val uri = LocalUriHandler.current
    Column(modifier, verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Text(info.credit, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            TextButton(onClick = { uri.openUri(info.licenceUrl) }) { Text(stringResource(R.string.sources_demo_link_licence)) }
            TextButton(onClick = { uri.openUri(info.filePageUrl) }) { Text(stringResource(R.string.sources_demo_link_commons)) }
            TextButton(onClick = { uri.openUri(info.makerUrl) }) { Text(stringResource(R.string.sources_demo_link_maker)) }
        }
    }
}

/**
 * Try it now, on the books page of first run: the demo's covers beside what it holds, with Read
 * the demo, and Play the demo too with Both. Tonal, so Done stays the page's one amber action.
 * Showing it asks the engine to fetch the comic's pages, so the reader opens on them.
 */
@Composable
internal fun DemoTryCard(both: Boolean, done: () -> Unit, modifier: Modifier = Modifier) {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val colors = MaterialTheme.colorScheme
    LaunchedEffect(Unit) { store.send(Command.BookSources(BookSourceCommand.PrefetchDemo)) }
    val works = readingDemo.works
    val comic = readingDemo.comic
    val story = works.firstOrNull { it.novel }
    val shape = RoundedCornerShape(16.dp)
    Row(
        modifier
            .fillMaxWidth()
            .clip(shape)
            .background(ObiSilk.Raised)
            .border(1.dp, colors.outlineVariant.copy(alpha = 0.6f), shape)
            .padding(horizontal = 16.dp, vertical = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            if (both) AnimeDemoPlate() else DemoCover(comic, 64.dp)
            DemoCover(if (both) comic else story, 64.dp)
        }
        Column(Modifier.weight(1f)) {
            Text(
                stringResource(R.string.onboarding_demo_heading),
                style = MaterialTheme.typography.titleMedium.copy(fontSize = 16.sp, lineHeight = 24.sp, fontWeight = FontWeight.SemiBold),
                color = colors.onSurface,
                modifier = Modifier.semantics { heading() },
            )
            Text(
                stringResource(if (both) R.string.onboarding_demo_both_detail else R.string.onboarding_demo_books_detail),
                style = MaterialTheme.typography.bodyMedium,
                color = colors.onSurfaceVariant,
            )
            if (both) {
                DemoTonal(stringResource(R.string.onboarding_play_demo), Modifier.padding(top = 10.dp)) {
                    store.playDemo()
                    done()
                }
            }
            DemoTonal(stringResource(R.string.onboarding_read_demo), Modifier.padding(top = if (both) 8.dp else 10.dp)) {
                DemoLaunch.asked = true
                if (both) navigator.go(Destination.Read)
                done()
            }
        }
    }
}

@Composable
private fun DemoTonal(label: String, modifier: Modifier = Modifier, onClick: () -> Unit) {
    FilledTonalButton(onClick = onClick, modifier = modifier.heightIn(min = 40.dp), contentPadding = PaddingValues(horizontal = 16.dp)) { Text(label) }
}

/** The episode's place beside the comic: its night colours, as its cover is not known here. */
@Composable
private fun AnimeDemoPlate() {
    Box(
        Modifier
            .size(64.dp, 96.dp)
            .clip(RoundedCornerShape(6.dp))
            .background(Brush.verticalGradient(listOf(Color(0xFF4A3A63), Color(0xFF241C33)))),
    )
}

/** A bin, for Remove demo's tile. */
internal val TrashIcon: ImageVector = ImageVector.Builder(
    name = "Trash", defaultWidth = 24.dp, defaultHeight = 24.dp, viewportWidth = 24f, viewportHeight = 24f,
).apply {
    for (path in listOf("M4 7 H20", "M9 7 V4.5 H15 V7", "M6.5 7 L7.5 20 H16.5 L17.5 7", "M10.5 11 V16", "M13.5 11 V16")) {
        addPath(
            pathData = addPathNodes(path),
            stroke = SolidColor(Color.Black),
            strokeLineWidth = 2f,
            strokeLineCap = StrokeCap.Round,
            strokeLineJoin = StrokeJoin.Round,
        )
    }
}.build()

/**
 * About the demo. With reading's demo on: what it is, its credits with the comic's licence linked,
 * the hosts it reaches and that it never signs in, and Remove demo, which takes every part of it.
 * With only the episode: the credit block, the hosts, and Remove demo.
 */
@Composable
internal fun DemoAbout(name: String) {
    val store = LocalAppStore.current
    val demo by store.demo.collectAsStateWithLifecycle()
    if (demo.reading) BooksDemoAbout(watching = demo.watching) else ShowDemoAbout(name)
}

@Composable
private fun BooksDemoAbout(watching: Boolean) {
    val context = androidx.compose.ui.platform.LocalContext.current
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val uri = LocalUriHandler.current
    val colors = MaterialTheme.colorScheme
    val licence = stringResource(R.string.sources_demo_link_licence)
    val licenceLink: @Composable (String?) -> Unit = { url ->
        // Haze, as the board draws it: Remove demo's page has no amber action.
        if (url != null) {
            TextButton(onClick = { uri.openUri(url) }, colors = ButtonDefaults.textButtonColors(contentColor = colors.onSurfaceVariant)) {
                Text(licence, style = MaterialTheme.typography.bodyLarge)
            }
        }
    }
    ScreenFrame(title = stringResource(R.string.sources_demo_about), onBack = navigator::back) { padding ->
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(top = padding.calculateTopPadding() + 8.dp, bottom = 48.dp)
                .padding(horizontal = 16.dp)
                .readingWidth(),
        ) {
            Text(
                stringResource(R.string.sources_demo_books_what),
                style = MaterialTheme.typography.bodyLarge,
                color = colors.onSurface,
            )
            val credits = buildList<@Composable (Place) -> Unit> {
                if (watching) {
                    val show = demoInfo()
                    add { place -> InfoRow(place, show.credit, null, titleStyle = MaterialTheme.typography.bodyMedium, trailing = { licenceLink(show.licenceUrl) }) }
                }
                for (work in readingDemo.works) {
                    add { place ->
                        InfoRow(
                            place,
                            work.series,
                            listOfNotNull(
                                demoCredit(context, work),
                                // The story's cover is a 1904 frontispiece, credited under the story.
                                work.coverCredit?.let { Strings.demoCreditCover(context, it.title, it.artist, it.year.toString()) },
                            ).joinToString("\n"), // i18n-skip
                            leading = { DemoCover(work, 36.dp, radius = 4.dp) },
                            trailing = { if (!work.novel) licenceLink(work.licenceUrl) },
                        )
                    }
                }
            }
            SettingsGroup(title = stringResource(R.string.demo_credits), modifier = Modifier.padding(top = 24.dp), rows = credits)
            val bodyRow: @Composable (Place, String) -> Unit = { place, text ->
                InfoRow(place, text, null, titleStyle = MaterialTheme.typography.bodyLarge)
            }
            SettingsGroup(
                modifier = Modifier.padding(top = 16.dp),
                rows = listOf(
                    { place -> bodyRow(place, stringResource(R.string.sources_demo_hosts)) },
                    { place -> bodyRow(place, stringResource(R.string.sources_demo_books_private)) },
                ),
            )
            SettingsGroup(
                modifier = Modifier.padding(top = 16.dp),
                rows = listOf({ place ->
                    InfoRow(
                        place,
                        stringResource(R.string.sources_demo_remove),
                        null,
                        leading = { DemoIconTile(TrashIcon) },
                        onClick = {
                            store.removeDemo()
                            navigator.back()
                        },
                    )
                }),
            )
        }
    }
}

/** About the demo: the credit block, the hosts it reaches, that it never signs in, and Remove demo. */
@Composable
private fun ShowDemoAbout(name: String) {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    ScreenFrame(title = stringResource(R.string.sources_demo_about), subtitle = name, onBack = navigator::back) { padding ->
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(top = padding.calculateTopPadding() + 8.dp, bottom = 48.dp)
                .padding(horizontal = 16.dp)
                .readingWidth(),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            DemoCredit(Modifier.padding(horizontal = 16.dp))
            Text(
                stringResource(R.string.sources_demo_hosts),
                style = MaterialTheme.typography.bodyMedium,
                modifier = Modifier.padding(horizontal = 16.dp),
            )
            Text(
                stringResource(R.string.sources_demo_private),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(horizontal = 16.dp),
            )
            OutlinedButton(
                onClick = {
                    store.removeDemo()
                    navigator.back()
                },
                modifier = Modifier.padding(horizontal = 16.dp),
            ) { Text(stringResource(R.string.sources_demo_remove)) }
        }
    }
}
