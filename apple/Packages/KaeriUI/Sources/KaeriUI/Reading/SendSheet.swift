// Send to e-reader: the sheet that prepares chosen chapters for one e-reader and hands the files
// to the share sheet, the status row a send keeps on screen after the sheet closes (its progress,
// or the line the engine gives while it waits for Wi-Fi, with Cancel), and Where did you stop,
// which the series page asks once the files were shared.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

/// The chapters a send was asked for, from a series' page or its Downloads row.
struct SendRequest: Identifiable, Equatable {
    let work: String
    let chapters: [String]
    var id: String { work + "|" + chapters.joined(separator: ",") } // i18n-skip
}

enum SendWords {
    /// A target's line in the picker: the format the files will have and the screen they are made for.
    @MainActor
    static func line(_ target: ExportTarget, novel: Bool) -> String {
        var parts: [String] = []
        if let format = SendModel.format(of: target, novel: novel) {
            parts.append(L.sendTargetLine(format: name(format), width: String(target.widthPx), height: String(target.heightPx)))
        }
        if target.split && !novel {
            parts.append(L.sendPartsOver(size: Wording.bytes(200 * 1_000_000)))
        }
        return parts.joined(separator: ", ") // i18n-skip
    }

    static func name(_ format: ExportFormat) -> String {
        switch format {
        case .fixedEpub: L.sendFormatFixedEpub
        case .kepub: L.sendFormatKepub
        case .epub: L.sendFormatEpub
        case .cbz: L.sendFormatCbz
        }
    }

    /// The e-reader's name, from its id: what the engine lists, else the id as it was kept.
    @MainActor
    static func device(_ id: String, _ store: AppStore) -> String {
        store.reading.sending.targets.first { $0.id == id }?.name ?? id
    }

    /// The chapters of a series in reading order, for the ones asked for.
    @MainActor
    static func rows(_ ids: [String], work: String, _ store: AppStore) -> [ChapterRow] {
        let all = store.reading.chapters[work]?.volumes.flatMap(\.chapters) ?? []
        let picked = all.filter { ids.contains($0.id) }
        let numbers = picked.compactMap { $0.number.flatMap(Double.init) }
        guard numbers.count == picked.count else { return picked }
        return picked.sorted { ($0.number.flatMap(Double.init) ?? 0) < ($1.number.flatMap(Double.init) ?? 0) }
    }

    static func label(_ row: ChapterRow) -> String { row.number ?? row.label }

    /// "41 to 43", or the one chapter.
    @MainActor
    static func chapters(_ rows: [ChapterRow]) -> String {
        guard let first = rows.first, let last = rows.last else { return "–" } // i18n-skip
        if rows.count == 1 { return L.sendChaptersOne(chapter: label(first)) }
        return L.sendChaptersRange(first: label(first), last: label(last))
    }

    /// What the files will take, from what is on this device: the downloaded chapters' share of
    /// the series' bytes, and what the engine will fetch for the rest. A guess until they are made.
    @MainActor
    static func size(_ ids: [String], work: String, _ store: AppStore) -> UInt64? {
        let rows = rows(ids, work: work, store)
        guard !rows.isEmpty else { return nil }
        let held = rows.filter { $0.download == .done }.count
        var bytes: UInt64 = 0
        if held > 0 {
            guard let kept = store.reading.downloads?.series.first(where: { $0.work == work }), kept.chapters > 0 else { return nil }
            bytes += kept.bytes / UInt64(kept.chapters) * UInt64(held)
        }
        if held < rows.count {
            guard let estimate = store.reading.estimates[work], estimate.pick == .chapters(ids: ids) else { return nil }
            bytes += estimate.bytes
        }
        return bytes
    }
}

// MARK: - The sheet

