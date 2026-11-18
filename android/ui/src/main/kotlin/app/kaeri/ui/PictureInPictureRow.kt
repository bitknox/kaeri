// Settings, Playback, Picture and sound: Picture in picture when leaving the app, as the Hayase
// holes board draws it. A switch while Android lets Kaeri open the floating window; while the
// system's per-app permission is off, "Off in Android settings" and Open settings in its place,
// read again on every return to the app, before the first frame. Coming back with it on crosses
// the words and the button to the switch in place, on reveal, while the row settles on standard.

package app.kaeri.ui

import android.app.AppOpsManager
import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Process
import android.provider.Settings
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.AnimationVector1D
import androidx.compose.animation.core.snap
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.layout
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.design.Motion
import app.kaeri.design.atFullSpeed
import app.kaeri.design.rememberAnimationsRemoved
import app.kaeri.engine.core.SettingValue
import app.kaeri.engine.model.SettingKey
import app.kaeri.i18n.R
import app.kaeri.store.SettingsCatalog
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.launch

/** Android's per-app Picture in Picture permission, and the page that turns it on. */
object PictureInPictureAccess {
    /** Whether this device has a floating window at all: a low-memory phone has none, and shows no row. */
    fun supported(context: Context): Boolean =
        context.packageManager.hasSystemFeature(PackageManager.FEATURE_PICTURE_IN_PICTURE)

    /** Whether the person lets Kaeri open the window: the system's app op, allowed unless turned off. */
    fun allowed(context: Context): Boolean {
        val ops = context.getSystemService(AppOpsManager::class.java) ?: return true
        val mode = runCatching {
            ops.unsafeCheckOpNoThrow(AppOpsManager.OPSTR_PICTURE_IN_PICTURE, Process.myUid(), context.packageName)
        }.getOrDefault(AppOpsManager.MODE_ALLOWED)
        return mode == AppOpsManager.MODE_ALLOWED || mode == AppOpsManager.MODE_DEFAULT
    }

    /**
     * Kaeri on Android's Picture-in-picture page; where a device has no such page, the app's own
     * details page. The page's action is not in the public SDK, so it is named by its string.
     */
    fun open(context: Context) {
        val uri = Uri.fromParts("package", context.packageName, null) // i18n-skip
        val pages = listOf(
            Intent(PICTURE_IN_PICTURE_SETTINGS, uri),
            Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, uri),
        )
        for (page in pages) {
            try {
                context.startActivity(page.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
                return
            } catch (_: ActivityNotFoundException) {
            } catch (_: SecurityException) {
            }
        }
    }

    private const val PICTURE_IN_PICTURE_SETTINGS = "android.settings.PICTURE_IN_PICTURE_SETTINGS" // i18n-skip
}

/**
 * The permission as it is now: read as the row first composes, so its first frame is right, and
 * again on every resume, so coming back from Android's page shows what was chosen there.
 */
@Composable
internal fun rememberPictureInPictureAllowed(): Boolean {
    val context = LocalContext.current
    var allowed by remember { mutableStateOf(PictureInPictureAccess.allowed(context)) }
    LifecycleResumeEffect(context) {
        allowed = PictureInPictureAccess.allowed(context)
        onPauseOrDispose {}
    }
    return allowed
}

/** The row, on devices that have the window; the switch, or why it would do nothing. */
@Composable
internal fun PictureInPictureRow(place: Place) {
    val store = LocalAppStore.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val row = store.settings.row(SettingKey.PICTURE_IN_PICTURE_ON_LEAVE, view)
    PictureInPictureRow(
        place,
        on = row == null || SettingsCatalog.isOn(row),
        allowed = rememberPictureInPictureAllowed(),
        editable = row == null || SettingsCatalog.editable(row),
    ) { store.setSetting(SettingKey.PICTURE_IN_PICTURE_ON_LEAVE, SettingValue.Bool(it)) }
}

private class RowLayer(val allowed: Boolean, val alpha: Animatable<Float, AnimationVector1D>)

/**
 * The row for [on] and [allowed]. A change of [allowed] crossfades the words and the button with
 * the switch in place on reveal, the leaving side drawn over the row without holding its height,
 * while the row's height follows on standard; with Remove animations the height changes at once
 * and the crossfade stays.
 */
