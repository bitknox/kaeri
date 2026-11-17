// The mini and the controller morph into each other as one surface. Frame by frame through a
// close, an open, and flights turned back early and late: the surface stays whole and shrinks or
// grows without a jump, the cover keeps its 16 corners as it flies, the side that leaves fades its
// words out over the first part and the side that arrives fades its words in over the last part.

package app.kaeri.ui

import androidx.activity.ComponentActivity
import androidx.compose.animation.ExperimentalSharedTransitionApi
import androidx.compose.animation.SharedTransitionLayout
import androidx.compose.animation.core.SeekableTransitionState
import androidx.compose.animation.core.rememberTransition
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asAndroidBitmap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.unit.dp
import app.kaeri.design.KaeriTheme
import app.kaeri.store.CastModel
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import kotlin.math.abs
import kotlin.math.sqrt

@OptIn(ExperimentalSharedTransitionApi::class)
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class CastCoverMorphTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private lateinit var state: SeekableTransitionState<Boolean>
    private lateinit var scope: CoroutineScope
    private var surface = Color.Unspecified
    private val density get() = compose.activity.resources.displayMetrics.density

    /**
     * The real two halves of the surface on one morph, over a black window: the mini in a 66 high
     * slot at the foot, with a 40 cover and green words, and the controller over the window, with
     * a 200 high cover and blue words.
     */
    private fun show(open: Boolean) {
        compose.mainClock.autoAdvance = false
        compose.setContent {
            KaeriTheme {
                surface = MaterialTheme.colorScheme.surfaceContainerHigh
                val seekable = remember { SeekableTransitionState(open) }
                state = seekable
                scope = rememberCoroutineScope()
                val transition = rememberTransition(seekable, label = "test") // i18n-skip
                SharedTransitionLayout(Modifier.fillMaxSize().background(Color.Black).testTag("window")) {
                    val morph = remember {
                        val drag = CastDrag(seekable, scope, CastModel(CoroutineScope(Dispatchers.Unconfined)), still = false, flick = 1000f)
                        CastMorph(this, transition, still = false, origin = CastOrigin(), drag = drag) { false }
                    }
                    Box(Modifier.fillMaxSize()) {
                        Box(Modifier.align(Alignment.BottomStart).fillMaxWidth().height(66.dp)) {
                            CastMiniSurface(morph) {
                                val mini = this
                                Row(Modifier.fillMaxSize().padding(horizontal = 16.dp), verticalAlignment = Alignment.CenterVertically) {
                                    Box(Modifier.size(40.dp).castCover(mini, morph).background(Color.Red))
                                    Spacer(Modifier.width(12.dp))
                                    Box(Modifier.size(MINI_WORDS_WIDTH.dp, WORDS_HEIGHT.dp).background(Color.Green))
                                }
                            }
                        }
                        CastControllerSurface(morph) {
                            val controller = this
                            Column(Modifier.fillMaxSize().padding(24.dp)) {
                                Box(Modifier.fillMaxWidth().height(ART.dp).castCover(controller, morph).background(Color.Red))
                                Spacer(Modifier.height(24.dp))
                                Box(Modifier.size(CONTROLLER_WORDS_WIDTH.dp, (2 * WORDS_HEIGHT).dp).background(Color.Blue))
                            }
                        }
                    }
                }
            }
        }
        compose.mainClock.advanceTimeBy(500)
    }

    /** One frame as drawn: the surface's extent, the cover, and how much of each side's words shows. */
    private data class Frame(
        val top: Int,
        val bottom: Int,
        val solid: Boolean,
        val coverHeight: Float,
        val radius: Float,
        val controllerWords: Float,
        val miniWords: Float,
    )

    private fun frame(): Frame {
        val image = compose.onNodeWithTag("window").captureToImage()
        // Read as sRGB, where the window was blended, so a colour over another mixes in a line.
        val bitmap = image.asAndroidBitmap()
        val w = bitmap.width
        val h = bitmap.height
        val pixels = IntArray(w * h).also { bitmap.getPixels(it, 0, w, 0, 0, w, h) }
        fun r(x: Int, y: Int) = (pixels[y * w + x] shr 16 and 0xff) / 255f
        fun g(x: Int, y: Int) = (pixels[y * w + x] shr 8 and 0xff) / 255f
        fun b(x: Int, y: Int) = (pixels[y * w + x] and 0xff) / 255f
        val s = surface
        // How much of a pure colour a pixel holds over the surface or the black window: the
        // surface's share comes from a channel none of the three colours shares with it.
        fun share(value: Float, rest: Float, restRef: Float, ref: Float) = (value - ref / restRef * rest).let { if (it < 0.04f) 0f else minOf(it, 1f) }
        fun red(x: Int, y: Int) = share(r(x, y), s.red, s.green, g(x, y))
        fun green(x: Int, y: Int) = share(g(x, y), s.green, s.blue, b(x, y))
        fun blue(x: Int, y: Int) = share(b(x, y), s.blue, s.green, g(x, y))

        // The surface runs the window's width; its rows are those not black at the right edge.
        val edge = w - 2
        var top = -1
        var bottom = -1
        for (y in 0 until h) if (maxOf(r(edge, y), g(edge, y), b(edge, y)) > 0.02f) {
            if (top < 0) top = y
            bottom = y
        }
        assertTrue("no surface drawn", top >= 0) // i18n-skip
        // Whole: every row between its edges is the surface's colour, not thinned toward black.
        var solid = true
        for (y in top + 2..bottom - 2) {
            if (abs(r(edge, y) - s.red) > SHADE || abs(g(edge, y) - s.green) > SHADE || abs(b(edge, y) - s.blue) > SHADE) solid = false
        }

        var left = Int.MAX_VALUE
        var coverTop = Int.MAX_VALUE
        var right = -1
        var coverBottom = -1
        var miniWords = 0f
        var controllerWords = 0f
        for (y in 0 until h) for (x in 0 until w) {
            if (red(x, y) > 0.5f) {
                left = minOf(left, x); right = maxOf(right, x); coverTop = minOf(coverTop, y); coverBottom = maxOf(coverBottom, y)
            }
            miniWords += green(x, y)
            controllerWords += blue(x, y)
        }
        if (right < 0) return Frame(top, bottom, solid, 0f, 0f, controllerWords / (CONTROLLER_WORDS * density * density), miniWords / (MINI_WORDS * density * density))
        val width = right - left + 1
        val height = coverBottom - coverTop + 1
        // A corner's curve crosses the diagonal r (1 - 1/√2) in from the corner, x and y alike:
        // what red the diagonal misses until it meets the cover, averaged over the four corners.
        val side = minOf(width, height) / 2
        var inset = 0f
        for (k in 0 until side) {
            inset += (1f - red(left + k, coverTop + k)) + (1f - red(right - k, coverTop + k)) +
                (1f - red(left + k, coverBottom - k)) + (1f - red(right - k, coverBottom - k))
        }
        val radius = inset / 4f / (1f - 1f / sqrt(2f))
        // In full, each side's words paint their whole area.
        val area = density * density
        return Frame(top, bottom, solid, height / density, radius / density, controllerWords / (CONTROLLER_WORDS * area), miniWords / (MINI_WORDS * area))
    }

    /** Steps [frames] frames, checking that the surface stays whole and the cover's corners stay 16. */
    private fun follow(frames: Int, seen: MutableList<Frame> = mutableListOf()): MutableList<Frame> {
        repeat(frames) {
            compose.mainClock.advanceTimeByFrame()
            val f = frame()
            println("frame ${seen.size}: $f") // i18n-skip
            seen += f
            assertTrue("the surface thinned at frame ${seen.size}: $f", f.solid) // i18n-skip
            assertTrue("the cover went missing at frame ${seen.size}: $f", f.coverHeight >= 36f) // i18n-skip
            assertEquals("corners at frame ${seen.size}: $f", 16f, f.radius, TOLERANCE) // i18n-skip
        }
        return seen
    }

    /** The surface moves its top edge one way only, through the middle, never jumping. */
    private fun assertTravels(seen: List<Frame>, closing: Boolean) {
        val tops = seen.map { it.top }
        tops.zipWithNext().forEach { (a, b) -> assertTrue("the surface turned back: $tops", if (closing) b >= a else b <= a) } // i18n-skip
        val travel = abs(tops.last() - tops.first()).toFloat()
        val window = seen.first().bottom.toFloat()
        assertTrue("the surface barely moved: $tops", travel > window * 0.8f) // i18n-skip
        val step = tops.zipWithNext().maxOf { (a, b) -> abs(b - a) }
        assertTrue("the surface jumped $step px in a frame: $tops", step < travel * 0.25f) // i18n-skip
    }

    /** [words] shows fully, then fades out over the first frames, through at least two between. */
    private fun assertFadesOutEarly(words: List<Float>, name: String) {
        assertTrue("$name start hidden: $words", words.first() > 0.6f) // i18n-skip
        val gone = words.indexOfFirst { it < 0.05f }
        assertTrue("$name never leave: $words", gone in 1..EARLY) // i18n-skip
        assertTrue("$name cut out at once: $words", words.take(gone).count { it in 0.05f..0.9f } >= 2) // i18n-skip
    }

    /** [words] stays hidden over the first frames, then fades in, through at least two between. */
    private fun assertFadesInLate(words: List<Float>, name: String) {
        assertTrue("$name show from the start: $words", words.take(LATE).all { it < 0.05f }) // i18n-skip
        val full = words.indexOfFirst { it > 0.9f }
        assertTrue("$name never arrive: $words", full > 0) // i18n-skip
        assertTrue("$name cut in at once: $words", words.take(full).count { it in 0.05f..0.9f } >= 2) // i18n-skip
    }

    @Test
    fun closingTheControllerShrinksIntoTheMini() {
        show(open = true)
        val start = frame()
        assertEquals(16f, start.radius, TOLERANCE)
        scope.launch { state.animateTo(false) }
        val seen = follow(FLIGHT, mutableListOf(start))
        assertTravels(seen, closing = true)
        assertFadesOutEarly(seen.map { it.controllerWords }, "the controller's words") // i18n-skip
        assertFadesInLate(seen.map { it.miniWords }, "the mini's words") // i18n-skip
        compose.mainClock.advanceTimeBy(1000)
        val end = frame()
        assertEquals(MINI, end.coverHeight, 1f)
        assertEquals(16f, end.radius, TOLERANCE)
        assertEquals(1f, end.miniWords, 0.05f)
        assertEquals(0f, end.controllerWords, 0.05f)
    }

    @Test
    fun openingTheMiniGrowsIntoTheController() {
        show(open = false)
        val start = frame()
        assertEquals(16f, start.radius, TOLERANCE)
        scope.launch { state.animateTo(true) }
        val seen = follow(FLIGHT, mutableListOf(start))
        assertTravels(seen, closing = false)
        assertFadesOutEarly(seen.map { it.miniWords }, "the mini's words") // i18n-skip
        assertFadesInLate(seen.map { it.controllerWords }, "the controller's words") // i18n-skip
        compose.mainClock.advanceTimeBy(1000)
        val end = frame()
        assertEquals(ART.toFloat(), end.coverHeight, 1f)
        assertEquals(16f, end.radius, TOLERANCE)
        assertEquals(1f, end.controllerWords, 0.05f)
        assertEquals(0f, end.miniWords, 0.05f)
    }

    @Test
    fun turnedBackMidwayTheSurfaceStaysWhole() {
        show(open = true)
        scope.launch { state.animateTo(false) }
        val seen = follow(6)
        scope.launch { state.animateTo(true) }
        follow(FLIGHT, seen)
        compose.mainClock.advanceTimeBy(1000)
        val end = frame()
        assertEquals(ART.toFloat(), end.coverHeight, 1f)
        assertEquals(16f, end.radius, TOLERANCE)
        assertEquals(1f, end.controllerWords, 0.05f)
        assertEquals(0, end.top)
    }

    @Test
    fun openingTurnedBackLateTheSurfaceStaysWhole() {
        show(open = false)
        scope.launch { state.animateTo(true) }
        val seen = follow(12)
        scope.launch { state.animateTo(false) }
        follow(FLIGHT, seen)
        compose.mainClock.advanceTimeBy(1000)
        val end = frame()
        assertEquals(MINI, end.coverHeight, 1f)
        assertEquals(16f, end.radius, TOLERANCE)
        assertEquals(1f, end.miniWords, 0.05f)
        assertEquals(0f, end.controllerWords, 0.05f)
    }

    private companion object {
        /** The controller's cover height in the test, in dp. */
        const val ART = 200

        /** The words' sizes, in dp, and their areas. */
        const val WORDS_HEIGHT = 24
        const val MINI_WORDS_WIDTH = 160
        const val CONTROLLER_WORDS_WIDTH = 240
        const val MINI_WORDS = (MINI_WORDS_WIDTH * WORDS_HEIGHT).toFloat()
        const val CONTROLLER_WORDS = (CONTROLLER_WORDS_WIDTH * 2 * WORDS_HEIGHT).toFloat()

        /** Frames stepped through a flight. */
        const val FLIGHT = 40

        /** Frames by which the leaving side's words are gone: about 160 ms. */
        const val EARLY = 10

        /** Frames over which the arriving side's words stay hidden: about 64 ms. */
        const val LATE = 4

        /** How far the surface's colour may stray in a channel, out of 1: three steps of 8 bits. */
        const val SHADE = 0.012f

        /** The mini's cover's side, in dp. */
        const val MINI = 40f

        /** How far a measured radius may be from 16, in dp: a pixel or so of edge. */
        const val TOLERANCE = 1.5f
    }
}
