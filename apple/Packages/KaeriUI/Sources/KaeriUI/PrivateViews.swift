// Private watching and reading: the switch, the badge and its popover, the line under a title's
// main button, and the Always private switch of a show or series.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

extension AppStore {
    /// The private mode switch's binding: from a switch, so no notice.
    var privateSwitch: Binding<Bool> {
        Binding(get: { self.privacy.on }, set: { self.setPrivate($0) })
    }

    /// A show's Always private binding.
    func alwaysPrivate(show id: AnilistId) -> Binding<Bool> {
        Binding(get: { self.privacy.isAlways(show: id) }, set: { self.setAlwaysPrivate(show: id, $0) })
    }

    /// A series' Always private binding.
    func alwaysPrivate(series work: WorkId) -> Binding<Bool> {
        Binding(get: { self.privacy.isAlways(series: work) }, set: { self.setAlwaysPrivate(series: work, $0) })
    }
}

/// The shortcut as the Mac's menu shows it, for the hint under the switch.
enum PrivateShortcut {
    static let keys = "⇧⌘N" // i18n-skip: key symbols
}

// MARK: - The line under the main button

/// "Private · Not saved or synced" under a title or series page's main button: the eye, the
/// reason in silk, and what it means in haze.
struct PrivateLineView: View {
    let line: PrivateLine

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Image(systemName: "eye.slash")
                .font(Self.font(weight: .medium))
                .foregroundStyle(.silk)
                .accessibilityHidden(true)
            (Text(head).fontWeight(.semibold).foregroundStyle(.silk)
                + Text(" · ").foregroundStyle(.haze) // i18n-skip: a separator
                + Text(tail).foregroundStyle(.haze))
                .font(Self.font())
                .lineLimit(2)
        }
        .padding(.top, 10)
        .accessibilityElement(children: .combine)
        .accessibilityLabel(head + ", " + tail) // i18n-skip: a pause between the two
    }

    var head: String { line.mode == .always ? L.titleAlwaysPrivate : L.privateBadge }
    var tail: String { line.mode == .session && line.kept ? L.privateLineKept : L.privateLineNotSaved }

    /// The words fade in a little after their room starts to open, and fade out as it closes.
    static func transition(reduceMotion: Bool) -> AnyTransition {
        .asymmetric(
            insertion: .opacity.animation(reduceMotion ? .reveal : .fadeIn.delay(0.1)),
            removal: .opacity.animation(.fadeOut))
    }

    /// 13 on the Mac, 15 on iPhone and iPad.
    static func font(weight: Font.Weight = .regular) -> Font {
        #if os(macOS)
        .mac(13, weight: weight)
        #else
        .ui(15, weight: weight)
        #endif
    }
}

/// The line's slot: it grows on standard while its words fade in a little after, and shrinks
/// away the same way. Reduce Motion jumps the room and fades the words.
struct PrivateLineSlot: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    let line: PrivateLine?

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            if let line {
                PrivateLineView(line: line)
                    .transition(PrivateLineView.transition(reduceMotion: reduceMotion))
            }
        }
        .animation(reduceMotion ? nil : .standard, value: line)
    }
}

// MARK: - The badge

/// The badge's face: the eye and "Private".
struct PrivateBadgeLabel: View {
    var size: CGFloat = 12
    var weight: Font.Weight = .medium

    var body: some View {
        HStack(spacing: 6) {
            Image(systemName: "eye.slash")
                .font(.mac(size + 1, weight: weight))
            Text(L.privateBadge)
                .font(.mac(size, weight: weight))
        }
        .foregroundStyle(.silk)
    }
}

/// What the badge opens on the Mac and iPad: the notice's words and Turn off. The Mac's also
/// keeps the sync status the capsule showed before, under a rule.
struct PrivatePopover: View {
    @Environment(AppStore.self) private var store
    /// Close the popover.
    let close: () -> Void

    var body: some View {
        #if os(macOS)
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 10) {
                Image(systemName: "eye.slash").font(.mac(14, weight: .medium))
                Text(L.privateMode).font(.mac(13, weight: .semibold))
            }
            .foregroundStyle(.silk)
            Text(L.privateModeOnNotice)
                .font(.mac(12))
                .macLineHeight(17)
                .foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, 6)
            Rectangle().fill(.faintRule).frame(height: 1).padding(.top, 12)
            HStack(spacing: 6) {
                StateDot(state: store.accounts.summary.state)
                Text(store.accounts.summary.text)
                    .font(.mac(12))
                    .foregroundStyle(.haze)
            }
            .padding(.top, 10)
            .accessibilityElement(children: .combine)
            HStack {
                Spacer()
                Button(L.actionTurnOff) {
                    close()
                    store.setPrivate(false)
                }
                .buttonStyle(SecondaryCapsuleStyle(height: 28, fontSize: 12))
            }
            .padding(.top, 12)
        }
        .padding(16)
        .frame(width: 330, alignment: .leading)
        #else
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 10) {
                Image(systemName: "eye.slash").font(.ui(17, weight: .semibold))
                Text(L.privateMode).font(.ui(17, weight: .semibold))
            }
            .foregroundStyle(.silk)
            Text(L.privateModeOnNotice)
                .font(.ui(15))
                .foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, 6)
            Button {
                close()
                store.setPrivate(false)
            } label: {
                Text(L.actionTurnOff).frame(maxWidth: .infinity)
            }
            .buttonStyle(SecondaryCapsuleStyle(height: 44, fontSize: 17))
            .padding(.top, 14)
        }
        .padding(18)
        .frame(width: 340, alignment: .leading)
        .presentationCompactAdaptation(.popover)
        #endif
    }
}

