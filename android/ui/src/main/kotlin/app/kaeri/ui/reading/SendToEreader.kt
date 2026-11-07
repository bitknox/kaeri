// Send to e-reader: the sheet that prepares chapters for an e-reader and hands them to the share
// sheet, from the series page and from Downloads (figs 62, 63); the rows that keep a send in view
// once its sheet is closed; and Where did you stop, which the series page asks after a send, since
// the e-reader keeps its place to itself (fig 64).

package app.kaeri.ui.reading

import android.content.Context
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.RadioButton
import androidx.compose.material3.RadioButtonDefaults
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.design.Motion
import app.kaeri.design.tabular
import app.kaeri.engine.core.ChapterId
import app.kaeri.engine.core.Medium
import app.kaeri.engine.core.WorkId
import app.kaeri.engine.model.ChapterRow
import app.kaeri.engine.model.DownloadPick
import app.kaeri.engine.model.ExportFormat
import app.kaeri.engine.model.ExportTarget
import app.kaeri.engine.model.SeriesAction
import app.kaeri.engine.model.SeriesView
import app.kaeri.i18n.Strings
import app.kaeri.store.AirTime
import app.kaeri.store.AppClock
import app.kaeri.store.ExportJob
import app.kaeri.store.Wording
import app.kaeri.store.text
import app.kaeri.ui.AppActions
import app.kaeri.ui.Icons
import app.kaeri.ui.LocalAppStore
import app.kaeri.ui.QuietButton
import app.kaeri.ui.SettingsSheet
import app.kaeri.ui.SheetButtons
import app.kaeri.ui.SheetProblem
import app.kaeri.ui.SheetText
import app.kaeri.ui.SheetTitle
import java.time.Instant
import java.time.format.DateTimeFormatter
import kotlinx.coroutines.delay

/** The size past which an e-reader that takes files in parts gets them in parts, near 200 MB. */
internal const val SEND_PARTS_OVER: Long = 200_000_000L

/** The e-readers the sheet lists by name, in order; every other screen sits under Other e-reader. */
private val LISTED = listOf("kobo", "kindle") // i18n-skip

/** The engine's catch-all screen, the one Other e-reader is named for. */
private const val OTHER = "other" // i18n-skip

/** Words for an e-reader's file format. */
internal fun formatWords(context: Context, format: ExportFormat): String = when (format) {
    ExportFormat.FIXED_EPUB -> Strings.sendFormatFixedEpub(context)
    ExportFormat.KEPUB -> Strings.sendFormatKepub(context)
    ExportFormat.EPUB -> Strings.sendFormatEpub(context)
    ExportFormat.CBZ -> Strings.sendFormatCbz(context)
}

/** The format [this] e-reader takes for [medium]: the first it lists for it. */
internal fun ExportTarget.formatFor(medium: Medium?): ExportFormat? = when (medium) {
    Medium.NOVEL -> novelFormats.firstOrNull()
    Medium.MANGA -> mangaFormats.firstOrNull()
    else -> null
} ?: formats.firstOrNull()

/** An e-reader's line in the picker: the format and screen, and "in parts over 200 MB" where it splits pages. */
internal fun targetLine(context: Context, target: ExportTarget, medium: Medium?): String {
    val parts = mutableListOf<String>()
    target.formatFor(medium)?.let {
        parts += Strings.sendTargetLine(context, formatWords(context, it), target.widthPx.toString(), target.heightPx.toString())
    }
    if (target.split && medium != Medium.NOVEL) parts += Strings.sendPartsOver(context, Wording.bytes(context, SEND_PARTS_OVER))
    return parts.joinToString(", ") // i18n-skip
}

/** A chapter as the send words name it: its number, else the source's label. */
internal fun sendLabel(row: ChapterRow): String = row.number ?: row.label

/** "41 to 43", or the one chapter. */
private fun chaptersWords(context: Context, rows: List<ChapterRow>): String? {
    val first = rows.firstOrNull() ?: return null
    val last = rows.last()
    return if (rows.size == 1) Strings.sendChaptersOne(context, sendLabel(first))
    else Strings.sendChaptersRange(context, sendLabel(first), sendLabel(last))
}

/** Why a send failed, with the chapter it stopped at when the engine names one. */
internal fun sendFailedWords(context: Context, failed: ExportJob.Failed): String {
    val cause = failureWords(context, failed.failure)
    return failed.chapter?.let { Strings.sendFailedChapter(context, it, cause) } ?: Strings.sendFailed(context, cause)
}

