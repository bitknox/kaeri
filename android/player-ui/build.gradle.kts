// The player's screens: the frame, its chrome, cards and sheets, and picture in picture, on the
// phone's design. The TV draws its own on the same player.
plugins {
    id("kaeri.android.library")
    id("kaeri.android.compose")
    id("kaeri.engine.flavours")
    // The player's boards drawn on the JVM, as the screens' are.
    id("kaeri.screenshots")
}

android {
    namespace = "app.kaeri.player.ui"
}

dependencies {
    api(project(":player"))
    implementation(project(":store"))
    implementation(project(":i18n"))
    // The frame's player slot, the art and the page's shared transitions.
    implementation(project(":ui"))
    implementation(libs.activity.compose)
    implementation(libs.lifecycle.runtime.compose)
    implementation(libs.core.ktx)
    implementation(libs.media3.session)
    implementation(libs.media3.common)

    testImplementation(testFixtures(project(":player")))
    testImplementation(libs.junit)
    testImplementation(libs.coroutines.test)
    testImplementation(project(":testkit"))
    testImplementation(libs.robolectric)
    testImplementation(libs.roborazzi)
    testImplementation(libs.navigation.compose)
    testImplementation(libs.roborazzi.compose)
    testImplementation(platform(libs.compose.bom))
    testImplementation(libs.compose.ui.test.junit4)
    testImplementation(libs.compose.ui.test.manifest)
    // JNA's JVM build, with this machine's native library: the marking boards ask the engine's
    // model for its made-up bars, as the app's fixtures do.
    testImplementation(libs.jna.jvm)
}

// The engine's model on this machine's build of it, android/.engine/<engine>/host; no engine runs.
KaeriSdk.engines.forEach { (flavour, engine) ->
    val name = flavour.replaceFirstChar(Char::uppercase)
    val library = rootDir.resolve(".engine/$engine/host")
    tasks.withType<Test>().matching { it.name.startsWith("test$name") }.configureEach {
        systemProperty("jna.library.path", library.path)
        jvmArgs("--enable-native-access=ALL-UNNAMED")
    }
}
