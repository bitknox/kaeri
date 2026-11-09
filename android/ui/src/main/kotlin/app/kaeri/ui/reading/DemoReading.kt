// The reading demo in reading's places: Home's spotlight while the demo is the only thing to read,
// the demo's own series page with its credit, its rows in Search, and its one row in Your sources.
// Titles, makers, licences and blurbs come from the engine's demo manifest, never from here.

package app.kaeri.ui.reading

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.layout.layout
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withLink
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.design.ObiSilk
import app.kaeri.design.ShowTitle
import app.kaeri.engine.core.WorkId
import app.kaeri.engine.ffi.readingDemoInfo
import app.kaeri.engine.model.DemoWorkInfo
import app.kaeri.engine.model.ReadingDemoInfo
import app.kaeri.engine.model.SeriesView
import app.kaeri.engine.model.WorkTile
import app.kaeri.i18n.Strings
import app.kaeri.ui.AppActions
import app.kaeri.ui.ArtworkImage
import app.kaeri.ui.Destination
import app.kaeri.ui.Icons
import app.kaeri.ui.InfoRow
import app.kaeri.ui.LocalAppStore
import app.kaeri.ui.LocalNavigator
import app.kaeri.ui.PROVIDERS_ROUTE
import app.kaeri.ui.Place
import app.kaeri.ui.SettingsGroup
import app.kaeri.ui.pixels
import app.kaeri.ui.providerRoute

// What the engine says of each demo work. Kept in one place, so the screens read the manifest's
// fields through these alone.

/** The demo's works and hosts, from the engine's manifest. */
internal val readingDemo: ReadingDemoInfo by lazy { readingDemoInfo() }

/** The work a demo work is read under, registered with the demo source. */
internal val DemoWorkInfo.workId: WorkId? get() = work.ifEmpty { null }

/** The line written for the work, for Home's spotlight. */
internal val DemoWorkInfo.blurb: String? get() = synopsis.ifEmpty { null }

/** The work's cover: the comic's pinned first page, served from the demo's own cache. */
internal val DemoWorkInfo.coverArt: String? get() = cover

/** The demo work a series is, for its credit and licence; `null` for every other series. */
internal val SeriesView.demoWork: DemoWorkInfo? get() = demo

/** A search result from the demo. */
internal val WorkTile.fromDemo: Boolean get() = demo

/** The licence as the person reads it: "CC BY 4.0", or "Public domain" for the story. */
internal val DemoWorkInfo.licenceWords: String get() = licence.orEmpty()

/** The comic, first in the manifest. */
internal val ReadingDemoInfo.comic: DemoWorkInfo? get() = works.firstOrNull { !it.novel }

// Read the demo, asked where there is no reader to hand

/**
 * "Read the demo" in first run: the pages close and reading's Home opens the reader on the comic,
 * as Home is where the demo is read from afterwards and where the reader comes back to.
 */
internal object DemoLaunch {
    var asked by mutableStateOf(false)
}

/** Open the reader on the comic once asked from first run and the engine has named its work. */
@Composable
internal fun DemoLaunchHost(actions: AppActions) {
    val asked = DemoLaunch.asked
    LaunchedEffect(asked) {
        if (!asked) return@LaunchedEffect
        DemoLaunch.asked = false
        val comic = readingDemo.comic ?: return@LaunchedEffect
        val work = comic.workId ?: return@LaunchedEffect
        actions.openReader(work, null, comic.coverArt, null)
    }
}

// Covers

/** A demo work's cover, from the engine's demo cache, on the ground's raised plate while it comes. */
@Composable
internal fun DemoCover(work: DemoWorkInfo?, width: Dp, modifier: Modifier = Modifier, radius: Dp = if (width >= 100.dp) 8.dp else 6.dp) {
    Box(modifier.size(width, width * 1.5f).clip(RoundedCornerShape(radius))) {
        ArtworkImage(work?.coverArt, plate = ObiSilk.RaisedSelected, maxPixelSize = pixels(width * 1.5f), modifier = Modifier.fillMaxSize())
    }
}

// Home

/**
 * Home's spotlight while the demo is the only thing to read: the comic's art across, darkened
 * from the foot, with Demo, free to share, its title and its line over it, and Read, the one amber
 * action. 230 high on a phone, as the board draws it; beside the shelves' width on a tablet.
 */
