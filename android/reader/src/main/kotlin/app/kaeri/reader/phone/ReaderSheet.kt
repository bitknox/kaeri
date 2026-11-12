// The reader's bottom sheets. Material's sheet slides up and down; in Paper mode, where nothing in
// the reader moves, the same sheet is drawn in place at once and leaves at once.

package app.kaeri.reader.phone

import app.kaeri.reader.ReaderBackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.SheetValue
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.snapshotFlow
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.flow.first

/**
 * A sheet over the reader in the chrome's colours, the page behind left undimmed. [instant] draws
 * it without its slide, for Paper mode; it then fills the box it is drawn in, which must be the
 * reader's own. [expanded] opens Material's sheet all the way, for a page whose last line would sit
 * below a half-open sheet; Paper's sheet is always whole.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun ReaderSheet(
    instant: Boolean,
    onDismiss: () -> Unit,
    /** Opens at the content's full height rather than half the screen. */
    whole: Boolean = false,
    expanded: Boolean = false,
    content: @Composable ColumnScope.() -> Unit,
) {
    if (!instant) {
        val sheet = rememberModalBottomSheetState(skipPartiallyExpanded = whole)
        LaunchedEffect(expanded) {
            if (!expanded) return@LaunchedEffect
            // After the sheet's own slide in, or at once when it is already up.
            snapshotFlow { sheet.currentValue }.first { it != SheetValue.Hidden }
            if (sheet.currentValue != SheetValue.Expanded) sheet.expand()
        }
        ModalBottomSheet(
            onDismissRequest = onDismiss,
            sheetState = sheet,
            containerColor = Chrome.plate,
            contentColor = Chrome.text,
            // The page behind stays undimmed.
            scrimColor = Color.Transparent,
            content = content,
        )
        return
    }
    // Drawn over the reader in its own layout, since a window of its own would animate.
    ReaderBackHandler(onBack = onDismiss)
    run {
        val height = LocalConfiguration.current.screenHeightDp.dp
        Box(Modifier.fillMaxSize()) {
            // Taps above the sheet close it, as they do on Material's.
            Box(
                Modifier.fillMaxSize().clickable(
                    interactionSource = remember { MutableInteractionSource() },
                    indication = null,
                    onClick = onDismiss,
                ),
            )
            val shape = RoundedCornerShape(topStart = 28.dp, topEnd = 28.dp)
            Column(
                Modifier
                    .align(Alignment.BottomCenter)
                    .windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Top))
                    .padding(top = 72.dp)
                    .heightIn(max = height)
                    .windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Horizontal))
                    .fillMaxWidth()
                    .clip(shape)
                    .background(Chrome.plate, shape)
                    // The sheet swallows its own taps.
                    .clickable(interactionSource = remember { MutableInteractionSource() }, indication = null) {},
            ) {
                Box(Modifier.fillMaxWidth().padding(vertical = 22.dp), contentAlignment = Alignment.Center) {
                    Box(
                        Modifier.size(width = 32.dp, height = 4.dp)
                            .background(Chrome.faint.copy(alpha = 0.4f), RoundedCornerShape(2.dp)),
                    )
                }
                content()
            }
        }
    }
}
