// What reading's places share: covers from a URL, a series' cover card, the shelves' heading,
// the empty state with no picture, and the words for lists, media and chapter states.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

/// A sheet over a series' page.
public enum SeriesSheet: Hashable, Identifiable, Sendable {
    /// Reading from: the source and version the chapters come from.
    case source(String)
    /// The trackers' progress for the series.
    case tracking(String)
    /// Read again from the start, asked first.
    case readAgain(String)
    /// Send to e-reader: the chapters to prepare for an e-reader.
    case send(String, chapters: [String])
    /// Where did you stop, after chapters were sent to an e-reader.
    case stopped(String)

    public var id: String {
        switch self {
        case .source(let work): "source:\(work)" // i18n-skip
        case .tracking(let work): "tracking:\(work)" // i18n-skip
        case .readAgain(let work): "again:\(work)" // i18n-skip
        case .send(let work, let chapters): "send:\(work):\(chapters.joined(separator: ","))" // i18n-skip
        case .stopped(let work): "stopped:\(work)" // i18n-skip
        }
    }
}

/// The steps of Add a source.
public enum AddBookSourceStep: Hashable, Identifiable, Sendable {
    case choose, server

    /// One sheet for both steps: the server step replaces the choice inside it.
    public var id: Int { 0 }
}

// MARK: - Covers

/// A book's cover at 2:3 from its URL, rounded and edged like a show's. With no art at all, a
/// title given goes on the plate, with the book glyph under it, so the cover is still told apart.
struct BookCover: View {
    @Environment(\.displayScale) private var scale
    let url: String?
    let width: CGFloat
    var radius: CGFloat?
    /// The title for the plate of a series with no cover.
    var title: String?

    var body: some View {
        let r = radius ?? ArtShape.coverRadius(width: width)
        ArtworkView(url: url, maxPixelSize: Int(width * 1.5 * scale), flat: url == nil && title != nil ? Palette.raised : nil)
            .overlay(alignment: .topLeading) {
                if url == nil, let title { plate(title) }
            }
            .frame(width: width, height: (width * 3 / 2).rounded())
            .clipShape(.rect(cornerRadius: r))
            .overlay(RoundedRectangle(cornerRadius: r).strokeBorder(Palette.glassEdge.opacity(1.25), lineWidth: 1))
            .accessibilityHidden(true)
    }

    private func plate(_ title: String) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(title)
                .font(.ui(width < 120 ? 12 : 13, weight: .semibold))
                .foregroundStyle(.silk)
                .lineLimit(4)
            Spacer(minLength: 8)
            Image(systemName: "book")
                .font(.ui(width < 120 ? 11 : 12))
                .foregroundStyle(.haze)
        }
        .padding(width < 120 ? 10 : 12)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .dynamicTypeSize(...DynamicTypeSize.large)
    }
}

/// The water dot beside new things.
struct WaterDot: View {
    var size: CGFloat = 6

    var body: some View {
        Circle().fill(.water).frame(width: size, height: size).accessibilityHidden(true)
    }
}

// MARK: - Words

/// What the line under a cover says, by where the cover is.
enum ReadCaption: Equatable {
    /// What comes next, else the facts: the person's own shelves and the library.
    case plain
    /// A list of one format, a Home row or its View all: the origin, then the status, or on New
    /// the month the series started ("Japan · since March"). Where the list fixes the status,
    /// as Finished does, the count takes its place ("Japan · 112 chapters").
    case formatRow(statusFixed: Bool)
    /// A list of several formats, a Home row or its View all: the format, then the status
    /// ("Manga · Finished"), or the count where the list fixes the status.
    case mixedRow(statusFixed: Bool)
    /// AniList's results in Search: the format, then the origin, even for a series the person
    /// reads (a joined cover's note says where it is).
    case formatOrigin
    /// A source's hit in Search: how much the source has, or nothing where it doesn't say.
    case amount

    /// The caption for AniList's list for a query: a row's own when the query names one format
    /// or several, or when it is a row's View all; else the format and the origin.
    static func list(_ query: ReadSearchQuery, viewAll: Bool) -> ReadCaption {
        let fixed = query.status != nil
        let formats = Set(query.formats.map { $0 == "one-shot" ? "manga" : $0 }) // i18n-skip: format keys
        if formats.count == 1 { return .formatRow(statusFixed: fixed) }
        if formats.count > 1 || viewAll { return .mixedRow(statusFixed: fixed) }
        return .formatOrigin
    }
}

