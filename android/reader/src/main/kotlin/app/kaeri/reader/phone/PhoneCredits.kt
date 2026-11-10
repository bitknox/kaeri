// Credit pages on the phone and tablet: the line on page 1 that names those skipped, the snackbar
// over one shown again with Not a credit page and then Undo, the line on the end page and on a
// strip's divider, and a strip's rows where the pages were. The session decides what is skipped,
// shown and kept; this draws it in the reader's own colours.

package app.kaeri.reader.phone

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.background
import androidx.compose.material3.Snackbar
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.onClick
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.kaeri.design.Motion
import app.kaeri.engine.core.ChapterId
import app.kaeri.engine.model.CreditPage
import app.kaeri.i18n.Strings
import app.kaeri.reader.CreditNotice
import app.kaeri.reader.CreditPageView
import app.kaeri.reader.PageImages
import app.kaeri.reader.ReaderSession
import app.kaeri.reader.ReaderState
import app.kaeri.reader.creditNotice
import app.kaeri.reader.creditPlace
import app.kaeri.reader.isRevealed
import kotlinx.coroutines.delay

/** How long the Undo after Not a credit page waits before the engine hears, on a phone; twice that with TalkBack. */
internal const val CREDIT_UNDO_MILLIS = 8_000L

/** How long page 1's line holds once the page has settled. */
internal const val CREDIT_LINE_MILLIS = 4_000L

/**
 * "1 credit page skipped": a quiet chip under the top, centred, on the raised colour with 8 dp
 * corners at 14 sp. The whole chip is the button that shows the pages.
 */
@Composable
internal fun CreditLineChip(count: Int, onShow: () -> Unit) {
    val context = LocalContext.current
    val hint = Strings.readerCreditShowHint(context)
    Box(
        Modifier
            .windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Top + WindowInsetsSides.Horizontal))
            .padding(top = 12.dp, start = 16.dp, end = 16.dp)
            .heightIn(min = 48.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            Strings.readerCreditPagesSkipped(context, count),
            color = Chrome.text,
            fontSize = 14.sp,
            lineHeight = 20.sp,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier
                .clip(RoundedCornerShape(8.dp))
                .background(Chrome.plate)
                .clickable(role = Role.Button, onClickLabel = hint, onClick = onShow)
                .padding(horizontal = 16.dp, vertical = 8.dp)
                .semantics { liveRegion = LiveRegionMode.Polite },
        )
    }
}

/**
 * The snackbar over a credit page shown again: Skipped as a credit page with Not a credit page,
 * held while the page shows, then This page will show from now on with Undo. At the foot, above
 * the controls while they show; 560 dp at most on a large screen, centred.
 */
@Composable
internal fun BoxScope.CreditSnackbar(
    state: ReaderState,
    session: ReaderSession,
    visible: Boolean,
    above: Int,
    fades: Boolean,
    timed: Boolean = true,
    /** TalkBack is on: Undo waits twice as long. */
    exploring: Boolean = false,
) {
    val context = LocalContext.current
    val density = LocalDensity.current
    val notice = state.creditNotice
    val held = remember { mutableStateOf(notice) }
    if (notice != null) held.value = notice
    // Undo runs out after 8 s, 16 s with TalkBack, and the engine hears then.
    val keeping = state.keeping
    LaunchedEffect(keeping) {
        if (keeping == null || !timed) return@LaunchedEffect
        delay(if (exploring) 2 * CREDIT_UNDO_MILLIS else CREDIT_UNDO_MILLIS)
        session.commitKeep()
    }
    AnimatedVisibility(
        visible = notice != null && visible,
        enter = if (fades) fadeIn(Motion.fadeIn()) else EnterTransition.None,
        exit = if (fades) fadeOut(Motion.fadeOut()) else ExitTransition.None,
        modifier = Modifier.align(Alignment.BottomCenter),
    ) {
        val shown = held.value ?: return@AnimatedVisibility
        val words = when (shown) {
            CreditNotice.SHOWN -> Strings.readerCreditShown(context)
            CreditNotice.KEPT -> Strings.readerCreditKept(context)
        }
        val action = when (shown) {
            CreditNotice.SHOWN -> Strings.readerCreditNot(context)
            CreditNotice.KEPT -> Strings.actionUndo(context)
        }
        Box(
            Modifier
                .fillMaxWidth()
                .windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Horizontal + WindowInsetsSides.Bottom))
                .padding(start = 12.dp, end = 12.dp, bottom = if (above > 0) with(density) { above.toDp() } + 8.dp else 24.dp),
            contentAlignment = Alignment.BottomCenter,
        ) {
            Snackbar(
                action = {
                    TextButton(onClick = {
                        when (shown) {
                            CreditNotice.SHOWN -> session.keepCredit()
                            CreditNotice.KEPT -> session.undoKeep()
                        }
                    }) {
                        Text(action, color = Chrome.ground, fontSize = 14.sp, fontWeight = FontWeight.Medium)
                    }
                },
                shape = RoundedCornerShape(4.dp),
                containerColor = Chrome.text,
                contentColor = Chrome.ground,
                modifier = Modifier.widthIn(max = 560.dp).semantics { liveRegion = LiveRegionMode.Polite },
            ) {
                Text(words, fontSize = 14.sp)
            }
        }
    }
}

