// Plugin stores: the stores joined, one row each; a store's page with its plugins and what it is
// (AndroidStore); a plugin's page, with the hosts it may contact; the update confirmation, which
// names the hosts an update adds; and the join sheet (AndroidJoinStore), opened empty or with an
// invite link the app was handed.

package app.kaeri.ui

import app.kaeri.store.plainReason
import android.content.Intent
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.background
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.draw.drawBehind
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.navigation.NavGraphBuilder
import androidx.navigation.compose.composable
import kotlinx.coroutines.delay
import app.kaeri.design.tabular
import app.kaeri.engine.model.StorePluginRow
import app.kaeri.engine.model.StorePluginStatus
import app.kaeri.engine.model.StoreRow
import app.kaeri.engine.model.StoreState
import app.kaeri.store.InviteLinks
import app.kaeri.store.InviteChip
import app.kaeri.store.CameInSteps
import androidx.compose.ui.draw.alpha
import app.kaeri.store.StoresModel
import app.kaeri.store.Wording
import app.kaeri.store.contactsLine
import app.kaeri.store.installedVersion
import app.kaeri.store.runByLine
import app.kaeri.store.standing
import app.kaeri.store.updates
import app.kaeri.i18n.R
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.res.pluralStringResource
import app.kaeri.i18n.Strings
import android.content.Context

internal const val STORES_ROUTE = "stores"

internal fun storeRoute(id: String) = "store/${android.net.Uri.encode(id)}"

internal fun pluginRoute(store: String, plugin: String) = "store/${android.net.Uri.encode(store)}/plugin/${android.net.Uri.encode(plugin)}"

/** The stores' routes: the list, a store's page and a plugin's. */
internal fun NavGraphBuilder.storePages() {
    composable(STORES_ROUTE) { StoresScreen() }
    composable("store/{id}") { entry ->
        StoreScreen(entry.arguments?.getString("id") ?: return@composable)
    }
    composable("store/{id}/plugin/{plugin}") { entry ->
        PluginScreen(
            entry.arguments?.getString("id") ?: return@composable,
            entry.arguments?.getString("plugin") ?: return@composable,
        )
    }
}

/** The stores joined, one row each with how it stands, and Join a store. */
@Composable
internal fun StoresScreen() {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val stores by store.stores.stores.collectAsStateWithLifecycle()
    val loaded by store.stores.loaded.collectAsStateWithLifecycle()
    val offline by store.offline.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { store.loadStores() }
    ScreenFrame(title = stringResource(R.string.nav_plugin_stores), onBack = navigator::back) { padding ->
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(top = padding.calculateTopPadding() + 8.dp, bottom = 48.dp)
                .padding(horizontal = 16.dp)
                .readingWidth(),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            if (!loaded && stores.isEmpty()) {
                PlaceholderRows(2)
            } else if (stores.isEmpty()) {
                Column(Modifier.fillMaxWidth().padding(vertical = 24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                    app.kaeri.design.Nagare(cell = 5)
                    Text(stringResource(R.string.stores_empty_title), style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(top = 16.dp))
                    SheetText(stringResource(R.string.stores_empty_detail), Modifier.padding(top = 4.dp, start = 24.dp, end = 24.dp))
                }
            } else {
                SettingsGroup(
                    rows = stores.map { row ->
                        @Composable { place: Place ->
                            InfoRow(
                                place,
                                row.name,
                                storeLine(LocalContext.current, row, offline),
                                onClick = { navigator.open(storeRoute(row.id)) },
                            )
                        }
                    },
                )
            }
            Row(Modifier.padding(horizontal = 4.dp)) {
                Button(onClick = { store.stores.openJoin() }) { Text(stringResource(R.string.stores_join_short)) }
            }
            SettingsFootnote(stringResource(R.string.stores_not_from_app))
        }
    }
}

/** A store's one line: its host, then what it has waiting or why it is not answering. */
private fun storeLine(context: Context, row: StoreRow, offline: Boolean): String = listOfNotNull(
    row.host,
    when (val state = row.state) {
        // Offline, no store is said to be failing: the one offline line says why.
        is StoreState.Offline -> if (offline) null else Strings.storesStatusNotAnswering(context)
        StoreState.Revoked -> Strings.storesStatusAccessEnded(context)
        StoreState.Ready -> row.updates.takeIf { it > 0 }?.let { context.resources.getQuantityString(R.plurals.stores_updates_waiting, it, it) }
    },
).joinToString(" · ") // i18n-skip

