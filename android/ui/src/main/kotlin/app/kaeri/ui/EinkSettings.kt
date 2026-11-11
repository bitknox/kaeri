// Settings on an e-ink panel, as the large panel board draws it: words on paper with 1 dp hairlines
// between rows, the selected group filled light grey and bold, each value in its row with Change,
// Off and On as a segmented control in place of a switch, and no icons, cards or amber. Where the
// window takes two panes (7 inch and up) the groups sit on the left and the pane on the right,
// with a hairline between them; a 6 inch panel keeps one page at a time. The rows Settings is
// built from (SettingsRows) draw these when the e-ink look is on.

package app.kaeri.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.requiredWidth
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.wrapContentHeight
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.BorderStroke
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.DpOffset
import androidx.compose.ui.unit.dp
import app.kaeri.eink.EinkColours
import app.kaeri.eink.EinkSegmented
import app.kaeri.eink.EinkSizes
import app.kaeri.eink.EinkTextButton
import app.kaeri.eink.EinkType
import app.kaeri.eink.einkClick
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.ui.reading.LocalEinkLayout
import app.kaeri.ui.reading.Side

/** Whether Settings draws on paper: the e-ink look is on. */
internal val einkSettings: Boolean
    @Composable @ReadOnlyComposable
    get() = LocalEinkLayout.current != null

/** A group's name on paper: 15 sp bold in #333, 22 dp over it. */
private val GroupName get() = EinkType.Secondary.copy(fontWeight = FontWeight.Bold)

/** The left pane's width at 10.3 inch, and on a 7 inch panel, where the right pane needs the room. */
private val ListWide = 340.dp
private val ListNarrow = 260.dp

/** The space on each side of the hairline between the panes. */
private val PaneGap = 32.dp
private val PaneGapNarrow = 20.dp

/** A 1 dp hairline over a row that is not the first of its group, as the board draws the lists. */
private fun Modifier.hairlineOver(first: Boolean): Modifier = if (first) this else drawBehind {
    val y = EinkSizes.Hairline.toPx() / 2
    drawLine(EinkColours.Tertiary, Offset(0f, y), Offset(size.width, y), EinkSizes.Hairline.toPx())
}

private val Place.first: Boolean get() = this == Place.First || this == Place.Only

/** A group on paper: its name, its rows with hairlines between, and its note under them. */
@Composable
internal fun EinkSettingsGroup(title: String?, footer: String?, modifier: Modifier, rows: List<@Composable (Place) -> Unit>) {
    Column(modifier) {
        title?.let { EinkSettingsHeader(it) }
        rows.forEachIndexed { index, row -> row(Place.of(index, rows.size)) }
        footer?.let { EinkSettingsFootnote(it) }
    }
}

@Composable
internal fun EinkSettingsHeader(title: String, modifier: Modifier = Modifier) {
    Text(title, style = GroupName, modifier = modifier.padding(top = 22.dp, bottom = 4.dp).semantics { heading() })
}

@Composable
internal fun EinkSettingsFootnote(text: String, modifier: Modifier = Modifier) {
    Text(text, style = EinkType.Secondary, modifier = modifier.padding(top = 8.dp))
}

/** A row's frame on paper: no fill, a hairline over it after the first, at least [height] tall. */
@Composable
internal fun EinkRowFrame(
    place: Place,
    onClick: (() -> Unit)?,
    modifier: Modifier = Modifier,
    height: Dp = 62.dp,
    content: @Composable () -> Unit,
) {
    Box(
        modifier
            .fillMaxWidth()
            .heightIn(min = height)
            .hairlineOver(place.first)
            .then(if (onClick != null) Modifier.einkClick(onClick = onClick) else Modifier)
            .padding(vertical = 10.dp),
        contentAlignment = Alignment.CenterStart,
    ) { content() }
}

/** A row's words on paper: its name, then the line under it in #333. */
@Composable
internal fun EinkRowWords(title: String, line: String?, note: String? = null, modifier: Modifier = Modifier, extra: String? = null) {
    Column(modifier) {
        Text(title, style = EinkType.Label)
        line?.let { Text(it, style = EinkType.Secondary) }
        note?.let { Text(it, style = EinkType.Secondary) }
        extra?.let { Text(it, style = EinkType.Label, modifier = Modifier.padding(top = 4.dp)) }
    }
}

/**
 * A setting chosen from a list, on paper: its name small over its value, Change at the end, and
 * the choices in a menu edged in ink with the current one filled grey and bold.
 */
