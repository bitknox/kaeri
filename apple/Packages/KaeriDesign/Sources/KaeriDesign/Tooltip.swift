// The browsing's tooltips, which keep quiet while the Mac's player covers the browsing (M4).

import SwiftUI

extension EnvironmentValues {
    /// The player has the main window, over the browsing it keeps beneath it for Back to return
    /// to. The player's host says so to the browsing.
    @Entry public var coveredByPlayer = false
}

extension View {
    /// `text` as this view's tooltip, as `help(_:)` gives it, except while the player covers the
    /// browsing. Hidden and out of hit testing is not enough there: SwiftUI hosts parts of the
    /// window in AppKit views of their own (the split view's columns, an inspector, a list's rows),
    /// and each one finds its tooltips by its own hit test, which never sees the player drawn
    /// over it from the window's root. The environment reaches all of them.
    public func tooltip(_ text: String) -> some View {
        modifier(Tooltip(text: text))
    }
}

private struct Tooltip: ViewModifier {
    @Environment(\.coveredByPlayer) private var covered
    let text: String

    func body(content: Content) -> some View {
        // An empty tooltip is none.
        content.help(covered ? "" : text)
    }
}
