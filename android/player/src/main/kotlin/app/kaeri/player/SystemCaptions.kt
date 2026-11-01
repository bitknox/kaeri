// The system's caption style, read for the engine: it merges it with the viewer's own style and
// writes mpv's options (mpv_subtitle_options). Nothing here draws or merges.
//
// Only while captions are on in Settings, Accessibility, Caption preferences: with them off,
// Android's style is its stock white-on-black preset, which nobody chose.

package app.kaeri.player

import android.content.Context
import android.graphics.Typeface
import android.view.accessibility.CaptioningManager
import app.kaeri.engine.model.CaptionEdge
import app.kaeri.engine.model.SystemCaptionStyle

object SystemCaptions {
    /**
     * The families Android's caption settings offer, by the names fonts.conf gives them
     * (MpvSetup.writeFontsConf), the default first.
     */
    val FAMILIES = listOf(
        "sans-serif", "sans-serif-condensed", "sans-serif-monospace", "serif", "serif-monospace",
        "casual", "cursive", "sans-serif-smallcaps",
    )

    /** The style the viewer set, with a field null where the system sets none; null while captions are off. */
    fun read(context: Context): SystemCaptionStyle? {
        val manager = context.getSystemService(CaptioningManager::class.java) ?: return null
        if (!manager.isEnabled) return null
        val style = manager.userStyle
        val typeface = style.typeface
        val family = FAMILIES.firstOrNull { typeface != null && Typeface.create(it, Typeface.NORMAL) == typeface }
        return SystemCaptionStyle(
            fontFamily = family?.takeIf { it != FAMILIES.first() },
            relativeSize = manager.fontScale.toDouble(),
            textRgba = if (style.hasForegroundColor()) rgba(style.foregroundColor) else null,
            backgroundRgba = if (style.hasBackgroundColor()) rgba(style.backgroundColor) else null,
            windowRgba = if (style.hasWindowColor()) rgba(style.windowColor) else null,
            edge = when (style.edgeType) {
                CaptioningManager.CaptionStyle.EDGE_TYPE_OUTLINE -> CaptionEdge.UNIFORM
                CaptioningManager.CaptionStyle.EDGE_TYPE_DROP_SHADOW -> CaptionEdge.DROP_SHADOW
                CaptioningManager.CaptionStyle.EDGE_TYPE_RAISED -> CaptionEdge.RAISED
                CaptioningManager.CaptionStyle.EDGE_TYPE_DEPRESSED -> CaptionEdge.DEPRESSED
                else -> CaptionEdge.PLAIN
            },
        )
    }

    /** Android's `0xAARRGGBB` as the engine's `0xRRGGBBAA`, in a UInt. */
    fun rgba(argb: Int): UInt = ((argb shl 8) or (argb ushr 24)).toUInt()
}
