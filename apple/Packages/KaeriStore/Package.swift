// swift-tools-version: 6.2
//
// The app's state over the engine: the client that reads its events, the store that applies them
// to a model per feature, artwork, the keychain host and where the engine's files live.
import PackageDescription

let package = Package(
    name: "KaeriStore",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [
        .library(name: "KaeriStore", targets: ["KaeriStore"]),
    ],
    dependencies: [
        .package(path: "../KaeriKit"),
        .package(path: "../KaeriStrings"),
        .package(path: "../KaeriWidgets"),
    ],
    targets: [
        .target(
            name: "KaeriStore",
            dependencies: [
                "KaeriKit",
                .product(name: "KaeriStrings", package: "KaeriStrings"),
                .product(name: "KaeriWidgetData", package: "KaeriWidgets"),
            ],
            // The packages build on the testing engine (see KaeriKit), so its starts are
            // there to name. The Xcode project defines this for the Fixtures app's flavour only.
            swiftSettings: [.define("FIXTURES")]
        ),
        .testTarget(
            name: "KaeriStoreTests",
            dependencies: [
                "KaeriStore",
                .product(name: "KaeriWidgetData", package: "KaeriWidgets"),
            ],
            swiftSettings: [.define("FIXTURES")]
        ),
    ]
)