/** A store's page: its plugins, each to install or update, and what the store is. */
@Composable
internal fun StoreScreen(id: String) {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val stores by store.stores.stores.collectAsStateWithLifecycle()
    val working by store.stores.working.collectAsStateWithLifecycle()
    val row = stores.firstOrNull { it.id == id }
    val offline by store.offline.collectAsStateWithLifecycle()
    var updating by remember { mutableStateOf<StorePluginRow?>(null) }
    var leaving by remember { mutableStateOf(false) }
    var inviting by remember { mutableStateOf(false) }
    var cancelling by remember { mutableStateOf<app.kaeri.engine.model.StoreInviteRow?>(null) }
    var looking by remember { mutableStateOf<app.kaeri.engine.model.StoreInviteRow?>(null) }
    val said by store.stores.members.collectAsStateWithLifecycle()
    val theirs = said?.takeIf { it.store == id }
    LaunchedEffect(Unit) { if (!store.stores.loaded.value) store.loadStores() }
    LaunchedEffect(row?.owner, row?.id) { if (row?.owner == true) store.loadMembers(id) }
    val lastLink by store.stores.invite.collectAsStateWithLifecycle()
    LaunchedEffect(lastLink) { if (lastLink != null && row?.owner == true) store.loadMembers(id) }
    ScreenFrame(title = row?.name ?: stringResource(R.string.stores_fallback_title), subtitle = row?.host, onBack = navigator::back) { padding ->
        row ?: return@ScreenFrame
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(top = padding.calculateTopPadding() + 8.dp, bottom = 48.dp)
                .padding(horizontal = 16.dp)
                .readingWidth(),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            when (val state = row.state) {
                is StoreState.Offline -> if (!offline) SettingsFootnote(stringResource(R.string.stores_offline_last_list, plainReason(LocalContext.current, state.v1).trimEnd('.')))
                StoreState.Revoked -> SettingsFootnote(stringResource(R.string.stores_access_ended_page))
                StoreState.Ready -> {}
            }
            SettingsGroup(
                title = stringResource(R.string.stores_plugins),
                footer = stringResource(R.string.stores_plugin_limits),
                rows = row.plugins.map { plugin ->
                    @Composable { place: Place ->
                        val installing = store.stores.isWorking(working, StoresModel.Work.Install(row.id, plugin.id))
                        InfoRow(
                            place,
                            plugin.name,
                            plugin.contactsLine(LocalContext.current),
                            onClick = { navigator.open(pluginRoute(row.id, plugin.id)) },
                            trailing = { PluginAction(plugin, installing, onUpdate = { updating = plugin }) { store.installPlugin(row.id, plugin.id) } },
                        )
                    }
                },
            )
            if (row.owner && !row.anonymous) {
                val context = LocalContext.current
                val live = theirs?.invites.orEmpty()
                SettingsGroup(
                    title = stringResource(R.string.stores_invite_links),
                    rows = buildList<@Composable (Place) -> Unit> {
                        add { place -> ActionRow(place, stringResource(R.string.stores_invite_someone)) { inviting = true } }
                        live.forEach { invite ->
                            add { place -> InfoRow(place, invite.title(context), invite.standing(context), onClick = { looking = invite }) }
                        }
                    },
                )
                val ended = theirs?.ended.orEmpty()
                if (ended.isNotEmpty()) {
                    SettingsGroup(
                        title = stringResource(R.string.stores_invites_ended),
                        rows = ended.map { invite ->
                            @Composable { place: Place -> InfoRow(place, invite.title(context), invite.standing(context), onClick = if (invite.uses.isNotEmpty()) ({ looking = invite }) else null) }
                        },
                    )
                }
            }
            SettingsGroup(
                title = stringResource(R.string.stores_this_store),
                footer = listOfNotNull(
                    if (row.anonymous) stringResource(R.string.stores_joined_nameless) else stringResource(R.string.stores_device_joined_as, row.member),
                    row.checkedAt?.let { stringResource(R.string.stores_checked_ago, Wording.ago(LocalContext.current, System.currentTimeMillis() / 1000 - it)) },
                ).joinToString(" · "), // i18n-skip
                rows = buildList<@Composable (Place) -> Unit> {
                    add { place -> InfoRow(place, stringResource(R.string.stores_address), row.host) }
                    add { place -> InfoRow(place, stringResource(R.string.stores_key_fingerprint), row.fingerprint) }
                    // Who can join is changed on the store's admin page, in a browser.
                    if (row.open) add { place -> InfoRow(place, stringResource(R.string.stores_who_can_join), stringResource(R.string.stores_anyone_short)) }
                },
            )
            FlowRow(Modifier.padding(horizontal = 4.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                // A device that joined with the store's link joins another the same way.
                if (!row.anonymous) {
                    TextButton(
                        onClick = { store.createInvite(row.id, name = null) },
                        enabled = !store.stores.isWorking(working, StoresModel.Work.Invite(row.id)),
                    ) { Text(stringResource(R.string.stores_link_device_title), color = MaterialTheme.colorScheme.onSurface) }
                }
                // Destructive, so confirmed; in silk, as vermilion is only for a problem.
                TextButton(onClick = { leaving = true }) { Text(stringResource(R.string.stores_leave), color = MaterialTheme.colorScheme.onSurface) }
            }
        }
    }
    updating?.let { plugin ->
        val status = plugin.status as? StorePluginStatus.Update
        UpdateDialog(
            plugin.name,
            storeName = row?.name ?: stringResource(R.string.stores_fallback_name),
            from = status?.from,
            to = plugin.version,
            newHosts = status?.newHosts.orEmpty(),
            onDismiss = { updating = null },
        ) {
            updating = null
            store.installPlugin(id, plugin.id)
        }
    }
    if (leaving && row != null) {
        AlertDialog(
            onDismissRequest = { leaving = false },
            properties = AskingDialog,
            title = { Text(stringResource(R.string.stores_leave_title, row.name)) },
            text = { Text(stringResource(R.string.stores_leave_rejoin)) },
            confirmButton = {
                TextButton(onClick = {
                    leaving = false
                    store.leaveStore(row.id)
                    navigator.back()
                }) { Text(stringResource(R.string.action_leave), color = MaterialTheme.colorScheme.onSurface) }
            },
            dismissButton = { QuietButton(stringResource(R.string.action_cancel)) { leaving = false } },
            containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
        )
    }
    if (inviting && row != null) InviteSomeoneSheet(row.id, theirs?.inviteChoices ?: true) { inviting = false }
    looking?.let { shown ->
        // The row as the engine last said it, so a removed member reads as removed.
        val current = (theirs?.invites.orEmpty() + theirs?.ended.orEmpty()).firstOrNull { it.id == shown.id } ?: shown
        InviteDetailSheet(current, id, onCancel = if (current.ended == null) ({ looking = null; cancelling = current }) else null, onDismiss = { looking = null })
    }
    cancelling?.let { invite ->
        CancelInviteDialog(invite, onDismiss = { cancelling = null }) {
            cancelling = null
            if (row != null) store.cancelInvite(row.id, invite.id)
        }
    }
    InviteLinkSheetHost()
}

