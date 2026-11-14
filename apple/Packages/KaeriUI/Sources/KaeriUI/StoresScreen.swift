// Plugin stores on the Mac and a wide iPad: the stores joined down the left, the selected one's
// plugins beside them with what each may contact, and the sheets for joining, updating and what
// an owner does.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

struct PluginStoresScreen: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @State private var selection: String?

    /// What the screen shows: nothing before the engine's first answer, its empty state, or
    /// the stores.
    private enum Phase { case waiting, empty, stores }

    var body: some View {
        let stores = store.stores.stores
        let phase = !store.stores.loaded && stores.isEmpty ? Phase.waiting : stores.isEmpty ? .empty : .stores
        // A stack, not a Group, so the first answer fades in over the ground rather than
        // cutting in, and the empty state and the stores crossfade.
        ZStack(alignment: .topLeading) {
            switch phase {
            case .waiting:
                Color.clear
            case .empty:
                NoStores()
                    .transition(.opacity)
            case .stores:
                HStack(alignment: .top, spacing: 0) {
                    StoreList(selection: $selection)
                        .frame(width: 300)
                    if let row = stores.first(where: { $0.id == selection }) ?? stores.first {
                        StoreDetail(row: row)
                            .id(row.id)
                    }
                }
                .padding(.leading, 12)
                .transition(.opacity)
            }
        }
        .animation(.fadeIn, value: phase)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .task { store.loadStores() }
        .onChange(of: store.stores.joined, initial: true) { _, joined in
            if let joined {
                selection = joined
                store.stores.clearJoined()
            }
        }
    }
}

/// No store joined yet: what stores are, and the way to join one.
struct NoStores: View {
    @Environment(Navigator.self) private var navigator

    var body: some View {
        EmptyState(
            title: L.storesEmptyTitle,
            line: L.storesEmptyDetail
        ) {
            Button(L.storesJoinShort) { navigator.invite = "" }
                .buttonStyle(SecondaryCapsuleStyle(height: ScreenFailed.buttonHeight, fontSize: ScreenFailed.buttonFont))
        }
    }
}

/// The stores joined, each with how it stands.
struct StoreList: View {
    @Environment(AppStore.self) private var store
    @Binding var selection: String?

    var body: some View {
        let stores = store.stores.stores
        let current = selection ?? stores.first?.id
        ScrollView {
            VStack(alignment: .leading, spacing: 2) {
                Text(L.storesJoined)
                    .groupHeaderStyle()
                    .accessibilityAddTraits(.isHeader)
                    .macLineHeight(14)
                    .padding(EdgeInsets(top: 10, leading: 12, bottom: 8, trailing: 12))
                ForEach(stores, id: \.id) { row in
                    let selected = row.id == current
                    Button {
                        selection = row.id
                    } label: {
                        HStack(spacing: 12) {
                            Text(row.name)
                                .font(.mac(13, weight: selected ? .medium : .regular))
                                .foregroundStyle(selected ? .amber : .silk)
                                .lineLimit(1)
                            Spacer(minLength: 0)
                            StoreStanding(row: row, selected: selected)
                        }
                        .padding(.horizontal, 12)
                        .frame(height: 36)
                        .background(selected ? Palette.selection : .clear, in: .rect(cornerRadius: 8))
                        .contentShape(.rect(cornerRadius: 8))
                    }
                    .buttonStyle(.plain)
                    .accessibilityAddTraits(selected ? .isSelected : [])
                }
                Text(L.storesEmptyDetail)
                    .font(.mac(12))
                    .macLineHeight(17)
                    .foregroundStyle(.haze)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(EdgeInsets(top: 14, leading: 12, bottom: 0, trailing: 12))
            }
            .padding(EdgeInsets(top: 4, leading: 4, bottom: 16, trailing: 8))
        }
        .scrollIndicators(.never)
    }
}

/// How a store stands, beside its name: updates waiting, offline, or no longer letting you in.
struct StoreStanding: View {
    let row: StoreRow
    var selected = false
    var size: CGFloat = 12

    var body: some View {
        switch row.state {
        case .revoked:
            label(L.storesStatusAccessEnded, dot: .vermilion, color: .silk)
        case .offline:
            label(L.storesStatusOffline, ring: true, color: .haze)
        case .ready where row.updates > 0:
            label(L.storesUpdatesAvailable(count: Int(row.updates)), dot: .water, color: .silk)
        case .ready:
            EmptyView()
        }
    }

    private func label(_ text: String, dot: Color? = nil, ring: Bool = false, color: Color) -> some View {
        HStack(spacing: 6) {
            if ring {
                Circle().strokeBorder(.haze, lineWidth: 1.5).frame(width: 6, height: 6)
            } else if let dot {
                Circle().fill(dot).frame(width: 6, height: 6)
            }
            Text(text).font(.mac(size)).foregroundStyle(color)
        }
    }
}

