// A series' page: the cover, the title, facts in small capitals and the synopsis, one main action
// naming chapter and page, Status and Download beside it; to the side the source in use with
// Change, tracker progress and the preferred group; then the chapters by volume, read volumes
// folded. On iPhone the same in one column.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

enum ReadSeriesWords {
    /// The other title under the shown one: romaji, else native, never the same words again.
    static func otherTitle(_ titles: WorkTitles, shown: String) -> String? {
        [titles.romaji, titles.native, titles.english]
            .compactMap { $0 }
            .first { !$0.isEmpty && $0 != shown }
    }

    /// The main action's words, or nil where there is nothing to read.
    static func action(_ action: SeriesAction?) -> String? {
        switch action {
        // "Read chapter 12", but a chapter with no number goes by its own title: "Read Oneshot".
        case .read(_, let label) where numbered(label): L.actionReadChapter(n: label)
        case .read(_, let label): L.actionReadNamed(label: label)
        case .continue(_, let label, let page) where numbered(label): L.actionContinueChapter(n: label, page: Int(page))
        case .continue(_, let label, let page): L.actionContinueNamedPage(label: label, page: Int(page))
        // A part goes by the source's own name for it where it has one, as its row does.
        case .continuePart(_, _, let percent?, let named?): L.actionContinueNamed(label: midSentence(named), percent: Int(percent))
        case .continuePart(_, let label, let percent?, nil) where !numbered(label): L.actionContinueNamed(label: label, percent: Int(percent))
        case .continuePart(_, let label, let percent?, nil): L.actionContinuePart(n: label, percent: Int(percent))
        case .continuePart(_, _, nil, let named?): L.actionContinueNamedPlain(label: midSentence(named))
        case .continuePart(_, let label, nil, nil) where !numbered(label): L.actionContinueNamedPlain(label: label)
        case .continuePart(_, let label, nil, nil): L.actionContinuePartPlain(n: label)
        case .readPart(_, _, let named?): L.actionReadNamed(label: midSentence(named))
        case .readPart(_, let label, nil) where !numbered(label): L.actionReadNamed(label: label)
        case .readPart(_, let label, nil): L.actionReadPart(n: label)
        case .readAgain: L.actionReadAgain
        case .noSource, .none: nil
        }
    }

    /// The label is a chapter's number, "12", "10.5" or "12-13", which the words put "chapter" or
    /// "part" before; anything else is the chapter's own title.
    static func numbered(_ label: String) -> Bool {
        label.first?.isASCII == true && label.first?.isNumber == true
            && label.allSatisfy { $0.isASCII && ($0.isNumber || $0 == "." || $0 == "-") }
    }

    /// A source's own name for a part, inside a sentence: "Chapter 3" reads "chapter 3" after
    /// Continue, as the app's own chapter words do.
    static func midSentence(_ named: String) -> String {
        let words = ["Chapter ", "Part ", "Volume "] // i18n-skip: a source's own words, matched
        guard let word = words.first(where: { named.hasPrefix($0) }) else { return named }
        return word.lowercased() + named.dropFirst(word.count)
    }

    @MainActor
    static func perform(_ action: SeriesAction?, work: String, store: AppStore, navigator: Navigator) {
        switch action {
        case .read(let chapter, _), .continue(let chapter, _, _), .continuePart(let chapter, _, _, _),
             .readPart(let chapter, _, _):
            store.reader.open(work: work, chapter: chapter)
        case .readAgain:
            navigator.seriesSheet = .readAgain(work)
        case .noSource, .none:
            navigator.seriesSheet = .source(work)
        }
    }

    /// "AniList and MangaUpdates": the trackers that follow the series.
    static func trackers(_ rows: [TrackerReadRow]) -> String {
        rows.map { AccountsModel.name($0.tracker) }.formatted(.list(type: .and).locale(AirTime.locale))
    }

    /// "87 of 88 chapters · volume 9", "Volume 5 of 5 read".
    static func progress(_ row: TrackerReadRow, series: SeriesView) -> String {
        if series.medium == .novel, let volumes = row.volumes {
            return L.seriesTrackerVolumes(volume: Int(volumes), total: Int(series.volumesTotal ?? volumes))
        }
        let chapters = series.chaptersTotal.map { L.seriesTrackerChaptersOf(n: Int(row.chapters), total: Int($0)) }
            ?? L.seriesTrackerChapters(n: Int(row.chapters))
        guard let volume = row.volumes, volume > 0 else { return chapters }
        return L.seriesTrackerWithVolume(chapters: chapters, volume: Int(volume))
    }

    /// "books.home.example · Kavita · 1 more": the source, its version, and how many others have it.
    static func source(_ edition: BookEditionRow, others: Int) -> String {
        var parts = [edition.sourceName]
        if let label = edition.versionLabel, !label.isEmpty { parts.append(label) }
        if others > 0 { parts.append(L.seriesSourcesMore(n: others)) }
        return parts.joined(separator: " · ") // i18n-skip
    }

    /// The second line of a chapter row: the engine's words ("Read · 4 days ago · Downloaded",
    /// "New · yesterday", "Not downloaded"), else its state, when it came out and the group.
    static func chapterLine(_ row: ChapterRow, preferred: String?) -> String {
        if !row.line.isEmpty { return row.line.map(\.text).joined(separator: " · ") } // i18n-skip
        var parts: [String] = []
        if let state = ReadWords.state(row) { parts.append(state) }
        // When it was read, or when a new chapter came out: the date always follows the state it
        // belongs to, so it never reads as when the download was made.
        if let at = row.state == .read ? (row.readAt ?? row.published) : (row.newChapter ? row.published : nil) {
            parts.append(ReadWords.ago(at))
        }
        if row.download == .done { parts.append(L.chapterStateDownloaded) }
        if let label = row.versionLabel, !label.isEmpty, label != preferred || row.versions > 1 { parts.append(label) }
        if row.versions > 1 { parts.append(L.chapterUpdatesVersions(n: Int(row.versions))) }
        return parts.joined(separator: " · ") // i18n-skip
    }

    /// "Volume 9 · chapters 77 to 84 · read", or "Not in a volume yet · chapters 85 to 88", for
    /// manga and novels alike.
    static func volume(_ group: VolumeGroup, novel: Bool) -> (name: String, rest: String) {
        let name = group.volume.map { L.seriesVolume(n: $0) } ?? L.seriesNoVolume
        var rest: [String] = []
        let numbers = group.chapters.compactMap { $0.number.flatMap(Double.init) }
        if let low = numbers.min(), let high = numbers.max(), low != high {
            rest.append(L.seriesChapterRange(from: number(low), to: number(high)))
        }
        if group.allRead { rest.append(L.seriesVolumeRead) }
        return (name, rest.joined(separator: " · ")) // i18n-skip
    }