/** What a store's plugin offers: Installed with a water check, Update in amber, Install tonal. */
@Composable
private fun PluginAction(plugin: StorePluginRow, installing: Boolean, onUpdate: () -> Unit, onInstall: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    when (val status = plugin.status) {
        StorePluginStatus.Installed -> Row(horizontalArrangement = Arrangement.spacedBy(4.dp), verticalAlignment = Alignment.CenterVertically) {
            Icon(Icons.Check, contentDescription = null, tint = colors.secondary, modifier = Modifier.size(18.dp))
            Text(stringResource(R.string.stores_plugin_state_installed), style = MaterialTheme.typography.labelLarge, color = colors.onSurfaceVariant)
        }
        is StorePluginStatus.Update -> Button(onClick = onUpdate, enabled = !installing) { Text(stringResource(R.string.action_update)) }
        StorePluginStatus.Available -> FilledTonalButton(onClick = onInstall, enabled = !installing) { WorkLabel(stringResource(R.string.action_install), installing) }
        StorePluginStatus.AddedByHand -> FilledTonalButton(onClick = onUpdate, enabled = !installing) { Text(stringResource(R.string.action_replace)) }
        is StorePluginStatus.OtherStore -> Text(stringResource(R.string.stores_plugin_from, status.v1), style = MaterialTheme.typography.labelLarge, color = colors.onSurfaceVariant)
    }
}

/** A store's plugin: its version, the hosts it may contact, and installing, updating or removing it. */
@Composable
internal fun PluginScreen(storeId: String, pluginId: String) {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val stores by store.stores.stores.collectAsStateWithLifecycle()
    val working by store.stores.working.collectAsStateWithLifecycle()
    val row = stores.firstOrNull { it.id == storeId }
    val plugin = row?.plugins?.firstOrNull { it.id == pluginId }
    var updating by remember { mutableStateOf(false) }
    var removing by remember { mutableStateOf(false) }
    ScreenFrame(title = plugin?.name ?: stringResource(R.string.stores_fallback_plugin), subtitle = row?.let { stringResource(R.string.stores_plugin_from, it.name) }, onBack = navigator::back) { padding ->
        if (row == null || plugin == null) return@ScreenFrame
        val installing = store.stores.isWorking(working, StoresModel.Work.Install(row.id, plugin.id))
        val status = plugin.status
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(top = padding.calculateTopPadding() + 8.dp, bottom = 48.dp)
                .padding(horizontal = 16.dp)
                .readingWidth(),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Row(Modifier.padding(horizontal = 4.dp, vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                when (status) {
                    is StorePluginStatus.Update -> Button(onClick = { updating = true }, enabled = !installing) { Text(stringResource(R.string.updates_update_to, plugin.version)) }
                    StorePluginStatus.Available -> Button(onClick = { store.installPlugin(row.id, plugin.id) }, enabled = !installing) { WorkLabel(stringResource(R.string.action_install), installing) }
                    else -> {}
                }
                if (status is StorePluginStatus.Installed || status is StorePluginStatus.Update) {
                    TextButton(onClick = { removing = true }) { Text(stringResource(R.string.action_remove), color = MaterialTheme.colorScheme.onSurface) }
                }
            }
            SettingsGroup(
                footer = stringResource(R.string.stores_plugin_limits),
                rows = listOf(
                    { place ->
                        InfoRow(
                            place,
                            stringResource(R.string.sources_version),
                            when (status) {
                                is StorePluginStatus.Update -> stringResource(R.string.stores_plugin_update_line, status.from, plugin.version)
                                StorePluginStatus.Available -> plugin.version
                                else -> plugin.installedVersion
                            },
                        )
                    },
                    { place -> InfoRow(place, stringResource(R.string.sources_contacts_label), plugin.hosts.ifEmpty { listOf(stringResource(R.string.sources_no_hosts)) }.joinToString("\n")) },
                ),
            )
        }
    }
    if (updating && plugin != null) {
        val status = plugin.status as? StorePluginStatus.Update
        UpdateDialog(plugin.name, row?.name ?: stringResource(R.string.stores_fallback_name), status?.from, plugin.version, status?.newHosts.orEmpty(), onDismiss = { updating = false }) {
            updating = false
            store.installPlugin(storeId, plugin.id)
        }
    }
    if (removing && plugin != null) {
        AlertDialog(
            onDismissRequest = { removing = false },
            properties = AskingDialog,
            title = { Text(stringResource(R.string.stores_remove_plugin_title, plugin.name)) },
            text = { Text(stringResource(R.string.stores_remove_plugin_detail)) },
            confirmButton = {
                QuietButton(stringResource(R.string.action_remove)) {
                    removing = false
                    store.removePlugin(plugin.id)
                }
            },
            dismissButton = { QuietButton(stringResource(R.string.action_cancel)) { removing = false } },
            containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
        )
    }
}

