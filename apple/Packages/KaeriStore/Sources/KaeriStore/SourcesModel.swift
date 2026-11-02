// Where streams come from, as Providers and Settings show it: each provider, why there are none,
// the torrent proxy's check, and the source pinned for the title on screen.

import KaeriKit
import Foundation
import Observation

/// The providers, as the engine last said them.
@Observable @MainActor
public final class ProvidersModel {
    /// Each provider, in the order failover tries them.
    public private(set) var rows: [ProviderRow] = []
    /// Whether the engine has said the providers at all.
    public private(set) var loaded = false
    /// Why there are no sources, when there are none.
    public private(set) var note: String?
    /// Whether the sources are being rebuilt, because a setting changed which ones exist: from
    /// `SourcesChanged` until the rebuilt list.
    public private(set) var rebuilding = false
    /// The saved torrent proxy's check, `nil` while torrents are off or before it said.
    public private(set) var proxyStatus: ProxyStatus?
    /// A proxy checked before it is saved, `nil` until a form asks and once it is done.
    public private(set) var trial: ProxyTrial?
    /// Whether a check went to the engine and its answer is still to come.
    @ObservationIgnored private var checkSent = false
    /// The check asked for while another was out, sent once that one's answer is in.
    @ObservationIgnored private var waiting: TorrentProxy?
    /// The source pinned for the title on screen, `nil` for automatic.
    public private(set) var preference: String?
    /// The metadata ladder's rungs.
    public private(set) var meta: [MetaSourceRow] = []

    public init() {}

    func apply(_ rows: [ProviderRow]) {
        self.rows = rows
        loaded = true
        rebuilding = false
        if !rows.isEmpty { note = nil }
    }

    func note(_ text: String) {
        note = text
    }

    func sourcesChanged() {
        rebuilding = true
    }

    func proxySaid(_ status: ProxyStatus?) {
        proxyStatus = status
    }

    /// A check asked for: the proxy to send now, or `nil` while an earlier check's answer is
    /// still to come. The engine checks off its loop, so answers can come back in any order: one
    /// check at a time is the only way to know which check an answer is for. The newest waits for
    /// its turn, and one it replaces is never sent.
    func trialStarted(_ proxy: TorrentProxy) -> TorrentProxy? {
        trial = .checking
        guard !checkSent else {
            waiting = proxy
            return nil
        }
        checkSent = true
        return proxy
    }

    /// The answer to the check sent: the check waiting to go next, if any. An answer is kept
    /// only when it is for the check a form still waits for, never an earlier one.
    func trialSaid(_ status: ProxyStatus) -> TorrentProxy? {
        checkSent = false
        if let next = waiting {
            waiting = nil
            checkSent = true
            return next
        }
        if trial == .checking { trial = .said(status) }
        return nil
    }

    /// Done with the check: its sheet closed, or what it checked changed. A check still waiting
    /// to be sent is dropped.
    public func clearTrial() {
        trial = nil
        waiting = nil
    }

    func preference(_ provider: String?) {
        preference = provider
    }

    func meta(_ rows: [MetaSourceRow]) {
        meta = rows
    }

    public func row(_ id: String) -> ProviderRow? {
        rows.first { $0.id == id }
    }
}

extension ProviderRow {
    /// How it stands, as the Providers list says it beside the name.
    public var standing: ProviderStanding {
        if disabled { return .off }
        if heldBack { return .heldBack }
        if !usable || state == "down" { return .failing }
        if state == "degraded" { return .slow }
        return .working
    }
}

/// A proxy checked before anything is saved: under way, then the check's answer.
public enum ProxyTrial: Equatable, Sendable {
    case checking
    case said(ProxyStatus)

    /// The check passed: the proxy may be saved.
    public var passed: Bool {
        if case .said(.passed) = self { true } else { false }
    }

    /// The check failed, and nothing was saved.
    public var failed: Bool {
        if case .said(let status) = self { status != .passed } else { false }
    }
}

/// How a provider stands, in the words and the dot the Providers list gives it.
public enum ProviderStanding: Equatable, Sendable {
    case working, slow, failing, heldBack
    /// Switched off by name: kept, and not tried until it is on again.
    case off
}
