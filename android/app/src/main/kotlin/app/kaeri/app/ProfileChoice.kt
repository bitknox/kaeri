// The profile the engine starts in: kept between launches, and changed by relaunching.

package app.kaeri.app

import android.content.Context
import android.content.Intent
import android.os.Process

internal object ProfileChoice {
    private const val FILE = "profile"
    private const val KEY = "id"

    /** The profile last switched to, or null for the one used last. */
    fun saved(context: Context): String? =
        context.getSharedPreferences(FILE, Context.MODE_PRIVATE).getString(KEY, null)

    /** Remember [id] and start the app again on it: the engine runs on one profile's data at a time. */
    fun switchTo(context: Context, id: String) {
        context.getSharedPreferences(FILE, Context.MODE_PRIVATE).edit().putString(KEY, id).commit()
        // The relaunch is the answer to Who's watching?, which does not ask again.
        app.kaeri.store.DevicePreferences.of(context).edit().putBoolean("profileSwitched", true).commit()
        val launch = context.packageManager.run { getLaunchIntentForPackage(context.packageName) ?: getLeanbackLaunchIntentForPackage(context.packageName) } ?: return
        launch.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK)
        context.startActivity(launch)
        Process.killProcess(Process.myPid())
    }
}