/** "Preparing chapter 2 of 3", as the engine counts the chapters, else as many as were asked for. */
private fun preparingWords(context: Context, job: ExportJob.Preparing?, asked: Int): String {
    val all = maxOf(if (job != null && job.chapters > 0u) job.chapters.toInt() else asked, 1)
    val at = minOf(all, (if (job != null && job.chapters > 0u) job.chaptersDone.toInt() else 0) + 1)
    return Strings.sendPreparing(context, at, all)
}

/** "2 November": the day a send went out. */
internal fun sentDay(unix: Long): String =
    DateTimeFormatter.ofPattern(AirTime.bestPattern("dMMMM", "d MMMM"), AirTime.locale)
        .format(Instant.ofEpochSecond(unix).atZone(AppClock.zone))

/** Where the sheet is: choosing, a send under way or held for Wi-Fi, ready, or failed. */
private enum class SendPhase { Choose, Preparing, Waiting, Ready, Failed }

/**
 * The Send to e-reader sheet for [chapters] of [work] (figs 62, 63): the e-reader with the format
 * it takes, what is sent and its size, the one line that progress won't come back, and Prepare
 * and share. Then the ring and what is happening, crossfading, and the share sheet once the files
 * are ready. Closing the sheet never stops a send: a send preparing or held for Wi-Fi goes on by
 * itself, and the series page and Downloads show it. Only Cancel send stops it.
 */
