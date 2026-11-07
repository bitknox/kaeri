// First launch (spec, First launch): short pages over the app before it opens on Continue.
// Welcome says what Kaeri is; where this build can read, the next page asks what Kaeri is for
// (Watch, Read or Both, nothing chosen until a row is); AniList offers the sign-in, worded for that
// choice; the last page says where shows or books come from: an invite link to a plugin store with
// the own-risk line, and for books a folder or a server. Every page can be passed over, each step
// is in Settings afterwards, and the pages never mention torrents. Once they close they do not come
// back; a device that already has an account or a store never sees them.

package app.kaeri.ui

import app.kaeri.i18n.R
import androidx.compose.ui.res.stringResource
import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import app.kaeri.i18n.Strings
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.semantics.Role
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.kaeri.store.copyAllSignIns
import app.kaeri.store.copyNoSignIns
import app.kaeri.store.joinStoresFrom
import app.kaeri.store.pairedDevice
import app.kaeri.store.signInNames
import app.kaeri.store.storeOffersFrom
import app.kaeri.store.pairInvite
import app.kaeri.store.pairStop
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.delay
import app.kaeri.design.Motion
import app.kaeri.design.Nagare
import app.kaeri.store.AppStore
import app.kaeri.store.DevicePreferences
import app.kaeri.store.InviteLinks
import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.engine.model.MediumAsk
import app.kaeri.ui.reading.AddBookServerSheet
import app.kaeri.ui.reading.MediumChoiceRows
import app.kaeri.ui.reading.rememberFolderPicker
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.background
import androidx.compose.ui.draw.clip

/** The pages, in order. */
internal enum class FirstLaunchPage { Welcome, Medium, AniList, Sources, SetUp }

/** The preference that says the pages have been through, on this device. */
internal const val FIRST_LAUNCH_DONE = "firstLaunchDone"

/** How long the pages wait for the engine to say the accounts and stores before deciding. */
private const val DECIDE_MILLIS = 3_000L

/**
 * The pages over the app, while this device has not been through them. Until the engine has said
 * whether anything is set up, the ground shows plain, so the app does not flash under them.
 * [onCover] hears whether they cover the app, which then keeps its own elements from TalkBack.
 */
@Composable
internal fun FirstLaunch(onCover: (Boolean) -> Unit, start: FirstLaunchPage = FirstLaunchPage.Welcome) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val preferences = remember { DevicePreferences.of(context) }
    var done by rememberSaveable { mutableStateOf(preferences.getBoolean(FIRST_LAUNCH_DONE, false)) }
    val phase by store.phase.collectAsStateWithLifecycle()
    // An engine that did not start says so on its own screen; the pages wait for the next launch.
    val stopped = phase is AppStore.Phase.Failed || phase is AppStore.Phase.Stopped
    // The pages and the answer chosen live here, so they wait while a server is added under them.
    val page = rememberSaveable { mutableStateOf(start) }
    val chosen = rememberSaveable { mutableStateOf<KaeriIsFor?>(null) }
    var away by rememberSaveable { mutableStateOf(false) }
    val serverFlow by store.servers.flow.collectAsStateWithLifecycle()
    var serverOpened by remember { mutableStateOf(false) }
    LaunchedEffect(away, serverFlow != null) {
        if (!away) return@LaunchedEffect
        // Add a server opened, then closed: the pages come back where they were.
        if (serverFlow != null) serverOpened = true else if (serverOpened) {
            serverOpened = false
            away = false
        }
    }
    val covering = !done && !stopped && !away
    LaunchedEffect(covering) { onCover(covering) }
    if (!covering) return
    val finish = {
        preferences.edit().putBoolean(FIRST_LAUNCH_DONE, true).apply()
        done = true
    }
    val trackers by store.accounts.trackers.collectAsStateWithLifecycle()
    val stores by store.stores.stores.collectAsStateWithLifecycle()
    val storesLoaded by store.stores.loaded.collectAsStateWithLifecycle()
    var waited by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) {
        store.loadStores()
        delay(DECIDE_MILLIS)
        waited = true
    }
    val setUp = trackers.values.any { it.connected } || stores.isNotEmpty()
    var showing by rememberSaveable { mutableStateOf(false) }
    // The stores said means the engine is up and has read what it keeps.
    val known = storesLoaded
    LaunchedEffect(known, waited, setUp) {
        when {
            showing -> {}
            // Set up already, as after an update: the pages have nothing to offer.
            setUp -> finish()
            known || waited -> showing = true
        }
    }
    Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.surface) {
        if (showing) FirstLaunchPages(page, chosen, finish, addShowServer = { away = true })
    }
}

