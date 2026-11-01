// A library module of the app: Android 15 and up, built against the newest SDK.
plugins {
    id("com.android.library")
}

android {
    compileSdk = KaeriSdk.COMPILE
    defaultConfig {
        minSdk = KaeriSdk.MIN
    }
    compileOptions {
        sourceCompatibility = KaeriSdk.JAVA
        targetCompatibility = KaeriSdk.JAVA
    }
    testOptions {
        // Android's own classes answer with their defaults in a unit test, rather than throwing.
        unitTests.isReturnDefaultValues = true
    }
}
