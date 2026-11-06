// The fixtures' watch party, for the chat board's shots and recordings (the `party` extra): a
// room on the testing engine's fake Syncplay server on 127.0.0.1, with Mika, Ren and Sol played
// by the run. Nothing leaves the device.

package app.kaeri.store

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.os.Build
import android.util.Log
import kotlinx.coroutines.delay
import kotlinx.coroutines.withTimeoutOrNull
import app.kaeri.engine.ffi.FfiChatFixture
import app.kaeri.engine.ffi.FfiException
import app.kaeri.engine.ffi.FfiFakeParty
import app.kaeri.engine.ffi.partyInviteLink
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.PartyInvite
import app.kaeri.engine.model.Reaction

/**
 * A party state the `party` extra names. The player scenes play the first fixture's episode with
 * the room, at 12:31 as the board has it; the controls stay up unless the scene says otherwise.
 */
enum class PartyShot(val playing: Boolean = true) {
    /** Mika's line arriving over the playing video, the controls gone. */
    CHATLINE,

    /** Six lines at once: three rows and "3 more in chat", the controls gone. */
    CHATBUSY,

    /** The conversation so far with its reactions, the controls up and your laugh's moment. */
    CHATREACT,

    /**
     * The conversation so far, the controls up: the run opens the field with a tap on Chat, then
     * sends go, and Sol's line arrives over the picture.
     */
    CHATFIELD,

    /** A server with chat turned off. */
    CHATOFF,

    /** A server that never echoes: your line comes back as "Couldn't send". */
    CHATFAIL,

    /** Ren muted on this device. */
    CHATMUTE,

    /** Hide chat on. */
    CHATHIDDEN,

    /** A server that takes 40 characters a message. */
    CHATSHORT,

    /** A timeline for recordings: lines arrive one by one, four at once fold, then reactions. */
    CHATLOOP,

    /** The lobby of a room joined from an invite, before play, with its conversation. */
    LOBBY(playing = false),

    /** In the room with the player closed: the party bar, which a new line takes over. */
    BAR(playing = false),
    ;

    companion object {
        fun named(name: String?): PartyShot? = entries.firstOrNull { it.name.equals(name, ignoreCase = true) }
    }
}

/** What the party stage asks of the app's player. */
interface PartyStage {
    /** The player is up with a picture and a length. */
    val playing: Boolean

    /** Hold the controls up for the shot, or let them go. */
    fun holdControls(on: Boolean)

    fun hideControls()

    fun seekTo(seconds: Double)

    /** Pause, or play: the room follows. */
    fun setPlaying(playing: Boolean)

    /** Hide chat, on this device. */
    fun hideChat(on: Boolean)
}

object FixtureParty {
    /** The room every scene's party is in. */
    const val ROOM = "quiet-harbor-k7m2xq9ad4tnwe3p"

    /** The first fixture's episode the party plays. */
    private const val ID = 900001u
    private const val EPISODE = "7"
    /** The name the room knows the episode by, with its tag, as the engine names it. */
    private const val FILE = "Stardust Wayfarer, episode 7 [anilist:900001:7]"
    private const val LENGTH = 1420.0

    /** Where the board's playhead stands: 12:31. */
    private const val AT = 751.0

    /** The fake server, held for the run: it stops when let go. */
    private var held: FfiFakeParty? = null

    private fun log(text: String) = Log.i("kaeri", "KAERI_PARTY $text")