struct SendSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    let request: SendRequest
    @State private var chosen: String?
    @State private var ids: [String]
    @State private var sendId: Int?
    @State private var choosingChapters = false
    /// The size is still being worked out after a moment: the row says so.
    @State private var waitedForSize = false
    #if os(iOS)
    @State private var sharing: SharedFiles?
    #endif

    init(request: SendRequest) {
        self.request = request
        _ids = State(initialValue: request.chapters)
    }

    private var novel: Bool { store.reading.series[request.work]?.medium == .novel }
    private var targets: [ExportTarget] { store.reading.sending.targets }
    private var target: ExportTarget? {
        targets.first { $0.id == chosen } ?? store.reading.sending.defaultTarget
    }
    private var send: EReaderSend? { sendId.flatMap { store.reading.sending.send($0) } }

    /// The board's three e-readers; every other screen the engine knows sits under Other e-reader.
    private static let listed = ["kobo", "kindle"]
    private var primary: [ExportTarget] { targets.filter { Self.listed.contains($0.id) } }
    private var others: [ExportTarget] { targets.filter { !Self.listed.contains($0.id) } }

    /// Closing once a send has started is not cancelling it: the status row keeps going.
    private var closeWords: String {
        guard let send else { return L.actionCancel }
        if case .ready = send.phase { return L.actionDone }
        return L.actionClose
    }

    var body: some View {
        ReadSheetFrame(title: L.sendTitle, subtitle: store.reading.series[request.work]?.titles.display, close: closeWords, tallest: 740, fitted: true, pinned: pinnedFooter) {
            VStack(alignment: .leading, spacing: 0) {
                SheetHeading(text: L.sendFor)
                SheetRows {
                    ForEach(primary, id: \.id) { item in targetRow(item) }
                    if !others.isEmpty { otherRow }
                }
                .disabled(send?.isRunning == true)
                SheetHeading(text: L.sendWhat)
                SheetRows {
                    chaptersRow
                    #if os(macOS)
                    if choosingChapters { chapterPicker }
                    #endif
                    sizeRow
                }
                .disabled(send?.isRunning == true)
                if let size = estimatedBytes, target?.split == true, !novel, size > Self.partsOver {
                    SheetNote(text: L.sendInParts(size: Wording.bytes(Self.partsOver)))
                }
                SheetNote(text: L.sendNoPlace)
                if pinnedFooter == nil {
                    footer
                        .padding(.top, 22)
                        .padding(.horizontal, 12)
                }
            }
        }
        .task {
            store.reading.sending.loadTargets()
            if store.reading.chapters[request.work] == nil { store.reading.loadChapters(request.work) }
            await stageIfAsked()
        }
        .task(id: ids) {
            waitedForSize = false
            try? await Task.sleep(for: .milliseconds(150))
            guard !Task.isCancelled else { return }
            store.reading.estimateDownload(request.work, .chapters(ids: ids))
            try? await Task.sleep(for: .milliseconds(450))
            guard !Task.isCancelled else { return }
            withAnimation(.standard) { waitedForSize = true }
        }
        .onChange(of: send?.phase) { _, phase in
            #if os(iOS)
            if case .ready(let files)? = phase, let id = sendId {
                sharing = SharedFiles(id: id, urls: files.map { URL(fileURLWithPath: $0.path) })
            }
            #endif
        }
        #if os(iOS)
        .sheet(item: $sharing) { files in
            ActivitySheet(urls: files.urls) { done in
                sharing = nil
                if done {
                    store.reading.sending.shared(files.id)
                    dismiss()
                }
            }
            .presentationDetents([.medium, .large])
        }
        #endif
    }

    // MARK: Rows

    private func targetRow(_ item: ExportTarget) -> some View {
        let on = item.id == target?.id
        return Button {
            chosen = item.id
        } label: {
            HStack(spacing: 12) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(item.name)
                        .font(.plat(mac: 13, touch: 17))
                        .foregroundStyle(.silk)
                        .lineLimit(1)
                    Text(SendWords.line(item, novel: novel))
                        .font(.plat(mac: 12, touch: 15))
                        .foregroundStyle(.haze)
                        .lineLimit(2)
                }
                Spacer(minLength: 8)
                if on {
                    Image(systemName: "checkmark")
                        .font(.ui(13, weight: .semibold))
                        .foregroundStyle(.silk)
                        .accessibilityHidden(true)
                }
            }
            .padding(.horizontal, 12)
            .padding(.vertical, Device.isMac ? 5 : 12)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .pointerHover()
        .accessibilityAddTraits(on ? .isSelected : [])
    }

    /// Other e-reader: the screens the board does not list, in a menu on the row.
    private var otherRow: some View {
        let kept = others.first { $0.id == target?.id }
        let on = kept != nil
        let shown = kept ?? others.first { $0.id == "other" } ?? others[0]
        return Menu {
            ForEach(others, id: \.id) { item in
                Button {
                    chosen = item.id
                } label: {
                    if item.id == target?.id { Image(systemName: "checkmark") }
                    Text(item.name)
                    Text(SendWords.line(item, novel: novel))
                }
            }
        } label: {
            HStack(spacing: 12) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(on && shown.id != "other" ? shown.name : (others.first { $0.id == "other" }?.name ?? shown.name))
                        .font(.plat(mac: 13, touch: 17))
                        .foregroundStyle(.silk)
                        .lineLimit(1)
                    Text(SendWords.line(shown, novel: novel))
                        .font(.plat(mac: 12, touch: 15))
                        .foregroundStyle(.haze)
                        .lineLimit(2)
                }
                Spacer(minLength: 8)
                if on {
                    Image(systemName: "checkmark")
                        .font(.ui(13, weight: .semibold))
                        .foregroundStyle(.silk)
                        .accessibilityHidden(true)
                }
                Image(systemName: "chevron.up.chevron.down")
                    .font(.ui(11, weight: .semibold))
                    .foregroundStyle(.haze)
                    .accessibilityHidden(true)
            }
            .padding(.horizontal, 12)
            .padding(.vertical, Device.isMac ? 5 : 12)
            .contentShape(.rect)
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .menuIndicator(.hidden)
        .pointerHover()
        .accessibilityAddTraits(on ? .isSelected : [])
    }

    private var chaptersLabel: some View {
        HStack(spacing: 8) {
            Text(L.sendChapters).font(.plat(mac: 13, touch: 17)).foregroundStyle(.silk)
            Spacer(minLength: 8)
            Text(SendWords.chapters(SendWords.rows(ids, work: request.work, store)))
                .font(.plat(mac: 13, touch: 17))
                .foregroundStyle(.haze)
                .monospacedDigit()
            Image(systemName: "chevron.right")
                .font(.ui(12, weight: .semibold))
                .foregroundStyle(.haze)
                .rotationEffect(.degrees(Device.isMac && choosingChapters ? 90 : 0))
                .accessibilityHidden(true)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, Device.isMac ? 9 : 12)
        .contentShape(.rect)
    }

    /// On the Mac the chapters open in place; on touch they are a screen of their own, by volume.
    @ViewBuilder
    private var chaptersRow: some View {
        #if os(iOS)
        NavigationLink {
            ScrollView {
                SendChapterList(work: request.work, ids: $ids)
                    .padding(.horizontal, 16)
                    .padding(.vertical, 8)
            }
            .background(.raised)
            .navigationTitle(L.sendChapters)
            .navigationBarTitleDisplayMode(.inline)
        } label: {
            chaptersLabel
        }
        .buttonStyle(.plain)
        #else
        Button {
            withAnimation(.standard) { choosingChapters.toggle() }
        } label: {
            chaptersLabel
        }
        .buttonStyle(.plain)
        .pointerHover()
        .accessibilityValue(choosingChapters ? L.sendChaptersOpen : L.sendChaptersClosed)
        #endif
    }

    /// What the files will take, as far as it is known.
    private var estimatedBytes: UInt64? { SendWords.size(ids, work: request.work, store) }
    /// The size where a file is split, near 200 MB.
    private static let partsOver: UInt64 = 200 * 1_000_000

    /// Every chapter of the series by volume, ticked or not, to change what is sent. Scrolls on
    /// its own past a height, so the sheet does not grow past the screen.
    private var chapterPicker: some View {
        ScrollView {
            SendChapterList(work: request.work, ids: $ids)
        }
        .frame(maxHeight: 240)
        .transition(.opacity)
    }

    private var sizeRow: some View {
        HStack {
            Text(L.sendSize).font(.plat(mac: 13, touch: 17)).foregroundStyle(.silk)
            Spacer(minLength: 8)
            if let bytes = estimatedBytes {
                Text(L.sendSizeAbout(size: Wording.bytes(bytes)))
                    .font(.plat(mac: 13, touch: 17))
                    .foregroundStyle(.haze)
                    .monospacedDigit()
            } else if waitedForSize {
                Text(L.sendSizeWorking)
                    .font(.plat(mac: 13, touch: 17))
                    .foregroundStyle(.haze)
                    .transition(.opacity)
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, Device.isMac ? 9 : 12)
    }

    // MARK: The foot

    /// On touch the footer stays under the list, which is long with every e-reader the engine knows.
    private var pinnedFooter: AnyView? {
        #if os(iOS)
        AnyView(footer)
        #else
        nil
        #endif
    }

    /// The button gives way to the ring and what is happening, in a crossfade.
    @ViewBuilder
    private var footer: some View {
        Group {
            if let send {
                SendProgress(send: send, count: ids.count, target: target, novel: novel,
                             share: { share(send) }, retry: { start() },
                             shared: { store.reading.sending.shared(send.id); dismiss() })
            } else {
                Button { start() } label: {
                    Text(L.sendPrepare)
                        .frame(maxWidth: Device.isMac ? nil : .infinity)
                }
                .buttonStyle(PrimaryCapsuleStyle(height: Device.isMac ? 30 : 50, fontSize: Device.isMac ? 13 : 17))
                .disabled(target == nil || ids.isEmpty || SendModel.format(of: target!, novel: novel) == nil)
                .frame(maxWidth: .infinity, alignment: Device.isMac ? .trailing : .center)
            }
        }
        .animation(.standard, value: send?.isRunning)
        .transition(.opacity)
    }

    /// A shot of a state a send is in is staged when the sheet comes up.
    func stageIfAsked() async {
        guard sendId == nil, let state = SendModel.launchState() else { return }
        for _ in 0..<30 where store.reading.sending.defaultTarget == nil {
            try? await Task.sleep(for: .milliseconds(100))
        }
        guard let target else { return }
        sendId = store.reading.sending.stage(state, work: request.work, chapters: ids, target: target.id)
    }

    private func start() {
        guard let target else { return }
        if let sendId { store.reading.sending.dismiss(sendId) }
        sendId = store.reading.sending.begin(work: request.work, chapters: ids, target: target, novel: novel)
    }

    private func share(_ send: EReaderSend) {
        guard case .ready(let files) = send.phase else { return }
        #if os(iOS)
        sharing = SharedFiles(id: send.id, urls: files.map { URL(fileURLWithPath: $0.path) })
        #endif
    }
}

