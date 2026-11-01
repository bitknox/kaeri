// The Style section under the subtitle tracks: the four things people change in the middle of an
// episode. A change reaches the video at once, because the player reads the style the settings
// hold, and it is saved to the device's style with it.

package app.kaeri.player.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExposedDropdownMenuAnchorType
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.design.ObiSilk
import app.kaeri.engine.core.SettingValue
import app.kaeri.engine.core.StyledSubtitles
import app.kaeri.engine.core.SubtitleBackground
import app.kaeri.engine.core.SubtitleEdge
import app.kaeri.engine.model.SettingKey
import app.kaeri.engine.model.SubtitleStyle
import app.kaeri.i18n.R
import app.kaeri.ui.LocalAppStore
import app.kaeri.store.SubtitleNames
import app.kaeri.ui.styledDetail
import app.kaeri.ui.styledWords

/** How the Background control reads the style: Outline, Shadow or a box behind the lines. */
internal enum class SubtitleLook { Outline, Shadow, Box }

internal fun SubtitleStyle.look(): SubtitleLook = when {
    background != SubtitleBackground.NO_BACKGROUND -> SubtitleLook.Box
    edge == SubtitleEdge.DROP_SHADOW -> SubtitleLook.Shadow
    else -> SubtitleLook.Outline
}

/** The next value in [steps] from [current], one step [up] or down, held at the ends. */
internal fun stepped(steps: List<Int>, current: Int, up: Boolean): Int {
    val at = steps.indexOfLast { it <= current }.coerceAtLeast(0)
    return steps[(if (up) at + 1 else at - 1).coerceIn(0, steps.lastIndex)]
}

