// The series page's two sheets. Read from lists every source that has the series with what it
// holds; choosing one keeps the read marks. Tracking shows what each tracker holds for the
// series, a tracker behind says when it catches up, and trackers that can't track it come last.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

/// The frame both sheets share: the title with Cancel or Done, as the board draws them.
struct ReadSheetFrame<Content: View>: View {
    @Environment(\.dismiss) private var dismiss
    let title: String
    let subtitle: String?
    let close: String
    /// How tall the Mac sheet may grow before its content scrolls.
    var tallest: CGFloat = 560
    /// Held under the scrolling content on touch, where a long list would push it off screen.
    var pinned: AnyView?
    /// The iPad sheet is as big as its content, not a tall form.
    var fitted = false
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
        .frame(minHeight: 280, maxHeight: tallest)
        .background(.raised)
        .closesOnEscape { dismiss() }
        #else
        NavigationStack {
            ScrollView { content().padding(.horizontal, 16).padding(.vertical, 8) }
                .background(.raised)
                .safeAreaBar(edge: .bottom, spacing: 0) {
                    if let pinned {
                        pinned.padding(.horizontal, 16).padding(.vertical, 12)
                    }
                }
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
        // A sheet with a pinned foot opens tall, so what it asks is on screen with the button.
        .presentationDetents(pinned == nil ? [.medium, .large] : [.large])
        .sheetSizing(fitted: fitted && pinned == nil)
        .presentationBackground(.raised)
        #endif
    }
}