@Composable
internal fun <T> EinkMenuRow(
    place: Place,
    title: String,
    value: String?,
    choices: List<MenuChoice<T>>,
    current: T?,
    note: String?,
    enabled: Boolean,
    menuWidth: Int,
    onPick: (T) -> Unit,
) {
    var open by remember { mutableStateOf(false) }
    val usable = enabled && choices.isNotEmpty()
    Box(Modifier.alpha(if (enabled) 1f else 0.5f)) {
        EinkRowFrame(place, onClick = if (usable) ({ open = true }) else null) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text(title, style = EinkType.Secondary)
                    value?.let { Text(it, style = EinkType.Label) }
                    note?.let { Text(it, style = EinkType.Secondary, modifier = Modifier.padding(top = 2.dp)) }
                }
                if (usable) EinkTextButton(stringResource(R.string.series_change), { open = true }, underline = true, modifier = Modifier.padding(start = 12.dp))
            }
        }
        DropdownMenu(
            expanded = open,
            onDismissRequest = { open = false },
            offset = DpOffset(0.dp, (-8).dp),
            shape = RoundedCornerShape(EinkSizes.SegmentRadius),
            containerColor = EinkColours.Paper,
            tonalElevation = 0.dp,
            shadowElevation = 0.dp,
            border = BorderStroke(EinkSizes.Rule, EinkColours.Ink),
            modifier = Modifier.widthIn(min = menuWidth.dp),
        ) {
            val selectedWord = stringResource(R.string.value_selected)
            choices.forEachIndexed { index, choice ->
                val picked = choice.value == current
                Column(
                    Modifier
                        .fillMaxWidth()
                        .heightIn(min = EinkSizes.SegmentHeight + 4.dp)
                        .background(if (picked) EinkColours.Selected else EinkColours.Paper)
                        .hairlineOver(index == 0)
                        .semantics { if (picked) stateDescription = selectedWord }
                        .einkClick {
                            open = false
                            onPick(choice.value)
                        }
                        .padding(horizontal = 16.dp, vertical = 10.dp),
                    verticalArrangement = Arrangement.Center,
                ) {
                    Text(choice.label, style = EinkType.Label.copy(fontWeight = if (picked) FontWeight.Bold else FontWeight.Medium))
                    choice.example?.let { Text(it, style = EinkType.Secondary, maxLines = 1, overflow = TextOverflow.Ellipsis) }
                }
            }
        }
    }
}

/** A setting that is on or off, on paper: its name and note, and Off and On as a segmented control. */
@Composable
internal fun EinkSwitchRow(place: Place, title: String, checked: Boolean, note: String?, enabled: Boolean, extra: String?, onChange: (Boolean) -> Unit) {
    val context = LocalContext.current
    val onWord = stringResource(R.string.value_on)
    val offWord = stringResource(R.string.value_off)
    EinkRowFrame(
        place,
        onClick = null,
        height = 74.dp,
        modifier = Modifier.alpha(if (enabled) 1f else 0.5f).semantics(mergeDescendants = false) {
            role = Role.Switch
            stateDescription = if (checked) onWord else offWord
        },
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            EinkRowWords(title, null, note, Modifier.weight(1f).padding(end = 16.dp), extra = extra)
            EinkSegmented(
                choices = listOf(false, true),
                chosen = checked,
                label = { if (it) Strings.valueOn(context) else Strings.valueOff(context) },
                onChoose = { if (enabled && it != checked) onChange(it) },
                modifier = Modifier.width(150.dp),
            )
        }
    }
}

/** A group in the list on the left: its name and what it says of itself, grey and bold when open. */
@Composable
internal fun EinkLinkRow(place: Place, title: String, value: String?, selected: Boolean, onClick: () -> Unit) {
    Column(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 74.dp)
            .background(if (selected) EinkColours.Selected else EinkColours.Paper)
            .hairlineOver(place.first)
            .einkClick(onClick = onClick)
            .semantics { this.selected = selected }
            .padding(horizontal = 14.dp, vertical = 12.dp),
        verticalArrangement = Arrangement.Center,
    ) {
        Text(title, style = EinkType.Label.copy(fontWeight = if (selected) FontWeight.Bold else FontWeight.SemiBold), maxLines = 1, overflow = TextOverflow.Ellipsis)
        value?.let { Text(it, style = EinkType.Secondary, maxLines = 1, overflow = TextOverflow.Ellipsis) }
    }
}