/// What a send shows where the button was: a ring and the words while it is made, the engine's
/// line and Cancel while it waits for Wi-Fi, the files ready to share, or why it failed.
struct SendProgress: View {
    @Environment(AppStore.self) private var store
    let send: EReaderSend
    let count: Int
    var target: ExportTarget?
    var novel = false
    let share: () -> Void
    let retry: () -> Void
    let shared: () -> Void

    var body: some View {
        VStack(spacing: 6) {
            switch send.phase {
            case .preparing(let fraction, let done, let total):
                HStack(spacing: 12) {
                    Button { store.reading.sending.cancel(send.id) } label: {
                        DownloadRing(fraction: fraction ?? 0)
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(L.sendWaitingCancel)
                    .accessibilityValue(L.sendProgressSpoken(percent: percent(fraction)))
                    Text(preparing(done: done, total: total))
                        .font(.plat(mac: 13, touch: 17))
                        .foregroundStyle(.silk)
                        .monospacedDigit()
                }
                .frame(minHeight: Device.isMac ? 30 : 50)
                if let target, !novel {
                    Text(L.sendSizedFor(device: target.name))
                        .font(.plat(mac: 12, touch: 13))
                        .foregroundStyle(.haze)
                }
            case .waiting(let line):
                SendWaitingLine(line: line, cancel: { store.reading.sending.cancel(send.id) })
            case .ready(let files):
                Text(L.sendReady(n: files.count))
                    .font(.plat(mac: 13, touch: 17))
                    .foregroundStyle(.silk)
                #if os(iOS)
                Button(action: share) {
                    Text(L.sendShareAgain).frame(maxWidth: .infinity)
                }
                .buttonStyle(PrimaryCapsuleStyle(height: 50, fontSize: 17))
                #else
                MacShareButton(files: files, shared: shared)
                #endif
            case .failed(let words):
                FailedLine(words)
                Button(L.actionTryAgain, action: retry)
                    .buttonStyle(SecondaryCapsuleStyle(height: Device.isMac ? 28 : 44, raisedSurface: true, fontSize: Device.isMac ? 13 : 17))
            }
        }
        .frame(maxWidth: .infinity)
        .multilineTextAlignment(.center)
        .animation(.standard, value: send.kind)
        .onChange(of: quarter) { _, now in
            guard now > 0 else { return }
            AccessibilityNotification.Announcement(L.sendProgressSpoken(percent: now * 25)).post()
        }
    }

    private func percent(_ fraction: Double?) -> Int { Int(((fraction ?? 0) * 100).rounded()) }

    /// How many quarters of the way, for the spoken updates.
    private var quarter: Int {
        guard case .preparing(let fraction, _, _) = send.phase else { return 0 }
        return min(4, Int((fraction ?? 0) * 4))
    }

    /// Which chapter of how many, as the engine counts them.
    private func preparing(done: Int, total: Int) -> String {
        let all = max(total > 0 ? total : count, 1)
        let at = min(all, (total > 0 ? done : 0) + 1)
        return L.sendPreparing(done: at, total: all)
    }
}

/// A send held for Wi-Fi: the engine's own line, and Cancel. Goes on by itself; the next
/// progress takes this away.
struct SendWaitingLine: View {
    let line: String
    let cancel: () -> Void

