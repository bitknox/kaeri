// What an episode row says beside its still: the sizes each screen sets it in, the facts line, the
// text outlook that decides tall rows from short, spoiler protection per row, and the ranges a
// long series splits into. The logic, apart from the views (EpisodeRowLabel.swift), so a test
// can run it.

import KaeriDesign
import KaeriKit
import KaeriStrings
import NaturalLanguage
import SwiftUI

/// The size an episode row is set in, by the screen it is on.
struct EpisodeMetrics: Equatable {
    /// Where the synopsis sits against the still.
    enum Text: Equatable {
        /// To the right of the still, in a column with the title and the facts.
        case beside
        /// Under the still and the title, across the row.
        case under
    }

    let text: Text
    let still: CGSize
    /// The row's height where it holds room for the text.
    let tall: CGFloat
    /// The row's height where it holds none: a protected row, and a show with no synopses coming.
    /// The same as `tall` where the text sits beside the still, which is as tall either way.
    let short: CGFloat

    /// The Mac with a list 560 pt wide or more: a 178 x 100 still, three lines beside it.
    static let macWide = EpisodeMetrics(text: .beside, still: CGSize(width: 178, height: 100), tall: 120, short: 120)
    /// The Mac with a narrower list: a 144 x 81 still, the text under.
    static let macNarrow = EpisodeMetrics(text: .under, still: CGSize(width: 144, height: 81), tall: 164, short: 100)
    /// An iPad at regular width: the Mac's row at touch sizes, a 192 x 108 still.
    static let pad = EpisodeMetrics(text: .beside, still: CGSize(width: 192, height: 108), tall: 128, short: 128)
    /// An iPhone, and an iPad at compact width.
    static let phone = EpisodeMetrics(text: .under, still: CGSize(width: 144, height: 81), tall: 172, short: 100)

    /// The Mac's list is wide enough for text beside the still from this width.
    static let wideFrom: CGFloat = 560

    /// The Mac's metrics for a list `width` points wide.
    static func mac(listWidth width: CGFloat) -> EpisodeMetrics {
        width >= wideFrom ? .macWide : .macNarrow
    }

    /// The row's height: tall where the table has room kept for text and this row can show it.
    func height(room: Bool, protected: Bool) -> CGFloat {
        room && !protected ? tall : short
    }
}

/// Whether a table's rows keep room for text, settled once when the table is laid out and again at
/// the next layout (coming back to the page, a range switch). Text that turns up for a table laid
/// out short waits for that; text that was expected and never comes leaves its room as space.
struct EpisodeRoom: Equatable {
    private(set) var tall: Bool?

    /// Lays out `rows`: tall if some have text or a lookup that may bring some is running. Settles
    /// on the first layout; `again` is the next one.
    mutating func settle(_ rows: [EpisodeRow], again: Bool = false) {
        guard tall == nil || again else { return }
        tall = Self.room(for: rows)
    }

    static func room(for rows: [EpisodeRow]) -> Bool {
        switch episodeText(rows: rows) {
        case .present, .expected: true
        case .none: false
        }
    }

    /// Whether rows have room, before the table has been laid out: they do not.
    var room: Bool { tall ?? false }
}

/// Spoiler protection for one row.
enum EpisodeProtection {
    /// Whether protection hides `row`'s still and synopsis: the setting is on and the engine says
    /// the row is ahead (`spoilerAhead`).
    static func hides(_ row: EpisodeRow, spoilers: Bool) -> Bool {
        spoilers && row.spoilerAhead
    }

    /// Whether `row` is hidden and nobody has asked to see it this visit.
    static func hidden(
        _ row: EpisodeRow, spoilers: Bool, revealed: Set<String>, all: Bool
    ) -> Bool {
        hides(row, spoilers: spoilers) && !all && !revealed.contains(row.number)
    }

