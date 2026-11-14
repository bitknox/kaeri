// The prelaunch board's states, staged for shots: `--es prelaunch p1-03` on the phone and the TV
// sets up piece 1's third state once the engine is up. Each piece stages its own states.

package app.kaeri.store

object Prelaunch {
    /** Every state, for the log's sake. */
    val STATES: List<String> get() = PrelaunchCameIn.STATES + PrelaunchDemo.STATES + PrelaunchJoin.STATES + PrelaunchProgress.STATES

    /** Set up [state]. False for a state there is none of. */
    suspend fun stage(store: AppStore, state: String): Boolean = when {
        state in PrelaunchCameIn.STATES -> PrelaunchCameIn.stage(store, state)
        state in PrelaunchDemo.STATES -> PrelaunchDemo.stage(store, state)
        state in PrelaunchJoin.STATES -> PrelaunchJoin.stage(store, state)
        state in PrelaunchProgress.STATES -> PrelaunchProgress.stage(store, state)
        else -> false
    }
}
