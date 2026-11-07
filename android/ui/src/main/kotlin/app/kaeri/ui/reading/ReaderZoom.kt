// The cover the reader grows out of when it opens from one, and closes back into.

package app.kaeri.ui.reading

import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import app.kaeri.engine.core.WorkId

/**
 * The cover's key as the reader's zoom names it: [place] is where the cover sits on screen
 * (`continue`, `spotlight`, `series`), unique on its screen.
 */
fun readerCoverKey(place: String, work: WorkId): String = "$place:$work"

/**
 * Mark this cover as the one the reader grows out of when [key] opened it, and closes back into.
 * Put it on the cover's art, inside its clip, and pass the same key as `from` to
 * [app.kaeri.ui.AppActions.openReader].
 */
@Composable
fun Modifier.readerZoomSource(key: String): Modifier = this
