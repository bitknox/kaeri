// The design system: the Obi & Silk colour scheme, the type, the shapes, the motion tokens,
// Nagare and the cover plate (KaeriDesign on Apple). Nothing of the engine.
plugins {
    id("kaeri.android.library")
    id("kaeri.android.compose")
}

android {
    namespace = "app.kaeri.design"
    // Shared with the TV app.
    defaultConfig { minSdk = KaeriSdk.TV_MIN }
    sourceSets {
        named("main") {
            // The two typefaces and their licences, from android/scripts/fonts.sh.
            res.directories.add("../.fonts/res")
            assets.directories.add("../.fonts/assets")
        }
    }
}

// Every build makes sure first that the fonts are there, and fails only if they cannot be had.
// In a new worktree they are fetched first, from the cache every worktree shares or else from
// google/fonts; once there it costs a checksum, and needs no network.
val checkFonts = tasks.register<Exec>("checkFonts") {
    description = "Fetches the fonts with android/scripts/fonts.sh unless they are there."
    commandLine(KaeriSdk.script(rootDir.resolve("scripts/fonts.sh")))
}
tasks.configureEach {
    if (name == "preBuild") dependsOn(checkFonts)
}