    var body: some View {
        VStack(spacing: 10) {
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Image(systemName: "wifi")
                    .font(.plat(mac: 12, touch: 14))
                    .foregroundStyle(.haze)
                    .accessibilityHidden(true)
                Text(line)
                    .font(.plat(mac: 13, touch: 15))
                    .foregroundStyle(.silk)
                    .fixedSize(horizontal: false, vertical: true)
            }
            Button(L.sendWaitingCancel, action: cancel)
                .buttonStyle(SecondaryCapsuleStyle(height: Device.isMac ? 28 : 44, raisedSurface: true, fontSize: Device.isMac ? 13 : 17))
        }
        .frame(maxWidth: .infinity)
    }
}

// MARK: - A send that outlives its sheet

/// The sends a screen has going, one quiet row each: what it is doing, or the engine's line
/// while it waits for Wi-Fi, with Cancel. A send held across a restart is here too, though no
/// sheet asked for it. Ready and failed ones stay until dismissed.
struct SendStatusRows: View {
    @Environment(AppStore.self) private var store
    /// A series' page shows its own; Downloads shows all.
    var work: String?

    var body: some View {
        let sends = store.reading.sending.sends.filter { work == nil || $0.work == work }
        Group {
            if !sends.isEmpty { rows(sends) }
        }
        .animation(.standard, value: sends.map(\.id))
    }