    /** Bring the party to [shot]; true once it is there. */
    suspend fun stage(shot: PartyShot, store: AppStore, stage: PartyStage): Boolean {
        val engine = withTimeoutOrNull(60_000) {
            while (store.engine == null) delay(50)
            store.engine
        } ?: return false
        val fake = try {
            engine.startFakeParty().also { held = it }
        } catch (e: FfiException) {
            Log.w("kaeri", "the fake party server did not start: ${e.message}")
            return false
        }
        val server = fake.server()
        // What the server says of chat is in its welcome, so it is set before joining.
        when (shot) {
            PartyShot.CHATOFF -> fake.setChat(false, null)
            PartyShot.CHATSHORT -> fake.setChat(true, 40u)
            else -> Unit
        }
        stage.hideChat(shot == PartyShot.CHATHIDDEN)
        if (shot == PartyShot.LOBBY) return lobby(store, fake, server)
        store.send(Command.PartyJoin(server, ROOM, "Noa"))
        if (!waited(20) { store.party.view.value != null }) return false
        if (!shot.playing) return bar(store, fake)
        store.send(Command.Play(ID, EPISODE))
        if (!waited(60) { stage.playing }) return false
        // Until this device's member has the episode open, for the others to open the same.
        waited(10) { store.party.members.value.firstOrNull { it.you }?.duration != null }
        val you = store.party.members.value.firstOrNull { it.you }
        val file = you?.file ?: FILE
        val length = you?.duration ?: LENGTH
        stage.holdControls(true)
        stage.setPlaying(false)
        stage.seekTo(AT)
        for (name in listOf("Mika", "Ren", "Sol")) {
            fake.addMember(ROOM, name)
            fake.memberOpens(name, file, length)
            fake.memberReady(name, true)
        }
        if (!waited(10) { store.party.members.value.size == 4 }) return false
        // The joins' notices go, so the column shows chat alone.
        delay(5_500)
        var there = true
        when (shot) {
            PartyShot.CHATLINE, PartyShot.CHATBUSY -> {
                stage.holdControls(false)
                stage.setPlaying(true)
                stage.hideControls()
                delay(800)
                if (shot == PartyShot.CHATLINE) fake.memberSays("Mika", "this part again") else fake.chatFixture(ROOM, FfiChatFixture.BURST)
                there = waited(10) { store.party.conversation.value.lines.isNotEmpty() }
            }
            PartyShot.CHATREACT -> {
                fake.chatFixture(ROOM, FfiChatFixture.CONVERSATION)
                there = waited(10) { store.party.conversation.value.marks.size >= 3 }
                // The column shows three lines, then the folded fourth, Mika's, as the board has it.
                delay(5_400)
                store.partyReact(Reaction.LAUGH)
                there = there && waited(10) { store.party.moment.value?.reaction?.you == true }
            }
            PartyShot.CHATFIELD, PartyShot.CHATMUTE, PartyShot.CHATHIDDEN, PartyShot.CHATSHORT, PartyShot.CHATOFF -> {
                if (shot != PartyShot.CHATOFF) {
                    fake.chatFixture(ROOM, FfiChatFixture.CONVERSATION)
                    store.partySay("back in 2, pause if it gets good")
                    there = waited(10) { store.party.conversation.value.lines.any { it.you } }
                }
                if (shot == PartyShot.CHATMUTE) {
                    store.partyMute("Ren", true)
                    there = there && waited(10) { "Ren" in store.party.chat.value?.muted.orEmpty() }
                }
                if (shot == PartyShot.CHATOFF) there = waited(10) { store.party.chat.value?.allowed == false }
                // The column's lines go before the shot.
                delay(5_000)
                if (shot == PartyShot.CHATFIELD) {
                    // Ready for the run to open the field; once it says go, Sol's line arrives over it.
                    log("ready chatfield")
                    withTimeoutOrNull(120_000) { while (!go) delay(50) }
                    fake.memberSays("Sol", "wait, who is the new runner")
                }
            }
            PartyShot.CHATFAIL -> {
                fake.chatFixture(ROOM, FfiChatFixture.CONVERSATION)
                waited(10) { store.party.conversation.value.lines.size >= 4 }
                delay(4_500)
                fake.swallowChat(true)
                store.partySay("the music in this part")
                there = waited(12) { store.party.conversation.value.failed != null }
            }
            PartyShot.CHATLOOP -> loop(store, fake, stage)
            PartyShot.LOBBY, PartyShot.BAR -> Unit
        }
        log("ready ${shot.name.lowercase()}")
        return there
    }

