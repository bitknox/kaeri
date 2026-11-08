// Artwork asked for once per URL, decoded small enough, and asked again while on screen.

@testable import KaeriStore
import CoreGraphics
import Foundation
import ImageIO
import Synchronization
import Testing
import UniformTypeIdentifiers

/// A PNG of `width` by `height` pixels in one colour.
func png(width: Int, height: Int) -> Data {
    let context = CGContext(
        data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0,
        space: CGColorSpace(name: CGColorSpace.sRGB)!,
        bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
    )!
    context.setFillColor(red: 0.89, green: 0.63, blue: 0.36, alpha: 1)
    context.fill(CGRect(x: 0, y: 0, width: width, height: height))
    let data = NSMutableData()
    let destination = CGImageDestinationCreateWithData(data, UTType.png.identifier as CFString, 1, nil)!
    CGImageDestinationAddImage(destination, context.makeImage()!, nil)
    CGImageDestinationFinalize(destination)
    return data as Data
}

@MainActor
struct ArtworkTests {
    @Test func aURLIsAskedForOnceWhileItIsOnItsWayAndOnceItIsHere() async throws {
        var asked: [String] = []
        let artwork = ArtworkStore { url in asked.append(url); return true }
        let url = "https://img.example/cover/900001.jpg"
        artwork.request(url, maxPixelSize: 108)
        artwork.request(url, maxPixelSize: 108)
        #expect(asked == [url])
        #expect(artwork.isPending(url))

        artwork.ready(url: url, bytes: png(width: 460, height: 690))
        try await waitFor { artwork.image(url) != nil }
        artwork.request(url, maxPixelSize: 108)
        #expect(asked == [url])
        #expect(!artwork.isPending(url))
    }

    @Test func artIsDownsampledToTheLargestSizeAsked() async throws {
        let artwork = ArtworkStore { _ in true }
        let url = "https://img.example/banner/900001.jpg"
        artwork.request(url, maxPixelSize: 300)
        artwork.request(url, maxPixelSize: 600)
        artwork.ready(url: url, bytes: png(width: 1900, height: 400))
        try await waitFor { artwork.image(url) != nil }
        let image = try #require(artwork.image(url))
        #expect(image.width == 600)
        #expect(image.height == 126)
    }

    @Test func aLargerAskDecodesAgainWhereTheFileHasMore() async throws {
        // A blurry banner: a list row decoded a cover at 108 pixels, and the preview's
        // banner, the same URL for a title with no banner, stretched that decode.
        let asked = Box(0)
        let artwork = ArtworkStore { _ in asked.value += 1; return true }
        let url = "https://img.example/cover/900005-xl.jpg"
        let bytes = png(width: 460, height: 690)
        artwork.request(url, maxPixelSize: 108)
        artwork.ready(url: url, bytes: bytes)
        try await waitFor { artwork.image(url) != nil }
        #expect(artwork.image(url)?.height == 108)

        artwork.request(url, maxPixelSize: 1800)
        #expect(asked.value == 2, "asked again, for the larger decode")
        #expect(artwork.image(url)?.height == 108, "the small one shows meanwhile")
        artwork.ready(url: url, bytes: bytes)
        try await waitFor { artwork.image(url)?.height == 690 }
        // The file has no more than that, so a larger ask still asks nothing.
        artwork.request(url, maxPixelSize: 2400)
        #expect(asked.value == 2)
    }

    @Test func aSmallerAskUsesTheLargerDecode() async throws {
        let asked = Box(0)
        let artwork = ArtworkStore { _ in asked.value += 1; return true }
        let url = "https://img.example/banner/900001.jpg"
        artwork.request(url, maxPixelSize: 600)
        artwork.ready(url: url, bytes: png(width: 1600, height: 900))
        try await waitFor { artwork.image(url) != nil }
        artwork.request(url, maxPixelSize: 108)
        #expect(asked.value == 1)
        #expect(artwork.image(url)?.width == 600)
    }

