// Watch together's lobby states for the screenshot tests, as the engine's party events say them,
// laid over the recorded Together scene: the room's episode, its members and their ready marks,
// and what this device's sources found. Made-up names and codes, on the engine's suggested server.

package app.kaeri.testkit

import app.kaeri.engine.model.Cause
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.Msg
import app.kaeri.engine.model.NoSourceWhy
import app.kaeri.engine.model.PartyConnection
import app.kaeri.engine.model.PartyFound
import app.kaeri.engine.model.PartyGlimpse
import app.kaeri.engine.model.PartyGuess
import app.kaeri.engine.model.PartyMember
import app.kaeri.engine.model.PartyRoomEpisode
import app.kaeri.engine.model.PartySources
import app.kaeri.engine.model.PartyView

object PartyStates {
    /** The suggested server's address, as the scene keeps its rooms on. */
    const val SERVER = "syncplay.pl:8995"
    const val CODE = "kite-7qm4-xr9d"
    const val LINK = "https://kaeri.app/party#v=2&s=syncplay.pl%3A8995&r=kite-7qm4-xr9d&by=Sol&e=9"

    /** Natsukaze Relay, the scene's airing title. */
    const val RELAY = 900006u

    fun episode(found: PartyFound = PartyFound.TAG, id: UInt? = RELAY, number: String = "9", plainBy: String? = null) = PartyRoomEpisode(
        id = id,
        episode = number,
        title = "Natsukaze Relay", // i18n-skip
        file = "Natsukaze Relay - 09 [1080p].mkv",
        found = found,
        plainBy = plainBy,
    )

    fun member(name: String, ready: Boolean?, you: Boolean = false, file: Boolean = true) = PartyMember(
        name = name,
        ready = ready,
        file = if (file) "Natsukaze Relay - 09 [1080p].mkv" else null,
        duration = if (file) 1440.0 else null,
        fileMatches = if (file) true else null,
        you = you,
    )

    /** Sol, Mika and Ren, as many of them ready as [ready] says, Sol being this device. */
    fun members(ready: Int = 1, youReady: Boolean = false): List<PartyMember> = listOf(
        member("Sol", youReady, you = true),
        member("Mika", ready >= 1),
        member("Ren", ready >= 2),
    )

    fun view(
        ready: Boolean = false,
        roomEpisode: PartyRoomEpisode? = episode(),
        sources: PartySources? = PartySources.Found,
        connection: PartyConnection = PartyConnection.Connected,
        startingIn: UInt? = null,
        playing: Boolean = false,
        canStartAnyway: Boolean = false,
        looking: Boolean = false,
        guesses: List<PartyGuess> = emptyList(),
    ) = PartyView(
        server = SERVER,
        room = CODE,
        name = "Sol",
        connection = connection,
        encrypted = true,
        ready = ready,
        episode = null,
        invite = LINK,
        motd = null,
        latencyMs = 40u,
        roomEpisode = roomEpisode,
        nextEpisode = null,
        starter = "Mika",
        starterPresent = true,
        canStartAnyway = canStartAnyway,
        code = CODE,
        link = LINK,
        playing = playing,
        startingIn = startingIn,
        sources = sources,
        guesses = guesses,
        looking = looking,
        movesOn = true,
    )

    /** In the room with [view] and [members]. */
    fun inRoom(view: PartyView = view(), members: List<PartyMember> = members()): List<Event> =
        listOf(Event.PartyJoined(view), Event.PartyMembers(members))

    /** A look into the room found [members] watching episode 9. */
    fun glimpse(members: List<PartyMember> = listOf(member("Mika", null), member("Ren", null)), roomEpisode: PartyRoomEpisode? = episode()): Event =
        Event.PartyGlimpse(PartyGlimpse(server = SERVER, room = CODE, members = members, roomEpisode = roomEpisode, nextEpisode = null))

    fun unreachable(): Event = Event.PartyLookFailed(SERVER, CODE, Msg.PartyServerUnreachable(SERVER))

    /** Episode 9 is not out: the sources line says when it airs. */
    fun notAired(): PartySources = PartySources.Missing(NoSourceWhy.NOT_AIRED, Msg.NoticeNotAired(9, Cause.AirsInDays(6L, "17:30")))

    fun guesses(): List<PartyGuess> = listOf(
        PartyGuess(900006u, "Natsukaze Relay", null),
        PartyGuess(900001u, "Hoshikuzu no Tabibito", null),
        PartyGuess(900004u, "Kumo no Ue no Toshokan", null),
    )
}
