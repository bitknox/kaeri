// The fixtures flavour's player panel demo (`--es panel live` on the playing scene): the testing
// engine has one source with one stream, so the engine's part in the panel's Sources tab is played
// by a script. It lists made-up sources and releases beside the scene's own, and answers a switch
// as the engine would: one source lands at the playhead, on the scene's clip loaded again, and one
// fails after a wait. The fixture's episodes get made-up synopses, one long enough for More, and
// its dub goes through episode 8. The player, the panel and its rows are the app's own.

package app.kaeri.player

import android.util.Log
import app.kaeri.engine.core.SourceCandidate
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.DubExtent
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.ListingMiss
import app.kaeri.engine.model.ListingState
import app.kaeri.engine.model.MpvOption
import app.kaeri.engine.model.Msg
import app.kaeri.engine.model.PlayerOp
import app.kaeri.engine.model.SourceListing
import app.kaeri.engine.model.SourceNote
import app.kaeri.engine.model.SourceOption
import app.kaeri.engine.model.SourceSwitch
import app.kaeri.engine.model.SwitchTo
import app.kaeri.store.AppStore
import app.kaeri.store.fixtureMedia
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull

object PanelDemo {
    private const val TAG = "KaeriPanel"

    /** On from the `panel` extra: the player's commands go through [send] first. */
    private var on = false
    private var model: PlayerModel? = null
    private val scope: CoroutineScope = MainScope()

    /** The made-up sources, after the scene's own, in the order Sources lists them. */
    private val sources = listOf(
        SourceOption(providerId = "harbor", name = "Harbor Mirror", covered = 12u, of = 12u, pinned = false, note = null, releases = emptyList()),
        SourceOption(providerId = "tidewater", name = "Tidewater", covered = 9u, of = 12u, pinned = false, note = null, releases = emptyList()),
        SourceOption(providerId = "northshelf", name = "North Shelf", covered = 4u, of = 12u, pinned = false, note = null, releases = emptyList()),
        SourceOption(providerId = "quietreel", name = "Quiet Reel", covered = 0u, of = 12u, pinned = false, note = SourceNote.Resting, releases = emptyList()),
    )

    /** Each made-up source's releases for any episode: Harbor's land, Tidewater's fails, North Shelf has none. */
    private fun releases(episode: String): List<SourceCandidate> {
        val title = "Stardust Wayfarer - ${episode.padStart(2, '0')}" // i18n-skip
        return listOf(
            candidate("harbor", "Harbor Mirror", "h-1080", "$title (1080p)", 1080u, "paper-lantern", "1.4 GiB", autoPick = true),
            candidate("harbor", "Harbor Mirror", "h-720-dub", "$title (720p, dubbed)", 720u, "paper-lantern", "620 MiB", dubbed = true),
            candidate("harbor", "Harbor Mirror", "h-1080-dual", "$title (1080p, dual audio, extended credits and signs)", 1080u, "a very long release label that runs past one line", "2.1 GiB", dualAudio = true),
            candidate("tidewater", "Tidewater", "t-1080", "$title [1080p]", 1080u, "tidewater-encodes", "1.2 GiB"),
        )
    }

    private fun candidate(
        providerId: String, name: String, id: String, title: String, quality: UInt, label: String, size: String,
        dubbed: Boolean = false, dualAudio: Boolean = false, autoPick: Boolean = false,
    ) = SourceCandidate(id = id, providerId = providerId, providerName = name, title = title, quality = quality, label = label, size = size, dualAudio = dualAudio, dubbed = dubbed, autoPick = autoPick)

    /** Made-up synopses by episode: the fifth's runs past three lines, the sixth's fits. */
    private val synopses = mapOf(
        "5" to "The beacon keepers of the northern ridge remember every traveller who ever passed under their lights, " +
            "and one of them remembers the lamplighter too, from a winter she cannot recall. Their ledger holds a page " +
            "in her hand, signed with a name she has never used, and a map of a road that no longer exists. " +
            "She spends the night copying it by lantern light while the keepers argue over whether to let her leave.",
        "6" to "A mapmaker's daughter offers to trade a route for a story.",
        "9" to "The last beacon on the old road goes dark, and the lamplighter has to choose which light to save.",
    )

