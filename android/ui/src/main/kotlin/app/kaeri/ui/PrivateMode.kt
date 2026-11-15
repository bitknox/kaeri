// Private watching and reading on the phone, as the private mode board draws it: the switch under
// the account card in Settings, the tonal chip before the avatar while it is on, the line under a
// title or series page's main action, the More menu's Always private and the screen reader's
// words as the mode turns on and off.

package app.kaeri.ui

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.tween
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.semantics.toggleableState
import androidx.compose.ui.state.ToggleableState
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.design.Motion
import app.kaeri.design.PrivateGlyph
import app.kaeri.engine.model.PrivateTitle
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.store.PrivacyModel
import app.kaeri.store.PrivateLine

/** The line [title]'s page shows now, following the mode and the title's own standing. */
@Composable
internal fun privateLine(title: PrivateTitle): PrivateLine? {
    val privacy = LocalAppStore.current.privacy
    val on by privacy.on.collectAsStateWithLifecycle()
    val always by privacy.always.collectAsStateWithLifecycle()
    val kept by privacy.kept.collectAsStateWithLifecycle()
    return PrivacyModel.line(on, title in always, title in kept)
}

/** "Private · Not saved or synced", its head in silk and the rest haze. */
@Composable
internal fun privateLineWords(line: PrivateLine): AnnotatedString {
    val head = stringResource(if (line == PrivateLine.Always) R.string.title_always_private else R.string.private_badge)
    val tail = stringResource(if (line == PrivateLine.Kept) R.string.private_line_kept else R.string.private_line_not_saved)
    val silk = MaterialTheme.colorScheme.onSurface
    return buildAnnotatedString {
        withStyle(SpanStyle(color = silk, fontWeight = FontWeight.Medium)) { append(head) }
        append(" · ") // i18n-skip
        append(tail)
    }
}

/**
 * The line under a title or series page's main action, 10 below it: the eye-slash and the words.
 * It grows on standard as it comes, its words fading in from 0.1 s, and goes the same way; the
 * main button, the list status and the episodes stay as they are.
 */