    private static func number(_ value: Double) -> String {
        value.rounded() == value ? String(Int(value)) : String(value)
    }
}

// MARK: - The page

struct SeriesPage: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    #if os(iOS)
    @Environment(\.horizontalSizeClass) private var sizeClass
    #endif
    let work: String
    @State private var choosing: Set<String>?
    @State private var newestFirst = true
    /// The heading has scrolled under the bar: the bar takes the title, as the title page does.
    @State private var headingGone = false

    private var wide: Bool {
        #if os(iOS)
        sizeClass == .regular
        #else
        true
        #endif
    }

    @ViewBuilder
    private func chooseBar(_ series: SeriesView?) -> some View {
        if let chosen = choosing, let series {
            ChooseChaptersBar(series: series, chosen: chosen) { choosing = nil }
                .transition(.move(edge: .bottom).combined(with: .opacity))
        }
    }

    var body: some View {
        let series = store.reading.series[work]
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                if let series {
                    if wide {
                        WideSeriesHeader(series: series, choosing: $choosing)
                    } else {
                        PhoneSeriesHeader(series: series, choosing: $choosing)
                    }
                    if case .send? = navigator.seriesSheet {} else { SendStatusRows(work: work) }
                    if let demo = series.demo {
                        // The demo's one chapter on its own row: no volumes, order or choosing.
                        DemoChapter(work: work, demo: demo, chapters: store.reading.chapters[work])
                            .padding(.top, wide ? 32 : 24)
                    } else {
                        SeriesChapters(series: series, chapters: store.reading.chapters[work],
                                       choosing: $choosing, newestFirst: $newestFirst)
                            .padding(.top, wide ? 32 : 24)
                    }
                    if let demo = series.demo, let other = series.demoOther,
                       let info = store.demo.reading.works.first(where: { $0.id != demo.id }) {
                        DemoAlso(id: other, info: info)
                            .padding(.top, wide ? 32 : 24)
                    }
                } else if let failure = store.reading.failures[.series] {
                    // The header that was known stays above why the rest did not come.
                    if let tile = store.reading.knownTile(work) {
                        KnownSeriesHeader(tile: tile, wide: wide).padding(.bottom, 24)
                    }
                    ScreenFailed(title: L.titleLoadFailed, reason: failure.words) { store.reading.loadSeries(work) }
                } else if let tile = store.reading.knownTile(work) {
                    // What the tile tapped already showed, at once and where the header puts it.
                    KnownSeriesHeader(tile: tile, wide: wide)
                } else {
                    HomeWait(active: true, activity: "loading the series") { // i18n-skip
                        SeriesPlaceholder()
                    }
                }
            }
            .padding(.horizontal, wide ? 24 : 16)
            .padding(.top, wide ? 8 : 0)
            .padding(.bottom, 32)
            .frame(maxWidth: 1_120, alignment: .leading)
            .frame(maxWidth: .infinity, alignment: .topLeading)
        }
        .scrollIndicators(.automatic)
        .onScrollGeometryChange(for: Bool.self) { geometry in
            geometry.contentOffset.y + geometry.contentInsets.top > (wide ? 80 : 56)
        } action: { _, gone in
            withAnimation(.quick) { headingGone = gone }
        }
        .background(.ground)
        // The list insets above the bar and fades under it, as a bottom toolbar does.
        .safeAreaBar(edge: .bottom) { chooseBar(series) }
        #if os(iOS)
        // Choosing is a mode of its own: its bar takes the tab bar's place, as in Photos.
        .toolbar(choosing == nil ? .automatic : .hidden, for: .tabBar)
        #endif
        .animation(.standard, value: choosing == nil)
        .task(id: work) {
            store.reading.loadSeries(work)
            store.reading.loadTracking(work)
        }
        // Shots: Read from with a source that had no series of this name, and choosing chapters.
        .onLaunchSheet("read-from", navigator) { // i18n-skip: a shot's sheet name
            navigator.seriesSheet = .source(work)
            Task { await store.bookSources.stageMissShot(work: work, cover: store.reading.series[work]?.cover) }
        }
        // Shots: Send to e-reader with the first three chapters, and Where did you stop after a send.
        .task {
            let name = navigator.launchSheet
            guard name == "send" || name == "stopped" else { return } // i18n-skip: a shot's sheet name
            for _ in 0..<50 where store.reading.chapters[work] == nil || store.reading.sending.targets.isEmpty {
                try? await Task.sleep(for: .milliseconds(100))
            }
            let ids = Array((store.reading.chapters[work]?.volumes.flatMap(\.chapters) ?? []).prefix(3).map(\.id))
            if name == "stopped", let target = store.reading.sending.defaultTarget { // i18n-skip: a shot's sheet name
                store.reading.sending.stageShared(work: work, chapters: ids, target: target.id)
                for _ in 0..<50 where store.reading.series[work]?.askWhereStopped == nil {
                    try? await Task.sleep(for: .milliseconds(100))
                }
                navigator.seriesSheet = .stopped(work)
            } else {
                navigator.seriesSheet = .send(work, chapters: ids)
            }
            try? await Task.sleep(for: .milliseconds(500))
            navigator.launchSheet = nil
        }
        .task {
            // The chapters Download's Choose chapters starts with.
            guard navigator.launchSheet == "choose-chapters" else { return } // i18n-skip: a shot's sheet name
            for _ in 0..<30 where store.reading.chapters[work] == nil {
                try? await Task.sleep(for: .milliseconds(100))
            }
            choosing = SeriesChoosing.start(store.reading.chapters[work], action: store.reading.series[work]?.action)
            // Taken once the bar is up, so the snapshot waits for it.
            try? await Task.sleep(for: .milliseconds(500))
            navigator.launchSheet = nil
        }
        .sheet(item: Binding(
            get: { navigator.seriesSheet.flatMap { $0.work == work ? $0 : nil } },
            set: { navigator.seriesSheet = $0 }
        )) { sheet in
            switch sheet {
            case .source: ReadFromSheet(work: work)
            case .tracking: SeriesTrackingSheet(work: work)
            case .readAgain: EmptyView()
            case .send(_, let chapters): SendSheet(request: SendRequest(work: work, chapters: chapters))
            case .stopped:
                if let sent = series?.askWhereStopped { StoppedSheet(work: work, sent: sent) }
            }
        }
        .confirmationDialog(
            L.readAgainTitle(title: series?.titles.display ?? ""),
            isPresented: Binding(
                get: { navigator.seriesSheet == .readAgain(work) },
                set: { if !$0 { navigator.seriesSheet = nil } }
            ),
            titleVisibility: .visible
        ) {
            Button(L.readAgainConfirm) { store.reading.readAgain(work) }
            Button(L.actionCancel, role: .cancel) {}
        } message: {
            Text(L.readAgainLine)
        }
        #if os(iOS)
        .navigationTitle(choosing.map { L.seriesChosen(n: $0.count) } ?? (headingGone ? series?.titles.display ?? "" : ""))
        .navigationBarTitleDisplayMode(.inline)
        .navigationBarBackButtonHidden(choosing != nil)
        .toolbar {
            if choosing != nil {
                ToolbarItem(placement: .topBarLeading) {
                    Button(L.actionCancel) { choosing = nil }
                }
                ToolbarItem(placement: .topBarTrailing) {
                    Button(L.seriesChooseAllUnread) {
                        choosing = SeriesChoosing.unread(store.reading.chapters[work])
                    }
                }
            } else if !wide {
                // A wide page has More in its action row.
                ToolbarItem(placement: .topBarTrailing) {
                    if let series { SeriesMoreMenu(series: series, choosing: $choosing) }
                }
            }
        }
        #endif
    }
}

