// First run's page 2: "Already use Kaeri on another device?" Two ways in as rows and No, start
// here as the one amber button; the code this device shows, with the waiting line after 600 ms,
// the Wi-Fi hint after 30 s and the run-out at 2 minutes; then, crossfading in place, what the
// paired device brings, each item growing in as its data lands, and Continue turning amber once
// everything has. Pages the other device answered are skipped.

package app.kaeri.ui

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.ContentTransform
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.core.MutableTransitionState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.design.Motion
import app.kaeri.design.SyncIcons
import app.kaeri.design.rememberAnimationsRemoved
import app.kaeri.engine.model.FirstRunPage
import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.engine.model.MediumAsk
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.store.AccountsModel
import app.kaeri.store.JoinModel
import app.kaeri.store.JoinStage
import app.kaeri.store.JoinState
import app.kaeri.store.SyncSheet
import app.kaeri.store.joinContinue
import app.kaeri.store.joinShowCode
import app.kaeri.store.joinStop
import app.kaeri.store.pagesLeft
import app.kaeri.store.text
import app.kaeri.ui.reading.serverKindWords
import kotlinx.coroutines.delay

/**
 * The join page. [step] is its place for the step label, or null where first run has no steps;
 * [back] returns to Welcome, [startHere] goes on to the next page. [next] hears where first run
 * goes once the join is through: the first page still to ask, or null when first run ends, with
 * the paired device's name for the notice. [restored] closes first run after a cloud folder restore.
 */
@Composable
internal fun JoinPage(
    step: Int?,
    choosable: Boolean,
    back: () -> Unit,
    startHere: () -> Unit,
    next: (FirstRunPage?, KaeriIsFor?, String?) -> Unit,
    restored: () -> Unit,
) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val model = store.sync.join
    val state by model.state.collectAsStateWithLifecycle()
    val now = rememberJoinNow(model, state)
    val removed = rememberAnimationsRemoved()
    // A restore through a cloud folder runs in the Devices sheets, over this page.
    val finished by store.sync.finished.collectAsStateWithLifecycle()
    val before = remember { finished }
    LaunchedEffect(finished) { if (finished != before && store.sync.recoveryKey.value == null) restored() }
    val restore = { store.sync.show(SyncSheet.PICKER) }
    val axis = with(LocalDensity.current) { 30.dp.roundToPx() }
    AnimatedContent(
        state.stage,
        transitionSpec = {
            when {
                removed -> fadeIn(tween(200)) togetherWith fadeOut(tween(200))
                // Paired: the code page's place takes what came over, crossfading in place.
                targetState == JoinStage.PAIRED || initialState == JoinStage.PAIRED -> fadeIn(Motion.reveal()) togetherWith fadeOut(Motion.reveal())
                else -> sharedAxisX(forward = targetState > initialState, distance = axis)
            }
        },
        label = "join", // i18n-skip
    ) { stage ->
        when (stage) {
            JoinStage.ASK -> JoinAsk(step, back = back, startHere = startHere, showCode = { store.joinShowCode(systemDeviceName(context)) }, restore = restore)
            JoinStage.CODE -> JoinCode(step, state, now, back = { store.joinStop() }, restore = restore, again = { store.joinShowCode(systemDeviceName(context)) })
            JoinStage.PAIRED -> JoinPaired(step, choosable, state, now, next)
        }
    }
    DevicesSheets(paired = false)
}

/** The clock the page reads its state against, ticking when the state says it changes next. */
@Composable
private fun rememberJoinNow(model: JoinModel, state: JoinState): Long {
    var now by remember { mutableLongStateOf(model.now()) }
    LaunchedEffect(state) {
        now = model.now()
        while (true) {
            val next = state.nextChange(now) ?: break
            delay((next - now).coerceAtLeast(16))
            val then = model.now()
            // A staged state holds the clock still: nothing more will change.
            if (then == now) break
            now = then
        }
    }
    return now
}

/** M3's emphasized easing, which shared axis moves on. */
internal val EMPHASIZED = CubicBezierEasing(0.2f, 0f, 0f, 1f)

/** M3 shared axis X: the page leaves [distance] px along and fades in its first 90 ms, the next comes in after it. */
internal fun sharedAxisX(forward: Boolean, distance: Int): ContentTransform {
    val sign = if (forward) 1 else -1
    return (slideInHorizontally(tween(300, easing = EMPHASIZED)) { sign * distance } + fadeIn(tween(210, delayMillis = 90))) togetherWith
        (slideOutHorizontally(tween(300, easing = EMPHASIZED)) { -sign * distance } + fadeOut(tween(90)))
}

