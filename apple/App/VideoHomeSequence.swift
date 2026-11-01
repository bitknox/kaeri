#if FIXTURES && os(macOS)
import AppKit
import KaeriStore
import KaeriUI

/// The Mac feature video's Home sequence, played by the Fixtures app itself when it is launched
/// with `-KaeriVideoHome <marks file>` (and `-KaeriHomeWorld 1`): the page settled, one eased
/// scroll down to a lower shelf, the pointer eased onto a title's card so its hover preview opens and
/// is read, then the title opened through the navigator, as a click on its card does, and held. The test only launches the app and reads the marks, so
/// no keystroke, click or accessibility query can miss. The marks go to the file as the recorder
/// reads them, one per line: `window`, `rest`, `start`, `click`, `end`. The release app does not
/// compile this file.
final class Arrival: @unchecked Sendable {
    var info: [AnyHashable: Any]?
}

@MainActor
enum VideoHomeSequence {
    static let argument = "-KaeriVideoHome"
    static let title = "The Inn of Falling Stars"

    static func marksFile() -> String? {
        let arguments = ProcessInfo.processInfo.arguments
        guard arguments.contains("-KaeriHomeWorld"), let index = arguments.firstIndex(of: argument),
            arguments.indices.contains(index + 1)
        else { return nil }
        return arguments[index + 1]
    }

    static func run(file: String, navigator: Navigator, store: AppStore) async {
        func mark(_ line: String) {
            let text = line + "\n"
            if let handle = FileHandle(forWritingAtPath: file) {
                handle.seekToEndOfFile()
                handle.write(Data(text.utf8))
                try? handle.close()
            } else {
                try? text.write(toFile: file, atomically: true, encoding: .utf8)
            }
        }
        func now() -> String { String(format: "%.3f", Date().timeIntervalSince1970) }

        // The window up and the Home world's art drawn.
        for _ in 0..<200 where NSApp.mainWindow == nil { try? await Task.sleep(for: .milliseconds(100)) }
        try? await Task.sleep(for: .seconds(4))
        guard let window = NSApp.mainWindow, let screen = NSScreen.screens.first else {
            mark("failed no window")
            return
        }
        // The real pointer parked below the window's corner: left over a shelf it would raise a
        // card's hover preview, and the recording leaves the pointer out anyway.
        CGWarpMouseCursorPosition(CGPoint(x: screen.frame.width - 4, y: screen.frame.height - 4))
        let frame = window.frame
        mark("window \(frame.minX) \(screen.frame.height - frame.maxY) \(frame.width) \(frame.height) screen \(screen.frame.width) \(screen.frame.height)")
        mark("rest 900.0 330.0 \(now())")
        try? await Task.sleep(for: .milliseconds(300))
        mark("start \(now())")
        try? await Task.sleep(for: .milliseconds(1500))
        scroll(in: window)
        try? await Task.sleep(for: .milliseconds(1600))
        guard let id = store.titles.entries.first(where: { $0.value.showTitle == title })?.key else {
            mark("failed no title")
            return
        }
        // The pointer onto the title's card along the eased path the drawn one takes: the hover
        // preview opens under it. The layer answers with where and when it arrived.
        let box = Arrival()
        let observer = NotificationCenter.default.addObserver(forName: .kaeriVideoHoverArrived, object: nil, queue: .main) { note in
            box.info = note.userInfo
        }
        NotificationCenter.default.post(name: .kaeriVideoHover, object: nil, userInfo: ["id": id, "from": CGPoint(x: 900, y: 330)])
        for _ in 0..<120 where box.info == nil { try? await Task.sleep(for: .milliseconds(50)) }
        NotificationCenter.default.removeObserver(observer)
        guard let info = box.info, let to = info["to"] as? CGPoint, let arrived = info["arrived"] as? Double else {
            mark("failed the pointer never arrived")
            return
        }
        mark("move \(String(format: "%.1f %.1f %.3f", to.x, to.y, arrived))")
        // The preview opens (0.45 s of rest, then its growth) and is read for 2 s.
        try? await Task.sleep(for: .milliseconds(2000))
        mark("click \(String(format: "%.1f %.1f %.3f", to.x, to.y, Date().timeIntervalSince1970))")
        navigator.openTitle(id)
        try? await Task.sleep(for: .milliseconds(2600))
        mark("end \(now())")
    }

    /// The page scrolled 600 points down in one move of 1.4 s with an ease-in-out curve.
    private static func scroll(in window: NSWindow) {
        guard let content = window.contentView, let scrollView = largestScrollView(in: content) else { return }
        let clip = scrollView.contentView
        var origin = clip.bounds.origin
        origin.y += clip.isFlipped ? 600 : -600
        NSAnimationContext.runAnimationGroup { context in
            context.duration = 1.4
            context.timingFunction = CAMediaTimingFunction(controlPoints: 0.45, 0, 0.55, 1)
            clip.animator().setBoundsOrigin(origin)
        } completionHandler: {
            MainActor.assumeIsolated { scrollView.reflectScrolledClipView(clip) }
        }
    }

    /// The page's scroll view: the largest in the window, as the sidebar's is narrower.
    private static func largestScrollView(in view: NSView) -> NSScrollView? {
        var best: NSScrollView?
        var stack = [view]
        while let next = stack.popLast() {
            if let scroll = next as? NSScrollView,
                scroll.frame.width * scroll.frame.height > (best.map { $0.frame.width * $0.frame.height } ?? 0)
            {
                best = scroll
            }
            stack.append(contentsOf: next.subviews)
        }
        return best
    }
}
#endif
