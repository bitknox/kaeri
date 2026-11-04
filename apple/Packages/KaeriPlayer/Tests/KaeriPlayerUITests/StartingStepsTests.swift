// The starting screen's steps: what the engine's fixtures make of the lines, the actions by
// time, the hold on each text, the failure screens and the ground's depth.

import CoreGraphics
import KaeriKit
import KaeriPlayer
import Foundation
import Testing

@testable import KaeriPlayerUI

@MainActor
struct StartingStepsTests {
    /// Play a fixture's events into a model on a stepped clock up to `seconds`.
    func run(_ state: StartingFixture, to seconds: Double, _ rig: Rig = Rig()) -> Rig {
        var elapsed = 0.0
        for fixture in startingFixture(state: state) {
            let at = Double(fixture.atMs) / 1000
            guard at <= seconds else { break }
            if at > elapsed {
                rig.step(.milliseconds(Int((at - elapsed) * 1000)))
                elapsed = at
            }
            rig.model.apply(fixture.event)
        }
        if seconds > elapsed { rig.step(.milliseconds(Int((seconds - elapsed) * 1000))) }
        return rig
    }

    func lines(_ rig: Rig) -> (step: String, detail: String) {
        let now = rig.model.clock
        return (rig.model.walk?.stepLine(at: now) ?? "", rig.model.walk?.detail(at: now) ?? "")
    }

    @Test func aSourceAskedNamesItAndItsPlace() async throws {
        let rig = run(.askingFirst, to: 2)
        #expect(lines(rig).step == "Asking Alpha")
        #expect(lines(rig).detail == "Resuming at 10:06 · 1 of 3 sources")
    }

    @Test func aPinnedSourceSaysSo() {
        let rig = run(.askingPinned, to: 2)
        #expect(lines(rig).detail.contains("Your pick for this show"))
    }

    @Test func aSourceThatCameUpEmptyIsSaidOnTheDetailLineOfTheNext() {
        let rig = run(.sourceMissed, to: 4)
        let words = lines(rig)
        #expect(words.step == "Asking Brook")
        #expect(words.detail.contains("Alpha doesn't have episode 7"))
        #expect(words.detail.contains("2 of 3 sources"))
        #expect(words.detail.hasSuffix(" s"), "seconds from 3 s")
    }

    @Test func aSlowStartSaysItPlainlyFromTwentySeconds() {
        let rig = run(.slow, to: 24)
        #expect(lines(rig).step == "Still waiting for Brook")
        #expect(rig.model.walk?.actions(at: rig.model.clock).contains(.skip("Skip Brook")) == true)
    }

    @Test func bufferingGivesAFillOnlyWhenMpvGivesANumber() {
        let torrent = run(.bufferingTorrent, to: 9)
        #expect(torrent.model.walk?.progress == 0.4)
        #expect(lines(torrent).step.hasPrefix("Buffering · 40"))
        #expect(lines(torrent).detail.hasPrefix("Resuming at 10:06 · 12 peers"))

        let direct = run(.bufferingDirect, to: 4)
        #expect(direct.model.walk?.progress == nil)
        #expect(lines(direct).step == "Opening the stream")
    }

    @Test func peersAreFoundAndCounted() {
        let rig = run(.findingPeers, to: 4)
        #expect(lines(rig).step == "Finding peers")
        #expect(lines(rig).detail.contains("From Harbor"))
    }

    @Test func aStreamWithNoVideoSaysSoOnTheStepLine() {
        let rig = run(.anotherStream, to: 10)
        #expect(lines(rig).step == "Trying another stream")
        #expect(lines(rig).detail.contains("2 of 3 streams"))
    }

    @Test func detailsIsInTheRowFromTheStartWhileItsListIsRememberedOpen() {
        let rig = run(.slow, to: 0.5)
        #expect(rig.model.walk?.actions(at: rig.model.clock, detailsOpen: true) == [.cancel, .details])
        rig.step(.seconds(1))
        #expect(rig.model.walk?.actions(at: rig.model.clock, detailsOpen: true) == [.cancel, .playFromBeginning, .details])
        #expect(rig.model.walk?.actions(at: rig.model.clock) == [.cancel, .playFromBeginning])
        rig.step(.seconds(2))
        #expect(rig.model.walk?.actions(at: rig.model.clock, detailsOpen: true) == [.cancel, .playFromBeginning, .chooseSource, .details])
    }

    @Test func theActionsJoinTheRowByTimeInArrivalOrder() {
        let rig = run(.slow, to: 0.5)
        #expect(rig.model.walk?.actions(at: rig.model.clock) == [.cancel])
        rig.step(.seconds(1))
        #expect(rig.model.walk?.actions(at: rig.model.clock) == [.cancel, .playFromBeginning])
        rig.step(.seconds(2))
        #expect(rig.model.walk?.actions(at: rig.model.clock) == [.cancel, .playFromBeginning, .chooseSource, .details])
        let slow = run(.slow, to: 12)
        #expect(slow.model.walk?.actions(at: slow.model.clock).last == .skip("Skip Brook"))
    }

