// The first-party sources on iPhone, in the Settings sheet's Sources page: Media servers and
// Licensed services as sections, and the page each row opens. Your media is not offered here in
// this release (the phones read no folders yet).

#if os(iOS)
import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

struct PhoneFirstPartySections: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        Group {
            if !store.servers.rows.isEmpty {
                Section {
                    ForEach(store.servers.rows, id: \.id) { row in
                        NavigationLink(value: PhoneSettingsPage.server(row.id)) {
                            HStack(spacing: 12) {
                                RowCover(title: row.name, url: nil, color: nil)
                                PhoneRowTitle(title: row.name, note: line(row))
                                if row.signedOut || row.signedOutHere || row.notAnswering {
                                    Spacer()
                                    Circle().fill(.vermilion).frame(width: 8, height: 8)
                                }
                            }
                        }
                        .listRowBackground(Palette.raised)
                    }
                } header: {
                    PhoneSectionHeader(L.sourcesSectionServers)
                }
            }
            Section {
                NavigationLink(value: PhoneSettingsPage.licensed) {
                    HStack(spacing: 12) {
                        RowCover(title: L.licensedTitle, url: nil, color: nil)
                        PhoneRowTitle(
                            title: L.licensedTitle,
                            note: store.licensed.rowLine(regionName: AppStore.regionName) ?? L.licensedRowNone)
                    }
                }
                .listRowBackground(Palette.raised)
            } header: {
                PhoneSectionHeader(L.sourcesSectionLicensed)
            }
        }
        .task {
            store.loadServers()
            store.loadLicensed()
        }
    }

    private func line(_ row: ServerRow) -> String {
        if row.signedOutHere { return L.serversRowSignedOutHere }
        if row.signedOut { return L.serversRowSignedOut }
        if row.notAnswering { return FirstPartySections.notAnsweringLine(row.lastSeen, now: .now) }
        if let progress = row.matching {
            return FirstPartySections.matchingLine(progress, left: store.servers.waitLeft(row.id, at: .now))
        }
        let summary = L.serversRowSummaryApple(series: Int(row.series))
        let unmatched = Int(row.series) - Int(row.matched)
        return unmatched > 0 ? summary + " · " + L.serversRowNeedsLook(count: unmatched) : summary
    }
}

struct PhoneLicensedPage: View {
    @Environment(AppStore.self) private var store
    @State private var showAll = false

    var body: some View {
        let view = store.licensed.view
        let available = view?.available ?? []
        let shown = showAll ? available : Array(available.prefix(8))
        PhoneList {
            Section {
                Text(L.licensedBody).font(.ui(15)).foregroundStyle(.haze)
                    .listRowBackground(Color.clear)
                    .listRowInsets(EdgeInsets(top: 0, leading: 4, bottom: 0, trailing: 4))
            }
            Section {
                Picker(selection: Binding(
                    get: { view?.settings.region ?? "" },
                    set: { store.setLicensedRegion($0.isEmpty ? nil : $0) }
                )) {
                    Text(L.licensedRegionSystem).tag("")
                    ForEach(view?.regions ?? [], id: \.self) { Text(AppStore.regionName($0)).tag($0) }
                } label: {
                    Text(L.licensedRegion).font(.ui(17)).foregroundStyle(.silk)
                }
                .tint(.haze)
                .listRowBackground(Palette.raised)
            } footer: {
                Text(L.licensedRegionLine)
            }
            Section {
                if available.isEmpty { Text(L.licensedLoading).foregroundStyle(.haze) }
                ForEach(shown, id: \.self) { service in
                    Button { toggle(service) } label: {
                        HStack {
                            Text(service).font(.ui(17)).foregroundStyle(.silk)
                            Spacer()
                            if store.licensed.uses(service) { Image(systemName: "checkmark").foregroundStyle(.amber) }
                        }
                    }
                    .listRowBackground(Palette.raised)
                    .accessibilityAddTraits(store.licensed.uses(service) ? .isSelected : [])
                }
                if !showAll, available.count > 8 {
                    Button(L.licensedShowAll(count: available.count)) { withAnimation(.standard) { showAll = true } }
                        .listRowBackground(Palette.raised)
                }
            } header: {
                PhoneSectionHeader(L.licensedYours)
            }
            Section {
                Toggle(isOn: Binding(
                    get: { view?.settings.showOthers ?? true }, set: { store.setLicensedShowOthers($0) }
                )) {
                    PhoneRowTitle(title: L.licensedShowOthers)
                }
                .tint(.amber)
                .listRowBackground(Palette.raised)
            } footer: {
                Text(L.licensedShowOthersLine)
            }
        }
        .navigationTitle(L.licensedTitle)
        .navigationBarTitleDisplayMode(.inline)
        .task { store.loadLicensed() }
    }

    private func toggle(_ service: String) {
        var services = store.licensed.yours
        if let at = services.firstIndex(where: { $0.caseInsensitiveCompare(service) == .orderedSame }) {
            services.remove(at: at)
        } else {
            services.append(service)
        }
        store.setLicensedServices(services)
    }
}

struct PhoneServerPage: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.dismiss) private var dismiss
    let id: String

    var body: some View {
        if let row = store.servers.row(id) {
            PhoneList {
                Section {
                    Text(L.serversPageLine(address: row.address, version: row.version, user: row.user))
                        .font(.ui(15)).foregroundStyle(.haze)
                        .listRowBackground(Color.clear)
                        .listRowInsets(EdgeInsets(top: 0, leading: 4, bottom: 0, trailing: 4))
                    if row.signedOut || row.signedOutHere {
                        Button(L.serversSignInAgain) { navigator.addServerShown = true }
                            .listRowBackground(Palette.raised)
                    }
                }
                Section {
                    ForEach(row.libraries, id: \.id) { library in
                        Button { toggle(library, in: row) } label: {
                            HStack {
                                PhoneRowTitle(title: library.name, note: library.kind)
                                Spacer()
                                if library.ticked { Image(systemName: "checkmark").foregroundStyle(.amber) }
                            }
                        }
                        .listRowBackground(Palette.raised)
                        .accessibilityAddTraits(library.ticked ? .isSelected : [])
                    }
                } header: {
                    PhoneSectionHeader(L.serversLibraries)
                }
                Section {
                    Toggle(isOn: Binding(get: { row.report }, set: { store.setServerReport(row.id, $0) })) {
                        PhoneRowTitle(title: L.serversReport)
                    }
                    .tint(.amber)
                    .listRowBackground(Palette.raised)
                } footer: {
                    Text(L.serversReportLine)
                }
                Section {
                    Button(L.serversRefresh) { store.refreshServer(row.id) }
                        .listRowBackground(Palette.raised)
                    if !row.signedOut, !row.signedOutHere {
                        Button(L.serversSignOut) { store.signOutOfServer(row.id) }
                            .listRowBackground(Palette.raised)
                    }
                    Button(L.serversRemove, role: .destructive) {
                        store.removeServer(row.id)
                        dismiss()
                    }
                    .listRowBackground(Palette.raised)
                } footer: {
                    Text(L.serversRemoveLine)
                }
            }
            .navigationTitle(row.name)
            .navigationBarTitleDisplayMode(.inline)
            .sheet(isPresented: Bindable(navigator).addServerShown) { AddServerSheet() }
        }
    }

    private func toggle(_ library: LibraryChoice, in row: ServerRow) {
        var ticked = row.libraries.filter(\.ticked).map(\.id)
        if library.ticked { ticked.removeAll { $0 == library.id } } else { ticked.append(library.id) }
        store.setServerLibraries(row.id, ticked)
    }
}
#endif
