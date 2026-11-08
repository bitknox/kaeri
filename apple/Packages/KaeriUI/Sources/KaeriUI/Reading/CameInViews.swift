// What came in after a source was added, in words: the shared pieces of the add-server sheet,
// the folder report and the plugin store result. The engine counts; these say it.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

/// A result's glyph, title and line: a silk check for what came in, a haze info mark for what
/// is empty, and the one vermilion glyph for what failed.
struct CameInNote: View {
    enum Kind { case found, info, failed }
    let kind: Kind
    let title: String
    var line: String?
    var size: CGFloat { Device.isMac ? 13 : 17 }

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            switch kind {
            case .found:
                Image(systemName: "checkmark").font(.plat(mac: 11, touch: 13, weight: .semibold)).foregroundStyle(.silk)
            case .info:
                Image(systemName: "info.circle").font(.plat(mac: 12, touch: 15)).foregroundStyle(.haze)
            case .failed:
                Image(systemName: "exclamationmark.triangle.fill").font(.ui(11)).foregroundStyle(.vermilion)
            }
            VStack(alignment: .leading, spacing: 2) {
                Text(title).font(.plat(mac: 13, touch: 17, weight: .semibold)).foregroundStyle(.silk)
                if let line {
                    Text(line).font(.plat(mac: 12, touch: 15)).foregroundStyle(.haze)
                }
            }
            .fixedSize(horizontal: false, vertical: true)
        }
        .accessibilityElement(children: .combine)
    }
}

/// A raised card for a result's rows.
struct CameInCard<Content: View>: View {
    @ViewBuilder var content: Content
    var body: some View {
        VStack(spacing: 0) { content }
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Palette.ground.opacity(0.4), in: .rect(cornerRadius: 10))
            .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(Palette.faintRule, lineWidth: 1))
    }
}

// MARK: - Folder

/// What a folder held, after it was added: series and files, the files Kaeri cannot open as a
/// count by format with Show files, and why nothing came in when nothing did.
struct FolderCameIn: View {
    let report: FolderReport
    let nothing: Bool
    /// Change on the folder's row, where the sheet can pick another.
    var change: (() -> Void)?
    private var skippedCount: UInt32 { (report.unsupported + report.locked).reduce(0) { $0 + $1.count } }
    @State private var showFiles = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 12) {
                Image(systemName: "folder").font(.ui(15)).foregroundStyle(.silk)
                    .frame(width: 32, height: 32).background(Palette.raised, in: .rect(cornerRadius: 8))
                Text(report.path?.replacingOccurrences(of: NSHomeDirectory(), with: "~") ?? report.name) // i18n-skip
                    .font(.plat(mac: 13, touch: 17)).foregroundStyle(.silk).lineLimit(1).truncationMode(.middle)
                if let change {
                    Spacer(minLength: 8)
                    Button(L.bookFolderChange, action: change)
                        .buttonStyle(.plain)
                        .font(.plat(mac: 13, touch: 15, weight: .semibold))
                        .foregroundStyle(.silk)
                        .fixedSize()
                        .frame(minHeight: Device.isMac ? 24 : 44)
                        .contentShape(.rect)
                        .pointerHover()
                }
            }
            if nothing {
                CameInNote(kind: .info, title: L.bookFolderEmpty(folder: report.name),
                           line: L.bookFolderEmptyLine(files: Int(report.files)))
            } else {
                CameInNote(kind: .found, title: L.bookFolderFound(series: Int(report.series), files: Int(report.files)),
                           line: L.bookFolderFoundLine(path: report.path?.replacingOccurrences(of: NSHomeDirectory(), with: "~") ?? report.name, // i18n-skip
                                                       matched: Int(report.matched)))
            }
            if skippedCount > 0 {
                CameInCard {
                    HStack(spacing: 12) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(L.bookFolderUnsupported(count: Int(skippedCount)))
                                .font(.plat(mac: 13, touch: 17)).foregroundStyle(.silk)
                            Text(L.bookFolderUnsupportedLine(kinds: kinds))
                                .font(.plat(mac: 12, touch: 15)).foregroundStyle(.haze)
                                .fixedSize(horizontal: false, vertical: true)
                        }
                        Spacer(minLength: 8)
                        if !report.listed.isEmpty {
                            Button { withAnimation(reduceMotion ? .easeInOut(duration: 0.2) : .standard) { showFiles.toggle() } } label: {
                                HStack(spacing: 4) {
                                    Text(L.bookFolderShowFiles)
                                    Image(systemName: "chevron.right").font(.ui(11, weight: .semibold))
                                        .rotationEffect(.degrees(showFiles ? 90 : 0))
                                }
                                .font(.plat(mac: 12, touch: 15)).foregroundStyle(.haze)
                            }
                            .buttonStyle(.plain)
                            .pointerHover()
                        }
                    }
                    .padding(.horizontal, 12).padding(.vertical, 10)
                    if showFiles {
                        Rectangle().fill(Palette.faintRule).frame(height: 1)
                        VStack(alignment: .leading, spacing: 4) {
                            ForEach(report.listed, id: \.self) { name in
                                Text(name).font(.plat(mac: 12, touch: 15)).foregroundStyle(.haze).lineLimit(1)
                            }
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.horizontal, 12).padding(.vertical, 10)
                        .transition(.opacity)
                    }
                }
            }
            Text(L.bookFolderOpens).font(.plat(mac: 12, touch: 13)).foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    /// "14 MOBI, 6 AZW3, 3 EPUB locked with DRM".
    private var kinds: String {
        var parts = report.unsupported.map { "\($0.count) \($0.format)" } // i18n-skip
        let locked = report.locked.reduce(0) { $0 + Int($1.count) }
        if locked > 0 { parts.append(L.bookFolderDrm(count: locked)) }
        return parts.joined(separator: ", ") // i18n-skip
    }
}