/// Choosing chapters to download or mark: it starts with every unread chapter chosen.
enum SeriesChoosing {
    /// How many chapters Choose chapters starts with.
    static let ahead = 10

    /// Where Download's Choose chapters starts: the next ten unread chapters not downloaded yet,
    /// from the reader's place on. Select all in the bar still takes every unread one.
    static func start(_ chapters: ReadChapters?, action: SeriesAction? = nil) -> Set<String> {
        let rows = chapters?.volumes.flatMap(\.chapters) ?? []
        let at = place(action).flatMap { place in rows.firstIndex { $0.id == place } }
            ?? rows.lastIndex { $0.state == .read }.map { $0 + 1 } ?? 0
        let next = rows[min(at, rows.count)...].filter { $0.state != .read && $0.download == nil }
        return Set(next.prefix(ahead).map(\.id))
    }

    /// The chapter the series' main action reads next, which is the reader's place.
    private static func place(_ action: SeriesAction?) -> ChapterId? {
        switch action {
        case .read(let chapter, _), .continue(let chapter, _, _), .continuePart(let chapter, _, _, _),
             .readPart(let chapter, _, _): chapter
        default: nil
        }
    }

    /// All unread, in the bar.
    static func unread(_ chapters: ReadChapters?) -> Set<String> {
        let rows = chapters?.volumes.flatMap(\.chapters) ?? []
        return Set(rows.filter { $0.state != .read }.map(\.id))
    }
}

extension SeriesSheet {
    var work: String {
        switch self {
        case .source(let work), .tracking(let work), .readAgain(let work), .send(let work, _), .stopped(let work): work
        }
    }
}

// MARK: - Headers

/// The Mac and a wide iPad: the cover, the text with the actions under it, and the side panel.
/// An iPad too narrow for three columns, such as one held upright, puts the panel's rows under
/// them, so the title keeps room for its line.
private struct WideSeriesHeader: View {
    @Environment(AppStore.self) private var store
    let series: SeriesView
    @Binding var choosing: Set<String>?
    @State private var width: CGFloat = 0
    private var demo: Bool { series.demo != nil }

    /// Cover, a text column of about 340 and the panel, with the gaps between.
    private var panelBeside: Bool { Device.isMac || width >= 940 }

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            HStack(alignment: .top, spacing: 28) {
                BookCover(url: series.cover ?? store.reading.knownTile(series.work)?.cover, width: Device.isMac ? 200 : 220)
                VStack(alignment: .leading, spacing: 0) {
                    SeriesTitleBlock(series: series, compact: false)
                    SeriesActions(series: series, stretch: false, choosing: $choosing)
                        .padding(.top, 20)
                }
                .frame(minWidth: 0, maxWidth: .infinity, alignment: .leading)
                if panelBeside, !demo {
                    SeriesPanel(series: series)
                        .frame(width: Device.isMac ? 300 : 320)
                }
            }
            if !panelBeside, !demo {
                SeriesPanel(series: series)
            }
        }
        .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { width = $0 }
    }
}

/// The iPhone: the cover beside the title and facts, the actions across, then the panel's rows.
private struct PhoneSeriesHeader: View {
    @Environment(AppStore.self) private var store
    let series: SeriesView
    @Binding var choosing: Set<String>?

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack(alignment: .top, spacing: 16) {
                BookCover(url: series.cover ?? store.reading.knownTile(series.work)?.cover, width: 116)
                SeriesTitleBlock(series: series, compact: true)
            }
            SeriesActions(series: series, stretch: true, choosing: $choosing)
            if series.demo == nil { SeriesPanel(series: series) }
        }
    }
}

private struct SeriesTitleBlock: View {
    @Environment(AppStore.self) private var store
    let series: SeriesView
    let compact: Bool

    var body: some View {
        if let demo = series.demo {
            DemoSeriesTitle(series: series, demo: demo, compact: compact)
        } else {
            standard
        }
    }

    private var standard: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Until the engine's header names it, the tile it was opened from does.
            Text(series.titles.display.isEmpty ? store.reading.knownTile(series.work)?.title ?? "" : series.titles.display)
                .font(.showTitle(size: compact ? 22 : (Device.isMac ? 30 : 34), relativeTo: .largeTitle))
                .foregroundStyle(.silk)
                .lineLimit(3)
                .fixedSize(horizontal: false, vertical: true)
                .accessibilityAddTraits(.isHeader)
            if !compact, let other = ReadSeriesWords.otherTitle(series.titles, shown: series.titles.display) {
                Text(other)
                    .font(.plat(mac: 13, touch: 15))
                    .foregroundStyle(.haze)
                    .lineLimit(1)
                    .padding(.top, 4)
            }
            let facts = series.facts.map(\.text)
            if compact {
                VStack(alignment: .leading, spacing: 2) {
                    if !facts.isEmpty {
                        Text(facts.joined(separator: " · ")) // i18n-skip
                    }
                    if let rhythm = series.rhythm?.text {
                        Text(rhythm).padding(.top, 6)
                    }
                }
                .font(.ui(15))
                .foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, 6)
            } else {
                let all = facts + [series.rhythm?.text].compactMap { $0 }
                if !all.isEmpty {
                    Text(all.joined(separator: " · ").uppercased()) // i18n-skip
                        .font(.ui(11, weight: .medium))
                        .tracking(0.88)
                        .fixedLineHeight(16, relativeTo: .caption2)
                        .foregroundStyle(.haze)
                        .lineLimit(2)
                        .padding(.top, 12)
                        .accessibilityLabel(Text(all.joined(separator: ", "))) // i18n-skip
                }
                if let synopsis = series.synopsis, !synopsis.isEmpty {
                    Text(synopsis.replacing(/\s*\n+\s*/, with: " "))
                        .font(.plat(mac: 13, touch: 15))
                        .fixedLineHeight(Device.isMac ? 20 : 22, relativeTo: .subheadline)
                        .foregroundStyle(.haze)
                        .lineLimit(4)
                        .fixedSize(horizontal: false, vertical: true)
                        .frame(maxWidth: 460, alignment: .leading)
                        .padding(.top, 14)
                }
            }
        }
    }
}

