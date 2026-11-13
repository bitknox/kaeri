// Settings, Playback, Subtitles: the style the engine keeps for this device, row by row, with a
// preview above. The engine owns the values and what they draw (mpv_subtitle_options); these rows
// only name the choices and send them.

package app.kaeri.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shadow
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.engine.core.SettingValue
import app.kaeri.engine.core.StyledSubtitles
import app.kaeri.engine.core.SubtitleBackground
import app.kaeri.engine.core.SubtitleColour
import app.kaeri.engine.core.SubtitleEdge
import app.kaeri.engine.core.SubtitleFont
import app.kaeri.engine.core.SubtitleThickness
import app.kaeri.engine.model.SettingKey
import app.kaeri.engine.model.SubtitleStyle
import app.kaeri.i18n.R
import app.kaeri.store.SubtitleNames

/** Whether the style's own look is what the player draws, so the rows beside it are dimmed. */
private fun SubtitleStyle.system() = followSystem

@Composable
private fun SubtitleStyle.fontWords(font: SubtitleFont) = when (font) {
    SubtitleFont.DEFAULT -> stringResource(R.string.settings_subs_font_default)
    SubtitleFont.ZEN_KAKU_GOTHIC_NEW -> "Zen Kaku Gothic New" // i18n-skip
    SubtitleFont.SERIF -> stringResource(R.string.settings_subs_font_serif)
    SubtitleFont.MONOSPACE -> stringResource(R.string.settings_subs_font_mono)
}

@Composable
internal fun colourWords(colour: SubtitleColour) = stringResource(
    when (colour) {
        SubtitleColour.WHITE -> R.string.settings_subs_colour_white
        SubtitleColour.YELLOW -> R.string.settings_subs_colour_yellow
        SubtitleColour.CYAN -> R.string.settings_subs_colour_cyan
        SubtitleColour.GREEN -> R.string.settings_subs_colour_green
    },
)

@Composable
internal fun edgeWords(edge: SubtitleEdge) = stringResource(
    when (edge) {
        SubtitleEdge.OUTLINE -> R.string.settings_subs_edge_outline
        SubtitleEdge.DROP_SHADOW -> R.string.settings_subs_edge_shadow
        SubtitleEdge.NO_EDGE -> R.string.settings_subs_edge_none
    },
)

@Composable
internal fun thicknessWords(thickness: SubtitleThickness) = stringResource(
    when (thickness) {
        SubtitleThickness.THIN -> R.string.settings_subs_thin
        SubtitleThickness.MEDIUM -> R.string.settings_subs_medium
        SubtitleThickness.THICK -> R.string.settings_subs_thick
    },
)

@Composable
internal fun backgroundWords(background: SubtitleBackground) = stringResource(
    when (background) {
        SubtitleBackground.NO_BACKGROUND -> R.string.settings_subs_background_none
        SubtitleBackground.EACH_LINE -> R.string.settings_subs_background_line
        SubtitleBackground.ALL_LINES -> R.string.settings_subs_background_all
    },
)

@Composable
fun styledWords(styled: StyledSubtitles) = stringResource(
    when (styled) {
        StyledSubtitles.KEEP -> R.string.settings_subs_styled_keep
        StyledSubtitles.RESTYLE -> R.string.settings_subs_styled_restyle
        StyledSubtitles.STRIP -> R.string.settings_subs_styled_strip
    },
)

@Composable
fun styledDetail(styled: StyledSubtitles) = stringResource(
    when (styled) {
        StyledSubtitles.KEEP -> R.string.settings_subs_styled_keep_detail
        StyledSubtitles.RESTYLE -> R.string.settings_subs_styled_restyle_detail
        StyledSubtitles.STRIP -> R.string.settings_subs_styled_strip_detail
    },
)

