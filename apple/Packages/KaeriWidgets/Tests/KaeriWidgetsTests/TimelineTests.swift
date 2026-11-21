import Foundation
import Testing
@testable import KaeriWidgetData

@Suite("Stale")
struct StalenessTests {
    let snapshot = WidgetSnapshot(version: 2, generatedAt: at(day: 20, 19, 10))

    @Test("Continue turns stale a day after the snapshot was made, from that moment on")
    func continueStale() {
        #expect(Staleness.continueStale(snapshot) == at(day: 21, 19, 10))
        #expect(!Staleness.isContinueStale(snapshot, at: at(day: 21, 19, 9)))
        #expect(Staleness.isContinueStale(snapshot, at: at(day: 21, 19, 10)), "the timeline's entry at that moment draws it stale")
    }

    @Test("A version 2 schedule covers its day and the six after; a version 1 its day alone")
    func scheduleCoverage() {
        #expect(Staleness.coversDay(snapshot, at: at(day: 20, 0, 0), calendar: copenhagen))
        #expect(Staleness.coversDay(snapshot, at: at(day: 26, 23, 59), calendar: copenhagen))
        #expect(!Staleness.coversDay(snapshot, at: at(day: 27, 0, 0), calendar: copenhagen))
        #expect(!Staleness.coversDay(snapshot, at: at(day: 19, 23, 0), calendar: copenhagen))
        var one = snapshot
        one.version = 1
        #expect(Staleness.coversDay(one, at: at(day: 20, 23, 59), calendar: copenhagen))
        #expect(!Staleness.coversDay(one, at: at(day: 21, 0, 0), calendar: copenhagen))
    }

    @Test("A week across the clocks going back still ends at midnight")
    func acrossDaylightSaving() {
        // Copenhagen leaves summer time on Sunday 25 October 2026.
        #expect(Staleness.coversDay(snapshot, at: at(day: 26, 12), calendar: copenhagen))
        #expect(!Staleness.coversDay(snapshot, at: at(day: 27, 0, 30), calendar: copenhagen))
    }
}

/// The device's calendar, which Continue's timeline counts its midnights in.
private let device: Calendar = {
    var calendar = Calendar(identifier: .gregorian)
    calendar.timeZone = .current
    return calendar
}()

/// A moment of October 2026 on the device's clock.
private func local(day: Int, _ hour: Int, _ minute: Int = 0) -> Date {
    device.date(from: DateComponents(year: 2026, month: 10, day: day, hour: hour, minute: minute))!
}

/// The midnights that start the days `days` of October 2026 on the device's clock.
private func midnights(_ days: ClosedRange<Int>) -> [Date] {
    days.map { local(day: $0, 0) }
}

@Suite("Timelines")
struct TimelineTests {
    @Test("Continue: now, the moment it turns stale and each midnight of the week ahead, never asked again")
    func continuePlan() {
        let snapshot = WidgetSnapshot(
            generatedAt: local(day: 20, 19, 10),
            continueWatching: [SnapshotContinue(id: 900001, title: "Hoshikuzu no Tabibito", episode: 7)])
        let plan = WidgetTimeline.continuePlan(snapshot, now: local(day: 20, 19, 30))
        #expect(plan.dates == [local(day: 20, 19, 30), local(day: 21, 0), local(day: 21, 19, 10)] + midnights(22...27))
        #expect(plan.reloadAt == nil)
        // Already stale: now, and the midnights its "Updated …" changes at.
        #expect(WidgetTimeline.continuePlan(snapshot, now: local(day: 22, 9)).dates == [local(day: 22, 9)] + midnights(23...29))
        #expect(WidgetTimeline.continuePlan(nil, now: local(day: 20, 19, 30)).dates == [local(day: 20, 19, 30)])
    }

    @Test("Continue, caught up: an entry when the next episode airs")
    func caughtUpPlan() {
        let snapshot = WidgetSnapshot(
            generatedAt: local(day: 20, 19, 10), signedIn: true,
            nextAiring: SnapshotNextAiring(id: 900002, title: "Kumo no Ue no Toshokan", episode: 6, airsAt: local(day: 20, 22)))
        let plan = WidgetTimeline.continuePlan(snapshot, now: local(day: 20, 19, 30))
        #expect(plan.dates == [local(day: 20, 19, 30), local(day: 20, 22), local(day: 21, 0), local(day: 21, 19, 10)] + midnights(22...27))
        #expect(plan.reloadAt == nil)
    }

    @Test("Continue, caught up with the next episode past the week's midnights: asked again at the last of them")
    func caughtUpFarOffPlan() {
        let snapshot = WidgetSnapshot(
            generatedAt: local(day: 20, 19, 10), signedIn: true,
            nextAiring: SnapshotNextAiring(id: 900002, title: "Kumo no Ue no Toshokan", episode: 7, airsAt: local(day: 30, 16, 30)))
        let plan = WidgetTimeline.continuePlan(snapshot, now: local(day: 20, 19, 30))
        #expect(plan.reloadAt == local(day: 27, 0))
        #expect(Array(plan.dates.suffix(2)) == [local(day: 27, 0), local(day: 30, 16, 30)])
    }