    @Test func anAskThatGrowsWhileTheBytesDecodeIsAskedAgain() async throws {
        let asked = Box(0)
        let artwork = ArtworkStore { _ in asked.value += 1; return true }
        let url = "https://img.example/cover/900002-xl.jpg"
        let bytes = png(width: 460, height: 690)
        artwork.request(url, maxPixelSize: 108)
        artwork.ready(url: url, bytes: bytes)
        // Before those bytes have decoded at 108.
        artwork.request(url, maxPixelSize: 1800)
        #expect(asked.value == 1, "the first is still on its way")
        try await waitFor { asked.value == 2 }
        artwork.ready(url: url, bytes: bytes)
        try await waitFor { artwork.image(url)?.height == 690 }
    }

    @Test func aURLThatFailedWaitsBeforeItIsAskedForAgain() {
        let clock = SteppedClock()
        var asked = 0
        let artwork = ArtworkStore(now: { clock.now }) { _ in asked += 1; return true }
        let url = "https://img.example/cover/900005.jpg"
        artwork.request(url, maxPixelSize: 108)
        artwork.failed(url: url)
        artwork.request(url, maxPixelSize: 108)
        #expect(asked == 1)
        clock.advance(.seconds(3))
        artwork.request(url, maxPixelSize: 108)
        #expect(asked == 2, "the first wait is short")
        artwork.failed(url: url)
        clock.advance(.seconds(3))
        artwork.request(url, maxPixelSize: 108)
        #expect(asked == 2, "the second is longer")
        clock.advance(.seconds(3))
        artwork.request(url, maxPixelSize: 108)
        #expect(asked == 3)
    }

    @Test func anAskWithNoEngineYetIsMadeAgainLater() {
        let engine = Box(false)
        let asked = Box(0)
        let artwork = ArtworkStore { _ in asked.value += 1; return engine.value }
        artwork.request("https://img.example/cover/900002.jpg", maxPixelSize: 108)
        engine.value = true
        artwork.request("https://img.example/cover/900002.jpg", maxPixelSize: 108)
        #expect(asked.value == 2)
    }

    @Test func anAskWithNoEngineYetIsSentOnceThereIsOne() {
        // A cover already on screen asks once, from its task: it never asks again by itself.
        let engine = Box(false)
        var sent: [String] = []
        let artwork = ArtworkStore { url in
            guard engine.value else { return false }
            sent.append(url)
            return true
        }
        let first = "https://img.example/cover/900002.jpg"
        let second = "https://img.example/cover/900003.jpg"
        artwork.request(first, maxPixelSize: 108)
        artwork.request(second, maxPixelSize: 108)
        artwork.request(first, maxPixelSize: 108)
        #expect(!artwork.hasPending)
        engine.value = true
        artwork.sendUnsent()
        #expect(sent == [first, second], "each once, in the order asked")
        #expect(artwork.isPending(first) && artwork.isPending(second))
        artwork.sendUnsent()
        #expect(sent.count == 2)
    }

    @Test func aURLOnScreenThatFailedIsAskedAgainOnTheBackoff() async throws {
        // A view stays on screen and asks once: its art does not stay a plate once the retry succeeds.
        let slept = Slept()
        let asked = Box(0)
        let artwork = ArtworkStore(
            now: { ContinuousClock.now },
            sleep: { wait in slept.add(wait); try await Task.sleep(for: .milliseconds(1)) }
        ) { _ in asked.value += 1; return true }
        let url = "https://img.example/cover/900004.jpg"
        let shown = Task { await artwork.show(url, maxPixelSize: 108) }
        defer { shown.cancel() }
        try await waitFor { asked.value == 1 }
        for times in 2...5 {
            artwork.failed(url: url)
            try await waitFor { asked.value == times }
        }
        // Tries left no more: only showing it again asks again.
        artwork.failed(url: url)
        try await Task.sleep(for: .milliseconds(50))
        #expect(asked.value == 5)
        let waits = slept.all.map { Int(($0 + .milliseconds(500)).components.seconds) }
        #expect(waits == [2, 5, 15, 60])
    }

