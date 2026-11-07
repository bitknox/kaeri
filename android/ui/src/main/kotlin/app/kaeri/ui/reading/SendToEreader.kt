// Send to e-reader: the sheet that prepares chapters for an e-reader and hands them to the share
// sheet, from the series page and from Downloads (figs 62, 63); and Where did you stop, which the
// series page asks after a send, since the e-reader keeps its place to itself (fig 64).

package app.kaeri.ui.reading

import android.content.Context
import androidx.compose.animation.AnimatedContent
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
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.RadioButton
import androidx.compose.material3.RadioButtonDefaults
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.design.Motion
import app.kaeri.design.tabular
import app.kaeri.engine.core.ChapterId
import app.kaeri.engine.core.Medium
import app.kaeri.engine.core.WorkId
import app.kaeri.engine.model.ChapterRow
import app.kaeri.engine.model.ExportFormat
import app.kaeri.engine.model.ExportTarget
import app.kaeri.engine.model.SeriesAction
import app.kaeri.engine.model.SeriesView
import app.kaeri.i18n.Strings
import app.kaeri.store.AirTime
import app.kaeri.store.AppClock
import app.kaeri.store.ExportJob
import app.kaeri.store.text
import app.kaeri.ui.AppActions
import app.kaeri.ui.LocalAppStore
import app.kaeri.ui.QuietButton
import app.kaeri.ui.SettingsSheet
import app.kaeri.ui.SheetButtons
import app.kaeri.ui.SheetProblem
import app.kaeri.ui.SheetText
import app.kaeri.ui.SheetTitle
import java.time.Instant
import java.time.format.DateTimeFormatter

/** Words for an e-reader's file format. */
internal fun formatWords(context: Context, format: ExportFormat): String = when (format) {
    ExportFormat.FIXED_EPUB -> Strings.sendEreaderFormatFixedEpub(context)
    ExportFormat.KEPUB -> Strings.sendEreaderFormatKepub(context)
    ExportFormat.EPUB -> Strings.sendEreaderFormatEpub(context)
    ExportFormat.CBZ -> Strings.sendEreaderFormatCbz(context)
}

/** The format [this] e-reader takes for [medium]: the first it lists for it. */
internal fun ExportTarget.formatFor(medium: Medium?): ExportFormat? = when (medium) {
    Medium.NOVEL -> novelFormats.firstOrNull()
    Medium.MANGA -> mangaFormats.firstOrNull()
    else -> null
} ?: formats.firstOrNull()

/** "2 November": the day a send went out. */
internal fun sentDay(unix: Long): String =
    DateTimeFormatter.ofPattern(AirTime.bestPattern("dMMMM", "d MMMM"), AirTime.locale)
        .format(Instant.ofEpochSecond(unix).atZone(AppClock.zone))

/** Where the sheet is: choosing, or the job's own state once it is asked for. */
private enum class SendPhase { Choose, Working, Failed }

/**
 * The Send to e-reader sheet for [chapters] of [work] (figs 62, 63): the e-reader with the format
 * it takes, what is sent, the one line that progress won't come back, and Prepare and share. Then
 * the ring and what is happening, crossfading, and the share sheet once the files are ready.
 */
