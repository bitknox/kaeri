// Reading's settings: the Reading pane (reader defaults per format, when a chapter counts as
// read, which series settings follow you), the Reading group in Downloads, and New chapters in
// Notifications, on the Mac and iPad panes and the iPhone's pages. The E-readers group holds
// Send to e-reader and, on the Mac, the OPDS feed.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

/// The reader defaults as the rows say them: manga in one menu of mode and direction, novels in
/// one of mode.
enum ReaderDefaultWords {
    enum Manga: String, CaseIterable, Identifiable {
        case rtl, ltr, strip
        var id: Self { self }

        var label: String {
            switch self {
            case .rtl: L.settingsReaderRtl
            case .ltr: L.settingsReaderLtr
            case .strip: L.settingsReaderStrip
            }
        }
    }

    enum Novel: String, CaseIterable, Identifiable {
        case paged, scrolled
        var id: Self { self }

        var label: String {
            switch self {
            case .paged: L.settingsReaderPages
            case .scrolled: L.settingsReaderScrolled
            }
        }
    }

    @MainActor
    static func manga(_ store: AppStore) -> Manga? {
        guard let view = store.reading.readerDefaults[.manga] else { return nil }
        let mode = choice(view, .mode)
        if mode == "strip" { return .strip } // i18n-skip
        return choice(view, .direction) == "ltr" ? .ltr : .rtl // i18n-skip
    }

    @MainActor
    static func novel(_ store: AppStore) -> Novel? {
        guard let view = store.reading.readerDefaults[.novel] else { return nil }
        return choice(view, .mode) == "scrolled" ? .scrolled : .paged // i18n-skip
    }

    @MainActor
    static func setManga(_ value: Manga, _ store: AppStore) {
        switch value {
        case .strip:
            store.reading.setReaderDefault(.manga, .mode, .choice(name: "strip")) // i18n-skip
        case .rtl, .ltr:
            store.reading.setReaderDefault(.manga, .mode, .choice(name: "paged")) // i18n-skip
            store.reading.setReaderDefault(.manga, .direction, .choice(name: value.rawValue))
        }
    }

    @MainActor
    static func setNovel(_ value: Novel, _ store: AppStore) {
        store.reading.setReaderDefault(.novel, .mode, .choice(name: value.rawValue))
    }

    /// Skip credit pages, on unless turned off. Nil until the manga defaults are here.
    @MainActor
    static func skipsCredits(_ store: AppStore) -> Bool? {
        guard let view = store.reading.readerDefaults[.manga] else { return nil }
        guard case .toggle(let on)? = view.rows.first(where: { $0.key == .skipCreditPages })?.value else { return true }
        return on
    }

    @MainActor
    static func setSkipsCredits(_ on: Bool, _ store: AppStore) {
        store.reading.setReaderDefault(.manga, .skipCreditPages, .toggle(on: on))
    }

    @MainActor
    static func skipsCreditsBinding(_ store: AppStore) -> Binding<Bool> {
        Binding(get: { skipsCredits(store) ?? true }, set: { setSkipsCredits($0, store) })
    }

    private static func choice(_ view: ReaderSettingsView, _ key: ReaderSettingKey) -> String? {
        guard case .choice(let name)? = view.rows.first(where: { $0.key == key })?.value else { return nil }
        return name
    }
}

// MARK: - Mac and iPad