@Composable
private fun FirstLaunchPages(
    pageState: androidx.compose.runtime.MutableState<FirstLaunchPage>,
    chosenState: androidx.compose.runtime.MutableState<KaeriIsFor?>,
    finish: () -> Unit,
    addShowServer: () -> Unit,
) {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    var page by pageState
    var chosen by chosenState
    // Asked only where this build can read and the answer is not fixed; otherwise three pages.
    val medium by store.medium.view.collectAsStateWithLifecycle()
    val choosable = medium?.let { it.readingAvailable && !it.fixed } == true
    val answer = chosen ?: medium?.answer ?: KaeriIsFor.WATCH
    val trackers by store.accounts.trackers.collectAsStateWithLifecycle()
    val stores by store.stores.stores.collectAsStateWithLifecycle()
    val joining by store.stores.joining.collectAsStateWithLifecycle()
    // Signed in, the next step; a store joined, the app; an invite link opened, its step.
    val anilist = trackers["anilist"]?.connected == true
    LaunchedEffect(anilist) { if (anilist && page == FirstLaunchPage.AniList) page = FirstLaunchPage.Sources }
    LaunchedEffect(stores.isNotEmpty()) { if (stores.isNotEmpty() && page == FirstLaunchPage.Sources) finish() }
    LaunchedEffect(joining) { if (!joining.isNullOrEmpty()) page = FirstLaunchPage.Sources }
    BackHandler(enabled = page != FirstLaunchPage.Welcome) {
        if (page == FirstLaunchPage.SetUp) {
            store.pairStop()
            store.sync.first(false)
            page = FirstLaunchPage.Welcome
        } else {
            page = when (page) {
                FirstLaunchPage.AniList -> if (choosable) FirstLaunchPage.Medium else FirstLaunchPage.Welcome
                else -> FirstLaunchPage.entries[page.ordinal - 1]
            }
        }
    }
    // A pairing that finished from here was read on its success page: the pages are through.
    val copied by store.sync.copied.collectAsStateWithLifecycle()
    val first by store.sync.first.collectAsStateWithLifecycle()
    LaunchedEffect(copied, first, page) { if ((copied != null || first) && page != FirstLaunchPage.SetUp) page = FirstLaunchPage.SetUp }
    AnimatedContent(
        page,
        transitionSpec = { fadeIn(Motion.fadeIn()) togetherWith fadeOut(Motion.fadeOut()) },
        label = "first launch", // i18n-skip
    ) { shown ->
        when (shown) {
            FirstLaunchPage.Welcome -> WelcomePage(
                four = choosable,
                next = { page = if (choosable) FirstLaunchPage.Medium else FirstLaunchPage.AniList },
                setUp = { page = FirstLaunchPage.SetUp },
            )
            FirstLaunchPage.Medium -> MediumPage(
                chosen = chosen,
                choose = { chosen = it },
                back = { page = FirstLaunchPage.Welcome },
                next = {
                    chosen?.let { picked ->
                        val ask = store.medium.asking.value
                        if (ask is MediumAsk.FirstRun) store.medium.answer(ask, picked) else store.medium.choose(picked)
                        page = FirstLaunchPage.AniList
                    }
                },
            )
            FirstLaunchPage.AniList -> if (choosable) {
                ListsPage(
                    answer = answer,
                    signIn = { store.connect("anilist") },
                    back = { page = FirstLaunchPage.Medium },
                    notNow = { page = FirstLaunchPage.Sources },
                    skip = finish,
                )
            } else {
                AniListPage(
                    signIn = { store.connect("anilist") },
                    notNow = { page = FirstLaunchPage.Sources },
                )
            }
            FirstLaunchPage.SetUp -> SetUpPage(done = finish)
            FirstLaunchPage.Sources -> when {
                choosable && answer == KaeriIsFor.READ -> BooksPage(
                    join = { link -> store.stores.openJoin(link) },
                    back = { page = FirstLaunchPage.AniList },
                    done = finish,
                )
                choosable && answer == KaeriIsFor.BOTH -> BooksPage(
                    join = { link -> store.stores.openJoin(link) },
                    back = { page = FirstLaunchPage.AniList },
                    done = finish,
                    showServer = {
                        addShowServer()
                        navigator.open(ADD_SERVER_ROUTE)
                    },
                )
                else -> SourcesPage(
                    step = if (choosable) 4 else null,
                    join = { link -> store.stores.openJoin(link) },
                    skip = finish,
                    playDemo = {
                        store.playDemo()
                        finish()
                    },
                )
            }
        }
    }
}

