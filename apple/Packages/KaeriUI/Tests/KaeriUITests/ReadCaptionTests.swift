// The line under a reading cover, by where the cover is: Home's rows, View all and Search.

import KaeriKit
import KaeriStore
@testable import KaeriUI
import Foundation
import Testing

@MainActor
@Suite(.britishEnglish)
struct ReadCaptionTests {
    /// A finished light novel from Japan with 9 volumes, as the engine's facts say it.
    func tile(next: Msg? = nil, started: Msg? = nil, finished: Bool = true) -> WorkTile {
        WorkTile(
            work: "a:900301", title: "Lantern Street Notes", medium: .novel, cover: nil, // i18n-skip
            formatLabel: "novel", progress: nil, next: next, newChapter: false, adult: false, // i18n-skip
            facts: [.reading(.readingKindLightNovel)]
                + (finished ? [.reading(.readingStatusFinished), .reading(.readingVolumes(count: 9))]
                            : [.reading(.readingStatusReleasing)]),
            demo: false, notInSources: false, origin: .reading(.readingOriginJapan), amount: nil,
            started: started)
    }

    func query(formats: [String] = [], status: String? = nil) -> ReadSearchQuery {
        var query = ReadingModel.emptyQuery
        query.formats = formats
        query.status = status
        return query
    }

    @Test func aMixedRowSaysTheFormatThenTheStatus() {
        #expect(ReadWords.line(tile(finished: false), .mixedRow(statusFixed: false)) == "Light novel · Releasing")
    }

    @Test func aRowThatFixesTheStatusSaysTheCountInstead() {
        #expect(ReadWords.line(tile(), .formatRow(statusFixed: true)) == "Japan · 9 volumes")
        #expect(ReadWords.line(tile(), .mixedRow(statusFixed: true)) == "Light novel · 9 volumes")
        #expect(ReadWords.line(tile(), .formatRow(statusFixed: false)) == "Japan · Finished")
    }

    @Test func aNewRowSaysWhenItStarted() {
        let started = tile(started: .reading(.readingSinceMarch), finished: false)
        #expect(ReadWords.line(started, .formatRow(statusFixed: false)) == "Japan · since March")
    }

    @Test func searchSaysTheFormatAndOriginEvenForASeriesBeingRead() {
        let reading = tile(next: .reading(.readingNextChapter(chapter: "13")))
        #expect(ReadWords.line(reading, .formatOrigin) == "Light novel · Japan")
        // On the person's own shelves it says what comes next.
        #expect(ReadWords.line(reading, .plain) == "Chapter 13")
    }

    @Test func viewAllKeepsTheRowsCaption() {
        #expect(ReadCaption.list(query(formats: ["manga"]), viewAll: true) == .formatRow(statusFixed: false))
        #expect(ReadCaption.list(query(formats: ["manga"], status: "finished"), viewAll: true) == .formatRow(statusFixed: true))
        #expect(ReadCaption.list(query(formats: ["manga", "novel"]), viewAll: false) == .mixedRow(statusFixed: false))
        // A genre row's View all names no format.
        #expect(ReadCaption.list(query(), viewAll: true) == .mixedRow(statusFixed: false))
        #expect(ReadCaption.list(query(), viewAll: false) == .formatOrigin)
    }
}