    @Test func skipWaitsForAnotherSourceToTry() {
        let rig = run(.sourcesResting, to: 12)
        // One of two sources asked: another is left, so skipping is offered.
        #expect(rig.model.walk?.actions(at: rig.model.clock).contains(.skip("Skip Alpha")) == true)
        let last = run(.failedNoMatch, to: 1.2)
        #expect(last.model.walk?.actions(at: last.model.clock).contains { if case .skip = $0 { true } else { false } } == false)
    }

    @Test func eachTextHoldsAndOnlyTheLatestWaiting() async throws {
        let rig = Rig()
        let now = rig.time.keeper.now()
        var walk = StartingWalk(id: 1, episode: "7", resume: nil, at: now)
        #expect(walk.line == "Looking up the title")
        walk.take(.gettingSourcesReady, line: .startGettingSourcesReady, at: now + .milliseconds(100))
        walk.take(.askingSource(name: "Alpha", place: 1, total: 2, pinned: false), line: nil, at: now + .milliseconds(200))
        walk.take(.openingStream(attempt: 1, total: 1), line: nil, at: now + .milliseconds(300))
        walk.take(.found(count: 1, source: "Alpha", quality: 1080, translation: .sub), line: .startOpeningStream, at: now + .milliseconds(400))
        #expect(walk.line == "Looking up the title", "still held")
        #expect(walk.queued == "Opening the stream", "the newest waits, the ones between are passed over")
        #expect(walk.releaseAt == now + StartingWalk.hold)
        let early = walk.release(at: now + .milliseconds(500))
        let due = walk.release(at: now + .milliseconds(600))
        #expect(!early)
        #expect(due)
        #expect(walk.line == "Opening the stream")
    }

    @Test func playFromBeginningFlipsTheLineAndSendsTheCommand() {
        let rig = run(.askingFirst, to: 2)
        rig.model.playFromBeginning()
        #expect(rig.sent.last == .player(.playFromBeginning))
        #expect(lines(rig).detail.hasPrefix("Starting from the beginning"))
        // Its place stays, so Choose source and Details do not slide left.
        #expect(rig.model.walk?.actions(at: rig.model.clock).contains(.playFromBeginning) == true)
    }

    @Test func skippingSendsTheCommand() {
        let rig = run(.slow, to: 12)
        rig.model.skipSource()
        #expect(rig.sent.last == .player(.skipSource))
    }

    @Test func detailsRowsFollowTheWalk() {
        let rig = run(.sourceMissed, to: 4)
        let rows = rig.model.walk?.detailRows ?? []
        #expect(rows.map(\.kind) == [.done, .missed, .now])
        #expect(rows[0].title == "Looked up the title")
        #expect(rows[1].title == "Alpha doesn't have episode 7")
        #expect(rows[2].title == "Asking Brook")
    }

    @Test func detailsStayOpenOnceOpened() {
        let key = PlayerModel.detailsKey
        let before = UserDefaults.standard.object(forKey: key)
        defer { UserDefaults.standard.set(before, forKey: key) }
        let rig = Rig()
        rig.model.startDetailsOpen = true
        #expect(UserDefaults.standard.bool(forKey: key))
        rig.model.startDetailsOpen = false
        #expect(!UserDefaults.standard.bool(forKey: key))
    }

    @Test(arguments: [
        (StartingFixture.failedNoEpisode, StartFailureView.Lead.chooseSource),
        (.failedNoMatch, .fixMatch),
        (.failedNoSources, .addStore),
        (.failedNotAired, .close),
    ])
    func eachFailureLeadsWithItsOwnAction(state: StartingFixture, lead: StartFailureView.Lead) {
        let rig = run(state, to: 3)
        let failure = rig.model.failure
        #expect(failure?.view.lead == lead)
        #expect(failure?.view.reason.isEmpty == false)
        if state != .failedNotAired {
            #expect(failure?.view.reason.contains(":") == false, "the first clause, not what each source said")
        }
    }

    @Test func aFailureNamesTheSourcesAsked() {
        let rig = run(.failedNoEpisode, to: 3)
        #expect(rig.model.failure?.view.reason == "No source has episode 7.")
        #expect(rig.model.failure?.view.asked == "Alpha and Brook were asked.")
        #expect(rig.model.failure?.view.detail == "No source has episode 7. Alpha and Brook were asked.")
        let none = run(.failedNoSources, to: 3)
        #expect(none.model.failure?.view.asked == nil)
    }

    @Test func offlineWaitsOnTheEnginesStepAndStartsOnReconnect() {
        let waiting = run(.offline, to: 3)
        #expect(waiting.model.walk?.waiting == true)
        #expect(lines(waiting).step == "Waiting for a connection")
        #expect(lines(waiting).detail.contains("starts when you're back"))
        #expect(waiting.model.walk?.actions(at: waiting.model.clock) == [.cancel, .showDownloads])
        let back = run(.offline, to: 10)
        #expect(back.model.walk?.waiting == false)
        #expect(lines(back).step == "Asking Alpha")
    }