enum ReadWords {
    /// The line under a cover: what comes next, else its facts joined. Nil where there is none.
    static func line(_ tile: WorkTile, _ caption: ReadCaption = .plain) -> String? {
        parts(tile, caption).map { $0.joined(separator: " · ") } // i18n-skip
    }

    /// The line as a screen reader hears it after the title, as a list.
    static func spoken(_ tile: WorkTile, _ caption: ReadCaption = .plain) -> String? {
        parts(tile, caption).map { $0.joined(separator: ", ") } // i18n-skip
    }

    private static func parts(_ tile: WorkTile, _ caption: ReadCaption) -> [String]? {
        if tile.notInSources { return [L.homeNotFound] }
        if caption != .formatOrigin, let next = tile.next { return [next.text] }
        let facts = tile.facts.map(\.text)
        switch caption {
        case .amount:
            return tile.amount.map { [$0.text] }
        case .formatRow(let fixed):
            // The facts lead with the format word, which the row's heading already says.
            if let origin = tile.origin?.text {
                if let started = tile.started?.text { return [origin, started] }
                return [origin] + second(tile, statusFixed: fixed)
            }
        case .mixedRow(let fixed):
            if let format = facts.first { return [format] + second(tile, statusFixed: fixed) }
        case .formatOrigin:
            if let origin = tile.origin?.text { return facts.prefix(1) + [origin] }
        case .plain:
            break
        }
        if let label = tile.formatLabel, facts.isEmpty { return [label] }
        // Two facts at most, so the line fits under a cover.
        return facts.isEmpty ? nil : Array(facts.prefix(2))
    }

    /// The fact after the format: the status, or where the list fixes the status, the count
    /// ("112 chapters"), so the line doesn't repeat the heading. Nothing where there is no count.
    private static func second(_ tile: WorkTile, statusFixed: Bool) -> [String] {
        guard statusFixed else { return tile.facts.dropFirst().prefix(1).map(\.text) }
        return tile.facts.dropFirst().first(where: isCount).map { [$0.text] } ?? []
    }

    private static func isCount(_ fact: Msg) -> Bool {
        guard case .reading(let words) = fact else { return false }
        switch words {
        case .readingChapters, .readingVolumes: return true
        default: return false
        }
    }

    static func list(_ segment: LibrarySegmentRead) -> String {
        switch segment {
        case .reading: L.readListReading
        case .planning: L.readListPlanning
        case .completed: L.readListCompleted
        case .paused: L.readListPaused
        case .dropped: L.readListDropped
        case .rereading: L.readListRereading
        case .yourFiles: L.readListYourFiles
        }
    }

    static let lists: [LibrarySegmentRead] = [.reading, .planning, .completed, .paused, .dropped, .rereading, .yourFiles]
    /// The lists a series can be put on.
    static let statuses: [LibrarySegmentRead] = [.reading, .planning, .completed, .paused, .dropped, .rereading]

    static func medium(_ answer: KaeriIsFor) -> String {
        switch answer {
        case .watch: L.mediumWatch
        case .read: L.mediumRead
        case .both: L.mediumBoth
        }
    }

    static func mediumLine(_ answer: KaeriIsFor) -> String {
        switch answer {
        case .watch: L.mediumWatchLine
        case .read: L.mediumReadLine
        case .both: L.mediumBothLine
        }
    }

    /// The answer as Settings' row says it: "Watching", "Reading", "Watching and reading".
    static func answer(_ answer: KaeriIsFor) -> String {
        switch answer {
        case .watch: L.mediumAnswerWatch
        case .read: L.mediumAnswerRead
        case .both: L.mediumAnswerBoth
        }
    }

    /// A chapter row's state in words, or nil for an unread one with nothing to say.
    static func state(_ row: ChapterRow) -> String? {
        if let download = row.download {
            switch download {
            case .queued: return L.chapterStateQueued
            case .waitsForWifi: return L.chapterStateWifi
            case .downloading(let done, let total): return L.chapterStateDownloading(done: Int(done), total: Int(total))
            case .failed(let failure): return L.chapterStateFailed(reason: failure.words)
            case .done: break
            }
        }
        switch row.state {
        case .inProgress(let page, let of):
            if let of { return L.chapterStatePageOf(page: Int(page), of: Int(of)) }
            return L.chapterStatePage(page: Int(page))
        case .read: return L.chapterStateRead
        case .unread: return row.newChapter ? L.chapterStateNew : nil
        }
    }

