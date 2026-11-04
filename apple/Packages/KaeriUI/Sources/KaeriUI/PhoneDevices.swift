// Settings on iPhone: the Devices page (sync, the devices in the group, the cloud folder, the
// recovery key) and the Profiles page. The sheets they open are SyncSheets'.

#if os(iOS)
import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

struct PhoneDevicesPage: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @State private var removing: SyncDeviceView?
    @State private var turningOff = false

    var body: some View {
        let sync = store.sync
        PhoneList {
            Section {
                HStack {
                    Text(L.devicesName).font(.ui(17)).foregroundStyle(.silk)
                    Spacer()
                    DevicesNameText()
                }
                .listRowBackground(Palette.raised)
            } header: { PhoneSectionHeader(L.devicesThisDevice) }
            if sync.enabled {
                enabled(sync)
            } else {
                Section {
                    VStack(alignment: .leading, spacing: 10) {
                        PhoneRowTitle(title: L.syncOffTitle, note: L.syncOffLine)
                        Button(L.syncTurnOn + "…") { sync.sheet = .road }
                            .buttonStyle(SyncButtonStyle2())
                    }
                    .listRowBackground(Palette.raised)
                } header: { PhoneSectionHeader(L.syncGroup) }
                if store.tv.available {
                    NavigationLink(value: PhoneSettingsPage.tvs) {
                        PhoneRowTitle(title: L.tvPairedTvs, note: L.devicesPairedTvsFoot)
                    }
                    .listRowBackground(Palette.raised)
                }
            }
        }
        .navigationTitle(L.devicesTitle)
        .navigationBarTitleDisplayMode(.inline)
        .modifier(SyncPresenter())
        .onLaunchSheet("sync-road", navigator) { sync.sheet = .road }
    }

    @ViewBuilder
    private func enabled(_ sync: SyncModel) -> some View {
        Section {
            if sync.others.isEmpty { PhoneRowTitle(title: L.devicesOnlyThis, note: L.devicesOnlyThisLine).listRowBackground(Palette.raised) }
            ForEach(sync.others, id: \.id) { device in
                let line = SyncWords.seen(device)
                Toggle(isOn: Binding(get: { true }, set: { if !$0 { removing = device } })) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(device.name).font(.ui(17)).foregroundStyle(.silk)
                        HStack(spacing: 6) { StateDot(state: line.state); Text(line.text) }
                            .font(.ui(13)).foregroundStyle(.haze)
                    }
                }
                .tint(.amber)
                .listRowBackground(Palette.raised)
            }
            ForEach(store.tv.paired.filter { tv in !sync.devices.contains { $0.name == tv.name } }, id: \.fingerprint) { tv in
                HStack {
                    PhoneRowTitle(title: tv.name, note: L.devicesRemoteOnly)
                    Spacer()
                    Button(L.devicesAlsoSyncShort) { sync.alsoSync(with: tv.name, address: tv.address) }
                        .buttonStyle(SyncButtonStyle2())
                        .accessibilityLabel(L.devicesAlsoSync(device: tv.name))
                }
                .listRowBackground(Palette.raised)
            }
            Button(L.pairShowCode) { sync.sheet = .pairShow }.listRowBackground(Palette.raised)
            Button(L.pairEnterCode) { sync.sheet = .pairEnter }.listRowBackground(Palette.raised)
        } header: { PhoneSectionHeader(L.devicesYours) } footer: {
            if let tv = store.tv.paired.first(where: { tv in !sync.devices.contains { $0.name == tv.name } }) { Text(L.pairAlsoSyncFoot(device: tv.name)) }
        }
        .confirmationDialog(
            L.devicesRemoveAlert(device: removing?.name ?? ""),
            isPresented: Binding(get: { removing != nil }, set: { if !$0 { removing = nil } }),
            titleVisibility: .visible, presenting: removing
        ) { device in
            Button(L.devicesRemove, role: .destructive) { sync.remove(device.id) }
        } message: { device in
            Text(L.devicesRemoveAlertLine(device: device.name))
        }
        if !sync.notes.isEmpty {
            Section {
                ForEach(sync.notes, id: \.id) { note in
                    VStack(alignment: .leading, spacing: 8) {
                        PhoneRowTitle(title: SyncWords.note(note, store: store))
                        Button(L.syncNoteUndo) { sync.undo(note) }
                            .buttonStyle(.borderless).font(.ui(17, weight: .semibold))
                    }
                    .listRowBackground(Palette.raised)
                }
                .task(id: sync.notes.map(\.anilistId)) { store.prepareCards(sync.notes.map(\.anilistId)) }
            } header: { PhoneSectionHeader(L.syncNotes) } footer: { Text(L.syncNotesFoot) }
        }
        Section {
            if let folder = sync.status.folder {
                let line = SyncWords.cloud(sync)
                VStack(alignment: .leading, spacing: 4) {
                    Text(SyncModel.name(folder.kind)).font(.ui(17, weight: .semibold)).foregroundStyle(.silk)
                    HStack(spacing: 6) {
                        StateDot(state: line.state)
                        Text(line.text)
                        if let account = folder.account { Text("· " + account).lineLimit(1) }
                    }
                    .font(.ui(13)).foregroundStyle(.haze)
                }
                .listRowBackground(Palette.raised)
                Button(L.cloudSyncNow) { sync.syncNow() }.listRowBackground(Palette.raised)
                if [SyncFolderKind.googleDrive, .dropbox, .oneDrive].contains(folder.kind) {
                    if line.state == .signedOut || line.state == .problem {
                        Button(sync.signedOutByHand ? L.cloudSignIn : L.cloudSignInAgain) { sync.signInAgain(folder.kind) }
                            .listRowBackground(Palette.raised)
                    } else {
                        Button(L.cloudSignOut) { sync.signOut(folder.kind) }.listRowBackground(Palette.raised)
                    }
                }
            } else {
                Button(L.cloudAdd) { sync.sheet = .picker }.listRowBackground(Palette.raised)
            }
        } header: { PhoneSectionHeader(L.cloudGroup) } footer: { Text(L.cloudFoot) }
        if sync.status.recoveryKey {
            Section {
                HStack {
                    PhoneRowTitle(title: L.rkeyRow, note: [SyncWords.made(sync.status.recoveryKeyMade).map { L.rkeyMade(date: $0) }, L.rkeyRestores].compactMap { $0 }.joined(separator: ". "))
                    Spacer()
                    Button(L.actionShow) {
                        DeviceLock.ask(reason: L.rkeyAgain(kind: Device.name)) { sync.showRecoveryKey() }
                    }
                    .buttonStyle(.borderless).font(.ui(17, weight: .semibold))
                }
                .listRowBackground(Palette.raised)
            }
        }
        Section {
            Button(L.syncTurnOff(kind: Device.name), role: .destructive) { turningOff = true }
                .foregroundStyle(.vermilion)
                .listRowBackground(Palette.raised)
        }
        .confirmationDialog(L.syncTurnOffAlert(device: sync.status.deviceName ?? Device.name), isPresented: $turningOff, titleVisibility: .visible) {
            Button(L.syncTurnOffButton, role: .destructive) { sync.turnOff() }
        } message: {
            Text(L.syncTurnOffAlertLine(kind: Device.name))
        }
        if store.tv.available {
            Section {
                NavigationLink(value: PhoneSettingsPage.tvs) {
                    PhoneRowTitle(title: L.tvPairedTvs)
                }
                .listRowBackground(Palette.raised)
            }
        }
    }
}

