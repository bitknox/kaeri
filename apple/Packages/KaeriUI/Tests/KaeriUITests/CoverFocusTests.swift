// Covers' focus worked out by Vision, for a screen of covers landing at once.

import CoreGraphics
import Foundation
@testable import KaeriUI
import Testing

@MainActor
struct CoverFocusTests {
    /// A cover-sized picture: a plain ground with a bright disc somewhere on it.
    func cover(_ seed: Int) -> CGImage {
        let context = CGContext(
            data: nil, width: 230, height: 345, bitsPerComponent: 8, bytesPerRow: 0,
            space: CGColorSpace(name: CGColorSpace.sRGB)!,
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        )!
        context.setFillColor(red: 0.2, green: 0.3, blue: 0.4, alpha: 1)
        context.fill(CGRect(x: 0, y: 0, width: 230, height: 345))
        context.setFillColor(red: 0.9, green: CGFloat(seed % 10) / 10, blue: 0.1, alpha: 1)
        context.fillEllipse(in: CGRect(x: 40 + seed % 40, y: 60 + seed % 200, width: 100, height: 100))
        return context.makeImage()!
    }

    /// Waits on the main actor alone, so a stuck concurrency pool fails the test instead of
    /// hanging it.
    func settle(within: Duration = .seconds(30), _ done: () -> Bool) async {
        let deadline = ContinuousClock.now + within
        while !done(), ContinuousClock.now < deadline {
            try? await Task.sleep(for: .milliseconds(20))
        }
    }

    @Test func aScreenOfCoversLandingAtOnceAllGetTheirFocusAndTheAppKeepsRunning() async {
        // Calendar's cover crops: twice as many covers at once as there are cores, each asking
        // Vision for its focus as it lands.
        let focus = CoverFocus()
        let urls = (0..<ProcessInfo.processInfo.activeProcessorCount * 2).map {
            "https://img.example/cover/91\($0).jpg"
        }
        for (seed, url) in urls.enumerated() {
            focus.prepare(url, image: cover(seed))
        }
        await settle { urls.allSatisfy { focus.focus($0) != nil } }
        let found = urls.filter { focus.focus($0) != nil }.count
        #expect(found == urls.count, "\(found) of \(urls.count) covers got a focus")

        // Work on the pool still runs after them, as a cover's decode does.
        let ran = Ran()
        Task.detached(priority: .utility) {
            await MainActor.run { ran.yes = true }
        }
        await settle(within: .seconds(10)) { ran.yes }
        #expect(ran.yes, "the concurrency pool still runs work")
    }
}

/// Whether a task ran.
@MainActor
final class Ran {
    var yes = false
}
