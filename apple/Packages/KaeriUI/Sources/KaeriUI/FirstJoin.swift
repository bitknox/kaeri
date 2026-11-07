import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

/// First run, after pairing: what the other device brings, one switch for each thing. The
/// medium answer is a row with a pop-up, sign-ins one switch, every store and every server its
/// own. Folders stay where they are. What the other device had already answered is skipped, and
/// when it answered everything first run ends here.
struct FirstJoinPaired: View {
    @Environment(AppStore.self) private var store
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    let device: String
    var done: () -> Void = {}
    /// The first-run pages left after this one: the page names the other device answered.
    var skip: (Set<FirstRunPage>) -> Void = { _ in }
    @State private var medium: KaeriIsFor?
    @State private var copySignIns = true
    @State private var offStores: Set<String> = []
    @State private var offServers: Set<String> = []
    /// The waiting line shows after 600 ms and stays 0.3 s; the group waits for that.
    @State private var lookingShown = false
    @State private var ready = false

    /// For shots: the state is staged, so no waiting line is timed.
    private var staged: Bool { store.sync.staged }

    var body: some View {
        let sync = store.sync
        SyncSheetFrame(title: L.pairOk(device: device)) {
            SyncNote(text: L.firstOkLine)
            if ready, let setup = sync.setup {
                group(setup, sync)
                    .transition(.opacity)
            } else if lookingShown || staged {
                HStack(spacing: 8) {
                    ProgressView().controlSize(.small)
                    Text(L.firstJoinLooking(device: device))
                        .font(.plat(mac: 12, touch: 15))
                        .foregroundStyle(.haze)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .transition(.opacity)
            }
        } bottom: {
            SyncButton(title: L.onboardingContinue, primary: true, wide: Device.name != "Mac") { apply() }
        }
        .animation(reduceMotion ? .easeInOut(duration: 0.2) : .standard, value: ready)
        .animation(reduceMotion ? .easeInOut(duration: 0.2) : .standard, value: lookingShown)
        .task {
            guard !staged else { return }
            try? await Task.sleep(for: .milliseconds(600))
            lookingShown = true
        }
        .task(id: sync.setup != nil) {
            guard sync.setup != nil else { return }
            if lookingShown && !staged { try? await Task.sleep(for: .milliseconds(300)) }
            ready = true
        }
        .onChange(of: sync.setup) { _, setup in
            if let setup, medium == nil { medium = setup.medium.map(KaeriIsFor.init) ?? nil }
        }
        .onAppear {
            if medium == nil, let setup = sync.setup { medium = setup.medium.map(KaeriIsFor.init) ?? nil }
        }
    }

    @ViewBuilder
    private func group(_ setup: JoinedSetup, _ sync: SyncModel) -> some View {
        let name = setup.deviceName.isEmpty ? device : setup.deviceName
        let trackers = setup.signedIn.map { AccountsModel.name($0) }.joined(separator: ", ")
        SyncSection(header: L.firstJoinGroup(device: name), footer: nil) {
            if setup.medium != nil {
                mediumRow(name)
            }
            if !setup.signedIn.isEmpty {
                switchRow(L.pairCopySignins(device: name), L.firstJoinSigninsLine(trackers: trackers), $copySignIns)
            }
            ForEach(setup.stores, id: \.address) { offer in
                switchRow(L.firstJoinStore(store: offer.name), L.firstJoinStoreLine(device: name, kind: Device.name),
                          binding(offer.address, in: $offStores))
            }
            ForEach(setup.servers.filter { !$0.added }, id: \.id) { offer in
                switchRow(L.firstJoinServer(host: offer.host),
                          L.firstJoinServerLine(kind: BookServerWords.kind(offer.kind), device: name, store: "the Keychain"), // i18n-skip: the system's name for it
                          binding(offer.id, in: $offServers))
            }
        }
        if let refused = sync.storeRefused {
            SyncProblemLine(text: L.firstJoinStoreRefused(store: refused, owner: name))
        }
        if setup.folders > 0 { SyncNote(text: L.firstJoinFolders(device: name)) }
        if !setup.answered.isEmpty { SyncNote(text: L.firstJoinStepsDone) }
    }

    private func answerName(_ answer: KaeriIsFor) -> String {
        switch answer {
        case .watch: L.mediumAnswerWatch
        case .read: L.mediumAnswerRead
        case .both: L.mediumAnswerBoth
        }
    }

    private func binding(_ id: String, in set: Binding<Set<String>>) -> Binding<Bool> {
        Binding(get: { !set.wrappedValue.contains(id) },
                set: { on in if on { set.wrappedValue.remove(id) } else { set.wrappedValue.insert(id) } })
    }

    private func mediumRow(_ name: String) -> some View {
        let picker = Menu {
            ForEach([KaeriIsFor.watch, .read, .both], id: \.self) { option in
                Button(answerName(option)) { medium = option }
            }
        } label: {
            HStack(spacing: 4) {
                Text(medium.map(answerName) ?? "")
                Image(systemName: "chevron.up.chevron.down").font(.ui(11))
            }
            .foregroundStyle(.silk)
        }
        #if os(macOS)
        return SettingsRow(title: L.firstJoinMedium, note: L.firstJoinMediumLine(device: name)) { picker.menuStyle(.button).fixedSize() }
        #else
        return HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text(L.firstJoinMedium).font(.ui(17)).foregroundStyle(.silk)
                Text(L.firstJoinMediumLine(device: name)).font(.ui(13)).foregroundStyle(.haze)
            }
            Spacer()
            picker
        }
        .listRowBackground(Palette.raised)
        #endif
    }

    private func switchRow(_ title: String, _ line: String, _ isOn: Binding<Bool>) -> some View {
        #if os(macOS)
        SettingsRow(title: title, note: line) {
            Toggle("", isOn: isOn).toggleStyle(.switch).tint(.amber).labelsHidden()
        }
        #else
        Toggle(isOn: isOn) {
            VStack(alignment: .leading, spacing: 2) {
                Text(title).font(.ui(17)).foregroundStyle(.silk)
                Text(line).font(.ui(13)).foregroundStyle(.haze)
            }
        }
        .tint(.amber)
        .listRowBackground(Palette.raised)
        #endif
    }

    private func apply() {
        let sync = store.sync
        if let setup = sync.setup {
            if let medium, setup.medium != nil, !setup.answered.contains(.medium) {
                store.medium.answer(.firstRun, with: medium)
            }
            let stores = setup.stores.map(\.address).filter { !offStores.contains($0) }
            if !stores.isEmpty { sync.joinStores(from: device, stores) }
            let servers = setup.servers.filter { !$0.added && !offServers.contains($0.id) }.map(\.id)
            if !servers.isEmpty { sync.addBookServers(from: device, servers) }
        }
        if copySignIns { sync.copySignIns(from: device, [.trackers]) }
        let answered = Set(sync.setup?.answered ?? [])
        sync.resetPair()
        sync.close()
        sync.noteJoined(device)
        skip(answered)
        done()
    }
}