    private func rows(_ sends: [EReaderSend]) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach(sends) { send in
                SendStatusRow(send: send)
                if send.id != sends.last?.id { Rectangle().fill(.faintRule).frame(height: 1) }
            }
        }
        .padding(.horizontal, 14)
        .background(.raised, in: .rect(cornerRadius: 12))
        .padding(.top, 20)
        .transition(.opacity)
    }
}

private struct SendStatusRow: View {
    @Environment(AppStore.self) private var store
    let send: EReaderSend
    #if os(iOS)
    @State private var sharing: SharedFiles?
    #endif

    var body: some View {
        let device = SendWords.device(send.target, store)
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 12) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(device.isEmpty ? L.sendTitle : L.sendBanner(device: device))
                        .font(.plat(mac: 13, touch: 17, weight: .semibold))
                        .foregroundStyle(.silk)
                    if let title = title {
                        Text(title).font(.plat(mac: 12, touch: 15)).foregroundStyle(.haze).lineLimit(1)
                    }
                }
                Spacer(minLength: 8)
                switch send.phase {
                case .preparing(let fraction, _, _):
                    Button { store.reading.sending.cancel(send.id) } label: {
                        DownloadRing(fraction: fraction ?? 0)
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(L.sendWaitingCancel)
                    .accessibilityValue(L.sendProgressSpoken(percent: Int(((fraction ?? 0) * 100).rounded())))
                case .waiting:
                    Button(L.sendWaitingCancel) { store.reading.sending.cancel(send.id) }
                        .buttonStyle(SecondaryCapsuleStyle(height: Device.isMac ? 24 : 30, raisedSurface: true, fontSize: Device.isMac ? 12 : 15))
                        .fixedSize()
                case .ready(let files):
                    HStack(spacing: 12) {
                        Button(L.actionDone) { store.reading.sending.dismiss(send.id) }
                            .buttonStyle(.plain)
                            .font(.plat(mac: 12, touch: 15, weight: .semibold))
                            .foregroundStyle(.haze)
                            .pointerHover()
                        #if os(iOS)
                        Button(L.actionShare) { sharing = SharedFiles(id: send.id, urls: files.map { URL(fileURLWithPath: $0.path) }) }
                            .buttonStyle(SecondaryCapsuleStyle(height: 30, raisedSurface: true, fontSize: 15))
                            .fixedSize()
                        #else
                        MacShareButton(files: files) { store.reading.sending.shared(send.id) }
                        #endif
                    }
                case .failed:
                    Button(L.actionDone) { store.reading.sending.dismiss(send.id) }
                        .buttonStyle(.plain)
                        .font(.plat(mac: 12, touch: 15, weight: .semibold))
                        .foregroundStyle(.silk)
                        .pointerHover()
                }
            }
            switch send.phase {
            case .waiting(let line):
                Text(line)
                    .font(.plat(mac: 12, touch: 15))
                    .foregroundStyle(.silk)
                    .fixedSize(horizontal: false, vertical: true)
            case .failed(let words):
                FailedLine(words)
            case .ready(let files):
                Text(L.sendReady(n: files.count)).font(.plat(mac: 12, touch: 15)).foregroundStyle(.haze)
            case .preparing:
                EmptyView()
            }
        }
        .padding(.vertical, 12)
        .accessibilityElement(children: .contain)
        .animation(.standard, value: send.kind)
        #if os(iOS)
        .sheet(item: $sharing) { files in
            ActivitySheet(urls: files.urls) { done in
                sharing = nil
                if done { store.reading.sending.shared(files.id) }
            }
            .presentationDetents([.medium, .large])
        }
        #endif
    }

    private var title: String? {
        store.reading.series[send.work]?.titles.display
    }
}

