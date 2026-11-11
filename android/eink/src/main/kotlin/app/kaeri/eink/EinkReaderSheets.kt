// The e-ink reader's settings and contents, as the e-ink board draws them. The sheet is opaque over
// the lower part of the page with a 2 dp rule, and the page above changes as a setting does; This
// series holds the mode, the medium's group holds the rest of the first level, and More holds every
// other row. Contents is the chapters as a paged list, opened on the page with the one being read.
// Nothing animates: each change is one paint through the refresh policy.

package app.kaeri.eink

import android.content.Context
import androidx.compose.foundation.background
import androidx.compose.foundation.focusable
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.focusTarget
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import app.kaeri.engine.core.ChapterId
import app.kaeri.engine.model.ChapterRow
import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.ReaderMode
import app.kaeri.engine.model.ReaderSettingKey
import app.kaeri.engine.model.ReaderSettingRow
import app.kaeri.engine.model.ReaderValue
import app.kaeri.engine.model.SettingScope
import app.kaeri.i18n.Strings
import app.kaeri.reader.phone.ReaderLook
import app.kaeri.reader.ReaderSession
import app.kaeri.reader.ReaderState
import app.kaeri.reader.phone.chapterWords
import app.kaeri.reader.phone.choiceLabel
import app.kaeri.reader.phone.rowTitle
import kotlin.math.abs
import kotlin.math.roundToInt

private typealias Key = ReaderSettingKey

/** What covers the e-ink page: nothing, the settings sheet, its More page, or Contents. */
enum class EinkLayer { None, Settings, More, Contents }

/** The keys the sheet's first level shows; every other row the engine sends is on More. */
private val mangaFirst = listOf(Key.FIT, Key.CROP_BORDERS, Key.REFRESH_IMAGE, Key.VOLUME_KEYS_TURN)
private val novelFirst = listOf(Key.TEXT_SIZE, Key.FONT, Key.LINE_SPACING, Key.MARGINS, Key.LOOK, Key.REFRESH_TEXT)

/** The rows for the e-ink screen itself, under their own heading on More. */
private val screenKeys = listOf(Key.REFRESH_IMAGE, Key.REFRESH_TEXT, Key.REFRESH_LIST, Key.GALLERY_SPEED, Key.RAINBOW_FILTER, Key.SATURATION, Key.CONTRAST)

/**
 * The settings sheet over the lower part of the page: Reader settings and Done, This series with
 * the mode for manga, the medium's own rows, and More reader settings.
 */
@Composable
fun BoxScope.EinkSettingsSheet(session: ReaderSession, state: ReaderState, onMore: () -> Unit, onDone: () -> Unit) {
    val context = LocalContext.current
    val view = state.view ?: return
    val rows = state.settings?.rows.orEmpty()
    fun row(key: Key) = rows.firstOrNull { it.key == key }
    val novel = view.medium == Medium.NOVEL
    SheetFrame(Strings.readerSettings(context), onDone, Modifier.align(Alignment.BottomCenter)) {
        if (novel) {
            SheetHeading(Strings.readerSheetAllNovels(context))
            novelFirst.forEach { key -> row(key)?.let { EinkSettingLine(context, session, it) } }
        } else {
            SheetHeading(Strings.readerSheetSeries(context))
            EinkModeLine(context, session, row(Key.MODE), row(Key.DIRECTION), view.title)
            val strip = view.mode == ReaderMode.STRIP
            SheetHeading(if (strip) Strings.readerSheetAllWebtoons(context) else Strings.readerSheetAllManga(context))
            mangaFirst.forEach { key -> row(key)?.let { EinkSettingLine(context, session, it) } }
        }
        Spacer(Modifier.height(8.dp))
        EinkRow(onClick = onMore, height = EinkSizes.SheetRow) {
            Text(Strings.readerSheetMore(context), style = EinkType.Label, modifier = Modifier.weight(1f))
            Text("›", style = EinkType.Button)
        }
    }
}

