// Downloads' Reading section: one row per series with what its chapters are doing, a failure's
// cause with Retry, and Delete read for chapters already read, sparing bookmarked ones. First on
// the screen with both media on, as the board has it; alone with reading alone.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

/// A series' downloaded and queued chapters, in the engine's order.
struct BookDownloadSeries: Identifiable {
    let work: String
    let title: String
    let rows: [BookDownloadRow]
    var id: String { work }

    static func group(_ rows: [BookDownloadRow]) -> [BookDownloadSeries] {
        var order: [String] = []
        var byWork: [String: [BookDownloadRow]] = [:]
        for row in rows {
            if byWork[row.work] == nil { order.append(row.work) }
            byWork[row.work, default: []].append(row)
        }
        return order.map { work in
            let rows = byWork[work] ?? []
            return BookDownloadSeries(work: work, title: rows.first?.title ?? "", rows: rows)
        }
    }

    var failed: BookDownloadRow? {
        rows.first { if case .failed = $0.state { true } else { false } }
    }

    /// Chapters done and already read, which Delete read removes; bookmarked ones stay.
    var readDone: [BookDownloadRow] {
        rows.filter { row in
            guard case .done = row.state, !row.chapter.bookmarked else { return false }
            if case .read = row.chapter.state { return true }
            return false
        }
    }

    var done: Int { rows.filter { if case .done = $0.state { true } else { false } }.count }

    /// What the row says: the chapter downloading with its pages and what waits after it, the
    /// failure with its cause, else how many are kept and how many of them are read.
    var line: String {
        if let failed, case .failed(let failure) = failed.state {
            return L.readDownloadsFailed(chapter: Self.name(failed.chapter), reason: failure.words)
        }
        let queued = rows.filter { row in
            switch row.state {
            case .queued, .waitsForWifi: true
            default: false
            }
        }.count
        if let active = rows.first(where: { if case .downloading = $0.state { true } else { false } }),
           case .downloading(let doneParts, let total) = active.state {
            let now = L.readDownloadsActive(chapter: Self.name(active.chapter), done: Int(doneParts), total: Int(total))
            return queued > 0 ? now + " · " + L.readDownloadsMoreQueued(n: queued) : now // i18n-skip
        }
        if queued > 0, done == 0 {
            return rows.contains { if case .waitsForWifi = $0.state { true } else { false } }
                ? L.chapterStateWifi : L.readDownloadsQueued(n: queued)
        }
        let kept = L.readDownloadsKept(n: done)
        let read = readDone.count
        return read > 0 ? kept + " · " + L.readDownloadsRead(n: read) : kept // i18n-skip
    }

    /// A chapter as the chapter list names it: by its number, else the source's label.
    static func name(_ chapter: ChapterRow) -> String {
        chapter.number.map { L.seriesChapterNumber(n: $0) } ?? chapter.label
    }
}

/// The section's heading: Reading, and its chapters and size.
struct ReadDownloadsHeading: View {
    @Environment(AppStore.self) private var store
    let title: String

    var body: some View {
        let meter = store.reading.downloads?.meter
        DownloadsSectionHeading(
            title: title,
            total: meter.flatMap { $0.chapters > 0 ? L.readDownloadsTotal(n: Int($0.chapters), size: Wording.bytes($0.used)) : nil }
        )
    }
}

/// A medium's heading in Downloads with both on: its name and what it holds.
struct DownloadsSectionHeading: View {
    let title: String
    let total: String?

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 12) {
            Text(title)
                .font(.plat(mac: 17, touch: 20, weight: .semibold))
                .foregroundStyle(.silk)
            if let total {
                Text(total)
                    .font(.plat(mac: 13, touch: 15))
                    .foregroundStyle(.haze)
                    .monospacedDigit()
            }
        }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isHeader)
    }
}