/// One store: its name, its address, its plugins with what each may contact and the action
/// each takes, and the key it was joined by.
struct StoreDetail: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let row: StoreRow
    @State private var sheet: StoreSheet?
    @State private var leaving = false

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                HStack(alignment: .top) {
                    VStack(alignment: .leading, spacing: 4) {
                        Text(row.name)
                            .font(.mac(22, weight: .bold))
                            .macLineHeight(28)
                            .foregroundStyle(.silk)
                            .accessibilityAddTraits(.isHeader)
                        Text(StoreWords.meta(row))
                            .font(.mac(13))
                            .foregroundStyle(.haze)
                        if row.open {
                            Text(L.storesAnyoneWithLink)
                                .font(.mac(12))
                                .foregroundStyle(.haze)
                        }
                    }
                    Spacer()
                    StoreMenu(row: row, sheet: $sheet, leaving: $leaving)
                }
                if case .offline = row.state {
                    Text(L.storesOfflineLastListPlain)
                        .font(.mac(12))
                        .foregroundStyle(.haze)
                        .padding(.top, 8)
                }
                if row.state == .revoked {
                    FailedLine(L.storesAccessEndedPage, size: 12)
                        .padding(.top, 8)
                }
                Text(L.storesPlugins)
                    .groupHeaderStyle()
                    .accessibilityAddTraits(.isHeader)
                    .macLineHeight(14)
                    .padding(EdgeInsets(top: 28, leading: 18, bottom: 8, trailing: 0))
                VStack(spacing: 0) {
                    Rectangle().fill(.faintRule).frame(height: 1)
                    ForEach(row.plugins, id: \.id) { plugin in
                        StorePluginLine(store: row, plugin: plugin, sheet: $sheet)
                        Rectangle().fill(.faintRule).frame(height: 1)
                    }
                }
                VStack(alignment: .leading, spacing: 4) {
                    Text(L.storesPluginLimits)
                    Text(L.storesKeyLine(key: row.fingerprint, joined: row.joinedAs))
                }
                .font(.mac(12))
                .macLineHeight(17)
                .foregroundStyle(.haze)
                .padding(EdgeInsets(top: 20, leading: 18, bottom: 0, trailing: 0))
            }
            .padding(EdgeInsets(top: 18, leading: 24, bottom: 24, trailing: 24))
        }
        .scrollIndicators(.never)
        .sheet(item: $sheet) { sheet in
            switch sheet {
            case .update(let plugin): UpdatePluginSheet(store: row, plugin: plugin)
            case .invite(let device): InviteSheet(store: row, device: device)
            case .members: MembersSheet(store: row)
            case .joining: OpenJoinSheet(store: row)
            case .publish: PublishSheet(store: row)
            }
        }
        .onLaunchSheet("update", navigator) {
            if let plugin = row.plugins.first(where: { if case .update = $0.status { true } else { false } }) {
                sheet = .update(plugin)
            }
        }
        .onLaunchSheet("members", navigator) { sheet = .members }
        .onLaunchSheet("invite", navigator) { sheet = .invite(device: false) }
        .confirmationDialog(L.storesLeaveTitle(name: row.name), isPresented: $leaving) {
            Button(L.storesLeave, role: .destructive) { store.leaveStore(row.id) }
        } message: {
            Text(L.storesLeaveDetail)
        }
    }
}

/// A sheet a store screen opens.
enum StoreSheet: Identifiable, Hashable {
    case update(StorePluginRow)
    case invite(device: Bool)
    case members
    case joining
    case publish

    var id: String {
        switch self {
        case .update(let plugin): "update-\(plugin.id)"
        case .invite(let device): "invite-\(device)"
        case .members: "members"
        case .joining: "joining"
        case .publish: "publish"
        }
    }
}

/// One plugin in a store: its name, what it may contact, and its action.
struct StorePluginLine: View {
    @Environment(AppStore.self) private var appStore
    let store: StoreRow
    let plugin: StorePluginRow
    @Binding var sheet: StoreSheet?

    var body: some View {
        HStack(spacing: 16) {
            VStack(alignment: .leading, spacing: 2) {
                Text(plugin.name)
                    .font(.mac(13, weight: .medium))
                    .macLineHeight(17)
                    .foregroundStyle(.silk)
                Text(line)
                    .font(.mac(12))
                    .macLineHeight(15)
                    .foregroundStyle(.haze)
                    .lineLimit(1)
            }
            Spacer(minLength: 0)
            action
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 7)
        .frame(minHeight: 48)
        .accessibilityElement(children: .combine)
    }

    private var line: String {
        if case .addedByHand = plugin.status {
            return L.storesAddedByHand(detail: plugin.contactsLine)
        }
        return plugin.contactsLine
    }

    @ViewBuilder
    private var action: some View {
        let live = store.state != .revoked
        let working = appStore.stores.isWorking(.install(store: store.id, plugin: plugin.id))
        switch plugin.status {
        case .installed:
            HStack(spacing: 6) {
                Image(systemName: "checkmark").font(.mac(11, weight: .semibold)).foregroundStyle(.water)
                Text(L.storesPluginStateInstalled).font(.mac(12)).foregroundStyle(.haze)
            }
        case .update where live:
            Button(L.actionUpdate) { sheet = .update(plugin) }
                .buttonStyle(PrimaryCapsuleStyle(height: 28))
                .disabled(working)
                .accessibilityLabel(L.storesUpdateLabel(name: plugin.name, version: plugin.version))
        case .available where store.state == .ready:
            Button(L.actionInstall) { appStore.installPlugin(plugin.id, from: store.id) }
                .buttonStyle(SecondaryCapsuleStyle())
                .disabled(working)
                .accessibilityLabel(L.storesInstallLabel(name: plugin.name))
        case .available:
            Text(live ? L.storesPluginOffline : L.storesPluginUnavailable).font(.mac(12)).foregroundStyle(.haze)
        case .addedByHand where live:
            Button(L.actionReplace) { sheet = .update(plugin) }
                .buttonStyle(SecondaryCapsuleStyle())
                .disabled(working)
                .accessibilityLabel(L.storesReplaceLabel(name: plugin.name))
        case .otherStore(let other):
            Text(L.storesPluginFrom(other: other)).font(.mac(12)).foregroundStyle(.haze)
        default:
            EmptyView()
        }
    }
}

