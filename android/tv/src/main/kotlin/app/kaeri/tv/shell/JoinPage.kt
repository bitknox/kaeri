// First run's page 2 on TV: "Already use Kaeri on another device?" Set up from your phone shows
// a code and its QR, beside the words; the waiting line after 600 ms, the Wi-Fi hint after 30 s
// and the run-out at 2 minutes follow the phone's page. Paired, the TV takes only what it can use
// for watching: the stores and the sign-ins, each On until pressed, and never a book server.

package app.kaeri.tv.shell

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.core.MutableTransitionState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.Canvas as DrawCanvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
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
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.Icon
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import app.kaeri.design.ObiSilk
import app.kaeri.design.SyncIcons
import app.kaeri.engine.model.FirstRunPage
import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.engine.model.MediumAsk
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.store.AccountsModel
import app.kaeri.store.JoinModel
import app.kaeri.store.JoinStage
import app.kaeri.store.JoinState
import app.kaeri.store.joinContinue
import app.kaeri.store.joinShowCode
import app.kaeri.store.joinStop
import app.kaeri.store.pagesLeft
import app.kaeri.store.text
import app.kaeri.tv.remote.QrCode
import app.kaeri.tv.sync.tvDeviceName
import app.kaeri.tv.ui.AmberButton
import app.kaeri.tv.ui.ButtonLabel
import app.kaeri.tv.ui.Canvas
import app.kaeri.tv.ui.LocalAppStore
import app.kaeri.tv.ui.LocalReducedMotion
import app.kaeri.tv.ui.TonalButton
import app.kaeri.tv.ui.TvListRow
import app.kaeri.tv.ui.entryFocus
import kotlinx.coroutines.delay

/** The step this page is: the TV's first run is Welcome, this page, AniList and Sources. */
private const val STEP = 2
private const val STEPS = 4

/**
 * The join page. [back] returns to Welcome, [startHere] goes on to AniList; [next] hears the first
 * page still to ask once the join is through, or null when first run ends, with the device's name.
 */
@Composable
internal fun JoinPage(back: () -> Unit, startHere: () -> Unit, next: (FirstRunPage?, String?) -> Unit) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val model = store.sync.join
    val state by model.state.collectAsStateWithLifecycle()
    val now = rememberJoinNow(model, state)
    BackHandler {
        when (state.stage) {
            JoinStage.ASK -> back()
            JoinStage.CODE -> store.joinStop()
            JoinStage.PAIRED -> {
                store.joinStop()
                back()
            }
        }
    }
    val reduced = LocalReducedMotion.current
    AnimatedContent(
        state.stage,
        transitionSpec = { if (reduced) fadeIn(tween(200)) togetherWith fadeOut(tween(200)) else fadeIn(app.kaeri.design.Motion.reveal()) togetherWith fadeOut(app.kaeri.design.Motion.reveal()) },
        label = "join", // i18n-skip
    ) { stage ->
        when (stage) {
            JoinStage.ASK -> Ask(startHere = startHere, showCode = { store.joinShowCode(tvDeviceName(context)) })
            JoinStage.CODE -> Code(state, now, back = { store.joinStop() }, again = { store.joinShowCode(tvDeviceName(context)) })
            JoinStage.PAIRED -> Paired(state, now, next)
        }
    }
}

@Composable
private fun rememberJoinNow(model: JoinModel, state: JoinState): Long {
    var now by remember { mutableLongStateOf(model.now()) }
    LaunchedEffect(state) {
        now = model.now()
        while (true) {
            val next = state.nextChange(now) ?: break
            delay((next - now).coerceAtLeast(16))
            val then = model.now()
            if (then == now) break
            now = then
        }
    }
    return now
}

