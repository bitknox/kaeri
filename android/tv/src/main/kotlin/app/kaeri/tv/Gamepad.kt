// A game controller's keys, as Settings' Controller group has them: on or off, and A and B swapped.

package app.kaeri.tv

import android.view.InputDevice
import android.view.KeyEvent
import app.kaeri.engine.model.SettingKey
import app.kaeri.store.SettingsCatalog
import app.kaeri.store.SettingsModel

/**
 * What the TV does with a key from a game controller. Android already turns the controller's A
 * into the D-pad centre and B into Back; this only stops the controller when it is switched off,
 * and swaps A and B for a controller laid out as Nintendo's are. The remote, and a paired phone's
 * presses, never count as a controller.
 */
internal object Gamepad {
    /** What becomes of one key. */
    sealed interface Fate {
        /** Goes on as it came. */
        data object Same : Fate

        /** Dropped: the controller is switched off. */
        data object Ignored : Fate

        /** Goes on as the key [code], which does what the other face button does. */
        data class As(val code: Int) : Fate
    }

    /** The Controller settings as they stand. Before the engine has said them, the defaults: on, not swapped. */
    data class Settings(val enabled: Boolean, val swapFace: Boolean) {
        companion object {
            fun of(settings: SettingsModel): Settings = Settings(
                enabled = settings.row(SettingKey.CONTROLLER_INPUT)?.let(SettingsCatalog::isOn) ?: true,
                swapFace = settings.isOn(SettingKey.SWAP_FACE_BUTTONS),
            )
        }
    }

    /** What becomes of the key [code] from a controller ([controller]) or anything else. */
    fun fate(code: Int, controller: Boolean, settings: Settings): Fate = when {
        !controller -> Fate.Same
        !settings.enabled -> Fate.Ignored
        !settings.swapFace -> Fate.Same
        code == KeyEvent.KEYCODE_BUTTON_A -> Fate.As(KeyEvent.KEYCODE_BACK)
        code == KeyEvent.KEYCODE_BUTTON_B -> Fate.As(KeyEvent.KEYCODE_DPAD_CENTER)
        else -> Fate.Same
    }

    /** Whether [device] is a game controller: Android numbers each one, and nothing else. */
    fun isController(device: InputDevice?): Boolean = device != null && device.controllerNumber > 0

    /** [event] as the key [code], everything else about it kept. */
    fun retyped(event: KeyEvent, code: Int): KeyEvent = KeyEvent(
        event.downTime, event.eventTime, event.action, code, event.repeatCount, event.metaState,
        event.deviceId, event.scanCode, event.flags, event.source,
    )
}
