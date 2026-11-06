// Reading's settings: the Reading pane (reader defaults per format, when a chapter counts as
// read, which series settings follow you), the Reading group in Downloads, and New chapters in
// Notifications, on the Mac and iPad panes and the iPhone's pages. E-readers have their own group
// in a later change; the pane leaves the room under the others.

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

    private static func choice(_ view: ReaderSettingsView, _ key: ReaderSettingKey) -> String? {
        guard case .choice(let name)? = view.rows.first(where: { $0.key == key })?.value else { return nil }
        return name
    }
}

// MARK: - Mac and iPad

/// The Reading pane: reader defaults, then what follows you.
struct ReadingPane: View {
    @Environment(AppStore.self) private var store

    var body: some View {
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
            SettingsRow(title: L.settingsReaderWebtoons, note: L.settingsReaderWebtoonsNote) {
                value(L.settingsReaderStrip)
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
            SettingsRow(title: L.settingsCountsRead, note: L.settingsCountsReadNote) {
                value(L.settingsCountsReadValue)
            }
        }
        SettingsGroup(title: L.settingsFollowGroup, footnote: L.settingsFollowFooter(device: Device.name)) {
            SettingsRow(title: L.settingsFollowMode, note: L.settingsFollowModeNote) {
                SettingSwitch(key: .seriesChoicesFollow)
            }
        }
        .task { store.reading.loadReaderDefaults() }
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

    private func value(_ text: String) -> some View {
        Text(text)
            .font(.mac(13))
            .foregroundStyle(.haze)
            .padding(.trailing, 6)
    }
}

/// Downloads' Reading group: over cellular on touch, keeping chapters ready, deleting after
/// reading.
struct ReadingDownloadsGroup: View {
    var body: some View {
        SettingsGroup(title: L.settingsGroupReading) {
            #if os(iOS)
            SettingsRow(title: SettingKey.chapterDownloadsOnCellular.title) {
                SettingSwitch(key: .chapterDownloadsOnCellular)
            }
            #endif
            SettingsRow(title: SettingKey.keepChaptersReady.title, note: L.settingsKeepChaptersReadyNote) {
                SettingSwitch(key: .keepChaptersReady)
            }
            SettingsRow(title: SettingKey.deleteAfterReading.title, note: L.settingsDeleteAfterReadingNote) {
                SettingSwitch(key: .deleteAfterReading)
            }
        }
    }
}

/// Notifications' New chapters row, with its own system channel.
struct ChapterNoticesGroup: View {
    var body: some View {
        SettingsGroup(title: L.settingsChapterNotifications, footnote: L.settingsChapterNotificationsFooter) {
            SettingsRow(title: SettingKey.chapterNotifications.title, note: L.settingsChapterNotificationsNote) {
                SettingSwitch(key: .chapterNotifications)
            }
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
                HStack {
                    PhoneRowTitle(title: L.settingsReaderWebtoons, note: L.settingsReaderWebtoonsNote)
                    Spacer()
                    Text(L.settingsReaderStrip).font(.ui(17)).foregroundStyle(.haze)
                }
                .listRowBackground(Palette.raised)
                .accessibilityElement(children: .combine)
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
                HStack {
                    PhoneRowTitle(title: L.settingsCountsRead, note: L.settingsCountsReadNote)
                    Spacer()
                    Text(L.settingsCountsReadValue).font(.ui(17)).foregroundStyle(.haze)
                }
                .listRowBackground(Palette.raised)
                .accessibilityElement(children: .combine)
            } header: {
                PhoneSectionHeader(L.settingsReaderDefaults)
            } footer: {
                Text(L.settingsReaderDefaultsFooter)
            }
            Section {
                PhoneSettingSwitch(key: .seriesChoicesFollow, note: L.settingsFollowModeNote, title: L.settingsFollowMode)
            } header: {
                PhoneSectionHeader(L.settingsFollowGroup)
            } footer: {
                Text(L.settingsFollowFooter(device: Device.name))
            }
        }
        .navigationTitle(L.settingsGroupReading)
        .navigationBarTitleDisplayMode(.inline)
        .task { store.reading.loadReaderDefaults() }
    }
}

/// The iPhone Downloads page's Reading section.
/// Headed Files when reading is the only medium, as the shows' settings are when watching is,
/// so the page does not carry two Reading headings.
struct PhoneReadingDownloadsSection: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        Section {
            PhoneSettingSwitch(key: .chapterDownloadsOnCellular)
            PhoneSettingSwitch(key: .keepChaptersReady, note: L.settingsKeepChaptersReadyNote)
            PhoneSettingSwitch(key: .deleteAfterReading, note: L.settingsDeleteAfterReadingNote)
        } header: {
            PhoneSectionHeader(store.medium.both ? L.settingsGroupReading : L.settingsGroupFiles)
        }
    }
}

/// The iPhone Notifications page's New chapters section.
struct PhoneChapterNoticesSection: View {
    var body: some View {
        Section {
            PhoneSettingSwitch(key: .chapterNotifications, note: L.settingsChapterNotificationsNote)
        } header: {
            PhoneSectionHeader(L.settingsChapterNotifications)
        } footer: {
            Text(L.settingsChapterNotificationsFooter)
        }
    }
}
#endif
