// Preload video, the row under Up next countdown. Each device names its own kind of limited
// network: iPhone and an iPad with cellular say mobile data, the Mac and a Wi-Fi iPad say Low
// Data Mode. While Auto-play next is off nothing preloads, so the row dims with the countdown.

import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

enum PreloadWords {
    /// The row's label.
    static func title(cellular: Bool) -> String {
        cellular ? L.settingsPreloadCellular : L.settingsPreloadLowData
    }

    /// The line under the row, with the torrent sentence while torrent playback is on.
    static func line(cellular: Bool, torrents: Bool) -> String {
        let line = cellular ? L.settingsPreloadDetailApplePhone : L.settingsPreloadDetailLowData
        return torrents ? line + " " + L.settingsPreloadTorrents : line
    }

    /// Whether the row says mobile data before an iPad has looked at its interfaces: on iPhone
    /// always, on an iPad as its last look found, on the Mac never.
    @MainActor static var namesMobileData: Bool {
        #if os(iOS)
        Device.isPad ? CellularCheck.known ?? false : true
        #else
        false
        #endif
    }

    /// Whether this device says mobile data, after one look at an iPad's interfaces.
    @MainActor static func lookUp() async -> Bool {
        #if os(iOS)
        guard Device.isPad else { return true }
        let cellular = await CellularCheck.available()
        CellularCheck.known = cellular
        return cellular
        #else
        return false
        #endif
    }
}

#if os(iOS)
extension CellularCheck {
    /// The last look's answer, kept for the run so the row words itself at once next time.
    @MainActor static var known: Bool?
}
#endif

/// The row on the Mac and iPad, in Playback's Episodes group.
struct PreloadRow: View {
    @Environment(AppStore.self) private var store
    @State private var cellular = PreloadWords.namesMobileData

    var body: some View {
        let autoNext = store.settings.isOn(.autoNext)
        let title = PreloadWords.title(cellular: cellular)
        SettingsRow(title: title, note: PreloadWords.line(cellular: cellular, torrents: store.settings.isOn(.torrents))) {
            SettingSwitch(key: .preloadMetered, title: title)
        }
        .disabled(!autoNext)
        .opacity(autoNext ? 1 : 0.45)
        .accessibilityHint(autoNext ? "" : L.settingsPreloadA11yDimmed)
        .task { cellular = await PreloadWords.lookUp() }
    }
}

#if os(iOS)
/// The row in the iPhone's Episodes section, and on an iPad that shows the phone's pages.
struct PhonePreloadRow: View {
    @Environment(AppStore.self) private var store
    @State private var cellular = PreloadWords.namesMobileData

    var body: some View {
        let autoNext = store.settings.isOn(.autoNext)
        PhoneSettingSwitch(
            key: .preloadMetered,
            note: PreloadWords.line(cellular: cellular, torrents: store.settings.isOn(.torrents)),
            title: PreloadWords.title(cellular: cellular)
        )
        .disabled(!autoNext)
        .opacity(autoNext ? 1 : 0.45)
        .accessibilityHint(autoNext ? "" : L.settingsPreloadA11yDimmed)
        .task { cellular = await PreloadWords.lookUp() }
    }
}
#endif