/// The main action, Status and Download, and More on a desktop.
private struct SeriesActions: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let series: SeriesView
    let stretch: Bool
    @Binding var choosing: Set<String>?
    @State private var removing = false
    @Environment(\.dynamicTypeSize) private var type

    private var height: CGFloat { Device.isMac ? 34 : (stretch ? 50 : 44) }
    private var font: CGFloat { Device.isMac ? 13 : 17 }
    private var secondaryHeight: CGFloat { Device.isMac ? 34 : 44 }
    private var secondaryFont: CGFloat { Device.isMac ? 13 : 15 }

    var body: some View {
        Group {
            if series.demo != nil {
                // The demo is for trying the reader: no lists, tracking or downloads.
                main
            } else if stretch, type.isAccessibilitySize {
                // At the largest sizes each action has the width to itself.
                VStack(spacing: 12) {
                    main
                    statusMenu
                    downloadMenu
                }
            } else if stretch {
                VStack(spacing: 12) {
                    main
                    HStack(spacing: 12) {
                        statusMenu
                        downloadMenu
                    }
                }
            } else {
                row
            }
        }
    }

    /// One line while it fits, else the main action on its own line and the menus under it. A
    /// layout rather than a choice of views, so each menu is made once: a ViewThatFits made every
    /// menu again for each size it was asked about, and in the iPad's three columns that never
    /// ended.
    private var row: some View {
        SeriesActionRow(spacing: 10) {
            main
            statusMenu
            downloadMenu
            SeriesMoreMenu(series: series, choosing: $choosing, circle: secondaryHeight)
        }
    }

    /// Read or go on in amber; with no source, Read from another source in its place, secondary.
    /// While chapters are being chosen, the choosing bar holds the one amber action, so this
    /// steps back and waits.
    private var main: some View { mainButton(icon: !stretch) }

    /// No source is known to hold it yet, but the engine is still asking: the chapters have not
    /// come. The button holds its place until they do, rather than say there is nothing to read.
    private var stillAsking: Bool {
        series.action == .noSource && store.reading.chapters[series.work] == nil
    }

    @ViewBuilder
    private func mainButton(icon: Bool) -> some View {
        if stillAsking {
            Button {} label: {
                Text(L.actionReadPart(n: "1")).lineLimit(1).frame(maxWidth: stretch ? .infinity : nil)
            }
            .buttonStyle(PrimaryCapsuleStyle(height: height, fontSize: font, offIsRaised: true))
            .disabled(true)
            .redacted(reason: .placeholder)
            .accessibilityHidden(true)
        } else if let words = ReadSeriesWords.action(series.action) {
            Button {
                ReadSeriesWords.perform(series.action, work: series.work, store: store, navigator: navigator)
            } label: {
                HStack(spacing: 8) {
                    if icon {
                        Image(systemName: "book").font(.ui(font < 15 ? 12 : 15)).accessibilityHidden(true)
                    }
                    Text(words).lineLimit(type.isAccessibilitySize ? 3 : 1)
                        .multilineTextAlignment(.center)
                        .fixedSize(horizontal: !stretch, vertical: type.isAccessibilitySize)
                }
                .frame(maxWidth: stretch ? .infinity : nil)
            }
            .buttonStyle(PrimaryCapsuleStyle(height: height, fontSize: font, offIsRaised: true))
            .disabled(choosing != nil)
            .pointerHover()
        } else {
            Button {
                navigator.seriesSheet = .source(series.work)
            } label: {
                Text(L.seriesReadFromAnother).lineLimit(1).frame(maxWidth: stretch ? .infinity : nil)
            }
            .buttonStyle(SecondaryCapsuleStyle(height: height, fontSize: font))
            .fixedSize(horizontal: !stretch, vertical: false)
            .pointerHover()
        }
    }

    /// The list it is on, or Add to list: AniList's lists and a score.
    private var statusMenu: some View {
        Menu {
            ForEach(ReadWords.statuses, id: \.self) { status in
                Button {
                    store.reading.setStatus(series.work, status)
                } label: {
                    if series.list == status {
                        Label(ReadWords.list(status), systemImage: "checkmark")
                    } else {
                        Text(ReadWords.list(status))
                    }
                }
            }
            Divider()
            Menu(L.seriesScore) {
                ForEach((1...10).reversed(), id: \.self) { score in
                    Button {
                        store.reading.setScore(series.work, Float(score))
                    } label: {
                        if let current = series.score, Int(current.rounded()) == score {
                            Label(String(score), systemImage: "checkmark")
                        } else {
                            Text(String(score))
                        }
                    }
                }
            }
            if series.list != nil {
                Divider()
                Button(L.seriesRemoveFromLists) { removing = true }
            }
        } label: {
            menuLabel(series.list.map(ReadWords.list) ?? L.seriesAddToList)
        }
        // Named by its words alone: UIKit reads a menu's label whole, the chevron as "Go Down".
        .accessibilityLabel(series.list.map(ReadWords.list) ?? L.seriesAddToList)
        .menuStyle(.button)
        .buttonStyle(SecondaryCapsuleStyle(height: secondaryHeight, fontSize: secondaryFont))
        .menuIndicator(.hidden)
        .fixedSize(horizontal: !stretch, vertical: false)
        .pointerHover()
        // Asked from this menu, so the question points at it.
        .removeFromLists(series: series, asking: $removing)
    }

    /// The next 5 or 10 unread, every unread chapter, a volume, Choose chapters, and delete.
    private var downloadMenu: some View {
        let volumes = store.reading.chapters[series.work]?.volumes ?? []
        let unread = volumes.flatMap(\.chapters).filter { $0.state != .read && $0.download == nil }
        let downloaded = volumes.flatMap(\.chapters).filter { $0.download != nil }
        return Menu {
            Button(L.downloadNext(n: 5)) { store.reading.download(series.work, .nextUnread(n: 5)) }
            Button(L.downloadNext(n: 10)) { store.reading.download(series.work, .nextUnread(n: 10)) }
            Button(L.downloadUnread) { store.reading.download(series.work, .unread) }
                .disabled(unread.isEmpty)
            let toFetch = volumes.filter { group in group.chapters.contains { $0.download == nil } }
            if !toFetch.isEmpty { Divider() }
            ForEach(toFetch, id: \.volume) { group in
                let count = group.chapters.filter { $0.download == nil }.count
                Button(L.downloadVolumeCount(volume: ReadSeriesWords.volume(group, novel: false).name, n: count)) {
                    if let volume = group.volume {
                        store.reading.download(series.work, .volume(volume: volume))
                    } else {
                        store.reading.download(series.work, .chapters(ids: group.chapters.filter { $0.download == nil }.map(\.id)))
                    }
                }
            }
            Divider()
            Button(L.downloadChoose) {
                choosing = SeriesChoosing.start(store.reading.chapters[series.work], action: series.action)
            }
            if !downloaded.isEmpty {
                Divider()
                Button(L.downloadDeleteSeries, role: .destructive) {
                    store.reading.deleteDownloads(series.work, chapters: downloaded.map(\.id))
                }
            }
        } label: {
            menuLabel(L.seriesDownload)
        }
        .accessibilityLabel(L.seriesDownload)
        .menuStyle(.button)
        .buttonStyle(SecondaryCapsuleStyle(height: secondaryHeight, fontSize: secondaryFont))
        .menuIndicator(.hidden)
        .fixedSize(horizontal: !stretch, vertical: false)
        .pointerHover()
    }

    private func menuLabel(_ title: String) -> some View {
        HStack(spacing: 6) {
            Text(title).lineLimit(1)
            Image(systemName: "chevron.down").font(.ui(secondaryFont - 3, weight: .semibold)).accessibilityHidden(true)
        }
        .frame(maxWidth: stretch ? .infinity : nil)
    }
}

