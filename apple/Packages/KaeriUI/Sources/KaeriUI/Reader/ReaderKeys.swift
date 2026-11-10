// The reader's keys: one table for the Go menu and for the keys the reader takes itself, so the
// two never disagree.

import KaeriKit
import SwiftUI

/// What a key asks of the reader.
enum ReaderKey: Equatable {
    /// A page; in a strip, a screen less a little overlap.
    case page(forward: Bool)
    /// Up and Down: in a strip a short smooth step, otherwise a page.
    case line(forward: Bool)
    /// Home and End: the chapter's first page or image, or its last.
    case edge(first: Bool)
    case chapter(next: Bool)
}

/// How the reader on screen reads, for its keys.
struct ReaderKeyContext: Equatable {
    var strip: Bool
    var rtl: Bool
}

extension ReaderKeyContext {
    /// A novel turns its pages, whatever mode its chapter had; Vertical turns them too.
    init(novel: Bool, mode: ReaderMode?, direction: ReadDirection?) {
        self.init(strip: !novel && mode == .strip, rtl: direction == .rightToLeft)
    }
}

enum ReaderKeys {
    /// The reader's action for `key` with `modifiers`, or nil when it is not the reader's.
    static func action(_ key: KeyEquivalent, modifiers: EventModifiers, in context: ReaderKeyContext) -> ReaderKey? {
        // Caps Lock, the numeric pad and the function key change nothing here.
        let pressed = modifiers.subtracting([.capsLock, .numericPad, .function])
        switch key {
        case .space where pressed == .shift:
            return .page(forward: false)
        case _ where !pressed.isEmpty:
            return nil
        case .space:
            return .page(forward: true)
        case .pageDown, .pageUp:
            return .page(forward: key == .pageDown)
        case .downArrow, .upArrow:
            return .line(forward: key == .downArrow)
        case .leftArrow, .rightArrow:
            // A strip runs down the screen: the sideways arrows have nothing to turn. On pages they
            // follow the page, not the words: in right to left, Left goes forward.
            if context.strip { return nil }
            return .page(forward: (key == .leftArrow) == context.rtl)
        case .home, .end:
            return .edge(first: key == .home)
        case "]", "[":
            return .chapter(next: key == "]")
        default:
            return nil
        }
    }

    /// The menu's shortcut for Next Page and Previous Page: the arrows on pages, none in a strip.
    static func arrow(forward: Bool, in context: ReaderKeyContext) -> KeyEquivalent? {
        if context.strip { return nil }
        return forward == context.rtl ? .leftArrow : .rightArrow
    }
}
