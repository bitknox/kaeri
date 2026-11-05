// Profiles: optional, one person by default. The Settings pane on the Mac and iPad, the sheet that
// adds or edits one, and the avatar.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

/// A profile's avatar: a round tile with the initials.
struct ProfileAvatar: View {
    let profile: ProfileView
    var size: CGFloat = 36

    var body: some View {
        Text(profile.initials)
            .font(.plat(mac: size * 0.4, touch: size * 0.4).weight(.semibold))
            .foregroundStyle(profile.active ? Palette.ground : Palette.silk)
            .frame(width: size, height: size)
            .background(profile.active ? Palette.amber : Palette.raised, in: .circle)
            .accessibilityHidden(true)
    }
}

struct ProfilesPane: View {
    @Environment(AppStore.self) private var store
    @AppStorage("askWhoAtLaunch") private var askAtLaunch = false
    @State private var deleting: ProfileView?

    var body: some View {
        let sync = store.sync
        if sync.profiles.isEmpty {
            SettingsGroup(title: L.profilesTitle, footnote: L.profilesOneFoot) {
                SettingsRow(title: L.profilesOnlyYou) {
                    Button(L.profilesAdd + "…") { sync.sheet = .profileAdd }
                        .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                }
            }
        } else {
            SettingsGroup(title: L.profilesTitle, footnote: L.profilesSharedFoot) {
                ForEach(sync.profiles, id: \.id) { profile in
                    SettingsRow(title: profile.name, note: profile.kids ? L.profilesKidsTag : nil) {
                        HStack(spacing: 8) {
                            ProfileAvatar(profile: profile, size: 28)
                            if !profile.active { Button(L.profilesSwitch) { sync.switchProfile(profile.id) } }
                            Button(L.actionEdit) { sync.sheet = .profileEdit(profile.id) }
                        }
                        .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                    }
                }
                SettingsRow(title: L.profilesAdd) {
                    Button(L.profilesAddButton) { sync.sheet = .profileAdd }
                        .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                }
            }
            #if os(iOS)
            if Device.isPad {
                SettingsGroup(footnote: L.profilesAskFoot) {
                    SettingsRow(title: L.profilesAskAtLaunch) {
                        Toggle("", isOn: $askAtLaunch).toggleStyle(.switch).tint(.amber).labelsHidden()
                    }
                }
            }
            #endif
        }
        Color.clear.frame(height: 0).modifier(SyncPresenter())
    }
}

struct ProfileSheet: View {
    @Environment(AppStore.self) private var store
    let editing: String?
    @State private var name = ""
    @State private var first = ""
    @State private var kids = false
    @State private var asking = false
    @State private var loaded = false

    var body: some View {
        let sync = store.sync
        let existing = editing.flatMap { id in sync.profiles.first { $0.id == id } }
        let firstEver = editing == nil && sync.profiles.isEmpty
        SyncSheetFrame(title: editing == nil ? L.profilesAdd : (existing?.name ?? L.profilesTitle), cancel: { sync.sheet = nil }) {
            if editing == nil { SyncNote(text: L.profilesAddLine) }
            if firstEver {
                SyncField(label: L.profilesYours, text: $first, note: L.profilesYoursNote)
            }
            SyncField(label: editing == nil ? L.profilesNew : L.devicesName, text: $name)
            SyncSection(footer: L.profilesKidsLine) {
                #if os(macOS)
                SettingsRow(title: L.profilesKids) {
                    Toggle("", isOn: $kids).toggleStyle(.switch).tint(.amber).labelsHidden()
                }
                #else
                Toggle(L.profilesKids, isOn: $kids).tint(.amber).listRowBackground(Palette.raised)
                #endif
            }
            if let existing {
                SyncSection {
                    SyncChoice(title: L.profilesDelete, lineColor: .vermilion) { asking = true }
                        .foregroundStyle(.vermilion)
                }
                .confirmationDialog(L.profilesDeleteAlert(name: existing.name), isPresented: $asking) {
                    Button(L.profilesDelete, role: .destructive) {
                        sync.deleteProfile(existing.id)
                        sync.sheet = nil
                    }
                } message: {
                    Text(L.profilesDeleteAlertLine(name: existing.name))
                }
            }
        } bottom: {
            #if os(macOS)
            SyncButton(title: L.actionCancel) { sync.sheet = nil }
            #endif
            SyncButton(title: editing == nil ? L.profilesAddButton : L.actionSave, primary: true, wide: true) {
                if let existing {
                    sync.editProfile(existing.id, name: name, kids: kids)
                } else {
                    sync.addProfile(name: name, kids: kids, firstName: firstEver ? first : nil)
                }
                sync.sheet = nil
            }
            .disabled(name.trimmingCharacters(in: .whitespaces).isEmpty || (firstEver && first.trimmingCharacters(in: .whitespaces).isEmpty))
        }
        .onAppear {
            guard !loaded, let existing else { return }
            loaded = true
            name = existing.name
            kids = existing.kids
        }
    }
}

#if os(iOS)
/// iPad: who is watching, asked at launch when Ask who's watching at launch is on and the
/// install has two or more profiles. A full-screen cover; the profile chosen opens, which is a
/// switch when it is not the one in use.
struct ProfilePickerPresenter: ViewModifier {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @AppStorage("askWhoAtLaunch") private var ask = false
    @State private var answered = false
    @State private var forced = false

    private var due: Bool {
        guard UIDevice.current.userInterfaceIdiom == .pad, !answered else { return false }
        return (ask || forced) && store.sync.hasProfiles
    }

    func body(content: Content) -> some View {
        content
            .task {
                // i18n-skip: a shot's sheet name
                if navigator.launchSheet == "profile-picker" { forced = true; navigator.launchSheet = nil }
            }
            .fullScreenCover(isPresented: Binding(get: { due }, set: { if !$0 { answered = true } })) {
                ProfilePicker { profile in
                    if !profile.active { store.sync.switchProfile(profile.id) }
                    answered = true
                }
                .environment(store)
                .transaction { if reduceMotion { $0.animation = nil } }
            }
    }
}

struct ProfilePicker: View {
    @Environment(AppStore.self) private var store
    let choose: (ProfileView) -> Void

    var body: some View {
        VStack(spacing: 32) {
            Spacer()
            Text(L.profilesPicker)
                .font(.ui(34, weight: .bold))
                .foregroundStyle(.silk)
                .accessibilityAddTraits(.isHeader)
            HStack(alignment: .top, spacing: 40) {
                ForEach(store.sync.profiles, id: \.id) { profile in
                    Button { choose(profile) } label: {
                        VStack(spacing: 10) {
                            Text(profile.initials)
                                .font(.ui(48, weight: .bold))
                                .foregroundStyle(.silk)
                                .frame(width: 120, height: 120)
                                .background(Palette.selection, in: .circle)
                            Text(profile.name).font(.ui(20, weight: .semibold)).foregroundStyle(.silk)
                            if profile.kids { Text(L.profilesKidsTag).font(.ui(15)).foregroundStyle(.haze) }
                        }
                        .contentShape(.rect)
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(profile.name)
                }
            }
            Text(L.profilesPickerLine)
                .font(.ui(15))
                .foregroundStyle(.haze)
                .multilineTextAlignment(.center)
            Spacer()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Palette.ground.ignoresSafeArea())
    }
}
#endif
