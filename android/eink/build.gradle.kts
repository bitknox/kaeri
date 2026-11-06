import java.io.File

// The e-ink pieces the phone app and the e-reader build share: the token set and its controls,
// the refresh controllers that turn the engine's refresh decisions into each vendor's call (and
// the fake the Fixtures app and the tests use), the detection bridge that tells the engine what
// screen this is, the questions it asks when it cannot tell, the page keys, and the e-ink reader
// drawn on the :reader surface.
plugins {
    id("kaeri.android.library")
    id("kaeri.android.compose")
    id("kaeri.engine.flavours")
    // The e-ink boards drawn on the JVM at each panel's size in dp.
    id("kaeri.screenshots")
}

android {
    namespace = "app.kaeri.eink"
    // Shared with the e-reader build, on Android 11 and up.
    defaultConfig { minSdk = KaeriSdk.TV_MIN }
}

dependencies {
    api(project(":reader"))
    implementation(project(":i18n"))

    testImplementation(libs.junit)
    testImplementation(libs.coroutines.test)
    testImplementation(libs.robolectric)
    testImplementation(libs.roborazzi)
    testImplementation(libs.roborazzi.compose)
    testImplementation(platform(libs.compose.bom))
    testImplementation(libs.compose.ui.test.junit4)
    testImplementation(libs.compose.ui.test.manifest)
    testImplementation(project(":testkit"))
    testImplementation(libs.jna.jvm)
}

// The refresh decisions come from the engine's own function, on this Mac's build of it, under a
// HOME of their own, never the user's.
KaeriSdk.engines.forEach { (flavour, engine) ->
    val name = flavour.replaceFirstChar(Char::uppercase)
    val library = rootDir.resolve(".engine/$engine/host")
    tasks.withType<Test>().matching { it.name.startsWith("test$name") }.configureEach {
        val home = layout.buildDirectory.dir("test-home/${this.name}").get().asFile
        val userHome = System.getProperty("user.home")
        systemProperty("jna.library.path", library.path)
        jvmArgs("--enable-native-access=ALL-UNNAMED")
        jvmArgs("--add-exports=java.base/jdk.internal.ref=ALL-UNNAMED")
        environment("HOME", home.path)
        environment("KAERI_TOKEN_STORAGE", "file")
        doFirst {
            check(home.canonicalPath != File(userHome).canonicalPath) { "the tests' HOME would be the user's own" }
            home.deleteRecursively()
            home.mkdirs()
        }
    }
}
