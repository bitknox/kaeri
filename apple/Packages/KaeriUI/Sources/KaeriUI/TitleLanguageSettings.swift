// Settings › General › Titles' first row, Title language (MacSettingsTitles,
// PhoneSettingsTitles): AniList's choice, English or Romaji, with what each does said under the
// label and each choice's menu item showing a title the way it would read.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

/// The three choices, as the settings file names them.
enum TitleChoice: String, CaseIterable, Sendable {
    case anilist, english, romaji

    /// As the menu says it.
    var label: String {
        switch self {
        case .anilist: L.settingsTitleTracker(tracker: AccountsModel.name("anilist"))
        case .english: L.settingsTitleEnglish
        case .romaji: L.settingsTitleRomaji
        }
    }
}

/// What the title-language row says.
enum TitleLanguageWords {
    /// The line under the label: "Romaji, from your AniList account" for AniList's choice, or
    /// what English and Romaji each mean.
    static func line(_ choice: TitleChoice, signedIn: Bool, viewer: TitleLanguage?) -> String {
        switch choice {
        case .anilist:
            guard signedIn, let viewer else { return L.settingsTitleLineSignedOut(tracker: AccountsModel.name("anilist")) }
            return L.settingsTitleLineFromAccount(language: name(viewer), tracker: AccountsModel.name("anilist"))
        case .english: return L.settingsTitleLineEnglish
        case .romaji: return L.settingsTitleLineRomaji
        }
    }

    /// The preview column's note under the examples.
    static func note(_ choice: TitleChoice, signedIn: Bool, viewer: TitleLanguage?, examples: [Entry]) -> String {
        switch choice {
        case .anilist:
            let signedOut = L.settingsTitleNoteSignedOut(tracker: AccountsModel.name("anilist"))
            guard signedIn, let viewer else { return L.settingsTitleNoteFollows(tracker: AccountsModel.name("anilist"), then: signedOut) }
            return L.settingsTitleNoteFollowsSet(tracker: AccountsModel.name("anilist"), language: name(viewer), then: signedOut)
        case .english:
            if let kept = examples.first(where: { ($0.titleEnglish ?? "").isEmpty }) {
                return L.settingsTitleNoteKept(title: kept.titleRomaji ?? kept.title, tracker: AccountsModel.name("anilist"))
            }
            return L.settingsTitleNoteEnglish(tracker: AccountsModel.name("anilist"))
        case .romaji:
            return L.settingsTitleNoteRomaji(tracker: AccountsModel.name("anilist"))
        }
    }

    /// A title language as every sentence names it, with its capital: "English", whatever case
    /// AniList's own word for it is in.
    static func name(_ language: TitleLanguage) -> String {
        switch language {
        case .romaji: L.settingsTitleRomaji
        case .english: L.settingsTitleEnglish
        case .native: L.settingsTitleNative
        }
    }

    /// `entry`'s title as `choice` would name it, the way the engine picks: the form asked for,
    /// else English, else romaji, else native. AniList's choice follows the signed-in account,
    /// and English while nobody is.
    static func example(_ entry: Entry, _ choice: TitleChoice, signedIn: Bool, viewer: TitleLanguage?) -> String {
        let language: TitleLanguage = switch choice {
        case .anilist: signedIn ? (viewer ?? .english) : .english
        case .english: .english
        case .romaji: .romaji
        }
        let chosen = switch language {
        case .romaji: entry.titleRomaji
        case .english: entry.titleEnglish
        case .native: entry.titleNative
        }
        return [chosen, entry.titleEnglish, entry.titleRomaji, entry.titleNative]
            .compactMap { $0 }
            .first { !$0.isEmpty } ?? entry.title
    }
}

extension AppStore {
    /// Whether AniList is signed in, for the title-language row's words.
    var anilistSignedIn: Bool {
        accounts.summary.state != .signedOut
    }

    /// The choice in effect, AniList's until the settings are here.
    var titleChoice: TitleChoice {
        if case .str(let name)? = settings.row(.titles)?.option?.value, let choice = TitleChoice(rawValue: name) {
            return choice
        }
        return .anilist
    }

    /// Choose how every title is named.
    func choose(_ choice: TitleChoice) {
        guard choice != titleChoice else { return }
        setSetting(.titles, to: .str(choice.rawValue))
    }

    /// Three titles to show the choice on: Continue's, else the Library's.
    var titleLanguageExamples: [Entry] {
        let watching = continuing.queue(titles).all.map(\.entry)
        let listed = LibraryModel.order.flatMap { library.segment($0).state.ids }.compactMap { titles[$0] }
        return Array((watching + listed).filter { $0.coverUrl != nil }.reduce(into: [Entry]()) { kept, entry in
            if !kept.contains(where: { $0.id == entry.id }) { kept.append(entry) }
        }.prefix(3))
    }

