// What the player asks of the device: the power it runs on, and the screen it plays on. And what
// the device says of itself to the engine, for skip times and trailers.

package app.kaeri.player

import android.accessibilityservice.AccessibilityServiceInfo
import android.app.ActivityManager
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.database.ContentObserver
import android.os.BatteryManager
import android.os.Handler
import android.os.Looper
import android.os.PowerManager
import android.provider.Settings
import android.view.accessibility.AccessibilityManager
import android.view.Display
import app.kaeri.engine.model.Command

object Device {
    /** On battery, and whether power saver is on: the upscaling chain starts a step lighter for either. */
    fun power(context: Context): PowerState {
        val battery = context.registerReceiver(null, IntentFilter(Intent.ACTION_BATTERY_CHANGED))
        val plugged = (battery?.getIntExtra(BatteryManager.EXTRA_PLUGGED, 0) ?: 0) != 0
        val saver = context.getSystemService(PowerManager::class.java)?.isPowerSaveMode ?: false
        return PowerState(onBattery = battery != null && !plugged, lowPower = saver)
    }

    /**
     * The display as the engine's upscaling reads it: its physical pixels the long way round, as
     * the phone's player is always landscape, with its density and refresh rate.
     */
    fun display(display: Display, density: Float): DisplayInfo {
        val mode = display.mode
        val (w, h) = mode.physicalWidth to mode.physicalHeight
        return DisplayInfo(maxOf(w, h), minOf(w, h), density.toDouble(), mode.refreshRate.toDouble().takeIf { it > 0 })
    }
}

/**
 * Tells the engine what the device is doing that finding skip times and trailers wait for:
 * battery saver on a phone, Remove animations, a hot device, a screen reader and a low-memory
 * device. It says so now and each time one of them changes. A TV has no battery saver, so it says
 * low power is never on and the engine pauses finding on storage alone. Data Saver goes with the
 * network path instead.
 */
class DeviceWatch(private val context: Context, private val send: (Command) -> Unit, private val hasSaver: Boolean = true) {
    private var said: Command.DeviceState? = null

    private fun read(): Command.DeviceState {
        val power = context.getSystemService(PowerManager::class.java)
        val accessibility = context.getSystemService(AccessibilityManager::class.java)
        val memory = context.getSystemService(ActivityManager::class.java)
        val info = ActivityManager.MemoryInfo().also { memory?.getMemoryInfo(it) }
        // Remove animations sets the animator scale to zero.
        val scale = Settings.Global.getFloat(context.contentResolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f)
        return Command.DeviceState(
            reduceMotion = scale == 0f,
            lowPower = hasSaver && (power?.isPowerSaveMode ?: false),
            thermalSerious = (power?.currentThermalStatus ?: PowerManager.THERMAL_STATUS_NONE) >= PowerManager.THERMAL_STATUS_SEVERE,
            screenReader = accessibility?.isEnabled == true &&
                accessibility.getEnabledAccessibilityServiceList(AccessibilityServiceInfo.FEEDBACK_SPOKEN).isNotEmpty(),
            lowMemory = memory?.isLowRamDevice == true || info.totalMem in 1 until FOUR_GB_REPORTED,
        )
    }

    private fun say() {
        val now = read()
        if (now == said) return
        said = now
        send(now)
    }

    /** Say it now, and again at every change. Call once, from the application. */
    fun start() {
        say()
        val main = Handler(Looper.getMainLooper())
        if (hasSaver) {
            context.registerReceiver(
                object : android.content.BroadcastReceiver() {
                    override fun onReceive(context: Context, intent: Intent) = say()
                },
                IntentFilter(PowerManager.ACTION_POWER_SAVE_MODE_CHANGED),
            )
        }
        context.getSystemService(PowerManager::class.java)?.addThermalStatusListener(context.mainExecutor) { say() }
        context.getSystemService(AccessibilityManager::class.java)?.addAccessibilityStateChangeListener { say() }
        context.contentResolver.registerContentObserver(
            Settings.Global.getUriFor(Settings.Global.ANIMATOR_DURATION_SCALE),
            false,
            object : ContentObserver(main) {
                override fun onChange(selfChange: Boolean) = say()
            },
        )
    }

    private companion object {
        /** Under 4 GB, as the system reports memory: a 4 GB phone reports about 3.7. */
        const val FOUR_GB_REPORTED = 3_500L * 1024 * 1024
    }
}