@Composable
internal fun DemoHomeSpotlight(actions: AppActions, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    val comic = readingDemo.comic ?: return
    val work = comic.workId
    val wide = LocalContext.current.resources.configuration.screenWidthDp >= 700
    val key = work?.let { readerCoverKey("spotlight", it) }
    BoxWithConstraints(
        modifier
            .padding(horizontal = 16.dp)
            .widthIn(max = 820.dp)
            .fillMaxWidth()
            .height(if (wide) 300.dp else 230.dp)
            .clip(RoundedCornerShape(if (wide) 14.dp else 20.dp))
            .then(if (work != null) Modifier.clickable { navigator.openSeries(work, "read-spotlight") } else Modifier),
    ) {
        // The cover is the comic's first page, three panels on white paper. The first panel alone
        // fills the spotlight, so no paper margin or gutter shows.
        val page = maxOf(maxWidth / 0.95f, maxHeight / 0.44f)
        Box(Modifier.fillMaxSize().then(key?.let { Modifier.readerZoomSource(it) } ?: Modifier)) {
            ArtworkImage(
                comic.coverArt,
                plate = ObiSilk.RaisedSelected,
                maxPixelSize = pixels(page * 1.5f),
                modifier = Modifier.layout { measurable, constraints ->
                    val width = page.roundToPx()
                    val placeable = measurable.measure(Constraints.fixed(width, width * 3 / 2))
                    layout(constraints.maxWidth, constraints.maxHeight) {
                        placeable.place((constraints.maxWidth - width) / 2, constraints.maxHeight / 2 - (width * 0.28f).toInt())
                    }
                },
            )
        }
        Box(
            Modifier
                .fillMaxSize()
                .background(Brush.verticalGradient(0f to ObiSilk.Ground.copy(alpha = 0.15f), 0.4f to ObiSilk.Ground.copy(alpha = 0.55f), 1f to ObiSilk.Ground.copy(alpha = 0.92f))),
        )
        val inset = if (wide) 28.dp else 18.dp
        Column(
            Modifier
                .align(Alignment.BottomStart)
                .padding(start = inset, end = inset, bottom = if (wide) 26.dp else 18.dp)
                .semantics(mergeDescendants = false) {},
        ) {
            Text(
                Strings.homeSpotlightDemo(context).uppercase(),
                style = MaterialTheme.typography.labelSmall.copy(fontSize = 11.sp, lineHeight = 16.sp, letterSpacing = 0.9.sp, fontWeight = FontWeight.Medium),
                color = ObiSilk.Silk.copy(alpha = 0.8f),
            )
            Text(
                comic.series,
                style = ShowTitle.page.copy(fontSize = if (wide) 30.sp else 24.sp, lineHeight = if (wide) 38.sp else 30.sp),
                color = ObiSilk.Silk,
                modifier = Modifier.padding(top = 4.dp, bottom = 6.dp).semantics { heading() },
            )
            comic.blurb?.let {
                Text(
                    it,
                    style = MaterialTheme.typography.bodyMedium,
                    color = ObiSilk.Silk.copy(alpha = 0.85f),
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.padding(bottom = 14.dp),
                )
            }
            if (work != null) {
                Button(
                    onClick = { actions.openReader(work, null, comic.coverArt, key) },
                    modifier = Modifier.heightIn(min = if (wide) 48.dp else 56.dp),
                    shape = RoundedCornerShape(28.dp),
                    contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 28.dp),
                ) { Text(Strings.homeSpotlightRead(context), style = MaterialTheme.typography.titleMedium) }
            }
        }
    }
}

/**
 * While the demo is the only thing to read: one line under its spotlight with Add a source beside
 * it, stacked under the words when large text would squeeze them.
 */
