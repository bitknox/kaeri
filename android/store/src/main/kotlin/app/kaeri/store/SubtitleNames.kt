// The names the settings file gives each subtitle choice, which the engine's settings write takes,
// and the steps of the sliders. Shared by the phone, the player's quick panel and the TV.

package app.kaeri.store

import app.kaeri.engine.core.StyledSubtitles
import app.kaeri.engine.core.SubtitleBackground
import app.kaeri.engine.core.SubtitleColour
import app.kaeri.engine.core.SubtitleEdge
import app.kaeri.engine.core.SubtitleFont
import app.kaeri.engine.core.SubtitleThickness

/** The names the settings file gives each choice, which the engine's settings write takes. */
object SubtitleNames {
    fun key(font: SubtitleFont) = font.name.lowercase().replace('_', '-')
    fun key(colour: SubtitleColour) = colour.name.lowercase()
    fun key(edge: SubtitleEdge) = when (edge) {
        SubtitleEdge.OUTLINE -> "outline"
        SubtitleEdge.DROP_SHADOW -> "drop-shadow"
        SubtitleEdge.NO_EDGE -> "none"
    }
    fun key(thickness: SubtitleThickness) = thickness.name.lowercase()
    fun key(background: SubtitleBackground) = when (background) {
        SubtitleBackground.NO_BACKGROUND -> "none"
        SubtitleBackground.EACH_LINE -> "each-line"
        SubtitleBackground.ALL_LINES -> "all-lines"
    }
    fun key(styled: StyledSubtitles) = styled.name.lowercase()

    val SIZES = (50..200 step 10).toList()
    val OPACITIES = listOf(50, 75, 100)
    val RAISES = (0..20 step 2).toList()
}

