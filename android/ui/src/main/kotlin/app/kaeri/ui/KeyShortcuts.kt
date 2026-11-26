// The app's keyboard shortcuts on a phone or tablet with a keyboard: Ctrl with a number goes to
// that section of the navigation bar, Ctrl+F to Search, Ctrl+, to Settings and Ctrl+[ back, as the
// main window's keys on the other apps. Meta + / lists them, in the system's own sheet.

package app.kaeri.ui

import android.content.Context
import android.view.KeyEvent
import android.view.KeyboardShortcutGroup
import android.view.KeyboardShortcutInfo
import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.i18n.R

/** What a shortcut asks for. */
sealed interface KeyShortcut {
    /** The navigation bar's [index]th section, from 0. */
    data class Section(val index: Int) : KeyShortcut

    data object Search : KeyShortcut

    data object Settings : KeyShortcut

    data object Back : KeyShortcut
}

/** The shortcut a key pressed with Ctrl alone stands for, or null. */
fun keyShortcut(keyCode: Int, ctrl: Boolean, alt: Boolean = false, shift: Boolean = false, meta: Boolean = false): KeyShortcut? {
    if (!ctrl || alt || shift || meta) return null
    return when (keyCode) {
        in KeyEvent.KEYCODE_1..KeyEvent.KEYCODE_9 -> KeyShortcut.Section(keyCode - KeyEvent.KEYCODE_1)
        KeyEvent.KEYCODE_F -> KeyShortcut.Search
        KeyEvent.KEYCODE_COMMA -> KeyShortcut.Settings
        KeyEvent.KEYCODE_LEFT_BRACKET -> KeyShortcut.Back
        else -> null
    }
}

/** The shortcuts as the system's list shows them, the sections named as the bar names them for [answer]. */
fun keyShortcutGroup(context: Context, title: String, answer: KaeriIsFor): KeyboardShortcutGroup {
    val ctrl = KeyEvent.META_CTRL_ON
    val sections = barDestinations(answer).take(9).mapIndexed { index, destination ->
        KeyboardShortcutInfo(context.getString(destination.labelFor(answer)), KeyEvent.KEYCODE_1 + index, ctrl)
    }
    return KeyboardShortcutGroup(
        title,
        sections + listOf(
            KeyboardShortcutInfo(context.getString(R.string.nav_search), KeyEvent.KEYCODE_F, ctrl),
            KeyboardShortcutInfo(context.getString(R.string.nav_settings), KeyEvent.KEYCODE_COMMA, ctrl),
            KeyboardShortcutInfo(context.getString(R.string.action_back), KeyEvent.KEYCODE_LEFT_BRACKET, ctrl),
        ),
    )
}

/** Acts on [shortcut] for the app on screen, with [answer]'s navigation bar. */
internal suspend fun Navigator.follow(shortcut: KeyShortcut, answer: KaeriIsFor, store: app.kaeri.store.AppStore, actions: AppActions, openSearch: () -> Unit) {
    when (shortcut) {
        is KeyShortcut.Section -> barDestinations(answer).getOrNull(shortcut.index)?.let(::go)
        KeyShortcut.Search -> follow(AppLink.Search, store, actions, openSearch)
        KeyShortcut.Settings -> openSettings()
        KeyShortcut.Back -> back()
    }
}
