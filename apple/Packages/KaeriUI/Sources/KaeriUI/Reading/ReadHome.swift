// Reading's Home: the spotlight, Continue as covers with a silk track, This week by day, Up next
// for you, Releasing now, Because you finished, one genre shelf and the closing line. The engine
// sends the whole view; this draws it in Home's sizes. There is no empty state: a section with
// nothing leaves with its heading.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

struct ReadHomeScreen: View {
    @Environment(AppStore.self) private var store
    #if os(iOS)
    @Environment(\.horizontalSizeClass) private var sizeClass
    #endif
    @State private var width: CGFloat = 1_000

    private var layout: HomeLayout {
        #if os(iOS)
        HomeLayout(surface: .touch(regular: sizeClass != .compact, column: width - 48))
        #else
        HomeLayout(surface: .mac)
        #endif
    }

    var body: some View {
        let layout = self.layout
        ScrollView {
            ReadHomeContent(contentWidth: max(0, width - layout.margin * 2))
                .padding(.horizontal, layout.margin)
                .padding(.top, 8)
                .padding(.bottom, 32)
                .frame(maxWidth: .infinity, alignment: .topLeading)
        }
        .scrollIndicators(.never)
        .environment(\.homeLayout, layout)
        .onOfferedWidth { width = $0 }
        #if os(iOS)
        .refreshable { store.reading.loadHome(refresh: true) }
        .screenSubtitle(subtitle)
        .toolbar {
            if !layout.wide { AccountButton() }
        }
        #endif
        .task { store.reading.loadHome() }
    }

    #if os(iOS)
    private var subtitle: String {
        layout.wide ? "" : store.reading.homeOfflineLine ?? store.syncSubtitle()
    }
    #endif
}

extension ReadingModel {
    /// "Offline. Showing Home from 14:20.", while reading's Home shows what it kept.
    var homeOfflineLine: String? {
        guard let home, home.offline, let since = home.offlineSince else { return nil }
        return L.homeOffline(time: AirTime.clock(Date(timeIntervalSince1970: TimeInterval(since))))
    }
}

private struct ReadHomeContent: View {
    @Environment(AppStore.self) private var store
    @Environment(\.homeLayout) private var layout
    let contentWidth: CGFloat

    var body: some View {
        let home = store.reading.home
        ZStack(alignment: .topLeading) {
            if let home {
                VStack(alignment: .leading, spacing: layout.shelfGap) {
                    loaded(home)
                }
            } else {
                HomeWait(active: true, activity: "loading home") { // i18n-skip
                    HomeSpotlightPlaceholder(contentWidth: contentWidth)
                    HomeShelfPlaceholder(posters: true)
                }
            }
        }
        .animation(.standard, value: home == nil)
        .animation(.standard, value: home?.spotlight?.work)
        .animation(.standard, value: home.map(ReadHomeContent.shape) ?? [])
    }

    /// Which sections there are, for the gap to close or open when one comes or goes.
    private static func shape(_ home: ReadingHomeView) -> [Bool] {
        [home.continueReading.isEmpty, home.week.isEmpty, home.upNext.isEmpty, home.releasing.isEmpty,
         home.because.isEmpty, home.genreShelf.isEmpty]
    }

