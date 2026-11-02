// Providers (AndroidProviders): the sources in the order failover tries them, each with the one
// quiet line saying where it came from, and its state only when it has a problem, an update or is
// off. A switch turns a plugin off without removing it. Reorder in the top bar swaps the switches
// for handles, and a source dragged by its handle moves through the order with a tick at each
// place it passes. A source's page says its version, response time and the hosts it may contact.

package app.kaeri.ui

import app.kaeri.store.text
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.foundation.selection.toggleable
import androidx.compose.material3.minimumInteractiveComponentSize
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.zIndex
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.navigation.NavGraphBuilder
import androidx.navigation.compose.composable
import kotlin.math.roundToInt
import app.kaeri.design.tabular
import app.kaeri.engine.core.SettingValue
import app.kaeri.engine.model.ProviderOrigin
import app.kaeri.engine.model.ProviderRow
import app.kaeri.engine.model.SettingKey
import app.kaeri.store.ProviderStanding
import app.kaeri.store.SourceOrder
import app.kaeri.store.standing
import app.kaeri.store.hasNoSources
import app.kaeri.store.sourceGroups
import app.kaeri.i18n.R
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.platform.LocalContext
import app.kaeri.i18n.Strings
import android.content.Context
import app.kaeri.store.firstClause

internal const val PROVIDERS_ROUTE = "providers"

internal fun providerRoute(id: String) = "provider/${android.net.Uri.encode(id)}"

/** The saved server a source row stands for: Jellyfin's rows are named `jellyfin-<server id>`. */
internal fun serverIdOf(row: ProviderRow): String? = row.id.takeIf { row.kind == "server" }?.removePrefix("jellyfin-")

/** Providers' routes: the order, and a source's page. */
internal fun NavGraphBuilder.providerPages(actions: AppActions) {
    composable(PROVIDERS_ROUTE) { ProvidersScreen() }
    serverPages()
    licensedPages()
    composable("provider/{id}") { entry ->
        val id = entry.arguments?.getString("id") ?: return@composable
        ProviderScreen(id, actions)
    }
}

/** What Providers says of a source: its name, and the quiet line under it. */
internal data class ProviderInfo(val row: ProviderRow, val context: Context, val line: String? = null) {
    val isServer get() = row.kind == "server"
    val isPlugin get() = row.kind == "plugin"
    val isDemo get() = row.isDemo

    val name: String
        get() = if (row.isDemo) Strings.sourcesDemoName(context) else row.name.ifEmpty { row.id }

    /** "Hotaru Plugins", "Added by hand", "Built in". */
    val origin: String
        get() = line ?: if (row.isDemo) Strings.sourcesDemoLine(context) else when (val origin = row.origin) {
            is ProviderOrigin.Store -> origin.name ?: Strings.sourcesOriginLeftStore(context)
            ProviderOrigin.ByHand -> Strings.sourcesOriginByHand(context)
            ProviderOrigin.BuiltIn -> Strings.sourcesOriginBuiltIn(context)
        }

    /** The quiet line under a source's name: that it plays through torrent playback, or its origin. */
    val detail: String get() = if (row.torrents) Strings.sourcesPlaysTorrents(context) else origin

    /**
     * The state beside the origin: only for a problem, an update or off. Offline, no source is
     * said to be failing: the one offline line says why nothing answers.
     */
    fun state(offline: Boolean): Pair<String, StateKind>? = when {
            row.disabled -> Strings.valueOff(context) to StateKind.Off
            row.standing == ProviderStanding.Failing && !offline -> Strings.sourcesStatusNotAnswering(context) to StateKind.Problem
            row.standing == ProviderStanding.HeldBack -> Strings.sourcesStatusHeldBack(context) to StateKind.Off
            row.update != null -> Strings.sourcesStatusUpdate(context) to StateKind.Update
            else -> null
        }

    enum class StateKind { Off, Problem, Update }
}

