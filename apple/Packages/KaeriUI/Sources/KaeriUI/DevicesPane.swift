// Settings, Devices on the Mac and iPad: this device's name, the devices in the group, the cloud
// folder, the recovery key and Turn off sync, in the pane's cards. The iPhone has its own page
// (PhoneDevices).

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

/// The name a new device takes: the system's where it says one, else the model.
@MainActor
enum DeviceNaming {
    static var system: String {
        #if os(macOS)
        Host.current().localizedName ?? "Mac" // i18n-skip
        #else
        UIDevice.current.name
        #endif
    }
}

struct DevicesPane: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator

    var body: some View {
        let sync = store.sync
        SettingsGroup(title: L.devicesThisDevice) {
            SettingsRow(title: L.devicesName) { DeviceNameField() }
        }
        if sync.enabled {
            YourDevicesGroup()
            CloudFolderGroup()
            RecoveryKeyGroup()
            TurnOffGroup()
        } else {
            SettingsGroup(title: L.syncGroup) {
                SettingsRow(title: L.syncOffTitle, note: L.syncOffLine) {
                    Button { sync.sheet = .road } label: { Text(L.syncTurnOn + "…") }
                        .buttonStyle(PrimaryCapsuleStyle(height: 28))
                }
            }
            TVsGroup(title: L.tvPairedTvs, footnoteOverride: L.devicesPairedTvsFoot)
        }
        if sync.enabled { TVsGroup() }
        Color.clear.frame(height: 0)
            .modifier(SyncPresenter())
            .onLaunchSheet("sync-road", navigator) { sync.sheet = .road }
    }
}

/// This device's name, which the engine takes when sync is turned on.
struct DeviceNameField: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let sync = store.sync
        @Bindable var model = sync
        TextField("", text: Binding(
            get: { sync.enabled ? (sync.status.deviceName ?? sync.deviceName) : (sync.deviceName.isEmpty ? DeviceNaming.system : sync.deviceName) },
            set: { model.deviceName = $0 }
        ))
        .textFieldStyle(.plain)
        .font(.plat(mac: 13, touch: 15))
        .multilineTextAlignment(.leading)
        .padding(.horizontal, 10)
        .frame(width: 210, height: 28)
        .background(.ground, in: .rect(cornerRadius: 8))
        .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(Palette.glassRule, lineWidth: 1))
        .disabled(sync.enabled)
        .accessibilityLabel(L.devicesName)
    }
}

/// The devices in the group, a switch each, and how to add one.
struct YourDevicesGroup: View {
    @Environment(AppStore.self) private var store
    @State private var removing: SyncDeviceView?

    var body: some View {
        let sync = store.sync
        let others = sync.others
        SettingsGroup(title: L.devicesYours) {
            if others.isEmpty {
                SettingsRow(title: L.devicesOnlyThis, note: L.devicesOnlyThisLine)
            }
            ForEach(others, id: \.id) { device in
                let line = SyncWords.seen(device)
                let tv = store.tv.paired.first { $0.name == device.name }
                SettingsRow(title: device.name, note: line.text, dot: line.state) {
                    HStack(spacing: 12) {
                        Text(L.devicesSyncSwitch).font(.mac(12)).foregroundStyle(.haze).fixedSize()
                        Toggle(L.devicesSyncSwitch, isOn: Binding(get: { true }, set: { if !$0 { removing = device } }))
                            .toggleStyle(.switch).tint(.amber).labelsHidden()
                        if device.folder == nil, tv != nil || device.name.hasSuffix("TV") {
                            Text(L.devicesRemoteSwitch).font(.mac(12)).foregroundStyle(.haze).fixedSize()
                            Toggle(L.devicesRemoteSwitch, isOn: Binding(
                                get: { tv != nil },
                                set: { on in
                                    if on { store.tv.openPicker() } else if let tv { store.tv.forget(tv.fingerprint) }
                                }
                            ))
                            .toggleStyle(.switch).tint(.amber).labelsHidden()
                        }
                    }
                }
            }
            SettingsRow(title: L.devicesAdd) {
                HStack(spacing: 8) {
                    Button(L.pairShowCode) { sync.sheet = .pairShow }
                    Button(L.pairEnterCode) { sync.sheet = .pairEnter }
                }
                .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
            }
        }
        .confirmationDialog(
            L.devicesRemoveAlert(device: removing?.name ?? ""),
            isPresented: Binding(get: { removing != nil }, set: { if !$0 { removing = nil } }),
            presenting: removing
        ) { device in
            Button(L.devicesRemove, role: .destructive) { sync.remove(device.id) }
        } message: { device in
            Text(L.devicesRemoveAlertLine(device: device.name))
        }
    }
}

