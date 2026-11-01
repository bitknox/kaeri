// swift-tools-version: 6.2
//
// The home-screen widgets: Continue watching and Airing today, on the Mac, iPad and iPhone.
//
// - KaeriWidgetData: the snapshot the app writes into the App Group container and the widgets
//   read, where its files are, when it is stale, the timelines' dates, what each state shows, the
//   links and the play intent into the app, and AniList's public schedule for a widget with no
//   snapshot. Engine-free (it knows the catalogue's strings only), so the app's store and the widget extension both link it.
// - KaeriWidgetUI: the widgets themselves, their layouts per size and state, and their
//   timeline providers. The Xcode project compiles these sources into the widget extension.
// - WidgetShots: renders every size and state from a fixture snapshot, for comparing with the
//   boards (`swift run WidgetShots <directory>`).
import PackageDescription

let package = Package(
    name: "KaeriWidgets",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [
        .library(name: "KaeriWidgetData", targets: ["KaeriWidgetData"]),
        .library(name: "KaeriWidgetUI", targets: ["KaeriWidgetUI"]),
        .executable(name: "WidgetShots", targets: ["WidgetShots"]),
    ],
    dependencies: [
        .package(path: "../KaeriDesign"),
        .package(path: "../KaeriStrings"),
    ],
    targets: [
        .target(
            name: "KaeriWidgetData",
            dependencies: [.product(name: "KaeriStrings", package: "KaeriStrings")]
        ),
        .target(
            name: "KaeriWidgetUI",
            dependencies: [
                "KaeriWidgetData",
                .product(name: "KaeriDesign", package: "KaeriDesign"),
                .product(name: "KaeriStrings", package: "KaeriStrings"),
            ]
        ),
        .executableTarget(
            name: "WidgetShots",
            dependencies: ["KaeriWidgetUI", "KaeriWidgetData"]
        ),
        .testTarget(
            name: "KaeriWidgetsTests",
            dependencies: ["KaeriWidgetData", "KaeriWidgetUI"],
            resources: [.copy("Snapshots")]
        ),
    ]
)
