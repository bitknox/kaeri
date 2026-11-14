// The strings every module shows, generated from the Fluent catalogue in ../i18n by tools/i18n:
// res/values*/strings.xml with plurals, the locale list, and Strings.kt for code outside
// Compose. Nothing here is edited by hand except this file. Resource names are the catalogue's
// ids in snake_case, so a removed key fails the build at the R.string.x that used it.
plugins {
    id("kaeri.android.library")
}

android {
    namespace = "app.kaeri.i18n"
    lint {
        // A language ships only the messages that are reviewed or drafted, and shows the rest in
        // English, per string (docs/translations.md); i18n-tool's draft lists what is missing.
        disable += "MissingTranslation"
    }
}
