// The strings every module shows, generated from the Fluent catalogue in ../i18n by tools/i18n:
// res/values*/strings.xml with plurals, the locale list, and Strings.kt for code outside
// Compose. Nothing here is edited by hand except this file. Resource names are the catalogue's
// ids in snake_case, so a removed key fails the build at the R.string.x that used it.
plugins {
    id("kaeri.android.library")
}

android {
    namespace = "app.kaeri.i18n"
    // Shared with the TV app.
    defaultConfig { minSdk = KaeriSdk.TV_MIN }
}
