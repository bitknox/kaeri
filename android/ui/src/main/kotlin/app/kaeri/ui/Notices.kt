// Notices: the engine's, and the screens' own (marking watched, with Undo), as snackbars above
// the navigation bar.

package app.kaeri.ui

import app.kaeri.store.alertProblem
import app.kaeri.store.text
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Snackbar
import androidx.compose.material3.SnackbarData
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.SwipeToDismissBox
import androidx.compose.material3.SwipeToDismissBoxDefaults
import androidx.compose.material3.SwipeToDismissBoxValue
import androidx.compose.material3.rememberSwipeToDismissBoxState
import androidx.compose.runtime.key
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.semantics
import kotlin.math.abs
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
import app.kaeri.design.recommendedNoticeMillis
import app.kaeri.engine.model.NoticeHint
import app.kaeri.i18n.R as I18nR
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalResources
import app.kaeri.i18n.Strings

/**
 * How long a snackbar stays: 4 s, 8 s with a button, plus 1 s for every 10 words past 12, so a
 * longer one stays long enough to read.
 */
fun snackbarMillis(text: String, hasAction: Boolean): Long {
    val words = text.split(Regex("\\s+")).count { it.isNotEmpty() }
    val extra = if (words > 12) ((words - 12) + 9) / 10 * 1_000L else 0L
    return (if (hasAction) 8_000L else 4_000L) + extra
}

/** The snackbars, for the screens to post their own. */
@Stable
class Snackbars(
    val host: SnackbarHostState,
    private val scope: CoroutineScope,
    private val context: android.content.Context? = null,
) {
    /**
     * Show [text], with [action] as its button, and [onAction] run when the button is pressed. A
     * notice without a button goes after [snackbarMillis], or the system's accessibility timeout
     * when that is longer. One with a button stays until it is dismissed, acted on or replaced.
     * A newer snackbar replaces this one.
     */
    fun show(text: String, action: String? = null, onAction: () -> Unit = {}) {
        scope.launch {
            host.currentSnackbarData?.dismiss()
            val millis = if (action != null || context == null) snackbarMillis(text, action != null)
            else recommendedNoticeMillis(context, snackbarMillis(text, false), hasAction = false)
            val result = if (action != null) {
                host.showSnackbar(text, actionLabel = action, withDismissAction = true, duration = SnackbarDuration.Indefinite)
            } else {
                withTimeoutOrNull(millis) {
                    host.showSnackbar(text, actionLabel = null, duration = SnackbarDuration.Indefinite)
                }
            }
            if (result == null) host.currentSnackbarData?.dismiss()
            if (result == SnackbarResult.ActionPerformed) onAction()
        }
    }
}

val LocalSnackbars = staticCompositionLocalOf<Snackbars> { error("no Snackbars in this composition") }

/**
 * A snackbar as AndroidFoundations draws it: surfaceContainerHigh with silk text at corners 4,
 * its button a silk text button. Every notice follows a horizontal swipe 1:1 in either direction
 * and goes once the swipe passes the threshold or flings; short of it, it springs back. A drag
 * can catch it mid-settle. TalkBack hears it as one node with a Dismiss action. Only a notice
 * with a button carries the close button, as on the system's own snackbars.
 */
@Composable
internal fun NoticeSnackbar(data: SnackbarData) {
    val colors = MaterialTheme.colorScheme
    val dismissLabel = stringResource(I18nR.string.notice_dismiss)
    key(data) {
        val swipe = rememberSwipeToDismissBoxState(SwipeToDismissBoxValue.Settled, SwipeToDismissBoxDefaults.positionalThreshold)
        SwipeToDismissBox(
            state = swipe,
            backgroundContent = {},
            onDismiss = { data.dismiss() },
            modifier = Modifier
                // Fades as it travels, so a notice swiped most of the way reads as going.
                .graphicsLayer {
                    val offset = runCatching { swipe.requireOffset() }.getOrDefault(0f)
                    alpha = if (size.width > 0f) (1f - abs(offset) / size.width).coerceIn(0f, 1f) else 1f
                }
                .semantics(mergeDescendants = true) {
                    customActions = listOf(CustomAccessibilityAction(dismissLabel) { data.dismiss(); true })
                },
        ) {
            Snackbar(
                data,
                shape = MaterialTheme.shapes.extraSmall,
                containerColor = colors.surfaceContainerHigh,
                contentColor = colors.onSurface,
                actionColor = colors.onSurface,
                actionContentColor = colors.onSurface,
                dismissActionContentColor = colors.onSurface,
                // Material's snackbar keeps 12 around itself: 16 from the sides, 12 above the bar.
                modifier = Modifier.padding(horizontal = 4.dp),
            )
        }
    }
}