/// The main action and its menus on one line, or the main action over the rest when the line
/// is too narrow. Each keeps its own width.
private struct SeriesActionRow: Layout {
    var spacing: CGFloat

    private func lines(_ subviews: Subviews, width: CGFloat?) -> [[Int]] {
        let all = Array(subviews.indices)
        guard let width, all.count > 1 else { return [all] }
        let sizes = subviews.map { $0.sizeThatFits(.unspecified).width }
        let one = sizes.reduce(0, +) + spacing * CGFloat(sizes.count - 1)
        return one <= width ? [all] : [[all[0]], Array(all.dropFirst())]
    }

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        var width: CGFloat = 0, height: CGFloat = 0
        for (n, line) in lines(subviews, width: proposal.width).enumerated() {
            let sizes = line.map { subviews[$0].sizeThatFits(.unspecified) }
            width = max(width, sizes.map(\.width).reduce(0, +) + spacing * CGFloat(max(sizes.count - 1, 0)))
            height += (n > 0 ? spacing : 0) + (sizes.map(\.height).max() ?? 0)
        }
        return CGSize(width: width, height: height)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        var y = bounds.minY
        for line in lines(subviews, width: bounds.width) {
            let sizes = line.map { subviews[$0].sizeThatFits(.unspecified) }
            let tall = sizes.map(\.height).max() ?? 0
            var x = bounds.minX
            for (index, size) in zip(line, sizes) {
                subviews[index].place(at: CGPoint(x: x, y: y + (tall - size.height) / 2), proposal: ProposedViewSize(size))
                x += size.width + spacing
            }
            y += tall + spacing
        }
    }
}

/// More: the trackers, Read from, and Read again from the start. Choosing chapters lives in
/// Download.
private struct SeriesMoreMenu: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let series: SeriesView
    @Binding var choosing: Set<String>?
    var circle: CGFloat?

    var body: some View {
        Menu {
            Button(L.seriesTracking) { navigator.seriesSheet = .tracking(series.work) }
            Button(L.seriesReadFromMenu) { navigator.seriesSheet = .source(series.work) }
            Divider()
            Button(L.seriesReadAgainMenu) { navigator.seriesSheet = .readAgain(series.work) }
        } label: {
            if let circle {
                Image(systemName: "ellipsis")
                    .font(.ui(Device.isMac ? 13 : 15, weight: .semibold))
                    .foregroundStyle(.silk)
                    .frame(width: circle, height: circle)
                    .background(Palette.raised, in: .circle)
                    .overlay(Circle().strokeBorder(Palette.faintRule, lineWidth: 1))
                    .contentShape(.circle)
            } else {
                Label(L.actionMore, systemImage: "ellipsis")
            }
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .menuIndicator(.hidden)
        .fixedSize()
        .pointerHover()
        .accessibilityLabel(L.homeMore)
    }
}

// MARK: - The side panel

/// Reading from, the trackers' progress, the preferred group, and the other medium's version.
private struct SeriesPanel: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.dynamicTypeSize) private var type
    let series: SeriesView

    var body: some View {
        let tracking = store.reading.tracking[series.work] ?? series.tracking
        let followed = tracking.filter { $0.mapping == .mapped }
        let editions = store.bookSources.forWork[series.work]?.editions ?? []
        VStack(alignment: .leading, spacing: 0) {
            if let edition = series.edition {
                rule
                row(label: L.seriesReadingFrom,
                    value: ReadSeriesWords.source(edition, others: max(0, editions.count - 1)),
                    action: L.seriesChange) { navigator.seriesSheet = .source(series.work) }
            }
            if let first = followed.first {
                rule
                row(label: ReadSeriesWords.trackers(followed), value: ReadSeriesWords.progress(first, series: series),
                    action: nil) { navigator.seriesSheet = .tracking(series.work) }
            } else if !tracking.isEmpty {
                rule
                row(label: ReadSeriesWords.trackers(tracking), value: L.seriesTrackerAsk,
                    action: L.seriesChange) { navigator.seriesSheet = .tracking(series.work) }
            }
            if let label = series.edition?.versionLabel, !label.isEmpty {
                rule
                row(label: label, value: L.seriesPreferredGroup, action: L.seriesChange) {
                    navigator.seriesSheet = .source(series.work)
                }
            }
            if let sent = series.askWhereStopped {
                rule
                row(label: L.sendTitle, value: L.stopNote(device: SendWords.device(sent.target, store)),
                    action: L.stopNoteAction) { navigator.seriesSheet = .stopped(series.work) }
            }
            if store.medium.both, let other = series.otherMedium {
                rule
                Button {
                    if other.medium == .anime, let id = AnilistId(other.work.split(separator: ":").last ?? "") {
                        navigator.openTitle(id)
                    } else {
                        navigator.openSeries(other.work)
                    }
                } label: {
                    HStack {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(other.relation.text).font(.plat(mac: 11, touch: 13)).foregroundStyle(.haze)
                            Text(other.title).font(.plat(mac: 13, touch: 17)).foregroundStyle(.silk).lineLimit(1)
                        }
                        Spacer(minLength: 8)
                        Image(systemName: "chevron.right").font(.ui(12, weight: .semibold)).foregroundStyle(.haze)
                            .accessibilityHidden(true)
                    }
                    .padding(.vertical, Device.isMac ? 8 : 12)
                    .contentShape(.rect)
                }
                .buttonStyle(.plain)
                .pointerHover()
            }
        }
        .task(id: series.work) {
            store.bookSources.loadForWork(series.work)
            store.reading.sending.loadTargets()
        }
    }

    private var rule: some View {
        Rectangle().fill(Palette.faintRule).frame(height: 1).accessibilityHidden(true)
    }

    private func row(label: String, value: String, action: String?, perform: @escaping () -> Void) -> some View {
        let words = VStack(alignment: .leading, spacing: 2) {
            Text(label).font(.plat(mac: 11, touch: 13)).foregroundStyle(.haze).lineLimit(type.isAccessibilitySize ? 3 : 1)
            Text(value).font(.plat(mac: 13, touch: 17)).foregroundStyle(.silk).lineLimit(type.isAccessibilitySize ? 4 : 2)
        }
        let button = action.map { action in
            Button(action, action: perform)
                .buttonStyle(.plain)
                .font(.plat(mac: 13, touch: 15, weight: .semibold))
                .foregroundStyle(.silk)
                .fixedSize()
                .frame(minHeight: Device.isMac ? 24 : 44)
                .contentShape(.rect)
                .pointerHover()
        }
        return Group {
            // At the largest sizes the button takes a line of its own under the words, whole.
            if type.isAccessibilitySize {
                VStack(alignment: .leading, spacing: 4) {
                    words
                    button
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            } else {
                HStack(alignment: .center, spacing: 12) {
                    words
                    Spacer(minLength: 8)
                    button
                }
            }
        }
        .padding(.vertical, Device.isMac ? 8 : 12)
        .accessibilityElement(children: .combine)
    }
}