/// The Reading pane: reader defaults, then what follows you.
struct ReadingPane: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    /// The Mac's OPDS feed, opened from the E-readers group.
    @State private var feedOpen = false

    var body: some View {
        #if os(macOS)
        Group {
            if feedOpen {
                FeedPane { withAnimation(.standard) { feedOpen = false } }
                    .transition(.opacity)
            } else {
                groups
                    .transition(.opacity)
            }
        }
        .animation(.standard, value: feedOpen)
        #else
        groups
        #endif
    }

    @ViewBuilder private var groups: some View {
        SettingsGroup(title: L.settingsReaderDefaults, footnote: L.settingsReaderDefaultsFooter) {
            SettingsRow(title: L.settingsReaderManga) {
                menu(current: ReaderDefaultWords.manga(store)?.label, label: L.settingsReaderManga) {
                    Picker(L.settingsReaderManga, selection: Binding(
                        get: { ReaderDefaultWords.manga(store) },
                        set: { if let value = $0 { ReaderDefaultWords.setManga(value, store) } }
                    )) {
                        ForEach(ReaderDefaultWords.Manga.allCases) { Text($0.label).tag(Optional($0)) }
                    }
                }
            }
            SettingsRow(title: L.settingsReaderWebtoons) {
                SettingMenu(key: .webtoonMode)
            }
            SettingsRow(title: L.settingsReaderNovels) {
                menu(current: ReaderDefaultWords.novel(store)?.label, label: L.settingsReaderNovels) {
                    Picker(L.settingsReaderNovels, selection: Binding(
                        get: { ReaderDefaultWords.novel(store) },
                        set: { if let value = $0 { ReaderDefaultWords.setNovel(value, store) } }
                    )) {
                        ForEach(ReaderDefaultWords.Novel.allCases) { Text($0.label).tag(Optional($0)) }
                    }
                }
            }
            SettingsRow(title: SettingKey.countsAsRead.title, note: CountsAsReadNote.text(store)) {
                SettingMenu(key: .countsAsRead)
            }
            // Pages a group repeats are skipped unless this is off; amber when on, as the app's switches are.
            SettingsRow(title: L.settingsSkipCredits, note: L.settingsSkipCreditsNote) {
                Toggle(L.settingsSkipCredits, isOn: ReaderDefaultWords.skipsCreditsBinding(store))
                    .toggleStyle(.switch)
                    .tint(.amber)
                    .labelsHidden()
                    .disabled(ReaderDefaultWords.skipsCredits(store) == nil)
            }
        }
        SettingsGroup(title: L.settingsFollowGroup, footnote: L.settingsFollowFooter(device: Device.name)) {
            SettingsRow(title: L.settingsModeFollows) {
                SettingSwitch(key: .seriesChoicesFollow)
            }
            SettingsRow(title: L.settingsLookFollows, note: L.settingsLookFollowsNote) {
                SettingSwitch(key: .lookFollows)
            }
        }
        SettingsGroup(title: L.ereadersGroup) {
            SettingsRow(title: L.ereadersSendRow) {
                SendTargetMenu()
            }
            #if os(macOS)
            SettingsRow(title: L.ereadersFeedRow) {
                Button {
                    withAnimation(.standard) { feedOpen = true }
                } label: {
                    HStack(spacing: 6) {
                        Text(store.reading.sending.feed?.on == true ? L.readerOn : L.readerOff)
                            .font(.mac(13))
                        Image(systemName: "chevron.right").font(.mac(10, weight: .semibold))
                    }
                    .foregroundStyle(.haze)
                    .contentShape(.rect)
                }
                .buttonStyle(.plain)
                .pointerHover()
                .accessibilityLabel(L.ereadersFeedRow)
            }
            #endif
        }
        .task {
            store.reading.loadReaderDefaults()
            store.reading.sending.loadTargets()
            #if os(macOS)
            store.reading.sending.loadFeed()
            #endif
        }
        #if os(macOS)
        .onLaunchSheet("feed", navigator) { feedOpen = true }
        #endif
    }

    private func menu<Content: View>(current: String?, label: String, @ViewBuilder _ content: () -> Content) -> some View {
        Menu {
            content().pickerStyle(.inline).labelsHidden()
        } label: {
            MenuCapsule(text: current ?? "–") // i18n-skip
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .menuIndicator(.hidden)
        .fixedSize()
        .disabled(current == nil)
        .accessibilityLabel(label)
    }
}

/// Downloads' Reading group: over cellular on touch, which new chapters download, deleting
/// after reading, and whether chapter lists show volumes.
struct ReadingDownloadsGroup: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        SettingsGroup(title: L.settingsGroupReading) {
            #if os(iOS)
            SettingsRow(title: SettingKey.chapterDownloadsOnCellular.title) {
                SettingSwitch(key: .chapterDownloadsOnCellular)
            }
            #endif
            SettingsRow(title: SettingKey.newChapters.title, note: NewChaptersNote.text(store)) {
                SettingMenu(key: .newChapters)
            }
            SettingsRow(title: SettingKey.deleteAfterReading.title, note: L.settingsDeleteAfterReadingNote) {
                SettingSwitch(key: .deleteAfterReading)
            }
            SettingsRow(title: SettingKey.showVolumes.title, note: L.settingsShowVolumesNote) {
                SettingSwitch(key: .showVolumes)
            }
        }
    }
}

/// Notifications' New chapters row.
struct ChapterNoticesRow: View {
    var body: some View {
        SettingsRow(title: SettingKey.chapterNotifications.title, note: L.settingsChapterNotificationsNote) {
            SettingSwitch(key: .chapterNotifications)
        }
    }
}

/// Notifications' Downloads finished row.
struct DownloadsFinishedRow: View {
    var body: some View {
        SettingsRow(title: SettingKey.downloadsFinished.title, note: L.settingsDownloadsFinishedNote) {
            DownloadsFinishedToggle { Text(SettingKey.downloadsFinished.title) }
                .toggleStyle(.switch)
                .labelsHidden()
        }
    }
}

// MARK: - iPhone

