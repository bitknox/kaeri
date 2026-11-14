// Quiet the phone: while the reader is open, notifications wait. The app owns an automatic Do Not
// Disturb rule and asks the person once for access to Do Not Disturb. Alarms and repeat callers
// still ring. Android 15 builds the rule with its Builder, as the immersive kind; Android 11 to
// 14 with the constructor, which wants the app's own activity named as the rule's settings. The
// rule's settings link opens the reader's Paper mode page. The rule's id is kept, so a rule a
// crash or a killed process left on is removed at the next start. Without access nothing changes.

package app.kaeri.reader.phone

import android.app.AutomaticZenRule
import android.app.NotificationManager
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.service.notification.Condition
import android.service.notification.ZenPolicy
import app.kaeri.i18n.Strings

class QuietPhone(private val context: Context) {
    private val manager get() = context.getSystemService(NotificationManager::class.java)
    private val prefs get() = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
    private var previousHandler: Thread.UncaughtExceptionHandler? = null

    /** Whether the app may change Do Not Disturb. */
    val allowed: Boolean get() = manager?.isNotificationPolicyAccessGranted == true

    /** Whether this Android has the rule the setting uses: every one the app runs on. */
    val supported: Boolean get() = true

    /** Whether the person was told once that notifications wait. */
    var noticeSeen: Boolean
        get() = prefs.getBoolean(SEEN, false)
        set(value) = prefs.edit().putBoolean(SEEN, value).apply()

    /** The system screen where the person grants access. */
    fun accessIntent(): Intent = Intent(android.provider.Settings.ACTION_NOTIFICATION_POLICY_ACCESS_SETTINGS)
        .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)

    /** Turn the quiet on or off. Does nothing, quietly, without access. */
    fun set(on: Boolean) {
        if (!supported || !allowed) return
        runCatching { if (on) start() else stop() }
    }

    /** Remove the rule a process that died while reading left on. Run at every start. */
    fun clearStale() {
        if (!supported || !allowed) {
            prefs.edit().remove(RULE).apply()
            return
        }
        runCatching { stop() }
    }

    private fun start() {
        if (prefs.getString(RULE, null) != null) return
        val manager = manager ?: return
        // Silence everything but alarms and a caller who rings twice within 15 minutes; a ZenPolicy
        // that only disallows sounds would silence those too.
        val policy = ZenPolicy.Builder()
            .disallowAllSounds()
            .allowAlarms(true)
            .allowRepeatCallers(true)
            .hideAllVisualEffects()
            .build()
        val main = context.packageManager.getLaunchIntentForPackage(context.packageName)?.component
            ?: ComponentName(context.packageName, context.packageName)
        val name = Strings.readerQuietRule(context)
        val condition = Uri.parse("kaeri://reader")
        val rule = if (Build.VERSION.SDK_INT >= 35) {
            val builder = AutomaticZenRule.Builder(name, condition)
                .setInterruptionFilter(NotificationManager.INTERRUPTION_FILTER_PRIORITY)
                .setZenPolicy(policy)
                .setConfigurationActivity(main)
                .setType(AutomaticZenRule.TYPE_IMMERSIVE)
                .setTriggerDescription(Strings.readerQuietTrigger(context))
                .setEnabled(true)
            if (context.applicationInfo.icon != 0) builder.setIconResId(context.applicationInfo.icon)
            builder.build()
        } else {
            AutomaticZenRule(name, null, main, condition, policy, NotificationManager.INTERRUPTION_FILTER_PRIORITY, true)
        }
        val id = manager.addAutomaticZenRule(rule)
        prefs.edit().putString(RULE, id).apply()
        manager.setAutomaticZenRuleState(id, Condition(rule.conditionId, "", Condition.STATE_TRUE))
        guardAgainstCrash()
    }

    /** The app's own launcher activity, which Android 14 and below want a rule to name. */
    private fun launcher(): ComponentName? = context.packageManager.getLaunchIntentForPackage(context.packageName)?.component

    private fun stop() {
        val id = prefs.getString(RULE, null) ?: return
        prefs.edit().remove(RULE).apply()
        runCatching { manager?.removeAutomaticZenRule(id) }
        previousHandler?.let { Thread.setDefaultUncaughtExceptionHandler(it) }
        previousHandler = null
    }

    /** A crash while the rule is on ends it before the process goes. */
    private fun guardAgainstCrash() {
        if (previousHandler != null) return
        val before = Thread.getDefaultUncaughtExceptionHandler()
        previousHandler = before
        Thread.setDefaultUncaughtExceptionHandler { thread, error ->
            runCatching { prefs.getString(RULE, null)?.let { manager?.removeAutomaticZenRule(it) } }
            prefs.edit().remove(RULE).commit()
            before?.uncaughtException(thread, error)
        }
    }

    private companion object {
        const val PREFS = "kaeri_reader_quiet"
        const val RULE = "rule"
        const val SEEN = "notice_seen"
    }
}
