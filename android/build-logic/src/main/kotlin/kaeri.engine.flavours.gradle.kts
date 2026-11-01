// The two builds of the app, by the engine inside: `app`, the engine the app ships, and
// `fixtures`, the engine's `testing` build on the test kit's fake AniList and fixture titles, for
// screenshots and tests, never for real use. Every module that reaches the engine has both, so a
// variant of the app takes the same engine all the way down.
pluginManager.withPlugin("com.android.library") {
    extensions.configure<com.android.build.api.dsl.LibraryExtension> {
        flavorDimensions += KaeriSdk.ENGINE
        productFlavors {
            create("app") { dimension = KaeriSdk.ENGINE }
            create("fixtures") { dimension = KaeriSdk.ENGINE }
        }
    }
}
// Each test class in a JVM of its own. The engine's native library keeps one table of callbacks
// per interface for the whole process, set by whichever copy of the Kotlin bindings loads last,
// while each copy keeps its own map of the objects behind those callbacks. A JVM that runs a plain
// JUnit test and a Robolectric one loads two copies, the app's class loader's and the sandbox's
// (and each graphics mode has a sandbox of its own), so a host's callbacks reached the other
// copy's empty map: TvShots' tvSender then failed with "not enough bytes remaining", every test
// of the class, or once an earlier class's engine was freed. The app itself has one copy.
//
// Several of those JVMs at once, half a core each and no more than four: one after another, the
// ui module's screenshot classes alone took five minutes of CI's Android job, most of it each JVM
// starting Robolectric while the other cores sat idle. Each JVM is a process of its own, so the
// one table of callbacks per process still holds.
tasks.withType<Test>().configureEach {
    forkEvery = 1
    maxParallelForks = (Runtime.getRuntime().availableProcessors() / 2).coerceIn(1, 4)
}
pluginManager.withPlugin("com.android.application") {
    extensions.configure<com.android.build.api.dsl.ApplicationExtension> {
        flavorDimensions += KaeriSdk.ENGINE
        productFlavors {
            create("app") {
                dimension = KaeriSdk.ENGINE
                applicationId = "app.kaeri.app"
            }
            create("fixtures") {
                dimension = KaeriSdk.ENGINE
                applicationId = "app.kaeri.fixtures"
            }
        }
    }
}
