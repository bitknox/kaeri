// Settings this device keeps for itself rather than in the engine: the keys the app's views store
// them under with `@AppStorage`, shared by the screens that set them and the parts that read them.

import Foundation

public enum DevicePreferences {
    /// Before the engine kept the subtitle style, whether plain-text subtitles follow the
    /// system's caption style lived here. Read once and moved to the engine (`migrateCaptionPreference`).
    static let followSystemCaptions = "kaeri.followSystemCaptions"

    /// Moves the old device switch into the engine's setting, once: a viewer who had turned it
    /// off keeps it off. The key is removed, so this never runs twice.
    @MainActor
    public static func migrateCaptionPreference(_ store: AppStore, defaults: UserDefaults = .standard) {
        guard defaults.object(forKey: followSystemCaptions) != nil else { return }
        if !defaults.bool(forKey: followSystemCaptions) {
            store.setSwitch(.followSystemCaptions, on: false)
        }
        defaults.removeObject(forKey: followSystemCaptions)
    }
}
