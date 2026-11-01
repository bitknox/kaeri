// The widgets' play button runs in the app: its intent is KaeriWidgetData's, which the app
// takes in here, and the app's routes take the link it opens (KaeriUI's WidgetRoutes).

import KaeriWidgetData
import AppIntents

struct KaeriAppIntentsPackage: AppIntentsPackage {
    static var includedPackages: [any AppIntentsPackage.Type] {
        [KaeriWidgetIntents.self]
    }
}
