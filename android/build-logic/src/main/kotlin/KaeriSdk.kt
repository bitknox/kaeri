import org.gradle.api.JavaVersion

/** The levels every module builds at. */
object KaeriSdk {
    /**
     * Android 11, for every app and module: TV boxes stay on old releases (Android TV 14 is the
     * newest the Chromecast with Google TV has), and e-ink readers ship Android 11 to 13. What
     * newer releases add, such as edge to edge and predictive back by default, the phone app
     * checks for at run time and falls back from.
     */
    const val MIN = 30

    /** The newest SDK installed. */
    const val COMPILE = 37

    val JAVA = JavaVersion.VERSION_17

    /** The flavour dimension: which engine is inside. */
    const val ENGINE = "engine"

    /** Each flavour's engine build, as android/scripts/engine.sh names it. */
    val engines = mapOf("app" to "release", "fixtures" to "testing")
}
