// What the first-party sources show: the Licensed services page and each title's availability, the
// media servers and the add-a-server flow, and the folders of Your media. Each model keeps what the
// engine last said; the commands that change them go through `AppStore` (AppStore+FirstParty).

import KaeriKit
import Foundation
import Observation

// MARK: Licensed services

/// A return from a licensed service that asks whether the episode was watched.
public struct LicensedPrompt: Equatable, Sendable {
    public let id: AnilistId
    public let episode: UInt32
    public let service: String
    public let question: String
    public let done: String
}

@Observable @MainActor
public final class LicensedModel {
    /// The Licensed services page, `nil` until the engine has said it.
    public private(set) var view: LicensedView?
    /// What each title page says about services.
    public private(set) var availability: [AnilistId: TitleAvailability] = [:]
    /// The return notice waiting for an answer.
    public private(set) var prompt: LicensedPrompt?

    public init() {}

    func apply(_ view: LicensedView) {
        self.view = view
    }

    func apply(_ availability: TitleAvailability) {
        self.availability[availability.id] = availability
    }

    func returned(_ value: LicensedReturn) {
        prompt = LicensedPrompt(
            id: value.id, episode: value.episode, service: value.service,
            question: value.prompt.text, done: value.done.text)
    }

    func answered() {
        prompt = nil
    }

    /// Case B: the service and address episode `number` opens, its own page when the service
    /// published one and else the title's. `nil` in the other cases.
    public func episodeLink(_ id: AnilistId, _ number: UInt32) -> (service: String, url: String)? {
        guard let a = availability[id], a.case == .onService, let main = a.main else { return nil }
        if let episode = a.episodes.first(where: { $0.episode == number }) {
            return (episode.service, episode.url)
        }
        return (main.service, main.url)
    }

    /// The services the viewer pays for, as the page last said.
    public var yours: [String] { view?.settings.services ?? [] }

    /// Whether `service` is ticked.
    public func uses(_ service: String) -> Bool {
        yours.contains { $0.caseInsensitiveCompare(service) == .orderedSame }
    }

    /// The line of the Sources row: the services chosen and the region, or the invitation to choose.
    public func rowLine(regionName: (String) -> String) -> String? {
        guard let view else { return nil }
        let chosen = view.settings.services
        guard !chosen.isEmpty else { return nil }
        let region = view.region.map(regionName)
        return ([chosen.formatted(.list(type: .and))] + (region.map { [$0] } ?? [])).joined(separator: " · ")
    }
}

// MARK: Media servers

/// The step the add-a-server sheet is on.
public enum AddServerStep: Equatable, Sendable {
    case address
    case signIn
    case quickConnect
    case libraries
    /// The server's certificate is not trusted: Check this server.
    case certificate
}

/// The media servers, and the sheet that adds one.
@Observable @MainActor
public final class ServersModel {
    public private(set) var rows: [ServerRow] = []
    public private(set) var loaded = false

    // The add flow.
    public private(set) var step: AddServerStep = .address
    public private(set) var found: [FoundServer] = []
    public private(set) var discovering = false
    public private(set) var discoveryDone = false
    public private(set) var checked: CheckedServer?
    /// The step is waiting on the server.
    public private(set) var busy = false
    public private(set) var failure: ServerFailure?
    public private(set) var failedStep: ServerStep?
    /// The certificate to compare on the Check this server step.
    public private(set) var certificate: CertificateCheck?
    private var waits = MatchWaits()
    public private(set) var code: String?
    public private(set) var signedIn: String?
    public private(set) var choices: [LibraryChoice] = []
    /// The server added, once its libraries are chosen and saved: the sheet closes on it.
    public private(set) var added: String?

    public init() {}

    /// A fresh sheet.
    public func begin() {
        step = .address
        found = []
        discovering = false
        discoveryDone = false
        checked = nil
        busy = false
        failure = nil
        failedStep = nil
        certificate = nil
        code = nil
        signedIn = nil
        choices = []
        added = nil
    }

    /// The Fixtures app's way to the Check this server step.
    public func fixtureCertificate(_ check: CertificateCheck) {
        certificate = check
        step = .certificate
    }

    func discoveryStarted() {
        discovering = true
        discoveryDone = false
        found = []
    }

    func checking() {
        busy = true
        failure = nil
    }