/** Every row the first level leaves out, grouped as the phone's More page groups them. */
@Composable
fun BoxScope.EinkMoreSheet(session: ReaderSession, state: ReaderState, onBack: () -> Unit, onDone: () -> Unit) {
    val context = LocalContext.current
    val view = state.view ?: return
    val rows = state.settings?.rows.orEmpty()
    val novel = view.medium == Medium.NOVEL
    val first = if (novel) novelFirst else mangaFirst + Key.MODE + Key.DIRECTION
    SheetFrame(Strings.readerSheetMore(context), onDone, Modifier.align(Alignment.BottomCenter), onBack = onBack) {
        val rest = rows.filter { it.key !in first && it.key !in screenKeys && einkRowTitle(context, it.key) != null }
        rest.forEach { EinkSettingLine(context, session, it) }
        val screen = rows.filter { it.key in screenKeys && it.key !in first }
        if (screen.isNotEmpty()) {
            SheetHeading(Strings.einkSheetScreen(context))
            screen.forEach { EinkSettingLine(context, session, it) }
        }
    }
}

/** The sheet's frame: opaque paper with a 2 dp rule over it, its title and Done, then the rows. */
@Composable
private fun SheetFrame(
    title: String,
    onDone: () -> Unit,
    modifier: Modifier,
    onBack: (() -> Unit)? = null,
    content: @Composable ColumnScope.() -> Unit,
) {
    val context = LocalContext.current
    BoxWithConstraints(Modifier.fillMaxSize()) {
        val most = maxHeight * 0.75f
        Column(
            modifier
                .fillMaxWidth()
                .heightIn(max = most)
                .background(EinkColours.Paper)
                .topRule()
                .holdsTouches()
                .navigationBarsPadding()
                .padding(horizontal = EinkSizes.SheetPadding, vertical = 12.dp),
        ) {
            Row(Modifier.fillMaxWidth().height(EinkSizes.SheetRow), verticalAlignment = Alignment.CenterVertically) {
                if (onBack != null) {
                    Box(
                        Modifier.size(52.dp).semantics { contentDescription = Strings.einkBack(context) }.einkClick(onClick = onBack),
                        contentAlignment = Alignment.Center,
                    ) { Text("‹", style = EinkType.PageTitle) }
                }
                Text(title, style = EinkType.ReaderTitle, modifier = Modifier.weight(1f).semantics { heading() })
                EinkTextButton(Strings.actionDone(context), onDone)
            }
            Column(Modifier.verticalScroll(rememberScrollState())) { content() }
        }
    }
}

@Composable
private fun SheetHeading(text: String) {
    Text(
        text,
        style = EinkType.Secondary,
        modifier = Modifier.fillMaxWidth().padding(top = 14.dp, bottom = 4.dp).semantics { heading() },
    )
}

