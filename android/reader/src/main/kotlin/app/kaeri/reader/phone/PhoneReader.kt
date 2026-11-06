// The phone's reader: a chapter's pages or strip over the engine's reader session, with the
// chrome the board draws (a top bar, a thumb group with a scrubber), the notices, the end page
// and the settings sheet. The engine decides what is shown and when; this draws it.

package app.kaeri.reader.phone

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.kaeri.design.ObiSilk
import app.kaeri.engine.model.ChapterRow
import app.kaeri.engine.model.ReadDirection
import app.kaeri.engine.model.ReaderAction as NoticeAction
import app.kaeri.engine.model.ReaderMode
import app.kaeri.engine.model.ReaderSettingKey
import app.kaeri.engine.model.ReaderValue
import app.kaeri.engine.model.TurnHow
import app.kaeri.i18n.Strings
import app.kaeri.reader.PageImages
import app.kaeri.reader.PagedReader
import app.kaeri.reader.ReaderAction
import app.kaeri.reader.ReaderKeyRules
import app.kaeri.reader.ReaderSession
import app.kaeri.reader.ReaderState
import app.kaeri.reader.StripReader
import app.kaeri.reader.TapZones
import app.kaeri.reader.readerKeys
import app.kaeri.reader.readerSurface
import app.kaeri.reader.rememberPagedReaderState
import app.kaeri.reader.rightToLeft
import app.kaeri.reader.text
import app.kaeri.store.text

/** What the phone reader leaves to the app around it. */
class PhoneReaderActions(
    val onClose: () -> Unit,
    val onSeries: () -> Unit = onClose,
    val onDownloads: () -> Unit = onClose,
)

/** The colours of the reader's own chrome: the app's, or Paper mode's dimmed white. */
internal object Chrome {
    val scrim = Color(0xE6161A2E)
    val plate = ObiSilk.Raised
    val text = ObiSilk.Silk
    val faint = ObiSilk.Haze
    val accent = ObiSilk.Amber
    val rule = ObiSilk.Rule
    val paper = Color(0xFFE9E5DA)
}

/** The setting [key]'s current choice name, or null. */
internal fun ReaderState.choice(key: ReaderSettingKey): String? =
    (settings?.rows?.firstOrNull { it.key == key }?.value as? ReaderValue.Choice)?.name

internal fun ReaderState.toggle(key: ReaderSettingKey): Boolean =
    (settings?.rows?.firstOrNull { it.key == key }?.value as? ReaderValue.Toggle)?.on == true

/**
 * The phone reader for [session]. Pages and strips are drawn here; a novel is drawn by [novel],
 * which the app supplies with the engine's book server.
 */
