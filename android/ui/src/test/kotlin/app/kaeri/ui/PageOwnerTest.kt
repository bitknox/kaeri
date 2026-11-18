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
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * A page opened over Downloads, which sits outside every destination, goes on the back stack of
 * the tab Downloads was opened from, so the bar keeps that tab lit, not the start one.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class PageOwnerTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private lateinit var nav: NavHostController
    private lateinit var navigator: Navigator

    private fun host() {
        compose.setContent {
            nav = rememberNavController()
            navigator = Navigator(nav, Destination.Home)
            NavHost(nav, startDestination = graphRoute(Destination.Home)) {
                for (destination in listOf(Destination.Home, Destination.Library)) {
                    navigation(route = graphRoute(destination), startDestination = destination.route) {
                        composable(destination.route) { BasicText(destination.route) }
                        composable(titlePattern(destination)) { BasicText("title") }
                        composable(seriesPattern(destination)) { BasicText("series") }
                        composable(readPagePattern(destination)) { BasicText("read page") }
                    }
                }
                composable(DOWNLOADS_ROUTE) { BasicText("downloads") }
            }
        }
        compose.waitForIdle()
    }

    private fun onScreen(): Destination? = nav.currentBackStackEntry?.destinationOf()

    private fun libraryDownloads() {
        compose.runOnUiThread { navigator.go(Destination.Library) }
        compose.waitForIdle()
        compose.runOnUiThread { navigator.open(DOWNLOADS_ROUTE) }
        compose.waitForIdle()
    }

    @Test
    fun a_series_opened_from_librarys_downloads_stays_in_library_and_back_returns_to_downloads() {
        host()
        libraryDownloads()
        compose.runOnUiThread { navigator.openSeries("a:900301", "downloads") }
        compose.waitForIdle()
        assertEquals(Destination.Library, onScreen())
        assertEquals(seriesPattern(Destination.Library), nav.currentBackStackEntry?.destination?.route)
        compose.runOnUiThread { navigator.back() }
        compose.waitForIdle()
        assertEquals(DOWNLOADS_ROUTE, nav.currentBackStackEntry?.destination?.route)
    }

    @Test
    fun a_title_opened_from_librarys_downloads_stays_in_library() {
        host()
        libraryDownloads()
        compose.runOnUiThread { navigator.openTitle(900001u, "downloads") }
        compose.waitForIdle()
        assertEquals(Destination.Library, onScreen())
    }

    @Test
    fun a_series_opened_on_a_tab_stays_on_that_tab() {
        host()
        compose.runOnUiThread { navigator.go(Destination.Library) }
        compose.waitForIdle()
        compose.runOnUiThread { navigator.openSeries("a:900301", "library") }
        compose.waitForIdle()
        assertEquals(Destination.Library, onScreen())
    }
}
