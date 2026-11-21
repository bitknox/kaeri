// When a widget's content changes on its own: Continue turning stale and the days it names
// turning, and Airing today's rows moving from upcoming to out, then the day turning.

import Foundation

/// The spec's thresholds.
public enum Staleness {
    /// Continue shows "Updated …" in place of progress once its snapshot is this old.
    public static let continueAfter: TimeInterval = 24 * 3_600

    /// The moment `snapshot`'s Continue turns stale.
    public static func continueStale(_ snapshot: WidgetSnapshot) -> Date {
        snapshot.generatedAt.addingTimeInterval(continueAfter)
    }

    /// Whether `snapshot`'s Continue is stale at `now`: from that moment on, so the timeline's
    /// entry at it draws the stale content.
    public static func isContinueStale(_ snapshot: WidgetSnapshot, at now: Date) -> Bool {
        now >= continueStale(snapshot)
    }

    /// How many days `snapshot`'s schedule covers, counting the day it was made: a week from
    /// version 2, which carries the six days after; that day alone in version 1.
    public static func scheduleDays(_ snapshot: WidgetSnapshot) -> Int {
        snapshot.version >= 2 ? 7 : 1
    }

    /// Whether `snapshot` holds the schedule of the day `now` is in. Past it, Airing today says
    /// "No schedule for today yet".
    public static func coversDay(
        _ snapshot: WidgetSnapshot, at now: Date, calendar: Calendar = .current
    ) -> Bool {
        let made = calendar.startOfDay(for: snapshot.generatedAt)
        let today = calendar.startOfDay(for: now)
        guard today >= made,
              let end = calendar.date(byAdding: .day, value: scheduleDays(snapshot), to: made)
        else { return false }
        return today < end
    }
}

/// The dates a widget's timeline has entries at, and when it asks for the next timeline.
public struct TimelinePlan: Sendable, Equatable {
    public var dates: [Date]
    /// When WidgetKit should ask again: nil for never, until the app reloads the widgets.
    public var reloadAt: Date?

    public init(dates: [Date], reloadAt: Date?) {
        self.dates = dates
        self.reloadAt = reloadAt
    }
}

public enum WidgetTimeline {
    /// How many midnights ahead Continue has entries at: enough for "Updated …" to reach its last
    /// form, the date it names from a week after the snapshot's day.
    static let continueMidnights = 7

    /// Continue's: now, the moment the snapshot turns stale while that is to come, caught up the
    /// moment the next episode airs, and each of the next seven midnights, since the days it
    /// names count from the entry's date: "Updated yesterday", "Episode 10 airs tomorrow 16:30".
    /// Asked again at the last of those midnights when the next episode airs after it, as its
    /// day still changes then; otherwise never, since the app reloads the widgets whenever it
    /// writes a snapshot.
    public static func continuePlan(
        _ snapshot: WidgetSnapshot?, now: Date, calendar: Calendar = .current
    ) -> TimelinePlan {
        guard let snapshot else { return TimelinePlan(dates: [now], reloadAt: nil) }
        let midnights = Self.midnights(after: now, count: continueMidnights, calendar: calendar)
        var dates = Set([now] + midnights)
        let stale = Staleness.continueStale(snapshot)
        if stale > now { dates.insert(stale) }
        var reload: Date?
        if snapshot.continueWatching.isEmpty, let next = snapshot.nextAiring?.airsAt, next > now {
            dates.insert(next)
            if let last = midnights.last, next > last { reload = last }
        }
        return TimelinePlan(dates: dates.sorted(), reloadAt: reload)
    }

    /// The first moment of each of the `count` days after the one `now` is in.
    static func midnights(after now: Date, count: Int, calendar: Calendar) -> [Date] {
        var midnights: [Date] = []
        var day = now
        while midnights.count < count, let next = calendar.dateInterval(of: .day, for: day)?.end {
            midnights.append(next)
            day = next
        }
        return midnights
    }

    /// How long AniList's public schedule, fetched by the extension itself, is shown before it
    /// is fetched again.
    public static let publicScheduleLifetime: TimeInterval = 3 * 3_600

    /// Airing today's: now, each broadcast of the day still to come, so its row turns from
    /// upcoming to out without the app, and the next midnight, when the day turns and the
    /// timeline is made again from the snapshot's next day. From the public schedule it is
    /// asked again sooner, so a delay shows.
    public static func airingPlan(
        _ rows: [SnapshotAiring], now: Date, publicSchedule: Bool = false, calendar: Calendar = .current
    ) -> TimelinePlan {
        let today = calendar.startOfDay(for: now)
        let midnight = calendar.date(byAdding: .day, value: 1, to: today) ?? now.addingTimeInterval(86_400)
        var dates = Set([now, midnight])
        for row in rows where row.airsAt > now && row.airsAt < midnight {
            dates.insert(row.airsAt)
        }
        var reload = midnight
        if publicSchedule { reload = min(reload, now.addingTimeInterval(publicScheduleLifetime)) }
        return TimelinePlan(dates: dates.filter { $0 <= reload }.sorted(), reloadAt: reload)
    }
}
