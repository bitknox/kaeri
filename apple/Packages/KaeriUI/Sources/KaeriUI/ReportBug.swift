// "Report a bug": every entry point opens one sheet. It says what a report holds, lets the viewer
// switch the last error and each saved crash off, shows exactly what is sent, and only then opens
// the pre-filled GitHub issue in the browser, where the reporter signs in and submits. Nothing
// leaves the device before that. Crash reports themselves are off until Settings, About turns
// them on (CrashReportsGroup).

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

/// What opens the sheet: an error to start from, and the crash the launch notice named.
struct ReportRequest: Identifiable, Equatable {
    let id = UUID()
    var context: BugContext?
    /// The crash ticked when the sheet opens: the one the notice named.
    var crash: String?
}

/// Opens the bug report sheet for this device, with an error when the report starts from one.
@MainActor
struct ReportBugAction {
    let store: AppStore
    let navigator: Navigator

    func callAsFunction(_ context: BugContext? = nil) {
        navigator.report = ReportRequest(context: context ?? lastProblem)
    }

    /// The newest problem the app has told of this session, for a report started from a menu.
    var lastProblem: BugContext? {
        store.notices.log.first { $0.isAlert }.map { BugContext(kind: "message", message: $0.notice.words) }
    }
}

/// A "Report" capsule beside an error: it opens the bug report with that error in it.
struct ReportButton: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let context: BugContext
    var height: CGFloat = 28
    var fontSize: CGFloat = 13
    /// On a raised sheet, where the capsule needs the lighter fill to show.
    var raisedSurface = false

    var body: some View {
        Button(L.actionReport) { ReportBugAction(store: store, navigator: navigator)(context) }
            .buttonStyle(SecondaryCapsuleStyle(height: height, raisedSurface: raisedSurface, fontSize: fontSize))
            .accessibilityLabel(L.aboutReportBug)
            .accessibilityHint(L.aboutReportBugDetail)
    }
}

/// Presents the report sheet over the app, wherever Report a bug was asked for.
struct ReportSheetPresenter: ViewModifier {
    @Environment(Navigator.self) private var navigator

    func body(content: Content) -> some View {
        @Bindable var navigator = navigator
        content
            .sheet(item: $navigator.report) { request in
                ReportSheet(request: request)
            }
            // A shot's `sheet=report`: opened once the window is up.
            .onLaunchSheet("report", navigator) { navigator.report = ReportRequest() } // i18n-skip
    }
}

// MARK: - The sheet