/**
 * The store form (AndroidJoinStore): the invite, its host in the field with the code's last four
 * on a chip; what the link leads to, checked a moment after it is pasted or typed; and Join. Once
 * joined, what came in: the plugins for what this device does, each with Install, and Install all
 * for reading when two or more fit; or why nothing did, with Try again when the store did not
 * answer. [onClose] runs once it closes.
 */
@Composable
internal fun storeForm(state: app.kaeri.ui.reading.CameInState, onClose: () -> Unit): app.kaeri.ui.reading.CameInForm {
    val store = LocalAppStore.current
    val model = store.cameIn
    val context = LocalContext.current
    val preview by store.stores.preview.collectAsStateWithLifecycle()
    val stores by store.stores.stores.collectAsStateWithLifecycle()
    val working by store.stores.working.collectAsStateWithLifecycle()
    val report by model.report.collectAsStateWithLifecycle()
    val installedHere by model.installedHere.collectAsStateWithLifecycle()
    val offline by store.offline.collectAsStateWithLifecycle()
    val link = state.address.trim()
    val chip = InviteChip.parse(link)
    val step = CameInSteps.store(state.outcome)
    LaunchedEffect(link, state.open) {
        if (state.open != app.kaeri.store.Adding.STORE || state.outcome != null || state.asking) return@LaunchedEffect
        delay(300)
        val checked = (store.stores.preview.value as? StoresModel.Preview.Store)?.store?.link
        when {
            !InviteLinks.looksLikeInvite(link) -> store.stores.clearPreview()
            // Checked already, as the form opened again with the same link.
            checked == link -> {}
            else -> store.previewStore(link)
        }
    }
    val invited = (preview as? StoresModel.Preview.Store)?.store
    val join = {
        model.joining()
        store.joinStore(link)
    }
    val joinedStore = when (step) {
        is CameInSteps.StoreStep.Joined -> step.report.store
        is CameInSteps.StoreStep.Installed -> step.installed.store
        is CameInSteps.StoreStep.OtherMedium -> step.report.store
        is CameInSteps.StoreStep.Empty -> step.report.store
        else -> null
    }
    val row = joinedStore?.let { id -> stores.firstOrNull { it.id == id } }
    val primary = when (CameInSteps.storeAction(step)) {
        CameInSteps.StoreAction.INSTALL_ALL -> app.kaeri.ui.reading.CameInAction(
            stringResource(R.string.store_install_all_reading),
            working = state.asking && state.looking,
        ) { joinedStore?.let(model::installAll) }
        CameInSteps.StoreAction.DONE -> app.kaeri.ui.reading.CameInAction(stringResource(R.string.action_done), onClick = onClose)
        CameInSteps.StoreAction.TRY_AGAIN -> app.kaeri.ui.reading.CameInAction(stringResource(R.string.action_try_again), working = state.asking && state.looking, onClick = join)
        null -> app.kaeri.ui.reading.CameInAction(
            stringResource(R.string.onboarding_join),
            enabled = chip != null && invited?.alreadyJoined != true && !state.asking,
            working = state.asking && state.looking,
            onClick = join,
        )
    }
    val secondary = if (step is CameInSteps.StoreStep.OtherMedium) {
        app.kaeri.ui.reading.CameInAction(stringResource(R.string.store_turn_on_watching)) { store.medium.choose(app.kaeri.engine.model.KaeriIsFor.BOTH) }
    } else {
        null
    }
    return app.kaeri.ui.reading.CameInForm(
        title = stringResource(R.string.store_join_title),
        primary = primary,
        secondary = secondary,
        close = onClose,
    ) { inPage ->
        app.kaeri.ui.reading.CameInField(
            state.address,
            model::typeAddress,
            stringResource(R.string.store_invite_field),
            chip = chip?.let { Strings.storeInviteChip(context, it.last4) },
            shownAs = chip?.host,
            supporting = if (link.isEmpty()) stringResource(R.string.stores_join_detail) else null,
            keyboard = KeyboardOptions(keyboardType = KeyboardType.Uri, autoCorrectEnabled = false),
        )
        app.kaeri.ui.reading.CameInSwap(storeResultKey(step, state.asking && state.looking), Modifier.padding(top = 16.dp)) { shown ->
            Column {
                when (shown) {
                    StoreShown.Looking -> app.kaeri.ui.reading.CameInWaiting(stringResource(R.string.stores_checking_link))
                    is StoreShown.Step -> when (val at = shown.step) {
                        CameInSteps.StoreStep.Preview -> JoinPreview(preview, offline)
                        is CameInSteps.StoreStep.Joined -> {
                            val offered = at.report.forReading.size + at.report.forWatching.size + at.report.otherMedium.toInt()
                            app.kaeri.ui.reading.CameInNote(
                                app.kaeri.ui.reading.NoteKind.FOUND,
                                Strings.storeJoinedTitle(context, at.report.name),
                                Strings.storeJoinedLine(context, at.report.owner ?: at.report.name, offered),
                            )
                            JoinedPlugins(at.report, row, installedHere, working, installing = state.asking)
                        }
                        is CameInSteps.StoreStep.Installed -> {
                            app.kaeri.ui.reading.CameInNote(
                                app.kaeri.ui.reading.NoteKind.FOUND,
                                Strings.storeInstalled(context, at.installed.installed.toInt()),
                            )
                            report?.let { JoinedPlugins(it, row, installedHere + it.forReading, working, installing = false) }
                        }
                        is CameInSteps.StoreStep.OtherMedium -> {
                            app.kaeri.ui.reading.CameInNote(
                                app.kaeri.ui.reading.NoteKind.INFO,
                                Strings.storeOtherMedium(context, at.report.name),
                                stringResource(R.string.store_other_medium_line),
                            )
                            val others = row?.plugins.orEmpty()
                            if (others.isNotEmpty()) {
                                app.kaeri.ui.reading.CameInGroupHeading(
                                    stringResource(if (at.report.otherIsReading) R.string.store_group_reading else R.string.store_group_watching),
                                    Modifier.padding(top = 24.dp),
                                )
                                app.kaeri.ui.reading.CameInGroup(others.map { plugin -> @Composable { JoinedPluginRow(plugin, dim = true) } })
                            }
                            app.kaeri.ui.reading.SecondaryInBody(secondary, inPage)
                        }
                        is CameInSteps.StoreStep.Empty -> app.kaeri.ui.reading.CameInNote(
                            app.kaeri.ui.reading.NoteKind.INFO,
                            Strings.storeEmpty(context, at.report.name),
                            Strings.storeEmptyLine(context, at.report.owner ?: at.report.name),
                        )
                        is CameInSteps.StoreStep.Unreachable -> app.kaeri.ui.reading.CameInNote(
                            app.kaeri.ui.reading.NoteKind.FAILED,
                            Strings.storeErrUnreachable(context, at.host),
                            Strings.storeErrUnreachableLine(context, invited?.owners?.firstOrNull() ?: at.host),
                        )
                    }
                }
            }
        }
    }
}