    @ViewBuilder
    private func loaded(_ home: ReadingHomeView) -> some View {
        #if os(iOS)
        if layout.wide, let line = store.reading.homeOfflineLine {
            HStack(spacing: 6) {
                Image(systemName: "wifi.slash").accessibilityHidden(true)
                Text(line)
            }
            .font(.ui(15))
            .foregroundStyle(.haze)
            .padding(.bottom, -12)
        }
        #endif
        if let tile = home.spotlight, let spot = home.spot {
            ZStack(alignment: .topLeading) {
                ReadSpotlightView(tile: tile, spot: spot, contentWidth: contentWidth)
                    .id(tile.work)
                    .transition(.opacity)
            }
        }
        if !home.continueReading.isEmpty {
            shelf(L.navContinue, tiles: home.continueReading, track: true)
        }
        if !home.week.isEmpty {
            ReadWeek(days: home.week)
        }
        if home.pending.contains(.yours) {
            HomeWait(active: true, activity: "loading home") { HomeShelfPlaceholder(posters: true) } // i18n-skip
        }
        if !home.upNext.isEmpty {
            shelf(L.homeUpNext, tiles: home.upNext, link: L.homeViewAll) { navigator.readingList(.planning) }
        }
        if !home.releasing.isEmpty {
            shelf(L.homeReadReleasing, tiles: home.releasing, link: L.homeSearch) { navigator.openReadingSearch() }
        }
        if let title = home.becauseTitle, !home.because.isEmpty {
            shelf(L.homeBecause(title: title), tiles: home.because)
        }
        if let genre = home.genre, !home.genreShelf.isEmpty {
            shelf(L.homeMoreGenre(genre: genre), tiles: home.genreShelf, link: L.homeSearch) {
                navigator.openReadingSearch(genre: genre)
            }
        }
        if home.pending.contains(.discovery) || home.pending.contains(.because) {
            HomeWait(active: true, activity: "loading home") { HomeShelfPlaceholder(posters: true) } // i18n-skip
        }
        if home.failed {
            PlainEmpty(title: L.homeFailed, line: store.reading.failures[.home]?.words ?? "") {
                Button(L.actionTryAgain) { store.reading.loadHome(refresh: true) }
                    .buttonStyle(SecondaryCapsuleStyle(height: Device.isMac ? 28 : 44, fontSize: Device.isMac ? 13 : 17))
                    .fixedSize()
            }
        }
        if let end = home.end {
            ReadHomeEndLine(end: end)
        }
    }

    @Environment(Navigator.self) private var navigator

    private func shelf(_ title: String, tiles: [WorkTile], track: Bool = false, link: String? = nil,
                       action: (() -> Void)? = nil) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            ReadShelfHeader(title: title, link: link, action: action)
            HomeShelf(spacing: layout.cardGap) {
                ForEach(tiles, id: \.work) { tile in
                    ReadTileCard(tile: tile, track: track)
                        .contextMenu { if track { ReadContinueMenu(tile: tile) } }
                        .transition(.opacity)
                }
            }
        }
        .transition(.opacity)
        .animation(.standard, value: tiles.map(\.work))
    }
}

/// What a Continue cover's menu holds.
private struct ReadContinueMenu: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let tile: WorkTile

    var body: some View {
        Button(L.homeDetails) { navigator.openSeries(tile.work) }
        Button(L.homeRemoveContinue) { store.reading.hideFromContinue(tile.work) }
    }
}

// MARK: - The spotlight