@Composable
internal fun SendToEreaderSheet(work: WorkId, chapters: List<ChapterId>, actions: AppActions, onDismiss: () -> Unit) {
    val context = LocalContext.current
    val store = LocalAppStore.current
    val targets by store.export.targets.collectAsStateWithLifecycle()
    val jobs by store.export.jobs.collectAsStateWithLifecycle()
    val seriesMap by store.reading.series.collectAsStateWithLifecycle()
    val chapterMap by store.reading.chapters.collectAsStateWithLifecycle()
    val series = seriesMap[work]
    var target by rememberSaveable { mutableStateOf<String?>(null) }
    var job by rememberSaveable { mutableStateOf<Long?>(null) }
    LaunchedEffect(Unit) {
        store.export.loadTargets()
        store.reading.loadSeriesInfo(work)
        if (chapterMap[work] == null) store.reading.loadChapters(work)
    }
    val chosen = targets.firstOrNull { it.id == target } ?: targets.firstOrNull()
    val now = job?.let { jobs[it.toULong()] }
    val rows = chapterMap[work]?.volumes?.chapterRows().orEmpty()
    val sent = chapters.mapNotNull { id -> rows.firstOrNull { it.id == id } }
    fun close() {
        when (val held = job?.toULong()?.let { jobs[it] }) {
            null, is ExportJob.Ready -> {}
            is ExportJob.Failed -> store.export.forget(held.job)
            else -> store.export.cancel(held.job)
        }
        onDismiss()
    }
    val ready = now as? ExportJob.Ready
    LaunchedEffect(ready?.job) {
        val done = ready ?: return@LaunchedEffect
        actions.shareFiles(done.files) {
            store.export.shared(done.job)
            onDismiss()
        }
    }
    SettingsSheet(onDismiss = ::close) {
        SheetTitle(Strings.sendEreaderTitle(context))
        series?.let { SheetText(it.titles.display, Modifier.padding(bottom = 8.dp)) }
        val phase = when {
            job == null -> SendPhase.Choose
            now is ExportJob.Failed -> SendPhase.Failed
            else -> SendPhase.Working
        }
        AnimatedContent(
            phase,
            transitionSpec = { fadeIn(Motion.fadeIn()) togetherWith fadeOut(Motion.fadeOut()) },
            label = "send", // i18n-skip
        ) { showing ->
            when (showing) {
                SendPhase.Choose -> Column {
                    SendHeading(Strings.sendEreaderFor(context))
                    targets.forEach { device ->
                        TargetRow(device, series?.medium, device.id == chosen?.id) { target = device.id }
                    }
                    SendHeading(Strings.sendEreaderWhat(context))
                    val (what, span) = whatWords(context, sent, chapters.size)
                    Row(Modifier.fillMaxWidth().heightIn(min = 48.dp), verticalAlignment = Alignment.CenterVertically) {
                        Text(what, style = MaterialTheme.typography.bodyLarge, modifier = Modifier.weight(1f))
                        if (span != null) {
                            Text(span, style = MaterialTheme.typography.bodyLarge.tabular(), color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                    }
                    SheetText(Strings.sendEreaderCantSee(context), Modifier.padding(top = 12.dp))
                    Button(
                        onClick = {
                            val device = chosen ?: return@Button
                            val format = device.formatFor(series?.medium) ?: return@Button
                            job = store.export.prepare(work, chapters, device.id, format)?.toLong()
                        },
                        enabled = chosen != null && chapters.isNotEmpty(),
                        modifier = Modifier.padding(top = 20.dp).fillMaxWidth().heightIn(min = 56.dp),
                    ) { Text(Strings.sendEreaderPrepare(context), style = MaterialTheme.typography.titleMedium) }
                }
                SendPhase.Failed -> Column {
                    val failed = now as? ExportJob.Failed
                    SheetProblem(Strings.sendEreaderFailed(context))
                    if (failed != null) SheetText(failureWords(context, failed.failure), Modifier.padding(top = 8.dp))
                    SheetButtons {
                        QuietButton(Strings.actionCancel(context), onClick = ::close)
                        Button(onClick = {
                            failed?.let { store.export.forget(it.job) }
                            job = null
                        }) { Text(Strings.actionTryAgain(context)) }
                    }
                }
                SendPhase.Working -> Column {
                    Row(
                        Modifier.fillMaxWidth().padding(vertical = 16.dp),
                        horizontalArrangement = Arrangement.spacedBy(16.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        val preparing = now as? ExportJob.Preparing
                        Box(Modifier.size(40.dp), contentAlignment = Alignment.Center) {
                            if (preparing != null && preparing.total > 0u) {
                                CircularProgressIndicator(
                                    progress = { (preparing.done.toFloat() / preparing.total.toFloat()).coerceIn(0.02f, 1f) },
                                    modifier = Modifier.size(36.dp),
                                    strokeWidth = 3.dp,
                                    color = MaterialTheme.colorScheme.onSurface,
                                    trackColor = MaterialTheme.colorScheme.outlineVariant,
                                )
                            } else {
                                CircularProgressIndicator(
                                    Modifier.size(36.dp),
                                    strokeWidth = 3.dp,
                                    color = MaterialTheme.colorScheme.onSurface,
                                    trackColor = MaterialTheme.colorScheme.outlineVariant,
                                )
                            }
                        }
                        Column(Modifier.weight(1f)) {
                            val total = preparing?.total?.toInt() ?: chapters.size
                            // The held state's own words, else what is being made, crossfading as they change.
                            AnimatedContent(
                                (now as? ExportJob.Waiting)?.line?.text(context)
                                    ?: Strings.sendEreaderPreparing(context, preparing?.done?.toInt() ?: 0, total),
                                transitionSpec = { fadeIn(Motion.fadeIn()) togetherWith fadeOut(Motion.fadeOut()) },
                                label = "send-line", // i18n-skip
                            ) { words -> Text(words, style = MaterialTheme.typography.bodyLarge.tabular()) }
                            chosen?.let { SheetText(Strings.sendEreaderSized(context, it.name), Modifier.padding(top = 2.dp)) }
                        }
                    }
                    SheetButtons { QuietButton(Strings.actionCancel(context), onClick = ::close) }
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

/** An e-reader to prepare for: its name, the format it takes and its screen, chosen with a radio. */
@Composable
private fun TargetRow(device: ExportTarget, medium: Medium?, selected: Boolean, onClick: () -> Unit) {
    val context = LocalContext.current
    val colors = MaterialTheme.colorScheme
    val line = listOfNotNull(
        device.formatFor(medium)?.let { formatWords(context, it) },
        Strings.sendEreaderScreen(context, device.widthPx.toInt(), device.heightPx.toInt()),
    ).joinToString(" · ") // i18n-skip
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 64.dp)
            .selectable(selected = selected, onClick = onClick, role = Role.RadioButton)
            .padding(vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(device.name, style = MaterialTheme.typography.bodyLarge)
            Text(line, style = MaterialTheme.typography.bodyMedium.tabular(), color = colors.onSurfaceVariant)
        }
        RadioButton(
            selected = selected,
            onClick = null,
            colors = RadioButtonDefaults.colors(selectedColor = colors.onSurface, unselectedColor = colors.onSurfaceVariant),
        )
    }
}

/**
 * What is sent, as a row's two sides: "Chapters" and "41 to 43" by the chapters' numbers, or one
 * chapter by its label and title with nothing beside it.
 */
private fun whatWords(context: Context, rows: List<ChapterRow>, count: Int): Pair<String, String?> {
    if (rows.size == 1) return SeriesWords.heading(context, rows.first()) to null
    val first = rows.firstOrNull()
    val last = rows.lastOrNull()
    val span = if (first != null && last != null) {
        Strings.sendEreaderRange(context, first.number ?: SeriesWords.name(context, first), last.number ?: SeriesWords.name(context, last))
    } else {
        null
    }
    return Strings.seriesChosen(context, count) to span
}

/**
 * The series page's note after a send (fig 64): where the chapters went and when, that Kaeri
 * can't see the place there, and Where did you stop.
 */
@Composable
internal fun WhereStopNote(series: SeriesView, onAsk: () -> Unit, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val sent = series.askWhereStopped ?: return
    val device = sentDevice(sent.target)
    val colors = MaterialTheme.colorScheme
    Column(
        modifier
            .fillMaxWidth()
            .background(colors.surfaceContainer, RoundedCornerShape(16.dp))
            .padding(16.dp),
    ) {
        Text(Strings.sendEreaderSent(context, device, sentDay(sent.sentAt)), style = MaterialTheme.typography.bodyLarge)
        Text(Strings.sendEreaderCantSeeThere(context), style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant)
        OutlinedButton(
            onClick = onAsk,
            modifier = Modifier.padding(top = 12.dp),
            colors = ButtonDefaults.outlinedButtonColors(contentColor = colors.onSurface),
        ) { Text(Strings.sendEreaderWhereStop(context)) }
    }
}

/** The e-reader a send went to, by name where the targets know it. */
@Composable
private fun sentDevice(target: String): String {
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
    val device = sentDevice(sent.target)
    val all = chapterMap[series.work]?.volumes?.chapterRows().orEmpty()
    val rows = sent.chapters.mapNotNull { id -> all.firstOrNull { it.id == id } }
    var picked by rememberSaveable { mutableStateOf(sent.chapters.lastOrNull()) }
    var finished by rememberSaveable { mutableStateOf(true) }
    SettingsSheet(onDismiss = onDismiss) {
        SheetTitle(Strings.sendEreaderWhereStop(context))
        SheetText(Strings.sendEreaderWent(context, sent.chapters.size, device, sentDay(sent.sentAt)), Modifier.padding(bottom = 8.dp))
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
                    Strings.sendEreaderFinished(context, SeriesWords.name(context, pickedRow)),
                    style = MaterialTheme.typography.bodyLarge,
                    modifier = Modifier.weight(1f),
                )
                Switch(checked = finished, onCheckedChange = null)
            }
        }
        val keep = (series.action as? SeriesAction.Continue)?.let { Strings.sendEreaderKeepChapter(context, it.label, it.page.toInt()) }
            ?: Strings.sendEreaderKeep(context)
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
            ) { Text(Strings.sendEreaderSetPlace(context)) }
        }
    }
}
