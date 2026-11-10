package app.kaeri.ui

import app.kaeri.store.FixtureScene
import app.kaeri.ui.reading.ReadPlaces
import java.io.File
import kotlin.math.sqrt
import org.junit.Assume.assumeTrue
import org.junit.Before
import org.junit.Test
import org.robolectric.annotation.Config

/**
 * Search's first switch from Shows to Reading on a fresh launch, frame by frame: reading's rows
 * not kept yet, so its start comes in on its placeholders while Home's rows are asked for. Each
 * clip writes `spread.csv`, the spread of brightness under the row per frame: an empty frame
 * shows as a spread near zero. Only with KAERI_SCOPE_FRAMES=1; frames go to
 * KAERI_SCOPE_FRAMES_OUT.
 */
class ReadScopeFrames : ReadingShotHarness() {
    private val out = File(System.getenv("KAERI_SCOPE_FRAMES_OUT") ?: "build/scope-frames")

    @Before
    fun only() {
        assumeTrue(System.getenv("KAERI_SCOPE_FRAMES") == "1")
        ReadPlaces.searchReading = false
    }

    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE)
    fun phone_shows_to_reading() = record("phone")

    @Test
    @Config(qualifiers = ReadingShotHarness.TABLET)
    fun tablet_shows_to_reading() = record("tablet")

    private fun record(name: String) {
        var kept = false
        frames(name, FixtureScene.READING_HOME, Destination.Search, out) { store ->
            kept = store.reading.home.value != null
            ReadPlaces.searchReading = true
        }
        val dir = File(out, name)
        val lines = dir.listFiles { file -> file.name.startsWith("frame-") }.orEmpty().sortedBy { it.name }.map { file ->
            "${file.name},${"%.1f".format(spread(file))}"
        }
        File(dir, "spread.csv").writeText((listOf("# reading's rows kept at the switch: $kept", "frame,spread") + lines).joinToString("\n", postfix = "\n"))
    }

    /** The spread of brightness from 35% to 85% of the frame's height, where the content is. */
    private fun spread(file: File): Double {
        val bitmap = android.graphics.BitmapFactory.decodeFile(file.path)
        val top = bitmap.height * 35 / 100
        val bottom = bitmap.height * 85 / 100
        var sum = 0.0
        var squares = 0.0
        var count = 0
        for (y in top until bottom step 4) for (x in 0 until bitmap.width step 4) {
            val pixel = bitmap.getPixel(x, y)
            val light = 0.2126 * ((pixel shr 16) and 0xff) + 0.7152 * ((pixel shr 8) and 0xff) + 0.0722 * (pixel and 0xff)
            sum += light
            squares += light * light
            count++
        }
        val mean = sum / count
        return sqrt((squares / count - mean * mean).coerceAtLeast(0.0))
    }
}
