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

/// A book server in Your sources: its name, "Kavita · 212 series in 2 libraries", and what became
/// of the progress sent to it. Try again sits beside the line it answers; a press dims it to 38%
/// until the engine answers, and the line, the button and the menu then follow together.
struct BookServerSourceRow<Menu: View>: View {
    @Environment(AppStore.self) private var store
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    let server: BookServerRow
    let note: String
    @ViewBuilder let menu: Menu

    var body: some View {
        let line = store.serverProgress.rowLines[server.id]
        let pressed = store.serverProgress.retrying.contains(server.id)
        HStack(spacing: 16) {
            VStack(alignment: .leading, spacing: 2) {
                Text(server.name)
                    .font(.plat(mac: 13, touch: 17))
                    .foregroundStyle(.silk)
                Text(note)
                    .font(.plat(mac: 12, touch: 15))
                    .foregroundStyle(.haze)
                    .fixedSize(horizontal: false, vertical: true)
                if let line {
                    progress(line)
                        .id(Self.key(line))
                        .transition(.opacity)
                }
            }
            .macLineHeight(16)
            Spacer(minLength: 0)
            HStack(spacing: 8) {
                if let line, line.canRetry {
                    Button(L.readerTryAgain) { store.serverProgress.retryRow(server.id) }
                        .buttonStyle(SecondaryCapsuleStyle(height: Device.isMac ? 24 : 32, fontSize: Device.isMac ? 12 : 15))
                        .fixedSize()
                        .opacity(pressed ? 0.38 : 1)
                        .disabled(pressed)
                        .transition(.opacity)
                }
                menu
            }
        }
        .animation(reduceMotion ? .easeInOut(duration: 0.2) : .reveal, value: line.map(Self.key))
        .animation(.quick, value: pressed)
        .padding(EdgeInsets(top: 8, leading: 10, bottom: 8, trailing: 6))
        .frame(minHeight: 50)
        .contentShape(.rect)
        .padding(.horizontal, 4)
        .accessibilityElement(children: .combine)
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
