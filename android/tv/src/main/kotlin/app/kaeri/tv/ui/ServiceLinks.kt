// A title on a licensed service, on the TV: Watch on <service> sends a VIEW intent with the URL. If
// no app handles it, a panel says so and shows the address as a QR code for a phone.

package app.kaeri.tv.ui

import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import app.kaeri.engine.ffi.FfiLinkSent
import app.kaeri.engine.ffi.FfiTvRemote
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import app.kaeri.design.ObiSilk
import app.kaeri.engine.core.AnilistId
import app.kaeri.engine.model.ServiceLink
import app.kaeri.i18n.R
import app.kaeri.store.AppStore
import app.kaeri.tv.remote.QrCode

/**
 * Open [link] (or [url], an episode's own) in whatever app handles it. The engine is told, so
 * coming back offers Mark as watched. [host] shows the not-installed panel when nothing handles it.
 */
fun openOnTv(context: Context, store: AppStore, host: ActionsHost?, id: AnilistId, link: ServiceLink, episode: Int? = null, url: String = link.url, remote: FfiTvRemote? = null) {
    store.openedService(id, link.service, url, episode)
    val intent = Intent(Intent.ACTION_VIEW, Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    try {
        context.startActivity(intent)
    } catch (_: ActivityNotFoundException) {
        host?.open(notInstalledPanel(context, link.service, url, remote?.takeIf { it.hasPairedPhone() }?.let { r -> { link: String -> r.sendLink(link) } }))
    }
}

/**
 * "Hikari Library isn't installed on this TV." with the address as a QR code, "Open on your phone",
 * and Send to phone beside it when a phone is paired: [send] hands the link to the paired phones.
 */
fun notInstalledPanel(context: Context, service: String, url: String, send: ((String) -> FfiLinkSent)? = null): ActionSheet =
    ActionSheet(
        title = context.getString(R.string.licensed_tv_not_installed, service),
        body = {
            var note by remember { mutableStateOf<String?>(null) }
            val first = remember { FocusRequester() }
            Column(Modifier.padding(start = 16.dp, top = 8.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                QrCode(url, label = stringResource(R.string.licensed_tv_open_phone))
                Text(stringResource(R.string.licensed_tv_open_phone), style = MaterialTheme.typography.labelLarge, color = ObiSilk.Haze, modifier = Modifier.widthIn(max = 320.dp))
                if (send != null) {
                    TonalButton(onClick = { note = sentWords(context, send(url)) }, modifier = Modifier.focusRequester(first)) {
                        ButtonLabel(stringResource(R.string.licensed_tv_send))
                    }
                    note?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, modifier = Modifier.widthIn(max = 320.dp)) }
                    LaunchedEffect(Unit) {
                        repeat(5) {
                            withFrameNanos { }
                            if (first.tryFocus()) return@LaunchedEffect
                        }
                    }
                }
            }
        },
    )

/** What became of a link sent to the phone, in words. */
fun sentWords(context: Context, sent: FfiLinkSent): String = when (sent) {
    is FfiLinkSent.Sent -> context.getString(R.string.licensed_tv_sent)
    FfiLinkSent.Held -> context.getString(R.string.licensed_tv_held)
    FfiLinkSent.Refused -> context.getString(R.string.licensed_tv_send_failed)
}
