// The parts every "what came in" form is made of: the result line with its glyph (a silk check for
// what came in, the haze info glyph for nothing yet, the vermilion one for an error), the waiting
// line, the outlined field with its key chip, the list container and its rows, and the container
// the form sits in: a full-screen dialog on a phone, a 560 wide dialog on a tablet, or first run's
// own page. Every change inside a form crossfades in place over 0.3 s, its height on standard.

package app.kaeri.ui.reading

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.SizeTransform
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.MutableTransitionState
import androidx.compose.animation.core.snap
import androidx.compose.animation.core.tween
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.border
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Checkbox
import androidx.compose.material3.CheckboxDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.addPathNodes
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.OffsetMapping
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.TransformedText
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.design.rememberAnimationsRemoved
import app.kaeri.eink.EinkColours
import app.kaeri.eink.EinkSizes
import app.kaeri.eink.EinkSolidButton
import app.kaeri.eink.EinkTheme
import app.kaeri.engine.model.CameIn
import app.kaeri.i18n.R
import app.kaeri.store.Adding
import app.kaeri.store.CameInModel
import app.kaeri.ui.Icons
import app.kaeri.ui.LocalAppStore
import app.kaeri.ui.LocalSnackbars
import app.kaeri.ui.SettingSwitchMark
import app.kaeri.ui.wideWindow
import app.kaeri.i18n.Strings

// Motion

/** The board's platform table for Android: sheets open on emphasized decelerate, close on emphasized accelerate. */
internal object CameInMotion {
    val EmphasizedDecelerate = CubicBezierEasing(0.05f, 0.7f, 0.1f, 1f)
    val EmphasizedAccelerate = CubicBezierEasing(0.3f, 0f, 0.8f, 0.15f)
    const val OPEN_MILLIS = 400
    const val CLOSE_MILLIS = 200

    /** A crossfade in place: the result, the waiting line, an action's colour. */
    const val CROSSFADE_MILLIS = 300

    /** The new row lands 70% of the way into the form's close. */
    const val LAND_DELAY_MILLIS = CLOSE_MILLIS * 7 / 10
}

// Glyphs

/** The glyphs the forms draw that the app's own set has no use for elsewhere, stroked as [Icons]. */
internal object CameInGlyphs {
    private fun stroked(name: String, vararg paths: String) = ImageVector.Builder(
        name = name, defaultWidth = 24.dp, defaultHeight = 24.dp, viewportWidth = 24f, viewportHeight = 24f,
    ).apply {
        for (path in paths) {
            addPath(
                pathData = addPathNodes(path),
                stroke = SolidColor(Color.Black),
                strokeLineWidth = 2f,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
            )
        }
    }.build()

    val Folder = stroked("Folder", "M3.5 7 A1.5 1.5 0 0 1 5 5.5 H9.5 L11.5 7.5 H19 A1.5 1.5 0 0 1 20.5 9 V17 A1.5 1.5 0 0 1 19 18.5 H5 A1.5 1.5 0 0 1 3.5 17 Z")
    val Server = stroked(
        "Server",
        "M5 4.5 H19 A1 1 0 0 1 20 5.5 V10 A1 1 0 0 1 19 11 H5 A1 1 0 0 1 4 10 V5.5 A1 1 0 0 1 5 4.5 Z",
        "M5 13 H19 A1 1 0 0 1 20 14 V18.5 A1 1 0 0 1 19 19.5 H5 A1 1 0 0 1 4 18.5 V14 A1 1 0 0 1 5 13 Z",
        "M8 7.75 H8.01 M8 16.25 H8.01",
    )
    val Plug = stroked("Plug", "M9 3.5 V7.5 M15 3.5 V7.5", "M6.5 7.5 H17.5 V11 A5.5 5.5 0 0 1 6.5 11 Z", "M12 16.5 V20.5")
    val Key = stroked("Key", "M4.5 15.5 A3.5 3.5 0 1 0 11.5 15.5 A3.5 3.5 0 1 0 4.5 15.5 Z", "M10.5 13 L19.5 4 M16 7.5 L18.5 10 M18 5.5 L20 7.5")
    val Info = stroked("Info", "M3 12 A9 9 0 1 0 21 12 A9 9 0 1 0 3 12 Z", "M12 11 V16.5 M12 7.75 V7.76")
    val Book = stroked("Book", "M12 6.5 C10.5 5.2 8 4.5 4 4.5 V18 C8 18 10.5 18.7 12 20 C13.5 18.7 16 18 20 18 V4.5 C16 4.5 13.5 5.2 12 6.5 Z M12 6.5 V20")
}

