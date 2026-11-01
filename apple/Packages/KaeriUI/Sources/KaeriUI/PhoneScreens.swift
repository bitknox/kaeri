// The iPhone's screens: the calendar a day at a time, the Library and Search.

#if os(iOS)
import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

/// The account button on Home and Library: Accounts, Sources, Plugin stores and Settings
/// in a sheet.
struct AccountButton: ToolbarContent {
    @Environment(Navigator.self) private var navigator

    var body: some ToolbarContent {
        ToolbarItem(placement: .topBarTrailing) {
            Button(L.navAccountsAndSettings, systemImage: "person.crop.circle") {
                navigator.phoneSettings = .root
            }
        }
    }
}

extension AppStore {
    /// Where sync stands, as the iPhone's Home and Library say it under their titles: the
    /// tracker's state, or with none signed in, that nothing is watched yet when that is so.
    func syncSubtitle(nothingWatched: Bool = false) -> String {
        let summary = accounts.summary
        if summary.state == .signedOut { return nothingWatched ? L.continueEmptyTitle : "" }
        if jobs.shown.contains(where: { $0.activity == "syncing" }) { return L.accountsSyncRunning }
        return summary.text
    }
}

/// A heading over a group on an iPhone screen: 20 semibold.
struct PhoneHeading: View {
    let text: String

    init(_ text: String) {
        self.text = text
    }

    var body: some View {
        Text(text)
            .font(.ui(20, weight: .semibold))
            .fixedLineHeight(24, relativeTo: .title3)
            .foregroundStyle(.silk)
            .accessibilityAddTraits(.isHeader)
    }
}

// MARK: - Calendar

/// The day while the calendar loads, past 600 ms: rows as they will stand, static and redacted.
struct PhoneCalendarPlaceholder: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(L.dayToday).font(.ui(20, weight: .semibold))
            ForEach(0..<3, id: \.self) { _ in
                HStack(spacing: 14) {
                    Text("17:30").font(.ui(15)).frame(minWidth: 52, alignment: .leading) // i18n-skip
                    RoundedRectangle(cornerRadius: 6).fill(.raised).frame(width: 50, height: 75)
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Natsukaze Relay").font(.showTitle(size: 17, relativeTo: .body)) // i18n-skip
                        Text("Episode 9 in 1h 30m").font(.ui(15)) // i18n-skip
                    }
                    Spacer(minLength: 0)
                }
                .padding(.vertical, 8)
            }
        }
        .redacted(reason: .placeholder)
        .foregroundStyle(.haze)
        .padding(.top, 8)
        .accessibilityHidden(true)
    }
}

/// A broadcast in the iPhone's calendar: its time, its cover, the title and what it is.
struct PhoneBroadcastRow: View {
    @Environment(Navigator.self) private var navigator
    let broadcast: Broadcast
    let now: Date