    func signingIn() {
        busy = true
        failure = nil
    }

    func quickConnecting() {
        step = .quickConnect
        code = nil
        failure = nil
    }

    public func toPassword() {
        step = .signIn
        failure = nil
    }

    func saved(_ server: String) {
        added = server
    }

    func apply(_ event: ServerEvent) {
        switch event {
        case .found(let server):
            if !found.contains(where: { $0.id == server.id }) { found.append(server) }
        case .discoveryDone:
            discovering = false
            discoveryDone = true
        case .checked(let server):
            busy = false
            failure = nil
            checked = server
            certificate = nil
            step = .signIn
        case .failed(let step, let failure):
            busy = false
            failedStep = step
            self.failure = failure
        case .certificate(let check):
            busy = false
            failure = nil
            certificate = check
            step = .certificate
        case .quickConnectCode(let code):
            self.code = code
            failure = nil
        case .signedIn(let server):
            busy = false
            signedIn = server
            step = .libraries
        case .libraries(let server, let libraries):
            if signedIn == server || signedIn == nil { choices = libraries }
        case .servers(let rows):
            self.rows = rows
            loaded = true
            for row in rows { waits.note(row.id, row.matching) }
        }
    }

    public func toggle(_ library: String) {
        guard let at = choices.firstIndex(where: { $0.id == library }) else { return }
        let old = choices[at]
        choices[at] = LibraryChoice(id: old.id, name: old.name, kind: old.kind, ticked: !old.ticked)
    }

    public func row(_ id: String) -> ServerRow? { rows.first { $0.id == id } }

    /// Seconds of AniList's wait left for a server's matching, counted from when it was sent.
    public func waitLeft(_ id: String, at now: Date) -> Int? { waits.left(id, at: now) }
}

/// AniList's wait is sent once, as it starts; the apps count it down. This keeps when each ends.
struct MatchWaits {
    private var ends: [String: (sent: UInt32, at: Date)] = [:]

    mutating func note(_ key: String, _ progress: MatchProgress?) {
        guard let seconds = progress?.waitSeconds else {
            ends[key] = nil
            return
        }
        // The same number again is the same wait; a new one starts a new count.
        if ends[key]?.sent != seconds { ends[key] = (seconds, Date().addingTimeInterval(TimeInterval(seconds))) }
    }

    func left(_ key: String, at now: Date) -> Int? {
        guard let end = ends[key]?.at else { return nil }
        return max(0, Int(end.timeIntervalSince(now).rounded(.up)))
    }
}

// MARK: Your media

@Observable @MainActor
public final class LocalLibraryModel {
    public private(set) var view: LocalView?
    /// The answer to the last Fix this match search.
    public private(set) var matches: LocalMatches?
    /// A search is out and unanswered.
    public private(set) var searching = false
    private var waits = MatchWaits()

    public init() {}

    func apply(_ view: LocalView) {
        self.view = view
        for folder in view.folders {
            if case .matching(let progress)? = folder.scan { waits.note(folder.path, progress) } else { waits.note(folder.path, nil) }
        }
    }

    func searchStarted() {
        searching = true
    }

    func apply(matches group: String, query: String, choices: [MatchChoice], offline: Bool) {
        searching = false
        matches = LocalMatches(group: group, query: query, choices: choices, offline: offline)
    }

    /// Forget the last answer, as the sheet closes.
    public func clearMatches() {
        matches = nil
        searching = false
    }

    public func waitLeft(_ folder: String, at now: Date) -> Int? { waits.left(folder, at: now) }

    /// Files waiting for Set episode.
    public var unnumbered: [UnnumberedFile] { view?.unnumbered ?? [] }

    public func groups(in folder: String, _ state: GroupState) -> [LocalGroup] {
        groups(state).filter { $0.folder == folder }
    }

    public var folders: [FolderRow] { view?.folders ?? [] }

    public func groups(_ state: GroupState) -> [LocalGroup] {
        (view?.groups ?? []).filter { $0.state == state }
    }

    /// Titles in the folders that need a look, for the row's line.
    public var needsLook: Int { groups(.needsLook).count }
}

/// What a Fix this match search found for a group.
public struct LocalMatches: Equatable, Sendable {
    public let group: String
    public let query: String
    public let choices: [MatchChoice]
    /// AniList could not be asked.
    public let offline: Bool
}
