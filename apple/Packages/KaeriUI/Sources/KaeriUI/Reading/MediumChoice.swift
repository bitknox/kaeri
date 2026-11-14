// What Kaeri is for: the three rows first run, the sheets and Settings share, the sheet that asks
// after the update or when this device joins devices that answer differently, and the turn-off
// confirmation Settings shows before a medium leaves.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

/// Watch, Read and Both as one group of rows, each a single target. The Mac marks the choice
/// with a radio, touch with a check at the row's end.
struct MediumChoiceRows: View {
    @Binding var chosen: KaeriIsFor?
    /// The group's fill: raised on the ground, a step lighter on a raised sheet.
    var fill: Color = Palette.raised
    /// A faint rule round the group, where its fill is close to what is under it.
    var outlined = false

    var body: some View {
        VStack(spacing: 0) {
            ForEach(Array([KaeriIsFor.watch, .read, .both].enumerated()), id: \.offset) { index, answer in
                if index > 0 {
                    Rectangle().fill(Palette.faintRule).frame(height: 1)
                        .padding(.leading, Device.isMac ? 40 : 16)
                        .accessibilityHidden(true)
                }
                row(answer, index: index)
            }
        }
        .background(fill, in: .rect(cornerRadius: Device.isMac ? 10 : 26))
        .overlay {
            if outlined {
                RoundedRectangle(cornerRadius: Device.isMac ? 10 : 26).strokeBorder(Palette.faintRule, lineWidth: 1)
            }
        }
    }

    private func row(_ answer: KaeriIsFor, index: Int) -> some View {
        let on = chosen == answer
        return Button {
            withAnimation(.quick) { chosen = answer }
        } label: {
            HStack(alignment: .center, spacing: 12) {
                #if os(macOS)
                RadioMark(on: on)
                #endif
                VStack(alignment: .leading, spacing: 2) {
                    Text(ReadWords.medium(answer))
                        .font(.plat(mac: 13, touch: 17, weight: .semibold))
                        .foregroundStyle(.silk)
                    Text(ReadWords.mediumLine(answer))
                        .font(.plat(mac: 12, touch: 15))
                        .foregroundStyle(.haze)
                        .fixedSize(horizontal: false, vertical: true)
                }
                Spacer(minLength: 8)
                #if os(iOS)
                Image(systemName: "checkmark")
                    .font(.ui(17, weight: .semibold))
                    .foregroundStyle(.silk)
                    .opacity(on ? 1 : 0)
                    .accessibilityHidden(true)
                #endif
            }
            .padding(.horizontal, Device.isMac ? 12 : 16)
            .padding(.vertical, Device.isMac ? 10 : 12)
            .frame(minHeight: Device.isMac ? 52 : 64)
            .contentShape(.rect)
        }
        .buttonStyle(RowPressStyle())
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(on ? [.isButton, .isSelected] : .isButton)
        .accessibilityValue(L.mediumChoicePosition(n: index + 1, total: 3))
    }
}

/// A row that darkens a touch while pressed, as list rows do.
private struct RowPressStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .background(configuration.isPressed ? Palette.rowPressed : .clear)
            .pointerHover()
    }
}

#if os(macOS)
/// The Mac's radio: a ring, filled with silk and a ground dot when chosen.
private struct RadioMark: View {
    let on: Bool

    var body: some View {
        ZStack {
            Circle().strokeBorder(on ? Palette.silk : Palette.edge, lineWidth: 1.5)
            if on {
                Circle().fill(.silk).frame(width: 7, height: 7)
            }
        }
        .frame(width: 16, height: 16)
        .accessibilityHidden(true)
    }
}
#endif

// MARK: - The sheet after the update, and on joining

/// Shows the medium question when the engine asks it outside first run: once after the update,
/// with Watch chosen, or when this device joined devices that answer otherwise, with theirs
/// chosen. A first-run question first run did not take is asked here too.
struct MediumAskPresenter: ViewModifier {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator

    func body(content: Content) -> some View {
        content
            .sheet(item: Binding(
                get: { asked },
                set: { if $0 == nil, let ask = asked { answerDefault(ask.ask) } }
            )) { item in
                MediumAskSheet(ask: item.ask)
                    .interactiveDismissDisabled()
            }
    }

