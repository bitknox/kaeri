// The picture on a SurfaceView, copied for the player panel's held picture: mpv clears its picture
// as a switch loads, and a SurfaceView cannot be drawn into a bitmap any other way.

package app.kaeri.player

import android.graphics.Bitmap
import android.os.Handler
import android.os.Looper
import android.view.PixelCopy
import android.view.SurfaceView
import androidx.core.graphics.createBitmap
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlin.coroutines.resume

object FrameCopy {
    /** [view]'s picture now, or null when it has no surface or the copy failed. */
    suspend fun of(view: SurfaceView): Bitmap? {
        val surface = view.holder?.surface
        if (surface == null || !surface.isValid || view.width == 0 || view.height == 0) return null
        val bitmap = createBitmap(view.width, view.height)
        return suspendCancellableCoroutine { done ->
            try {
                PixelCopy.request(view, bitmap, { result ->
                    if (result != PixelCopy.SUCCESS) android.util.Log.w("player", "the picture could not be copied: $result")
                    if (done.isActive) done.resume(bitmap.takeIf { result == PixelCopy.SUCCESS })
                }, Handler(Looper.getMainLooper()))
            } catch (e: IllegalArgumentException) {
                // The surface went between the check and the copy.
                if (done.isActive) done.resume(null)
            }
        }
    }
}