/** The error glyph, filled vermilion with its mark cut in the ground's colour, as the board draws it. */
@Composable
private fun ErrorGlyph(modifier: Modifier = Modifier) {
    val colors = MaterialTheme.colorScheme
    val glyph = remember(colors.error, colors.surface) {
        ImageVector.Builder(name = "Error", defaultWidth = 24.dp, defaultHeight = 24.dp, viewportWidth = 24f, viewportHeight = 24f).apply {
            addPath(
                pathData = addPathNodes("M10.27 4 A2 2 0 0 1 13.73 4 L21.53 17.5 A2 2 0 0 1 19.8 20.5 H4.2 A2 2 0 0 1 2.47 17.5 Z"),
                fill = SolidColor(colors.error),
            )
            addPath(
                pathData = addPathNodes("M12 9.5 V13.5 M12 16.8 V16.81"),
                stroke = SolidColor(colors.surface),
                strokeLineWidth = 2.2f,
                strokeLineCap = StrokeCap.Round,
            )
        }.build()
    }
    Icon(glyph, contentDescription = stringResource(R.string.error_label), tint = Color.Unspecified, modifier = modifier)
}

// Result lines

/** What a result line says by its glyph. */
internal enum class NoteKind { FOUND, INFO, FAILED }

/**
 * A result: its glyph (18 across, level with the title's first line), the title in semibold silk,
 * and the line under it in haze.
 */
@Composable
internal fun CameInNote(kind: NoteKind, title: String, line: String? = null, modifier: Modifier = Modifier) {
    val colors = MaterialTheme.colorScheme
    Row(
        modifier.fillMaxWidth().semantics(mergeDescendants = true) { liveRegion = LiveRegionMode.Polite },
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Box(Modifier.padding(top = 3.dp).size(18.dp), contentAlignment = Alignment.Center) {
            when (kind) {
                NoteKind.FOUND -> Icon(Icons.Check, contentDescription = null, tint = colors.onSurface, modifier = Modifier.size(18.dp))
                NoteKind.INFO -> Icon(CameInGlyphs.Info, contentDescription = null, tint = colors.onSurfaceVariant, modifier = Modifier.size(18.dp))
                NoteKind.FAILED -> ErrorGlyph(Modifier.size(18.dp))
            }
        }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(title, style = NoteTitle, color = colors.onSurface)
            line?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant) }
        }
    }
}

/** A result's title: 16 on 24, semibold. */
private val NoteTitle @Composable get() = MaterialTheme.typography.titleMedium.copy(fontSize = 16.sp, lineHeight = 24.sp, fontWeight = FontWeight.SemiBold)

/** The waiting line: a small ring and the words, after 600 ms with no answer. */
@Composable
internal fun CameInWaiting(text: String, modifier: Modifier = Modifier) {
    Row(
        modifier.fillMaxWidth().semantics(mergeDescendants = true) { liveRegion = LiveRegionMode.Polite },
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Text(text, style = NoteTitle, color = MaterialTheme.colorScheme.onSurface)
    }
}

/**
 * The part of a form that changes with each answer: [state] crossfades in place over 0.3 s, never
 * through empty, while the height moves on standard. Reduce motion keeps the crossfade, at 200 ms,
 * and lets the height jump.
 */