@Composable
internal fun ProvidersScreen() {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val view by store.settings.view.collectAsStateWithLifecycle()
    val rows by store.providers.rows.collectAsStateWithLifecycle()
    val loaded by store.providers.loaded.collectAsStateWithLifecycle()
    val note by store.providers.note.collectAsStateWithLifecycle()
    val offline by store.offline.collectAsStateWithLifecycle()
    var editing by rememberSaveable { mutableStateOf(false) }
    LaunchedEffect(Unit) {
        store.checkProviders()
        store.loadStores()
    }
    val order = view?.sources?.order.orEmpty()
    // The demo is always last, and not part of the order.
    val sorted = SourceOrder.sorted(rows, order).sortedBy { it.isDemo }
    val demo by store.demo.collectAsStateWithLifecycle()
    val servers by store.servers.rows.collectAsStateWithLifecycle()
    val licensed by store.licensed.view.collectAsStateWithLifecycle()
    var adding by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) {
        store.loadServers()
        store.loadLicensed()
    }
    ScreenFrame(
        title = stringResource(R.string.sources_title),
        onBack = navigator::back,
        actions = {
            IconButton(onClick = store.opened::openPaste) {
                Icon(Icons.Add, contentDescription = Strings.openedOpen(androidx.compose.ui.platform.LocalContext.current))
            }
            if (sorted.count { !it.isDemo } > 1) {
                IconButton(onClick = { editing = !editing }) {
                    Icon(if (editing) Icons.Check else Icons.Reorder, contentDescription = stringResource(if (editing) R.string.action_done else R.string.sources_reorder))
                }
            }
            // The one secondary Add: nothing on a full page is the one thing to do, so it is not amber.
            Box {
                androidx.compose.material3.TextButton(
                    onClick = { adding = true },
                    colors = androidx.compose.material3.ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.onSurface),
                ) { Text(stringResource(R.string.sources_add)) }
                androidx.compose.material3.DropdownMenu(
                    expanded = adding,
                    onDismissRequest = { adding = false },
                    shape = MaterialTheme.shapes.extraSmall,
                    containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
                ) {
                    androidx.compose.material3.DropdownMenuItem(
                        text = { Text(stringResource(R.string.sources_add_server)) },
                        onClick = {
                            adding = false
                            navigator.open(ADD_SERVER_ROUTE)
                        },
                    )
                    androidx.compose.material3.DropdownMenuItem(
                        text = { Text(stringResource(R.string.sources_add_store)) },
                        onClick = {
                            adding = false
                            store.stores.openJoin()
                        },
                    )
                    // The demo, while it can be added back: it was removed, or never added.
                    if (demo.offer) {
                        androidx.compose.material3.DropdownMenuItem(
                            text = { Text(stringResource(R.string.sources_demo_add)) },
                            onClick = {
                                adding = false
                                store.addDemo()
                            },
                        )
                    }
                }
            }
        },
    ) { padding ->
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState(), enabled = true)
                .padding(top = padding.calculateTopPadding() + 8.dp, bottom = 48.dp)
                .padding(horizontal = 16.dp)
                .readingWidth(),
        ) {
            when {
                !loaded -> PlaceholderRows()
                else -> {
                    // Nothing but the built-in sources: the amber is a media server, the plugin store one step away.
                    if (sorted.hasNoSources) {
                        EmptyState(
                            title = stringResource(R.string.sources_empty_title),
                            // Why there are none, when the engine said: a store that is down, a plugin it could not read.
                            line = note?.text(LocalContext.current) ?: stringResource(R.string.sources_empty_detail),
                            action = stringResource(R.string.sources_add_server_button),
                            onAction = { navigator.open(ADD_SERVER_ROUTE) },
                            secondary = stringResource(R.string.stores_join_action),
                            onSecondary = { store.stores.openJoin() },
                        )
                    }
                    if (editing) {
                        SourceList(
                            sorted,
                            editing,
                            offline,
                        ) { id, target ->
                            // The demo stays last, so nothing is placed after it.
                            val last = sorted.count { !it.isDemo } - 1
                            SourceOrder.movedTo(order, sorted, id, target.coerceAtMost(last))?.let { store.setSetting(SettingKey.SOURCE_ORDER, SettingValue.Strings(it)) }
                        }
                    } else {
                        SourceSections(
                            sorted,
                            servers,
                            licensedLine = licensedRowLine(licensed),
                            offline = offline,
                        )
                    }
                    SettingsFootnote(
                        stringResource(if (editing) R.string.sources_drag_hint else R.string.sources_order_detail),
                    )
                }
            }
        }
    }
}

