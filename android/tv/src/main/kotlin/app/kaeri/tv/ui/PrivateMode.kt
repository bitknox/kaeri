// Private watching on the TV, as the private mode board draws it: the badge at the rail's foot
// while it is on, the dialog it opens, the line under a title's actions, and the screen reader's
// words as the mode turns on and off.

package app.kaeri.tv.ui

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.tween
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.ClickableSurfaceDefaults
import androidx.tv.material3.Icon
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Surface
import androidx.tv.material3.Text
import app.kaeri.design.Motion
import app.kaeri.design.ObiSilk
import app.kaeri.design.PrivateGlyph
import app.kaeri.engine.model.PrivateTitle
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.store.PrivacyModel
import app.kaeri.store.PrivateLine

/**
 * The badge at the rail's foot while private mode is on: the eye-slash in a 40 circle, Private in
 * 12 under it. It fades in on 0.2 s and out on 0.3 s; selecting it asks whether to turn it off.
 */
@Composable
fun PrivateRailBadge(onClick: () -> Unit, modifier: Modifier = Modifier) {
    val store = LocalAppStore.current
    val on by store.privacy.on.collectAsStateWithLifecycle()
    val label = stringResource(R.string.private_badge_label)
    AnimatedVisibility(on, modifier, enter = fadeIn(Motion.fadeIn()), exit = fadeOut(Motion.fadeOut())) {
        Column(Modifier.width(48.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Surface(
                onClick = onClick,
                modifier = Modifier.size(40.dp).semantics { contentDescription = label },
                shape = ClickableSurfaceDefaults.shape(CircleShape),
                colors = ClickableSurfaceDefaults.colors(
                    containerColor = ObiSilk.RaisedSelected,
                    contentColor = ObiSilk.Silk,
                    focusedContainerColor = ObiSilk.Silk,
                    focusedContentColor = ObiSilk.Ground,
                    pressedContainerColor = ObiSilk.Silk,
                    pressedContentColor = ObiSilk.Ground,
                ),
                scale = ClickableSurfaceDefaults.scale(focusedScale = focusedScale(1.1f)),
            ) {
                Box(Modifier.size(40.dp), contentAlignment = Alignment.Center) {
                    Icon(PrivateGlyph.EyeSlash, contentDescription = null, modifier = Modifier.size(20.dp))
                }
            }
            Text(
                stringResource(R.string.private_badge),
                style = MaterialTheme.typography.labelSmall.copy(fontSize = 12.sp, lineHeight = 16.sp),
                color = ObiSilk.Silk,
                maxLines = 1,
                modifier = Modifier.clearAndSetSemantics {},
            )
        }
    }
}

/**
 * What the badge opens: the eye-slash and Private mode, the notice's words, then Turn off, focused,
 * and Cancel. Turning off closes it at once.
 */
@Composable
fun PrivateModeDialog(onDismiss: () -> Unit) {
    val store = LocalAppStore.current
    Dialog(onDismissRequest = onDismiss) {
        val turnOff = remember { FocusRequester() }
        Column(
            Modifier
                .entryFocus(turnOff)
                .widthIn(max = 460.dp)
                .background(ObiSilk.RaisedHigh, RoundedCornerShape(28.dp))
                .padding(28.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Icon(PrivateGlyph.EyeSlash, contentDescription = null, tint = ObiSilk.Silk, modifier = Modifier.size(26.dp))
                Text(stringResource(R.string.private_mode), style = MaterialTheme.typography.headlineSmall.copy(fontSize = 24.sp, lineHeight = 32.sp), color = ObiSilk.Silk)
            }
            Text(
                stringResource(R.string.private_mode_on_notice),
                style = MaterialTheme.typography.bodyLarge.copy(fontSize = 16.sp, lineHeight = 23.sp),
                color = ObiSilk.Haze,
                modifier = Modifier.padding(top = 10.dp),
            )
            Spacer(Modifier.height(26.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                TonalButton(
                    onClick = {
                        store.setPrivate(false)
                        onDismiss()
                    },
                    modifier = Modifier.focusRequester(turnOff),
                ) { ButtonLabel(stringResource(R.string.action_turn_off)) }
                TonalButton(onClick = onDismiss) { ButtonLabel(stringResource(R.string.action_cancel)) }
            }
        }
    }
}

/**
 * The line under a title's actions, as the phone's: the eye-slash, then "Private · Not saved or
 * synced", its head in silk and the rest haze, 16 on 22. It makes room on standard and its words
 * fade in after 0.1 s.
 */
@Composable
fun PrivateTitleLine(title: PrivateTitle, modifier: Modifier = Modifier) {
    val privacy = LocalAppStore.current.privacy
    val on by privacy.on.collectAsStateWithLifecycle()
    val always by privacy.always.collectAsStateWithLifecycle()
    val kept by privacy.kept.collectAsStateWithLifecycle()
    val line = PrivacyModel.line(on, title in always, title in kept)
    val shown = remember { mutableStateOf(line) }
    if (line != null) shown.value = line
    AnimatedVisibility(
        visible = line != null,
        modifier = modifier,
        enter = expandVertically(Motion.standard(), expandFrom = Alignment.Top) + fadeIn(tween(200, delayMillis = 100)),
        exit = shrinkVertically(Motion.standard(), shrinkTowards = Alignment.Top) + fadeOut(Motion.fadeOut()),
    ) {
        val said = shown.value ?: return@AnimatedVisibility
        val head = stringResource(if (said == PrivateLine.Always) R.string.title_always_private else R.string.private_badge)
        val tail = stringResource(if (said == PrivateLine.Kept) R.string.private_line_kept else R.string.private_line_not_saved)
        Row(
            Modifier.padding(top = 12.dp).semantics(mergeDescendants = true) {},
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Icon(PrivateGlyph.EyeSlash, contentDescription = null, tint = ObiSilk.Silk, modifier = Modifier.size(20.dp))
            Text(
                buildAnnotatedString {
                    withStyle(SpanStyle(color = ObiSilk.Silk, fontWeight = FontWeight.Medium)) { append(head) }
                    append(" · ") // i18n-skip
                    append(tail)
                },
                style = MaterialTheme.typography.bodyLarge.copy(fontSize = 16.sp, lineHeight = 22.sp),
                color = ObiSilk.Haze,
                maxLines = 1,
            )
        }
    }
}

/** "Private mode on" or "Private mode off", said once as the mode changes. Nothing at launch. */
@Composable
fun PrivateModeAnnouncer() {
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
        @Suppress("DEPRECATION")
        view.announceForAccessibility(if (on) Strings.privateModeOnAnnouncement(context) else Strings.privateModeOffAnnouncement(context))
    }
}
