// The reader over the app: a request on ReaderLauncher opens it, closing it shows the app again.
// It is the phone reader, or the e-ink reader for a request from an e-ink panel.

package app.kaeri.app

import androidx.compose.animation.ExperimentalSharedTransitionApi
import androidx.compose.animation.SharedTransitionLayout
import androidx.compose.animation.SharedTransitionScope
import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.MutableState
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.kaeri.design.ArtShape
import app.kaeri.ui.reading.LocalReaderTransitions
import app.kaeri.ui.reading.LocalReaderTransitionsView
import app.kaeri.ui.reading.LocalReaderZoomOpen
import app.kaeri.ui.reading.readerZoomKey
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import app.kaeri.design.Motion
import app.kaeri.design.atFullSpeed
import app.kaeri.design.rememberAnimationsRemoved
import app.kaeri.eink.EinkModel
import app.kaeri.eink.EinkPainter
import app.kaeri.eink.EinkQuestions
import app.kaeri.eink.EinkReader
import app.kaeri.eink.EinkReaderActions
import app.kaeri.eink.RefreshControllers
import app.kaeri.reader.PageImages
import app.kaeri.reader.ReaderSession
import app.kaeri.reader.phone.NovelScreen
import app.kaeri.reader.phone.PhoneReader
import app.kaeri.reader.phone.PhoneReaderActions
import app.kaeri.reader.phone.QuietPhone
import app.kaeri.reader.phone.ReaderLook
import app.kaeri.reader.phone.ReaderLauncher
import app.kaeri.reader.phone.ReaderRequest
import app.kaeri.reader.readerSurface
import app.kaeri.store.AppStore
import app.kaeri.ui.ArtworkImage
import app.kaeri.ui.LocalAppStore

/**
 * The app with the reader over it, in one shared transition layout so the reader can grow out of
 * the cover it opened from (see [ReaderOverlay]).
 */
@OptIn(ExperimentalSharedTransitionApi::class)
@Composable
fun ReaderHost(store: AppStore, eink: EinkModel, app: @Composable () -> Unit) {
    val view = LocalView.current
    // The cover the open reader grew out of: hidden in the app while the reader holds it.
    var zoomed by remember { mutableStateOf<String?>(null) }
    SharedTransitionLayout {
        CompositionLocalProvider(
            LocalReaderTransitions provides this,
            LocalReaderTransitionsView provides view,
            LocalReaderZoomOpen provides zoomed,
        ) {
            app()
        }
        // The cover it grows out of loads as the app's covers do.
        CompositionLocalProvider(LocalAppStore provides store) {
            ReaderOverlay(store, eink, this, zoomed) { zoomed = it }
        }
    }
}

/**
 * Opened from a cover (a request with `from`), the cover grows on the system's zoom into the
 * page's place while the pages load, and the reader crossfades over it on the quick spring once the
 * zoom is done and the first page is there; closing, the reader fades out over 0.3 s while the
 * cover shrinks back into its place. Opened any other way, the reader grows from 92% as it fades
 * in over 0.2 s, and leaves shrinking back as it fades out over 0.3 s. Remove animations keeps the
 * fades on a clock of their own and drops the zoom; Paper mode, where nothing moves, puts it there
 * and takes it away at once.
 */
