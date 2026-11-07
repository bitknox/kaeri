package app.kaeri.player.ui

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class PictureInPictureOnLeaveTest {
    @Test
    fun android_11_opens_the_window_on_leaving_while_a_video_plays() {
        assertTrue(PictureInPictureOnLeave.shouldEnter(sdk = 30, wanted = true))
    }

    @Test
    fun android_11_stays_out_of_it_while_nothing_plays() {
        assertFalse(PictureInPictureOnLeave.shouldEnter(sdk = 30, wanted = false))
    }

    @Test
    fun from_android_12_the_system_enters_by_itself_and_the_activity_leaves_it_be() {
        for (sdk in listOf(31, 34, 35, 37)) assertFalse(PictureInPictureOnLeave.shouldEnter(sdk, wanted = true))
    }
}