@Composable
internal fun <T> CameInSwap(state: T, modifier: Modifier = Modifier, content: @Composable (T) -> Unit) {
    val reduced = rememberAnimationsRemoved()
    val fade = if (reduced) 200 else CameInMotion.CROSSFADE_MILLIS
    AnimatedContent(
        targetState = state,
        modifier = modifier.fillMaxWidth(),
        transitionSpec = {
            (fadeIn(tween(fade)) togetherWith fadeOut(tween(fade)))
                .using(SizeTransform(clip = false) { _, _ -> if (reduced) snap() else app.kaeri.design.Motion.standard() })
        },
        contentAlignment = Alignment.TopStart,
        label = "came in", // i18n-skip
    ) { content(it) }
}

/** Something that grows in or leaves on standard with a fade, as the sign-in fields and the key's line. */
@Composable
internal fun CameInReveal(visible: Boolean, content: @Composable ColumnScope.() -> Unit) {
    val reduced = rememberAnimationsRemoved()
    AnimatedVisibility(
        visible = visible,
        enter = if (reduced) fadeIn(tween(200)) else fadeIn(tween(CameInMotion.CROSSFADE_MILLIS)) + expandVertically(app.kaeri.design.Motion.standard()),
        exit = if (reduced) fadeOut(tween(200)) else fadeOut(tween(CameInMotion.CROSSFADE_MILLIS)) + shrinkVertically(app.kaeri.design.Motion.standard()),
    ) { Column(Modifier.fillMaxWidth(), content = content) }
}

// The field