@Composable
internal fun SendToEreaderSheet(work: WorkId, chapters: List<ChapterId>, actions: AppActions, onDismiss: () -> Unit) {
    val context = LocalContext.current
    val store = LocalAppStore.current
    val targets by store.export.targets.collectAsStateWithLifecycle()
    val kept by store.export.chosen.collectAsStateWithLifecycle()
    val jobs by store.export.jobs.collectAsStateWithLifecycle()
    val seriesMap by store.reading.series.collectAsStateWithLifecycle()
    val chapterMap by store.reading.chapters.collectAsStateWithLifecycle()
    val estimates by store.reading.estimates.collectAsStateWithLifecycle()
    val series = seriesMap[work]
    val medium = series?.medium
    var picked by rememberSaveable { mutableStateOf<String?>(null) }
    var job by rememberSaveable { mutableStateOf<Long?>(null) }
    var waitedForSize by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) {
        store.export.loadTargets()
        store.reading.loadSeriesInfo(work)
        if (chapterMap[work] == null) store.reading.loadChapters(work)
    }
    LaunchedEffect(chapters) {
        waitedForSize = false
        delay(150)
        store.reading.estimateDownload(work, DownloadPick.Chapters(chapters))
        delay(450)
        waitedForSize = true
    }
    DisposableEffect(work) { onDispose { store.reading.clearEstimate(work) } }
    val target = targets.firstOrNull { it.id == picked }
        ?: targets.firstOrNull { it.id == kept }
        ?: targets.firstOrNull { it.id == LISTED.first() }
        ?: targets.firstOrNull()
    val now = job?.let { jobs[it.toULong()] }
    val rows = chapterMap[work]?.volumes?.chapterRows().orEmpty()
    val sent = rows.filter { it.id in chapters }
    val bytes = estimates[work]?.takeIf { it.pick == DownloadPick.Chapters(chapters) }?.allBytes?.toLong()?.takeIf { it > 0 }
    fun start() {
        val device = target ?: return
        val format = device.formatFor(medium) ?: return
        job?.let { store.export.forget(it.toULong()) }
        job = store.export.prepare(work, chapters, device.id, format)?.toLong()
    }

    // Leaving the sheet puts away only what was seen to its end: a failure, or files already shared.
    fun dismiss() {
        when (now) {
            is ExportJob.Failed -> store.export.forget(now.job)
            is ExportJob.Ready -> if (store.export.wasShared(now.job)) store.export.forget(now.job)
            else -> {}
        }
        onDismiss()
    }
    val ready = now as? ExportJob.Ready
    fun share(files: ExportJob.Ready) = actions.shareFiles(files.files) { store.export.shared(files.job) }
    LaunchedEffect(ready?.job) { ready?.let(::share) }
    SettingsSheet(onDismiss = ::dismiss) {
        SheetTitle(Strings.sendTitle(context))
        series?.let { SheetText(it.titles.display, Modifier.padding(bottom = 8.dp)) }
        val phase = when (now) {
            null -> SendPhase.Choose
            is ExportJob.Preparing -> SendPhase.Preparing
            is ExportJob.Waiting -> SendPhase.Waiting
            is ExportJob.Ready -> SendPhase.Ready
            is ExportJob.Failed -> SendPhase.Failed
        }
        AnimatedContent(
            phase,
            transitionSpec = { fadeIn(Motion.fadeIn()) togetherWith fadeOut(Motion.fadeOut()) },
            label = "send", // i18n-skip
        ) { showing ->
            when (showing) {
                SendPhase.Choose -> Column {
                    SendHeading(Strings.sendFor(context))
                    TargetPicker(targets, target, medium) { picked = it }
                    SendHeading(Strings.sendWhat(context))
                    ValueRow(Strings.sendChapters(context), chaptersWords(context, sent))
                    ValueRow(
                        Strings.sendSize(context),
                        bytes?.let { Strings.sendSizeAbout(context, Wording.bytes(context, it)) }
                            ?: Strings.sendSizeWorking(context).takeIf { waitedForSize },
                    )
                    if (bytes != null && target?.split == true && medium != Medium.NOVEL && bytes > SEND_PARTS_OVER) {
                        SheetText(Strings.sendInParts(context, Wording.bytes(context, SEND_PARTS_OVER)), Modifier.padding(top = 8.dp))
                    }
                    SheetText(Strings.sendNoPlace(context), Modifier.padding(top = 12.dp))
                    Button(
                        onClick = ::start,
                        enabled = target?.formatFor(medium) != null && chapters.isNotEmpty(),
                        modifier = Modifier.padding(top = 20.dp).fillMaxWidth().heightIn(min = 56.dp),
                    ) { Text(Strings.sendPrepare(context), style = MaterialTheme.typography.titleMedium) }
                }
                SendPhase.Preparing -> Column {
                    val preparing = now as? ExportJob.Preparing
                    Row(
                        Modifier.fillMaxWidth().padding(vertical = 12.dp),
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        val fraction = preparing?.fraction ?: 0f
                        ChapterRing(
                            fraction,
                            Strings.sendWaitingCancel(context),
                            Strings.sendProgressSpoken(context, (fraction * 100).toInt()),
                        ) { preparing?.let { store.export.cancel(it.job) } }
                        Column(Modifier.weight(1f)) {
                            Text(preparingWords(context, preparing, chapters.size), style = MaterialTheme.typography.bodyLarge.tabular())
                            if (target != null && medium != Medium.NOVEL) {
                                SheetText(Strings.sendSizedFor(context, target.name), Modifier.padding(top = 2.dp))
                            }
                        }
                    }
                    SheetButtons { QuietButton(Strings.actionClose(context), onClick = ::dismiss) }
                }
                SendPhase.Waiting -> Column {
                    (now as? ExportJob.Waiting)?.let { WaitingLine(it.line.text(context), Modifier.padding(vertical = 12.dp)) }
                    SheetButtons {
                        QuietButton(Strings.sendWaitingCancel(context)) {
                            now?.let { store.export.cancel(it.job) }
                            job = null
                        }
                        QuietButton(Strings.actionClose(context), onClick = ::dismiss)
                    }
                }
                SendPhase.Ready -> Column {
                    val files = (now as? ExportJob.Ready)?.files.orEmpty()
                    Text(
                        Strings.sendReady(context, files.size),
                        style = MaterialTheme.typography.bodyLarge,
                        modifier = Modifier.padding(vertical = 12.dp).semantics { liveRegion = LiveRegionMode.Polite },
                    )
                    SheetButtons {
                        QuietButton(Strings.actionDone(context), onClick = ::dismiss)
                        Button(onClick = { (now as? ExportJob.Ready)?.let(::share) }) { Text(Strings.sendShareAgain(context)) }
                    }
                }
                SendPhase.Failed -> Column {
                    (now as? ExportJob.Failed)?.let { SheetProblem(sendFailedWords(context, it)) }
                    SheetButtons {
                        QuietButton(Strings.actionClose(context), onClick = ::dismiss)
                        Button(onClick = ::start) { Text(Strings.actionTryAgain(context)) }
                    }
                }
            }
        }
    }
}