@Composable
internal fun DemoOnlyLine(modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    val stacked = androidx.compose.ui.platform.LocalDensity.current.fontScale > 1.3f
    val words = @Composable { m: Modifier ->
        Text(Strings.homeDemoOnlyBooks(context), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = m)
    }
    val button = @Composable { m: Modifier ->
        FilledTonalButton(onClick = { navigator.open(PROVIDERS_ROUTE) }, modifier = m.heightIn(min = 40.dp)) {
            Text(Strings.sourcesAddTitle(context))
        }
    }
    if (stacked) {
        Column(modifier.padding(start = 16.dp, end = 16.dp, top = 16.dp)) {
            words(Modifier)
            button(Modifier.padding(top = 12.dp))
        }
    } else {
        Row(
            modifier.padding(start = 16.dp, end = 16.dp, top = 16.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            words(Modifier.weight(1f))
            button(Modifier)
        }
    }
}

// The series page

/**
 * A demo work's page: the cover beside its facts, title and credit with the licence linked, Read
 * chapter 1, its one chapter, and the demo's other work. No lists, tracking or downloads: the demo
 * is for trying the reader, never told to a tracker.
 */
@Composable
internal fun DemoSeriesBody(series: SeriesView, demo: DemoWorkInfo, actions: AppActions, wide: Boolean, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val colors = MaterialTheme.colorScheme
    val chapterMap by store.reading.chapters.collectAsStateWithLifecycle()
    val first = chapterMap[series.work]?.volumes?.flatMap { it.chapters }?.firstOrNull()
    val number = first?.number ?: "1" // i18n-skip
    val coverKey = readerCoverKey("series", series.work)
    val read = { actions.openReader(series.work, first?.id, demo.coverArt, coverKey) }
    Column(
        modifier
            .then(if (wide) Modifier.widthIn(max = 720.dp) else Modifier)
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 16.dp)
            .padding(bottom = 24.dp),
    ) {
        Row(horizontalArrangement = Arrangement.spacedBy(if (wide) 24.dp else 16.dp)) {
            DemoCover(demo, if (wide) 180.dp else 120.dp, Modifier.readerZoomSource(coverKey).clickable(onClick = read))
            Column(Modifier.weight(1f)) {
                Text(
                    demoFacts(context, demo).uppercase(),
                    style = MaterialTheme.typography.labelSmall.copy(fontSize = 11.sp, lineHeight = 16.sp, letterSpacing = 0.9.sp, fontWeight = FontWeight.Medium),
                    color = colors.onSurfaceVariant,
                )
                Text(
                    demo.series,
                    style = ShowTitle.page.copy(fontSize = if (wide) 30.sp else 24.sp, lineHeight = if (wide) 36.sp else 30.sp),
                    color = colors.onSurface,
                    modifier = Modifier.padding(top = 4.dp).semantics { heading() },
                )
                DemoCreditLine(demo, linked = true, modifier = Modifier.padding(top = 8.dp))
                if (wide) DemoReadButton(number, read, Modifier.padding(top = 16.dp))
            }
        }
        if (!wide) DemoReadButton(number, read, Modifier.padding(top = 16.dp))
        SettingsGroup(
            title = Strings.demoChapters(context),
            modifier = Modifier.padding(top = 10.dp),
            rows = listOf({ place ->
                InfoRow(
                    place,
                    Strings.readingChapterTitled(context, number, first?.title ?: demo.chapter),
                    demo.pages.toInt().takeIf { it > 0 }?.let { Strings.demoPages(context, it) },
                    onClick = { actions.openReader(series.work, first?.id, demo.coverArt, null) },
                    trailing = { Chevron() },
                )
            }),
        )
        val other = readingDemo.works.firstOrNull { it.id != demo.id }
        val otherWork = series.demoOther
        if (other != null && otherWork != null) {
            SettingsGroup(
                title = Strings.demoAlso(context),
                modifier = Modifier.padding(top = 6.dp),
                rows = listOf({ place ->
                    InfoRow(
                        place,
                        other.series,
                        demoRowLine(context, other),
                        onClick = { navigator.openSeries(otherWork, "demo-also") },
                        leading = { DemoCover(other, 36.dp, radius = 4.dp) },
                        trailing = { Chevron() },
                    )
                }),
            )
        }
    }
}

/** The demo work's page in its frame: Demo over it, Back, and no menu. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun DemoSeriesScreen(series: SeriesView, demo: DemoWorkInfo, actions: AppActions) {
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    val wide = LocalContext.current.resources.configuration.screenWidthDp >= 700
    Scaffold(
        containerColor = MaterialTheme.colorScheme.surface,
        topBar = {
            TopAppBar(
                title = { Text(Strings.sourcesDemoName(context)) },
                navigationIcon = {
                    IconButton(onClick = navigator::back) { Icon(Icons.Back, contentDescription = Strings.actionBack(context)) }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = MaterialTheme.colorScheme.surface),
            )
        },
    ) { padding ->
        Box(Modifier.fillMaxSize().padding(top = padding.calculateTopPadding()), contentAlignment = Alignment.TopCenter) {
            DemoSeriesBody(series, demo, actions, wide, Modifier.padding(top = 8.dp))
        }
    }
}

/** Read chapter 1: amber, 56 high, as wide as its words. */
@Composable
private fun DemoReadButton(number: String, onClick: () -> Unit, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    Button(
        onClick = onClick,
        modifier = modifier.heightIn(min = 56.dp),
        shape = RoundedCornerShape(28.dp),
        contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 28.dp),
    ) { Text(Strings.demoReadChapter(context, number), style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold) }
}