    var body: some View {
        let source = TitleSource(broadcast.entry.id, .calendar, art: .poster)
        Button {
            navigator.openTitle(broadcast.entry.id, from: source)
        } label: {
            HStack(spacing: 14) {
                Text(broadcast.clock)
                    .font(.ui(15))
                    .monospacedDigit()
                    .foregroundStyle(.haze)
                    // One line, as wide as it needs: no larger than `.xxxLarge`.
                    .dynamicTypeSize(...DynamicTypeSize.xxxLarge)
                    .fixedSize()
                    .frame(minWidth: 52, alignment: .leading)
                ArtworkView(url: broadcast.entry.coverUrl, color: broadcast.entry.plateColor, maxPixelSize: 180)
                    .frame(width: 50, height: 75)
                    .clipShape(.rect(cornerRadius: 6))
                    .opacity(broadcast.watched ? 0.6 : 1)
                    .titleSource(source)
                VStack(alignment: .leading, spacing: 2) {
                    Text(broadcast.entry.showTitle)
                        .font(.showTitle(size: 17, relativeTo: .body))
                        .foregroundStyle(broadcast.watched ? .haze : .silk)
                        .rowTitle()
                    HStack(spacing: 6) {
                        if broadcast.isNew(now: now) {
                            Circle().fill(.water).frame(width: 6, height: 6)
                        }
                        Text(broadcast.watched ? broadcast.episodeLong : broadcast.line(now: now))
                    }
                    .font(.ui(15))
                    .foregroundStyle(.haze)
                    if !broadcast.watched, let note = broadcast.delayNote {
                        Text(note)
                            .font(.ui(13))
                            .monospacedDigit()
                            .foregroundStyle(.haze)
                    }
                }
                Spacer(minLength: 0)
                if broadcast.watched {
                    Image(systemName: "checkmark")
                        .font(.ui(15, weight: .semibold))
                        .foregroundStyle(.haze)
                        .accessibilityLabel(L.episodeWatched)
                }
            }
            .padding(.vertical, 8)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .pointerHover()
        .titleElement(broadcast.entry, value: broadcast.spoken(now: now)) { TitleActions(entry: broadcast.entry) }
        .contextMenu { TitleMenu(entry: broadcast.entry) }
    }
}

// MARK: - Library

/// The Library on iPhone and iPad: Downloads, the lists as chips with their counts, the covers;
/// or, signed out, where the lists come from.
struct PhoneLibrary: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.horizontalSizeClass) private var sizeClass

    var body: some View {
        @Bindable var navigator = navigator
        Group {
            if store.accounts.summary.state == .signedOut {
                ScrollView {
                    VStack(spacing: 0) {
                        DownloadsRow().padding(.top, 8)
                        SignedOutLibrary()
                            .frame(minHeight: 560)
                    }
                    .padding(.horizontal, 16)
                }
            } else {
                ScrollView {
                    VStack(alignment: .leading, spacing: 16) {
                        DownloadsRow().padding(.top, 8)
                        chips
                            .padding(.horizontal, -16)
                        LibraryScreen(embedded: true)
                    }
                    .padding(.horizontal, 16)
                    .padding(.bottom, 24)
                }
            }
        }
        .pullToRefresh(store, .library(navigator.librarySegment))
        .screenSubtitle(sizeClass == .compact ? store.syncSubtitle() : "")
        .toolbar { if sizeClass == .compact { AccountButton() } }
        .task {
            for segment in LibraryModel.order where store.library.segment(segment).state == .idle {
                store.loadLibrary(segment)
            }
        }
    }

    /// The lists as chips with their counts.
    private var chips: some View {
        ScrollView(.horizontal) {
            HStack(spacing: 8) {
                ForEach(LibraryModel.order, id: \.self) { segment in
                    let count = store.library.segment(segment).state.ids.count
                    let selected = navigator.librarySegment == segment
                    Button {
                        withAnimation(.quick) { navigator.librarySegment = segment }
                    } label: {
                        HStack(spacing: 6) {
                            Text(segment.title)
                            Text("\(count)").foregroundStyle(selected ? .amber : .haze)
                        }
                        .font(.ui(17))
                        .foregroundStyle(selected ? .amber : .silk)
                        .padding(.horizontal, 16)
                        .frame(height: 40)
                        .background(selected ? Palette.selection : .clear, in: .capsule)
                        .overlay(Capsule().strokeBorder(selected ? .clear : Palette.edge))
                        .contentShape(.capsule)
                    }
                    .buttonStyle(.plain)
                    .pointerHover()
                }
            }
            .padding(.horizontal, 16)
        }
        .scrollIndicators(.never)
    }
}

/// The Downloads row at the top of the Library, with how many there are: Downloads in the
/// sidebar of a wide iPad, pushed over the Library on iPhone and in a narrow window.
struct DownloadsRow: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.horizontalSizeClass) private var sizeClass

    var body: some View {
        let count = store.downloads.rows.count
        Button {
            navigator.showDownloads(compact: sizeClass == .compact)
        } label: {
            HStack(spacing: 12) {
                Image(systemName: "arrow.down.to.line").font(.ui(17)).accessibilityHidden(true)
                Text(L.downloadsTitle).font(.ui(17))
                Spacer()
                if count > 0 {
                    Text("\(count)").font(.ui(17)).monospacedDigit().foregroundStyle(.haze)
                }
                Image(systemName: "chevron.right").font(.ui(13, weight: .semibold)).foregroundStyle(.haze)
            }
            .foregroundStyle(.silk)
            .padding(.horizontal, 16)
            .frame(minHeight: 52)
            .background(.raised, in: .rect(cornerRadius: 20))
            .contentShape(.rect(cornerRadius: 20))
        }
        .buttonStyle(.plain)
        .pointerHover()
        .accessibilityValue(count > 0 ? L.episodesCount(count: count) : "")
        .task { store.loadDownloads() }
    }
}

// MARK: - Search

/// Search on iPhone and iPad: recent searches, this season and the genres before anything is
/// typed, the titles found after, with the search field the tab bar gives it.
struct PhoneSearch: View {
    var body: some View {
        SearchScreen(compact: true)
    }
}
#endif
