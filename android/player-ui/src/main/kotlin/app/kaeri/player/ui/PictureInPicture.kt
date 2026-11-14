// Picture in Picture, as motion-android.md has it: the system's window and its motion, and ours
// only the parameters, set again whenever one of them changes, and the three actions. Android 12
// enters the window by itself on the way home (auto-enter); on Android 11 the activity enters it
// as the person leaves (PictureInPictureOnLeave), with the same parameters.

package app.kaeri.player.ui

import app.kaeri.i18n.Strings
import android.app.Activity
import android.app.PendingIntent
import android.app.PictureInPictureParams
import android.app.RemoteAction
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.graphics.Rect
import android.graphics.drawable.Icon
import android.os.Build
import android.util.Rational
import kotlin.math.roundToInt
import androidx.activity.compose.LocalActivity
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.platform.LocalContext
import androidx.core.content.ContextCompat
import app.kaeri.player.PlayerModel
import app.kaeri.player.ui.R

/**
 * The window's parameters while the player is up: the video's aspect, its bounds in the window as
 * the source-rect hint, auto-enter only while playing, seamless resize, and back 10 s, play or
 * pause, and forward 10 s. Auto-enter goes off with the player.
 */
@Composable
internal fun PictureInPicture(model: PlayerModel, videoBounds: Rect?) {
    val activity = LocalActivity.current ?: return
    val context = LocalContext.current
    val playing = model.hasPicture && !model.paused
    DisposableEffect(context) {
        val receiver = object : BroadcastReceiver() {
            override fun onReceive(context: Context, intent: Intent) {
                when (intent.getStringExtra(EXTRA)) {
                    BACK -> model.seekBy(-10.0)
                    PLAY_PAUSE -> model.togglePause()
                    FORWARD -> model.seekBy(10.0)
                }
            }
        }
        ContextCompat.registerReceiver(context, receiver, IntentFilter(ACTION), ContextCompat.RECEIVER_NOT_EXPORTED)
        onDispose {
            context.unregisterReceiver(receiver)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                activity.setPictureInPictureParams(PictureInPictureParams.Builder().setAutoEnterEnabled(false).build())
            }
            PictureInPictureOnLeave.wanted = false
        }
    }
    LaunchedEffect(videoBounds, playing, model.aspect) {
        val params = PictureInPictureParams.Builder()
            .setAspectRatio(pipAspectRatio(model.aspect))
            .setActions(
                listOf(
                    action(context, R.drawable.pip_back, Strings.playerBack10(context), BACK, 1),
                    if (playing) action(context, R.drawable.pip_pause, Strings.actionPause(context), PLAY_PAUSE, 2)
                    else action(context, R.drawable.pip_play, Strings.actionPlay(context), PLAY_PAUSE, 2),
                    action(context, R.drawable.pip_forward, Strings.playerForward10(context), FORWARD, 3),
                ),
            )
        videoBounds?.takeIf { !it.isEmpty }?.let(params::setSourceRectHint)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            params.setAutoEnterEnabled(playing).setSeamlessResizeEnabled(true)
        }
        activity.setPictureInPictureParams(params.build())
        PictureInPictureOnLeave.wanted = playing
    }
}

/**
 * Auto-enter where Android has none, Android 11: the activity calls [enter] from
 * `onUserLeaveHint`, as the person goes home or to another app, and the window opens while a
 * video plays, with the parameters the player last set. From Android 12 the system does this.
 */
object PictureInPictureOnLeave {
    /** Whether the player is up and playing, which is when leaving opens the window. */
    @Volatile
    internal var wanted = false

    /**
     * Enter the window as the person leaves [activity], where the system would not: [inPip] is
     * told true just before, so the overlays go first, and false again if the window did not open.
     */
    fun enter(activity: Activity, inPip: (Boolean) -> Unit) {
        if (!shouldEnter(Build.VERSION.SDK_INT, wanted)) return
        inPip(true)
        // Entering adds nothing to the parameters set last, which the window takes.
        val entered = runCatching { activity.enterPictureInPictureMode(PictureInPictureParams.Builder().build()) }.getOrDefault(false)
        if (!entered) inPip(false)
    }

    internal fun shouldEnter(sdk: Int, wanted: Boolean): Boolean = sdk < Build.VERSION_CODES.S && wanted
}

private fun action(context: Context, icon: Int, title: String, what: String, code: Int): RemoteAction {
    val intent = Intent(ACTION).setPackage(context.packageName).putExtra(EXTRA, what)
    val pending = PendingIntent.getBroadcast(context, code, intent, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
    return RemoteAction(Icon.createWithResource(context, icon), title, title, pending)
}

private const val ACTION = "app.kaeri.player.PIP_ACTION"
private const val EXTRA = "action"
private const val BACK = "back"
private const val PLAY_PAUSE = "play-pause"
private const val FORWARD = "forward"

/**
 * The ratio the system's window takes, 1 : 2.39 to 2.39 : 1, as an exact [Rational] at the edges.
 * Truncating a double (418/1000) lands below the minimum and throws.
 */
internal fun pipAspectRatio(aspect: Double?): Rational {
    val wanted = aspect?.takeIf { it.isFinite() && it > 0.0 } ?: (16.0 / 9.0)
    return when {
        wanted <= 1 / 2.39 -> Rational(100, 239)
        wanted >= 2.39 -> Rational(239, 100)
        else -> Rational((wanted * 10_000).roundToInt(), 10_000)
    }.let { if (it.toDouble() < 100.0 / 239.0) Rational(100, 239) else if (it.toDouble() > 2.39) Rational(239, 100) else it }
}
