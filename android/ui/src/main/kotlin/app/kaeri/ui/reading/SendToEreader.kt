// Send to e-reader: the sheet that prepares chapters for an e-reader and hands them to the share
// sheet, from the series page and from Downloads.

package app.kaeri.ui.reading

import androidx.compose.runtime.Composable
import app.kaeri.engine.core.ChapterId
import app.kaeri.engine.core.WorkId
import app.kaeri.ui.AppActions

/** The Send to e-reader sheet for [chapters] of [work]; [onDismiss] closes it. */
@Composable
internal fun SendToEreaderSheet(work: WorkId, chapters: List<ChapterId>, actions: AppActions, onDismiss: () -> Unit) {
}
