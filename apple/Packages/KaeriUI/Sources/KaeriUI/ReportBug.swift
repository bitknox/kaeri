// "Report a bug": every entry point opens one sheet. It says what a report holds, lets the viewer
// switch the last error and each saved crash off, shows exactly what is sent, and only then opens
// the pre-filled GitHub issue in the browser, where the reporter signs in and submits. Nothing
// leaves the device before that. Crash reports themselves are off until Settings, About turns
// them on (CrashReportsGroup). The player log is never sent: ticked, it is saved as a file for
// the reporter to attach to the issue by hand.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI
import UniformTypeIdentifiers
#if os(macOS)
import AppKit
#endif

/// What opens the sheet: an error to start from, and the crash the launch notice named.
struct ReportRequest: Identifiable, Equatable {
    let id = UUID()
    var context: BugContext?
    /// The crash ticked when the sheet opens: the one the notice named.
    var crash: String?
    /// A shot's: What is sent open on the Player log segment.
    var showLog = false
}

/// Opens the bug report sheet for this device, with an error when the report starts from one.
@MainActor
struct ReportBugAction {
    let store: AppStore
    let navigator: Navigator

    func callAsFunction(_ context: BugContext? = nil) {
        navigator.report = ReportRequest(context: context ?? lastProblem(playback: true))
    }

    /// From Settings, where Player log starts unticked: the newest problem told of, but never a
    /// failed playback.
    func fromSettings() {
        navigator.report = ReportRequest(context: lastProblem(playback: false))
    }

    /// The newest problem the app has told of this session, for a report started from a menu:
    /// the last playback's failure, until another playback starts, when nothing was told after it.
    func lastProblem(playback: Bool) -> BugContext? {
        let alert = store.notices.log.first { $0.isAlert }
        if playback, let failure = store.playbackFailure, alert.map({ $0.at <= failure.at }) ?? true {
            return failure.context
        }
        return alert.map { BugContext(kind: "message", message: $0.notice.words) }
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
            // Shots of the player log: Report from a playback error, and with the log's text shown.
            .onLaunchSheet("report-error", navigator) { navigator.report = ReportRequest(context: Self.shotError) } // i18n-skip
            .onLaunchSheet("report-log", navigator) { // i18n-skip
                navigator.report = ReportRequest(context: Self.shotError, showLog: true)
            }
    }
}

extension ReportSheetPresenter {
    /// A made-up playback error for the shots.
    static let shotError = BugContext(kind: BugReports.playbackError, message: "Segment 212 failed after 3 tries") // i18n-skip
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
    /// What the player log holds, read when the sheet opens.
    @State private var logStats: PlayerLogStats?
    @State private var includeLog = false
    @State private var logSave = LogSave.idle
    /// The redacted log, made off the main actor once the row is ticked.
    @State private var logText: String?
    @State private var sentTab = SentTab.report
    /// Open on GitHub asked for the file first: the issue opens once it is saved.
    @State private var openAfterSave = false
    @FocusState private var logFocused: Bool
    #if os(iOS)
    @State private var exporting = false
    @State private var exportDocument: LogDocument?
    #endif

