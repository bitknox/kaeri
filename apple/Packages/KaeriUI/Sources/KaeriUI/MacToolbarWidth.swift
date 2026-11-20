// One width for a Mac toolbar item, in the bar and in its overflow menu alike.

import Foundation
import Synchronization
import SwiftUI

/// A toolbar item's content at the size it has had in the bar, until what it shows changes.
///
/// SwiftUI draws an item again for the toolbar's overflow menu, without the bar's padding and
/// with its controls as menu rows, so the item measures narrower there: the sync capsule by 16
/// points, the cast button by 19, the Library's lists by hundreds. AppKit then finds room for it
/// and puts it back in the bar, where it is wider again and has no room, back and forth within
/// one layout pass until AppKit ends the app ("more Update Constraints in Window passes than
/// there are views").
///
/// So each size asked of the item answers at least what it last answered, and may come out
/// narrower only once in a while (`ToolbarWidths.settle`): often enough for a new item to take its own
/// size as it settles in, too seldom for the back and forth. The widths are kept by the place in
/// the code the item comes from, not in the view, because the toolbar makes its items' views
/// again as it moves them.
///
/// `key` is what the item shows: words, counts, which buttons are there. A new key lets the item
/// take its new size at once.
struct ToolbarItemWidth: Layout {
    var site: String
    /// What the item shows, by its hash.
    var key: Int

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        guard let content = subviews.first else { return .zero }
        let size = content.sizeThatFits(proposal)
        let width = ToolbarWidths.answer(site: site, key: key, asked: proposal, width: size.width)
        return CGSize(width: width, height: size.height)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        // From the leading edge, where the toolbar puts an item's content without this layout.
        subviews.first?.place(at: CGPoint(x: bounds.minX, y: bounds.midY), anchor: .leading, proposal: ProposedViewSize(bounds.size))
    }
}

/// The widths each toolbar item has answered, by where in the code the item comes from.
enum ToolbarWidths {
    /// How long after an item's answer to a size came out narrower it may next do so. The back
    /// and forth runs within one layout pass, in a few milliseconds.
    static let settle: TimeInterval = 0.5

    /// A size asked of an item, as a dictionary key.
    struct Asked: Hashable {
        var width: CGFloat?
        var height: CGFloat?
    }

    struct Item {
        /// What the item shows, by its hash.
        var key: Int
        /// The answer to each size asked since the key last changed.
        var widths: [Asked: CGFloat] = [:]
        /// When each size asked last came out narrower.
        var narrowed: [Asked: TimeInterval] = [:]
    }

    private static let items = Mutex<[String: Item]>([:])

    /// The width the item at `site` answers to `asked`, its content measuring `width` now.
    static func answer(site: String, key: Int, asked proposal: ProposedViewSize, width: CGFloat) -> CGFloat {
        let asked = Asked(width: proposal.width, height: proposal.height)
        let now = ProcessInfo.processInfo.systemUptime
        return items.withLock { items in
            var item = items[site] ?? Item(key: key)
            // Sizes asked as the window moves pile up; a fresh start costs one settling.
            if item.key != key || item.widths.count > 64 {
                item = Item(key: key)
            }
            var answer = width
            if let held = item.widths[asked], width < held {
                if let last = item.narrowed[asked], now - last < settle {
                    answer = held
                } else {
                    item.narrowed[asked] = now
                }
            }
            item.widths[asked] = answer
            items[site] = item
            return answer
        }
    }
}

extension View {
    /// The Mac toolbar item's content at one width in the bar and in the overflow menu
    /// (`ToolbarItemWidth`). Every item in the Mac's toolbar goes through it. An item that can
    /// show nothing is wrapped only when it shows, so the toolbar still leaves it out.
    func toolbarItemWidth(_ key: some Hashable, fileID: String = #fileID, line: Int = #line) -> some View {
        ToolbarItemWidth(site: "\(fileID):\(line)", key: key.hashValue) { self }
    }
}
