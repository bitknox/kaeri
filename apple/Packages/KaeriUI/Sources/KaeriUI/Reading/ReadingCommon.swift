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

    public var id: String {
        switch self {
        case .source(let work): "source:\(work)" // i18n-skip
        case .tracking(let work): "tracking:\(work)" // i18n-skip
        case .readAgain(let work): "again:\(work)" // i18n-skip
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

/// A book's cover at 2:3 from its URL, rounded and edged like a show's.
struct BookCover: View {
    @Environment(\.displayScale) private var scale
    let url: String?
    let width: CGFloat
    var radius: CGFloat?

    var body: some View {
        let r = radius ?? ArtShape.coverRadius(width: width)
        ArtworkView(url: url, maxPixelSize: Int(width * 1.5 * scale))
            .frame(width: width, height: (width * 3 / 2).rounded())
            .clipShape(.rect(cornerRadius: r))
            .overlay(RoundedRectangle(cornerRadius: r).strokeBorder(Palette.glassEdge.opacity(1.25), lineWidth: 1))
            .accessibilityHidden(true)
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

enum ReadWords {
    /// The line under a cover: what comes next, else its facts joined.
    static func line(_ tile: WorkTile) -> String {
        if let next = tile.next { return next.text }
        if let label = tile.formatLabel, tile.facts.isEmpty { return label }
        return tile.facts.map(\.text).joined(separator: " · ") // i18n-skip
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
    @Environment(Navigator.self) private var navigator
    @Environment(\.homeLayout) private var layout
    let tile: WorkTile
    var width: CGFloat?
    /// The silk track under the cover, for Continue.
    var track = false
    /// Narrower than this, as a phone's grid cells are, the line may wrap once.
    static let narrow: CGFloat = 130

    var body: some View {
        let width = width ?? layout.poster.width
        Button {
            navigator.openSeries(tile.work)
        } label: {
            VStack(alignment: .leading, spacing: 0) {
                BookCover(url: tile.cover, width: width)
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
                    .lineLimit(1)
                    .padding(.top, track ? 6 : 8)
                HStack(alignment: .firstTextBaseline, spacing: 6) {
                    if tile.newChapter { WaterDot() }
                    // A phone's grid cell is narrow: its line takes a second line, not an ellipsis.
                    Text(ReadWords.line(tile))
                        .lineLimit(width < Self.narrow ? 2 : 1)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .font(.ui(layout.posterLine.size))
                .fixedLineHeight(layout.posterLine.line, relativeTo: .footnote)
                .foregroundStyle(tile.newChapter ? .silk : .haze)
                .padding(.top, 2)
            }
            .frame(width: width, alignment: .leading)
            .contentShape(.rect)
            .dynamicTypeSize(...DynamicTypeSize.xxxLarge)
        }
        .buttonStyle(.plain)
        .pointerHover()
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(Text(tile.title))
        .accessibilityValue(Text(ReadWords.line(tile)))
        .accessibilityAddTraits(.isButton)
    }
}

/// A shelf's heading with its link at the end, as Home's shelves have it.
struct ReadShelfHeader: View {
    @Environment(\.homeLayout) private var layout
    let title: String
    var link: String?
    var action: (() -> Void)?

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 10) {
            Text(title)
                .font(.ui(layout.heading.size, weight: layout.heading.weight))
                .fixedLineHeight(layout.heading.line, relativeTo: .title3)
                .foregroundStyle(.silk)
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
                .pointerHover()
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
