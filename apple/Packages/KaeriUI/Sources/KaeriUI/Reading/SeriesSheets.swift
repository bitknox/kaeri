// The series page's two sheets. Read from lists every source that has the series with what it
// holds; choosing one keeps the read marks. Tracking shows what each tracker holds for the
// series, a tracker behind says when it catches up, and trackers that can't track it come last.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

/// The frame both sheets share: the title with Cancel or Done, as the board draws them.
private struct ReadSheetFrame<Content: View>: View {
    @Environment(\.dismiss) private var dismiss
    let title: String
    let subtitle: String?
    let close: String
    @ViewBuilder var content: () -> Content

    var body: some View {
        #if os(macOS)
        VStack(alignment: .leading, spacing: 0) {
            HStack(alignment: .firstTextBaseline) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(title).font(.mac(17, weight: .bold)).foregroundStyle(.silk)
                        .accessibilityAddTraits(.isHeader)
                    if let subtitle { Text(subtitle).font(.mac(12)).foregroundStyle(.haze) }
                }
                Spacer()
                Button(close) { dismiss() }
                    .buttonStyle(SecondaryCapsuleStyle(height: 28, raisedSurface: true, fontSize: 13))
                    .keyboardShortcut(.cancelAction)
            }
            ScrollView { content().padding(.top, 14) }
                .scrollBounceBehavior(.basedOnSize)
        }
        .padding(24)
        .sheetWidth(480)
        .frame(minHeight: 280, maxHeight: 560)
        .background(.raised)
        .closesOnEscape { dismiss() }
        #else
        NavigationStack {
            ScrollView { content().padding(.horizontal, 16).padding(.vertical, 8) }
                .background(.raised)
                .navigationTitle(title)
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .cancellationAction) { Button(close) { dismiss() } }
                    if let subtitle {
                        ToolbarItem(placement: .principal) {
                            VStack(spacing: 0) {
                                Text(title).font(.ui(17, weight: .semibold)).foregroundStyle(.silk)
                                Text(subtitle).font(.ui(12)).foregroundStyle(.haze)
                            }
                        }
                    }
                }
        }
        .presentationDetents([.medium, .large])
        .presentationBackground(.raised)
        #endif
    }
}

/// Rows in one raised group, with hairlines between them.
private struct SheetRows<Content: View>: View {
    @ViewBuilder var content: () -> Content

    var body: some View {
        VStack(spacing: 0) {
            Group(subviews: content()) { rows in
                ForEach(Array(rows.enumerated()), id: \.offset) { index, row in
                    if index > 0 {
                        Rectangle().fill(Palette.faintRule).frame(height: 1).padding(.leading, 12)
                            .accessibilityHidden(true)
                    }
                    row
                }
            }
        }
        .background(Palette.ground.opacity(0.4), in: .rect(cornerRadius: 10))
        .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(Palette.faintRule, lineWidth: 1))
    }
}

private struct SheetHeading: View {
    let text: String

    var body: some View {
        Text(text)
            .font(.plat(mac: 12, touch: 15, weight: .semibold))
            .foregroundStyle(.haze)
            .padding(.leading, 12)
            .padding(.top, 18)
            .padding(.bottom, 6)
            .accessibilityAddTraits(.isHeader)
    }
}

private struct SheetNote: View {
    let text: String

    var body: some View {
        Text(text)
            .font(.plat(mac: 12, touch: 13))
            .foregroundStyle(.haze)
            .fixedSize(horizontal: false, vertical: true)
            .padding(.horizontal, 12)
            .padding(.top, 8)
    }
}

// MARK: - Read from

