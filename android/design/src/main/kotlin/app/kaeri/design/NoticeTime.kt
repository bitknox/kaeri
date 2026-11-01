// How long a notice stays: the system's own answer. Android 10 and later keep a "time to take
// action" setting that accessibility services and the user can raise, and
// `getRecommendedTimeoutMillis` applies it. A notice with a button asks for the controls flag, so
// it is never shorter than the user's setting.

package app.kaeri.design

import android.content.Context
import android.view.accessibility.AccessibilityManager

/** [millis], or longer when the user's accessibility timeout is longer. */
fun recommendedNoticeMillis(context: Context, millis: Long, hasAction: Boolean): Long {
    val manager = context.getSystemService(AccessibilityManager::class.java) ?: return millis
    val flags = if (hasAction) {
        AccessibilityManager.FLAG_CONTENT_TEXT or AccessibilityManager.FLAG_CONTENT_CONTROLS
    } else {
        AccessibilityManager.FLAG_CONTENT_TEXT
    }
    return manager.getRecommendedTimeoutMillis(millis.coerceAtMost(Int.MAX_VALUE.toLong()).toInt(), flags).toLong()
}