    /// The question, while first launch is not asking it itself.
    private var asked: AskItem? {
        guard let ask = store.medium.asking else { return nil }
        if case .firstRun = ask, !navigator.firstLaunchSettled { return nil }
        return AskItem(ask: ask)
    }

    private func answerDefault(_ ask: MediumAsk) {
        store.medium.answer(ask, with: MediumAskSheet.preset(ask, current: store.medium.answer) ?? store.medium.answer)
    }

    struct AskItem: Identifiable {
        let ask: MediumAsk
        var id: Int { 0 }
    }
}

/// "Watch, read, or both?" over Home after the update, or "Kim's iPad are set to reading" on
/// joining. Continue is the one action; nothing leaves without an answer.
struct MediumAskSheet: View {
    @Environment(AppStore.self) private var store
    let ask: MediumAsk
    @State private var chosen: KaeriIsFor?

    init(ask: MediumAsk) {
        self.ask = ask
        _chosen = State(initialValue: Self.preset(ask, current: .watch))
    }

    /// What is chosen as the sheet opens: Watch after the update, the others' answer on joining,
    /// nothing on a first run.
    static func preset(_ ask: MediumAsk, current: KaeriIsFor) -> KaeriIsFor? {
        switch ask {
        case .firstRun: nil
        case .updated: .watch
        case .joined(_, let answer): answer
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(title)
                .font(.plat(mac: 17, touch: 22, weight: Device.isMac ? .semibold : .bold))
                .foregroundStyle(.silk)
                .fixedSize(horizontal: false, vertical: true)
                .accessibilityAddTraits(.isHeader)
            Text(line)
                .font(.plat(mac: 13, touch: 17))
                .foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, Device.isMac ? 6 : 8)
            // On the Mac's raised sheet the group sits a step down with a rule round it; touch's
            // sheet takes it a step up.
            MediumChoiceRows(chosen: $chosen, fill: Device.isMac ? Palette.ground.opacity(0.3) : Palette.raisedSelected,
                             outlined: Device.isMac)
                .padding(.top, Device.isMac ? 16 : 20)
            if !Device.isMac {
                Text(L.mediumChangeLater)
                    .font(.ui(13))
                    .foregroundStyle(.haze)
                    .padding(.horizontal, 16)
                    .padding(.top, 8)
            }
            #if os(iOS)
            Spacer(minLength: 16)
            #endif
            HStack {
                if Device.isMac { Spacer() }
                Button {
                    if let chosen { store.medium.answer(ask, with: chosen) }
                } label: {
                    Text(L.onboardingContinue).touchFill()
                }
                .buttonStyle(PrimaryCapsuleStyle(height: Device.isMac ? 28 : 50, fontSize: Device.isMac ? 13 : 17, offIsRaised: true))
                .disabled(chosen == nil)
                .keyboardShortcut(.defaultAction)
            }
            .padding(.top, Device.isMac ? 20 : 22)
        }
        #if os(macOS)
        .padding(EdgeInsets(top: 26, leading: 28, bottom: 22, trailing: 28))
        .frame(width: 520)
        #else
        .padding(EdgeInsets(top: 26, leading: 20, bottom: 12, trailing: 20))
        .frame(maxWidth: 560)
        .presentationDetents([.height(560), .large])
        .presentationDragIndicator(.visible)
        #endif
        .background(.raised)
        #if os(iOS)
        .presentationBackground(.raised)
        #endif
    }

    private var title: String {
        switch ask {
        case .firstRun, .updated: L.onboardingMediumTitle
        case .joined(let devices, let answer):
            MediumWords.joinTitle(ListFormatter.localizedString(byJoining: devices), count: devices.count, answer)
        }
    }

    private var line: String {
        switch ask {
        case .firstRun: L.onboardingMediumLine
        case .updated: L.mediumUpdateLine(device: Device.name)
        case .joined: L.mediumJoinLine(device: Device.name)
        }
    }
}

// MARK: - Settings

/// Settings' choice: on at once for a medium turned on or Both, and for one turned off only after
/// the confirmation says what leaves and what stays.
@MainActor
@Observable
final class MediumSwitch {
    /// The medium about to be turned off, and the answer that follows.
    var leaving: (medium: KaeriIsFor, to: KaeriIsFor)?
    var deleteDownloads = false

