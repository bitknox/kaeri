// The parts the sync sheets are made of, once for the three Apple devices: on iPhone and iPad a
// grouped list in a navigation bar, on the Mac the raised sheet with its card of rows and buttons
// at the foot.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import LocalAuthentication
import SwiftUI

/// A sheet: its title, its rows, and the buttons that end it.
struct SyncSheetFrame<Content: View, Bottom: View>: View {
    let title: String
    /// The cancel in the navigation bar on iPhone and iPad.
    var cancel: (() -> Void)?
    var width: CGFloat = 480
    @ViewBuilder let content: Content
    @ViewBuilder let bottom: Bottom

    var body: some View {
        #if os(macOS)
        VStack(alignment: .leading, spacing: 0) {
            Text(title)
                .font(.mac(15, weight: .semibold))
                .macLineHeight(20)
                .foregroundStyle(.silk)
                .accessibilityAddTraits(.isHeader)
            VStack(alignment: .leading, spacing: 16) { content }
                .padding(.top, 14)
            HStack(spacing: 10) {
                Spacer()
                bottom
            }
            .padding(.top, 20)
        }
        .padding(30)
        .frame(width: width)
        .background(.raised)
        #else
        NavigationStack {
            PhoneList { content }
                .navigationTitle(title)
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    if let cancel {
                        ToolbarItem(placement: .topBarLeading) {
                            Button(L.actionCancel, action: cancel).tint(.silk)
                        }
                    }
                }
                .safeAreaInset(edge: .bottom) {
                    HStack(spacing: 10) { bottom }
                        .padding(.horizontal, 16)
                        .padding(.vertical, 8)
                }
        }
        .tint(.silk)
        .preferredColorScheme(.dark)
        #endif
    }
}

extension SyncSheetFrame where Bottom == EmptyView {
    init(title: String, cancel: (() -> Void)? = nil, width: CGFloat = 480, @ViewBuilder content: () -> Content) {
        self.init(title: title, cancel: cancel, width: width, content: content, bottom: { EmptyView() })
    }
}

/// A group of rows under a heading: a section of the list, or a card.
struct SyncSection<Content: View>: View {
    var header: String?
    var footer: String?
    @ViewBuilder let content: Content

    var body: some View {
        #if os(macOS)
        SettingsGroup(title: header, footnote: footer) { content }
        #else
        Section {
            content
        } header: {
            if let header { PhoneSectionHeader(header) }
        } footer: {
            if let footer { Text(footer) }
        }
        #endif
    }
}

/// A row that does something: its title, a line beneath, and a chevron where it opens a step.
struct SyncChoice: View {
    let title: String
    var line: String?
    var lineColor: Color = .haze
    var glyph: String?
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            #if os(macOS)
            HStack(spacing: 12) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(title).font(.mac(13)).foregroundStyle(.silk)
                    if let line { Text(line).font(.mac(12)).foregroundStyle(lineColor).fixedSize(horizontal: false, vertical: true) }
                }
                .macLineHeight(16)
                Spacer(minLength: 0)
            }
            .padding(EdgeInsets(top: 6, leading: 10, bottom: 6, trailing: 6))
            .frame(minHeight: 44, alignment: .leading)
            .padding(.horizontal, 4)
            .contentShape(.rect)
            #else
            HStack(spacing: 14) {
                if let glyph {
                    Image(systemName: glyph).font(.ui(18)).foregroundStyle(.haze).frame(width: 24)
                        .accessibilityHidden(true)
                }
                VStack(alignment: .leading, spacing: 2) {
                    Text(title).font(.ui(17)).foregroundStyle(.silk)
                    if let line { Text(line).font(.ui(13)).foregroundStyle(lineColor).fixedSize(horizontal: false, vertical: true) }
                }
                Spacer(minLength: 0)
            }
            .padding(.vertical, 4)
            .contentShape(.rect)
            #endif
        }
        .buttonStyle(.plain)
        .pointerHover()
        .listRowBackground(Palette.raised)
    }
}

/// A line of plain text in a sheet: a note or an error.
struct SyncNote: View {
    let text: String
    var color: Color = .haze