/// The store's actions: invites, members and who can join for its owner, another device, and
/// leaving.
struct StoreMenu: View {
    @Environment(AppStore.self) private var store
    let row: StoreRow
    @Binding var sheet: StoreSheet?
    @Binding var leaving: Bool

    var body: some View {
        Menu {
            if row.state != .revoked {
                if row.owner {
                    Button(L.storesInviteSomeone) { sheet = .invite(device: false) }
                    Button(L.storesShowMembers) { sheet = .members }
                    Button(L.storesWhoCanJoinMenu) { sheet = .joining }
                    Button(L.storesPublishMenu) { sheet = .publish }
                    Button(L.storesOpenAdmin) { store.openAdmin(row.id) }
                    Divider()
                }
                // A device that joined with the store's link joins another the same way.
                if !row.anonymous {
                    Button(L.storesLinkDevice) { sheet = .invite(device: true) }
                    Divider()
                }
            }
            Button(L.storesLeave) { leaving = true }
        } label: {
            Image(systemName: "ellipsis")
                .font(.mac(13, weight: .semibold))
                .foregroundStyle(.silk)
                .frame(width: 28, height: 28)
                .background(.raised, in: .circle)
                .overlay(Circle().strokeBorder(.faintRule, lineWidth: 1))
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .menuIndicator(.hidden)
        .fixedSize()
        .accessibilityLabel(L.storesActionsFor(name: row.name))
    }
}

enum StoreWords {
    /// Who runs the store and where it is: "Run by Ren · plugins.example", or its host alone.
    static func meta(_ row: StoreRow) -> String {
        row.runByLine
    }

    /// When the store's list was last fetched and checked: "checked 2 minutes ago".
    static func checked(_ row: StoreRow) -> String? {
        row.checkedAt.map {
            L.storesCheckedAt(when: Date(timeIntervalSince1970: TimeInterval($0)).formatted(.relative(presentation: .named).locale(AirTime.locale)))
        }
    }
}

// MARK: - Sheets

/// Confirming an update, or replacing a copy added by hand: the version, and what it adds to
/// what the plugin may contact.
struct UpdatePluginSheet: View {
    @Environment(AppStore.self) private var appStore
    @Environment(\.dismiss) private var dismiss
    let store: StoreRow
    let plugin: StorePluginRow

    var body: some View {
        let added: [String] = if case .update(_, let new) = plugin.status { new } else { [] }
        VStack(alignment: .leading, spacing: 0) {
            Text(title)
                .font(.mac(15, weight: .semibold))
                .foregroundStyle(.silk)
            Text(subtitle)
                .font(.mac(13))
                .foregroundStyle(.haze)
                .padding(.top, 4)
            if !added.isEmpty {
                Text(L.storesUpdateContacts(hosts: added.formatted(.list(type: .and).locale(AirTime.locale))))
                    .font(.mac(13))
                    .foregroundStyle(.silk)
                    .padding(.top, 16)
            }
            HStack(alignment: .top, spacing: 24) {
                Text(L.sourcesContactsLabel).foregroundStyle(.haze)
                VStack(alignment: .leading, spacing: 8) {
                    ForEach(plugin.hosts, id: \.self) { host in
                        HStack {
                            Text(host).foregroundStyle(.silk)
                            Spacer()
                            if added.contains(host) {
                                Text(L.storesNewIn(version: plugin.version)).foregroundStyle(.haze)
                            }
                        }
                    }
                }
            }
            .font(.mac(13))
            .padding(14)
            .background(.ground, in: .rect(cornerRadius: 10))
            .padding(.top, 14)
            Text(L.storesPluginLimitsShort)
                .font(.mac(12))
                .foregroundStyle(.haze)
                .padding(.top, 14)
            HStack(spacing: 10) {
                Spacer()
                Button(L.actionCancel) { dismiss() }
                    .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                    .keyboardShortcut(.cancelAction)
                Button(confirm) {
                    appStore.installPlugin(plugin.id, from: store.id)
                    dismiss()
                }
                .buttonStyle(PrimaryCapsuleStyle(height: 28))
                .keyboardShortcut(.defaultAction)
            }
            .padding(.top, 20)
        }
        .padding(30)
        .sheetWidth(540)
        .background(.raised)
    }

    private var byHand: Bool {
        if case .addedByHand = plugin.status { true } else { false }
    }

    private var title: String {
        byHand ? L.sourcesReplaceTitle(name: plugin.name) : L.sourcesUpdateTitle(name: plugin.name)
    }

    private var subtitle: String {
        if case .update(let from, _) = plugin.status {
            return L.sourcesUpdateDetail(version: plugin.version, store: store.name, from: from)
        }
        return L.storesReplaceByHandDetail(version: plugin.version, store: store.name)
    }