    /// The day heading in Updates: Today, Yesterday, else the weekday and date.
    static func day(_ seconds: Int64) -> String {
        let date = Date(timeIntervalSince1970: TimeInterval(seconds))
        let calendar = Calendar.current
        if calendar.isDateInToday(date) { return L.chapterUpdatesToday }
        if calendar.isDateInYesterday(date) { return L.chapterUpdatesYesterday }
        return date.formatted(.dateTime.weekday(.wide).day().month(.wide).locale(AirTime.locale))
    }

    static func clock(_ seconds: Int64) -> String {
        AirTime.clock(Date(timeIntervalSince1970: TimeInterval(seconds)))
    }

    /// "12 min ago", as Updates says when it last checked.
    static func ago(_ seconds: Int64) -> String {
        let date = Date(timeIntervalSince1970: TimeInterval(seconds))
        return date.formatted(.relative(presentation: .named, unitsStyle: .abbreviated).locale(AirTime.locale))
    }
}

// MARK: - Empty

/// An empty place with no picture: a title, a line and at most one button where the content
/// would be (the reading board's decision).
struct PlainEmpty<Action: View>: View {
    let title: String
    let line: String
    @ViewBuilder var action: () -> Action

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(title)
                .font(.plat(mac: 15, touch: 17, weight: .semibold))
                .foregroundStyle(.silk)
                .accessibilityAddTraits(.isHeader)
            Text(line)
                .font(.plat(mac: 13, touch: 15))
                .foregroundStyle(.haze)
                .frame(maxWidth: 420, alignment: .leading)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, 4)
            action()
                .padding(.top, 16)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.vertical, 24)
    }
}

extension PlainEmpty where Action == EmptyView {
    init(title: String, line: String) {
        self.init(title: title, line: line) { EmptyView() }
    }
}

// MARK: - Cover card

/// A series to open: its cover, the title and one line under it.
struct ReadTileCard: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.homeLayout) private var layout
    let tile: WorkTile
    var width: CGFloat?
    /// The silk track under the cover, for Continue.
    var track = false
    /// Two lines kept for the title whatever its length, so a row's or grid's covers and lines
    /// stand level: discovery rows, Search's grid and the sources' rows.
    var twoLines = false
    /// A silk line under the facts, which wraps: "In Home library" on a joined AniList result.
    var note: String?
    /// Add to Planning as a screen reader's action, for a series not on a list yet.
    var planAction = false
    /// What the line under the cover says.
    var caption = ReadCaption.plain

    var body: some View {
        let width = width ?? layout.poster.width
        Button {
            navigator.openSeries(tile.work)
        } label: {
            VStack(alignment: .leading, spacing: 0) {
                BookCover(url: tile.cover, width: width, title: tile.title)
                if track {
                    Group {
                        if let progress = tile.progress, !tile.newChapter {
                            Meter(fraction: Double(progress), height: 3, silk: true)
                        } else {
                            Color.clear.frame(height: 3)
                        }
                    }
                    .padding(.top, 6)
                }
                Text(tile.title)
                    .font(.showTitle(size: layout.posterTitle.size, relativeTo: .body))
                    .fixedLineHeight(layout.posterTitle.line)
                    .foregroundStyle(.silk)
                    .lineLimit(twoLines ? 2 : 1, reservesSpace: twoLines)
                    .fixedSize(horizontal: false, vertical: twoLines)
                    .padding(.top, track ? 6 : 8)
                if let line = ReadWords.line(tile, caption) {
                    HStack(alignment: .firstTextBaseline, spacing: 6) {
                        if tile.newChapter { WaterDot() }
                        // One line with an ellipsis on every device, as the board draws it; Not
                        // found in your sources takes a second line rather than lose words.
                        Text(line)
                            .lineLimit(tile.notInSources ? 2 : 1)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    .font(.ui(layout.posterLine.size))
                    .fixedLineHeight(layout.posterLine.line, relativeTo: .footnote)
                    .foregroundStyle(tile.newChapter ? .silk : .haze)
                    .padding(.top, 2)
                }
                if let note {
                    Text(note)
                        .font(.ui(layout.posterLine.size, weight: .semibold))
                        .fixedLineHeight(layout.posterLine.line, relativeTo: .footnote)
                        .foregroundStyle(.silk)
                        .fixedSize(horizontal: false, vertical: true)
                        .padding(.top, 4)
                        .transition(.opacity)
                }
            }
            .frame(width: width, alignment: .leading)
            .contentShape(.rect)
            .dynamicTypeSize(...DynamicTypeSize.xxxLarge)
        }
        .buttonStyle(.plain)
        .pointerHover()
        .animation(.fadeIn, value: note)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(Text(tile.title))
        // The source a joined result is in comes last.
        .accessibilityValue(Text([ReadWords.spoken(tile, caption), note].compactMap { $0 }.joined(separator: ", "))) // i18n-skip
        .accessibilityAddTraits(.isButton)
        .accessibilityActions {
            if planAction {
                Button(L.libraryAddPlanning) { store.reading.setStatus(tile.work, .planning) }
            }
        }
    }
}