    func choose(_ answer: KaeriIsFor, store: AppStore) {
        let current = store.medium.answer
        guard answer != current else { return }
        let leaves: KaeriIsFor? = switch (current, answer) {
        case (_, .both): nil
        case (_, .watch): .read
        case (_, .read): .watch
        }
        guard let leaves else {
            store.medium.choose(answer)
            return
        }
        deleteDownloads = false
        leaving = (leaves, answer)
    }

    func confirm(store: AppStore) {
        guard let leaving else { return }
        store.medium.choose(leaving.to, deleteDownloads: deleteDownloads)
        self.leaving = nil
    }
}

/// "Turn off watching?": what leaves, that the list, progress and downloads stay, and deleting
/// the downloads as its own switch, off.
struct MediumOffSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    @Bindable var model: MediumSwitch
    let medium: KaeriIsFor

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text((medium == .watch ? L.mediumOffTitleWatch : L.mediumOffTitleRead))
                .font(.plat(mac: 17, touch: 22, weight: Device.isMac ? .semibold : .bold))
                .foregroundStyle(.silk)
                .accessibilityAddTraits(.isHeader)
            Text(medium == .watch ? L.mediumOffWatchBody(device: Device.name) : L.mediumOffReadBody(device: Device.name))
                .font(.plat(mac: 13, touch: 17))
                .foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, Device.isMac ? 6 : 10)
            Toggle(isOn: $model.deleteDownloads) {
                VStack(alignment: .leading, spacing: 2) {
                    Text((medium == .watch ? L.mediumOffDeleteWatch : L.mediumOffDeleteRead))
                        .font(.plat(mac: 13, touch: 17))
                        .foregroundStyle(.silk)
                    if let size {
                        Text(L.mediumOffDeleteSize(size: size, device: Device.name))
                            .font(.plat(mac: 12, touch: 15))
                            .foregroundStyle(.haze)
                    }
                }
            }
            .toggleStyle(.switch)
            .tint(.amber)
            .padding(.horizontal, Device.isMac ? 12 : 16)
            .padding(.vertical, Device.isMac ? 8 : 12)
            .background(Palette.raisedSelected, in: .rect(cornerRadius: Device.isMac ? 10 : 26))
            .padding(.top, Device.isMac ? 16 : 20)
            #if os(macOS)
            HStack(spacing: 8) {
                Spacer()
                Button(L.actionCancel) { model.leaving = nil }
                    .buttonStyle(SecondaryCapsuleStyle(height: 28, raisedSurface: true, fontSize: 13))
                    .keyboardShortcut(.cancelAction)
                Button((medium == .watch ? L.mediumOffConfirmWatch : L.mediumOffConfirmRead)) { model.confirm(store: store) }
                    .buttonStyle(PrimaryCapsuleStyle(height: 28, fontSize: 13))
                    .keyboardShortcut(.defaultAction)
            }
            .padding(.top, 20)
            #else
            VStack(spacing: 8) {
                Button { model.confirm(store: store) } label: {
                    Text((medium == .watch ? L.mediumOffConfirmWatch : L.mediumOffConfirmRead)).touchFill()
                }
                .buttonStyle(PrimaryCapsuleStyle(height: 50, fontSize: 17))
                Button { model.leaving = nil } label: {
                    Text(L.actionCancel)
                        .font(.ui(17, weight: .semibold))
                        .foregroundStyle(.silk)
                        .frame(maxWidth: .infinity)
                        .frame(height: 44)
                        .contentShape(.rect)
                }
                .buttonStyle(.plain)
            }
            .padding(.top, 24)
            #endif
        }
        #if os(macOS)
        .padding(EdgeInsets(top: 24, leading: 28, bottom: 22, trailing: 28))
        .frame(width: 480)
        #else
        .padding(EdgeInsets(top: 22, leading: 20, bottom: 12, trailing: 20))
        .presentationDetents([.height(470)])
        .presentationDragIndicator(.visible)
        .presentationBackground(.raised)
        #endif
        .background(.raised)
    }

    /// The downloads' size on this device, once the engine has said it.
    private var size: String? {
        let bytes: UInt64 = medium == .watch
            ? store.downloads.bytesOnDisk
            : store.reading.downloads?.meter.used ?? 0
        return bytes > 0 ? Wording.bytes(bytes) : nil
    }
}