    /**
     * The recordings' timeline, once the run says go (a broadcast, so the screen recording is
     * already rolling): the controls go, one line, then two more, then four at once, which fold;
     * then reactions and their ticks, the controls up.
     */
    private suspend fun loop(store: AppStore, fake: FfiFakeParty, stage: PartyStage) {
        stage.holdControls(false)
        stage.setPlaying(true)
        stage.hideControls()
        log("waiting")
        withTimeoutOrNull(120_000) { while (!go) delay(50) }
        delay(1_000)
        fake.memberSays("Mika", "ok starting")
        delay(1_400)
        fake.memberSays("Ren", "the handoff in the rain, I knew it")
        delay(900)
        fake.memberSays("Sol", "wait, who is the new runner")
        delay(5_200)
        for ((who, text) in listOf("Mika" to "this part again", "Ren" to "ready", "Sol" to "one sec", "Mika" to "here it comes")) {
            fake.memberSays(who, text)
            delay(250)
        }
        delay(6_500)
        fake.memberReacts("Ren", Reaction.GASP, AT + 8)
        delay(2_200)
        fake.memberReacts("Sol", Reaction.CLAP, AT + 10)
        delay(2_400)
        store.partyReact(Reaction.LAUGH)
        delay(3_000)
        log("loop done")
    }

    /** Set by the run's broadcast: the recording rolls, start the timeline. */
    @Volatile
    private var go = false

    /** The broadcast a run sends once its screen recording rolls. */
    private const val GO = "app.kaeri.fixtures.PARTY_GO"

    /** Hear the run's go: `adb shell am broadcast -a app.kaeri.fixtures.PARTY_GO`. */
    fun listen(context: Context) {
        val receiver = object : BroadcastReceiver() {
            override fun onReceive(context: Context, intent: Intent) {
                go = true
            }
        }
        // The shell sends it, so it is exported; Android 13 asks to be told so.
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            context.applicationContext.registerReceiver(receiver, IntentFilter(GO), Context.RECEIVER_EXPORTED)
        } else {
            context.applicationContext.registerReceiver(receiver, IntentFilter(GO))
        }
    }

    /** The lobby of the room an invite opened, joined, with its members and conversation. */
    private suspend fun lobby(store: AppStore, fake: FfiFakeParty, server: String): Boolean {
        // The room is there before the invite is followed, Mika with the episode open, so the
        // lobby's look in finds them.
        for (name in listOf("Mika", "Ren", "Sol")) fake.addMember(ROOM, name)
        fake.memberSetsPlaylist("Mika", listOf(FILE))
        fake.memberSetsIndex("Mika", 0u)
        fake.memberOpens("Mika", FILE, LENGTH)
        fake.memberReady("Mika", true)
        val link = partyInviteLink(PartyInvite(server, ROOM, ID, EPISODE))
        if (!store.followPartyLink(link)) return false
        waited(20) { store.party.glimpses.value.values.any { it is Glimpse.Found } }
        delay(1_000)
        store.joinParty(server, ROOM, "Noa")
        if (!waited(20) { store.party.view.value != null }) return false
        fake.chatFixture(ROOM, FfiChatFixture.LOBBY)
        val there = waited(10) { store.party.conversation.value.lines.size >= 3 }
        delay(1_000)
        log("ready lobby")
        return there
    }

    /** In the room with the player closed; once the run says go, Mika says something. */
    private suspend fun bar(store: AppStore, fake: FfiFakeParty): Boolean {
        for (name in listOf("Mika", "Ren")) {
            fake.addMember(ROOM, name)
            if (name == "Mika") {
                fake.memberSetsPlaylist(name, listOf(FILE))
                fake.memberSetsIndex(name, 0u)
            }
            fake.memberOpens(name, FILE, LENGTH)
            fake.memberReady(name, true)
        }
        fake.memberPlays("Mika")
        waited(10) { store.party.members.value.size == 3 }
        delay(4_500)
        log("waiting")
        withTimeoutOrNull(120_000) { while (!go) delay(50) }
        delay(800)
        fake.memberSays("Mika", "this part again")
        val there = waited(10) { store.party.conversation.value.lines.isNotEmpty() }
        log("ready bar")
        return there
    }

    private suspend fun waited(seconds: Int, condition: () -> Boolean): Boolean =
        withTimeoutOrNull(seconds * 1_000L) {
            while (!condition()) delay(50)
            true
        } ?: false
}
