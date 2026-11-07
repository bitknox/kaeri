// Updates: new chapters by day, newest first, from series you follow. Unread rows are silk and
// bold with the water dot, read rows haze. A row opens the reader at its chapter; its cover opens
// the series. Download new is the one tool, secondary: nothing here is amber.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

struct UpdatesScreen: View {
    @Environment(AppStore.self) private var store
    #if os(iOS)
    @Environment(\.horizontalSizeClass) private var sizeClass
    #endif

    var body: some View {
        let updates = store.reading.updates
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 0) {
                #if os(macOS)
                if let line = UpdatesWords.checked(updates, long: true) {
                    Text(line)
                        .font(.mac(12))
                        .foregroundStyle(.haze)
                        .padding(.horizontal, 12)
                        .padding(.bottom, 10)
                }
                #endif
                if let updates {
                    ForEach(updates.failures, id: \.work) { failure in
                        UpdateFailureRow(failure: failure)
                    }
                    if updates.days.isEmpty {
                        PlainEmpty(title: L.updatesEmptyTitle, line: L.chapterUpdatesEmptyLine)
                            .padding(.horizontal, Self.inset)
                    }
                    ForEach(updates.days, id: \.day) { day in
                        UpdateDayView(day: day)
                    }
                } else {
                    HomeWait(active: true, activity: "loading updates") { // i18n-skip
                        UpdatesPlaceholder()
                    }
                }
            }
            .frame(maxWidth: Self.width, alignment: .leading)
            .padding(.horizontal, Self.margin)
            .padding(.top, Self.top)
            .padding(.bottom, 32)
            .frame(maxWidth: .infinity, alignment: .topLeading)
        }
        .scrollIndicators(.automatic)
        .background(.ground)
        #if os(iOS)
        .refreshable { store.reading.checkUpdates() }
        .screenSubtitle(UpdatesWords.checked(updates, long: false) ?? "")
        .toolbar {
            ToolbarItem(placement: .primaryAction) { UpdatesDownloadNew() }
            if sizeClass == .compact { AccountButton() }
        }
        #endif
        .task { store.reading.loadUpdates() }
        .animation(.standard, value: updates == nil)
    }

    #if os(macOS)
    static let width: CGFloat = 772
    static let margin: CGFloat = 16
    static let top: CGFloat = 8
    static let inset: CGFloat = 12
    #else
    static let width: CGFloat = 820
    static let margin: CGFloat = 16
    static let top: CGFloat = 4
    static let inset: CGFloat = 0
    #endif
}

enum UpdatesWords {
    /// "Checked 12 min ago", with the Mac's "· series you are caught up on" when `long`.
    static func checked(_ updates: UpdatesView?, long: Bool) -> String? {
        guard let updates else { return nil }
        if updates.checking { return L.chapterUpdatesChecking }
        guard let at = updates.checkedAt else { return nil }
        let ago = ReadWords.ago(at)
        return long ? L.chapterUpdatesCheckedCaughtUp(ago: ago) : L.updatesChecked(ago: ago)
    }

    /// "Chapter 88 · Bluefinch Group", "Chapter 43 · 2 versions": the label, then the versions.
    static func line(_ row: ChapterRow) -> String {
        var parts = [row.label]
        if row.versions > 1 {
            parts.append(L.chapterUpdatesVersions(n: Int(row.versions)))
        } else if let label = row.versionLabel, !label.isEmpty {
            parts.append(label)
        }
        return parts.joined(separator: " · ") // i18n-skip
    }
}

/// Download new: every unread chapter in the list, queued.
struct UpdatesDownloadNew: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let unread = store.reading.unread
        Button {
            store.reading.downloadNew()
        } label: {
            #if os(macOS)
            Label(L.updatesDownloadNew, systemImage: "arrow.down.to.line").labelStyle(.titleAndIcon)
            #else
            Text(L.updatesDownloadNew)
            #endif
        }
        .disabled(unread == 0)
    }
}

private struct UpdateDayView: View {
    let day: UpdateDay

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(ReadWords.day(day.day))
                .font(.plat(mac: 11, touch: 15, weight: .bold))
                .foregroundStyle(.silk)
                .padding(.horizontal, UpdatesScreen.inset)
                .padding(.top, 14)
                .padding(.bottom, 6)
                .accessibilityAddTraits(.isHeader)
            ForEach(day.rows, id: \.chapter.id) { row in
                Rectangle().fill(Palette.faintRule).frame(height: 1).accessibilityHidden(true)
                UpdateRowView(row: row)
            }
        }
        .padding(.top, 8)
    }
}

