// Quiet the phone: while the reader is open, notifications wait. Android 15 lets an app turn on
// an automatic Do Not Disturb rule that only dims the device's interruptions, and asks the
// person once for access to Do Not Disturb. Without that access nothing changes and the setting
// row says what to grant.

package app.kaeri.reader.phone

import android.app.AutomaticZenRule
import android.app.NotificationManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.service.notification.ZenPolicy

class QuietPhone(private val context: Context) {
    private val manager get() = context.getSystemService(NotificationManager::class.java)
    private var ruleId: String? = null

    /** Whether the app may change Do Not Disturb. */
    val allowed: Boolean get() = manager.isNotificationPolicyAccessGranted

    /** Whether this Android has the rule the setting uses. */
    val supported: Boolean get() = Build.VERSION.SDK_INT >= 28

    /** The system screen where the person grants access. */
    fun accessIntent(): Intent = Intent(android.provider.Settings.ACTION_NOTIFICATION_POLICY_ACCESS_SETTINGS)
        .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)

    /** Turn the quiet on or off. Does nothing without access. */
    fun set(on: Boolean) {
        if (!supported || !allowed) return
        runCatching { if (on) start() else stop() }
    }

    private fun start() {
        if (ruleId != null) return
        val policy = ZenPolicy.Builder()
            .disallowAllSounds()
            .hideAllVisualEffects()
            .build()
        val rule = AutomaticZenRule.Builder("Reader", Uri.parse("kaeri://reader"))
            .setInterruptionFilter(NotificationManager.INTERRUPTION_FILTER_PRIORITY)
            .setZenPolicy(policy)
            .setEnabled(true)
            .build()
        val id = manager.addAutomaticZenRule(rule)
        ruleId = id
        manager.setAutomaticZenRuleState(
            id,
            android.service.notification.Condition(rule.conditionId, "", android.service.notification.Condition.STATE_TRUE),
        )
    }

    private fun stop() {
        val id = ruleId ?: return
        ruleId = null
        manager.removeAutomaticZenRule(id)
    }
}