/// One series: cover, title, its line, and Retry or Delete read.
struct BookDownloadSeriesRow: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let series: BookDownloadSeries

    var body: some View {
        HStack(spacing: 14) {
            BookCover(url: cover, width: Device.isMac ? 36 : 40)
            VStack(alignment: .leading, spacing: 2) {
                Text(series.title)
                    .font(.plat(mac: 15, touch: 17, weight: .semibold))
                    .foregroundStyle(.silk)
                    .lineLimit(2)
                HStack(alignment: .firstTextBaseline, spacing: 6) {
                    if series.failed != nil {
                        // The one vermilion on the row, as the chapter rows mark a failure.
                        Image(systemName: "exclamationmark.triangle.fill")
                            .font(.plat(mac: 11, touch: 13))
                            .foregroundStyle(.vermilion)
                            .accessibilityHidden(true)
                    }
                    Text(series.line)
                        .font(.plat(mac: 13, touch: 15))
                        .foregroundStyle(series.failed != nil ? .silk : .haze)
                        .monospacedDigit()
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            Spacer(minLength: 8)
            if let active = activeFraction {
                DownloadRing(fraction: active, size: Device.isMac ? 16 : 22)
            }
            if let failed = series.failed {
                Button(L.chapterRetry) {
                    store.reading.download(series.work, .chapters(ids: [failed.chapter.id]))
                }
                .buttonStyle(SecondaryCapsuleStyle(height: Device.isMac ? 24 : 30, raisedSurface: !Device.isMac, fontSize: Device.isMac ? 12 : 15))
                .fixedSize()
            } else if !series.readDone.isEmpty {
                Button(L.readDownloadsDeleteRead) {
                    store.reading.deleteDownloads(series.work, chapters: series.readDone.map(\.chapter.id))
                }
                .buttonStyle(.plain)
                .font(.plat(mac: 12, touch: 15, weight: .semibold))
                .foregroundStyle(.silk)
                .pointerHover()
            }
        }
        .padding(.vertical, Device.isMac ? 10 : 6)
        .contentShape(.rect)
        .onTapGesture { navigator.openSeries(series.work) }
        // Download rows carry no cover; the series' details bring it.
        .task(id: series.work) { if cover == nil { store.reading.loadSeriesInfo(series.work) } }
        .accessibilityElement(children: .combine)
        .accessibilityAction(named: L.homeDetails) { navigator.openSeries(series.work) }
    }

    /// The cover from the series' details, else from a list holding it.
    private var cover: String? {
        if let cover = store.reading.series[series.work]?.cover { return cover }
        for tiles in store.reading.lists.values {
            if let tile = tiles.first(where: { $0.work == series.work }) { return tile.cover }
        }
        return nil
    }

    private var activeFraction: Double? {
        for row in series.rows {
            if case .downloading(let done, let total) = row.state, total > 0 { return Double(done) / Double(total) }
        }
        return nil
    }
}

/// A small ring for a chapter downloading, as the chapter rows draw it.
private struct DownloadRing: View {
    let fraction: Double
    let size: CGFloat

    var body: some View {
        ZStack {
            Circle().stroke(Palette.faintRule, lineWidth: 2)
            Circle().trim(from: 0, to: fraction).stroke(.silk, style: StrokeStyle(lineWidth: 2, lineCap: .round))
                .rotationEffect(.degrees(-90))
        }
        .frame(width: size, height: size)
        .accessibilityHidden(true)
    }
}

/// The Mac's and iPad's Reading section: heading, then a row per series.
struct ReadDownloadsSection: View {
    @Environment(AppStore.self) private var store
    /// With both media on, the section is headed Reading, above Shows.
    var headed = true

    var body: some View {
        let series = BookDownloadSeries.group(store.reading.downloads?.rows ?? [])
        VStack(alignment: .leading, spacing: 0) {
            if headed {
                ReadDownloadsHeading(title: L.settingsGroupReading)
                    .padding(.top, 12)
                    .padding(.bottom, 8)
            }
            ForEach(Array(series.enumerated()), id: \.element.id) { index, item in
                if index > 0 { Rectangle().fill(.faintRule).frame(height: 1).padding(.leading, 50) }
                BookDownloadSeriesRow(series: item)
            }
        }
    }
}
