import java.io.File

// The engine as the app holds it, and the app's state: the client that reads the engine's events,
// the store that applies them to a model per feature, the launch, and the host the engine calls
// back into (KaeriStore on Apple).
plugins {
    id("kaeri.android.library")
    id("kaeri.engine.flavours")
}

android {
    namespace = "app.kaeri.store"
    // The tests read the words the person would, from the catalogue.
    testOptions { unitTests.isIncludeAndroidResources = true }
}

dependencies {
    api(project(":engine"))
    // The strings, for the engine's messages rendered in the person's language (generated/MsgText.kt).
    api(project(":i18n"))
    api(libs.coroutines.android)
    implementation(libs.webkit)

    testImplementation(libs.junit)
    testImplementation(libs.coroutines.test)
    // Real bitmaps, for the artwork budget.
    testImplementation(libs.robolectric)
    // JNA's JVM build, with this Mac's native library, for the tests that start the engine.
    testImplementation(libs.jna.jvm)
    // The scenes the recorder writes, as the screenshot tests read them.
    testImplementation(project(":testkit"))
}

// The unit tests start the engine on this Mac's build of it, android/.engine/<engine>/host, under
// a home of their own: HOME is a new directory under build/, never the user's, and tokens are
// files, so nothing a test does reaches the user's own Kaeri.
KaeriSdk.engines.forEach { (flavour, engine) ->
    val name = flavour.replaceFirstChar(Char::uppercase)
    val library = rootDir.resolve(".engine/$engine/host")
    tasks.withType<Test>().matching { it.name.startsWith("test$name") }.configureEach {
        val home = layout.buildDirectory.dir("test-home/${this.name}").get().asFile
        val userHome = System.getProperty("user.home")
        systemProperty("jna.library.path", library.path)
        // JNA loads its native library itself, which Java now asks to be allowed.
        jvmArgs("--enable-native-access=ALL-UNNAMED")
        // Robolectric's Android 16 reaches into the JDK's file descriptors, as in the screenshot tests.
        jvmArgs("--add-opens=java.base/jdk.internal.access=ALL-UNNAMED", "--add-exports=java.base/jdk.internal.access=ALL-UNNAMED")
        environment("HOME", home.path)
        environment("KAERI_TOKEN_STORAGE", "file")
        // Home's invented world, for the run that records its scenes.
        providers.environmentVariable("KAERI_FIXTURES_HOME_WORLD").orNull?.let { environment("KAERI_FIXTURES_HOME_WORLD", it) }
        // android/scripts/scenes.sh: record the scenes into the test kit, every time it is run, or
        // into the directory the property names (-Pkaeri.scenes=/some/dir), to compare two runs.
        providers.gradleProperty("kaeri.scenes").orNull?.let { into ->
            val kit = rootDir.resolve("testkit/src/main/resources/app/kaeri/testkit/scenes")
            systemProperty("kaeri.scenes.out", if (into.isBlank() || into == "true") kit.path else File(into).path)
            // The engine's clock stopped at one moment, a Wednesday morning, so that every run
            // records the same scenes whatever the day it runs on.
            environment("KAERI_SCENES_AT", "1793782800")
            outputs.upToDateWhen { false }
        }
        doFirst {
            check(home.canonicalPath != File(userHome).canonicalPath) {
                "the tests' HOME would be the user's own"
            }
            home.deleteRecursively()
            home.mkdirs()
        }
    }
}