/// The folder report as a sheet of its own, over Your sources or first run, as the board lays
/// it: the close button and the title, the folder with Change, what it holds, and Add folder
/// across the width. The folder is read before the sheet shows; closing it takes the folder
/// back out, and Add folder keeps it.
struct FolderCameInSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    let report: FolderReport
    let nothing: Bool
    @State private var picking = false
    @State private var kept = false

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 12) {
                Button { dismiss() } label: {
                    Image(systemName: "xmark")
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundStyle(Color.silk)
                        .frame(width: 28, height: 28)
                        .background(Circle().fill(Palette.ground.opacity(0.5)))
                        .frame(width: 44, height: 44)
                        .contentShape(.circle)
                }
                .buttonStyle(.plain)
                .pointerHover()
                .accessibilityLabel(L.actionClose)
                .padding(.leading, -8)
                Text(L.sourcesAddFolderButton)
                    .font(.plat(mac: 17, touch: 17, weight: .semibold)).foregroundStyle(.silk)
                    .accessibilityAddTraits(.isHeader)
            }
            .padding(.bottom, 14)
            ScrollView {
                FolderCameIn(report: report, nothing: nothing, change: change)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            .scrollBounceBehavior(.basedOnSize)
            // Across the sheet on iPhone and iPad; at the trailing end on the Mac, as its sheets
            // put their buttons.
            Group {
                if nothing {
                    Button(action: change) { primaryLabel(L.bookFolderChooseOther) }
                } else {
                    Button {
                        kept = true
                        dismiss()
                    } label: {
                        primaryLabel(L.bookFolderAdd)
                    }
                }
            }
            .buttonStyle(PrimaryCapsuleStyle(height: Device.isMac ? 28 : 50, fontSize: Device.isMac ? 13 : 17))
            .frame(maxWidth: .infinity, alignment: .trailing)
            .keyboardShortcut(.defaultAction)
            .padding(.top, 20)
        }
        .padding(Device.isMac ? 24 : 20)
        .sheetWidth(480)
        .background(.raised)
        .presentationBackground(.raised)
        .presentationDetents([.medium, .large])
        .closesOnEscape { dismiss() }
        #if os(iOS)
        .fileImporter(isPresented: $picking, allowedContentTypes: [.folder]) { result in
            guard case .success(let url) = result else { return }
            store.bookSources.removeFolder(report.id)
            BookFolders.add(url, to: store)
        }
        #endif
        .onDisappear {
            // Closed without Add folder: the folder goes back out, as a form's cancel would.
            if !kept { store.bookSources.removeFolder(report.id) }
            store.cameIn.reset()
        }
    }

    /// The primary button's words: the label fills the width on iPhone and iPad.
    @ViewBuilder private func primaryLabel(_ text: String) -> some View {
        if Device.isMac {
            Text(text)
        } else {
            Text(text).frame(maxWidth: .infinity)
        }
    }

    /// Another folder in this one's place.
    private func change() {
        #if os(macOS)
        FolderPicker.choose { path in
            store.bookSources.removeFolder(report.id)
            BookFolders.add(URL(filePath: path), to: store)
        }
        #else
        picking = true
        #endif
    }
}

