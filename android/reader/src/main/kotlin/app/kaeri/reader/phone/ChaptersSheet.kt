// The Chapters sheet: a grid of the open chapter's pages with the current one ringed, and the
// series' chapters by volume, with read volumes folded and the open chapter on the selected fill.

package app.kaeri.reader.phone

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.focusable
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.itemsIndexed
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.kaeri.design.centredOffset
import app.kaeri.design.robotoFlex
import app.kaeri.engine.model.ChapterRow
import app.kaeri.i18n.Strings
import app.kaeri.reader.PageFileView
import app.kaeri.reader.PageWaits
import app.kaeri.engine.model.LeafPart
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import kotlinx.coroutines.delay
import app.kaeri.reader.PageImages
import app.kaeri.reader.ReaderSession
import app.kaeri.reader.ReaderState

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ChaptersSheet(
    session: ReaderSession,
    state: ReaderState,
    images: PageImages,
    currentLeaf: Int,
    rtl: Boolean,
    onGo: (leaf: Int) -> Unit,
    onDismiss: () -> Unit,
    /** Paper mode: the sheet comes and goes at once. */
    instant: Boolean = false,
) {
    val context = LocalContext.current
    val view = state.view ?: return
    var tab by remember { mutableStateOf(0) }
    val novel = view.medium == app.kaeri.engine.core.Medium.NOVEL
    LaunchedEffect(view.work) { session.loadChapters() }
    // Whole, so the chapter list's middle, where the open chapter sits, is on screen.
    ReaderSheet(instant = instant, onDismiss = onDismiss, whole = true) {
        Column(Modifier.navigationBarsPadding().padding(horizontal = 20.dp)) {
            Text(view.title, color = Chrome.text, fontFamily = robotoFlex(FontWeight.Bold, 18.sp), fontSize = 18.sp, maxLines = 1)
            Text(chapterWords(context, view.chapter), color = Chrome.faint, fontSize = 13.sp)
            Spacer(Modifier.height(12.dp))
            if (novel) {
                tab = 1
            } else {
                Segments(listOf(Strings.readerGridTitle(context), Strings.readerChapters(context)), tab) { tab = it }
            }
            Spacer(Modifier.height(12.dp))
            // A fixed height, so the sheet stays still when the chapters come in after it opened,
            // and fits a phone held sideways.
            val listHeight = (LocalConfiguration.current.screenHeightDp * 0.45f).dp.coerceAtMost(460.dp)
            Box(Modifier.height(listHeight).fillMaxWidth()) {
                if (tab == 0) {
                    PageGrid(state, images, currentLeaf, rtl, onGo, session::needPage)
                } else {
                    ChapterList(session, state)
                }
            }
            Spacer(Modifier.height(16.dp))
        }
    }
}

fun chapterWords(context: android.content.Context, chapter: ChapterRow): String {
    val number = chapterNumber(chapter)
    return chapter.title?.let { Strings.readerChapterLineTitled(context, number, it) } ?: Strings.readerChapterLine(context, number)
}

/**
 * Every page small, three across, opened at the row before the page being read. Each cell asks for
 * its page as it comes into view.
 */