@Composable
fun PhoneReader(
    session: ReaderSession,
    state: ReaderState,
    images: PageImages,
    actions: PhoneReaderActions,
    novel: @Composable (onControls: () -> Unit) -> Unit,
    modifier: Modifier = Modifier,
    quiet: QuietPhone? = null,
) {
    val context = LocalContext.current
    val view = state.view
    val density = LocalDensity.current
    val dark = true
    val direction = view?.direction ?: ReadDirection.RIGHT_TO_LEFT
    val paper = state.toggle(ReaderSettingKey.PAPER_MODE)
    var chrome by rememberSaveable { mutableStateOf(false) }
    var sheet by rememberSaveable { mutableStateOf(false) }
    val pager = rememberPagedReaderState()
    val volume = state.toggle(ReaderSettingKey.VOLUME_KEYS_TURN) || paper

    // Quiet the phone while the reader is open and the setting is on.
    val quietOn = state.toggle(ReaderSettingKey.QUIET_PHONE)
    LaunchedEffect(quietOn, quiet) { quiet?.set(quietOn) }
    androidx.compose.runtime.DisposableEffect(quiet) { onDispose { quiet?.set(false) } }

    fun act(action: ReaderAction) {
        when (action) {
            ReaderAction.FORWARD -> pager.forward(TurnHow.KEY)
            ReaderAction.BACK -> pager.back(TurnHow.KEY)
            ReaderAction.CONTROLS -> chrome = !chrome
            ReaderAction.CLEAN -> {}
        }
    }

    // Back closes what is over the pages first, then the reader.
    BackHandler(enabled = sheet || chrome) {
        if (sheet) sheet = false else chrome = false
    }
    BackHandler(enabled = !sheet && !chrome) { actions.onClose() }

    Box(
        modifier
            .fillMaxSize()
            .background(if (paper) Chrome.paper else Color.Black)
            .readerSurface(density, eink = false, dark = dark) { surface ->
                if (state.view == null && !state.opening) return@readerSurface
                session.surface(surface)
            }
            .readerKeys(ReaderKeyRules(direction, volumeTurns = volume), ::act),
    ) {
        val failure = state.failure
        val novelOpen = state.novel != null || view?.medium == app.kaeri.engine.core.Medium.NOVEL
        when {
            failure != null && state.leaves.isEmpty() && state.novel == null ->
                Failed(Strings.readerFailedChapter(context, view?.chapter?.let(::chapterNumber) ?: ""), failure.text(context), actions.onClose) {
                    session.retry(null)
                }
            novelOpen && view != null -> novel { chrome = !chrome }
            state.leaves.isEmpty() -> Opening(view?.chapter?.let(::chapterNumber))
            view?.mode == ReaderMode.STRIP -> StripReader(
                state = state,
                images = images,
                onAt = { at, foot -> session.stripAt(at, foot) },
                onTap = { chrome = !chrome },
                onScrolled = { chrome = false },
                onMissing = session::needPage,
                onRetry = { session.retry(it.toUInt()) },
                onJumped = session::jumped,
                footer = { PhoneEndPage(state, session, actions) },
            )
            else -> PagedReader(
                state = state,
                pager = pager,
                images = images,
                direction = direction,
                zones = TapZones.phone(direction),
                onControls = { chrome = !chrome },
                onShown = { leaf, how ->
                    session.shown(leaf, how)
                    if (how != TurnHow.JUMP) chrome = false
                },
                onMissing = session::needPage,
                onRetry = { session.retry(it.toUInt()) },
                onJumped = session::jumped,
                endPage = { PhoneEndPage(state, session, actions) },
                motion = !paper,
            )
        }

        if (view != null) {
            AnimatedVisibility(
                visible = chrome,
                enter = fadeIn(),
                exit = fadeOut(),
                modifier = Modifier.align(Alignment.TopCenter),
            ) {
                TopBar(view.title, view.chapter, novelOpen, actions.onClose) { sheet = true }
            }
            AnimatedVisibility(
                visible = chrome && !novelOpen,
                enter = fadeIn(),
                exit = fadeOut(),
                modifier = Modifier.align(Alignment.BottomCenter),
            ) {
                ThumbGroup(
                    state = state,
                    rtl = direction.rightToLeft && view.mode != ReaderMode.STRIP,
                    shownLeaf = pager.leaf,
                    onGo = { pager.go(it, TurnHow.JUMP) },
                    onPrevious = { view.previous?.let { session.openChapter(it.id) } },
                    onNext = { view.next?.let { session.openChapter(it.id) } },
                    hasPrevious = view.previous != null,
                    hasNext = view.next != null,
                )
            }
        }

        // Notices sit over the foot of the page while the chrome is away.
        val notice = state.notice
        if (notice != null && !chrome) {
            if (notice.notice.action == NoticeAction.BACK_TO) {
                BackToPage(notice.notice.msg.text(context), Modifier.align(Alignment.TopCenter)) {
                    session.backTo()
                    session.dismissNotice(notice.id)
                }
            } else {
                NoticePlate(notice, Modifier.align(Alignment.BottomCenter)) { action ->
                    when (action) {
                        NoticeAction.START_OVER -> session.startOver()
                        NoticeAction.BACK_TO -> session.backTo()
                        NoticeAction.GO_BACK -> session.goBack()
                        NoticeAction.CHANGE_MODE -> sheet = true
                    }
                    session.dismissNotice(notice.id)
                }
            }
        }
        if (notice != null) {
            val id = notice.id
            val lasts = notice.notice.lastsMs.toLong()
            LaunchedEffect(id) {
                if (lasts > 0) {
                    kotlinx.coroutines.delay(lasts)
                    session.dismissNotice(id)
                }
            }
        }
    }

    if (sheet && view != null) {
        ReaderSettingsSheet(session, state, onDismiss = { sheet = false })
    }
}

