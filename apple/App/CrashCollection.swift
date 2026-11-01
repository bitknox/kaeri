// Crash reports kept on this device, only while Settings has saving on. The system's MetricKit is
// subscribed to only then, and its past records are read only when the switch is turned on, to
// offer what the system held from before. With saving off nothing here runs and nothing is
// written.

import KaeriKit
import KaeriStore
import Foundation
import MetricKit
import Observation

@MainActor
final class CrashCollection: NSObject, MXMetricManagerSubscriber {
    private let store: AppStore
    private var follower: CrashFollower?

    init(store: AppStore) {
        self.store = store
        super.init()
        store.crash.readSystemRecords = { [weak self] in self?.pastRecords() ?? [] }
        follower = CrashFollower(
            isOn: { store.settings.isOn(.saveCrashReports) },
            subscribe: { [unowned self] in MXMetricManager.shared.add(self) },
            unsubscribe: { [unowned self] in MXMetricManager.shared.remove(self) })
    }

    /// The system's records of the past days, to offer once saving has been switched on.
    private func pastRecords() -> [CrashRecord] {
        MXMetricManager.shared.pastDiagnosticPayloads.flatMap(Self.records)
    }

    nonisolated func didReceive(_ payloads: [MXDiagnosticPayload]) {
        let records = payloads.flatMap(Self.records)
        guard !records.isEmpty else { return }
        Task { @MainActor in
            for var record in records { record.origin = .saved; _ = crashSave(record: record) }
            store.crash.refresh()
        }
    }

    nonisolated private static func blank() -> CrashRecord {
        CrashRecord(
            id: "", atMs: 0, uptimeSecs: 0, platform: "", osVersion: "", appVersion: "", arch: Self.arch,
            place: .other, detail: "", kind: .signal, reason: "", frames: [], sources: 0, demoOn: false,
            origin: .heldBySystem)
    }

    nonisolated private static var arch: String {
        #if arch(arm64)
        "arm64"
        #else
        "x86_64"
        #endif
    }

    nonisolated private static func records(_ payload: MXDiagnosticPayload) -> [CrashRecord] {
        let at = Int64(payload.timeStampEnd.timeIntervalSince1970 * 1000)
        let version = ProcessInfo.processInfo.operatingSystemVersion
        #if os(macOS)
        let platform = "Mac"
        let system = "macOS"
        #else
        let platform = "iPhone"
        let system = "iOS"
        #endif
        let crashes = (payload.crashDiagnostics ?? []).map { crash -> CrashRecord in
            var record = Self.blank()
            record.atMs = at
            record.platform = platform
            record.osVersion = "\(system) \(version.majorVersion).\(version.minorVersion)"
            record.appVersion = crash.applicationVersion
            record.kind = crash.signal == nil ? .exception : .signal
            record.reason = [crash.exceptionType.map { "exception \($0)" }, crash.signal.map { "signal \($0)" }]
                .compactMap { $0 }.joined(separator: " ")
            record.origin = .heldBySystem
            return record
        }
        let hangs = (payload.hangDiagnostics ?? []).map { hang -> CrashRecord in
            var record = Self.blank()
            record.atMs = at
            record.platform = platform
            record.osVersion = "\(system) \(version.majorVersion).\(version.minorVersion)"
            record.appVersion = hang.applicationVersion
            record.kind = .hang
            record.reason = "hang"
            record.origin = .heldBySystem
            return record
        }
        return crashes + hangs
    }
}
