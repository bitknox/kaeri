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
import androidx.compose.material3.ModalBottomSheetProperties
import androidx.compose.material3.SheetValue
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.snapshotFlow
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import app.kaeri.reader.ReaderPredictiveBackHandler
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.material3.BottomSheetDefaults
import androidx.compose.material3.SheetState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.onPlaced
import androidx.compose.ui.layout.positionInParent
import androidx.compose.ui.unit.Density
import kotlin.coroutines.cancellation.CancellationException
import kotlin.math.min

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
    /**
     * For a sheet with pages of its own: false while a page other than the first is shown, where
     * Back calls [pageBack] to go to the page it came from rather than closing the sheet.
     */
    firstPage: Boolean = true,
    pageBack: (() -> Unit)? = null,
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
        if (pageBack == null) {
            ModalBottomSheet(
                onDismissRequest = onDismiss,
                sheetState = sheet,
                containerColor = Chrome.plate,
                contentColor = Chrome.text,
                // The page behind stays undimmed.
                scrimColor = Color.Transparent,
                content = content,
            )
        } else {
            PagedSheet(sheet, firstPage, pageBack, onDismiss, content)
        }
        return
    }
    // Drawn over the reader in its own layout, since a window of its own would animate.
    ReaderBackHandler { if (!firstPage && pageBack != null) pageBack() else onDismiss() }
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

/**
 * Material's sheet with Back taken by one handler of the sheet's own from open to close: Back on a
 * page goes to the page it came from, and on the first page it shrinks the sheet as the gesture is
 * drawn and settles it as Material's own Back does. A handler taken away or switched off as the
 * page changed left the next Back to land on nothing, so Material's own is off and its shrink is
 * drawn here with its values.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun PagedSheet(
    sheet: SheetState,
    firstPage: Boolean,
    pageBack: () -> Unit,
    onDismiss: () -> Unit,
    content: @Composable ColumnScope.() -> Unit,
) {
    val scope = rememberCoroutineScope()
    val shrink = remember { Animatable(0f) }
    val first by rememberUpdatedState(firstPage)
    val back by rememberUpdatedState(pageBack)
    val dismiss by rememberUpdatedState(onDismiss)
    val column = remember { SheetColumn() }
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        modifier = Modifier.sheetShrink(sheet) { shrink.value },
        sheetState = sheet,
        containerColor = Chrome.plate,
        contentColor = Chrome.text,
        // The page behind stays undimmed.
        scrimColor = Color.Transparent,
        dragHandle = { Box(Modifier.contentShrink(column, top = { 0f }) { shrink.value }) { BottomSheetDefaults.DragHandle() } },
        properties = ModalBottomSheetProperties(shouldDismissOnBackPress = false),
    ) {
        // In the sheet's window, where Back goes while it is open.
        ReaderPredictiveBackHandler { progress ->
            if (!first) {
                progress.collect {}
                back()
                return@ReaderPredictiveBackHandler
            }
            try {
                progress.collect { shrink.snapTo(ShrinkEasing.transform(it)) }
            } catch (e: CancellationException) {
                scope.launch { shrink.animateTo(0f) }
                throw e
            }
            if (sheet.currentValue == SheetValue.Expanded && sheet.hasPartiallyExpandedState) {
                scope.launch { sheet.partialExpand() }
                scope.launch { shrink.animateTo(0f) }
            } else {
                // The shrink stays on as the sheet slides away.
                scope.launch { sheet.hide() }.invokeOnCompletion { if (!sheet.isVisible) dismiss() }
            }
        }
        Column(
            Modifier.fillMaxWidth()
                .onPlaced {
                    column.width = it.size.width.toFloat()
                    column.top = it.positionInParent().y
                    column.height = column.top + it.size.height
                }
                .contentShrink(column, top = { column.top }) { shrink.value },
            content = content,
        )
    }
}

// Material's predictive Back on its sheet (BottomSheet.kt and internal/BackHandler.kt in
// material3): the gesture's progress through this curve, the sheet narrowed by up to 48 dp and
// lowered by up to 24 dp about a point below it, and what it holds stretched back to its width's
// scale from the top so nothing in it looks squashed.
private val ShrinkEasing = CubicBezierEasing(0.1f, 0.1f, 0f, 1f)
private val ShrinkWidth = 48.dp
private val ShrinkHeight = 24.dp

private fun Density.shrinkX(progress: Float, width: Float): Float =
    if (width.isNaN() || width == 0f) 1f else 1f - min(ShrinkWidth.toPx(), width) * progress / width

private fun Density.shrinkY(progress: Float, height: Float): Float =
    if (height.isNaN() || height == 0f) 1f else 1f - min(ShrinkHeight.toPx(), height) * progress / height

/**
 * On the sheet, outside Material's own layers. Material's layer sits at the sheet's offset and
 * scales about a point the offset and the sheet's height below its top; this one sits at the top
 * of the sheet's box, so the same point is twice the offset and the height below.
 */
@OptIn(ExperimentalMaterial3Api::class)
private fun Modifier.sheetShrink(sheet: SheetState, progress: () -> Float) = graphicsLayer {
    val p = progress()
    val offset = if (p == 0f) Float.NaN else runCatching { sheet.requireOffset() }.getOrDefault(Float.NaN)
    val height = size.height
    if (!offset.isNaN() && !height.isNaN() && height != 0f) {
        scaleX = shrinkX(p, size.width)
        scaleY = shrinkY(p, height)
        transformOrigin = TransformOrigin(0.5f, (2 * offset + height) / height)
    }
}

/** The size of Material's column in the sheet, the drag handle and what the sheet holds. */
private class SheetColumn {
    var width by mutableFloatStateOf(0f)
    /** Where what the sheet holds starts, below the drag handle. */
    var top by mutableFloatStateOf(0f)
    var height by mutableFloatStateOf(0f)
}

/**
 * On a part of Material's column, as Material draws the whole column: stretched from the column's
 * top, [top] above this layer's own, by the column's own scale.
 */
private fun Modifier.contentShrink(column: SheetColumn, top: () -> Float, progress: () -> Float) = graphicsLayer {
    val p = progress()
    val height = size.height
    if (p == 0f || height.isNaN() || height == 0f) return@graphicsLayer
    val x = shrinkX(p, column.width)
    val y = shrinkY(p, column.height)
    scaleY = if (y != 0f) x / y else 1f
    transformOrigin = TransformOrigin(0.5f, -top() / height)
}