/// Rows in one raised group, with hairlines between them.
struct SheetRows<Content: View>: View {
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

struct SheetHeading: View {
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

struct SheetNote: View {
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
    /// The source "Search there" opened, and the words in its field.
    @State private var searching: String?
    @State private var query = ""

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
                } else if editions?.missing.isEmpty ?? true {
                    PlainEmpty(title: L.seriesReadFromNoneTitle, line: L.seriesReadFromNoneLine)
                        .padding(.horizontal, 12)
                }
                if let missing = editions?.missing, !missing.isEmpty {
                    SheetHeading(text: L.readFromNotFound)
                    SheetRows {
                        ForEach(missing, id: \.sourceId) { miss in
                            missRow(miss, title: title ?? "")
                        }
                    }
                }
            }
        }
        .task { store.bookSources.loadForWork(work) }
        .onDisappear { store.bookSources.clearSourceSearch(work) }
    }

    /// A source searched without finding the series: its name and why in haze, and Search
    /// there, which opens a field under it and lists what the source finds. A possible match,
    /// another edition of the series, sits under the reason, chosen with one tap.
    @ViewBuilder
    private func missRow(_ miss: EditionMiss, title: String) -> some View {
        let open = searching == miss.sourceId
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 12) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(miss.sourceName)
                        .font(.plat(mac: 13, touch: 17))
                        .lineLimit(1)
                    Text(miss.reason.text)
                        .font(.plat(mac: 12, touch: 15))
                        .lineLimit(2)
                }
                .foregroundStyle(.haze)
                Spacer(minLength: 8)
                if !open {
                    Button(L.readFromSearchThere) {
                        // The words most likely to find it there: the romaji when the English
                        // name found nothing.
                        let words = miss.query ?? title
                        searching = miss.sourceId
                        query = words
                        store.bookSources.searchSource(work, sourceId: miss.sourceId, query: words)
                    }
                    .buttonStyle(.plain)
                    .font(.plat(mac: 12, touch: 15, weight: .semibold))
                    .foregroundStyle(.silk)
                    .pointerHover()
                }
            }
            .padding(.horizontal, 12)
            .padding(.vertical, Device.isMac ? 9 : 12)
            if let possible = miss.possible, !open {
                choiceRow(possible, sourceId: miss.sourceId)
                    .padding(.horizontal, 12)
                    .padding(.top, -4)
                    .padding(.bottom, 6)
            }
            if open {
                searchThere(miss)
                    .padding(.horizontal, 12)
                    .padding(.bottom, 10)
            }
        }
    }

    @ViewBuilder
    private func searchThere(_ miss: EditionMiss) -> some View {
        let found = store.bookSources.sourceSeries[work]
        HStack(spacing: 8) {
            TextField(L.readFromSearchPlaceholder(source: miss.sourceName), text: $query)
                .textFieldStyle(.roundedBorder)
                .autocorrectionDisabled()
                .onSubmit { search(miss) }
            Button(L.readFromSearch) { search(miss) }
                .buttonStyle(SecondaryCapsuleStyle(height: Device.isMac ? 24 : 32, raisedSurface: true, fontSize: Device.isMac ? 12 : 15))
                .fixedSize()
                .disabled(query.trimmingCharacters(in: .whitespaces).isEmpty)
        }
        if let found, found.sourceId == miss.sourceId {
            if found.searching {
                ProgressView().controlSize(.small).padding(.top, 8)
            } else if let failure = found.failure {
                FailedLine(failure.words).padding(.top, 8)
            } else if found.choices.isEmpty {
                Text(L.readFromSearchNone)
                    .font(.plat(mac: 12, touch: 15))
                    .foregroundStyle(.haze)
                    .padding(.top, 8)
            } else {
                VStack(alignment: .leading, spacing: 0) {
                    ForEach(found.choices, id: \.seriesRef) { choice in
                        choiceRow(choice, sourceId: miss.sourceId)
                    }
                }
                .padding(.top, 6)
            }
        }
    }

    /// One of a source's series: its cover, title, and year and language, chosen as this work
    /// with one tap.
    private func choiceRow(_ choice: SourceSeriesChoice, sourceId: String) -> some View {
        Button {
            searching = nil
            store.bookSources.chooseSourceSeries(work, sourceId: sourceId, seriesRef: choice.seriesRef)
        } label: {
            HStack(spacing: 10) {
                BookCover(url: choice.cover, width: Device.isMac ? 28 : 34, radius: 3)
                VStack(alignment: .leading, spacing: 2) {
                    Text(choice.title)
                        .font(.plat(mac: 13, touch: 17))
                        .foregroundStyle(.silk)
                        .lineLimit(1)
                    let facts = choiceFacts(choice)
                    if !facts.isEmpty {
                        Text(facts)
                            .font(.plat(mac: 12, touch: 13))
                            .foregroundStyle(.haze)
                    }
                }
                Spacer(minLength: 0)
            }
            .padding(.vertical, 6)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .pointerHover()
    }

    private func search(_ miss: EditionMiss) {
        let words = query.trimmingCharacters(in: .whitespaces)
        guard !words.isEmpty else { return }
        store.bookSources.searchSource(work, sourceId: miss.sourceId, query: words)
    }

    /// "2019 · Japanese": the year and language a source gives for one of its series.
    private func choiceFacts(_ choice: SourceSeriesChoice) -> String {
        var parts: [String] = []
        if let year = choice.year { parts.append(String(year)) }
        if let language = choice.language, !language.isEmpty,
           let name = AirTime.locale.localizedString(forLanguageCode: language) {
            parts.append(name)
        }
        return parts.joined(separator: " · ") // i18n-skip
    }

    private func row(_ edition: BookEditionRow) -> some View {
        Button {
            // The one in use is chosen again too: the series page may still name the source it
            // read from before a new match, and a choice puts it right.
            store.reading.chooseEdition(work, edition)
            dismiss()
        } label: {
            HStack(spacing: 12) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(edition.sourceName)
                        .font(.plat(mac: 13, touch: 17, weight: edition.inUse ? .semibold : .regular))
                        .foregroundStyle(.silk)
                        .lineLimit(1)
                    Text(line(edition))
                        .font(.plat(mac: 12, touch: 15))
                        .foregroundStyle(.haze)
                        .lineLimit(2)
                }
                Spacer(minLength: 8)
                if edition.inUse {
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
        .accessibilityAddTraits(edition.inUse ? .isSelected : [])
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
        var parts = lead
        parts.append(Self.chapters(edition, first: lead.isEmpty))
        if folder { parts.append(L.seriesReadFromOnDevice(device: Device.name)) }
        return parts.joined(separator: " · ") // i18n-skip
    }

    /// The chapters part of an edition's line. A list not asked for yet, or still on its way,
    /// says it is being checked; an empty one says so; one that failed says why. Only a list the
    /// source gave has a range, so it never reads "1 to 0".
    static func chapters(_ edition: BookEditionRow, first: Bool) -> String {
        let checking = first ? L.seriesReadFromChecking : L.seriesReadFromCheckingLater
        let empty = first ? L.seriesReadFromEmpty : L.seriesReadFromEmptyLater
        let has = edition.chapters > 0 || edition.latest != nil
        switch edition.listing {
        case .failed(let failure):
            return failure.words
        case .empty:
            return empty
        case .asking:
            return checking
        case .listed, nil:
            guard has else { return edition.listing == nil ? checking : empty }
            let latest = edition.latest ?? String(edition.chapters)
            return first ? L.seriesReadFromChapters(latest: latest) : L.seriesReadFromChaptersLater(latest: latest)
        }
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

private extension View {
    @ViewBuilder
    func sheetSizing(fitted: Bool) -> some View {
        #if os(iOS)
        if fitted { presentationSizing(.fitted) } else { presentationSizing(.automatic) }
        #else
        self
        #endif
    }
}