    /// The first row that protection hides, which the line about it sits above.
    static func startsAt(_ rows: [EpisodeRow], spoilers: Bool) -> String? {
        start(rows, spoilers: spoilers)?.row
    }

    /// Where protection starts in `rows`: the first row it hides, and the episode the line names,
    /// the last one before it (the row above, else the number before the first hidden one).
    static func start(_ rows: [EpisodeRow], spoilers: Bool) -> (row: String, after: Int)? {
        guard let at = rows.firstIndex(where: { hides($0, spoilers: spoilers) }) else { return nil }
        let before = at > 0 ? whole(rows[at - 1].number) : whole(rows[at].number).map { $0 - 1 }
        return (rows[at].number, max(before ?? 0, 0))
    }

    /// An episode number as a whole number, a half episode taking the one before it.
    private static func whole(_ number: String) -> Int? {
        Int(number) ?? Double(number).map { Int($0.rounded(.down)) }
    }
}

/// The words of an episode's facts line.
enum EpisodeFacts {
    /// "14 min left · Aired 21 May 2014", "Watched · Aired 14 May 2014", "24 min · Filler": the
    /// length, or what is left of it or Watched, then when it aired, then what marks it. On phones
    /// the episode's number leads it ("E7 · 14 min left"), where the row has no column for it.
    static func line(_ row: EpisodeRow, current: Bool, minutesLeft: Int?, leadsWithNumber: Bool) -> String {
        var parts: [String] = []
        if leadsWithNumber { parts.append(L.episodeShort(episode: Int(episode: row.number))) }
        if row.completed {
            parts.append(L.episodeWatched)
        } else if current, let minutesLeft {
            parts.append(L.episodeFactsLeft(minutes: minutesLeft))
        } else if let seconds = row.durationSecs {
            parts.append(L.episodeFactsLength(minutes: max(1, Int((Double(seconds) / 60).rounded()))))
        }
        // The length and the date read as one part: the first part and then Aired.
        if let aired = row.airsDate, !row.isUnaired() { parts.append(L.episodeAired(date: day(aired))) }
        if row.kind == .filler { parts.append(L.episodeFiller) }
        if row.unsourced { parts.append(L.episodeUnsourced) }
        guard var line = parts.first else { return "" }
        for part in parts.dropFirst() { line = L.episodeFactsJoin(first: line, second: part) }
        return line
    }

    /// "21 May 2014", in the person's language.
    static func day(_ date: Date) -> String {
        date.formatted(.dateTime.day().month(.abbreviated).year().locale(AirTime.locale))
    }

    /// The minutes left of the episode the viewer is partway through, as the rows say them.
    static func minutesLeft(_ entry: Entry) -> Int? {
        guard let resume = entry.resume, let fraction = resume.fraction, fraction > 0 else { return nil }
        return max(1, Int(((resume.position / fraction - resume.position) / 60).rounded()))
    }
}

/// A long series' table split into ranges of 100, and what the list shows of the one picked.
struct EpisodePlan {
    /// The ranges: none for a table of 100 or fewer.
    let ranges: [EpisodeRange]
    /// The range shown; 0 where there are none.
    let index: Int
    /// The rows shown: the range's, or all of them.
    let shown: [EpisodeRow]
    /// The watched rows folded into a line above the rest, where there are enough of them.
    let fold: WatchedFold?

    /// `rows` as the list shows them. `picked` is the range the viewer chose, else the one that
    /// holds the next episode.
    init(rows: [EpisodeRow], picked: Int?, unfolded: Bool = false, now: Date = .now) {
        ranges = episodeRanges(rows: rows, now: Int64(now.timeIntervalSince1970))
        if ranges.isEmpty {
            index = 0
            shown = rows
            fold = WatchedFold(rows)
        } else {
            let open = Int(episodeRangeToOpen(rows: rows))
            index = min(max(picked ?? open, 0), ranges.count - 1)
            let range = ranges[index]
            shown = Array(rows[Int(range.start)..<Int(range.end)])
            let count = Int(episodeFold(rows: rows, range: range))
            fold = count >= 2 ? WatchedFold(count: count, of: shown) : nil
        }
    }