struct ReportSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.openURL) private var openURL
    @Environment(\.dismiss) private var dismiss
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    let request: ReportRequest
    @State private var includeError = true
    @State private var crashes: Set<String> = []
    @State private var showSent = false
    @State private var copied = false

    private var saving: Bool { store.settings.isOn(.saveCrashReports) }
    private var recent: [CrashSummary] {
        let week = Date.now.addingTimeInterval(-7 * 86_400)
        return store.crash.saved.filter { Date(timeIntervalSince1970: TimeInterval($0.atMs) / 1000) >= week }
    }
    private var context: BugContext? { includeError ? request.context : nil }
    private var ids: [String] { recent.map(\.id).filter(crashes.contains) }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            ScrollView {
                VStack(alignment: .leading, spacing: 14) {
                    Text(L.reportTitle)
                        .font(.plat(mac: 15, touch: 22, weight: .semibold))
                        .foregroundStyle(.silk)
                        .accessibilityAddTraits(.isHeader)
                    Text(L.reportDetail)
                        .font(.plat(mac: 13, touch: 15))
                        .foregroundStyle(.haze)
                        .fixedSize(horizontal: false, vertical: true)
                    included
                    sent
                }
                .padding(.bottom, 14)
            }
            .scrollBounceBehavior(.basedOnSize)
            Text(L.reportPrivacy)
                .font(.plat(mac: 12, touch: 13))
                .foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true)
            buttons.padding(.top, 14)
        }
        .padding(24)
        .sheetWidth(520)
        .background(.raised)
        #if os(iOS)
        .presentationDetents([.large])
        #endif
        .onAppear {
            store.crash.refresh()
            if let crash = request.crash { crashes = [crash] }
        }
    }

    private var included: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(L.reportIncluded).groupHeaderStyle()
            VStack(spacing: 0) {
                line(L.reportVersions, on: .constant(true), fixed: true)
                if let message = request.context {
                    Rectangle().fill(.faintRule).frame(height: 1)
                    line(L.reportLastError, detail: message.message.firstClause, on: $includeError)
                }
                if saving {
                    ForEach(recent, id: \.id) { crash in
                        Rectangle().fill(.faintRule).frame(height: 1)
                        line(
                            L.reportCrashItem(when: CrashWords.when(crash), place: CrashWords.place(crash.place)),
                            on: Binding(
                                get: { crashes.contains(crash.id) },
                                set: { on in
                                    if on { crashes.insert(crash.id) } else { crashes.remove(crash.id) }
                                }))
                    }
                }
            }
            .background(.ground, in: .rect(cornerRadius: 10))
            if !saving {
                VStack(alignment: .leading, spacing: 2) {
                    Text(L.reportCrashOff)
                        .font(.plat(mac: 12, touch: 13))
                        .foregroundStyle(.haze)
                        .fixedSize(horizontal: false, vertical: true)
                    Button(L.reportCrashSettings) {
                        dismiss()
                        navigator.openCrashSettings()
                    }
                    .buttonStyle(.plain)
                    .font(.plat(mac: 12, touch: 13))
                    .foregroundStyle(.silk)
                    .underline()
                    .pointerHover()
                }
            }
        }
    }

    private func line(_ title: String, detail: String? = nil, on: Binding<Bool>, fixed: Bool = false) -> some View {
        Toggle(isOn: on) {
            VStack(alignment: .leading, spacing: 1) {
                Text(title).font(.plat(mac: 13, touch: 17)).foregroundStyle(.silk)
                if let detail {
                    Text(detail).font(.plat(mac: 12, touch: 13)).foregroundStyle(.haze).lineLimit(2)
                }
            }
        }
        #if os(macOS)
        .toggleStyle(.checkbox)
        #else
        .tint(.amber)
        #endif
        .disabled(fixed)
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .frame(maxWidth: .infinity, alignment: .leading)
        .animation(.quick, value: on.wrappedValue)
    }

    /// The exact text, in a disclosure: an attachment switched off changes it at once.
    private var sent: some View {
        VStack(alignment: .leading, spacing: 8) {
            Button {
                withAnimation(reduceMotion ? .fadeIn : .standard) { showSent.toggle() }
            } label: {
                HStack(spacing: 6) {
                    Image(systemName: "chevron.right")
                        .font(.plat(mac: 10, touch: 12, weight: .semibold))
                        .rotationEffect(.degrees(showSent ? 90 : 0))
                    Text(showSent ? L.reportHideSent : L.reportShowSent)
                        .font(.plat(mac: 13, touch: 15, weight: .semibold))
                }
                .foregroundStyle(.silk)
                .contentShape(.rect)
            }
            .buttonStyle(.plain)
            if showSent {
                Text(store.bugReportText(context: context, crashes: ids))
                    .font(.system(size: 11.5, design: .monospaced))
                    .foregroundStyle(.silk)
                    .textSelection(.enabled)
                    .padding(12)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(.ground, in: .rect(cornerRadius: 8))
                    .transition(.opacity)
            }
        }
    }

    private var buttons: some View {
        #if os(macOS)
        HStack(spacing: 10) {
            copyButton
            Spacer()
            cancelButton
            openButton
        }
        #else
        // Full width and stacked, so no label is cut short at a phone's width.
        VStack(spacing: 10) {
            openButton.touchFill()
            copyButton.touchFill()
            cancelButton.touchFill()
        }
        #endif
    }

    private var copyButton: some View {
        Button(copied ? L.reportCopied : L.reportCopy) {
            Pasteboard.copy(store.bugReportText(context: context, crashes: ids))
            copied = true
        }
        .buttonStyle(SecondaryCapsuleStyle(height: buttonHeight, raisedSurface: true, fontSize: buttonFont))
    }

    private var cancelButton: some View {
        Button(L.actionCancel) { dismiss() }
            .buttonStyle(SecondaryCapsuleStyle(height: buttonHeight, raisedSurface: true, fontSize: buttonFont))
            .keyboardShortcut(.cancelAction)
    }

    private var openButton: some View {
        Button(L.reportOpenGithub) {
            openURL(store.bugReportURL(context: context, crashes: ids))
            dismiss()
        }
        .buttonStyle(PrimaryCapsuleStyle(height: buttonHeight, fontSize: buttonFont))
        .keyboardShortcut(.defaultAction)
    }

    private var buttonHeight: CGFloat {
        #if os(macOS)
        28
        #else
        44
        #endif
    }

    private var buttonFont: CGFloat {
        #if os(macOS)
        13
        #else
        17
        #endif
    }
}