// MARK: - Chapters

private struct SeriesChapters: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let series: SeriesView
    let chapters: ReadChapters?
    @Binding var choosing: Set<String>?
    @Binding var newestFirst: Bool
    @State private var open: Set<String> = []
    @State private var closed: Set<String> = []

    private var novel: Bool { series.medium == .novel }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(alignment: .firstTextBaseline) {
                Text(novel ? L.seriesVolumes : L.seriesChapters)
                    .font(.ui(Device.isMac ? 17 : 20, weight: Device.isMac ? .semibold : .bold))
                    .foregroundStyle(.silk)
                    .accessibilityAddTraits(.isHeader)
                Spacer()
                Menu {
                    Picker(L.seriesOrder, selection: $newestFirst) {
                        Text(L.seriesNewestFirst).tag(true)
                        Text(L.seriesOldestFirst).tag(false)
                    }
                    .pickerStyle(.inline)
                } label: {
                    HStack(spacing: 4) {
                        Text(newestFirst ? L.seriesNewestFirst : L.seriesOldestFirst)
                        Image(systemName: "chevron.down").font(.ui(10, weight: .semibold)).accessibilityHidden(true)
                    }
                    .font(.plat(mac: 13, touch: 15, weight: .semibold))
                    .foregroundStyle(.silk)
                }
                .accessibilityLabel(L.seriesOrder)
                .accessibilityValue(newestFirst ? L.seriesNewestFirst : L.seriesOldestFirst)
                .menuStyle(.button)
                .buttonStyle(.plain)
                .menuIndicator(.hidden)
                .fixedSize()
                .pointerHover()
            }
            .padding(.bottom, 8)
            // The rows take the placeholders' place in one crossfade, with nothing above moving.
            // Reduce Motion keeps it: a fade moves nothing.
            ZStack(alignment: .topLeading) {
                if let chapters {
                    list(chapters).transition(.opacity)
                } else {
                    HomeWait(active: true, activity: "loading chapters") { // i18n-skip
                        ForEach(0..<5, id: \.self) { _ in
                            VStack(alignment: .leading, spacing: 4) {
                                Text("Chapter 12 The Far Shore").font(.ui(13)) // i18n-skip
                                Text("Read · 3 weeks ago").font(.ui(12)) // i18n-skip
                            }
                            .foregroundStyle(.haze)
                        }
                    }
                    .transition(.opacity)
                }
            }
            .animation(.standard, value: chapters == nil)
        }
    }

    @ViewBuilder
    private func list(_ chapters: ReadChapters) -> some View {
        VStack(alignment: .leading, spacing: 0) {
                if chapters.failure == .noSource {
                    // Nothing was ever read from a source, so nothing failed: an empty state that
                    // leads to Read from.
                    rule
                    PlainEmpty(title: L.seriesNoSourceTitle, line: L.seriesNoSourceLine) {
                        Button(L.seriesNoSourceSearch) { navigator.seriesSheet = .source(series.work) }
                            .buttonStyle(SecondaryCapsuleStyle(height: Device.isMac ? 28 : 40, fontSize: Device.isMac ? 13 : 15))
                    }
                } else if let failure = chapters.failure {
                    failed(failure, chapters: chapters)
                }
                // The engine keeps the source's order, oldest first.
                let groups = newestFirst ? chapters.volumes.reversed() : chapters.volumes
                if groups.isEmpty, chapters.failure == nil {
                    rule
                    PlainEmpty(title: L.seriesNoChaptersTitle, line: L.seriesNoChaptersLine) {
                        Button(L.seriesReadFromAnother) { navigator.seriesSheet = .source(series.work) }
                            .buttonStyle(SecondaryCapsuleStyle(height: Device.isMac ? 28 : 40, fontSize: Device.isMac ? 13 : 15))
                    }
                }
                ForEach(groups, id: \.volume) { group in
                    volume(group)
                }
        }
    }

    private var rule: some View {
        Rectangle().fill(Palette.faintRule).frame(height: 1).accessibilityHidden(true)
    }

    /// What failed and where, Try again, and the downloaded chapters still listed under it.
    private func failed(_ failure: ReadingFailure, chapters: ReadChapters) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(.vermilion).accessibilityHidden(true)
                Text(L.seriesChaptersFailed(source: chapters.edition?.sourceName ?? series.edition?.sourceName ?? ""))
                    .foregroundStyle(.silk)
            }
            .font(.plat(mac: 13, touch: 17, weight: .semibold))
            Text(failure.words)
                .font(.plat(mac: 12, touch: 15))
                .foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true)
            Button(L.readerTryAgain) { store.reading.loadChapters(series.work, edition: chapters.edition?.sourceId) }
                .buttonStyle(SecondaryCapsuleStyle(height: Device.isMac ? 28 : 40, fontSize: Device.isMac ? 13 : 15))
                .padding(.top, 6)
        }
        .padding(.vertical, 16)
    }

    private func key(_ group: VolumeGroup) -> String { group.volume ?? "" }

    private func isOpen(_ group: VolumeGroup) -> Bool {
        group.allRead ? open.contains(key(group)) : !closed.contains(key(group))
    }

    @ViewBuilder
    private func volume(_ group: VolumeGroup) -> some View {
        let words = ReadSeriesWords.volume(group, novel: novel)
        let shown = isOpen(group)
        rule
        Button {
            withAnimation(.standard) {
                if group.allRead {
                    if shown { open.remove(key(group)) } else { open.insert(key(group)) }
                } else {
                    if shown { closed.insert(key(group)) } else { closed.remove(key(group)) }
                }
            }
        } label: {
            HStack(spacing: 0) {
                Text(words.name).font(.plat(mac: 13, touch: 17, weight: .semibold)).foregroundStyle(.silk)
                if !words.rest.isEmpty {
                    Text(" · " + words.rest) // i18n-skip
                        .font(.plat(mac: 13, touch: 17))
                        .foregroundStyle(.haze)
                }
                Spacer(minLength: 8)
                Image(systemName: shown ? "chevron.down" : "chevron.right")
                    .font(.ui(12, weight: .semibold))
                    .foregroundStyle(.haze)
                    .accessibilityHidden(true)
            }
            .lineLimit(1)
            .padding(.horizontal, Device.isMac ? 12 : 0)
            .frame(minHeight: Device.isMac ? 40 : 52)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .pointerHover()
        // Read as "Volume 9, chapters 77 to 84": the dot between them is only for the eye.
        .accessibilityLabel(words.rest.isEmpty ? words.name : "\(words.name), \(words.rest)") // i18n-skip
        .accessibilityValue(shown ? L.seriesExpanded : L.seriesCollapsed)
        if shown {
            let rows = newestFirst ? group.chapters.reversed() : group.chapters
            ForEach(rows, id: \.id) { row in
                rule
                ChapterRowView(series: series, row: row, preferred: chapters?.edition?.versionLabel, choosing: $choosing)
            }
        }
    }
}