/** "Demo · Comic · 3 pages" for the comic; the story's row line for the story. */
private fun demoFacts(context: android.content.Context, demo: DemoWorkInfo): String =
    if (demo.novel) Strings.demoStoryRow(context, demo.author, demo.licenceWords) else Strings.demoSeriesFacts(context, demo.pages.toInt())

/** A demo work as a row's second line: the story's maker and licence, the comic's facts. */
internal fun demoRowLine(context: android.content.Context, demo: DemoWorkInfo): String = demoFacts(context, demo)

/** The credit the licence asks for, with Licence linked where the work has one. */
@Composable
internal fun DemoCreditLine(demo: DemoWorkInfo, linked: Boolean, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val uri = LocalUriHandler.current
    val colors = MaterialTheme.colorScheme
    val credit = demoCredit(context, demo)
    val url = demo.licenceUrl
    val text = buildAnnotatedString {
        append(credit)
        if (linked && url != null) {
            append(" ")
            withLink(
                LinkAnnotation.Clickable(
                    "licence", // i18n-skip
                    TextLinkStyles(SpanStyle(color = colors.onSurface, textDecoration = TextDecoration.Underline)),
                ) { uri.openUri(url) },
            ) { append(Strings.sourcesDemoLinkLicence(context)) }
        }
    }
    Text(text, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant, modifier = modifier)
}

/** "The Star Post by Mina Aster. Licensed CC BY 4.0. Read from Wikimedia Commons.", or the story's. */
internal fun demoCredit(context: android.content.Context, demo: DemoWorkInfo): String =
    if (demo.novel || demo.licenceUrl == null) {
        Strings.demoCreditStory(context, demo.series, demo.author)
    } else {
        Strings.demoCreditBook(context, demo.series, demo.author, demo.licenceWords)
    }

@Composable
private fun Chevron() {
    Icon(Icons.Chevron, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
}

// Your sources

/**
 * The demo's one row among the book sources: Demo, what it holds for reading, and About the demo
 * behind it. Remove demo there takes the episode, the comic and the story together.
 */
@Composable
internal fun BookDemoRow(place: Place) {
    val context = LocalContext.current
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    InfoRow(
        place,
        Strings.sourcesDemoName(context),
        Strings.sourcesDemoBooksLine(context),
        onClick = {
            store.checkProviders()
            navigator.open(providerRoute("demo")) // i18n-skip
        },
        leading = { DemoIconTile(Icons.Read) },
        trailing = { Chevron() },
    )
}

/** The demo's row in a group of its own, first in Your sources while reading's demo is on. */
@Composable
internal fun BookDemoGroup() {
    val store = LocalAppStore.current
    val demo by store.demo.collectAsStateWithLifecycle()
    if (demo.reading) SettingsGroup(rows = listOf({ place -> BookDemoRow(place) }))
}

/** A row's icon on its 36 tile, as the board's source rows draw it. */
@Composable
internal fun DemoIconTile(icon: androidx.compose.ui.graphics.vector.ImageVector) {
    Box(
        Modifier.size(36.dp).clip(RoundedCornerShape(9.dp)).background(MaterialTheme.colorScheme.secondaryContainer),
        contentAlignment = Alignment.Center,
    ) { Icon(icon, contentDescription = null, modifier = Modifier.size(18.dp), tint = MaterialTheme.colorScheme.onSurface) }
}

// Search

/** Under the results while only the demo is on: other titles appear once a source is added. */
@Composable
internal fun DemoSearchNote(modifier: Modifier = Modifier) {
    val context = LocalContext.current
    Text(
        Strings.searchDemoOnlyBooks(context),
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = modifier.padding(top = 4.dp, bottom = 12.dp),
    )
}