extension View {
    /// The folder report, over whatever added the folder.
    func folderCameInSheet(_ store: AppStore) -> some View {
        modifier(FolderCameInPresenter(store: store))
    }
}

private struct FolderCameInPresenter: ViewModifier {
    let store: AppStore
    func body(content: Content) -> some View {
        let outcome = store.cameIn.outcome
        let folder: (FolderReport, Bool)? = {
            switch outcome {
            case .folder(let report)?: (report, false)
            case .empty(.unsupportedFiles(let report))?, .empty(.emptyFolder(let report))?: (report, true)
            default: nil
            }
        }()
        content.sheet(isPresented: Binding(get: { folder != nil }, set: { if !$0 { store.cameIn.reset() } })) {
            if let (report, nothing) = folder {
                FolderCameInSheet(report: report, nothing: nothing)
            }
        }
    }
}

// MARK: - Plugin store

/// What a joined store holds for this device: the plugins that fit, with Install and Install all
/// for reading, a count of the rest, and why nothing is offered when nothing is.
struct StoreCameIn: View {
    @Environment(AppStore.self) private var store
    let outcome: CameIn
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            switch outcome {
            case .store(let report):
                let total = report.forReading.count + report.forWatching.count
                CameInNote(kind: .found, title: L.storeJoinedTitle(store: report.name),
                           line: L.storeJoinedLine(owner: report.owner ?? report.name, count: total))
                group(report, names: report.forReading, title: L.storeGroupReading)
                group(report, names: report.forWatching, title: L.storeGroupWatching)
                if report.otherMedium > 0 {
                    Text(L.storeOtherMediumMore(count: Int(report.otherMedium)))
                        .font(.plat(mac: 12, touch: 15)).foregroundStyle(.haze)
                }
            case .installed(let done):
                CameInNote(kind: .found, title: L.storeInstalled(count: Int(done.installed)))
            case .empty(.otherMedium(let report)):
                CameInNote(kind: .info, title: L.storeOtherMedium(store: report.name), line: L.storeOtherMediumLine)
            case .empty(.emptyStore(let report)):
                CameInNote(kind: .info, title: L.storeEmpty(store: report.name),
                           line: L.storeEmptyLine(owner: report.owner ?? report.name))
            case .empty(.storeUnreachable(let host)):
                CameInNote(kind: .failed, title: L.storeErrUnreachable(host: host),
                           line: L.storeErrUnreachableLine(owner: host))
            default:
                EmptyView()
            }
        }
    }

    /// A group of plugins by name, with what the store's own row knows of each.
    @ViewBuilder
    private func group(_ report: StoreReport, names: [String], title: String) -> some View {
        if !names.isEmpty {
            VStack(alignment: .leading, spacing: 6) {
                Text(title).font(.plat(mac: 13, touch: 15, weight: .semibold)).foregroundStyle(.silk)
                    .accessibilityAddTraits(.isHeader).padding(.top, 4)
                CameInCard {
                    ForEach(Array(names.enumerated()), id: \.offset) { index, name in
                        if index > 0 { Rectangle().fill(Palette.faintRule).frame(height: 1).padding(.leading, 56) }
                        pluginRow(report, name: name)
                    }
                }
            }
        }
    }

    private func pluginRow(_ report: StoreReport, name: String) -> some View {
        let row = store.stores.stores.first { $0.id == report.store }
        let plugin = row?.plugins.first { $0.name == name || $0.id == name }
        let installed = plugin.map { $0.status == .installed } ?? store.cameIn.installed.contains(name)
        return HStack(spacing: 12) {
            Image(systemName: "powerplug").font(.ui(14)).foregroundStyle(.silk)
                .frame(width: 32, height: 32).background(Palette.raised, in: .rect(cornerRadius: 8))
            VStack(alignment: .leading, spacing: 2) {
                Text(plugin?.name ?? name).font(.plat(mac: 13, touch: 17)).foregroundStyle(.silk)
                if let plugin {
                    Text(L.storePluginRow(version: plugin.version, hosts: plugin.contactsLine))
                        .font(.plat(mac: 12, touch: 15)).foregroundStyle(.haze).lineLimit(1)
                }
            }
            Spacer(minLength: 8)
            if installed {
                Text(L.storeInstalledRow).font(.plat(mac: 12, touch: 15)).foregroundStyle(.haze)
            } else {
                Button(L.storeInstall) { store.cameIn.install(store: report.store, plugin: plugin?.id ?? name) }
                    .buttonStyle(SecondaryCapsuleStyle(height: Device.isMac ? 24 : 36, raisedSurface: true, fontSize: Device.isMac ? 12 : 15))
                    .fixedSize()
            }
        }
        .padding(.horizontal, 12).padding(.vertical, 8)
    }

    /// The sheet's actions for this outcome: the amber one, and Done.
    @ViewBuilder
    static func actions(_ outcome: CameIn, store: AppStore, done: @escaping () -> Void) -> some View {
        let height: CGFloat = Device.isMac ? 28 : 48
        let font: CGFloat = Device.isMac ? 13 : 17
        HStack(spacing: 10) {
            Spacer()
            switch outcome {
            case .store(let report) where report.installAll:
                Button(L.actionDone, action: done)
                    .buttonStyle(SecondaryCapsuleStyle(height: height, raisedSurface: true, fontSize: font))
                Button(L.storeInstallAllReading) { store.cameIn.installAll(store: report.store) }
                    .buttonStyle(PrimaryCapsuleStyle(height: height, fontSize: font))
                    .keyboardShortcut(.defaultAction)
            case .empty(.otherMedium):
                Button(L.actionDone, action: done)
                    .buttonStyle(SecondaryCapsuleStyle(height: height, raisedSurface: true, fontSize: font))
                Button(L.storeTurnOnWatching) { store.medium.choose(.both) }
                    .buttonStyle(PrimaryCapsuleStyle(height: height, fontSize: font))
                    .keyboardShortcut(.defaultAction)
            default:
                Button(L.actionDone, action: done)
                    .buttonStyle(PrimaryCapsuleStyle(height: height, fontSize: font))
                    .keyboardShortcut(.defaultAction)
            }
        }
    }

    /// Whether an outcome is one this view tells.
    static func tells(_ outcome: CameIn?) -> Bool {
        switch outcome {
        case .store?, .installed?, .empty(.otherMedium)?, .empty(.emptyStore)?, .empty(.storeUnreachable)?: true
        default: false
        }
    }
}

