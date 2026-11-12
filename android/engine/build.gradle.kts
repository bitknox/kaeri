// The engine as the app holds it: the Kotlin bindings UniFFI generates and the library they load,
// both from android/scripts/engine.sh, per flavour (KaeriKit on Apple).
plugins {
    id("kaeri.android.library")
    id("kaeri.engine.flavours")
}

android {
    namespace = "app.kaeri.engine"
    sourceSets {
        KaeriSdk.engines.forEach { (flavour, engine) ->
            named(flavour) {
                kotlin.directories.add("../.engine/$engine/kotlin")
                jniLibs.directories.add("../.engine/$engine/jniLibs")
            }
        }
    }
}

dependencies {
    // The bindings call into the library through JNA, and await its async calls as coroutines.
    api("net.java.dev.jna:jna:${libs.versions.jna.get()}@aar")
    api(libs.coroutines.core)
    // The bindings free what they hold through Android's SystemCleaner, behind @RequiresApi.
    implementation(libs.annotation)
}

// Every build of a flavour checks first that its engine is current, and fails, saying what to
// run, when the Rust sources have moved on since it was built.
KaeriSdk.engines.forEach { (flavour, engine) ->
    val name = flavour.replaceFirstChar(Char::uppercase)
    val check = tasks.register<Exec>("check${name}Engine") {
        description = "Checks that the $engine engine in android/.engine is current."
        commandLine(KaeriSdk.script(rootDir.resolve("scripts/engine.sh"), "--check", engine))
    }
    tasks.configureEach {
        if (this.name == "pre${name}DebugBuild" || this.name == "pre${name}ReleaseBuild") dependsOn(check)
    }
}