@Composable
fun PictureInPictureRow(place: Place, on: Boolean, allowed: Boolean, editable: Boolean = true, onChange: (Boolean) -> Unit) {
    val title = stringResource(R.string.settings_label_picture_in_picture_on_leave)
    if (einkSettings) {
        if (allowed) return SwitchRow(place, title, on, enabled = editable, onChange = onChange)
        val context = LocalContext.current
        return InfoRow(place, title, stringResource(R.string.settings_picture_in_picture_system_off), onClick = { PictureInPictureAccess.open(context) })
    }
    val removed = rememberAnimationsRemoved()
    val layers = remember { mutableStateListOf(RowLayer(allowed, Animatable(1f))) }
    LaunchedEffect(allowed) {
        if (layers.last().allowed == allowed) return@LaunchedEffect
        val coming = RowLayer(allowed, Animatable(0f))
        val leaving = layers.toList()
        layers.add(coming)
        suspend fun fade(block: suspend () -> Unit) = if (removed) atFullSpeed { block() } else block()
        coroutineScope {
            launch { fade { coming.alpha.animateTo(1f, Motion.reveal()) } }
            for (layer in leaving) launch { fade { layer.alpha.animateTo(0f, Motion.reveal()) } }
        }
        layers.removeAll { it !== coming }
    }
    val onWord = stringResource(R.string.value_on)
    val offWord = stringResource(R.string.value_off)
    val usable = allowed && editable
    RowFrame(
        place,
        onClick = if (usable) ({ onChange(!on) }) else null,
        modifier = Modifier
            .then(
                if (allowed) {
                    Modifier.semantics(mergeDescendants = true) {
                        role = Role.Switch
                        stateDescription = if (on) onWord else offWord
                    }
                } else {
                    Modifier
                },
            ),
    ) { selected ->
        // Both sides from the top, where they stand once settled, and the height animated here, so
        // the row's fill follows it.
        Box(Modifier.fillMaxWidth().animateContentSize(if (removed) snap() else Motion.standard())) {
            for (layer in layers.toList()) {
                key(layer) {
                    val current = layer === layers.last()
                    Box(
                        Modifier
                            .graphicsLayer { alpha = layer.alpha.value }
                            // The leaving side is drawn where it was, and takes no room.
                            .then(if (current) Modifier else Modifier.clearAndSetSemantics { }.zeroHeight())
                            .fillMaxWidth(),
                    ) {
                        if (layer.allowed) {
                            SwitchContent(title, on, editable, selected)
                        } else {
                            SystemOffContent(title, selected)
                        }
                    }
                }
            }
        }
    }
}

/** One line, 40 high inside the row's padding, and the switch at the end: the row stands 56 high. */
@Composable
private fun SwitchContent(title: String, on: Boolean, editable: Boolean, selected: Boolean) {
    Row(Modifier.heightIn(min = 40.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        RowText(title, null, null, selected, Modifier.weight(1f))
        SettingSwitchMark(on, editable)
    }
}

/** Two lines and Open settings under them, from 12 at the top: the row stands 112 high. */
@Composable
private fun SystemOffContent(title: String, selected: Boolean) {
    val context = LocalContext.current
    Column(Modifier.heightIn(min = 96.dp).padding(top = 4.dp)) {
        // TalkBack reads the name and why as one: "…, Off in Android settings", then the button.
        Column(Modifier.semantics(mergeDescendants = true) {}) {
            RowText(title, null, null, selected, extra = stringResource(R.string.settings_picture_in_picture_system_off))
        }
        TextButton(
            onClick = { PictureInPictureAccess.open(context) },
            contentPadding = PaddingValues(horizontal = 12.dp),
            colors = ButtonDefaults.textButtonColors(contentColor = LocalContentColor.current),
            modifier = Modifier.heightIn(min = 40.dp).offset(x = (-12).dp),
        ) {
            Text(stringResource(R.string.action_open_settings), style = MaterialTheme.typography.labelLarge)
        }
    }
}

/** Measured as it is, laid out as though it took no height: the leaving side of the crossfade. */
private fun Modifier.zeroHeight(): Modifier = layout { measurable, constraints ->
    val placeable = measurable.measure(constraints.copy(minHeight = 0, maxHeight = androidx.compose.ui.unit.Constraints.Infinity))
    layout(placeable.width, 0) { placeable.place(0, 0) }
}
