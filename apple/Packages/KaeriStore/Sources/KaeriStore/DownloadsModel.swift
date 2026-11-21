// The download queue and what is still seeding, as Downloads shows them: grouped by title, with
// each running download's speed worked out from how far it moved between two of its moves.

import KaeriKit
import Foundation
import Observation

/// The downloads, as the engine last said them.
@Observable @MainActor
public final class DownloadsModel {
    /// The queue, in the engine's order.
    public private(set) var rows: [DownloadRow] = []
    /// Watched episodes still seeding.
    public private(set) var seeding: [SeedRow] = []
    /// Whether the engine has said the queue at all.
    public private(set) var loaded = false
    /// Bytes a second for each running download.
    public private(set) var rates: [Int64: Double] = [:]
    @ObservationIgnored private var marks: [Int64: Mark] = [:]

    /// Where a running download last moved to and when, and how long that move took.
    private struct Mark {
        var bytes: UInt64
        var at: Date
        var took: TimeInterval?

        /// How long it may sit still before its speed says nothing: three of its moves, and at
        /// least ten seconds. Progress comes a piece at a time, so a slow download sits still
        /// between pieces while the queue updates for the downloads beside it.
        var patience: TimeInterval { max(10, 3 * (took ?? 0)) }
    }

    public init() {}

    func apply(_ rows: [DownloadRow], at now: Date = .now) {
        var rates: [Int64: Double] = [:]
        var marks: [Int64: Mark] = [:]
        for row in rows where row.state == .active {
            guard let mark = self.marks[row.id], row.downloaded >= mark.bytes else {
                // The first reading, or one that went back: measure from here.
                marks[row.id] = Mark(bytes: row.downloaded, at: now)
                rates[row.id] = self.rates[row.id]
                continue
            }
            let seconds = now.timeIntervalSince(mark.at)
            if row.downloaded == mark.bytes || seconds <= 0.2 {
                // Not moved since its last move, or too soon to tell. Sitting still between pieces is
                // not a speed of zero, so the speed stays until it has sat still too long.
                marks[row.id] = mark
                rates[row.id] = seconds > mark.patience ? nil : self.rates[row.id]
            } else if mark.took != nil, seconds > mark.patience {
                // Moving again after a stall, which is not its speed: measure from here.
                marks[row.id] = Mark(bytes: row.downloaded, at: now)
                rates[row.id] = self.rates[row.id]
            } else {
                let rate = Double(row.downloaded - mark.bytes) / seconds
                // Half the last speed and half this move's: steady enough to read, quick to follow.
                rates[row.id] = self.rates[row.id].map { ($0 + rate) / 2 } ?? rate
                marks[row.id] = Mark(bytes: row.downloaded, at: now, took: seconds)
            }
        }
        self.rows = rows
        self.rates = rates
        self.marks = marks
        loaded = true
    }

    func seeding(_ rows: [SeedRow]) {
        seeding = rows
    }

    /// A finished download is on the list: Clear Finished Downloads has something to clear.
    public var hasFinished: Bool { rows.contains { $0.state == .done } }

    /// The queue by title, each title where its first row is.
    public var groups: [DownloadGroup] {
        var order: [AnilistId] = []
        var byTitle: [AnilistId: [DownloadRow]] = [:]
        for row in rows {
            if byTitle[row.anilistId] == nil { order.append(row.anilistId) }
            byTitle[row.anilistId, default: []].append(row)
        }
        return order.map { DownloadGroup(id: $0, rows: byTitle[$0] ?? []) }
    }

    /// What the downloads take on disk: a finished one's size, and what the rest have so far.
    public var bytesOnDisk: UInt64 {
        rows.reduce(0) { $0 + $1.bytesOnDisk }
    }

    /// Seconds until a running download finishes, at its speed.
    public func secondsLeft(_ row: DownloadRow) -> Int64? {
        guard row.state == .active, row.total > row.downloaded, let rate = rates[row.id] else { return nil }
        return Self.seconds(toMove: row.total - row.downloaded, at: rate)
    }

    /// Seconds to move `bytes` at `rate` bytes a second. There is no forecast at no speed, nor
    /// past a year, where it says nothing.
    nonisolated static func seconds(toMove bytes: UInt64, at rate: Double) -> Int64? {
        let seconds = (Double(bytes) / rate).rounded(.up)
        guard rate > 0, seconds <= 365 * 24 * 60 * 60 else { return nil }
        return Int64(seconds)
    }
}

/// One title's downloads.
public struct DownloadGroup: Identifiable, Equatable, Sendable {
    public let id: AnilistId
    public let rows: [DownloadRow]

    /// The title, as the queue names it.
    public var title: String { rows.first?.title ?? "" }

    public var bytesOnDisk: UInt64 {
        rows.reduce(0) { $0 + $1.bytesOnDisk }
    }
}

extension DownloadRow {
    /// What it takes on disk now.
    public var bytesOnDisk: UInt64 {
        state == .done ? max(total, downloaded) : downloaded
    }
}

extension SeedRow {
    /// The file's size: what has gone up over the ratio that makes.
    public var size: UInt64? {
        ratio > 0 ? UInt64((Double(uploadedBytes) / ratio).rounded()) : nil
    }
}