/** Two columns: the step, the title and the line at the start; what the page holds beside them. */
@Composable
private fun Frame(title: String, line: String?, focus: FocusRequester, content: @Composable () -> Unit) {
    Row(
        Modifier
            .entryFocus(focus)
            .fillMaxSize()
            .background(ObiSilk.Ground)
            .padding(start = 58.dp, end = Canvas.SafeX, top = 63.dp),
        horizontalArrangement = Arrangement.spacedBy(48.dp),
    ) {
        Column(Modifier.weight(0.445f), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Text(Strings.onboardingStep(LocalContext.current, STEP, STEPS), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
            Text(title, style = MaterialTheme.typography.headlineLarge, color = ObiSilk.Silk, modifier = Modifier.semantics { heading() })
            line?.let { Text(it, style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze, modifier = Modifier.padding(top = 4.dp)) }
        }
        Column(Modifier.weight(0.555f), verticalArrangement = Arrangement.spacedBy(20.dp)) { content() }
    }
}

/** Set up from your phone, as a row under focus, and No, start here, the one amber button. */
@Composable
private fun Ask(startHere: () -> Unit, showCode: () -> Unit) {
    val focus = remember { FocusRequester() }
    Frame(stringResource(R.string.first_other_title), stringResource(R.string.first_other_line), focus) {
        TvListRow(
            stringResource(R.string.first_setup_phone),
            onClick = showCode,
            icon = SyncIcons.Phone,
            height = 66.dp,
            modifier = Modifier.focusRequester(focus),
        )
        AmberButton(onClick = startHere, icon = null) { ButtonLabel(stringResource(R.string.first_other_no)) }
    }
}

/** The code in boxes, how long it is good for, the QR, the line under them, and Back. */
@Composable
private fun Code(state: JoinState, now: Long, back: () -> Unit, again: () -> Unit) {
    val context = LocalContext.current
    val focus = remember { FocusRequester() }
    val expired = state.expired(now)
    Frame(stringResource(R.string.pair_show_title), stringResource(R.string.pair_tv_line), focus) {
        val dim by animateFloatAsState(if (expired) 0.38f else 1f, app.kaeri.design.Motion.reveal(), label = "code") // i18n-skip
        state.code?.let { code ->
            Column(Modifier.alpha(dim), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                CodeBoxes(code)
                val seconds = state.secondsLeft(now)
                Text(Strings.pairGoodFor(context, "%d:%02d".format(seconds / 60, seconds % 60)), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze) // i18n-skip
                QrCode("kaeri://pair?port=${state.port}&code=$code", size = 132.dp, label = stringResource(R.string.pair_show_scan))
            }
        }
        Column(Modifier.semantics { liveRegion = LiveRegionMode.Polite }, verticalArrangement = Arrangement.spacedBy(12.dp)) {
            AnimatedContent(
                when {
                    expired -> 2
                    state.waiting(now) -> 1
                    else -> 0
                },
                transitionSpec = { fadeIn(app.kaeri.design.Motion.fadeIn()) togetherWith fadeOut(app.kaeri.design.Motion.fadeOut()) },
                label = "code line", // i18n-skip
            ) { line ->
                when (line) {
                    1 -> StatusLine(stringResource(R.string.pair_waiting)) { Ring() }
                    2 -> StatusLine(stringResource(R.string.pair_err_expired)) { WarnGlyph() }
                    else -> Spacer(Modifier.width(1.dp))
                }
            }
            AnimatedVisibility(state.slow(now) && !expired, enter = fadeIn(app.kaeri.design.Motion.fadeIn()), exit = fadeOut(app.kaeri.design.Motion.fadeOut())) {
                StatusLine(stringResource(R.string.pair_nothing_yet)) { InfoGlyph() }
            }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            if (expired) AmberButton(onClick = again, icon = null, modifier = Modifier.focusRequester(focus)) { ButtonLabel(stringResource(R.string.pair_show_code)) }
            TonalButton(onClick = back, modifier = if (expired) Modifier else Modifier.focusRequester(focus)) { ButtonLabel(stringResource(R.string.action_back)) }
        }
    }
}

@Composable
private fun CodeBoxes(code: String) {
    val spoken = code.chunked(1).joinToString(" ") // i18n-skip
    Row(Modifier.semantics(mergeDescendants = true) { contentDescription = spoken }) {
        code.forEachIndexed { index, digit ->
            if (index == 3) Spacer(Modifier.width(30.dp))
            if (index % 3 != 0) Spacer(Modifier.width(10.dp))
            Box(
                Modifier
                    .size(width = 52.dp, height = 66.dp)
                    .background(ObiSilk.Raised, RoundedCornerShape(12.dp))
                    .border(1.dp, ObiSilk.FaintRule, RoundedCornerShape(12.dp)),
                contentAlignment = Alignment.Center,
            ) { Text(digit.toString(), style = MaterialTheme.typography.headlineLarge.copy(fontWeight = FontWeight.SemiBold, fontSize = 32.sp), color = ObiSilk.Silk) }
        }
    }
}

@Composable
private fun StatusLine(text: String, mark: @Composable () -> Unit) {
    Row(horizontalArrangement = Arrangement.spacedBy(16.dp), verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(20.dp), contentAlignment = Alignment.Center) { mark() }
        Text(text, style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.SemiBold), color = ObiSilk.Silk)
    }
}

