// Credit pages on the phone and tablet: the line on page 1 that names those skipped, the snackbar
// over one shown again with Not a credit page and then Undo, the line on the end page and on a
// strip's divider, and a strip's rows where the pages were. The session decides what is skipped,
// shown and kept; this draws it in the reader's own colours.

package app.kaeri.reader.phone

import android.view.accessibility.AccessibilityManager
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.SizeTransform
import androidx.compose.animation.togetherWith
import androidx.compose.animation.expandVertically
import androidx.compose.animation.shrinkVertically
import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Spacer
import androidx.compose.material3.minimumInteractiveComponentSize
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.produceState
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.role
import androidx.compose.ui.unit.Density
import app.kaeri.design.SwapFade
import app.kaeri.design.atFullSpeed
import app.kaeri.design.rememberAnimationsRemoved
import app.kaeri.reader.PageDecodeResult
import app.kaeri.reader.shownCredit
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
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
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
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
import androidx.compose.runtime.setValue
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

/** At most one and a half times the text size: the line and the notice never cover the page at 200%. */
@Composable
internal fun CappedText(content: @Composable () -> Unit) {
    val density = LocalDensity.current
    CompositionLocalProvider(LocalDensity provides Density(density.density, density.fontScale.coerceAtMost(1.5f)), content = content)
}

/**
 * "1 credit page skipped": a quiet chip under the top, centred, on the raised colour with 8 dp
 * corners at 14 sp, 32 dp high. The whole chip is the button that shows the pages, with a 48 dp
 * target round it.
 */
@Composable
internal fun CreditLineChip(count: Int, onShow: () -> Unit) {
    val context = LocalContext.current
    val hint = Strings.readerCreditShowAction(context)
    CappedText {
        Box(
            Modifier
                .windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Top + WindowInsetsSides.Horizontal))
                .padding(top = 12.dp, start = 16.dp, end = 16.dp)
                .minimumInteractiveComponentSize()
                .clip(RoundedCornerShape(8.dp))
                .clickable(role = Role.Button, onClickLabel = hint, onClick = onShow)
                .semantics { liveRegion = LiveRegionMode.Polite },
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
                    .padding(horizontal = 16.dp, vertical = 6.dp),
            )
        }
    }
}

/**
 * The snackbar over a credit page shown again: Skipped as a credit page with Not a credit page,
 * held while the page shows and once it has decoded, then This page will show from now on with
 * Undo, the words crossfading. At the foot, above the controls while they show; 560 dp at most on
 * a large screen, centred. Undo waits 8 s, 16 s with TalkBack, longer when the system asks more
 * time to act, and only while it can be seen; the engine hears when it runs out.
 */
@Composable
internal fun BoxScope.CreditSnackbar(
    state: ReaderState,
    session: ReaderSession,
    images: PageImages,
    visible: Boolean,
    above: Int,
    fades: Boolean,
    timed: Boolean = true,
    /** TalkBack is on: Undo waits twice as long. */
    exploring: Boolean = false,
) {
    val context = LocalContext.current
    val density = LocalDensity.current
    val haptics = LocalHapticFeedback.current
    val notice = state.creditNotice
    val held = remember { mutableStateOf(notice) }
    if (notice != null) held.value = notice
    // Not a credit page is offered for a page that can be seen, once its picture has decoded.
    val file = state.shownCredit?.file
    val decoded by produceState(file == null || images.cached(file) != null, file) {
        value = file == null || images.cached(file) != null || withContext(Dispatchers.IO) { images.load(file) } is PageDecodeResult.Decoded
    }
    val showing = notice != null && visible && (notice != CreditNotice.SHOWN || decoded)
    val keeping = state.keeping
    val access = remember { context.getSystemService(AccessibilityManager::class.java) }
    val wait = remember(exploring) {
        val floor = if (exploring) 2 * CREDIT_UNDO_MILLIS else CREDIT_UNDO_MILLIS
        val asked = access?.getRecommendedTimeoutMillis(CREDIT_UNDO_MILLIS.toInt(), AccessibilityManager.FLAG_CONTENT_TEXT or AccessibilityManager.FLAG_CONTENT_CONTROLS)?.toLong() ?: 0L
        maxOf(floor, asked)
    }
    LaunchedEffect(keeping, showing) {
        if (keeping == null || !timed || !showing) return@LaunchedEffect
        delay(wait)
        session.commitKeep()
    }
    CreditFade(showing, fades, Modifier.align(Alignment.BottomCenter)) {
        val shown = held.value ?: return@CreditFade
        CappedText {
            Box(
                Modifier
                    .fillMaxWidth()
                    .windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Horizontal + WindowInsetsSides.Bottom))
                    .padding(start = 12.dp, end = 12.dp, bottom = if (above > 0) with(density) { above.toDp() } + 8.dp else 24.dp),
                contentAlignment = Alignment.BottomCenter,
            ) {
                Snackbar(
                    action = {
                        Swap(shown, fades) { now ->
                            TextButton(onClick = {
                                haptics.performHapticFeedback(HapticFeedbackType.ContextClick)
                                when (now) {
                                    CreditNotice.SHOWN -> session.keepCredit()
                                    CreditNotice.KEPT -> session.undoKeep()
                                }
                            }) {
                                Text(
                                    when (now) {
                                        CreditNotice.SHOWN -> Strings.readerCreditNot(context)
                                        CreditNotice.KEPT -> Strings.actionUndo(context)
                                    },
                                    color = Chrome.ground,
                                    fontSize = 14.sp,
                                    fontWeight = FontWeight.Medium,
                                )
                            }
                        }
                    },
                    shape = RoundedCornerShape(4.dp),
                    containerColor = Chrome.text,
                    contentColor = Chrome.ground,
                    modifier = Modifier.widthIn(max = 560.dp).semantics { liveRegion = LiveRegionMode.Polite },
                ) {
                    Swap(shown, fades) { now ->
                        Text(
                            when (now) {
                                CreditNotice.SHOWN -> Strings.readerCreditShown(context)
                                CreditNotice.KEPT -> Strings.readerCreditKept(context)
                            },
                            fontSize = 14.sp,
                        )
                    }
                }
            }
        }
    }
}

