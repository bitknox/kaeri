// How an episode row is sized, what its facts say, when it keeps room for text, what spoiler
// protection hides and how a long series splits into ranges.

import Foundation
import KaeriKit
@testable import KaeriUI
import Testing

@MainActor
struct EpisodeSynopsisTests {
    /// Episodes numbered from 1, the first `watched` of them watched, each with `text` if given.
    func rows(
        _ count: Int, watched: Int = 0, text: (Int) -> String? = { _ in nil }, pending: Bool = false
    ) -> [EpisodeRow] {
        (0..<count).map { index in
            var row = EpisodeRow(
                number: String(index + 1), title: "Episode \(index + 1)", thumbnail: nil,
                description: text(index + 1), durationSecs: 1440, watched: 0, completed: index < watched,
                kind: nil, skippable: false, airsAt: nil
            )
            row.synopsisPending = pending
            // What the engine says: after the next episode, which comes after those watched.
            row.spoilerAhead = index > watched
            return row
        }
    }

    // MARK: Sizes

    @Test func theMacSetsTextBesideTheStillFromFiveSixtyPoints() {
        #expect(EpisodeMetrics.mac(listWidth: 560) == .macWide)
        #expect(EpisodeMetrics.mac(listWidth: 559) == .macNarrow)
        #expect(EpisodeMetrics.macWide.still == CGSize(width: 178, height: 100))
        #expect(EpisodeMetrics.macNarrow.still == CGSize(width: 144, height: 81))
        #expect(EpisodeMetrics.pad.still == CGSize(width: 192, height: 108))
        #expect(EpisodeMetrics.phone.still == CGSize(width: 144, height: 81))
    }

    @Test func aRowIsTallOnlyWhereTextCanLand() {
        let phone = EpisodeMetrics.phone
        #expect(phone.height(room: true, protected: false) == 172)
        #expect(phone.height(room: true, protected: true) == 100, "protected rows are short")
        #expect(phone.height(room: false, protected: false) == 100, "no text coming")
        #expect(EpisodeMetrics.macNarrow.height(room: true, protected: false) == 164)
        #expect(EpisodeMetrics.macNarrow.height(room: false, protected: false) == 100)
    }

    @Test func whereTextSitsBesideTheStillTheRowDoesNotChangeHeight() {
        for metrics in [EpisodeMetrics.macWide, .pad] {
            let heights = Set([true, false].flatMap { room in
                [true, false].map { metrics.height(room: room, protected: $0) }
            })
            #expect(heights.count == 1)
        }
        #expect(EpisodeMetrics.macWide.tall == 120)
        #expect(EpisodeMetrics.pad.tall == 128)
    }

    // MARK: Room for text

    @Test func roomIsKeptWhereTextIsPresentOrExpected() {
        #expect(EpisodeRoom.room(for: rows(5, text: { $0 == 2 ? "A storm." : nil })))
        #expect(EpisodeRoom.room(for: rows(5, pending: true)), "a lookup is running")
        #expect(!EpisodeRoom.room(for: rows(5)), "none, and none coming")
    }

    @Test func textForATableLaidOutShortWaitsForTheNextLayout() {
        var room = EpisodeRoom()
        room.settle(rows(5))
        #expect(room.room == false)
        room.settle(rows(5, text: { _ in "Late text." }))
        #expect(room.room == false, "text turning up does not grow rows on screen")
        room.settle(rows(5, text: { _ in "Late text." }), again: true)
        #expect(room.room, "the next layout has room for it")
    }

    @Test func roomKeptForTextThatNeverComesStaysAsSpace() {
        var room = EpisodeRoom()
        room.settle(rows(5, pending: true))
        #expect(room.room)
        room.settle(rows(5))
        #expect(room.room, "the table is laid out once")
    }

    // MARK: Spoiler protection

    @Test func protectionHidesWhatComesAfterTheNextEpisodeAndWhatIsNotWatched() {
        let table = rows(10, watched: 6)
        let hidden = table.filter { EpisodeProtection.hides($0, spoilers: true) }.map(\.number)
        #expect(hidden == ["8", "9", "10"])
        #expect(table.allSatisfy { !EpisodeProtection.hides($0, spoilers: false) })
        #expect(EpisodeProtection.startsAt(table, spoilers: true) == "8")
        #expect(EpisodeProtection.startsAt(table, spoilers: false) == nil)
        #expect(EpisodeProtection.startsAt(rows(5, watched: 5), spoilers: true) == nil)
    }

    @Test func aWatchedRowKeepsItsTextWhateverItsNumber() {
        var row = rows(3)[2]
        #expect(EpisodeProtection.hides(row, spoilers: true))
        row.spoilerAhead = false
        #expect(!EpisodeProtection.hides(row, spoilers: true))
    }