    private var confirm: String {
        byHand ? L.actionReplace : L.actionUpdate
    }
}

/// Joining a store from an invite link: pasted, or the one the app was opened with, checked
/// against the store's key before anything is spent.
struct JoinStoreSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.dismiss) private var dismiss
    @State var link: String

    var body: some View {
        if let outcome = store.cameIn.outcome, StoreCameIn.tells(outcome) {
            VStack(alignment: .leading, spacing: 0) {
                StoreCameIn(outcome: outcome)
                StoreCameIn.actions(outcome, store: store) {
                    store.cameIn.reset()
                    navigator.open(.pluginStores)
                    dismiss()
                }
                .padding(.top, 20)
            }
            .padding(30)
            .sheetWidth(600)
            .background(.raised)
            .transition(.opacity)
            .onDisappear { store.cameIn.reset() }
        } else {
            form
        }
    }

    private var form: some View {
        let working = store.stores.isWorking(.join)
        return VStack(alignment: .leading, spacing: 0) {
            Text(L.storesJoinTitle)
                .font(.mac(15, weight: .semibold))
                .foregroundStyle(.silk)
            Text(L.storesJoinDetail)
                .font(.mac(13))
                .foregroundStyle(.haze)
                .padding(.top, 4)
            Text(L.storesInviteLink)
                .font(.mac(12))
                .foregroundStyle(.haze)
                .padding(.top, 18)
            TextField(L.storesInviteLink, text: $link, prompt: Text(L.storesInviteLinkHint))
                .textFieldStyle(.plain)
                .font(.mac(13))
                .padding(.horizontal, 12)
                .frame(height: 32)
                .background(.ground, in: .rect(cornerRadius: 8))
                .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(Palette.glassRule, lineWidth: 1))
                .labelsHidden()
                .padding(.top, 6)
            preview.padding(.top, 14)
            if case .store(let invited)? = store.stores.preview, !invited.anonymous {
                deviceName.padding(.top, 14)
            }
            if case .store(let invited)? = store.stores.preview, invited.namesJoiner {
                yourName.padding(.top, 14)
            }
            Text(L.storesNotFromApp)
                .font(.mac(12))
                .macLineHeight(17)
                .foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, 14)
            HStack(spacing: 10) {
                Spacer()
                if working { ProgressView().controlSize(.small) }
                Button(L.actionCancel) { dismiss() }
                    .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                    .keyboardShortcut(.cancelAction)
                Button(L.storesJoinButton) { store.joinStore(link) }
                    .buttonStyle(PrimaryCapsuleStyle(height: 28))
                    .keyboardShortcut(.defaultAction)
                    .disabled(!canJoin || working)
            }
            .padding(.top, 20)
        }
        .padding(30)
        .sheetWidth(600)
        .background(.raised)
        .task(id: link) {
            // A pause while typing, then the link is checked.
            try? await Task.sleep(for: .milliseconds(300))
            guard !Task.isCancelled else { return }
            if InviteLinks.looksLikeInvite(link) { store.previewStore(link) } else { store.stores.clearPreview() }
        }
        .onChange(of: store.stores.joined) { _, joined in
            // The store's own answer says what it brought; without one, on into the store.
            guard joined != nil else { return }
            Task {
                try? await Task.sleep(for: .seconds(1.5))
                guard !StoreCameIn.tells(store.cameIn.outcome) else { return }
                navigator.open(.pluginStores)
                dismiss()
            }
        }
        .onDisappear { store.stores.clearPreview() }
    }

    private var canJoin: Bool {
        if case .store(let invited) = store.stores.preview { return !invited.alreadyJoined }
        return false
    }

    /// What the store will call this Mac: its kind, offered with the preview, to change before
    /// joining. Never the Mac's own name, which is often its owner's.
    private var deviceName: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(L.storesDeviceName)
                .font(.mac(12))
                .foregroundStyle(.haze)
            TextField(L.storesDeviceName,
                      text: Binding(get: { store.stores.deviceName }, set: { store.stores.deviceName = $0 }),
                      prompt: Text(Device.name))
                .textFieldStyle(.plain)
                .font(.mac(13))
                .padding(.horizontal, 12)
                .frame(height: 32)
                .background(.ground, in: .rect(cornerRadius: 8))
                .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(Palette.glassRule, lineWidth: 1))
                .labelsHidden()
                .padding(.top, 6)
            Text(L.storesDeviceNameDetail)
                .font(.mac(12))
                .foregroundStyle(.haze)
                .padding(.top, 6)
        }
    }

    /// The person's own name, for an invite that works more than once: the owners see it in the
    /// members list.
    private var yourName: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(L.storesJoinYourName)
                .font(.mac(12))
                .foregroundStyle(.haze)
            TextField(L.storesJoinYourName,
                      text: Binding(get: { store.stores.joinerName }, set: { store.stores.joinerName = $0 }))
                .textFieldStyle(.plain)
                .font(.mac(13))
                .padding(.horizontal, 12)
                .frame(height: 32)
                .background(.ground, in: .rect(cornerRadius: 8))
                .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(Palette.glassRule, lineWidth: 1))
                .labelsHidden()
                .padding(.top, 6)
            Text(L.storesJoinYourNameDetail)
                .font(.mac(12))
                .foregroundStyle(.haze)
                .padding(.top, 6)
        }
    }

    @ViewBuilder
    private var preview: some View {
        switch store.stores.preview {
        case .checking?:
            HStack(spacing: 8) {
                ProgressView().controlSize(.small)
                Text(L.storesCheckingLink).font(.mac(13)).foregroundStyle(.haze)
            }
        case .failed(let reason, let contact)?:
            VStack(alignment: .leading, spacing: 4) {
                FailedLine(joinFailureWords(reason))
                if let contact { Text(L.storeInviteContact(contact: contact)).font(.mac(12)).foregroundStyle(.haze) }
            }
            .fixedSize(horizontal: false, vertical: true)
        case .store(let invited)?:
            JoinPreview(invited: invited)
        case nil:
            EmptyView()
        }
    }
}