    @Test func offlineThatLastsFailsWithNothingAsked() {
        let rig = run(.offlineTimedOut, to: 121)
        #expect(rig.model.failure?.view.lead == .offline)
        #expect(rig.model.failure?.view.asked == nil)
        #expect(rig.model.failure?.view.reason == "No connection.")
    }

    @Test func theSlowWordingAppliesInAnyStepHeldPastTwentySeconds() {
        let now = ContinuousClock.now
        var walk = StartingWalk(id: 1, episode: "7", resume: nil, at: now)
        walk.take(.askingSource(name: "Alpha", place: 1, total: 2, pinned: false), line: nil, at: now)
        walk.take(.found(count: 1, source: "Alpha", quality: 1080, translation: .sub), line: .startOpeningStream, at: now + .seconds(1))
        walk.take(.buffering(percent: 40), line: nil, at: now + .seconds(2))
        #expect(walk.stepLine(at: now + .seconds(21)) == "Still waiting for Alpha")
        walk.take(.findingPeers(name: "Harbor"), line: .startFindingPeers, at: now + .seconds(3))
        #expect(walk.stepLine(at: now + .seconds(21)) == "Still waiting for Harbor")
    }

    @Test func aSpeedShowsOneDecimal() {
        // The decimal mark is the person's: compare as a point.
        func point(_ text: String) -> String { text.replacingOccurrences(of: ",", with: ".") }
        #expect(point(StartingWalk.rate(3_400_000)) == "3.4 MB", "\(StartingWalk.rate(3_400_000))")
        #expect(point(StartingWalk.rate(3_000_000)) == "3.0 MB", "\(StartingWalk.rate(3_000_000))")
        #expect(point(StartingWalk.rate(812_000)) == "812 kB", "\(StartingWalk.rate(812_000))")
    }

    @Test func theBreathingIsFourSecondsFromFullToFortyPercent() {
        #expect(abs(LoaderLine.breath(after: 0) - 1) < 0.0001)
        #expect(abs(LoaderLine.breath(after: 2) - 0.4) < 0.0001)
        #expect(abs(LoaderLine.breath(after: 4) - 1) < 0.0001)
    }

    @Test func aDetailsRowKeepsATenthAndASpace() {
        func point(_ text: String) -> String { text.replacingOccurrences(of: ",", with: ".") }
        #expect(point(StartDetails.seconds(.milliseconds(700))) == "0.7 s", "\(StartDetails.seconds(.milliseconds(700)))")
        #expect(point(StartDetails.seconds(.milliseconds(8_340))) == "8.3 s")
    }

    @Test func aFailureKeepsTheStepsForDetails() {
        let rig = run(.failedNoEpisode, to: 3)
        #expect(rig.model.walk?.detailRows.contains { $0.kind == .missed } == true)
    }

    @Test func everyFixtureRunsToItsEnd() {
        let all: [StartingFixture] = [
            .titleLookup, .sourcesLoading, .askingFirst, .askingPinned, .sourceMissed, .sourceFailed, .sourcesResting,
            .slow, .findingPeers, .peersKnown, .found, .fetchingSubtitles, .anotherStream, .bufferingTorrent,
            .bufferingDirect, .offline, .offlineTimedOut, .failedNoEpisode, .failedNoMatch, .failedNoSources, .failedNotAired,
        ]
        for state in all {
            let rig = run(state, to: 30)
            let words = lines(rig)
            #expect(!words.step.isEmpty || rig.model.failure != nil, "\(state) has a step")
        }
    }
}

struct GroundDepthTests {
    func image(_ red: UInt8, _ green: UInt8, _ blue: UInt8) -> CGImage {
        let context = CGContext(
            data: nil, width: 40, height: 24, bitsPerComponent: 8, bytesPerRow: 0, space: CGColorSpace(name: CGColorSpace.sRGB)!,
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        context.setFillColor(CGColor(srgbRed: CGFloat(red) / 255, green: CGFloat(green) / 255, blue: CGFloat(blue) / 255, alpha: 1))
        context.fill(CGRect(x: 0, y: 0, width: 40, height: 24))
        return context.makeImage()!
    }

    @Test func darkArtKeepsTheNormalGroundAndBrightArtGetsTheDeepOne() throws {
        let dark = try #require(GroundDepth.centreLuminance(of: image(20, 24, 50)))
        let bright = try #require(GroundDepth.centreLuminance(of: image(190, 210, 235)))
        #expect(dark < 0.05)
        #expect(bright > 0.5)
        #expect(GroundDepth.depth(forLuminance: dark) == 0.55)
        #expect(GroundDepth.depth(forLuminance: bright) == 0.80)
    }

    @Test func theThresholdIsAtFifteenPercent() {
        #expect(GroundDepth.depth(forLuminance: 0.15) == 0.55)
        #expect(GroundDepth.depth(forLuminance: 0.151) == 0.80)
    }
}