/**
 * The page at rest: a section per kind of source, as the Your sources board draws it. Media
 * servers open their own page, Licensed services is one row for all of them, and each store's
 * plugins keep their switches. Reordering is the top bar's, where every source is one list.
 */
@Composable
private fun SourceSections(
    rows: List<ProviderRow>,
    servers: List<app.kaeri.engine.model.ServerRow>,
    licensedLine: String,
    offline: Boolean,
) {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val context = LocalContext.current
    val groups = sourceGroups(rows)
    var licensedShown = false
    @Composable
    fun Licensed() {
        licensedShown = true
        SettingsHeader(stringResource(R.string.sources_section_licensed))
        LinkRow(Place.Only, stringResource(R.string.licensed_title), licensedLine, onClick = { navigator.open(LICENSED_ROUTE) })
    }
    // Saved servers the sources list has no row for yet still show, so a server is never missing here.
    val unlisted = servers.filter { server -> rows.none { serverIdOf(it) == server.id } }
    @Composable
    fun Unlisted() {
        SettingsHeader(stringResource(R.string.sources_section_servers))
        Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
            unlisted.forEachIndexed { index, server ->
                LinkRow(Place.of(index, unlisted.size), server.name, serverRowLine(server), onClick = { navigator.open(serverRoute(server.id)) })
            }
        }
    }
    var unlistedShown = unlisted.isEmpty() || groups.any { it.section == app.kaeri.engine.model.SourceSection.MEDIA_SERVERS }
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        for (group in groups) {
            if (!unlistedShown && group.section.ordinal > app.kaeri.engine.model.SourceSection.MEDIA_SERVERS.ordinal) {
                unlistedShown = true
                Unlisted()
            }
            // Licensed services follows Media servers and comes before the catalogues and plugins.
            if (!licensedShown && group.section.ordinal >= app.kaeri.engine.model.SourceSection.FREE_CATALOGUES.ordinal) Licensed()
            val first = ProviderInfo(group.rows.first(), context)
            SettingsHeader(
                when (group.section) {
                    app.kaeri.engine.model.SourceSection.YOUR_MEDIA -> stringResource(R.string.sources_section_media)
                    app.kaeri.engine.model.SourceSection.MEDIA_SERVERS -> stringResource(R.string.sources_section_servers)
                    app.kaeri.engine.model.SourceSection.LICENSED_SERVICES -> stringResource(R.string.sources_section_licensed)
                    app.kaeri.engine.model.SourceSection.FREE_CATALOGUES -> stringResource(R.string.sources_section_catalogues)
                    app.kaeri.engine.model.SourceSection.PLUGINS -> stringResource(R.string.sources_section_store, first.origin)
                    app.kaeri.engine.model.SourceSection.BUILT_IN -> stringResource(R.string.sources_section_builtin)
                },
            )
            Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                group.rows.forEachIndexed { index, row ->
                    val server = serverIdOf(row)?.let { id -> servers.firstOrNull { it.id == id } }
                    val info = ProviderInfo(row, context, line = server?.let { serverRowLine(it) })
                    ProviderListRow(
                        Place.of(index, group.rows.size),
                        info,
                        editing = false,
                        offline = offline,
                        lifted = false,
                        onOpen = {
                            when {
                                info.isServer && server != null -> navigator.open(serverRoute(server.id))
                                else -> navigator.open(providerRoute(row.id))
                            }
                        },
                        switchOn = !row.disabled,
                        onToggle = { on -> store.disablePlugin(row.id, disabled = !on) },
                        handle = Modifier,
                    )
                }
            }
        }
        if (!unlistedShown) Unlisted()
        if (!licensedShown) Licensed()
    }
}