@Composable
internal fun PrivatePageLine(line: PrivateLine?, modifier: Modifier = Modifier, top: Dp = 10.dp) {
    // The words stay through the line's going, so it never empties as it shrinks.
    val shown = remember { mutableStateOf(line) }
    if (line != null) shown.value = line
    AnimatedVisibility(
        visible = line != null,
        modifier = modifier,
        enter = expandVertically(Motion.standard(), expandFrom = Alignment.Top) + fadeIn(tween(200, delayMillis = 100)),
        exit = shrinkVertically(Motion.standard(), shrinkTowards = Alignment.Top) + fadeOut(Motion.fadeOut()),
    ) {
        val said = shown.value ?: return@AnimatedVisibility
        Row(
            Modifier.padding(top = top).semantics(mergeDescendants = true) {},
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(PrivateGlyph.EyeSlash, contentDescription = null, tint = MaterialTheme.colorScheme.onSurface, modifier = Modifier.size(18.dp))
            Text(
                privateLineWords(said),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

/**
 * The badge while private mode is on: a tonal chip before the avatar, 32 high at corners 8, the
 * eye-slash and Private. It fades into its place as the mode turns on and out as it turns off,
 * and opens Settings at the switch.
 */
@Composable
internal fun PrivateChip() {
    val store = LocalAppStore.current
    val navigator = LocalNavigatorOrNull.current
    val on by store.privacy.on.collectAsStateWithLifecycle()
    val label = stringResource(R.string.private_badge_label)
    AnimatedVisibility(
        visible = on,
        enter = fadeIn(Motion.fadeIn()),
        exit = fadeOut(Motion.fadeOut()),
    ) {
        FilledTonalButton(
            onClick = { navigator?.openSettings() },
            shape = RoundedCornerShape(8.dp),
            contentPadding = PaddingValues(start = 8.dp, end = 12.dp),
            modifier = Modifier
                .padding(end = 4.dp)
                .heightIn(min = 32.dp)
                .semantics(mergeDescendants = true) { contentDescription = label },
        ) {
            Icon(PrivateGlyph.EyeSlash, contentDescription = null, modifier = Modifier.size(18.dp))
            Text(stringResource(R.string.private_badge), style = MaterialTheme.typography.labelLarge, modifier = Modifier.padding(start = 6.dp))
        }
    }
}

/**
 * The switch under the account card: the eye-slash, Private mode and what it does, the switch at
 * the end, and under it what it does not. Neutral in silk rather than amber: it is a state, not
 * the page's one action.
 */
@Composable
internal fun PrivateModeSwitch(modifier: Modifier = Modifier) {
    Column(modifier) {
        PrivateModeRow(Place.Only)
        SettingsFootnote(stringResource(R.string.private_mode_limits), Modifier.padding(start = 40.dp))
    }
}

/**
 * The switch's row alone at [place]. On paper it is one row in the list's run, what it does not do
 * under its words rather than as a note below.
 */
@Composable
internal fun PrivateModeRow(place: Place) {
    val store = LocalAppStore.current
    val on by store.privacy.on.collectAsStateWithLifecycle()
    val onWord = stringResource(R.string.value_on)
    val offWord = stringResource(R.string.value_off)
    RowFrame(
        place,
        onClick = { store.setPrivate(!on) },
        modifier = Modifier
            .heightIn(min = 72.dp)
            .semantics(mergeDescendants = true) {
                role = Role.Switch
                stateDescription = if (on) onWord else offWord
            },
    ) { selected ->
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            if (!einkSettings) {
                Icon(PrivateGlyph.EyeSlash, contentDescription = null, tint = MaterialTheme.colorScheme.onSurface, modifier = Modifier.size(24.dp))
            }
            RowText(
                stringResource(R.string.private_mode),
                null,
                stringResource(R.string.private_mode_detail),
                selected,
                Modifier.weight(1f),
                extra = if (einkSettings) stringResource(R.string.private_mode_limits) else null,
            )
            PrivateSwitchMark(on)
        }
    }
}

/** An M3 switch filled with silk when on, haze on a ring when off: private mode's and Always private's. */
@Composable
internal fun PrivateSwitchMark(checked: Boolean) {
    if (einkSettings) return SettingSwitchMark(checked)
    val colors = MaterialTheme.colorScheme
    Switch(
        checked = checked,
        onCheckedChange = null,
        colors = SwitchDefaults.colors(
            checkedTrackColor = colors.onSurface,
            checkedThumbColor = colors.surface,
            checkedBorderColor = Color.Transparent,
            uncheckedTrackColor = colors.surfaceContainerHighest,
            uncheckedThumbColor = colors.onSurfaceVariant,
            uncheckedBorderColor = colors.onSurfaceVariant,
        ),
    )
}

/**
 * Always private as a check item in a title's or series' More menu: a check in the icon's slot
 * while on, the slot kept empty while off. The menu closes with [onDone].
 */
@Composable
internal fun AlwaysPrivateItem(title: PrivateTitle, onDone: () -> Unit) {
    val store = LocalAppStore.current
    val always by store.privacy.always.collectAsStateWithLifecycle()
    val on = title in always
    DropdownMenuItem(
        text = { Text(stringResource(R.string.title_always_private)) },
        leadingIcon = {
            Box(Modifier.size(24.dp), contentAlignment = Alignment.Center) {
                if (on) Icon(Icons.Check, contentDescription = null)
            }
        },
        modifier = Modifier.semantics {
            role = Role.Checkbox
            toggleableState = ToggleableState(on)
        },
        onClick = {
            onDone()
            store.setAlwaysPrivate(title, !on)
        },
    )
}

/**
 * Always private as a switch in a title's own settings: the eye-slash, its name and [detail], the
 * switch at the end. What was saved before stays.
 */
@Composable
internal fun AlwaysPrivateRow(title: PrivateTitle, detail: String, modifier: Modifier = Modifier) {
    val store = LocalAppStore.current
    val always by store.privacy.always.collectAsStateWithLifecycle()
    val on = title in always
    val onWord = stringResource(R.string.value_on)
    val offWord = stringResource(R.string.value_off)
    Row(
        modifier
            .heightIn(min = 72.dp)
            .clickable(role = Role.Switch) { store.setAlwaysPrivate(title, !on) }
            .semantics(mergeDescendants = true) { stateDescription = if (on) onWord else offWord }
            .padding(horizontal = 16.dp, vertical = 12.dp),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(PrivateGlyph.EyeSlash, contentDescription = null, modifier = Modifier.size(24.dp))
        Column(Modifier.weight(1f)) {
            Text(stringResource(R.string.title_always_private), style = MaterialTheme.typography.bodyLarge)
            Text(detail, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        PrivateSwitchMark(on)
    }
}

/**
 * Says "Private mode on" or "Private mode off" to a screen reader once as the mode changes, from
 * whatever turned it: the switch, or the badge's way to it. Nothing is said at launch.
 */
@Composable
internal fun PrivateModeAnnouncer() {
    val store = LocalAppStore.current
    val view = LocalView.current
    val context = LocalContext.current
    val on by store.privacy.on.collectAsStateWithLifecycle()
    val first = remember { mutableStateOf(true) }
    LaunchedEffect(on) {
        if (first.value) {
            first.value = false
            return@LaunchedEffect
        }
        val words = if (on) Strings.privateModeOnAnnouncement(context) else Strings.privateModeOffAnnouncement(context)
        @Suppress("DEPRECATION")
        view.announceForAccessibility(words)
    }
}