    /// The first example as `choice` would name it, for a menu item's second line.
    func titleExample(_ choice: TitleChoice) -> String? {
        titleLanguageExamples.first.map {
            TitleLanguageWords.example($0, choice, signedIn: anilistSignedIn, viewer: accounts.viewerTitleLanguage)
        }
    }
}

/// The menu's items: each choice ticked when in effect, with a title as it would read under it.
private struct TitleChoiceItems: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let current = store.titleChoice
        ForEach(TitleChoice.allCases, id: \.self) { choice in
            Toggle(isOn: Binding(get: { current == choice }, set: { if $0 { store.choose(choice) } })) {
                Text(choice.label)
                if let example = store.titleExample(choice) { Text(example) }
            }
        }
    }
}

/// The Mac's and iPad's row: a popup and the line under the label.
struct TitleLanguageRow: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let choice = store.titleChoice
        SettingsRow(
            title: SettingKey.titles.title,
            note: TitleLanguageWords.line(choice, signedIn: store.anilistSignedIn, viewer: store.accounts.viewerTitleLanguage),
            // Selected: the column beside the pane is about it.
            selected: true
        ) {
            Menu {
                TitleChoiceItems()
            } label: {
                MenuCapsule(text: choice.label)
            }
            .menuStyle(.button)
            .buttonStyle(.plain)
            .menuIndicator(.hidden)
            .fixedSize()
            .disabled(store.settings.row(.titles) == nil)
            .accessibilityLabel(SettingKey.titles.title)
        }
        .task { store.send(.loadTitleLanguage) }
    }
}

/// The column beside the General pane: three covers with their titles as the
/// current choice names them, and what the choice does. The covers are 88 points wide, narrower
/// when the column is, so the three always fit.
struct TitleLanguagePreview: View {
    @Environment(AppStore.self) private var store
    /// The column's width inside its padding.
    @State private var width: CGFloat = 288

    var body: some View {
        let choice = store.titleChoice
        let examples = store.titleLanguageExamples
        let viewer = store.accounts.viewerTitleLanguage
        let cover = min(88, ((width - 24) / 3).rounded(.down))
        VStack(alignment: .leading, spacing: 12) {
            Text(L.settingsTitleLanguage).groupHeaderStyle().macLineHeight(14)
                .accessibilityAddTraits(.isHeader)
            HStack(alignment: .top, spacing: 12) {
                ForEach(examples, id: \.id) { entry in
                    VStack(alignment: .leading, spacing: 6) {
                        CoverArt(entry: entry, width: cover)
                        Text(TitleLanguageWords.example(entry, choice, signedIn: store.anilistSignedIn, viewer: viewer))
                            .font(.macShowTitle(size: 12))
                            .foregroundStyle(.silk)
                            .lineLimit(2)
                    }
                    .frame(width: cover, alignment: .leading)
                }
            }
            Text(TitleLanguageWords.note(choice, signedIn: store.anilistSignedIn, viewer: viewer, examples: examples))
                .font(.mac(13))
                .macLineHeight(18)
                .foregroundStyle(.silk)
                .fixedSize(horizontal: false, vertical: true)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { width = $0 }
        // The examples are what Continue lists, asked for when Settings opens first.
        .task { store.loadContinue() }
    }
}

#if os(iOS)
/// The iPhone's row: a menu whose items each carry the example title under their label, in place
/// of a preview column.
struct PhoneTitleLanguageRow: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let choice = store.titleChoice
        Menu {
            TitleChoiceItems()
        } label: {
            HStack {
                PhoneRowTitle(
                    title: SettingKey.titles.title,
                    note: TitleLanguageWords.line(choice, signedIn: store.anilistSignedIn, viewer: store.accounts.viewerTitleLanguage)
                )
                Spacer()
                HStack(spacing: 6) {
                    Text(choice.label)
                    Image(systemName: "chevron.up.chevron.down").font(.ui(12, weight: .semibold))
                }
                .font(.ui(17))
                .foregroundStyle(.haze)
            }
            // A menu's label centres its lines; the row's title and note sit flush left.
            .multilineTextAlignment(.leading)
        }
        .disabled(store.settings.row(.titles) == nil)
        .listRowBackground(Palette.raised)
        .task {
            store.send(.loadTitleLanguage)
            store.loadContinue()
        }
    }
}
#endif

// MARK: - App language

/// What the app-language row says on the Mac, iPad and iPhone.
enum AppLanguageWords {
    /// A language as the menu names it, a draft one labelled: "Deutsch · Draft translation".
    static func name(_ language: L.Language) -> String {
        language.draft ? "\(language.name) · \(L.languageDraftLabel)" : language.name // i18n-skip: two strings
    }

    /// The row's value: the chosen language, or following the system's.
    static func value(_ chosen: L.Language?, labelDraft: Bool = true) -> String {
        guard let chosen else { return L.languageFollowSystem }
        return labelDraft ? name(chosen) : chosen.name
    }

