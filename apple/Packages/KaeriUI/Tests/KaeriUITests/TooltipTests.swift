#if os(macOS)
import KaeriDesign
import AppKit
import Foundation
import ObjectiveC
import SwiftUI
import Testing

/// The browsing's tooltips while the Mac's player covers it. The player's host hides the browsing
/// and takes it out of hit testing, but a split view's columns, an inspector and a list's rows are
/// AppKit views of their own, each finding its tooltips by its own hit test.
@MainActor
struct TooltipTests {
    /// A window like the main one, the player's host over it as `hostsPlayer` covers it.
    private struct Browsing: View {
        let covered: Bool

        var body: some View {
            NavigationSplitView {
                List { Text(verbatim: "Home").tooltip("side") }
            } detail: {
                ScrollView {
                    VStack {
                        ForEach(0..<6) { row in
                            Text(verbatim: "Episode \(row)").frame(width: 300, height: 40).tooltip("episode \(row)")
                        }
                    }
                }
                .inspector(isPresented: .constant(true)) {
                    List { Text(verbatim: "Filter").tooltip("filter") }
                }
            }
            .opacity(covered ? 0 : 1)
            .allowsHitTesting(!covered)
            .environment(\.coveredByPlayer, covered)
            .overlay {
                if covered { Color.black.contentShape(.rect).help(Text(verbatim: "player")) }
            }
        }
    }

    /// Every tooltip each view in the window answers somewhere in it, as AppKit asks SwiftUI's
    /// hosting views for the one under the pointer, by the view that answers it.
    private func tooltips(covered: Bool) -> [String: Set<String>] {
        _ = NSApplication.shared
        let size = NSSize(width: 900, height: 420)
        let window = NSWindow(contentRect: NSRect(origin: .zero, size: size), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        defer { window.close() }
        let root = NSHostingView(rootView: Browsing(covered: covered))
        // Never ordered in: the views answer all the same.
        window.contentView = root
        RunLoop.main.run(until: Date().addingTimeInterval(1))
        var views: [NSView] = []
        func collect(_ view: NSView) {
            views.append(view)
            view.subviews.forEach(collect)
        }
        collect(root)
        let manager = NSSelectorFromString("_dynamicToolTipManager")
        let ask = NSSelectorFromString("dynamicToolTipStringAtPoint:trackingRect:")
        typealias Ask = @convention(c) (AnyObject, Selector, NSPoint, UnsafeMutablePointer<NSRect>) -> Unmanaged<AnyObject>?
        var found: [String: Set<String>] = [:]
        for view in views where view.responds(to: manager) {
            guard let tips = view.perform(manager)?.takeUnretainedValue() as? NSObject,
                  let method = class_getInstanceMethod(type(of: tips), ask)
            else { continue }
            let call = unsafeBitCast(method_getImplementation(method), to: Ask.self)
            let name = view === root ? "root" : "\(type(of: view))"
            for x in stride(from: 5.0, to: size.width, by: 10) {
                for y in stride(from: 5.0, to: size.height, by: 10) {
                    var area = NSRect.zero
                    let point = view.convert(NSPoint(x: x, y: y), from: root)
                    if let text = call(tips, ask, point, &area)?.takeUnretainedValue() as? String, !text.isEmpty {
                        found[name, default: []].insert(text)
                    }
                }
            }
        }
        return found
    }

    @Test func theBrowsingShowsItsTooltips() {
        let found = tooltips(covered: false).values.reduce(into: Set<String>()) { $0.formUnion($1) }
        // Answered by views AppKit hosts on their own: what the covered case must silence.
        #expect(found.contains("episode 2"), "\(found)")
        #expect(found.contains("filter"), "\(found)")
    }

    @Test func noTooltipOfTheBrowsingShowsThroughThePlayer() {
        let found = tooltips(covered: true)
        #expect(found["root"] == ["player"], "the player's own still shows: \(found)")
        #expect(found.filter { $0.key != "root" }.isEmpty, "\(found)")
    }

    /// The browsing's tooltips all go through `tooltip(_:)`, which the player silences.
    @Test func theBrowsingNeverUsesHelpItself() throws {
        let packages = URL(filePath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent()
        var found: [String] = []
        for package in ["KaeriUI", "KaeriDesign"] {
            let files = FileManager.default.enumerator(at: packages.appending(path: package), includingPropertiesForKeys: nil)
            while let url = files?.nextObject() as? URL {
                let path = url.path
                guard url.pathExtension == "swift", path.contains("/Sources/"), !path.contains("/.build/"),
                      url.lastPathComponent != "Tooltip.swift"
                else { continue }
                let text = try String(contentsOf: url, encoding: .utf8)
                if text.contains(".help(") || text.contains(".toolTip") { found.append(url.lastPathComponent) }
            }
        }
        #expect(found.isEmpty, "\(found)")
    }
}
#endif