    @Test func aURLNoLongerOnScreenIsNotAskedAgainByItself() async throws {
        let slept = Slept()
        let asked = Box(0)
        let artwork = ArtworkStore(
            sleep: { wait in slept.add(wait); try await Task.sleep(for: .milliseconds(1)) }
        ) { _ in asked.value += 1; return true }
        let url = "https://img.example/cover/900004.jpg"
        let shown = Task { await artwork.show(url, maxPixelSize: 108) }
        try await waitFor { asked.value == 1 }
        shown.cancel()
        await shown.value
        artwork.failed(url: url)
        try await Task.sleep(for: .milliseconds(50))
        #expect(asked.value == 1)
        #expect(slept.all.isEmpty)
    }

    @Test func aRetryWaitingIsDroppedWhenTheScreenGoes() async throws {
        let asked = Box(0)
        let artwork = ArtworkStore(sleep: { _ in try await Task.sleep(for: .milliseconds(100)) }) { _ in
            asked.value += 1
            return true
        }
        let url = "https://img.example/cover/900004.jpg"
        let shown = Task { await artwork.show(url, maxPixelSize: 108) }
        try await waitFor { asked.value == 1 }
        artwork.failed(url: url)
        shown.cancel()
        await shown.value
        try await Task.sleep(for: .milliseconds(200))
        #expect(asked.value == 1)
    }

    @Test func aScreenShowingAURLThatFailedAMomentAgoAsksOnceTheWaitIsOver() async throws {
        let clock = SteppedClock()
        let slept = Slept()
        let asked = Box(0)
        let artwork = ArtworkStore(
            now: { clock.now },
            sleep: { wait in slept.add(wait); try await Task.sleep(for: .milliseconds(1)) }
        ) { _ in asked.value += 1; return true }
        let url = "https://img.example/cover/900006.jpg"
        // Failed for the widgets, with no screen showing it.
        artwork.request(url, maxPixelSize: 108)
        artwork.failed(url: url)
        clock.advance(.milliseconds(500))
        let shown = Task { await artwork.show(url, maxPixelSize: 108) }
        defer { shown.cancel() }
        try await waitFor { asked.value == 2 }
        #expect(slept.all == [.milliseconds(1500)], "what was left of the wait")
    }

    @Test func artLandingIsSeenOnceEachTime() async throws {
        let artwork = ArtworkStore { _ in true }
        let url = "https://img.example/cover/900001.jpg"
        let bytes = png(width: 460, height: 690)
        artwork.request(url, maxPixelSize: 108)
        artwork.ready(url: url, bytes: bytes)
        try await waitFor { artwork.image(url) != nil }
        #expect(artwork.arrivalCount(url) == 1)
        // A larger decode replaces it, which the view reading it sees.
        artwork.request(url, maxPixelSize: 460)
        artwork.ready(url: url, bytes: bytes)
        try await waitFor { artwork.arrivalCount(url) == 2 }
        #expect(artwork.image(url)?.height == 460)
    }

    @Test func artGoneOffScreenIsTheCachesAgain() async throws {
        let asked = Box(0)
        let artwork = ArtworkStore { _ in asked.value += 1; return true }
        let url = "https://img.example/cover/900001.jpg"
        let shown = Task { await artwork.show(url, maxPixelSize: 108) }
        try await waitFor { asked.value == 1 }
        artwork.ready(url: url, bytes: png(width: 460, height: 690))
        try await waitFor { artwork.image(url) != nil }
        shown.cancel()
        try await Task.sleep(for: .milliseconds(20))
        artwork.evict(url)
        #expect(artwork.image(url) == nil, "no longer held")
    }