/// One chapter: "Chapter 88" in silk with its title in haze, and its state in words under it.
/// The ring shows only while bytes move, and stops the download when pressed.
private struct ChapterRowView: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let series: SeriesView
    let row: ChapterRow
    let preferred: String?
    @Binding var choosing: Set<String>?

    var body: some View {
        let unreadNew = row.newChapter && row.state != .read
        let chosen = choosing?.contains(row.id) ?? false
        Button {
            if var set = choosing {
                if chosen { set.remove(row.id) } else { set.insert(row.id) }
                choosing = set
            } else {
                store.reader.open(work: series.work, chapter: row.id)
            }
        } label: {
            HStack(spacing: 12) {
                if choosing != nil {
                    Image(systemName: chosen ? "checkmark.circle.fill" : "circle")
                        .font(.ui(Device.isMac ? 16 : 22))
                        .foregroundStyle(chosen ? .silk : .haze)
                        .accessibilityHidden(true)
                }
                VStack(alignment: .leading, spacing: 2) {
                    HStack(spacing: 6) {
                        let name = row.number.map { L.seriesChapterNumber(n: $0) } ?? row.label
                        Text(name)
                            .foregroundStyle(row.state == .read ? .haze : .silk)
                        // A file named for its chapter has no title beyond the name.
                        if let title = row.title, !title.isEmpty, title != name, title != row.label {
                            Text(title).foregroundStyle(.haze)
                        }
                        if row.bookmarked {
                            Image(systemName: "bookmark.fill").font(.ui(10)).foregroundStyle(.haze)
                                .accessibilityLabel(L.seriesBookmarked)
                        }
                    }
                    .font(.plat(mac: 13, touch: 17))
                    .lineLimit(1)
                    let line = ReadSeriesWords.chapterLine(row, preferred: preferred)
                    if !line.isEmpty {
                        HStack(spacing: 6) {
                            if unreadNew { WaterDot() }
                            Text(line).lineLimit(1)
                        }
                        .font(.plat(mac: 12, touch: 15))
                        .foregroundStyle(unreadNew ? .silk : .haze)
                    }
                }
                Spacer(minLength: 8)
                if case .downloading(let done, let total) = row.download, total > 0 {
                    Button {
                        store.reading.cancelDownloads(series.work, chapters: [row.id])
                    } label: {
                        DownloadRing(fraction: Double(done) / Double(total))
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(L.chapterDownloadStop)
                }
                if case .failed = row.download {
                    Button(L.chapterRetry) { store.reading.download(series.work, .chapters(ids: [row.id])) }
                        .buttonStyle(SecondaryCapsuleStyle(height: Device.isMac ? 24 : 32, fontSize: Device.isMac ? 12 : 15))
                        .fixedSize()
                }
            }
            .padding(.horizontal, Device.isMac ? 12 : 0)
            .padding(.vertical, Device.isMac ? 6 : 9)
            .frame(minHeight: Device.isMac ? 46 : 62)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .pointerHover()
        .contextMenu { ChapterMenu(series: series, row: row) }
        #if os(iOS)
        .swipeActions(edge: .trailing) {
            Button(row.state == .read ? L.seriesMarkUnread : L.seriesMarkRead) {
                store.reading.markRead(series.work, chapters: [row.id], read: row.state != .read)
            }
            .tint(Palette.raisedSelected)
            if row.download == nil {
                Button(L.seriesDownload) { store.reading.download(series.work, .chapters(ids: [row.id])) }
                    .tint(Palette.raised)
            }
        }
        .accessibilityAction(named: L.sendChapterAction) { navigator.seriesSheet = .send(series.work, chapters: [row.id]) }
        #endif
    }
}

/// A chapter's menu: read it, mark it, download it, bookmark it, and its versions last.
private struct ChapterMenu: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let series: SeriesView
    let row: ChapterRow

    var body: some View {
        Button(L.seriesReadChapterMenu) { store.reader.open(work: series.work, chapter: row.id) }
        if row.state == .read {
            Button(L.seriesMarkUnread) { store.reading.markRead(series.work, chapters: [row.id], read: false) }
        } else {
            Button(L.seriesMarkRead) { store.reading.markRead(series.work, chapters: [row.id], read: true) }
        }
        Button(L.seriesMarkReadUpTo) { store.reading.markReadUpTo(series.work, chapter: row.id) }
        switch row.download {
        case .none:
            Button(L.seriesDownload) { store.reading.download(series.work, .chapters(ids: [row.id])) }
        case .done:
            Button(L.chapterDownloadDelete, role: .destructive) { store.reading.deleteDownloads(series.work, chapters: [row.id]) }
        case .failed:
            Button(L.chapterRetry) { store.reading.download(series.work, .chapters(ids: [row.id])) }
        default:
            Button(L.chapterDownloadStop) { store.reading.cancelDownloads(series.work, chapters: [row.id]) }
        }
        Button(L.sendTitle) { navigator.seriesSheet = .send(series.work, chapters: [row.id]) }
        Button(row.bookmarked ? L.seriesRemoveBookmark : L.seriesBookmark) {
            store.reading.bookmark(series.work, chapter: row.id, on: !row.bookmarked)
        }
        if row.versions > 1 {
            Divider()
            ChapterVersionsMenu(series: series, row: row)
        }
    }
}

