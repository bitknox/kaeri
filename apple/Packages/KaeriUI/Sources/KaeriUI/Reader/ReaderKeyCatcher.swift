// The reader's keys on iPad. The Go menu's commands hold Space, the brackets and G, but iPadOS
// hands the arrows, Page Up and Down, Home, End and Esc to the responder that holds the
// keyboard first, for its own scrolling and focus moves, and a menu command for one of them
// fires at most until something takes the keyboard. So the reader holds the keyboard itself,
// in a view of its own that takes those presses first, while no sheet and no Go to page is up.

#if os(iOS)
import SwiftUI
import UIKit

struct ReaderKeyCatcher: UIViewRepresentable {
    /// The reader takes the keyboard: it is in front with no sheet or prompt over it.
    let holds: Bool
    /// The reader's answer to a key, true when it took it.
    let press: @MainActor (KeyEquivalent, EventModifiers) -> Bool

    func makeUIView(context: Context) -> CatcherView {
        let view = CatcherView()
        view.press = press
        view.holds = holds
        return view
    }

    func updateUIView(_ view: CatcherView, context: Context) {
        view.press = press
        view.holds = holds
    }

    final class CatcherView: UIView {
        var press: @MainActor (KeyEquivalent, EventModifiers) -> Bool = { _, _ in false }
        /// Taken presses, so their end and cancel go nowhere else either.
        private var taken: Set<UIPress> = []
        var holds = false {
            didSet {
                if holds {
                    claim()
                } else if isFirstResponder {
                    // A sheet or Go to page has the keys now.
                    resignFirstResponder()
                }
            }
        }

        override var canBecomeFirstResponder: Bool { holds }

        override func didMoveToWindow() {
            super.didMoveToWindow()
            claim()
        }

        /// Holds the keyboard once the view is in its window.
        private func claim() {
            takeKeyboard { [weak self] in self?.holds == true }
        }

        override func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
            var rest: Set<UIPress> = []
            for press in presses {
                if let key = press.key, let named = Self.equivalent(key), self.press(named.0, named.1) {
                    taken.insert(press)
                } else {
                    rest.insert(press)
                }
            }
            if !rest.isEmpty { super.pressesBegan(rest, with: event) }
        }

        override func pressesEnded(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
            let rest = presses.subtracting(taken)
            taken.subtract(presses)
            if !rest.isEmpty { super.pressesEnded(rest, with: event) }
        }

        override func pressesCancelled(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
            let rest = presses.subtracting(taken)
            taken.subtract(presses)
            if !rest.isEmpty { super.pressesCancelled(rest, with: event) }
        }

        override func pressesChanged(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
            let rest = presses.subtracting(taken)
            if !rest.isEmpty { super.pressesChanged(rest, with: event) }
        }

        /// The key as SwiftUI names it, for the reader's key table: only the keys iPadOS hands to
        /// the responder holding the keyboard before the menu's commands.
        static func equivalent(_ key: UIKey) -> (KeyEquivalent, EventModifiers)? {
            var modifiers: EventModifiers = []
            if key.modifierFlags.contains(.shift) { modifiers.insert(.shift) }
            if key.modifierFlags.contains(.command) { modifiers.insert(.command) }
            if key.modifierFlags.contains(.alternate) { modifiers.insert(.option) }
            if key.modifierFlags.contains(.control) { modifiers.insert(.control) }
            let equivalent: KeyEquivalent
            switch key.keyCode {
            case .keyboardLeftArrow: equivalent = .leftArrow
            case .keyboardRightArrow: equivalent = .rightArrow
            case .keyboardUpArrow: equivalent = .upArrow
            case .keyboardDownArrow: equivalent = .downArrow
            case .keyboardPageUp: equivalent = .pageUp
            case .keyboardPageDown: equivalent = .pageDown
            case .keyboardHome: equivalent = .home
            case .keyboardEnd: equivalent = .end
            case .keyboardEscape: equivalent = .escape
            // Space, the brackets and G reach the Go menu's commands, which take them first.
            default: return nil
            }
            return (equivalent, modifiers)
        }
    }
}

extension UIView {
    /// Takes the keyboard once the view is in its window, while `wanted` says so. UIKit turns a
    /// responder down while the screen bringing it in is still coming in, so during a
    /// presentation the view asks as it ends, and a view turned down anyway asks again for a
    /// little while.
    func takeKeyboard(while wanted: @escaping @MainActor () -> Bool) {
        guard window != nil, wanted(), !isFirstResponder else { return }
        if let coming = owningController?.transitionCoordinator {
            coming.animate(alongsideTransition: nil) { [weak self] _ in
                MainActor.assumeIsolated { self?.askForKeyboard(while: wanted, left: 10) }
            }
        } else {
            askForKeyboard(while: wanted, left: 10)
        }
    }

    private func askForKeyboard(while wanted: @escaping @MainActor () -> Bool, left: Int) {
        guard window != nil, wanted(), !isFirstResponder else { return }
        if becomeFirstResponder() || left == 0 { return }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { [weak self] in
            self?.askForKeyboard(while: wanted, left: left - 1)
        }
    }

    /// The view controller whose view holds this one.
    private var owningController: UIViewController? {
        var responder = next
        while let current = responder {
            if let controller = current as? UIViewController { return controller }
            responder = current.next
        }
        return nil
    }
}
#endif
