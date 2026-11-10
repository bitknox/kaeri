// The reader's keys in every mode: what each key asks of the reader, through the one table the Go
// menu and the reader's own key handling share.

import KaeriKit
@testable import KaeriUI
import SwiftUI
import Testing

struct ReaderKeysTests {
    let rightToLeft = ReaderKeyContext(novel: false, mode: .paged, direction: .rightToLeft)
    let leftToRight = ReaderKeyContext(novel: false, mode: .paged, direction: .leftToRight)
    let strip = ReaderKeyContext(novel: false, mode: .strip, direction: .topToBottom)
    let vertical = ReaderKeyContext(novel: false, mode: .vertical, direction: .topToBottom)
    let novel = ReaderKeyContext(novel: true, mode: .scrolled, direction: .leftToRight)

    func press(_ key: KeyEquivalent, _ modifiers: EventModifiers = [], in context: ReaderKeyContext) -> ReaderKey? {
        ReaderKeys.action(key, modifiers: modifiers, in: context)
    }

    @Test func rightToLeftPagesTurnForwardOnLeft() {
        #expect(press(.leftArrow, in: rightToLeft) == .page(forward: true))
        #expect(press(.rightArrow, in: rightToLeft) == .page(forward: false))
        #expect(ReaderKeys.arrow(forward: true, in: rightToLeft) == .leftArrow)
        #expect(ReaderKeys.arrow(forward: false, in: rightToLeft) == .rightArrow)
    }

    @Test func leftToRightPagesTurnForwardOnRight() {
        #expect(press(.rightArrow, in: leftToRight) == .page(forward: true))
        #expect(press(.leftArrow, in: leftToRight) == .page(forward: false))
        #expect(ReaderKeys.arrow(forward: true, in: leftToRight) == .rightArrow)
    }

    @Test func upAndDownStepInEveryMode() {
        for context in [rightToLeft, leftToRight, strip, vertical, novel] {
            #expect(press(.downArrow, in: context) == .line(forward: true))
            #expect(press(.upArrow, in: context) == .line(forward: false))
        }
    }

    @Test func spaceAndThePageKeysGoAScreenInEveryMode() {
        for context in [rightToLeft, leftToRight, strip, vertical, novel] {
            #expect(press(.space, in: context) == .page(forward: true))
            #expect(press(.space, .shift, in: context) == .page(forward: false))
            #expect(press(.pageDown, in: context) == .page(forward: true))
            #expect(press(.pageUp, in: context) == .page(forward: false))
            #expect(press(.home, in: context) == .edge(first: true))
            #expect(press(.end, in: context) == .edge(first: false))
            #expect(press("]", in: context) == .chapter(next: true))
            #expect(press("[", in: context) == .chapter(next: false))
        }
    }

    @Test func aStripLeavesTheSidewaysArrowsAlone() {
        #expect(press(.leftArrow, in: strip) == nil)
        #expect(press(.rightArrow, in: strip) == nil)
        #expect(ReaderKeys.arrow(forward: true, in: strip) == nil)
    }

    @Test func verticalPagesAndANovelTurnOnTheArrows() {
        #expect(press(.rightArrow, in: vertical) == .page(forward: true))
        #expect(press(.leftArrow, in: vertical) == .page(forward: false))
        #expect(press(.rightArrow, in: novel) == .page(forward: true))
        // A novel's chapter can carry a strip's mode; it still turns pages.
        #expect(ReaderKeyContext(novel: true, mode: .strip, direction: .leftToRight).strip == false)
    }

    @Test func aKeyWithACommandIsNotTheReaders() {
        #expect(press(.rightArrow, .command, in: leftToRight) == nil)
        #expect(press(.space, .option, in: strip) == nil)
        #expect(press("[", .command, in: strip) == nil)
        // Caps Lock and the arrows' own function and keypad flags change nothing.
        #expect(press(.downArrow, [.function, .numericPad, .capsLock], in: strip) == .line(forward: true))
    }

    @Test func otherKeysAreLeftToTheWindow() {
        #expect(press("a", in: leftToRight) == nil)
        #expect(press(.escape, in: strip) == nil)
        #expect(press(.return, in: novel) == nil)
    }
}