    /// The line under the row: the draft note when the text is in a draft language, and how the
    /// new language takes effect when it differs from the one showing.
    static func note(next: L.Language, restart: Bool) -> String? {
        var lines: [String] = []
        if next.draft { lines.append(L.languageDraftNote) }
        if restart { lines.append(restartNote) }
        return lines.isEmpty ? nil : lines.joined(separator: "\n")
    }

    /// The Mac restarts from the row; iPhone and iPad can't restart an app, so it waits for the next open.
    static var restartNote: String {
        #if os(macOS)
        L.languageRestartNote
        #else
        L.languageNextOpenNote
        #endif
    }
}

/// The language list: follow the system's, then each shipped language with its flag. A pick is
/// stored at once, as the system's own per-app setting stores it.
struct AppLanguageMenu<Label: View>: View {
    @Binding var chosen: L.Language?
    var language = AppLanguage()
    @ViewBuilder let label: Label

    var body: some View {
        Menu {
            Picker(L.settingsLanguageApp, selection: Binding(get: { chosen?.tag }, set: pick)) {
                Text(L.languageFollowSystem).tag(String?.none)
                Divider()
                ForEach(AppLanguage.shipped, id: \.tag) { shipped in
                    SwiftUI.Label {
                        Text(AppLanguageWords.name(shipped))
                    } icon: {
                        if let flag = shipped.flag, let icon = LanguageFlag.menuIcon(flag) { icon }
                    }
                    .tag(Optional(shipped.tag))
                }
            }
            .pickerStyle(.inline)
            .labelsHidden()
        } label: {
            label
        }
        .accessibilityLabel(L.settingsLanguageApp)
        .accessibilityValue(AppLanguageWords.value(chosen))
    }

    /// Picks the language a menu item names, or `nil` to follow the system's.
    func pick(_ tag: String?) {
        let picked = AppLanguage.shipped.first { $0.tag == tag }
        chosen = picked
        language.choose(picked)
    }
}

/// The language of the app's own text on the Mac and iPad. Apple reads it when the app starts, so
/// a new pick says so: the Mac offers Restart beside the menu, the iPad says it applies the next
/// time the app opens.
struct AppLanguageRow: View {
    @State private var chosen = AppLanguage().chosen

    var body: some View {
        let language = AppLanguage()
        let restart = language.needsRestart()
        SettingsRow(title: L.settingsLanguageApp, note: AppLanguageWords.note(next: language.next, restart: restart)) {
            HStack(spacing: 10) {
                #if os(macOS)
                if restart {
                    Button(L.languageRestart) { AppRelaunch.now() }
                }
                #endif
                AppLanguageMenu(chosen: $chosen) {
                    MenuCapsule(text: AppLanguageWords.value(chosen), flag: chosen?.flag)
                }
                .menuStyle(.button)
                .buttonStyle(.plain)
                .menuIndicator(.hidden)
                .fixedSize()
            }
        }
    }
}

#if os(iOS)
/// The iPhone's row: the whole row opens the language list, its value the plain name, the draft
/// label left to the note beneath.
struct PhoneAppLanguageRow: View {
    @State private var chosen = AppLanguage().chosen

    var body: some View {
        let language = AppLanguage()
        AppLanguageMenu(chosen: $chosen) {
            HStack {
                PhoneRowTitle(
                    title: L.settingsLanguageApp,
                    note: AppLanguageWords.note(next: language.next, restart: language.needsRestart())
                )
                Spacer()
                HStack(spacing: 8) {
                    if let flag = chosen?.flag { LanguageFlag(code: flag) }
                    PhoneMenuValue(text: AppLanguageWords.value(chosen, labelDraft: false))
                }
            }
            // A menu's label centres its lines; the row's title and note sit flush left.
            .multilineTextAlignment(.leading)
        }
        .listRowBackground(Palette.raised)
    }
}
#endif

#if os(macOS)
/// Closes the app and opens it again, for a new language. A quit the app cancels (a watch party
/// asks first) leaves it open, and nothing opens it later.
@MainActor
enum AppRelaunch {
    static func now() {
        let pid = String(ProcessInfo.processInfo.processIdentifier)
        let path = Bundle.main.bundlePath
        let observer = NotificationCenter.default.addObserver(
            forName: NSApplication.willTerminateNotification, object: nil, queue: nil
        ) { _ in
            // Waits for this process to end, for a minute at most, then opens the app again.
            let waiter = Process()
            waiter.executableURL = URL(fileURLWithPath: "/bin/sh")
            waiter.arguments = [
                "-c",
                "i=0; while /bin/kill -0 \"$1\" 2>/dev/null && [ $i -lt 300 ]; do /bin/sleep 0.2; i=$((i+1)); done; /usr/bin/open \"$2\"", // i18n-skip
                "kaeri-relaunch", pid, path, // i18n-skip
            ]
            try? waiter.run()
        }
        NSApp.terminate(nil)
        // Reached only when the quit was cancelled: the app stays, and so does its language.
        NotificationCenter.default.removeObserver(observer)
    }
}
#endif