/**
 * The sources, one group. While [editing], each row carries a handle, and a row dragged by it
 * follows the finger, the others making way at each place it passes, with a tick each time; let
 * go, the order is written. Each row also offers Move up and Move down to TalkBack.
 */
@Composable
private fun SourceList(
    rows: List<ProviderRow>,
    editing: Boolean,
    offline: Boolean,
    move: (id: String, target: Int) -> Unit,
) {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val haptics = LocalHapticFeedback.current
    val heights = remember { mutableStateMapOf<String, Int>() }
    var dragging by remember { mutableStateOf<String?>(null) }
    var offset by remember { mutableFloatStateOf(0f) }
    var target by remember { mutableStateOf(-1) }
    val gap = with(androidx.compose.ui.platform.LocalDensity.current) { 2.dp.toPx() }
    val moveUp = stringResource(R.string.sources_move_up)
    val moveDown = stringResource(R.string.sources_move_down)

    Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
        rows.forEachIndexed { index, row ->
            val info = ProviderInfo(row, LocalContext.current)
            val from = rows.indexOfFirst { it.id == dragging }
            // Where this row sits while another is dragged past it: one row's height up or down.
            val shift = when {
                dragging == null || dragging == row.id || from < 0 || target < 0 -> 0f
                from < target && index in (from + 1)..target -> -((heights[dragging] ?: 0) + gap)
                from > target && index in target until from -> (heights[dragging] ?: 0) + gap
                else -> 0f
            }
            // Read where it is placed, not here, so making way moves the row and composes nothing.
            val shiftPx = animateFloatAsState(shift, animationSpec = app.kaeri.design.Motion.standard(), label = "shift") // i18n-skip
            val lifted = dragging == row.id
            Box(
                Modifier
                    .zIndex(if (lifted) 1f else 0f)
                    .onSizeChanged { heights[row.id] = it.height }
                    .graphicsLayer { translationY = if (lifted) offset else 0f }
                    .offset { IntOffset(0, if (lifted) 0 else shiftPx.value.roundToInt()) }
                    .semantics {
                        customActions = listOfNotNull(
                            if (index > 0) CustomAccessibilityAction(moveUp) { move(row.id, index - 1); true } else null,
                            if (index < rows.lastIndex) CustomAccessibilityAction(moveDown) { move(row.id, index + 1); true } else null,
                        )
                    },
            ) {
                ProviderListRow(
                    Place.of(index, rows.size),
                    info,
                    editing = editing,
                    offline = offline,
                    lifted = lifted,
                    onOpen = { navigator.open(providerRoute(row.id)) },
                    switchOn = !row.disabled,
                    onToggle = { on -> store.disablePlugin(row.id, disabled = !on) },
                    handle = Modifier.pointerInput(row.id, rows) {
                        detectDragGestures(
                            onDragStart = {
                                dragging = row.id
                                offset = 0f
                                target = index
                                haptics.performHapticFeedback(HapticFeedbackType.SegmentFrequentTick)
                            },
                            onDrag = { change, amount ->
                                change.consume()
                                offset += amount.y
                                val next = targetFor(rows, index, offset, heights, gap)
                                if (next != target) {
                                    target = next
                                    haptics.performHapticFeedback(HapticFeedbackType.SegmentFrequentTick)
                                }
                            },
                            onDragEnd = {
                                val to = target
                                dragging = null
                                offset = 0f
                                target = -1
                                if (to >= 0 && to != index) move(row.id, to)
                            },
                            onDragCancel = {
                                dragging = null
                                offset = 0f
                                target = -1
                            },
                        )
                    },
                )
            }
        }
    }
}