@OptIn(ExperimentalSharedTransitionApi::class)
@Composable
private fun ReaderOverlay(
    store: AppStore,
    eink: EinkModel,
    layout: SharedTransitionScope,
    zoomed: String?,
    zoom: (String?) -> Unit,
) {
    val context = LocalContext.current
    val request by ReaderLauncher.request.collectAsState()
    // The request on screen: kept while the reader leaves, so it can fade with its pages.
    var shown by remember { mutableStateOf(request) }
    // The cover this open zoomed from, kept while it shrinks back.
    var zoomFrom by remember { mutableStateOf<String?>(null) }
    val progress = remember { Animatable(if (request != null) 1f else 0f) }
    // The reader's own alpha over the cover while it zooms.
    val pages = remember { Animatable(1f) }
    val corner = remember { Animatable(0f) }
    val ready = remember { mutableStateOf(false) }
    val removed = rememberAnimationsRemoved()
    suspend fun zoomSettled() {
        withTimeoutOrNull(150) { snapshotFlow { layout.isTransitionActive }.first { it } }
        withTimeoutOrNull(1500) { snapshotFlow { layout.isTransitionActive }.first { !it } }
    }
    LaunchedEffect(request) {
        val next = request
        val paper = ReaderLook.paper(context)
        if (next != null) {
            val from = next.from?.takeIf { !paper && !removed && next.cover != null && !next.eink }
            ready.value = false
            shown = next
            zoomFrom = from
            if (from != null) {
                progress.snapTo(1f)
                pages.snapTo(0f)
                corner.snapTo(ArtShape.coverRadius(Dp.Infinity).value)
                zoom(from)
                coroutineScope {
                    launch { corner.animateTo(0f, Motion.zoom()) }
                    zoomSettled()
                }
                // Page 8 crossfades over the cover once it is there; a slow source shows the
                // reader's own opening state, the same cover held dimmed in the same place.
                withTimeoutOrNull(600) { snapshotFlow { ready.value }.first { it } }
                pages.animateTo(1f, Motion.quick())
            } else {
                pages.snapTo(1f)
                when {
                    paper -> progress.snapTo(1f)
                    removed -> atFullSpeed { progress.animateTo(1f, Motion.fadeIn()) }
                    else -> progress.animateTo(1f, Motion.fadeIn())
                }
            }
        } else if (shown != null) {
            val paper = ReaderLook.paper(context)
            if (zoomFrom != null && !paper && !removed) {
                corner.snapTo(0f)
                zoom(null)
                coroutineScope {
                    launch { pages.animateTo(0f, Motion.fadeOut()) }
                    launch { corner.animateTo(ArtShape.coverRadius(Dp.Infinity).value, Motion.zoom()) }
                    zoomSettled()
                }
                progress.snapTo(0f)
            } else {
                zoom(null)
                when {
                    paper -> progress.snapTo(0f)
                    removed -> atFullSpeed { progress.animateTo(0f, Motion.fadeOut()) }
                    else -> progress.animateTo(0f, Motion.fadeOut())
                }
            }
            shown = null
            zoomFrom = null
        }
    }
    val current = shown ?: return
    val scaled = zoomFrom == null && !removed
    Box(
        Modifier.fillMaxSize().graphicsLayer {
            val p = progress.value
            alpha = p
            if (scaled) {
                scaleX = 0.92f + 0.08f * p
                scaleY = 0.92f + 0.08f * p
            }
        },
    ) {
        val from = zoomFrom
        val url = current.cover
        if (from != null && url != null) {
            // The cover the reader grows out of: centred in the page's place, a page's shape as
            // wide as it fits, its corners going square as it lands.
            BoxWithConstraints(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                val width = minOf(maxWidth, maxHeight * 0.703f)
                with(layout) {
                    Box(
                        Modifier
                            .width(width)
                            .aspectRatio(0.703f)
                            .sharedElementWithCallerManagedVisibility(
                                rememberSharedContentState(readerZoomKey(from)),
                                visible = zoomed == from,
                                boundsTransform = { _, _ -> Motion.zoom() },
                            )
                            .clip(RoundedCornerShape(corner.value.dp)),
                    ) {
                        ArtworkImage(url, plate = null, maxPixelSize = 1200, modifier = Modifier.fillMaxSize())
                    }
                }
            }
        }
        Box(Modifier.fillMaxSize().graphicsLayer { alpha = pages.value }) {
            key(current) { OpenReader(store, eink, current, leaving = request == null, ready = ready) }
        }
    }
}

@Composable
private fun OpenReader(
    store: AppStore,
    einkModel: EinkModel,
    request: ReaderRequest,
    leaving: Boolean,
    ready: MutableState<Boolean>,
) {
    val context = LocalContext.current
    val density = LocalDensity.current
    val session = remember { ReaderSession { store.send(it) } }
    val images = remember { PageImages() }
    val quiet = remember { QuietPhone(context) }
    val state by session.state.collectAsState()
    val settingsAsked by ReaderLauncher.settings.collectAsState()
    // Paper mode is known before the next open, so the reader can come in at once.
    LaunchedEffect(state.settings) { state.settings?.let { ReaderLook.remember(context, it) } }
    // Something to show in the page's place: the first page, the novel, or why there is none.
    val shows = state.leaves.isNotEmpty() || state.novel != null || state.failure != null || state.closed
    LaunchedEffect(shows) { if (shows) ready.value = true }
    val opened = remember { booleanArrayOf(false) }
    DisposableEffect(session) {
        val unbind = session.bind(store)
        onDispose {
            session.close()
            unbind()
        }
    }
    Box(
        Modifier
            .fillMaxSize()
            .readerSurface(density, eink = request.eink, dark = true) { surface ->
                if (!opened[0]) {
                    opened[0] = true
                    session.open(request.work, surface, request.chapter)
                } else {
                    session.surface(surface)
                }
            },
    ) {
        val close = { ReaderLauncher.close() }
        if (request.eink) {
            val profile by einkModel.state.collectAsState()
            // The panel's questions come first when the engine is unsure of it.
            if (profile.question != null) {
                EinkQuestions(profile, einkModel)
            } else {
                val painter = remember(profile.profile?.family, profile.profile?.panel) {
                    EinkPainter.forProfile(profile.profile) { RefreshControllers.of(it, context) }
                }
                // The Refresh settings, sent again when the person changes one.
                SideEffect { painter.policy = EinkPainter.policyOf(profile.profile) }
                EinkReader(
                    session = session,
                    state = state,
                    images = images,
                    painter = painter,
                    actions = EinkReaderActions(onBack = close, onContents = close, onSettings = {}, onSeries = close, onDownloads = close),
                )
            }
        } else {
            PhoneReader(
                session = session,
                state = state,
                images = images,
                actions = PhoneReaderActions(onClose = close),
                quiet = quiet,
                novel = { place, onControls ->
                    NovelScreen(
                        session = session,
                        state = state,
                        place = place,
                        assets = remember { store.novelAssets() },
                        resource = store::bookResource,
                        onControls = onControls,
                    )
                },
                cover = request.cover?.let { url ->
                    { modifier -> ArtworkImage(url, plate = null, maxPixelSize = 1200, modifier = modifier) }
                },
                settingsAsked = settingsAsked,
                onSettingsShown = ReaderLauncher::settingsShown,
                // Leaving, it no longer answers touches.
                enabled = !leaving,
            )
        }
    }
}
