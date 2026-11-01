// What Home's cards say and do from what the engine sent and what the sources check found.

import Foundation
import KaeriKit
@testable import KaeriUI
import Testing

struct HomeTests {
    @Test func aMissTurnsPlayIntoAddToPlanning() {
                #expect(HomeFacts(playable: .notFound, action: .play, listed: false).action == .addToPlanning)
        #expect(HomeFacts(playable: .notFound, action: .resume, listed: false).action == .addToPlanning)
        #expect(HomeFacts(playable: .found, action: .play, listed: false).action == .play)
        // A check that could not tell changes nothing.
        #expect(HomeFacts(playable: .unknown, action: .resume, listed: false).action == .resume)
    }

    @Test func aMissOnATitleOnTheListOffersDetailsNotPlanning() {
        #expect(HomeFacts(playable: .notFound, action: .resume, listed: true).action == .details)
        #expect(HomeFacts(playable: .notFound, action: .play, listed: true).action == .details)
        #expect(HomeFacts(playable: .found, action: .resume, listed: true).action == .resume)
        #expect(HomeFacts(playable: .unknown, action: .play, listed: true).action == .play)
    }

    @Test func nothingToPressHasNoMainAction() {
        #expect(HomeMain(action: .nothing, episode: 1) == nil)
        #expect(HomeMain(action: .resume, episode: 3)?.symbol == "play.fill")
        #expect(HomeMain(action: .addToPlanning, episode: 1)?.symbol == "plus")
        #expect(HomeMain(action: .details, episode: 5)?.symbol == nil)
    }

    @Test func aStartDateIsWordedByItsPrecision() {
        #expect(HomeWords.starts(nil) == nil)
        #expect(HomeWords.starts(FuzzyDate(year: 2027, month: nil, day: nil)) == "2027")
        #expect(HomeWords.starts(FuzzyDate(year: 2027, month: 4, day: nil))?.contains("2027") == true)
        #expect(HomeWords.starts(FuzzyDate(year: 2027, month: 4, day: 9))?.contains("2027") == false)
    }

    @Test func todayShowsTheWindowAroundNow() {
        let airs: [Int64] = [100, 200, 300, 400, 500]
        #expect(Self.pick(50, airs) == [100, 200, 300])
        #expect(Self.pick(250, airs) == [200, 300, 400])
        #expect(Self.pick(350, airs) == [300, 400, 500])
        #expect(Self.pick(450, airs) == [300, 400, 500])
        #expect(Self.pick(900, airs) == [300, 400, 500])
        #expect(Self.pick(250, [100, 200]) == [100, 200])
    }

    private static func pick(_ now: Int64, _ list: [Int64]) -> [Int64] {
        HomeDay.window(list, now: Date(timeIntervalSince1970: TimeInterval(now)), shown: 3, airs: { (t: Int64) -> Int64? in t })
    }
}