/** The chapter's number as the strings name it. */
internal fun chapterNumber(row: ChapterRow): String = row.number ?: row.label

@Composable
private fun TopBar(title: String, chapter: ChapterRow, novel: Boolean, onClose: () -> Unit, onSettings: () -> Unit) {
    val context = LocalContext.current
    Row(
        Modifier
            .fillMaxWidth()
            .background(Chrome.scrim)
            .windowInsetsPadding(WindowInsets.statusBars)
            .padding(horizontal = 8.dp, vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        TextButton(Strings.readerClose(context), onClose)
        Column(Modifier.weight(1f).padding(horizontal = 8.dp), horizontalAlignment = Alignment.CenterHorizontally) {
            Text(title, color = Chrome.text, fontSize = 15.sp, fontWeight = FontWeight.Medium, maxLines = 1, overflow = TextOverflow.Ellipsis)
            val number = chapterNumber(chapter)
            val line = when {
                novel -> Strings.readerNovelLine(context, number)
                chapter.title != null -> Strings.readerChapterLineTitled(context, number, chapter.title!!)
                else -> Strings.readerChapterLine(context, number)
            }
            Text(line, color = Chrome.faint, fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        TextButton(Strings.readerSettings(context), onSettings)
    }
}

@Composable
internal fun TextButton(label: String, onClick: () -> Unit, modifier: Modifier = Modifier, colour: Color = Chrome.accent) {
    Text(
        label,
        color = colour,
        fontSize = 15.sp,
        fontWeight = FontWeight.Medium,
        maxLines = 1,
        modifier = modifier
            .clip(RoundedCornerShape(10.dp))
            .clickable(onClick = onClick)
            .padding(horizontal = 10.dp, vertical = 10.dp),
    )
}

@Composable
private fun ThumbGroup(
    state: ReaderState,
    rtl: Boolean,
    shownLeaf: Int,
    onGo: (Int) -> Unit,
    onPrevious: () -> Unit,
    onNext: () -> Unit,
    hasPrevious: Boolean,
    hasNext: Boolean,
) {
    val context = LocalContext.current
    val count = state.pages.coerceAtLeast(1)
    val leaf = shownLeaf.coerceIn(0, (state.leaves.size - 1).coerceAtLeast(0))
    val page = (state.leaves.getOrNull(leaf)?.parts?.firstOrNull()?.page?.toInt() ?: 0).coerceIn(0, count - 1)
    var dragging by remember { mutableStateOf<Float?>(null) }
    val shown = dragging ?: page.toFloat()
    val left = (count - 1 - page).coerceAtLeast(0)
    Column(
        Modifier
            .fillMaxWidth()
            .background(Chrome.scrim)
            .windowInsetsPadding(WindowInsets.navigationBars)
            .padding(horizontal = 16.dp, vertical = 10.dp),
    ) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Text("${shown.toInt() + 1} / $count", color = Chrome.text, fontSize = 13.sp)
            Text(Strings.readerPagesLeft(context, left), color = Chrome.faint, fontSize = 13.sp)
        }
        val desc = Strings.readerScrubber(context)
        // A right-to-left chapter's scrubber runs from the right, as its pages do.
        val direction = if (rtl) androidx.compose.ui.unit.LayoutDirection.Rtl else androidx.compose.ui.unit.LayoutDirection.Ltr
        androidx.compose.runtime.CompositionLocalProvider(androidx.compose.ui.platform.LocalLayoutDirection provides direction) {
            Slider(
                value = shown,
                onValueChange = { dragging = it },
                onValueChangeFinished = {
                    dragging?.let { target ->
                        state.leafOf(target.toInt().toUInt())?.let(onGo)
                    }
                    dragging = null
                },
                valueRange = 0f..(count - 1).coerceAtLeast(1).toFloat(),
                colors = SliderDefaults.colors(thumbColor = Chrome.accent, activeTrackColor = Chrome.accent, inactiveTrackColor = Chrome.rule),
                modifier = Modifier.semantics { contentDescription = desc },
            )
        }
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            if (hasPrevious) TextButton("‹", onPrevious, colour = Chrome.text) else Spacer(Modifier.height(1.dp))
            if (hasNext) TextButton("›", onNext, colour = Chrome.text) else Spacer(Modifier.height(1.dp))
        }
    }
}