/** The Style section's items, under the tracks. [more] opens Settings. */
internal fun LazyListScope.subtitleStyleSection(more: () -> Unit) {
    item(key = "style") { SubtitleStylePanel(more) }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun SubtitleStylePanel(more: () -> Unit) {
    val store = LocalAppStore.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val style = view?.subtitleStyle ?: return
    val context = LocalContext.current
    fun send(key: SettingKey, value: String) = store.setSetting(key, SettingValue.Str(value))
    fun send(key: SettingKey, value: Int) = store.setSetting(key, SettingValue.Int(value.toLong()))
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(stringResource(R.string.player_subs_style), style = MaterialTheme.typography.titleMedium, color = ObiSilk.Silk)
        Stepper(
            label = stringResource(R.string.settings_subs_size),
            value = context.getString(R.string.settings_subs_size_value, style.size.toInt()),
            lessLabel = stringResource(R.string.player_subs_smaller),
            moreLabel = stringResource(R.string.player_subs_larger),
            canLess = style.size.toInt() > SubtitleNames.SIZES.first(),
            canMore = style.size.toInt() < SubtitleNames.SIZES.last(),
            onLess = { send(SettingKey.SUBTITLE_SIZE, stepped(SubtitleNames.SIZES, style.size.toInt(), up = false)) },
            onMore = { send(SettingKey.SUBTITLE_SIZE, stepped(SubtitleNames.SIZES, style.size.toInt(), up = true)) },
        )
        Stepper(
            label = stringResource(R.string.settings_subs_raise),
            value = context.getString(R.string.settings_subs_raise_value, style.raise.toInt()),
            lessLabel = stringResource(R.string.player_subs_lower),
            moreLabel = stringResource(R.string.player_subs_raise),
            canLess = style.raise.toInt() > SubtitleNames.RAISES.first(),
            canMore = style.raise.toInt() < SubtitleNames.RAISES.last(),
            onLess = { send(SettingKey.SUBTITLE_RAISE, stepped(SubtitleNames.RAISES, style.raise.toInt(), up = false)) },
            onMore = { send(SettingKey.SUBTITLE_RAISE, stepped(SubtitleNames.RAISES, style.raise.toInt(), up = true)) },
        )
        Text(stringResource(R.string.settings_subs_background), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
        val looks = SubtitleLook.entries
        SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth()) {
            looks.forEachIndexed { index, look ->
                SegmentedButton(
                    selected = style.look() == look,
                    onClick = {
                        when (look) {
                            SubtitleLook.Outline -> {
                                send(SettingKey.SUBTITLE_EDGE, SubtitleNames.key(SubtitleEdge.OUTLINE))
                                send(SettingKey.SUBTITLE_BACKGROUND, SubtitleNames.key(SubtitleBackground.NO_BACKGROUND))
                            }
                            SubtitleLook.Shadow -> {
                                send(SettingKey.SUBTITLE_EDGE, SubtitleNames.key(SubtitleEdge.DROP_SHADOW))
                                send(SettingKey.SUBTITLE_BACKGROUND, SubtitleNames.key(SubtitleBackground.NO_BACKGROUND))
                            }
                            SubtitleLook.Box -> send(SettingKey.SUBTITLE_BACKGROUND, SubtitleNames.key(SubtitleBackground.EACH_LINE))
                        }
                    },
                    shape = SegmentedButtonDefaults.itemShape(index, looks.size),
                    colors = SegmentedButtonDefaults.colors(
                        activeContainerColor = ObiSilk.Silk.copy(alpha = 0.16f),
                        activeContentColor = ObiSilk.Silk,
                        inactiveContainerColor = androidx.compose.ui.graphics.Color.Transparent,
                        inactiveContentColor = ObiSilk.Haze,
                    ),
                ) {
                    Text(
                        stringResource(
                            when (look) {
                                SubtitleLook.Outline -> R.string.player_subs_bg_outline
                                SubtitleLook.Shadow -> R.string.player_subs_bg_shadow
                                SubtitleLook.Box -> R.string.player_subs_bg_box
                            },
                        ),
                    )
                }
            }
        }
        var open by remember { mutableStateOf(false) }
        ExposedDropdownMenuBox(expanded = open, onExpandedChange = { open = it }) {
            OutlinedTextField(
                value = styledWords(style.styled),
                onValueChange = {},
                readOnly = true,
                label = { Text(stringResource(R.string.settings_subs_styled)) },
                trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(open) },
                modifier = Modifier.menuAnchor(ExposedDropdownMenuAnchorType.PrimaryNotEditable).fillMaxWidth(),
            )
            androidx.compose.material3.DropdownMenu(open, { open = false }) {
                StyledSubtitles.entries.forEach { choice ->
                    DropdownMenuItem(
                        text = { Column { Text(styledWords(choice)); Text(styledDetail(choice), style = MaterialTheme.typography.bodySmall) } },
                        onClick = {
                            open = false
                            send(SettingKey.STYLED_SUBTITLES, SubtitleNames.key(choice))
                        },
                    )
                }
            }
        }
        TextButton(onClick = more) { Text(stringResource(R.string.player_subs_more), color = ObiSilk.Silk) }
    }
}

@Composable
private fun Stepper(
    label: String,
    value: String,
    lessLabel: String,
    moreLabel: String,
    canLess: Boolean,
    canMore: Boolean,
    onLess: () -> Unit,
    onMore: () -> Unit,
) {
    Row(Modifier.fillMaxWidth().heightIn(min = 48.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(label, style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk, modifier = Modifier.weight(1f))
        TextButton(onClick = onLess, enabled = canLess, modifier = Modifier.semantics { contentDescription = lessLabel }) { Text("−", color = ObiSilk.Silk) } // i18n-skip
        Text(value, style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk, modifier = Modifier.widthIn(min = 64.dp, max = 140.dp), textAlign = androidx.compose.ui.text.style.TextAlign.Center)
        TextButton(onClick = onMore, enabled = canMore, modifier = Modifier.semantics { contentDescription = moreLabel }) { Text("+", color = ObiSilk.Silk) } // i18n-skip
    }
}
