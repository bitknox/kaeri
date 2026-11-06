// The Chapters sheet: a grid of the open chapter's pages with the current one ringed, and the
// series' chapters by volume, with read volumes folded and the open chapter on the selected fill.

package app.kaeri.reader.phone

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
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
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.itemsIndexed
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.kaeri.design.robotoFlex
import app.kaeri.engine.model.ChapterRow
import app.kaeri.i18n.Strings
import app.kaeri.reader.PageCanvas
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
) {
    val context = LocalContext.current
    val view = state.view ?: return
    var tab by remember { mutableStateOf(0) }
    val novel = view.medium == app.kaeri.engine.core.Medium.NOVEL
    LaunchedEffect(view.work) { session.loadChapters() }
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        containerColor = Chrome.plate,
        contentColor = Chrome.text,
        scrimColor = Color.Transparent,
    ) {
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
            Box(Modifier.heightIn(min = 240.dp, max = 460.dp).fillMaxWidth()) {
                if (tab == 0) {
                    PageGrid(state, images, currentLeaf, rtl, onGo)
                } else {
                    ChapterList(session, state)
                }
            }
            Spacer(Modifier.height(16.dp))
        }
    }
}

internal fun chapterWords(context: android.content.Context, chapter: ChapterRow): String {
    val number = chapterNumber(chapter)
    return chapter.title?.let { Strings.readerChapterLineTitled(context, number, it) } ?: Strings.readerChapterLine(context, number)
}

@Composable
private fun PageGrid(state: ReaderState, images: PageImages, currentLeaf: Int, rtl: Boolean, onGo: (Int) -> Unit) {
    val context = LocalContext.current
    val story = state.leaves.filter { !it.endPage }
    // A right-to-left series lays its grid out from the right, as its pages run.
    CompositionLocalProvider(LocalLayoutDirection provides if (rtl) LayoutDirection.Rtl else LayoutDirection.Ltr) {
        LazyVerticalGrid(
            columns = GridCells.Fixed(3),
            horizontalArrangement = Arrangement.spacedBy(10.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
            modifier = Modifier.fillMaxSize(),
        ) {
            itemsIndexed(story, key = { _, leaf -> leaf.index.toInt() }) { index, leaf ->
                val part = leaf.parts.firstOrNull()
                val bitmap = part?.let(state::file)?.let(images::cached)
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
                    if (bitmap != null) PageCanvas(bitmap, rotate = false, modifier = Modifier.fillMaxSize())
                    Text(
                        page.toString(),
                        color = if (bitmap != null) Color.White else Chrome.faint,
                        fontSize = 13.sp,
                        modifier = Modifier.align(Alignment.BottomEnd).padding(6.dp),
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
        Text(Strings.readerChaptersLoading(context), color = Chrome.faint, fontSize = 15.sp)
        return
    }
    LazyColumn(Modifier.fillMaxSize()) {
        state.volumes.forEachIndexed { index, group ->
            val folded = group.allRead && index !in open && group.chapters.none { it.id == current }
            item(key = "v$index") {
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
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .heightIn(min = 46.dp)
                            .clip(RoundedCornerShape(12.dp))
                            .background(if (here) Chrome.selected else Color.Transparent)
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
