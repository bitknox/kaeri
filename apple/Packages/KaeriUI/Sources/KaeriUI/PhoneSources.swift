// Providers and plugin stores on iPhone, as pages of the Settings sheet, and the sheets for
// joining a store and turning torrents on.

#if os(iOS)
import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

// MARK: - Providers

extension View {
    /// "Update Mukashi Archive?" before a plugin's update installs: the version it replaces and
    /// the hosts it adds, as the PhoneStore board asks (F19).
    func pluginUpdateAlert(isPresented: Binding<Bool>, store: StoreRow, plugin: StorePluginRow) -> some View {
        modifier(PluginUpdateAlert(isPresented: isPresented, store: store, plugin: plugin))
    }
}

private struct PluginUpdateAlert: ViewModifier {
    @Environment(AppStore.self) private var appStore
    @Binding var isPresented: Bool
    let store: StoreRow
    let plugin: StorePluginRow

    func body(content: Content) -> some View {
        content.alert(L.sourcesUpdateTitle(name: plugin.name), isPresented: $isPresented) {
            Button(L.actionUpdate) { appStore.installPlugin(plugin.id, from: store.id) }
            Button(L.actionCancel, role: .cancel) {}
        } message: {
            Text(message)
        }
    }

    private var message: String {
        guard case .update(let from, let added) = plugin.status else { return "" }
        let hosts = added.formatted(.list(type: .and).locale(AirTime.locale))
        let replaces = L.sourcesUpdateDetail(version: plugin.version, store: store.name, from: from)
        return added.isEmpty ? replaces : replaces + " " + L.storesUpdateContacts(hosts: hosts)
    }
}

/// The sources in failover order, each with its switch; Edit to drag them into another order.
struct PhoneProvidersPage: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator

    var body: some View {
        let rows = store.orderedProviders
        PhoneList {
            if store.hasNoSources {
                NoSources()
                    .listRowBackground(Color.clear)
                    .listRowInsets(EdgeInsets())
            }
            PhoneFirstPartySections()
            let sections = store.sourceSections(rows)
            ForEach(Array(sections.enumerated()), id: \.element.title) { position, section in
                Section {
                    let movable = section.rows.filter { !$0.isDemo }
                    ForEach(movable, id: \.id) { row in
                        PhoneProviderRow(info: store.info(row))
                    }
                    .onMove { from, to in
                        guard let index = from.first else { return }
                        // `to` counts the row's old place; a move down lands one before it.
                        let target = to > index ? to - 1 : to
                        guard let global = rows.firstIndex(where: { $0.id == movable[target].id }) else { return }
                        store.moveSource(movable[index].id, to: global)
                    }
                    // The demo is last and stays last: no handle, and a page of its own.
                    ForEach(section.rows.filter(\.isDemo), id: \.id) { row in
                        NavigationLink { PhoneDemoPage() } label: {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(L.sourcesDemoName).font(.ui(17)).foregroundStyle(.silk)
                                Text(L.sourcesDemoLine).font(.ui(15)).foregroundStyle(.haze)
                            }
                        }
                        .listRowBackground(Palette.raised)
                        .moveDisabled(true)
                    }
                } header: {
                    PhoneSectionHeader(section.title)
                } footer: {
                    if position == sections.count - 1 { footer }
                }
            }
            if sections.isEmpty {
                Section {} footer: { footer }
            }
        }
        .navigationTitle(L.sourcesTitle)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) { AddSourceMenu(height: 32, fontSize: 15) }
            ToolbarItem(placement: .topBarTrailing) { EditButton() }
        }
        .sheet(isPresented: Bindable(navigator).addServerShown) { AddServerSheet() }
        .task {
            store.checkProviders()
            if !store.stores.loaded { store.loadStores() }
        }
    }
}

extension PhoneProvidersPage {
    fileprivate var footer: some View {
        VStack(alignment: .leading, spacing: 8) {
            if store.demo.state.only { Text(L.sourcesDemoOnly) }
            Text(L.sourcesOrderDetailEdit)
        }
    }
}

