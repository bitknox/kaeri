package app.kaeri.app

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class NotificationPermissionTest {
    @Test
    fun below_android_13_the_install_granted_it_and_nothing_is_asked() {
        for (sdk in 30..32) {
            assertTrue(NotificationPermission.granted(sdk) { false })
            assertFalse(NotificationPermission.askable(sdk))
        }
    }

    @Test
    fun from_android_13_it_is_what_the_person_gave() {
        for (sdk in listOf(33, 35, 37)) {
            assertFalse(NotificationPermission.granted(sdk) { false })
            assertTrue(NotificationPermission.granted(sdk) { true })
            assertTrue(NotificationPermission.askable(sdk))
        }
    }

    @Test
    fun below_android_13_a_switch_turned_on_while_the_system_blocks_the_app_opens_its_settings() {
        // Granted with the install, but the app's notifications are off in the system.
        val granted = NotificationPermission.granted(30) { false }
        assertTrue(EpisodeAlerts.Turn.of(on = true, canPost = false, permitted = granted) == EpisodeAlerts.Turn.SystemSettings)
    }
}