    /**
     * Start the demo for [player]: the made-up sources are kept beside the scene's own in the
     * picker's options while it runs.
     */
    fun start(store: AppStore, player: PlayerModel) {
        on = true
        model = player
        Log.w(TAG, "KAERI_PANEL on")
        scope.launch {
            store.titles.episodes.collect { all ->
                for ((id, rows) in all) {
                    if (rows.none { it.number in synopses && it.description == null }) continue
                    store.apply(Event.Episodes(id, rows.map { row -> synopses[row.number]?.let { row.copy(description = it) } ?: row }, null))
                    store.apply(Event.TitleDub(id, DubExtent(through = 8u, complete = false)))
                }
            }
        }
        scope.launch {
            store.sourceChoices.options.collect { all ->
                for ((id, listed) in all) {
                    if (listed.none { it.providerId == "harbor" }) store.apply(Event.SourceOptions(id, listed + sources))
                }
            }
        }
    }

    /**
     * `--es panel sources`, on the sourcesloading scene: plays the first fixture's episode 7 and,
     * once its picture is up, opens the panel on Sources, which asks every source then. The log
     * says "KAERI_PANEL sources <ms>" (the device's uptime) once the tab has asked.
     */
    fun openSources(store: AppStore, player: PlayerModel) {
        scope.launch {
            withTimeoutOrNull(60_000) { while (store.phase.value != AppStore.Phase.Running) delay(100) }
            store.send(Command.Play(900_001u, "7"))
            if (withTimeoutOrNull(120_000) { while (!player.hasPicture) delay(100) } == null) {
                Log.w(TAG, "KAERI_PANEL no picture")
                return@launch
            }
            delay(1_000)
            player.openPanel(PanelTab.Sources)
            // The moment the tab asks its sources, which on TV is a frame or more after it opens.
            withTimeoutOrNull(10_000) { while (player.panelReleases == null) delay(5) }
            Log.w(TAG, "KAERI_PANEL sources ${android.os.SystemClock.uptimeMillis()}")
        }
    }

    /** The player's [command] to the engine: the panel's own are answered here while the demo is on. */
    fun send(command: Command, send: (Command) -> Unit) {
        if (!on) return send(command)
        when (command) {
            is Command.LoadSources -> scope.launch {
                // Every row at once, asked, then each as it answers, as the engine says them.
                fun row(option: SourceOption, state: ListingState) = SourceListing(option.providerId, option.name, state)
                val asking = sources.map { row(it, if (it.note == SourceNote.Resting) ListingState.Off else ListingState.Asking) }
                model?.apply(Event.SourcesProgress(command.id, command.episode, asking))
                delay(700)
                val all = releases(command.episode)
                val answered = sources.map { source ->
                    val listed = all.filter { it.providerId == source.providerId }
                    when {
                        source.note == SourceNote.Resting -> row(source, ListingState.Off)
                        listed.isEmpty() -> row(source, ListingState.Missed(ListingMiss.NoEpisode))
                        else -> row(source, ListingState.Listed(listed))
                    }
                }
                model?.apply(Event.SourcesProgress(command.id, command.episode, answered))
                model?.apply(Event.Sources(command.id, command.episode, all, emptyList()))
            }
            is Command.SwitchSource -> scope.launch { switch(command) }
            else -> send(command)
        }
    }

    private suspend fun switch(command: Command.SwitchSource) {
        val model = model ?: return
        val playback = model.playback ?: return
        val release = releases(command.episode).firstOrNull { it.providerId == command.providerId && it.id == command.sourceId } ?: return
        val to = SwitchTo(release.providerId, release.providerName, release.id, null)
        fun say(state: SourceSwitch) {
            Log.w(TAG, "KAERI_PANEL ${state::class.simpleName} ${release.id} ${System.currentTimeMillis()}")
            model.apply(Event.PlayerSwitch(playback, state))
        }
        delay(150)
        say(SourceSwitch.Finding(to))
        if (release.providerId == "tidewater") {
            // Long enough for the row's spinner and its "Starting · 3 s" to be seen.
            delay(5_000)
            val playing = model.playingSource ?: "Fixture Source" // i18n-skip
            say(SourceSwitch.Failed(to, model.playhead, Msg.PlayerPanelSwitchFailed(release.providerName, playing), null))
            return
        }
        delay(900)
        val at = model.playhead
        say(SourceSwitch.Loading(to, at))
        model.apply(Event.PlayerOps(playback, listOf(PlayerOp.Load(fixtureMedia, listOf(MpvOption("start", "%.1f".format(java.util.Locale.ROOT, at)))))))
        withTimeoutOrNull(15_000) { while (model.hasPicture) delay(20) }
        withTimeoutOrNull(15_000) { while (!model.hasPicture) delay(50) }
        say(SourceSwitch.Landed(to, at))
    }
}
