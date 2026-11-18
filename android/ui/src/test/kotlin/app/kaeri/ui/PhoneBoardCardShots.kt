package app.kaeri.ui

import androidx.activity.ComponentActivity
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.unit.dp
import com.github.takahirom.roborazzi.roborazziSystemPropertyOutputDirectory
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import org.junit.After
import org.junit.Assume.assumeTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import app.kaeri.design.ArtShape
import app.kaeri.design.KaeriTheme
import app.kaeri.engine.model.Entry
import app.kaeri.store.AppStore
import app.kaeri.store.HAS_TESTING_ENGINE
import app.kaeri.testkit.captureRoboImage

/**
 * The phone board's cards with no banner, direction A, one per step of the ladder: banner, the
 * seen still, the cover cropped within 1.6 times, a cover too small for that blurred behind
 * itself, and a plate in the cover's colour. Continue cards at 208 dp and the spotlight at the
 * phone's width, from made-up shows and drawn art.
 *
 * `./gradlew :ui:recordRoborazziFixturesDebug --tests '*PhoneBoardCardShots*'` writes the shot.
 */
@OptIn(com.github.takahirom.roborazzi.ExperimentalRoborazziApi::class)
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w412dp-h1400dp-port-420dpi")
class PhoneBoardCardShots {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)

    @After
    fun tearDown() {
        scope.cancel()
    }

    /** Drawn art: a [width] by [height] picture in two tones, with a ring where its focus is. */
    private fun picture(width: Int, height: Int, top: Int, bottom: Int): ByteArray {
        val bitmap = android.graphics.Bitmap.createBitmap(width, height, android.graphics.Bitmap.Config.ARGB_8888)
        val canvas = android.graphics.Canvas(bitmap)
        val paint = android.graphics.Paint(android.graphics.Paint.ANTI_ALIAS_FLAG)
        paint.shader = android.graphics.LinearGradient(0f, 0f, 0f, height.toFloat(), top, bottom, android.graphics.Shader.TileMode.CLAMP)
        canvas.drawRect(0f, 0f, width.toFloat(), height.toFloat(), paint)
        paint.shader = null
        paint.color = android.graphics.Color.rgb(240, 214, 160)
        paint.style = android.graphics.Paint.Style.STROKE
        paint.strokeWidth = width / 40f
        canvas.drawCircle(width / 2f, height / 3f, width / 6f, paint)
        return java.io.ByteArrayOutputStream().also { bitmap.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, it) }.toByteArray()
    }

    private val art: Map<String, ByteArray> by lazy {
        mapOf(
            BANNER to picture(1900, 400, android.graphics.Color.rgb(40, 70, 120), android.graphics.Color.rgb(20, 30, 60)),
            STILL to picture(640, 360, android.graphics.Color.rgb(120, 90, 60), android.graphics.Color.rgb(40, 30, 30)),
            COVER to picture(460, 690, android.graphics.Color.rgb(60, 140, 110), android.graphics.Color.rgb(20, 50, 50)),
            SMALL to picture(230, 345, android.graphics.Color.rgb(150, 70, 90), android.graphics.Color.rgb(50, 20, 40)),
        )
    }

    private fun entry(id: UInt, title: String, banner: String? = null, still: String? = null, cover: String? = null, color: String) = Entry(
        id = id, title = title, secondary = null, format = "TV", episodes = 12u,
        year = 2026u, score = null, studio = null, synopsis = "", coverUrl = cover, coverColor = color, bannerUrl = banner,
        genres = emptyList(), availableOn = emptyList(), progress = null,
        airingIn = null, airingEpisode = null, nextEpisode = null, nextAiringIn = null, lastAired = null,
        resume = null, related = emptyList(), recommended = emptyList(), trailerUrl = null, listStatus = null,
        titleRomaji = null, titleEnglish = null, titleNative = null, season = null, startDate = null,
        endDate = null, source = null, adult = false, detail = null,
        status = null, synopsisSpans = emptyList(),
    ).copy(seenStill = still)

    @Test
    fun phone_board_cards() {
        assumeTrue(HAS_TESTING_ENGINE)
        val store = AppStore(scope, clock = { 0L }, artworkWithoutEngine = { art[it] })
        val steps = listOf(
            "Banner" to entry(900101u, "The Lantern Ferry", banner = BANNER, cover = COVER, color = "#2B4F80"),
            "Seen still" to entry(900102u, "The Lantern Ferry", still = STILL, cover = COVER, color = "#2B4F80"),
            "Cover, cropped" to entry(900103u, "The Steel Gardener", cover = COVER, color = "#2E8064"),
            "Cover, blurred" to entry(900104u, "Moth Orchard", cover = SMALL, color = "#8A3550"),
            "Colour plate" to entry(900105u, "The Long Quiet Signal Box", color = "#6B4FA0"),
        )
        compose.setContent {
            KaeriTheme {
                CompositionLocalProvider(LocalAppStore provides store) {
                    Column(
                        Modifier.fillMaxWidth().background(MaterialTheme.colorScheme.background).padding(16.dp),
                        verticalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        for ((label, entry) in steps) {
                            Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
                                BannerArt(entry, Modifier.size(208.dp, 117.dp).clip(ArtShape.banner).border(1.dp, ArtEdge, ArtShape.banner))
                                Text(label, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                        }
                        // The spotlight at the phone's width: wider than a card, so the cover blurs.
                        for ((label, entry) in listOf(steps[2], steps[4])) {
                            Text("Spotlight, ${label.lowercase()}", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                            BannerArt(entry, Modifier.width(380.dp).aspectRatio(16f / 9f).clip(ArtShape.banner).border(1.dp, ArtEdge, ArtShape.banner))
                        }
                    }
                }
            }
        }
        // The art decodes off the main thread, then fades in.
        repeat(20) {
            compose.mainClock.advanceTimeBy(200)
            compose.waitForIdle()
            Thread.sleep(50)
        }
        compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/phone-board-cards.png")
    }

    private companion object {
        const val BANNER = "https://img.example/banner/900101.png"
        const val STILL = "https://img.example/ep/900102/3.png"
        const val COVER = "https://img.example/cover/900103-xl.png"
        const val SMALL = "https://img.example/cover/900104-small.png"
    }
}