/**
 * Nagare, the name, what Kaeri is, and Continue. With four pages, the first of them: Welcome to
 * Kaeri, keeping track of what is watched and read, and that lists and progress stay the person's.
 */
@Composable
private fun WelcomePage(four: Boolean, next: () -> Unit, setUp: () -> Unit) {
    val step: (@Composable () -> Unit)? = if (four) ({ StepLabel(1) }) else null
    PageFrame(
        top = step,
        buttons = {
            Button(onClick = next, modifier = Modifier.fillMaxWidth()) { Text(stringResource(R.string.onboarding_continue)) }
            TextButton(
                onClick = setUp,
                modifier = Modifier.fillMaxWidth(),
                colors = ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.onSurface),
            ) { Text(stringResource(R.string.first_setup_other)) }
        },
    ) {
        Nagare(cell = 5, modifier = Modifier.align(Alignment.CenterHorizontally))
        Text(
            if (four) stringResource(R.string.onboarding_welcome_to) else "Kaeri", // i18n-skip
            style = MaterialTheme.typography.headlineMedium,
            textAlign = TextAlign.Center,
            modifier = Modifier.fillMaxWidth().padding(top = 24.dp).semantics { heading() },
        )
        PageLine(stringResource(if (four) R.string.onboarding_welcome_line else R.string.onboarding_tagline), Modifier.padding(top = 8.dp), center = true)
        if (four) {
            Text(
                stringResource(R.string.onboarding_welcome_note),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = TextAlign.Center,
                modifier = Modifier.fillMaxWidth().padding(top = 16.dp),
            )
        }
    }
}

/**
 * Set up from another device: this device shows the code and its QR, a device that has Kaeri
 * enters or scans it, and the page turns to the success line, whose Continue closes the pages.
 */