    private var saving: Bool { store.settings.isOn(.saveCrashReports) }
    private var recent: [CrashSummary] {
        let week = Date.now.addingTimeInterval(-7 * 86_400)
        return store.crash.saved.filter { Date(timeIntervalSince1970: TimeInterval($0.atMs) / 1000) >= week }
    }
    private var context: BugContext? { includeError ? request.context : nil }
    private var ids: [String] { recent.map(\.id).filter(crashes.contains) }
    private var hasLog: Bool { (logStats?.playbacks ?? 0) > 0 }
    private var savedLog: URL? { if case .saved(let url) = logSave { url } else { nil } }

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
            let stats = BugReports.playerLogStats()
            logStats = stats
            // Opened from a playback error or with a crash in the player: the log helps most.
            let report = store.bugReport(context: request.context, crashes: request.crash.map { [$0] } ?? [])
            if stats.playbacks > 0, BugReports.playerLogStartsTicked(report) {
                includeLog = true
                logFocused = true
                prepareLog()
                if request.showLog {
                    showSent = true
                    sentTab = .log
                }
            }
        }
        #if os(iOS)
        .fileExporter(
            isPresented: $exporting, document: exportDocument, contentType: .plainText,
            defaultFilename: logStats.map { ($0.fileName as NSString).deletingPathExtension }
        ) { result in
            if case .success(let url) = result { saved(url) } else { saveFailed() }
        } onCancellation: {
            openAfterSave = false
        }
        #endif
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
                Rectangle().fill(.faintRule).frame(height: 1)
                playerLogRow
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

    private func line(
        _ title: String, detail: String? = nil, on: Binding<Bool>, fixed: Bool = false, dimmed: Bool = false
    ) -> some View {
        Toggle(isOn: on) {
            VStack(alignment: .leading, spacing: 1) {
                Text(title).font(.plat(mac: 13, touch: 17)).foregroundStyle(.silk)
                if let detail {
                    Text(detail).font(.plat(mac: 12, touch: 13)).foregroundStyle(.haze).lineLimit(2)
                }
            }
            .opacity(dimmed ? 0.45 : 1)
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
                if includeLog {
                    Picker(L.reportShowSent, selection: $sentTab.animation(reduceMotion ? nil : .quick)) {
                        Text(L.reportSentReport).tag(SentTab.report)
                        Text(L.reportPlayerLog).tag(SentTab.log)
                    }
                    .pickerStyle(.segmented)
                    .labelsHidden()
                    .frame(width: Self.touch ? 260 : 200)
                    .transition(.opacity)
                    sentPair.transition(.opacity)
                } else {
                    reportText.transition(.opacity)
                }
            }
        }
    }

    private var reportText: some View {
        Text(store.bugReportText(context: context, crashes: ids, playerLog: includeLog))
            .font(.system(size: 11.5, design: .monospaced))
            .foregroundStyle(.silk)
            .textSelection(.enabled)
            .padding(12)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(.ground, in: .rect(cornerRadius: 8))
    }

    /// The report and the log in one box of a fixed height, crossfading between them, so nothing
    /// below moves when the segment changes. The log's text view lays out only what shows.
    private var sentPair: some View {
        ZStack(alignment: .topLeading) {
            ScrollView {
                Text(store.bugReportText(context: context, crashes: ids, playerLog: true))
                    .font(.system(size: 11.5, design: .monospaced))
                    .foregroundStyle(.silk)
                    .textSelection(.enabled)
                    .padding(12)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            .opacity(sentTab == .report ? 1 : 0)
            .allowsHitTesting(sentTab == .report)
            .accessibilityHidden(sentTab != .report)
            LogTextView(text: logText ?? "")
                .opacity(sentTab == .log && logText != nil ? 1 : 0)
                .allowsHitTesting(sentTab == .log)
                .accessibilityHidden(sentTab != .log)
        }
        .frame(height: Self.touch ? 220 : 196)
        .frame(maxWidth: .infinity)
        .background(.ground, in: .rect(cornerRadius: 8))
        .clipShape(.rect(cornerRadius: 8))
        .animation(.reveal, value: sentTab)
        .animation(.fadeIn, value: logText != nil)
    }

    // MARK: Player log

    /// The last row in Included: how much the log holds, and once ticked, the line that saves it.
    private var playerLogRow: some View {
        VStack(alignment: .leading, spacing: 0) {
            line(
                L.reportPlayerLog,
                detail: hasLog ? L.reportPlayerLogDetail(playbacks: Int(logStats?.playbacks ?? 0), size: logSize) : L.reportPlayerLogEmpty,
                on: Binding(get: { includeLog && hasLog }, set: { tickLog($0) }),
                fixed: !hasLog, dimmed: !hasLog
            )
            .focused($logFocused)
            .accessibilityHint(L.reportPlayerLogA11yHint)
            if includeLog, hasLog {
                logLine
                    .padding(.leading, Self.touch ? 12 : 32)
                    .padding(.trailing, 12)
                    .padding(.bottom, 10)
                    // The row opens on the spring; its words follow 60 ms later.
                    .transition(.asymmetric(
                        insertion: .opacity.animation(.fadeIn.delay(0.06)),
                        removal: .opacity.animation(.fadeOut)))
            }
        }
    }

    private var logSize: String {
        Int64(clamping: logStats?.bytes ?? 0).formatted(.byteCount(style: .file).locale(AirTime.locale))
    }

    private func tickLog(_ on: Bool) {
        withAnimation(reduceMotion ? nil : .standard) {
            includeLog = on
            if !on {
                // A file already saved stays where it is; the row starts over.
                logSave = .idle
                sentTab = .report
                openAfterSave = false
            }
        }
        if on { prepareLog() }
    }

    /// Save log… and, once written, the saved line in its place, over the one sentence on what to
    /// do with the file. Both layers share one height, so the sheet holds still.
    private var logLine: some View {
        VStack(alignment: .leading, spacing: 6) {
            ZStack(alignment: .leading) {
                HStack(spacing: 10) {
                    Button(L.reportPlayerLogSave) { saveLog(thenOpen: false) }
                        .buttonStyle(SecondaryCapsuleStyle(height: Self.touch ? 34 : 22, fontSize: Self.touch ? 15 : 12))
                    if logSave == .failed {
                        Text(L.reportPlayerLogFailed)
                            .font(.plat(mac: 12, touch: 13))
                            .foregroundStyle(.vermilion)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
                .opacity(savedLog == nil ? 1 : 0)
                .allowsHitTesting(savedLog == nil)
                .accessibilityHidden(savedLog != nil)
                if let url = savedLog {
                    savedLine(url).transition(.opacity)
                }
            }
            .frame(minHeight: Self.touch ? 34 : 22)
            .animation(.reveal, value: logSave)
            Text(L.reportPlayerLogAttach)
                .font(.plat(mac: 12, touch: 13))
                .foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    private func savedLine(_ url: URL) -> some View {
        HStack(spacing: 8) {
            Image(systemName: "checkmark")
                .font(.plat(mac: 11, touch: 14, weight: .semibold))
                .foregroundStyle(.water)
                .accessibilityHidden(true)
            Text(L.reportPlayerLogSaved(file: url.lastPathComponent))
                .font(.plat(mac: 12, touch: 13))
                .foregroundStyle(.silk)
                .lineLimit(1)
                .truncationMode(.middle)
                .frame(maxWidth: .infinity, alignment: .leading)
            #if os(macOS)
            Button(L.menuShowInFinder) { NSWorkspace.shared.activateFileViewerSelecting([url]) }
                .buttonStyle(.plain)
                .font(.mac(12))
                .foregroundStyle(.silk)
                .underline()
                .fixedSize()
                .pointerHover()
            #endif
        }
    }

    /// Starts the redaction off the main actor, once, so the preview and the save find it ready.
    private func prepareLog() {
        guard logText == nil else { return }
        let report = store.bugReport(context: request.context, crashes: [])
        Task { logText = await BugReports.playerLogText(report) }
    }

    private func logTextNow() async -> String {
        if let logText { return logText }
        let text = await BugReports.playerLogText(store.bugReport(context: request.context, crashes: []))
        logText = text
        return text
    }

    /// The system's save dialog with the engine's file name: the save panel on the Mac, the Files
    /// save sheet on iPhone and iPad.
    private func saveLog(thenOpen: Bool) {
        openAfterSave = thenOpen
        let name = logStats?.fileName ?? ""
        #if os(macOS)
        let panel = NSSavePanel()
        panel.nameFieldStringValue = name
        panel.allowedContentTypes = [.plainText]
        panel.canCreateDirectories = true
        Task {
            let response: NSApplication.ModalResponse
            if let window = NSApp.keyWindow {
                response = await panel.beginSheetModal(for: window)
            } else {
                response = await panel.begin()
            }
            guard response == .OK, let url = panel.url else {
                openAfterSave = false
                return
            }
            if await BugReports.write(await logTextNow(), to: url) { saved(url) } else { saveFailed() }
        }
        #else
        Task {
            exportDocument = LogDocument(text: await logTextNow())
            exporting = true
        }
        #endif
    }

    private func saved(_ url: URL) {
        logSave = .saved(url)
        var words = AttributedString(L.reportPlayerLogA11ySaved)
        words.accessibilitySpeechAnnouncementPriority = .high
        AccessibilityNotification.Announcement(words).post()
        if openAfterSave {
            openAfterSave = false
            openIssue(attached: true)
        }
    }

    private func saveFailed() {
        logSave = .failed
        openAfterSave = false
    }

    private func openIssue(attached: Bool) {
        openURL(store.bugReportURL(context: context, crashes: ids, playerLog: includeLog && attached))
        dismiss()
    }

    private static var touch: Bool {
        #if os(macOS)
        false
        #else
        true
        #endif
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
            // The report only, never the log.
            Pasteboard.copy(store.bugReportText(context: context, crashes: ids, playerLog: includeLog))
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
            // Ticked and not saved: the file first. Cancel there opens nothing.
            if includeLog, hasLog, savedLog == nil { saveLog(thenOpen: true) } else { openIssue(attached: savedLog != nil) }
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

/// Where saving the player log stands.
enum LogSave: Equatable {
    case idle
    case failed
    case saved(URL)
}

/// The two segments of What is sent while Player log is ticked.
enum SentTab: Hashable {
    case report
    case log
}

/// The player log as the Files save sheet writes it.
struct LogDocument: FileDocument {
    static let readableContentTypes: [UTType] = [.plainText]
    let text: String
    init(text: String) { self.text = text }
    init(configuration: ReadConfiguration) throws { text = "" }
    func fileWrapper(configuration: WriteConfiguration) throws -> FileWrapper {
        FileWrapper(regularFileWithContents: Data(text.utf8))
    }
}

/// The player log in What is sent: a read-only, selectable text view that lays out only the part
/// on screen, since the log can run to a megabyte.
#if os(macOS)
struct LogTextView: NSViewRepresentable {
    let text: String

    func makeNSView(context: Context) -> NSScrollView {
        let scroll = NSTextView.scrollableTextView()
        scroll.drawsBackground = false
        scroll.autohidesScrollers = true
        if let view = scroll.documentView as? NSTextView {
            view.isEditable = false
            view.isSelectable = true
            view.drawsBackground = false
            view.textContainerInset = NSSize(width: 7, height: 12)
            view.font = .monospacedSystemFont(ofSize: 11.5, weight: .regular)
            view.textColor = NSColor(Palette.silk)
        }
        return scroll
    }

    func updateNSView(_ scroll: NSScrollView, context: Context) {
        guard let view = scroll.documentView as? NSTextView, view.string != text else { return }
        view.string = text
        view.scrollToBeginningOfDocument(nil)
    }
}
#else
struct LogTextView: UIViewRepresentable {
    let text: String

    func makeUIView(context: Context) -> UITextView {
        let view = UITextView()
        view.isEditable = false
        view.isSelectable = true
        view.backgroundColor = .clear
        view.textContainerInset = UIEdgeInsets(top: 12, left: 7, bottom: 12, right: 7)
        view.font = .monospacedSystemFont(ofSize: 11.5, weight: .regular)
        view.textColor = UIColor(Palette.silk)
        view.adjustsFontForContentSizeCategory = false
        return view
    }

    func updateUIView(_ view: UITextView, context: Context) {
        guard view.text != text else { return }
        view.text = text
        view.setContentOffset(.zero, animated: false)
    }
}
#endif

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
                Button(L.actionReport) { ReportBugAction(store: store, navigator: navigator).fromSettings() }
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
