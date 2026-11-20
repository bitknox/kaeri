package app.kaeri.store

import app.kaeri.engine.model.Event
import app.kaeri.engine.model.SyncFolderKind
import app.kaeri.engine.model.SyncFolderSpec
import app.kaeri.engine.model.SyncFolderView
import app.kaeri.engine.model.SyncProblem
import app.kaeri.engine.model.SyncSignInStep
import app.kaeri.engine.model.SyncStatus
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Start over without the key, on the restore sheet: its steps, its key and its stop stay there. */
class StartOverTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)
    private val store = AppStore(scope, clock = { 0L })
    private val drive = SyncFolderSpec(SyncFolderKind.GOOGLE_DRIVE, null, null, null)

    @After
    fun tearDown() = scope.cancel()

    /** The picker's set-up found another group: the restore sheet opens for it. */
    private fun restoreSheet() {
        store.sync.show(SyncSheet.PICKER)
        store.startSync("Kim's phone", drive)
        store.apply(Event.SyncProblem(SyncProblem.GroupExists))
        store.sync.askKey(drive)
    }

    @Test
    fun a_start_over_begun_before_the_sign_in_that_found_the_group_ended_keeps_its_steps() {
        restoreSheet()
        store.startSyncOver("Kim's phone", drive)
        // The sign-in that found the other group ends late, after the start over began.
        store.apply(Event.SyncSignIn(SyncFolderKind.GOOGLE_DRIVE, null))
        assertEquals(SyncFolderKind.GOOGLE_DRIVE, store.sync.startOver.value)
        assertEquals(SyncSheet.KEY, store.sync.sheet.value)
        store.apply(Event.SyncSignIn(SyncFolderKind.GOOGLE_DRIVE, SyncSignInStep.Clearing))
        store.apply(Event.SyncSignIn(SyncFolderKind.GOOGLE_DRIVE, SyncSignInStep.Finishing))
        assertTrue(store.sync.settingUp.value)
    }

    @Test
    fun the_new_key_shows_on_the_restore_sheet_with_its_first_line() {
        restoreSheet()
        store.startSyncOver("Kim's phone", drive)
        store.apply(Event.SyncSignIn(SyncFolderKind.GOOGLE_DRIVE, SyncSignInStep.Clearing))
        store.apply(Event.SyncSignIn(SyncFolderKind.GOOGLE_DRIVE, SyncSignInStep.Finishing))
        // A status that says the folder is in use before the key still leaves the key on the sheet.
        store.apply(Event.SyncStatus(SyncStatus(enabled = true, syncing = false, folder = SyncFolderView(SyncFolderKind.GOOGLE_DRIVE, "Google Drive", null))))
        store.apply(Event.SyncRecoveryKey("K7QM4XR9TB2HWN6CP3ZD8FLA"))
        store.apply(Event.SyncSignIn(SyncFolderKind.GOOGLE_DRIVE, null))
        assertTrue(store.sync.keyInSheet.value)
        assertEquals(SyncSheet.KEY, store.sync.sheet.value)
        assertEquals(SyncFolderKind.GOOGLE_DRIVE, store.sync.startOver.value)
        store.sync.recoveryKeySeen()
        store.sync.show(null)
        assertNull(store.sync.startOver.value)
    }

    @Test
    fun a_stop_stays_a_start_over_until_it_is_left() {
        restoreSheet()
        store.startSyncOver("Kim's phone", drive)
        store.apply(Event.SyncSignIn(SyncFolderKind.GOOGLE_DRIVE, SyncSignInStep.Clearing))
        store.apply(Event.SyncProblem(SyncProblem.ClearRefused))
        assertEquals(SyncProblem.ClearRefused, store.sync.problem.value)
        assertEquals(SyncFolderKind.GOOGLE_DRIVE, store.sync.startOver.value)
        // Try again sends it once more, from a clean start.
        store.startSyncOver("Kim's phone", drive)
        assertNull(store.sync.problem.value)
        assertEquals(SyncFolderKind.GOOGLE_DRIVE, store.sync.startOver.value)
        store.apply(Event.SyncProblem(SyncProblem.Unreachable))
        store.sync.startOverLeft()
        assertNull(store.sync.startOver.value)
        assertNull(store.sync.problem.value)
        assertEquals(SyncSheet.KEY, store.sync.sheet.value)
    }

    @Test
    fun a_join_is_no_start_over() {
        restoreSheet()
        store.startSyncOver("Kim's phone", drive)
        store.apply(Event.SyncProblem(SyncProblem.Unreachable))
        store.joinSync("Kim's phone", drive, "K7QM4XR9TB2HWN6CP3ZD8FLA")
        assertNull(store.sync.startOver.value)
    }

    @Test
    fun enter_new_key_opens_the_restore_sheet_whose_cancel_closes_it() {
        store.sync.askKey(drive, back = null)
        assertEquals(SyncSheet.KEY, store.sync.sheet.value)
        assertNull(store.sync.keyBack)
        store.sync.confirmStartOver(true)
        store.sync.show(store.sync.keyBack)
        assertNull(store.sync.sheet.value)
        assertFalse(store.sync.confirmingStartOver.value)
        store.sync.askKey(drive)
        assertEquals(SyncSheet.PICKER, store.sync.keyBack)
    }
}