#if os(iOS)
/// The badge on iPhone and iPad, a glass capsule in the toolbar while private mode is on. On
/// iPhone it opens Settings at the switch; on iPad, the popover.
struct PrivateToolbarBadge: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @State private var showing = false

    var body: some View {
        Button {
            if Device.isPad { showing = true } else { navigator.phoneSettings = .root }
        } label: {
            HStack(spacing: 6) {
                Image(systemName: "eye.slash").font(.ui(15, weight: .semibold))
                Text(L.privateBadge).font(.ui(15, weight: .semibold))
            }
            .foregroundStyle(.silk)
            .padding(.horizontal, 4)
            .fixedSize()
        }
        .accessibilityLabel(L.privateBadgeLabel)
        .accessibilityHint(L.privateModeOnNotice)
        .popover(isPresented: $showing, arrowEdge: .top) {
            PrivatePopover { showing = false }
        }
        .onAppear {
            // `sheet=private-badge`: the popover open, for its shot.
            guard Device.isPad, navigator.launchSheet == "private-badge" else { return }
            navigator.launchSheet = nil
            showing = true
        }
    }
}
#endif

// MARK: - The switches

/// Private mode's switch on the Mac and iPad, first in Settings › Accounts: its description in
/// the row, and under the group what it does not hide and, on the Mac, its shortcut.
struct PrivateModeGroup: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            SettingsGroup {
                HStack(spacing: 16) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(L.privateMode)
                            .font(.mac(13, weight: .semibold))
                            .foregroundStyle(.silk)
                        Text(L.privateModeDetail)
                            .font(.mac(12))
                            .foregroundStyle(.haze)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    .macLineHeight(17)
                    Spacer(minLength: 0)
                    Toggle(L.privateMode, isOn: store.privateSwitch)
                        .toggleStyle(.switch)
                        .tint(.amber)
                        .labelsHidden()
                }
                .padding(EdgeInsets(top: 8, leading: 14, bottom: 8, trailing: 10))
                .frame(minHeight: 50)
                .accessibilityElement(children: .combine)
            }
            SettingsFootnote(L.privateModeLimits)
            #if os(macOS)
            SettingsFootnote(L.privateModeShortcutHint(keys: PrivateShortcut.keys))
                .padding(.top, -2)
            #endif
        }
    }
}

/// A show's or series' Always private switch on the Mac, in a card at the top of its settings.
struct AlwaysPrivateCard: View {
    let isOn: Binding<Bool>
    let detail: String

    var body: some View {
        HStack(spacing: 16) {
            VStack(alignment: .leading, spacing: 2) {
                Text(L.titleAlwaysPrivate)
                    .font(.mac(13, weight: .semibold))
                    .foregroundStyle(.silk)
                Text(detail)
                    .font(.mac(12))
                    .macLineHeight(17)
                    .foregroundStyle(.haze)
                    .fixedSize(horizontal: false, vertical: true)
            }
            Spacer(minLength: 0)
            Toggle(L.titleAlwaysPrivate, isOn: isOn)
                .toggleStyle(.switch)
                .tint(.amber)
                .labelsHidden()
        }
        .padding(EdgeInsets(top: 12, leading: 14, bottom: 12, trailing: 12))
        .background(Palette.ground.opacity(0.4), in: .rect(cornerRadius: 10))
        .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(Palette.faintRule, lineWidth: 1))
        .accessibilityElement(children: .combine)
    }
}

#if os(iOS)
/// A switch row with the eye before its title, in an iPhone or iPad list: private mode in
/// Settings, Always private in a show's or series' settings.
struct PhonePrivateSwitch: View {
    let title: String
    let isOn: Binding<Bool>

    var body: some View {
        Toggle(isOn: isOn) {
            Label {
                Text(title).font(.ui(17)).foregroundStyle(.silk)
            } icon: {
                Image(systemName: "eye.slash").foregroundStyle(.silk)
            }
        }
        .tint(.amber)
        .listRowBackground(Palette.raised)
    }
}
#endif
