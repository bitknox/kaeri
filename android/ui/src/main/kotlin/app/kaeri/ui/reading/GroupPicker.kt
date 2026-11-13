// The group picker: whose version of each chapter a series reads, inside the source in use. A
// modal bottom sheet of radio rows on a phone, an M3 menu from Change on a tablet, and a page of
// its own with large bordered choices on e-ink. Most chapters comes first and names the group it
// means today; then each group with its chapters out of the series' and its language. A pick
// closes it.

package app.kaeri.ui.reading

import android.content.Context
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.systemBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.RadioButtonDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.onClick
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.compose.ui.window.DialogWindowProvider
import app.kaeri.design.ObiSilk
import app.kaeri.eink.EinkColours
import app.kaeri.eink.EinkLook
import app.kaeri.eink.EinkSizes
import app.kaeri.eink.EinkTheme
import app.kaeri.eink.EinkType
import app.kaeri.eink.einkClick
import app.kaeri.engine.core.WorkId
import app.kaeri.engine.model.BookEditionRow
import app.kaeri.engine.model.GroupChoice
import app.kaeri.i18n.Strings
import app.kaeri.store.AirTime
import app.kaeri.ui.Icons
import app.kaeri.ui.LocalAppStore
import app.kaeri.ui.SettingsSheet
import app.kaeri.ui.SheetText
import app.kaeri.ui.SheetTitle

/** What the picker says, from the edition the engine sent. */
internal object GroupWords {
    /**
     * What Most chapters picks today: the group in use while nothing is picked, else the first in
     * the app's language, else the one with the most chapters.
     */
    fun mostNow(edition: BookEditionRow): String? {
        if (!edition.groupChosen) return edition.groups.firstOrNull { it.inUse }?.label ?: edition.versionLabel
        val app = AirTime.locale.language.lowercase()
        val own = edition.groups.firstOrNull { group -> group.language?.let(SeriesWords::primary) == app }
        return (own ?: edition.groups.maxByOrNull { it.chapters })?.label
    }

    /** The series' chapter count, which each group's is out of. */
    fun total(edition: BookEditionRow): Int = maxOf(edition.chapters.toInt(), edition.groups.maxOfOrNull { it.chapters.toInt() } ?: 0)

    /** The group in use: "Lanternlight Scans", for the Group row. */
    fun inUse(edition: BookEditionRow): String = edition.groups.firstOrNull { it.inUse }?.label ?: edition.versionLabel.orEmpty()

    /** "Lanternlight Scans · most chapters" or "· your pick", as the name and why apart. */
    fun why(context: Context, edition: BookEditionRow): String =
        if (edition.groupChosen) Strings.seriesGroupPicked(context) else Strings.seriesGroupMost(context)

    /** Whether the choice is the one in use: Most chapters with null. */
    fun checked(edition: BookEditionRow, group: GroupChoice?): Boolean =
        if (group == null) !edition.groupChosen else edition.groupChosen && group.inUse

    /** The line under a choice: "Now Lanternlight Scans", or "84 of 88 chapters · English". */
    fun line(context: Context, edition: BookEditionRow, group: GroupChoice?): String? {
        if (group == null) return mostNow(edition)?.let { Strings.groupPickerMostNow(context, it) }
        return listOfNotNull(
            Strings.groupPickerCount(context, group.chapters.toInt(), total(edition)),
            group.language?.let(SeriesWords::language),
        ).joinToString(" · ") // i18n-skip
    }

    /** A choice as TalkBack reads it, with commas for the dots: "Saltmarsh Team, 12 of 18 chapters, English". */
    fun spoken(context: Context, edition: BookEditionRow, group: GroupChoice?): String {
        if (group == null) return listOfNotNull(Strings.groupPickerMost(context), mostNow(edition)?.let { Strings.groupPickerMostNow(context, it) }).joinToString(", ") // i18n-skip
        return listOfNotNull(
            group.label,
            Strings.groupPickerCount(context, group.chapters.toInt(), total(edition)),
            group.language?.let(SeriesWords::language),
        ).joinToString(", ") // i18n-skip
    }

