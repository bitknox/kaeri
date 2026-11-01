// swift-tools-version: 6.2
//
// The Mac dev harness: the app's shell on the testing engine, run with `swift run KaeriDev`,
// which renders its own window to a PNG with `--snapshot`. See apple/README.md.
import PackageDescription

let package = Package(
    name: "KaeriDev",
    platforms: [.macOS(.v26)],
    dependencies: [
        .package(path: "../KaeriUI"),
        .package(path: "../KaeriStore"),
        .package(path: "../KaeriDesign"),
        .package(path: "../KaeriStrings"),
        .package(path: "../KaeriPlayer"),
    ],
    targets: [
        .executableTarget(
            name: "KaeriDev",
            dependencies: [
                .product(name: "KaeriUI", package: "KaeriUI"),
                .product(name: "KaeriStore", package: "KaeriStore"),
                .product(name: "KaeriDesign", package: "KaeriDesign"),
                .product(name: "KaeriPlayerUI", package: "KaeriPlayer"),
                .product(name: "KaeriPlayer", package: "KaeriPlayer"),
            ],
            swiftSettings: [.define("FIXTURES")]
        ),
    ]
)
