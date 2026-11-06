import java.io.File

// The reader surface: the session it binds to the engine's reader commands and events, the page
// files it decodes, the paged and strip views, and the web view host novels are read in. The
// phone app and the e-reader build both draw their reader on it; :eink adds the e-ink pieces.
plugins {
    id("kaeri.android.library")
    id("kaeri.android.compose")
    id("kaeri.engine.flavours")
    // The reader's surfaces drawn on the JVM from made-up pages.
    id("kaeri.screenshots")
}

android {
    namespace = "app.kaeri.reader"
}

dependencies {
    api(project(":store"))
    api(project(":design"))
    implementation(project(":i18n"))
    // The novel host's document-start script and message listener.
    implementation(libs.webkit)
    // Back closes what is over the pages before the reader.
    implementation(libs.activity.compose)

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

// Like the store's tests: the engine's own functions on this Mac's build of it, under a HOME of
// their own, never the user's.
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