/// Versions of a chapter: each group's with when it arrived, and making another preferred.
private struct ChapterVersionsMenu: View {
    @Environment(AppStore.self) private var store
    let series: SeriesView
    let row: ChapterRow

    var body: some View {
        let versions = store.reading.versions[row.id] ?? []
        Menu(L.seriesVersions(n: Int(row.versions))) {
            if versions.isEmpty {
                Text(L.seriesVersionsLoading)
            }
            ForEach(versions, id: \.chapterRef) { version in
                let name = version.edition.versionLabel ?? version.edition.sourceName
                if version.edition.preferred {
                    Label(L.seriesVersionInUse(name: name), systemImage: "checkmark")
                } else {
                    Button(L.seriesPreferVersion(name: name)) {
                        store.reading.chooseEdition(series.work, version.edition)
                    }
                }
            }
        }
        .task { store.reading.loadVersions(series.work, chapter: row.id) }
    }
}

/// Choose chapters' bar: Mark read and the one amber action with the count, on glass, with what
/// the download will take above it ("About 62 MB · over Wi-Fi"). Cancel sits in the bar on a
/// desktop and in the navigation bar on touch.
private struct ChooseChaptersBar: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let series: SeriesView
    let chosen: Set<String>
    let done: () -> Void

    var body: some View {
        let estimate = store.reading.estimates[series.work]
        let line = estimate.map { $0.line.map(\.text).joined(separator: " · ") } ?? "" // i18n-skip
        let height: CGFloat = Device.isMac ? 30 : 44
        VStack(spacing: 10) {
            if !chosen.isEmpty, !line.isEmpty {
                // Plain words above the capsule: the bar's scroll edge keeps them readable.
                Text(line)
                    .font(.plat(mac: 12, touch: 13))
                    .foregroundStyle(.haze)
                    .monospacedDigit()
                    .lineLimit(1)
                    .transition(.opacity)
            }
            HStack(spacing: 8) {
                #if os(macOS)
                Button(L.actionCancel, action: done)
                    .buttonStyle(SecondaryCapsuleStyle(height: height, fontSize: 13))
                #endif
                Button(L.seriesMarkRead) {
                    store.reading.markRead(series.work, chapters: Array(chosen), read: true)
                    done()
                }
                .buttonStyle(SecondaryCapsuleStyle(height: height, fontSize: Device.isMac ? 13 : 15))
                .fixedSize()
                .disabled(chosen.isEmpty)
                Button {
                    navigator.seriesSheet = .send(series.work, chapters: Array(chosen))
                    done()
                } label: {
                    #if os(macOS)
                    Text(L.sendTitle)
                    #else
                    Image(systemName: "paperplane")
                        .font(.ui(15, weight: .semibold))
                        .accessibilityLabel(L.sendTitle)
                    #endif
                }
                .buttonStyle(SecondaryCapsuleStyle(height: height, fontSize: Device.isMac ? 13 : 15))
                .fixedSize()
                .disabled(chosen.isEmpty)
                Button {
                    store.reading.download(series.work, .chapters(ids: Array(chosen)))
                    done()
                } label: {
                    Text(L.downloadChapters(n: chosen.count))
                        .lineLimit(1)
                        .frame(maxWidth: Device.isMac ? nil : .infinity)
                }
                .buttonStyle(PrimaryCapsuleStyle(height: height, fontSize: Device.isMac ? 13 : 17))
                .disabled(chosen.isEmpty)
            }
            .padding(8)
            .chromeGlass(in: .capsule)
            .fixedSize(horizontal: Device.isMac, vertical: false)
        }
        .frame(maxWidth: Device.isMac ? nil : 520)
        .frame(maxWidth: .infinity)
        .padding(.horizontal, 16)
        .padding(.top, 8)
        .padding(.bottom, Device.isMac ? 20 : 8)
        .animation(.standard, value: line)
        .task(id: chosen) {
            // A pause while the person is still ticking, then one ask.
            try? await Task.sleep(for: .milliseconds(150))
            guard !Task.isCancelled else { return }
            store.reading.estimateDownload(series.work, .chapters(ids: Array(chosen)))
        }
    }
}

/// Remove from lists: asked first, then what each tracker made of it, a line each.
private struct RemoveFromLists: ViewModifier {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let series: SeriesView
    @Binding var asking: Bool
    @State private var sent = false

    func body(content: Content) -> some View {
        let results = store.reading.removals[series.work]
        content
            .confirmationDialog(
                L.seriesRemoveFromListsTitle(title: series.titles.display),
                isPresented: $asking,
                titleVisibility: .visible
            ) {
                Button(L.actionRemove, role: .destructive) {
                    sent = true
                    store.reading.removeFromLists(series.work)
                }
                Button(L.actionCancel, role: .cancel) {}
            } message: {
                Text(L.seriesRemoveFromListsLine)
            }
            .alert(L.seriesRemovedTitle, isPresented: Binding(
                get: { sent && results != nil },
                set: { if !$0 { sent = false } }
            )) {
                Button(L.actionDone) { sent = false }
            } message: {
                Text((results ?? []).map { $0.line.map(\.text).joined(separator: " · ") }.joined(separator: "\n")) // i18n-skip
            }
            .onLaunchSheet("remove-lists", navigator) { asking = true } // i18n-skip: a shot's sheet name
    }
}

extension View {
    fileprivate func removeFromLists(series: SeriesView, asking: Binding<Bool>) -> some View {
        modifier(RemoveFromLists(series: series, asking: asking))
    }
}

/// The cover and title the tapped tile carried, in the header's own sizes, until the series' own
/// view comes and takes their place without moving them.
private struct KnownSeriesHeader: View {
    let tile: WorkTile
    let wide: Bool

    var body: some View {
        HStack(alignment: .top, spacing: wide ? 28 : 16) {
            BookCover(url: tile.cover, width: wide ? (Device.isMac ? 200 : 220) : 116)
            Text(tile.title)
                .font(.showTitle(size: wide ? (Device.isMac ? 30 : 34) : 22, relativeTo: .largeTitle))
                .foregroundStyle(.silk)
                .lineLimit(3)
                .fixedSize(horizontal: false, vertical: true)
                .accessibilityAddTraits(.isHeader)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
    }
}

private struct SeriesPlaceholder: View {
    var body: some View {
        HStack(alignment: .top, spacing: 28) {
            RoundedRectangle(cornerRadius: 8).fill(.raised).frame(width: 200, height: 300)
            VStack(alignment: .leading, spacing: 10) {
                Text("Kumo no Ue no Toshokan").font(.showTitle(size: 30, relativeTo: .largeTitle)) // i18n-skip
                Text("Manga · Japan · Releasing").font(.ui(12)) // i18n-skip
            }
        }
        .foregroundStyle(.haze)
    }
}