#if os(iOS)
/// The iPhone's Reading page: the same groups as rows.
struct PhoneReadingPage: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        PhoneList {
            Section {
                Picker(selection: Binding(
                    get: { ReaderDefaultWords.manga(store) },
                    set: { if let value = $0 { ReaderDefaultWords.setManga(value, store) } }
                )) {
                    ForEach(ReaderDefaultWords.Manga.allCases) { Text($0.label).tag(Optional($0)) }
                } label: {
                    Text(L.settingsReaderManga).font(.ui(17)).foregroundStyle(.silk)
                }
                .pickerStyle(.navigationLink)
                .listRowBackground(Palette.raised)
                PhoneSettingPicker(key: .webtoonMode)
                Picker(selection: Binding(
                    get: { ReaderDefaultWords.novel(store) },
                    set: { if let value = $0 { ReaderDefaultWords.setNovel(value, store) } }
                )) {
                    ForEach(ReaderDefaultWords.Novel.allCases) { Text($0.label).tag(Optional($0)) }
                } label: {
                    Text(L.settingsReaderNovels).font(.ui(17)).foregroundStyle(.silk)
                }
                .pickerStyle(.navigationLink)
                .listRowBackground(Palette.raised)
                PhoneSettingPicker(key: .countsAsRead, note: CountsAsReadNote.text(store))
                    .listRowBackground(Palette.raised)
                Toggle(isOn: ReaderDefaultWords.skipsCreditsBinding(store)) {
                    PhoneRowTitle(title: L.settingsSkipCredits, note: L.settingsSkipCreditsNote, status: nil)
                }
                .tint(.amber)
                .disabled(ReaderDefaultWords.skipsCredits(store) == nil)
                .listRowBackground(Palette.raised)
            } header: {
                PhoneSectionHeader(L.settingsReaderDefaults)
            } footer: {
                Text(L.settingsReaderDefaultsFooter)
            }
            Section {
                PhoneSettingSwitch(key: .seriesChoicesFollow)
                PhoneSettingSwitch(key: .lookFollows, note: L.settingsLookFollowsNote)
            } header: {
                PhoneSectionHeader(L.settingsFollowGroup)
            } footer: {
                Text(L.settingsFollowFooter(device: Device.name))
            }
            Section {
                let sending = store.reading.sending
                Picker(selection: Binding(
                    get: { sending.defaultTarget?.id },
                    set: { if let id = $0 { sending.choose(target: id) } }
                )) {
                    ForEach(sending.targets, id: \.id) { Text($0.name).tag(Optional($0.id)) }
                } label: {
                    Text(L.ereadersSendRow).font(.ui(17)).foregroundStyle(.silk)
                }
                .pickerStyle(.navigationLink)
                .listRowBackground(Palette.raised)
                .disabled(sending.targets.isEmpty)
            } header: {
                PhoneSectionHeader(L.ereadersGroup)
            }
        }
        .navigationTitle(L.settingsGroupReading)
        .navigationBarTitleDisplayMode(.inline)
        .task {
            store.reading.loadReaderDefaults()
            store.reading.sending.loadTargets()
        }
    }
}

/// The iPhone Downloads page's Reading settings. Right under the Reading downloads, which carry
/// the heading, so the page has one Reading heading; with none downloaded, headed Reading with
/// both media on and Files with reading alone, as the shows' settings are.
struct PhoneReadingDownloadsSection: View {
    @Environment(AppStore.self) private var store
    var headed = true

    var body: some View {
        if headed {
            Section { rows } header: {
                PhoneSectionHeader(store.medium.both ? L.settingsGroupReading : L.settingsGroupFiles)
            }
        } else {
            Section { rows }
        }
    }

    @ViewBuilder
    private var rows: some View {
        PhoneSettingSwitch(key: .chapterDownloadsOnCellular)
        PhoneSettingPicker(key: .newChapters, note: NewChaptersNote.text(store))
        PhoneSettingSwitch(key: .deleteAfterReading, note: L.settingsDeleteAfterReadingNote)
        PhoneSettingSwitch(key: .showVolumes, note: L.settingsShowVolumesNote)
    }
}
#endif

/// Counts as read's note says what the chosen rule does.
/// New chapters' line, only under the choice it describes: Off needs none.
enum NewChaptersNote {
    @MainActor
    static func text(_ store: AppStore) -> String? {
        store.settings.row(.newChapters)?.value == "reading" ? L.settingsNewChaptersNote : nil // i18n-skip: the engine's words
    }
}

enum CountsAsReadNote {
    @MainActor
    static func text(_ store: AppStore) -> String {
        switch store.settings.row(.countsAsRead)?.value {
        case "moving-on": L.settingsCountsReadMovingOnNote // i18n-skip: the engine's words
        case "marked": L.settingsCountsReadMarkedNote // i18n-skip: the engine's words
        default: L.settingsCountsReadNote
        }
    }
}
