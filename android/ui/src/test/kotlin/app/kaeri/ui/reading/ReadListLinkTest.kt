// Home's links into a Library list (Continue's All, Up next's View all) open Library at that list,
// even when a series page was left open on Library's tab.

package app.kaeri.ui.reading

import androidx.activity.ComponentActivity
import androidx.compose.material3.Text
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.navigation
import androidx.navigation.compose.rememberNavController
import app.kaeri.engine.model.LibrarySegmentRead
import app.kaeri.ui.Destination
import app.kaeri.ui.Navigator
import app.kaeri.ui.graphRoute
import app.kaeri.ui.seriesPattern
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class ReadListLinkTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private lateinit var nav: NavHostController

    private fun host(): Navigator {
        compose.setContent {
            nav = rememberNavController()
            NavHost(nav, startDestination = graphRoute(Destination.Home)) {
                for (destination in listOf(Destination.Home, Destination.Library)) {
                    navigation(route = graphRoute(destination), startDestination = destination.route) {
                        composable(destination.route) { Text(destination.route) }
                        composable(seriesPattern(destination)) { Text("series") }
                    }
                }
            }
        }
        compose.waitForIdle()
        return Navigator(nav, Destination.Home)
    }

    @Test
    fun all_opens_the_library_list_not_the_series_left_open_there() {
        val navigator = host()
        compose.runOnIdle {
            navigator.go(Destination.Library)
        }
        compose.runOnIdle {
            navigator.openSeries("l:rooftop-garden", "test")
        }
        compose.runOnIdle {
            navigator.go(Destination.Home)
        }
        compose.runOnIdle {
            ReadPlaces.openList(navigator, LibrarySegmentRead.READING)
        }
        compose.runOnIdle {
            assertEquals(Destination.Library.route, nav.currentDestination?.route)
            assertEquals(LibrarySegmentRead.READING, ReadPlaces.list)
        }
    }
}