/** What the store form's result area shows: the waiting line, or a step. */
private sealed interface StoreShown {
    data object Looking : StoreShown
    data class Step(val step: CameInSteps.StoreStep) : StoreShown
}

private fun storeResultKey(step: CameInSteps.StoreStep, looking: Boolean): StoreShown =
    if (looking) StoreShown.Looking else StoreShown.Step(step)

/** Before joining: what the link leads to, or why it can't be read, and the name the store will use. */
@Composable
private fun JoinPreview(preview: StoresModel.Preview?, offline: Boolean) {
    val store = LocalAppStore.current
    val deviceName by store.stores.deviceName.collectAsStateWithLifecycle()
    when (preview) {
        is StoresModel.Preview.Checking -> app.kaeri.ui.reading.CameInWaiting(stringResource(R.string.stores_checking_link))
        is StoresModel.Preview.Failed -> {
            app.kaeri.ui.reading.CameInNote(
                app.kaeri.ui.reading.NoteKind.FAILED,
                if (offline) stringResource(R.string.state_offline_line) else plainReason(LocalContext.current, preview.reason),
                preview.contact?.takeIf { !offline }?.let { stringResource(R.string.store_invite_contact, it) },
            )
        }
        is StoresModel.Preview.Store -> {
            val invited = preview.store
            InvitedStorePreview(invited)
            // What the store will call this device: its kind, never its own name, to change first.
            if (!invited.anonymous && app.kaeri.ui.reading.largePaper) {
                app.kaeri.ui.reading.CameInField(
                    deviceName,
                    store.stores::nameDevice,
                    stringResource(if (invited.namesJoiner) R.string.stores_join_your_name else R.string.stores_device_name),
                    modifier = Modifier.padding(top = 18.dp),
                    keyboard = KeyboardOptions(capitalization = KeyboardCapitalization.Words),
                    supporting = stringResource(if (invited.namesJoiner) R.string.stores_join_your_name_detail else R.string.stores_device_name_detail),
                )
            } else if (!invited.anonymous) {
                SheetField(
                    deviceName,
                    store.stores::nameDevice,
                    stringResource(if (invited.namesJoiner) R.string.stores_join_your_name else R.string.stores_device_name),
                    modifier = Modifier.padding(top = 16.dp),
                    keyboard = KeyboardOptions(capitalization = KeyboardCapitalization.Words),
                    supporting = stringResource(if (invited.namesJoiner) R.string.stores_join_your_name_detail else R.string.stores_device_name_detail),
                )
            }
            if (app.kaeri.ui.reading.largePaper) {
                Text(
                    stringResource(R.string.stores_not_from_app),
                    style = app.kaeri.eink.EinkType.Secondary.copy(lineHeight = app.kaeri.eink.EinkType.Secondary.fontSize * 1.4f),
                    modifier = Modifier.padding(top = 16.dp),
                )
            } else {
                SheetText(stringResource(R.string.stores_not_from_app), Modifier.padding(top = 16.dp))
            }
        }
        null -> {}
    }
}

