// The permission to post notifications: a runtime permission asked for from Android 13, and
// granted with the install before it, where the system's switch for the app is the only say.

package app.kaeri.app

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import androidx.annotation.ChecksSdkIntAtLeast
import androidx.annotation.RequiresApi
import androidx.core.content.ContextCompat

object NotificationPermission {
    /** Android 13, where posting a notification takes a permission the person gives. */
    const val RUNTIME_SDK = Build.VERSION_CODES.TIRAMISU

    /** Whether this Android asks for the permission: below 13 there is nothing to ask. */
    @get:ChecksSdkIntAtLeast(api = RUNTIME_SDK)
    val askable: Boolean get() = askable(Build.VERSION.SDK_INT)

    /** The permission asked for, on an Android that has it. */
    @get:RequiresApi(RUNTIME_SDK)
    val name: String get() = Manifest.permission.POST_NOTIFICATIONS

    /** Whether the app holds the permission: always below Android 13, which grants it with the install. */
    fun granted(context: Context): Boolean = granted(Build.VERSION.SDK_INT) {
        Build.VERSION.SDK_INT >= RUNTIME_SDK &&
            ContextCompat.checkSelfPermission(context, name) == PackageManager.PERMISSION_GRANTED
    }

    internal fun askable(sdk: Int): Boolean = sdk >= RUNTIME_SDK

    internal fun granted(sdk: Int, held: () -> Boolean): Boolean = !askable(sdk) || held()
}