// MARK: - Where did you stop

/// Asked from the series page once the files were shared: the chapters sent, the last one
/// chosen, and whether it was read to its end. Keep leaves the place as it is.
struct StoppedSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    let work: String
    let sent: SentChapters
    @State private var picked: String?
    @State private var finished = true

    var body: some View {
        let rows = SendWords.rows(sent.chapters, work: work, store)
        let at = rows.first { $0.id == picked } ?? rows.last
        ReadSheetFrame(title: L.stopTitle, subtitle: nil, close: L.actionCancel, fitted: true) {
            VStack(alignment: .leading, spacing: 0) {
                Text(sentLine(rows))
                    .font(.plat(mac: 13, touch: 15))
                    .foregroundStyle(.haze)
                    .padding(.horizontal, 12)
                    .padding(.bottom, 12)
                SheetRows {
                    ForEach(rows, id: \.id) { row in
                        Button { picked = row.id } label: {
                            HStack(spacing: 8) {
                                Text(row.label).font(.plat(mac: 13, touch: 17)).foregroundStyle(.silk)
                                if let title = row.title {
                                    Text(title).font(.plat(mac: 13, touch: 17)).foregroundStyle(.haze).lineLimit(1)
                                }
                                Spacer(minLength: 8)
                                if row.id == at?.id {
                                    Image(systemName: "checkmark").font(.ui(13, weight: .semibold)).foregroundStyle(.silk)
                                        .accessibilityHidden(true)
                                }
                            }
                            .padding(.horizontal, 12)
                            .padding(.vertical, Device.isMac ? 9 : 12)
                            .contentShape(.rect)
                        }
                        .buttonStyle(.plain)
                        .pointerHover()
                        .accessibilityAddTraits(row.id == at?.id ? .isSelected : [])
                    }
                }
                if let at {
                    SheetRows {
                        HStack {
                            Text(L.stopFinished(chapter: SendWords.label(at)))
                                .font(.plat(mac: 13, touch: 17))
                                .foregroundStyle(.silk)
                            Spacer(minLength: 8)
                            Toggle(L.stopFinished(chapter: SendWords.label(at)), isOn: $finished)
                                .toggleStyle(.switch)
                                .tint(.amber)
                                .labelsHidden()
                        }
                        .padding(.horizontal, 12)
                        .padding(.vertical, Device.isMac ? 6 : 10)
                    }
                    .padding(.top, 12)
                }
                VStack(spacing: 6) {
                    Button { set(rows, at: at) } label: {
                        Text(L.stopSet).frame(maxWidth: Device.isMac ? nil : .infinity)
                    }
                    .buttonStyle(PrimaryCapsuleStyle(height: Device.isMac ? 30 : 50, fontSize: Device.isMac ? 13 : 17))
                    .disabled(at == nil)
                    Button(keepWords) {
                        store.reading.sending.stoppedAt(work: work, chapter: nil)
                        dismiss()
                    }
                    .buttonStyle(.plain)
                    .font(.plat(mac: 13, touch: 17, weight: .semibold))
                    .foregroundStyle(.silk)
                    .frame(minHeight: 44)
                    .pointerHover()
                }
                .frame(maxWidth: .infinity)
                .padding(.top, 22)
            }
        }
        .task { if store.reading.chapters[work] == nil { store.reading.loadChapters(work) } }
    }

    private func sentLine(_ rows: [ChapterRow]) -> String {
        let device = SendWords.device(sent.target, store)
        let date = Date(timeIntervalSince1970: TimeInterval(sent.sentAt))
            .formatted(.dateTime.day().month(.wide).locale(AirTime.locale))
        guard let first = rows.first, let last = rows.last else { return "" }
        if rows.count == 1 {
            return L.stopSentOne(chapter: SendWords.label(first), device: device, date: date)
        }
        return L.stopSentRange(first: SendWords.label(first), last: SendWords.label(last), device: device, date: date)
    }

    /// The place as it stands: Keep says it, with the page when the series is mid-chapter.
    private var keepWords: String {
        if case .continue(_, let label, let page)? = store.reading.series[work]?.action {
            return L.stopKeep(chapter: label, page: Int(page))
        }
        return L.stopKeepPlain
    }

    /// Finished counts the chosen chapter as read; stopped inside it counts the one before it,
    /// and the chosen chapter is where reading goes on.
    private func set(_ rows: [ChapterRow], at: ChapterRow?) {
        guard let at else { return }
        if finished {
            store.reading.sending.stoppedAt(work: work, chapter: at.id)
        } else {
            let before = rows.firstIndex { $0.id == at.id }.flatMap { $0 > 0 ? rows[$0 - 1].id : nil }
            store.reading.sending.stoppedAt(work: work, chapter: before)
        }
        dismiss()
    }
}