/// What a link leads to, before anything is joined: the store, who runs it and how many plugins it
/// offers, its key against the link's, and each plugin with its version, what it may contact and
/// where it stands here. From the store's signed list; a store from before the preview shows only
/// its name and key.
struct JoinPreview: View {
    let invited: InvitedStore

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            VStack(alignment: .leading, spacing: 3) {
                HStack(alignment: .firstTextBaseline, spacing: 12) {
                    Text(invited.name)
                        .font(.mac(15, weight: .semibold))
                        .foregroundStyle(.silk)
                    Spacer(minLength: 0)
                    if !invited.plugins.isEmpty {
                        Text(Wording.plural(invited.plugins.count, "plugin"))
                            .font(.mac(12))
                            .monospacedDigit()
                            .foregroundStyle(.haze)
                    }
                }
                Text(invited.runByLine)
                    .font(.mac(12))
                    .foregroundStyle(.haze)
                HStack(spacing: 5) {
                    Image(systemName: "checkmark").font(.mac(10, weight: .semibold)).foregroundStyle(.water)
                    Text(L.storesKeyMatches(key: invited.fingerprint))
                        .font(.mac(12))
                        .foregroundStyle(.haze)
                }
                if invited.open {
                    HStack(spacing: 5) {
                        Image(systemName: "person.2").font(.mac(10, weight: .semibold)).foregroundStyle(.haze)
                        Text(L.storesAnyoneWithLink).font(.mac(12)).foregroundStyle(.silk)
                    }
                }
                if invited.anonymous {
                    Text(L.storesAnyoneNote)
                        .font(.mac(12))
                        .foregroundStyle(.haze)
                }
                if invited.alreadyJoined {
                    Text(L.storesAlreadyJoinedNote)
                        .font(.mac(12))
                        .foregroundStyle(.haze)
                }
            }
            .padding(EdgeInsets(top: 12, leading: 12, bottom: 10, trailing: 12))
            if !invited.plugins.isEmpty {
                Rectangle().fill(.faintRule).frame(height: 1)
                ForEach(Array(invited.plugins.enumerated()), id: \.element.id) { index, plugin in
                    if index > 0 { Rectangle().fill(.faintRule).frame(height: 1) }
                    HStack(spacing: 8) {
                        Text(plugin.name)
                            .font(.mac(13, weight: .medium))
                            .foregroundStyle(.silk)
                            .lineLimit(1)
                            .layoutPriority(1)
                        Text(plugin.version)
                            .font(.mac(12))
                            .monospacedDigit()
                            .foregroundStyle(.haze)
                        if let standing = plugin.standing {
                            Text(standing)
                                .font(.mac(12))
                                .foregroundStyle(.water)
                                .lineLimit(1)
                        }
                        Spacer(minLength: 8)
                        Text(plugin.contactsLine)
                            .font(.mac(12))
                            .foregroundStyle(.haze)
                            .lineLimit(1)
                            .truncationMode(.tail)
                    }
                    .padding(.horizontal, 12)
                    .frame(height: 32)
                    .accessibilityElement(children: .combine)
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(.ground, in: .rect(cornerRadius: 10))
    }
}

/// An invite link to send: for someone new, named, or for another device of yours.
struct InviteSheet: View {
    @Environment(AppStore.self) private var appStore
    @Environment(\.dismiss) private var dismiss
    let store: StoreRow
    let device: Bool
    @State private var name = ""
    @State private var uses = InviteUses.once
    @State private var lasts = InviteLasts.week