@Composable
private fun SetUpPage(done: () -> Unit) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val invite by store.sync.invite.collectAsStateWithLifecycle()
    val copied by store.sync.copied.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) {
        // Decision 11: sign-ins come over from first launch unless the person says otherwise.
        store.sync.first(true)
        if (store.sync.invite.value == null && store.sync.copied.value == null) {
            store.copyAllSignIns()
            store.joinStoresFrom(null)
            store.pairInvite(systemDeviceName(context))
        }
    }
    val shown = copied
    if (shown != null) {
        val status by store.sync.status.collectAsStateWithLifecycle()
        val offers = status.storeOffersFrom(shown.device)
        var off by rememberSaveable { mutableStateOf(setOf<String>()) }
        val paired = status.pairedDevice(shown.device)
        val names = paired?.signInNames().orEmpty()
        var copy by rememberSaveable { mutableStateOf(true) }
        PageFrame(
            buttons = {
                Button(
                    onClick = {
                        if (paired != null && names.isNotEmpty() && !copy) store.copyNoSignIns(paired.id)
                        offers?.let { (device, list) ->
                            val chosen = list.map { it.address }.filter { it !in off }
                            if (chosen.isNotEmpty()) store.joinStoresFrom(device.id, chosen)
                        }
                        store.sync.copiedSeen()
                        store.sync.first(false)
                        done()
                    },
                    modifier = Modifier.fillMaxWidth(),
                ) { Text(stringResource(R.string.onboarding_continue)) }
            },
        ) {
            PageTitle(stringResource(R.string.pair_ok, shown.device))
            PageLine(stringResource(R.string.first_ok_line), Modifier.padding(top = 8.dp))
            if (paired != null && names.isNotEmpty()) {
                JoinStoreSwitch(
                    title = Strings.pairCopySignins(context, paired.name),
                    line = Strings.pairCopySigninsLine(context, names.joinToString(", ")),
                    checked = copy,
                    onChange = { copy = it },
                )
            }
            offers?.let { (device, list) ->
                list.forEach { offer ->
                    JoinStoreSwitch(
                        title = Strings.firstJoinStore(context, offer.name),
                        line = Strings.firstJoinStoreLine(context, device.name, kindWord()),
                        checked = offer.address !in off,
                        onChange = { on -> off = if (on) off - offer.address else off + offer.address },
                    )
                }
            }
        }
        return
    }
    PageFrame(
        buttons = {
            TextButton(
                onClick = {
                    store.pairStop()
                    store.sync.first(false)
                    store.sync.copiedSeen()
                    done()
                },
                modifier = Modifier.fillMaxWidth(),
                colors = ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.onSurface),
            ) { Text(stringResource(R.string.onboarding_not_now)) }
        },
    ) {
        PageTitle(stringResource(R.string.first_setup_other))
        PageLine(stringResource(R.string.first_setup_line), Modifier.padding(top = 8.dp))
        val code = invite
        if (code != null) {
            Text(
                code.code.chunked(3).joinToString(" "), // i18n-skip
                style = MaterialTheme.typography.displaySmall.copy(fontFamily = androidx.compose.ui.text.font.FontFamily.Monospace, letterSpacing = 4.sp),
                modifier = Modifier.padding(top = 24.dp).align(Alignment.CenterHorizontally),
            )
            Box(Modifier.padding(top = 16.dp).align(Alignment.CenterHorizontally)) {
                QrCode("kaeri://pair?port=${code.port}&code=${code.code}", label = stringResource(R.string.pair_show_scan))
            }
        }
    }
}

/** The AniList sign-in, offered and never asked for: Sign in to AniList, or Not now. */
@Composable
private fun AniListPage(signIn: () -> Unit, notNow: () -> Unit) {
    val store = LocalAppStore.current
    val working by store.accounts.working.collectAsStateWithLifecycle()
    val busy = store.accounts.isWorking(working, "anilist")
    PageFrame(
        buttons = {
            Button(onClick = signIn, enabled = !busy, modifier = Modifier.fillMaxWidth()) { WorkLabel(stringResource(R.string.accounts_sign_in_to, "AniList"), busy) }
            TextButton(
                onClick = notNow,
                modifier = Modifier.fillMaxWidth(),
                colors = ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.onSurface),
            ) { Text(stringResource(R.string.onboarding_not_now)) }
        },
    ) {
        PageTitle("AniList") // i18n-skip
        PageLine(stringResource(R.string.accounts_welcome_detail, "AniList"), Modifier.padding(top = 8.dp))
        Text(
            stringResource(R.string.accounts_other_trackers_note),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(top = 16.dp),
        )
    }
}

/**
 * Where episodes come from: sources are plugins from a plugin store, joined by invite, and the
 * demo plays at once. Skip is in the top bar; the demo is the one filled button, at the bottom.
 * The link pasted here opens the join sheet on it, which shows the store before anything is joined.
 */