/**
 * [content] shown and hidden on the notice's fades. With Remove animations on the fade still runs,
 * on a full-speed clock of its own, as the board keeps the notice's fade under Reduce Motion.
 */
@Composable
internal fun CreditFade(visible: Boolean, fades: Boolean, modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    if (!fades || !rememberAnimationsRemoved()) {
        AnimatedVisibility(
            visible = visible,
            modifier = modifier,
            enter = if (fades) fadeIn(Motion.fadeIn()) else EnterTransition.None,
            exit = if (fades) fadeOut(Motion.fadeOut()) else ExitTransition.None,
        ) { content() }
        return
    }
    val alpha = remember { Animatable(if (visible) 1f else 0f) }
    LaunchedEffect(visible) {
        atFullSpeed { alpha.animateTo(if (visible) 1f else 0f, if (visible) Motion.fadeIn() else Motion.fadeOut()) }
    }
    if (visible || alpha.value > 0f) Box(modifier.graphicsLayer { this.alpha = alpha.value }) { content() }
}

/** [SwapFade] on the notice's fade-in over its fade-out, or at once when nothing fades. */
@Composable
private fun <T> Swap(state: T, fades: Boolean, content: @Composable (T) -> Unit) {
    if (fades) SwapFade(state, animationSpec = Motion.fadeIn()) { content(it) } else content(state)
}

/** "credit page" or "credit page 1 of 2", for the chapter line while a credit page shows. */
internal fun creditPlaceWords(context: android.content.Context, state: ReaderState): String? {
    val (n, of) = state.creditPlace ?: return null
    return if (of > 1) Strings.readerCreditPageOf(context, n, of) else Strings.readerCreditPage(context)
}

/**
 * Words in haze and an action in silk, underlined, centred on one line when they fit and the
 * action wrapped under the words when they do not, so it never goes at a large text size.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun ActionLine(words: String, action: String, modifier: Modifier = Modifier, onAction: (() -> Unit)? = null) {
    FlowRow(
        modifier.heightIn(min = 44.dp).padding(horizontal = 16.dp, vertical = 12.dp),
        horizontalArrangement = Arrangement.spacedBy(6.dp, Alignment.CenterHorizontally),
        verticalArrangement = Arrangement.Center,
        itemVerticalAlignment = Alignment.CenterVertically,
    ) {
        Text(words, color = Chrome.faint, fontSize = 13.sp)
        Text(
            action,
            color = Chrome.text,
            fontSize = 13.sp,
            fontWeight = FontWeight.SemiBold,
            textDecoration = TextDecoration.Underline,
            modifier = if (onAction == null) Modifier else Modifier.clickable(role = Role.Button, onClick = onAction),
        )
    }
}

/**
 * "1 credit page skipped Show", on the end page and a strip's divider: the words faint, Show
 * underlined, and the whole line is the button.
 */
@Composable
internal fun SkippedLine(count: Int, modifier: Modifier = Modifier, onShow: () -> Unit) {
    val context = LocalContext.current
    val words = Strings.readerCreditPagesSkipped(context, count)
    val hint = Strings.readerCreditShowAction(context)
    ActionLine(
        words,
        Strings.readerCreditShow(context),
        modifier
            .clip(RoundedCornerShape(4.dp))
            .clickable(role = Role.Button, onClickLabel = hint, onClick = onShow)
            .clearAndSetSemantics {
                contentDescription = words
                role = Role.Button
                onClick(label = hint) { onShow(); true }
            },
    )
}