    var body: some View {
        let link = appStore.stores.invite.flatMap { $0.store == store.id ? $0 : nil }
        // A server from before the choices makes links that work once, for a week.
        let choices = !device && (appStore.stores.members[store.id]?.inviteChoices ?? true)
        let group = choices && uses != .once
        VStack(alignment: .leading, spacing: 14) {
            Text(device ? L.storesLinkDeviceTitle : L.storesInviteForName(name: link?.forWhom ?? store.name))
                .font(.mac(15, weight: .semibold))
                .foregroundStyle(.silk)
            if let link {
                Text(Self.line(link, device: device))
                    .font(.mac(13))
                    .foregroundStyle(.haze)
                    .fixedSize(horizontal: false, vertical: true)
                Text(link.link)
                    .font(.mac(12, design: .monospaced))
                    .foregroundStyle(.silk)
                    .textSelection(.enabled)
                    .padding(12)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(.ground, in: .rect(cornerRadius: 8))
                InviteQR(link: link.link, device: device)
                HStack {
                    Spacer()
                    ShareLink(item: link.link) { Text(L.actionShare) }
                        .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                    Button(L.actionCopy) { Pasteboard.copy(link.link) }
                        .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                    Button(L.actionDone) { dismiss() }
                        .buttonStyle(PrimaryCapsuleStyle(height: 28))
                        .keyboardShortcut(.defaultAction)
                }
            } else {
                if !device {
                    VStack(alignment: .leading, spacing: 6) {
                        TextField(group ? L.storesInviteGroupName : L.storesTheirName, text: $name)
                            .textFieldStyle(SheetFieldStyle())
                        if group {
                            Text(L.storesInviteGroupDetail).font(.mac(12)).foregroundStyle(.haze)
                        }
                    }
                    if choices {
                        VStack(alignment: .leading, spacing: 8) {
                            choice(L.storesInviteUses, selection: $uses) {
                                Text(L.storesInviteUsesOnce).tag(InviteUses.once)
                                Text(L.storesInviteUsesFive).tag(InviteUses.five)
                                Text(L.storesInviteUsesAny).tag(InviteUses.any)
                            }
                            choice(L.storesInviteLasts, selection: $lasts) {
                                Text(L.storesInviteLastsDay).tag(InviteLasts.day)
                                Text(L.storesInviteLastsWeek).tag(InviteLasts.week)
                                Text(L.storesInviteLastsMonth).tag(InviteLasts.month)
                                Text(L.storesInviteLastsForever).tag(InviteLasts.forever)
                            }
                            Text(Self.reading(uses: uses, lasts: lasts))
                                .font(.mac(12)).foregroundStyle(.haze)
                                .fixedSize(horizontal: false, vertical: true)
                        }
                    } else {
                        Text(L.storesServerOlderInvites)
                            .font(.mac(12)).foregroundStyle(.haze)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
                HStack {
                    Spacer()
                    Button(L.actionCancel) { dismiss() }
                        .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                        .keyboardShortcut(.cancelAction)
                    Button(L.storesMakeLink) {
                        appStore.createInvite(store.id, for: device ? nil : name,
                                              uses: choices ? uses : nil, lasts: choices ? lasts : nil)
                    }
                    .buttonStyle(PrimaryCapsuleStyle(height: 28))
                    .keyboardShortcut(.defaultAction)
                    .disabled((!device && !group && name.trimmingCharacters(in: .whitespaces).isEmpty)
                              || appStore.stores.isWorking(.invite(store: store.id)))
                }
            }
        }
        .padding(24)
        .sheetWidth(480)
        .background(.raised)
        .onAppear { appStore.stores.clearInvite() }
        .holdsInput(name)
        .task { if !device { appStore.loadMembers(store.id) } }
    }

    private func choice<V: Hashable, Content: View>(
        _ title: String, selection: Binding<V>, @ViewBuilder content: () -> Content
    ) -> some View {
        HStack {
            Text(title).font(.mac(13)).foregroundStyle(.silk)
            Spacer()
            Picker(title, selection: selection, content: content)
                .labelsHidden()
                .pickerStyle(.menu)
                .fixedSize()
        }
    }

    /// The made link's line: who it is for and what it allows.
    static func line(_ link: InviteLinkView, device: Bool) -> String {
        if device { return L.storesLinkOnceUntil(until: expiry(link.expiresAt)) }
        if link.max == 1, let who = link.forWhom {
            return L.storesLinkSendOnce(who: who, until: expiry(link.expiresAt))
        }
        if link.max == 1, link.until != nil || link.forWhom == nil {
            return link.until == nil ? L.storesInviteOnceForever : L.storesLinkSendOnceAnyone(until: expiry(link.expiresAt))
        }
        return works(max: Int(link.max), until: link.until)
    }

    /// What the choices read back, before the link is made.
    static func reading(uses: InviteUses, lasts: InviteLasts) -> String {
        let days: Int? = switch lasts { case .day: 1; case .week: 7; case .month: 30; case .forever: nil }
        let until = days.flatMap { Calendar.current.date(byAdding: .day, value: $0, to: Date()) }
            .map { Int64($0.timeIntervalSince1970) }
        let max = switch uses { case .once: 1; case .five: 5; case .any: 0 }
        return works(max: max, until: until)
    }

    /// "Works 5 times, until 7 November." and its forms.
    static func works(max: Int, until: Int64?) -> String {
        let day = until.map { day($0) }
        switch (max, day) {
        case (1, nil): return L.storesInviteOnceForever
        case (1, let day?): return L.storesInviteOnceUntil(until: day)
        case (0, nil): return L.storesInviteWorksAnyForever
        case (0, let day?): return L.storesInviteWorksAny(until: day)
        case (_, nil): return L.storesInviteWorksTimesForever(count: max)
        case (_, let day?): return L.storesInviteWorksTimes(count: max, until: day)
        }
    }

    static func day(_ seconds: Int64) -> String {
        Date(timeIntervalSince1970: TimeInterval(seconds)).formatted(date: .long, time: .omitted)
    }

    static func expiry(_ seconds: Int64) -> String {
        Date(timeIntervalSince1970: TimeInterval(seconds)).formatted(date: .abbreviated, time: .shortened)
    }

    /// A past moment as a short word: "today", "yesterday", or the date.
    static func when(_ seconds: Int64) -> String {
        let date = Date(timeIntervalSince1970: TimeInterval(seconds))
        let calendar = Calendar.current
        if calendar.isDateInToday(date) { return L.dayToday.lowercased() }
        if calendar.isDateInYesterday(date) { return L.dayYesterday.lowercased() }
        return date.formatted(date: .abbreviated, time: .omitted)
    }
}

/// A store's members and unused invites, for its owner.
struct MembersSheet: View {
    @Environment(AppStore.self) private var appStore
    @Environment(\.dismiss) private var dismiss
    let store: StoreRow
    /// The member Remove asks about, until the viewer answers.
    @State private var removing: StoreMemberRow?
    /// The invite Cancel asks about, when people joined with it.
    @State private var cancelling: StoreInviteRow?
    /// The invites whose uses are showing.
    @State private var showingUses: Set<String> = []

    var body: some View {
        let list = appStore.stores.members[store.id]
        VStack(alignment: .leading, spacing: 14) {
            Text(L.storesMembersOf(name: store.name))
                .font(.mac(15, weight: .semibold))
                .foregroundStyle(.silk)
            if let list {
                VStack(spacing: 0) {
                    ForEach(list.members, id: \.id) { member in
                        HStack {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(member.you ? L.partyMemberYou(name: member.name) : member.name).foregroundStyle(.silk)
                                Text(member.owner ? L.storesOwnerDevices(devices: L.storesDevicesCount(count: Int(member.devices)))
                                                  : L.storesDevicesCount(count: Int(member.devices)))
                                    .font(.mac(12)).foregroundStyle(.haze)
                            }
                            Spacer()
                            if !member.you {
                                Button(L.actionRemove) { removing = member }
                                    .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                            }
                        }
                        .font(.mac(13))
                        .padding(.vertical, 8)
                    }
                    Text(L.storesInviteLinks)
                        .font(.mac(12, weight: .semibold)).foregroundStyle(.haze)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.top, 12)
                    if list.invites.isEmpty {
                        Text(L.storesNoInvites)
                            .font(.mac(12)).foregroundStyle(.haze)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(.vertical, 8)
                    }
                    ForEach(list.invites, id: \.id) { invite in
                        inviteRow(invite, members: list.members, ended: false)
                    }
                    if !list.ended.isEmpty {
                        Text(L.storesInvitesEnded)
                            .font(.mac(12, weight: .semibold)).foregroundStyle(.haze)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(.top, 12)
                        ForEach(list.ended, id: \.id) { invite in
                            inviteRow(invite, members: list.members, ended: true)
                        }
                    }
                }
                .confirmationDialog(
                    L.storesCancelInvite,
                    isPresented: Binding(get: { cancelling != nil }, set: { if !$0 { cancelling = nil } }),
                    titleVisibility: .visible,
                    presenting: cancelling
                ) { invite in
                    Button(L.storesCancelInvite, role: .destructive) {
                        appStore.cancelInvite(invite.id, in: store.id)
                    }
                    Button(L.actionCancel, role: .cancel) {}
                } message: { invite in
                    Text(L.storesCancelInviteDetail(count: invite.uses.filter { !$0.removed }.count))
                }
                .confirmationDialog(
                    removing.map { L.storesRemoveMemberTitle(name: $0.name) } ?? "",
                    isPresented: Binding(get: { removing != nil }, set: { if !$0 { removing = nil } }),
                    titleVisibility: .visible,
                    presenting: removing
                ) { member in
                    Button(L.storesRemoveMember, role: .destructive) {
                        appStore.removeMember(member.id, from: store.id)
                    }
                    Button(L.actionCancel, role: .cancel) {}
                } message: { member in
                    Text(L.storesRemoveMemberDetail)
                }
            } else {
                ProgressView().controlSize(.small)
            }
            HStack {
                Spacer()
                Button(L.actionDone) { dismiss() }
                    .buttonStyle(PrimaryCapsuleStyle(height: 28))
                    .keyboardShortcut(.defaultAction)
            }
        }
        .padding(24)
        .sheetWidth(480)
        .background(.raised)
        .task { appStore.loadMembers(store.id) }
    }

