// The subtitle style on the TV: the rows in Settings, Playback, and the Style rows in the
// player's Subtitles panel. Each row shows its value; Left and Right change it in the panel, and
// Select opens its choices in Settings. The engine's style is the only state.

package app.kaeri.tv.screens

import android.content.Context
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import app.kaeri.engine.core.SettingValue
import app.kaeri.engine.core.StyledSubtitles
import app.kaeri.engine.core.SubtitleBackground
import app.kaeri.engine.core.SubtitleColour
import app.kaeri.engine.core.SubtitleEdge
import app.kaeri.engine.core.SubtitleFont
import app.kaeri.engine.core.SubtitleThickness
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.SettingKey
import app.kaeri.engine.model.SubtitleStyle
import app.kaeri.i18n.R
import app.kaeri.store.AppStore
import app.kaeri.store.SubtitleNames
import app.kaeri.tv.ui.ActionSheet
import app.kaeri.tv.ui.CardAction
import app.kaeri.tv.ui.LocalActions
import app.kaeri.tv.ui.LocalAppStore
import app.kaeri.tv.ui.TvListRow
import androidx.compose.ui.unit.dp

/** One subtitle setting as a row: its label, the words of its value, and the values it can take. */
internal class SubtitleSpec(
    val id: String,
    val label: String,
    val choices: List<Pair<SettingValue, String>>,
    val at: Int,
    val key: SettingKey,
    val note: String? = null,
    val enabled: Boolean = true,
) {
    val words: String get() = choices.getOrNull(at)?.second.orEmpty()

    /** The value one step [forward] or back from this one, held at the ends. */
    fun step(forward: Boolean): SettingValue? = choices.getOrNull((at + if (forward) 1 else -1).coerceIn(0, choices.lastIndex))?.first
}

private fun str(name: String) = SettingValue.Str(name)
private fun int(n: Int) = SettingValue.Int(n.toLong())

private fun font(context: Context, f: SubtitleFont) = when (f) {
    SubtitleFont.DEFAULT -> context.getString(R.string.settings_subs_font_default)
    SubtitleFont.ZEN_KAKU_GOTHIC_NEW -> "Zen Kaku Gothic New" // i18n-skip
    SubtitleFont.SERIF -> context.getString(R.string.settings_subs_font_serif)
    SubtitleFont.MONOSPACE -> context.getString(R.string.settings_subs_font_mono)
}

private fun colour(context: Context, c: SubtitleColour) = context.getString(
    when (c) {
        SubtitleColour.WHITE -> R.string.settings_subs_colour_white
        SubtitleColour.YELLOW -> R.string.settings_subs_colour_yellow
        SubtitleColour.CYAN -> R.string.settings_subs_colour_cyan
        SubtitleColour.GREEN -> R.string.settings_subs_colour_green
    },
)

private fun edge(context: Context, e: SubtitleEdge) = context.getString(
    when (e) {
        SubtitleEdge.OUTLINE -> R.string.settings_subs_edge_outline
        SubtitleEdge.DROP_SHADOW -> R.string.settings_subs_edge_shadow
        SubtitleEdge.NO_EDGE -> R.string.settings_subs_edge_none
    },
)

private fun thickness(context: Context, t: SubtitleThickness) = context.getString(
    when (t) {
        SubtitleThickness.THIN -> R.string.settings_subs_thin
        SubtitleThickness.MEDIUM -> R.string.settings_subs_medium
        SubtitleThickness.THICK -> R.string.settings_subs_thick
    },
)

private fun background(context: Context, b: SubtitleBackground) = context.getString(
    when (b) {
        SubtitleBackground.NO_BACKGROUND -> R.string.settings_subs_background_none
        SubtitleBackground.EACH_LINE -> R.string.settings_subs_background_line
        SubtitleBackground.ALL_LINES -> R.string.settings_subs_background_all
    },
)

private fun styled(context: Context, s: StyledSubtitles) = context.getString(
    when (s) {
        StyledSubtitles.KEEP -> R.string.settings_subs_styled_keep
        StyledSubtitles.RESTYLE -> R.string.settings_subs_styled_restyle
        StyledSubtitles.STRIP -> R.string.settings_subs_styled_strip
    },
)

private fun styledDetail(context: Context, s: StyledSubtitles) = context.getString(
    when (s) {
        StyledSubtitles.KEEP -> R.string.settings_subs_styled_keep_detail
        StyledSubtitles.RESTYLE -> R.string.settings_subs_styled_restyle_detail
        StyledSubtitles.STRIP -> R.string.settings_subs_styled_strip_detail
    },
)

private fun percent(context: Context, n: Int) = context.getString(R.string.settings_subs_size_value, n)

private fun size(context: Context, style: SubtitleStyle) = SubtitleSpec(
    "size", context.getString(R.string.settings_subs_size),
    SubtitleNames.SIZES.map { int(it) to percent(context, it) }, SubtitleNames.SIZES.indexOfLast { it <= style.size.toInt() }.coerceAtLeast(0), SettingKey.SUBTITLE_SIZE,
)

private fun raise(context: Context, style: SubtitleStyle) = SubtitleSpec(
    "raise", context.getString(R.string.settings_subs_raise),
    SubtitleNames.RAISES.map { int(it) to context.getString(R.string.settings_subs_raise_value, it) },
    SubtitleNames.RAISES.indexOfLast { it <= style.raise.toInt() }.coerceAtLeast(0), SettingKey.SUBTITLE_RAISE,
)

