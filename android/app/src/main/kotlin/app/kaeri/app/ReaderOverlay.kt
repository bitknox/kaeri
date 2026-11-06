// The reader over the app: a request on ReaderLauncher opens it, closing it shows the app again.
// It is the phone reader, or the e-ink reader for a request from an e-ink panel.

package app.kaeri.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import app.kaeri.engine.model.EinkPanel
import app.kaeri.eink.EinkModel
import app.kaeri.eink.EinkPainter
import app.kaeri.eink.EinkQuestions
import app.kaeri.eink.EinkReader
import app.kaeri.eink.EinkReaderActions
import app.kaeri.eink.NoRefresh
import app.kaeri.reader.PageImages
import app.kaeri.reader.ReaderSession
import app.kaeri.reader.phone.NovelScreen
import app.kaeri.reader.phone.PhoneReader
import app.kaeri.reader.phone.PhoneReaderActions
import app.kaeri.reader.phone.QuietPhone
import app.kaeri.reader.phone.ReaderLauncher
import app.kaeri.reader.phone.ReaderRequest
import app.kaeri.reader.readerSurface
import app.kaeri.store.AppStore

@Composable
fun ReaderOverlay(store: AppStore, eink: EinkModel) {
    val request by ReaderLauncher.request.collectAsState()
    val current = request ?: return
    key(current) { OpenReader(store, eink, current) }
}

@Composable
private fun OpenReader(store: AppStore, einkModel: EinkModel, request: ReaderRequest) {
    val context = LocalContext.current
    val density = LocalDensity.current
    val session = remember { ReaderSession { store.send(it) } }
    val images = remember { PageImages() }
    val quiet = remember { QuietPhone(context) }
    val state by session.state.collectAsState()
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
                val painter = remember(profile.profile?.panel) {
                    EinkPainter.defaults(profile.profile?.panel ?: EinkPanel.GREY, NoRefresh)
                }
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
                novel = { onControls ->
                    NovelScreen(
                        session = session,
                        state = state,
                        assets = remember { store.novelAssets() },
                        resource = store::bookResource,
                        onControls = onControls,
                    )
                },
            )
        }
    }
}