/// The one series at the top: its label, title, facts in small capitals, synopsis, what is new or
/// where you stopped, then the one amber action. The art is the banner, or the cover where there
/// is none, 16:9 at radius 16.
private struct ReadSpotlightView: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.homeLayout) private var layout
    let tile: WorkTile
    let spot: ReadSpotlight
    let contentWidth: CGFloat

    var body: some View {
        let beside = HomeSpotlightView.beside(layout, contentWidth: contentWidth)
        Group {
            if beside {
                HStack(alignment: .center, spacing: layout.spotGap) {
                    text(beside: true)
                        .frame(minWidth: 0, maxWidth: .infinity, alignment: .leading)
                    art(width: contentWidth * layout.artShare)
                }
            } else {
                VStack(alignment: .leading, spacing: layout.spotGap) {
                    art(width: contentWidth)
                    text(beside: false)
                }
            }
        }
        .accessibilityElement(children: .contain)
        .task(id: tile.work) {
            if store.reading.series[tile.work] == nil { store.reading.loadSeries(tile.work) }
        }
    }

    private func art(width: CGFloat) -> some View {
        Button {
            navigator.openSeries(tile.work)
        } label: {
            Group {
                if let banner = spot.banner {
                    ArtworkView(url: banner, maxPixelSize: 1_600)
                        .frame(width: width, height: width * 9 / 16)
                        .clipShape(.rect(cornerRadius: ArtShape.bannerRadius))
                } else {
                    BookCover(url: tile.cover, width: width * 0.4)
                        .frame(width: width, height: width * 9 / 16)
                }
            }
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .pointerHover()
        .accessibilityHidden(true)
        .frame(width: width, height: width * 9 / 16)
    }

    private func text(beside: Bool) -> some View {
        let other = store.reading.series[tile.work].flatMap { ReadSeriesWords.otherTitle($0.titles, shown: tile.title) }
        return VStack(alignment: .leading, spacing: 0) {
            Text(spot.label.text)
                .font(.ui(layout.spotLabel, weight: .medium))
                .foregroundStyle(.haze)
                .accessibilityAddTraits(.isHeader)
                .padding(.bottom, 8)
            Text(tile.title)
                .font(.showTitle(size: layout.spotTitle.size, relativeTo: .largeTitle))
                .fixedLineHeight(layout.spotTitle.line, relativeTo: .largeTitle)
                .foregroundStyle(.silk)
                .lineLimit(2)
                .dynamicTypeSize(...DynamicTypeSize.accessibility3)
                .fixedSize(horizontal: false, vertical: true)
            if let other {
                Text(other)
                    .font(.ui(layout.spotOther.size))
                    .fixedLineHeight(layout.spotOther.line, relativeTo: .footnote)
                    .foregroundStyle(.haze)
                    .lineLimit(1)
                    .padding(.top, 4)
            }
            if !spot.facts.isEmpty {
                Text(spot.facts.map(\.text).joined(separator: " · ").uppercased()) // i18n-skip
                    .font(.ui(11, weight: .medium))
                    .tracking(0.88)
                    .fixedLineHeight(16, relativeTo: .caption2)
                    .foregroundStyle(.haze)
                    .lineLimit(1)
                    .padding(.top, 10)
                    .accessibilityLabel(Text(spot.facts.map(\.text).joined(separator: ", "))) // i18n-skip
            }
            if let synopsis = spot.synopsis, !synopsis.isEmpty {
                Text(synopsis.replacing(/\s*\n+\s*/, with: " "))
                    .font(.ui(layout.synopsis.size))
                    .fixedLineHeight(layout.synopsis.line, relativeTo: .subheadline)
                    .foregroundStyle(.haze)
                    .lineLimit(layout.synopsisLines)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(.top, 12)
            }
            line.padding(.top, 16)
            actions.padding(.top, 18)
        }
        .frame(maxWidth: beside ? layout.textWidth : .infinity, alignment: .leading)
        .accessibilityElement(children: .contain)
    }

    /// Under the synopsis: what is new with the water dot, where you stopped with the track, or
    /// that sources are asked once the series is opened.
    @ViewBuilder
    private var line: some View {
        switch spot.case {
        case .newChapter:
            if let line = spot.line?.text {
                HStack(spacing: 8) {
                    WaterDot()
                    Text(line).foregroundStyle(.silk).lineLimit(1)
                }
                .font(.ui(layout.secondary))
            }
        case .continuing:
            if let line = spot.line?.text {
                HStack(spacing: 10) {
                    if let progress = tile.progress {
                        Meter(fraction: Double(progress), height: 4, spoken: line, silk: true)
                            .frame(width: layout.track)
                    }
                    Text(line).foregroundStyle(.haze).lineLimit(1)
                }
                .font(.ui(layout.secondary))
            }
        case .upNext:
            EmptyView()
        case .popular:
            Text(spot.line?.text ?? L.homeReadCheckedOnOpen)
                .font(.ui(layout.secondary))
                .foregroundStyle(.haze)
                .lineLimit(1)
        }
    }

    @ViewBuilder
    private var actions: some View {
        if layout.wide {
            ViewThatFits(in: .horizontal) {
                HStack(spacing: 10) {
                    main(stretch: false)
                    secondary(stretch: false)
                    more
                }
                VStack(alignment: .leading, spacing: 10) {
                    main(stretch: false)
                    HStack(spacing: 10) {
                        secondary(stretch: false)
                        more
                    }
                }
            }
        } else {
            VStack(spacing: 12) {
                main(stretch: true)
                HStack(spacing: 12) {
                    secondary(stretch: true)
                    more
                }
            }
        }
    }

    /// The amber action: read or go on, or Add to Planning for a series not checked yet.
    @ViewBuilder
    private func main(stretch: Bool) -> some View {
        if let words = ReadSeriesWords.action(spot.action) {
            Button {
                ReadSeriesWords.perform(spot.action, work: tile.work, store: store, navigator: navigator)
            } label: {
                HStack(spacing: 8) {
                    Image(systemName: "book").font(.ui(layout.button.font < 15 ? 12 : 15)).accessibilityHidden(true)
                    Text(words).lineLimit(1).fixedSize(horizontal: !stretch, vertical: false)
                }
                .frame(maxWidth: stretch ? .infinity : nil)
            }
            .buttonStyle(PrimaryCapsuleStyle(height: layout.button.height, fontSize: layout.button.font))
            .pointerHover()
        } else if !spot.planned {
            Button {
                store.reading.setStatus(tile.work, .planning)
            } label: {
                HStack(spacing: 8) {
                    Image(systemName: "plus").font(.ui(layout.button.font < 15 ? 11 : 14)).accessibilityHidden(true)
                    Text(L.libraryAddPlanning).lineLimit(1).fixedSize(horizontal: !stretch, vertical: false)
                }
                .frame(maxWidth: stretch ? .infinity : nil)
            }
            .buttonStyle(PrimaryCapsuleStyle(height: layout.button.height, fontSize: layout.button.font))
            .pointerHover()
        }
    }

    /// Details, or On your Planning list with a check where the series is planned already.
    @ViewBuilder
    private func secondary(stretch: Bool) -> some View {
        let size = layout.secondaryButton
        if spot.planned {
            Button {
                navigator.openSeries(tile.work)
            } label: {
                HStack(spacing: 6) {
                    Image(systemName: "checkmark").font(.ui(size.font - 2, weight: .semibold)).accessibilityHidden(true)
                    Text(L.readingOnPlanning).lineLimit(1)
                }
                .frame(maxWidth: stretch ? .infinity : nil)
            }
            .buttonStyle(SecondaryCapsuleStyle(height: size.height, fontSize: size.font))
            .fixedSize(horizontal: !stretch, vertical: false)
        } else {
            Button {
                navigator.openSeries(tile.work)
            } label: {
                Text(L.homeDetails).lineLimit(1).frame(maxWidth: stretch ? .infinity : nil)
            }
            .buttonStyle(SecondaryCapsuleStyle(height: size.height, fontSize: size.font))
            .fixedSize(horizontal: !stretch, vertical: false)
        }
    }

    /// More: Remove from Continue, for a series in Continue.
    @ViewBuilder
    private var more: some View {
        if spot.case == .newChapter || spot.case == .continuing {
            let side = layout.secondaryButton.height
            Menu {
                Button(L.homeRemoveContinue) { store.reading.hideFromContinue(tile.work) }
            } label: {
                Image(systemName: "ellipsis")
                    .font(.ui(layout.secondaryButton.font, weight: .semibold))
                    .foregroundStyle(.silk)
                    .frame(width: side, height: side)
                    .background(Palette.raised, in: .circle)
                    .contentShape(.circle)
            }
            .menuStyle(.button)
            .buttonStyle(.plain)
            .menuIndicator(.hidden)
            .fixedSize()
            .pointerHover()
            .accessibilityLabel(L.homeMore)
        }
    }
}