/// The confirmation over whatever shows Settings' choice.
struct MediumOffPresenter: ViewModifier {
    @Bindable var model: MediumSwitch

    func body(content: Content) -> some View {
        content.sheet(item: Binding(
            get: { model.leaving.map { LeavingItem(medium: $0.medium) } },
            set: { if $0 == nil { model.leaving = nil } }
        )) { item in
            MediumOffSheet(model: model, medium: item.medium)
        }
    }

    struct LeavingItem: Identifiable {
        let medium: KaeriIsFor
        var id: Int { 0 }
    }
}

/// The Mac's and iPad's first group in General: Kaeri is for, with its menu.
struct MediumSettingsRow: View {
    @Environment(AppStore.self) private var store
    @State private var model = MediumSwitch()

    var body: some View {
        SettingsGroup {
            SettingsRow(title: L.settingsKaeriIsFor, note: L.settingsMediumNote) {
                Menu {
                    Picker(L.settingsKaeriIsFor, selection: Binding(
                        get: { store.medium.answer },
                        set: { model.choose($0, store: store) }
                    )) {
                        ForEach([KaeriIsFor.watch, .read, .both], id: \.self) { answer in
                            Text(ReadWords.answer(answer)).tag(answer)
                        }
                    }
                    .pickerStyle(.inline)
                    .labelsHidden()
                } label: {
                    MenuCapsule(text: ReadWords.answer(store.medium.answer))
                }
                .menuStyle(.button)
                .buttonStyle(.plain)
                .menuIndicator(.hidden)
                .fixedSize()
                .accessibilityLabel(L.settingsKaeriIsFor)
            }
        }
        .modifier(MediumOffPresenter(model: model))
        .task(id: store.medium.answer) { store.loadBookDownloadsIfReading() }
    }
}

#if os(iOS)
/// The iPhone's page: the three rows with a check, and the footnote.
struct PhoneMediumPage: View {
    @Environment(AppStore.self) private var store
    @State private var model = MediumSwitch()

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                MediumChoiceRows(chosen: Binding(
                    get: { store.medium.answer },
                    set: { if let answer = $0 { model.choose(answer, store: store) } }
                ))
                Text(L.settingsMediumFooter)
                    .font(.ui(13))
                    .foregroundStyle(.haze)
                    .padding(.horizontal, 16)
                    .padding(.top, 8)
            }
            .padding(.horizontal, 16)
            .padding(.top, 8)
        }
        .background(.ground)
        .navigationTitle(L.settingsKaeriIsFor)
        .navigationBarTitleDisplayMode(.inline)
        .modifier(MediumOffPresenter(model: model))
        .task(id: store.medium.answer) { store.loadBookDownloadsIfReading() }
    }
}
#endif

/// The sentences that differ by the answer, one message each.
enum MediumWords {
    static func joinTitle(_ devices: String, count: Int, _ answer: KaeriIsFor) -> String {
        switch answer {
        case .watch: L.mediumJoinTitleWatch(devices: devices, n: count)
        case .read: L.mediumJoinTitleRead(devices: devices, n: count)
        case .both: L.mediumJoinTitleBoth(devices: devices, n: count)
        }
    }

    static func listsLine(_ answer: KaeriIsFor, tracker: String) -> String {
        switch answer {
        case .watch: L.onboardingListsLineWatch(tracker: tracker)
        case .read: L.onboardingListsLineRead(tracker: tracker)
        case .both: L.onboardingListsLineBoth(tracker: tracker)
        }
    }

    static func listsLater(_ answer: KaeriIsFor) -> String {
        switch answer {
        case .watch: L.onboardingListsLaterWatch
        case .read: L.onboardingListsLaterRead
        case .both: L.onboardingListsLaterBoth
        }
    }
}

extension AppStore {
    /// The chapter downloads' size, for the turn-off confirmation.
    func loadBookDownloadsIfReading() {
        if medium.reads { reading.loadDownloads() }
    }
}