    @Test func artTheCacheLetGoOffScreenWaitsToBeShownAgain() async throws {
        let asked = Box(0)
        let artwork = ArtworkStore { _ in asked.value += 1; return true }
        let url = "https://img.example/cover/900001.jpg"
        artwork.request(url, maxPixelSize: 108)
        artwork.ready(url: url, bytes: png(width: 460, height: 690))
        try await waitFor { artwork.image(url) != nil }
        artwork.evict(url)
        #expect(artwork.image(url) == nil)
        try await Task.sleep(for: .milliseconds(20))
        #expect(asked.value == 1)
    }

    @Test func artOnScreenOutlastsTheCacheLettingItGo() async throws {
        // A title page, every image flashing to its plate at once on a season switch: the
        // cache let go of art still on screen, as NSCache may whenever memory is wanted, and the
        // view found nothing the next time it read it.
        let asked = Box(0)
        let artwork = ArtworkStore { _ in asked.value += 1; return true }
        let url = "https://img.example/cover/920001.jpg"
        let shown = Task { await artwork.show(url, maxPixelSize: 460) }
        defer { shown.cancel() }
        try await waitFor { asked.value == 1 }
        artwork.ready(url: url, bytes: png(width: 460, height: 690))
        try await waitFor { artwork.image(url) != nil }

        artwork.evict(url)
        #expect(artwork.image(url) != nil, "still on screen")
        try await Task.sleep(for: .milliseconds(20))
        #expect(asked.value == 1, "and not asked for again")
    }

    @Test func artArrivingIsSeenOnlyByTheViewsShowingIt() async throws {
        // What turned one eviction into every image at once: each arrival changed what every
        // view reading art observes, so all of them read their art again on any arrival, a season
        // switch's burst of them among it.
        let artwork = ArtworkStore { _ in true }
        let shownURL = "https://img.example/cover/920001.jpg"
        let other = "https://img.example/banner/920002.jpg"
        artwork.request(shownURL, maxPixelSize: 460)
        artwork.ready(url: shownURL, bytes: png(width: 460, height: 690))
        try await waitFor { artwork.image(shownURL) != nil }

        let changed = Box(false)
        withObservationTracking {
            _ = artwork.image(shownURL)
        } onChange: {
            Task { @MainActor in changed.value = true }
        }
        artwork.request(other, maxPixelSize: 1200)
        artwork.ready(url: other, bytes: png(width: 1900, height: 400))
        try await waitFor { artwork.image(other) != nil }
        try await Task.sleep(for: .milliseconds(20))
        #expect(!changed.value)
    }

    @Test func aDecodeTheCacheTurnsAwayIsNotAskedForInALoop() async throws {
        // Over its limit, NSCache can turn away the very decode just put in it. The store asked
        // the engine again whenever it found no decode right after one landed, which with a full
        // cache asks, decodes and is turned away again.
        let asked = Box(0)
        let cover = png(width: 460, height: 690)
        let artwork = ArtworkStore(costLimit: 1) { _ in asked.value += 1; return true }
        let url = "https://img.example/cover/920003.jpg"
        artwork.request(url, maxPixelSize: 460)
        artwork.ready(url: url, bytes: cover)
        try await waitFor { artwork.arrivalCount(url) > 0 }
        try await Task.sleep(for: .milliseconds(20))
        #expect(asked.value == 1)
    }