    /// One invite link: its name, how much it was used and until when, or why it ended, and
    /// who joined with it.
    @ViewBuilder
    private func inviteRow(_ invite: StoreInviteRow, members: [StoreMemberRow], ended: Bool) -> some View {
        let open = showingUses.contains(invite.id)
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text(invite.device ? L.storesInviteUnnamed : invite.name).foregroundStyle(.silk)
                    Text(Self.detail(invite)).font(.mac(12)).foregroundStyle(.haze)
                }
                Spacer()
                if !invite.uses.isEmpty {
                    Button(L.storesInviteShowUses) {
                        if open { showingUses.remove(invite.id) } else { showingUses.insert(invite.id) }
                    }
                    .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                }
                if !ended {
                    Button(L.storesCancelInvite) {
                        if invite.uses.contains(where: { !$0.removed }) {
                            cancelling = invite
                        } else {
                            appStore.cancelInvite(invite.id, in: store.id)
                        }
                    }
                    .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                }
            }
            if open {
                ForEach(invite.uses, id: \.member) { use in
                    HStack {
                        Text(L.storesInviteJoined(name: use.name, when: InviteSheet.when(use.at)))
                            .font(.mac(12)).foregroundStyle(.haze)
                        Spacer()
                        if !use.removed, let member = members.first(where: { $0.id == use.member && !$0.you }) {
                            Button(L.actionRemove) { removing = member }
                                .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                        }
                    }
                    .padding(.leading, 12)
                }
            }
        }
        .font(.mac(13))
        .padding(.vertical, 8)
    }

    /// "Used 2 of 5 · Until 7 Nov", "Not used yet · Until 7 Nov", or why it ended.
    static func detail(_ invite: StoreInviteRow) -> String {
        if let ended = invite.ended {
            let when = InviteSheet.when(ended.at)
            switch ended.reason {
            case .usedUp: return L.storesInviteUsedUp(when: when)
            case .ranOut: return L.storesInviteRanOut(when: when)
            case .cancelled: return L.storesInviteCancelledOn(when: when)
            }
        }
        let used = invite.used == 0 ? L.storesInviteNotUsed
            : invite.max == 0 ? L.storesInviteUsed(used: Int(invite.used))
            : L.storesInviteUsedOf(used: Int(invite.used), max: Int(invite.max))
        guard let until = invite.until else { return used }
        return used + " · " + L.storesInvitedUntil(until: InviteSheet.day(until))
    }
}

/// Who can join a store, for its owner: only people they invite, or anyone with the store's link.
/// While it is open, the link to copy and share; either way, the devices that joined with it,
/// which have no names, to remove one at a time or all at once.
struct OpenJoinSheet: View {
    @Environment(AppStore.self) private var appStore
    @Environment(\.dismiss) private var dismiss
    let store: StoreRow
    @State private var removingAll = false
    /// Long enough without an answer that the store's server is taken to have no open joining.
    @State private var waited = false

