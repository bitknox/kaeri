// Did my server get it: the words of the server line on the page between chapters, and the
// server's row in Your sources with its progress line and Try again.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

enum ServerProgressWords {
    /// The page between chapters' line, without Try again.
    static func line(_ line: ServerLine) -> String {
        switch line {
        case .saved(let host): L.progressServerSaved(server: host)
        case .sending(let host): L.progressServerSending(server: host)
        case .offline(let host): L.progressServerOffline(server: host)
        case .retrying(let host): L.progressServerRetrying(server: host)
        case .failed(let host, let why): L.progressServerFailed(server: host, reason: reason(why))
        }
    }

    /// The server row's second line.
    static func row(_ line: ServerRowLine) -> String {
        switch line {
        case .off: L.progressRowOff(kind: Device.name)
        case .saved(let at): L.progressRowSaved(ago: ReadWords.ago(at))
        case .sending(let count): L.progressRowSending(count: count)
        case .offline(let count): L.progressRowOffline(count: count)
        case .retrying(let count): L.progressRowRetrying(count: count)
        case .failed(let count, let why): L.progressRowFailed(count: count, reason: reason(why))
        }
    }

    static func reason(_ why: ProgressWhyNot) -> String {
        switch why {
        case .key: L.progressWhyKey
        case .password: L.progressWhyPassword
        case .gone: L.progressWhyGone
        case .gaveUp: L.progressWhyGaveUp
        }
    }
}

/// A book server in Your sources: its tile, its name, "Kavita · 212 series in 2 libraries", and
/// what became of the progress sent to it. Try again takes the chevron's place while the line asks
/// for it; a press dims it to 38% until the engine answers. A tap anywhere else opens the server.
struct BookServerSourceRow<Health: View>: View {
    @Environment(AppStore.self) private var store
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    let server: BookServerRow
    let note: String
    /// Opens the server's page; nil on that page itself, where the row ends in no chevron.
    let open: (() -> Void)?
    @ViewBuilder let health: Health

    var body: some View {
        let line = store.serverProgress.rowLines[server.id]
        let pressed = store.serverProgress.retrying.contains(server.id)
        let retry = line?.canRetry == true
        BookSourceRow(title: server.name, note: note, open: open) {
            SourceTile(systemImage: "server.rack")
        } extra: {
            if let line {
                progress(line)
                    .id(Self.key(line))
                    .transition(.opacity)
            }
        } trailing: {
            HStack(spacing: 8) {
                health
                ZStack(alignment: .trailing) {
                    if retry {
                        Button(L.readerTryAgain) { store.serverProgress.retryRow(server.id) }
                            .buttonStyle(SecondaryCapsuleStyle(height: Device.isMac ? 28 : 36, fontSize: Device.isMac ? 13 : 15))
                            .fixedSize()
                            .opacity(pressed ? 0.38 : 1)
                            .disabled(pressed)
                            .transition(.opacity)
                    } else if open != nil {
                        RowChevron().transition(.opacity)
                    }
                }
            }
        }
        .animation(reduceMotion ? .easeInOut(duration: 0.2) : .reveal, value: line.map(Self.key))
        .animation(.quick, value: pressed)
    }

    private func progress(_ line: ServerRowLine) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 6) {
            if case .failed = line {
                Image(systemName: "exclamationmark.triangle.fill")
                    .font(.plat(mac: 10, touch: 12))
                    .foregroundStyle(.vermilion)
                    .accessibilityHidden(true)
            }
            Text(ServerProgressWords.row(line))
                .font(.plat(mac: 12, touch: 15))
                .foregroundStyle(Self.failed(line) ? .silk : .haze)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    private static func failed(_ line: ServerRowLine) -> Bool {
        if case .failed = line { true } else { false }
    }

    /// One key per kind of line: a change of kind crossfades, a count that moves does not.
    private static func key(_ line: ServerRowLine) -> String {
        switch line {
        case .off: "off" // i18n-skip: an animation key
        case .saved: "saved" // i18n-skip: an animation key
        case .sending: "sending" // i18n-skip: an animation key
        case .offline: "offline" // i18n-skip: an animation key
        case .retrying: "retrying" // i18n-skip: an animation key
        case .failed: "failed" // i18n-skip: an animation key
        }
    }
}

/// A source's row in Your sources: a tile or cover at the lead, its name and line, what else it
/// says beneath, and what it ends in. With `open`, a tap on the row opens the source's page; a
/// button at its end keeps its own press.
struct BookSourceRow<Lead: View, Extra: View, Trailing: View>: View {
    let title: String
    var note: String?
    var noteColor: Color = .haze
    let open: (() -> Void)?
    @ViewBuilder let lead: Lead
    @ViewBuilder let extra: Extra
    @ViewBuilder let trailing: Trailing

    init(title: String, note: String? = nil, noteColor: Color = .haze, open: (() -> Void)?,
         @ViewBuilder lead: () -> Lead, @ViewBuilder extra: () -> Extra = { EmptyView() },
         @ViewBuilder trailing: () -> Trailing) {
        self.title = title
        self.note = note
        self.noteColor = noteColor
        self.open = open
        self.lead = lead()
        self.extra = extra()
        self.trailing = trailing()
    }

    var body: some View {
        HStack(spacing: Device.isMac ? 12 : 14) {
            lead
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .font(.plat(mac: 13, touch: 17))
                    .foregroundStyle(.silk)
                if let note {
                    Text(note)
                        .font(.plat(mac: 12, touch: 15))
                        .foregroundStyle(noteColor)
                        .fixedSize(horizontal: false, vertical: true)
                }
                extra
            }
            .macLineHeight(16)
            Spacer(minLength: 0)
            trailing
        }
        .padding(EdgeInsets(top: Device.isMac ? 8 : 12, leading: 10, bottom: Device.isMac ? 8 : 12, trailing: 10))
        .frame(minHeight: Device.isMac ? 50 : 64)
        .contentShape(.rect)
        .onTapGesture { open?() }
        .rowHover()
        .padding(.horizontal, 4)
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(open == nil ? [] : .isButton)
        .accessibilityAction { open?() }
    }
}

/// The square at a source row's lead: its kind's glyph on the selected raised fill.
struct SourceTile: View {
    let systemImage: String

    var body: some View {
        let side: CGFloat = Device.isMac ? 28 : 36
        Image(systemName: systemImage)
            .font(.ui(Device.isMac ? 15 : 18))
            .foregroundStyle(.silk)
            .frame(width: side, height: side)
            .background(.raisedSelected, in: .rect(cornerRadius: Device.isMac ? 7 : 9))
            .accessibilityHidden(true)
    }
}

/// The chevron at the end of a row that opens a page.
struct RowChevron: View {
    var body: some View {
        Image(systemName: "chevron.right")
            .font(.ui(Device.isMac ? 12 : 14, weight: .semibold))
            .foregroundStyle(.haze)
            .opacity(0.7)
            .accessibilityHidden(true)
    }
}