extension String {
    /// The first clause of an error's message: up to its first colon, full stop or dash.
    var firstClause: String {
        let cut = firstIndex { ":.—–".contains($0) } ?? endIndex
        return String(self[..<cut]).trimmingCharacters(in: .whitespaces)
    }
}

extension Navigator {
    /// Open Settings where Save crash reports is: Integrations on the Mac, Advanced on an iPad,
    /// About on an iPhone.
    func openCrashSettings() {
        #if os(macOS)
        openSettings(.integrations)
        #else
        if UIDevice.current.userInterfaceIdiom == .pad { openSettings(.advanced) } else { phoneSettings = .about }
        #endif
    }
}

/// How a saved crash reads in a row.
enum CrashWords {
    static func when(_ crash: CrashSummary) -> String {
        Date(timeIntervalSince1970: TimeInterval(crash.atMs) / 1000)
            .formatted(.dateTime.day().month(.wide).hour().minute().locale(AirTime.locale))
    }

    static func place(_ place: CrashPlace) -> String {
        switch place {
        case .home: L.crashPlaceHome
        case .player: L.crashPlacePlayer
        case .title: L.crashPlaceTitle
        case .search: L.crashPlaceSearch
        case .settings: L.crashPlaceSettings
        case .other: L.crashPlaceOther
        }
    }
}

// MARK: - Settings, About: crash reports

/// Settings › About on the Mac and iPad: the bug report, the legal line with Legal and contact,
/// and the crash reports.
struct AboutGroup: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.openURL) private var openURL

    var body: some View {
        SettingsGroup(
            title: L.aboutTitle,
            footnote: L.aboutReportBugDetail
        ) {
            SettingsRow(title: L.aboutReportBug) {
                Button(L.actionReport) { ReportBugAction(store: store, navigator: navigator)() }
                    .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
            }
        }
        SettingsGroup(footnote: L.legalHostsNoContent) {
            SettingsRow(title: L.aboutLegalLink) {
                Button(L.settingsReadOnline) { openURL(AppVersion.legal) }
                    .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
            }
        }
        CrashReportsGroup()
    }
}

/// Save crash reports (off), Ask after a crash, what the system kept, and the saved reports.
struct CrashReportsGroup: View {
    @Environment(AppStore.self) private var store
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var viewing: CrashSummary?

