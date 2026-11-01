// A server that stopped answering says when it last did: the time alone for today, the date and
// time before that, so an older time never reads as today's.

@testable import KaeriUI
import Foundation
import Testing

struct ServerSeenTests {
    private var calendar: Calendar {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = TimeZone(identifier: "UTC")!
        return calendar
    }

    private let locale = Locale(identifier: "en_GB")
    /// 1 November 2026, 14:20 UTC.
    private let now = Date(timeIntervalSince1970: 1_793_542_800)

    @Test func todayShowsTheTimeAlone() {
        let seen = now.addingTimeInterval(-2 * 3_600)
        let text = FirstPartySections.seenTime(seen, now: now, calendar: calendar, locale: locale)
        #expect(text == "12:20")
    }

    @Test func anEarlierDayShowsTheDateAndTheTime() {
        let seen = now.addingTimeInterval(-35 * 3_600 - 56 * 60)
        let text = FirstPartySections.seenTime(seen, now: now, calendar: calendar, locale: locale)
        #expect(text.contains("31 Oct"))
        #expect(text.contains("02:24"))
    }
}
