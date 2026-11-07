// The cover the reader grows out of when it opens from one, and closes back into: the two ends of
// a shared element in a layout around the whole app and the reader drawn over it.

package app.kaeri.ui.reading

import android.view.View
import androidx.compose.animation.ExperimentalSharedTransitionApi
import androidx.compose.animation.SharedTransitionScope
import androidx.compose.runtime.Composable
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.unit.Dp
import app.kaeri.design.ArtShape
import app.kaeri.design.Motion
import app.kaeri.engine.core.WorkId

/**
 * The cover's key as the reader's zoom names it: [place] is where the cover sits on screen
 * (`continue`, `spotlight`, `series`), unique on its screen.
 */
fun readerCoverKey(place: String, work: WorkId): String = "$place:$work"

/** The shared element's key for the cover at [key]. */
fun readerZoomKey(key: String): String = "reader-cover-$key"

/** The layout the reader's zoom flies in: around the app and the reader over it. */
@OptIn(ExperimentalSharedTransitionApi::class)
val LocalReaderTransitions = compositionLocalOf<SharedTransitionScope?> { null }

/** The window the reader's zoom lays out in; a cover in a dialog or popup takes no part. */
val LocalReaderTransitionsView = staticCompositionLocalOf<View?> { null }

/** The cover the open reader grew out of, by its key: that cover is hidden while the reader is up. */
val LocalReaderZoomOpen = compositionLocalOf<String?> { null }

/**
 * Mark this cover as the one the reader grows out of when [key] opened it, and closes back into.
 * Put it on the cover's art, inside its clip, and pass the same key as `from` to
 * [app.kaeri.ui.AppActions.openReader]. While the reader is up the cover is not drawn here: it is
 * the reader's, until it shrinks back.
 */
@OptIn(ExperimentalSharedTransitionApi::class)
@Composable
fun Modifier.readerZoomSource(key: String): Modifier {
    val layout = LocalReaderTransitions.current ?: return this
    if (LocalView.current !== LocalReaderTransitionsView.current) return this
    val open = LocalReaderZoomOpen.current
    return with(layout) {
        sharedElementWithCallerManagedVisibility(
            rememberSharedContentState(readerZoomKey(key)),
            visible = open != key,
            boundsTransform = { _, _ -> Motion.zoom() },
            // Shrinking back it keeps a cover's corners, as it has in its place.
            clipInOverlayDuringTransition = OverlayClip(ArtShape.cover(Dp.Infinity)),
        )
    }
}