    var body: some View {
        let saving = store.settings.isOn(.saveCrashReports)
        SettingsGroup(title: L.settingsCrashSave, footnote: L.settingsCrashFootnote) {
            SettingsRow(title: L.settingsCrashSave, note: L.settingsCrashSaveDetail) {
                SettingSwitch(key: .saveCrashReports)
            }
            if saving {
                SettingsRow(title: L.settingsCrashAsk, note: L.settingsCrashAskDetail) {
                    SettingSwitch(key: .askAfterCrash)
                }
            }
            if saving, !store.crash.held.isEmpty {
                SettingsRow(
                    title: L.settingsCrashOsHeading(system: CrashSystem.name),
                    note: L.settingsCrashOsDetail(system: CrashSystem.name))
                ForEach(store.crash.held, id: \.id) { crash in
                    SettingsRow(title: L.settingsCrashOsItem(when: CrashWords.when(crash))) {
                        Button(L.settingsCrashOsAdd) { store.crash.add(crash) }
                            .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                    }
                }
            }
            if saving || !store.crash.saved.isEmpty {
                SettingsRow(title: L.settingsCrashSaved, note: store.crash.saved.isEmpty ? L.settingsCrashNone : nil) {
                    Text("\(store.crash.saved.count)").font(.mac(13)).monospacedDigit().foregroundStyle(.haze)
                }
                ForEach(store.crash.saved, id: \.id) { crash in
                    SettingsRow(title: L.reportCrashItem(when: CrashWords.when(crash), place: CrashWords.place(crash.place))) {
                        HStack(spacing: 8) {
                            Button(L.settingsCrashView) { viewing = crash }
                                .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                            Button(L.settingsCrashDelete) { store.crash.delete(crash.id) }
                                .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                        }
                    }
                }
                if !store.crash.saved.isEmpty {
                    SettingsRow(title: L.settingsCrashDeleteAll) {
                        Button(L.settingsCrashDeleteAll) { store.crash.deleteAll() }
                            .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                    }
                }
            }
        }
        .animation(reduceMotion ? .fadeIn : .standard, value: saving)
        .onAppear { store.crash.refresh() }
        .onChange(of: saving) { _, on in
            if on { store.crash.offerHeld() } else { store.crash.forgetHeld() }
        }
        .sheet(item: $viewing) { CrashTextSheet(crash: $0) }
    }
}

extension CrashSummary: @retroactive Identifiable {}

/// What the system is called in the Kept by line.
enum CrashSystem {
    static var name: String {
        #if os(macOS)
        "macOS" // i18n-skip: a system's name
        #else
        UIDevice.current.userInterfaceIdiom == .pad ? "iPadOS" : "iOS" // i18n-skip: a system's name
        #endif
    }
}

/// A saved report's text, as Report a bug would send it.
struct CrashTextSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    let crash: CrashSummary

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text(L.reportCrashItem(when: CrashWords.when(crash), place: CrashWords.place(crash.place)))
                .font(.plat(mac: 15, touch: 20, weight: .semibold))
                .foregroundStyle(.silk)
            ScrollView {
                Text(store.crash.text(crash.id))
                    .font(.system(size: 11.5, design: .monospaced))
                    .foregroundStyle(.silk)
                    .textSelection(.enabled)
                    .padding(12)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            .background(.ground, in: .rect(cornerRadius: 8))
            HStack {
                Button(L.reportCopy) { Pasteboard.copy(store.crash.text(crash.id)) }
                    .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                Spacer()
                Button(L.actionDone) { dismiss() }
                    .buttonStyle(PrimaryCapsuleStyle(height: 28))
                    .keyboardShortcut(.defaultAction)
            }
        }
        .padding(24)
        .sheetWidth(520)
        .frame(minHeight: 320)
        .background(.raised)
    }
}

/// The notice after a crash: at launch, a second after the first content is up, once per crash.
/// Nothing is asked while saving is off, because nothing was saved.
struct CrashNoticePresenter: ViewModifier {
    @Environment(AppStore.self) private var store

    func body(content: Content) -> some View {
        content.task {
            // Wait for the engine to run and the settings to be said, then a second for Home.
            while store.settings.view == nil { try? await Task.sleep(for: .milliseconds(200)) }
            try? await Task.sleep(for: .seconds(1))
            guard !Task.isCancelled else { return }
            store.sayCrashNotice()
        }
    }
}