/// A new chapter: the cover, the title, the chapter with its source's words, and the time.
private struct UpdateRowView: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let row: UpdateRow

    var body: some View {
        let unread = row.chapter.state != .read
        HStack(spacing: 12) {
            Button {
                navigator.openSeries(row.tile.work)
            } label: {
                BookCover(url: row.tile.cover, width: Self.cover, radius: 4)
            }
            .buttonStyle(.plain)
            .pointerHover()
            .accessibilityLabel(L.chapterUpdatesOpenSeries(title: row.tile.title))
            Button {
                store.reader.open(work: row.tile.work, chapter: row.chapter.id)
            } label: {
                HStack(spacing: 12) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(row.tile.title)
                            .font(.plat(mac: 13, touch: 17, weight: unread ? .semibold : .regular))
                            .foregroundStyle(unread ? .silk : .haze)
                            .lineLimit(1)
                        HStack(spacing: 6) {
                            if unread { WaterDot() }
                            Text(UpdatesWords.line(row.chapter)).lineLimit(1)
                        }
                        .font(.plat(mac: 12, touch: 15))
                        .foregroundStyle(unread ? .silk : .haze)
                        if let state = row.chapter.download.flatMap({ _ in ReadWords.state(row.chapter) }) {
                            Text(state)
                                .font(.plat(mac: 11, touch: 13))
                                .foregroundStyle(.haze)
                                .lineLimit(1)
                        }
                    }
                    Spacer(minLength: 8)
                    if let at = row.chapter.published {
                        Text(ReadWords.clock(at))
                            .font(.plat(mac: 12, touch: 15))
                            .monospacedDigit()
                            .foregroundStyle(.haze)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .contentShape(.rect)
            }
            .buttonStyle(.plain)
            .pointerHover()
            .accessibilityElement(children: .combine)
            .accessibilityHint(L.chapterUpdatesOpenChapterHint)
        }
        .padding(.horizontal, UpdatesScreen.inset)
        .padding(.vertical, 8)
        .contextMenu {
            if unread {
                Button(L.seriesMarkRead) { store.reading.markRead(row.tile.work, chapters: [row.chapter.id], read: true) }
            } else {
                Button(L.seriesMarkUnread) { store.reading.markRead(row.tile.work, chapters: [row.chapter.id], read: false) }
            }
            if row.chapter.download == nil {
                Button(L.seriesDownload) { store.reading.download(row.tile.work, .chapters(ids: [row.chapter.id])) }
            }
            Button(L.chapterUpdatesOpenSeriesMenu) { navigator.openSeries(row.tile.work) }
        }
    }

    #if os(macOS)
    static let cover: CGFloat = 32
    #else
    static let cover: CGFloat = 40
    #endif
}

/// A series the check could not ask: its title, the reason, and Try again.
private struct UpdateFailureRow: View {
    @Environment(AppStore.self) private var store
    let failure: UpdateFailure

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 12) {
            VStack(alignment: .leading, spacing: 2) {
                Text(failure.title)
                    .font(.plat(mac: 13, touch: 17, weight: .semibold))
                    .foregroundStyle(.silk)
                Text(failure.failure.words)
                    .font(.plat(mac: 12, touch: 15))
                    .foregroundStyle(.haze)
                    .fixedSize(horizontal: false, vertical: true)
            }
            Spacer(minLength: 8)
            Button(L.readerTryAgain) { store.reading.checkUpdates() }
                .buttonStyle(SecondaryCapsuleStyle(height: Device.isMac ? 28 : 36, fontSize: Device.isMac ? 12 : 15))
                .fixedSize()
                .pointerHover()
        }
        .padding(.horizontal, UpdatesScreen.inset)
        .padding(.vertical, 10)
    }
}

private struct UpdatesPlaceholder: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Today").font(.ui(12, weight: .bold)) // i18n-skip
            ForEach(0..<4, id: \.self) { _ in
                HStack(spacing: 12) {
                    RoundedRectangle(cornerRadius: 4).fill(.raised).frame(width: 32, height: 48)
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Kumo no Ue no Toshokan").font(.ui(13, weight: .semibold)) // i18n-skip
                        Text("Chapter 12").font(.ui(12)) // i18n-skip
                    }
                }
            }
        }
        .foregroundStyle(.haze)
    }
}
