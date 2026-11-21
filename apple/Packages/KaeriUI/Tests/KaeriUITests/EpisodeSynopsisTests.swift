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

    /// The middle Mac size, with the list at 553 pt.
    private static let middle = EpisodeMetrics.mac(listWidth: 553)

    /// The room a Mac row keeps over and under its still.
    private static func room(_ metrics: EpisodeMetrics) -> CGFloat {
        (metrics.short - metrics.still.height) / 2
    }

    @Test func theMacSetsTextBesideTheStillDownToFiveHundredPoints() {
        #expect(EpisodeMetrics.mac(listWidth: 560) == .macWide)
        for width: CGFloat in [559, 553, 500] {
            let metrics = EpisodeMetrics.mac(listWidth: width)
            #expect(metrics.text == .beside, "text beside the still at \(width)")
            #expect(metrics.still == CGSize(width: 144, height: 81))
        }
        #expect(EpisodeMetrics.mac(listWidth: 499) == .macNarrow)
        #expect(EpisodeMetrics.mac(listWidth: 450) == .macNarrow)
        #expect(EpisodeMetrics.macWide.text == .beside)
        #expect(EpisodeMetrics.macNarrow.text == .under)
        #expect(EpisodeMetrics.macWide.still == CGSize(width: 178, height: 100))
        #expect(EpisodeMetrics.macNarrow.still == CGSize(width: 144, height: 81))
        #expect(EpisodeMetrics.pad.still == CGSize(width: 192, height: 108))
        #expect(EpisodeMetrics.phone.still == CGSize(width: 144, height: 81))
    }

    @Test func aRowIsTallOnlyWhereTextCanLand() {
        let phone = EpisodeMetrics.phone
        #expect(phone.height(room: true, protected: false) == 182)
        #expect(phone.height(room: true, protected: true) == 114, "protected rows are short")
        #expect(phone.height(room: false, protected: false) == 114, "no text coming")
        #expect(EpisodeMetrics.macNarrow.height(room: true, protected: false) == 161)
        #expect(EpisodeMetrics.macNarrow.height(room: false, protected: false) == 101)
    }

    @Test func aPhoneRowEndsInARuleSixteenPointsUnderItsText() {
        let phone = EpisodeMetrics.phone
        #expect(phone.ruled)
        #expect(phone.pad == 16)
        // 16 over the still, the still, 8, three lines of 20, 16 under them, then the rule.
        #expect(phone.tall == phone.pad + phone.still.height + 8 + 3 * 20 + phone.pad + EpisodeMetrics.rule)
        // A short row is the still between the same room and the rule.
        #expect(phone.short == phone.pad + phone.still.height + phone.pad + EpisodeMetrics.rule)
        // The Mac and the iPad have no rule.
        for metrics in [EpisodeMetrics.macWide, Self.middle, .macNarrow, .pad] {
            #expect(!metrics.ruled)
        }
    }

    @Test func aNarrowMacRowKeepsTheWideRowsRoomOverAndUnderItsStill() {
        let narrow = EpisodeMetrics.macNarrow
        let wide = EpisodeMetrics.macWide
        #expect(narrow.pad == Self.room(wide))
        #expect(Self.room(wide) == 10)
        // 10 over the still, the still, 10 under it.
        #expect(narrow.short == narrow.pad + narrow.still.height + narrow.pad)
        // 10, the still, 8, three lines of 13 on 18 (16 each and 2 between), 10.
        let lines: CGFloat = 3 * 16 + 2 * 2
        #expect(narrow.tall == narrow.pad + narrow.still.height + 8 + lines + narrow.pad)
    }

    @Test func theMiddleMacRowIsItsStillWithTheWideRowsRoom() {
        let middle = Self.middle
        let room: CGFloat = Self.room(middle)
        #expect(room == Self.room(EpisodeMetrics.macWide))
        #expect(middle.short == middle.still.height + 2 * room)
        #expect(middle.tall == middle.short)
        // The text column beside the smaller still from the switch: the list less its padding,
        // the number, the still, the check and three gaps of 14.
        let fixed: CGFloat = 2 * 8 + 22 + 14 + 3 * 14
        let column: CGFloat = 500 - middle.still.width - fixed
        #expect(column >= 260)
        // A title and facts on a line each and two lines of synopsis fit beside the still.
        let words: CGFloat = 17 + 2 + 15 + 8 + 34
        #expect(words <= middle.still.height)
    }

    @Test func aPhoneRowKeepsItsRoomOnlyWhileItsTextMayStillCome() {
        let phone = EpisodeMetrics.phone
        // Loading: the tall room, so nothing moves while the details come.
        #expect(phone.least(room: true, protected: false, pending: true) == 182)
        #expect(phone.holdsText(room: true, protected: false, pending: true, text: nil))
        // Known with no synopsis: the still, title and facts alone.
        #expect(phone.least(room: true, protected: false, pending: false) == 114)
        #expect(!phone.holdsText(room: true, protected: false, pending: false, text: nil))
        #expect(!phone.holdsText(room: true, protected: false, pending: false, text: ""))
        // Known with a synopsis: from the short row, as tall as its own lines.
        #expect(phone.holdsText(room: true, protected: false, pending: false, text: "A storm."))
        // Protected rows and tables with no text coming stay short and hold none.
        #expect(phone.least(room: true, protected: true, pending: true) == 114)
        #expect(!phone.holdsText(room: true, protected: true, pending: false, text: "A storm."))
        #expect(!phone.holdsText(room: false, protected: false, pending: true, text: nil))
        // The widest Mac row and the iPad keep the room their kind sets, text or not.
        for metrics in [EpisodeMetrics.macWide, .pad] {
            #expect(metrics.least(room: true, protected: false, pending: false) == metrics.tall)
            #expect(metrics.holdsText(room: true, protected: false, pending: false, text: nil))
        }
    }

    @Test func aNarrowMacRowFitsItsTextOnceItsDetailsAreKnown() {
        let narrow = EpisodeMetrics.macNarrow
        // Loading: three lines' room, so nothing moves while the details come.
        #expect(narrow.least(room: true, protected: false, pending: true) == 161)
        #expect(narrow.holdsText(room: true, protected: false, pending: true, text: nil))
        // Known with no synopsis: the still alone, no band under it.
        #expect(narrow.least(room: true, protected: false, pending: false) == 101)
        #expect(!narrow.holdsText(room: true, protected: false, pending: false, text: nil))
        #expect(!narrow.holdsText(room: true, protected: false, pending: false, text: ""))
        // Known with a short synopsis: from the short row, as tall as its own lines.
        #expect(narrow.holdsText(room: true, protected: false, pending: false, text: "A storm."))
        // Protected rows stay short and hold none.
        #expect(narrow.least(room: true, protected: true, pending: true) == 101)
        #expect(!narrow.holdsText(room: true, protected: true, pending: false, text: "A storm."))
    }

    @Test func aMiddleMacRowKeepsItsLinesOpenOnlyWhileTheyMayCome() {
        let middle = Self.middle
        // The still is the floor, pending or not.
        #expect(middle.least(room: true, protected: false, pending: true) == 101)
        #expect(middle.least(room: true, protected: false, pending: false) == 101)
        // Loading: the synopsis may still come, so the row holds it.
        #expect(middle.holdsText(room: true, protected: false, pending: true, text: nil))
        // Known: the row holds only the text it has.
        #expect(!middle.holdsText(room: true, protected: false, pending: false, text: nil))
        #expect(middle.holdsText(room: true, protected: false, pending: false, text: "A storm."))
    }

    @Test func whereTextSitsBesideTheStillTheRowDoesNotChangeHeight() {
        for metrics in [EpisodeMetrics.macWide, Self.middle, .pad] {
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

    @Test func theLineNamesTheLastEpisodeBeforeTheRowsItHides() throws {
        let start = try #require(EpisodeProtection.start(rows(10, watched: 6), spoilers: true))
        #expect(start.row == "8")
        #expect(start.after == 7)
        // A range that opens on hidden rows names the episode before its first.
        let range = Array(rows(230, watched: 6)[150..<200])
        let ranged = try #require(EpisodeProtection.start(range, spoilers: true))
        #expect(ranged.row == "151")
        #expect(ranged.after == 150)
    }

    @Test func aRevealLastsWhileThePageIsOpenUnderAnotherAndEndsWhenItIsClosed() {
        let navigator = Navigator(route: LaunchRoute())
        navigator.openTitle(920_021)
        navigator.showSynopsis(920_021, "8")
        navigator.episodeRange[920_021] = 1
        // Another page over it: the visit goes on.
        navigator.openTitle(920_020)
        #expect(navigator.synopsisIsShown(920_021, "8"))
        #expect(navigator.episodeRange[920_021] == 1)
        navigator.paths[navigator.section] = []
        #if os(macOS)
        navigator.back()
        #endif
        #expect(!navigator.synopsisIsShown(920_021, "8"))
        #expect(navigator.episodeRange[920_021] == nil)
    }

    @Test func theViewMenuStepsThroughTheRangesOnScreenAndStopsAtTheEnds() {
        let navigator = Navigator(route: LaunchRoute())
        navigator.episodeRangesShown = EpisodeRangesShown(title: 920_020, index: 0, count: 3)
        navigator.stepEpisodeRange(by: -1)
        #expect(navigator.episodeRange[920_020] == nil)
        navigator.stepEpisodeRange(by: 1)
        #expect(navigator.episodeRange[920_020] == 1)
        #expect(EpisodeRangesShown(title: 920_020, index: 2, count: 3).hasNext == false)
        #expect(EpisodeRangesShown(title: 920_020, index: 0, count: 3).hasPrevious == false)
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

    @Test func aPhoneRowWhoseTitleIsTheNumberLeavesItOutOfTheFacts() {
        var row = rows(8, watched: 8)[3]
        row.title = nil
        #expect(EpisodeFacts.line(row, current: false, minutesLeft: nil, leadsWithNumber: true) == "Watched")
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

    @Test func aRangeOfOneEpisodeIsNamedByThatEpisode() {
        // A show on the air lists what is out and the next to air: 201 after 200 is a range alone.
        var table = rows(201, watched: 100)
        table[200].airsAt = Int64(Date.now.timeIntervalSince1970) + 86_400
        let plan = EpisodePlan(rows: table, picked: nil)
        #expect(plan.ranges.count == 3)
        #expect(EpisodePlan.item(plan.ranges[2], in: table) == "201 · Not out yet")
    }

    @Test func pickersAListJumpsFarPastStandWhereTheJumpLeftThem() {
        let place = PickersPlace()
        place.offset = 200
        place.saw(395)
        place.offset = 9_000
        #expect(place.now < 0, "scrolled away above, though the row last said 395")
        place.offset = 200
        #expect(place.now == 395)
    }

    // MARK: Language

    @Test func aSynopsisIsTaggedWithTheLanguageItIsWrittenIn() {
        #expect(EpisodeLanguage.of("A storm closes the pass with the mail still aboard.") == "en")
        #expect(EpisodeLanguage.of("嵐が峠を閉ざし、郵便を載せたまま船は動けなくなる。") == "ja")
        #expect(EpisodeLanguage.of("") == nil)
    }
}