@Composable
private fun SourcesPage(step: Int?, join: (String) -> Unit, skip: () -> Unit, playDemo: () -> Unit) {
    var link by rememberSaveable { mutableStateOf("") }
    val ready = InviteLinks.looksLikeInvite(link)
    PageFrame(
        top = {
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End, verticalAlignment = Alignment.CenterVertically) {
                step?.let { Box(Modifier.weight(1f)) { StepLabel(it) } }
                TextButton(
                    onClick = skip,
                    colors = ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.onSurface),
                ) { Text(stringResource(R.string.onboarding_skip_for_now)) }
            }
        },
        buttons = { Button(onClick = playDemo, modifier = Modifier.fillMaxWidth()) { Text(stringResource(R.string.onboarding_play_demo)) } },
    ) {
        PageTitle(stringResource(R.string.onboarding_where_title))
        PageLine(stringResource(R.string.onboarding_where_hosts), Modifier.padding(top = 8.dp))
        PageLine(stringResource(R.string.onboarding_where_stores), Modifier.padding(top = 8.dp))
        Text(
            stringResource(R.string.onboarding_demo_heading),
            style = MaterialTheme.typography.titleMedium,
            modifier = Modifier.padding(top = 24.dp).semantics { heading() },
        )
        PageLine(stringResource(R.string.onboarding_demo_detail), Modifier.padding(top = 4.dp))
        OutlinedTextField(
            value = link,
            onValueChange = { link = it },
            label = { Text(stringResource(R.string.onboarding_invite_hint)) },
            singleLine = true,
            trailingIcon = { TextButton(onClick = { join(link.trim()) }, enabled = ready) { Text(stringResource(R.string.onboarding_join)) } },
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, autoCorrectEnabled = false, imeAction = ImeAction.Go),
            keyboardActions = KeyboardActions(onGo = { if (ready) join(link.trim()) }),
            modifier = Modifier.fillMaxWidth().padding(top = 24.dp),
        )
        Text(
            stringResource(R.string.plugin_risk_warning),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(top = 16.dp),
        )
    }
}

/**
 * A page: its content in a column at most 480 wide, centred on the ground and scrolling when the
 * window is short, with its buttons at the foot.
 */
@Composable
private fun PageFrame(buttons: @Composable ColumnScope.() -> Unit, top: (@Composable () -> Unit)? = null, content: @Composable ColumnScope.() -> Unit) {
    Box(
        Modifier
            .fillMaxSize()
            .windowInsetsPadding(WindowInsets.safeDrawing),
        contentAlignment = Alignment.Center,
    ) {
        Column(
            Modifier
                .widthIn(max = 480.dp)
                .fillMaxSize()
                .padding(horizontal = 24.dp, vertical = 24.dp),
        ) {
            top?.invoke()
            Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.Center) {
                Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()), content = content)
            }
            Column(
                Modifier.fillMaxWidth().padding(top = 16.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
                content = buttons,
            )
        }
    }
}

/** The success page's "Join the plugin store" switch: its name, what the paired device does, and the switch. */
@Composable
private fun JoinStoreSwitch(title: String, line: String, checked: Boolean, onChange: (Boolean) -> Unit) {
    val onWord = stringResource(R.string.value_on)
    val offWord = stringResource(R.string.value_off)
    Row(
        Modifier
            .padding(top = 24.dp)
            .fillMaxWidth()
            .heightIn(min = 72.dp)
            .toggleable(checked, role = Role.Switch, onValueChange = onChange)
            .semantics { stateDescription = if (checked) onWord else offWord },
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.bodyLarge)
            Text(line, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        SettingSwitchMark(checked)
    }
}

@Composable
private fun PageTitle(text: String) {
    Text(text, style = MaterialTheme.typography.headlineSmall, modifier = Modifier.semantics { heading() })
}

@Composable
private fun PageLine(text: String, modifier: Modifier = Modifier, center: Boolean = false) {
    Text(
        text,
        style = MaterialTheme.typography.bodyLarge,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        textAlign = if (center) TextAlign.Center else TextAlign.Start,
        modifier = modifier.fillMaxWidth(),
    )
}

// The four pages, where this build can read

