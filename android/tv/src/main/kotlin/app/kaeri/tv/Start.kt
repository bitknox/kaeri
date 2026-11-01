// Where a launch opens: Continue for the launcher, and for the fixtures flavour, what its extras
// name.

package app.kaeri.tv

import app.kaeri.engine.core.AnilistId
import app.kaeri.tv.shell.Destination

data class Start(
    val destination: Destination = Destination.Home,
    val title: AnilistId? = null,
    val query: String? = null,
    /** Whether first launch's pages may show: always on a release; on fixtures, `--ez firstLaunch true`. */
    val firstLaunch: Boolean = true,
    /** Whether the update's page opens over the destination: fixtures' `--es update <stage>`. */
    val update: Boolean = false,
    /** A tracker whose sign-in opens over the destination: fixtures only, `--es signin trakt`. */
    val signIn: String? = null,
)