/** Settings, Playback, Subtitles. [onReset] says the reset was done, with its Undo. */
@Composable
internal fun SubtitlesGroup(onReset: (before: SubtitleStyle) -> Unit) {
    val store = LocalAppStore.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val style = view?.subtitleStyle ?: return
    val context = LocalContext.current
    val resources = LocalResources.current
    fun send(key: SettingKey, value: String) = store.setSetting(key, SettingValue.Str(value))
    fun send(key: SettingKey, value: Int) = store.setSetting(key, SettingValue.Int(value.toLong()))
    val rows = buildList<@Composable (Place) -> Unit> {
        add { place ->
            SwitchRow(place, stringResource(R.string.settings_system_captions), style.followSystem, note = stringResource(R.string.settings_system_captions_detail)) {
                store.setSetting(SettingKey.FOLLOW_SYSTEM_CAPTIONS, SettingValue.Bool(it))
            }
        }
        add { place ->
            MenuRow(
                place, stringResource(R.string.settings_subs_size), resources.getString(R.string.settings_subs_size_value, style.size.toInt()),
                SubtitleNames.SIZES.map { MenuChoice(it, resources.getString(R.string.settings_subs_size_value, it)) }, style.size.toInt(),
            ) { send(SettingKey.SUBTITLE_SIZE, it) }
        }
        add { place ->
            MenuRow(
                place, stringResource(R.string.settings_subs_font), style.fontWords(style.font),
                SubtitleFont.entries.map { MenuChoice(it, style.fontWords(it)) }, style.font,
                note = if (style.system()) stringResource(R.string.settings_subs_system_value) else null,
                enabled = !style.system(),
            ) { send(SettingKey.SUBTITLE_FONT, SubtitleNames.key(it)) }
        }
        add { place ->
            MenuRow(
                place, stringResource(R.string.settings_subs_colour), colourWords(style.colour),
                SubtitleColour.entries.map { MenuChoice(it, colourWords(it)) }, style.colour,
                enabled = !style.system(),
            ) { send(SettingKey.SUBTITLE_COLOUR, SubtitleNames.key(it)) }
        }
        add { place ->
            MenuRow(
                place, stringResource(R.string.settings_subs_edge), edgeWords(style.edge),
                SubtitleEdge.entries.map { MenuChoice(it, edgeWords(it)) }, style.edge,
                enabled = !style.system(),
            ) { send(SettingKey.SUBTITLE_EDGE, SubtitleNames.key(it)) }
        }
        if (style.edge != SubtitleEdge.NO_EDGE) add { place ->
            MenuRow(
                place, stringResource(R.string.settings_subs_thickness), thicknessWords(style.thickness),
                SubtitleThickness.entries.map { MenuChoice(it, thicknessWords(it)) }, style.thickness,
                enabled = !style.system(),
            ) { send(SettingKey.SUBTITLE_THICKNESS, SubtitleNames.key(it)) }
        }
        add { place ->
            MenuRow(
                place, stringResource(R.string.settings_subs_background), backgroundWords(style.background),
                SubtitleBackground.entries.map { MenuChoice(it, backgroundWords(it)) }, style.background,
                enabled = !style.system(),
            ) { send(SettingKey.SUBTITLE_BACKGROUND, SubtitleNames.key(it)) }
        }
        if (style.background != SubtitleBackground.NO_BACKGROUND) add { place ->
            MenuRow(
                place, stringResource(R.string.settings_subs_opacity), resources.getString(R.string.settings_subs_size_value, style.opacity.toInt()),
                SubtitleNames.OPACITIES.map { MenuChoice(it, resources.getString(R.string.settings_subs_size_value, it)) }, style.opacity.toInt(),
            ) { send(SettingKey.SUBTITLE_OPACITY, it) }
        }
        add { place ->
            MenuRow(
                place, stringResource(R.string.settings_subs_raise), resources.getString(R.string.settings_subs_raise_value, style.raise.toInt()),
                SubtitleNames.RAISES.map { MenuChoice(it, resources.getString(R.string.settings_subs_raise_value, it)) }, style.raise.toInt(),
            ) { send(SettingKey.SUBTITLE_RAISE, it) }
        }
        add { place ->
            MenuRow(
                place, stringResource(R.string.settings_subs_styled), styledWords(style.styled),
                StyledSubtitles.entries.map { MenuChoice(it, styledWords(it), example = styledDetail(it)) }, style.styled,
                note = styledDetail(style.styled), menuWidth = 320,
            ) { send(SettingKey.STYLED_SUBTITLES, SubtitleNames.key(it)) }
        }
        add { place ->
            LinkRow(place, stringResource(R.string.settings_subs_reset), null) {
                onReset(style)
                store.send(app.kaeri.engine.model.Command.ResetSubtitleStyle)
            }
        }
    }
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        SettingsHeader(stringResource(R.string.settings_subs_preview))
        SubtitlePreview(style, Modifier.padding(horizontal = 16.dp))
        SettingsGroup(rows = rows)
    }
}