/** The join page's titles break short, as the board sets them. */
private val TITLE_MAX = 260.dp

/** The question and its two ways in, as rows; No, start here is the amber button. */
@Composable
private fun JoinAsk(step: Int?, back: () -> Unit, startHere: () -> Unit, showCode: () -> Unit, restore: () -> Unit) {
    val chevron: @Composable () -> Unit = { Icon(Icons.Chevron, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant) }
    StepFrame(
        step = step,
        title = stringResource(R.string.first_other_title),
        back = back,
        titleMax = TITLE_MAX,
        actions = { Button(onClick = startHere) { Text(stringResource(R.string.first_other_no)) } },
    ) {
        PageLine(stringResource(R.string.first_other_line), Modifier.padding(top = 12.dp))
        SettingsGroup(
            modifier = Modifier.padding(top = 24.dp),
            rows = listOf(
                { place ->
                    InfoRow(
                        place,
                        stringResource(R.string.first_setup_other),
                        stringResource(R.string.first_other_pair_line),
                        onClick = showCode,
                        leading = { IconTile(SyncIcons.Laptop) },
                        trailing = chevron,
                        titleStyle = MaterialTheme.typography.bodyLarge,
                    )
                },
                { place ->
                    InfoRow(
                        place,
                        stringResource(R.string.first_other_restore),
                        stringResource(R.string.first_other_restore_line),
                        onClick = restore,
                        leading = { IconTile(SyncIcons.Cloud) },
                        trailing = chevron,
                        titleStyle = MaterialTheme.typography.bodyLarge,
                    )
                },
            ),
        )
    }
}

/** A row's icon on its 36 tile, as the Devices page's ways in draw it. */
@Composable
private fun IconTile(icon: ImageVector) {
    Box(
        Modifier.size(36.dp).clip(RoundedCornerShape(9.dp)).background(MaterialTheme.colorScheme.secondaryContainer),
        contentAlignment = Alignment.Center,
    ) { Icon(icon, contentDescription = null, modifier = Modifier.size(18.dp), tint = MaterialTheme.colorScheme.onSurface) }
}

/**
 * The code, in two groups of three boxes, how long it is good for, and the line under it: nothing
 * for 600 ms, then the waiting line, the hint after 30 s, and at 2 minutes the run-out, the digits
 * at 38% and Show a code.
 */
@Composable
private fun JoinCode(step: Int?, state: JoinState, now: Long, back: () -> Unit, restore: () -> Unit, again: () -> Unit) {
    val context = LocalContext.current
    val expired = state.expired(now)
    StepFrame(
        step = step,
        title = stringResource(R.string.pair_show_title),
        back = back,
        titleMax = TITLE_MAX,
        actions = {
            TextButton(onClick = restore, colors = ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.onSurface)) {
                Text(stringResource(R.string.first_restore))
            }
            if (expired) Button(onClick = again) { Text(stringResource(R.string.pair_show_code)) }
        },
    ) {
        PageLine(stringResource(R.string.first_setup_line), Modifier.padding(top = 12.dp))
        val dim by animateFloatAsState(if (expired) 0.38f else 1f, Motion.reveal(), label = "code") // i18n-skip
        state.code?.let { code ->
            CodeBoxes(code, Modifier.padding(top = 24.dp).fillMaxWidth().alpha(dim))
            val seconds = state.secondsLeft(now)
            Text(
                Strings.pairGoodFor(context, "%d:%02d".format(seconds / 60, seconds % 60)), // i18n-skip
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = TextAlign.Center,
                modifier = Modifier.padding(top = 16.dp).fillMaxWidth().alpha(dim),
            )
        }
        Column(Modifier.padding(top = 24.dp).semantics { liveRegion = LiveRegionMode.Polite }, verticalArrangement = Arrangement.spacedBy(16.dp)) {
            // The run-out crossfades in over the waiting line, in its place.
            AnimatedContent(
                when {
                    expired -> 2
                    state.waiting(now) -> 1
                    else -> 0
                },
                transitionSpec = { fadeIn(Motion.fadeIn()) togetherWith fadeOut(Motion.fadeOut()) },
                label = "code line", // i18n-skip
            ) { line ->
                when (line) {
                    1 -> StatusLine(stringResource(R.string.pair_waiting)) { Ring() }
                    2 -> StatusLine(stringResource(R.string.pair_err_expired)) { WarnGlyph() }
                    else -> Spacer(Modifier.fillMaxWidth())
                }
            }
            AnimatedVisibility(state.slow(now) && !expired, enter = fadeIn(Motion.fadeIn()), exit = fadeOut(Motion.fadeOut())) {
                StatusLine(stringResource(R.string.pair_nothing_yet)) { InfoGlyph() }
            }
        }
    }
}