/// About the demo, on its own page on the phone.
struct PhoneDemoPage: View {
    var body: some View {
        ScrollView {
            DemoAbout(size: 15)
                .padding(20)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
        .background(.ground)
        .navigationTitle(L.sourcesDemoName)
        .navigationBarTitleDisplayMode(.inline)
    }
}

/// One source: its name, where it came from and how it stands, and its switch.
struct PhoneProviderRow: View {
    @Environment(AppStore.self) private var store
    let info: ProviderInfo
    var body: some View {
        HStack {
            if info.row.kind == "demo" {
                label
            } else {
                NavigationLink(value: PhoneSettingsPage.plugin(info.row.id)) { label }
            }
            // Off without removing it: its source leaves the chain until it is on again.
            Toggle(info.name, isOn: Binding(
                get: { !store.isPluginDisabled(info.row.id) },
                set: { store.setPluginDisabled(info.row.id, !$0) }
            ))
            .labelsHidden()
            .tint(.amber)
        }
        .listRowBackground(Palette.raised)
    }

    private var label: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(info.name).font(.ui(17)).foregroundStyle(.silk)
            HStack(spacing: 6) {
                Text(info.origin).foregroundStyle(.haze)
                ProviderStandingLabel(info: info, size: 15)
            }
            .font(.ui(15))
            if info.row.torrents {
                Text(L.sourcesPlaysTorrents).font(.ui(15)).foregroundStyle(.haze)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

// MARK: - Plugin stores

struct PhoneStoresPage: View {
    @Environment(AppStore.self) private var store
    @State private var joining = false

    var body: some View {
        PhoneList {
            Section {
                ForEach(store.stores.stores, id: \.id) { row in
                    NavigationLink(value: PhoneSettingsPage.store(row.id)) {
                        HStack {
                            Text(row.name).font(.ui(17)).foregroundStyle(.silk)
                            Spacer()
                            StoreStanding(row: row, size: 17)
                        }
                    }
                    .listRowBackground(Palette.raised)
                }
            } footer: {
                Text(L.storesEmptyDetail)
            }
        }
        .navigationTitle(L.navPluginStores)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) { Button(L.storesJoinButton) { joining = true } }
        }
        .sheet(isPresented: $joining) { PhoneJoinStoreSheet(link: "") }
        .task { store.loadStores() }
    }
}

/// One store: its plugins with their actions, another device, leaving, and the key it was joined by.
struct PhoneStorePage: View {
    @Environment(AppStore.self) private var appStore
    @Environment(\.dismiss) private var dismiss
    let id: String
    @State private var leaving = false
    @State private var inviting = false

    var body: some View {
        // A store left while its page shows goes Back to the stores.
        page.onRemoved(PagePresence(key: id, present: appStore.stores.store(id) != nil)) { dismiss() }
    }

    @ViewBuilder
    private var page: some View {
        if let row = appStore.stores.store(id) {
            PhoneList {
                Section {
                    ForEach(row.plugins, id: \.id) { plugin in
                        NavigationLink(value: PhoneSettingsPage.plugin(plugin.id)) {
                            HStack {
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(plugin.name).font(.ui(17)).foregroundStyle(.silk)
                                    Text(plugin.contactsLine).font(.ui(15)).foregroundStyle(.haze)
                                }
                                Spacer()
                                PhonePluginAction(store: row, plugin: plugin)
                            }
                        }
                        .listRowBackground(Palette.raised)
                    }
                } header: {
                    PhoneSectionHeader(L.storesPlugins)
                } footer: {
                    Text(L.storesPluginLimits)
                }
                Section {
                    // A device that joined with the store's link joins another the same way.
                    if row.state != .revoked, !row.anonymous {
                        Button(L.storesLinkDeviceTitle) { inviting = true }
                            .fontWeight(.semibold)
                            .foregroundStyle(.silk)
                            .listRowBackground(Palette.raised)
                    }
                    Button(L.storesLeave, role: .destructive) { leaving = true }
                        .fontWeight(.semibold)
                        .listRowBackground(Palette.raised)
                } footer: {
                    // Who can join is changed on the Mac, an iPad or the store's admin page.
                    let about = [StoreWords.meta(row), row.open ? L.storesAnyoneShort : nil]
                    let key = [L.storesKeyLine(key: row.fingerprint, joined: row.joinedAs), StoreWords.checked(row)]
                    Text(about.compactMap { $0 }.joined(separator: " · ") + "\n"
                         + key.compactMap { $0 }.joined(separator: " · "))
                }
            }
            .navigationTitle(row.name)
            .navigationBarTitleDisplayMode(.inline)
            .confirmationDialog(L.storesLeaveTitle(name: row.name), isPresented: $leaving, titleVisibility: .visible) {
                Button(L.storesLeave, role: .destructive) { appStore.leaveStore(row.id) }
            } message: {
                Text(L.storesLeaveDetail)
            }
            .sheet(isPresented: $inviting) { InviteSheet(store: row, device: true) }
        } else {
            Text(L.storesNoLongerJoined).foregroundStyle(.haze)
        }
    }
}