@Composable
private fun Ring() {
    androidx.compose.material3.CircularProgressIndicator(
        modifier = Modifier.size(18.dp),
        color = ObiSilk.Haze,
        trackColor = ObiSilk.Rule,
        strokeWidth = 2.dp,
    )
}

@Composable
private fun InfoGlyph() {
    DrawCanvas(Modifier.size(20.dp)) {
        val w = 1.6.dp.toPx()
        drawCircle(ObiSilk.Haze, radius = size.minDimension / 2 - w, style = Stroke(w))
        drawLine(ObiSilk.Haze, Offset(center.x, center.y - 1.dp.toPx()), Offset(center.x, center.y + 4.5.dp.toPx()), w, StrokeCap.Round)
        drawCircle(ObiSilk.Haze, radius = w * 0.75f, center = Offset(center.x, center.y - 4.5.dp.toPx()))
    }
}

/**
 * Paired: what came over, each item On until pressed. The medium is said, not chosen; a store that
 * said no shows why in its row. Continue is the one amber button.
 */
@Composable
private fun Paired(state: JoinState, now: Long, next: (FirstRunPage?, String?) -> Unit) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val focus = remember { FocusRequester() }
    val device = state.deviceName.orEmpty()
    val setup = state.setup
    var signIns by rememberSaveable { mutableStateOf(true) }
    var storesOff by rememberSaveable { mutableStateOf(setOf<String>()) }
    val trackers = (setup?.signedIn ?: state.signIns).orEmpty()
    val sources = setup?.stores.orEmpty().any { it.name !in storesOff && state.refusal(it.name) == null }
    val left = setup.pagesLeft(signIns = signIns && trackers.isNotEmpty(), sources = sources)
    val goTo = when {
        setup == null -> FirstRunPage.LISTS
        FirstRunPage.LISTS in left -> FirstRunPage.LISTS
        FirstRunPage.SOURCES in left -> FirstRunPage.SOURCES
        else -> null
    }
    LaunchedEffect(state.leaving(now)) { if (state.leaving(now)) next(goTo, device) }
    val on = stringResource(R.string.value_on)
    val off = stringResource(R.string.value_off)
    val amber by animateFloatAsState(if (state.landed(now)) 1f else 0.38f, tween(200), label = "continue") // i18n-skip
    Frame(Strings.pairOk(context, device), stringResource(R.string.first_ok_line), focus) {
        Column(Modifier.animateContentSize(app.kaeri.design.Motion.standard()), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(Strings.firstJoinGroup(context, device), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
            setup?.medium?.let { medium ->
                Arriving {
                    InfoCard(
                        stringResource(R.string.first_join_medium),
                        when (medium) {
                            KaeriIsFor.WATCH -> Strings.joinMediumLineWatch(context, device)
                            KaeriIsFor.READ -> Strings.firstJoinMediumLine(context, device)
                            KaeriIsFor.BOTH -> Strings.joinMediumLineBoth(context, device)
                        },
                    )
                }
            }
            if (trackers.isNotEmpty()) Arriving {
                TvListRow(
                    Strings.pairCopySignins(context, device),
                    onClick = { signIns = !signIns },
                    supporting = Strings.firstJoinSigninsLine(context, android.icu.text.ListFormatter.getInstance().format(trackers.map { AccountsModel.name(it) })),
                    value = if (signIns) on else off,
                    height = 64.dp,
                    modifier = Modifier.background(ObiSilk.Raised, RoundedCornerShape(12.dp)),
                )
            }
            setup?.stores.orEmpty().forEach { offer ->
                val refused = state.refusal(offer.name)
                Arriving {
                    if (refused != null) {
                        InfoCard(Strings.firstJoinStore(context, offer.name), refused.text(context, device), problem = true)
                    } else {
                        val joined = offer.name !in storesOff
                        TvListRow(
                            Strings.firstJoinStore(context, offer.name),
                            onClick = { storesOff = if (joined) storesOff + offer.name else storesOff - offer.name },
                            supporting = Strings.firstJoinStoreLine(context, device, stringResource(R.string.device_kind_tv)),
                            supportingLines = 2,
                            value = if (joined) on else off,
                            height = 64.dp,
                            modifier = Modifier.background(ObiSilk.Raised, RoundedCornerShape(12.dp)),
                        )
                    }
                }
            }
            if (state.looking(now)) Arriving {
                Row(
                    Modifier.fillMaxWidth().heightIn(min = 64.dp).background(ObiSilk.Raised, RoundedCornerShape(12.dp)).padding(horizontal = 16.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(Strings.firstJoinLooking(context, device), style = MaterialTheme.typography.titleMedium, color = ObiSilk.Silk, modifier = Modifier.weight(1f))
                    Ring()
                }
            }
            if (setup != null && goTo == null) {
                Text(stringResource(R.string.first_join_steps_done), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, modifier = Modifier.padding(top = 4.dp))
            }
        }
        AmberButton(
            onClick = {
                if (!state.landed(now) || state.working(now)) return@AmberButton
                val medium = setup?.medium
                val view = store.medium.view.value
                if (state.presses == 0 && medium != null && view != null && view.readingAvailable && !view.fixed) {
                    val ask = store.medium.asking.value
                    if (ask is MediumAsk.FirstRun) store.medium.answer(ask, medium) else store.medium.choose(medium)
                }
                store.joinContinue(signIns = signIns, stores = setup?.stores.orEmpty().filter { it.name !in storesOff }, servers = emptyList())
            },
            icon = null,
            modifier = Modifier.focusRequester(focus).alpha(amber),
        ) { ButtonLabel(stringResource(R.string.onboarding_continue)) }
    }
}

/** A card that says something and is not pressed: the medium, or a store that said no. */
@Composable
private fun InfoCard(title: String, line: String, problem: Boolean = false) {
    Row(
        Modifier.fillMaxWidth().heightIn(min = 64.dp).background(ObiSilk.Raised, RoundedCornerShape(12.dp)).padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.titleMedium, color = ObiSilk.Silk)
            Text(line, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
        }
        if (problem) Box(Modifier.padding(start = 16.dp)) { WarnGlyph() }
    }
}