/** The chip inside a field: a key glyph and the secret's last four characters, in mono. */
@Composable
internal fun KeyChip(text: String, modifier: Modifier = Modifier) {
    val colors = MaterialTheme.colorScheme
    // The chip fades in over 0.2 s once a link is recognised.
    val shown = remember { MutableTransitionState(false) }.apply { targetState = true }
    AnimatedVisibility(shown, enter = fadeIn(tween(200)), exit = fadeOut(tween(200))) {
        Row(
            modifier
                .height(22.dp)
                .clip(RoundedCornerShape(6.dp))
                .background(colors.surfaceContainerHighest)
                .padding(horizontal = 8.dp),
            horizontalArrangement = Arrangement.spacedBy(6.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(CameInGlyphs.Key, contentDescription = null, tint = colors.onSurfaceVariant, modifier = Modifier.size(12.dp))
            Text(text, fontFamily = FontFamily.Monospace, fontSize = 12.sp, lineHeight = 16.sp, color = colors.onSurface, maxLines = 1)
        }
    }
}

/** Shows [shown] in place of what is typed, so a secret in it never shows; the cursor stays at the end. */
private class ShownAs(private val shown: String) : VisualTransformation {
    override fun filter(text: AnnotatedString): TransformedText = TransformedText(
        AnnotatedString(shown),
        object : OffsetMapping {
            // The start of what stands in shows, so a long link is cut at its end, never its scheme.
            override fun originalToTransformed(offset: Int): Int = 0
            override fun transformedToOriginal(offset: Int): Int = text.length
        },
    )

    override fun equals(other: Any?): Boolean = other is ShownAs && other.shown == shown
    override fun hashCode(): Int = shown.hashCode()
}

/**
 * The M3 outlined field the forms use: its label on the outline, water when focused, the [chip] at
 * its end, and [shownAs] drawn in place of a pasted secret.
 */
@Composable
internal fun CameInField(
    value: String,
    onValueChange: (String) -> Unit,
    label: String,
    modifier: Modifier = Modifier,
    chip: String? = null,
    shownAs: String? = null,
    supporting: String? = null,
    password: Boolean = false,
    keyboard: KeyboardOptions = KeyboardOptions.Default,
    actions: KeyboardActions = KeyboardActions.Default,
) {
    if (largePaper) return PaperField(value, onValueChange, label, modifier, chip, shownAs, supporting, password, keyboard, actions)
    val colors = MaterialTheme.colorScheme
    OutlinedTextField(
        value = value,
        onValueChange = onValueChange,
        label = { Text(label) },
        singleLine = true,
        visualTransformation = when {
            password -> PasswordVisualTransformation()
            shownAs != null -> ShownAs(shownAs)
            else -> VisualTransformation.None
        },
        trailingIcon = chip?.let { { KeyChip(it, Modifier.padding(end = 12.dp)) } },
        supportingText = supporting?.let { { Text(it) } },
        keyboardOptions = keyboard,
        keyboardActions = actions,
        colors = OutlinedTextFieldDefaults.colors(
            focusedBorderColor = colors.secondary,
            focusedLabelColor = colors.secondary,
            cursorColor = colors.secondary,
            unfocusedBorderColor = colors.outline,
            unfocusedLabelColor = colors.onSurfaceVariant,
            focusedSupportingTextColor = colors.onSurfaceVariant,
            unfocusedSupportingTextColor = colors.onSurfaceVariant,
        ),
        modifier = modifier.fillMaxWidth(),
    )
}

/**
 * A field on a large e-ink panel: its name over it in bold #333, then the words in a 54 dp box
 * edged in 2 dp ink at corners 10, and the chip's words at its end. No floating label, no colour.
 */
@Composable
private fun PaperField(
    value: String,
    onValueChange: (String) -> Unit,
    label: String,
    modifier: Modifier,
    chip: String?,
    shownAs: String?,
    supporting: String?,
    password: Boolean,
    keyboard: KeyboardOptions,
    actions: KeyboardActions,
) {
    val ink = app.kaeri.eink.EinkColours.Ink
    Column(modifier.fillMaxWidth()) {
        Text(label, style = app.kaeri.eink.EinkType.Secondary.copy(fontWeight = androidx.compose.ui.text.font.FontWeight.Bold), modifier = Modifier.padding(bottom = 6.dp))
        androidx.compose.foundation.text.BasicTextField(
            value = value,
            onValueChange = onValueChange,
            singleLine = true,
            textStyle = app.kaeri.eink.EinkType.Body.copy(fontSize = 17.sp, color = ink),
            cursorBrush = androidx.compose.ui.graphics.SolidColor(ink),
            visualTransformation = when {
                password -> PasswordVisualTransformation()
                shownAs != null -> ShownAs(shownAs)
                else -> VisualTransformation.None
            },
            keyboardOptions = keyboard,
            keyboardActions = actions,
            modifier = Modifier.fillMaxWidth().semantics { contentDescription = label },
            decorationBox = { field ->
                Row(
                    Modifier
                        .fillMaxWidth()
                        .heightIn(min = 54.dp)
                        .border(BorderStroke(EinkSizes.Rule, ink), RoundedCornerShape(EinkSizes.SegmentRadius))
                        .padding(horizontal = 16.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Box(Modifier.weight(1f)) { field() }
                    chip?.let { Text(it, style = app.kaeri.eink.EinkType.Secondary, modifier = Modifier.padding(start = 12.dp)) }
                }
            },
        )
        supporting?.let { Text(it, style = app.kaeri.eink.EinkType.Secondary, modifier = Modifier.padding(top = 6.dp)) }
    }
}

// The list container and its rows

/** The list container: rows on surfaceContainer, corners 16, a hairline between rows inset 16. */
@Composable
internal fun CameInGroup(rows: List<@Composable () -> Unit>, modifier: Modifier = Modifier) {
    val colors = MaterialTheme.colorScheme
    Column(
        modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(16.dp))
            .background(colors.surfaceContainer),
    ) {
        rows.forEachIndexed { index, row ->
            if (index > 0) HorizontalDivider(Modifier.padding(horizontal = 16.dp), color = colors.outlineVariant)
            row()
        }
    }
}

/** A group's heading over the container: 14, medium, haze. */
@Composable
internal fun CameInGroupHeading(text: String, modifier: Modifier = Modifier) {
    Text(
        text,
        style = MaterialTheme.typography.titleSmall.copy(fontSize = 14.sp, lineHeight = 20.sp, fontWeight = FontWeight.Medium),
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = modifier.padding(start = 16.dp, end = 16.dp, bottom = 8.dp).semantics { heading() },
    )
}

/** A note under a container: 12, haze. */
@Composable
internal fun CameInFootnote(text: String, modifier: Modifier = Modifier) {
    Text(
        text,
        style = MaterialTheme.typography.bodySmall.copy(fontSize = 12.sp, lineHeight = 16.sp),
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = modifier.padding(horizontal = 16.dp).padding(top = 8.dp),
    )
}

/** A row's words: the title in bodyLarge silk, the line under it in haze. */
@Composable
internal fun CameInRowText(title: String, line: String?, modifier: Modifier = Modifier) {
    Column(modifier) {
        Text(title, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurface, maxLines = 2, overflow = TextOverflow.Ellipsis)
        line?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant) }
    }
}