@Composable
private fun SendHeading(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.titleSmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(top = 16.dp, bottom = 4.dp).semantics { heading() },
    )
}

/** A row of the What group: its name, and its value beside it once known. */
@Composable
private fun ValueRow(name: String, value: String?) {
    Row(
        Modifier.fillMaxWidth().heightIn(min = 48.dp).semantics(mergeDescendants = true) {},
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(name, style = MaterialTheme.typography.bodyLarge, modifier = Modifier.weight(1f))
        AnimatedContent(
            value,
            transitionSpec = { fadeIn(Motion.fadeIn()) togetherWith fadeOut(Motion.fadeOut()) },
            label = "send-value", // i18n-skip
        ) { words ->
            Text(words.orEmpty(), style = MaterialTheme.typography.bodyLarge.tabular(), color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

/** A send held for Wi-Fi: the Wi-Fi mark, still, and the engine's line saying why. */
@Composable
private fun WaitingLine(line: String, modifier: Modifier = Modifier, style: TextStyle = MaterialTheme.typography.bodyLarge) {
    Row(
        modifier.semantics(mergeDescendants = true) { liveRegion = LiveRegionMode.Polite },
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(Icons.Wifi, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(20.dp))
        Text(line, style = style)
    }
}

/**
 * The e-readers, as one group of choices: Kobo, Kindle, and Other e-reader, which opens a menu of
 * every other screen the engine knows and shows the one kept from it.
 */
@Composable
private fun TargetPicker(targets: List<ExportTarget>, chosen: ExportTarget?, medium: Medium?, onPick: (String) -> Unit) {
    val context = LocalContext.current
    val listed = LISTED.mapNotNull { id -> targets.firstOrNull { it.id == id } }
    val others = targets.filter { it.id !in LISTED }
    Column(Modifier.fillMaxWidth().selectableGroup()) {
        listed.forEach { device ->
            TargetRow(device.name, targetLine(context, device, medium), device.id == chosen?.id, menu = false) { onPick(device.id) }
        }
        if (others.isNotEmpty()) {
            var open by remember { mutableStateOf(false) }
            val keptOther = others.firstOrNull { it.id == chosen?.id }
            val catchAll = others.firstOrNull { it.id == OTHER } ?: others.first()
            val shown = keptOther ?: catchAll
            Box {
                TargetRow(shown.name, targetLine(context, shown, medium), keptOther != null, menu = true) { open = true }
                DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
                    others.forEach { device ->
                        DropdownMenuItem(
                            text = {
                                Column {
                                    Text(device.name, style = MaterialTheme.typography.bodyLarge)
                                    Text(
                                        targetLine(context, device, medium),
                                        style = MaterialTheme.typography.bodyMedium.tabular(),
                                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    )
                                }
                            },
                            trailingIcon = if (device.id == chosen?.id) {
                                { Icon(Icons.Check, contentDescription = null, modifier = Modifier.size(20.dp)) }
                            } else {
                                null
                            },
                            onClick = {
                                open = false
                                onPick(device.id)
                            },
                        )
                    }
                }
            }
        }
    }
}

/** An e-reader to prepare for: its name and line, chosen with a radio; [menu] marks the row that opens the rest. */
@Composable
private fun TargetRow(name: String, line: String, selected: Boolean, menu: Boolean, onClick: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 64.dp)
            .selectable(selected = selected, onClick = onClick, role = if (menu) Role.DropdownList else Role.RadioButton)
            .padding(vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Column(Modifier.weight(1f)) {
            Text(name, style = MaterialTheme.typography.bodyLarge)
            Text(line, style = MaterialTheme.typography.bodyMedium.tabular(), color = colors.onSurfaceVariant)
        }
        if (menu) Icon(Icons.Down, contentDescription = null, tint = colors.onSurfaceVariant, modifier = Modifier.size(20.dp))
        RadioButton(
            selected = selected,
            onClick = null,
            colors = RadioButtonDefaults.colors(selectedColor = colors.onSurface, unselectedColor = colors.onSurfaceVariant),
        )
    }
}

/**
 * The sends going on outside their sheet, one quiet row each: "Sending to Kobo" and the series,
 * with the ring that stops it while it is made; the engine's line and Cancel send while it waits
 * for Wi-Fi; the files ready with Share and Done; or why it failed, with Done. A send held across
 * a restart is here too, though no sheet asked for it. [work] limits it to one series; `null` is
 * every send, for Downloads.
 */
@Composable
internal fun SendStatusRows(work: WorkId?, actions: AppActions, modifier: Modifier = Modifier) {
    val store = LocalAppStore.current
    val jobs by store.export.jobs.collectAsStateWithLifecycle()
    val sends = jobs.values.filter { work == null || it.work == work }.sortedBy { it.job }
    // The last rows shown, so they fade out whole rather than empty first.
    val last = remember { mutableStateOf(sends) }
    if (sends.isNotEmpty()) last.value = sends
    AnimatedVisibility(sends.isNotEmpty(), enter = fadeIn(Motion.fadeIn()), exit = fadeOut(Motion.fadeOut())) {
        val colors = MaterialTheme.colorScheme
        Column(
            modifier
                .fillMaxWidth()
                .background(colors.surfaceContainer, RoundedCornerShape(16.dp))
                .padding(horizontal = 16.dp),
        ) {
            last.value.forEachIndexed { index, send ->
                if (index > 0) HorizontalDivider(color = colors.outlineVariant)
                SendStatusRow(send, actions)
            }
        }
    }
}

@Composable
private fun SendStatusRow(send: ExportJob, actions: AppActions) {
    val context = LocalContext.current
    val store = LocalAppStore.current
    val seriesMap by store.reading.series.collectAsStateWithLifecycle()
    val colors = MaterialTheme.colorScheme
    val device = deviceName(send.target)
    Column(Modifier.fillMaxWidth().padding(vertical = 12.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Column(Modifier.weight(1f)) {
                Text(
                    if (device.isEmpty()) Strings.sendTitle(context) else Strings.sendBanner(context, device),
                    style = MaterialTheme.typography.bodyLarge,
                    fontWeight = FontWeight.Medium,
                )
                seriesMap[send.work]?.titles?.display?.let {
                    Text(it, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
            }
            when (send) {
                is ExportJob.Preparing -> {
                    val fraction = send.fraction ?: 0f
                    ChapterRing(
                        fraction,
                        Strings.sendWaitingCancel(context),
                        Strings.sendProgressSpoken(context, (fraction * 100).toInt()),
                    ) { store.export.cancel(send.job) }
                }
                is ExportJob.Waiting -> OutlinedButton(
                    onClick = { store.export.cancel(send.job) },
                    colors = ButtonDefaults.outlinedButtonColors(contentColor = colors.onSurface),
                ) { Text(Strings.sendWaitingCancel(context)) }
                is ExportJob.Ready -> Row(verticalAlignment = Alignment.CenterVertically) {
                    QuietButton(Strings.actionDone(context)) { store.export.forget(send.job) }
                    OutlinedButton(
                        onClick = { actions.shareFiles(send.files) { store.export.shared(send.job) } },
                        colors = ButtonDefaults.outlinedButtonColors(contentColor = colors.onSurface),
                    ) { Text(Strings.actionShare(context)) }
                }
                is ExportJob.Failed -> QuietButton(Strings.actionDone(context)) { store.export.forget(send.job) }
            }
        }
        when (send) {
            is ExportJob.Waiting -> WaitingLine(send.line.text(context), Modifier.padding(top = 8.dp), MaterialTheme.typography.bodyMedium)
            is ExportJob.Failed -> Row(
                Modifier.padding(top = 8.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Icon(Icons.Problem, contentDescription = null, tint = colors.error, modifier = Modifier.size(18.dp))
                Text(sendFailedWords(context, send), style = MaterialTheme.typography.bodyMedium)
            }
            is ExportJob.Ready -> Text(
                Strings.sendReady(context, send.files.size),
                style = MaterialTheme.typography.bodyMedium,
                color = colors.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp),
            )
            is ExportJob.Preparing -> {}
        }
    }
}

/**
 * The series page's note after a send (fig 64): that chapters went to the e-reader, asking where
 * they stopped there, with Set my place.
 */
@Composable
internal fun WhereStopNote(series: SeriesView, onAsk: () -> Unit, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val sent = series.askWhereStopped ?: return
    val device = deviceName(sent.target)
    val colors = MaterialTheme.colorScheme
    Column(
        modifier
            .fillMaxWidth()
            .background(colors.surfaceContainer, RoundedCornerShape(16.dp))
            .padding(16.dp),
    ) {
        Text(Strings.stopNote(context, device), style = MaterialTheme.typography.bodyLarge)
        OutlinedButton(
            onClick = onAsk,
            modifier = Modifier.padding(top = 12.dp),
            colors = ButtonDefaults.outlinedButtonColors(contentColor = colors.onSurface),
        ) { Text(Strings.stopNoteAction(context)) }
    }
}

/** An e-reader by name where the targets know it, else its id as it was kept. */
@Composable
private fun deviceName(target: String): String {
    val store = LocalAppStore.current
    val targets by store.export.targets.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { if (targets.isEmpty()) store.export.loadTargets() }
    return targets.firstOrNull { it.id == target }?.name ?: target
}

/**
 * Where did you stop (fig 64): the chapters that went, one to choose, whether it was finished,
 * Set my place as the one amber action, and keeping the place as it is.
 */
@Composable
internal fun WhereStopSheet(series: SeriesView, onDismiss: () -> Unit) {
    val context = LocalContext.current
    val store = LocalAppStore.current
    val chapterMap by store.reading.chapters.collectAsStateWithLifecycle()
    val sent = series.askWhereStopped ?: return
    val device = deviceName(sent.target)
    val all = chapterMap[series.work]?.volumes?.chapterRows().orEmpty()
    val rows = sent.chapters.mapNotNull { id -> all.firstOrNull { it.id == id } }
    var picked by rememberSaveable { mutableStateOf(sent.chapters.lastOrNull()) }
    var finished by rememberSaveable { mutableStateOf(true) }
    SettingsSheet(onDismiss = onDismiss) {
        SheetTitle(Strings.stopTitle(context))
        val date = sentDay(sent.sentAt)
        val first = rows.firstOrNull()
        val last = rows.lastOrNull()
        if (first != null && last != null) {
            val went = if (rows.size == 1) Strings.stopSentOne(context, sendLabel(first), device, date)
            else Strings.stopSentRange(context, sendLabel(first), sendLabel(last), device, date)
            SheetText(went, Modifier.padding(bottom = 8.dp))
        }
        rows.forEach { row ->
            val selected = row.id == picked
            Row(
                Modifier
                    .fillMaxWidth()
                    .heightIn(min = 56.dp)
                    .selectable(selected = selected, onClick = { picked = row.id }, role = Role.RadioButton),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    SeriesWords.heading(context, row),
                    style = MaterialTheme.typography.bodyLarge,
                    fontWeight = if (selected) FontWeight.Medium else null,
                    modifier = Modifier.weight(1f),
                )
                RadioButton(
                    selected = selected,
                    onClick = null,
                    colors = RadioButtonDefaults.colors(
                        selectedColor = MaterialTheme.colorScheme.onSurface,
                        unselectedColor = MaterialTheme.colorScheme.onSurfaceVariant,
                    ),
                )
            }
        }
        val pickedRow = rows.firstOrNull { it.id == picked }
        if (pickedRow != null) {
            Row(
                Modifier
                    .fillMaxWidth()
                    .heightIn(min = 56.dp)
                    .toggleable(value = finished, onValueChange = { finished = it }, role = Role.Switch)
                    .padding(top = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    Strings.stopFinished(context, sendLabel(pickedRow)),
                    style = MaterialTheme.typography.bodyLarge,
                    modifier = Modifier.weight(1f),
                )
                Switch(checked = finished, onCheckedChange = null)
            }
        }
        val keep = (series.action as? SeriesAction.Continue)?.let { Strings.stopKeep(context, it.label, it.page.toInt()) }
            ?: Strings.stopKeepPlain(context)
        SheetButtons {
            QuietButton(keep) {
                store.reading.stoppedAt(series.work, null, null)
                onDismiss()
            }
            Button(
                onClick = {
                    val chapter = pickedRow ?: return@Button
                    // Read up to and including the chapter when it was finished, else up to the one before it.
                    val upTo = if (finished) chapter.id else all.indexOfFirst { it.id == chapter.id }.takeIf { it > 0 }?.let { all[it - 1].id }
                    store.reading.stoppedAt(series.work, upTo, null)
                    onDismiss()
                },
                enabled = pickedRow != null,
                modifier = Modifier.heightIn(min = 48.dp),
            ) { Text(Strings.stopSet(context)) }
        }
    }
}