/** The words on a notice's button, for what can be done about it. */
fun NoticeHint.label(context: android.content.Context): String = when (this) {
    NoticeHint.STOP_PLAYBACK -> context.getString(I18nR.string.action_stop)
    NoticeHint.OPEN_STORES -> context.getString(I18nR.string.notice_open_stores)
    NoticeHint.KITSU_CREDENTIALS -> context.getString(I18nR.string.action_sign_in)
    NoticeHint.UNDO_REMOVE_DEMO, NoticeHint.UNDO_SUBTITLE_RESET -> context.getString(I18nR.string.action_undo)
    NoticeHint.REMOVE_DEMO -> context.getString(I18nR.string.sources_demo_remove)
    NoticeHint.REPORT_CRASH -> context.getString(I18nR.string.crash_notice_report)
}

/** Each notice the engine posts, as a snackbar, with its hint as the button. */
@Composable
internal fun EngineNotices(onHint: (NoticeHint) -> Unit) {
    val store = LocalAppStore.current
    val snackbars = LocalSnackbars.current
    val context = androidx.compose.ui.platform.LocalContext.current
    val resources = LocalResources.current
    val current by store.notices.current.collectAsStateWithLifecycle()
    LaunchedEffect(current?.id) {
        val item = current ?: return@LaunchedEffect
        val hint = item.notice.hint
        // An alert's button is its hint when it has one, and Report when it has none.
        if (hint == null && item.isAlert) {
            val text = item.notice.text(context)
            snackbars.show(text, resources.getString(I18nR.string.action_report)) {
                openBugReport(context, store, store.alertProblem(text))
            }
        } else {
            snackbars.show(item.notice.text(context), hint?.label(context)) { hint?.let(onHint) }
        }
        store.notices.dismiss(item)
    }
}

/** A pinned source the engine could not use, said once as a snackbar. */
@Composable
internal fun SourcePinNotice() {
    val store = LocalAppStore.current
    val snackbars = LocalSnackbars.current
    val context = androidx.compose.ui.platform.LocalContext.current
    val notice by store.sourceChoices.pinNotice.collectAsStateWithLifecycle()
    LaunchedEffect(notice?.serial) {
        val said = notice ?: return@LaunchedEffect
        snackbars.show(app.kaeri.i18n.Strings.sourcePinUnavailable(context, said.name))
        store.sourceChoices.told(said)
    }
}

/** What joining plugin stores through a paired device came to, said as snackbars: each store that was not joined, and why. */
@Composable
internal fun StoreJoinNotices() {
    val store = LocalAppStore.current
    val snackbars = LocalSnackbars.current
    val context = androidx.compose.ui.platform.LocalContext.current
    val joins by store.sync.storeJoins.collectAsStateWithLifecycle()
    val status by store.sync.status.collectAsStateWithLifecycle()
    LaunchedEffect(joins) {
        if (joins.isEmpty()) return@LaunchedEffect
        joins.filterIsInstance<app.kaeri.store.StoreJoin.Failed>().forEach { failed ->
            val device = status?.devices?.firstOrNull { it.id == failed.device }?.name ?: failed.device
            snackbars.show(failed.text(context, device))
        }
        store.sync.storeJoinsSeen()
    }
}