    var body: some View {
        let view = appStore.stores.openJoin[store.id]
        let working = appStore.stores.isWorking(.openJoin(store: store.id))
            || appStore.stores.isWorking(.removeAnonymous(store: store.id))
        VStack(alignment: .leading, spacing: 14) {
            Text(L.storesWhoCanJoinTitle(name: store.name))
                .font(.mac(15, weight: .semibold))
                .foregroundStyle(.silk)
            if let view {
                Toggle(isOn: Binding(
                    get: { view.open },
                    set: { appStore.setOpenJoin(store.id, open: $0) }
                )) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(L.storesAnyoneWithStoreLink).foregroundStyle(.silk)
                        Text(view.open
                             ? L.storesOpenJoinOnDetail
                             : L.storesOpenJoinOffDetail)
                            .font(.mac(12))
                            .foregroundStyle(.haze)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
                .toggleStyle(.switch)
                .disabled(working)
                .font(.mac(13))
                if let link = view.link {
                    Text(link)
                        .font(.mac(12, design: .monospaced))
                        .foregroundStyle(.silk)
                        .textSelection(.enabled)
                        .padding(12)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .background(.ground, in: .rect(cornerRadius: 8))
                    HStack {
                        Spacer()
                        ShareLink(item: link) { Text(L.actionShare) }
                            .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                        Button(L.actionCopy) { Pasteboard.copy(link) }
                            .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                    }
                }
                if view.devices > 0 {
                    Rectangle().fill(.faintRule).frame(height: 1)
                    HStack {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(L.storesDevicesJoined(count: Int(view.devices)))
                                .foregroundStyle(.silk)
                            Text(view.open ? L.storesOpenJoinNoNames : L.storesOpenJoinKeepWorking)
                                .font(.mac(12)).foregroundStyle(.haze)
                        }
                        Spacer()
                        Button(view.devices == 1 ? L.actionRemove : L.storesRemoveAll) { removingAll = true }
                            .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                            .disabled(working)
                    }
                    .font(.mac(13))
                    ForEach(view.recent, id: \.id) { device in
                        HStack {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(L.storesDeviceJoinedAt(when: Self.when(device.joinedAt))).foregroundStyle(.silk)
                                Text(device.lastSeen.map { L.storesDeviceSeenAt(when: Self.when($0)) } ?? L.storesDeviceNotSeen)
                                    .font(.mac(12)).foregroundStyle(.haze)
                            }
                            Spacer()
                            Button(L.actionRemove) { appStore.removeAnonymous(device.id, from: store.id) }
                                .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                                .disabled(working)
                        }
                        .font(.mac(13))
                    }
                    if view.devices > UInt32(view.recent.count) {
                        Text(L.storesNewestListed(count: view.recent.count))
                            .font(.mac(12)).foregroundStyle(.haze)
                    }
                }
            } else if waited {
                Text(L.storesServerOlder)
                    .font(.mac(13)).foregroundStyle(.haze)
                    .fixedSize(horizontal: false, vertical: true)
            } else {
                ProgressView().controlSize(.small)
            }
            HStack {
                Spacer()
                Button(L.actionDone) { dismiss() }
                    .buttonStyle(PrimaryCapsuleStyle(height: 28))
                    .keyboardShortcut(.defaultAction)
            }
        }
        .padding(24)
        .sheetWidth(520)
        .background(.raised)
        .task {
            appStore.loadMembers(store.id)
            try? await Task.sleep(for: .seconds(4))
            waited = true
        }
        .confirmationDialog(L.storesRemoveDevicesTitle, isPresented: $removingAll) {
            Button(L.actionRemove, role: .destructive) { appStore.removeAnonymous(nil, from: store.id) }
        } message: {
            Text(L.storesRemoveDevicesDetail(name: store.name))
        }
    }

    static func when(_ seconds: Int64) -> String {
        Date(timeIntervalSince1970: TimeInterval(seconds)).formatted(.relative(presentation: .named))
    }
}

/// The local plugins an owner could publish to the store.
struct PublishSheet: View {
    @Environment(AppStore.self) private var appStore
    @Environment(\.dismiss) private var dismiss
    let store: StoreRow

    var body: some View {
        let plugins = appStore.stores.localPlugins[store.id]
        VStack(alignment: .leading, spacing: 14) {
            Text(L.storesPublishTitle(name: store.name))
                .font(.mac(15, weight: .semibold))
                .foregroundStyle(.silk)
            if let plugins {
                if plugins.isEmpty {
                    Text(L.storesPublishNone)
                        .font(.mac(13)).foregroundStyle(.haze)
                }
                ForEach(plugins, id: \.id) { plugin in
                    HStack {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(L.storesPluginNameVersion(name: plugin.name, version: plugin.version)).foregroundStyle(.silk)
                            Text(plugin.published.map { L.storesPublishOffered(version: $0) } ?? L.storesPublishNotYet)
                                .font(.mac(12)).foregroundStyle(.haze)
                        }
                        Spacer()
                        Button(L.actionPublish) { appStore.publishPlugin(plugin.id, to: store.id) }
                            .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                            .disabled(plugin.published == plugin.version)
                    }
                    .font(.mac(13))
                }
            } else {
                ProgressView().controlSize(.small)
            }
            HStack {
                Spacer()
                Button(L.actionDone) { dismiss() }
                    .buttonStyle(PrimaryCapsuleStyle(height: 28))
                    .keyboardShortcut(.defaultAction)
            }
        }
        .padding(24)
        .sheetWidth(480)
        .background(.raised)
        .task { appStore.loadLocalPlugins(for: store.id) }
    }
}

/// The system pasteboard.
enum Pasteboard {
    /// The invite link on the pasteboard, when there is one.
    @MainActor
    static var invite: String? {
        #if os(macOS)
        let text = NSPasteboard.general.string(forType: .string)
        #else
        let text = UIPasteboard.general.string
        #endif
        return text.flatMap { URL(string: $0.trimmingCharacters(in: .whitespacesAndNewlines)) }
            .flatMap(InviteLinks.invite(in:))
    }

    @MainActor
    static func copy(_ text: String) {
        #if os(macOS)
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
        #else
        UIPasteboard.general.string = text
        #endif
    }
}