// MARK: - The share sheet

#if os(iOS)
struct SharedFiles: Identifiable {
    let id: Int
    let urls: [URL]
}

/// The system's share sheet for the prepared files; `done` says whether anything was shared.
struct ActivitySheet: UIViewControllerRepresentable {
    let urls: [URL]
    let done: (Bool) -> Void

    func makeUIViewController(context: Context) -> UIActivityViewController {
        let controller = UIActivityViewController(activityItems: urls, applicationActivities: nil)
        controller.completionWithItemsHandler = { _, completed, _, _ in done(completed) }
        return controller
    }

    func updateUIViewController(_ controller: UIActivityViewController, context: Context) {}
}
#else
import AppKit

/// Share…, which opens the Mac's share menu for the prepared files, from the button itself.
struct MacShareButton: View {
    let files: [ExportFile]
    let shared: () -> Void
    @State private var anchor = ShareAnchor()

    var body: some View {
        Button(L.actionShare) {
            anchor.show(files.map { URL(fileURLWithPath: $0.path) }, shared: shared)
        }
        .buttonStyle(PrimaryCapsuleStyle(height: 30, fontSize: 13))
        .background(ShareAnchorView(anchor: anchor))
    }
}

@MainActor
final class ShareAnchor {
    weak var view: NSView?
    private var picker: NSSharingServicePicker?
    private var delegate: Delegate?

