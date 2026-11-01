// What the app's tests share, and nothing the app ships: the engine's events recorded as scenes,
// which the screenshot tests replay into the store, and the fixture titles' art, read from the
// Rust test kit beside it. Only ever a test dependency.
plugins {
    id("kaeri.android.library")
    id("kaeri.engine.flavours")
}

android {
    namespace = "app.kaeri.testkit"
    // The TV app's tests read the scenes too.
    defaultConfig { minSdk = KaeriSdk.TV_MIN }
}

dependencies {
    api(project(":engine"))
    implementation(libs.kotlin.reflect)
    implementation(libs.kotlinx.serialization.json)

    // The screenshot tests' capture wrapper, which checks the semantics it draws (A11yShots).
    implementation(platform(libs.compose.bom))
    implementation(libs.compose.ui)
    implementation(libs.compose.ui.test.junit4)
    implementation(libs.roborazzi)
    implementation(libs.roborazzi.compose)

    testImplementation(libs.junit)
}