/** A row of the sheet: its name in the label column, the control beside it. */
@Composable
private fun SheetLine(title: String, content: @Composable RowScope.() -> Unit) {
    Row(Modifier.fillMaxWidth().heightIn(min = EinkSizes.SheetRow).padding(vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(title, style = EinkType.Label, modifier = Modifier.width(EinkSizes.SheetLabel).padding(end = 12.dp))
        content()
    }
}

/**
 * Reading mode for this series: Auto, Right to left, Left to right or Strip, and where it came
 * from under it. Auto takes the series details' mode back; the others are kept for the series.
 */
@Composable
private fun EinkModeLine(context: Context, session: ReaderSession, mode: ReaderSettingRow?, direction: ReaderSettingRow?, title: String) {
    if (mode == null) return
    val m = (mode.value as? ReaderValue.Choice)?.name
    val d = (direction?.value as? ReaderValue.Choice)?.name
    val auto = !mode.overridden && direction?.overridden != true
    val chosen = when {
        auto -> "auto"
        m == "strip" -> "strip"
        d == "ltr" -> "ltr"
        else -> "rtl"
    }
    SheetLine(Strings.readerRowReadingMode(context)) {
        EinkSegmented(
            choices = listOf("auto", "rtl", "ltr", "strip"),
            chosen = chosen,
            label = { choice ->
                when (choice) {
                    "auto" -> Strings.readerModeAutoName(context)
                    "rtl" -> Strings.readerModeRtl(context)
                    "ltr" -> Strings.readerModeLtr(context)
                    else -> Strings.readerModeStrip(context)
                }
            },
            onChoose = { choice ->
                when (choice) {
                    "auto" -> {
                        session.useDefault(Key.MODE)
                        session.useDefault(Key.DIRECTION)
                    }
                    "strip" -> session.setSetting(Key.MODE, SettingScope.SERIES, ReaderValue.Choice("strip"))
                    else -> {
                        session.setSetting(Key.MODE, SettingScope.SERIES, ReaderValue.Choice("paged"))
                        session.setSetting(Key.DIRECTION, SettingScope.SERIES, ReaderValue.Choice(choice))
                    }
                }
            },
            modifier = Modifier.weight(1f),
        )
    }
    Text(
        if (auto) Strings.readerModeDetected(context) else Strings.readerModeSet(context, title),
        style = EinkType.Tertiary,
        modifier = Modifier.padding(bottom = 4.dp),
    )
}

/** One setting as the e-ink sheet draws it: a segmented control, or a stepper for an amount. */
@Composable
private fun EinkSettingLine(context: Context, session: ReaderSession, row: ReaderSettingRow) {
    val title = einkRowTitle(context, row.key) ?: return
    val value = row.value
    fun set(to: ReaderValue) = session.setSetting(row.key, row.scope, to)
    SheetLine(title) {
        when {
            // The foot shows pages left or nothing: On and Off, as on the phone.
            row.key == Key.FOOT_VALUE && value is ReaderValue.Choice -> EinkSegmented(
                choices = listOf(true, false),
                chosen = value.name == "pages_left",
                label = { if (it) Strings.readerValueOn(context) else Strings.readerValueOff(context) },
                onChoose = { set(ReaderValue.Choice(if (it) "pages_left" else "none")) },
                modifier = Modifier.weight(1f),
            )
            value is ReaderValue.Toggle -> EinkSegmented(
                choices = listOf(true, false),
                chosen = value.on,
                label = { if (it) Strings.readerValueOn(context) else Strings.readerValueOff(context) },
                onChoose = { set(ReaderValue.Toggle(it)) },
                modifier = Modifier.weight(1f),
            )
            value is ReaderValue.Amount && (row.key == Key.LINE_SPACING || row.key == Key.MARGINS) -> {
                val presets = if (row.key == Key.LINE_SPACING) spacingPresets else marginPresets
                val labels = if (row.key == Key.LINE_SPACING) {
                    listOf(Strings.readerSpacingTight(context), Strings.readerSpacingNormal(context), Strings.readerSpacingLoose(context))
                } else {
                    listOf(Strings.readerMarginsNarrow(context), Strings.readerSpacingNormal(context), Strings.readerMarginsWide(context))
                }
                val at = presets.indices.minBy { abs(presets[it] - value.value) }
                EinkSegmented(presets.indices.toList(), at, { labels[it] }, { set(ReaderValue.Amount(presets[it])) }, Modifier.weight(1f))
            }
            value is ReaderValue.Amount -> {
                val steps = amountSteps(row.key)
                val words = if (steps.pages) {
                    Strings.einkRefreshEvery(context, value.value.roundToInt())
                } else {
                    Strings.einkPercent(context, (value.value * 100).roundToInt())
                }
                fun step(by: Int) {
                    val next = ((value.value + by * steps.step) / steps.step).roundToInt() * steps.step
                    set(ReaderValue.Amount(next.coerceIn(steps.least, steps.most)))
                }
                val text = row.key == Key.TEXT_SIZE
                EinkStepper(
                    words = words,
                    title = title,
                    down = if (text) Strings.readerSizeSmall(context) else Strings.einkStepDown(context),
                    up = if (text) Strings.readerSizeLarge(context) else Strings.einkStepUp(context),
                    canDown = value.value > steps.least + 1e-6,
                    canUp = value.value < steps.most - 1e-6,
                    onDown = { step(-1) },
                    onUp = { step(1) },
                    modifier = Modifier.weight(1f),
                )
            }
            row.choices.isNotEmpty() -> {
                // On paper the looks are Paper and Dark.
                val offered = if (row.key == Key.LOOK) row.choices.filter { (it as? ReaderValue.Choice)?.name in setOf("paper", "dark") } else row.choices
                val labelled = offered.mapNotNull { choice -> einkChoiceLabel(context, row.key, choice)?.let { choice to it } }
                if (labelled.size <= 4) {
                    EinkSegmented(labelled.map { it.first }, value, { choice -> labelled.first { it.first == choice }.second }, { set(it) }, Modifier.weight(1f))
                } else {
                    // Too many for one control: the chosen one between arrows that go round them.
                    val at = labelled.indexOfFirst { it.first == value }.coerceAtLeast(0)
                    EinkStepper(
                        words = labelled[at].second,
                        title = title,
                        down = Strings.readerActionPrevious(context),
                        up = Strings.readerActionNext(context),
                        canDown = true,
                        canUp = true,
                        onDown = { set(labelled[(at - 1 + labelled.size) % labelled.size].first) },
                        onUp = { set(labelled[(at + 1) % labelled.size].first) },
                        modifier = Modifier.weight(1f),
                        glyphs = "‹" to "›",
                    )
                }
            }
        }
    }
    // Which key goes forward, under the switch that lets them turn pages: the phone's own choice.
    if (row.key == Key.VOLUME_KEYS_TURN && (value as? ReaderValue.Toggle)?.on == true) {
        SheetLine(Strings.readerRowVolumeSwap(context)) {
            EinkSegmented(
                choices = listOf(true, false),
                chosen = ReaderLook.volumeSwapped(context),
                label = { if (it) Strings.readerValueOn(context) else Strings.readerValueOff(context) },
                onChoose = { ReaderLook.swapVolume(context, it) },
                modifier = Modifier.weight(1f),
            )
        }
    }
}

/** Less and more around the value, the value between them as the stepper's state. */
@Composable
private fun EinkStepper(
    words: String,
    title: String,
    down: String,
    up: String,
    canDown: Boolean,
    canUp: Boolean,
    onDown: () -> Unit,
    onUp: () -> Unit,
    modifier: Modifier = Modifier,
    glyphs: Pair<String, String> = "−" to "+",
) {
    Row(modifier.semantics { stateDescription = words }, verticalAlignment = Alignment.CenterVertically) {
        StepButton(glyphs.first, down, canDown, onDown)
        Text(
            words,
            style = EinkType.Label.copy(fontWeight = FontWeight.Bold, fontFeatureSettings = "tnum"),
            textAlign = TextAlign.Center,
            modifier = Modifier.weight(1f).semantics { contentDescription = "$title, $words" }, // i18n-skip
        )
        StepButton(glyphs.second, up, canUp, onUp)
    }
}

@Composable
private fun StepButton(glyph: String, description: String, enabled: Boolean, onClick: () -> Unit) {
    val ink = if (enabled) EinkColours.Ink else EinkColours.PlaceholderEdge
    val shape = RoundedCornerShape(EinkSizes.SegmentRadius)
    Box(
        Modifier
            .size(EinkSizes.SegmentHeight + 8.dp, EinkSizes.SegmentHeight)
            .clip(shape)
            .border(EinkSizes.Rule, ink, shape)
            .semantics { contentDescription = description }
            .einkClick(enabled = enabled, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Text(glyph, style = EinkType.Button.copy(color = ink))
    }
}

private class Steps(val step: Double, val least: Double, val most: Double, val pages: Boolean = false)

private fun amountSteps(key: Key): Steps = when (key) {
    Key.TEXT_SIZE -> Steps(0.1, 0.7, 2.0)
    Key.REFRESH_TEXT, Key.REFRESH_LIST -> Steps(1.0, 1.0, 20.0, pages = true)
    Key.SATURATION, Key.CONTRAST -> Steps(0.1, 0.5, 1.5)
    else -> Steps(0.1, 0.5, 2.0)
}

private val spacingPresets = listOf(1.3, 1.6, 1.9)
private val marginPresets = listOf(0.6, 1.0, 1.5)

/** The row's name on e-ink: the reader's own, or the name of a row only an e-ink screen has. */
fun einkRowTitle(context: Context, key: Key): String? = when (key) {
    Key.VOLUME_KEYS_TURN -> Strings.readerRowVolume(context)
    Key.REFRESH_IMAGE -> Strings.einkRowRefreshImage(context)
    Key.REFRESH_TEXT -> Strings.einkRowRefreshText(context)
    Key.REFRESH_LIST -> Strings.einkRowRefreshList(context)
    Key.GALLERY_SPEED -> Strings.einkRowScreenSpeed(context)
    Key.RAINBOW_FILTER -> Strings.einkRowRainbow(context)
    Key.SATURATION -> Strings.einkRowSaturation(context)
    Key.CONTRAST -> Strings.einkRowContrast(context)
    else -> rowTitle(context, key)
}

/** The choice's name on e-ink: the reader's own, or one of Screen speed's. */
fun einkChoiceLabel(context: Context, key: Key, value: ReaderValue): String? {
    if (key != Key.GALLERY_SPEED) return choiceLabel(context, key, value)
    return when ((value as? ReaderValue.Choice)?.name) {
        "standard" -> Strings.einkSpeedStandard(context)
        "fast" -> Strings.einkSpeedFast(context)
        "best" -> Strings.einkSpeedBest(context)
        else -> null
    }
}

/** A line of Contents: a volume's heading, or a chapter. */
private sealed interface ContentsLine {
    data class Heading(val words: String) : ContentsLine
    data class Chapter(val row: ChapterRow) : ContentsLine
}

/**
 * Contents: the series' chapters as a paged list under a bar with Back, opened on the page with
 * the chapter being read. A chapter opens in the reader; Back returns to the page. [onPage] paints
 * each page of the list.
 */
@Composable
fun EinkContents(
    session: ReaderSession,
    state: ReaderState,
    onBack: () -> Unit,
    onOpen: (ChapterId) -> Unit,
    onPage: () -> Unit,
    keys: ContentsKeys = remember { ContentsKeys() },
) {
    val context = LocalContext.current
    val view = state.view ?: return
    LaunchedEffect(view.work) { session.loadChapters() }
    val current = state.chapter
    Column(Modifier.fillMaxSize().background(EinkColours.Paper).holdsTouches()) {
        Row(
            Modifier.fillMaxWidth().background(EinkColours.Paper).bottomRule().statusBarsPadding().height(EinkSizes.TopBar).padding(horizontal = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Box(
                Modifier.size(56.dp).semantics { contentDescription = Strings.einkBack(context) }.einkClick(onClick = onBack),
                contentAlignment = Alignment.Center,
            ) { Text("‹", style = EinkType.PageTitle) }
            Column(Modifier.weight(1f).padding(horizontal = 8.dp)) {
                Text(Strings.einkContents(context), style = EinkType.ReaderTitle, maxLines = 1, modifier = Modifier.semantics { heading() })
                Text(view.title, style = EinkType.Secondary, maxLines = 1)
            }
        }
        val volumes = state.volumes
        if (volumes.isEmpty()) {
            PaperWaiting {
                Text(
                    Strings.readerChaptersLoading(context),
                    style = EinkType.Waiting,
                    modifier = Modifier.fillMaxWidth().padding(24.dp),
                    textAlign = TextAlign.Center,
                )
            }
            return@Column
        }
        val lines = remember(volumes) {
            volumes.flatMap { group ->
                val heading = group.volume?.let { Strings.readerVolumeLine(context, it) } ?: Strings.readerVolumeNone(context)
                listOf<ContentsLine>(ContentsLine.Heading(heading)) + group.chapters.map { ContentsLine.Chapter(it) }
            }
        }
        BoxWithConstraints(Modifier.fillMaxWidth().weight(1f).navigationBarsPadding()) {
            val perPage = rowsThatFit(maxHeight, EinkSizes.ChapterRow)
            val pages = remember(lines, perPage) { pagesOf(lines, perPage) }
            val here = lines.indexOfFirst { it is ContentsLine.Chapter && it.row.id == current }.coerceAtLeast(0)
            var page by remember(pages.size) { mutableIntStateOf((here / perPage).coerceIn(0, pages.size - 1)) }
            // TalkBack and the keyboard start on the chapter being read, once, as the page opens.
            val focus = remember { FocusRequester() }
            var focused by remember { mutableStateOf(false) }
            // A turned page drops the row that had the focus, and the reader's keys follow the
            // focus, so the page itself keeps it.
            val pageFocus = remember { FocusRequester() }
            LaunchedEffect(page) {
                onPage()
                if (focused) runCatching { pageFocus.requestFocus() }
            }
            // The page buttons and volume keys turn the list's pages while it shows.
            SideEffect { keys.turn = { by -> page = (page + by).coerceIn(0, pages.size - 1) } }
            Column(Modifier.fillMaxSize()) {
                Column(Modifier.weight(1f).focusRequester(pageFocus).focusTarget()) {
                    pages[page].forEach { line ->
                        when (line) {
                            is ContentsLine.Heading -> Text(
                                line.words,
                                style = EinkType.Secondary,
                                modifier = Modifier.fillMaxWidth().height(EinkSizes.ChapterRow).padding(horizontal = 20.dp).padding(top = 28.dp).semantics { heading() },
                            )
                            is ContentsLine.Chapter -> {
                                val reading = line.row.id == current
                                if (reading && !focused) {
                                    LaunchedEffect(Unit) {
                                        runCatching { focus.requestFocus() }
                                        focused = true
                                    }
                                }
                                EinkRow(
                                    onClick = { onOpen(line.row.id) },
                                    modifier = if (reading) Modifier.focusRequester(focus).focusable() else Modifier,
                                    selected = reading,
                                    height = EinkSizes.ChapterRow,
                                ) {
                                    Text(
                                        chapterWords(context, line.row),
                                        style = if (reading) EinkType.Label.copy(fontWeight = FontWeight.Bold) else EinkType.Label,
                                        maxLines = 1,
                                        modifier = Modifier.weight(1f),
                                    )
                                    if (reading) Text(Strings.einkContentsReading(context), style = EinkType.Secondary)
                                }
                            }
                        }
                    }
                }
                if (pages.size > 1) EinkPager(page, pages.size, onPage = { page = it.coerceIn(0, pages.size - 1) })
            }
        }
    }
}

/** The reader's keys while Contents shows: its pages turn, not the reader's page under it. */
class ContentsKeys {
    /** Turn [by] pages of the list, -1 back and 1 on. */
    internal var turn: (by: Int) -> Unit = {}

    fun forward() = turn(1)

    fun back() = turn(-1)
}

/** Touches here stay here, never reaching the page under it. */
private fun Modifier.holdsTouches(): Modifier = pointerInput(Unit) { detectTapGestures { } }