/// A plugin's action in a store's list: installed, an update, or install.
struct PhonePluginAction: View {
    @Environment(AppStore.self) private var appStore
    let store: StoreRow
    let plugin: StorePluginRow
    /// Whether the update is asking first, with the hosts it adds.
    @State private var confirming = false

    var body: some View {
        let working = appStore.stores.isWorking(.install(store: store.id, plugin: plugin.id))
        switch plugin.status {
        case .installed:
            HStack(spacing: 6) {
                Image(systemName: "checkmark").foregroundStyle(.water)
                Text(L.storesPluginStateInstalled).foregroundStyle(.haze)
            }
            .font(.ui(15))
        case .update where store.state != .revoked:
            Button(L.actionUpdate) { confirming = true }
                .buttonStyle(PrimaryCapsuleStyle(height: 30, fontSize: 15))
                .disabled(working)
                .pluginUpdateAlert(isPresented: $confirming, store: store, plugin: plugin)
        case .available where store.state == .ready:
            Button(L.actionInstall) { appStore.installPlugin(plugin.id, from: store.id) }
                .buttonStyle(SecondaryCapsuleStyle(height: 30, raisedSurface: true, fontSize: 15))
                .disabled(working)
        case .otherStore(let other):
            Text(L.storesPluginFrom(other: other)).font(.ui(15)).foregroundStyle(.haze)
        default:
            EmptyView()
        }
    }
}

/// A plugin: its version, its store, how fast it answers, an update with what it adds, what it
/// may contact, and turning it off or removing it.
struct PhonePluginPage: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    let id: String
    @State private var removing = false
    @State private var confirmingUpdate = false

    var body: some View {
        let row = store.providers.row(id)
        let listing = store.stores.origin(of: id)
        PhoneList {
            Section {
                value(L.sourcesVersion, listing?.row.installedVersion ?? L.sourcesOriginNoStore)
                if let listing {
                    NavigationLink(value: PhoneSettingsPage.store(listing.store.id)) {
                        HStack {
                            Text(L.storesStoreLabel).font(.ui(17)).foregroundStyle(.silk)
                            Spacer()
                            Text(listing.store.name).font(.ui(17)).foregroundStyle(.haze)
                        }
                    }
                    .listRowBackground(Palette.raised)
                }
                value(L.sourcesResponseTime, row.map { r in
                    r.standing == .failing ? L.sourcesNoAnswer : r.latencyMs.map { L.sourcesResponseMs(ms: Int($0)) } ?? L.sourcesNotChecked
                } ?? L.sourceNoteGone)
            }
            if let listing, case .update(_, let added) = listing.row.status {
                Section {
                    Button {
                        confirmingUpdate = true
                    } label: {
                        Text(L.sourcesUpdateTo(version: listing.row.version)).frame(maxWidth: .infinity)
                    }
                    .buttonStyle(PrimaryCapsuleStyle(height: 50, fontSize: 17))
                    .disabled(store.stores.isWorking(.install(store: listing.store.id, plugin: id)))
                    .pluginUpdateAlert(isPresented: $confirmingUpdate, store: listing.store, plugin: listing.row)
                    .listRowBackground(Color.clear)
                    .listRowInsets(EdgeInsets())
                } footer: {
                    if !added.isEmpty {
                        Text(L.storesUpdateContacts(hosts: added.formatted(.list(type: .and).locale(AirTime.locale))))
                    }
                }
            }
            Section {
                ForEach(listing?.row.hosts ?? [], id: \.self) { host in
                    Text(host).font(.ui(17)).foregroundStyle(.silk).listRowBackground(Palette.raised)
                }
            } header: {
                PhoneSectionHeader(L.sourcesContactsLabel)
            } footer: {
                Text(L.storesPluginLimits)
            }
            Section {
                let disabled = store.isPluginDisabled(id)
                Button(disabled ? L.actionEnable : L.actionDisable) { store.setPluginDisabled(id, !disabled) }
                    .listRowBackground(Palette.raised)
                Button(L.actionRemove, role: .destructive) { removing = true }
                    .fontWeight(.semibold)
                    .listRowBackground(Palette.raised)
            }
        }
        .navigationTitle(listing?.row.name ?? id)
        .navigationBarTitleDisplayMode(.inline)
        .confirmationDialog(L.storesRemovePluginTitle(name: listing?.row.name ?? id), isPresented: $removing, titleVisibility: .visible) {
            Button(L.actionRemove, role: .destructive) { store.removePlugin(id) }
        }
        // Removed while its page shows: Back to the list it came from.
        .onRemoved(PagePresence(key: id, present: row != nil)) { dismiss() }
    }

    private func value(_ title: String, _ value: String) -> some View {
        HStack {
            Text(title).font(.ui(17)).foregroundStyle(.silk)
            Spacer()
            Text(value).font(.ui(17)).foregroundStyle(.haze)
        }
        .listRowBackground(Palette.raised)
    }
}

