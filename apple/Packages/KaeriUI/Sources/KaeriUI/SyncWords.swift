// What the Devices screens say about a device, the cloud folder and the key, in the board's words.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

@MainActor
enum SyncWords {
    /// A sync note's sentence: the episode kept watched, who unwatched it and who had finished it.
    static func note(_ note: SyncNoteView, store: AppStore) -> String {
        let title = store.titles[note.anilistId]?.showTitle ?? ""
        return L.syncNoteKeptWatched(n: Int(note.episode) ?? 0, title: title, other: note.unwatchDevice, device: note.watchDevice)
    }

    /// "Synced just now", "Synced 2 min ago", or "Not seen for 31 days" past 30 days.
    static func seen(_ device: SyncDeviceView, now: Date = .now) -> (text: String, state: SyncSummary.State) {
        guard let at = device.lastSeen else { return (L.settingsSyncedJustNow, .synced) }
        let seconds = max(0, Int64(now.timeIntervalSince1970) - at)
        let days = Int(seconds / 86_400)
        if days >= 30 { return (L.devicesNotSeen(days: days), .signedOut) }
        return (agoLine(seconds), .synced)
    }

    /// "Synced just now" or "Synced 2 min ago".
    static func agoLine(_ seconds: Int64) -> String {
        seconds < 60 ? L.settingsSyncedJustNow : L.settingsSyncedAgo(ago: Wording.ago(seconds))
    }

    /// The Devices row's value in Settings.
    static func rowValue(_ sync: SyncModel, tvs: Int, now: Date = .now) -> String {
        switch sync.rowState {
        case .off: tvs > 0 ? L.devicesRowOff + " · " + L.devicesTvsForPlay(count: tvs) : L.devicesRowOff
        case .error: L.devicesRowError
        case .synced(let at):
            if let at { agoLine(Int64(now.timeIntervalSince(at))) } else { L.settingsSyncedJustNow }
        }
    }

    /// What the cloud row says under the service: its state, in a line, and the dot beside it.
    static func cloud(_ sync: SyncModel, now: Date = .now) -> (text: String, state: SyncSummary.State) {
        let status = sync.status
        let kind = status.folder?.kind
        let service = kind.map(SyncModel.name) ?? ""
        if sync.settingUpShown { return (L.cloudSettingUp, .waiting) }
        if let problem = status.problem ?? sync.problem {
            switch problem {
            case .signInNeeded:
                return (isSignedOutByHand(sync) ? L.cloudSignedOutLine(kind: Device.name)
                        : L.cloudErrSignedOut(service: service, kind: Device.name), isSignedOutByHand(sync) ? .signedOut : .problem)
            case .unreachable: return (L.cloudErrTitle(service: service), .problem)
            default: return (status.error ?? L.cloudErrTitle(service: service), .problem)
            }
        }
        if status.syncing { return (L.syncSyncing, .waiting) }
        if let at = status.lastSynced {
            return (agoLine(max(0, Int64(now.timeIntervalSince1970) - at)), .synced)
        }
        return (L.settingsSyncedJustNow, .synced)
    }

    /// Signed out on purpose rather than by the service: nothing in the problem says which, so
    /// the model remembers the Sign out that was pressed.
    static func isSignedOutByHand(_ sync: SyncModel) -> Bool { sync.signedOutByHand }

    /// "kim@mail.example" or the folder's label, under the service name.
    static func account(_ sync: SyncModel) -> String? {
        sync.status.folder?.account
    }

    /// The date a key was made: "2 Nov".
    static func made(_ seconds: Int64?) -> String? {
        seconds.map { Date(timeIntervalSince1970: TimeInterval($0)).formatted(.dateTime.day().month(.abbreviated).locale(AirTime.locale)) }
    }

    /// The device words in a sentence: "this Mac", "this iPhone".
    static var kind: String { Device.name }
}

extension String {
    /// The first letter in lower case, for a line that continues another.
    var lowercasedFirst: String { prefix(1).lowercased() + dropFirst() }
}