/** Six digits in two groups of three, each in its own box. */
@Composable
private fun CodeBoxes(code: String, modifier: Modifier) {
    val colors = MaterialTheme.colorScheme
    val spoken = code.chunked(1).joinToString(" ") // i18n-skip
    Row(modifier.semantics(mergeDescendants = true) { contentDescription = spoken }, horizontalArrangement = Arrangement.Center) {
        code.forEachIndexed { index, digit ->
            if (index == 3) Spacer(Modifier.width(22.dp))
            if (index % 3 != 0) Spacer(Modifier.width(10.dp))
            Box(
                Modifier
                    .size(width = 48.dp, height = 60.dp)
                    .clip(RoundedCornerShape(12.dp))
                    .background(colors.surfaceContainer)
                    .border(1.dp, colors.outlineVariant, RoundedCornerShape(12.dp)),
                contentAlignment = Alignment.Center,
            ) {
                Text(digit.toString(), style = MaterialTheme.typography.headlineLarge.copy(fontWeight = FontWeight.SemiBold, fontSize = 32.sp))
            }
        }
    }
}

/** A line about the wait: its mark in a 20 slot, and the words. */
@Composable
private fun StatusLine(text: String, mark: @Composable () -> Unit) {
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        Box(Modifier.padding(top = 2.dp).size(20.dp), contentAlignment = Alignment.Center) { mark() }
        Text(text, style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.SemiBold), modifier = Modifier.weight(1f))
    }
}

@Composable
private fun Ring(size: Int = 20) {
    CircularProgressIndicator(
        modifier = Modifier.size(size.dp),
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        trackColor = MaterialTheme.colorScheme.outlineVariant,
        strokeWidth = 2.dp,
    )
}

/** The haze info glyph: a ring with its i. */
@Composable
private fun InfoGlyph() {
    val haze = MaterialTheme.colorScheme.onSurfaceVariant
    Canvas(Modifier.size(20.dp)) {
        val w = 1.6.dp.toPx()
        drawCircle(haze, radius = size.minDimension / 2 - w, style = Stroke(w))
        drawLine(haze, Offset(center.x, center.y - 1.dp.toPx()), Offset(center.x, center.y + 4.5.dp.toPx()), w, StrokeCap.Round)
        drawCircle(haze, radius = w * 0.75f, center = Offset(center.x, center.y - 4.5.dp.toPx()))
    }
}

/**
 * Paired: what the other device brings, under "From …". The medium as a row with a pop-up, the
 * sign-ins, each store and each book server with a switch, on; a store that said no shows why in
 * its row. Continue waits for the setup, then turns amber.
 */
