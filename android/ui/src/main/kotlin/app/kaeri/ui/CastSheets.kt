// Casting to TVs with Chromecast built in: the Chromecast group of the Play on TV sheet the
// player's menu opens, and the notice when the TV is lost or someone else takes it. The cast
// button, its dialogs and the controller are in CastGlyph, CastDialogs and CastController.

package app.kaeri.ui

import android.content.ActivityNotFoundException
import android.content.Intent
import android.net.Uri
import android.provider.Settings
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.engine.ffi.FfiCastRouteHint
import app.kaeri.engine.ffi.FfiCastRow
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.store.CastModel
import app.kaeri.store.TvPlayRequest
import app.kaeri.store.text

// The picker's Chromecast group

/** What a Chromecast row says under its name: whether the stream goes out through this device. */
@Composable
internal fun castRowDetail(row: FfiCastRow): String {
    val tablet = LocalConfiguration.current.smallestScreenWidthDp >= 600
    return when (row.routeHint) {
        FfiCastRouteHint.THROUGH_THIS_DEVICE ->
            stringResource(if (tablet) R.string.cast_sheet_through_tablet else R.string.cast_sheet_through_phone)
        FfiCastRouteHint.DIRECT -> row.model
    }
}

/**
 * The TVs with Chromecast built in, between On this network and Not listed, looked for while the
 * picker is up. Only with an episode to play: a cast plays one, and pairs with nothing.
 */
@Composable
internal fun CastPickerGroup(cast: CastModel, request: TvPlayRequest?) {
    if (request == null || !cast.available) return
    val rows by cast.rows.collectAsStateWithLifecycle()
    val problem by cast.problem.collectAsStateWithLifecycle()
    val colors = MaterialTheme.colorScheme
    DisposableEffect(cast) {
        cast.startBrowsing()
        onDispose { cast.stopBrowsing() }
    }
    if (rows.isEmpty()) return
    TvHeading(stringResource(R.string.cast_sheet_group))
    rows.forEachIndexed { index, row ->
        if (index > 0) TvDivider()
        TvSheetRow(
            row.name,
            castRowDetail(row),
            leading = { CastGlyph(CastGlyphState.Outline, tint = colors.onSurfaceVariant) },
            trailing = { Icon(Icons.Chevron, contentDescription = null, tint = colors.onSurfaceVariant) },
            onClick = { cast.cast(row, request) },
        )
    }
    problem?.let { ProblemLine(it) }
}

/**
 * Android 17's local network permission, turned down: no TV can be found or reached until it is
 * allowed in the app's settings, which Open settings goes to.
 */
@Composable
internal fun CastLocalNetworkRow() {
    val context = LocalContext.current
    Column(Modifier.fillMaxWidth().padding(vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text(stringResource(R.string.cast_sheet_local_network), style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurface)
        Button(
            onClick = {
                try {
                    context.startActivity(
                        Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.fromParts("package", context.packageName, null)) // i18n-skip
                            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
                    )
                } catch (_: ActivityNotFoundException) {
                }
            },
        ) { Text(stringResource(R.string.action_open_settings)) }
    }
}

// The notices

/**
 * The cast button's dialogs, and the notice once a cast ended with something to say after its
 * controller had gone: someone else took the TV, with Play here, or the TV dropped off.
 */
@Composable
internal fun CastSheetHost() {
    val cast = LocalAppStore.current.cast
    val snackbars = LocalSnackbars.current
    val context = LocalContext.current
    LaunchedEffect(cast) {
        cast.ended.collect { ended ->
            val id = ended.id
            val episode = ended.episode
            if (id != null && episode != null) {
                snackbars.show(ended.notice.text(context), Strings.castCardPlayHere(context)) { cast.playHere?.invoke(id, episode) }
            } else {
                snackbars.show(ended.notice.text(context))
            }
        }
    }
    LaunchedEffect(cast) { cast.notes.collect { snackbars.show(it.text(context)) } }
    CastDialogHost()
}
