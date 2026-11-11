// The app's shell on an e-ink panel, as the e-ink board draws it: paper and ink in place of the
// dark scheme, no ripples and no motion, and on an e-reader's shape the five tabs Home, Library,
// Search, Downloads and More.

package app.kaeri.ui

import androidx.compose.material3.ColorScheme
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.LocalRippleConfiguration
import androidx.compose.material3.MaterialExpressiveTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.MotionScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalConfiguration
import app.kaeri.design.LocalStillPanel
import app.kaeri.eink.EinkColours
import app.kaeri.eink.EinkLook
import app.kaeri.engine.model.KaeriIsFor

/**
 * The screens on paper while [look] is set: the light scheme of the board's tokens, ink for every
 * line and word, the solid action in ink (amber on a colour panel), no ripple, and every
 * reduced-motion path taken. Without a look, what is inside keeps the app's own scheme.
 */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun EinkPaperTheme(look: EinkLook?, content: @Composable () -> Unit) {
    val outer = MaterialTheme.colorScheme
    val scheme = remember(look, outer) { if (look != null) paperScheme(look) else outer }
    MaterialExpressiveTheme(
        colorScheme = scheme,
        motionScheme = MotionScheme.standard(),
        typography = MaterialTheme.typography,
        shapes = MaterialTheme.shapes,
    ) {
        CompositionLocalProvider(
            LocalRippleConfiguration provides if (look != null) null else LocalRippleConfiguration.current,
            LocalStillPanel provides (look != null),
            content = content,
        )
    }
}

/** The board's paper and ink as a Material scheme: grey fills only where something is chosen. */
private fun paperScheme(look: EinkLook): ColorScheme = lightColorScheme(
    primary = look.primary,
    onPrimary = look.onPrimary,
    primaryContainer = EinkColours.Selected,
    onPrimaryContainer = EinkColours.Ink,
    inversePrimary = EinkColours.Paper,
    secondary = EinkColours.Ink,
    onSecondary = EinkColours.Paper,
    secondaryContainer = EinkColours.Selected,
    onSecondaryContainer = EinkColours.Ink,
    tertiary = EinkColours.Ink,
    onTertiary = EinkColours.Paper,
    tertiaryContainer = EinkColours.Selected,
    onTertiaryContainer = EinkColours.Ink,
    background = EinkColours.Paper,
    onBackground = EinkColours.Ink,
    surface = EinkColours.Paper,
    onSurface = EinkColours.Ink,
    surfaceVariant = EinkColours.Placeholder,
    onSurfaceVariant = EinkColours.Secondary,
    surfaceTint = EinkColours.Paper,
    inverseSurface = EinkColours.Ink,
    inverseOnSurface = EinkColours.Paper,
    error = EinkColours.Ink,
    onError = EinkColours.Paper,
    errorContainer = EinkColours.Selected,
    onErrorContainer = EinkColours.Ink,
    outline = EinkColours.Ink,
    outlineVariant = EinkColours.Tertiary,
    scrim = EinkColours.Ink,
    surfaceBright = EinkColours.Paper,
    surfaceDim = EinkColours.Paper,
    surfaceContainerLowest = EinkColours.Paper,
    surfaceContainerLow = EinkColours.Paper,
    surfaceContainer = EinkColours.Paper,
    surfaceContainerHigh = EinkColours.Paper,
    surfaceContainerHighest = EinkColours.Placeholder,
)

/**
 * Whether the window has an e-reader's shape rather than a phone's: about 4:3, as the 6, 7 and
 * 10.3 inch panels are, against a phone's 2:1. The panel itself doesn't say which it is.
 */
@Composable
internal fun einkReaderShape(): Boolean {
    val configuration = LocalConfiguration.current
    val long = maxOf(configuration.screenWidthDp, configuration.screenHeightDp)
    val short = minOf(configuration.screenWidthDp, configuration.screenHeightDp)
    return short > 0 && long.toFloat() / short < 1.6f
}

/**
 * The e-ink bar's tabs: on an e-reader with Read alone, Home, Library, Search, Downloads and More,
 * with Updates opened from Home; else the phone's own.
 */
internal fun einkBarDestinations(answer: KaeriIsFor, reader: Boolean): List<Destination> =
    if (reader && answer == KaeriIsFor.READ) {
        listOf(Destination.Home, Destination.Library, Destination.Search, Destination.Downloads, Destination.More)
    } else {
        barDestinations(answer)
    }

/**
 * The tab Settings keeps marked on a 10 inch e-reader ([large]), where the bar stays under it as
 * the large panel board draws it: More, which opens Settings. Elsewhere Settings takes the window.
 */
internal fun einkSettingsTab(answer: KaeriIsFor, reader: Boolean, large: Boolean): Destination? =
    Destination.More.takeIf { large && it in einkBarDestinations(answer, reader) }

/** The e-ink tab that stands for [destination]: Updates sits under Home on an e-reader. */
internal fun einkBarOwner(destination: Destination, answer: KaeriIsFor, reader: Boolean): Destination {
    val tabs = einkBarDestinations(answer, reader)
    return when {
        destination in tabs -> destination
        destination == Destination.Downloads || destination == Destination.More -> Destination.Home
        else -> barOwner(destination, answer).takeIf { it in tabs } ?: Destination.Home
    }
}