@Composable
private fun NoticePlate(notice: app.kaeri.reader.ShownNotice, modifier: Modifier, onAction: (NoticeAction) -> Unit) {
    val context = LocalContext.current
    val action = notice.notice.action
    Row(
        modifier
            .windowInsetsPadding(WindowInsets.navigationBars)
            .padding(16.dp)
            .fillMaxWidth()
            .background(Chrome.plate, RoundedCornerShape(14.dp))
            .border(1.dp, Chrome.rule, RoundedCornerShape(14.dp))
            .padding(start = 16.dp, end = 6.dp, top = 4.dp, bottom = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(notice.notice.msg.text(context), color = Chrome.text, fontSize = 14.sp, modifier = Modifier.weight(1f))
        if (action != null) {
            val words = when (action) {
                NoticeAction.START_OVER -> Strings.readerNoticeStartOver(context)
                NoticeAction.BACK_TO -> notice.notice.msg.text(context)
                NoticeAction.GO_BACK -> Strings.readerNoticeGoBack(context)
                NoticeAction.CHANGE_MODE -> Strings.readerNoticeChange(context)
            }
            TextButton(words, { onAction(action) })
        }
    }
}

@Composable
private fun BackToPage(words: String, modifier: Modifier, onClick: () -> Unit) {
    Box(
        modifier
            .windowInsetsPadding(WindowInsets.statusBars)
            .padding(top = 12.dp)
            .clip(RoundedCornerShape(50))
            .background(Chrome.plate)
            .border(1.dp, Chrome.rule, RoundedCornerShape(50))
            .clickable(onClick = onClick)
            .padding(horizontal = 18.dp, vertical = 10.dp),
    ) {
        Text(words, color = Chrome.accent, fontSize = 14.sp, fontWeight = FontWeight.Medium)
    }
}

@Composable
private fun Opening(number: String?) {
    val context = LocalContext.current
    Column(Modifier.fillMaxSize(), verticalArrangement = Arrangement.Center, horizontalAlignment = Alignment.CenterHorizontally) {
        CircularProgressIndicator(color = Chrome.accent, strokeWidth = 2.dp, modifier = Modifier.height(28.dp))
        Spacer(Modifier.height(14.dp))
        if (number != null) Text(Strings.readerOpening(context, number), color = Chrome.faint, fontSize = 14.sp)
    }
}

@Composable
private fun Failed(title: String, line: String, onClose: () -> Unit, onRetry: () -> Unit) {
    val context = LocalContext.current
    Column(
        Modifier.fillMaxSize().padding(32.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(title, color = Chrome.text, fontSize = 18.sp, fontWeight = FontWeight.Medium)
        Spacer(Modifier.height(8.dp))
        Text(line, color = Chrome.faint, fontSize = 14.sp)
        Spacer(Modifier.height(20.dp))
        Row {
            TextButton(app.kaeri.i18n.Strings.readerClose(context), onClose, colour = Chrome.text)
            TextButton(Strings.readerTryAgain(context), onRetry)
        }
    }
}