struct ReadFromSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    let work: String

    var body: some View {
        let title = store.reading.series[work]?.titles.display
        let editions = store.bookSources.forWork[work]
        ReadSheetFrame(title: L.seriesReadFromTitle, subtitle: title, close: L.actionCancel) {
            VStack(alignment: .leading, spacing: 0) {
                if let editions, !editions.editions.isEmpty {
                    SheetRows {
                        ForEach(Array(editions.editions.enumerated()), id: \.offset) { _, edition in
                            row(edition)
                        }
                    }
                    SheetNote(text: L.seriesReadFromNote)
                } else if editions == nil || editions?.asking == true {
                    HStack(spacing: 8) {
                        ProgressView().controlSize(.small)
                        Text(L.seriesReadFromAsking).foregroundStyle(.haze)
                    }
                    .font(.plat(mac: 13, touch: 15))
                    .padding(12)
                } else {
                    PlainEmpty(title: L.seriesReadFromNoneTitle, line: L.seriesReadFromNoneLine)
                        .padding(.horizontal, 12)
                }
            }
        }
        .task { store.bookSources.loadForWork(work) }
    }

    private func row(_ edition: BookEditionRow) -> some View {
        Button {
            if !edition.preferred { store.reading.chooseEdition(work, edition) }
            dismiss()
        } label: {
            HStack(spacing: 12) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(edition.sourceName)
                        .font(.plat(mac: 13, touch: 17, weight: edition.preferred ? .semibold : .regular))
                        .foregroundStyle(.silk)
                        .lineLimit(1)
                    Text(line(edition))
                        .font(.plat(mac: 12, touch: 15))
                        .foregroundStyle(.haze)
                        .lineLimit(2)
                }
                Spacer(minLength: 8)
                if edition.preferred {
                    Image(systemName: "checkmark")
                        .font(.ui(13, weight: .semibold))
                        .foregroundStyle(.silk)
                        .accessibilityHidden(true)
                }
            }
            .padding(.horizontal, 12)
            .padding(.vertical, Device.isMac ? 9 : 12)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .pointerHover()
        .accessibilityAddTraits(edition.preferred ? .isSelected : [])
    }

    /// "Kavita · chapters 1 to 88", "Chapters 1 to 60 · on this iPhone".
    private func line(_ edition: BookEditionRow) -> String {
        let view = store.bookSources.view
        var lead: [String] = []
        var folder = false
        if let server = view?.servers.first(where: { $0.id == edition.sourceId }) {
            lead.append(BookServerWords.kind(server.kind))
        } else if let plugin = view?.plugins.first(where: { $0.id == edition.sourceId }), let from = plugin.store {
            lead.append(L.bookSourcesFromStore(store: from))
        } else if view?.folders.contains(where: { $0.id == edition.sourceId }) == true {
            folder = true
        }
        if let label = edition.versionLabel, !label.isEmpty { lead.append(label) }
        if let language = edition.language, !language.isEmpty,
           let name = AirTime.locale.localizedString(forLanguageCode: language) {
            lead.append(name)
        }
        let latest = edition.latest ?? String(edition.chapters)
        var parts = lead
        parts.append(lead.isEmpty ? L.seriesReadFromChapters(latest: latest) : L.seriesReadFromChaptersLater(latest: latest))
        if folder { parts.append(L.seriesReadFromOnDevice(device: Device.name)) }
        return parts.joined(separator: " · ") // i18n-skip
    }
}

// MARK: - Tracking

struct SeriesTrackingSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.dismiss) private var dismiss
    let work: String

    /// The trackers that count reading.
    static let readingTrackers = ["anilist", "mangaupdates", "kitsu", "myanimelist"] // i18n-skip

    var body: some View {
        let series = store.reading.series[work]
        let rows = store.reading.tracking[work] ?? series?.tracking ?? []
        let tracked = rows.filter { $0.mapping != .unmapped }
        let away = Self.readingTrackers.filter { id in !rows.contains { $0.tracker == id } }
        let unmapped = rows.filter { $0.mapping == .unmapped }
        let newest = tracked.filter { !$0.divergent }.map(\.chapters).max() ?? tracked.map(\.chapters).max() ?? 0
        ReadSheetFrame(title: L.seriesTracking, subtitle: series?.titles.display, close: L.actionDone) {
            VStack(alignment: .leading, spacing: 0) {
                if !tracked.isEmpty, let series {
                    SheetRows {
                        ForEach(tracked, id: \.tracker) { row in
                            TrackerSheetRow(row: row, series: series, newest: newest)
                        }
                    }
                }
                if let note = tracked.compactMap(\.splitNote).first {
                    SheetNote(text: note.text)
                }
                if !away.isEmpty || !unmapped.isEmpty {
                    SheetHeading(text: L.seriesNotTracked)
                    SheetRows {
                        ForEach(unmapped, id: \.tracker) { row in
                            plain(AccountsModel.name(row.tracker), line: L.seriesTrackerNotThere)
                        }
                        ForEach(away, id: \.self) { tracker in
                            Button {
                                dismiss()
                                navigator.openSettings(.accounts)
                            } label: {
                                plain(AccountsModel.name(tracker), line: L.seriesTrackerConnect)
                            }
                            .buttonStyle(.plain)
                            .pointerHover()
                        }
                    }
                }
            }
        }
        .task { store.reading.loadTracking(work) }
    }

    private func plain(_ name: String, line: String) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(name).font(.plat(mac: 13, touch: 17)).foregroundStyle(.haze)
            Text(line).font(.plat(mac: 12, touch: 15)).foregroundStyle(.haze)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 12)
        .padding(.vertical, Device.isMac ? 9 : 12)
        .contentShape(.rect)
    }
}

