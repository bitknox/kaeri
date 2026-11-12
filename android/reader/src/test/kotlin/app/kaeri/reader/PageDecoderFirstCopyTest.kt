// The first pages of a run decoded at once on several threads, as the page shown and the pages
// beside it are: every one decodes and the test runtime's native buffer setup holds. A class of its
// own, so its decodes are the first in a fresh test JVM.

package app.kaeri.reader

import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36])
class PageDecoderFirstCopyTest {
    @get:Rule
    val dir = TemporaryFolder()

    @Test
    fun first_pages_decoded_together_all_decode() {
        val threads = 8
        val files = List(threads) { ReaderFixtures.grey(dir.root, "$it.gray", width = 64, height = 64) }
        val pool = Executors.newFixedThreadPool(threads)
        val start = CountDownLatch(1)
        try {
            val results = files.map { file -> pool.submit<PageDecodeResult> { start.await(); PageDecoder.decode(file) } }
            start.countDown()
            val decoded = results.count { it.get(30, TimeUnit.SECONDS) is PageDecodeResult.Decoded }
            assertEquals(threads, decoded)
        } finally {
            pool.shutdownNow()
        }
    }
}