/** "2 of 4", over a page's title. */
@Composable
private fun StepLabel(step: Int) {
    Text(
        Strings.onboardingStep(LocalContext.current, step, 4),
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

/**
 * A step after Welcome, as Android draws it: the step and the title at the top, the content under
 * them scrolling when the window is short, and the foot's row with Back as a text button at the
 * start and the page's actions at the end, the filled one last. Skip, where a page has it, sits at
 * the top's end.
 */
@Composable
private fun StepFrame(
    step: Int,
    title: String,
    back: () -> Unit,
    skip: (() -> Unit)? = null,
    actions: @Composable RowScope.() -> Unit,
    content: @Composable ColumnScope.() -> Unit,
) {
    Box(
        Modifier
            .fillMaxSize()
            .windowInsetsPadding(WindowInsets.safeDrawing),
        contentAlignment = Alignment.TopCenter,
    ) {
        Column(
            Modifier
                .widthIn(max = 560.dp)
                .fillMaxSize()
                .padding(horizontal = 24.dp, vertical = 16.dp),
        ) {
            Row(Modifier.fillMaxWidth().heightIn(min = 48.dp), verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.weight(1f)) { StepLabel(step) }
                skip?.let {
                    TextButton(
                        onClick = it,
                        colors = ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.onSurface),
                    ) { Text(stringResource(R.string.onboarding_skip)) }
                }
            }
            Column(
                Modifier
                    .weight(1f)
                    .fillMaxWidth()
                    .verticalScroll(rememberScrollState()),
            ) {
                Text(
                    title,
                    style = MaterialTheme.typography.headlineMedium,
                    modifier = Modifier.padding(top = 8.dp).semantics { heading() },
                )
                content()
            }
            Row(
                Modifier.fillMaxWidth().padding(top = 16.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                TextButton(
                    onClick = back,
                    colors = ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.onSurface),
                ) { Text(stringResource(R.string.action_back)) }
                Spacer(Modifier.weight(1f))
                actions()
            }
        }
    }
}

/** Watch, read, or both? Nothing chosen at first, and Continue waits for a row. */
@Composable
private fun MediumPage(chosen: KaeriIsFor?, choose: (KaeriIsFor) -> Unit, back: () -> Unit, next: () -> Unit) {
    val hint = stringResource(R.string.onboarding_medium_hint)
    StepFrame(
        step = 2,
        title = stringResource(R.string.onboarding_medium_title),
        back = back,
        actions = {
            Button(
                onClick = next,
                enabled = chosen != null,
                modifier = Modifier.semantics { if (chosen == null) stateDescription = hint },
            ) { Text(stringResource(R.string.onboarding_continue)) }
        },
    ) {
        PageLine(stringResource(R.string.onboarding_medium_line), Modifier.padding(top = 8.dp))
        MediumChoiceRows(chosen, Modifier.padding(top = 16.dp), onChoose = choose)
    }
}

/**
 * Bring in your lists: AniList first, worded for what Kaeri is for, and the other trackers named
 * for later. Connect AniList is the one filled button; Not now goes on, Skip closes the pages.
 */
@Composable
private fun ListsPage(answer: KaeriIsFor, signIn: () -> Unit, back: () -> Unit, notNow: () -> Unit, skip: () -> Unit) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val working by store.accounts.working.collectAsStateWithLifecycle()
    val busy = store.accounts.isWorking(working, "anilist")
    val anilist = "AniList" // i18n-skip
    StepFrame(
        step = 3,
        title = stringResource(R.string.onboarding_lists_title),
        back = back,
        skip = skip,
        actions = {
            TextButton(
                onClick = notNow,
                colors = ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.onSurface),
            ) { Text(stringResource(R.string.onboarding_not_now)) }
            Button(onClick = signIn, enabled = !busy) { WorkLabel(Strings.onboardingConnect(context, anilist), busy) }
        },
    ) {
        PageLine(
            when (answer) {
                KaeriIsFor.WATCH -> Strings.onboardingListsLineWatch(context, anilist)
                KaeriIsFor.READ -> Strings.onboardingListsLineRead(context, anilist)
                KaeriIsFor.BOTH -> Strings.onboardingListsLineBoth(context, anilist)
            },
            Modifier.padding(top = 8.dp),
        )
        Column(
            Modifier
                .padding(top = 24.dp)
                .fillMaxWidth()
                .clip(RoundedCornerShape(16.dp))
                .background(MaterialTheme.colorScheme.surfaceContainer)
                .padding(horizontal = 16.dp, vertical = 12.dp),
        ) {
            Text(anilist, style = MaterialTheme.typography.titleMedium)
            Text(
                stringResource(R.string.onboarding_lists_anilist_line),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Text(
            stringResource(
                when (answer) {
                    KaeriIsFor.WATCH -> R.string.onboarding_lists_later_watch
                    KaeriIsFor.READ -> R.string.onboarding_lists_later_read
                    KaeriIsFor.BOTH -> R.string.onboarding_lists_later_both
                },
            ),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 8.dp),
        )
    }
}