/**
 * A joined store's plugins for what this device does, under For reading (and For watching where it
 * watches too), each with Install, or Installed once it is; then how many more are for the other
 * medium.
 */
@Composable
private fun JoinedPlugins(
    report: app.kaeri.engine.model.StoreReport,
    row: StoreRow?,
    installed: Set<String>,
    working: Map<app.kaeri.engine.model.JobId, StoresModel.Work>,
    installing: Boolean,
) {
    val store = LocalAppStore.current
    val model = store.cameIn
    val context = LocalContext.current
    val plugins = row?.plugins.orEmpty().associateBy { it.id }
    val groups = listOf(R.string.store_group_reading to report.forReading, R.string.store_group_watching to report.forWatching)
    groups.forEach { (heading, ids) ->
        val shown = ids.mapNotNull { plugins[it] }
        if (shown.isEmpty()) return@forEach
        app.kaeri.ui.reading.CameInGroupHeading(stringResource(heading), Modifier.padding(top = 24.dp))
        app.kaeri.ui.reading.CameInGroup(
            shown.map { plugin ->
                @Composable {
                    val done = plugin.id in installed || plugin.status == StorePluginStatus.Installed
                    val busy = installing || store.stores.isWorking(working, StoresModel.Work.Install(report.store, plugin.id))
                    JoinedPluginRow(plugin) {
                        if (done) {
                            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                                Text(stringResource(R.string.store_installed_row), style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
                                Icon(Icons.Check, contentDescription = null, tint = MaterialTheme.colorScheme.onSurface, modifier = Modifier.size(20.dp))
                            }
                        } else {
                            FilledTonalButton(
                                onClick = {
                                    store.installPlugin(report.store, plugin.id)
                                    model.installedOne(plugin.id)
                                },
                                enabled = !busy,
                                contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 16.dp),
                            ) { WorkLabel(stringResource(R.string.store_install), busy) }
                        }
                    }
                }
            },
        )
    }
    if (report.otherMedium > 0u && !report.otherIsReading) {
        app.kaeri.ui.reading.CameInFootnote(Strings.storeOtherMediumMore(context, report.otherMedium.toInt()))
    }
}

/** A plugin in the store form: its tile, name, version and the hosts it may contact; dimmed when it can't run here. */
@Composable
private fun JoinedPluginRow(plugin: StorePluginRow, dim: Boolean = false, trailing: @Composable () -> Unit = {}) {
    val context = LocalContext.current
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 72.dp)
            .alpha(if (dim) 0.6f else 1f)
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        app.kaeri.ui.reading.CameInTile(app.kaeri.ui.reading.CameInGlyphs.Plug)
        app.kaeri.ui.reading.CameInRowText(
            plugin.name,
            Strings.storePluginRow(context, plugin.version, Strings.storePluginHosts(context, plugin.hosts.size)),
            Modifier.weight(1f),
        )
        trailing()
    }
}