@Composable
private fun PageGrid(state: ReaderState, images: PageImages, currentLeaf: Int, rtl: Boolean, onGo: (Int) -> Unit, onMissing: (LeafPart) -> Unit) {
    val context = LocalContext.current
    val story = state.leaves.filter { !it.endPage }
    val grid = rememberLazyGridState(initialFirstVisibleItemIndex = (currentLeaf / 3 * 3 - 3).coerceIn(0, (story.size - 1).coerceAtLeast(0)))
    // A right-to-left series lays its grid out from the right, as its pages run.
    CompositionLocalProvider(LocalLayoutDirection provides if (rtl) LayoutDirection.Rtl else LayoutDirection.Ltr) {
        LazyVerticalGrid(
            columns = GridCells.Fixed(3),
            horizontalArrangement = Arrangement.spacedBy(10.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
            state = grid,
            modifier = Modifier.fillMaxSize(),
        ) {
            itemsIndexed(story, key = { _, leaf -> leaf.index.toInt() }) { index, leaf ->
                val part = leaf.parts.firstOrNull()
                val file = part?.let(state::file)
                LaunchedEffect(part?.page, file == null) { if (file == null && part != null && state.failures[part.page] == null) onMissing(part) }
                val page = (part?.page?.toInt() ?: index) + 1
                val shape = RoundedCornerShape(8.dp)
                Box(
                    Modifier
                        .fillMaxWidth()
                        .aspectRatio(0.7f)
                        .clip(shape)
                        .background(Chrome.selected)
                        .then(if (index == currentLeaf) Modifier.border(2.dp, Chrome.text, shape) else Modifier)
                        .clickable { onGo(index) }
                        .semantics { contentDescription = Strings.readerGridPage(context, page) },
                ) {
                    PageFileView(file, images, Modifier.fillMaxSize())
                    Text(
                        page.toString(),
                        color = if (file != null) Color.White else Chrome.faint,
                        fontSize = 13.sp,
                        modifier = Modifier.align(Alignment.BottomEnd).padding(6.dp)
                            // On a drawn page the number sits on a dark chip, so it reads on light paper.
                            .then(
                                if (file != null) {
                                    Modifier.background(Color.Black.copy(alpha = 0.6f), RoundedCornerShape(4.dp)).padding(horizontal = 5.dp, vertical = 1.dp)
                                } else {
                                    Modifier
                                },
                            ),
                    )
                }
            }
        }
    }
}

@Composable
private fun ChapterList(session: ReaderSession, state: ReaderState) {
    val context = LocalContext.current
    val current = state.chapter
    var open by remember { mutableStateOf(setOf<Int>()) }
    if (state.volumes.isEmpty()) {
        // Nothing for the first 600 ms, as a page's plate waits, then the line.
        var waited by remember { mutableStateOf(false) }
        LaunchedEffect(Unit) {
            delay(PageWaits.PLATE_MS)
            waited = true
        }
        if (waited) Text(Strings.readerChaptersLoading(context), color = Chrome.faint, fontSize = 15.sp)
        return
    }
    // A flat list is the chapters alone: no heading, never folded.
    fun folded(index: Int, group: app.kaeri.engine.model.VolumeGroup) =
        !group.flat && group.allRead && index !in open && group.chapters.none { it.id == current }
    // The list opens with the open chapter's row as near the middle as its ends allow, set before
    // its first frame so nothing scrolls. A list that fits stays at its top.
    val here = remember(state.volumes) {
        var item = 0
        state.volumes.forEachIndexed { index, group ->
            if (!group.flat) item += 1
            val at = group.chapters.indexOfFirst { it.id == current }
            if (at >= 0) return@remember item + at
            if (!folded(index, group)) item += group.chapters.size
        }
        -1
    }
    val focus = remember { FocusRequester() }
    var focused by remember { mutableStateOf(false) }
    BoxWithConstraints(Modifier.fillMaxSize()) {
        val density = LocalDensity.current
        val list = rememberLazyListState(
            initialFirstVisibleItemIndex = here.coerceAtLeast(0),
            initialFirstVisibleItemScrollOffset = if (here < 0) 0 else with(density) { centredOffset(maxHeight.roundToPx(), ROW_HEIGHT.roundToPx()) },
        )
        LazyColumn(Modifier.fillMaxSize(), state = list) {
            state.volumes.forEachIndexed { index, group ->
                val folded = folded(index, group)
                if (!group.flat) item(key = "v$index") {
                    val name = group.volume
                    val words = when {
                        name == null -> Strings.readerVolumeNone(context)
                        folded -> Strings.readerVolumeRead(context, name)
                        else -> Strings.readerVolumeLine(context, name)
                    }
                    Text(
                        words,
                        color = Chrome.faint,
                        fontSize = 13.sp,
                        fontWeight = FontWeight.Medium,
                        modifier = Modifier
                            .fillMaxWidth()
                            .clickable(enabled = group.allRead) { open = if (index in open) open - index else open + index }
                            .padding(top = 12.dp, bottom = 6.dp),
                    )
                }
                if (!folded) {
                    items(group.chapters.size, key = { "c$index-$it" }) { at ->
                        val chapter = group.chapters[at]
                        val here = chapter.id == current
                        if (here && !focused) {
                            LaunchedEffect(Unit) {
                                runCatching { focus.requestFocus() }
                                focused = true
                            }
                        }
                        Row(
                            Modifier
                                .fillMaxWidth()
                                .heightIn(min = ROW_HEIGHT)
                                .clip(RoundedCornerShape(12.dp))
                                .background(if (here) Chrome.selected else Color.Transparent)
                                // TalkBack and the keyboard start on the open chapter.
                                .then(if (here) Modifier.focusRequester(focus).focusable() else Modifier)
                                .clickable { session.openChapter(chapter.id) }
                                .padding(horizontal = 12.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Text(
                                chapterWords(context, chapter),
                                color = if (here || chapter.state !is app.kaeri.engine.model.ChapterReadState.Read) Chrome.text else Chrome.faint,
                                fontSize = 15.sp,
                                maxLines = 1,
                            )
                        }
                    }
                }
            }
        }
    }
}

/** A chapter row's least height, which the list centres the open chapter's row by. */
private val ROW_HEIGHT = 46.dp