@Composable
private fun JoinPaired(step: Int?, choosable: Boolean, state: JoinState, now: Long, next: (FirstRunPage?, KaeriIsFor?, String?) -> Unit) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val device = state.deviceName.orEmpty()
    val setup = state.setup
    var signIns by rememberSaveable { mutableStateOf(true) }
    var storesOff by rememberSaveable { mutableStateOf(setOf<String>()) }
    var serversOff by rememberSaveable { mutableStateOf(setOf<String>()) }
    var picked by rememberSaveable { mutableStateOf<KaeriIsFor?>(null) }
    val medium = (picked ?: setup?.medium)?.takeIf { choosable }
    val trackers = (setup?.signedIn ?: state.signIns).orEmpty()
    val servers = setup?.servers.orEmpty().filter { !it.added }
    val sources = setup?.stores.orEmpty().any { it.name !in storesOff && state.refusal(it.name) == null } || servers.any { it.id !in serversOff }
    val left = setup.pagesLeft(signIns = signIns && trackers.isNotEmpty(), sources = sources)
    val goTo = when {
        setup == null -> if (choosable) FirstRunPage.MEDIUM else FirstRunPage.LISTS
        choosable && FirstRunPage.MEDIUM in left -> FirstRunPage.MEDIUM
        FirstRunPage.LISTS in left -> FirstRunPage.LISTS
        FirstRunPage.SOURCES in left -> FirstRunPage.SOURCES
        else -> null
    }
    val ready = state.landed(now) && !state.working(now)
    LaunchedEffect(state.leaving(now)) { if (state.leaving(now)) next(goTo, medium, device) }
    val amber by animateFloatAsState(if (state.landed(now)) 1f else 0.38f, tween(200), label = "continue") // i18n-skip
    StepFrame(
        step = step,
        // The device's name keeps to one line.
        title = Strings.pairOk(context, device.replace(' ', '\u00A0')),
        back = null,
        titleMax = TITLE_MAX,
        actions = {
            Button(
                onClick = {
                    if (!ready) return@Button
                    if (state.presses == 0 && medium != null) {
                        val ask = store.medium.asking.value
                        if (ask is MediumAsk.FirstRun) store.medium.answer(ask, medium) else store.medium.choose(medium)
                    }
                    store.joinContinue(
                        signIns = signIns,
                        stores = setup?.stores.orEmpty().filter { it.name !in storesOff },
                        servers = servers.filter { it.id !in serversOff }.map { it.id },
                    )
                },
                colors = ButtonDefaults.buttonColors(
                    disabledContainerColor = MaterialTheme.colorScheme.primary,
                    disabledContentColor = MaterialTheme.colorScheme.onPrimary,
                ),
                modifier = Modifier.alpha(amber),
            ) { WorkLabel(stringResource(R.string.onboarding_continue), state.working(now)) }
        },
    ) {
        PageLine(stringResource(R.string.first_ok_line), Modifier.padding(top = 12.dp))
        val rows = buildList<@Composable (Place) -> Unit> {
            if (medium != null) add { place -> MediumPopRow(place, device, medium) { picked = it } }
            if (trackers.isNotEmpty()) add { place ->
                SwitchRow(
                    place,
                    Strings.pairCopySignins(context, device),
                    signIns,
                    note = Strings.firstJoinSigninsLine(context, joinServices(trackers.map { AccountsModel.name(it) })),
                ) { signIns = it }
            }
            setup?.stores.orEmpty().forEach { offer ->
                val refused = state.refusal(offer.name)
                add { place ->
                    if (refused != null) {
                        InfoRow(
                            place,
                            Strings.firstJoinStore(context, offer.name),
                            refused.text(context, device),
                            trailing = { WarnGlyph() },
                            titleStyle = MaterialTheme.typography.bodyLarge,
                        )
                    } else {
                        SwitchRow(place, Strings.firstJoinStore(context, offer.name), offer.name !in storesOff, note = Strings.firstJoinStoreLine(context, device, kindWord())) { on ->
                            storesOff = if (on) storesOff - offer.name else storesOff + offer.name
                        }
                    }
                }
            }
            servers.forEach { server ->
                add { place ->
                    SwitchRow(
                        place,
                        Strings.firstJoinServer(context, server.host),
                        server.id !in serversOff,
                        note = Strings.firstJoinServerLine(context, serverKindWords(server.kind), device, stringResource(R.string.join_key_store)),
                    ) { on -> serversOff = if (on) serversOff - server.id else serversOff + server.id }
                }
            }
            if (state.looking(now)) add { place -> LookingRow(place, device) }
        }
        Column(Modifier.padding(top = 16.dp).animateContentSize(Motion.standard())) {
            SettingsGroup(title = Strings.firstJoinGroup(context, device), rows = rows.map { row -> @Composable { place: Place -> Arriving { row(place) } } })
            if (setup != null && setup.folders > 0u) Note(Strings.firstJoinFolders(context, device))
            if (setup != null && goTo == null) Note(stringResource(R.string.first_join_steps_done))
        }
    }
}

/** An item growing in as its data lands: M3 standard height with a fade. */
@Composable
private fun Arriving(content: @Composable () -> Unit) {
    val shown = remember { MutableTransitionState(false).apply { targetState = true } }
    AnimatedVisibility(shown, enter = expandVertically(Motion.standard()) + fadeIn(Motion.reveal())) { content() }
}

@Composable
private fun Note(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(top = 12.dp).fillMaxWidth(),
    )
}

/** "Looking at what … has", with its ring, while the setup is on its way. */
@Composable
private fun LookingRow(place: Place, device: String) {
    RowFrame(place, onClick = null, modifier = Modifier.heightIn(min = 72.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            Text(Strings.firstJoinLooking(LocalContext.current, device), style = MaterialTheme.typography.bodyLarge, modifier = Modifier.weight(1f))
            Ring()
        }
    }
}