/**
 * What the link leads to (AndroidJoinStore): the store, who runs it and how many plugins it
 * offers, its key matched against the link's, and each plugin with what it may contact, where it
 * stands here and its version. For a store anyone with its link can join, that it is open, and
 * that its open link joins without a name. A store from before the preview shows its name and key.
 */
@Composable
internal fun InvitedStorePreview(invited: app.kaeri.engine.model.InvitedStore) {
    if (app.kaeri.ui.reading.largePaper) return PaperStorePreview(invited)
    val colors = MaterialTheme.colorScheme
    val tabular = MaterialTheme.typography.bodyMedium.tabular()
    HorizontalDivider(Modifier.padding(top = 16.dp), color = colors.outlineVariant)
    PreviewLine(invited.name, invited.runByLine(LocalContext.current), titleWeight = FontWeight.Medium) {
        if (invited.plugins.isNotEmpty()) {
            Text(pluralStringResource(R.plurals.stores_offers_plugins, invited.plugins.size, invited.plugins.size), style = tabular, color = colors.onSurfaceVariant)
        }
    }
    PreviewLine(
        stringResource(if (invited.anonymous) R.string.stores_preview_key_link else R.string.stores_preview_key_invite),
        invited.fingerprint,
        subtitleStyle = MaterialTheme.typography.bodyMedium.copy(fontFamily = FontFamily.Monospace),
    ) {
        Icon(Icons.Shield, contentDescription = stringResource(R.string.stores_key_verified), tint = colors.secondary)
    }
    if (invited.open) {
        PreviewLine(
            stringResource(R.string.stores_anyone_with_link),
            if (invited.anonymous) stringResource(R.string.stores_anyone_note) else null,
        )
    }
    if (invited.alreadyJoined) SheetText(stringResource(R.string.stores_already_joined_note), Modifier.padding(vertical = 8.dp))
    if (invited.plugins.isNotEmpty()) {
        HorizontalDivider(color = colors.outlineVariant)
        invited.plugins.forEach { plugin ->
            val context = LocalContext.current
            val line = listOfNotNull(plugin.contactsLine(context), plugin.standing(context)).joinToString(" · ") // i18n-skip
            PreviewLine(plugin.name, line) {
                Text(plugin.version, style = tabular, color = colors.onSurfaceVariant)
            }
        }
    }
}

/**
 * What the link leads to, on a large e-ink panel: under a 2 dp rule the store, who runs it and how
 * many plugins it offers; its key in words; another rule; then the plugins in two columns, each
 * with the hosts it may contact and its version. Words only: no shield, no tiles.
 */
@Composable
private fun PaperStorePreview(invited: app.kaeri.engine.model.InvitedStore) {
    val context = LocalContext.current
    val ink = app.kaeri.eink.EinkColours.Ink
    val secondary = app.kaeri.eink.EinkType.Secondary
    Box(Modifier.padding(top = 18.dp).fillMaxWidth().height(app.kaeri.eink.EinkSizes.Rule).background(ink))
    PaperLine(invited.name, invited.runByLine(context), first = true) {
        if (invited.plugins.isNotEmpty()) Text(pluralStringResource(R.plurals.stores_offers_plugins, invited.plugins.size, invited.plugins.size), style = secondary)
    }
    PaperLine(
        stringResource(if (invited.anonymous) R.string.stores_preview_key_link else R.string.stores_preview_key_invite),
        invited.fingerprint,
        lineStyle = secondary.copy(fontFamily = FontFamily.Monospace),
    ) {
        Text(stringResource(R.string.stores_key_verified), style = secondary.copy(fontWeight = FontWeight.Bold))
    }
    if (invited.open) {
        PaperLine(stringResource(R.string.stores_anyone_with_link), if (invited.anonymous) stringResource(R.string.stores_anyone_note) else null)
    }
    Box(Modifier.fillMaxWidth().height(app.kaeri.eink.EinkSizes.Rule).background(ink))
    if (invited.alreadyJoined) Text(stringResource(R.string.stores_already_joined_note), style = secondary, modifier = Modifier.padding(top = 12.dp))
    if (invited.plugins.isEmpty()) return
    Row(Modifier.fillMaxWidth().padding(top = 14.dp, bottom = 4.dp).semantics(mergeDescendants = true) { heading() }, verticalAlignment = Alignment.CenterVertically) {
        Text(stringResource(R.string.stores_preview_plugins), style = app.kaeri.eink.EinkType.RowTitle.copy(fontWeight = FontWeight.Bold), modifier = Modifier.weight(1f))
        Text(invited.plugins.size.toString(), style = secondary) // i18n-skip
    }
    // The first half down the left column, the rest down the right, as a list reads.
    val half = (invited.plugins.size + 1) / 2
    Row(horizontalArrangement = Arrangement.spacedBy(32.dp)) {
        listOf(invited.plugins.take(half), invited.plugins.drop(half)).forEach { column ->
            Column(Modifier.weight(1f)) {
                column.forEachIndexed { index, plugin ->
                    val line = listOfNotNull(plugin.contactsLine(context), plugin.standing(context)).joinToString(" · ") // i18n-skip
                    PaperLine(plugin.name, line, first = index == 0, height = 60.dp, titleStyle = app.kaeri.eink.EinkType.Label) {
                        Text(plugin.version, style = secondary)
                    }
                }
            }
        }
    }
}

