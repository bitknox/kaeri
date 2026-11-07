// The reader over the app: a request on ReaderLauncher opens it, closing it shows the app again.
// It is the phone reader, or the e-ink reader for a request from an e-ink panel.

package app.kaeri.app

import androidx.compose.animation.core.Animatable
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

/**
 * The reader comes in on the system's zoom, growing from 92% as it fades in over 0.2 s, and
 * leaves shrinking back as it fades out over 0.3 s. Remove animations keeps the fades on a clock of
 * their own; Paper mode, where nothing moves, puts it there and takes it away at once.
 */
@Composable
fun ReaderOverlay(store: AppStore, eink: EinkModel) {
    val context = LocalContext.current
    val request by ReaderLauncher.request.collectAsState()
    // The request on screen: kept while the reader leaves, so it can fade with its pages.
    var shown by remember { mutableStateOf(request) }
    val progress = remember { Animatable(if (request != null) 1f else 0f) }
    val removed = rememberAnimationsRemoved()
    LaunchedEffect(request) {
        val next = request
        val paper = ReaderLook.paper(context)
        if (next != null) {
            shown = next
            if (paper) {
                progress.snapTo(1f)
            } else if (removed) {
                atFullSpeed { progress.animateTo(1f, Motion.fadeIn()) }
            } else {
                progress.animateTo(1f, Motion.fadeIn())
            }
        } else if (shown != null) {
            when {
                paper -> progress.snapTo(0f)
                removed -> atFullSpeed { progress.animateTo(0f, Motion.fadeOut()) }
                else -> progress.animateTo(0f, Motion.fadeOut())
            }
            shown = null
        }
    }
    val current = shown ?: return
    Box(
        Modifier.fillMaxSize().graphicsLayer {
            val p = progress.value
            alpha = p
            if (!removed) {
                scaleX = 0.92f + 0.08f * p
                scaleY = 0.92f + 0.08f * p
            }
        },
    ) {
        key(current) { OpenReader(store, eink, current, leaving = request == null) }
    }
}

@Composable
private fun OpenReader(store: AppStore, einkModel: EinkModel, request: ReaderRequest, leaving: Boolean) {
    val context = LocalContext.current
    val density = LocalDensity.current
    val session = remember { ReaderSession { store.send(it) } }
    val images = remember { PageImages() }
    val quiet = remember { QuietPhone(context) }
    val state by session.state.collectAsState()
    val settingsAsked by ReaderLauncher.settings.collectAsState()
    // Paper mode is known before the next open, so the reader can come in at once.
    LaunchedEffect(state.settings) { state.settings?.let { ReaderLook.remember(context, it) } }
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