// MARK: - This week

/// The days from today under a faint rule: what came out today with its time and the water dot,
/// the rest "Usually", and a day with nothing says Nothing usually.
private struct ReadWeek: View {
    @Environment(\.homeLayout) private var layout
    @Environment(Navigator.self) private var navigator
    let days: [ReadWeekDay]

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            ReadShelfHeader(title: L.homeThisWeek, link: L.navUpdates) { navigator.open(.updates) }
            Rectangle().fill(Palette.faintRule).frame(height: 1).accessibilityHidden(true)
            if layout.wide {
                HStack(alignment: .top, spacing: 16) {
                    ForEach(Array(days.prefix(layout.weekColumns).enumerated()), id: \.offset) { _, day in
                        ReadDayColumn(day: day)
                            .frame(minWidth: 0, maxWidth: .infinity, alignment: .topLeading)
                    }
                }
            } else {
                ScrollView(.horizontal) {
                    HStack(alignment: .top, spacing: 16) {
                        ForEach(Array(days.enumerated()), id: \.offset) { _, day in
                            ReadDayColumn(day: day).frame(width: 150, alignment: .topLeading)
                        }
                    }
                }
                .scrollIndicators(.never)
                .scrollClipDisabled()
            }
        }
        .transition(.opacity)
    }
}