/// One tracker: what it holds, its score, and when a tracker behind catches up. A tracker that
/// isn't sure which series this is lists its guesses to choose from.
private struct TrackerSheetRow: View {
    @Environment(AppStore.self) private var store
    let row: TrackerReadRow
    let series: SeriesView
    let newest: UInt32

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(alignment: .firstTextBaseline, spacing: 12) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(AccountsModel.name(row.tracker))
                        .font(.plat(mac: 13, touch: 17, weight: .semibold))
                        .foregroundStyle(.silk)
                    Text(line)
                        .font(.plat(mac: 12, touch: 15))
                        .foregroundStyle(row.divergent ? .silk : .haze)
                        .fixedSize(horizontal: false, vertical: true)
                }
                Spacer(minLength: 8)
                if let score = row.score, score > 0, !row.divergent {
                    Text(L.seriesTrackerScore(score: score.formatted(.number.precision(.fractionLength(0...1)))))
                        .font(.plat(mac: 12, touch: 15))
                        .monospacedDigit()
                        .foregroundStyle(.haze)
                }
                if row.divergent {
                    Menu {
                        Button(L.seriesTrackerKeepKaeri) { store.reading.resolveConflict(series.work, tracker: row.tracker, keepLocal: true) }
                        Button(L.seriesTrackerTakeTheirs(tracker: AccountsModel.name(row.tracker))) {
                            store.reading.resolveConflict(series.work, tracker: row.tracker, keepLocal: false)
                        }
                    } label: {
                        Image(systemName: "ellipsis").foregroundStyle(.haze)
                    }
                    .menuStyle(.button)
                    .buttonStyle(.plain)
                    .menuIndicator(.hidden)
                    .fixedSize()
                    .accessibilityLabel(L.actionMore)
                }
            }
            if case .ask(let candidates) = row.mapping {
                VStack(alignment: .leading, spacing: 0) {
                    ForEach(candidates, id: \.trackerId) { candidate in
                        Button {
                            store.reading.chooseMapping(series.work, tracker: row.tracker, id: candidate.trackerId)
                        } label: {
                            HStack {
                                Text(candidate.title).foregroundStyle(.silk).lineLimit(1)
                                if let year = candidate.year {
                                    Text(String(year)).foregroundStyle(.haze) // i18n-skip
                                }
                                Spacer(minLength: 8)
                                Text(L.libraryMatchPercent(percent: Int(candidate.confidence))).foregroundStyle(.haze)
                            }
                            .font(.plat(mac: 12, touch: 15))
                            .padding(.vertical, 6)
                            .contentShape(.rect)
                        }
                        .buttonStyle(.plain)
                        .pointerHover()
                    }
                    Button(L.seriesTrackerNoneOfThese) {
                        store.reading.chooseMapping(series.work, tracker: row.tracker, id: nil)
                    }
                    .buttonStyle(.plain)
                    .font(.plat(mac: 12, touch: 15))
                    .foregroundStyle(.haze)
                    .padding(.vertical, 6)
                }
                .padding(.top, 6)
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, Device.isMac ? 9 : 12)
        .accessibilityElement(children: .contain)
    }

    private var line: String {
        switch row.mapping {
        case .looking: return L.seriesTrackerLooking
        case .ask: return L.seriesTrackerAsk
        case .mapped, .unmapped: break
        }
        if row.divergent {
            if row.chapters < newest {
                return L.seriesTrackerBehind(has: Int(row.chapters), gets: Int(newest))
            }
            return L.seriesTrackerAhead(has: Int(row.chapters))
        }
        var parts: [String] = []
        if let status = row.status { parts.append(ReadWords.list(status)) }
        parts.append(ReadSeriesWords.progress(row, series: series))
        return parts.joined(separator: " · ") // i18n-skip
    }
}
