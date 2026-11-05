// Pairing by a typed code: this device's DNS-SD for `_kaeri-sync._tcp`. While a code shows, the
// device is registered under its name on the port the engine listens on (DNSServiceRegister
// needs no socket of its own, so it never competes with the engine for the port); PairWithCode
// browses with Network's NWBrowser, resolves each service found, and the engine tries the typed
// code on each one.
//
// Browsing asks for local network access the first time, with the Info.plist's
// NSLocalNetworkUsageDescription and `_kaeri-sync._tcp` under NSBonjourServices.

import KaeriKit
import Foundation
import Network
import Synchronization
import dnssd

public final class BonjourSyncNetwork: FfiSyncNetwork {
    public static let serviceType = "_kaeri-sync._tcp"

    private let queue = DispatchQueue(label: "app.kaeri.sync-network") // i18n-skip
    private struct Browse {
        var browser: NWBrowser
        var sink: FfiSyncSink
        var resolving: [String: Resolve] = [:]
    }
    private let browse = Mutex<Browse?>(nil)
    private let registrations = Mutex<[String: Registration]>([:])

    public init() {}

    // MARK: Advertising

    public func advertise(advert: FfiSyncAdvert) {
        let registration = Registration(advert: advert, queue: queue)
        let old = registrations.withLock { $0.updateValue(registration, forKey: advert.instance) }
        old?.cancel()
        registration.start()
    }

    public func withdraw(instance: String) {
        let old = registrations.withLock { $0.removeValue(forKey: instance) }
        old?.cancel()
    }

    // MARK: Browsing

    public func browse(sink: FfiSyncSink) {
        stopBrowsing()
        let browser = NWBrowser(for: .bonjourWithTXTRecord(type: Self.serviceType, domain: nil), using: .tcp)
        browser.browseResultsChangedHandler = { [weak self] _, changes in
            self?.changed(changes)
        }
        browser.stateUpdateHandler = { state in
            if case .failed(let error) = state {
                AppLog.error("sync", "the sync browse failed: \(error)")
            }
        }
        browse.withLock { $0 = Browse(browser: browser, sink: sink) }
        browser.start(queue: queue)
    }

    public func stopBrowsing() {
        let stopped = browse.withLock { browse -> Browse? in
            defer { browse = nil }
            return browse
        }
        stopped?.browser.cancel()
        stopped?.resolving.values.forEach { $0.cancel() }
    }

    private func changed(_ changes: Set<NWBrowser.Result.Change>) {
        for change in changes {
            switch change {
            case .added(let result):
                found(result)
            case .changed(_, let new, _):
                found(new)
            case .removed(let result):
                guard case .service(let name, _, _, _) = result.endpoint else { continue }
                let sink = browse.withLock { browse -> FfiSyncSink? in
                    browse?.resolving.removeValue(forKey: name)?.cancel()
                    return browse?.sink
                }
                sink?.lost(instance: name)
            default:
                break
            }
        }
    }

    private func found(_ result: NWBrowser.Result) {
        guard case .service(let name, let type, let domain, let interface) = result.endpoint else { return }
        let txt: [FfiSyncTxt] = if case .bonjour(let record) = result.metadata {
            record.dictionary.sorted { $0.key < $1.key }.map { FfiSyncTxt(key: $0.key, value: $0.value) }
        } else {
            []
        }
        let resolve = Resolve(name: name, type: type, domain: domain, interface: UInt32(interface?.index ?? 0), queue: queue) {
            [weak self] host, port in
            guard let self else { return }
            let addresses = BonjourTVBrowser.addresses(of: host)
            let sink = self.browse.withLock { $0?.sink }
            guard !addresses.isEmpty else {
                AppLog.info("sync", "found \(name), but not its address")
                return
            }
            sink?.found(instance: name, addresses: addresses, port: port, txt: txt)
        }
        browse.withLock { browse in
            browse?.resolving.updateValue(resolve, forKey: name)?.cancel()
        }
        resolve.start()
    }
}

/// One registration of this device while it shows a code.
private final class Registration: @unchecked Sendable {
    private let advert: FfiSyncAdvert
    private let queue: DispatchQueue
    private var ref: DNSServiceRef?

    init(advert: FfiSyncAdvert, queue: DispatchQueue) {
        self.advert = advert
        self.queue = queue
    }

    func start() {
        queue.async { [self] in
            var txt = TXTRecordRef()
            TXTRecordCreate(&txt, 0, nil)
            defer { TXTRecordDeallocate(&txt) }
            for pair in advert.txt {
                let value = Array(pair.value.utf8)
                _ = TXTRecordSetValue(&txt, pair.key, UInt8(min(value.count, 255)), value)
            }
            let status = DNSServiceRegister(
                &ref, 0, 0, advert.instance, BonjourSyncNetwork.serviceType, nil, nil,
                (advert.port).bigEndian, TXTRecordGetLength(&txt), TXTRecordGetBytesPtr(&txt), nil, nil)
            guard status == kDNSServiceErr_NoError, let ref else {
                AppLog.error("sync", "the sync code was not advertised: \(status)")
                self.ref = nil
                return
            }
            DNSServiceSetDispatchQueue(ref, queue)
        }
    }

    func cancel() {
        queue.async { [self] in
            if let ref { DNSServiceRefDeallocate(ref) }
            ref = nil
        }
    }
}