    @Test func aWeekScrolledThroughFastLeavesNothingWaitingAndTheLastScreenFilled() async throws {
        // Calendar's week, scrolled through: each card on screen a moment, ten at a time, while
        // the engine answers late, newest first, and every fifth cover fails the first time.
        var asked: [String] = []
        let artwork = ArtworkStore(sleep: { _ in try await Task.sleep(for: .milliseconds(1)) }) { url in
            asked.append(url)
            return true
        }
        let urls = (0..<300).map { "https://img.example/cover/93\($0).jpg" }
        var screen: [(url: String, task: Task<Void, Never>)] = []
        for url in urls {
            screen.append((url, Task { await artwork.show(url, maxPixelSize: 108) }))
            if screen.count > 10 { screen.removeFirst().task.cancel() }
            await Task.yield()
        }
        try await waitFor { asked.count == urls.count }
        #expect(Set(asked).count == urls.count, "each asked once")

        let cover = png(width: 46, height: 69)
        for (index, url) in asked.enumerated().reversed() {
            if index % 5 == 0 { artwork.failed(url: url) } else { artwork.ready(url: url, bytes: cover) }
        }
        // The cards still on screen whose cover failed are asked again; answered, they land.
        let failedOnScreen = screen.map(\.url).filter { asked.firstIndex(of: $0)! % 5 == 0 }
        try await waitFor { asked.count == urls.count + failedOnScreen.count }
        for url in failedOnScreen { artwork.ready(url: url, bytes: cover) }
        try await waitFor { !artwork.hasPending }
        for (url, _) in screen { #expect(artwork.image(url) != nil, "\(url) still a plate") }
        for (_, task) in screen { task.cancel() }
    }

    @Test func artScrolledPastStaysWithinTheCacheLimitAndItsShowsEnd() async throws {
        // Forty covers, each on screen until it lands and then scrolled past: what stays decoded
        // off screen is within the limit, and each show ends as its view goes.
        let cover = png(width: 46, height: 69)
        let decoded = try #require(ArtworkStore.decode(cover, maxPixelSize: 108))
        let limit = decoded.bytesPerRow * decoded.height * 8
        let artwork = ArtworkStore(costLimit: limit) { _ in true }
        let urls = (0..<40).map { "https://img.example/cover/95\($0).jpg" }
        for url in urls {
            let shown = Task { await artwork.show(url, maxPixelSize: 108) }
            try await waitFor { artwork.isPending(url) }
            artwork.ready(url: url, bytes: cover)
            try await waitFor { artwork.arrivalCount(url) > 0 }
            shown.cancel()
            await shown.value
        }
        let kept = urls.compactMap { artwork.image($0) }.reduce(0) { $0 + $1.bytesPerRow * $1.height }
        #expect(kept > 0)
        #expect(kept <= limit, "\(kept) bytes kept over a \(limit) byte limit")
    }

    @Test func bytesThatAreNoImageDecodeToNothing() {
        #expect(ArtworkStore.decode(Data([0xFF, 0xD8, 0x00, 0x7F]), maxPixelSize: 100) == nil)
    }
}

/// The waits a store's retries asked for, from whichever thread they ask.
final class Slept: Sendable {
    private let waits = Mutex<[Duration]>([])
    func add(_ wait: Duration) { waits.withLock { $0.append(wait) } }
    var all: [Duration] { waits.withLock { $0 } }
}

/// A value the main actor changes from inside a closure.
@MainActor
final class Box<Value> {
    var value: Value
    init(_ value: Value) { self.value = value }
}

/// Wait, a little at a time, until `done` says so.
///
/// What is waited for is a task hopping actors or a utility-priority decode of a few hundred
/// bytes: milliseconds of work that either happens or never does. How long it takes to be
/// scheduled is the machine's, not the code's: with Gradle, Docker and cargo running, or the run
/// held to a low quality of service, a task waits seconds for a thread, and a 5 s limit failed
/// ten correct tests at once. `within` is therefore only a guard against a wait that can never
/// end (a lost callback), so that it fails instead of hanging; it is far above any scheduling
/// delay and says nothing about speed.
@MainActor
func waitFor(_ done: () -> Bool, within: Duration = .seconds(120)) async throws {
    let deadline = ContinuousClock.now + within
    while !done() {
        try #require(ContinuousClock.now < deadline, "waited \(within)")
        try await Task.sleep(for: .milliseconds(10))
    }
}