// MARK: - Sheets

/// Joining a store on iPhone: the link, what it leads to, and Join at the foot.
struct PhoneJoinStoreSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.dismiss) private var dismiss
    @State var link: String

    var body: some View {
        if let outcome = store.cameIn.outcome, StoreCameIn.tells(outcome) {
            NavigationStack {
                ScrollView {
                    StoreCameIn(outcome: outcome).padding(.horizontal, 20).padding(.vertical, 12)
                }
                .background(.raised)
                .navigationTitle(L.storesJoinTitle)
                .navigationBarTitleDisplayMode(.inline)
                .safeAreaInset(edge: .bottom) {
                    StoreCameIn.actions(outcome, store: store) {
                        store.cameIn.reset()
                        dismiss()
                    }
                    .padding(.horizontal, 16).padding(.bottom, 8)
                }
            }
            .tint(.silk)
            .presentationBackground(.raised)
            .transition(.opacity)
            .onDisappear { store.cameIn.reset() }
        } else {
            form
        }
    }

    private var form: some View {
        NavigationStack {
            PhoneList {
                Section {
                    TextField(L.storesInviteLink, text: $link, prompt: Text(L.storesInviteLinkHint))
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .keyboardType(.URL)
                        .listRowBackground(Palette.raised)
                } header: {
                    PhoneSectionHeader(L.storesInviteLink)
                } footer: {
                    Text(L.storesJoinDetailOpened)
                }
                preview
                if case .store(let invited)? = store.stores.preview, !invited.anonymous {
                    Section {
                        TextField(L.storesDeviceName,
                                  text: Binding(get: { store.stores.deviceName }, set: { store.stores.deviceName = $0 }),
                                  prompt: Text(Device.name))
                            .autocorrectionDisabled()
                            .listRowBackground(Palette.raised)
                    } header: {
                        PhoneSectionHeader(L.storesDeviceName)
                    } footer: {
                        Text(L.storesDeviceNameDetail)
                    }
                }
                if case .store(let invited)? = store.stores.preview, invited.namesJoiner {
                    Section {
                        TextField(L.storesJoinYourName,
                                  text: Binding(get: { store.stores.joinerName }, set: { store.stores.joinerName = $0 }))
                            .autocorrectionDisabled()
                            .listRowBackground(Palette.raised)
                    } header: {
                        PhoneSectionHeader(L.storesJoinYourName)
                    } footer: {
                        Text(L.storesJoinYourNameDetail)
                    }
                }
            }
            .navigationTitle(L.storesJoinTitle)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarLeading) { Button(L.actionCancel) { dismiss() }.tint(.silk) }
            }
            .safeAreaInset(edge: .bottom) {
                Button {
                    store.joinStore(link)
                } label: {
                    Text(L.storesJoinButton).frame(maxWidth: .infinity)
                }
                .buttonStyle(PrimaryCapsuleStyle(height: 50, fontSize: 17))
                .disabled(!canJoin || store.stores.isWorking(.join))
                .padding(.horizontal, 16)
                .padding(.bottom, 8)
            }
        }
        .tint(.silk)
        .task(id: link) {
            try? await Task.sleep(for: .milliseconds(300))
            guard !Task.isCancelled else { return }
            if InviteLinks.looksLikeInvite(link) { store.previewStore(link) } else { store.stores.clearPreview() }
        }
        .onChange(of: store.stores.joined) { _, joined in
            guard joined != nil else { return }
            Task {
                try? await Task.sleep(for: .seconds(1.5))
                if !StoreCameIn.tells(store.cameIn.outcome) { dismiss() }
            }
        }
        .onDisappear { store.stores.clearPreview() }
        .holdsInput(link)
    }

    private var canJoin: Bool {
        if case .store(let invited) = store.stores.preview { return !invited.alreadyJoined }
        return false
    }

    @ViewBuilder
    private var preview: some View {
        switch store.stores.preview {
        case .checking?:
            Section {
                HStack(spacing: 8) {
                    ProgressView()
                    Text(L.storesCheckingLink).foregroundStyle(.haze)
                }
                .listRowBackground(Palette.raised)
            }
        case .failed(let reason, let contact)?:
            Section {
                VStack(alignment: .leading, spacing: 4) {
                    FailedLine(joinFailureWords(reason), size: 15)
                    if let contact { Text(L.storeInviteContact(contact: contact)).font(.ui(13)).foregroundStyle(.haze) }
                }
                .listRowBackground(Palette.raised)
            }
        case .store(let invited)?:
            Section {
                HStack(spacing: 12) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(invited.name).font(.ui(17, weight: .semibold)).foregroundStyle(.silk)
                        Text(invited.runByLine).font(.ui(13)).foregroundStyle(.haze)
                    }
                    Spacer(minLength: 0)
                    if !invited.plugins.isEmpty {
                        Text(L.storesPluginsCount(count: invited.plugins.count))
                            .font(.ui(15)).monospacedDigit().foregroundStyle(.haze)
                    }
                }
                .listRowBackground(Palette.raised)
                HStack(spacing: 10) {
                    Image(systemName: "checkmark").font(.ui(15, weight: .semibold)).foregroundStyle(.water)
                    VStack(alignment: .leading, spacing: 1) {
                        Text(L.storesKeyMatches(key: invited.fingerprint)).font(.ui(15)).foregroundStyle(.silk)
                        Text(invited.fingerprint).font(.ui(13, design: .monospaced)).foregroundStyle(.haze)
                    }
                }
                .listRowBackground(Palette.raised)
                if invited.open {
                    HStack(alignment: .top, spacing: 12) {
                        Image(systemName: "person.2").foregroundStyle(.haze)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(L.storesAnyoneWithLink).font(.ui(17)).foregroundStyle(.silk)
                            if invited.anonymous {
                                Text(L.storesAnyoneNote).font(.ui(15)).foregroundStyle(.haze)
                            }
                        }
                    }
                    .listRowBackground(Palette.raised)
                }
                ForEach(invited.plugins, id: \.id) { plugin in
                    HStack(spacing: 12) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(plugin.name).font(.ui(17)).foregroundStyle(.silk).lineLimit(1)
                            Text(([plugin.contactsLine] + [plugin.standing].compactMap { $0 }).joined(separator: " · "))
                                .font(.ui(13)).foregroundStyle(.haze).lineLimit(1)
                        }
                        Spacer(minLength: 0)
                        Text(plugin.version).font(.ui(15)).monospacedDigit().foregroundStyle(.haze)
                    }
                    .accessibilityElement(children: .combine)
                    .listRowBackground(Palette.raised)
                }
            } footer: {
                Text(invited.alreadyJoined
                     ? L.storesAlreadyJoinedNote
                     : L.storesNotFromApp)
            }
        case nil:
            EmptyView()
        }
    }
}