private struct ReadDayColumn: View {
    @Environment(Navigator.self) private var navigator
    let day: ReadWeekDay
    private let shown = 3

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(label)
                .font(.ui(12, weight: .semibold))
                .fixedLineHeight(16, relativeTo: .caption)
                .foregroundStyle(day.today ? .silk : .haze)
                .accessibilityAddTraits(.isHeader)
            if day.entries.isEmpty {
                Text(L.weekNothingUsually)
                    .font(.ui(12))
                    .fixedLineHeight(16, relativeTo: .caption)
                    .foregroundStyle(.haze)
            } else {
                ForEach(day.entries.prefix(shown), id: \.tile.work) { entry in
                    ReadWeekCell(entry: entry)
                }
                if day.entries.count > shown {
                    Button(L.homeDayMore(count: day.entries.count - shown)) { navigator.open(.updates) }
                        .buttonStyle(.plain)
                        .font(.ui(12, weight: .medium))
                        .underline()
                        .foregroundStyle(.silk)
                        .pointerHover()
                }
            }
        }
    }

    private var label: String {
        if day.today { return L.homeToday }
        return CalendarWords.weekday(Date(timeIntervalSince1970: TimeInterval(day.day)))
    }
}

private struct ReadWeekCell: View {
    @Environment(Navigator.self) private var navigator
    @Environment(\.homeLayout) private var layout
    let entry: ReadWeekEntry

    var body: some View {
        let tile = entry.tile
        Button {
            navigator.openSeries(tile.work)
        } label: {
            HStack(alignment: .top, spacing: 8) {
                BookCover(url: tile.cover, width: layout.weekCover, radius: 4)
                VStack(alignment: .leading, spacing: 2) {
                    HStack(spacing: 4) {
                        if let at = entry.outAt {
                            WaterDot()
                            Text(ReadWords.clock(at)).foregroundStyle(.silk)
                        } else {
                            Text(L.weekUsually).foregroundStyle(.haze)
                        }
                    }
                    .font(.ui(12, weight: entry.outAt == nil ? .regular : .semibold))
                    .monospacedDigit()
                    .fixedLineHeight(16, relativeTo: .caption)
                    Text(tile.title)
                        .font(.showTitle(size: 13, relativeTo: .footnote))
                        .fixedLineHeight(17, relativeTo: .footnote)
                        .foregroundStyle(.silk)
                        .lineLimit(2)
                    // The chapter that came out this day; for one only expected, the person's place.
                    if let line = entry.chapter?.text ?? tile.next?.text {
                        Text(line)
                            .font(.ui(12))
                            .fixedLineHeight(16, relativeTo: .caption)
                            .foregroundStyle(.haze)
                            .lineLimit(1)
                    }
                }
                .frame(minWidth: 0, maxWidth: .infinity, alignment: .leading)
            }
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .pointerHover()
        .accessibilityElement(children: .combine)
    }
}

// MARK: - The end

/// The closing line: Updates and Search as links, or Add a source while there is none.
private struct ReadHomeEndLine: View {
    @Environment(Navigator.self) private var navigator
    let end: ReadHomeEnd

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            switch end {
            case .everything:
                Text(L.homeReadEnd)
            case .noSources:
                Text(L.homeReadNoSources)
                Button(L.sourcesAddTitle) { navigator.addBookSource = .choose }
                    .buttonStyle(SecondaryCapsuleStyle(height: 40, fontSize: 15))
                    .pointerHover()
            }
        }
        .font(.ui(15))
        .foregroundStyle(.haze)
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.top, 4)
        .transition(.opacity)
    }
}