/** "credit page" or "credit page 1 of 2", for the chapter line while a credit page shows. */
internal fun creditPlaceWords(context: android.content.Context, state: ReaderState): String? {
    val (n, of) = state.creditPlace ?: return null
    return if (of > 1) Strings.readerCreditPageOf(context, n, of) else Strings.readerCreditPage(context)
}

/**
 * "1 credit page skipped Show", on the end page and a strip's divider: the words faint, Show
 * underlined, and the whole line is the button.
 */
@Composable
internal fun SkippedLine(count: Int, modifier: Modifier = Modifier, onShow: () -> Unit) {
    val context = LocalContext.current
    val words = Strings.readerCreditPagesSkipped(context, count)
    val hint = Strings.readerCreditShowHint(context)
    Row(
        modifier
            .heightIn(min = 44.dp)
            .clip(RoundedCornerShape(4.dp))
            .clickable(role = Role.Button, onClickLabel = hint, onClick = onShow)
            .semantics(mergeDescendants = true) {
                contentDescription = words
                onClick(label = hint) { onShow(); true }
            },
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Text(words, color = Chrome.faint, fontSize = 13.sp)
        Text(
            Strings.readerCreditShow(context),
            color = Chrome.text,
            fontSize = 13.sp,
            fontWeight = FontWeight.SemiBold,
            textDecoration = TextDecoration.Underline,
        )
    }
}

/**
 * Credit pages in the flow of a strip, where the engine took them out. While any is skipped,
 * one 44 dp row on the ground names them, and the whole row is Show. Shown, each image sits under
 * its caption, Skipped as a credit page with Not a credit page; a page kept loses its caption.
 */
@Composable
internal fun StripCredits(
    state: ReaderState,
    session: ReaderSession,
    images: PageImages,
    chapter: ChapterId,
    pages: List<CreditPage>,
    ground: Color,
) {
    val context = LocalContext.current
    val skipped = pages.filter { !state.isRevealed(it, chapter) }
    Column(Modifier.fillMaxWidth()) {
        if (skipped.isNotEmpty()) {
            val words = Strings.readerCreditPagesSkipped(context, skipped.size)
            val hint = Strings.readerCreditShowHint(context)
            Row(
                Modifier
                    .fillMaxWidth()
                    .heightIn(min = 44.dp)
                    .background(ground)
                    .clickable(role = Role.Button, onClickLabel = hint) { session.reveal(skipped, chapter) }
                    .semantics(mergeDescendants = true) { contentDescription = words },
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp, Alignment.CenterHorizontally),
            ) {
                Text(words, color = Chrome.faint, fontSize = 13.sp, maxLines = 1)
                Text(Strings.readerCreditShow(context), color = Chrome.text, fontSize = 13.sp, fontWeight = FontWeight.SemiBold, textDecoration = TextDecoration.Underline, maxLines = 1)
            }
        }
        for (credit in pages) {
            if (!state.isRevealed(credit, chapter)) continue
            val kept = state.keeping?.let { it.chapter == chapter && it.credit.page == credit.page } == true
            if (!kept) {
                Row(
                    Modifier.fillMaxWidth().heightIn(min = 44.dp).background(ground),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(6.dp, Alignment.CenterHorizontally),
                ) {
                    Text(Strings.readerCreditShown(context), color = Chrome.faint, fontSize = 13.sp, maxLines = 1)
                    Text(
                        Strings.readerCreditNot(context),
                        color = Chrome.text,
                        fontSize = 13.sp,
                        fontWeight = FontWeight.SemiBold,
                        textDecoration = TextDecoration.Underline,
                        maxLines = 1,
                        modifier = Modifier
                            .heightIn(min = 44.dp)
                            .clickable(role = Role.Button) { session.keepCredit(credit, chapter) }
                            .padding(vertical = 12.dp),
                    )
                }
            }
            val w = (credit.file.crop?.width ?: credit.file.width).toFloat()
            val h = (credit.file.crop?.height ?: credit.file.height).toFloat()
            val ratio = if (w > 0f && h > 0f) w / h else 2f / 3f
            CreditPageView(credit, images, ground, Modifier.fillMaxWidth().aspectRatio(ratio))
        }
    }
}
