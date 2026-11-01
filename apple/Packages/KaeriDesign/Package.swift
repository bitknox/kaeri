// swift-tools-version: 6.2
//
// The Obi & Silk design system as SwiftUI takes it: the colour, motion and type tokens, the show
// title face, Nagare, and the cover placeholder. Nothing here knows the engine.
import PackageDescription

let package = Package(
    name: "KaeriDesign",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [
        .library(name: "KaeriDesign", targets: ["KaeriDesign"]),
    ],
    targets: [
        .target(
            name: "KaeriDesign",
            // Zen Kaku Gothic New Bold, under the SIL Open Font License beside it, and the
            // drawing Settings shows the upscaling modes on.
            resources: [.copy("Fonts"), .copy("Art")]
        ),
        .testTarget(name: "KaeriDesignTests", dependencies: ["KaeriDesign"]),
    ]
)
