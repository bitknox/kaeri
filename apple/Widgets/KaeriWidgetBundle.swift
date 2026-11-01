// The widget extension: Continue watching and Airing today, and on iPhone the Live Activities for
// downloads, playback and an episode airing soon, whose layouts, timelines and edit sheet are in
// apple/Packages/KaeriWidgets (KaeriWidgetUI), compiled into this target.

import KaeriWidgetData
import AppIntents
import SwiftUI
import WidgetKit

@main
struct KaeriWidgetBundle: WidgetBundle {
    var body: some Widget {
        ContinueWatchingWidget()
        AiringTodayWidget()
        #if os(iOS)
        DownloadsLiveActivity()
        PlaybackLiveActivity()
        AiringLiveActivity()
        #endif
    }
}

/// The App Intents the extension brings: its own edit sheet's, and the play button's from
/// KaeriWidgetData.
struct KaeriWidgetsIntentsPackage: AppIntentsPackage {
    static var includedPackages: [any AppIntentsPackage.Type] {
        [KaeriWidgetIntents.self]
    }
}