@Composable
private fun Arriving(content: @Composable () -> Unit) {
    val shown = remember { MutableTransitionState(false).apply { targetState = true } }
    AnimatedVisibility(shown, enter = expandVertically(app.kaeri.design.Motion.standard()) + fadeIn(app.kaeri.design.Motion.reveal())) { content() }
}

/** The error glyph: a vermilion triangle with its mark cut in the ground's colour. */
@Composable
private fun WarnGlyph() {
    DrawCanvas(Modifier.size(20.dp)) {
        val u = size.minDimension / 24f
        val triangle = androidx.compose.ui.graphics.Path().apply {
            moveTo(12f * u, 3.2f * u)
            lineTo(1.8f * u, 20.5f * u)
            lineTo(22.2f * u, 20.5f * u)
            close()
        }
        drawPath(triangle, ObiSilk.Vermilion)
        drawPath(triangle, ObiSilk.Vermilion, style = Stroke(2f * u, join = androidx.compose.ui.graphics.StrokeJoin.Round))
        drawLine(ObiSilk.Ground, Offset(12f * u, 9.5f * u), Offset(12f * u, 14.5f * u), 2f * u, StrokeCap.Round)
        drawCircle(ObiSilk.Ground, 1.2f * u, Offset(12f * u, 17.4f * u))
    }
}
