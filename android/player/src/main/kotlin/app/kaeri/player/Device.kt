// What the player asks of the device: the power it runs on, and the screen it plays on.

package app.kaeri.player

import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.os.BatteryManager
import android.os.PowerManager
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
 * Tells the engine what the device is doing that finding skip times waits for: battery saver, on
 * a phone. It says so now and each time the system's power saving changes. A TV has no battery
 * saver, so it says [lowPower] is never on and the engine pauses finding on storage alone.
 */
class DeviceWatch(private val context: Context, private val send: (Command) -> Unit, private val hasSaver: Boolean = true) {
    private fun say() {
        val saver = hasSaver && (context.getSystemService(PowerManager::class.java)?.isPowerSaveMode ?: false)
        send(Command.DeviceState(lowPower = saver))
    }

    /** Say it now, and again at every change. Call once, from the application. */
    fun start() {
        say()
        if (!hasSaver) return
        context.registerReceiver(
            object : android.content.BroadcastReceiver() {
                override fun onReceive(context: Context, intent: Intent) = say()
            },
            IntentFilter(PowerManager.ACTION_POWER_SAVE_MODE_CHANGED),
        )
    }
}