/// The cloud folder: the service with its state, then Sync now and Sign out; or the way to add one.
struct CloudFolderGroup: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let sync = store.sync
        SettingsGroup(title: L.cloudGroup, footnote: L.cloudFoot) {
            if let folder = sync.status.folder {
                CloudRow(folder: folder)
            } else {
                SettingsRow(title: L.cloudAdd) {
                    Button(L.actionAdd) { sync.sheet = .picker }
                        .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                }
            }
        }
    }
}

/// The cloud folder's AccountRow: the service, how it stands, and what to do about it.
struct CloudRow: View {
    @Environment(AppStore.self) private var store
    let folder: SyncFolderView

    var body: some View {
        let sync = store.sync
        let line = SyncWords.cloud(sync)
        let dimmed = sync.settingUpShown
        TimelineView(.periodic(from: .now, by: 30)) { _ in
            HStack(spacing: 16) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(SyncModel.name(folder.kind))
                        .font(.mac(13, weight: .semibold))
                        .foregroundStyle(.silk)
                    HStack(spacing: 6) {
                        StateDot(state: line.state)
                        Text(line.text).font(.mac(12)).foregroundStyle(.haze)
                        if let account = folder.account, !dimmed {
                            Text("· " + account).font(.mac(12)).foregroundStyle(.haze).lineLimit(1)
                        }
                    }
                }
                .macLineHeight(16)
                Spacer(minLength: 0)
                HStack(spacing: 8) {
                    if sync.settingUpShown {
                        ProgressView().controlSize(.small)
                    }
                    buttons(sync, state: line.state)
                }
                .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                .fixedSize()
                .disabled(dimmed)
            }
            .padding(EdgeInsets(top: 8, leading: 10, bottom: 8, trailing: 6))
            .frame(minHeight: 50)
            .padding(.horizontal, 4)
            .accessibilityElement(children: .combine)
        }
    }

    @ViewBuilder
    private func buttons(_ sync: SyncModel, state: SyncSummary.State) -> some View {
        let needsSignIn = state == .signedOut || sync.status.problem.map(Self.isSignIn) == true
        let cloud = [SyncFolderKind.googleDrive, .dropbox, .oneDrive].contains(folder.kind)
        if needsSignIn, cloud {
            Button(sync.signedOutByHand ? L.cloudSignIn : L.cloudSignInAgain) { sync.signInAgain(folder.kind) }
        } else if sync.status.problem == .unreachable, folder.kind == .directory {
            Button(L.cloudChooseFolder) { sync.sheet = .picker }
        } else {
            Button(L.cloudSyncNow) { sync.syncNow() }
            if cloud { Button(L.cloudSignOut) { sync.signOut(folder.kind) } }
        }
    }

    private static func isSignIn(_ problem: SyncProblem) -> Bool {
        if case .signInNeeded = problem { return true }
        return false
    }
}

/// The recovery key: when it was made, and Show behind the device's own lock.
struct RecoveryKeyGroup: View {
    @Environment(AppStore.self) private var store
    @State private var unlocking = false

    var body: some View {
        let sync = store.sync
        if sync.status.recoveryKey {
            SettingsGroup {
                SettingsRow(title: L.rkeyRow, note: note(sync)) {
                    Button(L.actionShow) {
                        DeviceLock.ask(reason: L.rkeyAgain(kind: Device.name)) { sync.showRecoveryKey() }
                    }
                    .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                }
            }
        }
    }

    private func note(_ sync: SyncModel) -> String {
        let made = SyncWords.made(sync.status.recoveryKeyMade).map { L.rkeyMade(date: $0) }
        return [made, L.rkeyRestores].compactMap { $0 }.joined(separator: ". ")
    }
}

/// Turn off sync on this device, with an alert.
struct TurnOffGroup: View {
    @Environment(AppStore.self) private var store
    @State private var asking = false

    var body: some View {
        let sync = store.sync
        SettingsGroup {
            SettingsRow(title: L.syncTurnOff(kind: Device.name)) {
                Button(L.syncTurnOffButton) { asking = true }
                    .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
            }
        }
        .confirmationDialog(
            L.syncTurnOffAlert(device: sync.status.deviceName ?? Device.name),
            isPresented: $asking
        ) {
            Button(L.syncTurnOffButton, role: .destructive) { sync.turnOff() }
        } message: {
            Text(L.syncTurnOffAlertLine(kind: Device.name))
        }
    }
}
