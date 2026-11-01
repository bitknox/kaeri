// swift-tools-version: 6.2
//
// The app's screens: the shells for Mac, iPad and iPhone and what they show, over the store and
// the design system. The app targets and the dev harness both show these.
import PackageDescription

let package = Package(
    name: "KaeriUI",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [
        .library(name: "KaeriUI", targets: ["KaeriUI"]),
    ],
    dependencies: [
        .package(path: "../KaeriStore"),
        .package(path: "../KaeriDesign"),
        .package(path: "../KaeriStrings"),
        .package(path: "../KaeriWidgets"),
    ],
    targets: [
        .target(
            name: "KaeriUI",
            dependencies: [
                .product(name: "KaeriStore", package: "KaeriStore"),
                .product(name: "KaeriDesign", package: "KaeriDesign"),
                .product(name: "KaeriStrings", package: "KaeriStrings"),
                .product(name: "KaeriWidgetData", package: "KaeriWidgets"),
            ]
        ),
        .testTarget(name: "KaeriUITests", dependencies: ["KaeriUI"]),
    ]
)