/**
 * Where your books come from, after Read: a folder, a server and an invite, with no list of
 * sources and no suggestion. After Both, one page with a Shows part (a Jellyfin server, through
 * [showServer]) and a Books part. Done closes the pages.
 */
@Composable
private fun BooksPage(join: (String) -> Unit, back: () -> Unit, done: () -> Unit, showServer: (() -> Unit)? = null) {
    val context = LocalContext.current
    val both = showServer != null
    var serverSheet by rememberSaveable { mutableStateOf(false) }
    var folderFailed by rememberSaveable { mutableStateOf(false) }
    val pickFolder = rememberFolderPicker(onFailed = { folderFailed = true })
    StepFrame(
        step = 4,
        title = stringResource(if (both) R.string.onboarding_both_title else R.string.onboarding_books_title),
        back = back,
        actions = { Button(onClick = done) { Text(stringResource(R.string.action_done)) } },
    ) {
        PageLine(stringResource(if (both) R.string.onboarding_both_line else R.string.onboarding_books_line), Modifier.padding(top = 8.dp))
        val chevron: @Composable () -> Unit = { androidx.compose.material3.Icon(Icons.Chevron, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant) }
        val bookRows = listOf<@Composable (Place) -> Unit>(
            { place ->
                InfoRow(
                    place,
                    stringResource(R.string.onboarding_books_folder),
                    stringResource(R.string.onboarding_books_folder_line),
                    onClick = {
                        folderFailed = false
                        pickFolder()
                    },
                    trailing = chevron,
                )
            },
            { place ->
                InfoRow(place, stringResource(R.string.onboarding_books_server), stringResource(R.string.onboarding_books_server_line), onClick = { serverSheet = true }, trailing = chevron)
            },
        )
        if (showServer != null) {
            SettingsGroup(
                title = stringResource(R.string.onboarding_both_shows),
                modifier = Modifier.padding(top = 16.dp),
                rows = listOf({ place ->
                    InfoRow(place, stringResource(R.string.onboarding_books_server), stringResource(R.string.onboarding_both_shows_server_line), onClick = showServer, trailing = chevron)
                }),
            )
            SettingsGroup(title = stringResource(R.string.onboarding_both_books), modifier = Modifier.padding(top = 8.dp), rows = bookRows)
        } else {
            SettingsGroup(modifier = Modifier.padding(top = 24.dp), rows = bookRows)
        }
        if (folderFailed) SheetProblem(stringResource(R.string.book_folder_failed), Modifier.padding(top = 8.dp))
        InviteField(join)
    }
    if (serverSheet) AddBookServerSheet(onDismiss = { serverSheet = false })
}

/** The plugin store invite: its heading, the field with Join, and the line about plugins beside it. */
@Composable
private fun InviteField(join: (String) -> Unit) {
    var link by rememberSaveable { mutableStateOf("") }
    val ready = InviteLinks.looksLikeInvite(link)
    Text(
        stringResource(R.string.onboarding_books_invite_heading),
        style = MaterialTheme.typography.titleSmall,
        modifier = Modifier.padding(start = 16.dp, top = 24.dp, bottom = 8.dp).semantics { heading() },
    )
    OutlinedTextField(
        value = link,
        onValueChange = { link = it },
        placeholder = { Text(stringResource(R.string.onboarding_books_invite_placeholder)) },
        singleLine = true,
        trailingIcon = { TextButton(onClick = { join(link.trim()) }, enabled = ready) { Text(stringResource(R.string.onboarding_join)) } },
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, autoCorrectEnabled = false, imeAction = ImeAction.Go),
        keyboardActions = KeyboardActions(onGo = { if (ready) join(link.trim()) }),
        modifier = Modifier.fillMaxWidth(),
    )
    Text(
        stringResource(R.string.onboarding_books_plugin_line),
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 8.dp),
    )
}
