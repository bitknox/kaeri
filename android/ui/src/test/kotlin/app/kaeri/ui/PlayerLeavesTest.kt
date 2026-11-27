package app.kaeri.ui

import androidx.activity.ComponentActivity
import androidx.compose.foundation.text.BasicText
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.navigation
import androidx.navigation.compose.rememberNavController
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * A page the player opens as it closes (No source's Open sources and Search titles, the start's
 * Downloads and stores) comes after the player's route has left the back stack, so Back from it
 * returns to where the player was opened, not to a player that has gone.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class PlayerLeavesTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private lateinit var nav: NavHostController
    private lateinit var navigator: Navigator

    private fun host() {
        compose.setContent {
            nav = rememberNavController()
            navigator = Navigator(nav, Destination.Home)
            NavHost(nav, startDestination = graphRoute(Destination.Home)) {
                for (destination in listOf(Destination.Home, Destination.Search)) {
                    navigation(route = graphRoute(destination), startDestination = destination.route) {
                        composable(destination.route) { BasicText(destination.route) }
                    }
                }
                composable(PLAYER_ROUTE) { BasicText("player") }
                composable(PROVIDERS_ROUTE) { BasicText("sources") }
                composable(DOWNLOADS_ROUTE) { BasicText("downloads") }
            }
        }
        compose.waitForIdle()
        compose.runOnUiThread { navigator.open(PLAYER_ROUTE) }
        compose.waitForIdle()
    }

    private fun route() = nav.currentBackStackEntry?.destination?.route

    private fun playerOnStack() = nav.currentBackStack.value.any { it.destination.route == PLAYER_ROUTE }

    @Test
    fun open_sources_from_the_player_backs_out_to_home() {
        host()
        compose.runOnUiThread { navigator.leavingPlayer().openSources() }
        compose.waitForIdle()
        assertEquals(PROVIDERS_ROUTE, route())
        assertFalse(playerOnStack())
        compose.runOnUiThread { navigator.back() }
        compose.waitForIdle()
        assertEquals(Destination.Home.route, route())
    }

    @Test
    fun search_titles_from_the_player_leaves_no_player_behind() {
        host()
        compose.runOnUiThread { navigator.leavingPlayer().go(Destination.Search) }
        compose.waitForIdle()
        assertEquals(Destination.Search.route, route())
        assertFalse(playerOnStack())
    }

    @Test
    fun downloads_from_the_start_backs_out_to_home() {
        host()
        compose.runOnUiThread { navigator.leavingPlayer().openDownloads() }
        compose.waitForIdle()
        assertEquals(DOWNLOADS_ROUTE, route())
        assertFalse(playerOnStack())
    }

    @Test
    fun leaving_without_a_player_open_pops_nothing() {
        host()
        compose.runOnUiThread { nav.popBackStack() }
        compose.waitForIdle()
        compose.runOnUiThread { navigator.leavingPlayer().openDownloads() }
        compose.waitForIdle()
        compose.runOnUiThread { navigator.back() }
        compose.waitForIdle()
        assertEquals(Destination.Home.route, route())
    }
}