/**
 * What a credit page's row in a strip says: the line naming those skipped, its caption, an empty
 * row while its picture loads, or nothing once kept or when the picture failed.
 */
private enum class StripSlot { Line, Caption, Waiting, None }

/**
 * Credit pages in the flow of a strip, where the engine took them out. While any is skipped, one
 * 44 dp row on the ground names them, and the whole row is Show. On Show the row grows into the
 * image as the image fades in, and the line crossfades into the image's caption, Skipped as a
 * credit page with Not a credit page. A page kept loses its caption the same way, and Undo brings
 * it back. With Remove animations on the image fades in at its full height and what is below
 * moves once.
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
    val haptics = LocalHapticFeedback.current
    val removed = rememberAnimationsRemoved()
    val skipped = pages.filter { !state.isRevealed(it, chapter) }
    Column(Modifier.fillMaxWidth().background(ground)) {
        for (credit in pages) {
            key(credit.page) {
                val revealed = state.isRevealed(credit, chapter)
                val kept = state.keeping?.let { it.chapter == chapter && it.credit.page == credit.page } == true
                // Not a credit page is offered for a picture that can be seen: none while it loads,
                // and none for one that failed, which shows the failed page with Try again.
                // The row keeps its height while the picture loads, so nothing below moves twice.
                var decoded by remember(credit.file) { mutableStateOf(if (images.cached(credit.file) != null) true else null) }
                val slot = when {
                    !revealed -> if (credit == skipped.firstOrNull()) StripSlot.Line else StripSlot.None
                    kept || decoded == false -> StripSlot.None
                    decoded == null -> StripSlot.Waiting
                    else -> StripSlot.Caption
                }
                val row: @Composable (StripSlot) -> Unit = { now ->
                    when (now) {
                        StripSlot.Line -> {
                            val words = Strings.readerCreditPagesSkipped(context, skipped.size)
                            val hint = Strings.readerCreditShowAction(context)
                            ActionLine(
                                words,
                                Strings.readerCreditShow(context),
                                Modifier
                                    .fillMaxWidth()
                                    .clickable(role = Role.Button, onClickLabel = hint) { session.reveal(skipped, chapter) }
                                    .clearAndSetSemantics {
                                        contentDescription = words
                                        role = Role.Button
                                        onClick(label = hint) { session.reveal(skipped, chapter); true }
                                    },
                            )
                        }
                        StripSlot.Caption -> ActionLine(
                            Strings.readerCreditShown(context),
                            Strings.readerCreditNot(context),
                            Modifier.fillMaxWidth(),
                        ) {
                            haptics.performHapticFeedback(HapticFeedbackType.ContextClick)
                            session.keepCredit(credit, chapter)
                        }
                        StripSlot.Waiting -> Spacer(Modifier.fillMaxWidth().height(44.dp))
                        StripSlot.None -> Spacer(Modifier.fillMaxWidth())
                    }
                }
                if (removed) {
                    // The line and the caption still crossfade, and the row takes its new height once.
                    SwapFade(slot, animationSpec = Motion.fadeIn()) { row(it) }
                } else {
                    AnimatedContent(
                        slot,
                        transitionSpec = {
                            (fadeIn(Motion.fadeIn()) togetherWith fadeOut(Motion.fadeOut()))
                                .using(SizeTransform(clip = true) { _, _ -> Motion.standard() })
                        },
                        contentAlignment = Alignment.TopCenter,
                        label = "credit-row", // i18n-skip
                    ) { row(it) }
                }
                val w = (credit.file.crop?.width ?: credit.file.width).toFloat()
                val h = (credit.file.crop?.height ?: credit.file.height).toFloat()
                val ratio = if (w > 0f && h > 0f) w / h else 2f / 3f
                val page = @Composable { CreditPageView(credit, images, ground, Modifier.fillMaxWidth().aspectRatio(ratio), strip = true) { decoded = it } }
                val shownAtFirst = remember { revealed }
                if (removed) {
                    // At its full height at once, faded in on the turn's own clock.
                    if (revealed) if (shownAtFirst) page() else FadeInAtFullSpeed(page)
                } else {
                    AnimatedVisibility(
                        visible = revealed,
                        enter = expandVertically(Motion.standard(), expandFrom = Alignment.Top) + fadeIn(Motion.fadeIn()),
                        exit = shrinkVertically(Motion.standard(), shrinkTowards = Alignment.Top) + fadeOut(Motion.fadeOut()),
                    ) { page() }
                }
            }
        }
    }
}

/** [content] faded in on [Motion.fadeIn] whatever the animator scale, unless it was there from the start. */
@Composable
private fun FadeInAtFullSpeed(content: @Composable () -> Unit) {
    val alpha = remember { Animatable(0f) }
    LaunchedEffect(Unit) { atFullSpeed { alpha.animateTo(1f, Motion.fadeIn()) } }
    Box(Modifier.graphicsLayer { this.alpha = alpha.value }) { content() }
}
