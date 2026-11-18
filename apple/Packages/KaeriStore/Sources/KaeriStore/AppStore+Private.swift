// Private mode and always-private titles: the commands, and the notice a menu item or a shortcut
// puts up.

import KaeriKit
import KaeriStrings
import Foundation

extension AppStore {
    /// Turn private mode on or off. `notice` puts up the notice that says what it means, with
    /// Turn off: for the menu item and the shortcut, where no description is in view. The
    /// switch never does.
    public func setPrivate(_ on: Bool, notice: Bool = false) {
        privacy.set(on: on)
        send(.setPrivate(on: on))
        if on, notice {
            notices.post(
                Notice(kind: .info, text: L.privateModeOnNotice, hint: nil),
                action: NoticeButton(title: L.actionTurnOff) { [weak self] in self?.setPrivate(false) },
                glyph: "eye.slash", spoken: false)
        } else if !on, let item = notices.current, item.glyph == "eye.slash" {
            notices.dismiss(item)
        }
    }

    /// Keep a show always private on this device, or not.
    public func setAlwaysPrivate(show id: AnilistId, _ on: Bool) {
        privacy.set(show: id, always: on)
        send(.setAlwaysPrivate(title: .show(id: id), on: on))
    }

    /// Keep a series always private on this device, or not.
    public func setAlwaysPrivate(series work: WorkId, _ on: Bool) {
        privacy.set(series: work, always: on)
        send(.setAlwaysPrivate(title: .series(work: work), on: on))
    }

    /// The engine said the mode: once it is off, the notice that said it was on goes.
    func privacyChanged(_ on: Bool) {
        if !on, let item = notices.current, item.glyph == "eye.slash" { notices.dismiss(item) }
    }
}
