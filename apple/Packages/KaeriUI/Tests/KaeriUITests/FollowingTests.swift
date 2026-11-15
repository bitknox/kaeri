// How people followed on AniList read: the line under a name, the score, the phone row's names
// and the sentence a mark says.

import KaeriKit
@testable import KaeriUI
import Foundation
import Testing

private func person(_ name: String, _ status: LibrarySegment?, progress: UInt32 = 0, score: UInt32? = nil) -> FollowedEntry {
    FollowedEntry(name: name, avatarUrl: nil, status: status, score: score, progress: progress)
}

private func mark(_ name: String) -> FollowedMark {
    FollowedMark(name: name, avatarUrl: nil)
}

private func row(_ number: Int, _ names: [String]) -> EpisodeRow {
    var row = EpisodeRow(
        number: String(number), title: nil, thumbnail: nil, description: nil, durationSecs: nil,
        watched: 0, completed: true, kind: nil, skippable: false, airsAt: nil)
    row.followed = names.map(mark)
    return row
}

@Suite(.britishEnglish)
@MainActor
struct FollowingTests {
    @Test func theLineSaysHowFarWatchingPausedAndDroppedGo() {
        #expect(FollowWords.line(person("Lumi", .watching, progress: 8)) == "Watching · Episode 8")
        #expect(FollowWords.line(person("TeoVale", .paused, progress: 5)) == "Paused · Episode 5")
        #expect(FollowWords.line(person("Yunami", .dropped, progress: 3)) == "Dropped · Episode 3")
    }

    @Test func completedPlanningAndNoProgressSayTheStatusAlone() {
        #expect(FollowWords.line(person("Rowanpaper", .completed, progress: 12)) == "Completed")
        #expect(FollowWords.line(person("Kofi", .planning)) == "Planning")
        #expect(FollowWords.line(person("Halden", .watching)) == "Watching")
        #expect(FollowWords.line(person("Halden", nil, progress: 4)).isEmpty)
    }

    @Test func theScoreIsOutOfTenAndLeftOutWhenNone() {
        #expect(FollowWords.score(person("Ines", .watching, score: 80)) == 8)
        #expect(FollowWords.score(person("Ines", .watching, score: 75)) == 8)
        #expect(FollowWords.score(person("Ines", .watching, score: 4)) == nil)
        #expect(FollowWords.score(person("Ines", .watching, score: 0)) == nil)
        #expect(FollowWords.score(person("Ines", .watching)) == nil)
    }

    @Test func aPersonIsSpokenWithTheScoreInWords() {
        let spoken = FollowWords.spoken(person("Lumi", .watching, progress: 8, score: 70))
        #expect(spoken.contains("Lumi"))
        #expect(spoken.contains("Watching · Episode 8"))
        #expect(spoken.contains("Score 7 out of 10"))
        #expect(!spoken.contains("7/10"))
    }

    @Test func thePhoneRowNamesOneTwoOrTwoAndMore() {
        #expect(FollowWords.names(["Ines"]) == "Ines")
        #expect(FollowWords.names(["Ines", "Lumi"]) == "Ines and Lumi")
        #expect(FollowWords.names(["Ines", "Lumi", "SannaQ", "Dario", "Kofi"]) == "Ines, Lumi and 3 more")
    }

    @Test func aMarkNamesEveryoneInTheListFormatAndNoScore() {
        #expect(FollowWords.marked([]) == nil)
        #expect(FollowWords.marked([mark("Ines")]) == "Ines watched up to here.")
        #expect(FollowWords.marked([mark("Lumi"), mark("Dario"), mark("Oskar")]) == "Lumi, Dario and Oskar watched up to here.")
    }

    @Test func aFoldedRunCarriesItsRowsMarksInOrderEachOnce() {
        let rows = [row(1, []), row(3, ["Yunami"]), row(5, ["TeoVale", "Ines"]), row(6, ["Ines"])]
        #expect(FollowWords.union(rows).map(\.name) == ["Yunami", "TeoVale", "Ines"])
    }

    @Test func anEpisodeRowsValueEndsWithWhoStoppedThere() {
        let e = entry { _ in }
        let value = row(8, ["Lumi", "Dario"]).spokenValue(e, current: false, upcoming: false)
        #expect(value.last == "Lumi and Dario watched up to here.")
        #expect(row(9, []).spokenValue(e, current: false, upcoming: false).allSatisfy { !$0.contains("watched up to here") })
    }
}