// MARK: - Notice

/// "Added Books · 212 series" for 4 s after a server or plugins came in, as the board's notice.
struct CameInNotice: View {
    @Environment(AppStore.self) private var store
    @State private var text: String?
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        Color.clear
            .frame(height: 0)
            .overlay(alignment: .bottom) {
                if let text {
                    Text(text)
                        .font(.plat(mac: 13, touch: 15, weight: .semibold)).foregroundStyle(.silk)
                        .padding(.horizontal, 16).padding(.vertical, 10)
                        .background(Palette.raised, in: .capsule)
                        .overlay(Capsule().strokeBorder(Palette.faintRule, lineWidth: 1))
                        .transition(.opacity)
                        .padding(.bottom, 16)
                        .accessibilityAddTraits(.updatesFrequently)
                }
            }
            .onChange(of: store.sync.joinedFrom, initial: true) { _, device in
                guard let device else { return }
                show(L.firstJoinSyncing(device: device))
            }
            .onChange(of: store.cameIn.outcome, initial: true) { _, outcome in
                let said: String? = {
                    switch outcome {
                    case .serverAdded(let added)?: L.sourceAdded(name: added.name, series: Int(added.series ?? 0))
                    case .installed(let done)?: L.sourceAddedPlugins(count: Int(done.installed), store: done.name)
                    default: nil
                    }
                }()
                guard let said else { return }
                show(said)
            }
    }

    private func show(_ said: String) {
        Task {
            withAnimation(reduceMotion ? .easeInOut(duration: 0.2) : .standard) { text = said }
            try? await Task.sleep(for: .seconds(4))
            withAnimation(reduceMotion ? .easeInOut(duration: 0.2) : .standard) { text = nil }
        }
    }
}
