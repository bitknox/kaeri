package app.kaeri.ui

import androidx.activity.ComponentActivity
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.performClick
import app.kaeri.design.KaeriTheme
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.SyncCommand
import app.kaeri.engine.model.SyncDeviceView
import app.kaeri.store.AppStore
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * A device's page in Settings, Devices: its Sync and Remote switches name the device to the
 * engine by its id, which is all the engine knows it by. A name turned no switch at all.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class DeviceSwitchesTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val store = AppStore(CoroutineScope(SupervisorJob() + Dispatchers.Main))

    private fun switch(title: String) = hasText(title) and SemanticsMatcher.expectValue(SemanticsProperties.Role, Role.Switch)

    @Test
    fun the_switches_send_the_devices_id_and_the_new_value() {
        val tv = SyncDeviceView(id = "b7e1c0d2", name = "Living room TV", thisDevice = false, remoteKey = "ab12")
        compose.setContent {
            KaeriTheme {
                CompositionLocalProvider(LocalAppStore provides store) { DevicePage(tv, onRemove = {}, onDismiss = {}) }
            }
        }
        compose.onNode(switch("Sync"), useUnmergedTree = false).performClick()
        compose.onNode(switch("Remote"), useUnmergedTree = false).performClick()
        val sent = store.waiting.filterIsInstance<Command.SyncGroup>().map { it.v1 }
        assertEquals(listOf(SyncCommand.SetDeviceSync("b7e1c0d2", false), SyncCommand.SetDeviceRemote("b7e1c0d2", false)), sent)
    }
}
