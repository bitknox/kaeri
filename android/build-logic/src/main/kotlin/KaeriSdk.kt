import java.io.File
import org.gradle.api.JavaVersion

/** The levels every module builds at. */
object KaeriSdk {
    /** Android 15: edge to edge and predictive back on by default, which the design assumes. */
    const val MIN = 35

    /**
     * Android 11, for the TV app and the modules it shares with the phone (the engine, the store,
     * the design system and the player): TV boxes stay on old releases, and Android TV 14 is the
     * newest the Chromecast with Google TV has.
     */
    const val TV_MIN = 30

    /** The newest SDK installed. */
    const val COMPILE = 37

    val JAVA = JavaVersion.VERSION_17

    /** The flavour dimension: which engine is inside. */
    const val ENGINE = "engine"

    /** Each flavour's engine build, as android/scripts/engine.sh names it. */
    val engines = mapOf("app" to "release", "fixtures" to "testing")

    /**
     * The command line that runs one of android/scripts' bash scripts. Elsewhere the script runs
     * itself; Windows cannot start a .sh file, so there it goes through Git for Windows' bash:
     * the first bash.exe on PATH, or the one beside git.exe's Git\cmd, as an IDE's PATH has it.
     * The bash.exe under the Windows folder is WSL's launcher, which would run it in Linux, so
     * that one is passed over.
     */
    fun script(file: File, vararg args: String): List<String> {
        if (!System.getProperty("os.name").startsWith("Windows")) return listOf(file.path, *args)
        val windows = System.getenv("SystemRoot")?.let { File(it).absolutePath.lowercase() }
        val bash = System.getenv("PATH").orEmpty().split(File.pathSeparator)
            .filter { it.isNotBlank() }
            .flatMap { listOf(File(it, "bash.exe"), File(it, "../bin/bash.exe")) }
            .map { it.normalize() }
            .firstOrNull { it.isFile && (windows == null || !it.absolutePath.lowercase().startsWith(windows)) }
            ?: error("no Git for Windows bash.exe on PATH, which android/scripts/${file.name} needs")
        return listOf(bash.path, file.path.replace('\\', '/'), *args)
    }
}