/** What the engine's style looks like on the two halves of the frame: Plain, or Styled with a sign. */
@Composable
internal fun SubtitlePreview(style: SubtitleStyle, modifier: Modifier = Modifier) {
    var styled by rememberSaveable { mutableStateOf(false) }
    Column(modifier, verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            FilterChip(!styled, { styled = false }, { Text(stringResource(R.string.settings_subs_preview_plain)) })
            FilterChip(styled, { styled = true }, { Text(stringResource(R.string.settings_subs_preview_styled)) })
        }
        Box(Modifier.fillMaxWidth().aspectRatio(16f / 9f).clip(RoundedCornerShape(12.dp))) {
            Row(Modifier.fillMaxSize()) {
                Box(Modifier.weight(1f).fillMaxSize().background(Color(0xFFA9CBEA)))
                Box(Modifier.weight(1f).fillMaxSize().background(Color(0xFF111A33)))
            }
            if (styled) {
                Text(
                    stringResource(R.string.settings_subs_preview_sign),
                    Modifier.align(Alignment.TopCenter).padding(top = 12.dp),
                    style = TextStyle(color = Color(0xFFFFD166), fontSize = 14.sp, fontFamily = FontFamily.Serif),
                )
            }
            Column(Modifier.align(Alignment.BottomCenter).padding(bottom = 12.dp + (style.raise.toInt() * 2).dp), horizontalAlignment = Alignment.CenterHorizontally) {
                val lines = listOf(R.string.settings_subs_preview_line, R.string.settings_subs_preview_line_2)
                for (line in lines) Text(stringResource(line), style = previewText(style), textAlign = TextAlign.Center)
            }
        }
    }
}

/** An approximation of the engine's look for the preview: the font, colour, size, edge and plate. */
@Composable
private fun previewText(style: SubtitleStyle): TextStyle {
    val colour = when (style.colour) {
        SubtitleColour.WHITE -> Color.White
        SubtitleColour.YELLOW -> Color(0xFFFFEB3B)
        SubtitleColour.CYAN -> Color(0xFF00E5FF)
        SubtitleColour.GREEN -> Color(0xFF69F0AE)
    }
    val shadow = when (style.edge) {
        SubtitleEdge.NO_EDGE -> null
        SubtitleEdge.OUTLINE -> Shadow(Color.Black, blurRadius = 1f + style.thickness.ordinal * 2f)
        SubtitleEdge.DROP_SHADOW -> Shadow(Color.Black, androidx.compose.ui.geometry.Offset(2f + style.thickness.ordinal * 2f, 2f + style.thickness.ordinal * 2f), 2f)
    }
    val plate = when (style.background) {
        SubtitleBackground.NO_BACKGROUND -> Color.Transparent
        else -> Color.Black.copy(alpha = style.opacity.toInt() / 100f)
    }
    return TextStyle(
        color = colour,
        fontSize = (13f * style.size.toInt() / 100f).sp,
        fontFamily = when (style.font) {
            SubtitleFont.SERIF -> FontFamily.Serif
            SubtitleFont.MONOSPACE -> FontFamily.Monospace
            else -> FontFamily.SansSerif
        },
        shadow = shadow,
        background = plate,
    )
}