private fun backgroundSpec(context: Context, style: SubtitleStyle) = SubtitleSpec(
    "background", context.getString(R.string.settings_subs_background),
    SubtitleBackground.entries.map { str(SubtitleNames.key(it)) to background(context, it) }, style.background.ordinal, SettingKey.SUBTITLE_BACKGROUND,
    enabled = !style.followSystem,
)

private fun styledSpec(context: Context, style: SubtitleStyle) = SubtitleSpec(
    "styled", context.getString(R.string.settings_subs_styled),
    StyledSubtitles.entries.map { str(SubtitleNames.key(it)) to styled(context, it) }, style.styled.ordinal, SettingKey.STYLED_SUBTITLES,
    note = styledDetail(context, style.styled),
)

/** The four things people change in the middle of an episode. */
internal fun quickSpecs(context: Context, style: SubtitleStyle) =
    listOf(size(context, style), raise(context, style), backgroundSpec(context, style), styledSpec(context, style))

/** Every row of Settings, Playback, Subtitles, after Follow system captions. */
internal fun settingsSpecs(context: Context, style: SubtitleStyle): List<SubtitleSpec> = buildList {
    val own = !style.followSystem
    add(size(context, style))
    add(SubtitleSpec("font", context.getString(R.string.settings_subs_font), SubtitleFont.entries.map { str(SubtitleNames.key(it)) to font(context, it) }, style.font.ordinal, SettingKey.SUBTITLE_FONT, enabled = own))
    add(SubtitleSpec("colour", context.getString(R.string.settings_subs_colour), SubtitleColour.entries.map { str(SubtitleNames.key(it)) to colour(context, it) }, style.colour.ordinal, SettingKey.SUBTITLE_COLOUR, enabled = own))
    add(SubtitleSpec("edge", context.getString(R.string.settings_subs_edge), SubtitleEdge.entries.map { str(SubtitleNames.key(it)) to edge(context, it) }, style.edge.ordinal, SettingKey.SUBTITLE_EDGE, enabled = own))
    if (style.edge != SubtitleEdge.NO_EDGE) {
        add(SubtitleSpec("thickness", context.getString(R.string.settings_subs_thickness), SubtitleThickness.entries.map { str(SubtitleNames.key(it)) to thickness(context, it) }, style.thickness.ordinal, SettingKey.SUBTITLE_THICKNESS, enabled = own))
    }
    add(backgroundSpec(context, style))
    if (style.background != SubtitleBackground.NO_BACKGROUND) {
        add(SubtitleSpec("opacity", context.getString(R.string.settings_subs_opacity), SubtitleNames.OPACITIES.map { int(it) to percent(context, it) }, SubtitleNames.OPACITIES.indexOfLast { it <= style.opacity.toInt() }.coerceAtLeast(0), SettingKey.SUBTITLE_OPACITY))
    }
    add(raise(context, style))
    add(styledSpec(context, style))
}

/** A row for [spec]: Left and Right step the value, Select opens the choices when [panel] is false. */
@Composable
internal fun SubtitleRow(spec: SubtitleSpec, store: AppStore, panel: Boolean, modifier: Modifier = Modifier) {
    val actions = LocalActions.current
    TvListRow(
        spec.label,
        onClick = {
            if (panel) {
                spec.step(forward = spec.at < spec.choices.lastIndex)?.let { store.setSetting(spec.key, it) }
            } else {
                actions?.open(
                    ActionSheet(spec.label, spec.choices.mapIndexed { index, (value, words) -> CardAction(words, selected = index == spec.at) { store.setSetting(spec.key, value) } }),
                )
            }
        },
        supporting = spec.note,
        value = spec.words,
        enabled = spec.enabled,
        modifier = modifier.onPreviewKeyEvent { event ->
            val left = event.key == Key.DirectionLeft
            val right = event.key == Key.DirectionRight
            if (panel && (left || right)) {
                if (event.type == KeyEventType.KeyDown) spec.step(forward = right)?.let { store.setSetting(spec.key, it) }
                true
            } else {
                false
            }
        },
    )
}

/** Settings, Playback, Subtitles: Follow system captions, every row, and Reset. */
internal fun LazyListScope.subtitleSettingsRows(store: AppStore, style: SubtitleStyle?) {
    if (style == null) return
    item(key = "subs/follow", contentType = "setting") {
        TvListRow(
            stringResource(R.string.settings_system_captions),
            onClick = { store.setSetting(SettingKey.FOLLOW_SYSTEM_CAPTIONS, SettingValue.Bool(!style.followSystem)) },
            supporting = stringResource(R.string.settings_system_captions_detail),
            switch = style.followSystem,
        )
    }
    item(key = "subs/rows", contentType = "setting") {
        val context = LocalContext.current
        androidx.compose.foundation.layout.Column(verticalArrangement = androidx.compose.foundation.layout.Arrangement.spacedBy(4.dp)) {
            settingsSpecs(context, style).forEach { SubtitleRow(it, store, panel = false) }
            TvListRow(stringResource(R.string.settings_subs_reset), onClick = { store.send(Command.ResetSubtitleStyle) })
        }
    }
}

/** The player's Style rows, under the tracks. */
internal fun LazyListScope.subtitleStyleRows(store: AppStore, style: SubtitleStyle?) {
    if (style == null) return
    item(key = "style/rows") {
        val context = LocalContext.current
        androidx.compose.foundation.layout.Column(verticalArrangement = androidx.compose.foundation.layout.Arrangement.spacedBy(4.dp)) {
            quickSpecs(context, style).forEach { SubtitleRow(it, store, panel = true) }
        }
    }
}
