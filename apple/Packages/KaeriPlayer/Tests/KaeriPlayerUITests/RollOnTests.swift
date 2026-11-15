// Rolling on to the next episode: the ground alone for 600 ms, so a warmed episode goes from the
// ground straight to its picture, and a starting screen that does show stays its 0.6 s.

import KaeriKit
import KaeriPlayer
import Foundation
import Testing

@testable import KaeriPlayerUI

@MainActor
struct RollOnTests {
    /// Episode 7 playing, with Up next for 8.
    func playing(_ rig: Rig) async throws {
        rig.model.apply(.playbackStaged(id: 900_001, title: "Stardust Wayfarer", episode: "7", episodeTitle: nil, speed: 1))
        #expect(!rig.model.startingQuiet, "a start of its own shows the starting screen at once")
        try await rig.opened(1)
        rig.mpv.emit(.playbackRestart)
        try await until("the picture") { rig.model.hasPicture }
        rig.ui(1, .upNext(next: NextEpisode(number: "8", title: nil, thumbnail: nil), auto: true, endingAt: 42, countdown: 10, stillWatchingAfter: 3))
    }

    /// The engine stages episode 8 and loads its file into the same mpv.
    func nextLoads(_ rig: Rig) async throws {
        rig.model.apply(.playbackStaged(id: 900_001, title: "Stardust Wayfarer", episode: "8", episodeTitle: nil, speed: 1))
        rig.ui(2, .starting(id: 900_001, title: "Stardust Wayfarer", episode: "8", episodeTitle: nil, coverUrl: nil, bannerUrl: nil))
        rig.ops(1, [.stop])
        rig.ops(2, [.load(url: "/media/8.mkv", options: [])])
        await rig.handedOver()
        rig.mpv.startFile()
    }

    func firstFrame(_ rig: Rig) async throws {
        rig.mpv.emit(.playbackRestart)
        try await until("the next picture") { rig.model.hasPicture }
    }

    @Test func aWarmedRollOnGoesFromTheGroundStraightToThePicture() async throws {
        let rig = Rig()
        try await playing(rig)
        rig.model.playNext()
        try await nextLoads(rig)
        #expect(rig.model.startingQuiet, "the ground alone")
        rig.step(.milliseconds(400))
        try await firstFrame(rig)
        #expect(rig.model.starting == nil)
        #expect(!rig.model.startingHeld)
        #expect(rig.model.startingQuiet, "no starting screen while the ground fades to the picture")
        try await afterTurns("the fade's wait") { rig.time.sleeping(PlayerTiming.rollOnPictureFades) }
        rig.step(PlayerTiming.rollOnPictureFades)
        try await afterTurns("quiet over") { !rig.model.startingQuiet }
    }

    @Test func aSlowRollOnShowsTheStartingScreenAt600msAndHoldsIt() async throws {
        let rig = Rig()
        try await playing(rig)
        // The engine rolled on by itself at the end.
        rig.model.apply(.playbackEnded(watched: true, ending: .complete(next: "8"), rollingOn: true))
        try await nextLoads(rig)
        #expect(rig.model.startingQuiet)
        try await afterTurns("the quiet's wait") { rig.time.sleeping(PlayerTiming.rollOnQuiet) }
        rig.step(.milliseconds(600))
        try await afterTurns("the starting screen") { !rig.model.startingQuiet }

        // The first frame 100 ms after the screen came: it stays up until 1.2 s.
        rig.step(.milliseconds(100))
        try await firstFrame(rig)
        #expect(rig.model.startingHeld, "held its 0.6 s")
        try await afterTurns("the hold's wait") { rig.time.sleeping(.milliseconds(500)) }
        rig.step(.milliseconds(500))
        try await afterTurns("the hold over") { !rig.model.startingHeld }
    }

    @Test func aStartingScreenUpLongEnoughIsNotHeld() async throws {
        let rig = Rig()
        try await playing(rig)
        rig.model.playNext()
        try await nextLoads(rig)
        try await afterTurns("the quiet's wait") { rig.time.sleeping(PlayerTiming.rollOnQuiet) }
        rig.step(.milliseconds(600))
        try await afterTurns("the starting screen") { !rig.model.startingQuiet }
        rig.step(.seconds(2))
        try await firstFrame(rig)
        #expect(!rig.model.startingHeld)
    }

    @Test func aStartAfterTheRollOnLapsedShowsAtOnce() async throws {
        let rig = Rig()
        try await playing(rig)
        rig.model.playNext()
        // Nothing came of it; much later the viewer picks an episode.
        rig.step(.seconds(30))
        try await nextLoads(rig)
        #expect(!rig.model.startingQuiet)
        try await firstFrame(rig)
        #expect(!rig.model.startingHeld)
    }
}
