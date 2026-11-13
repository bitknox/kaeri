// What came in: the waiting rule on the server form (nothing for 600 ms, then Looking for at least
// 0.3 s), each answer mapped to the form's state, Add server only once there is a result, the
// notices a closing form leaves, and Kavita and invite links recognised on paste.

package app.kaeri.store

import app.kaeri.engine.model.AddedServer
import app.kaeri.engine.model.BookServerKind
import app.kaeri.engine.model.BookSourceCommand
import app.kaeri.engine.model.CameIn
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.FolderReport
import app.kaeri.engine.model.FormatCount
import app.kaeri.engine.model.NothingCameIn
import app.kaeri.engine.model.PluginsInstalled
import app.kaeri.engine.model.ServerFound
import app.kaeri.engine.model.ServerLibrary
import app.kaeri.engine.model.StoreReport
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class CameInModelTest {
    private val host = "books.home.example"
    private val link = "https://$host/api/opds/0f3c9e2d-77aa-4b1e-9d2b-5c1e8a0b3f9a"

    private val found = ServerFound(
        pending = "p1",
        kind = BookServerKind.KAVITA,
        name = host,
        host = host,
        address = "https://$host",
        series = 212u,
        libraries = listOf(ServerLibrary("1", "Manga", 168u), ServerLibrary("2", "Comics", 44u)),
        keyLast4 = "3f9a",
    )

    private fun store(installAll: Boolean = true) = StoreReport(
        store = "kim",
        name = "Kim’s shelf",
        owner = "Kim",
        forReading = listOf("tsuru", "lantern"),
        forWatching = listOf("kawa"),
        otherMedium = 1u,
        otherIsReading = false,
        installAll = installAll,
    )

    private fun folder(series: Int, alreadyAdded: Boolean = false) = FolderReport(
        id = "f1",
        name = "Manga",
        path = "Manga",
        series = series.toUInt(),
        books = 0u,
        files = 1206u,
        matched = 58u,
        unsupported = listOf(FormatCount("MOBI", 14u)),
        locked = emptyList(),
        listed = emptyList(),
        alreadyAdded = alreadyAdded,
    )

    private class Sent : (Command) -> Unit {
        val commands = mutableListOf<Command>()
        override fun invoke(command: Command) {
            commands += command
        }
    }

    private fun TestScope.model(sent: Sent = Sent()) = CameInModel(backgroundScope, sent)

    private fun TestScope.form(model: CameInModel): ServerForm {
        runCurrent()
        return CameInSteps.server(
            model.address.value, model.user.value, model.password.value, model.asked.value,
            model.outcome.value, model.asking.value, model.looking.value,
        )
    }

    // The waiting rule.

    @Test
    fun an_answer_inside_600_ms_shows_with_no_looking_line() = runTest {
        val model = model()
        model.open(Adding.SERVER)
        model.typeAddress(link)
        model.probe(link)
        advanceTimeBy(599)
        assertEquals(ServerStep.Typing, form(model).step)
        model.apply(CameIn.Found(found))
        assertEquals(ServerStep.Found(found), form(model).step)
    }

    @Test
    fun looking_shows_at_600_ms_and_holds_300_ms_before_the_answer() = runTest {
        val model = model()
        model.open(Adding.SERVER)
        model.typeAddress(link)
        model.probe(link)
        advanceTimeBy(601)
        assertEquals(ServerStep.Looking, form(model).step)
        advanceTimeBy(50)
        model.apply(CameIn.Found(found))
        assertEquals(ServerStep.Looking, form(model).step)
        advanceTimeBy(248)
        assertEquals(ServerStep.Looking, form(model).step)
        advanceTimeBy(3)
        assertEquals(ServerStep.Found(found), form(model).step)
    }

    @Test
    fun the_probe_sends_the_address_alone_until_a_password_is_given() = runTest {
        val sent = Sent()
        val model = model(sent)
        model.open(Adding.SERVER)
        model.probe(" comics.home.example ", "kim", "")
        model.probe("comics.home.example", "kim", "secret")
        assertEquals(
            listOf(
                Command.BookSources(BookSourceCommand.Probe("comics.home.example", null, null)),
                Command.BookSources(BookSourceCommand.Probe("comics.home.example", "kim", "secret")),
            ),
            sent.commands,
        )
    }

    // Each answer, as the form shows it.

    private fun step(outcome: CameIn?, address: String = link, withPassword: Boolean = false, user: String = "", password: String = "") =
        CameInSteps.server(address, user, password, CameInModel.Asked(address, withPassword), outcome, asking = false, looking = false)

    @Test
    fun each_answer_maps_to_its_state() {
        val c = "comics.home.example"
        assertEquals(ServerStep.Found(found), step(CameIn.Found(found)).step)
        assertEquals(ServerStep.Empty(found), step(CameIn.Empty(NothingCameIn.EmptyCatalogue(found))).step)
        val unreachable = NothingCameIn.Unreachable(c)
        assertEquals(ServerStep.Failed(unreachable), step(CameIn.Empty(unreachable), c).step)
        val key = NothingCameIn.WrongKey(host)
        assertEquals(ServerStep.Failed(key), step(CameIn.Empty(key)).step)
        val notBooks = NothingCameIn.NotABookServer("media.home.example")
        assertEquals(ServerStep.Failed(notBooks), step(CameIn.Empty(notBooks), "media.home.example").step)
    }

    @Test
    fun a_server_that_wants_a_password_asks_for_one_then_says_it_was_refused() {
        val wants = CameIn.Empty(NothingCameIn.WrongPassword("comics.home.example", BookServerKind.KOMGA))
        val first = step(wants, "comics.home.example")
        assertEquals(ServerStep.SignIn(BookServerKind.KOMGA, "comics.home.example", failed = false), first.step)
        assertTrue(first.signIn)
        assertFalse("no password typed yet", first.canAdd)
        val typed = step(wants, "comics.home.example", user = "kim", password = "x")
        assertTrue(typed.canAdd)
        val refused = step(wants, "comics.home.example", withPassword = true, user = "kim", password = "x")
        assertEquals(ServerStep.SignIn(BookServerKind.KOMGA, "comics.home.example", failed = true), refused.step)
    }

    @Test
    fun add_server_turns_on_only_with_a_result() {
        assertFalse(step(null).canAdd)
        assertFalse(step(CameIn.Empty(NothingCameIn.Unreachable(host))).canAdd)
        assertTrue(step(CameIn.Found(found)).canAdd)
        assertTrue(step(CameIn.Empty(NothingCameIn.EmptyCatalogue(found))).canAdd)
    }

    @Test
    fun typing_another_address_hides_the_answer() {
        val form = CameInSteps.server(
            "media.home.example", "", "", CameInModel.Asked("comics.home.example", false),
            CameIn.Empty(NothingCameIn.Unreachable("comics.home.example")), asking = false, looking = false,
        )
        assertEquals(ServerStep.Typing, form.step)
        assertFalse(form.canAdd)
    }

    @Test
    fun a_kavita_link_hides_the_sign_in_fields() {
        val wants = CameIn.Empty(NothingCameIn.WrongPassword(host, BookServerKind.KAVITA))
        val form = step(wants, link)
        assertFalse(form.signIn)
        assertEquals("3f9a", form.kavita?.last4)
    }

    @Test
    fun a_password_being_tried_keeps_the_fields_and_never_goes_through_empty() {
        val c = "comics.home.example"
        val wants = CameIn.Empty(NothingCameIn.WrongPassword(c, BookServerKind.KOMGA))
        val asked = CameInModel.Asked(c, withPassword = true)
        val quick = CameInSteps.server(c, "kim", "x", asked, wants, asking = true, looking = false)
        assertTrue(quick.step is ServerStep.SignIn)
        assertFalse(quick.canAdd)
        val slow = CameInSteps.server(c, "kim", "x", asked, wants, asking = true, looking = true)
        assertEquals(ServerStep.Looking, slow.step)
        assertTrue(slow.signIn)
    }

    // The folder and store forms.

    @Test
    fun the_folder_and_store_answers_map_to_their_states() {
        assertEquals(CameInSteps.FolderStep.Scanning, CameInSteps.folder(null))
        assertEquals(CameInSteps.FolderStep.Found(folder(64)), CameInSteps.folder(CameIn.Folder(folder(64))))
        val none = CameIn.Empty(NothingCameIn.EmptyFolder(folder(0)))
        assertEquals(CameInSteps.FolderStep.Nothing(folder(0)), CameInSteps.folder(none))
        // A folder that could not be read ends the wait on its own state, in the folder form.
        val unread = CameIn.Empty(NothingCameIn.FolderUnreadable(folder(0)))
        assertEquals(CameInSteps.FolderStep.Unreadable(folder(0)), CameInSteps.folder(unread))
        assertEquals(Adding.FOLDER, CameInModel.formFor(unread))

        val joined = CameInSteps.store(CameIn.Store(store()))
        assertEquals(CameInSteps.StoreAction.INSTALL_ALL, CameInSteps.storeAction(joined))
        assertEquals(CameInSteps.StoreAction.DONE, CameInSteps.storeAction(CameInSteps.store(CameIn.Store(store(installAll = false)))))
        val other = CameInSteps.store(CameIn.Empty(NothingCameIn.OtherMedium(store(false))))
        assertEquals(CameInSteps.StoreStep.OtherMedium(store(false)), other)
        val gone = CameInSteps.store(CameIn.Empty(NothingCameIn.StoreUnreachable("shelf.kim.example")))
        assertEquals(CameInSteps.StoreAction.TRY_AGAIN, CameInSteps.storeAction(gone))
        assertEquals(CameInSteps.StoreStep.Preview, CameInSteps.store(null))
    }

    /**
     * A folder picked again while it is a source says so, and nothing the form does on closing
     * (Cancel, Change, Choose another folder) takes it back out with its series. A folder picked
     * for the first time is still taken back when the form closes without Add folder.
     */
    @Test
    fun a_folder_picked_again_is_never_taken_back() = runTest {
        for (series in listOf(64, 0)) {
            val again = folder(series, alreadyAdded = true)
            val came = if (series > 0) CameIn.Folder(again) else CameIn.Empty(NothingCameIn.EmptyFolder(again))
            val step = CameInSteps.folder(came)
            assertEquals(CameInSteps.FolderStep.Already(again), step)
            assertNull(CameInSteps.takeBack(step))
        }
        // A source that can't be read now, picked again: it says so, and stays.
        val lost = CameInSteps.folder(CameIn.Empty(NothingCameIn.FolderUnreadable(folder(0, alreadyAdded = true))))
        assertTrue(lost is CameInSteps.FolderStep.Unreadable)
        assertNull(CameInSteps.takeBack(lost))
        // Picked for the first time: Cancel and Change take it back out.
        assertEquals("f1", CameInSteps.takeBack(CameInSteps.folder(CameIn.Folder(folder(64)))))
        assertEquals("f1", CameInSteps.takeBack(CameInSteps.folder(CameIn.Empty(NothingCameIn.EmptyFolder(folder(0))))))
        assertEquals("f1", CameInSteps.takeBack(CameInSteps.folder(CameIn.Empty(NothingCameIn.FolderUnreadable(folder(0))))))
        assertNull(CameInSteps.takeBack(CameInSteps.FolderStep.Scanning))

        // Closing on it says nothing was added, and its row doesn't land as new.
        val model = model()
        model.open(Adding.FOLDER)
        model.apply(CameIn.Folder(folder(64, alreadyAdded = true)))
        runCurrent()
        model.close()
        assertNull(model.notice.value)
        assertEquals(emptySet<String>(), model.justAdded.value)
    }

    // What a closing form leaves.

    @Test
    fun an_added_server_closes_its_form_and_leaves_a_notice() = runTest {
        val model = model()
        model.open(Adding.SERVER)
        model.apply(CameIn.ServerAdded(AddedServer("s1", BookServerKind.KAVITA, host, host, 212u, 2u, false)))
        runCurrent()
        assertNull(model.open.value)
        assertEquals(setOf("s1"), model.justAdded.value)
        assertEquals(CameInModel.Notice.Added(host, 212), model.notice.value)
    }

    @Test
    fun installing_all_then_done_leaves_the_plugins_notice() = runTest {
        val model = model()
        model.open(Adding.STORE)
        model.apply(CameIn.Store(store()))
        model.installAll("kim")
        model.apply(CameIn.Installed(PluginsInstalled("kim", "Kim’s shelf", 2u, emptyList())))
        advanceTimeBy(1_000)
        assertTrue(CameInSteps.store(model.outcome.value) is CameInSteps.StoreStep.Installed)
        model.close()
        assertEquals(CameInModel.Notice.Plugins(2, "Kim’s shelf"), model.notice.value)
        assertNull(model.outcome.value)
    }

    @Test
    fun an_answer_for_a_closed_form_is_not_shown_in_another() = runTest {
        val model = model()
        model.open(Adding.STORE)
        model.apply(CameIn.Empty(NothingCameIn.Unreachable(host)))
        runCurrent()
        assertNull(model.outcome.value)
    }

    @Test
    fun closing_first_run_says_nothing() = runTest {
        val model = model()
        model.open(Adding.FOLDER)
        model.apply(CameIn.Folder(folder(64)))
        runCurrent()
        model.closeQuietly()
        assertNull(model.notice.value)
        assertEquals(setOf("f1"), model.justAdded.value)
    }

    // Links recognised on paste.

    @Test
    fun a_kavita_link_is_recognised_as_the_engine_recognises_it() {
        assertEquals(KavitaLink("https://$host", host, "3f9a"), KavitaLink.parse(link))
        assertEquals("https://$host", KavitaLink.parse("$host/api/opds/0f3c9e2d3f9a")?.root)
        assertEquals("https://$host/kavita", KavitaLink.parse("https://$host/kavita/API/OPDS/0f3c9e2d3f9a")?.root)
        assertEquals("http://$host:5000", KavitaLink.parse("http://$host:5000/api/opds/0f3c9e2d3f9a/series")?.root)
        assertEquals(host, KavitaLink.parse("http://$host:5000/api/opds/0f3c9e2d3f9a")?.host)
        assertNull("a short key", KavitaLink.parse("https://$host/api/opds/abc123"))
        assertNull("a key with other characters", KavitaLink.parse("https://$host/api/opds/0f3c9e2d.3f9a"))
        assertNull("a user in the address", KavitaLink.parse("https://kim:pw@$host/api/opds/0f3c9e2d3f9a"))
        assertNull("another OPDS catalogue", KavitaLink.parse("https://$host/opds/v1.2/catalog"))
        assertNull("a bare host", KavitaLink.parse(host))
        assertNull(KavitaLink.parse(""))
    }

    @Test
    fun an_invite_shows_its_host_and_the_codes_last_four() {
        assertEquals(InviteChip("shelf.kim.example", "7Q2M"), InviteChip.parse("https://shelf.kim.example/join/k3v9x7q2m#secret"))
        assertNull(InviteChip.parse("shelf.kim.example"))
    }
}