/** "Kaeri is for", the other device's answer under it, and the answer to change in a pop-up at the end. */
@Composable
private fun MediumPopRow(place: Place, device: String, medium: KaeriIsFor, pick: (KaeriIsFor) -> Unit) {
    val context = LocalContext.current
    var open by remember { mutableStateOf(false) }
    val words = mapOf(
        KaeriIsFor.WATCH to stringResource(R.string.medium_answer_watch),
        KaeriIsFor.READ to stringResource(R.string.medium_answer_read),
        KaeriIsFor.BOTH to stringResource(R.string.medium_answer_both),
    )
    val line = when (medium) {
        KaeriIsFor.WATCH -> Strings.joinMediumLineWatch(context, device)
        KaeriIsFor.READ -> Strings.firstJoinMediumLine(context, device)
        KaeriIsFor.BOTH -> Strings.joinMediumLineBoth(context, device)
    }
    val selectedWord = stringResource(R.string.value_selected)
    Box {
        RowFrame(place, onClick = { open = true }, modifier = Modifier.heightIn(min = 72.dp)) { selected ->
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                RowText(stringResource(R.string.first_join_medium), line, null, selected, Modifier.weight(1f), MaterialTheme.typography.bodyLarge)
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(words.getValue(medium), style = MaterialTheme.typography.bodyLarge)
                    UpDown()
                }
            }
        }
        DropdownMenu(
            expanded = open,
            onDismissRequest = { open = false },
            shape = MaterialTheme.shapes.extraSmall,
            containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
        ) {
            words.forEach { (answer, label) ->
                DropdownMenuItem(
                    text = { Text(label, style = MaterialTheme.typography.labelLarge) },
                    leadingIcon = {
                        Box(Modifier.size(24.dp), contentAlignment = Alignment.Center) {
                            if (answer == medium) Icon(Icons.Check, contentDescription = null, tint = MaterialTheme.colorScheme.onSurface)
                        }
                    },
                    modifier = Modifier.heightIn(min = 48.dp).semantics { if (answer == medium) contentDescription = "$label, $selectedWord" }, // i18n-skip
                    onClick = {
                        open = false
                        pick(answer)
                    },
                )
            }
        }
    }
}

/** The pop-up's mark: a small chevron up over one down. */
@Composable
private fun UpDown() {
    val haze = MaterialTheme.colorScheme.onSurface
    Canvas(Modifier.size(width = 12.dp, height = 16.dp)) {
        val w = 1.5.dp.toPx()
        val x = size.width / 2
        val d = 3.5.dp.toPx()
        drawLine(haze, Offset(x - d, 6.dp.toPx()), Offset(x, 2.5.dp.toPx()), w, StrokeCap.Round)
        drawLine(haze, Offset(x, 2.5.dp.toPx()), Offset(x + d, 6.dp.toPx()), w, StrokeCap.Round)
        drawLine(haze, Offset(x - d, 10.dp.toPx()), Offset(x, 13.5.dp.toPx()), w, StrokeCap.Round)
        drawLine(haze, Offset(x, 13.5.dp.toPx()), Offset(x + d, 10.dp.toPx()), w, StrokeCap.Round)
    }
}

/** The error glyph: a vermilion triangle with its mark cut in the ground's colour. */
@Composable
internal fun WarnGlyph(colour: androidx.compose.ui.graphics.Color = MaterialTheme.colorScheme.error, ground: androidx.compose.ui.graphics.Color = MaterialTheme.colorScheme.surface) {
    androidx.compose.foundation.Canvas(Modifier.size(20.dp)) {
        val u = size.minDimension / 24f
        val triangle = androidx.compose.ui.graphics.Path().apply {
            moveTo(12f * u, 3.2f * u)
            lineTo(1.8f * u, 20.5f * u)
            lineTo(22.2f * u, 20.5f * u)
            close()
        }
        drawPath(triangle, colour)
        drawPath(triangle, colour, style = androidx.compose.ui.graphics.drawscope.Stroke(2f * u, join = androidx.compose.ui.graphics.StrokeJoin.Round))
        drawLine(ground, androidx.compose.ui.geometry.Offset(12f * u, 9.5f * u), androidx.compose.ui.geometry.Offset(12f * u, 14.5f * u), 2f * u, androidx.compose.ui.graphics.StrokeCap.Round)
        drawCircle(ground, 1.2f * u, androidx.compose.ui.geometry.Offset(12f * u, 17.4f * u))
    }
}