/** A line of the preview on paper: a title over a line in #333, what sits at its end, and a hairline over it after the first. */
@Composable
private fun PaperLine(
    title: String,
    line: String?,
    first: Boolean = false,
    height: androidx.compose.ui.unit.Dp = 72.dp,
    titleStyle: androidx.compose.ui.text.TextStyle = app.kaeri.eink.EinkType.RowTitle,
    lineStyle: androidx.compose.ui.text.TextStyle = app.kaeri.eink.EinkType.Secondary,
    trailing: @Composable () -> Unit = {},
) {
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = height)
            .then(if (first) Modifier else Modifier.drawBehind {
                val y = app.kaeri.eink.EinkSizes.Hairline.toPx() / 2
                drawLine(app.kaeri.eink.EinkColours.Tertiary, androidx.compose.ui.geometry.Offset(0f, y), androidx.compose.ui.geometry.Offset(size.width, y), app.kaeri.eink.EinkSizes.Hairline.toPx())
            })
            .padding(vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Column(Modifier.weight(1f)) {
            Text(title, style = titleStyle, maxLines = 1, overflow = TextOverflow.Ellipsis)
            line?.let { Text(it, style = lineStyle, maxLines = 2, overflow = TextOverflow.Ellipsis) }
        }
        trailing()
    }
}

/** One line of the join sheet's preview: a title, a line under it in haze, and what sits at its end. */
@Composable
private fun PreviewLine(
    title: String,
    subtitle: String?,
    titleWeight: FontWeight? = null,
    subtitleStyle: androidx.compose.ui.text.TextStyle = MaterialTheme.typography.bodyMedium,
    trailing: @Composable () -> Unit = {},
) {
    Row(
        Modifier.fillMaxWidth().heightIn(min = 72.dp).padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.bodyLarge, fontWeight = titleWeight, maxLines = 1, overflow = TextOverflow.Ellipsis)
            subtitle?.let { Text(it, style = subtitleStyle, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 2, overflow = TextOverflow.Ellipsis) }
        }
        trailing()
    }
}

/** An invite link for another device of yours, to share or copy, once the store has made it. */
@Composable
private fun InviteLinkSheetHost() {
    val store = LocalAppStore.current
    val invite by store.stores.invite.collectAsStateWithLifecycle()
    val shown = invite ?: return
    val context = LocalContext.current
    val clipboard = LocalClipboardManager.current
    SettingsSheet(onDismiss = store.stores::clearInvite) {
        val forWhom = shown.forWhom
        if (forWhom != null) {
            SheetTitle(stringResource(R.string.stores_invite_for_name, forWhom))
            SheetText(inviteReadBack(context, shown.max.toInt(), shown.until))
        } else {
            SheetTitle(stringResource(R.string.stores_link_device_title))
            SheetText(stringResource(R.string.stores_link_sheet_detail))
        }
        Text(
            shown.link,
            style = MaterialTheme.typography.bodyMedium.copy(fontFamily = FontFamily.Monospace),
            modifier = Modifier.padding(top = 16.dp),
        )
        SheetButtons {
            QuietButton(stringResource(R.string.stores_copy_link)) { clipboard.setText(AnnotatedString(shown.link)) }
            Button(onClick = {
                val send = Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_TEXT, shown.link)
                context.startActivity(Intent.createChooser(send, null))
            }) { Text(stringResource(R.string.action_share)) }
        }
    }
}

/**
 * The store form over any screen, outside first run: opened empty from Plugin stores, or with an
 * invite link the app was handed. A dialog on a phone and a tablet alike.
 */
@Composable
internal fun JoinStoreSheetHost() {
    val store = LocalAppStore.current
    val joining by store.stores.joining.collectAsStateWithLifecycle()
    val state = app.kaeri.ui.reading.rememberCameInState()
    // A link to join opens the form with it, once.
    LaunchedEffect(joining) {
        val link = joining ?: return@LaunchedEffect
        if (store.cameIn.open.value == null) store.cameIn.open(app.kaeri.store.Adding.STORE)
        if (store.cameIn.open.value == app.kaeri.store.Adding.STORE && link.isNotEmpty()) store.cameIn.typeAddress(link)
    }
    // The store's own page is not opened on a join: the form says what came in.
    val joined by store.stores.joined.collectAsStateWithLifecycle()
    LaunchedEffect(joined) { if (joined != null) store.stores.clearJoined() }
    app.kaeri.ui.reading.CameInDialog(open = state.open == app.kaeri.store.Adding.STORE && !state.inPage) {
        storeForm(state) {
            store.stores.closeJoin()
            store.cameIn.close()
        }
    }
}