    /** Most chapters, then each group. */
    fun choices(edition: BookEditionRow): List<GroupChoice?> = listOf<GroupChoice?>(null) + edition.groups
}

/** Send the pick unless it is the one in use already, then close. */
@Composable
private fun rememberPick(work: WorkId, edition: BookEditionRow, onDismiss: () -> Unit): (GroupChoice?) -> Unit {
    val store = LocalAppStore.current
    return { group ->
        if (!GroupWords.checked(edition, group)) store.reading.chooseGroup(work, group?.label)
        onDismiss()
    }
}

/** On a phone: a modal bottom sheet titled Group with the source under it, radio rows, and the fallback note. */
@Composable
internal fun GroupSheet(work: WorkId, edition: BookEditionRow, onDismiss: () -> Unit) {
    val context = LocalContext.current
    val pick = rememberPick(work, edition, onDismiss)
    SettingsSheet(onDismiss = onDismiss) {
        SheetTitle(Strings.seriesGroup(context))
        SheetText(Strings.groupPickerIn(context, edition.sourceName), Modifier.padding(bottom = 8.dp))
        Column(Modifier.selectableGroup()) {
            GroupWords.choices(edition).forEachIndexed { index, group ->
                if (index == 1) HorizontalDivider(color = ObiSilk.FaintRule)
                val on = GroupWords.checked(edition, group)
                val spoken = GroupWords.spoken(context, edition, group)
                Row(
                    Modifier
                        .fillMaxWidth()
                        .heightIn(min = 64.dp)
                        .selectable(selected = on, role = Role.RadioButton, onClick = { pick(group) })
                        .clearAndSetSemantics { contentDescription = spoken; selected = on; role = Role.RadioButton; onClick { pick(group); true } }
                        .padding(vertical = 8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(16.dp),
                ) {
                    RadioButton(
                        selected = on,
                        onClick = null,
                        colors = RadioButtonDefaults.colors(selectedColor = MaterialTheme.colorScheme.onSurface, unselectedColor = MaterialTheme.colorScheme.onSurfaceVariant),
                    )
                    Column(Modifier.weight(1f)) {
                        Text(group?.label ?: Strings.groupPickerMost(context), style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurface)
                        GroupWords.line(context, edition, group)?.let {
                            Text(it, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                    }
                }
            }
        }
        SheetText(Strings.groupPickerFallback(context), Modifier.padding(top = 12.dp))
    }
}

/**
 * On a tablet: the menu's rows, from Change. A line of source heads it, the group in use sits on
 * the selected fill with a check, and the fallback note ends it.
 */
@Composable
internal fun ColumnScope.GroupMenuRows(work: WorkId, edition: BookEditionRow, onDismiss: () -> Unit) {
    val context = LocalContext.current
    val colors = MaterialTheme.colorScheme
    val pick = rememberPick(work, edition, onDismiss)
    Text(
        Strings.groupMenuIn(context, edition.sourceName),
        style = MaterialTheme.typography.bodySmall,
        color = colors.onSurfaceVariant,
        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp).semantics { heading() },
    )
    Column(Modifier.width(300.dp).selectableGroup()) {
        GroupWords.choices(edition).forEachIndexed { index, group ->
            if (index == 1) HorizontalDivider(color = colors.outlineVariant, modifier = Modifier.padding(vertical = 4.dp))
            val on = GroupWords.checked(edition, group)
            val spoken = GroupWords.spoken(context, edition, group)
            Row(
                Modifier
                    .fillMaxWidth()
                    .heightIn(min = 56.dp)
                    .background(if (on) ObiSilk.RaisedSelected else Color.Transparent)
                    .selectable(selected = on, role = Role.RadioButton, onClick = { pick(group) })
                    .clearAndSetSemantics { contentDescription = spoken; selected = on; role = Role.RadioButton; onClick { pick(group); true } }
                    .padding(horizontal = 12.dp, vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Box(Modifier.width(40.dp), contentAlignment = Alignment.CenterStart) {
                    if (on) Icon(Icons.Check, contentDescription = null, tint = colors.onSurface, modifier = Modifier.size(20.dp))
                }
                Column(Modifier.weight(1f)) {
                    Text(group?.label ?: Strings.groupPickerMost(context), style = MaterialTheme.typography.bodyLarge, color = colors.onSurface)
                    GroupWords.line(context, edition, group)?.let {
                        Text(it, style = MaterialTheme.typography.bodySmall, color = colors.onSurfaceVariant)
                    }
                }
            }
        }
    }
    Text(
        Strings.groupPickerFallback(context),
        style = MaterialTheme.typography.bodySmall,
        color = colors.onSurfaceVariant,
        modifier = Modifier.width(300.dp).padding(horizontal = 16.dp, vertical = 10.dp),
    )
}

/**
 * On e-ink: a page of its own over the series, the series' name as the way back, large bordered
 * radio choices with the one in use filled grey, and the fallback note. A tap picks and goes
 * back. Nothing moves: the page is there in one refresh, and gone in one.
 */
@Composable
internal fun EinkGroupPage(work: WorkId, title: String, edition: BookEditionRow, onDismiss: () -> Unit) {
    val context = LocalContext.current
    val pick = rememberPick(work, edition, onDismiss)
    Dialog(onDismissRequest = onDismiss, properties = DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false)) {
        // One refresh each way: no window animation, no dim, and paper under the system bars.
        val view = LocalView.current
        DisposableEffect(view) {
            generateSequence(view.parent) { it.parent }.filterIsInstance<DialogWindowProvider>().firstOrNull()?.window?.apply {
                setWindowAnimations(0)
                setDimAmount(0f)
            }
            onDispose {}
        }
        EinkTheme(LocalEinkLayout.current ?: EinkLook()) {
            Column(
                Modifier
                    .fillMaxSize()
                    .background(EinkColours.Paper)
                    .windowInsetsPadding(WindowInsets.systemBars)
                    .verticalScroll(rememberScrollState())
                    .padding(horizontal = 28.dp),
            ) {
                Text(
                    "‹ $title", // i18n-skip
                    style = EinkType.Label,
                    modifier = Modifier.heightIn(min = 48.dp).einkClick(onClick = onDismiss).padding(top = 14.dp),
                )
                Text(Strings.seriesGroup(context), style = EinkType.PageTitle, modifier = Modifier.padding(top = 8.dp).semantics { heading() })
                Text(Strings.groupPickerIn(context, edition.sourceName), style = EinkType.Secondary)
                Spacer(Modifier.height(16.dp))
                Column(Modifier.selectableGroup(), verticalArrangement = Arrangement.spacedBy(14.dp)) {
                    for (group in GroupWords.choices(edition)) {
                        val on = GroupWords.checked(edition, group)
                        val spoken = GroupWords.spoken(context, edition, group)
                        val shape = RoundedCornerShape(10.dp)
                        Row(
                            Modifier
                                .fillMaxWidth()
                                .heightIn(min = 72.dp)
                                .background(if (on) EinkColours.Selected else EinkColours.Paper, shape)
                                .border(EinkSizes.Rule, EinkColours.Ink, shape)
                                .einkClick(role = Role.RadioButton, shape = shape, inset = EinkSizes.FocusInsideBorder) { pick(group) }
                                .clearAndSetSemantics { contentDescription = spoken; selected = on; role = Role.RadioButton; onClick { pick(group); true } }
                                .padding(horizontal = 18.dp, vertical = 10.dp),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(16.dp),
                        ) {
                            EinkRadio(on)
                            Column(Modifier.weight(1f)) {
                                Text(group?.label ?: Strings.groupPickerMost(context), style = EinkType.RowTitle)
                                GroupWords.line(context, edition, group)?.let { Text(it, style = EinkType.Secondary) }
                            }
                        }
                    }
                }
                Text(Strings.groupPickerFallback(context), style = EinkType.Secondary, modifier = Modifier.padding(top = 16.dp, bottom = 24.dp))
            }
        }
    }
}

/** A 26 dp ring in ink, with a dot in it when chosen. */
@Composable
private fun EinkRadio(on: Boolean) {
    Box(Modifier.size(26.dp).border(2.dp, EinkColours.Ink, CircleShape), contentAlignment = Alignment.Center) {
        if (on) Box(Modifier.size(12.dp).background(EinkColours.Ink, CircleShape))
    }
}
