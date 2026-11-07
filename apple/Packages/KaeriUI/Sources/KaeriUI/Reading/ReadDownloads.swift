// Downloads' Reading section: one row per series with what its chapters are doing, a failure's
// cause with Retry, and Delete read for chapters already read, sparing bookmarked ones. First on
// the screen with both media on, as the board has it; alone with reading alone.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

extension BookDownloadSeries: @retroactive Identifiable {
    public var id: String { work }

    /// The row's line as the engine words it: "6 chapters · 214 MB · 2 read", the chapter
    /// downloading, or the failure and its cause.
    var words: String { line.map(\.text).joined(separator: " · ") } // i18n-skip

    var hasFailed: Bool { !failed.isEmpty }
}

/// The section's heading: Reading, and its chapters and size.
struct ReadDownloadsHeading: View {
    @Environment(AppStore.self) private var store
    let title: String

    var body: some View {
        DownloadsSectionHeading(title: title, total: ReadDownloadsWords.total(store))
    }
}

enum ReadDownloadsWords {
    /// "38 chapters · 1.1 GB", as the engine words the Reading total; nil with nothing kept.
    @MainActor
    static func total(_ store: AppStore) -> String? {
        guard let view = store.reading.downloads else { return nil }
        if !view.total.isEmpty { return view.total.map(\.text).joined(separator: " · ") } // i18n-skip
        guard view.meter.chapters > 0 else { return nil }
        return L.readDownloadsTotal(n: Int(view.meter.chapters), size: Wording.bytes(view.meter.used))
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
            BookCover(url: series.cover ?? cover, width: Device.isMac ? 36 : 40)
            VStack(alignment: .leading, spacing: 2) {
                Text(series.title)
                    .font(.plat(mac: 15, touch: 17, weight: .semibold))
                    .foregroundStyle(.silk)
                    .lineLimit(2)
                HStack(alignment: .firstTextBaseline, spacing: 6) {
                    if series.hasFailed {
                        // The one vermilion on the row, as the chapter rows mark a failure.
                        Image(systemName: "exclamationmark.triangle.fill")
                            .font(.plat(mac: 11, touch: 13))
                            .foregroundStyle(.vermilion)
                            .accessibilityHidden(true)
                    }
                    Text(series.words)
                        .font(.plat(mac: 13, touch: 15))
                        .foregroundStyle(series.hasFailed ? .silk : .haze)
                        .monospacedDigit()
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            Spacer(minLength: 8)
            if let progress = series.progress {
                Button {
                    store.reading.cancelDownloads(series.work, chapters: downloading)
                } label: {
                    DownloadRing(fraction: Double(progress))
                }
                .buttonStyle(.plain)
                .accessibilityLabel(L.chapterDownloadStop)
            }
            if series.hasFailed {
                Button(L.chapterRetry) {
                    store.reading.download(series.work, .chapters(ids: series.failed))
                }
                .buttonStyle(SecondaryCapsuleStyle(height: Device.isMac ? 24 : 30, raisedSurface: !Device.isMac, fontSize: Device.isMac ? 12 : 15))
                .fixedSize()
            } else if series.read > 0 {
                Button(L.readDownloadsDeleteRead) {
                    store.reading.deleteRead(series.work)
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
        .task(id: series.work) { if series.cover == nil, cover == nil { store.reading.loadSeriesInfo(series.work) } }
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

    /// The series' chapters moving now, which the ring stops.
    private var downloading: [String] {
        (store.reading.downloads?.rows ?? []).filter { row in
            guard row.work == series.work else { return false }
            switch row.state {
            case .downloading, .queued, .waitsForWifi: return true
            default: return false
            }
        }.map(\.chapter.id)
    }
}

/// The ring while bytes move: a silk arc on the rule with a stop square inside, 16 on the Mac
/// and 22 on touch, as the chapter rows and Downloads draw it.
struct DownloadRing: View {
    let fraction: Double

    var body: some View {
        let size: CGFloat = Device.isMac ? 16 : 22
        ZStack {
            Circle().stroke(Palette.rule, lineWidth: 2)
            Circle().trim(from: 0, to: max(0.02, min(fraction, 1)))
                .stroke(.silk, style: StrokeStyle(lineWidth: 2, lineCap: .round))
                .rotationEffect(.degrees(-90))
            RoundedRectangle(cornerRadius: 1)
                .fill(.silk)
                .frame(width: size * 0.3, height: size * 0.3)
        }
        .frame(width: size, height: size)
        .padding(Device.isMac ? 4 : 11)
        .contentShape(.circle)
    }
}

/// The Mac's and iPad's Reading section: heading, then a row per series.
struct ReadDownloadsSection: View {
    @Environment(AppStore.self) private var store
    /// With both media on, the section is headed Reading, above Shows.
    var headed = true

    var body: some View {
        let series = store.reading.downloads?.series ?? []
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