    func show(_ urls: [URL], shared: @escaping () -> Void) {
        guard let view else { return }
        let picker = NSSharingServicePicker(items: urls)
        let delegate = Delegate(shared: shared)
        picker.delegate = delegate
        self.picker = picker
        self.delegate = delegate
        picker.show(relativeTo: view.bounds, of: view, preferredEdge: .minY)
    }

    private final class Delegate: NSObject, NSSharingServicePickerDelegate, NSSharingServiceDelegate {
        let shared: () -> Void
        init(shared: @escaping () -> Void) { self.shared = shared }

        func sharingServicePicker(_ picker: NSSharingServicePicker, delegateFor sharingService: NSSharingService) -> NSSharingServiceDelegate? { self }
        func sharingService(_ sharingService: NSSharingService, didShareItems items: [Any]) { shared() }
    }
}

private struct ShareAnchorView: NSViewRepresentable {
    let anchor: ShareAnchor

    func makeNSView(context: Context) -> NSView {
        let view = NSView()
        anchor.view = view
        return view
    }

    func updateNSView(_ view: NSView, context: Context) { anchor.view = view }
}
#endif


/// Every chapter of a series by volume, ticked or not. At least one stays ticked.
struct SendChapterList: View {
    @Environment(AppStore.self) private var store
    let work: String
    @Binding var ids: [String]

    var body: some View {
        let groups = store.reading.chapters[work]?.volumes ?? []
        VStack(alignment: .leading, spacing: 0) {
            ForEach(Array(groups.enumerated()), id: \.offset) { _, group in
                if groups.count > 1 || group.volume != nil {
                    SheetHeading(text: group.volume.map { L.seriesVolume(n: $0) } ?? L.seriesNoVolume)
                }
                SheetRows {
                    ForEach(group.chapters, id: \.id) { row in chapter(row) }
                }
            }
        }
    }

    private func chapter(_ row: ChapterRow) -> some View {
        let on = ids.contains(row.id)
        return Button {
            if on {
                if ids.count > 1 { ids.removeAll { $0 == row.id } }
            } else {
                ids.append(row.id)
            }
        } label: {
            HStack(spacing: 8) {
                Text(row.label).font(.plat(mac: 13, touch: 17)).foregroundStyle(.silk).lineLimit(1)
                if let title = row.title {
                    Text(title).font(.plat(mac: 13, touch: 17)).foregroundStyle(.haze).lineLimit(1)
                }
                Spacer(minLength: 8)
                if on {
                    Image(systemName: "checkmark").font(.ui(12, weight: .semibold)).foregroundStyle(.silk)
                        .accessibilityHidden(true)
                }
            }
            .padding(.horizontal, 12)
            .padding(.vertical, Device.isMac ? 6 : 12)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .pointerHover()
        .accessibilityAddTraits(on ? .isSelected : [])
    }
}