    var body: some View {
        Text(text)
            #if os(macOS)
            .font(.mac(12)).macLineHeight(17)
            #else
            .font(.ui(13))
            #endif
            .foregroundStyle(color)
            .fixedSize(horizontal: false, vertical: true)
            .frame(maxWidth: .infinity, alignment: .leading)
            #if os(iOS)
            .listRowBackground(Color.clear)
            .listRowInsets(EdgeInsets(top: 4, leading: 16, bottom: 4, trailing: 16))
            #endif
    }
}

/// A field with its label above, on the ground like the proxy form's.
struct SyncField: View {
    let label: String
    @Binding var text: String
    var prompt = ""
    var secure = false
    var invalid = false
    var note: String?
    var contentType: SyncFieldKind = .text

    enum SyncFieldKind { case text, url, number }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(label)
                .font(.plat(mac: 12, touch: 13))
                .foregroundStyle(.haze)
            Group {
                if secure {
                    SecureField("", text: $text, prompt: Text(prompt))
                } else {
                    TextField("", text: $text, prompt: Text(prompt))
                        #if os(iOS)
                        .keyboardType(contentType == .url ? .URL : .default)
                        .textInputAutocapitalization(.never)
                        #endif
                }
            }
            .autocorrectionDisabled()
            .textFieldStyle(.plain)
            .font(.plat(mac: 13, touch: 17))
            .padding(.horizontal, 12)
            .frame(height: touchHeight)
            .background(.ground, in: .rect(cornerRadius: 8))
            .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(invalid ? Palette.vermilion : Palette.glassRule, lineWidth: 1))
            if let note {
                Text(note)
                    .font(.plat(mac: 12, touch: 13))
                    .foregroundStyle(invalid ? .silk : .haze)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        #if os(iOS)
        .listRowBackground(Color.clear)
        .listRowInsets(EdgeInsets(top: 8, leading: 16, bottom: 8, trailing: 16))
        #endif
    }

    private var touchHeight: CGFloat {
        #if os(macOS)
        30
        #else
        48
        #endif
    }
}

/// An error under a field: a vermilion glyph and the words in silk, as the board draws it.
struct SyncProblemLine: View {
    let text: String

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 6) {
            Image(systemName: "exclamationmark.circle.fill")
                .font(.plat(mac: 12, touch: 13))
                .foregroundStyle(.vermilion)
                .accessibilityHidden(true)
            Text(text)
                .font(.plat(mac: 12, touch: 13))
                .foregroundStyle(.silk)
                .fixedSize(horizontal: false, vertical: true)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        #if os(iOS)
        .listRowBackground(Color.clear)
        .listRowInsets(EdgeInsets(top: 4, leading: 16, bottom: 4, trailing: 16))
        #endif
    }
}

/// A button of the sheet's foot: primary in amber, or secondary.
struct SyncButton: View {
    let title: String
    var primary = false
    var wide = false
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Text(title).frame(maxWidth: wide ? .infinity : nil)
        }
        .buttonStyle(SyncButtonStyle(primary: primary))
    }
}

private struct SyncButtonStyle: ButtonStyle {
    let primary: Bool

    func makeBody(configuration: Configuration) -> some View {
        if primary {
            PrimaryCapsuleStyle(height: height, fontSize: font).makeBody(configuration: configuration)
        } else {
            SecondaryCapsuleStyle(height: height, raisedSurface: true, fontSize: font).makeBody(configuration: configuration)
        }
    }

    private var height: CGFloat {
        #if os(macOS)
        28
        #else
        44
        #endif
    }

    private var font: CGFloat {
        #if os(macOS)
        13
        #else
        17
        #endif
    }
}

/// The device's own lock, asked before the recovery key shows again. The Fixtures app and the dev
/// harness never ask.
@MainActor
enum DeviceLock {
    static func ask(reason: String, then granted: @escaping @MainActor () -> Void) {
        // Only the real app asks: the Fixtures app and the dev harness have no lock to ask.
        guard Bundle.main.bundleIdentifier == "app.kaeri.app" else {
            granted()
            return
        }
        let context = LAContext()
        var error: NSError?
        guard context.canEvaluatePolicy(.deviceOwnerAuthentication, error: &error) else {
            granted()
            return
        }
        context.evaluatePolicy(.deviceOwnerAuthentication, localizedReason: reason) { success, _ in
            if success { Task { @MainActor in granted() } }
        }
    }
}