    @Test("Airing today: now, each broadcast still to come today, and midnight, when it is made again")
    func airingPlan() {
        let rows = [
            SnapshotAiring(id: 1, title: "A", episode: 1, airsAt: at(day: 20, 13)),
            SnapshotAiring(id: 2, title: "B", episode: 1, airsAt: at(day: 20, 21)),
            SnapshotAiring(id: 3, title: "C", episode: 1, airsAt: at(day: 20, 23, 45)),
            SnapshotAiring(id: 4, title: "D", episode: 1, airsAt: at(day: 21, 9)),
        ]
        let plan = WidgetTimeline.airingPlan(rows, now: at(day: 20, 19, 30), calendar: copenhagen)
        #expect(plan.dates == [at(day: 20, 19, 30), at(day: 20, 21), at(day: 20, 23, 45), at(day: 21, 0)])
        #expect(plan.reloadAt == at(day: 21, 0))
    }

    @Test("Airing today from the public schedule is asked again within three hours")
    func publicPlan() {
        let rows = [
            SnapshotAiring(id: 2, title: "B", episode: 1, airsAt: at(day: 20, 21)),
            SnapshotAiring(id: 3, title: "C", episode: 1, airsAt: at(day: 20, 23, 45)),
        ]
        let plan = WidgetTimeline.airingPlan(rows, now: at(day: 20, 19, 30), publicSchedule: true, calendar: copenhagen)
        #expect(plan.reloadAt == at(day: 20, 22, 30))
        #expect(plan.dates == [at(day: 20, 19, 30), at(day: 20, 21)])
    }
}

/// Continue drawn as WidgetKit draws a timeline: each moment shows the entry at or before it, and
/// the timeline is made again at its reload date. What it says must be what that moment says.
@Suite("Continue's days")
struct ContinueDayTests {
    let format = WidgetFormat(calendar: device, locale: Locale(identifier: "en_GB"))

    /// What Continue says at `now` that changes with the time: stale or not, and the days it names.
    func words(_ snapshot: WidgetSnapshot, at now: Date) -> String {
        switch ContinuePresentation.content(snapshot, at: now) {
        case .signedOut: "signed out"
        case .caughtUp(let next?, _): format.airs(next, from: now)
        case .caughtUp(nil, _): "caught up"
        case .shows(_, let updated?): format.updated(updated, from: now)
        case .shows(_, nil): "progress"
        }
    }

    /// The first half hour of the `days` from `start` at which the widget shows other words than
    /// that moment's, with both; nil when it never does.
    func firstWrong(_ snapshot: WidgetSnapshot, from start: Date, days: Int) -> String? {
        var plan = WidgetTimeline.continuePlan(snapshot, now: start)
        for step in 0...(days * 48) {
            let moment = start.addingTimeInterval(TimeInterval(step) * 1_800)
            if let reload = plan.reloadAt, moment >= reload {
                plan = WidgetTimeline.continuePlan(snapshot, now: reload)
            }
            guard let entry = plan.dates.last(where: { $0 <= moment }) else { return "\(moment): no entry" }
            let (shown, due) = (words(snapshot, at: entry), words(snapshot, at: moment))
            if shown != due { return "\(format.longDay(moment)) \(format.time(moment)): \"\(shown)\" for \"\(due)\"" }
        }
        return nil
    }

    @Test func continueSaysTheRightDayAtEveryHalfHour() {
        // Caught up late in the evening with the next episode tomorrow afternoon: from midnight
        // it airs today.
        let next = SnapshotNextAiring(id: 900002, title: "Kumo no Ue no Toshokan", episode: 6, airsAt: local(day: 21, 16, 30))
        let caughtUp = WidgetSnapshot(generatedAt: local(day: 20, 22), signedIn: true, nextAiring: next)
        #expect(firstWrong(caughtUp, from: local(day: 20, 22), days: 3) == nil)

        // The app closed for ten days, as a Mac's can be: stale a day on, then updated yesterday,
        // on a weekday, and on a date from a week on.
        let item = SnapshotContinue(id: 900001, title: "Hoshikuzu no Tabibito", episode: 7, fraction: 0.4, resume: true)
        let stale = WidgetSnapshot(generatedAt: local(day: 20, 19, 10), signedIn: true, continueWatching: [item])
        #expect(firstWrong(stale, from: local(day: 20, 19, 30), days: 10) == nil)

        // Caught up with the next episode three weeks off: its date, then its weekday, tomorrow
        // and today, past the week the first timeline covers.
        let farOff = device.date(byAdding: .day, value: 21, to: local(day: 20, 16, 30))!
        var later = caughtUp
        later.nextAiring = SnapshotNextAiring(id: 900002, title: "Kumo no Ue no Toshokan", episode: 7, airsAt: farOff)
        #expect(firstWrong(later, from: local(day: 20, 22), days: 22) == nil)
    }
}
