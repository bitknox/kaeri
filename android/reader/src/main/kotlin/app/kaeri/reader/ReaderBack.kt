package app.kaeri.reader

import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.navigationevent.NavigationEvent
import androidx.navigationevent.NavigationEventDispatcher
import androidx.navigationevent.NavigationEventHandler
import androidx.navigationevent.NavigationEventInfo
import androidx.navigationevent.compose.LocalNavigationEventDispatcherOwner
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.consumeAsFlow
import kotlinx.coroutines.launch

/**
 * Back while the reader is open, taken ahead of the app under it: the reader's handlers sit at
 * the overlay priority, so the app's, which come in after the reader's when the activity is made
 * anew, never take it. Among the reader's own the newest wins, as with [androidx.activity.compose.BackHandler].
 * Sheets in a window of their own, as the app's are, keep their Back.
 */
@Composable
fun ReaderBackHandler(enabled: Boolean = true, onBack: () -> Unit) {
    val current by rememberUpdatedState(onBack)
    ReaderPredictiveBackHandler(enabled) { progress ->
        progress.collect {}
        current()
    }
}

/**
 * As [ReaderBackHandler], with the gesture's progress from 0 to 1: [onBack] collects it and does
 * its work once the flow ends; a gesture let go of cancels it.
 */
@Composable
fun ReaderPredictiveBackHandler(enabled: Boolean = true, onBack: suspend (progress: Flow<Float>) -> Unit) {
    val owner = LocalNavigationEventDispatcherOwner.current ?: return
    val scope = rememberCoroutineScope()
    val current by rememberUpdatedState(onBack)
    // Added while enabled and removed when not, rather than switched off in place: a handler
    // switched off took the next Back anyway. Added again, it is the newest.
    DisposableEffect(owner, scope, enabled) {
        if (!enabled) return@DisposableEffect onDispose {}
        val handler = OverlayBack(scope) { current(it) }
        owner.navigationEventDispatcher.addHandler(handler, NavigationEventDispatcher.PRIORITY_OVERLAY)
        onDispose { handler.remove() }
    }
}

private class OverlayBack(
    private val scope: CoroutineScope,
    private val onBack: suspend (Flow<Float>) -> Unit,
) : NavigationEventHandler<NavigationEventInfo>(NavigationEventInfo.None, true) {
    private var progress: Channel<Float>? = null
    private var job: Job? = null

    private fun begin(): Channel<Float> {
        job?.cancel(CancellationException("Back began again"))
        val channel = Channel<Float>(Channel.UNLIMITED)
        progress = channel
        job = scope.launch { onBack(channel.consumeAsFlow()) }
        return channel
    }

    override fun onBackStarted(event: NavigationEvent) {
        begin()
    }

    override fun onBackProgressed(event: NavigationEvent) {
        progress?.trySend(event.progress)
    }

    override fun onBackCompleted() {
        (progress ?: begin()).close()
        progress = null
        job = null
    }

    override fun onBackCancelled() {
        progress?.close()
        progress = null
        job?.cancel(CancellationException("Back was let go"))
        job = null
    }
}
