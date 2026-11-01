// The first-party sources: licensed services, media servers and Your media. Each sends the engine's
// own commands, and the answers come back through `apply` to the models in `FirstPartyModels`.

import KaeriKit
import KaeriStrings
import Foundation

extension AppStore {
    // MARK: Licensed services

    /// Ask for the Licensed services page. `region` is the device's own country.
    public func loadLicensed(region: String? = Locale.current.region?.identifier) {
        send(.licensed(.load(systemRegion: region)))
    }

    public func setLicensedServices(_ services: [String]) {
        send(.licensed(.setServices(services: services)))
    }

    public func setLicensedRegion(_ region: String?) {
        send(.licensed(.setRegion(region: region)))
    }

    public func setLicensedShowOthers(_ show: Bool) {
        send(.licensed(.setShowOthers(show: show)))
    }

    /// Ask what `id`'s page says about licensed services. `playsHere` is whether a source lists it.
    public func loadAvailability(_ id: AnilistId, playsHere: Bool) {
        send(.licensed(.loadAvailability(id: id, playsHere: playsHere)))
    }

    /// Whether title `id`'s page knows what to offer in its main button: its episodes are here,
    /// so whether a source lists it, and the services have answered for that. Until then Add a
    /// source or Watch on a service would show only to give way to the other.
    public func offersKnown(_ id: AnilistId) -> Bool {
        guard titles.episodes[id] != nil, let said = licensed.availability[id] else { return false }
        return said.playsHere == !titles.unlisted(id)
    }

    /// Title `id`'s episodes landed: what the services say is asked again when it was answered
    /// for a source listing it that turned out not to, or the other way.
    func episodesLanded(_ id: AnilistId) {
        let playsHere = !titles.unlisted(id)
        if let said = licensed.availability[id], said.playsHere != playsHere {
            loadAvailability(id, playsHere: playsHere)
        }
    }

    /// The viewer leaves for a service: the engine remembers it, and the app opens `url`.
    public func openLicensed(_ id: AnilistId, service: String, url: String, episode: UInt32?) {
        send(.licensed(.open(id: id, service: service, url: url, episode: episode)))
    }

    /// The app came to the front: the engine says whether the viewer is back from a service.
    public func returnedToFront() {
        send(.licensed(.returned))
    }

    /// Mark watched on the return notice.
    public func markReturnWatched() {
        guard let prompt = licensed.prompt else { return }
        licensed.answered()
        send(.setWatched(id: prompt.id, episodes: [String(prompt.episode)], watched: true))
        notices.post(
            Notice(kind: .info, text: prompt.done, hint: nil),
            action: NoticeButton(title: L.actionUndo) { [weak self] in
                self?.send(.setWatched(id: prompt.id, episodes: [String(prompt.episode)], watched: false))
            })
    }

    // MARK: Media servers

    public func loadServers() {
        send(.server(.load))
    }

    /// Open the add-a-server sheet afresh. Discovery runs where the device may broadcast.
    public func beginAddServer(discover: Bool) {
        servers.begin()
        if discover {
            servers.discoveryStarted()
            send(.server(.discover))
        }
    }

    public func checkServer(address: String) {
        let address = address.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !address.isEmpty else { return }
        servers.checking()
        send(.server(.check(address: address)))
    }

    public func signInToServer(user: String, password: String) {
        guard let address = servers.checked?.address else { return }
        servers.signingIn()
        send(.server(.signIn(address: address, user: user, password: password)))
    }

    public func startQuickConnect() {
        guard let address = servers.checked?.address else { return }
        servers.quickConnecting()
        send(.server(.startQuickConnect(address: address)))
    }

    public func cancelQuickConnect() {
        send(.server(.cancelQuickConnect))
    }

    /// Save the ticked libraries and close the sheet.
    public func finishAddingServer(report: Bool) {
        guard let server = servers.signedIn else { return }
        let ticked = servers.choices.filter(\.ticked).map(\.id)
        send(.server(.setLibraries(server: server, libraries: ticked)))
        if !report { send(.server(.setReport(server: server, report: false))) }
        servers.saved(server)
        send(.server(.load))
    }

    public func setServerLibraries(_ server: String, _ libraries: [String]) {
        send(.server(.setLibraries(server: server, libraries: libraries)))
    }

    public func setServerReport(_ server: String, _ report: Bool) {
        send(.server(.setReport(server: server, report: report)))
    }

    public func refreshServer(_ server: String) {
        send(.server(.refresh(server: server)))
    }

    public func removeServer(_ server: String) {
        send(.server(.remove(server: server)))
        notices.post(
            Notice(kind: .info, text: L.serversRemoved(name: servers.row(server)?.name ?? ""), hint: nil),
            action: NoticeButton(title: L.actionUndo) { [weak self] in
                self?.send(.server(.restore(server: server)))
                self?.send(.server(.load))
            })
    }

    public func signOutOfServer(_ server: String) {
        send(.server(.signOut(server: server)))
    }

    /// Allow the certificate the step shows, and check the server again.
    public func trustCertificate() {
        guard let check = servers.certificate else { return }
        servers.checking()
        send(.server(.trustCertificate(address: check.address, fingerprint: check.fingerprint)))
    }

    // MARK: Your media

    public func loadLocal() {
        send(.local(.load))
    }

    public func addFolder(_ path: String) {
        send(.local(.addFolder(path: path)))
    }

    public func removeFolder(_ path: String) {
        send(.local(.removeFolder(path: path)))
        notices.post(
            Notice(kind: .info, text: L.libraryRemoved(name: URL(filePath: path).lastPathComponent), hint: nil),
            action: NoticeButton(title: L.actionUndo) { [weak self] in
                self?.send(.local(.restoreFolder(path: path)))
            })
    }

    /// Fix this match: ask AniList for titles for a group. An empty query searches the name read
    /// from the files.
    public func findLocalMatch(group: String, query: String) {
        local.searchStarted()
        send(.local(.findMatch(group: group, query: query)))
    }

    public func setEpisode(group: String, path: String, season: UInt32, episode: UInt32) {
        send(.local(.setEpisode(group: group, path: path, season: season, episode: episode)))
    }

    public func rescanFolders() {
        send(.local(.scan))
    }

    public func chooseLocalMatch(group: String, title: AnilistId) {
        send(.local(.choose(group: group, anilist: title)))
    }

    public func keepLocal(group: String) {
        send(.local(.keep(group: group)))
    }

    // MARK: Answers

    func applyFirstParty(_ event: Event) {
        switch event {
        case .licensed(let view): licensed.apply(view)
        case .availability(let availability): licensed.apply(availability)
        case .licensedReturn(let value):
            licensed.returned(value)
            let prompt = licensed.prompt
            notices.post(
                Notice(kind: .info, text: value.prompt.text, hint: nil),
                action: NoticeButton(title: L.licensedMarkWatched) { [weak self] in
                    guard self?.licensed.prompt == prompt else { return }
                    self?.markReturnWatched()
                })
        case .server(let event):
            servers.apply(event)
            if case .signedIn(let server) = event { send(.server(.loadLibraries(server: server))) }
        case .local(.view(let view)): local.apply(view)
        case .local(.matches(let group, let query, let choices, let offline)):
            local.apply(matches: group, query: query, choices: choices, offline: offline)
        default: break
        }
    }
}

extension TitleAvailability {
    /// Whether it was answered for a title a source lists.
    public var playsHere: Bool { self.case == .playsHere || self.case == .playsAlsoOn }
}