/** The place a row dragged [offset] from [index] would land on, by the rows' heights. */
private fun targetFor(rows: List<ProviderRow>, index: Int, offset: Float, heights: Map<String, Int>, gap: Float): Int {
    var at = index
    var left = offset
    if (left > 0) {
        while (at < rows.lastIndex) {
            val next = (heights[rows[at + 1].id] ?: 0) + gap
            if (left < next / 2) break
            left -= next
            at++
        }
    } else {
        while (at > 0) {
            val previous = (heights[rows[at - 1].id] ?: 0) + gap
            if (-left < previous / 2) break
            left += previous
            at--
        }
    }
    return at
}

/** The demo row's trailing menu: Remove demo. It fades out and the engine's notice offers Undo. */
@Composable
private fun DemoRowMenu() {
    val store = LocalAppStore.current
    var open by remember { mutableStateOf(false) }
    Box {
        IconButton(onClick = { open = true }) {
            Icon(Icons.More, contentDescription = stringResource(R.string.action_more))
        }
        androidx.compose.material3.DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            androidx.compose.material3.DropdownMenuItem(
                text = { Text(stringResource(R.string.sources_demo_remove)) },
                onClick = {
                    open = false
                    store.removeDemo()
                },
            )
        }
    }
}

/** One source: its name, its origin and state, and the switch, or the handle while reordering. */
@Composable
private fun ProviderListRow(
    place: Place,
    info: ProviderInfo,
    editing: Boolean,
    offline: Boolean,
    lifted: Boolean,
    switchOn: Boolean,
    onOpen: () -> Unit,
    onToggle: (Boolean) -> Unit,
    handle: Modifier,
) {
    val colors = MaterialTheme.colorScheme
    val on = !info.row.disabled
    val moveLabel = stringResource(R.string.sources_move, info.name)
    val useLabel = stringResource(R.string.sources_use, info.name)
    // Lifted by its handle, the row rises a container role, not onto a shadow.
    RowFrame(
        place,
        onClick = if (editing) null else onOpen,
        modifier = Modifier.heightIn(min = 72.dp),
        container = if (lifted) colors.surfaceContainerHigh else null,
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            Column(Modifier.weight(1f)) {
                Text(info.name, style = MaterialTheme.typography.titleMedium, color = if (on) colors.onSurface else colors.onSurfaceVariant)
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text(info.detail, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant)
                    info.state(offline)?.let { (text, kind) ->
                        StateDot(
                            when (kind) {
                                ProviderInfo.StateKind.Problem -> colors.error
                                ProviderInfo.StateKind.Update -> colors.secondary
                                ProviderInfo.StateKind.Off -> null
                            },
                        )
                        Text(
                            text,
                            style = MaterialTheme.typography.bodyMedium,
                            color = if (kind == ProviderInfo.StateKind.Off) colors.onSurfaceVariant else colors.onSurface,
                        )
                    }
                }
            }
            if (info.isDemo) {
                DemoRowMenu()
            } else if (editing) {
                Box(
                    handle
                        .size(48.dp)
                        .semantics { contentDescription = moveLabel },
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(Icons.DragHandle, contentDescription = null, tint = colors.onSurfaceVariant)
                }
            } else if (info.isPlugin) {
                // One toggle named "Use ...": a Switch is a node of its own, so the name sits on the
                // toggle around it and the Switch only draws.
                Box(
                    Modifier
                        .minimumInteractiveComponentSize()
                        .toggleable(value = switchOn, role = Role.Switch, onValueChange = onToggle)
                        .semantics { contentDescription = useLabel },
                ) {
                    androidx.compose.material3.Switch(
                        checked = switchOn,
                        onCheckedChange = null,
                        colors = androidx.compose.material3.SwitchDefaults.colors(
                            checkedTrackColor = colors.primary,
                            checkedThumbColor = colors.onPrimary,
                            checkedBorderColor = androidx.compose.ui.graphics.Color.Transparent,
                            uncheckedTrackColor = colors.surfaceContainerHighest,
                            uncheckedThumbColor = colors.onSurfaceVariant,
                            uncheckedBorderColor = colors.onSurfaceVariant,
                        ),
                    )
                }
            }
        }
    }
}