/// The proxy form on iPhone, in a list: asked for when the route switch goes on, or editing the
/// saved proxy. A proxy is saved only once the check through it passes.
struct PhoneProxySheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    var mode: ProxyFormMode = .add
    @State private var draft: ProxyDraft?
    /// What the check under way is for, saved once it passes.
    @State private var checking: ProxyDraft?
    /// The draft as the form opened, to tell a change from it.
    @State private var opened: ProxyDraft?

    private var editing: Bool { mode == .edit }

    var body: some View {
        let draft = Binding(get: { self.draft ?? ProxyDraft(store) }, set: { self.draft = $0 })
        let trial = store.providers.trial
        NavigationStack {
            PhoneList {
                Section {
                    Text(editing ? ProxyWords.editNote : L.torrentsRisk)
                        .font(.ui(17))
                        .foregroundStyle(.silk)
                        .fixedSize(horizontal: false, vertical: true)
                        .listRowBackground(Color.clear)
                        // In line with the section headers and the rows' words below.
                        .listRowInsets(EdgeInsets(top: 0, leading: 16, bottom: 0, trailing: 16))
                }
                // Each field named beside it, so a filled-in one still says what it is.
                Section {
                    LabeledContent(L.torrentsProxyHost) {
                        TextField(L.torrentsProxyHost, text: draft.host, prompt: Text("proxy.example")) // i18n-skip
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                            .multilineTextAlignment(.trailing)
                    }
                    LabeledContent(L.torrentsProxyPort) {
                        TextField(L.torrentsProxyPort, text: draft.port, prompt: Text("1080"))
                            .keyboardType(.numberPad)
                            .multilineTextAlignment(.trailing)
                    }
                    LabeledContent(L.torrentsProxyUser) {
                        TextField(L.torrentsProxyUser, text: draft.user, prompt: Text(L.torrentsProxyOptional))
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                            .multilineTextAlignment(.trailing)
                    }
                    LabeledContent(L.torrentsProxyPassword) {
                        SecureField(L.torrentsProxyPassword, text: draft.password, prompt: Text(L.torrentsProxyOptional))
                            .multilineTextAlignment(.trailing)
                    }
                } header: {
                    PhoneSectionHeader(L.torrentsProxyRoute)
                } footer: {
                    VStack(alignment: .leading, spacing: 6) {
                        if editing { Text(ProxyWords.passwordKept) }
                        Text(ProxyWords.checkNote)
                        if draft.wrappedValue.invalid { Text(L.torrentsProxyInvalid) }
                    }
                }
                .listRowBackground(Palette.raised)
            }
            // Held still while the check through the proxy runs.
            .disabled(trial == .checking)
            .navigationTitle(editing ? L.torrentsSheetEditProxy : L.torrentsProxyRoute)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarLeading) { Button(L.actionCancel) { dismiss() }.tint(.silk) }
            }
            .safeAreaInset(edge: .bottom) {
                VStack(alignment: .leading, spacing: 12) {
                    if ProxyCheckLine.shows(trial) {
                        ProxyCheckLine(trial: trial, phone: true).padding(.horizontal, 20)
                    }
                    Button {
                        send(draft.wrappedValue)
                    } label: {
                        Text(L.actionSave).frame(maxWidth: .infinity)
                    }
                    .buttonStyle(PrimaryCapsuleStyle(height: 50, fontSize: 17))
                    .disabled(!draft.wrappedValue.complete || trial == .checking)
                    .padding(.horizontal, 16)
                }
                .padding(.bottom, 8)
            }
        }
        .tint(.silk)
        .onAppear {
            let first = ProxyDraft(store)
            self.draft = first
            opened = first
        }
        .modifier(ProxySaving(checking: checking, draft: self.draft) { dismiss() })
        // Changed from what it opened with, or with a check under way, it stays open.
        .holdsInput(when: self.draft != opened || checking != nil)
    }

    private func send(_ draft: ProxyDraft) {
        checking = draft
        store.check(draft)
    }
}
#endif
