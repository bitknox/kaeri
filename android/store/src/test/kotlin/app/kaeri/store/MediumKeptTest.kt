// What Kaeri is for, kept on the device: a launch reads the last answer before the engine has
// started, so the bar and rail are drawn for it from the first frame.

package app.kaeri.store

import android.content.Context
import app.kaeri.engine.model.AppEdition
import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.engine.model.MediumView
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class MediumKeptTest {
    private val prefs = RuntimeEnvironment.getApplication().getSharedPreferences("medium-kept", Context.MODE_PRIVATE)

    private fun view(answer: KaeriIsFor) =
        MediumView(answer = answer, edition = AppEdition.FULL, fixed = false, readingAvailable = true, ask = null, account = null)

    @Test
    fun a_launch_starts_from_the_last_answer_until_the_engine_says() {
        val first = MediumModel {}
        first.keepIn(prefs)
        assertNull(first.kept)
        assertEquals(KaeriIsFor.WATCH, first.answer)
        first.apply(view(KaeriIsFor.BOTH))

        // The next launch, before the engine's first view.
        val next = MediumModel {}
        next.keepIn(prefs)
        assertNull(next.view.value)
        assertEquals(KaeriIsFor.BOTH, next.answer)

        // The engine's answer wins and is kept for the launch after.
        next.apply(view(KaeriIsFor.READ))
        assertEquals(KaeriIsFor.READ, next.answer)
        val third = MediumModel {}
        third.keepIn(prefs)
        assertEquals(KaeriIsFor.READ, third.kept)
    }
}