/** The tile a row leads with: its glyph on surfaceContainerHighest, 36 across. */
@Composable
internal fun CameInTile(glyph: ImageVector, modifier: Modifier = Modifier) {
    Box(
        modifier
            .size(36.dp)
            .clip(RoundedCornerShape(10.dp))
            .background(MaterialTheme.colorScheme.surfaceContainerHighest),
        contentAlignment = Alignment.Center,
    ) {
        Icon(glyph, contentDescription = null, tint = MaterialTheme.colorScheme.onSurface, modifier = Modifier.size(20.dp))
    }
}

/** A library the server found, ticked to add it: the whole row toggles the amber checkbox. */
@Composable
internal fun LibraryRow(name: String, line: String, checked: Boolean, onChange: (Boolean) -> Unit) {
    Row(
        Modifier
            .fillMaxWidth()
            .toggleable(checked, role = Role.Checkbox, onValueChange = onChange)
            .heightIn(min = 56.dp)
            .padding(start = 16.dp, end = 4.dp, top = 12.dp, bottom = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        CameInRowText(name, line, Modifier.weight(1f))
        Checkbox(checked, onCheckedChange = null, colors = CheckboxDefaults.colors(checkedColor = MaterialTheme.colorScheme.primary), modifier = Modifier.padding(horizontal = 12.dp))
    }
}

/** A switch in its own container: Send progress to the server. */
@Composable
internal fun SwitchCard(title: String, line: String, checked: Boolean, onChange: (Boolean) -> Unit, modifier: Modifier = Modifier) {
    val onWord = stringResource(R.string.value_on)
    val offWord = stringResource(R.string.value_off)
    CameInGroup(
        modifier = modifier,
        rows = listOf {
            Row(
                Modifier
                    .fillMaxWidth()
                    .toggleable(checked, role = Role.Switch, onValueChange = onChange)
                    .semantics { stateDescription = if (checked) onWord else offWord }
                    .padding(horizontal = 16.dp, vertical = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                CameInRowText(title, line, Modifier.weight(1f))
                SettingSwitchMark(checked)
            }
        },
    )
}

/** The folder being added: its tile and name, and Change to pick another. */
@Composable
internal fun FolderCard(name: String, onChange: () -> Unit, modifier: Modifier = Modifier) {
    CameInGroup(
        modifier = modifier,
        rows = listOf {
            Row(
                Modifier.fillMaxWidth().heightIn(min = 64.dp).padding(start = 16.dp, end = 16.dp, top = 12.dp, bottom = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                CameInTile(CameInGlyphs.Folder)
                Text(name, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
                FilledTonalButton(onClick = onChange) { Text(stringResource(R.string.book_folder_change)) }
            }
        },
    )
}

// The container

/** One of a form's actions. */
@Immutable
internal class CameInAction(val label: String, val enabled: Boolean = true, val working: Boolean = false, val onClick: () -> Unit)

/**
 * A form, as the dialog or first run's page draws it: its title, the one filled action, a second
 * action (tonal in the dialog's body, a text button in first run's foot), what closing does, and
 * its body.
 */
@Immutable
internal class CameInForm(
    val title: String,
    val primary: CameInAction?,
    val secondary: CameInAction?,
    val close: () -> Unit,
    val body: @Composable ColumnScope.(inPage: Boolean) -> Unit,
)

/** The second action inside a dialog's body: tonal, under the result. */
@Composable
internal fun SecondaryInBody(action: CameInAction?, inPage: Boolean) {
    if (inPage || action == null) return
    FilledTonalButton(onClick = action.onClick, enabled = action.enabled, modifier = Modifier.padding(top = 16.dp)) { Text(action.label) }
}

/**
 * [form] in a dialog over the app while [open]: full screen on a phone, with the close button,
 * the title and the action at the top's end, amber once it can be pressed; 560 wide and centred on
 * a tablet. It opens on emphasized decelerate over 0.4 s and closes on emphasized accelerate over
 * 0.2 s, showing what it last showed while it leaves.
 */
@Composable
internal fun CameInDialog(open: Boolean, form: @Composable () -> CameInForm) {
    val visible = remember { MutableTransitionState(false) }
    visible.targetState = open
    if (!visible.currentState && !visible.targetState && visible.isIdle) return
    val reduced = rememberAnimationsRemoved()
    // What the dialog showed when it was last open: leaving, it shows that, not an emptied form.
    val shown = form()
    val held = remember { arrayOfNulls<CameInForm>(1) }
    if (open) held[0] = shown
    val drawn = held[0] ?: shown
    val dismiss by rememberUpdatedState(drawn.close)
    Dialog(
        onDismissRequest = { dismiss() },
        properties = DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false),
    ) {
        val wide = wideWindow()
        // On paper: no motion, and a frame of its own, a 2 dp ink edge round solid paper, as
        // a shadow does not draw on the panel.
        val paper = LocalEinkLayout.current
        if (paper != null) {
            // On paper, at every size: no dim over the page behind and no window animation; the
            // ink edge alone sets the dialog apart, and a grey page would cost a full refresh.
            val window = (androidx.compose.ui.platform.LocalView.current.parent as? androidx.compose.ui.window.DialogWindowProvider)?.window
            androidx.compose.runtime.SideEffect {
                window?.setDimAmount(0f)
                window?.setWindowAnimations(0)
            }
        }
        val enter = if (paper != null) EnterTransition.None else if (reduced) fadeIn(tween(200)) else fadeIn(tween(CameInMotion.OPEN_MILLIS, easing = CameInMotion.EmphasizedDecelerate)) +
            slideInVertically(tween(CameInMotion.OPEN_MILLIS, easing = CameInMotion.EmphasizedDecelerate)) { it / 10 }
        val exit = if (paper != null) ExitTransition.None else if (reduced) fadeOut(tween(200)) else fadeOut(tween(CameInMotion.CLOSE_MILLIS, easing = CameInMotion.EmphasizedAccelerate)) +
            slideOutVertically(tween(CameInMotion.CLOSE_MILLIS, easing = CameInMotion.EmphasizedAccelerate)) { it / 10 }
        if (wide && paper != null && largePaper) {
            // A 10.3 inch panel: 720 wide, under the status bar, with its title and Cancel at the
            // top and Cancel and the solid action at the foot, as the large panel board draws it.
            Box(Modifier.fillMaxSize().statusBarsPadding().imePadding().padding(top = 56.dp, bottom = 24.dp), contentAlignment = Alignment.TopCenter) {
                AnimatedVisibility(visible, enter = enter, exit = exit) {
                    Surface(
                        shape = RoundedCornerShape(16.dp),
                        color = EinkColours.Paper,
                        contentColor = EinkColours.Ink,
                        border = BorderStroke(EinkSizes.Rule, EinkColours.Ink),
                        modifier = Modifier.width(720.dp),
                    ) { LargePaperContent(drawn, paper) }
                }
            }
        } else if (wide) {
            Box(Modifier.fillMaxSize().imePadding(), contentAlignment = Alignment.Center) {
                AnimatedVisibility(visible, enter = enter, exit = exit) {
                    Surface(
                        shape = RoundedCornerShape(if (paper != null) EinkSizes.SegmentRadius else 28.dp),
                        color = if (paper != null) EinkColours.Paper else MaterialTheme.colorScheme.surfaceContainerHigh,
                        contentColor = if (paper != null) EinkColours.Ink else MaterialTheme.colorScheme.onSurface,
                        border = if (paper != null) BorderStroke(EinkSizes.Rule, EinkColours.Ink) else null,
                        modifier = Modifier.widthIn(max = 560.dp).fillMaxWidth().heightIn(max = 720.dp).padding(vertical = 24.dp),
                    ) { DialogContent(drawn, wide = true) }
                }
            }
        } else {
            AnimatedVisibility(visible, enter = enter, exit = exit) {
                Surface(color = MaterialTheme.colorScheme.surface, modifier = Modifier.fillMaxSize()) { DialogContent(drawn, wide = false) }
            }
        }
    }
}

/** Whether a form draws on a large e-ink panel, 10.3 inch and up. */
internal val largePaper: Boolean
    @Composable @androidx.compose.runtime.ReadOnlyComposable
    get() = LocalEinkLayout.current != null && androidx.compose.ui.platform.LocalConfiguration.current.screenWidthDp >= 900

/** A form on a large panel: the title and Cancel, the body 28 in from the edge, then Cancel and the solid action. */
@Composable
private fun LargePaperContent(form: CameInForm, look: app.kaeri.eink.EinkLook) {
    val cancel = stringResource(R.string.action_cancel)
    EinkTheme(look) {
        Column {
            Row(
                Modifier.fillMaxWidth().height(66.dp).padding(start = 28.dp, end = 16.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    form.title,
                    style = app.kaeri.eink.EinkType.ReaderTitle.copy(fontWeight = androidx.compose.ui.text.font.FontWeight.Bold),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f).semantics { heading() },
                )
                app.kaeri.eink.EinkTextButton(cancel, form.close)
            }
            Column(
                Modifier
                    .weight(1f, fill = false)
                    .fillMaxWidth()
                    .verticalScroll(rememberScrollState())
                    .padding(horizontal = 28.dp),
            ) { form.body(this, false) }
            Row(
                Modifier.fillMaxWidth().padding(start = 28.dp, end = 28.dp, top = 18.dp, bottom = 24.dp),
                horizontalArrangement = Arrangement.spacedBy(12.dp, Alignment.End),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                app.kaeri.eink.EinkTextButton(cancel, form.close)
                form.primary?.let { action -> EinkSolidButton(action.label, action.onClick, enabled = action.enabled && !action.working) }
            }
        }
    }
}

@Composable
private fun DialogContent(form: CameInForm, wide: Boolean) {
    val colors = MaterialTheme.colorScheme
    Column(Modifier.then(if (wide) Modifier else Modifier.fillMaxSize().statusBarsPadding().navigationBarsPadding().imePadding())) {
        Row(
            Modifier.fillMaxWidth().height(64.dp).padding(start = 4.dp, end = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            IconButton(onClick = form.close) {
                Icon(Icons.Clear, contentDescription = stringResource(R.string.action_close), tint = colors.onSurface)
            }
            Text(
                form.title,
                style = MaterialTheme.typography.titleLarge,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f).semantics { heading() },
            )
            val paper = LocalEinkLayout.current
            if (paper != null) {
                // On paper the action is the screen's one solid button, as every e-ink primary is.
                form.primary?.let { action ->
                    EinkTheme(paper) {
                        EinkSolidButton(action.label, action.onClick, enabled = action.enabled && !action.working)
                    }
                }
            } else form.primary?.let { action ->
                // Amber once there is a result to act on, faded in over 0.2 s; silk at 38% until then.
                val tint by animateColorAsState(
                    if (action.enabled) colors.primary else colors.onSurface.copy(alpha = 0.38f),
                    tween(200),
                    label = "action", // i18n-skip
                )
                TextButton(
                    onClick = action.onClick,
                    enabled = action.enabled && !action.working,
                    colors = ButtonDefaults.textButtonColors(contentColor = tint, disabledContentColor = tint),
                ) { Text(action.label, style = MaterialTheme.typography.labelLarge) }
            }
        }
        Column(
            Modifier
                .then(if (wide) Modifier else Modifier.weight(1f))
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 24.dp),
        ) { form.body(this, false) }
    }
}

/** First run's foot for a form: its second action as a text button, then the filled one, dimmed amber until it can be pressed. */
@Composable
internal fun CameInPageActions(form: CameInForm) {
    val colors = MaterialTheme.colorScheme
    form.secondary?.let { action ->
        TextButton(
            onClick = action.onClick,
            enabled = action.enabled,
            colors = ButtonDefaults.textButtonColors(contentColor = colors.onSurface),
        ) { Text(action.label) }
    }
    form.primary?.let { action ->
        Button(
            onClick = action.onClick,
            enabled = action.enabled && !action.working,
            colors = ButtonDefaults.buttonColors(
                disabledContainerColor = colors.primary.copy(alpha = 0.38f),
                disabledContentColor = colors.onPrimary.copy(alpha = 0.6f),
            ),
        ) { app.kaeri.ui.WorkLabel(action.label, action.working) }
    }
}

// The notice

/** The words of a notice a form left. */
@Composable
internal fun CameInModel.Notice.words(): String {
    val context = androidx.compose.ui.platform.LocalContext.current
    return when (this) {
        is CameInModel.Notice.Added -> Strings.sourceAdded(context, name, series)
        is CameInModel.Notice.Plugins -> Strings.sourceAddedPlugins(context, count, store)
    }
}

/** Says what a closed form added, once, as a 4 s notice, while no form is open over it. */
@Composable
internal fun CameInNoticeHost() {
    val store = LocalAppStore.current
    val snackbars = LocalSnackbars.current
    val notice by store.cameIn.notice.collectAsStateWithLifecycle()
    val open by store.cameIn.open.collectAsStateWithLifecycle()
    val said = notice ?: return
    val text = said.words()
    LaunchedEffect(said, open) {
        if (open != null) return@LaunchedEffect
        // After the form's close, so the notice fades in on the page the row landed on.
        kotlinx.coroutines.delay(CameInMotion.CLOSE_MILLIS.toLong())
        snackbars.show(text)
        store.cameIn.noticed()
    }
}

/** For the forms: the open form and its answer, held as one value. */
@Immutable
internal data class CameInState(
    val open: Adding?,
    val inPage: Boolean,
    val address: String,
    val user: String,
    val password: String,
    val asked: CameInModel.Asked?,
    val outcome: CameIn?,
    val asking: Boolean,
    val looking: Boolean,
    val folder: String?,
)

@Composable
internal fun rememberCameInState(): CameInState {
    val model = LocalAppStore.current.cameIn
    val open by model.open.collectAsStateWithLifecycle()
    val inPage by model.inPage.collectAsStateWithLifecycle()
    val address by model.address.collectAsStateWithLifecycle()
    val user by model.user.collectAsStateWithLifecycle()
    val password by model.password.collectAsStateWithLifecycle()
    val asked by model.asked.collectAsStateWithLifecycle()
    val outcome by model.outcome.collectAsStateWithLifecycle()
    val asking by model.asking.collectAsStateWithLifecycle()
    val looking by model.looking.collectAsStateWithLifecycle()
    val folder by model.folder.collectAsStateWithLifecycle()
    return CameInState(open, inPage, address, user, password, asked, outcome, asking, looking, folder)
}

/** A row that lands in Your sources: it grows in on standard over 0.3 s, 70% of the way into the form's close. */
@Composable
internal fun LandingRow(landing: Boolean, content: @Composable () -> Unit) {
    val reduced = rememberAnimationsRemoved()
    val shown = remember { MutableTransitionState(!landing) }.apply { targetState = true }
    AnimatedVisibility(
        shown,
        enter = if (reduced) {
            fadeIn(tween(200, delayMillis = CameInMotion.LAND_DELAY_MILLIS))
        } else {
            fadeIn(tween(CameInMotion.CROSSFADE_MILLIS, delayMillis = CameInMotion.LAND_DELAY_MILLIS)) +
                expandVertically(tween(CameInMotion.CROSSFADE_MILLIS, delayMillis = CameInMotion.LAND_DELAY_MILLIS, easing = androidx.compose.animation.core.FastOutSlowInEasing))
        },
    ) { content() }
}
