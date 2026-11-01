// The app's theme: Material 3 Expressive components with calm motion, on the fixed palette.

package app.kaeri.design

import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.LocalTonalElevationEnabled
import androidx.compose.material3.MaterialExpressiveTheme
import androidx.compose.material3.MotionScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider

/**
 * The palette, the type and the shapes, with the standard motion scheme: its spatial springs damp
 * at 0.9 and never visibly pass their target, where the expressive scheme's overshoot. Tonal
 * elevation is off, so depth shows only as the container roles. The system's contrast level and
 * high contrast text lift the supporting text and the edges, live.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun KaeriTheme(content: @Composable () -> Unit) {
    MaterialExpressiveTheme(
        colorScheme = if (rememberHighContrast()) ObiSilkDarkHighContrast else ObiSilkDark,
        motionScheme = MotionScheme.standard(),
        typography = KaeriType,
        shapes = KaeriShapes,
    ) {
        CompositionLocalProvider(LocalTonalElevationEnabled provides false, content = content)
    }
}