/**
 * A source's page: its name and where it came from, its state, what can be done with it, and its
 * version, response time and the hosts it may contact.
 */
@Composable
internal fun ProviderScreen(id: String, actions: AppActions) {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val rows by store.providers.rows.collectAsStateWithLifecycle()
    val row = rows.firstOrNull { it.id == id }
    val offline by store.offline.collectAsStateWithLifecycle()
    var checking by remember { mutableStateOf(false) }
    // Check again works until the engine says the providers again, or for 15 s at most.
    LaunchedEffect(rows) { checking = false }
    LaunchedEffect(checking) {
        if (checking) {
            kotlinx.coroutines.delay(15_000)
            checking = false
        }
    }
    var removing by remember { mutableStateOf(false) }
    var updating by remember { mutableStateOf(false) }
    val context = LocalContext.current
    val info = row?.let { ProviderInfo(it, context) }
    if (info?.isDemo == true) {
        DemoAbout(info.name)
        return
    }
    ScreenFrame(title = info?.name ?: stringResource(R.string.sources_fallback_title), subtitle = info?.let { if (it.isPlugin && it.row.origin is ProviderOrigin.Store) stringResource(R.string.sources_origin_from, it.origin) else it.origin }, onBack = navigator::back) { padding ->
        if (info == null) return@ScreenFrame
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(top = padding.calculateTopPadding() + 8.dp, bottom = 48.dp)
                .padding(horizontal = 16.dp)
                .readingWidth(),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            stateLine(LocalContext.current, info, offline)?.let { (text, problem) ->
                Row(Modifier.padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                    StateDot(if (problem) MaterialTheme.colorScheme.error else null)
                    Text(text, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
            Row(Modifier.padding(horizontal = 16.dp, vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                info.row.update?.let { update ->
                    Button(onClick = { updating = true }) { Text(stringResource(R.string.updates_update_to, update.version)) }
                }
                val silk = androidx.compose.material3.ButtonDefaults.outlinedButtonColors(contentColor = MaterialTheme.colorScheme.onSurface)
                if (info.row.standing == ProviderStanding.Failing && !offline) {
                    OutlinedButton(
                        onClick = {
                            checking = true
                            store.checkProviders()
                        },
                        enabled = !checking,
                        colors = silk,
                    ) {
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                            WorkRing(checking)
                            Text(stringResource(R.string.action_check_again))
                        }
                    }
                }
                if (info.isPlugin) OutlinedButton(onClick = { removing = true }, colors = silk) { Text(stringResource(R.string.action_remove)) }
            }
            if (info.isPlugin) {
                SettingsGroup(
                    footer = stringResource(R.string.stores_plugin_limits),
                    rows = listOf(
                        { place -> SwitchRow(place, stringResource(R.string.sources_use, info.name), checked = !info.row.disabled) { store.disablePlugin(info.row.id, disabled = !it) } },
                        { place ->
                            InfoRow(
                                place,
                                stringResource(R.string.sources_version),
                                listOfNotNull(info.row.version ?: stringResource(R.string.sources_origin_no_store), info.row.update?.let { stringResource(R.string.sources_version_available, it.version) }).joinToString(" · "), // i18n-skip
                            )
                        },
                        { place ->
                            InfoRow(
                                place,
                                stringResource(R.string.sources_response_time),
                                when {
                                    info.row.standing == ProviderStanding.Failing && !offline -> stringResource(R.string.sources_no_answer)
                                    info.row.latencyMs != null -> stringResource(R.string.sources_response_ms, info.row.latencyMs!!.toInt())
                                    else -> stringResource(R.string.sources_not_checked)
                                },
                            )
                        },
                        { place ->
                            InfoRow(
                                place,
                                stringResource(R.string.sources_contacts_label),
                                info.row.hosts.ifEmpty { listOf(stringResource(R.string.sources_no_hosts)) }.joinToString("\n"), // i18n-skip
                            )
                        },
                    ),
                )
            }
        }
    }
    if (removing && info != null) {
        AlertDialog(
            onDismissRequest = { removing = false },
            properties = AskingDialog,
            title = { Text(stringResource(R.string.stores_remove_plugin_title, info.name)) },
            text = { Text(stringResource(R.string.stores_remove_plugin_detail)) },
            confirmButton = {
                QuietButton(stringResource(R.string.action_remove)) {
                    removing = false
                    store.removePlugin(info.row.id)
                    navigator.back()
                }
            },
            dismissButton = { QuietButton(stringResource(R.string.action_cancel)) { removing = false } },
            containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
        )
    }
    if (updating && info != null) {
        val update = info.row.update
        val storeId = (info.row.origin as? ProviderOrigin.Store)?.id
        if (update != null && storeId != null) {
            UpdateDialog(info.name, storeName = info.origin, from = info.row.version, to = update.version, newHosts = update.newHosts, onDismiss = { updating = false }) {
                updating = false
                store.installPlugin(storeId, info.row.id)
            }
        }
    }
}

/** The page's state line. The engine's error goes to Messages whole; the page says it plainly. */
private fun stateLine(context: Context, info: ProviderInfo, offline: Boolean): Pair<String, Boolean>? = when {
    info.row.disabled -> Strings.sourcesDetailOff(context) to false
    info.row.standing == ProviderStanding.Failing && offline -> null
    info.row.standing == ProviderStanding.Failing ->
        (info.row.lastError?.let { Strings.sourcesDetailNotAnswering(context, providerReason(context, it)) } ?: Strings.sourcesDetailNotAnsweringPlain(context)) to true
    info.row.standing == ProviderStanding.HeldBack -> Strings.sourcesDetailHeldBack(context) to false
    else -> null
}

/**
 * The update confirmation: which version replaces which, and the hosts the update adds, named
 * before anything is installed. Update is the one amber action.
 */
@Composable
internal fun UpdateDialog(
    plugin: String,
    storeName: String,
    from: String?,
    to: String,
    newHosts: List<String>,
    onDismiss: () -> Unit,
    onUpdate: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        properties = AskingDialog,
        title = { Text(stringResource(R.string.sources_update_title, plugin)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(if (from != null) stringResource(R.string.sources_update_detail, to, storeName, from) else stringResource(R.string.sources_update_from, to, storeName))
                if (newHosts.isNotEmpty()) {
                    Text(if (newHosts.size == 1) stringResource(R.string.sources_update_contacts_one, newHosts.first()) else pluralStringResource(R.plurals.sources_update_contacts, newHosts.size - 1, newHosts.first(), newHosts.size - 1))
                }
            }
        },
        confirmButton = { Button(onClick = onUpdate) { Text(stringResource(R.string.action_update)) } },
        dismissButton = { QuietButton(stringResource(R.string.action_cancel), onClick = onDismiss) },
        containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
    )
}

/** A source's error in plain words: what kind of failure it was, never the request chain. */
internal fun providerReason(context: Context, raw: String): String {
    val lower = raw.lowercase(java.util.Locale.ROOT)
    return when {
        "timed out" in lower || "timeout" in lower -> Strings.reasonSourceTimeout(context)
        "429" in lower || "too many requests" in lower -> Strings.reasonSourceBusy(context)
        Regex("\\b5\\d\\d\\b").containsMatchIn(lower) -> Strings.reasonSourceProblem(context)
        Regex("\\b40[34]\\b").containsMatchIn(lower) -> Strings.reasonSourceTurnedAway(context)
        "dns" in lower || "resolve" in lower -> Strings.reasonSourceNoAddress(context)
        "error sending request" in lower || "connect" in lower -> Strings.reasonSourceUnreachable(context)
        else -> firstClause(context, raw)
    }
}