/// This device's name: what the group knows it by, or what it will take.
private struct DevicesNameText: View {
    @Environment(AppStore.self) private var store
    var body: some View {
        let sync = store.sync
        Text(sync.status.deviceName ?? (sync.deviceName.isEmpty ? DeviceNaming.system : sync.deviceName))
            .font(.ui(17)).foregroundStyle(.haze)
    }
}

private struct SyncButtonStyle2: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        PrimaryCapsuleStyle(height: 44, fontSize: 17).makeBody(configuration: configuration)
    }
}

struct PhoneProfilesPage: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let sync = store.sync
        PhoneList {
            if sync.profiles.isEmpty {
                Section {
                    PhoneRowTitle(title: L.profilesOnlyYou).listRowBackground(Palette.raised)
                    Button(L.profilesAdd + "…") { sync.sheet = .profileAdd }.listRowBackground(Palette.raised)
                } footer: { Text(L.profilesOneFoot) }
            } else {
                Section {
                    ForEach(sync.profiles, id: \.id) { profile in
                        Button {
                            sync.sheet = .profileEdit(profile.id)
                        } label: {
                            HStack(spacing: 14) {
                                ProfileAvatar(profile: profile, size: 40)
                                PhoneRowTitle(title: profile.name, note: profile.kids ? L.profilesKidsTag : nil)
                                Spacer()
                                if profile.active { Image(systemName: "checkmark").foregroundStyle(.amber) }
                            }
                        }
                        .swipeActions(edge: .leading) {
                            if !profile.active { Button(L.profilesSwitch) { sync.switchProfile(profile.id) }.tint(.amber) }
                        }
                        .listRowBackground(Palette.raised)
                    }
                    Button(L.profilesAdd) { sync.sheet = .profileAdd }.listRowBackground(Palette.raised)
                } footer: { Text(L.profilesSharedFoot) }
            }
        }
        .navigationTitle(L.profilesTitle)
        .navigationBarTitleDisplayMode(.inline)
        .modifier(SyncPresenter())
    }
}
#endif