/** A row that says something, on paper, with its own action at the end when it has one. */
@Composable
internal fun EinkInfoRow(place: Place, title: String, value: String?, note: String?, onClick: (() -> Unit)?, trailing: (@Composable () -> Unit)?, modifier: Modifier) {
    EinkRowFrame(place, onClick, modifier) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            EinkRowWords(title, value, note, Modifier.weight(1f))
            trailing?.invoke()
        }
    }
}

/**
 * Settings in two panes on paper: "‹ More" and the title over the groups on the left, a hairline,
 * and the open group's rows on the right under its name. Picking a group swaps the right pane at
 * once; nothing fades.
 */
@Composable
internal fun EinkTabletSettings(pane: SettingsPane, onPane: (SettingsPane) -> Unit, actions: AppActions) {
    val navigator = LocalNavigator.current
    val context = LocalContext.current
    val large = LocalConfiguration.current.screenWidthDp >= 900
    val listWidth = if (large) ListWide else ListNarrow
    val gap = if (large) PaneGap else PaneGapNarrow
    Box(
        Modifier
            .fillMaxSize()
            .background(EinkColours.Paper)
            .windowInsetsPadding(WindowInsets.safeDrawing)
            .padding(horizontal = Side),
    ) {
        Column(Modifier.fillMaxSize().padding(top = 6.dp, bottom = 16.dp)) {
            Text(
                "‹ " + Strings.einkTabMore(context), // i18n-skip
                style = EinkType.Label,
                modifier = Modifier.heightIn(min = 48.dp).einkClick { navigator.back() }.wrapContentHeight(Alignment.CenterVertically),
            )
            // The two titles share a line; the hairline between the panes starts there.
            Row(Modifier.fillMaxWidth().weight(1f).padding(top = 2.dp)) {
                Column(Modifier.width(listWidth).fillMaxHeight()) {
                    Text(stringResource(R.string.settings_title), style = EinkType.PageTitle, modifier = Modifier.semantics { heading() })
                    Spacer(Modifier.height(16.dp))
                    // The grey of the open group reaches 14 dp past the words on each side.
                    Box(Modifier.fillMaxWidth().weight(1f)) {
                        Column(Modifier.requiredWidth(listWidth + 28.dp).fillMaxHeight().verticalScroll(rememberScrollState()).padding(bottom = 24.dp)) {
                            SettingsList(selected = pane) { onPane(it) }
                        }
                    }
                }
                Spacer(Modifier.width(gap))
                Box(Modifier.width(EinkSizes.Hairline).fillMaxHeight().background(EinkColours.Tertiary))
                Spacer(Modifier.width(gap))
                Column(Modifier.weight(1f).fillMaxHeight()) {
                    Text(stringResource(pane.titleRes), style = EinkType.PageTitle, modifier = Modifier.semantics { heading() })
                    app.kaeri.ui.reading.BookSourcesRefresh(pane == SettingsPane.Sources) {
                        Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(bottom = 24.dp)) {
                            PaneContent(pane, actions)
                        }
                    }
                }
            }
        }
    }
}

/** A page of Settings on a 6 inch panel: "‹ Back", the title, then the list or the pane, on paper. */
@Composable
internal fun EinkPhoneSettings(title: String, content: @Composable () -> Unit) {
    val navigator = LocalNavigator.current
    val context = LocalContext.current
    Column(
        Modifier
            .fillMaxSize()
            .background(EinkColours.Paper)
            .windowInsetsPadding(WindowInsets.safeDrawing)
            .padding(horizontal = Side),
    ) {
        Text(
            "‹ " + Strings.einkBack(context), // i18n-skip
            style = EinkType.Label,
            modifier = Modifier.heightIn(min = 48.dp).einkClick { navigator.back() }.padding(top = 14.dp),
        )
        Text(title, style = EinkType.PageTitle, modifier = Modifier.padding(top = 8.dp, bottom = 8.dp).semantics { heading() })
        app.kaeri.ui.reading.BookSourcesRefresh(false) {
            Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(bottom = 24.dp)) { content() }
        }
    }
}

/** The mark at the end of a settings row (a chevron, a way out of the app); on paper the words say it, so it is left out. */
@Composable
internal fun RowMark(icon: androidx.compose.ui.graphics.vector.ImageVector) {
    if (!einkSettings) androidx.compose.material3.Icon(icon, contentDescription = null, tint = androidx.compose.material3.MaterialTheme.colorScheme.onSurfaceVariant)
}
