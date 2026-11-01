// swift-tools-version: 6.2
//
// libmpv in the app's own view: the client over mpv's API, the layer and the SwiftUI surface it
// draws into, and screenshots. mpv itself is Mpv.xcframework, which apple/scripts/mpv.sh builds
// and links in here (Mpv/, gitignored). See apple/README.md.
//
// KaeriPlayerUI is the player the app shows: the engine's hosted player protocol carried out
// on that client, and the player's screens for Mac, iPhone and iPad.
import PackageDescription

let package = Package(
    name: "KaeriPlayer",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [
        .library(name: "KaeriPlayer", targets: ["KaeriPlayer"]),
        .library(name: "KaeriPlayerUI", targets: ["KaeriPlayerUI"]),
    ],
    dependencies: [
        .package(path: "../KaeriStore"),
        .package(path: "../KaeriDesign"),
        .package(path: "../KaeriStrings"),
    ],
    targets: [
        // A dynamic framework per slice that exports only `_mpv_*`, with mpv's headers.
        .binaryTarget(name: "Mpv", path: "Mpv/Mpv.xcframework"),
        .target(name: "KaeriPlayer", dependencies: ["Mpv"]),
        .target(
            name: "KaeriPlayerUI",
            dependencies: [
                "KaeriPlayer",
                .product(name: "KaeriStore", package: "KaeriStore"),
                .product(name: "KaeriDesign", package: "KaeriDesign"),
                .product(name: "KaeriStrings", package: "KaeriStrings"),
            ],
            // On the testing engine, as the other packages are (see KaeriKit): the Fixtures
            // app's previews of each state are there to name. The Xcode project defines this
            // for the Fixtures app's flavour only.
            swiftSettings: [.define("FIXTURES")]
        ),
        .testTarget(name: "KaeriPlayerTests", dependencies: ["KaeriPlayer"]),
        .testTarget(
            name: "KaeriPlayerUITests",
            dependencies: ["KaeriPlayerUI"],
            swiftSettings: [.define("FIXTURES")]
        ),
    ]
)