    /// "301–400": the range by its first and last episode numbers.
    static func title(_ range: EpisodeRange, in rows: [EpisodeRow]) -> String {
        let first = rows[Int(range.start)].number
        let last = rows[Int(range.end) - 1].number
        return L.episodeRangeShort(
            from: Int(episode: first).formatted(), to: Int(episode: last).formatted()
        )
    }

    /// The menu item for a range: its numbers, then Watched or Not out yet where that is all it is.
    static func item(_ range: EpisodeRange, in rows: [EpisodeRow]) -> String {
        let name = title(range, in: rows)
        if range.watched { return L.episodeRangeItem(range: name, state: L.episodeWatched) }
        if range.notOut { return L.episodeRangeItem(range: name, state: L.episodeRowNotOut) }
        return name
    }
}

/// The language a synopsis is written in, so a screen reader and the line breaking follow the
/// text and not the interface.
@MainActor
enum EpisodeLanguage {
    private static var known: [String: String] = [:]

    /// The BCP 47 code of the text's language, when it can be told.
    static func of(_ text: String) -> String? {
        if let cached = known[text] { return cached.isEmpty ? nil : cached }
        let recognizer = NLLanguageRecognizer()
        recognizer.processString(text)
        let code = recognizer.dominantLanguage?.rawValue ?? ""
        if known.count > 2_000 { known.removeAll() }
        known[text] = code
        return code.isEmpty ? nil : code
    }
}

extension Navigator {
    /// Whether the viewer asked for `number`'s still and synopsis, or for every row after
    /// protection's line, on this visit to `title`.
    func synopsisIsShown(_ title: AnilistId, _ number: String) -> Bool {
        synopsisAllShown.contains(title) || synopsisShown[title]?.contains(number) == true
    }

    func showSynopsis(_ title: AnilistId, _ number: String) {
        synopsisShown[title, default: []].insert(number)
    }

    func showAllSynopses(_ title: AnilistId, _ shown: Bool) {
        if shown { synopsisAllShown.insert(title) } else { synopsisAllShown.remove(title) }
        if !shown { synopsisShown[title] = nil }
    }

    /// The titles whose pages are open: the Mac's page and the pages under it that Back returns
    /// to, and on iPhone and iPad every tab's pushed pages. A page covered by another page or by
    /// the player is still open.
    var openTitles: Set<AnilistId> {
        #if os(macOS)
        guard let titleOnMac else { return [] }
        return Set((paths[section] ?? []).map(\.id)).union([titleOnMac])
        #else
        return Set(paths.values.flatMap { $0.map(\.id) })
        #endif
    }

    /// Ends the visit to each page no longer open: what it showed past protection hides again,
    /// and the next visit opens on the range of the next episode.
    func endClosedEpisodeVisits() {
        let open = openTitles
        let visited = Set(synopsisShown.keys).union(synopsisAllShown).union(episodeRange.keys)
        for title in visited.subtracting(open) {
            synopsisShown[title] = nil
            synopsisAllShown.remove(title)
            episodeRange[title] = nil
        }
    }

    /// Steps the episode list on screen to its previous or next range, as View › Previous
    /// Episodes and Next Episodes do.
    func stepEpisodeRange(by step: Int) {
        guard let shown = episodeRangesShown else { return }
        let index = shown.index + step
        guard (0..<shown.count).contains(index) else { return }
        episodeRange[shown.title] = index
    }
}

/// The ranges of the episode list on screen, for the View menu's Previous Episodes and Next
/// Episodes.
struct EpisodeRangesShown: Equatable {
    let title: AnilistId
    let index: Int
    let count: Int

    var hasPrevious: Bool { index > 0 }
    var hasNext: Bool { index < count - 1 }
}