/// A shelf's heading with its link at the end, as Home's shelves have it, and the small line
/// beside the heading some rows have ("Last 12 months").
struct ReadShelfHeader: View {
    @Environment(\.homeLayout) private var layout
    @Environment(\.dynamicTypeSize) private var typeSize
    let title: String
    var side: String?
    var link: String?
    /// What a screen reader hears for the heading, where it differs: "Trending manga, loading".
    var spoken: String?
    var action: (() -> Void)?

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 10) {
            HStack(alignment: .firstTextBaseline, spacing: 12) {
                Text(title)
                    .font(.ui(layout.heading.size, weight: layout.heading.weight))
                    .fixedLineHeight(layout.heading.line, relativeTo: .title3)
                    .foregroundStyle(.silk)
                    // A long heading ("Because you finished …") wraps rather than lose the title
                    // that explains the row; View all stays on its first line.
                    .lineLimit(typeSize.isAccessibilitySize ? 3 : 2)
                    .fixedSize(horizontal: false, vertical: true)
                if let side {
                    Text(side)
                        .font(.ui(13))
                        .foregroundStyle(.haze)
                        .lineLimit(1)
                        .fixedSize()
                }
            }
            .accessibilityElement(children: .combine)
            .accessibilityLabel(spoken.map { Text($0) } ?? Text([title, side].compactMap { $0 }.joined(separator: ", "))) // i18n-skip
            .accessibilityAddTraits(.isHeader)
            Spacer(minLength: 8)
            if let link, let action {
                Button(action: action) {
                    HStack(spacing: 4) {
                        Text(link)
                        Image(systemName: "chevron.right")
                            .font(.ui(layout.secondary - 2, weight: .semibold))
                            .accessibilityHidden(true)
                    }
                    .font(.ui(layout.secondary, weight: .semibold))
                    .foregroundStyle(.silk)
                    .frame(minHeight: layout.surface == .mac ? 24 : 44)
                    .contentShape(.rect)
                }
                .buttonStyle(.plain)
                .fixedSize()
                .pointerHover()
                // "View all, Trending manga".
                .accessibilityLabel(link)
                .accessibilityValue(title)
            }
        }
    }
}

// MARK: - The shell and the medium

/// Keeps the places in step with what Kaeri is for: the sections' names, a section that is no
/// longer one of its places, and Open at with both on, once at launch.
struct MediumPlaces: ViewModifier {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @State private var opened = false

    func body(content: Content) -> some View {
        content
            // Only once the engine has answered: before that the answer is Watch by default, and
            // a section opened at launch would give way to it.
            .onChange(of: store.medium.view?.answer, initial: true) { _, answer in
                guard let answer else { return }
                AppSection.answer = answer
                navigator.section = navigator.section.standIn(for: answer)
            }
            .onChange(of: store.medium.view != nil) { _, known in
                guard known, !opened else { return }
                opened = true
                if store.medium.both, navigator.section == .home, store.settings.value(.openAt) == "read" { // i18n-skip
                    navigator.section = .read
                }
            }
    }
}

extension View {
    func mediumPlaces() -> some View { modifier(MediumPlaces()) }
}