    @Test func aRevealCoversOneEpisodeOrEveryRowAfterTheLine() {
        let row = rows(10, watched: 6)[8]
        func hidden(_ shown: Set<String>, all: Bool) -> Bool {
            EpisodeProtection.hidden(row, spoilers: true, revealed: shown, all: all)
        }
        #expect(hidden([], all: false))
        #expect(!hidden(["9"], all: false))
        #expect(hidden(["8"], all: false))
        #expect(!hidden([], all: true))
    }

    // MARK: Facts

    @Test func theFactsSayLengthThenWhenItAiredThenWhatMarksIt() {
        var row = rows(3)[1]
        row.airsAt = 1_400_000_000
        let line = EpisodeFacts.line(row, current: false, minutesLeft: nil, leadsWithNumber: false)
        #expect(line.hasPrefix("24 min · Aired "))
        row.kind = .filler
        #expect(EpisodeFacts.line(row, current: false, minutesLeft: nil, leadsWithNumber: false).hasSuffix(" · Filler"))
    }

    @Test func theFactsOfTheEpisodeInProgressSayHowMuchIsLeft() {
        let row = rows(3)[1]
        #expect(EpisodeFacts.line(row, current: true, minutesLeft: 14, leadsWithNumber: false) == "14 min left")
        #expect(EpisodeFacts.line(row, current: false, minutesLeft: 14, leadsWithNumber: false) == "24 min")
    }

    @Test func aWatchedEpisodeSaysWatchedAndPhonesLeadWithTheNumber() {
        let row = rows(8, watched: 8)[6]
        #expect(EpisodeFacts.line(row, current: false, minutesLeft: nil, leadsWithNumber: false) == "Watched")
        #expect(EpisodeFacts.line(row, current: false, minutesLeft: nil, leadsWithNumber: true) == "E7 · Watched")
    }

    @Test func aRowWithNoLengthAndNoDateHasNoFactsRatherThanAGap() {
        var row = rows(3)[1]
        row.durationSecs = nil
        #expect(EpisodeFacts.line(row, current: false, minutesLeft: nil, leadsWithNumber: false) == "")
    }

    // MARK: Ranges

    @Test func aShowOfAHundredOrFewerHasNoRanges() {
        #expect(EpisodePlan(rows: rows(100), picked: nil).ranges.isEmpty)
        #expect(EpisodePlan(rows: rows(100), picked: nil).shown.count == 100)
    }

    @Test func aLongShowOpensOnTheRangeOfTheNextEpisodeWithItsWatchedRunFolded() throws {
        let table = rows(230, watched: 147)
        let plan = EpisodePlan(rows: table, picked: nil)
        #expect(plan.ranges.count == 3)
        #expect(plan.index == 1)
        #expect(plan.shown.first?.number == "101")
        #expect(plan.shown.count == 100)
        let fold = try #require(plan.fold)
        #expect(fold.count == 47)
        #expect(fold.label == "Episodes 101 to 147 watched")
    }

    @Test func aPickedRangeWinsAndAWatchedRangeFoldsNothing() {
        let table = rows(230, watched: 147)
        let first = EpisodePlan(rows: table, picked: 0)
        #expect(first.index == 0)
        #expect(first.fold == nil, "a whole watched range folds nothing")
        #expect(EpisodePlan(rows: table, picked: 9).index == 2, "clamped to the last range")
        #expect(EpisodePlan(rows: table, picked: 2).shown.count == 30)
    }

    @Test func theRangeMenuNamesRangesByTheirNumbersAndSaysWatchedOrNotOut() {
        var table = rows(230, watched: 100)
        let now = Int64(Date.now.timeIntervalSince1970)
        for index in 200..<230 { table[index].airsAt = now + 86_400 }
        let plan = EpisodePlan(rows: table, picked: nil)
        #expect(EpisodePlan.title(plan.ranges[1], in: table) == "101–200")
        #expect(EpisodePlan.item(plan.ranges[0], in: table) == "1–100 · Watched")
        #expect(EpisodePlan.item(plan.ranges[1], in: table) == "101–200")
        #expect(EpisodePlan.item(plan.ranges[2], in: table) == "201–230 · Not out yet")
    }

    // MARK: Language

    @Test func aSynopsisIsTaggedWithTheLanguageItIsWrittenIn() {
        #expect(EpisodeLanguage.of("A storm closes the pass with the mail still aboard.") == "en")
        #expect(EpisodeLanguage.of("嵐が峠を閉ざし、郵便を載せたまま船は動けなくなる。") == "ja")
        #expect(EpisodeLanguage.of("") == nil)
    }
}
